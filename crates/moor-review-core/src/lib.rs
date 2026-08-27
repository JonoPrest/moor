//! Moor review core: event store, git engine, diff render model, anchoring.
//!
//! Everything here is daemon-only. Public functions take and return
//! `moor-protocol` types so every transport is a thin adapter.

#![deny(clippy::wildcard_enum_match_arm)]

pub mod anchor;
pub mod comments;
pub mod core;
pub mod git;
pub mod patch;
pub mod render;
pub mod review;
pub mod store;

pub use core::{Core, CoreError, Ctx, DataDir};

pub use moor_protocol as protocol;
