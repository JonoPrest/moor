//! Plan 3.5 keyboard model: every action is reachable from a key, the
//! table has no conflicts, sequences resolve and expire, `?` always helps,
//! and focus drives the viewport.

// Scenario tests and their fixture read top to bottom.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeSet;

use moor_client_core::{
    Action, ActionKind, CacheConfig, ClientCore, Command, Config, Context, CoreError, Effect,
    Focus, IdSeed, Input, KeyChord, Keymap, NamedKey, NoTarget, SEQ_TIMEOUT_MS, TransportEvent,
};
use moor_protocol::{
    Anchor, Author, BuildInfo, ChangeKind, ChunkIndex, ClientId, ClientMsg, Comment, CommentId,
    CommentKind, CommentState, FileChange, FileRenderHeader, NonEmpty, Oid, ProtocolVersion,
    RefSpec, RenderChunk, RenderContent, RenderOpts, RenderTarget, RepoId, RepoPath, Request,
    Review, ReviewId, ReviewSnapshot, ReviewStatus, ReviewTarget, Row, SchemaVersion, Seq,
    ServerMsg, StreamItem, Timestamp, TreeEntry, TreeEntryKind, TreeOid, TreeSnapshot, ViewSection,
    WorkspaceId,
};
use strum::IntoEnumIterator;

fn repo_id() -> RepoId {
    RepoId::from_parts(2, 2)
}

fn review_id() -> ReviewId {
    ReviewId::from_parts(4, 1)
}

fn path(p: &str) -> RepoPath {
    RepoPath::new(p).unwrap()
}

fn blob(fill: u8) -> moor_protocol::BlobOid {
    moor_protocol::BlobOid::new(Oid::from_bytes([fill; 20]))
}

fn review() -> Review {
    Review {
        id: review_id(),
        workspace_id: WorkspaceId::from_parts(3, 3),
        title: "a review".into(),
        targets: NonEmpty::singleton(ReviewTarget {
            repo_id: repo_id(),
            base: RefSpec::Branch {
                name: "main".into(),
            },
            head: RefSpec::Head,
        }),
        created: Timestamp::from_millis(0),
        status: ReviewStatus::Open,
    }
}

fn comment(n: u128, anchor: Anchor) -> Comment {
    let id = CommentId::from_parts(5, n);
    Comment {
        id,
        review_id: review_id(),
        thread_id: moor_client_core::thread_id_of(id),
        author: Author::Human {
            name: "other".into(),
            machine: "host".into(),
        },
        kind: CommentKind::Note,
        anchor,
        body: format!("c{n}"),
        created: Timestamp::from_millis(i64::try_from(n).unwrap()),
        edited: None,
        state: CommentState::Live,
        context: None,
    }
}

/// Snapshot with one thread on line 5 (head) of a.rs and one review-level.
fn snapshot() -> ReviewSnapshot {
    use moor_protocol::{ContextHash, LineNo, LineRange, Side};
    let c1 = comment(
        1,
        Anchor::Lines {
            repo_id: repo_id(),
            path: path("src/a.rs"),
            side: Side::Head,
            blob_oid: blob(11),
            lines: LineRange::single(LineNo::new(5).unwrap()),
            context_hash: ContextHash::new(0),
        },
    );
    let c2 = comment(2, Anchor::Review);
    let mut s = ReviewSnapshot {
        review: review(),
        resolved: Some(NonEmpty::singleton(moor_protocol::ResolvedTarget {
            repo_id: repo_id(),
            base: moor_protocol::ResolvedRef {
                tree: TreeOid::from_bytes([1; 20]),
                source: moor_protocol::ResolvedSource::Commit {
                    oid: moor_protocol::CommitOid::from_bytes([1; 20]),
                },
            },
            head: moor_protocol::ResolvedRef {
                tree: TreeOid::from_bytes([2; 20]),
                source: moor_protocol::ResolvedSource::Commit {
                    oid: moor_protocol::CommitOid::from_bytes([2; 20]),
                },
            },
        })),
        threads: Vec::new(),
        comments: Vec::new(),
        viewed: Vec::new(),
        seq: Seq::new(1),
    };
    let meta = moor_client_core::EventMeta {
        author: c1.author.clone(),
        ts: Timestamp::from_millis(0),
    };
    for c in [c1, c2] {
        moor_client_core::apply_body(
            &mut s,
            &meta,
            &moor_protocol::EventBody::CommentCreated { comment: c },
        );
    }
    s
}

