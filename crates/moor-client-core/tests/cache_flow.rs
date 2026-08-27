//! Plan 3.2: content flows memory → disk → daemon, with the exact effects
//! each tier produces; pins, budgets, viewport prefetch and restart.

// Scenario tests read top to bottom; splitting them would hide the flow.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;

use moor_client_core::{
    Action, Bytes, CacheConfig, CacheKey, CacheValue, ClientCore, Config, ConnectionView,
    CoreError, DiskTier, Effect, FileRef, IdSeed, Input, PREFETCH_RADIUS, RenderKey,
    TransportEvent,
};
use moor_protocol::{
    Author, BlobOid, BuildInfo, ChangeKind, ChunkIndex, ClientId, ClientMsg, ClientSeq, CommitOid,
    Event, EventBody, FileChange, FileRenderHeader, NonEmpty, Oid, ProtocolVersion, RefSpec,
    RenderChunk, RenderContent, RenderOpts, RenderTarget, RepoId, RepoPath, Request, RequestId,
    ResolvedRef, ResolvedSource, ResolvedTarget, Response, Review, ReviewId, ReviewSnapshot,
    ReviewStatus, ReviewTarget, Row, SchemaVersion, Seq, ServerMsg, StreamItem, Timestamp,
    TreeDelta, TreeEntry, TreeEntryKind, TreeOid, TreeSnapshot, ViewSection, WorkspaceId,
};

// ---- fixtures -----------------------------------------------------------

fn config(cache: CacheConfig) -> Config {
    Config {
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
        cache,
    }
}

fn local() -> CacheConfig {
    CacheConfig::default()
}

fn remote(memory: Bytes, disk: Bytes) -> CacheConfig {
    CacheConfig {
        memory_budget: memory,
        disk: DiskTier::Enabled { budget: disk },
        ..CacheConfig::default()
    }
}

fn repo_id() -> RepoId {
    RepoId::from_parts(2, 2)
}

fn review_id() -> ReviewId {
    ReviewId::from_parts(4, 1)
}

fn tree_oid(fill: u8) -> TreeOid {
    TreeOid::new(Oid::from_bytes([fill; 20]))
}

fn blob_oid(fill: u8) -> BlobOid {
    BlobOid::new(Oid::from_bytes([fill; 20]))
}

fn path(p: &str) -> RepoPath {
    RepoPath::new(p).unwrap()
}

fn resolved(base: u8, head: u8) -> NonEmpty<ResolvedTarget> {
    let commit = |fill: u8| ResolvedRef {
        tree: tree_oid(fill),
        source: ResolvedSource::Commit {
            oid: CommitOid::new(Oid::from_bytes([fill; 20])),
        },
    };
    NonEmpty::singleton(ResolvedTarget {
        repo_id: repo_id(),
        base: commit(base),
        head: commit(head),
    })
}

fn snapshot(base: u8, head: u8) -> ReviewSnapshot {
    ReviewSnapshot {
        review: Review {
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
        },
        resolved: Some(resolved(base, head)),
        threads: Vec::new(),
        comments: Vec::new(),
        viewed: Vec::new(),
        seq: Seq::new(1),
    }
}

fn tree(root: u8, files: &[&str]) -> TreeSnapshot {
    TreeSnapshot {
        repo_id: repo_id(),
        root_oid: tree_oid(root),
        entries: files
            .iter()
            .map(|p| TreeEntry {
                path: path(p),
                kind: TreeEntryKind::File {
                    oid: blob_oid(1),
                    size: 1,
                    executable: false,
                },
            })
            .collect(),
    }
}

fn change(p: &str) -> FileChange {
    FileChange {
        repo_id: repo_id(),
        path: path(p),
        kind: ChangeKind::Modified {
            old: blob_oid(10),
            new: blob_oid(11),
        },
    }
}

fn render_key(p: &str) -> RenderKey {
    RenderKey {
        repo_id: repo_id(),
        path: path(p),
        target: RenderTarget::Diff {
            change: change(p).kind,
        },
        opts: RenderOpts::default(),
    }
}

fn header(p: &str, chunk_rows: u32, chunk_count: u32) -> FileRenderHeader {
    FileRenderHeader {
        repo_id: repo_id(),
        path: path(p),
        target: RenderTarget::Diff {
            change: change(p).kind,
        },
        opts: RenderOpts::default(),
        lang: Some("rust".into()),
        content: RenderContent::Text {
            total_rows: chunk_rows * chunk_count,
            chunk_rows,
            chunk_count,
            highlighted: true,
            additions: 1,
            deletions: 1,
        },
    }
}

fn chunk(index: u32) -> RenderChunk {
    RenderChunk {
        index: ChunkIndex::new(index),
        rows: vec![Row::HunkHeader {
            text: format!("@@ chunk {index} @@"),
        }],
    }
}

