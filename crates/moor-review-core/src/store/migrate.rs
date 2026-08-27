//! Forward-only schema migrations. One entry per version bump; the store
//! opens at any older version and is walked to `SchemaVersion::CURRENT`
//! one step per transaction.

use moor_protocol::SchemaVersion;
use redb::{Database, WriteTransaction};

use super::{StoreError, tables};

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("no migration from schema {from} to {to}")]
    Missing {
        from: SchemaVersion,
        to: SchemaVersion,
    },
    #[error("migration {from}->{to} failed: {reason}")]
    Failed {
        from: SchemaVersion,
        to: SchemaVersion,
        reason: String,
    },
}

type Migration = fn(&WriteTransaction) -> Result<(), String>;

/// `MIGRATIONS[n]` upgrades a store at schema `n` to `n + 1`.
///
/// Schema 0 is "a store created before versioning existed" (no `meta` stamp);
/// upgrading it to 1 is a no-op because the tables are identical.
const MIGRATIONS: &[Migration] = &[migrate_0_to_1];

#[allow(clippy::unnecessary_wraps)] // must match the `Migration` fn-pointer type
fn migrate_0_to_1(_txn: &WriteTransaction) -> Result<(), String> {
    Ok(())
}

/// Bring `db` from `stored` to `SchemaVersion::CURRENT`, or refuse.
pub(super) fn run(db: &Database, stored: SchemaVersion) -> Result<(), StoreError> {
    let current = SchemaVersion::CURRENT;
    if stored > current {
        return Err(StoreError::SchemaTooNew {
            found: stored,
            supported: current,
        });
    }
    let mut at = stored;
    while at < current {
        let next = SchemaVersion::new(at.get() + 1);
        let step = MIGRATIONS
            .get(at.get() as usize)
            .ok_or(MigrationError::Missing { from: at, to: next })?;
        let txn = db.begin_write()?;
        step(&txn).map_err(|reason| MigrationError::Failed {
            from: at,
            to: next,
            reason,
        })?;
        {
            let mut meta = txn.open_table(tables::META)?;
            meta.insert(tables::META_SCHEMA, u64::from(next.get()))?;
        }
        txn.commit()?;
        at = next;
    }
    Ok(())
}
