//! Section patches (plan 4.2/4.3): what a host pushes to its UI after an
//! `Effect::Render`. One patch per [`ViewSection`] carrying only that
//! section's part of the [`ViewModel`], so an IPC message is bounded by the
//! viewport (diff rows) or a list, never by the whole model. The UI applies
//! patches to its own copy of the model; `review` (the raw open-review
//! state) is core-internal and never pushed — everything the UI shows is
//! derived into the other sections.

use moor_protocol::{Review, ReviewId, RpcError, ViewSection, Workspace};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;

use crate::diff::{CommitStepper, DiffView, ThreadView};
use crate::explorer::{Progress, TreeView};
use crate::focus::Focus;
use crate::keymap::{HelpView, Hint};
use crate::view::{ConnectionView, Draft, ViewDelta, ViewModel, ViewPrefs};

/// The part of the view one section owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(ViewPatchKind), derive(Hash, strum::EnumIter))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ViewPatch {
    Connection {
        connection: ConnectionView,
        last_error: Option<RpcError>,
    },
    ReviewList {
        workspaces: Vec<Workspace>,
        reviews: Vec<Review>,
        open_review: Option<ReviewId>,
    },
    Tree {
        tree: TreeView,
    },
    /// The diff over the viewport plus the prefs that shape it (layout,
    /// whitespace), which change together.
    Diff {
        diff: Option<DiffView>,
        prefs: ViewPrefs,
    },
    Threads {
        threads: Vec<ThreadView>,
    },
    Conversation {
        conversation: Vec<ThreadView>,
    },
    CommitStepper {
        stepper: Option<CommitStepper>,
    },
    Progress {
        progress: Progress,
    },
    Focus {
        focus: Focus,
    },
    Hints {
        hints: Vec<Hint>,
    },
    Help {
        help: Option<HelpView>,
    },
    Draft {
        draft: Option<Draft>,
        pending_refresh: bool,
    },
}

impl ViewPatch {
    #[must_use]
    pub fn section(&self) -> ViewSection {
        match self {
            ViewPatch::Connection { .. } => ViewSection::Connection,
            ViewPatch::ReviewList { .. } => ViewSection::ReviewList,
            ViewPatch::Tree { .. } => ViewSection::Tree,
            ViewPatch::Diff { .. } => ViewSection::Diff,
            ViewPatch::Threads { .. } => ViewSection::Threads,
            ViewPatch::Conversation { .. } => ViewSection::Conversation,
            ViewPatch::CommitStepper { .. } => ViewSection::CommitStepper,
            ViewPatch::Progress { .. } => ViewSection::Progress,
            ViewPatch::Focus { .. } => ViewSection::Focus,
            ViewPatch::Hints { .. } => ViewSection::Hints,
            ViewPatch::Help { .. } => ViewSection::Help,
            ViewPatch::Draft { .. } => ViewSection::Draft,
        }
    }
}

impl ViewModel {
    /// The patch that carries `section` of this model.
    #[must_use]
    pub fn patch(&self, section: ViewSection) -> ViewPatch {
        match section {
            ViewSection::Connection => ViewPatch::Connection {
                connection: self.connection.clone(),
                last_error: self.last_error.clone(),
            },
            ViewSection::ReviewList => ViewPatch::ReviewList {
                workspaces: self.workspaces.clone(),
                reviews: self.reviews.clone(),
                open_review: self.open_review,
            },
            ViewSection::Tree => ViewPatch::Tree {
                tree: self.tree.clone(),
            },
            ViewSection::Diff => ViewPatch::Diff {
                diff: self.diff.clone(),
                prefs: self.prefs,
            },
            ViewSection::Threads => ViewPatch::Threads {
                threads: self.threads.clone(),
            },
            ViewSection::Conversation => ViewPatch::Conversation {
                conversation: self.conversation.clone(),
            },
            ViewSection::CommitStepper => ViewPatch::CommitStepper {
                stepper: self.stepper.clone(),
            },
            ViewSection::Progress => ViewPatch::Progress {
                progress: self.progress,
            },
            ViewSection::Focus => ViewPatch::Focus { focus: self.focus },
            ViewSection::Hints => ViewPatch::Hints {
                hints: self.hints.clone(),
            },
            ViewSection::Help => ViewPatch::Help {
                help: self.help.clone(),
            },
            ViewSection::Draft => ViewPatch::Draft {
                draft: self.draft.clone(),
                pending_refresh: self.pending_refresh,
            },
        }
    }

    /// Patches for every section a render delta names, in delta order.
    #[must_use]
    pub fn patches(&self, delta: &ViewDelta) -> Vec<ViewPatch> {
        delta.sections.iter().map(|s| self.patch(*s)).collect()
    }

    /// Every section, for a UI that just attached.
    #[must_use]
    pub fn full_patches(&self) -> Vec<ViewPatch> {
        use strum::IntoEnumIterator;
        ViewSection::iter().map(|s| self.patch(s)).collect()
    }

    /// Install a patch into this model (the UI-side copy).
    pub fn apply(&mut self, patch: ViewPatch) {
        match patch {
            ViewPatch::Connection {
                connection,
                last_error,
            } => {
                self.connection = connection;
                self.last_error = last_error;
            }
            ViewPatch::ReviewList {
                workspaces,
                reviews,
                open_review,
            } => {
                self.workspaces = workspaces;
                self.reviews = reviews;
                self.open_review = open_review;
            }
            ViewPatch::Tree { tree } => self.tree = tree,
            ViewPatch::Diff { diff, prefs } => {
                self.diff = diff;
                self.prefs = prefs;
            }
            ViewPatch::Threads { threads } => self.threads = threads,
            ViewPatch::Conversation { conversation } => self.conversation = conversation,
            ViewPatch::CommitStepper { stepper } => self.stepper = stepper,
            ViewPatch::Progress { progress } => self.progress = progress,
            ViewPatch::Focus { focus } => self.focus = focus,
            ViewPatch::Hints { hints } => self.hints = hints,
            ViewPatch::Help { help } => self.help = help,
            ViewPatch::Draft {
                draft,
                pending_refresh,
            } => {
                self.draft = draft;
                self.pending_refresh = pending_refresh;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn every_section_has_a_patch_and_full_patches_rebuild_the_view() {
        let source = ViewModel {
            pending_refresh: true,
            focus: Focus::Help,
            progress: Progress {
                viewed: 1,
                changed_since_viewed: 2,
                total: 3,
            },
            connection: ConnectionView::Subscribed,
            ..ViewModel::default()
        };
        for s in ViewSection::iter() {
            assert_eq!(source.patch(s).section(), s);
        }
        let mut copy = ViewModel::default();
        for p in source.full_patches() {
            copy.apply(p);
        }
        // `review` is never pushed; everything else arrives.
        assert_eq!(copy.review, None);
        copy.review.clone_from(&source.review);
        assert_eq!(copy, source);
    }
}
