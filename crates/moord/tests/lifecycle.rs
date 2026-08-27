//! Lifecycle (plan 2.7): the real `moord` binary is `SIGKILL`ed while a client
//! hammers it with mutations; the data dir reopens consistent and a fresh
//! daemon takes over the stale socket path.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use moor_protocol::{
    Anchor, Author, BuildInfo, ClientId, ClientSeq, CommentId, CommentKind, EventBody, Mutation,
    NonEmpty, RefSpec, RepoId, Request, Response, ReviewId, ReviewTarget, WorkspaceId,
};
use moor_review_core::store::Store;
use moor_review_core::{Core, DataDir};
use moor_test_support::{RepoBuilder, TestRepo, files};
use moord::client::{Client, ClientError, Identity};

struct Spawned {
    child: Child,
}

impl Drop for Spawned {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn short_socket(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("moord-{tag}-{}.sock", std::process::id()))
}

/// Start the daemon binary and wait until it accepts connections.
async fn spawn_daemon(data_dir: &Path, socket: &Path) -> Spawned {
    let child = Command::new(env!("CARGO_BIN_EXE_moord"))
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--socket")
        .arg(socket)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while tokio::net::UnixStream::connect(socket).await.is_err() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "daemon never listened"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Spawned { child }
}

async fn connect(socket: &Path) -> Client {
    Client::connect_unix(
        socket,
        Identity {
            client_id: ClientId::from_parts(1, 1),
            client: BuildInfo {
                name: "test".into(),
                version: "0".into(),
            },
            author: Author::Human {
                name: "ada".into(),
                machine: "box".into(),
            },
        },
    )
    .await
    .unwrap()
}

fn ids() -> (WorkspaceId, RepoId, ReviewId) {
    (
        WorkspaceId::from_parts(1, 1),
        RepoId::from_parts(1, 1),
        ReviewId::from_parts(1, 1),
    )
}

async fn seed(c: &Client, repo: &TestRepo) {
    let (ws, rid, review) = ids();
    let muts = [
        Mutation::CreateWorkspace {
            workspace_id: ws,
            name: "w".into(),
        },
        Mutation::AttachRepo {
            workspace_id: ws,
            repo_id: rid,
            path: repo.path().to_str().unwrap().into(),
            display_name: "r".into(),
        },
        Mutation::CreateReview {
            review_id: review,
            workspace_id: ws,
            title: "t".into(),
            targets: NonEmpty::singleton(ReviewTarget {
                repo_id: rid,
                base: RefSpec::Branch {
                    name: "main".into(),
                },
                head: RefSpec::Branch {
                    name: "feature".into(),
                },
            }),
        },
    ];
    for (n, mutation) in muts.into_iter().enumerate() {
        c.request(Request::Mutate {
            client_seq: ClientSeq::new(n as u64 + 1),
            mutation,
        })
        .await
        .unwrap();
    }
}

