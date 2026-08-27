//! Daemon-level trigger (ARCHITECTURE §10): 200 comments appended through
//! the serialised writer thread, as an MCP agent burst would. Compare with
//! the `Core::add_comment` line in `moor-review-core`'s bench to see the
//! channel/broadcast overhead. See `docs/BENCHMARKS.md`.

use std::sync::Arc;
use std::time::Instant;

use moor_protocol::{
    Anchor, Author, BuildInfo, ClientId, ClientSeq, CommentId, CommentKind, NonEmpty, RefSpec,
    RepoId, ReviewId, ReviewTarget, WorkspaceId,
};
use moor_review_core::DataDir;
use moor_test_support::synthetic_repo;
use moord::Daemon;

fn author() -> Author {
    Author::Agent {
        name: "bench".into(),
        model: "bench".into(),
        session_id: "bench".into(),
        invoked_by: None,
        via: moor_protocol::AgentVia::Mcp,
    }
}

#[tokio::main]
async fn main() {
    let repo = synthetic_repo(100).expect("repo");
    let dir = tempfile::tempdir().expect("tempdir");
    let daemon = Daemon::open(
        &DataDir::new(dir.path().join("moor")),
        BuildInfo {
            name: "bench".into(),
            version: "0".into(),
        },
    )
    .expect("daemon");
    let ws = WorkspaceId::from_parts(1, 1);
    let rid = RepoId::from_parts(1, 2);
    let review = ReviewId::from_parts(1, 3);
    let path = repo.path().to_string_lossy().into_owned();
    daemon
        .write(move |c| {
            let ctx = Daemon::ctx(author(), ClientId::from_parts(1, 1), ClientSeq::new(0));
            c.create_workspace(&ctx, ws, "bench".into())?;
            c.attach_repo(&ctx, ws, rid, &path, "r".into())?;
            c.create_review(
                &ctx,
                review,
                ws,
                "bench".into(),
                NonEmpty::singleton(ReviewTarget {
                    repo_id: rid,
                    base: RefSpec::Head,
                    head: RefSpec::WorkingTree,
                }),
            )?;
            Ok(())
        })
        .await
        .expect("setup");

    let mut medians = Vec::new();
    let mut next = 0u128;
    for _ in 0..5 {
        let t = Instant::now();
        for _ in 0..200 {
            next += 1;
            let d = Arc::clone(&daemon);
            let body = format!("comment {next}");
            d.write(move |c| {
                let ctx = Daemon::ctx(author(), ClientId::from_parts(1, 1), ClientSeq::new(0));
                c.add_comment(
                    &ctx,
                    review,
                    CommentId::from_parts(2, next),
                    CommentKind::Note,
                    Anchor::Review,
                    body,
                )
            })
            .await
            .expect("comment");
        }
        medians.push(t.elapsed());
    }
    medians.sort();
    let m = medians[2];
    let flag = if m.as_millis() > 1000 {
        " **TRIPPED**"
    } else {
        ""
    };
    println!("| case | median | trigger | note |");
    println!("|------|-------:|--------:|------|");
    println!(
        "| 200-comment burst (Daemon::write) | {:.1} ms{flag} | 1000 ms | sequential awaits, writer thread + broadcast |",
        m.as_secs_f64() * 1000.0
    );
}