fn header(p: &str) -> FileRenderHeader {
    FileRenderHeader {
        repo_id: repo_id(),
        path: path(p),
        target: RenderTarget::Diff {
            change: ChangeKind::Modified {
                old: blob(10),
                new: blob(11),
            },
        },
        opts: RenderOpts::default(),
        lang: None,
        content: RenderContent::Text {
            total_rows: 300,
            chunk_rows: 100,
            chunk_count: 3,
            highlighted: false,
            additions: 1,
            deletions: 1,
        },
    }
}

fn chunk(index: u32) -> RenderChunk {
    use moor_protocol::{Cell, LineNo};
    let cell = |n: u32| Cell {
        line_no: LineNo::new(n).unwrap(),
        text: format!("l{n}"),
        spans: Vec::new(),
        changed: Vec::new(),
    };
    RenderChunk {
        index: ChunkIndex::new(index),
        rows: (0..100)
            .map(|i| {
                let n = index * 100 + i + 1;
                if n == 1 || n == 150 {
                    Row::HunkHeader {
                        text: format!("@@ {n} @@"),
                    }
                } else {
                    Row::Context {
                        left: cell(n),
                        right: cell(n),
                    }
                }
            })
            .collect(),
    }
}

fn requests(effects: &[Effect]) -> Vec<Request> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Send(ClientMsg::Request { request, .. }) => Some(request.clone()),
            Effect::Send(ClientMsg::Hello { .. } | ClientMsg::Cancel { .. })
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Persist { .. }
            | Effect::Load { .. }
            | Effect::Remove { .. } => None,
        })
        .collect()
}

fn rendered(effects: &[Effect]) -> Vec<ViewSection> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Render(d) => Some(d.sections.clone()),
            Effect::Send(_)
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Load { .. }
            | Effect::Persist { .. }
            | Effect::Remove { .. } => None,
        })
        .flatten()
        .collect()
}

/// A subscribed core with the review open, two files (src/a.rs, b.rs),
/// head tree cached, every chunk of a.rs cached, the file list known.
fn ready() -> ClientCore {
    let mut core = ClientCore::new(Config {
        client_id: ClientId::from_parts(1, 1),
        client: BuildInfo {
            name: "test".into(),
            version: "0".into(),
        },
        author: Author::Human {
            name: "someone".into(),
            machine: "host".into(),
        },
        id_seed: IdSeed(7),
        cache: CacheConfig::default(),
    });
    core.handle(Input::User(Action::Connect)).unwrap();
    core.handle(Input::Transport(TransportEvent::Connected))
        .unwrap();
    let effects = core
        .handle(Input::Server(ServerMsg::Welcome {
            protocol: ProtocolVersion::CURRENT,
            daemon: BuildInfo {
                name: "moord".into(),
                version: "0".into(),
            },
            schema: SchemaVersion::CURRENT,
            upgrade: None,
        }))
        .unwrap();
    let id = effects
        .iter()
        .find_map(|e| match e {
            Effect::Send(ClientMsg::Request { id, .. }) => Some(*id),
            Effect::Send(ClientMsg::Hello { .. } | ClientMsg::Cancel { .. })
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Persist { .. }
            | Effect::Load { .. }
            | Effect::Remove { .. } => None,
        })
        .unwrap();
    core.handle(Input::Server(ServerMsg::Response {
        id,
        response: moor_protocol::Response::Subscribed { seq: Seq::new(1) },
    }))
    .unwrap();
    core.handle(Input::Server(ServerMsg::Event {
        event: moor_protocol::Event {
            seq: Seq::new(2),
            ts: Timestamp::from_millis(0),
            author: Author::Human {
                name: "other".into(),
                machine: "host".into(),
            },
            client_id: ClientId::from_parts(9, 9),
            client_seq: moor_protocol::ClientSeq::new(1),
            body: moor_protocol::EventBody::ReviewCreated { review: review() },
        },
    }))
    .unwrap();
    let effects = core
        .handle(Input::User(Action::OpenReview {
            review_id: review_id(),
        }))
        .unwrap();
    let id = effects
        .iter()
        .find_map(|e| match e {
            Effect::Send(ClientMsg::Request { id, .. }) => Some(*id),
            Effect::Send(ClientMsg::Hello { .. } | ClientMsg::Cancel { .. })
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Persist { .. }
            | Effect::Load { .. }
            | Effect::Remove { .. } => None,
        })
        .unwrap();
    let item = |core: &mut ClientCore, item: StreamItem| {
        core.handle(Input::Server(ServerMsg::StreamItem { id, item }))
            .unwrap();
    };
    item(
        &mut core,
        StreamItem::ReviewSnapshot {
            snapshot: snapshot(),
        },
    );
    let entry = |p: &str| TreeEntry {
        path: path(p),
        kind: TreeEntryKind::File {
            oid: blob(11),
            size: 1,
            executable: false,
        },
    };
    item(
        &mut core,
        StreamItem::TreeSnapshot {
            snapshot: TreeSnapshot {
                repo_id: repo_id(),
                root_oid: TreeOid::from_bytes([2; 20]),
                entries: vec![entry("b.rs"), entry("src/a.rs")],
            },
        },
    );
    for p in ["src/a.rs", "b.rs"] {
        item(&mut core, StreamItem::Header { header: header(p) });
    }
    for c in 0..3 {
        item(
            &mut core,
            StreamItem::Chunk {
                repo_id: repo_id(),
                path: path("src/a.rs"),
                chunk: chunk(c),
            },
        );
    }
    core.handle(Input::Server(ServerMsg::StreamEnd { id }))
        .unwrap();
    let _ = FileChange {
        repo_id: repo_id(),
        path: path("x"),
        kind: ChangeKind::Added { new: blob(1) },
    };
    core
}

