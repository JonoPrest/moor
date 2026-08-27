//! Moor review core: git engine, diff render model, anchoring, event store.
//!
//! Milestone 1.3+ fills this in. The crate exists now so the workspace, lints
//! (`wildcard_enum_match_arm`) and CI are wired from the start.

#![deny(clippy::wildcard_enum_match_arm)]

pub use moor_protocol as protocol;
