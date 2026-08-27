//! Diff, thread, conversation and commit-stepper views (plan 3.5). All
//! pure functions of the open review, the cache and the viewport; derived
//! after every input like the explorer.
//!
//! Comment placement (§6.5): a `Lines` anchor lands on the last row whose
//! cell on the anchored side is inside the range *and* whose blob matches
//! the render target; a `File` anchor lands on row 0; `Review` anchors go
//! to the conversation panel. Outdated comments are placed by their last
//! good anchor and flagged; deleted ones are not placed at all.

use moor_protocol::{
    Anchor, Author, BlobOid, ChunkIndex, Comment, CommentId, CommentKind, CommentState, CommitInfo,
    CommitOid, FileRenderHeader, RenderChunk, RenderContent, RenderTarget, RepoId, ReviewSnapshot,
    Row, Side, Thread, ThreadId, ThreadResolution, Timestamp,
};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;

use crate::cache::{CacheKey, CacheValue, ContentCache, RenderKey};
use crate::content::FileRef;
use crate::explorer::ViewedState;

/// Where a thread is anchored, for the thread list and the diff overlay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(ThreadPlaceKind), derive(Hash, strum::EnumIter))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ThreadPlace {
    Review,
    File {
        file: FileRef,
    },
    Lines {
        file: FileRef,
        side: Side,
        /// 1-based first and last line.
        start: u32,
        end: u32,
    },
}

/// One thread as the thread list shows it.
// Four independent flags is the domain; a bit set would hide the names.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadView {
    pub id: ThreadId,
    pub root: CommentId,
    pub author: Author,
    pub created: Timestamp,
    /// First line of the root comment.
    pub summary: String,
    pub replies: u32,
    pub resolved: bool,
    pub place: ThreadPlace,
    /// The root could not be re-anchored after the head moved.
    pub outdated: bool,
    /// Some comment in the thread is still awaiting the daemon.
    pub pending: bool,
    /// The root is a `CommentKind::Suggestion` (a patch that can be applied).
    pub suggestion: bool,
}

/// A row of the open file with the threads placed on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffRow {
    /// Index into the file's rows (across chunks).
    pub index: u32,
    pub row: Row,
    pub threads: Vec<ThreadId>,
}

/// The open file, over the viewport window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffView {
    pub file: FileRef,
    pub lang: Option<String>,
    pub content: RenderContent,
    /// Whether the current viewer marked this file viewed at its head blob;
    /// hosts collapse `Viewed` files.
    pub viewed: ViewedState,
    pub first_row: u32,
    pub last_row: u32,
    /// Rows of the window that are cached, in order; gaps are chunks still
    /// on their way (`missing`).
    pub rows: Vec<DiffRow>,
    pub missing: Vec<ChunkIndex>,
    /// Threads anchored to the whole file (shown above the rows).
    pub file_threads: Vec<ThreadId>,
}

/// One commit of the stepper, with what the commit panel shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepperCommit {
    pub oid: CommitOid,
    pub parents: Vec<CommitOid>,
    pub subject: String,
    /// Everything after the subject; empty if none.
    pub body: String,
    pub author: String,
    pub time: Timestamp,
    pub committer: String,
    pub committer_time: Timestamp,
}

/// Commits of one repo of the review, oldest first, with a cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitStepper {
    pub repo_id: RepoId,
    pub commits: Vec<StepperCommit>,
    /// Index into `commits`; `None` shows the whole range.
    pub selected: Option<usize>,
}

impl CommitStepper {
    pub(crate) fn from_commits(repo_id: RepoId, commits: &[CommitInfo]) -> Self {
        Self {
            repo_id,
            commits: commits
                .iter()
                .map(|c| StepperCommit {
                    oid: c.oid,
                    parents: c.parents.clone(),
                    subject: c.subject.clone(),
                    body: c.body.clone(),
                    author: c.author.name.clone(),
                    time: c.author.time,
                    committer: c.committer.name.clone(),
                    committer_time: c.committer.time,
                })
                .collect(),
            selected: None,
        }
    }
}

/// The comment list in thread order: threads by creation of their root,
/// each with root then replies. Deleted comments keep their slot.
#[must_use]
pub fn threads(snapshot: &ReviewSnapshot, pending: &PendingIds) -> Vec<ThreadView> {
    let mut out: Vec<ThreadView> = snapshot
        .threads
        .iter()
        .filter_map(|t| thread_view(snapshot, t, pending))
        .collect();
    out.sort_by_key(|t| t.created);
    out
}

/// What the pending mutations touch, for the `pending` marks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PendingIds {
    pub comments: Vec<CommentId>,
    pub threads: Vec<ThreadId>,
}

