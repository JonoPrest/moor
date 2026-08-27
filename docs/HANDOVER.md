# Handover notes

Written 2026-08-27 at the end of the session that finished Milestones 3.2
through 3.5 (`moor-client-core` complete). Read this, then `AGENTS.md`,
then `docs/PLAN.md`.

## State of the tree

`main` is clean; every commit below is pushed. All gates green:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
(clippy 1.97), `cargo check -p moor-protocol -p moor-client-core --target
wasm32-unknown-unknown` (needs `rustup target add wasm32-unknown-unknown`),
`cargo test --workspace`, `cargo xtask fixtures` (unchanged). Run all five
before every commit.

Commits this session, oldest first:

- `ContentCache` memory tier (3.2 step 1)
- Fix lints new in clippy 1.97
- Content flow, disk tier, viewport prefetch (3.2)
- Proptest covers 3.2 inputs; ARCHITECTURE §5.1 on the two open flows
- Optimistic mutations, core side (3.3)
- `moor_test_support::Sim` two-client simulator + race tests (3.3)
- 3.4 test: draft anchors at the head seen when opened; daemon re-anchor
- Explorer, view prefs, viewed marks (3.5 part 1)
- Diff overlays, thread list, conversation, commit stepper (3.5 part 2)
- Keyboard model: keymap, focus, hints, help (3.5 part 3)
- `insta` snapshots of the `ViewModel`; docs (this commit)

## Map of `crates/moor-client-core/src`

| file | what |
| --- | --- |
| `lib.rs` | `ClientCore`, `Input`/`Action`/`Effect`/`CoreError`, connection + request routing, optimistic mutation bookkeeping (`committed` + `pending`, `rebase`), prefs/keymap KV round-trips, `handle` → `derive` (one `Render` per input) |
| `connection.rs` | `Connection` typestate |
| `cache.rs` | `ContentCache` memory LRU, `CacheKey`/`CacheValue`/`RenderKey`, byte budget, pins |
| `content.rs` | the one fetch path (memory → `Load` → queue → `Send`), `DiskTier`/`DiskIndex`, review open flows, viewport window, `TreeDelta`, `FileRef` |
| `events.rs` | pure `local_event` (mutation → optimistic event) and `apply_body` (event → snapshot); shared with the Sim's daemon model |
| `explorer.rs` | merged tree, expand/search state, fuzzy search, `viewed_state`, `Progress` |
| `diff.rs` | `DiffView` rows + overlays from anchors, `ThreadView` list, conversation, `CommitStepper`, `all_rows` for navigation |
| `keymap.rs` | `Context`, `Command`, `KeyChord`/`KeySeq` text form, default table, `Overrides`, conflicts, hints, help |
| `focus.rs` | `Focus`, `clamp`, `resolve(Command) → Action` |
| `view.rs` | `ViewModel` and its parts (`ViewPrefs`, `OpenReview`, `Draft`, …) |
| `ids.rs` | clock/RNG-free id minting |

Tests: `tests/state_machine.rs` (3.1 + proptest over every input kind,
now also asserting focus validity and cache untouched on rejection),
`tests/cache_flow.rs` (3.2 + explorer/prefs/viewed/diff/stepper scenarios,
host-KV simulator `Kv::drive`), `tests/sim.rs` (3.3 races over
`moor_test_support::Sim`, interleaving proptest), `tests/keys.rs` (3.5
keyboard: reachability, sequences, help, overrides),
`tests/snapshots.rs` (+ `tests/snapshots/*.snap`, `insta`; regenerate with
`INSTA_UPDATE=always cargo test -p moor-client-core --test snapshots` and
review the diff).

## Design points worth knowing

- `handle` collapses every `Render` of an input into one at the end,
  after `derive()` recomputed tree/progress/diff/threads/conversation/
  stepper/focus/hints/help and compared them with the view. Add new
  derived panels there; never render them by hand.
- Rejected inputs leave view, connection and cache untouched (proptest).
  The documented exception is the chord buffer, which resets on an
  unbound sequence.
- Commands are unit-only; `resolve` needs the view to make an `Action`.
  Actions that carry text (`DraftSubmitted`, `Reply`, `EditComment`) are
  host-supplied; `tests/keys.rs` lists them as the allowlist.
- The daemon replays `Since::After` events *before* answering `Subscribe`;
  the client accepts `Event` frames while `Connecting` once its
  `Subscribe` is out (this was a real bug found by the Sim).
- Disk tier: content is persisted on arrival; the disk LRU index is
  session-local (see `content.rs` docs). `DiskTier::Disabled` (local
  daemon) streams `OpenReview`; `Enabled` opens piecewise so a restart is
  served from the KV.

## Next: Milestone 4 (UI + Tauri), PLAN §4

- 4.0 scaffold: ReScript + React + Vite + Tailwind v4 under `ui/`, CI test
  that every `SpanClass` / `RowKind` / cell side has a class in
  `ui/src/styles/app.css` (ARCHITECTURE §6.6).
- The ReScript Sury schemas for `ViewModel`/`Action`/`Input` need writing
  against the Rust types (serde: payload enums `tag = "type"`, unit-only
  enums bare strings, `KeySeq` as a string like `"g g"`). Boundary
  fixtures for the client types are not generated yet — `xtask fixtures`
  only covers `moor-protocol`; extend it (or add a second fixture crate)
  before the boundary test.
- Hosts must: answer `Effect::Load` for `ViewPrefs::KEY` and
  `Keymap::KEY` (with `None` when absent), feed `Input::Tick` regularly
  (chord timeouts, id timestamps), send `Input::Key` for chords outside
  text inputs, and dispatch `Action::Viewport` on scroll.

## Deferred / known gaps

- `moor-mcp`: `initialize`'s `clientInfo.name` is free text by design.
- Benchmark-triggered optimisations (PLAN "deferred").
- Per-version serialiser hook and Hunk comment watching not started.
- Edit-comment through the composer (an "edit draft") is not modelled;
  `Action::EditComment` takes the text directly.
- `Command::Commits` / the stepper only list commits; stepping does not
  re-target the diff (no per-commit render request in the protocol yet).
- Tree roots are labelled by `RepoId`; workspace repo display names are
  not fetched by the core yet (`Request::ListWorkspaces` unused).
