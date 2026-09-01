//! Workspaces, reviews, files, commits, renders and viewed marks on `Core`.

use moor_protocol::{
    BlobOid, ChangeKind, CommitInfo, CommitOid, DiffScope, EntityKind, EventBody, FileChange,
    FileRenderHeader, NonEmpty, RefSpec, RenderOpts, RenderTarget, Repo, RepoId, RepoPath,
    ResolvedRef, ResolvedSource, ResolvedTarget, Review, ReviewId, ReviewSnapshot, ReviewStatus,
    ReviewTarget, TreeDelta, TreeEntryKind, TreeSnapshot, ViewedMark, Workspace, WorkspaceId,
};

use crate::core::{Core, CoreError, Ctx};
use crate::render::cache::RenderKey;
use crate::render::{Rendered, render_blob, render_file};
use crate::store::{ReviewLifecycle, ReviewRecord};

/// Derived per-file viewed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewedState {
    Unviewed,
    Viewed,
    /// Marked viewed at a different head blob than the current one.
    ChangedSinceViewed {
        marked: Option<BlobOid>,
    },
}

impl Core {
    // ---- workspaces -------------------------------------------------------

    pub fn workspaces(&self) -> Result<Vec<Workspace>, CoreError> {
        Ok(self.store.workspaces()?)
    }

    pub fn workspace(&self, id: WorkspaceId) -> Result<Workspace, CoreError> {
        self.store
            .workspace(id)?
            .ok_or_else(|| CoreError::not_found(EntityKind::Workspace, &id))
    }

    pub fn create_workspace(
        &self,
        ctx: &Ctx,
        id: WorkspaceId,
        name: String,
    ) -> Result<Workspace, CoreError> {
        if self.store.workspace(id)?.is_some() {
            return Err(CoreError::invalid(format!("workspace {id} already exists")));
        }
        if name.trim().is_empty() {
            return Err(CoreError::invalid("workspace name must not be empty"));
        }
        let workspace = Workspace {
            id,
            name,
            repos: vec![],
        };
        self.append(
            ctx,
            EventBody::WorkspaceCreated {
                workspace: workspace.clone(),
            },
        )?;
        Ok(workspace)
    }

    pub fn rename_workspace(
        &self,
        ctx: &Ctx,
        id: WorkspaceId,
        name: String,
    ) -> Result<(), CoreError> {
        self.workspace(id)?;
        self.append(
            ctx,
            EventBody::WorkspaceUpdated {
                workspace_id: id,
                name,
            },
        )?;
        Ok(())
    }

    /// Attach a repository; the path must be a git work tree.
    pub fn attach_repo(
        &self,
        ctx: &Ctx,
        workspace_id: WorkspaceId,
        repo_id: RepoId,
        path: &str,
        display_name: String,
    ) -> Result<Repo, CoreError> {
        let ws = self.workspace(workspace_id)?;
        if ws.repos.iter().any(|r| r.id == repo_id) {
            return Err(CoreError::invalid(format!(
                "repo {repo_id} already attached"
            )));
        }
        let canonical = std::fs::canonicalize(path)?;
        self.open_repo_at(repo_id, &canonical)?;
        let repo = Repo {
            id: repo_id,
            path: canonical.to_string_lossy().into_owned(),
            display_name,
        };
        self.append(
            ctx,
            EventBody::RepoAttached {
                workspace_id,
                repo: repo.clone(),
            },
        )?;
        Ok(repo)
    }

    pub fn detach_repo(
        &self,
        ctx: &Ctx,
        workspace_id: WorkspaceId,
        repo_id: RepoId,
    ) -> Result<(), CoreError> {
        let ws = self.workspace(workspace_id)?;
        if !ws.repos.iter().any(|r| r.id == repo_id) {
            return Err(CoreError::not_found(EntityKind::Repo, &repo_id));
        }
        self.append(
            ctx,
            EventBody::RepoDetached {
                workspace_id,
                repo_id,
            },
        )?;
        self.forget_repo(repo_id);
        Ok(())
    }