fn repo() -> TestRepo {
    RepoBuilder::new()
        .commit("base", files!["a.rs" => "fn a() {}\n"])
        .branch("feature")
        .commit("feat", files!["a.rs" => "fn a() { 1; }\n"])
        .build()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigkill_mid_burst_reopens_consistent_and_socket_is_reclaimed() {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path());
    let socket = short_socket("kill");
    let repo = repo();
    let (_, _, review) = ids();

    let mut daemon = spawn_daemon(&data.root, &socket).await;
    let c = connect(&socket).await;
    seed(&c, &repo).await;

    // Hammer with comments; every Committed reply is a durability promise.
    let mut acked = Vec::new();
    let mut n = 10u64;
    let killed_after = 40;
    loop {
        n += 1;
        let r = c
            .request(Request::Mutate {
                client_seq: ClientSeq::new(n),
                mutation: Mutation::AddComment {
                    review_id: review,
                    comment_id: CommentId::from_parts(1, u128::from(n)),
                    kind: CommentKind::Note,
                    anchor: Anchor::Review,
                    body: format!("c{n}"),
                },
            })
            .await;
        match r {
            Ok(Response::Committed { event }) => acked.push(event.seq),
            Ok(other) => panic!("{other:?}"),
            Err(ClientError::Closed) => break,
            Err(e) => panic!("{e}"),
        }
        if acked.len() == killed_after {
            // SIGKILL while the next appends are in flight.
            daemon.child.kill().unwrap();
        }
        assert!(acked.len() < 10_000, "daemon did not die");
    }
    daemon.child.wait().unwrap();
    assert!(acked.len() >= killed_after);
    let last_acked = *acked.last().unwrap();

    // Reopen offline: log is a prefix-closed sequence ending at or after
    // the last ack, and the views match a from-scratch fold of it.
    {
        let store = Store::open(&data.state()).unwrap();
        let last = store.last_seq().unwrap().unwrap();
        assert!(
            last >= last_acked,
            "acked {last_acked} but log ends at {last}"
        );
        let events = store.events_after(None).unwrap();
        assert_eq!(events.len() as u64, last.get());
        for (i, e) in events.iter().enumerate() {
            assert_eq!(e.seq.get(), i as u64 + 1, "gap in the log");
        }
        let incremental = store.dump_views().unwrap();
        store.rebuild_views().unwrap();
        assert_eq!(store.dump_views().unwrap(), incremental);
        let created = events
            .iter()
            .filter(|e| matches!(e.body, EventBody::CommentCreated { .. }))
            .count();
        assert_eq!(store.comments(review).unwrap().len(), created);
        assert!(created >= killed_after);
    }
    // And through Core, which is what the daemon opens.
    let core = Core::open(&data).unwrap();
    assert_eq!(
        core.last_seq().unwrap().unwrap().get(),
        core.events_after(None).unwrap().len() as u64
    );
    drop(core);

    // A fresh daemon reclaims the stale socket file and serves the state.
    assert!(socket.exists(), "stale socket left behind by the kill");
    let _daemon2 = spawn_daemon(&data.root, &socket).await;
    let c2 = connect(&socket).await;
    let Response::ReviewSnapshot { snapshot } = c2
        .request(Request::ReviewSnapshot { review_id: review })
        .await
        .unwrap()
    else {
        panic!("shape");
    };
    assert!(snapshot.comments.len() >= killed_after);
    assert!(snapshot.seq >= last_acked);
    // Appending continues from where the log ended.
    let Response::Committed { event } = c2
        .request(Request::Mutate {
            client_seq: ClientSeq::new(1),
            mutation: Mutation::AddComment {
                review_id: review,
                comment_id: CommentId::from_parts(2, 1),
                kind: CommentKind::Note,
                anchor: Anchor::Review,
                body: "after".into(),
            },
        })
        .await
        .unwrap()
    else {
        panic!("shape");
    };
    assert_eq!(event.seq.get(), snapshot.seq.get() + 1);
    let _ = std::fs::remove_file(&socket);
}

/// `--stdio` proxies to the machine's daemon, starting it detached when
/// nothing listens, and exits on EOF; `Request::Shutdown` stops the daemon.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_proxies_to_an_autostarted_daemon_and_shutdown_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let socket = short_socket("stdio");
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_moord"))
        .arg("--data-dir")
        .arg(dir.path())
        .arg("--socket")
        .arg(&socket)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stream = tokio::io::join(stdout, stdin);
    let c = Client::handshake(
        stream,
        Identity {
            client_id: ClientId::from_parts(1, 1),
            client: BuildInfo {
                name: "test".into(),
                version: "0".into(),
            },
            author: Author::Human {
                name: "ada".into(),
                machine: "box".into(),
            },
        },
        moor_protocol::ProtocolVersion::CURRENT,
    )
    .await
    .unwrap();
    assert_eq!(c.welcome.daemon.name, "moord");
    let Response::Workspaces { workspaces } = c.request(Request::ListWorkspaces).await.unwrap()
    else {
        panic!("shape");
    };
    assert!(workspaces.is_empty());
    assert!(
        moord::launch::is_listening(&socket).await,
        "a detached daemon was started"
    );
    drop(c);
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("proxy exits when stdin closes")
        .unwrap();
    assert!(status.success(), "{status}");

    // The daemon outlived the proxy; a direct client can stop it.
    let direct = connect(&socket).await;
    let Response::ShuttingDown = direct.request(Request::Shutdown).await.unwrap() else {
        panic!("shape");
    };
    let start = Instant::now();
    while moord::launch::is_listening(&socket).await {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "daemon did not exit"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
