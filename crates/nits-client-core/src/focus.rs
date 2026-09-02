//! Focus model and command resolution (ARCHITECTURE §6.4). Focus is core
//! state, so "what does `j` do" is answered here from the `ViewModel`, never
//! from DOM focus. [`resolve`] turns a [`Command`] into the one [`Action`]
//! it means right now, or says why it means nothing.

use nits_protocol::{
    Anchor, BlobOid, ContextHash, DiffScope, LineNo, LineRange, RenderTarget, Row, Side,
};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;

use crate::content::FileRef;
use crate::diff::ThreadPlace;
use crate::explorer::{TreeNode, ViewedState};
use crate::keymap::{Command, Context};
use crate::view::{Layout, Tab, ViewModel};
use crate::{Action, ClientCore, ScopeChoice};

/// Rows a file opens with, and a page for `PageDown`/`PageUp`.
pub const PAGE_ROWS: u32 = 60;

/// Where the user is. Indices are into the corresponding view lists; the
/// tree index counts visible nodes in display order (expanded dirs only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(FocusKind), derive(Hash, strum::EnumIter))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Focus {
    ReviewList {
        index: usize,
    },
    Tree {
        index: usize,
    },
    /// A row of the open diff, and which half of it is the target: a
    /// modified row is two commentable cells, so the side is part of
    /// where the user is, not something inferred when they comment.
    Diff {
        row: u32,
        side: Side,
    },
    Thread {
        index: usize,
    },
    Composer,
    CommitStepper {
        index: usize,
    },
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

/// Where a Visual selection started: the row `V` was pressed on and the
/// half of it that was targeted. The side is held for the whole selection
/// — what `c` anchors to must not depend on which rows it happens to span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualAnchor {
    pub row: u32,
    pub side: Side,
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
            nits_protocol::RenderContent::Text { total_rows, .. } => total_rows as usize,
            nits_protocol::RenderContent::Binary => 0,
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
        Focus::Diff { side, .. } => Focus::Diff {
            row: u32::try_from(index).unwrap_or(u32::MAX),
            side,
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
        Focus::Diff { row, .. } => Some(row as usize),
        Focus::Composer | Focus::Help => None,
    }
}

/// Clamp a focus to the lists it indexes; a focus over an empty list or a
/// closed panel falls back to the tree or the review list.
#[must_use]
/// The side a new diff focus keeps: the one the focus is already on, so
/// stepping down the red half of a hunk stays on the red half.
fn side_of(focus: Focus) -> Side {
    match focus {
        Focus::Diff { side, .. } => side,
        Focus::ReviewList { .. }
        | Focus::Tree { .. }
        | Focus::Thread { .. }
        | Focus::Composer
        | Focus::CommitStepper { .. }
        | Focus::Help => Side::Head,
    }
}