    // ---- reviews ----------------------------------------------------------

    pub fn reviews(&self, workspace_id: WorkspaceId) -> Result<Vec<Review>, CoreError> {
        Ok(self
            .store
            .reviews(workspace_id)?
            .into_iter()
            .map(|r| r.review)
            .collect())
    }

    /// A live review's record (review + resolved targets).
    pub fn review(&self, id: ReviewId) -> Result<ReviewRecord, CoreError> {
        let rec = self
            .store
            .review(id)?
            .ok_or_else(|| CoreError::not_found(EntityKind::Review, &id))?;
        match rec.lifecycle {
            ReviewLifecycle::Live => Ok(rec),
            ReviewLifecycle::Deleted { .. } => Err(CoreError::not_found(EntityKind::Review, &id)),
        }
    }

    /// Create a review and resolve its targets immediately.
    pub fn create_review(
        &self,
        ctx: &Ctx,
        id: ReviewId,
        workspace_id: WorkspaceId,
        title: String,
        targets: NonEmpty<ReviewTarget>,
    ) -> Result<ReviewRecord, CoreError> {
        let ws = self.workspace(workspace_id)?;
        if self.store.review(id)?.is_some() {
            return Err(CoreError::invalid(format!("review {id} already exists")));
        }
        for t in &targets {
            if !ws.repos.iter().any(|r| r.id == t.repo_id) {
                return Err(CoreError::invalid(format!(
                    "repo {} is not in workspace {workspace_id}",
                    t.repo_id
                )));
            }
        }
        // Pre-flight: every target must resolve before anything is
        // committed, or an unresolvable base (say `Upstream` with no
        // upstream configured) would leave a ghost review behind.
        for t in &targets {
            let repo = self.repo(t.repo_id)?;
            repo.resolve(&t.base)?;
            repo.resolve(&t.head)?;
        }
        let review = Review {
            id,
            workspace_id,
            title,
            targets,
            created: ctx.now,
            status: ReviewStatus::Open,
        };
        self.append(
            ctx,
            EventBody::ReviewCreated {
                review: review.clone(),
            },
        )?;
        let (resolved, _) = self.resolve_targets(ctx, id)?;
        Ok(ReviewRecord {
            review,
            resolved: Some(resolved),
            lifecycle: ReviewLifecycle::Live,
        })
    }

    pub fn update_review(
        &self,
        ctx: &Ctx,
        id: ReviewId,
        title: String,
        status: ReviewStatus,
    ) -> Result<(), CoreError> {
        self.review(id)?;
        self.append(
            ctx,
            EventBody::ReviewUpdated {
                review_id: id,
                title,
                status,
            },
        )?;
        Ok(())
    }

    pub fn delete_review(&self, ctx: &Ctx, id: ReviewId) -> Result<(), CoreError> {
        self.review(id)?;
        self.append(ctx, EventBody::ReviewDeleted { review_id: id })?;
        Ok(())
    }

    /// Re-resolve every target. Emits `ReviewTargetsResolved` only when the
    /// resolved OIDs differ from the stored ones, then re-anchors comments.
    /// Returns `(targets, changed)`.
    pub fn resolve_targets(
        &self,
        ctx: &Ctx,
        id: ReviewId,
    ) -> Result<(NonEmpty<ResolvedTarget>, bool), CoreError> {
        let rec = self.review(id)?;
        let mut resolved = Vec::new();
        for t in &rec.review.targets {
            let repo = self.repo(t.repo_id)?;
            resolved.push(ResolvedTarget {
                repo_id: t.repo_id,
                base: repo.resolve(&t.base)?,
                head: repo.resolve(&t.head)?,
            });
        }
        let resolved = NonEmpty::new(resolved).map_err(|e| CoreError::invalid(e.to_string()))?;
        let changed = rec.resolved.as_ref() != Some(&resolved);
        if changed {
            self.append(
                ctx,
                EventBody::ReviewTargetsResolved {
                    review_id: id,
                    targets: resolved.clone(),
                },
            )?;
            if let Some(old) = &rec.resolved {
                self.reanchor_review(ctx, id, old, &resolved)?;
            }
        }
        Ok((resolved, changed))
    }