fn press(core: &mut ClientCore, keys: &str) -> Result<Vec<Effect>, CoreError> {
    let seq: moor_client_core::KeySeq = keys.parse().unwrap();
    let mut out = Vec::new();
    for c in seq.chords() {
        out.extend(core.handle(Input::Key(*c))?);
    }
    Ok(out)
}

#[test]
fn every_action_is_reachable_from_a_binding() {
    // Actions whose payload only the host can supply (text, a workspace
    // choice, a scroll position — though `Open` also produces Viewport).
    let host_only: BTreeSet<ActionKind> = [
        ActionKind::DraftSubmitted,
        ActionKind::Reply,        // `r` opens a reply draft; the text is host's
        ActionKind::EditComment,  // edit text is host's
        ActionKind::CreateReview, // title and targets come from a form
        ActionKind::ListReviews,
        ActionKind::FileSearch,   // produced (with an empty query) — see below
        ActionKind::SetBrowseRef, // the ref text comes from the Browse picker
        ActionKind::RunCommand,   // the actions palette picks the command
        ActionKind::CommentLines, // mouse drag across lines
        ActionKind::CommentFile,  // the file header's comment button
        ActionKind::SearchStep,   // search inputs forward Down/Up
    ]
    .into_iter()
    .collect();
    let mut reached: BTreeSet<ActionKind> = BTreeSet::new();
    let map = Keymap::default_table();
    // Every command has a binding somewhere.
    for cmd in Command::iter() {
        assert!(
            map.bindings().iter().any(|b| b.command == cmd),
            "{cmd} is not bound"
        );
    }
    // Drive the core through states where each command means something.
    let mut core = ready();
    let record = |core: &mut ClientCore, keys: &str| {
        if let Ok(effects) = press(core, keys) {
            let _ = effects;
        }
    };
    // Tree focus.
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Tree { index: 0 },
    }))
    .unwrap();
    for keys in [
        "enter", "j", "k", "G", "g g", "v", "v", "c", "] f", "[ f", "ctrl+p", "esc",
    ] {
        record(&mut core, keys);
    }
    // Diff focus (after `enter` on a file).
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Tree { index: 0 },
    }))
    .unwrap();
    // The record loop's `enter` collapsed the (default-open) root;
    // reopen it, then walk: root, src (a.rs), b.rs.
    press(&mut core, "enter").unwrap();
    press(&mut core, "j").unwrap(); // src dir
    press(&mut core, "j").unwrap(); // src/a.rs
    let names: Vec<String> = moor_client_core::visible_nodes(core.view())
        .iter()
        .map(|n| match n {
            moor_client_core::TreeNode::Dir { name, .. }
            | moor_client_core::TreeNode::File { name, .. } => name.clone(),
        })
        .collect();
    assert_eq!(names[1..], ["src", "a.rs", "b.rs"]);
    press(&mut core, "enter").unwrap(); // open
    assert!(matches!(core.view().focus, Focus::Diff { .. }));
    for keys in [
        "j", "k", "ctrl+d", "ctrl+u", "G", "g g", "n", "p", "] c", "[ c", "] f", "[ f", "s", "v",
        "tab",
    ] {
        record(&mut core, keys);
    }
    // Comment from a diff row, then discard.
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Diff { row: 4 },
    }))
    .unwrap();
    press(&mut core, "c").unwrap();
    assert_eq!(core.view().focus, Focus::Composer);
    press(&mut core, "esc").unwrap();
    // Thread focus: open, reply, resolve.
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Thread { index: 0 },
    }))
    .unwrap();
    for keys in ["enter", "x", "x", "r", "esc", "j", "k", "?", "?", "esc"] {
        record(&mut core, keys);
    }
    // Whitespace toggle re-keys every render (file list refetched), so last.
    press(&mut core, "w").unwrap();
    press(&mut core, "ctrl+shift+d").ok();
    press(&mut core, "ctrl+shift+c").ok();
    // Collect what every command resolves to in a few representative
    // states rather than what happened to run: resolution is the contract.
    let mut states: Vec<ClientCore> = Vec::new();
    let base = ready();
    for focus in [
        Focus::ReviewList { index: 0 },
        Focus::Tree { index: 0 },
        Focus::Thread { index: 0 },
        Focus::Help,
    ] {
        let mut c = ready();
        let _ = c.handle(Input::User(Action::SetFocus { focus }));
        states.push(c);
    }
    let mut with_file = ready();
    with_file
        .handle(Input::User(Action::Viewport {
            file: moor_client_core::FileRef {
                repo_id: repo_id(),
                path: path("src/a.rs"),
            },
            first_row: 0,
            last_row: 59,
        }))
        .unwrap();
    with_file
        .handle(Input::User(Action::SetFocus {
            focus: Focus::Diff { row: 4 },
        }))
        .unwrap();
    states.push(with_file);
    // Visual mode on: `esc` (Back) and `V` resolve to LeaveVisual.
    let mut with_visual = ready();
    with_visual
        .handle(Input::User(Action::Viewport {
            file: moor_client_core::FileRef {
                repo_id: repo_id(),
                path: path("src/a.rs"),
            },
            first_row: 0,
            last_row: 59,
        }))
        .unwrap();
    with_visual
        .handle(Input::User(Action::SetFocus {
            focus: Focus::Diff { row: 4 },
        }))
        .unwrap();
    press(&mut with_visual, "V").unwrap();
    states.push(with_visual);
    let mut with_draft = ready();
    with_draft
        .handle(Input::User(Action::DraftOpened {
            anchor: Anchor::Review,
        }))
        .unwrap();
    states.push(with_draft);
    let mut with_stepper = ready();
    let effects = with_stepper
        .handle(Input::User(Action::ListCommits { repo_id: repo_id() }))
        .unwrap();
    let id = effects
        .iter()
        .find_map(|e| match e {
            Effect::Send(ClientMsg::Request { id, .. }) => Some(*id),
            Effect::Send(ClientMsg::Hello { .. } | ClientMsg::Cancel { .. })
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Persist { .. }
            | Effect::Load { .. }
            | Effect::Remove { .. } => None,
        })
        .unwrap();
    let sig = moor_protocol::Sig {
        name: "ada".into(),
        email: "ada@example.com".into(),
        time: Timestamp::from_millis(5),
        offset_minutes: 0,
    };
    with_stepper
        .handle(Input::Server(ServerMsg::Response {
            id,
            response: moor_protocol::Response::Commits {
                commits: vec![moor_protocol::CommitInfo {
                    oid: moor_protocol::CommitOid::from_bytes([2; 20]),
                    parents: Vec::new(),
                    tree: TreeOid::from_bytes([2; 20]),
                    author: sig.clone(),
                    committer: sig,
                    subject: "one".into(),
                    body: String::new(),
                }],
            },
        }))
        .unwrap();
    with_stepper
        .handle(Input::User(Action::SetFocus {
            focus: Focus::CommitStepper { index: 0 },
        }))
        .unwrap();
    states.push(with_stepper);
    // A resolved thread of our own, and a viewed file: the toggles' other
    // halves.
    let mut toggled = ready();
    let mine = Author::Human {
        name: "someone".into(),
        machine: "host".into(),
    };
    let own = Comment {
        author: mine.clone(),
        ..comment(9, Anchor::Review)
    };
    let thread_id = own.thread_id;
    let bodies = [
        moor_protocol::EventBody::CommentCreated { comment: own },
        moor_protocol::EventBody::ThreadResolved {
            review_id: review_id(),
            thread_id,
        },
        moor_protocol::EventBody::FileViewed {
            review_id: review_id(),
            repo_id: repo_id(),
            path: path("src/a.rs"),
            viewer: mine.as_human().unwrap(),
            blob_oid: Some(blob(11)),
        },
    ];
    for (i, body) in bodies.into_iter().enumerate() {
        let seq = 3 + u64::try_from(i).unwrap();
        toggled
            .handle(Input::Server(ServerMsg::Event {
                event: moor_protocol::Event {
                    seq: Seq::new(seq),
                    ts: Timestamp::from_millis(0),
                    author: mine.clone(),
                    client_id: ClientId::from_parts(9, 9),
                    client_seq: moor_protocol::ClientSeq::new(1),
                    body,
                },
            }))
            .unwrap();
    }
    let index = toggled
        .view()
        .threads
        .iter()
        .position(|t| t.id == thread_id)
        .unwrap();
    toggled
        .handle(Input::User(Action::SetFocus {
            focus: Focus::Thread { index },
        }))
        .unwrap();
    states.push(toggled);
    // A suggestion thread, focused: `a` applies it.
    let mut with_suggestion = ready();
    let suggestion = Comment {
        kind: CommentKind::Suggestion {
            patch: "@@ -1 +1 @@\n-a\n+b\n".into(),
        },
        ..comment(11, Anchor::Review)
    };
    let suggestion_thread = suggestion.thread_id;
    with_suggestion
        .handle(Input::Server(ServerMsg::Event {
            event: moor_protocol::Event {
                seq: Seq::new(3),
                ts: Timestamp::from_millis(0),
                author: Author::Human {
                    name: "other".into(),
                    machine: "host".into(),
                },
                client_id: ClientId::from_parts(9, 9),
                client_seq: moor_protocol::ClientSeq::new(1),
                body: moor_protocol::EventBody::CommentCreated {
                    comment: suggestion,
                },
            },
        }))
        .unwrap();
    let index = with_suggestion
        .view()
        .threads
        .iter()
        .position(|t| t.id == suggestion_thread)
        .unwrap();
    with_suggestion
        .handle(Input::User(Action::SetFocus {
            focus: Focus::Thread { index },
        }))
        .unwrap();
    assert!(with_suggestion.view().threads[index].suggestion);
    states.push(with_suggestion);
    let mut viewed = ready();
    viewed
        .handle(Input::User(Action::Viewport {
            file: moor_client_core::FileRef {
                repo_id: repo_id(),
                path: path("src/a.rs"),
            },
            first_row: 0,
            last_row: 59,
        }))
        .unwrap();
    viewed
        .handle(Input::User(Action::MarkViewed {
            file: moor_client_core::FileRef {
                repo_id: repo_id(),
                path: path("src/a.rs"),
            },
        }))
        .unwrap();
    states.push(viewed);
    let mut with_search = ready();
    with_search
        .handle(Input::User(Action::FileSearch {
            query: Some("a".into()),
        }))
        .unwrap();
    states.push(with_search);
    // A thread whose root is outdated with a recorded context: `enter`
    // jumps to the original diff (OpenOriginalDiff).
    let mut with_outdated = ready();
    let anchor = Anchor::Lines {
        repo_id: repo_id(),
        path: path("src/a.rs"),
        side: moor_protocol::Side::Head,
        blob_oid: blob(4),
        lines: moor_protocol::LineRange::single(moor_protocol::LineNo::new(2).unwrap()),
        context_hash: moor_protocol::ContextHash::new(0),
    };
    let outdated = moor_protocol::Comment {
        state: CommentState::Outdated {
            last_good_anchor: anchor.clone(),
        },
        context: Some(ChangeKind::Modified {
            old: blob(3),
            new: blob(4),
        }),
        anchor,
        ..comment(12, Anchor::Review)
    };
    let outdated_thread = outdated.thread_id;
    with_outdated
        .handle(Input::Server(ServerMsg::Event {
            event: moor_protocol::Event {
                seq: Seq::new(3),
                ts: Timestamp::from_millis(0),
                author: Author::Human {
                    name: "other".into(),
                    machine: "host".into(),
                },
                client_id: ClientId::from_parts(9, 9),
                client_seq: moor_protocol::ClientSeq::new(1),
                body: moor_protocol::EventBody::CommentCreated { comment: outdated },
            },
        }))
        .unwrap();
    let index = with_outdated
        .view()
        .threads
        .iter()
        .position(|t| t.id == outdated_thread)
        .unwrap();
    with_outdated
        .handle(Input::User(Action::SetFocus {
            focus: Focus::Thread { index },
        }))
        .unwrap();
    assert!(with_outdated.view().threads[index].outdated);
    states.push(with_outdated);
    states.push(base);
    for state in &states {
        for cmd in Command::iter() {
            if let Ok(action) = moor_client_core::resolve_command(state, cmd) {
                reached.insert(ActionKind::from(&action));
            }
        }
    }
    let all: BTreeSet<ActionKind> = ActionKind::iter().collect();
    let missing: Vec<ActionKind> = all
        .difference(&reached)
        .filter(|k| !host_only.contains(k))
        .copied()
        .collect();
    assert!(missing.is_empty(), "unreachable from any key: {missing:?}");
    // FileSearch is reachable too (Global ctrl+p) — listed above only so the
    // allowlist documents that its *query* is host text.
    assert!(reached.contains(&ActionKind::FileSearch));
}

