//! End-to-end scenarios over `Core` (plan 1.6–1.8).

use moor_protocol::{
    AgentVia, Anchor, Author, ClientId, ClientSeq, CommentId, CommentKind, CommentState, EventBody,
    NonEmpty, RefSpec, RenderOpts, RepoId, RepoPath, ReviewId, ReviewTarget, Row, Side,
    ThreadResolution, Timestamp, WorkspaceId,
};
use moor_review_core::comments::{lines_anchor, thread_id_of};
use moor_review_core::review::ViewedState;
use moor_review_core::{Core, CoreError, Ctx, DataDir};
use moor_test_support::{RepoBuilder, TestRepo, files};

fn human() -> Ctx {
    Ctx {
        author: Author::Human {
            name: "ada".into(),
            machine: "box".into(),
        },
        client_id: ClientId::from_parts(1, 1),
        client_seq: ClientSeq::new(0),
        now: Timestamp::from_millis(1_700_000_000_000),
    }
}
fn other_human() -> Ctx {
    Ctx {
        author: Author::Human {
            name: "bob".into(),
            machine: "box".into(),
        },
        ..human()
    }
}
fn agent() -> Ctx {
    Ctx {
        author: Author::Agent {
            name: "claude-code".into(),
            model: "claude-fable-5".into(),
            session_id: "s1".into(),
            invoked_by: None,
            via: AgentVia::Mcp,
        },
        ..human()
    }
}
fn ws() -> WorkspaceId {
    WorkspaceId::from_parts(1, 10)
}
fn rid(n: u128) -> RepoId {
    RepoId::from_parts(1, n)
}
fn review_id(n: u128) -> ReviewId {
    ReviewId::from_parts(2, n)
}
fn cid(n: u128) -> CommentId {
    CommentId::from_parts(3, n)
}
fn p(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}

struct World {
    _dir: tempfile::TempDir,
    data: DataDir,
    core: Core,
    a: TestRepo,
    b: TestRepo,
}

const SRC: &str = "fn main() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let d = 4;\n    let e = 5;\n    let f = 6;\n    let g = 7;\n    let h = 8;\n    println!(\"{}\", a + b);\n}\n";

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path().join("moor"));
    let core = Core::open(&data).unwrap();
    let a = RepoBuilder::new()
        .commit("base", files!["src/main.rs" => SRC, "README.md" => "# a\n"])
        .branch("feature")
        .commit(
            "feat",
            files!["src/main.rs" => SRC.replace("let h = 8;", "let h = 80;"), "new.txt" => "n\n"],
        )
        .build()
        .unwrap();
    let b = RepoBuilder::new()
        .commit("base", files!["lib.rs" => "pub fn x() {}\n"])
        .branch("feature")
        .commit("feat", files!["lib.rs" => "pub fn x() {}\npub fn y() {}\n"])
        .build()
        .unwrap();
    let ctx = human();
    core.create_workspace(&ctx, ws(), "hacks".into()).unwrap();
    core.attach_repo(
        &ctx,
        ws(),
        rid(1),
        a.path().to_str().unwrap(),
        "zeta".into(),
    )
    .unwrap();
    core.attach_repo(
        &ctx,
        ws(),
        rid(2),
        b.path().to_str().unwrap(),
        "alpha".into(),
    )
    .unwrap();
    World {
        _dir: dir,
        data,
        core,
        a,
        b,
    }
}

fn targets() -> NonEmpty<ReviewTarget> {
    NonEmpty::new(vec![
        ReviewTarget {
            repo_id: rid(1),
            base: RefSpec::Branch {
                name: "main".into(),
            },
            head: RefSpec::Branch {
                name: "feature".into(),
            },
        },
        ReviewTarget {
            repo_id: rid(2),
            base: RefSpec::Branch {
                name: "main".into(),
            },
            head: RefSpec::Branch {
                name: "feature".into(),
            },
        },
    ])
    .unwrap()
}

fn head_blob(core: &Core, review: ReviewId, repo: RepoId, path: &str) -> moor_protocol::BlobOid {
    let f = core.file_change(review, repo, &p(path)).unwrap();
    f.kind.new_blob().unwrap()
}

