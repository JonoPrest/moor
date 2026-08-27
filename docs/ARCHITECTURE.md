# Moor — Architecture

Moor is a daemon-backed code review tool. Comments are *moored* to content (blobs), not to diffs or line numbers.

Status: **draft v1** — core decisions resolved (§10).

## 1. Goals

- A single always-running **daemon** per machine that owns all state: workspaces, reviews, comments.
- Multiple **clients** (desktop, browser, TUI, CLI, agents) attach to the daemon over the same protocol.
- Clients work over **SSH** to a remote daemon with no perceptible latency: all navigation and typing is served from a local cache; only mutations and cache misses touch the wire.
- **GitHub-style diff review** plus a **file explorer** over any ref, in one UI.
- Review **any base against any head** (branch, commit, tag, working tree), and **step through commits** within a range.
- A **workspace** groups multiple git repos; one review can span repos.
- **Comments** are first-class, persisted, content-anchored, and record provenance (human vs agent, and which agent/session).
- Comments can be **inline** (lines of a blob), **file-level** (a whole file, whether or not it is in the diff), or **review-level** (like a non-inline GitHub PR comment).
- **Agents are peers**: everything a human can do through the UI, an agent can do through MCP/CLI, using the same core API.
- Clients apply mutations **optimistically** and reconcile after the fact.
- State **persists across daemon restarts**; reviews live until explicitly deleted.

Non-goals (for now): multi-machine sync of comments, hosting/PR integration, auth beyond SSH.

## 2. System overview

```
┌─────────────┐  ┌─────────────┐  ┌───────────┐  ┌──────────────┐
│ Tauri app   │  │ Browser     │  │ TUI       │  │ Agent / CLI  │
│ (webview UI)│  │ (wasm core) │  │ (ratatui) │  │ (MCP / moor)   │
└──────┬──────┘  └──────┬──────┘  └─────┬─────┘  └──────┬───────┘
       │ unix sock      │ websocket     │ unix sock     │ MCP (stdio/ws)
       └────────────────┴───────┬───────┴───────────────┘
                                ▼
                    ┌───────────────────────┐
                    │        daemon         │
                    │  transports: unix, ws, mcp  (thin adapters)
                    │  ┌─────────────────┐  │
                    │  │   review-core   │  │  git engine · diff · anchoring
                    │  │   event store   │  │  append-only log + views (redb)
                    │  └─────────────────┘  │
                    └───────────────────────┘
                          ▲ reads repos
                    ┌─────┴─────┐ ┌─────────┐
                    │  repo A   │ │ repo B  │  (a workspace)
                    └───────────┘ └─────────┘
```

Remote use: the client tunnels the daemon's socket/port over SSH (`ssh -L` or `ssh host moord --stdio`). SSH is the only auth layer.

## 3. Crate / package layout

```
crates/
  moor-protocol/      wire types: requests, events, view/render models. serde. wasm-safe.
  moor-review-core/   git access, diffing, anchoring, event store, review logic. daemon-only.
  moord/              daemon: socket + websocket + MCP transports over review-core. file watcher.
  moor-client-core/   sans-I/O client state machine: cache, optimistic state, ViewModel. wasm-safe.
  moor-client-wasm/   wasm-bindgen shim around client-core.
  moor-client-tauri/  Tauri host: runs client-core natively, exposes dispatch/subscribe to the webview.
  moor-client-tui/    ratatui host (later).
  moor-cli/           `moor` command; thin RPC client. Used by shell-based agents and scripts.
  moor-mcp/           MCP stdio shim proxying to moord.
ui/              ReScript + React + Vite. Shared by Tauri and browser.
  src/core/      Core.res interface + CoreTauri.res / CoreWasm.res adapters
  src/protocol/  Protocol.res — hand-written Sury schemas mirroring the Rust types
  src/components/
```

Dependency rule: `moor-protocol` ← everything. `moor-client-core` never depends on `moor-review-core`. Nothing in `moor-client-core` or `moor-protocol` may use tokio, std I/O, threads, `Instant`, or non-`js` `rand` (enforced by a CI `cargo check --target wasm32-unknown-unknown`).

