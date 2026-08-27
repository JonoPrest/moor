//! Git engine tests over real repositories.

use moor_protocol::{
    ChangeKind, CommitOid, RefSpec, RepoId, ResolvedSource, TreeEntryKind, TreeOid,
};
use moor_review_core::git::{Repo, is_binary};
use moor_test_support::{RepoBuilder, files};

fn commit(s: &str) -> CommitOid {
    CommitOid::new(s.parse().unwrap())
}

#[test]
fn resolves_every_refspec_variant() {
    let t = RepoBuilder::new()
        .commit("one", files!["a.txt" => "a\n"])
        .tag("v1")
        .branch("feature")
        .commit("two", files!["a.txt" => "b\n"])
        .build()
        .unwrap();
    // an upstream for `feature`: point it at main
    t.git(&["branch", "--set-upstream-to=main", "feature"])
        .unwrap();
    let repo = Repo::open(t.path()).unwrap();
    let main = commit(&t.rev_parse("main").unwrap());
    let feature = commit(&t.rev_parse("feature").unwrap());

    let r = repo
        .resolve(&RefSpec::Branch {
            name: "main".into(),
        })
        .unwrap();
    assert_eq!(r.source, ResolvedSource::Commit { oid: main });
    let r = repo.resolve(&RefSpec::Tag { name: "v1".into() }).unwrap();
    assert_eq!(r.source, ResolvedSource::Commit { oid: main });
    let r = repo.resolve(&RefSpec::Head).unwrap();
    assert_eq!(r.source, ResolvedSource::Commit { oid: feature });
    let r = repo.resolve(&RefSpec::Upstream).unwrap();
    assert_eq!(r.source, ResolvedSource::Commit { oid: main });
    let r = repo.resolve(&RefSpec::Commit { oid: feature }).unwrap();
    assert_eq!(
        r.tree.to_string(),
        t.git(&["rev-parse", "feature^{tree}"]).unwrap()
    );

    let err = repo
        .resolve(&RefSpec::Branch {
            name: "nope".into(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("nope"), "{err}");
}

#[test]
fn changed_files_detects_add_delete_modify_rename() {
    let t = RepoBuilder::new()
        .commit(
            "one",
            files![
                "keep.txt" => "same\n",
                "mod.txt" => "old\n",
                "gone.txt" => "bye\n",
                "old_name.rs" => "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n",
            ],
        )
        .branch("feature")
        .commit_removing("rm", &["gone.txt", "old_name.rs"])
        .commit(
            "two",
            files![
                "mod.txt" => "new\n",
                "added.txt" => "hi\n",
                "new_name.rs" => "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n",
            ],
        )
        .build()
        .unwrap();
    let repo = Repo::open(t.path()).unwrap();
    let base = repo
        .resolve(&RefSpec::Branch {
            name: "main".into(),
        })
        .unwrap();
    let head = repo
        .resolve(&RefSpec::Branch {
            name: "feature".into(),
        })
        .unwrap();
    let mut changes = repo.changed_files(base.tree, head.tree).unwrap();
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    let summary: Vec<(String, &str)> = changes
        .iter()
        .map(|c| {
            let k = match &c.kind {
                ChangeKind::Added { .. } => "A",
                ChangeKind::Deleted { .. } => "D",
                ChangeKind::Modified { .. } => "M",
                ChangeKind::Renamed { .. } => "R",
            };
            (c.path.to_string(), k)
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("added.txt".to_string(), "A"),
            ("gone.txt".to_string(), "D"),
            ("mod.txt".to_string(), "M"),
            ("new_name.rs".to_string(), "R"),
        ]
    );
    let ChangeKind::Renamed { from, old, new } = &changes[3].kind else {
        panic!("expected rename");
    };
    assert_eq!(from.as_str(), "old_name.rs");
    assert_eq!(old, new, "identical content keeps the same blob");
    assert_eq!(
        repo.blob(*new).unwrap(),
        b"fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n"
    );
}

#[test]
fn tree_snapshot_is_flat_sorted_and_typed() {
    let t = RepoBuilder::new()
        .commit(
            "one",
            files!["src/lib.rs" => "x", "README.md" => "r", "src/bin/main.rs" => "m"],
        )
        .build()
        .unwrap();
    t.git(&["update-index", "--chmod=+x", "src/bin/main.rs"])
        .unwrap();
    t.git(&["commit", "-q", "-m", "exec"]).unwrap();
    let repo = Repo::open(t.path()).unwrap();
    let head = repo.resolve(&RefSpec::Head).unwrap();
    let snap = repo.tree_snapshot(RepoId::nil(), head.tree).unwrap();
    let paths: Vec<&str> = snap.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "README.md",
            "src",
            "src/bin",
            "src/bin/main.rs",
            "src/lib.rs"
        ]
    );
    assert!(matches!(snap.entries[1].kind, TreeEntryKind::Dir { .. }));
    assert!(matches!(
        snap.entries[3].kind,
        TreeEntryKind::File {
            executable: true,
            size: 1,
            ..
        }
    ));
    assert!(matches!(
        snap.entries[4].kind,
        TreeEntryKind::File {
            executable: false,
            size: 1,
            ..
        }
    ));
}

