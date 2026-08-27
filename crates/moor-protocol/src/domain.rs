//! Domain model: workspaces, repos, reviews, comments, anchors, authors.
//!
//! See `docs/ARCHITECTURE.md` §4.4.

use serde::{Deserialize, Serialize};
use strum::{EnumDiscriminants, EnumIter};

use crate::ids::{
    BlobOid, CommentId, CommitOid, RepoId, ReviewId, ThreadId, Timestamp, TreeOid, WorkspaceId,
};
use crate::invariants::{LineRange, NonEmpty, RepoPath};

/// A named group of repositories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub repos: Vec<Repo>,
}

/// A git repository attached to a workspace.
///
/// `path` is the absolute path on the daemon's machine. It is a `String`
/// rather than `PathBuf` because clients (possibly on another machine, or in a
/// browser) only display it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Repo {
    pub id: RepoId,
    pub path: String,
    pub display_name: String,
}

/// What the user asked to review — unresolved. See [`ResolvedRef`] for the
/// resolved form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(RefSpecKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum RefSpec {
    Branch {
        name: String,
    },
    Commit {
        oid: CommitOid,
    },
    Tag {
        name: String,
    },
    WorkingTree,
    /// The upstream of the current branch (`@{upstream}`).
    Upstream,
    /// `HEAD`.
    Head,
}

/// A resolved ref: the tree it points at, plus what kind of thing it was
/// resolved from. Every resolved ref has a tree, so that lives outside the
/// enum; only the parts that differ are variants.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResolvedRef {
    /// Real tree OID for commits; a synthetic id over hashed working files
    /// for the working tree.
    pub tree: TreeOid,
    pub source: ResolvedSource,
}

/// What a [`ResolvedRef`] was resolved from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(ResolvedSourceKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ResolvedSource {
    Commit {
        oid: CommitOid,
    },
    /// The working tree at the moment of resolution; `dirty` lists paths
    /// that differ from `HEAD`.
    WorkingTree {
        dirty: Vec<RepoPath>,
    },
}

/// One repo's base/head pair within a review, as requested.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReviewTarget {
    pub repo_id: RepoId,
    pub base: RefSpec,
    pub head: RefSpec,
}

/// One repo's base/head pair within a review, resolved to content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResolvedTarget {
    pub repo_id: RepoId,
    pub base: ResolvedRef,
    pub head: ResolvedRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ReviewStatus {
    Open,
    Archived,
}

/// A review: a set of targets across a workspace's repos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub id: ReviewId,
    pub workspace_id: WorkspaceId,
    pub title: String,
    pub targets: NonEmpty<ReviewTarget>,
    pub created: Timestamp,
    pub status: ReviewStatus,
}

/// A git signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Sig {
    pub name: String,
    pub email: String,
    pub time: Timestamp,
    /// Timezone offset from UTC in minutes, as recorded in the commit.
    pub offset_minutes: i32,
}

/// A commit, as shown in the commit stepper panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CommitInfo {
    pub oid: CommitOid,
    pub parents: Vec<CommitOid>,
    pub tree: TreeOid,
    pub author: Sig,
    pub committer: Sig,
    /// First paragraph of the message.
    pub subject: String,
    /// Everything after the subject, trimmed. Empty if none.
    pub body: String,
}

/// A human, as recorded on events and marks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Human {
    pub name: String,
    /// Hostname of the machine the action was taken on.
    pub machine: String,
}

/// How an agent reached the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum AgentVia {
    Mcp,
    Cli,
}

/// Who did something. Provenance is structured, never a free-text tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(AuthorKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Author {
    Human {
        name: String,
        machine: String,
    },
    Agent {
        name: String,
        model: String,
        session_id: String,
        invoked_by: Option<Human>,
        via: AgentVia,
    },
    /// The daemon itself, e.g. re-resolving targets after a file change.
    Daemon {
        machine: String,
    },
}

impl Author {
    #[must_use]
    pub fn human(h: Human) -> Self {
        Author::Human {
            name: h.name,
            machine: h.machine,
        }
    }

    #[must_use]
    pub fn as_human(&self) -> Option<Human> {
        match self {
            Author::Human { name, machine } => Some(Human {
                name: name.clone(),
                machine: machine.clone(),
            }),
            Author::Agent { .. } | Author::Daemon { .. } => None,
        }
    }
}

/// Which side of a diff a line anchor refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Side {
    Base,
    Head,
}

/// Hash of the ±3 lines surrounding an anchored range, used to detect that a
/// mapped anchor still points at the same content. Serialised as 16 hex chars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "String", into = "String")]
pub struct ContextHash(u64);