## 4. Daemon

### 4.1 Core API

`moor-review-core` exposes one `Core` type with all operations. Transports (`unix`, `ws`, `mcp`) are adapters that map 1:1 onto it. There is no capability a transport adds; this is what guarantees human/agent parity.

### 4.2 Event-sourced store

All mutable state is an **append-only event log**. Materialized views are derived from it and can be rebuilt.

```
events:            seq (u64) → Event           source of truth
reviews:           review_id → ReviewView
comments_by_review review_id, comment_id → CommentView
anchors_by_blob:   (repo, blob_oid) → [comment_id]   for fast re-anchoring
workspaces:        workspace_id → Workspace
```

Event kinds (initial set): `WorkspaceCreated/Updated`, `RepoAttached/Detached`, `ReviewCreated/Updated/Deleted`, `ReviewTargetsResolved` (snapshot of resolved OIDs), `CommentCreated/Edited/Deleted`, `ThreadResolved/Unresolved`, `ReviewRequested`, `SuggestionApplied`.

Every event carries `{ seq, ts, author, client_id, client_seq }`. `seq` is assigned by the daemon and is the global order. Deletion is a tombstone event; a later GC pass may compact.

Storage engine: **redb** (pure Rust, single file, ACID).

### 4.3 Git engine

- `gix` for object access (trees, blobs, commits, refs). Shell out to `git` for things gix does poorly (rename detection, worktree status) until it doesn't.
- Diffing via `imara-diff`; the daemon produces both raw hunks and a **render model** (see §4.6).
- Working tree is a first-class "ref": `RefSpec::WorkingTree`. A `notify` watcher on each repo invalidates and emits `ReviewTargetsResolved` (debounced) for reviews targeting the working tree. The daemon always emits; **holding** the refresh is a client concern (§5.4).
- All content is addressed by OID. Diffs are cached by `(base_oid, head_oid, path, opts)`.

### 4.4 Data model

```
Workspace { id, name, repos: [Repo { id, path, display_name }] }

Review {
  id, workspace_id, title,
  targets: [ReviewTarget { repo_id, base: RefSpec, head: RefSpec }],
  created, status: Open | Archived
}
RefSpec = Branch(name) | Commit(oid) | Tag | WorkingTree | Upstream | Head

Comment {
  id: ulid,               client-generated → enables optimistic create
  review_id, thread_id,
  author: Author,
  kind: Note | Suggestion { patch } | Request,
  anchor: Anchor,
  body, created, edited,
  state: Live | Outdated { last_good_anchor } | Deleted
}

Author = Human { name, machine }
       | Agent { name, model, session_id, invoked_by: Option<Human>, via: Mcp | Cli }

Anchor
  = Review                                   review-level, no file
  | File  { repo_id, path, blob_oid }        whole-file, any ref; need not be in the diff
  | Lines { repo_id, path, side: Base | Head,
            blob_oid,                        exact blob the comment was made on
            lines: Range,                    in that blob
            context_hash }                   hash of ±3 surrounding lines

Anchors reference blobs, not diffs. Opening a file in the explorer and commenting
on it uses the same path as commenting inside a diff; the diff is just a way to
navigate to a blob.
```

### 4.5 Re-anchoring

When a review's resolved head/base changes:

1. same `blob_oid` → anchor unchanged.
2. blob changed → diff old→new blob, map `lines` through the diff.
3. mapped region's context hash mismatches → mark `Outdated`, keep last good anchor, still shown (collapsed) in the UI.

`File` anchors follow the path; they become `Outdated` if the file is deleted or renamed away.
`Review` anchors never change. Comments are never dropped by ref movement.

### 4.6 Diff render model

Three levels of diff data:

1. **Raw diff** — hunks of `+/-/context` lines from git. Daemon.
2. **Render model** — the flat list of rows the screen shows. Daemon. Pure function of
   `(base_oid, head_oid, path, opts)`, cached on disk, identical for every client.
