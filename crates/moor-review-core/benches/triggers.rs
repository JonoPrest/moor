//! "Measure before optimising" triggers (ARCHITECTURE §10, plan 3.0).
//!
//! Not a micro-benchmark harness: each case is one realistic operation on a
//! synthetic repo, timed a few times, printed as a table against its trigger
//! threshold. `MOOR_BENCH_FILES` scales the repo (default 50 000; CI uses a
//! small value so the suite stays quick). Numbers live in
//! `docs/BENCHMARKS.md`. Exit status is always 0: triggers inform, they do
//! not gate.

use std::time::{Duration, Instant};

use moor_protocol::{
    Anchor, Author, ClientId, ClientSeq, CommentId, CommentKind, NonEmpty, RefSpec, RepoId,
    ReviewId, ReviewTarget, Timestamp, WorkspaceId,
};
use moor_review_core::git::Repo;
use moor_review_core::{Core, Ctx, DataDir};
use moor_test_support::{TestRepo, synthetic_files, synthetic_repo};

const RUNS: usize = 5;

fn files() -> usize {
    std::env::var("MOOR_BENCH_FILES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000)
}

fn ctx() -> Ctx {
    Ctx {
        author: Author::Human {
            name: "bench".into(),
            machine: "bench".into(),
        },
        client_id: ClientId::from_parts(1, 1),
        client_seq: ClientSeq::new(0),
        now: Timestamp::from_millis(1_700_000_000_000),
    }
}

/// Median of `RUNS` timings of `f`, which returns a label-worthy value.
fn median<T>(mut f: impl FnMut() -> T) -> (Duration, T) {
    let mut times = Vec::with_capacity(RUNS);
    let mut last = None;
    for _ in 0..RUNS {
        let t = Instant::now();
        last = Some(f());
        times.push(t.elapsed());
    }
    times.sort();
    (times[RUNS / 2], last.expect("RUNS > 0"))
}

struct Line {
    name: &'static str,
    median: Duration,
    trigger: Duration,
    note: String,
}

fn report(lines: &[Line]) {
    println!("| case | median | trigger | note |");
    println!("|------|-------:|--------:|------|");
    for l in lines {
        let flag = if l.median > l.trigger {
            " **TRIPPED**"
        } else {
            ""
        };
        println!(
            "| {} | {:.1} ms{flag} | {} ms | {} |",
            l.name,
            l.median.as_secs_f64() * 1000.0,
            l.trigger.as_millis(),
            l.note
        );
    }
}

fn snapshot_after_single_edit(repo: &TestRepo, n: usize) -> Line {
    let r = Repo::open(repo.path()).expect("open");
    // Warm the temp-index path once so stat caching applies.
    r.working_tree().expect("warm");
    let mut i = 0usize;
    let (median, _) = median(|| {
        i += 1;
        repo.write_file("dir_000/file_00000.txt", format!("edit {i}\n").as_bytes())
            .expect("write");
        r.working_tree().expect("snapshot")
    });
    Line {
        name: "working-tree snapshot after one edit",
        median,
        trigger: Duration::from_millis(100),
        note: format!("{n} files"),
    }
}

fn changed_files_on_directory_move(repo: &TestRepo, n: usize) -> Line {
    let r = Repo::open(repo.path()).expect("open");
    let base = r.resolve(&RefSpec::Head).expect("head");
    repo.git(&["mv", "dir_000", "moved_000"]).expect("mv");
    repo.git(&["commit", "-q", "-m", "move"]).expect("commit");
    let head = r.resolve(&RefSpec::Head).expect("head");
    let (median, changes) = median(|| r.changed_files(base.tree, head.tree).expect("diff"));
    repo.git(&["reset", "-q", "--hard", "HEAD~1"])
        .expect("reset");
    Line {
        name: "changed_files on a directory move",
        median,
        trigger: Duration::from_millis(500),
        note: format!("{} renames of {n} files", changes.len()),
    }
}

fn tree_snapshot(repo: &TestRepo, n: usize) -> Line {
    let r = Repo::open(repo.path()).expect("open");
    let head = r.resolve(&RefSpec::Head).expect("head");
    let (median, snap) = median(|| {
        r.tree_snapshot(RepoId::from_parts(1, 1), head.tree)
            .expect("snapshot")
    });
    let bytes = serde_json::to_vec(&snap).expect("json").len();
    Line {
        name: "tree_snapshot",
        median,
        trigger: Duration::from_millis(200),
        note: format!("{n} files, {} MB JSON (trigger 5 MB)", bytes / 1_000_000),
    }
}

fn comment_burst(repo: &TestRepo) -> Line {
    let dir = tempfile::tempdir().expect("tempdir");
    let core = Core::open(&DataDir::new(dir.path().join("moor"))).expect("core");
    let c = ctx();
    let ws = WorkspaceId::from_parts(1, 1);
    let rid = RepoId::from_parts(1, 2);
    core.create_workspace(&c, ws, "bench".into()).expect("ws");
    core.attach_repo(&c, ws, rid, &repo.path().to_string_lossy(), "r".into())
        .expect("attach");
    let review = ReviewId::from_parts(1, 3);
    core.create_review(
        &c,
        review,
        ws,
        "bench".into(),
        NonEmpty::singleton(ReviewTarget {
            repo_id: rid,
            base: RefSpec::Head,
            head: RefSpec::WorkingTree,
        }),
    )
    .expect("review");
    let mut next = 0u128;
    let (median, ()) = median(|| {
        for _ in 0..200 {
            next += 1;
            core.add_comment(
                &c,
                review,
                CommentId::from_parts(2, next),
                CommentKind::Note,
                Anchor::Review,
                format!("comment {next}"),
                None,
            )
            .expect("comment");
        }
    });
    Line {
        name: "200-comment burst (Core::add_comment)",
        median,
        trigger: Duration::from_secs(1),
        note: "durable redb append per comment".into(),
    }
}

fn main() {
    let n = files();
    let t = Instant::now();
    let repo = synthetic_repo(n).expect("synthetic repo");
    println!(
        "synthetic repo: {n} files in {:.1} s\n",
        t.elapsed().as_secs_f64()
    );
    // `synthetic_files` is the same list `synthetic_repo` wrote; sanity.
    assert_eq!(synthetic_files(n).len(), n);
    let lines = vec![
        snapshot_after_single_edit(&repo, n),
        changed_files_on_directory_move(&repo, n),
        tree_snapshot(&repo, n),
        comment_burst(&repo),
    ];
    report(&lines);
}