fn chunk_key(p: &str, index: u32) -> CacheKey {
    CacheKey::Chunk {
        render: render_key(p),
        index: ChunkIndex::new(index),
    }
}

fn header_key(p: &str) -> CacheKey {
    CacheKey::Header {
        render: render_key(p),
    }
}

fn tree_key(fill: u8) -> CacheKey {
    CacheKey::Tree {
        root: tree_oid(fill),
    }
}

fn file(p: &str) -> FileRef {
    FileRef {
        repo_id: repo_id(),
        path: path(p),
    }
}

fn welcome() -> ServerMsg {
    ServerMsg::Welcome {
        protocol: ProtocolVersion::CURRENT,
        daemon: BuildInfo {
            name: "moord".into(),
            version: "0".into(),
        },
        schema: SchemaVersion::CURRENT,
        upgrade: None,
    }
}

// ---- effect helpers -------------------------------------------------------

fn requests(effects: &[Effect]) -> Vec<(RequestId, Request)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Send(ClientMsg::Request { id, request }) => Some((*id, request.clone())),
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

fn loads(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Load { key } => Some(key.clone()),
            Effect::Send(_)
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Persist { .. }
            | Effect::Remove { .. } => None,
        })
        .collect()
}

fn persists(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Persist { key, .. } => Some(key.clone()),
            Effect::Send(_)
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Load { .. }
            | Effect::Remove { .. } => None,
        })
        .collect()
}

fn removes(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Remove { key } => Some(key.clone()),
            Effect::Send(_)
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Load { .. }
            | Effect::Persist { .. } => None,
        })
        .collect()
}

fn cancels(effects: &[Effect]) -> Vec<RequestId> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Send(ClientMsg::Cancel { id }) => Some(*id),
            Effect::Send(ClientMsg::Hello { .. } | ClientMsg::Request { .. })
            | Effect::Connect
            | Effect::Disconnect
            | Effect::Render(_)
            | Effect::Load { .. }
            | Effect::Persist { .. }
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

fn is_content(r: &Request) -> bool {
    matches!(
        r,
        Request::TreeSnapshot { .. } | Request::FileRender { .. } | Request::RenderChunk { .. }
    )
}

/// The host's KV store: applies `Persist`/`Remove`, answers `Load`s.
#[derive(Default)]
struct Kv {
    map: BTreeMap<String, Vec<u8>>,
}

impl Kv {
    /// Apply storage effects and feed every `Load` answer back into the
    /// core, collecting everything the core emitted in response (recursively
    /// — an answer can trigger more loads).
    fn drive(&mut self, core: &mut ClientCore, effects: Vec<Effect>) -> Vec<Effect> {
        let mut out = Vec::new();
        let mut pending = effects;
        while !pending.is_empty() {
            let mut next = Vec::new();
            for e in pending {
                match &e {
                    Effect::Persist { key, value } => {
                        self.map.insert(key.clone(), value.clone());
                    }
                    Effect::Remove { key } => {
                        self.map.remove(key);
                    }
                    Effect::Load { key } => {
                        let value = self.map.get(key).cloned();
                        next.extend(
                            core.handle(Input::Stored {
                                key: key.clone(),
                                value,
                            })
                            .unwrap(),
                        );
                    }
                    Effect::Send(_) | Effect::Connect | Effect::Disconnect | Effect::Render(_) => {}
                }
                out.push(e);
            }
            pending = next;
        }
        out
    }
}

// ---- drivers --------------------------------------------------------------

fn subscribed(cache: CacheConfig) -> ClientCore {
    let mut core = ClientCore::new(config(cache));
    core.handle(Input::User(Action::Connect)).unwrap();
    core.handle(Input::Transport(TransportEvent::Connected))
        .unwrap();
    let effects = core.handle(Input::Server(welcome())).unwrap();
    let (id, _) = requests(&effects)[0].clone();
    core.handle(Input::Server(ServerMsg::Response {
        id,
        response: Response::Subscribed { seq: Seq::new(1) },
    }))
    .unwrap();
    assert_eq!(core.view().connection, ConnectionView::Subscribed);
    core
}

fn item(core: &mut ClientCore, id: RequestId, item: StreamItem) -> Vec<Effect> {
    core.handle(Input::Server(ServerMsg::StreamItem { id, item }))
        .unwrap()
}

