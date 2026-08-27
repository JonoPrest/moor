# Moor — Architecture

Moor is a daemon-backed code review tool. Comments are *moored* to content (blobs), not to diffs or line numbers.

Status: **draft v1** — core decisions resolved (§10).

## 1. Goals

- A single always-running **daemon** per machine that owns all state: workspaces, reviews, comments.
- Multiple **clients** (desktop, browser, TUI, CLI, agents) attach to the daemon over the same protocol.
- Clients work over **SSH** to a remote daemon with no perceptible latency: all navigation and typing is served from a local cache; only mutations and cache misses touch the wire.
- **GitHub-style diff review** plus a **file explorer** over any ref, in one UI.
- Review **any base against any head** (branch, commit, tag, working tree), and **step through commits** within a range, seeing each commit's full message, author and dates.
- **Split (side-by-side) or unified** diff layout, switchable instantly; **hide whitespace** diffing as a toggle, and **mark as viewed** per file that auto-clears when that file changes in a later head.
- A **workspace** groups multiple git repos; one review can span repos.
- **Comments** are first-class, persisted, content-anchored, and record provenance (human vs agent, and which agent/session).
- Comments can be **inline** (lines of a blob), **file-level** (a whole file, whether or not it is in the diff), or **review-level** (like a non-inline GitHub PR comment).
- **Agents are peers**: everything a human can do through the UI, an agent can do through MCP/CLI, using the same core API.
- **Keyboard-first**: everything is reachable without a mouse. A persistent hint bar shows the main bindings for the current context; `?` opens a full, searchable help overlay.
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

Event kinds (initial set): `WorkspaceCreated/Updated`, `RepoAttached/Detached`, `ReviewCreated/Updated/Deleted`, `ReviewTargetsResolved` (snapshot of resolved OIDs), `CommentCreated/Edited/Deleted/Reanchored`, `ThreadResolved/Unresolved`, `FileViewed/Unviewed`, `ReviewRequested`, `SuggestionApplied`.

Every event carries `{ seq, ts, author, client_id, client_seq }`. `seq` is assigned by the daemon and is the global order. Deletion is a tombstone event; a later GC pass may compact.

Storage engine: **redb** (pure Rust, single file, ACID).

### 4.3 Git engine

- `gix` for object access (trees, blobs, commits, refs). Shell out to `git` for things gix does poorly (rename detection, worktree status) until it doesn't.
- Diffing via `imara-diff`; the daemon produces both raw hunks and a **render model** (see §4.6). `RenderOpts.ignore_whitespace` diffs a whitespace-normalised view of each line while rows carry the original text; a file whose diff is whitespace-only renders as a single collapsed "whitespace changes only" row.
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

CommitInfo { oid, parents, author: Sig, committer: Sig, subject, body }
Sig        { name, email, time: Timestamp, offset }
              returned by commits(review) for stepping; shown in full in the commit panel

ViewedMark { review_id, repo_id, path, viewer: Human, blob_oid }
              "viewed" is bound to the head blob seen; if the current head blob differs the
              file shows as changed-since-viewed and the mark is cleared in the UI.
              Human-only: agents cannot set it.

RenderOpts { ignore_whitespace: bool, context_lines }
              part of every render cache key; whitespace-ignored rows keep the real text,
              only line pairing/`changed` ranges differ

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

`File` anchors follow the path, including detected renames; they become `Outdated` only if the
file disappears. `Lines` anchors follow renames the same way. `Review` anchors never change.
Comments are never dropped by ref movement, and an `Outdated` comment is re-tried from its last
good anchor on every resolution, so it returns to `Live` when the content does.

The `context_hash` covers the anchored lines plus 3 on each side. The daemon computes it from
blob content when a comment is created (a client-supplied value is replaced) and rejects line
ranges beyond the blob's length.

Re-anchoring runs off the core actor: the actor records `ReviewTargetsResolved`, then the blob
diffs run on the blocking pool and emit one `CommentReanchored` event per comment as they finish.
Clients show affected comments as "re-anchoring" in the interim rather than the daemon stalling.

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

### 4.7 Tree snapshots (file explorer)

The explorer must never load per folder. The daemon serves a whole recursive listing in one
message, keyed by the root tree OID:

```
TreeSnapshot { root_oid, entries: [TreeEntry { path, kind: File | Dir | Symlink | Submodule,
                                                oid, size }] }   flat, sorted, one pass to nest
TreeDelta    { from_root, to_root, added: [TreeEntry], removed: [path], changed: [TreeEntry] }
```

- `tree_snapshot(repo, ref)`; for reviews the daemon sends snapshots for every target ref (base
  and head) on open. Cached by `root_oid`; pinned while the ref is open.
- Working-tree refs get `TreeDelta`s from the watcher instead of repeated full snapshots.
- Fallback for very large repos (> ~200k entries, configurable): depth-limited snapshot plus lazy
  subtrees keyed by their own tree OID — same caching, not upfront.

### 4.8 Transports

- **Unix socket**, length-prefixed JSON frames. Multiplexed: `Request{id}` / `Response{id}` / `Event{seq}`.
- **WebSocket**, same JSON envelopes, one per binary (or text) message — the socket does the framing, so no length prefix. Plain TCP, opt-in via `moord --ws <addr>`, for browser clients and remote daemons. Inside `moord` both share `connection::serve_framed` over the `FrameRead`/`FrameWrite` traits.
- **MCP**, `moor-mcp` on stdio (newline-delimited JSON-RPC), proxying to the daemon's unix socket or ws port. Tools: `list_workspaces`, `list_reviews`, `get_review` (snapshot + changed files), `create_review`, `update_review`, `get_diff`, `get_file` (numbered text, any side, unchanged files too), `list_comments`, `add_comment` (review / file / line anchors), `suggest`, `reply`, `resolve`, `request_review`, `subscribe_events` (long-poll; pass `last_seq` back as `since_seq`). Author is `Agent{name: clientInfo.name, model: $MOOR_AGENT_MODEL, session_id: $MOOR_SESSION_ID, invoked_by: $USER@host, via: Mcp}`. `mark_viewed` is deliberately not offered. Anchors go up with a zero `context_hash`; the daemon computes the real one.
- **Subscriptions**: `subscribe(scope, since_seq)` streams events from `since_seq`. Reconnect = resubscribe from last seen seq; no other sync mechanism.
- **Review open is one streamed request.** `open_review(id)` answers with an ordered stream — `ReviewSnapshot` (review, threads, comments) → `TreeSnapshot` per target ref → `FileRenderHeader` per changed file → first `RenderChunk` per file — rather than the client issuing hundreds of round-trips over SSH. The client consumes and its cache fills as a side effect; per-item requests remain for cache misses and viewport-driven chunks.
- **Fresh clients never replay the log.** A client with no `last_seq` gets a materialized `ReviewSnapshot` plus `subscribe(since = current_seq)`. Only reconnects with a known `last_seq` replay, and only the gap.

Encoding is an isolated layer; the Rust↔Rust hop may move to capnproto/flatbuffers later if measured to matter. JSON is the fixed contract between `moor-client-core` and the UI.

### 4.9 Versioning and evolution

Two independent versions, both typed in `moor-protocol::version`.

**Wire protocol — `ProtocolVersion` (semver string, e.g. `"0.1.0"`).**

- Every frame is an `Envelope { v: ProtocolVersion, msg }`, so the version is on each message,
  not only at handshake. One socket/port serves all versions; the version selects how the
  daemon *serialises*, not where the client connects.
- Handshake: the client's first frame is `Hello { client_id, protocol, client: BuildInfo }`.
  The daemon answers `Welcome { protocol, daemon, schema, upgrade }` — `protocol` is the
  version all following frames use — or `Rejected { UnsupportedProtocol { requested, supported } }`
  and closes.
- Compatibility rule: same `major`, daemon `minor >= client minor`. Minor bumps are additive
  (new variants/fields); the daemon serialises responses at the client's requested minor so a
  strict (`deny_unknown_fields`) older client never sees fields it doesn't know. Major bumps
  are never bridged silently.
- Deprecation path: a daemon may keep serving an old minor for a time and attach
  `Welcome.upgrade: UpgradeNotice { latest, message }`; clients surface it. Once dropped, the
  handshake is rejected with the supported list, so the error is specific and actionable.
- A frame whose `v` differs from the negotiated version is answered with `VersionMismatch`.
- Bumping: any change to a fixture under `fixtures/protocol/` requires bumping
  `ProtocolVersion::CURRENT` (minor if additive, major otherwise); CI diffs fixtures.