#[test]
fn sequences_resolve_and_expire() {
    let mut core = ready();
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Tree { index: 0 },
    }))
    .unwrap();
    // Diffing mode auto-expands: root, src (with a.rs), b.rs are visible.
    press(&mut core, "G").unwrap();
    assert_eq!(core.view().focus, Focus::Tree { index: 3 });
    // `g` alone waits for the rest; the hint bar switches to the group.
    let effects = core.handle(Input::Key(KeyChord::char('g'))).unwrap();
    assert_eq!(rendered(&effects), vec![ViewSection::Hints]);
    assert_eq!(core.view().pending_keys, "g");
    assert!(!core.view().hints.is_empty());
    assert_eq!(core.pending_chords().len(), 1);
    core.handle(Input::Tick(100)).unwrap();
    let effects = core.handle(Input::Key(KeyChord::char('g'))).unwrap();
    assert_eq!(
        rendered(&effects),
        vec![ViewSection::Focus, ViewSection::Hints]
    );
    assert_eq!(core.view().pending_keys, "");
    assert_eq!(core.view().focus, Focus::Tree { index: 0 });
    assert!(core.pending_chords().is_empty());
    // A pending prefix never expires on the clock (vim-like): it waits.
    core.handle(Input::Key(KeyChord::char('g'))).unwrap();
    core.handle(Input::Tick(100 + SEQ_TIMEOUT_MS)).unwrap();
    assert_eq!(core.pending_chords().len(), 1);
    // A key outside the group (esc included) cancels the sequence
    // silently and re-renders the hints.
    let effects = core.handle(Input::Key(KeyChord::char('q'))).unwrap();
    assert!(core.pending_chords().is_empty());
    assert_eq!(rendered(&effects), vec![ViewSection::Hints]);
    // A single unbound key is still a typed error.
    assert_eq!(
        core.handle(Input::Key(KeyChord::char('q'))),
        Err(CoreError::UnboundKey("q".into()))
    );
    // At an edge, movement is a typed no-op.
    assert_eq!(
        core.handle(Input::Key(KeyChord::char('k'))),
        Err(CoreError::NoTarget(NoTarget::AtEdge))
    );
    // In the composer, unbound keys are text for the host: accepted, ignored.
    core.handle(Input::User(Action::DraftOpened {
        anchor: Anchor::Review,
    }))
    .unwrap();
    assert_eq!(core.view().focus, Focus::Composer);
    assert!(
        core.handle(Input::Key(KeyChord::char('j')))
            .unwrap()
            .is_empty()
    );
    // Esc discards and focus returns to the tree.
    press(&mut core, "esc").unwrap();
    assert!(core.view().draft.is_none());
    assert_eq!(core.view().focus, Focus::Tree { index: 0 });
}