    fn resolved(
        &self,
        id: ReviewId,
    ) -> Result<(ReviewRecord, NonEmpty<ResolvedTarget>), CoreError> {
        let rec = self.review(id)?;
        let resolved = rec
            .resolved
            .clone()
            .ok_or_else(|| CoreError::invalid(format!("review {id} has not been resolved")))?;
        Ok((rec, resolved))
    }

    pub(crate) fn target(
        resolved: &NonEmpty<ResolvedTarget>,
        repo_id: RepoId,
    ) -> Result<ResolvedTarget, CoreError> {
        resolved
            .iter()
            .find(|t| t.repo_id == repo_id)
            .cloned()
            .ok_or_else(|| CoreError::not_found(EntityKind::Repo, &repo_id))
    }

    /// The targets `scope` selects (UI-DESIGN §Diff scope), starting from
    /// the review's resolved targets. `Commit` and `Worktree` must name a
    /// repo the review targets.
    pub fn scoped_targets(
        &self,
        id: ReviewId,
        scope: &DiffScope,
    ) -> Result<NonEmpty<ResolvedTarget>, CoreError> {
        let (_, resolved) = self.resolved(id)?;
        match scope {
            DiffScope::All => Ok(resolved),
            DiffScope::Committed => {
                let mapped = resolved
                    .into_iter()
                    .map(|mut t| {
                        if matches!(t.head.source, ResolvedSource::WorkingTree { .. }) {
                            t.head = self.repo(t.repo_id)?.resolve(&RefSpec::Head)?;
                        }
                        Ok(t)
                    })
                    .collect::<Result<Vec<_>, CoreError>>()?;
                NonEmpty::new(mapped).map_err(|_| CoreError::invalid("review has no targets"))
            }
            DiffScope::Commit { repo_id, oid } => {
                Self::target(&resolved, *repo_id)?;
                Ok(NonEmpty::singleton(self.commit_step(*repo_id, *oid)?))
            }
            DiffScope::Worktree { repo_id } => {
                Self::target(&resolved, *repo_id)?;
                let repo = self.repo(*repo_id)?;
                Ok(NonEmpty::singleton(ResolvedTarget {
                    repo_id: *repo_id,
                    base: repo.resolve(&RefSpec::Head)?,
                    head: repo.resolve(&RefSpec::WorkingTree)?,
                }))
            }
        }
    }

    /// Every changed file across all targets, ordered by repo display name
    /// then path. This is the flat form of the merged tree.
    pub fn files(&self, id: ReviewId) -> Result<Vec<FileChange>, CoreError> {
        self.files_scoped(id, &DiffScope::All)
            .map(|(files, _)| files)
    }

    /// [`Self::files`] under `scope`, with the targets the files were
    /// diffed between.
    pub fn files_scoped(
        &self,
        id: ReviewId,
        scope: &DiffScope,
    ) -> Result<(Vec<FileChange>, NonEmpty<ResolvedTarget>), CoreError> {
        let rec = self.review(id)?;
        let resolved = self.scoped_targets(id, scope)?;
        let ws = self.workspace(rec.review.workspace_id)?;
        let mut targets: Vec<&ResolvedTarget> = resolved.iter().collect();
        let name = |rid: RepoId| {
            ws.repos
                .iter()
                .find(|r| r.id == rid)
                .map(|r| r.display_name.clone())
                .unwrap_or_default()
        };
        targets.sort_by_key(|t| (name(t.repo_id), t.repo_id));
        let mut out = Vec::new();
        for t in targets {
            let repo = self.repo(t.repo_id)?;
            let mut changes = repo.changed_files(t.base.tree, t.head.tree)?;
            changes.sort_by(|a, b| a.path.cmp(&b.path));
            out.extend(changes.into_iter().map(|c| FileChange {
                repo_id: t.repo_id,
                path: c.path,
                kind: c.kind,
            }));
        }
        Ok((out, resolved))
    }