impl ContextHash {
    #[must_use]
    pub const fn new(h: u64) -> Self {
        Self(h)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<String> for ContextHash {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.len() != 16 {
            return Err(format!("context hash must be 16 hex chars, got {s:?}"));
        }
        u64::from_str_radix(&s, 16)
            .map(Self)
            .map_err(|e| format!("invalid context hash {s:?}: {e}"))
    }
}

impl From<ContextHash> for String {
    fn from(h: ContextHash) -> Self {
        format!("{:016x}", h.0)
    }
}

/// Where a comment is moored. Anchors reference blobs, never diffs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(AnchorKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Anchor {
    /// Review-level; no file.
    Review,
    /// A whole file at a specific blob. Need not be in the diff.
    File {
        repo_id: RepoId,
        path: RepoPath,
        blob_oid: BlobOid,
    },
    /// A line range within a specific blob.
    Lines {
        repo_id: RepoId,
        path: RepoPath,
        side: Side,
        blob_oid: BlobOid,
        lines: LineRange,
        context_hash: ContextHash,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(CommentKindKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum CommentKind {
    Note,
    /// A unified diff against the anchored blob that a client can apply.
    Suggestion {
        patch: String,
    },
    /// Asks an agent (or human) to act.
    Request,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(CommentStateKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum CommentState {
    Live,
    /// The content moved or changed such that the anchor could not be mapped.
    /// The comment is still shown (collapsed) at its last good anchor.
    Outdated {
        last_good_anchor: Anchor,
    },
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Comment {
    pub id: CommentId,
    pub review_id: ReviewId,
    pub thread_id: ThreadId,
    pub author: Author,
    pub kind: CommentKind,
    pub anchor: Anchor,
    pub body: String,
    pub created: Timestamp,
    pub edited: Option<Timestamp>,
    pub state: CommentState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(ThreadResolutionKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ThreadResolution {
    Open,
    Resolved { by: Author, at: Timestamp },
}

/// A comment thread. Its id equals the root comment's id; replies share the
/// root's anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Thread {
    pub id: ThreadId,
    pub review_id: ReviewId,
    pub root: CommentId,
    pub replies: Vec<CommentId>,
    pub resolution: ThreadResolution,
}

/// A human marked a file as viewed at a specific head blob. Agents cannot set
/// these; the type says so by carrying a [`Human`], not an [`Author`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ViewedMark {
    pub review_id: ReviewId,
    pub repo_id: RepoId,
    pub path: RepoPath,
    pub viewer: Human,
    /// Head blob at the time of marking; `None` for a file deleted in head.
    pub blob_oid: Option<BlobOid>,
}

/// Options that change the render model; part of every render cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RenderOpts {
    pub ignore_whitespace: bool,
    pub context_lines: u32,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            ignore_whitespace: false,
            context_lines: 3,
        }
    }
}

/// How a file differs between base and head. Carries exactly the blobs that
/// exist, so there is no `Option<old> + Option<new>` pair to keep consistent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
// The discriminant is a view type too (explorer "change" badge), hence serde
// on it; unit-only, so it is a bare string on the wire.
#[strum_discriminants(name(ChangeKindKind), derive(EnumIter, Hash, Serialize, Deserialize))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ChangeKind {
    Added {
        new: BlobOid,
    },
    Deleted {
        old: BlobOid,
    },
    Modified {
        old: BlobOid,
        new: BlobOid,
    },
    Renamed {
        from: RepoPath,
        old: BlobOid,
        new: BlobOid,
    },
}

impl ChangeKind {
    #[must_use]
    pub fn old_blob(&self) -> Option<BlobOid> {
        match self {
            ChangeKind::Added { .. } => None,
            ChangeKind::Deleted { old }
            | ChangeKind::Modified { old, .. }
            | ChangeKind::Renamed { old, .. } => Some(*old),
        }
    }

    #[must_use]
    pub fn new_blob(&self) -> Option<BlobOid> {
        match self {
            ChangeKind::Deleted { .. } => None,
            ChangeKind::Added { new }
            | ChangeKind::Modified { new, .. }
            | ChangeKind::Renamed { new, .. } => Some(*new),
        }
    }
}

/// A changed file in one repo. `path` is the head-side path (for renames,
/// the destination).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FileChange {
    pub repo_id: RepoId,
    pub path: RepoPath,
    pub kind: ChangeKind,
}

/// What a tree entry points at.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum_discriminants(name(TreeEntryKindKind), derive(EnumIter, Hash))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TreeEntryKind {
    File {
        oid: BlobOid,
        size: u64,
        executable: bool,
    },
    Dir {
        oid: TreeOid,
    },
    Symlink {
        oid: BlobOid,
    },
    Submodule {
        commit: CommitOid,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TreeEntry {
    pub path: RepoPath,
    pub kind: TreeEntryKind,
}

/// Full recursive listing of a tree; flat, sorted by path, one pass to nest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TreeSnapshot {
    pub repo_id: RepoId,
    pub root_oid: TreeOid,
    pub entries: Vec<TreeEntry>,
}

/// Difference between two tree snapshots (used for working-tree refs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TreeDelta {
    pub repo_id: RepoId,
    pub from_root: TreeOid,
    pub to_root: TreeOid,
    pub added: Vec<TreeEntry>,
    pub removed: Vec<RepoPath>,
    pub changed: Vec<TreeEntry>,
}