#[test]
fn diff_focus_scrolls_the_viewport_and_navigates_hunks_and_comments() {
    let mut core = ready();
    core.handle(Input::User(Action::Viewport {
        file: moor_client_core::FileRef {
            repo_id: repo_id(),
            path: path("src/a.rs"),
        },
        first_row: 0,
        last_row: 59,
    }))
    .unwrap();
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Diff { row: 0 },
    }))
    .unwrap();
    // Page down twice: the second leaves the window, which follows.
    press(&mut core, "ctrl+d").unwrap();
    assert_eq!(core.view().focus, Focus::Diff { row: 60 });
    let f = core
        .view()
        .review
        .as_ref()
        .unwrap()
        .open_file
        .clone()
        .unwrap();
    assert!(f.first_row <= 60 && 60 <= f.last_row);
    // Every chunk is cached: no requests, just renders.
    let effects = press(&mut core, "ctrl+d").unwrap();
    assert!(requests(&effects).is_empty());
    assert_eq!(core.view().focus, Focus::Diff { row: 120 });
    // Next hunk from the top is the header at row 149; previous is row 0.
    press(&mut core, "g g").unwrap();
    press(&mut core, "n").unwrap();
    assert_eq!(core.view().focus, Focus::Diff { row: 149 });
    press(&mut core, "p").unwrap();
    assert_eq!(core.view().focus, Focus::Diff { row: 0 });
    // Next comment is the thread on line 5 (row 4); Enter focuses it.
    press(&mut core, "] c").unwrap();
    assert_eq!(core.view().focus, Focus::Diff { row: 4 });
    press(&mut core, "enter").unwrap();
    assert_eq!(core.view().focus, Focus::Thread { index: 0 });
    // Enter on the thread goes back to its row; Esc closes the file.
    press(&mut core, "enter").unwrap();
    assert!(matches!(core.view().focus, Focus::Diff { .. }));
    press(&mut core, "esc").unwrap();
    assert!(core.view().diff.is_none());
    assert_eq!(core.view().focus, Focus::Tree { index: 0 });
    // Esc again closes the review; focus lands on the review list.
    press(&mut core, "esc").unwrap();
    assert!(core.view().review.is_none());
    assert_eq!(core.view().focus, Focus::ReviewList { index: 0 });
}