    /// The change for one file, if it is in the review under `scope`.
    pub fn file_change(
        &self,
        id: ReviewId,
        repo_id: RepoId,
        path: &RepoPath,
        scope: &DiffScope,
    ) -> Result<FileChange, CoreError> {
        self.files_scoped(id, scope)?
            .0
            .into_iter()
            .find(|f| f.repo_id == repo_id && &f.path == path)
            .ok_or_else(|| CoreError::not_found(EntityKind::Path, &path))
    }

    /// Commits between base and head for one repo target (newest first).
    /// A working-tree head steps through the checked-out branch's commits
    /// (the worktree itself is the final step). Empty when the base is not
    /// a commit.
    pub fn commits(&self, id: ReviewId, repo_id: RepoId) -> Result<Vec<CommitInfo>, CoreError> {
        let (_, resolved) = self.resolved(id)?;
        let t = Self::target(&resolved, repo_id)?;
        let ResolvedSource::Commit { oid: base } = t.base.source else {
            return Ok(vec![]);
        };
        let head = match t.head.source {
            ResolvedSource::Commit { oid } => oid,
            ResolvedSource::WorkingTree { .. } => {
                match self.repo(repo_id)?.resolve(&RefSpec::Head)?.source {
                    ResolvedSource::Commit { oid } => oid,
                    ResolvedSource::WorkingTree { .. } => return Ok(vec![]),
                }
            }
        };
        Ok(self.repo(repo_id)?.commits_between(base, head)?)
    }

    /// A single-commit sub-target for stepping: base = first parent.
    pub fn commit_step(
        &self,
        repo_id: RepoId,
        commit: CommitOid,
    ) -> Result<ResolvedTarget, CoreError> {
        let repo = self.repo(repo_id)?;
        let info = repo.commit_info(commit)?;
        let base = match info.parents.first() {
            Some(p) => {
                let parent = repo.commit_info(*p)?;
                ResolvedRef {
                    tree: parent.tree,
                    source: ResolvedSource::Commit { oid: *p },
                }
            }
            None => ResolvedRef {
                tree: EMPTY_TREE,
                source: ResolvedSource::Commit { oid: commit },
            },
        };
        Ok(ResolvedTarget {
            repo_id,
            base,
            head: ResolvedRef {
                tree: info.tree,
                source: ResolvedSource::Commit { oid: commit },
            },
        })
    }

    pub fn tree_snapshot(
        &self,
        repo_id: RepoId,
        spec: &RefSpec,
    ) -> Result<TreeSnapshot, CoreError> {
        let repo = self.repo(repo_id)?;
        let r = repo.resolve(spec)?;
        Ok(repo.tree_snapshot(repo_id, r.tree)?)
    }

    pub fn tree_snapshot_of(
        &self,
        repo_id: RepoId,
        r: &ResolvedRef,
    ) -> Result<TreeSnapshot, CoreError> {
        Ok(self.repo(repo_id)?.tree_snapshot(repo_id, r.tree)?)
    }

    // ---- render -----------------------------------------------------------

    /// Render one changed file of a review under `scope`. Cached by content.
    pub fn file_render(
        &self,
        id: ReviewId,
        repo_id: RepoId,
        path: &RepoPath,
        opts: RenderOpts,
        scope: &DiffScope,
    ) -> Result<(FileRenderHeader, Rendered), CoreError> {
        let change = self.file_change(id, repo_id, path, scope)?;
        self.render_change(repo_id, path, change.kind, opts)
    }

    /// Render a change directly (used by commit stepping).
    pub fn render_change(
        &self,
        repo_id: RepoId,
        path: &RepoPath,
        kind: ChangeKind,
        opts: RenderOpts,
    ) -> Result<(FileRenderHeader, Rendered), CoreError> {
        let repo = self.repo(repo_id)?;
        let old = kind.old_blob().map(|b| repo.blob(b)).transpose()?;
        let new = kind.new_blob().map(|b| repo.blob(b)).transpose()?;
        let sample = new.as_deref().or(old.as_deref()).unwrap_or_default();
        let lang = self.hl.detect(path.as_str(), first_line(sample));
        let target = RenderTarget::Diff { change: kind };
        let rendered = self.cached_render(&target, opts, lang.as_deref(), || {
            render_file(
                &self.hl,
                old.as_deref(),
                new.as_deref(),
                lang.as_deref(),
                opts,
            )
        })?;
        Ok((
            FileRenderHeader {
                repo_id,
                path: path.clone(),
                target,
                opts,
                lang,
                content: rendered.content.clone(),
            },
            rendered,
        ))
    }

