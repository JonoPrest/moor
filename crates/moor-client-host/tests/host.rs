//! Plan 4.3: the host against a real daemon in a temp dir. Connect,
//! subscribe, open a review with a 100k-line file, scroll it end to end,
//! comment; every patch the UI would receive stays under 64 KB and the
//! prefs survive a host restart through the redb KV.

// The harness reads top to bottom.
#![allow(clippy::too_many_lines)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use moor_client_core::{Action, ConnectionView, FileRef, IdSeed, Layout, ViewPatch};
use moor_client_host::{Handle, HostConfig, Identity, KvConfig, local_config, spawn};
use moor_protocol::{
    Author, BuildInfo, ClientId, ClientSeq, Mutation, NonEmpty, RefSpec, RepoId, RepoPath, Request,
    Response, ReviewId, ReviewTarget, ViewSection, WorkspaceId,
};
use moor_review_core::DataDir;
use moor_test_support::{RepoBuilder, TestRepo};
use moord::Daemon;
use moord::client::Client;
use moord::server::UnixServer;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const IPC_LIMIT: usize = 64 * 1024;

struct Harness {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    shutdown: CancellationToken,
    _repo: TestRepo,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

fn big_source(lines: usize) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for i in 0..lines {
        let _ = writeln!(out, "fn line_{i}() -> u32 {{ {i} }}");
    }
    out
}

fn identity(n: u128) -> Identity {
    Identity {
        client_id: ClientId::from_parts(1, n),
        client: BuildInfo {
            name: "host-test".into(),
            version: "0".into(),
        },
        author: Author::Human {
            name: "ada".into(),
            machine: "box".into(),
        },
    }
}

fn ws() -> WorkspaceId {
    WorkspaceId::from_parts(1, 1)
}
fn rid() -> RepoId {
    RepoId::from_parts(1, 1)
}
fn review_id() -> ReviewId {
    ReviewId::from_parts(1, 1)
}

/// Daemon on a unix socket with one workspace, repo and review whose head
/// branch adds a 100k-line file and changes README.
async fn start() -> Harness {
    let repo = RepoBuilder::new()
        .commit(
            "base",
            &[("README.md", b"hello\n"), ("src/lib.rs", b"fn a() {}\n")],
        )
        .branch("feature")
        .checkout("feature")
        .commit(
            "big",
            &[
                ("README.md", b"hello world\n"),
                ("src/big.rs", big_source(100_000).as_bytes()),
            ],
        )
        .checkout("main")
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let socket = std::env::temp_dir().join(format!(
        "moor-host-{}.sock",
        std::process::id()
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
    ));
    let daemon = Daemon::open(
        &DataDir::new(dir.path()),
        BuildInfo {
            name: "moord".into(),
            version: "test".into(),
        },
    )
    .unwrap();
    let server = UnixServer::bind(&socket).unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(server.run(Arc::clone(&daemon), shutdown.clone()));

    let seed = identity(99);
    let client = Client::connect_unix(
        &socket,
        moord::client::Identity {
            client_id: seed.client_id,
            client: seed.client,
            author: seed.author,
        },
    )
    .await
    .unwrap();
    let mutate = |seq: u64, m: Mutation| {
        let client = &client;
        async move {
            match client
                .request(Request::Mutate {
                    client_seq: ClientSeq::new(seq),
                    mutation: m,
                })
                .await
                .unwrap()
            {
                Response::Committed { .. } => {}
                other => panic!("unexpected {other:?}"),
            }
        }
    };
    mutate(
        1,
        Mutation::CreateWorkspace {
            workspace_id: ws(),
            name: "w".into(),
        },
    )
    .await;
    mutate(
        2,
        Mutation::AttachRepo {
            workspace_id: ws(),
            repo_id: rid(),
            path: repo.path().to_str().unwrap().into(),
            display_name: "r".into(),
        },
    )
    .await;
    mutate(
        3,
        Mutation::CreateReview {
            review_id: review_id(),
            workspace_id: ws(),
            title: "big".into(),
            targets: NonEmpty::new(vec![ReviewTarget {
                repo_id: rid(),
                base: RefSpec::Branch {
                    name: "main".into(),
                },
                head: RefSpec::Branch {
                    name: "feature".into(),
                },
            }])
            .unwrap(),
        },
    )
    .await;
    // Resolve so the review has trees and files before a client opens it.
    let _ = client
        .request(Request::ResolveTargets {
            review_id: review_id(),
        })
        .await
        .unwrap();
    Harness {
        _dir: dir,
        socket,
        shutdown,
        _repo: repo,
    }
}

/// Collect patches until `done` says so or the timeout passes. Every patch
/// is size-checked on the way.
async fn until(
    rx: &mut mpsc::UnboundedReceiver<Vec<ViewPatch>>,
    seen: &mut Vec<ViewPatch>,
    mut done: impl FnMut(&ViewPatch) -> bool,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let batch = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("timed out waiting for a patch")
            .expect("host ended");
        let mut hit = false;
        for p in batch {
            let size = serde_json::to_vec(&vec![p.clone()]).unwrap().len();
            assert!(size < IPC_LIMIT, "{:?} patch is {size} bytes", p.section());
            hit |= done(&p);
            seen.push(p);
        }
        if hit {
            return;
        }
    }
}

fn config(h: &Harness, kv: KvConfig) -> HostConfig {
    local_config(&h.socket, identity(1), IdSeed(0x5eed), kv)
}