**Store schema — `SchemaVersion` (monotonic integer).**

- Stamped in the redb `meta` table on creation. `SchemaVersion::CURRENT` is what this build
  writes.
- On open: equal → proceed; older → run migrations forward in one transaction per step and
  restamp; newer → refuse to open with a clear error (a newer `moord` wrote this; upgrade).
- Events are stored as JSON with a per-event `schema` tag, so the event log itself migrates by
  re-serialisation, and materialised views can always be rebuilt from the migrated log.
- The daemon reports `schema` in `Welcome` for diagnostics only; clients never depend on it.

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

Content-addressed, so never stale: blobs by OID, trees by OID, diff render models by `(base_oid, head_oid, opts)`. On opening a review the daemon streams the full diff set and touched blobs; the file explorer prefetches siblings. Cache hit ⇒ zero-latency navigation.

Two tiers, both LRU with a **byte budget**:

1. **Memory** (default 256 MB) — inside `client-core`. Entries for the currently open review are pinned and never evicted.
2. **Disk** (default 2 GB) — via the host KV store (`Persist`/`Load` effects). Memory eviction writes through to disk; a memory miss checks disk before asking the daemon. Survives client restarts, so reopening a review over SSH is served locally.

Both budgets are configurable. Because keys are OIDs, on-disk entries never need invalidation; the disk tier is only ever trimmed by LRU or cleared explicitly.

**Local daemon ⇒ no client disk tier.** When the client connects to a unix socket on its own host, the daemon's `render-cache.redb` already holds every header and chunk, and a local socket round-trip is sub-millisecond, so the client runs memory-only and misses go to the daemon. The disk tier is enabled only for remote daemons (SSH/WebSocket). This avoids a second copy on disk without sharing a file between processes: redb is single-process (exclusive lock), so "daemon writes, client reads the same tables" is not possible without changing the store engine. If a shared local cache is ever wanted, `RenderCache` is the isolated seam to swap for sqlite.

Cache entries are `TreeSnapshot`s (§4.7), render headers and render **chunks** (§4.6), never whole files. Chunks of the open file are pinned while it is open; on close they return to normal LRU.

### 5.2 Optimistic mutations

1. Client generates the ULID, applies the event locally (marked `pending`), emits `Send`.
2. Daemon assigns `seq`, broadcasts to all subscribers including the originator.
3. On seeing its own event, client clears `pending`. On foreign events, it re-applies pending events on top.
4. Conflicts are limited to edits of the same comment and resolve/unresolve toggles: **last writer by `seq` wins**; the view re-renders.

### 5.3 Client-local state

Ephemeral UI state (hover, focus) lives in the UI. Navigational state (open file, expanded dirs, scroll, drafts) lives in `ViewModel` so all hosts behave identically; drafts are `Persist`ed via the injected KV.

### 5.4 Deferred refresh

Working-tree reviews auto-refresh, but `moor-client-core` will not swap in a new render model while the user has an open comment editor. Incoming `ReviewTargetsResolved` events are queued; when the draft is submitted or discarded, the queue is drained, the comment is re-anchored against the new head, and the view updates. The UI shows a subtle "changes pending" indicator while held.

### 5.5 File explorer

Folder expand/collapse, breadcrumbs and fuzzy file search (`Cmd+P`) operate entirely on the
cached `TreeSnapshot` — no requests. Only opening a file fetches content (header + chunks),
which is then cached. Switching ref prefetches that ref's snapshot.

### 5.6 Multi-repo tree

A review across repos presents one merged file tree with each repo as a top-level root (`repo-a/…`, `repo-b/…`). Progress ("N of M files viewed"), comment lists and navigation are review-wide, not per repo.

## 6. UI (ReScript + React)

### 6.1 Principle

The UI is a renderer over `ViewModel` and a source of `Action`s. It never talks to the daemon and contains no reconciliation logic.

### 6.2 Adapters

`Core.res` defines `dispatch: Action.t => unit` and `subscribe: (ViewModel.t => unit) => unsubscribe`. Two implementations, chosen at startup: `CoreTauri.res` (`invoke`/`listen`) and `CoreWasm.res` (wasm-bindgen exports).

### 6.3 Type bridge

