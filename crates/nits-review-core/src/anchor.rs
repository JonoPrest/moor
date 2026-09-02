//! Re-anchoring: follow a comment's anchor from one blob to another.
//! Pure functions of blob content so the daemon can run them off-actor.
//! See `docs/ARCHITECTURE.md` §4.5.
//!
//! Rules:
//! 1. same blob → unchanged;
//! 2. path gone (deleted, or renamed away with no detected rename) →
//!    `Outdated`, keeping the last good anchor;
//! 3. blob changed → diff old→new, map the line range through the equal
//!    regions; if every anchored line maps and the context hash over the
//!    mapped range (±[`CONTEXT_LINES`]) still matches → `Live` at the new
//!    blob; otherwise `Outdated`.
//!
//! `File` anchors follow the path (including detected renames) and only go
//! `Outdated` when the file disappears. An `Outdated` comment is re-tried
//! from its last good anchor on every resolution, so it comes back to `Live`
//! when the content does.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use nits_protocol::{
    Anchor, BlobOid, CommentState, ContextHash, LineNo, LineRange, RepoPath, Side,
};

use crate::render::lines_of;

/// Lines above and below the anchored range that the context hash covers.
pub const CONTEXT_LINES: u32 = 3;

/// Hash of the anchored lines plus [`CONTEXT_LINES`] on each side.
#[must_use]
pub fn context_hash(lines: &[String], range: LineRange) -> ContextHash {
    let start = range.start().index().saturating_sub(CONTEXT_LINES) as usize;
    let end = ((range.end().index() + CONTEXT_LINES) as usize + 1).min(lines.len());
    let mut h = DefaultHasher::new();
    for l in &lines[start.min(end)..end] {
        l.hash(&mut h);
        0xffu8.hash(&mut h);
    }
    ContextHash::new(h.finish())
}

/// Where a path ended up on the new side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathFate {
    /// Present (possibly renamed) with this blob.
    Present {
        path: RepoPath,
        blob: BlobOid,
    },
    Gone,
}

/// Outcome of re-anchoring one comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reanchor {
    Unchanged,
    Moved { anchor: Anchor, state: CommentState },
}

/// The anchor to attempt from: a live comment's anchor, or an outdated
/// comment's last good one.
#[must_use]
pub fn effective_anchor<'a>(anchor: &'a Anchor, state: &'a CommentState) -> &'a Anchor {
    match state {
        CommentState::Outdated { last_good_anchor } => last_good_anchor,
        CommentState::Live | CommentState::Deleted => anchor,
    }
}

/// Re-anchor. `fate` is where the anchored path is on the new side;
/// `old_blob`/`new_blob` provide content for line mapping.
pub fn reanchor(
    anchor: &Anchor,
    state: &CommentState,
    fate: &PathFate,
    old_blob: impl FnOnce(BlobOid) -> Option<Vec<u8>>,
    new_blob: impl FnOnce(BlobOid) -> Option<Vec<u8>>,
) -> Reanchor {
    if matches!(state, CommentState::Deleted) {
        return Reanchor::Unchanged;
    }
    let base = effective_anchor(anchor, state);
    let outdated = || Reanchor::Moved {
        anchor: anchor.clone(),
        state: CommentState::Outdated {
            last_good_anchor: base.clone(),
        },
    };
    match base {
        Anchor::Review => Reanchor::Unchanged,
        Anchor::File { repo_id, .. } => match fate {
            PathFate::Gone => match state {
                CommentState::Outdated { .. } => Reanchor::Unchanged,
                CommentState::Live | CommentState::Deleted => outdated(),
            },
            PathFate::Present { path, blob } => {
                let moved = Anchor::File {
                    repo_id: *repo_id,
                    path: path.clone(),
                    blob_oid: *blob,
                };
                if &moved == anchor && matches!(state, CommentState::Live) {
                    Reanchor::Unchanged
                } else {
                    Reanchor::Moved {
                        anchor: moved,
                        state: CommentState::Live,
                    }
                }
            }
        },
        Anchor::Lines {
            repo_id,
            side,
            blob_oid,
            lines,
            context_hash: hash,
            ..
        } => match fate {
            PathFate::Gone => match state {
                CommentState::Outdated { .. } => Reanchor::Unchanged,
                CommentState::Live | CommentState::Deleted => outdated(),
            },
            PathFate::Present { path, blob } => {
                if blob == blob_oid {
                    return if matches!(state, CommentState::Live) && path == anchor_path(anchor) {
                        Reanchor::Unchanged
                    } else {
                        Reanchor::Moved {
                            anchor: Anchor::Lines {
                                repo_id: *repo_id,
                                path: path.clone(),
                                side: *side,
                                blob_oid: *blob,
                                lines: *lines,
                                context_hash: *hash,
                            },
                            state: CommentState::Live,
                        }
                    };
                }
                let (Some(old), Some(new)) = (old_blob(*blob_oid), new_blob(*blob)) else {
                    return outdated();
                };
                let old_lines = lines_of(&old);
                let new_lines = lines_of(&new);
                let Some(mapped) = map_range(&old_lines, &new_lines, *lines) else {
                    return outdated();
                };
                if context_hash(&new_lines, mapped) != *hash {
                    return outdated();
                }
                Reanchor::Moved {
                    anchor: Anchor::Lines {
                        repo_id: *repo_id,
                        path: path.clone(),
                        side: *side,
                        blob_oid: *blob,
                        lines: mapped,
                        context_hash: *hash,
                    },
                    state: CommentState::Live,
                }
            }
        },
    }
}