    /// Render a blob for the explorer.
    pub fn blob_render(
        &self,
        repo_id: RepoId,
        path: &RepoPath,
        blob: BlobOid,
    ) -> Result<(FileRenderHeader, Rendered), CoreError> {
        let repo = self.repo(repo_id)?;
        let bytes = repo.blob(blob)?;
        let lang = self.hl.detect(path.as_str(), first_line(&bytes));
        let target = RenderTarget::Blob { oid: blob };
        let opts = RenderOpts::default();
        let rendered = self.cached_render(&target, opts, lang.as_deref(), || {
            render_blob(&self.hl, &bytes, lang.as_deref())
        })?;
        Ok((
            FileRenderHeader {
                repo_id,
                path: path.clone(),
                target,
                opts,
                lang,
                content: rendered.content.clone(),
            },
            rendered,
        ))
    }

    /// Serve from the render cache or compute and store.
    fn cached_render(
        &self,
        target: &RenderTarget,
        opts: RenderOpts,
        lang: Option<&str>,
        render: impl FnOnce() -> Rendered,
    ) -> Result<Rendered, CoreError> {
        let key = RenderKey { target, opts, lang };
        if let Some(content) = self.cache.header(&key)? {
            let mut rows = Vec::new();
            let mut i = 0;
            while let Some(chunk) = self.cache.chunk(&key, moor_protocol::ChunkIndex::new(i))? {
                rows.extend(chunk.rows);
                i += 1;
            }
            return Ok(Rendered { content, rows });
        }
        let rendered = render();
        self.cache.put(&key, &rendered)?;
        Ok(rendered)
    }

    /// Raw blob bytes from a repo (CLI/MCP `get_file`).
    pub fn repo_blob(&self, repo_id: RepoId, blob: BlobOid) -> Result<Vec<u8>, CoreError> {
        Ok(self.repo(repo_id)?.blob(blob)?)
    }

    // ---- viewed -----------------------------------------------------------

    /// Head-side blob for `path` in the review (None if absent in head).
    pub(crate) fn head_blob(
        &self,
        id: ReviewId,
        repo_id: RepoId,
        path: &RepoPath,
    ) -> Result<Option<BlobOid>, CoreError> {
        let (_, resolved) = self.resolved(id)?;
        let t = Self::target(&resolved, repo_id)?;
        let snap = self.repo(repo_id)?.tree_snapshot(repo_id, t.head.tree)?;
        Ok(snap
            .entries
            .iter()
            .find(|e| &e.path == path)
            .and_then(|e| match e.kind {
                TreeEntryKind::File { oid, .. } | TreeEntryKind::Symlink { oid } => Some(oid),
                TreeEntryKind::Dir { .. } | TreeEntryKind::Submodule { .. } => None,
            }))
    }

    /// Human-only: agents get `Forbidden`.
    pub fn mark_viewed(
        &self,
        ctx: &Ctx,
        id: ReviewId,
        repo_id: RepoId,
        path: RepoPath,
    ) -> Result<ViewedMark, CoreError> {
        let viewer = ctx
            .author
            .as_human()
            .ok_or_else(|| CoreError::forbidden("agents cannot mark files as viewed"))?;
        let blob_oid = self.head_blob(id, repo_id, &path)?;
        let mark = ViewedMark {
            review_id: id,
            repo_id,
            path: path.clone(),
            viewer: viewer.clone(),
            blob_oid,
        };
        self.append(
            ctx,
            EventBody::FileViewed {
                review_id: id,
                repo_id,
                path,
                viewer,
                blob_oid,
            },
        )?;
        Ok(mark)
    }