/// Open a review over a local daemon: the full `OpenReview` stream with two
/// files, `a.rs` (10 chunks) and `b.rs` (1 chunk), first chunks included.
fn open_streamed(core: &mut ClientCore) -> Vec<Effect> {
    let effects = core
        .handle(Input::User(Action::OpenReview {
            review_id: review_id(),
        }))
        .unwrap();
    let (id, request) = requests(&effects)[0].clone();
    assert_eq!(
        request,
        Request::OpenReview {
            review_id: review_id(),
            opts: RenderOpts::default()
        }
    );
    let mut all = Vec::new();
    all.extend(item(
        core,
        id,
        StreamItem::ReviewSnapshot {
            snapshot: snapshot(1, 2),
        },
    ));
    all.extend(item(
        core,
        id,
        StreamItem::TreeSnapshot {
            snapshot: tree(1, &["a.rs", "b.rs"]),
        },
    ));
    all.extend(item(
        core,
        id,
        StreamItem::TreeSnapshot {
            snapshot: tree(2, &["a.rs", "b.rs", "c.rs"]),
        },
    ));
    for (p, count) in [("a.rs", 10), ("b.rs", 1)] {
        all.extend(item(
            core,
            id,
            StreamItem::Header {
                header: header(p, 100, count),
            },
        ));
        all.extend(item(
            core,
            id,
            StreamItem::Chunk {
                repo_id: repo_id(),
                path: path(p),
                chunk: chunk(0),
            },
        ));
    }
    all.extend(
        core.handle(Input::Server(ServerMsg::StreamEnd { id }))
            .unwrap(),
    );
    all
}

/// Answer every outstanding content request in `effects` from a "daemon"
/// that knows `a.rs` has 10 chunks; returns the effects those answers caused.
fn daemon_answers(core: &mut ClientCore, effects: &[Effect]) -> Vec<Effect> {
    let mut out = Vec::new();
    for (id, request) in requests(effects) {
        let msg = match request {
            Request::RenderChunk { index, .. } => Some(ServerMsg::Response {
                id,
                response: Response::RenderChunk {
                    chunk: chunk(index.get()),
                },
            }),
            Request::TreeSnapshot { .. }
            | Request::FileRender { .. }
            | Request::ListWorkspaces
            | Request::ListReviews { .. }
            | Request::GetReview { .. }
            | Request::ReviewSnapshot { .. }
            | Request::ListFiles { .. }
            | Request::OpenReview { .. }
            | Request::ResolveTargets { .. }
            | Request::ListCommits { .. }
            | Request::BlobRender { .. }
            | Request::Subscribe { .. }
            | Request::Unsubscribe { .. }
            | Request::Mutate { .. }
            | Request::Shutdown => None,
        };
        if let Some(msg) = msg
            && let Ok(effects) = core.handle(Input::Server(msg))
        {
            out.extend(effects);
        }
    }
    out
}

// ---- tests ----------------------------------------------------------------

#[test]
fn streamed_open_fills_and_pins_the_cache_and_renders_once_at_end() {
    let mut core = subscribed(local());
    let effects = open_streamed(&mut core);
    // Snapshot renders the review sections; the stream end renders tree+diff.
    assert_eq!(
        rendered(&effects),
        vec![
            ViewSection::Diff,
            ViewSection::Threads,
            ViewSection::Conversation,
            ViewSection::Draft,
            ViewSection::Tree, // tree 1
            ViewSection::Tree, // tree 2
            ViewSection::Tree, // header a.rs
            ViewSection::Progress,
            ViewSection::Tree, // header b.rs
            ViewSection::Progress,
            ViewSection::Tree, // stream end
            ViewSection::Diff,
        ]
    );
    // Nothing goes to the daemon or disk: the stream brought everything.
    assert!(requests(&effects).is_empty());
    assert!(loads(&effects).is_empty());
    assert!(persists(&effects).is_empty());
    let cache = core.cache();
    for key in [
        tree_key(1),
        tree_key(2),
        header_key("a.rs"),
        header_key("b.rs"),
    ] {
        assert!(cache.contains(&key), "{key:?} missing");
        assert!(cache.is_pinned(&key), "{key:?} not pinned");
    }
    assert!(cache.contains(&chunk_key("a.rs", 0)));
    assert!(!cache.is_pinned(&chunk_key("a.rs", 0)));
    let open = core.view().review.as_ref().unwrap();
    assert_eq!(open.trees, vec![tree_oid(1), tree_oid(2)]);
    assert_eq!(open.files, vec![render_key("a.rs"), render_key("b.rs")]);
}

#[test]
fn memory_hit_produces_no_effects_but_a_render() {
    let mut core = subscribed(local());
    open_streamed(&mut core);
    // b.rs has one chunk, already cached: the viewport is served locally.
    let effects = core
        .handle(Input::User(Action::Viewport {
            file: file("b.rs"),
            first_row: 0,
            last_row: 20,
        }))
        .unwrap();
    assert_eq!(
        effects,
        vec![Effect::Render(moor_client_core::ViewDelta::new(&[
            ViewSection::Diff,
            ViewSection::Focus
        ]))]
    );
    assert!(core.cache().is_pinned(&chunk_key("b.rs", 0)));
    // Closing the file releases the chunk pin but keeps the header pinned.
    let effects = core.handle(Input::User(Action::CloseFile)).unwrap();
    assert_eq!(
        rendered(&effects),
        vec![ViewSection::Diff, ViewSection::Focus]
    );
    assert!(!core.cache().is_pinned(&chunk_key("b.rs", 0)));
    assert!(core.cache().is_pinned(&header_key("b.rs")));
    assert_eq!(
        core.handle(Input::User(Action::CloseFile)),
        Err(CoreError::NoOpenFile)
    );
    assert_eq!(
        core.handle(Input::User(Action::Viewport {
            file: file("zzz.rs"),
            first_row: 0,
            last_row: 0
        })),
        Err(CoreError::UnknownFile(file("zzz.rs")))
    );
}