async fn connect_and_open(
    handle: &Handle,
    rx: &mut mpsc::UnboundedReceiver<Vec<ViewPatch>>,
    seen: &mut Vec<ViewPatch>,
) {
    assert!(handle.dispatch(Action::Connect));
    until(rx, seen, |p| {
        matches!(
            p,
            ViewPatch::Connection {
                connection: ConnectionView::Subscribed,
                ..
            }
        )
    })
    .await;
    assert!(handle.dispatch(Action::ListReviews { workspace_id: ws() }));
    until(
        rx,
        seen,
        |p| matches!(p, ViewPatch::ReviewList { reviews } if reviews.len() == 1),
    )
    .await;
    assert!(handle.dispatch(Action::OpenReview {
        review_id: review_id()
    }));
    // The stream ends with the tree carrying both changed files.
    until(
        rx,
        seen,
        |p| matches!(p, ViewPatch::Progress { progress } if progress.total == 2),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scripted_session_keeps_every_patch_small() {
    let h = start().await;
    let shutdown = CancellationToken::new();
    let (handle, mut rx) = spawn(config(&h, KvConfig::Memory), shutdown.clone()).unwrap();
    let mut seen = Vec::new();
    connect_and_open(&handle, &mut rx, &mut seen).await;

    let big = FileRef {
        repo_id: rid(),
        path: RepoPath::new("src/big.rs").unwrap(),
    };
    // Open the big file at the top: the diff patch holds the viewport only.
    assert!(handle.dispatch(Action::Viewport {
        file: big.clone(),
        first_row: 0,
        last_row: 59,
    }));
    until(&mut rx, &mut seen, |p| {
        matches!(p, ViewPatch::Diff { diff: Some(d), .. } if !d.rows.is_empty() && d.missing.is_empty())
    })
    .await;
    let total_rows = seen
        .iter()
        .rev()
        .find_map(|p| match p {
            ViewPatch::Diff { diff: Some(d), .. } => match d.content {
                moor_protocol::RenderContent::Text { total_rows, .. } => Some(total_rows),
                moor_protocol::RenderContent::Binary => None,
            },
            _ => None,
        })
        .unwrap();
    assert!(total_rows >= 100_000, "total rows {total_rows}");

    // Scroll to the end: only the far viewport is fetched and pushed.
    assert!(handle.dispatch(Action::Viewport {
        file: big.clone(),
        first_row: total_rows - 60,
        last_row: total_rows - 1,
    }));
    until(&mut rx, &mut seen, |p| {
        matches!(
            p,
            ViewPatch::Diff { diff: Some(d), .. }
                if d.first_row == total_rows - 60 && !d.rows.is_empty() && d.missing.is_empty()
        )
    })
    .await;
    let last = seen
        .iter()
        .rev()
        .find_map(|p| match p {
            ViewPatch::Diff { diff: Some(d), .. } => Some(d.clone()),
            _ => None,
        })
        .unwrap();
    assert!(last.rows.len() <= 60);

    // Comment on the first row: the thread shows up (pending, then committed).
    assert!(handle.dispatch(Action::Viewport {
        file: big,
        first_row: 0,
        last_row: 59,
    }));
    assert!(handle.dispatch(Action::SetFocus {
        focus: moor_client_core::Focus::Diff { row: 1 }
    }));
    assert!(handle.dispatch(Action::DraftOpened {
        anchor: moor_protocol::Anchor::Review
    }));
    assert!(handle.dispatch(Action::DraftSubmitted {
        body: "x".repeat(10_000)
    }));
    until(&mut rx, &mut seen, |p| {
        matches!(p, ViewPatch::Conversation { conversation } if conversation.iter().any(|t| !t.pending))
    })
    .await;

    // Every section was pushed at least once during the session.
    for s in [
        ViewSection::Connection,
        ViewSection::ReviewList,
        ViewSection::Tree,
        ViewSection::Diff,
        ViewSection::Threads,
        ViewSection::Progress,
        ViewSection::Focus,
        ViewSection::Hints,
        ViewSection::Draft,
    ] {
        assert!(seen.iter().any(|p| p.section() == s), "no {s:?} patch");
    }
    assert!(handle.attach());
    until(&mut rx, &mut seen, |p| matches!(p, ViewPatch::Help { .. })).await;
    shutdown.cancel();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefs_survive_a_host_restart_through_redb() {
    let h = start().await;
    let kv = h.socket.with_extension("redb");
    {
        let shutdown = CancellationToken::new();
        let (handle, mut rx) =
            spawn(config(&h, KvConfig::Redb(kv.clone())), shutdown.clone()).unwrap();
        let mut seen = Vec::new();
        assert!(handle.dispatch(Action::Connect));
        until(&mut rx, &mut seen, |p| {
            matches!(
                p,
                ViewPatch::Connection {
                    connection: ConnectionView::Subscribed,
                    ..
                }
            )
        })
        .await;
        assert!(handle.dispatch(Action::SetLayout {
            layout: Layout::Split
        }));
        until(
            &mut rx,
            &mut seen,
            |p| matches!(p, ViewPatch::Diff { prefs, .. } if prefs.layout == Layout::Split),
        )
        .await;
        shutdown.cancel();
        // Let the task drop the database before reopening it.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let shutdown = CancellationToken::new();
    let (handle, mut rx) = spawn(config(&h, KvConfig::Redb(kv)), shutdown.clone()).unwrap();
    let mut seen = Vec::new();
    assert!(handle.dispatch(Action::Connect));
    until(
        &mut rx,
        &mut seen,
        |p| matches!(p, ViewPatch::Diff { prefs, .. } if prefs.layout == Layout::Split),
    )
    .await;
    shutdown.cancel();
}