3. **Overlays** — comment threads, selection, hover. `moor-client-core` / UI.

Render model rows:

```
Row = HunkHeader { text }
    | Context    { left: Cell, right: Cell }
    | Removed    { left: Cell }
    | Added      { right: Cell }
    | Modified   { left: Cell, right: Cell }     paired -/+ with intra-line ranges
    | Expander   { hidden: u32, dir }            "show N more lines"
Cell = { line_no, text, spans: [{ start, end, class }], changed: [Range] }
```

Building rows involves pairing `-`/`+` lines for split view, intra-line word diff, context
collapsing, and syntax highlighting (tree-sitter/syntect, native, run once). Unified vs
split is a UI choice over the same rows. Whole-file views (explorer) are the same `Cell`
list with no diff rows.

Comment → row placement is done in `moor-client-core` (anchor `blob_oid + lines` → row by
`line_no` per side) so the daemon's render model stays comment-agnostic and cacheable.

### 4.7 Transports

- **Unix socket**, length-prefixed JSON frames. Multiplexed: `Request{id}` / `Response{id}` / `Event{seq}`.
- **WebSocket**, same frames, for browser clients.
- **MCP**, over stdio or the ws port. Tools map to core methods: `list_workspaces`, `list_reviews`, `create_review`, `get_diff`, `get_file`, `list_comments`, `add_comment`, `reply`, `resolve`, `suggest`, `request_review`, `subscribe_events`.
- **Subscriptions**: `subscribe(scope, since_seq)` streams events from `since_seq`. Reconnect = resubscribe from last seen seq; no other sync mechanism.

Encoding is an isolated layer; the Rust↔Rust hop may move to capnproto/flatbuffers later if measured to matter. JSON is the fixed contract between `moor-client-core` and the UI.

## 5. Client core (sans-I/O)

`moor-client-core` is a pure state machine. It performs no I/O; the host injects everything.

```rust
pub struct ClientCore { ... }

impl ClientCore {
    pub fn handle(&mut self, input: Input) -> Vec<Effect>;
    pub fn view(&self) -> &ViewModel;
}

pub enum Input  { User(Action), Server(ServerMsg), Stored(Key, Bytes), Tick(Millis) }
pub enum Effect { Send(ClientMsg), Persist(Key, Bytes), Load(Key), Render }
```

Hosts (Tauri, wasm, TUI) own: transport, local KV store, clock. This makes the core testable without mocks and compilable to wasm.

### 5.1 Cache

Content-addressed, so never stale: blobs by OID, trees by OID, diff render models by `(base, head, path)`. On opening a review the daemon streams the full diff set and touched blobs; the file explorer prefetches siblings. Cache hit ⇒ zero-latency navigation.

### 5.2 Optimistic mutations

1. Client generates the ULID, applies the event locally (marked `pending`), emits `Send`.
2. Daemon assigns `seq`, broadcasts to all subscribers including the originator.
3. On seeing its own event, client clears `pending`. On foreign events, it re-applies pending events on top.
4. Conflicts are limited to edits of the same comment and resolve/unresolve toggles: **last writer by `seq` wins**; the view re-renders.

### 5.3 Client-local state

Ephemeral UI state (hover, focus) lives in the UI. Navigational state (open file, expanded dirs, scroll, drafts) lives in `ViewModel` so all hosts behave identically; drafts are `Persist`ed via the injected KV.

### 5.4 Deferred refresh

Working-tree reviews auto-refresh, but `moor-client-core` will not swap in a new render model while the user has an open comment editor. Incoming `ReviewTargetsResolved` events are queued; when the draft is submitted or discarded, the queue is drained, the comment is re-anchored against the new head, and the view updates. The UI shows a subtle "changes pending" indicator while held.

### 5.5 Multi-repo tree

A review across repos presents one merged file tree with each repo as a top-level root (`repo-a/…`, `repo-b/…`). Progress ("N of M files viewed"), comment lists and navigation are review-wide, not per repo.