#[test]
fn viewport_requests_only_the_window_and_bounds_in_flight() {
    let mut core = subscribed(local());
    open_streamed(&mut core);
    let max = core.cache_config().max_in_flight;
    // Rows 500..600 of a.rs span chunks 5..=6; the window is 3..=8, nearest
    // to the first visible chunk first.
    let effects = core
        .handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 500,
            last_row: 600,
        }))
        .unwrap();
    let sent: Vec<u32> = requests(&effects)
        .into_iter()
        .filter_map(|(_, r)| match r {
            Request::RenderChunk { index, .. } => Some(index.get()),
            Request::TreeSnapshot { .. }
            | Request::FileRender { .. }
            | Request::ListWorkspaces
            | Request::ListReviews { .. }
            | Request::GetReview { .. }
            | Request::ReviewSnapshot { .. }
            | Request::ListFiles { .. }
            | Request::OpenReview { .. }
            | Request::ResolveTargets { .. }
            | Request::ListCommits { .. }
            | Request::BlobRender { .. }
            | Request::Subscribe { .. }
            | Request::Unsubscribe { .. }
            | Request::Mutate { .. }
            | Request::Shutdown => None,
        })
        .collect();
    assert_eq!(sent, vec![5, 4, 6, 3]);
    let all = effects.clone();
    assert_eq!(core.content_in_flight(), max);
    assert_eq!(core.content_queued(), 2); // chunks 7 and 8 wait for a slot
    assert!(loads(&effects).is_empty());

    // The user scrolls to the end before anything answers: window 7..=9
    // (clipped to the file). 7 and 8 stay queued, 9 joins; nothing else is
    // sent while the slots are full.
    let effects = core
        .handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 950,
            last_row: 999,
        }))
        .unwrap();
    assert!(requests(&effects).is_empty());
    assert_eq!(core.content_in_flight(), max);
    assert_eq!(core.content_queued(), 3);
    // Scrolling back to the top drops the queued far-away chunks (7, 8, 9)
    // before they are sent; 0 is cached, 1 and 2 queue.
    core.handle(Input::User(Action::Viewport {
        file: file("a.rs"),
        first_row: 0,
        last_row: 10,
    }))
    .unwrap();
    assert_eq!(core.content_queued(), 2);
    assert_eq!(core.content_in_flight(), max);

    // Answers free slots; the queue drains, never exceeding the cap.
    let mut outstanding = all;
    outstanding.extend(effects);
    let mut rounds = 0;
    while core.content_in_flight() > 0 {
        assert!(core.content_in_flight() <= max);
        outstanding = daemon_answers(&mut core, &outstanding);
        rounds += 1;
        assert!(rounds < 10, "did not drain");
    }
    assert_eq!(core.content_queued(), 0);
    for i in [0, 1, 2, 3, 4, 5, 6] {
        assert!(core.cache().contains(&chunk_key("a.rs", i)), "chunk {i}");
    }
    for i in [7, 8, 9] {
        assert!(!core.cache().contains(&chunk_key("a.rs", i)), "chunk {i}");
    }
    // Chunks of the open file are pinned.
    assert!(core.cache().is_pinned(&chunk_key("a.rs", 1)));
}

