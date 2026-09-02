# AGENTS.md

Guidance for anyone (human or agent) working in this repo. Read `docs/ARCHITECTURE.md`
for the design and `docs/PLAN.md` for the milestones before making changes.

## Principles

### Type-driven design — make invalid state unrepresentable

- Newtypes for every ID and OID (`ReviewId`, `CommentId`, `BlobOid`, `Seq`, …). No bare
  `String`/`u64` in signatures.
- Enums over flags: `Anchor = Review | File | Lines`, `CommentState = Live | Outdated | Deleted`.
  Never `Option<a> + Option<b>` or `deleted: bool`.
- Parse, don't validate: wire input becomes a validated domain type once at the boundary
  (`TryFrom<protocol::X>`); the core never re-checks.
- Distinct types for distinct lifecycle stages: `RefSpec` vs `ResolvedTarget`,
  `PendingEvent` vs `CommittedEvent`, typestate for connections.
- `NonEmpty` and range types with constructor-enforced invariants where required.
- Exhaustive matching on domain enums — no `_ =>` arms in `nits-review-core`/`nits-client-core`
  (clippy `wildcard_enum_match_arm` is on).
- Sans-I/O cores (`nits-client-core`) return `Vec<Effect>`; tests assert exactly which effects.
- **Names are enums, not strings — in adapters too.** Tool names, RPC methods, subcommands
  and config keys are parsed into an enum once at the edge (`ToolName`, `Method`,
  `ToolCall::parse`) and matched exhaustively; never `match s.as_str() { "list_reviews" => .. }`
  or a `const MUTATING: &[&str]`. Classification (read vs write, streamed vs single) is a
  type (`Call::Query | Call::Mutating`), so a new variant fails to compile until every
  site handles it. The wire spelling lives in one `serde`/`strum` rename; what is advertised
  is *derived* by iterating the enum (`ToolName::iter()`), and schemas come from the serde
  types themselves (`schemars` behind `nits-protocol`'s `schema` feature) — never a
  hand-written `json!` schema beside a `Deserialize` struct.

### Testing — every piece proves itself

- Every crate ships with its own tests; a change without tests is incomplete.
- `insta` snapshots for render models, fixtures, and `ViewModel`s; `proptest` for
  invariants (round-trips, view rebuilds, anchoring, client convergence).
- Real git repos via `nits_test_support::RepoBuilder` — no mocked git.
- Protocol changes must update the JSON fixtures (`cargo xtask fixtures`) and the
  ReScript Sury schemas; the boundary round-trip test enforces both directions.
- `nits-client-core` behaviour under races is tested with the two-client simulator.

## Style (from review)

- **Lift shared fields out of enums.** If every variant carries the same field, it belongs on
  an outer struct; the enum holds only what differs (`ResolvedRef { tree, source }`, not
  `Commit { tree, .. } | WorkingTree { tree, .. }` plus a `tree()` accessor).
- **Unit-only enums are bare strings on the wire** (`"Open"`), not `{"type":"Open"}`. Only
  payload-carrying enums get `tag = "type"`.
- **No panics outside tests.** Library code (including fixture builders and xtask helpers)
  returns `Result`; `expect`/`unwrap` are allowed only in `#[cfg(test)]` modules and `tests/`,
  which other files cannot reach. An `&str` argument is untrusted until a type has validated it.
- **Document macros.** Every `macro_rules!` has a doc comment saying what it expands to and
  why it exists; readers should not have to expand it mentally.
- **Placeholder data is neutral.** Fixtures and tests use invented names (`ada@example.com`),
  never real people or paths from a contributor's machine.
- **Sample data lives in dev-only crates.** `nits-protocol` ships wire types and nothing else; example values are in `nits-protocol-fixtures` (`publish = false`, used by `xtask` and its own tests).
- **Say what the underlying system stores.** When a field's precision or shape is dictated by
  git/redb/etc., the doc comment says so (e.g. git offsets are whole minutes).

## Conventions

- `nits-protocol` and `nits-client-core` must stay wasm-safe: no tokio, std I/O, threads,
  `Instant`, or non-`js` `rand`. CI checks `--target wasm32-unknown-unknown`.
- Serde enums with payloads are `#[serde(tag = "type")]` with `deny_unknown_fields`; unit-only enums serialise as bare PascalCase strings.
- Transports (`unix`, `ws`, `mcp`, `nits-cli`) are thin adapters over `Core`; never add a
  capability to a transport that `Core` doesn't have.
- Run before pushing: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run`.

## UI (`ui/`, ReScript + React)

Product design: `docs/UI-DESIGN.md` (settled on the design canvas; working
artboards in `design/`). Keyboard-first is a hard rule there: every control
is a mouse alias for a keymap chord, tooltips derive from the keymap, and
canonical bindings are terminal-safe for TUI parity.

Modelled on the Envio UI (`~/code/ui`), which is the reference for how to work with
ReScript and React here.

- ReScript 12 with warnings as errors (`+A-4-9-102-3`): no dead variables, no `_foo` to
  silence them — remove the code instead.
- **`@schema` on the type**, never a hand-written `S.object` (only for recursion or
  generics). One `module X = { @schema type t = ... }` per Rust type; `@as("snake_case")`
  on fields, `@tag("type")` + `@as("Variant") Variant({})` for payload enums (wrap in
  `@@warning("-27")`), `@s.null option<_>` for serde `Option`. The boundary test over
  `fixtures/` proves the shape.
- **Design system in `src/ui/UI.res`**: shared primitives (`Panel`, `Button`, `Kbd`,
  `Badge`, `Box`, `TextInput`) take configuration props (`~kind`, `~tone`, `~gap`), never a
  `className` escape hatch. Extend a primitive with a variant rather than adding a parallel
  one. The render model's semantic classes (`row-*`, `cell-*`, `span-*`) are the exception
  and live in `src/styles/app.css` (§6.6).
- **Tests are written in ReScript** (`__tests__/*_test.res`, run by vitest through the
  bindings in `__tests__/Vitest.res` / `TestingLibrary.res`); the fixture-driven harness
  tests (`tests/*.test.ts`) stay in TypeScript because they only read files and mock modules.
- Components never touch Sury or IPC: `Core.res` is the only door (dispatch / key /
  subscribe / attach).
- Run before pushing: `pnpm rescript`, `pnpm test`, `pnpm vite build`.

