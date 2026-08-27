# Handover notes

Written 2026-08-27 at the end of the session that finished Milestone 3.2
(client cache). Read this, then `AGENTS.md`, then `docs/PLAN.md`.

## State of the tree

`main` is clean. Last commits:

- `client-core: content flow, disk tier, viewport prefetch (3.2 steps 2-6)`
- `Fix lints new in clippy 1.97`
- `cf959ee` client-core: `ContentCache` memory tier (3.2 step 1)
- `742953c` MCP: derive tool schemas from the serde types with schemars

All gates green: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo check -p moor-protocol -p moor-client-core --target wasm32-unknown-unknown`
(needs `rustup target add wasm32-unknown-unknown`), `cargo test --workspace`,
`cargo xtask fixtures` (unchanged). Run all five before every commit.

## What 3.2 added (`crates/moor-client-core`)

- `cache.rs` — `ContentCache`: memory LRU with a byte budget and pins.
  Keys: `CacheKey::{Tree{root}, Header{render}, Chunk{render,index}}` over
  `RenderKey { repo_id, path, target, opts }`. `storage_key()` (serde_json
  of the key) is the host KV key; `CacheValue::encode/decode` the value.
- `content.rs` — the one fetch path (`ClientCore::want`): memory hit →
  nothing; miss → `Effect::Load` when `DiskTier::Enabled` → `Input::Stored`
  miss → queued → `Send`, capped at `CacheConfig::max_in_flight`, one
  outstanding fetch per key. Daemon content is persisted on arrival; a
  session-local `DiskIndex` trims the disk budget via `Effect::Remove`.
- `Action::OpenReview`: local (`DiskTier::Disabled`) → `Request::OpenReview`
  stream; remote → piecewise (`ReviewSnapshot`, trees by key, `ListFiles`,
  headers via `FileRender` cancelled after chunk 0). See ARCHITECTURE §5.1.
- `Action::Viewport { file, first_row, last_row }` / `CloseFile`: chunk
  window = viewport ±`PREFETCH_RADIUS` (2), nearest first; queued fetches
  outside the window are dropped; open-file chunks pinned.
- `ServerMsg::TreeDelta` applied in place; `ReviewTargetsResolved` re-wants
  trees and re-lists files. `view.review` is `OpenReview { snapshot, trees,
  files, open_file }`; content itself is read from `core.cache()`.
- Tests: `tests/cache_flow.rs` (host KV simulator `Kv::drive`) covers every
  scenario in PLAN §3.2; `tests/state_machine.rs` proptest now generates
  the new actions, stream items and tree deltas and checks a rejected input
  leaves the cache untouched.

Known simplifications, all documented in code:
- The disk index does not survive restarts (entries from earlier sessions
  are counted again once loaded). Persisting the index is a later step.
- A viewport on a file whose header is unknown streams from chunk 0 and
  cancels after the radius (no chunk geometry to aim with yet).
- `ClientMsg::Cancel` is sent when a `FileRender` stream passes what was
  wanted; items after the cancel are still cached.

## Next: 3.3 optimistic mutations (PLAN §3.3, ARCHITECTURE §5.2)

- `PendingEvent` list in the core; local apply of `AddComment` marked
  pending; on own `Committed`/broadcast → drop pending; on foreign event →
  re-apply pending on top. LWW by `Seq` for edits and resolve toggles.
- `moor_test_support::Sim`: two `ClientCore`s + an in-memory daemon model
  with controllable delivery order; cases listed in PLAN §3.3; proptest
  that any interleaving converges both clients to the daemon state.
- Reconnect: pending re-sent exactly once, idempotent by `CommentId`.

Then 3.4 (deferred refresh — mostly present already, see `deferred` in
`lib.rs`; needs the re-anchor test), 3.5 `ViewModel`, 4.0 UI scaffold.

## Deferred / known gaps (unchanged)

- `moor-mcp`: `initialize`'s `clientInfo.name` is free text by design.
- Benchmark-triggered optimisations (PLAN "deferred").
- Per-version serialiser hook and Hunk comment watching not started.