The Rust `ViewModel`/`Action`/`Event` types and their ReScript counterparts are both **hand-written**. The ReScript side uses Sury (`rescript-schema`) schemas, which give static types plus a validator; the adapters parse at the boundary so drift is caught at runtime, not deep in a component. Rust enums use `#[serde(tag = "type")]` so they map to Sury tagged unions.

Drift is prevented by a **boundary test**: Rust emits a JSON fixture for every type/variant (serialize, and check it deserializes back); a ReScript test parses each fixture with the Sury schema and re-serializes it, and the outputs must match byte-for-byte after canonicalisation. Both directions are covered, so a protocol change that isn't mirrored fails CI.

### 6.4 Keyboard model

Every user operation is an `Action`; keybindings are a data table mapping `(Context, KeyChord) → Action`, not ad-hoc handlers in components.

```
Keymap  { bindings: [Binding { context, chord, action, label, primary: bool }] }
Context = Global | ReviewList | Tree | Diff | Thread | Composer | CommitStepper | Help
```

- `client-core` owns the keymap (default table + user overrides from the host KV) and resolves
  `Input::Key { context, chord }` → `Action`. The UI captures keys, sends chords, and renders
  the results — it contains no key → behaviour logic, so bindings are identical across hosts
  (Tauri, browser, TUI) and testable without a DOM.
- Chords support sequences (`g g`, `] c`) with a short timeout; vim-style movement
  (`j`/`k`, `n`/`p` next/prev hunk, `] f`/`[ f` next/prev file, `] c` next comment,
  `v` mark viewed, `c` comment, `r` reply, `Enter` open, `Esc` back, `Cmd/Ctrl+P` file search,
  `s` toggle split/unified, `w` toggle whitespace).
- **Hint bar**: the UI renders the `primary: true` bindings for the active context along the
  bottom edge, from the keymap — never hand-written.
- **`?` help overlay**: all bindings for the active context plus Global, grouped and searchable,
  generated from the same table. Shows user overrides and conflicts.
- Focus is explicit state in the `ViewModel` (`focus: Context + target`), so "what does `j` do"
  is always determined by core state, not DOM focus.

### 6.5 Diff rendering

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

## 10. Measure before optimising

Suspected bottlenecks with a ready solution, deliberately **not** built until a benchmark in the plan shows they matter. Each has a trigger and a candidate fix.

| Suspect | Trigger to act | Candidate fix |
|---------|----------------|---------------|
| Working-tree snapshot cost on large repos | snapshot > 100 ms after a single-file edit in a 50k-file repo | rehash only watcher-reported paths; ignore rules at the watcher (`.gitignore`, `target/`, `node_modules/`) so builds don't storm it |
| Rename detection on big add/delete sets | `changed_files` > 500 ms on a directory move | cap candidates (like `diff.renameLimit`); compute renames after the header stream and patch the tree with a follow-up message |
| redb write rate under agent load | appending 200 comments in a loop > 1 s, or fsync visible in profiles | batch appends per actor tick; keep ephemeral state (viewed flags) out of the durable log |
| Event log growth | log > 1M events or startup rebuild > 1 s | offline compaction of tombstoned reviews (design already permits it) |
| Cold start over SSH with empty cache | first usable frame > 1 s on a 300-file review | already mitigated by streamed `open_review` order; further: gzip frames, lower first-chunk size |
| Depth-limited tree fallback | `tree_snapshot` > 200 ms or > 5 MB | lazy subtrees (§4.7) — implement only when a real repo trips it |

## 11. Decisions

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
| 12 | Multi-repo review UI | merged tree with repo roots (§5.6) |
| 14 | Highlighter | syntect |
| 16 | Daemon concurrency | one writer thread (mutations + re-anchoring, strictly serialised) and the tokio blocking pool for reads/renders against the shared `Core`; events fan out via a broadcast channel, connections filter by scope |
| 17 | Client cache when daemon is local | memory tier only; disk tier for remote daemons (§5.1) |
| 15 | Evolution | semver `ProtocolVersion` negotiated in `Hello`/`Welcome`, on every `Envelope`; integer `SchemaVersion` in redb `meta` with forward-only migrations (§4.9) |

### Deferred

- Cross-machine comment sync (log makes it feasible; not needed now).
- Rust↔Rust wire encoding beyond JSON.
- Browser/wasm client, TUI client.
- Export to GitHub PR review / `.review/` directory.
