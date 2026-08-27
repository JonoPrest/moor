//! Content-keyed disk cache for render results.
//!
//! Key: `(RenderTarget, RenderOpts, lang)` — all derived from OIDs, so an
//! entry can never be stale; the cache is only ever trimmed. Header and
//! chunks are stored separately so a header can be served before all chunks
//! exist and so a single chunk read does not deserialise the whole file.

use std::path::Path;

use moor_protocol::{ChunkIndex, RenderChunk, RenderContent, RenderOpts, RenderTarget};
use redb::{Database, ReadableDatabase, TableDefinition};

use super::Rendered;

const RENDERS: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("renders");

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("encoding: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Redb(#[from] redb::Error),
}

macro_rules! from_redb {
    ($($e:ty),*) => {$(
        impl From<$e> for CacheError {
            fn from(e: $e) -> Self { CacheError::Redb(e.into()) }
        }
    )*};
}
from_redb!(
    redb::DatabaseError,
    redb::TransactionError,
    redb::TableError,
    redb::StorageError,
    redb::CommitError
);

/// Identifies one render. Serialised (canonically) to form the cache key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct RenderKey<'a> {
    pub target: &'a RenderTarget,
    pub opts: RenderOpts,
    pub lang: Option<&'a str>,
}

impl RenderKey<'_> {
    fn prefix(&self) -> Result<String, CacheError> {
        // JSON of a struct is deterministic for a fixed field order.
        Ok(serde_json::to_string(self)?)
    }
}

#[derive(Debug)]
pub struct RenderCache {
    db: Database,
}

impl RenderCache {
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        let db = Database::create(path)?;
        let txn = db.begin_write()?;
        txn.open_table(RENDERS)?;
        txn.commit()?;
        Ok(Self { db })
    }

    pub fn put(&self, key: &RenderKey<'_>, rendered: &Rendered) -> Result<(), CacheError> {
        let prefix = key.prefix()?;
        let txn = self.db.begin_write()?;
        {
            let mut t = txn.open_table(RENDERS)?;
            t.insert(
                format!("{prefix}/h").as_str(),
                serde_json::to_vec(&rendered.content)?.as_slice(),
            )?;
            for chunk in rendered.chunks() {
                t.insert(
                    format!("{prefix}/{}", chunk.index.get()).as_str(),
                    serde_json::to_vec(&chunk)?.as_slice(),
                )?;
            }
        }
        txn.commit()?;
        Ok(())
    }

    pub fn header(&self, key: &RenderKey<'_>) -> Result<Option<RenderContent>, CacheError> {
        let prefix = key.prefix()?;
        let txn = self.db.begin_read()?;
        let t = txn.open_table(RENDERS)?;
        t.get(format!("{prefix}/h").as_str())?
            .map(|v| Ok(serde_json::from_slice(v.value())?))
            .transpose()
    }

    pub fn chunk(
        &self,
        key: &RenderKey<'_>,
        index: ChunkIndex,
    ) -> Result<Option<RenderChunk>, CacheError> {
        let prefix = key.prefix()?;
        let txn = self.db.begin_read()?;
        let t = txn.open_table(RENDERS)?;
        t.get(format!("{prefix}/{}", index.get()).as_str())?
            .map(|v| Ok(serde_json::from_slice(v.value())?))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_protocol::{BlobOid, ChangeKind};

    #[test]
    fn header_and_chunks_roundtrip_and_miss_on_other_opts() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RenderCache::open(&dir.path().join("render.redb")).unwrap();
        let hl = super::super::Highlighter::new();
        let rendered = super::super::render_file(
            &hl,
            Some(b"a\nb\n"),
            Some(b"a\nc\n"),
            None,
            RenderOpts::default(),
        );
        let target = RenderTarget::Diff {
            change: ChangeKind::Modified {
                old: BlobOid::from_bytes([1; 20]),
                new: BlobOid::from_bytes([2; 20]),
            },
        };
        let key = RenderKey {
            target: &target,
            opts: RenderOpts::default(),
            lang: None,
        };
        assert!(cache.header(&key).unwrap().is_none());
        cache.put(&key, &rendered).unwrap();
        assert_eq!(cache.header(&key).unwrap(), Some(rendered.content.clone()));
        assert_eq!(
            cache.chunk(&key, ChunkIndex::FIRST).unwrap(),
            rendered.chunk(ChunkIndex::FIRST)
        );
        assert!(cache.chunk(&key, ChunkIndex::new(9)).unwrap().is_none());
        let other = RenderKey {
            opts: RenderOpts {
                ignore_whitespace: true,
                context_lines: 3,
            },
            ..key
        };
        assert!(cache.header(&other).unwrap().is_none());
    }
}