## 6. UI (ReScript + React)

### 6.1 Principle

The UI is a renderer over `ViewModel` and a source of `Action`s. It never talks to the daemon and contains no reconciliation logic.

### 6.2 Adapters

`Core.res` defines `dispatch: Action.t => unit` and `subscribe: (ViewModel.t => unit) => unsubscribe`. Two implementations, chosen at startup: `CoreTauri.res` (`invoke`/`listen`) and `CoreWasm.res` (wasm-bindgen exports).

### 6.3 Type bridge

The Rust `ViewModel`/`Action`/`Event` types and their ReScript counterparts are both **hand-written**. The ReScript side uses Sury (`rescript-schema`) schemas, which give static types plus a validator; the adapters parse at the boundary so drift is caught at runtime, not deep in a component. Rust enums use `#[serde(tag = "type")]` so they map to Sury tagged unions.

Drift is prevented by a **boundary test**: Rust emits a JSON fixture for every type/variant (serialize, and check it deserializes back); a ReScript test parses each fixture with the Sury schema and re-serializes it, and the outputs must match byte-for-byte after canonicalisation. Both directions are covered, so a protocol change that isn't mirrored fails CI.

### 6.4 Diff rendering

The UI renders the daemon's render model (§4.6) plus `moor-client-core` overlays as a virtualized list (`@tanstack/react-virtual`). Unified vs split is a view option on the same rows. Review-level comments render in a review "conversation" panel; file-level comments render at the top of the file view.

## 7. Agent integration

- Agents connect via MCP (or `moor` CLI) with `Author::Agent{...}` provenance. Provenance is a structured field, not a tag.
- `ReviewRequested` events show as a card in human clients; agents can subscribe to events addressed to them (`awaiting_agent`).
- **Suggestions**: a comment kind carrying a unified diff against a specific `blob_oid`. The UI renders "apply", which writes to the working tree and records `SuggestionApplied`.
- Threads keep agent `session_id`, so a human reply to an agent comment can be routed back to that session.

## 8. Remote / SSH

The daemon is unaware of remoteness. Clients connect to a local socket/port; the user (or the client, as a convenience) forwards it over SSH. The `moor` CLI can be run remotely via `ssh host moord --stdio` as a fallback transport.

## 9. Persistence & lifecycle

- One daemon per machine, data dir `~/.local/share/moor/` (`state.redb`, logs, per-repo diff cache).
- Reviews persist until `ReviewDeleted`. Deletion tombstones; compaction is offline and optional.
- Daemon restart: reopen store; clients resubscribe from `last_seq`.

## 10. Decisions

### Resolved

| # | Decision | Choice |
|---|----------|--------|
| 1 | Store engine | redb |
| 2 | Wire encoding | JSON everywhere; encoding layer isolated so Rust↔Rust can change later |
| 4 | Diff render model location | daemon (§4.6) |
| 5 | Syntax highlighting | daemon, token spans in render model |
| 6 | Review identity | persistent object, cheap to create, lives until deleted |
| 7 | Comment IDs | client-generated ULID |
| 10 | First client | Tauri (native client-core; wasm/browser second) |
| 13 | Comment scopes | review-level, file-level, inline — all via `Anchor` enum |

| 3 | Rust↔ReScript types | hand-written both sides; Sury schemas in ReScript; JSON fixture round-trip test in CI (§6.3) |
| 8 | Working-tree changes | auto-refresh, debounced; client defers while a comment draft is open (§5.4) |
| 9 | Agent event delivery | subscribe via MCP |
| 11 | MCP transport | stdio shim proxying to daemon first; direct ws later |
| 12 | Multi-repo review UI | merged tree with repo roots (§5.5) |
| 14 | Highlighter | syntect |

### Deferred

- Cross-machine comment sync (log makes it feasible; not needed now).
- Rust↔Rust wire encoding beyond JSON.
- Browser/wasm client, TUI client.
- Export to GitHub PR review / `.review/` directory.
