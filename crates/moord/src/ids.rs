//! Fresh entity ids for clients (CLI, MCP). Ids are client-generated (see
//! ARCHITECTURE §5.2): a ULID from the wall clock plus 80 random bits.

/// `(timestamp_ms, random)` parts for any `*Id::from_parts`.
#[must_use]
pub fn fresh_parts() -> (u64, u128) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    (ts, fastrand::u128(..))
}
