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
- Exhaustive matching on domain enums — no `_ =>` arms in `moor-review-core`/`moor-client-core`
  (clippy `wildcard_enum_match_arm` is on).
- Sans-I/O cores (`moor-client-core`) return `Vec<Effect>`; tests assert exactly which effects.
- **Names are enums, not strings — in adapters too.** Tool names, RPC methods, subcommands
  and config keys are parsed into an enum once at the edge (`ToolName`, `Method`,
  `ToolCall::parse`) and matched exhaustively; never `match s.as_str() { "list_reviews" => .. }`
  or a `const MUTATING: &[&str]`. Classification (read vs write, streamed vs single) is a
  type (`Call::Query | Call::Mutating`), so a new variant fails to compile until every
  site handles it. The wire spelling lives in one `serde`/`strum` rename, and a test
  checks that every variant is advertised.

### Testing — every piece proves itself

- Every crate ships with its own tests; a change without tests is incomplete.
- `insta` snapshots for render models, fixtures, and `ViewModel`s; `proptest` for
  invariants (round-trips, view rebuilds, anchoring, client convergence).
- Real git repos via `moor_test_support::RepoBuilder` — no mocked git.
- Protocol changes must update the JSON fixtures (`cargo xtask fixtures`) and the
  ReScript Sury schemas; the boundary round-trip test enforces both directions.
- `moor-client-core` behaviour under races is tested with the two-client simulator.

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
- **Sample data lives in dev-only crates.** `moor-protocol` ships wire types and nothing else; example values are in `moor-protocol-fixtures` (`publish = false`, used by `xtask` and its own tests).
- **Say what the underlying system stores.** When a field's precision or shape is dictated by
  git/redb/etc., the doc comment says so (e.g. git offsets are whole minutes).

## Conventions

- `moor-protocol` and `moor-client-core` must stay wasm-safe: no tokio, std I/O, threads,
  `Instant`, or non-`js` `rand`. CI checks `--target wasm32-unknown-unknown`.
- Serde enums with payloads are `#[serde(tag = "type")]` with `deny_unknown_fields`; unit-only enums serialise as bare PascalCase strings.
- Transports (`unix`, `ws`, `mcp`, `moor-cli`) are thin adapters over `Core`; never add a
  capability to a transport that `Core` doesn't have.
- Run before pushing: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run`.
