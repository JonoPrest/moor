//! Git engine: resolve refs, read objects, list changes, snapshot trees and
//! the working tree.
//!
//! `gix` reads objects and commits; the `git` binary is used where gix is
//! still weaker (rename detection, working-tree snapshotting, `@{upstream}`
//! resolution). See `docs/ARCHITECTURE.md` §4.3.
//!
//! The working-tree snapshot is a *real* tree object: files are added to a
//! temporary index and written with `git write-tree`, so dirty blobs exist in
//! the object database and every later read goes through OIDs like any other
//! ref. Unchanged files keep their index OID.

use std::path::{Path, PathBuf};
use std::process::Command;

use gix::bstr::ByteSlice;
use gix::objs::tree::EntryKind as K;
use nits_protocol::{
    BlobOid, ChangeKind, CommitInfo, CommitOid, Oid, RefSpec, RepoId, RepoPath, ResolvedRef,
    ResolvedSource, Sig, Timestamp, TreeDelta, TreeEntry, TreeEntryKind, TreeOid, TreeSnapshot,
};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("cannot open repository at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: Box<gix::open::Error>,
    },
    #[error("git {args:?} failed: {stderr}")]
    Command { args: Vec<String>, stderr: String },
    #[error("cannot resolve {rev}: {reason}")]
    Resolve { rev: String, reason: String },
    #[error("object {oid} not found or unreadable: {reason}")]
    Object { oid: Oid, reason: String },
    #[error("unexpected git output: {0}")]
    Parse(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// A git repository, safe to share across threads.
pub struct Repo {
    inner: gix::ThreadSafeRepository,
    workdir: PathBuf,
}

impl std::fmt::Debug for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repo")
            .field("workdir", &self.workdir)
            .finish_non_exhaustive()
    }
}

fn oid_from_gix(id: gix::ObjectId) -> Option<Oid> {
    let bytes: [u8; 20] = id.as_bytes().try_into().ok()?;
    Some(Oid::from_bytes(bytes))
}

fn gix_id(oid: Oid) -> gix::ObjectId {
    gix::ObjectId::from_bytes_or_panic(oid.as_bytes())
}

/// Git's own heuristic: a NUL in the first 8000 bytes means binary.
#[must_use]
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0)
}

