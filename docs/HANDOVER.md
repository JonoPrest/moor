# Handover notes

Written 2026-08-27 at the end of the session that finished Milestone 3.1 and
the type-driven MCP rework. Read this, then `AGENTS.md`, then `docs/PLAN.md`.

## State of the tree

`main` is clean. Last commits:

- `742953c` MCP: derive tool schemas from the serde types with schemars
- `80c06da` MCP: typed `ToolCall` / `Method` dispatch (no string matching)
- `0f40355` Docs: Tailwind v4 decision for the UI (ARCHITECTURE §6.6, PLAN 4.0)
- `8c2b20b` Milestone 3.1: `moor-client-core` (sans-I/O client state machine)

All gates green at that point: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo check -p moor-protocol -p moor-client-core --target wasm32-unknown-unknown`,
`cargo test --workspace` (132 tests), `cargo xtask fixtures` (unchanged).
Run all five before every commit; use the commit trailer in `AGENTS.md`.

## Next chunk: Milestone 3.2 cache (`docs/PLAN.md` §3.2, ARCHITECTURE §5.1)

Everything lives in `crates/moor-client-core`. Suggested order, each a
committable step:

1. `cache.rs`: `ContentCache` — memory tier only. Keys are `Oid` for blobs /
   `(base, head, opts, chunk_index)` for diff chunks; values are headers and
   chunks, never whole files. LRU with a byte budget plus a pin set.
   Unit tests: hit/miss, eviction respects budget, pinned entries survive.
2. Wire `Action::OpenReview` to stream: today `Response::StreamItem` /
   `StreamEnd` are rejected as `UnexpectedResponse` in `lib.rs` `handle`.
   Add `InFlight::OpenReview { review_id }`, accept items, fill the cache,
   render `ViewSection::Review` once. Pin open-review headers.
3. Disk tier via effects: memory miss → `Effect::Load { key }` → host answers
   with `Input::Stored { key, value }` → on `None` emit `Effect::Send`.
   Eviction writes through with `Effect::Persist`. Dedupe concurrent misses
   (one outstanding request per key).
4. `TreeSnapshot` cached by root oid; apply `TreeDelta` in place (currently
   accepted with no effects when subscribed).
5. `Action::Viewport { file, first_row, last_row }` + prefetch policy
   (viewport chunk ±2, cancel queued requests that drift away).
6. Tests listed in PLAN §3.2, including the restart simulation (new
   `ClientCore` over the same KV map serves the review with no `Send`).

Design constraints to keep (see AGENTS.md):

- Core is sans-I/O: no clock, RNG, or I/O. Time comes from `Input::Tick`,
  ids from `IdGen` (`ids.rs`), storage only via `Persist`/`Load` effects.
- A rejected input (`Err(CoreError)`) must leave state and effects
  untouched; the proptest in `tests/state_machine.rs` enforces this — extend
  its action generator when you add `Action` variants.
- `#![deny(clippy::wildcard_enum_match_arm)]` is on: every new enum variant
  forces exhaustive matches. That is intentional.
- Names are enums, not strings; advertised sets are derived by iterating the
  enum; schemas come from the serde types (`schemars` behind
  `moor-protocol`'s `schema` feature). Do not hand-write `json!` schemas.

## After 3.2

- 3.3 optimistic mutations with the two-client `Sim` in `moor-test-support`.
- 3.4/3.5 per PLAN, then 4.0 UI scaffold (ReScript + React + Vite +
  Tailwind v4; CI test that every `SpanClass` / row kind / cell side has a
  class in `ui/src/styles/app.css`).

## Deferred / known gaps

- `moor-mcp`: `initialize`'s `clientInfo.name` is free text by design.
- Benchmark-triggered optimisations still deferred (see PLAN "deferred"):
  watcher path-only rehash, batched redb appends.
- Per-version serialiser hook and Hunk comment watching not started.
- The `strum::EnumIter` on `ToolName` drives `tools::all()`; adding a
  `ToolCall` variant will not compile until `ToolName::schemas()` has
  argument + result structs for it — that is the intended workflow.
