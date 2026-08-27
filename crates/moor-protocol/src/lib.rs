//! Moor wire protocol.
//!
//! Every type that crosses a process boundary — daemon ⇄ client, client-core ⇄
//! UI — lives here. The crate is wasm-safe: no I/O, no clocks, no threads.
//!
//! Conventions (see `AGENTS.md`): enums with payloads are
//! `#[serde(tag = "type")]`; enums whose variants are all unit serialise as a
//! bare `PascalCase` string (`"Open"`, not `{"type":"Open"}`); structs
//! and enums are `deny_unknown_fields`, ids are newtypes, invariants are
//! enforced by constructors. `cargo xtask fixtures` writes one JSON fixture per
//! enum variant under `fixtures/protocol/`; a test in this crate asserts that
//! every variant has one and that the on-disk files are current.

pub mod domain;
pub mod events;
pub mod fixtures;
pub mod ids;
pub mod invariants;
pub mod render;
pub mod rpc;
pub mod version;

pub use domain::*;
pub use events::*;
pub use ids::*;
pub use invariants::*;
pub use render::*;
pub use rpc::*;
pub use version::*;