impl Repo {
    pub fn open(path: &Path) -> Result<Self, GitError> {
        let repo = gix::open(path).map_err(|e| GitError::Open {
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| GitError::Open {
                path: path.to_path_buf(),
                source: Box::new(gix::open::Error::NotARepository {
                    source: gix::discover::is_git::Error::MissingHead,
                    path: path.to_path_buf(),
                }),
            })?
            .to_path_buf();
        Ok(Self {
            inner: repo.into_sync(),
            workdir,
        })
    }

    #[must_use]
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    fn local(&self) -> gix::Repository {
        self.inner.to_thread_local()
    }

    /// Run `git` in the work dir with extra env; returns raw stdout.
    fn git(&self, args: &[&str], env: &[(&str, &Path)]) -> Result<Vec<u8>, GitError> {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.workdir)
            .envs(env.iter().map(|(k, v)| (*k, *v)))
            .output()?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(GitError::Command {
                args: args.iter().map(|s| (*s).to_owned()).collect(),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            })
        }
    }

    // ---- refs -------------------------------------------------------------

    /// Resolve a [`RefSpec`] to concrete content. `WorkingTree` snapshots the
    /// working tree as a new tree object.
    pub fn resolve(&self, spec: &RefSpec) -> Result<ResolvedRef, GitError> {
        let rev = match spec {
            RefSpec::Branch { name } => format!("refs/heads/{name}"),
            RefSpec::Tag { name } => format!("refs/tags/{name}"),
            RefSpec::Commit { oid } => oid.to_string(),
            RefSpec::Head => "HEAD".to_owned(),
            RefSpec::Upstream => "@{upstream}".to_owned(),
            RefSpec::WorkingTree => return self.working_tree(),
        };
        let commit = self.rev_parse_commit(&rev)?;
        let tree = self.commit_tree(commit)?;
        Ok(ResolvedRef {
            tree,
            source: ResolvedSource::Commit { oid: commit },
        })
    }

    /// `git rev-parse` of `<rev>^{commit}`.
    pub fn rev_parse_commit(&self, rev: &str) -> Result<CommitOid, GitError> {
        let spec = format!("{rev}^{{commit}}");
        let out = self
            .git(
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &spec,
                ],
                &[],
            )
            .map_err(|e| GitError::Resolve {
                rev: rev.to_owned(),
                reason: match e {
                    GitError::Command { stderr, .. } if stderr.is_empty() => {
                        "no such revision".into()
                    }
                    other @ (GitError::Open { .. }
                    | GitError::Command { .. }
                    | GitError::Resolve { .. }
                    | GitError::Object { .. }
                    | GitError::Parse(_)
                    | GitError::Io(_)) => other.to_string(),
                },
            })?;
        let text = String::from_utf8_lossy(&out);
        text.trim()
            .parse::<Oid>()
            .map(CommitOid::new)
            .map_err(|e| GitError::Parse(format!("rev-parse output {text:?}: {e}")))
    }

    fn commit_tree(&self, commit: CommitOid) -> Result<TreeOid, GitError> {
        let repo = self.local();
        let c = repo
            .find_commit(gix_id(commit.oid()))
            .map_err(|e| GitError::Object {
                oid: commit.oid(),
                reason: e.to_string(),
            })?;
        let tree = c.tree_id().map_err(|e| GitError::Object {
            oid: commit.oid(),
            reason: e.to_string(),
        })?;
        oid_from_gix(tree.detach())
            .map(TreeOid::new)
            .ok_or_else(|| GitError::Parse("non-SHA1 object id".into()))
    }

    // ---- objects ----------------------------------------------------------

    pub fn blob(&self, oid: BlobOid) -> Result<Vec<u8>, GitError> {
        let repo = self.local();
        let mut obj = repo
            .find_blob(gix_id(oid.oid()))
            .map_err(|e| GitError::Object {
                oid: oid.oid(),
                reason: e.to_string(),
            })?;
        Ok(obj.take_data())
    }

    /// Write `bytes` as a blob object and return its id.
    pub fn hash_blob(&self, bytes: &[u8]) -> Result<BlobOid, GitError> {
        let repo = self.local();
        let id = repo.write_blob(bytes).map_err(|e| GitError::Object {
            oid: Oid::zero(),
            reason: e.to_string(),
        })?;
        oid_from_gix(id.detach())
            .map(BlobOid::new)
            .ok_or_else(|| GitError::Parse("non-SHA1 object id".into()))
    }

    fn blob_size(repo: &gix::Repository, id: gix::ObjectId) -> Result<u64, GitError> {
        let header = repo.find_header(id).map_err(|e| GitError::Object {
            oid: oid_from_gix(id).unwrap_or(Oid::zero()),
            reason: e.to_string(),
        })?;
        Ok(header.size())
    }

    pub fn commit_info(&self, oid: CommitOid) -> Result<CommitInfo, GitError> {
        let repo = self.local();
        let err = |reason: String| GitError::Object {
            oid: oid.oid(),
            reason,
        };
        let c = repo
            .find_commit(gix_id(oid.oid()))
            .map_err(|e| err(e.to_string()))?;
        let tree = c.tree_id().map_err(|e| err(e.to_string()))?;
        let parents = c
            .parent_ids()
            .map(|p| oid_from_gix(p.detach()).map(CommitOid::new))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| err("non-SHA1 parent".into()))?;
        let author = sig(&c.author().map_err(|e| err(e.to_string()))?)?;
        let committer = sig(&c.committer().map_err(|e| err(e.to_string()))?)?;
        let message = c.message().map_err(|e| err(e.to_string()))?;
        let subject = message.summary().to_str_lossy().into_owned();
        let body = message
            .body()
            .map(|b| b.to_str_lossy().trim().to_owned())
            .unwrap_or_default();
        Ok(CommitInfo {
            oid,
            parents,
            tree: oid_from_gix(tree.detach())
                .map(TreeOid::new)
                .ok_or_else(|| err("non-SHA1 tree".into()))?,
            author,
            committer,
            subject,
            body,
        })
    }

    /// Commits reachable from `head` but not `base`, newest first
    /// (`git rev-list --topo-order base..head`).
    pub fn commits_between(
        &self,
        base: CommitOid,
        head: CommitOid,
    ) -> Result<Vec<CommitInfo>, GitError> {
        let range = format!("{base}..{head}");
        let out = self.git(&["rev-list", "--topo-order", &range], &[])?;
        String::from_utf8_lossy(&out)
            .lines()
            .map(|l| {
                let oid: Oid = l
                    .trim()
                    .parse()
                    .map_err(|e| GitError::Parse(format!("rev-list line {l:?}: {e}")))?;
                self.commit_info(CommitOid::new(oid))
            })
            .collect()
    }

    // ---- trees ------------------------------------------------------------

    /// Full recursive listing, sorted by path.
    pub fn tree_snapshot(&self, repo_id: RepoId, root: TreeOid) -> Result<TreeSnapshot, GitError> {
        let repo = self.local();
        let err = |reason: String| GitError::Object {
            oid: root.oid(),
            reason,
        };
        let tree = repo
            .find_tree(gix_id(root.oid()))
            .map_err(|e| err(e.to_string()))?;
        let mut entries = Vec::new();
        let recorded = tree
            .traverse()
            .breadthfirst
            .files()
            .map_err(|e| err(e.to_string()))?;
        for rec in recorded {
            let Some(path) = RepoPath::new(rec.filepath.to_str_lossy().into_owned()).ok() else {
                continue;
            };
            let Some(oid) = oid_from_gix(rec.oid) else {
                return Err(err("non-SHA1 entry".into()));
            };
            let kind = match rec.mode.kind() {
                K::Tree => TreeEntryKind::Dir {
                    oid: TreeOid::new(oid),
                },
                K::Blob | K::BlobExecutable => TreeEntryKind::File {
                    oid: BlobOid::new(oid),
                    size: Self::blob_size(&repo, rec.oid)?,
                    executable: matches!(rec.mode.kind(), K::BlobExecutable),
                },
                K::Link => TreeEntryKind::Symlink {
                    oid: BlobOid::new(oid),
                },
                K::Commit => TreeEntryKind::Submodule {
                    commit: CommitOid::new(oid),
                },
            };
            entries.push(TreeEntry { path, kind });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(TreeSnapshot {
            repo_id,
            root_oid: root,
            entries,
        })
    }

    /// Entries added/removed/changed between two trees (no rename pairing).
    pub fn tree_delta(
        &self,
        repo_id: RepoId,
        from: TreeOid,
        to: TreeOid,
    ) -> Result<TreeDelta, GitError> {
        let mut delta = TreeDelta {
            repo_id,
            from_root: from,
            to_root: to,
            added: vec![],
            removed: vec![],
            changed: vec![],
        };
        let repo = self.local();
        for raw in self.raw_diff(from, to, false)? {
            match raw.status {
                RawStatus::Added => {
                    delta
                        .added
                        .push(Self::entry_for(&repo, &raw.path, raw.new_mode, raw.new)?);
                }
                RawStatus::Deleted => delta.removed.push(raw.path),
                RawStatus::Modified | RawStatus::TypeChanged => {
                    delta
                        .changed
                        .push(Self::entry_for(&repo, &raw.path, raw.new_mode, raw.new)?);
                }
                RawStatus::Renamed { from } => {
                    delta.removed.push(from);
                    delta
                        .added
                        .push(Self::entry_for(&repo, &raw.path, raw.new_mode, raw.new)?);
                }
            }
        }
        Ok(delta)
    }

    fn entry_for(
        repo: &gix::Repository,
        path: &RepoPath,
        mode: u32,
        oid: Oid,
    ) -> Result<TreeEntry, GitError> {
        let kind = match mode & 0o170_000 {
            0o040_000 => TreeEntryKind::Dir {
                oid: TreeOid::new(oid),
            },
            0o120_000 => TreeEntryKind::Symlink {
                oid: BlobOid::new(oid),
            },
            0o160_000 => TreeEntryKind::Submodule {
                commit: CommitOid::new(oid),
            },
            _ => TreeEntryKind::File {
                oid: BlobOid::new(oid),
                size: Self::blob_size(repo, gix_id(oid))?,
                executable: mode & 0o111 != 0,
            },
        };
        Ok(TreeEntry {
            path: path.clone(),
            kind,
        })
    }

    /// Changed files between two trees with rename detection. Submodule
    /// entries are skipped (they are not blobs and cannot be rendered).
    pub fn changed_files(
        &self,
        base: TreeOid,
        head: TreeOid,
    ) -> Result<Vec<FileChangeRaw>, GitError> {
        let mut out = Vec::new();
        for raw in self.raw_diff(base, head, true)? {
            if raw.old_mode & 0o170_000 == 0o160_000 || raw.new_mode & 0o170_000 == 0o160_000 {
                continue;
            }
            let kind = match raw.status {
                RawStatus::Added => ChangeKind::Added {
                    new: BlobOid::new(raw.new),
                },
                RawStatus::Deleted => ChangeKind::Deleted {
                    old: BlobOid::new(raw.old),
                },
                RawStatus::Modified | RawStatus::TypeChanged => ChangeKind::Modified {
                    old: BlobOid::new(raw.old),
                    new: BlobOid::new(raw.new),
                },
                RawStatus::Renamed { from } => ChangeKind::Renamed {
                    from,
                    old: BlobOid::new(raw.old),
                    new: BlobOid::new(raw.new),
                },
            };
            out.push(FileChangeRaw {
                path: raw.path,
                kind,
            });
        }
        Ok(out)
    }

    fn raw_diff(
        &self,
        from: TreeOid,
        to: TreeOid,
        renames: bool,
    ) -> Result<Vec<RawChange>, GitError> {
        let (f, t) = (from.to_string(), to.to_string());
        let mut args = vec!["diff-tree", "-r", "-z", "--raw", "--no-color"];
        if renames {
            args.push("-M");
        }
        args.push(&f);
        args.push(&t);
        let out = self.git(&args, &[])?;
        parse_raw_diff(&out)
    }

    // ---- working tree -----------------------------------------------------

    /// Snapshot the working tree into a real tree object via a temporary
    /// index. Respects `.gitignore`; untracked files are included; deleted
    /// files are absent.
    pub fn working_tree(&self) -> Result<ResolvedRef, GitError> {
        let git_dir = self.local().git_dir().to_path_buf();
        let tmp = tempfile::Builder::new()
            .prefix("nits-index-")
            .tempfile_in(&git_dir)?;
        let tmp_path = tmp.path().to_path_buf();
        // Seed from the real index so stat caching makes `add -A` incremental.
        let real_index = git_dir.join("index");
        if real_index.exists() {
            std::fs::copy(&real_index, &tmp_path)?;
        }
        let env: &[(&str, &Path)] = &[("GIT_INDEX_FILE", tmp_path.as_path())];
        self.git(&["add", "-A", "--ignore-errors", "--", "."], env)?;
        let out = self.git(&["write-tree"], env)?;
        let text = String::from_utf8_lossy(&out);
        let tree: Oid = text
            .trim()
            .parse()
            .map_err(|e| GitError::Parse(format!("write-tree output {text:?}: {e}")))?;
        let tree = TreeOid::new(tree);

        let branch = self
            .git(&["symbolic-ref", "--short", "--quiet", "HEAD"], &[])
            .ok()
            .and_then(|out| {
                let name = String::from_utf8_lossy(&out).trim().to_owned();
                (!name.is_empty()).then_some(name)
            });
        let dirty = match self.rev_parse_commit("HEAD") {
            Ok(head) => {
                let head_tree = self.commit_tree(head)?;
                let mut paths: Vec<RepoPath> = Vec::new();
                for raw in self.raw_diff(head_tree, tree, false)? {
                    paths.push(raw.path);
                }
                paths.sort();
                paths
            }
            // Unborn branch: everything is new.
            Err(GitError::Resolve { .. }) => {
                let snap = self.tree_snapshot(RepoId::nil(), tree)?;
                snap.entries
                    .into_iter()
                    .filter(|e| !matches!(e.kind, TreeEntryKind::Dir { .. }))
                    .map(|e| e.path)
                    .collect()
            }
            Err(e) => return Err(e),
        };
        Ok(ResolvedRef {
            tree,
            source: ResolvedSource::WorkingTree { dirty, branch },
        })
    }
}