/// The side a diff focus settles on for `row`: the asked-for side when
/// that row has a cell there, else the only side it does have. A row
/// that is not cached (or has no cell at all, like a hunk header) keeps
/// the asked-for side.
fn settled_side(diff: &crate::diff::DiffView, row: u32, side: Side) -> Side {
    let Some(r) = diff.rows.iter().find(|r| r.index == row) else {
        return side;
    };
    if crate::diff::line_on(&r.row, side).is_some() {
        return side;
    }
    let other = side.other();
    if crate::diff::line_on(&r.row, other).is_some() {
        other
    } else {
        side
    }
}

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
        Focus::Diff { row, side } => match &view.diff {
            Some(diff) => {
                let n = extent(view, Context::Diff);
                let row = if n == 0 {
                    0
                } else {
                    row.min(u32::try_from(n - 1).unwrap_or(u32::MAX))
                };
                Focus::Diff {
                    row,
                    side: settled_side(diff, row, side),
                }
            }
            // A file is open but its header has not landed: the diff pane
            // is still the place to be (jump-to-context opens this way).
            None if view.review.as_ref().is_some_and(|r| r.open_file.is_some()) => {
                Focus::Diff { row, side }
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

/// Whether the stacked section for `render` is folded (skipped by motions).
fn collapsed_of(view: &ViewModel, render: &crate::cache::RenderKey) -> bool {
    view.diffs
        .iter()
        .find(|d| d.file.repo_id == render.repo_id && d.file.path == render.path)
        .is_some_and(|d| d.collapsed)
}

/// The next/previous file relative to the open one. A folded file is
/// still landed on (its header takes focus; `enter` unfolds); an open
/// one lands at its start (forward) or last cached-total row (backward).
fn adjacent_file(core: &ClientCore, forward: bool) -> Result<Action, NoTarget> {
    let view = core.view();
    let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
    let cur = open
        .open_file
        .as_ref()
        .and_then(|f| open.files.iter().position(|k| *k == f.render))
        .ok_or(NoTarget::NoOpenFile)?;
    let i = if forward {
        if cur + 1 >= open.files.len() {
            return Err(NoTarget::AtEdge);
        }
        cur + 1
    } else {
        cur.checked_sub(1).ok_or(NoTarget::AtEdge)?
    };
    let k = &open.files[i];
    let row = if forward || collapsed_of(view, k) {
        0
    } else {
        crate::diff::total_rows_of(core.cache(), k)
            .unwrap_or(1)
            .saturating_sub(1)
    };
    Ok(open_file(
        FileRef {
            repo_id: k.repo_id,
            path: k.path.clone(),
        },
        row,
    ))
}

/// Whether the open file's section is folded (a single motion stop).
fn open_file_collapsed(core: &ClientCore) -> bool {
    let view = core.view();
    view.review
        .as_ref()
        .and_then(|o| o.open_file.as_ref())
        .is_some_and(|f| collapsed_of(view, &f.render))
}

/// A `SetFocus` on a settled focus: a motion onto a row that has no cell
/// on the side being followed lands on the side it does have.
fn set_focus(view: &ViewModel, focus: Focus) -> Action {
    Action::SetFocus {
        focus: clamp(view, focus),
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
        Ok(set_focus(
            view,
            with_index(focus, usize::try_from(next).unwrap_or(0)),
        ))
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
        Ok(set_focus(view, with_index(focus, target)))
    };
    let in_visual = core.visual_anchor().is_some() && matches!(focus, Focus::Diff { .. });
    match command {
        // In the diff the edges continue into the adjacent file — the
        // stacked view is one long document. A folded file is one stop:
        // any vertical motion leaves it immediately. Visual mode stays in
        // the file: the selection can only span the open file's rows.
        Command::MoveDown => match focus {
            Focus::Diff { .. } if in_visual => step(1),
            Focus::Diff { .. } if open_file_collapsed(core) => adjacent_file(core, true),
            Focus::Diff { .. } => match step(1) {
                Err(NoTarget::AtEdge) => adjacent_file(core, true),
                r => r,
            },
            Focus::ReviewList { .. }
            | Focus::Tree { .. }
            | Focus::Thread { .. }
            | Focus::Composer
            | Focus::CommitStepper { .. }
            | Focus::Help => step(1),
        },
        Command::MoveUp => match focus {
            Focus::Diff { .. } if in_visual => step(-1),
            Focus::Diff { .. } if open_file_collapsed(core) => adjacent_file(core, false),
            Focus::Diff { .. } => match step(-1) {
                Err(NoTarget::AtEdge) => adjacent_file(core, false),
                r => r,
            },
            Focus::ReviewList { .. }
            | Focus::Tree { .. }
            | Focus::Thread { .. }
            | Focus::Composer
            | Focus::CommitStepper { .. }
            | Focus::Help => step(-1),
        },
        Command::PageDown => step(i64::from(PAGE_ROWS)),
        Command::PageUp => step(-i64::from(PAGE_ROWS)),
        Command::GoTop => jump(false),
        Command::GoBottom => jump(true),
        Command::NextHunk | Command::PrevHunk | Command::NextComment | Command::PrevComment => {
            // In by-commit scope `n`/`p` step commits (UI-DESIGN
            // §bindings), from any list the keys are bound in.
            if matches!(command, Command::NextHunk | Command::PrevHunk)
                && let Some(open) = &view.review
                && matches!(
                    open.scope,
                    DiffScope::Commit { .. } | DiffScope::Worktree { .. }
                )
            {
                let stepper = view.stepper.as_ref().ok_or_else(nothing)?;
                // Steps run oldest → newest → worktree; commits are listed
                // newest first, so "next" moves toward index 0, then None.
                let worktree_last = matches!(open.scope, DiffScope::Worktree { .. })
                    || open.snapshot.resolved.as_ref().is_some_and(|r| {
                        r.iter().any(|t| {
                            t.repo_id == stepper.repo_id
                                && matches!(
                                    t.head.source,
                                    nits_protocol::ResolvedSource::WorkingTree { .. }
                                )
                        })
                    });
                let selected = match open.scope {
                    DiffScope::Commit { oid, .. } => {
                        stepper.commits.iter().position(|c| c.oid == oid)
                    }
                    DiffScope::All | DiffScope::Committed | DiffScope::Worktree { .. } => None,
                };
                let next = match (command == Command::NextHunk, selected) {
                    // Toward the worktree (newer).
                    (true, Some(0)) if worktree_last => None,
                    (true, Some(0) | None) => return Err(NoTarget::AtEdge),
                    (true, Some(i)) => Some(i - 1),
                    // Toward the base (older).
                    (false, None) if !stepper.commits.is_empty() => Some(0),
                    (false, Some(i)) if i + 1 < stepper.commits.len() => Some(i + 1),
                    (false, Some(_) | None) => return Err(NoTarget::AtEdge),
                };
                return Ok(Action::StepCommit { selected: next });
            }
            let Focus::Diff { row, .. } = focus else {
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
                | Command::ApplySuggestion
                | Command::ToggleResolved
                | Command::FileSearch
                | Command::ToggleLayout
                | Command::ToggleWhitespace
                | Command::ToggleHelp
                | Command::TabFiles
                | Command::TabConversation
                | Command::TabBrowse
                | Command::CopyPath
                | Command::ToggleSidebar
                | Command::CollapseParent
                | Command::CollapseAll
                | Command::ToggleFileCollapse
                | Command::ExpandFile
                | Command::FocusTree
                | Command::FocusDiff
                | Command::FocusThreads
                | Command::FocusCommits
                | Command::Submit
                | Command::Connect
                | Command::Disconnect
                | Command::Commits
                | Command::Refresh
                | Command::ScopeAll
                | Command::ScopeByCommit
                | Command::ScopeWorktree
                | Command::ExpandContext
                | Command::ContentSearch
                | Command::ActionPalette
                | Command::VisualMode
                | Command::ExpandUp
                | Command::ExpandDown
                | Command::CommentOnFile
                | Command::SideBase
                | Command::SideHead => false,
            };
            // A folded open file contributes no in-file stops.
            let found = if collapsed_of(view, render) {
                None
            } else if forward {
                rows.iter().find(|r| r.index > row && wanted(r))
            } else {
                rows.iter().rev().find(|r| r.index < row && wanted(r))
            };
            if let Some(r) = found {
                return Ok(set_focus(
                    view,
                    Focus::Diff {
                        row: r.index,
                        side: side_of(focus),
                    },
                ));
            }
            // No further match in this file: scan onward through the
            // stacked files (skipping folded ones) for the next one.
            let cur = open
                .files
                .iter()
                .position(|k| k == render)
                .ok_or(NoTarget::AtEdge)?;
            let mut i = cur;
            loop {
                i = if forward {
                    if i + 1 >= open.files.len() {
                        return Err(NoTarget::AtEdge);
                    }
                    i + 1
                } else {
                    i.checked_sub(1).ok_or(NoTarget::AtEdge)?
                };
                let k = &open.files[i];
                // A folded file is one stop: its header takes focus.
                if collapsed_of(view, k) {
                    return Ok(open_file(
                        FileRef {
                            repo_id: k.repo_id,
                            path: k.path.clone(),
                        },
                        0,
                    ));
                }
                let rows = crate::diff::all_rows(core.cache(), &open.snapshot, k);
                let hit = if forward {
                    rows.iter().find(|r| wanted(r))
                } else {
                    rows.iter().rev().find(|r| wanted(r))
                };
                if let Some(r) = hit {
                    let row = r.index;
                    return Ok(open_file(
                        FileRef {
                            repo_id: k.repo_id,
                            path: k.path.clone(),
                        },
                        row,
                    ));
                }
            }
        }
        Command::NextFile | Command::PrevFile => {
            let Some(open) = &view.review else {
                return Err(NoTarget::NoOpenReview);
            };
            if open.files.is_empty() {
                return Err(nothing());
            }
            // The daemon serves files in tree display order already.
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
                        Some(d) if d.file == file => Ok(set_focus(
                            view,
                            Focus::Diff {
                                row: d.first_row,
                                side: side_of(focus),
                            },
                        )),
                        Some(_) | None => Ok(open_file(file, 0)),
                    }
                }
                Some(TreeNode::Dir { repo_id, path, .. }) => Ok(Action::ToggleDir {
                    repo_id: *repo_id,
                    path: path.clone(),
                }),
                None => Err(nothing()),
            },
            Focus::Diff { row, side } => {
                let diff = view.diff.as_ref().ok_or(NoTarget::NoOpenFile)?;
                // On a folded file, enter unfolds (C folds it back).
                if collapsed_of(
                    view,
                    view.review
                        .as_ref()
                        .and_then(|o| o.open_file.as_ref().map(|f| &f.render))
                        .ok_or(NoTarget::NoOpenFile)?,
                ) {
                    return Ok(Action::ToggleFileCollapse {
                        file: diff.file.clone(),
                    });
                }
                // The focused half's thread first: on a modified row the
                // red and the green cell can each carry one.
                let thread = diff
                    .rows
                    .iter()
                    .find(|r| r.index == row)
                    .and_then(|r| {
                        r.threads
                            .iter()
                            .find(|t| t.side == side)
                            .or_else(|| r.threads.first())
                    })
                    .ok_or_else(nothing)?;
                let index = view
                    .threads
                    .iter()
                    .position(|t| t.id == thread.thread)
                    .ok_or_else(nothing)?;
                Ok(Action::SetFocus {
                    focus: Focus::Thread { index },
                })
            }
            Focus::Thread { index } => {
                let t = view.threads.get(index).ok_or_else(nothing)?;
                // An outdated thread's location is gone from the current
                // diff: open the diff it was made on instead (UI-DESIGN
                // §Comments).
                if t.outdated && t.context.is_some() {
                    return Ok(Action::OpenOriginalDiff { thread_id: t.id });
                }
                let (file, row) = match &t.place {
                    ThreadPlace::Lines { file, end, .. } => (file, end - 1),
                    ThreadPlace::File { file } => (file, 0),
                    ThreadPlace::Review => return Err(nothing()),
                };
                // Already looking at that file: jump to the row (the
                // viewport follows); otherwise open it around the row.
                if view.diff.as_ref().is_some_and(|d| d.file == *file) {
                    Ok(set_focus(
                        view,
                        Focus::Diff {
                            row,
                            // A line thread is shown against the side it is
                            // anchored to, so stepping to it lands there.
                            side: match &t.place {
                                ThreadPlace::Lines { side, .. } => *side,
                                ThreadPlace::File { .. } | ThreadPlace::Review => Side::Head,
                            },
                        },
                    ))
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
            if in_visual {
                return Ok(Action::LeaveVisual);
            }
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
                        Focus::Diff {
                            row: 0,
                            side: Side::Head,
                        }
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
            Ok(set_focus(view, next))
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
        Command::Comment if in_visual => {
            let (Some(anchor), Focus::Diff { row, .. }) = (core.visual_anchor(), focus) else {
                return Err(nothing());
            };
            let diff = view.diff.as_ref().ok_or(NoTarget::NoOpenFile)?;
            let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
            let render = open
                .open_file
                .as_ref()
                .map(|f| &f.render)
                .ok_or(NoTarget::NoOpenFile)?;
            let (lo, hi) = (anchor.row.min(row), anchor.row.max(row));
            // The side is the one the selection started on, held for its
            // duration; a row with no cell there contributes no line.
            let side = anchor.side;
            let rows = crate::diff::all_rows(core.cache(), &open.snapshot, render);
            let lines: Vec<u32> = rows
                .iter()
                .filter(|r| r.index >= lo && r.index <= hi)
                .filter_map(|r| crate::diff::line_on(&r.row, side))
                .collect();
            let (Some(start), Some(end)) = (lines.iter().min(), lines.iter().max()) else {
                return Err(nothing());
            };
            Ok(Action::CommentLines {
                file: diff.file.clone(),
                side,
                start_line: *start,
                end_line: *end,
            })
        }
        Command::Comment => {
            let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
            let anchor = match focus {
                Focus::Diff { row, side } => {
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
                    line_anchor(&diff.file, target, &r.row, side).ok_or_else(nothing)?
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
        Command::ApplySuggestion => {
            let Focus::Thread { index } = focus else {
                return Err(nothing());
            };
            let t = view.threads.get(index).ok_or_else(nothing)?;
            if !t.suggestion {
                return Err(nothing());
            }
            Ok(Action::ApplySuggestion { comment_id: t.root })
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
        Command::TabFiles => Ok(Action::SetTab {
            tab: Tab::FilesChanged,
        }),
        Command::TabConversation => Ok(Action::SetTab {
            tab: Tab::Conversation,
        }),
        Command::TabBrowse => Ok(Action::SetTab { tab: Tab::Browse }),
        Command::ToggleSidebar => Ok(Action::ToggleSidebar),
        Command::CopyPath => {
            let file = target_file(view, focus).ok_or_else(nothing)?;
            Ok(Action::CopyPath { path: file.path })
        }
        Command::CollapseParent => match focus {
            Focus::Tree { .. } => Ok(Action::CollapseParent),
            Focus::ReviewList { .. }
            | Focus::Diff { .. }
            | Focus::Thread { .. }
            | Focus::Composer
            | Focus::CommitStepper { .. }
            | Focus::Help => Err(nothing()),
        },
        Command::CollapseAll => Ok(Action::CollapseAll),
        Command::ToggleFileCollapse => {
            let file = target_file(view, focus).ok_or_else(nothing)?;
            Ok(Action::ToggleFileCollapse { file })
        }
        Command::ExpandFile => {
            let file = target_file(view, focus).ok_or_else(nothing)?;
            Ok(Action::ExpandContext { file, full: true })
        }
        Command::FocusTree => {
            if view.review.is_some() {
                Ok(Action::SetFocus {
                    focus: Focus::Tree { index: 0 },
                })
            } else {
                Err(NoTarget::NoOpenReview)
            }
        }
        Command::FocusDiff => match &view.diff {
            Some(d) => Ok(set_focus(
                view,
                Focus::Diff {
                    row: d.first_row,
                    side: side_of(focus),
                },
            )),
            None => Err(NoTarget::NoOpenFile),
        },
        Command::FocusThreads => {
            if view.threads.is_empty() {
                Err(nothing())
            } else {
                Ok(Action::SetFocus {
                    focus: Focus::Thread { index: 0 },
                })
            }
        }
        Command::FocusCommits => match &view.stepper {
            Some(_) => Ok(Action::SetFocus {
                focus: Focus::CommitStepper { index: 0 },
            }),
            None => Err(NoTarget::Nothing(command)),
        },
        // The composer lives in the host, which handles the submit chord
        // itself; the binding exists for hints and tooltips only.
        Command::Submit => Err(nothing()),
        Command::Connect => Ok(Action::Connect),
        Command::Disconnect => Ok(Action::Disconnect),
        Command::Refresh => Ok(Action::ListWorkspaces),
        Command::ContentSearch => {
            if view.review.is_none() {
                return Err(NoTarget::NoOpenReview);
            }
            let (query, all_files) = view
                .content_search
                .as_ref()
                .map_or((String::new(), false), |c| (c.query.clone(), c.all_files));
            Ok(Action::ContentSearch {
                query: Some(query),
                all_files,
            })
        }
        Command::ActionPalette => Ok(Action::ActionPalette {
            open: !view.action_palette,
        }),
        // The directional expands re-render the whole file with more
        // context until band splicing gives them distinct semantics.
        Command::ExpandContext | Command::ExpandUp | Command::ExpandDown => {
            let Focus::Diff { .. } = focus else {
                return Err(nothing());
            };
            let diff = view.diff.as_ref().ok_or(NoTarget::NoOpenFile)?;
            Ok(Action::ExpandContext {
                file: diff.file.clone(),
                full: false,
            })
        }
        Command::SideBase | Command::SideHead => {
            // The visual selection's side is fixed at its start, so
            // flipping half-way would lie about what `c` will anchor to.
            if in_visual {
                return Err(nothing());
            }
            let want = if command == Command::SideBase {
                Side::Base
            } else {
                Side::Head
            };
            let Focus::Diff { row, side } = focus else {
                return Err(nothing());
            };
            if side == want {
                return Err(NoTarget::AtEdge);
            }
            let diff = view.diff.as_ref().ok_or(NoTarget::NoOpenFile)?;
            let r = diff
                .rows
                .iter()
                .find(|r| r.index == row)
                .ok_or_else(nothing)?;
            if crate::diff::line_on(&r.row, want).is_none() {
                return Err(NoTarget::AtEdge);
            }
            Ok(Action::SetFocus {
                focus: Focus::Diff { row, side: want },
            })
        }
        Command::CommentOnFile => {
            let file = target_file(view, focus).ok_or_else(nothing)?;
            Ok(Action::CommentFile { file })
        }
        Command::ScopeAll => {
            if view.review.is_none() {
                return Err(NoTarget::NoOpenReview);
            }
            Ok(Action::SetScope {
                scope: ScopeChoice::All,
            })
        }
        Command::ScopeByCommit => {
            if view.review.is_none() {
                return Err(NoTarget::NoOpenReview);
            }
            Ok(Action::SetScope {
                scope: ScopeChoice::ByCommit,
            })
        }
        Command::ScopeWorktree => {
            let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
            // Toggle the `+ working tree` half of the all-changes scope;
            // from any other scope it lands on Committed.
            Ok(Action::SetScope {
                scope: match open.scope {
                    DiffScope::Committed => ScopeChoice::All,
                    DiffScope::All | DiffScope::Commit { .. } | DiffScope::Worktree { .. } => {
                        ScopeChoice::Committed
                    }
                },
            })
        }
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
        Command::VisualMode => {
            if in_visual {
                return Ok(Action::LeaveVisual);
            }
            let Focus::Diff { row, side } = focus else {
                return Err(nothing());
            };
            let diff = view.diff.as_ref().ok_or(NoTarget::NoOpenFile)?;
            let open = view.review.as_ref().ok_or(NoTarget::NoOpenReview)?;
            let target = open
                .files
                .iter()
                .find(|k| k.repo_id == diff.file.repo_id && k.path == diff.file.path)
                .map(|k| &k.target)
                .ok_or_else(nothing)?;
            // Only a commentable row starts a selection.
            let r = diff
                .rows
                .iter()
                .find(|r| r.index == row)
                .ok_or_else(nothing)?;
            line_anchor(&diff.file, target, &r.row, side).ok_or_else(nothing)?;
            Ok(Action::EnterVisual)
        }
    }
}

/// A `Lines` anchor for `side` of `row` of `file`, or `None` when that half
/// of the row has no line: an added row has no base cell, a removed row no
/// head cell, a hunk header neither. The context hash is a placeholder the
/// daemon replaces (it hashes the surrounding lines itself).
fn line_anchor(file: &FileRef, target: &RenderTarget, row: &Row, side: Side) -> Option<Anchor> {
    let line = LineNo::new(crate::diff::line_on(row, side)?)?;
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