#[test]
fn multi_repo_review_lists_files_ordered_by_repo_display_name() {
    let w = world();
    let rec = w
        .core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    assert!(rec.resolved.is_some());
    let files: Vec<(RepoId, String)> = w
        .core
        .files(review_id(1))
        .unwrap()
        .into_iter()
        .map(|f| (f.repo_id, f.path.to_string()))
        .collect();
    assert_eq!(
        files,
        vec![
            (rid(2), "lib.rs".into()),
            (rid(1), "new.txt".into()),
            (rid(1), "src/main.rs".into())
        ],
        "alpha (repo 2) sorts before zeta (repo 1)"
    );
    // Unknown repo in targets is rejected up front.
    let bad = NonEmpty::singleton(ReviewTarget {
        repo_id: rid(9),
        base: RefSpec::Head,
        head: RefSpec::Head,
    });
    assert!(matches!(
        w.core
            .create_review(&human(), review_id(2), ws(), "x".into(), bad),
        Err(CoreError::Invalid { .. })
    ));
}

#[test]
fn re_resolve_is_idempotent_and_emits_on_change() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    let before = w.core.last_seq().unwrap();
    let (_, changed) = w.core.resolve_targets(&human(), review_id(1)).unwrap();
    assert!(!changed);
    assert_eq!(
        w.core.last_seq().unwrap(),
        before,
        "no duplicate ReviewTargetsResolved"
    );

    w.a.write_file(
        "src/main.rs",
        SRC.replace("let h = 8;", "let h = 800;").as_bytes(),
    )
    .unwrap();
    w.a.git(&["commit", "-qam", "more"]).unwrap();
    let (_, changed) = w.core.resolve_targets(&human(), review_id(1)).unwrap();
    assert!(changed);
    let last = w.core.events_after(before).unwrap();
    assert!(matches!(
        last[0].body,
        EventBody::ReviewTargetsResolved { .. }
    ));
}

#[test]
fn commit_stepping_yields_parent_based_targets_with_full_messages() {
    let w = world();
    w.a.write_file("src/main.rs", b"changed\n").unwrap();
    w.a.git(&[
        "commit",
        "-qam",
        "second: subject\n\nbody paragraph\n\nmore body",
    ])
    .unwrap();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    let commits = w.core.commits(review_id(1), rid(1)).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].subject, "second: subject");
    assert_eq!(commits[0].body, "body paragraph\n\nmore body");
    assert_eq!(commits[0].author.name, "Test User");
    let step = w.core.commit_step(rid(1), commits[0].oid).unwrap();
    assert_eq!(step.head.tree, commits[0].tree);
    assert_eq!(step.base.tree, commits[1].tree, "base is the first parent");
    // Working-tree targets have no commit range.
    let wt = NonEmpty::singleton(ReviewTarget {
        repo_id: rid(1),
        base: RefSpec::Head,
        head: RefSpec::WorkingTree,
    });
    w.core
        .create_review(&human(), review_id(2), ws(), "wt".into(), wt)
        .unwrap();
    assert!(w.core.commits(review_id(2), rid(1)).unwrap().is_empty());
}

