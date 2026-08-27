# Implementation Plan

Companion to `ARCHITECTURE.md`. Four milestones, each shippable and tested on its own.
Two cross-cutting principles apply to every task.

## Principles

### Type-driven design

Invalid state should not be expressible. Concretely:

- **Newtypes for every ID and OID**: `WorkspaceId`, `ReviewId`, `CommentId`, `ThreadId`, `RepoId`, `BlobOid`, `CommitOid`, `Seq`, `ClientSeq`. No bare `String`/`u64` in signatures.
- **Enums over flags.** `Anchor` is `Review | File | Lines`, never `Option<path> + Option<lines>`. `CommentState` is `Live | Outdated { last_good } | Deleted`, never `deleted: bool`.
- **Parse, don't validate.** Wire input is parsed into a validated domain type once at the boundary (`TryFrom<protocol::X> for core::X`); the core never re-checks.
- **Resolved vs unresolved are different types.** `RefSpec` (what the user asked for) vs `ResolvedTarget { base: CommitOid | WorkTreeSnapshot, head: ... }`. A `Review` holds both; diffing only accepts the resolved one.
- **Pending vs committed events are different types.** `moor-client-core` has `PendingEvent { client_seq, .. }` and `CommittedEvent { seq, .. }`; you cannot render a `Seq` that doesn't exist.
- **Typestate for connections/subscriptions** where cheap (`Disconnected → Connecting → Subscribed { since: Seq }`).
- **Non-empty where required**: `Review.targets: NonEmpty<ReviewTarget>`; `Lines.range: LineRange` with `start <= end` enforced by constructor.
- **Exhaustive matching, no `_ =>` on domain enums** (clippy `wildcard_enum_match_arm` in core crates).
- **Sans-I/O boundaries** make this checkable: `moor-client-core` returns `Vec<Effect>`; a test asserts exactly which effects, no more.

### Testing

Each crate has its own test strategy, listed per milestone. Common tools: `insta` for snapshots, `proptest` for invariants, `tempfile` + real `git` for repo fixtures, `cargo nextest`. A shared `moor-test-support` crate provides repo builders (`RepoBuilder::new().commit("a", files![...]).branch("x")...`).

CI: `cargo nextest run`, `cargo clippy -D warnings`, `cargo check -p moor-protocol -p moor-client-core --target wasm32-unknown-unknown`, `cargo fmt --check`, ReScript build + tests.

---

## Milestone 1 — `moor-protocol` + `moor-review-core`

Goal: headless engine. Given repos on disk, create workspaces/reviews, produce render models, store and re-anchor comments across restarts.

### 1.1 Workspace scaffold
- Cargo workspace; crates `moor-protocol`, `moor-review-core`, `moor-test-support`, `xtask`.
- Lints: `#![deny(missing_debug_implementations)]`, clippy pedantic subset, `wildcard_enum_match_arm` in `moor-review-core`/`moor-client-core`.
- CI as above.

### 1.2 `moor-protocol` types
- IDs (ULID-backed newtypes, `Display`/`FromStr`), OIDs, `Seq`.
- Domain: `Workspace`, `Repo`, `Review`, `ReviewTarget`, `RefSpec`, `Comment`, `Author`, `Anchor`, `CommentKind`, `CommentState`, `Thread`.
- Events: `Event { seq, ts, author, client_id, client_seq, body: EventBody }` and each `EventBody` variant.
- Render model: `Row`, `Cell`, `Span`, `FileRender`, `DiffSummary`.
- RPC: `ClientMsg`, `ServerMsg`, `Request`/`Response` per method, `SubscribeScope`.
- All `#[serde(tag = "type")]`, `deny_unknown_fields`.
- **Fixtures**: `cargo xtask fixtures` writes `fixtures/protocol/<Type>/<variant>.json` for every variant (a `Fixtures` trait implemented per type; a test asserts every enum variant has a fixture via exhaustive match).
- Tests: serde round-trip per fixture; `insta` snapshot of every fixture so wire changes are visible in review; `proptest` round-trip for IDs/ranges.

### 1.3 Event store (`moor_review_core::store`)
- redb tables: `events`, `reviews`, `comments`, `threads`, `anchors_by_blob`, `workspaces`, `meta`.
- `Store::append(NewEvent) -> CommittedEvent` assigns `Seq` atomically and updates views in the same txn.
- `Store::replay_from(Seq) -> impl Iterator<CommittedEvent>`.
- `Store::rebuild_views()` from log; startup verifies `meta.view_seq == last_seq` or rebuilds.
- Tests: append/read; views match a fold over the log (`proptest`: random event sequences, compare `rebuild_views` vs incremental); reopen after drop preserves everything; tombstoned review excluded from listings; concurrent appenders get strictly increasing `Seq`.