#[test]
fn help_and_hints_follow_focus_and_help_is_never_empty() {
    let mut core = ready();
    // The auto-open focuses the diff; this test reads the tree's hints.
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Tree { index: 0 },
    }))
    .unwrap();
    for ctx in Context::iter() {
        assert!(
            !Keymap::default_table().help(ctx).groups[0]
                .entries
                .is_empty(),
            "{ctx}"
        );
    }
    let hints_tree = core.view().hints.clone();
    assert!(hints_tree.iter().any(|h| h.command == Command::Open));
    let effects = press(&mut core, "?").unwrap();
    assert!(rendered(&effects).contains(&ViewSection::Help));
    assert_eq!(core.view().focus, Focus::Help);
    let help = core.view().help.as_ref().unwrap();
    assert_eq!(help.groups[0].context, Context::Help);
    assert!(help.conflicts.is_empty());
    press(&mut core, "?").unwrap();
    assert!(core.view().help.is_none());
    assert_eq!(core.view().focus, Focus::Tree { index: 0 });
    // Hints changed with the context and back.
    assert_eq!(core.view().hints, hints_tree);
    // A keys config from the KV re-derives the hints: `open` rebinds to
    // `o` across normal mode.
    let mut normal = std::collections::BTreeMap::new();
    normal.insert("open".to_owned(), vec!["o".to_owned()]);
    let mut bindings = std::collections::BTreeMap::new();
    bindings.insert(moor_client_core::Mode::Normal, normal);
    let config = moor_client_core::KeysConfig {
        leader: None,
        bindings,
        groups: std::collections::BTreeMap::new(),
    };
    let effects = core
        .handle(Input::Stored {
            key: Keymap::KEY.into(),
            value: Some(serde_json::to_vec(&config).unwrap()),
        })
        .unwrap();
    assert_eq!(rendered(&effects), vec![ViewSection::Hints]);
    assert!(core.view().hints.iter().any(|h| h.keys == "o"));
    assert_eq!(
        core.handle(Input::Key(KeyChord::named(NamedKey::Enter))),
        Err(CoreError::UnboundKey("enter".into()))
    );
    // `o` on the root dir toggles it: diffing dirs default open, so it
    // collapses.
    press(&mut core, "o").unwrap();
    assert!(core.view().tree.roots.iter().all(|n| matches!(
        n,
        moor_client_core::TreeNode::Dir {
            expanded: false,
            ..
        }
    )));
}