fn anchor_path(a: &Anchor) -> &RepoPath {
    match a {
        Anchor::File { path, .. } | Anchor::Lines { path, .. } => path,
        Anchor::Review => unreachable_path(),
    }
}

fn unreachable_path() -> &'static RepoPath {
    // `Review` anchors never reach path comparisons; a static sentinel keeps
    // the function total without a panic.
    static SENTINEL: std::sync::OnceLock<RepoPath> = std::sync::OnceLock::new();
    SENTINEL.get_or_init(|| {
        RepoPath::new("<review>").unwrap_or_else(|_| RepoPath::new("review").expect("valid"))
    })
}

/// Map a 1-based line range through the diff of `old` → `new`. Succeeds
/// only if every line in the range lies in an unchanged region; the result
/// is the corresponding range in `new`.
#[must_use]
pub fn map_range(old: &[String], new: &[String], range: LineRange) -> Option<LineRange> {
    let before: Vec<&str> = old.iter().map(String::as_str).collect();
    let after: Vec<&str> = new.iter().map(String::as_str).collect();
    let input = imara_diff::InternedInput::new(Slice(&before), Slice(&after));
    let diff = imara_diff::Diff::compute(imara_diff::Algorithm::Histogram, &input);
    let start = range.start().index();
    let end = range.end().index();
    if end as usize >= old.len() {
        return None;
    }
    // Walk hunks; accumulate the offset for equal regions.
    let mut offset: i64 = 0;
    for h in diff.hunks() {
        if h.before.start > end {
            break;
        }
        // Range intersects a changed region → cannot map.
        if h.before.start <= end && h.before.end > start && !h.before.is_empty() {
            return None;
        }
        // Pure insertion exactly at `start`.. still fine; insertion inside
        // the range (start < pos <= end) splits it → cannot map.
        if h.before.is_empty() && h.before.start > start && h.before.start <= end {
            return None;
        }
        if h.before.end <= start || (h.before.is_empty() && h.before.start <= start) {
            offset +=
                i64::from(h.after.end - h.after.start) - i64::from(h.before.end - h.before.start);
        }
    }
    let ns = i64::from(start) + offset;
    let ne = i64::from(end) + offset;
    let ns = u32::try_from(ns).ok()?;
    let ne = u32::try_from(ne).ok()?;
    if ne as usize >= new.len() {
        return None;
    }
    LineRange::new(LineNo::from_index(ns), LineNo::from_index(ne)).ok()
}

struct Slice<'a>(&'a [&'a str]);

impl<'a> imara_diff::TokenSource for Slice<'a> {
    type Token = &'a str;
    type Tokenizer = std::iter::Copied<std::slice::Iter<'a, &'a str>>;
    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter().copied()
    }
    fn estimate_tokens(&self) -> u32 {
        u32::try_from(self.0.len()).unwrap_or(u32::MAX)
    }
}

/// Which side's tree a `Lines` anchor lives in.
#[must_use]
pub fn side_of(anchor: &Anchor) -> Option<Side> {
    match anchor {
        Anchor::Lines { side, .. } => Some(*side),
        // File anchors follow the head side.
        Anchor::File { .. } => Some(Side::Head),
        Anchor::Review => None,
    }
}