### 1.4 Git engine (`moor_review_core::git`)
- `Repo::open(path)`, `resolve(RefSpec) -> ResolvedRef`, `tree(CommitOid)`, `blob(BlobOid)`, `commits_between(base, head)`.
- `WorkingTree` snapshot: hash working files into a virtual tree (`WorkTreeSnapshot { tree_oid, dirty: [path] }`); unchanged files reuse index OIDs.
- `changed_files(base, head) -> [FileChange { path, kind: Added|Deleted|Modified|Renamed{from}, old: Option<BlobOid>, new: Option<BlobOid> }]`.
- Tests with `moor-test-support` repos: each `RefSpec` variant resolves; renames detected; working-tree snapshot reflects unstaged edits and untracked files; binary files flagged.

### 1.5 Diff + render model (`moor_review_core::render`)
- `render_file(old: Option<&[u8]>, new: Option<&[u8]>, lang, opts) -> FileRender`.
- Pipeline: `imara-diff` hunks → pair `-`/`+` into `Modified` → intra-line ranges → context collapsing with `Expander` → syntect spans.
- `render_blob(bytes, lang) -> FileRender` for explorer views (all `Context` rows).
- Content-keyed disk cache `(old_oid, new_oid, opts_hash) -> FileRender`.
- Tests: `insta` snapshots for a corpus (add/delete/modify/rename/whitespace-only/binary/huge/no-trailing-newline/CRLF); invariants via `proptest`: every source line appears exactly once on its side, line numbers monotonic, spans within cell bounds, `Expander.hidden` sums to the omitted count; unified and split derive from the same rows.

### 1.6 Reviews (`moor_review_core::review`)
- `create_review(workspace, NonEmpty<ReviewTarget>)`, `resolve_targets(review) -> ResolvedTargets` (emits `ReviewTargetsResolved` only when OIDs change), `files(review) -> merged tree`, `commits(review)` for commit stepping, `file_render(review, repo, path)`.
- Merged tree: repo roots at top level; deterministic ordering.
- Tests: multi-repo review yields one tree with both roots; commit stepping produces `base=parent` sub-reviews; re-resolve is idempotent (no duplicate events).

### 1.7 Comments + anchoring (`moor_review_core::comments`)
- `add_comment`, `reply`, `edit`, `delete`, `resolve_thread`, `unresolve_thread`; all emit events, all validate against current review state (e.g. `Lines` range within blob length).
- `reanchor(review, old: ResolvedTargets, new: ResolvedTargets)` per §4.5, emitting `CommentReanchored { comment, anchor, state }`.
- Tests: table-driven anchoring cases (unchanged blob; lines shifted above; lines shifted within; lines modified → `Outdated`; file deleted; file renamed; base-side anchor when base moves). `proptest`: random edits *outside* the anchored range never produce `Outdated`. Persistence: comments survive store reopen with anchors intact.

### 1.8 `Core` façade
- One `Core` struct composing the above; every public method takes/returns `moor-protocol` types; this is the surface every transport adapts.
- Tests: end-to-end scenario tests written as scripts (`open workspace → attach 2 repos → review → comment → commit on head → re-resolve → assert comment state`).

Exit criteria: all above green; `cargo check --target wasm32-unknown-unknown -p protocol` passes.

---

## Milestone 2 — `moord`

Goal: `moord` running, multiple clients connected, events streaming, MCP working.

### 2.1 Transport framing
- `codec` module: length-prefixed JSON frames over `AsyncRead/Write`; `ClientMsg`/`ServerMsg` mux with request ids.
- Tests: framing round-trip, partial reads, oversized frame rejected, interleaved requests answered by id.

### 2.2 Unix socket server
- tokio; one task per connection; `Core` behind `Arc<RwLock>` or actor (decide during impl; prefer actor with a command channel so `Core` stays single-threaded and simple).
- `subscribe(scope, since)` → replay from store then live tail via broadcast.
- Tests: two clients, one writes, other receives with correct `Seq`; reconnect with `since` receives exactly the gap; slow subscriber doesn't block others.

### 2.3 File watcher
- `notify` per repo; debounce; triggers `resolve_targets` for working-tree reviews.
- Tests: edit file → `ReviewTargetsResolved` emitted once for a burst of writes; no event when content unchanged.

### 2.4 WebSocket transport
- Same codec over `tokio-tungstenite`; enables browser client later.
- Tests: shared transport test-suite run against both unix and ws (`#[test_case]` / generic harness).