#[test]
fn viewed_marks_track_the_head_blob_and_reject_agents() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    let err = w
        .core
        .mark_viewed(&agent(), review_id(1), rid(1), p("src/main.rs"))
        .unwrap_err();
    assert!(matches!(err, CoreError::Forbidden { .. }), "{err}");

    w.core
        .mark_viewed(&human(), review_id(1), rid(1), p("src/main.rs"))
        .unwrap();
    assert_eq!(
        w.core
            .viewed_state(review_id(1), rid(1), &p("src/main.rs"))
            .unwrap(),
        ViewedState::Viewed
    );

    // Head moves without touching the file: still viewed.
    w.a.write_file("other.txt", b"o\n").unwrap();
    w.a.git(&["add", "."]).unwrap();
    w.a.git(&["commit", "-qm", "unrelated"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    assert_eq!(
        w.core
            .viewed_state(review_id(1), rid(1), &p("src/main.rs"))
            .unwrap(),
        ViewedState::Viewed
    );

    // Head moves and touches the file: changed since viewed.
    let marked = head_blob(&w.core, review_id(1), rid(1), "src/main.rs");
    w.a.write_file("src/main.rs", b"totally new\n").unwrap();
    w.a.git(&["commit", "-qam", "touch"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    assert_eq!(
        w.core
            .viewed_state(review_id(1), rid(1), &p("src/main.rs"))
            .unwrap(),
        ViewedState::ChangedSinceViewed {
            marked: Some(marked)
        }
    );
    w.core
        .unmark_viewed(&human(), review_id(1), rid(1), p("src/main.rs"))
        .unwrap();
    assert_eq!(
        w.core
            .viewed_state(review_id(1), rid(1), &p("src/main.rs"))
            .unwrap(),
        ViewedState::Unviewed
    );
}

#[test]
fn comments_validate_anchors_and_stamp_context_hash() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    let blob = head_blob(&w.core, review_id(1), rid(1), "src/main.rs");
    let anchor = lines_anchor(rid(1), p("src/main.rs"), Side::Head, blob, 9, 9).unwrap();
    let c = w
        .core
        .add_comment(
            &human(),
            review_id(1),
            cid(1),
            CommentKind::Note,
            anchor,
            "why 80?".into(),
        )
        .unwrap();
    let Anchor::Lines { context_hash, .. } = c.anchor else {
        panic!()
    };
    assert_ne!(context_hash.get(), 0, "daemon stamps the hash");
    assert_eq!(c.thread_id, thread_id_of(cid(1)));

    let too_far = lines_anchor(rid(1), p("src/main.rs"), Side::Head, blob, 1, 999).unwrap();
    let err = w
        .core
        .add_comment(
            &human(),
            review_id(1),
            cid(2),
            CommentKind::Note,
            too_far,
            "x".into(),
        )
        .unwrap_err();
    assert!(matches!(err, CoreError::Invalid { .. }), "{err}");

    let wrong_repo = lines_anchor(rid(9), p("src/main.rs"), Side::Head, blob, 1, 1).unwrap();
    assert!(matches!(
        w.core.add_comment(
            &human(),
            review_id(1),
            cid(3),
            CommentKind::Note,
            wrong_repo,
            "x".into()
        ),
        Err(CoreError::Invalid { .. })
    ));
}

#[test]
fn threads_replies_permissions_and_resolution() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    w.core
        .add_comment(
            &human(),
            review_id(1),
            cid(1),
            CommentKind::Note,
            Anchor::Review,
            "overall".into(),
        )
        .unwrap();
    let th = thread_id_of(cid(1));
    let reply = w
        .core
        .reply(
            &agent(),
            review_id(1),
            th,
            cid(2),
            CommentKind::Note,
            "ack".into(),
        )
        .unwrap();
    assert_eq!(reply.anchor, Anchor::Review);
    assert!(matches!(reply.author, Author::Agent { .. }));

    // Only the author edits/deletes.
    assert!(matches!(
        w.core
            .edit_comment(&other_human(), review_id(1), cid(1), "x".into()),
        Err(CoreError::Forbidden { .. })
    ));
    w.core
        .edit_comment(&human(), review_id(1), cid(1), "edited".into())
        .unwrap();
    let c = w
        .core
        .comments(review_id(1))
        .unwrap()
        .into_iter()
        .find(|c| c.id == cid(1))
        .unwrap();
    assert_eq!(c.body, "edited");
    assert!(c.edited.is_some());

    w.core.resolve_thread(&human(), review_id(1), th).unwrap();
    assert!(matches!(
        w.core.resolve_thread(&human(), review_id(1), th),
        Err(CoreError::Invalid { .. })
    ));
    let t = w.core.threads(review_id(1)).unwrap().pop().unwrap();
    assert!(matches!(t.resolution, ThreadResolution::Resolved { .. }));
    assert_eq!(t.replies, vec![cid(2)]);
    w.core.unresolve_thread(&human(), review_id(1), th).unwrap();

    w.core
        .delete_comment(&agent(), review_id(1), cid(2))
        .unwrap();
    assert!(matches!(
        w.core.reply(
            &human(),
            review_id(1),
            thread_id_of(cid(2)),
            cid(3),
            CommentKind::Note,
            "?".into()
        ),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn comments_reanchor_when_head_moves() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    let blob = head_blob(&w.core, review_id(1), rid(1), "src/main.rs");
    // Anchor on "let g = 7;" (line 8).
    let anchor = lines_anchor(rid(1), p("src/main.rs"), Side::Head, blob, 8, 8).unwrap();
    w.core
        .add_comment(
            &human(),
            review_id(1),
            cid(1),
            CommentKind::Note,
            anchor,
            "g".into(),
        )
        .unwrap();
    // File-level comment on README (not in the diff).
    let readme_blob = w
        .core
        .tree_snapshot(
            rid(1),
            &RefSpec::Branch {
                name: "feature".into(),
            },
        )
        .unwrap()
        .entries
        .into_iter()
        .find(|e| e.path.as_str() == "README.md")
        .map(|e| match e.kind {
            moor_protocol::TreeEntryKind::File { oid, .. } => oid,
            _ => panic!(),
        })
        .unwrap();
    w.core
        .add_comment(
            &human(),
            review_id(1),
            cid(2),
            CommentKind::Note,
            Anchor::File {
                repo_id: rid(1),
                path: p("README.md"),
                blob_oid: readme_blob,
            },
            "readme".into(),
        )
        .unwrap();

    // 1) Insert two lines at the top of the file: comment shifts, stays live.
    let shifted = format!(
        "// header\n// header2\n{}",
        SRC.replace("let h = 8;", "let h = 80;")
    );
    w.a.write_file("src/main.rs", shifted.as_bytes()).unwrap();
    w.a.git(&["commit", "-qam", "shift"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    let c = w
        .core
        .comments(review_id(1))
        .unwrap()
        .into_iter()
        .find(|c| c.id == cid(1))
        .unwrap();
    let Anchor::Lines {
        lines, blob_oid, ..
    } = &c.anchor
    else {
        panic!()
    };
    assert_eq!(lines.start().get(), 10);
    assert_eq!(
        *blob_oid,
        head_blob(&w.core, review_id(1), rid(1), "src/main.rs")
    );
    assert_eq!(c.state, CommentState::Live);
    let readme = w
        .core
        .comments(review_id(1))
        .unwrap()
        .into_iter()
        .find(|c| c.id == cid(2))
        .unwrap();
    assert_eq!(
        readme.state,
        CommentState::Live,
        "untouched file-level comment is unchanged"
    );

    // 2) Edit the anchored line: outdated, keeps last good anchor.
    let edited = shifted.replace("let g = 7;", "let g = 70;");
    w.a.write_file("src/main.rs", edited.as_bytes()).unwrap();
    w.a.git(&["commit", "-qam", "edit"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    let c = w
        .core
        .comments(review_id(1))
        .unwrap()
        .into_iter()
        .find(|c| c.id == cid(1))
        .unwrap();
    let CommentState::Outdated {
        last_good_anchor: Anchor::Lines { lines, .. },
    } = &c.state
    else {
        panic!("{:?}", c.state)
    };
    assert_eq!(lines.start().get(), 10);

    // 3) Revert the edit: comment comes back to life.
    w.a.write_file("src/main.rs", shifted.as_bytes()).unwrap();
    w.a.git(&["commit", "-qam", "revert"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    let c = w
        .core
        .comments(review_id(1))
        .unwrap()
        .into_iter()
        .find(|c| c.id == cid(1))
        .unwrap();
    assert_eq!(c.state, CommentState::Live);

    // 4) Rename the file: both comments follow the new path.
    w.a.git(&["mv", "src/main.rs", "src/app.rs"]).unwrap();
    w.a.git(&["mv", "README.md", "README.txt"]).unwrap();
    w.a.git(&["commit", "-qm", "rename"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    let cs = w.core.comments(review_id(1)).unwrap();
    let c1 = cs.iter().find(|c| c.id == cid(1)).unwrap();
    let Anchor::Lines { path, .. } = &c1.anchor else {
        panic!()
    };
    assert_eq!(path.as_str(), "src/app.rs");
    let c2 = cs.iter().find(|c| c.id == cid(2)).unwrap();
    let Anchor::File { path, .. } = &c2.anchor else {
        panic!()
    };
    assert_eq!(path.as_str(), "README.txt");

    // 5) Delete the file: outdated.
    w.a.git(&["rm", "-q", "src/app.rs"]).unwrap();
    w.a.git(&["commit", "-qm", "rm"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    let c = w
        .core
        .comments(review_id(1))
        .unwrap()
        .into_iter()
        .find(|c| c.id == cid(1))
        .unwrap();
    assert!(matches!(c.state, CommentState::Outdated { .. }));
}

#[test]
fn base_side_anchor_survives_base_move() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    let old_blob = w
        .core
        .file_change(review_id(1), rid(1), &p("src/main.rs"))
        .unwrap()
        .kind
        .old_blob()
        .unwrap();
    let anchor = lines_anchor(rid(1), p("src/main.rs"), Side::Base, old_blob, 8, 8).unwrap();
    w.core
        .add_comment(
            &human(),
            review_id(1),
            cid(1),
            CommentKind::Note,
            anchor,
            "base side".into(),
        )
        .unwrap();
    // Move main forward with an insert above.
    w.a.git(&["checkout", "-q", "main"]).unwrap();
    w.a.write_file("src/main.rs", format!("// top\n{SRC}").as_bytes())
        .unwrap();
    w.a.git(&["commit", "-qam", "base moves"]).unwrap();
    w.a.git(&["checkout", "-q", "feature"]).unwrap();
    w.core.resolve_targets(&human(), review_id(1)).unwrap();
    let c = w.core.comments(review_id(1)).unwrap().pop().unwrap();
    let Anchor::Lines { lines, side, .. } = &c.anchor else {
        panic!()
    };
    assert_eq!(*side, Side::Base);
    assert_eq!(lines.start().get(), 9);
    assert_eq!(c.state, CommentState::Live);
}

#[test]
fn suggestion_applies_once_to_the_working_tree() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    // feature is checked out in repo a; suggest on its head blob.
    let blob = head_blob(&w.core, review_id(1), rid(1), "src/main.rs");
    let anchor = lines_anchor(rid(1), p("src/main.rs"), Side::Head, blob, 2, 2).unwrap();
    let patch = "@@ -2,1 +2,1 @@\n-    let a = 1;\n+    let a = 10;\n".to_string();
    w.core
        .add_comment(
            &agent(),
            review_id(1),
            cid(1),
            CommentKind::Suggestion { patch },
            anchor,
            "ten".into(),
        )
        .unwrap();
    let result = w
        .core
        .apply_suggestion(&human(), review_id(1), cid(1))
        .unwrap();
    let on_disk = std::fs::read_to_string(w.a.path().join("src/main.rs")).unwrap();
    assert!(on_disk.contains("let a = 10;"));
    assert_eq!(
        w.core.repo_blob(rid(1), result).unwrap(),
        on_disk.as_bytes()
    );
    let err = w
        .core
        .apply_suggestion(&human(), review_id(1), cid(1))
        .unwrap_err();
    assert!(
        matches!(err, CoreError::Invalid { .. }),
        "second apply: {err}"
    );
    assert!(
        w.core
            .events_after(None)
            .unwrap()
            .iter()
            .any(|e| matches!(e.body, EventBody::SuggestionApplied { .. }))
    );
}

#[test]
fn file_render_and_snapshot_and_reopen() {
    let w = world();
    w.core
        .create_review(&human(), review_id(1), ws(), "r".into(), targets())
        .unwrap();
    let (header, rendered) = w
        .core
        .file_render(
            review_id(1),
            rid(1),
            &p("src/main.rs"),
            RenderOpts::default(),
        )
        .unwrap();
    assert_eq!(header.lang.as_deref(), Some("Rust"));
    assert!(
        rendered
            .rows
            .iter()
            .any(|r| matches!(r, Row::Modified { .. }))
    );
    // Second call is served from the cache and identical.
    let again = w
        .core
        .file_render(
            review_id(1),
            rid(1),
            &p("src/main.rs"),
            RenderOpts::default(),
        )
        .unwrap();
    assert_eq!(again, (header.clone(), rendered.clone()));
    // Blob render for the explorer.
    let (bh, br) = w
        .core
        .blob_render(
            rid(1),
            &p("src/main.rs"),
            match &header.target {
                moor_protocol::RenderTarget::Diff { change } => change.new_blob().unwrap(),
                moor_protocol::RenderTarget::Blob { oid } => *oid,
            },
        )
        .unwrap();
    assert!(br.rows.iter().all(|r| matches!(r, Row::Context { .. })));
    assert_eq!(bh.lang.as_deref(), Some("Rust"));
    assert!(matches!(
        w.core
            .file_render(review_id(1), rid(1), &p("nope.rs"), RenderOpts::default()),
        Err(CoreError::NotFound { .. })
    ));

    w.core
        .add_comment(
            &human(),
            review_id(1),
            cid(1),
            CommentKind::Note,
            Anchor::Review,
            "hi".into(),
        )
        .unwrap();
    w.core
        .mark_viewed(&human(), review_id(1), rid(2), p("lib.rs"))
        .unwrap();
    let snap = w.core.review_snapshot(review_id(1)).unwrap();
    assert_eq!(snap.comments.len(), 1);
    assert_eq!(snap.threads.len(), 1);
    assert_eq!(snap.viewed.len(), 1);
    assert_eq!(Some(snap.seq), w.core.last_seq().unwrap());

    // Reopen from the same data dir: everything persists; repos reopen lazily.
    drop(w.core);
    let core = Core::open(&w.data).unwrap();
    assert_eq!(core.review_snapshot(review_id(1)).unwrap(), snap);
    assert_eq!(core.files(review_id(1)).unwrap().len(), 3);
    core.delete_review(&human(), review_id(1)).unwrap();
    assert!(core.reviews(ws()).unwrap().is_empty());
    assert!(matches!(
        core.review(review_id(1)),
        Err(CoreError::NotFound { .. })
    ));
    drop(w.b);
}