#[test]
fn viewport_before_header_streams_the_file_and_cancels_past_the_radius() {
    let mut core = subscribed(local());
    open_streamed(&mut core);
    // Forget a.rs's header as if it were evicted; the file is still listed.
    let mut core = {
        // No public eviction hook: rebuild with a tiny budget so the header
        // is evicted by later inserts instead.
        let _ = &mut core;
        subscribed(CacheConfig {
            memory_budget: Bytes(1),
            ..local()
        })
    };
    open_streamed(&mut core);
    // Pinned entries survive the 1-byte budget; the unpinned chunks did not.
    assert!(core.cache().is_pinned(&header_key("a.rs")));
    assert!(!core.cache().contains(&chunk_key("a.rs", 0)));
    // Unpin by closing the review: everything unpinned is evicted.
    core.handle(Input::User(Action::CloseReview)).unwrap();
    assert!(core.cache().is_empty());

    // Reopen with only the snapshot streamed, then look at a file.
    let effects = core
        .handle(Input::User(Action::OpenReview {
            review_id: review_id(),
        }))
        .unwrap();
    let (id, _) = requests(&effects)[0].clone();
    item(
        &mut core,
        id,
        StreamItem::ReviewSnapshot {
            snapshot: snapshot(1, 2),
        },
    );
    core.handle(Input::Server(ServerMsg::StreamEnd { id }))
        .unwrap();
    // No headers were streamed, so the file list is empty: refresh it.
    assert_eq!(
        core.handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 0,
            last_row: 10
        })),
        Err(CoreError::UnknownFile(file("a.rs")))
    );
    let effects = core
        .handle(Input::Server(ServerMsg::Event {
            event: Event {
                seq: Seq::new(2),
                ts: Timestamp::from_millis(0),
                author: Author::Human {
                    name: "other".into(),
                    machine: "host".into(),
                },
                client_id: ClientId::from_parts(9, 9),
                client_seq: ClientSeq::new(1),
                body: EventBody::ReviewTargetsResolved {
                    review_id: review_id(),
                    targets: resolved(1, 3),
                },
            },
        }))
        .unwrap();
    let reqs = requests(&effects);
    // New head: its tree is fetched, the base tree too (evicted), and the
    // file list is refreshed.
    assert!(
        reqs.iter()
            .any(|(_, r)| matches!(r, Request::TreeSnapshot { .. }))
    );
    let (files_id, _) = reqs
        .iter()
        .find(|(_, r)| matches!(r, Request::ListFiles { .. }))
        .cloned()
        .unwrap();
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id: files_id,
            response: Response::Files {
                files: vec![change("a.rs")],
            },
        }))
        .unwrap();
    // Header unknown → FileRender from chunk 0, stopping after chunk 0.
    let (render_id, request) = requests(&effects)
        .into_iter()
        .find(|(_, r)| matches!(r, Request::FileRender { .. }))
        .unwrap();
    assert_eq!(
        request,
        Request::FileRender {
            review_id: review_id(),
            repo_id: repo_id(),
            path: path("a.rs"),
            opts: RenderOpts::default(),
            first_chunk: ChunkIndex::FIRST,
        }
    );
    item(
        &mut core,
        render_id,
        StreamItem::Header {
            header: header("a.rs", 100, 10),
        },
    );
    let effects = item(
        &mut core,
        render_id,
        StreamItem::Chunk {
            repo_id: repo_id(),
            path: path("a.rs"),
            chunk: chunk(0),
        },
    );
    assert_eq!(cancels(&effects), vec![render_id]);
    // Items still arriving after the cancel are cached, not rejected.
    item(
        &mut core,
        render_id,
        StreamItem::Chunk {
            repo_id: repo_id(),
            path: path("a.rs"),
            chunk: chunk(1),
        },
    );
    core.handle(Input::Server(ServerMsg::StreamEnd { id: render_id }))
        .unwrap();
    // Only the two (unanswered) tree requests remain in flight.
    assert_eq!(core.content_in_flight(), 2);
    assert!(core.cache().is_pinned(&header_key("a.rs")));
    // The late chunk was accepted (no error above) but, unpinned under the
    // 1-byte budget, immediately evicted.
    assert!(!core.cache().contains(&chunk_key("a.rs", 1)));
}