fn thread_view(snapshot: &ReviewSnapshot, t: &Thread, pending: &PendingIds) -> Option<ThreadView> {
    let root = snapshot.comments.iter().find(|c| c.id == t.root)?;
    let (anchor, outdated) = placed_anchor(root)?;
    let in_thread = |id: &CommentId| *id == t.root || t.replies.contains(id);
    Some(ThreadView {
        id: t.id,
        root: root.id,
        author: root.author.clone(),
        created: root.created,
        summary: root.body.lines().next().unwrap_or_default().to_owned(),
        replies: u32::try_from(t.replies.len()).unwrap_or(u32::MAX),
        resolved: matches!(t.resolution, ThreadResolution::Resolved { .. }),
        place: place_of(anchor),
        outdated,
        pending: pending.comments.iter().any(in_thread) || pending.threads.contains(&t.id),
        suggestion: matches!(root.kind, CommentKind::Suggestion { .. }),
    })
}

/// Threads anchored at review level, oldest first.
#[must_use]
pub fn conversation(threads: &[ThreadView]) -> Vec<ThreadView> {
    threads
        .iter()
        .filter(|t| t.place == ThreadPlace::Review)
        .cloned()
        .collect()
}

/// The anchor a comment is shown at, and whether it is the stale one.
fn placed_anchor(c: &Comment) -> Option<(&Anchor, bool)> {
    match &c.state {
        CommentState::Live => Some((&c.anchor, false)),
        CommentState::Outdated { last_good_anchor } => Some((last_good_anchor, true)),
        CommentState::Deleted => None,
    }
}

fn place_of(anchor: &Anchor) -> ThreadPlace {
    match anchor {
        Anchor::Review => ThreadPlace::Review,
        Anchor::File { repo_id, path, .. } => ThreadPlace::File {
            file: FileRef {
                repo_id: *repo_id,
                path: path.clone(),
            },
        },
        Anchor::Lines {
            repo_id,
            path,
            side,
            lines,
            ..
        } => ThreadPlace::Lines {
            file: FileRef {
                repo_id: *repo_id,
                path: path.clone(),
            },
            side: *side,
            start: lines.start().get(),
            end: lines.end().get(),
        },
    }
}

/// Blob shown on `side` of a render target.
fn blob_on(target: &RenderTarget, side: Side) -> Option<BlobOid> {
    match target {
        RenderTarget::Diff { change } => match side {
            Side::Base => change.old_blob(),
            Side::Head => change.new_blob(),
        },
        RenderTarget::Blob { oid } => Some(*oid),
    }
}

/// Line number of `row` on `side`, if it has one.
fn line_on(row: &Row, side: Side) -> Option<u32> {
    match (row, side) {
        (
            Row::Context { left, .. } | Row::Modified { left, .. } | Row::Removed { left },
            Side::Base,
        ) => Some(left.line_no.get()),
        (
            Row::Context { right, .. } | Row::Modified { right, .. } | Row::Added { right },
            Side::Head,
        ) => Some(right.line_no.get()),
        (Row::Removed { .. }, Side::Head)
        | (Row::Added { .. }, Side::Base)
        | (
            Row::HunkHeader { .. } | Row::Expander { .. } | Row::WhitespaceOnly,
            Side::Base | Side::Head,
        ) => None,
    }
}

/// A thread anchored somewhere in `file`, resolved to the render target.
struct Placement {
    thread: ThreadId,
    kind: PlacementKind,
}

enum PlacementKind {
    File,
    Lines { side: Side, end: u32 },
}

fn placements(snapshot: &ReviewSnapshot, render: &RenderKey) -> Vec<Placement> {
    let mut out = Vec::new();
    for t in &snapshot.threads {
        let Some(root) = snapshot.comments.iter().find(|c| c.id == t.root) else {
            continue;
        };
        let Some((anchor, _)) = placed_anchor(root) else {
            continue;
        };
        let kind = match anchor {
            Anchor::Review => continue,
            Anchor::File {
                repo_id,
                path,
                blob_oid,
            } => {
                if *repo_id != render.repo_id || *path != render.path {
                    continue;
                }
                // A file anchor is shown if its blob is on either side.
                if blob_on(&render.target, Side::Head) != Some(*blob_oid)
                    && blob_on(&render.target, Side::Base) != Some(*blob_oid)
                {
                    continue;
                }
                PlacementKind::File
            }
            Anchor::Lines {
                repo_id,
                path,
                side,
                blob_oid,
                lines,
                ..
            } => {
                if *repo_id != render.repo_id
                    || *path != render.path
                    || blob_on(&render.target, *side) != Some(*blob_oid)
                {
                    continue;
                }
                PlacementKind::Lines {
                    side: *side,
                    end: lines.end().get(),
                }
            }
        };
        out.push(Placement { thread: t.id, kind });
    }
    out
}

