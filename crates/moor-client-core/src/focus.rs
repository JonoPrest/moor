//! Focus model and command resolution (ARCHITECTURE §6.4). Focus is core
//! state, so "what does `j` do" is answered here from the `ViewModel`, never
//! from DOM focus. [`resolve`] turns a [`Command`] into the one [`Action`]
//! it means right now, or says why it means nothing.

use moor_protocol::{Anchor, BlobOid, ContextHash, LineNo, LineRange, RenderTarget, Row, Side};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;

use crate::content::FileRef;
use crate::diff::ThreadPlace;
use crate::explorer::{TreeNode, ViewedState};
use crate::keymap::{Command, Context};
use crate::view::{Layout, ViewModel};
use crate::{Action, ClientCore};

/// Rows a file opens with, and a page for `PageDown`/`PageUp`.
pub const PAGE_ROWS: u32 = 60;

/// Where the user is. Indices are into the corresponding view lists; the
/// tree index counts visible nodes in display order (expanded dirs only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(FocusKind), derive(Hash, strum::EnumIter))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Focus {
    ReviewList { index: usize },
    Tree { index: usize },
    Diff { row: u32 },
    Thread { index: usize },
    Composer,
    CommitStepper { index: usize },
    Help,
}

impl Default for Focus {
    fn default() -> Self {
        Focus::ReviewList { index: 0 }
    }
}

impl Focus {
    #[must_use]
    pub fn context(self) -> Context {
        match self {
            Focus::ReviewList { .. } => Context::ReviewList,
            Focus::Tree { .. } => Context::Tree,
            Focus::Diff { .. } => Context::Diff,
            Focus::Thread { .. } => Context::Thread,
            Focus::Composer => Context::Composer,
            Focus::CommitStepper { .. } => Context::CommitStepper,
            Focus::Help => Context::Help,
        }
    }
}

/// Why a command means nothing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NoTarget {
    #[error("nothing is focused that {0:?} applies to")]
    Nothing(Command),
    #[error("already at the edge")]
    AtEdge,
    #[error("no file is open")]
    NoOpenFile,
    #[error("no review is open")]
    NoOpenReview,
}