### 2.5 MCP
- `moor-mcp` stdio binary proxying to daemon socket; tool per `Core` method; `Author::Agent` from MCP client info + session.
- `subscribe_events` tool for long-poll/streaming.
- Tests: JSON-RPC conformance for tool list; each tool maps to core and round-trips; agent-authored comment carries provenance.

### 2.6 `moor` CLI
- `moor workspace add/list`, `moor review create --base --head [--repo ...]`, `moor comment ...`, `moor events --follow`. Same client lib as MCP.
- Tests: `assert_cmd` against a spawned daemon in a temp dir.

### 2.7 Lifecycle
- Data dir, socket path, `moord --stdio` mode for `ssh host moord --stdio`, graceful shutdown, crash-safe reopen.
- Tests: kill -9 mid-append → reopen consistent (redb guarantees; assert view rebuild path works).

---

## Milestone 3 — `moor-client-core`

Goal: sans-I/O client that models everything the UI needs; proven under races.

### 3.1 State machine skeleton
- `ClientCore::handle(Input) -> Vec<Effect>`, `view() -> &ViewModel`.
- Typestate connection: `Disconnected | Connecting | Subscribed { last_seq }`.
- Tests: every `Input` in every state either transitions or is rejected with a typed error; no panics (`proptest` over random input sequences).

### 3.2 Cache
- `ContentCache` keyed by OID / `(base, head, path, opts)`; LRU with size budget; `Load`/`Persist` effects for the host KV.
- Prefetch policy: on review open request all `FileRender`s; on file open request sibling entries.
- Tests: cache hit produces no `Send`; miss produces exactly one `Send` and dedupes concurrent misses; eviction respects budget.

### 3.3 Optimistic mutations
- `PendingEvent` list; local apply; on own `CommittedEvent` → drop pending; on foreign → rebase.
- LWW by `Seq` for edits/resolve.
- Tests: **two-client simulator** (`moor_test_support::Sim`) that drives two `ClientCore`s and an in-memory daemon model with controllable delivery order. Cases: concurrent replies to one thread; concurrent edit of same comment (LWW); resolve/unresolve race; disconnect mid-pending then reconnect (pending re-sent exactly once, idempotent by `CommentId`). `proptest`: any interleaving converges both clients to the daemon state.

### 3.4 Deferred refresh (§5.4)
- `Draft` state in `ViewModel`; `ReviewTargetsResolved` queued while a draft is open; drained on submit/discard; new comment anchored to the head at draft-open time and re-anchored by the daemon.
- Tests: refresh during draft → view unchanged + `pending_refresh: true`; submit → refresh applied, comment lands on new head with correct state.

### 3.5 ViewModel
- Merged file tree, current file rows + comment overlays, thread list, review conversation, commit stepper, progress.
- Comment→row placement from anchors.
- Tests: `insta` snapshots of `ViewModel` for scenario scripts; placement tests for each `Anchor` variant incl. `Outdated`.

Exit criteria: wasm target check passes for `moor-client-core`; simulator suite green.

---

## Milestone 4 — `ui` + Tauri

Goal: usable desktop app.

### 4.1 Sury schemas + boundary test (§6.3)
- `ui/src/protocol/*.res` hand-written Sury schemas for `ViewModel`, `Action`, and everything they contain.
- Test: for each `fixtures/protocol/**.json`, parse with Sury, serialize, canonicalise, compare. Run in CI; a missing schema for a new fixture fails.

### 4.2 Adapters
- `Core.res` interface; `CoreTauri.res` (`invoke("dispatch")`, `listen("view")`); `CoreWasm.res` stub.
- Tests: adapter unit tests with a mocked Tauri API.

### 4.3 Tauri host (`moor-client-tauri`)
- Owns `ClientCore`, unix-socket transport (and ssh-forwarded path), KV via file, clock; pushes `ViewModel` diffs to webview.
- Tests: host integration test against a real daemon in a temp dir.

### 4.4 Screens
- Review list / create (any base vs any head, multi-repo).
- Merged file tree with progress.
- Diff view: virtualized rows, unified/split, expanders, inline comment composer, threads, outdated collapse.
- File explorer over any ref with file-level comments.
- Review conversation panel (review-level comments, agent request cards).
- Commit stepper.
- Suggestions with apply.
- Tests: ReScript component tests for row rendering (each `Row` variant), composer state; Playwright smoke against the Tauri dev build for the core flow.

### 4.5 Polish
- Keyboard nav, "changes pending" indicator, remote connection UX (ssh target picker).

---

## Later
- Browser build (`moor-client-wasm`, daemon serves `ui/`), TUI, cross-machine sync, GitHub export.
