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

### Testing — every piece proves itself

- Every crate ships with its own tests; a change without tests is incomplete.
- `insta` snapshots for render models, fixtures, and `ViewModel`s; `proptest` for
  invariants (round-trips, view rebuilds, anchoring, client convergence).
- Real git repos via `moor_test_support::RepoBuilder` — no mocked git.
- Protocol changes must update the JSON fixtures (`cargo xtask fixtures`) and the
  ReScript Sury schemas; the boundary round-trip test enforces both directions.
- `moor-client-core` behaviour under races is tested with the two-client simulator.

## Conventions

- `moor-protocol` and `moor-client-core` must stay wasm-safe: no tokio, std I/O, threads,
  `Instant`, or non-`js` `rand`. CI checks `--target wasm32-unknown-unknown`.
- All serde enums are `#[serde(tag = "type")]` with `deny_unknown_fields`.
- Transports (`unix`, `ws`, `mcp`, `moor-cli`) are thin adapters over `Core`; never add a
  capability to a transport that `Core` doesn't have.
- Run before pushing: `cargo fmt`, `cargo clippy -D warnings`, `cargo nextest run`.