#[test]
fn disk_tier_load_before_send_and_dedupes_concurrent_misses() {
    let mut kv = Kv::default();
    let mut core = subscribed(remote(Bytes::mib(1), Bytes::mib(1)));
    // Piecewise open: snapshot first.
    let effects = core
        .handle(Input::User(Action::OpenReview {
            review_id: review_id(),
        }))
        .unwrap();
    let (id, request) = requests(&effects)[0].clone();
    assert_eq!(
        request,
        Request::ReviewSnapshot {
            review_id: review_id()
        }
    );
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id,
            response: Response::ReviewSnapshot {
                snapshot: snapshot(1, 2),
            },
        }))
        .unwrap();
    // Trees: memory miss → exactly one Load each, no Send yet; ListFiles goes
    // out because the file list is not content-addressed.
    assert_eq!(
        loads(&effects),
        vec![tree_key(1).storage_key(), tree_key(2).storage_key()]
    );
    let reqs = requests(&effects);
    assert_eq!(reqs.len(), 1);
    assert!(matches!(reqs[0].1, Request::ListFiles { .. }));
    let files_id = reqs[0].0;

    // Disk answers: tree 1 is there, tree 2 is not → one Send for tree 2.
    kv.map.insert(
        tree_key(1).storage_key(),
        CacheValue::Tree {
            snapshot: tree(1, &["a.rs"]),
        }
        .encode(),
    );
    let effects = kv.drive(&mut core, effects);
    let reqs: Vec<_> = requests(&effects)
        .into_iter()
        .filter(|(_, r)| is_content(r))
        .collect();
    assert_eq!(reqs.len(), 1);
    assert_eq!(
        reqs[0].1,
        Request::TreeSnapshot {
            repo_id: repo_id(),
            ref_spec: RefSpec::Commit {
                oid: CommitOid::new(Oid::from_bytes([2; 20]))
            }
        }
    );
    assert!(core.cache().contains(&tree_key(1)));
    assert!(core.cache().is_pinned(&tree_key(1)));
    // Loading from disk does not write back to disk.
    assert!(persists(&effects).is_empty());

    // The daemon's answer is cached and written through.
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id: reqs[0].0,
            response: Response::TreeSnapshot {
                snapshot: tree(2, &["a.rs"]),
            },
        }))
        .unwrap();
    assert_eq!(persists(&effects), vec![tree_key(2).storage_key()]);
    kv.drive(&mut core, effects);

    // Files: header miss on both tiers → FileRender; chunk 0 then follows.
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id: files_id,
            response: Response::Files {
                files: vec![change("a.rs")],
            },
        }))
        .unwrap();
    assert_eq!(loads(&effects), vec![header_key("a.rs").storage_key()]);
    let effects = kv.drive(&mut core, effects);
    let (render_id, _) = requests(&effects)
        .into_iter()
        .find(|(_, r)| matches!(r, Request::FileRender { .. }))
        .unwrap();

    // Two viewports want chunk 5 before anything is known: the second is a
    // no-op on the fetch side (one outstanding request per key).
    let effects = item(
        &mut core,
        render_id,
        StreamItem::Header {
            header: header("a.rs", 100, 10),
        },
    );
    kv.drive(&mut core, effects);
    core.handle(Input::Server(ServerMsg::StreamEnd { id: render_id }))
        .unwrap();
    assert_eq!(core.content_in_flight(), 0);
    let first = core
        .handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 500,
            last_row: 510,
        }))
        .unwrap();
    let first_loads = loads(&first);
    assert_eq!(first_loads.len(), 5);
    assert!(requests(&first).is_empty());
    let second = core
        .handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 500,
            last_row: 510,
        }))
        .unwrap();
    assert!(loads(&second).is_empty());
    assert!(requests(&second).is_empty());
    // All disk misses: each becomes one daemon request, capped in flight.
    let effects = kv.drive(&mut core, first);
    let chunk_reqs: Vec<_> = requests(&effects)
        .into_iter()
        .filter(|(_, r)| matches!(r, Request::RenderChunk { .. }))
        .collect();
    assert_eq!(chunk_reqs.len(), core.cache_config().max_in_flight);
    assert_eq!(core.content_queued(), 1);
}

#[test]
fn eviction_respects_both_budgets_and_pins_survive() {
    let mut kv = Kv::default();
    // Memory: room for ~2 chunks; disk: room for ~4.
    let one = Bytes(CacheValue::Chunk { chunk: chunk(0) }.encode().len() as u64);
    let mut core = subscribed(remote(Bytes(one.get() * 2 + 1), Bytes(one.get() * 4 + 1)));
    // Reach a state with a.rs open (header known, chunks flowing).
    let effects = core
        .handle(Input::User(Action::OpenReview {
            review_id: review_id(),
        }))
        .unwrap();
    let (id, _) = requests(&effects)[0].clone();
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id,
            response: Response::ReviewSnapshot {
                snapshot: snapshot(1, 2),
            },
        }))
        .unwrap();
    let effects = kv.drive(&mut core, effects);
    let files_id = requests(&effects)
        .into_iter()
        .find(|(_, r)| matches!(r, Request::ListFiles { .. }))
        .unwrap()
        .0;
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id: files_id,
            response: Response::Files {
                files: vec![change("a.rs")],
            },
        }))
        .unwrap();
    let effects = kv.drive(&mut core, effects);
    let (render_id, _) = requests(&effects)
        .into_iter()
        .find(|(_, r)| matches!(r, Request::FileRender { .. }))
        .unwrap();
    let effects = item(
        &mut core,
        render_id,
        StreamItem::Header {
            header: header("a.rs", 100, 20),
        },
    );
    kv.drive(&mut core, effects);
    core.handle(Input::Server(ServerMsg::StreamEnd { id: render_id }))
        .unwrap();

    // Not open: chunks are unpinned. Stream 6 chunks through RenderChunk
    // answers and watch the tiers trim.
    let mut all = Vec::new();
    let mut effects = core
        .handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 1000,
            last_row: 1000,
        }))
        .unwrap();
    // Viewport pins the open file's chunks; close it again so they are
    // ordinary LRU entries, but keep the wants flowing.
    effects.extend(core.handle(Input::User(Action::CloseFile)).unwrap());
    let effects = kv.drive(&mut core, effects);
    let mut effects = effects;
    for _ in 0..4 {
        let answered = daemon_answers(&mut core, &effects);
        let driven = kv.drive(&mut core, answered);
        all.extend(driven.clone());
        effects = driven;
    }
    let mem_chunks = core
        .cache()
        .keys()
        .filter(|k| matches!(k, CacheKey::Chunk { .. }))
        .count();
    assert!(core.cache().used() <= core.cache().budget() || mem_chunks == 0);
    assert!(mem_chunks <= 2, "memory holds {mem_chunks} chunks");
    // Every chunk that arrived was persisted, and the disk was trimmed.
    let persisted = persists(&all).len();
    assert!(persisted >= 5, "persisted {persisted}");
    assert!(!removes(&all).is_empty(), "disk tier never trimmed");
    let disk_chunks = kv.map.keys().filter(|k| k.contains("\"Chunk\"")).count();
    assert!(disk_chunks <= 4, "disk holds {disk_chunks}");
    // The pinned header outlived all that pressure.
    assert!(core.cache().is_pinned(&header_key("a.rs")));
    assert!(core.cache().contains(&header_key("a.rs")));
}

