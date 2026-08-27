//! `moor` against a daemon running in this test process (plan 2.6).

use std::path::PathBuf;
use std::sync::Arc;

use assert_cmd::Command;
use moor_protocol::BuildInfo;
use moor_review_core::DataDir;
use moor_test_support::{RepoBuilder, TestRepo, files};
use moord::Daemon;
use moord::server::UnixServer;
use predicates::prelude::*;
use tokio_util::sync::CancellationToken;

struct Harness {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    shutdown: CancellationToken,
    repo: TestRepo,
    _rt: tokio::runtime::Runtime,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

impl Harness {
    fn moor(&self) -> Command {
        let mut c = Command::cargo_bin("moor").unwrap();
        c.env("MOOR_SOCKET", &self.socket)
            .env("MOOR_USER", "ada")
            .env_remove("MOOR_AGENT");
        c
    }

    /// Run and return trimmed stdout.
    fn out(&self, args: &[&str]) -> String {
        let a = self.moor().args(args).assert().success();
        String::from_utf8(a.get_output().stdout.clone())
            .unwrap()
            .trim()
            .to_string()
    }
}

static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn start() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let socket = std::env::temp_dir().join(format!(
        "moor-cli-{}-{}.sock",
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let daemon = Daemon::open(
        &DataDir::new(dir.path()),
        BuildInfo {
            name: "moord".into(),
            version: "test".into(),
        },
    )
    .unwrap();
    let shutdown = CancellationToken::new();
    let server = {
        let _g = rt.enter();
        UnixServer::bind(&socket).unwrap()
    };
    rt.spawn(server.run(Arc::clone(&daemon), shutdown.clone()));
    let repo = RepoBuilder::new()
        .commit("base", files!["a.rs" => "fn a() {}\nfn z() {}\n"])
        .branch("feature")
        .commit("feat", files!["a.rs" => "fn a() { 1; }\nfn z() {}\n"])
        .build()
        .unwrap();
    Harness {
        _dir: dir,
        socket,
        shutdown,
        repo,
        _rt: rt,
    }
}

#[test]
fn workspace_review_comment_round_trip() {
    let h = start();
    let ws = h.out(&["workspace", "add", "w"]);
    assert_eq!(ws.len(), 26, "ULID: {ws}");
    let repo_path = h.repo.path().to_str().unwrap();
    let rid = h.out(&["workspace", "attach", &ws, repo_path]);
    h.moor()
        .args(["workspace", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&ws).and(predicate::str::contains(&rid)));

    let review = h.out(&[
        "review",
        "create",
        "--workspace",
        &ws,
        "--base",
        "main",
        "--head",
        "feature",
    ]);
    h.moor()
        .args(["review", "list", "--workspace", &ws])
        .assert()
        .success()
        .stdout(predicate::str::contains("main..feature"));
    h.moor()
        .args(["files", &review])
        .assert()
        .success()
        .stdout("Modified a.rs\n");
    h.moor()
        .args(["diff", &review, "a.rs"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("-fn a() {}").and(predicate::str::contains("+fn a() { 1; }")),
        );
    h.moor()
        .args(["show", &review, "a.rs", "--side", "base"])
        .assert()
        .success()
        .stdout("    1│fn a() {}\n    2│fn z() {}\n");

    let thread = h.out(&[
        "comment", "add", &review, "--path", "a.rs", "--line", "1", "--body", "hmm",
    ]);
    h.out(&["comment", "reply", &review, &thread, "--body", "ok"]);
    h.out(&["comment", "add", &review, "--body", "lgtm"]);
    h.moor()
        .args(["comment", "list", &review])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(format!("thread {thread} [open]"))
                .and(predicate::str::contains("ada @ a.rs:1-1 (Head): hmm"))
                .and(predicate::str::contains("ada @ review: lgtm")),
        );
    h.out(&["comment", "resolve", &review, &thread]);
    h.moor()
        .args(["comment", "list", &review])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "thread {thread} [resolved]"
        )));

    // JSON output is the protocol value.
    let json = h.out(&["--json", "review", "show", &review]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["review"]["id"], serde_json::json!(review));
    assert_eq!(v["files"][0]["path"], serde_json::json!("a.rs"));
    assert_eq!(v["threads"].as_array().unwrap().len(), 2);

    // Events replay from the start, attributed to the human.
    let events = h.out(&["events", "--since", "0"]);
    assert!(
        events.starts_with("#1 ada: workspace created w"),
        "{events}"
    );
    assert!(events.contains("comment "), "{events}");
    assert!(events.contains("thread resolved"), "{events}");
}

#[test]
fn agent_flag_attributes_to_an_agent() {
    let h = start();
    let ws = h.out(&["workspace", "add", "w"]);
    let json = h.out(&["--json", "--agent", "bot", "workspace", "list"]);
    assert!(json.contains(&ws));
    let ev = h.out(&["--json", "--agent", "bot", "workspace", "add", "w2"]);
    let v: serde_json::Value = serde_json::from_str(&ev).unwrap();
    assert_eq!(v["author"]["type"], serde_json::json!("Agent"));
    assert_eq!(v["author"]["name"], serde_json::json!("bot"));
    assert_eq!(v["author"]["via"], serde_json::json!("Cli"));
    assert_eq!(v["author"]["invoked_by"]["name"], serde_json::json!("ada"));
}

#[test]
fn errors_are_reported_not_panicked() {
    let h = start();
    h.moor()
        .args(["review", "show", "01ARZ3NDEKTSV4RRFFQ69G5FAV"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("NotFound"));
    let mut c = Command::cargo_bin("moor").unwrap();
    c.env("MOOR_SOCKET", "/tmp/definitely-not-a-moord.sock")
        .args(["workspace", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is moord running"));
}