#[test]
fn working_tree_snapshot_reflects_unstaged_edits_untracked_and_deletes() {
    let t = RepoBuilder::new()
        .commit("one", files!["a.txt" => "a\n", "b.txt" => "b\n", "c.txt" => "c\n", ".gitignore" => "ignored.txt\n"])
        .write(files!["a.txt" => "changed\n", "new.txt" => "n\n", "ignored.txt" => "i\n"])
        .remove(&["c.txt"])
        .build()
        .unwrap();
    let repo = Repo::open(t.path()).unwrap();
    let wt = repo.resolve(&RefSpec::WorkingTree).unwrap();
    let ResolvedSource::WorkingTree { dirty } = &wt.source else {
        panic!("expected working tree");
    };
    let dirty: Vec<&str> = dirty.iter().map(moor_protocol::RepoPath::as_str).collect();
    assert_eq!(dirty, vec!["a.txt", "c.txt", "new.txt"]);

    let snap = repo.tree_snapshot(RepoId::nil(), wt.tree).unwrap();
    let paths: Vec<&str> = snap.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, vec![".gitignore", "a.txt", "b.txt", "new.txt"]);
    let TreeEntryKind::File { oid, .. } = snap.entries[1].kind else {
        panic!()
    };
    assert_eq!(repo.blob(oid).unwrap(), b"changed\n");

    // The real index was not touched.
    assert_eq!(t.git(&["diff", "--cached", "--name-only"]).unwrap(), "");
    // Snapshot is stable when nothing changed, and changes when content does.
    assert_eq!(repo.resolve(&RefSpec::WorkingTree).unwrap().tree, wt.tree);
    t.write_file("a.txt", b"again\n").unwrap();
    assert_ne!(repo.resolve(&RefSpec::WorkingTree).unwrap().tree, wt.tree);
}

#[test]
fn tree_delta_between_snapshots() {
    let t = RepoBuilder::new()
        .commit("one", files!["a.txt" => "a\n", "b.txt" => "b\n"])
        .build()
        .unwrap();
    let repo = Repo::open(t.path()).unwrap();
    let before = repo.resolve(&RefSpec::WorkingTree).unwrap().tree;
    t.write_file("a.txt", b"a2\n").unwrap();
    t.write_file("d/new.txt", b"n\n").unwrap();
    std::fs::remove_file(t.path().join("b.txt")).unwrap();
    let after = repo.resolve(&RefSpec::WorkingTree).unwrap().tree;
    let delta = repo.tree_delta(RepoId::nil(), before, after).unwrap();
    assert_eq!(
        delta
            .added
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        vec!["d/new.txt"]
    );
    assert_eq!(
        delta
            .removed
            .iter()
            .map(moor_protocol::RepoPath::as_str)
            .collect::<Vec<_>>(),
        vec!["b.txt"]
    );
    assert_eq!(
        delta
            .changed
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a.txt"]
    );
}

#[test]
fn commits_between_carries_full_message_and_signatures_incl_merges() {
    let t = RepoBuilder::new()
        .commit("base", files!["a.txt" => "a\n"])
        .branch("feature")
        .commit(
            "feat: add b\n\nLonger body here.\n\nSecond paragraph.",
            files!["b.txt" => "b\n"],
        )
        .checkout("main")
        .commit("main moves", files!["c.txt" => "c\n"])
        .build()
        .unwrap();
    t.git(&["merge", "--no-ff", "-q", "-m", "merge feature", "feature"])
        .unwrap();
    let repo = Repo::open(t.path()).unwrap();
    let base = repo.rev_parse_commit("feature~1").unwrap();
    let head = repo.rev_parse_commit("main").unwrap();
    let commits = repo.commits_between(base, head).unwrap();
    // Topo order: the merge first; the two sides are siblings and git may
    // emit either side first.
    assert_eq!(commits[0].subject, "merge feature");
    assert_eq!(commits[0].parents.len(), 2, "merge has two parents");
    let mut sides: Vec<&str> = commits[1..].iter().map(|c| c.subject.as_str()).collect();
    sides.sort_unstable();
    assert_eq!(sides, vec!["feat: add b", "main moves"]);
    let feat = commits.iter().find(|c| c.subject == "feat: add b").unwrap();
    assert_eq!(feat.body, "Longer body here.\n\nSecond paragraph.");
    assert_eq!(feat.author.name, "Test User");
    assert_eq!(feat.author.email, "test@example.com");
    assert_eq!(feat.author.time.millis(), 1_704_067_200_000);
    assert_eq!(feat.author.offset_minutes, 0);
    assert_eq!(
        feat.tree.to_string(),
        t.git(&["rev-parse", "feature^{tree}"]).unwrap()
    );
}

#[test]
fn binary_detection_and_blob_read() {
    assert!(is_binary(b"abc\0def"));
    assert!(!is_binary(b"plain text\n"));
    let t = RepoBuilder::new()
        .commit("bin", files!["img.png" => b"\x89PNG\0\0\x1a".as_slice()])
        .build()
        .unwrap();
    let repo = Repo::open(t.path()).unwrap();
    let head = repo.resolve(&RefSpec::Head).unwrap();
    let changes = repo
        .changed_files(
            TreeOid::new("4b825dc642cb6eb9a060e54bf8d69288fbee4904".parse().unwrap()),
            head.tree,
        )
        .unwrap();
    let ChangeKind::Added { new } = changes[0].kind else {
        panic!()
    };
    assert!(is_binary(&repo.blob(new).unwrap()));
}