#[test]
fn restart_serves_the_previous_review_from_disk_without_content_requests() {
    let mut kv = Kv::default();
    // Session 1: open piecewise, everything comes from the daemon.
    {
        let mut core = subscribed(remote(Bytes::mib(1), Bytes::mib(1)));
        let effects = core
            .handle(Input::User(Action::OpenReview {
                review_id: review_id(),
            }))
            .unwrap();
        let (id, _) = requests(&effects)[0].clone();
        let effects = core
            .handle(Input::Server(ServerMsg::Response {
                id,
                response: Response::ReviewSnapshot {
                    snapshot: snapshot(1, 2),
                },
            }))
            .unwrap();
        let effects = kv.drive(&mut core, effects);
        let mut files_id = None;
        for (id, r) in requests(&effects) {
            match r {
                Request::TreeSnapshot { ref_spec, .. } => {
                    let RefSpec::Commit { oid } = ref_spec else {
                        panic!("commit ref");
                    };
                    let fill = oid.oid().as_bytes()[0];
                    let effects = core
                        .handle(Input::Server(ServerMsg::Response {
                            id,
                            response: Response::TreeSnapshot {
                                snapshot: tree(fill, &["a.rs"]),
                            },
                        }))
                        .unwrap();
                    kv.drive(&mut core, effects);
                }
                Request::ListFiles { .. } => files_id = Some(id),
                Request::FileRender { .. }
                | Request::RenderChunk { .. }
                | Request::ListWorkspaces
                | Request::ListReviews { .. }
                | Request::GetReview { .. }
                | Request::ReviewSnapshot { .. }
                | Request::OpenReview { .. }
                | Request::ResolveTargets { .. }
                | Request::ListCommits { .. }
                | Request::BlobRender { .. }
                | Request::Subscribe { .. }
                | Request::Unsubscribe { .. }
                | Request::Mutate { .. }
                | Request::Shutdown => panic!("unexpected {r:?}"),
            }
        }
        let effects = core
            .handle(Input::Server(ServerMsg::Response {
                id: files_id.unwrap(),
                response: Response::Files {
                    files: vec![change("a.rs")],
                },
            }))
            .unwrap();
        let effects = kv.drive(&mut core, effects);
        let (render_id, _) = requests(&effects)
            .into_iter()
            .find(|(_, r)| matches!(r, Request::FileRender { .. }))
            .unwrap();
        let effects = item(
            &mut core,
            render_id,
            StreamItem::Header {
                header: header("a.rs", 100, 3),
            },
        );
        kv.drive(&mut core, effects);
        let effects = item(
            &mut core,
            render_id,
            StreamItem::Chunk {
                repo_id: repo_id(),
                path: path("a.rs"),
                chunk: chunk(0),
            },
        );
        kv.drive(&mut core, effects);
        core.handle(Input::Server(ServerMsg::StreamEnd { id: render_id }))
            .unwrap();
    }
    assert_eq!(kv.map.len(), 4, "trees, header, chunk 0 on disk");

    // Session 2: a fresh core over the same store.
    let mut core = subscribed(remote(Bytes::mib(1), Bytes::mib(1)));
    let effects = core
        .handle(Input::User(Action::OpenReview {
            review_id: review_id(),
        }))
        .unwrap();
    let (id, _) = requests(&effects)[0].clone();
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id,
            response: Response::ReviewSnapshot {
                snapshot: snapshot(1, 2),
            },
        }))
        .unwrap();
    let effects = kv.drive(&mut core, effects);
    let files_id = requests(&effects)
        .into_iter()
        .find(|(_, r)| matches!(r, Request::ListFiles { .. }))
        .unwrap()
        .0;
    let effects = core
        .handle(Input::Server(ServerMsg::Response {
            id: files_id,
            response: Response::Files {
                files: vec![change("a.rs")],
            },
        }))
        .unwrap();
    let effects = kv.drive(&mut core, effects);
    let effects2 = core
        .handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 0,
            last_row: 10,
        }))
        .unwrap();
    let effects2 = kv.drive(&mut core, effects2);
    for (_, request) in requests(&effects) {
        assert!(!is_content(&request), "went to the daemon for {request:?}");
    }
    for key in [
        tree_key(1),
        tree_key(2),
        header_key("a.rs"),
        chunk_key("a.rs", 0),
    ] {
        assert!(core.cache().contains(&key), "{key:?} not served from disk");
    }
    // Chunks 1 and 2 (prefetch radius) were never on disk: those, and only
    // those, go to the daemon.
    let mut fetched: Vec<u32> = requests(&effects2)
        .into_iter()
        .map(|(_, r)| match r {
            Request::RenderChunk { index, .. } => index.get(),
            Request::TreeSnapshot { .. }
            | Request::FileRender { .. }
            | Request::ListWorkspaces
            | Request::ListReviews { .. }
            | Request::GetReview { .. }
            | Request::ReviewSnapshot { .. }
            | Request::ListFiles { .. }
            | Request::OpenReview { .. }
            | Request::ResolveTargets { .. }
            | Request::ListCommits { .. }
            | Request::BlobRender { .. }
            | Request::Subscribe { .. }
            | Request::Unsubscribe { .. }
            | Request::Mutate { .. }
            | Request::Shutdown => panic!("unexpected {r:?}"),
        })
        .collect();
    fetched.sort_unstable();
    assert_eq!(fetched, vec![1, 2]);
    assert_eq!(core.content_in_flight(), 2);
    assert_eq!(core.content_queued(), 0);
}

