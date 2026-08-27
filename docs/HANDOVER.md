# Handover notes

Written 2026-08-27 at the end of the session that finished Milestone 3 and
4.0–4.3 of Milestone 4. Read this, then `AGENTS.md`, then `docs/PLAN.md`.

## State of the tree

`main` is clean; every commit is pushed. Gates, all green:

- Rust: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  -- -D warnings` (clippy 1.97), `cargo check -p moor-protocol -p
  moor-client-core --target wasm32-unknown-unknown`, `cargo test
  --workspace`, `cargo xtask fixtures` (writes `fixtures/protocol` and
  `fixtures/client`; must be a no-op).
- UI (`ui/`, pnpm 11, node 24): `pnpm install`, `pnpm rescript`,
  `pnpm test` (vitest: CSS coverage, boundary round-trips over every
  fixture, adapter tests with mocked Tauri), `pnpm vite build`. CI runs
  both jobs (`.github/workflows/ci.yml`).

## Where things are

| area | crate / dir | notes |
| --- | --- | --- |
| client state machine | `crates/moor-client-core` | complete (3.1–3.5): cache, optimistic mutations, explorer, diff overlays, keymap/focus, `ViewPatch` |
| client fixtures | `crates/moor-client-core-fixtures` → `fixtures/client/` | one example per type/variant; boundary test consumes them |
| host loop | `crates/moor-client-host` | transport + KV + ticks + patches; tested against a real daemon |
| two-client simulator | `crates/moor-test-support/src/sim.rs` | `Sim` drives N cores against a daemon model |
| UI scaffold | `ui/` | ReScript 11 + React 19 + Vite 8 + Tailwind v4; `src/styles/app.css` tokens + semantic classes |
| UI schemas | `ui/src/protocol/*.res`, `ui/src/view/*.res` | hand-written Sury (rescript-schema 9.3.4) schemas; `Registry` / `ClientRegistry` by Rust type name |
| UI adapters | `ui/src/core/{Core,CoreTauri,CoreWasm}.res` | `dispatch` / `key` / `subscribe` / `attach`; patches applied by `Core.Store` |

## The host ↔ UI contract (4.2/4.3)

- Tauri commands the UI calls: `dispatch {action}`, `key {chord}`,
  `attach {}`. Events the host emits: `view` with an array of `ViewPatch`.
  `moor_client_host::Handle::{dispatch,key,attach}` are those commands;
  the patch receiver is what `emit("view", …)` drains.
- `ViewPatch` carries exactly one `ViewSection`; `review` (raw open review
  state) is never pushed. The UI keeps its own `ViewModel` copy
  (`View.empty` + `ViewPatch.apply`). Every patch stayed under 64 KB in
  the host test (100k-line file scrolled end to end) and the adapter test.

## Not done / blocked

- **Tauri wrapper crate** (`moor-client-tauri`): needs `webkit2gtk-4.1`
  and `gtk+-3.0` dev libraries to build; absent on this machine, so it is
  not written. It is thin: `tauri::Builder` with three commands that call
  `Handle`, a task draining the patch receiver into `app.emit("view", …)`,
  socket path from `moor-config`, KV at `app_data_dir()/kv.redb`,
  `IdSeed` from `getrandom`. CI would need the libs installed (the `ui`
  job does not build it).
- 4.4 screens and 4.5 keyboard UI: not started. `App.res` is a placeholder.
  Components render from `View.viewModel`; `Keys.ofBrowser` normalises
  `KeyboardEvent` → chord for `core.key`. Playwright against the Tauri
  dev build is impossible here for the same reason as above.
- `CoreWasm` is a stub that refuses actions (PLAN "Later").
- Edit-comment through the composer (an "edit draft") is not modelled;
  `Action::EditComment` takes the text directly.
- Stepping commits only moves a cursor; no per-commit render request
  exists in the protocol yet.
- Tree roots are labelled by `RepoId` (workspace repo names not fetched).
- The disk-tier LRU index is session-local (see `content.rs`).

## Design points worth knowing

- `ClientCore::handle` emits at most one `Render` per input, after
  `derive()` recomputed every derived panel; `ViewPatch`es are built from
  that delta. Add derived state to `derive()`, never render by hand.
- Rejected inputs leave view/connection/cache untouched (proptest), except
  the chord buffer, which resets on an unbound sequence.
- The daemon replays `Since::After` events *before* answering
  `Subscribe`; the core accepts events while `Connecting` once its
  `Subscribe` is out.
- Sury: rescript-schema 9.5 uses ReScript 12 syntax; 9.3.4 is the last
  version that compiles on ReScript 11. The package is namespaced:
  `-open RescriptSchema` in `rescript.json`.