/// A changed file without its `repo_id` (the review layer attaches that).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChangeRaw {
    pub path: RepoPath,
    pub kind: ChangeKind,
}

fn sig(s: &gix::actor::SignatureRef<'_>) -> Result<Sig, GitError> {
    let time = s
        .time()
        .map_err(|e| GitError::Parse(format!("signature time: {e}")))?;
    Ok(Sig {
        name: s.name.to_str_lossy().into_owned(),
        email: s.email.to_str_lossy().into_owned(),
        time: Timestamp::from_millis(time.seconds * 1000),
        offset_minutes: time.offset / 60,
    })
}

#[derive(Debug)]
enum RawStatus {
    Added,
    Deleted,
    Modified,
    TypeChanged,
    Renamed { from: RepoPath },
}

#[derive(Debug)]
struct RawChange {
    old_mode: u32,
    new_mode: u32,
    old: Oid,
    new: Oid,
    status: RawStatus,
    path: RepoPath,
}

/// Parse `git diff-tree -r -z --raw` output.
fn parse_raw_diff(out: &[u8]) -> Result<Vec<RawChange>, GitError> {
    let mut fields = out.split(|b| *b == 0).filter(|f| !f.is_empty());
    let mut changes = Vec::new();
    while let Some(meta) = fields.next() {
        let meta = std::str::from_utf8(meta).map_err(|e| GitError::Parse(e.to_string()))?;
        let meta = meta
            .strip_prefix(':')
            .ok_or_else(|| GitError::Parse(format!("raw line {meta:?}")))?;
        let parts: Vec<&str> = meta.split(' ').collect();
        let [old_mode, new_mode, old, new, status] = parts[..] else {
            return Err(GitError::Parse(format!("raw line {meta:?}")));
        };
        let parse_mode =
            |m: &str| u32::from_str_radix(m, 8).map_err(|e| GitError::Parse(e.to_string()));
        let parse_oid = |o: &str| o.parse::<Oid>().map_err(|e| GitError::Parse(e.to_string()));
        let next_path = |fields: &mut dyn Iterator<Item = &[u8]>| -> Result<RepoPath, GitError> {
            let p = fields
                .next()
                .ok_or_else(|| GitError::Parse("missing path".into()))?;
            RepoPath::new(String::from_utf8_lossy(p).into_owned())
                .map_err(|e| GitError::Parse(e.to_string()))
        };
        let (status, path) = match status.as_bytes()[0] {
            b'A' => (RawStatus::Added, next_path(&mut fields)?),
            b'D' => (RawStatus::Deleted, next_path(&mut fields)?),
            b'M' => (RawStatus::Modified, next_path(&mut fields)?),
            b'T' => (RawStatus::TypeChanged, next_path(&mut fields)?),
            b'R' | b'C' => {
                let from = next_path(&mut fields)?;
                let to = next_path(&mut fields)?;
                (RawStatus::Renamed { from }, to)
            }
            other => {
                return Err(GitError::Parse(format!("unknown status {}", other as char)));
            }
        };
        changes.push(RawChange {
            old_mode: parse_mode(old_mode)?,
            new_mode: parse_mode(new_mode)?,
            old: parse_oid(old)?,
            new: parse_oid(new)?,
            status,
            path,
        });
    }
    Ok(changes)
}
