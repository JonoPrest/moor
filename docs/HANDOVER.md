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
| UI scaffold | `ui/` | ReScript 12 + React 19 + Vite 8 + Tailwind v4; `src/styles/app.css` tokens + semantic classes; conventions follow the Envio UI (`~/code/ui`), see AGENTS.md |
| UI schemas | `ui/src/protocol/*.res`, `ui/src/view/*.res` | `@schema`-derived Sury schemas (rescript-schema 9.3.0-rescript12.0 + ppx 9.0.1); `Registry` / `ClientRegistry` by Rust type name |
| UI components | `ui/src/ui/*.res`, `ui/src/App.res` | design system `UI.res`; Row/DiffView (virtualized)/Tree/Threads/Composer/Stepper/HintBar/HelpOverlay/SearchBox; key capture in `App` |
| UI tests | `ui/__tests__/*_test.res` (ReScript, vitest+jsdom), `ui/tests/*.test.ts` (fixture harness) | 380 checks |
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

## What landed after 4.3

- ReScript 12 + `@schema` ppx migration of every schema; `UI.res` design
  system; tests in ReScript (see AGENTS.md "UI").
- Screens: review list with create form (any base vs head, multi-repo,
  `RefSpecText` parser), tree with viewed checkboxes and repo display
  names, virtualized diff with placeholders and viewed-collapse, composer,
  threads with suggestion apply, conversation, commit stepper with commit
  panel, hint bar, help overlay, fuzzy search, key capture, focus
  scroll-into-view.
- Core: workspaces listed on subscribe (reviews = union of all
  workspaces), `Action::{ListWorkspaces, CreateReview, ApplySuggestion}`,
  `Command::{Refresh, ApplySuggestion}`, `StepperCommit` details,
  `DiffView.viewed`, `ThreadView.suggestion`.

## Not done / blocked

- **Tauri wrapper crate** (`moor-client-tauri`, binary `moor-desktop`):
  landed 2026-08-27 (macOS needs no extra libs; CI's rust job now installs
  webkit2gtk for Ubuntu). `tauri::Builder` with three commands that call
  `Handle`, a task draining the patch receiver into `app.emit("view", …)`,
  socket via `moor-config` → `moord::contexts::local_spec` +
  `ensure_daemon` (autostart), KV at `app_data_dir()/kv.redb`, seed from
  `fastrand`. `moor-desktop [context]`; only `Local` contexts work (ssh/ws
  → `SetupError::NotLocal`, see 4.6). Dev: `cargo tauri dev` is not
  installed; `pnpm --dir ui build` then `cargo run -p moor-client-tauri`
  serves `ui/dist`. Icon is a placeholder square. Smoke-tested by hand:
  launches, daemon autostarts, no panic; Playwright still not wired.
  Gotcha: `moor_client_host::spawn` calls `tokio::spawn`, and Tauri's
  `setup` runs outside its runtime — `start_host` enters
  `tauri::async_runtime::handle()` first.
- 4.4: expanders — `Row::Expander` renders but has no action; the
  protocol has no "expand hidden lines" request (a render with more
  context, or `BlobRender` rows spliced in, would be needed). Playwright
  against the Tauri dev build is impossible here (no Tauri libs), so the
  smoke/keyboard-only flows exist only as component tests.
- 4.6: remote connection UX (ssh target picker) is a host concern —
  `moor-client-host` takes a socket path; the picker belongs in the Tauri
  wrapper with `moor-config` contexts.
- `CoreWasm` is a stub that refuses actions (PLAN "Later").
- Edit-comment through the composer (an "edit draft") is not modelled;
  `Action::EditComment` takes the text directly.
- Stepping commits only moves a cursor; no per-commit render request
  exists in the protocol yet.
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
- Sury: the `-rescript12.0` line of rescript-schema is the one for
  ReScript 12 (9.5.x also uses 12 syntax but the reference pins 9.3.0);
  the package is namespaced: `-open RescriptSchema` in `rescript.json`.
  `pnpm-workspace.yaml` must allow the build scripts of `rescript`,
  `rescript-schema-ppx` and `@rescript/react` (pnpm 11 `allowBuilds`).
