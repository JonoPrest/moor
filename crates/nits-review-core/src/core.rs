//! The `Core` façade: one type composing store, git, render and anchoring.
//! Every transport (unix, ws, mcp, cli) is a thin adapter over this.
//!
//! `Core` performs no clock or id generation of its own: callers pass a
//! [`Ctx`] carrying who is acting, when, and the client-side sequence. Ids
//! for created entities come from the client (see §5.2 optimistic creation).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use nits_protocol::{
    Author, ClientId, ClientSeq, EntityKind, Event, EventBody, RepoId, Seq, Timestamp,
};

use crate::git::{GitError, Repo};
use crate::render::Highlighter;
use crate::render::cache::{CacheError, RenderCache};
use crate::store::{NewEvent, Store, StoreError};

/// Who is acting, from where, and when. Built by the transport per request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctx {
    pub author: Author,
    pub client_id: ClientId,
    pub client_seq: ClientSeq,
    pub now: Timestamp,
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{kind:?} {id} not found")]
    NotFound { kind: EntityKind, id: String },
    #[error("invalid request: {reason}")]
    Invalid { reason: String },
    #[error("forbidden: {reason}")]
    Forbidden { reason: String },
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl CoreError {
    pub(crate) fn not_found(kind: EntityKind, id: &impl ToString) -> Self {
        CoreError::NotFound {
            kind,
            id: id.to_string(),
        }
    }
    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        CoreError::Invalid {
            reason: reason.into(),
        }
    }
    pub(crate) fn forbidden(reason: impl Into<String>) -> Self {
        CoreError::Forbidden {
            reason: reason.into(),
        }
    }
}

/// Files under the data dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    pub root: PathBuf,
}

impl DataDir {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    #[must_use]
    pub fn state(&self) -> PathBuf {
        self.root.join("state.redb")
    }
    #[must_use]
    pub fn render_cache(&self) -> PathBuf {
        self.root.join("render-cache.redb")
    }
}

pub struct Core {
    pub(crate) store: Store,
    pub(crate) cache: RenderCache,
    pub(crate) hl: Highlighter,
    repos: RwLock<HashMap<RepoId, Arc<Repo>>>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl Core {
    /// Open (or create) everything under `data_dir`.
    pub fn open(data_dir: &DataDir) -> Result<Self, CoreError> {
        std::fs::create_dir_all(&data_dir.root)?;
        Ok(Self {
            store: Store::open(&data_dir.state())?,
            cache: RenderCache::open(&data_dir.render_cache())?,
            hl: Highlighter::new(),
            repos: RwLock::new(HashMap::new()),
        })
    }

    pub(crate) fn append(&self, ctx: &Ctx, body: EventBody) -> Result<Event, CoreError> {
        Ok(self.store.append(NewEvent {
            ts: ctx.now,
            author: ctx.author.clone(),
            client_id: ctx.client_id,
            client_seq: ctx.client_seq,
            body,
        })?)
    }

    /// The opened repository for `id`, opening it from its stored path on
    /// first use.
    pub(crate) fn repo(&self, id: RepoId) -> Result<Arc<Repo>, CoreError> {
        if let Some(r) = self
            .repos
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
        {
            return Ok(Arc::clone(r));
        }
        let path = self
            .store
            .workspaces()?
            .iter()
            .flat_map(|w| w.repos.iter())
            .find(|r| r.id == id)
            .map(|r| PathBuf::from(&r.path))
            .ok_or_else(|| CoreError::not_found(EntityKind::Repo, &id))?;
        let repo = Arc::new(Repo::open(&path)?);
        self.repos
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, Arc::clone(&repo));
        Ok(repo)
    }

    pub(crate) fn open_repo_at(&self, id: RepoId, path: &Path) -> Result<(), CoreError> {
        let repo = Arc::new(Repo::open(path)?);
        self.repos
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, repo);
        Ok(())
    }

    pub(crate) fn forget_repo(&self, id: RepoId) {
        self.repos
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
    }

    // ---- log access -------------------------------------------------------

    pub fn events_after(&self, after: Option<Seq>) -> Result<Vec<Event>, CoreError> {
        Ok(self.store.events_after(after)?)
    }

    pub fn last_seq(&self) -> Result<Option<Seq>, CoreError> {
        Ok(self.store.last_seq()?)
    }
}