#[test]
fn search_step_moves_the_highlighted_hit() {
    let mut core = ready();
    // File find: two hits for "rs"; Down steps, the selection clamps.
    core.handle(Input::User(Action::FileSearch {
        query: Some("rs".into()),
    }))
    .unwrap();
    let hits = core.view().tree.search.as_ref().unwrap().hits.len();
    assert!(hits >= 2, "expected 2+ hits, got {hits}");
    assert_eq!(core.view().tree.search.as_ref().unwrap().selected, 0);
    core.handle(Input::User(Action::SearchStep { delta: 1 }))
        .unwrap();
    assert_eq!(core.view().tree.search.as_ref().unwrap().selected, 1);
    core.handle(Input::User(Action::SearchStep { delta: 100 }))
        .unwrap();
    assert_eq!(core.view().tree.search.as_ref().unwrap().selected, hits - 1);
    core.handle(Input::User(Action::SearchStep { delta: -100 }))
        .unwrap();
    assert_eq!(core.view().tree.search.as_ref().unwrap().selected, 0);
    // A new query resets the selection.
    core.handle(Input::User(Action::SearchStep { delta: 1 }))
        .unwrap();
    core.handle(Input::User(Action::FileSearch {
        query: Some("a".into()),
    }))
    .unwrap();
    assert_eq!(core.view().tree.search.as_ref().unwrap().selected, 0);
    // No search open: stepping is rejected and changes nothing.
    core.handle(Input::User(Action::FileSearch { query: None }))
        .unwrap();
    assert!(
        core.handle(Input::User(Action::SearchStep { delta: 1 }))
            .is_err()
    );
}