    pub fn unmark_viewed(
        &self,
        ctx: &Ctx,
        id: ReviewId,
        repo_id: RepoId,
        path: RepoPath,
    ) -> Result<(), CoreError> {
        let viewer = ctx
            .author
            .as_human()
            .ok_or_else(|| CoreError::forbidden("agents cannot mark files as viewed"))?;
        self.review(id)?;
        self.append(
            ctx,
            EventBody::FileUnviewed {
                review_id: id,
                repo_id,
                path,
                viewer,
            },
        )?;
        Ok(())
    }

    pub fn viewed_marks(&self, id: ReviewId) -> Result<Vec<ViewedMark>, CoreError> {
        Ok(self.store.viewed(id)?)
    }

    pub fn viewed_state(
        &self,
        id: ReviewId,
        repo_id: RepoId,
        path: &RepoPath,
    ) -> Result<ViewedState, CoreError> {
        let Some(mark) = self
            .store
            .viewed(id)?
            .into_iter()
            .find(|m| m.repo_id == repo_id && &m.path == path)
        else {
            return Ok(ViewedState::Unviewed);
        };
        let current = self.head_blob(id, repo_id, path)?;
        Ok(if current == mark.blob_oid {
            ViewedState::Viewed
        } else {
            ViewedState::ChangedSinceViewed {
                marked: mark.blob_oid,
            }
        })
    }

    // ---- snapshot ---------------------------------------------------------

    /// Materialised state for a fresh client.
    pub fn review_snapshot(&self, id: ReviewId) -> Result<ReviewSnapshot, CoreError> {
        let rec = self.review(id)?;
        Ok(ReviewSnapshot {
            review: rec.review,
            resolved: rec.resolved,
            threads: self.store.threads(id)?,
            comments: self.store.comments(id)?,
            viewed: self.store.viewed(id)?,
            seq: self.store.last_seq()?.unwrap_or(moor_protocol::Seq::new(0)),
        })
    }
}

/// git's well-known empty tree.
pub const EMPTY_TREE: moor_protocol::TreeOid = moor_protocol::TreeOid::from_bytes([
    0x4b, 0x82, 0x5d, 0xc6, 0x42, 0xcb, 0x6e, 0xb9, 0xa0, 0x60, 0xe5, 0x4b, 0xf8, 0xd6, 0x92, 0x88,
    0xfb, 0xee, 0x49, 0x04,
]);

fn first_line(bytes: &[u8]) -> Option<&str> {
    let end = bytes
        .iter()
        .position(|b| *b == b'\n')
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end]).ok()
}

impl Core {
    // ---- working tree (used by the file watcher) --------------------------

    /// Snapshot the working tree of `repo_id` as a real tree object.
    pub fn working_tree(&self, repo_id: RepoId) -> Result<ResolvedRef, CoreError> {
        Ok(self.repo(repo_id)?.working_tree()?)
    }

    /// Entries that differ between two trees of `repo_id`.
    pub fn tree_delta(
        &self,
        repo_id: RepoId,
        from: moor_protocol::TreeOid,
        to: moor_protocol::TreeOid,
    ) -> Result<TreeDelta, CoreError> {
        Ok(self.repo(repo_id)?.tree_delta(repo_id, from, to)?)
    }

    /// Live reviews with a working-tree target on `repo_id`, across all
    /// workspaces.
    pub fn working_tree_reviews(&self, repo_id: RepoId) -> Result<Vec<ReviewId>, CoreError> {
        let mut out = Vec::new();
        for ws in self.store.workspaces()? {
            for rec in self.store.reviews(ws.id)? {
                let live = matches!(rec.lifecycle, ReviewLifecycle::Live);
                let uses = rec.review.targets.iter().any(|t| {
                    t.repo_id == repo_id
                        && (t.base == RefSpec::WorkingTree || t.head == RefSpec::WorkingTree)
                });
                if live && uses {
                    out.push(rec.review.id);
                }
            }
        }
        Ok(out)
    }
}