/// The tree in display order: every node whose ancestors are expanded.
#[must_use]
pub fn visible_nodes(view: &ViewModel) -> Vec<&TreeNode> {
    fn walk<'a>(nodes: &'a [TreeNode], out: &mut Vec<&'a TreeNode>) {
        for n in nodes {
            out.push(n);
            if let TreeNode::Dir {
                expanded: true,
                children,
                ..
            } = n
            {
                walk(children, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&view.tree.roots, &mut out);
    out
}

/// Number of entries the focus of `context` ranges over.
fn extent(view: &ViewModel, context: Context) -> usize {
    match context {
        Context::ReviewList => view.reviews.len(),
        Context::Tree => visible_nodes(view).len(),
        Context::Diff => view.diff.as_ref().map_or(0, |d| match d.content {
            moor_protocol::RenderContent::Text { total_rows, .. } => total_rows as usize,
            moor_protocol::RenderContent::Binary => 0,
        }),
        Context::Thread => view.threads.len(),
        Context::CommitStepper => view.stepper.as_ref().map_or(0, |s| s.commits.len()),
        Context::Composer | Context::Help | Context::Global => 0,
    }
}

fn with_index(focus: Focus, index: usize) -> Focus {
    match focus {
        Focus::ReviewList { .. } => Focus::ReviewList { index },
        Focus::Tree { .. } => Focus::Tree { index },
        Focus::Thread { .. } => Focus::Thread { index },
        Focus::CommitStepper { .. } => Focus::CommitStepper { index },
        Focus::Diff { .. } => Focus::Diff {
            row: u32::try_from(index).unwrap_or(u32::MAX),
        },
        Focus::Composer | Focus::Help => focus,
    }
}

fn index_of(focus: Focus) -> Option<usize> {
    match focus {
        Focus::ReviewList { index }
        | Focus::Tree { index }
        | Focus::Thread { index }
        | Focus::CommitStepper { index } => Some(index),
        Focus::Diff { row } => Some(row as usize),
        Focus::Composer | Focus::Help => None,
    }
}

/// Clamp a focus to the lists it indexes; a focus over an empty list or a
/// closed panel falls back to the tree or the review list.
#[must_use]
pub fn clamp(view: &ViewModel, focus: Focus) -> Focus {
    let fallback = if view.review.is_some() {
        Focus::Tree { index: 0 }
    } else {
        Focus::ReviewList { index: 0 }
    };
    match focus {
        Focus::Composer => {
            if view.draft.is_some() {
                focus
            } else {
                fallback
            }
        }
        Focus::Help => focus,
        Focus::Diff { row } => match &view.diff {
            Some(_) => {
                let n = extent(view, Context::Diff);
                if n == 0 {
                    Focus::Diff { row: 0 }
                } else {
                    Focus::Diff {
                        row: row.min(u32::try_from(n - 1).unwrap_or(u32::MAX)),
                    }
                }
            }
            None => fallback,
        },
        Focus::ReviewList { .. }
        | Focus::Tree { .. }
        | Focus::Thread { .. }
        | Focus::CommitStepper { .. } => {
            let n = extent(view, focus.context());
            let Some(i) = index_of(focus) else {
                return fallback;
            };
            if n == 0 {
                match focus {
                    // An empty list is still the place to be (a review with
                    // no tree yet, no reviews listed); a closed panel is not.
                    Focus::ReviewList { .. } => with_index(focus, 0),
                    Focus::Tree { .. } if view.review.is_some() => with_index(focus, 0),
                    Focus::Tree { .. }
                    | Focus::Thread { .. }
                    | Focus::CommitStepper { .. }
                    | Focus::Diff { .. }
                    | Focus::Composer
                    | Focus::Help => fallback,
                }
            } else {
                with_index(focus, i.min(n - 1))
            }
        }
    }
}

fn file_of_node(node: &TreeNode) -> Option<FileRef> {
    match node {
        TreeNode::File { repo_id, path, .. } => Some(FileRef {
            repo_id: *repo_id,
            path: path.clone(),
        }),
        TreeNode::Dir { .. } => None,
    }
}

/// The file a command applies to: the open one from the diff, or the
/// focused tree file.
fn target_file(view: &ViewModel, focus: Focus) -> Option<FileRef> {
    match focus {
        Focus::Diff { .. } => view.diff.as_ref().map(|d| d.file.clone()),
        Focus::Tree { index } => visible_nodes(view).get(index).and_then(|n| file_of_node(n)),
        Focus::Thread { index } => view.threads.get(index).and_then(|t| match &t.place {
            ThreadPlace::File { file } | ThreadPlace::Lines { file, .. } => Some(file.clone()),
            ThreadPlace::Review => None,
        }),
        Focus::ReviewList { .. } | Focus::Composer | Focus::CommitStepper { .. } | Focus::Help => {
            None
        }
    }
}

fn open_file(file: FileRef, around_row: u32) -> Action {
    let first_row = around_row.saturating_sub(PAGE_ROWS / 2);
    Action::Viewport {
        file,
        first_row,
        last_row: first_row + PAGE_ROWS - 1,
    }
}

/// The action `command` means with the core in its current state.
// One arm per command; splitting would hide the exhaustive match.
#[allow(clippy::too_many_lines)]
pub(crate) fn resolve(core: &ClientCore, command: Command) -> Result<Action, NoTarget> {
    let view = core.view();
    let focus = view.focus;
    let nothing = || NoTarget::Nothing(command);
    let step = |delta: i64| -> Result<Action, NoTarget> {
        let n = extent(view, focus.context());
        let Some(i) = index_of(focus) else {
            return Err(nothing());
        };
        if n == 0 {
            return Err(nothing());
        }
        let max = i64::try_from(n - 1).unwrap_or(i64::MAX);
        let next = (i64::try_from(i).unwrap_or(i64::MAX) + delta).clamp(0, max);
        if next == i64::try_from(i).unwrap_or(i64::MAX) {
            return Err(NoTarget::AtEdge);
        }
        Ok(Action::SetFocus {
            focus: with_index(focus, usize::try_from(next).unwrap_or(0)),
        })
    };
    let jump = |to_end: bool| -> Result<Action, NoTarget> {
        let n = extent(view, focus.context());
        if n == 0 || index_of(focus).is_none() {
            return Err(nothing());
        }
        let target = if to_end { n - 1 } else { 0 };
        if index_of(focus) == Some(target) {
            return Err(NoTarget::AtEdge);
        }
        Ok(Action::SetFocus {
            focus: with_index(focus, target),
        })
    };
    match command {
        Command::MoveDown => step(1),
        Command::MoveUp => step(-1),
        Command::PageDown => step(i64::from(PAGE_ROWS)),
        Command::PageUp => step(-i64::from(PAGE_ROWS)),
        Command::GoTop => jump(false),
        Command::GoBottom => jump(true),
        Command::NextHunk | Command::PrevHunk | Command::NextComment | Command::PrevComment => {
            let Focus::Diff { row } = focus else {
                return Err(nothing());
            };
            let Some(diff) = &view.diff else {
                return Err(NoTarget::NoOpenFile);
            };
            let forward = matches!(command, Command::NextHunk | Command::NextComment);
            // Search every cached chunk, not just the viewport.
            let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
            let render = open
                .open_file
                .as_ref()
                .map(|f| &f.render)
                .ok_or(NoTarget::NoOpenFile)?;
            let rows = crate::diff::all_rows(core.cache(), &open.snapshot, render);
            let _ = diff;
            let wanted = |r: &crate::diff::DiffRow| match command {
                Command::NextHunk | Command::PrevHunk => matches!(r.row, Row::HunkHeader { .. }),
                Command::NextComment | Command::PrevComment => !r.threads.is_empty(),
                Command::MoveDown
                | Command::MoveUp
                | Command::PageDown
                | Command::PageUp
                | Command::GoTop
                | Command::GoBottom
                | Command::NextFile
                | Command::PrevFile
                | Command::Open
                | Command::Back
                | Command::NextPanel
                | Command::ToggleViewed
                | Command::Comment
                | Command::Reply
                | Command::Delete
                | Command::ToggleResolved
                | Command::FileSearch
                | Command::ToggleLayout
                | Command::ToggleWhitespace
                | Command::ToggleHelp
                | Command::Connect
                | Command::Disconnect
                | Command::Commits => false,
            };
            let found = if forward {
                rows.iter().find(|r| r.index > row && wanted(r))
            } else {
                rows.iter().rev().find(|r| r.index < row && wanted(r))
            };
            found
                .map(|r| Action::SetFocus {
                    focus: Focus::Diff { row: r.index },
                })
                .ok_or(NoTarget::AtEdge)
        }
        Command::NextFile | Command::PrevFile => {
            let Some(open) = &view.review else {
                return Err(NoTarget::NoOpenReview);
            };
            if open.files.is_empty() {
                return Err(nothing());
            }
            let current = open
                .open_file
                .as_ref()
                .and_then(|f| open.files.iter().position(|k| *k == f.render));
            let next = match (command, current) {
                (Command::NextFile, None) => 0,
                (Command::NextFile, Some(i)) => {
                    if i + 1 >= open.files.len() {
                        return Err(NoTarget::AtEdge);
                    }
                    i + 1
                }
                (_, None) => open.files.len() - 1,
                (_, Some(i)) => i.checked_sub(1).ok_or(NoTarget::AtEdge)?,
            };
            let k = &open.files[next];
            Ok(open_file(
                FileRef {
                    repo_id: k.repo_id,
                    path: k.path.clone(),
                },
                0,
            ))
        }
        Command::Open => match focus {
            Focus::ReviewList { index } => view
                .reviews
                .get(index)
                .map(|r| Action::OpenReview { review_id: r.id })
                .ok_or_else(nothing),
            Focus::Tree { index } => match visible_nodes(view).get(index) {
                Some(TreeNode::File { repo_id, path, .. }) => {
                    let file = FileRef {
                        repo_id: *repo_id,
                        path: path.clone(),
                    };
                    // Already open: just move into it.
                    match &view.diff {
                        Some(d) if d.file == file => Ok(Action::SetFocus {
                            focus: Focus::Diff { row: d.first_row },
                        }),
                        Some(_) | None => Ok(open_file(file, 0)),
                    }
                }
                Some(TreeNode::Dir { repo_id, path, .. }) => Ok(Action::ToggleDir {
                    repo_id: *repo_id,
                    path: path.clone(),
                }),
                None => Err(nothing()),
            },
            Focus::Diff { row } => {
                let diff = view.diff.as_ref().ok_or(NoTarget::NoOpenFile)?;
                let thread = diff
                    .rows
                    .iter()
                    .find(|r| r.index == row)
                    .and_then(|r| r.threads.first())
                    .ok_or_else(nothing)?;
                let index = view
                    .threads
                    .iter()
                    .position(|t| t.id == *thread)
                    .ok_or_else(nothing)?;
                Ok(Action::SetFocus {
                    focus: Focus::Thread { index },
                })
            }
            Focus::Thread { index } => {
                let t = view.threads.get(index).ok_or_else(nothing)?;
                let (file, row) = match &t.place {
                    ThreadPlace::Lines { file, end, .. } => (file, end - 1),
                    ThreadPlace::File { file } => (file, 0),
                    ThreadPlace::Review => return Err(nothing()),
                };
                // Already looking at that file: jump to the row (the
                // viewport follows); otherwise open it around the row.
                if view.diff.as_ref().is_some_and(|d| d.file == *file) {
                    Ok(Action::SetFocus {
                        focus: Focus::Diff { row },
                    })
                } else {
                    Ok(open_file(file.clone(), row))
                }
            }
            Focus::CommitStepper { index } => Ok(Action::StepCommit {
                selected: Some(index),
            }),
            Focus::Composer | Focus::Help => Err(nothing()),
        },
        Command::Back => {
            if view.tree.search.is_some() {
                return Ok(Action::FileSearch { query: None });
            }
            match focus {
                Focus::Help => Ok(Action::ToggleHelp),
                Focus::Composer => Ok(Action::DraftDiscarded),
                Focus::Diff { .. } => Ok(Action::CloseFile),
                Focus::Tree { .. } => {
                    if view.review.is_some() {
                        Ok(Action::CloseReview)
                    } else {
                        Err(nothing())
                    }
                }
                Focus::Thread { .. } | Focus::CommitStepper { .. } => Ok(Action::SetFocus {
                    focus: Focus::Tree { index: 0 },
                }),
                Focus::ReviewList { .. } => Err(nothing()),
            }
        }
        Command::NextPanel => {
            if view.review.is_none() {
                return Err(NoTarget::NoOpenReview);
            }
            let next = match focus {
                Focus::Tree { .. } => {
                    if view.diff.is_some() {
                        Focus::Diff { row: 0 }
                    } else if !view.threads.is_empty() {
                        Focus::Thread { index: 0 }
                    } else {
                        return Err(nothing());
                    }
                }
                Focus::Diff { .. } => {
                    if view.threads.is_empty() {
                        Focus::Tree { index: 0 }
                    } else {
                        Focus::Thread { index: 0 }
                    }
                }
                Focus::Thread { .. }
                | Focus::CommitStepper { .. }
                | Focus::ReviewList { .. }
                | Focus::Composer
                | Focus::Help => Focus::Tree { index: 0 },
            };
            Ok(Action::SetFocus { focus: next })
        }
        Command::ToggleViewed => {
            let file = target_file(view, focus).ok_or_else(nothing)?;
            let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
            let head = open
                .files
                .iter()
                .find(|k| k.repo_id == file.repo_id && k.path == file.path)
                .and_then(|k| match &k.target {
                    RenderTarget::Diff { change } => change.new_blob(),
                    RenderTarget::Blob { oid } => Some(*oid),
                });
            let state = crate::explorer::viewed_state(
                &open.snapshot,
                core.author(),
                file.repo_id,
                &file.path,
                head,
            );
            Ok(match state {
                ViewedState::Viewed => Action::UnmarkViewed { file },
                ViewedState::ChangedSinceViewed | ViewedState::Unviewed => {
                    Action::MarkViewed { file }
                }
            })
        }
        Command::Comment => {
            let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
            let anchor = match focus {
                Focus::Diff { row } => {
                    let diff = view.diff.as_ref().ok_or(NoTarget::NoOpenFile)?;
                    let target = open
                        .files
                        .iter()
                        .find(|k| k.repo_id == diff.file.repo_id && k.path == diff.file.path)
                        .map(|k| &k.target)
                        .ok_or_else(nothing)?;
                    let r = diff
                        .rows
                        .iter()
                        .find(|r| r.index == row)
                        .ok_or_else(nothing)?;
                    line_anchor(&diff.file, target, &r.row).ok_or_else(nothing)?
                }
                Focus::Tree { .. } => {
                    let file = target_file(view, focus).ok_or_else(nothing)?;
                    let blob = open
                        .files
                        .iter()
                        .find(|k| k.repo_id == file.repo_id && k.path == file.path)
                        .and_then(|k| match &k.target {
                            RenderTarget::Diff { change } => change.new_blob(),
                            RenderTarget::Blob { oid } => Some(*oid),
                        })
                        .ok_or_else(nothing)?;
                    Anchor::File {
                        repo_id: file.repo_id,
                        path: file.path,
                        blob_oid: blob,
                    }
                }
                Focus::ReviewList { .. }
                | Focus::Thread { .. }
                | Focus::CommitStepper { .. }
                | Focus::Help => Anchor::Review,
                Focus::Composer => return Err(nothing()),
            };
            Ok(Action::DraftOpened { anchor })
        }
        Command::Reply => {
            let Focus::Thread { index } = focus else {
                return Err(nothing());
            };
            let t = view.threads.get(index).ok_or_else(nothing)?;
            Ok(Action::ReplyOpened { thread_id: t.id })
        }
        Command::Delete => {
            let Focus::Thread { index } = focus else {
                return Err(nothing());
            };
            let t = view.threads.get(index).ok_or_else(nothing)?;
            if t.author != *core.author() {
                return Err(nothing());
            }
            Ok(Action::DeleteComment { comment_id: t.root })
        }
        Command::ToggleResolved => {
            let Focus::Thread { index } = focus else {
                return Err(nothing());
            };
            let t = view.threads.get(index).ok_or_else(nothing)?;
            Ok(if t.resolved {
                Action::UnresolveThread { thread_id: t.id }
            } else {
                Action::ResolveThread { thread_id: t.id }
            })
        }
        Command::FileSearch => {
            if view.review.is_none() {
                return Err(NoTarget::NoOpenReview);
            }
            Ok(Action::FileSearch {
                query: if view.tree.search.is_some() {
                    None
                } else {
                    Some(String::new())
                },
            })
        }
        Command::ToggleLayout => Ok(Action::SetLayout {
            layout: match view.prefs.layout {
                Layout::Unified => Layout::Split,
                Layout::Split => Layout::Unified,
            },
        }),
        Command::ToggleWhitespace => Ok(Action::SetRenderOpts {
            ignore_whitespace: !view.prefs.ignore_whitespace,
            context_lines: view.prefs.context_lines,
        }),
        Command::ToggleHelp => Ok(Action::ToggleHelp),
        Command::Connect => Ok(Action::Connect),
        Command::Disconnect => Ok(Action::Disconnect),
        Command::Commits => {
            let file = target_file(view, focus);
            let repo_id = match (file, view.review.as_ref()) {
                (Some(f), _) => f.repo_id,
                (None, Some(open)) => open
                    .snapshot
                    .review
                    .targets
                    .iter()
                    .next()
                    .map(|t| t.repo_id)
                    .ok_or_else(nothing)?,
                (None, None) => return Err(NoTarget::NoOpenReview),
            };
            Ok(Action::ListCommits { repo_id })
        }
    }
}

/// A `Lines` anchor for `row` of `file`: the head side when the row has
/// one, else the base side. The context hash is a placeholder the daemon
/// replaces (it hashes the surrounding lines itself).
fn line_anchor(file: &FileRef, target: &RenderTarget, row: &Row) -> Option<Anchor> {
    let (side, line): (Side, LineNo) = match row {
        Row::Context { right, .. } | Row::Modified { right, .. } | Row::Added { right } => {
            (Side::Head, right.line_no)
        }
        Row::Removed { left } => (Side::Base, left.line_no),
        Row::HunkHeader { .. } | Row::Expander { .. } | Row::WhitespaceOnly => return None,
    };
    let blob: BlobOid = match (target, side) {
        (RenderTarget::Diff { change }, Side::Head) => change.new_blob()?,
        (RenderTarget::Diff { change }, Side::Base) => change.old_blob()?,
        (RenderTarget::Blob { oid }, Side::Head | Side::Base) => *oid,
    };
    Some(Anchor::Lines {
        repo_id: file.repo_id,
        path: file.path.clone(),
        side,
        blob_oid: blob,
        lines: LineRange::single(line),
        context_hash: ContextHash::new(0),
    })
}