#[test]
fn visual_mode_extends_a_selection_and_comments_on_it() {
    use moor_client_core::{Mode, VisualView};
    let mut core = ready();
    core.handle(Input::User(Action::Viewport {
        file: moor_client_core::FileRef {
            repo_id: repo_id(),
            path: path("src/a.rs"),
        },
        first_row: 0,
        last_row: 59,
    }))
    .unwrap();
    core.handle(Input::User(Action::SetFocus {
        focus: Focus::Diff { row: 4 },
    }))
    .unwrap();
    // `V` enters Visual on the focused row; motions extend the selection
    // from the anchor.
    press(&mut core, "V").unwrap();
    assert_eq!(core.view().mode, Mode::Visual);
    assert_eq!(core.view().visual, Some(VisualView { start: 4, end: 4 }));
    press(&mut core, "j").unwrap();
    assert_eq!(core.view().visual, Some(VisualView { start: 4, end: 5 }));
    press(&mut core, "k k").unwrap();
    assert_eq!(core.view().visual, Some(VisualView { start: 3, end: 4 }));
    // `c` opens a draft anchored to the selected line range and clears the
    // selection; the composer takes the keys (Insert).
    press(&mut core, "c").unwrap();
    assert_eq!(core.view().focus, Focus::Composer);
    assert_eq!(core.view().mode, Mode::Insert);
    assert_eq!(core.view().visual, None);
    let draft = core.view().draft.clone().unwrap();
    let Anchor::Lines { lines, .. } = draft.anchor else {
        panic!(
            "visual comment should anchor to lines, got {:?}",
            draft.anchor
        );
    };
    assert!(lines.start() <= lines.end());
    // Discard; `V` then `esc` leaves Visual without a draft.
    press(&mut core, "esc").unwrap();
    press(&mut core, "V").unwrap();
    assert_eq!(core.view().mode, Mode::Visual);
    press(&mut core, "esc").unwrap();
    assert_eq!(core.view().mode, Mode::Normal);
    assert_eq!(core.view().visual, None);
    assert!(core.view().draft.is_none());
}

#[test]
fn search_step_moves_the_content_search_selection() {
    let mut core = ready();
    let effects = core
        .handle(Input::User(Action::ContentSearch {
            query: Some("l1".into()),
            all_files: false,
        }))
        .unwrap();
    let id = effects
        .iter()
        .find_map(|e| match e {
            Effect::Send(ClientMsg::Request { id, .. }) => Some(*id),
            _ => None,
        })
        .unwrap();
    let hit = |line: u32| moor_protocol::ContentHit {
        repo_id: repo_id(),
        path: path("src/a.rs"),
        line: moor_protocol::LineNo::new(line).unwrap(),
        text: "l1".into(),
    };
    core.handle(Input::Server(ServerMsg::Response {
        id,
        response: moor_protocol::Response::Search {
            hits: vec![hit(1), hit(2), hit(3)],
            truncated: false,
        },
    }))
    .unwrap();
    assert_eq!(core.view().content_search.as_ref().unwrap().selected, 0);
    let effects = core
        .handle(Input::User(Action::SearchStep { delta: 1 }))
        .unwrap();
    assert_eq!(core.view().content_search.as_ref().unwrap().selected, 1);
    assert_eq!(rendered(&effects), vec![ViewSection::Search]);
}