/// Every cached row of `render` with its threads, for navigation that must
/// see past the viewport (next hunk, next comment). Chunks not cached are
/// simply absent.
pub(crate) fn all_rows(
    cache: &ContentCache,
    snapshot: &ReviewSnapshot,
    render: &RenderKey,
) -> Vec<DiffRow> {
    let Some(CacheValue::Header { header }) = cache.peek(&CacheKey::Header {
        render: render.clone(),
    }) else {
        return Vec::new();
    };
    let RenderContent::Text {
        chunk_rows,
        chunk_count,
        ..
    } = header.content
    else {
        return Vec::new();
    };
    let placements = placements(snapshot, render);
    let mut rows = Vec::new();
    for ci in 0..chunk_count {
        if let Some(CacheValue::Chunk { chunk }) = cache.peek(&CacheKey::Chunk {
            render: render.clone(),
            index: ChunkIndex::new(ci),
        }) {
            place_rows(chunk, ci * chunk_rows, 0, u32::MAX, &placements, &mut rows);
        }
    }
    rows
}

/// Build the diff view for `render` over rows `first_row..=last_row`.
pub(crate) fn diff_view(
    cache: &ContentCache,
    snapshot: &ReviewSnapshot,
    viewer: &Author,
    render: &RenderKey,
    first_row: u32,
    last_row: u32,
) -> Option<DiffView> {
    let CacheValue::Header { header } = cache.peek(&CacheKey::Header {
        render: render.clone(),
    })?
    else {
        return None;
    };
    let file = FileRef {
        repo_id: render.repo_id,
        path: render.path.clone(),
    };
    let viewed_state = crate::explorer::viewed_state(
        snapshot,
        viewer,
        render.repo_id,
        &render.path,
        blob_on(&render.target, Side::Head),
    );
    let placements = placements(snapshot, render);
    let file_threads: Vec<ThreadId> = placements
        .iter()
        .filter(|p| matches!(p.kind, PlacementKind::File))
        .map(|p| p.thread)
        .collect();
    let RenderContent::Text {
        chunk_rows,
        chunk_count,
        total_rows,
        ..
    } = header.content
    else {
        return Some(DiffView {
            file,
            lang: header.lang.clone(),
            content: header.content.clone(),
            viewed: viewed_state,
            first_row,
            last_row,
            rows: Vec::new(),
            missing: Vec::new(),
            file_threads,
        });
    };
    if chunk_rows == 0 || chunk_count == 0 || total_rows == 0 {
        return Some(empty(
            header,
            file,
            viewed_state,
            first_row,
            last_row,
            file_threads,
        ));
    }
    let last_row = last_row.min(total_rows - 1);
    let first_row = first_row.min(last_row);
    let first_chunk = first_row / chunk_rows;
    let last_chunk = (last_row / chunk_rows).min(chunk_count - 1);
    let mut rows = Vec::new();
    let mut missing = Vec::new();
    for ci in first_chunk..=last_chunk {
        let index = ChunkIndex::new(ci);
        match cache.peek(&CacheKey::Chunk {
            render: render.clone(),
            index,
        }) {
            Some(CacheValue::Chunk { chunk }) => {
                place_rows(
                    chunk,
                    ci * chunk_rows,
                    first_row,
                    last_row,
                    &placements,
                    &mut rows,
                );
            }
            Some(CacheValue::Tree { .. } | CacheValue::Header { .. }) | None => missing.push(index),
        }
    }
    Some(DiffView {
        file,
        lang: header.lang.clone(),
        content: header.content.clone(),
        viewed: viewed_state,
        first_row,
        last_row,
        rows,
        missing,
        file_threads,
    })
}

fn empty(
    header: &FileRenderHeader,
    file: FileRef,
    viewed: ViewedState,
    first_row: u32,
    last_row: u32,
    file_threads: Vec<ThreadId>,
) -> DiffView {
    DiffView {
        file,
        lang: header.lang.clone(),
        content: header.content.clone(),
        viewed,
        first_row,
        last_row,
        rows: Vec::new(),
        missing: Vec::new(),
        file_threads,
    }
}

/// Append the rows of `chunk` inside the window, each with the threads
/// whose range ends on it.
fn place_rows(
    chunk: &RenderChunk,
    base_index: u32,
    first_row: u32,
    last_row: u32,
    placements: &[Placement],
    out: &mut Vec<DiffRow>,
) {
    for (i, row) in chunk.rows.iter().enumerate() {
        let index = base_index + u32::try_from(i).unwrap_or(u32::MAX);
        if index < first_row || index > last_row {
            continue;
        }
        let mut threads = Vec::new();
        for p in placements {
            let PlacementKind::Lines { side, end } = p.kind else {
                continue;
            };
            // Anchor on the last line of the range; a row whose cell is
            // that line carries the thread.
            if line_on(row, side) == Some(end) {
                threads.push(p.thread);
            }
        }
        out.push(DiffRow {
            index,
            row: row.clone(),
            threads,
        });
    }
}
