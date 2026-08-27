//! Moor review core: event store, git engine, diff render model, anchoring.
//!
//! Everything here is daemon-only. Public functions take and return
//! `moor-protocol` types so every transport is a thin adapter.

#![deny(clippy::wildcard_enum_match_arm)]

pub mod git;
pub mod store;

pub use moor_protocol as protocol;