#[test]
fn tree_delta_applies_in_place_and_renders_the_tree() {
    let mut core = subscribed(local());
    open_streamed(&mut core);
    let delta = TreeDelta {
        repo_id: repo_id(),
        from_root: tree_oid(2),
        to_root: tree_oid(9),
        added: vec![TreeEntry {
            path: path("d.rs"),
            kind: TreeEntryKind::File {
                oid: blob_oid(4),
                size: 4,
                executable: false,
            },
        }],
        removed: vec![path("c.rs")],
        changed: Vec::new(),
    };
    let effects = core
        .handle(Input::Server(ServerMsg::TreeDelta { delta }))
        .unwrap();
    assert_eq!(rendered(&effects), vec![ViewSection::Tree]);
    assert!(!core.cache().contains(&tree_key(2)));
    assert!(core.cache().is_pinned(&tree_key(9)));
    let Some(CacheValue::Tree { snapshot }) = core.cache().peek(&tree_key(9)) else {
        panic!("tree 9 missing");
    };
    let paths: Vec<String> = snapshot
        .entries
        .iter()
        .map(|e| e.path.to_string())
        .collect();
    assert_eq!(paths, vec!["a.rs", "b.rs", "d.rs"]);
    assert_eq!(
        core.view().review.as_ref().unwrap().trees,
        vec![tree_oid(1), tree_oid(9)]
    );
    // A delta for an unknown root is ignored.
    let effects = core
        .handle(Input::Server(ServerMsg::TreeDelta {
            delta: TreeDelta {
                repo_id: repo_id(),
                from_root: tree_oid(42),
                to_root: tree_oid(43),
                added: Vec::new(),
                removed: Vec::new(),
                changed: Vec::new(),
            },
        }))
        .unwrap();
    assert!(effects.is_empty());
}

#[test]
fn close_review_releases_pins_and_drops_queued_fetches() {
    let mut core = subscribed(local());
    open_streamed(&mut core);
    core.handle(Input::User(Action::Viewport {
        file: file("a.rs"),
        first_row: 500,
        last_row: 600,
    }))
    .unwrap();
    assert_eq!(core.content_queued(), 2);
    let effects = core.handle(Input::User(Action::CloseReview)).unwrap();
    assert_eq!(
        rendered(&effects),
        vec![
            ViewSection::Tree,
            ViewSection::Diff,
            ViewSection::Threads,
            ViewSection::Draft
        ]
    );
    assert_eq!(core.content_queued(), 0);
    assert!(core.cache().keys().all(|k| !core.cache().is_pinned(k)));
    // Late answers to the in-flight chunk requests are still cached.
    let answered = daemon_answers(&mut core, &effects);
    assert!(answered.is_empty());
    assert_eq!(
        core.handle(Input::User(Action::Viewport {
            file: file("a.rs"),
            first_row: 0,
            last_row: 0
        })),
        Err(CoreError::NoOpenReview)
    );
    assert_eq!(PREFETCH_RADIUS, 2);
}

#[test]
fn stored_answers_for_unknown_keys_are_rejected() {
    let mut core = subscribed(remote(Bytes::mib(1), Bytes::mib(1)));
    assert_eq!(
        core.handle(Input::Stored {
            key: "nope".into(),
            value: None
        }),
        Err(CoreError::UnknownKey("nope".into()))
    );
    let _ = ClientMsg::Cancel {
        id: RequestId::new(1),
    };
}
