//! What the UI renders. Hosts read it after each `Effect::Render`, which
//! names the sections that changed.

use moor_protocol::{
    Anchor, ClientSeq, EventBody, RenderOpts, Review, ReviewId, ReviewSnapshot, RpcError, ThreadId,
    TreeOid, ViewSection, Workspace,
};

use crate::cache::RenderKey;
use crate::diff::{CommitStepper, DiffView, ThreadView};
use crate::explorer::{Progress, TreeView};
use crate::focus::Focus;
use crate::keymap::{HelpView, Hint};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;

/// The sections of the [`ViewModel`] that changed, in first-touched order,
/// without duplicates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewDelta {
    pub sections: Vec<ViewSection>,
}

impl ViewDelta {
    #[must_use]
    pub fn new(sections: &[ViewSection]) -> Self {
        let mut out: Vec<ViewSection> = Vec::with_capacity(sections.len());
        for s in sections {
            if !out.contains(s) {
                out.push(*s);
            }
        }
        Self { sections: out }
    }
}

/// Connection state as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumDiscriminants, Default)]
#[strum_discriminants(name(ConnectionViewKind), derive(Hash, strum::EnumIter))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ConnectionView {
    #[default]
    Disconnected,
    Connecting,
    Subscribed,
    /// The daemon refused the handshake; shown until the next `Connect`.
    Rejected {
        error: RpcError,
    },
}

/// A comment editor is open at `anchor`. Its text stays in the host.
/// `reply_to` makes it a reply in that thread (the anchor is the root's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub anchor: Anchor,
    pub reply_to: Option<ThreadId>,
}

/// The file the user is looking at and where. Rows are indices into the
/// render model; the core turns them into chunk requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenFile {
    pub render: RenderKey,
    pub first_row: u32,
    pub last_row: u32,
}

/// A mutation this client applied locally that the daemon has not echoed
/// yet. Hosts mark what `body` touches (a comment, a thread) as pending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingEvent {
    pub client_seq: ClientSeq,
    pub body: EventBody,
}

/// The review currently on screen. Content (trees, headers, chunks) lives in
/// the cache; this names what of it belongs to the review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenReview {
    /// Committed state plus every pending mutation applied on top (§5.2).
    pub snapshot: ReviewSnapshot,
    pub pending: Vec<PendingEvent>,
    /// Tree roots of the resolved targets (base and head per repo).
    pub trees: Vec<TreeOid>,
    /// Changed files, in daemon order; one render key each.
    pub files: Vec<RenderKey>,
    pub open_file: Option<OpenFile>,
}

impl OpenReview {
    #[must_use]
    pub fn new(snapshot: ReviewSnapshot) -> Self {
        Self {
            snapshot,
            pending: Vec::new(),
            trees: Vec::new(),
            files: Vec::new(),
            open_file: None,
        }
    }
}

/// How diff rows are laid out.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, strum::EnumIter,
)]
pub enum Layout {
    #[default]
    Unified,
    Split,
}

/// User preferences, persisted in the host KV under [`ViewPrefs::KEY`].
/// `ignore_whitespace` / `context_lines` are the render options every
/// request uses, so changing them re-keys the render cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewPrefs {
    pub layout: Layout,
    pub ignore_whitespace: bool,
    pub context_lines: u32,
}

impl ViewPrefs {
    /// Host KV key the preferences live under.
    pub const KEY: &'static str = "moor/prefs";

    #[must_use]
    pub fn render_opts(self) -> RenderOpts {
        RenderOpts {
            ignore_whitespace: self.ignore_whitespace,
            context_lines: self.context_lines,
        }
    }
}

impl Default for ViewPrefs {
    fn default() -> Self {
        let opts = RenderOpts::default();
        Self {
            layout: Layout::Unified,
            ignore_whitespace: opts.ignore_whitespace,
            context_lines: opts.context_lines,
        }
    }
}

/// Everything the UI needs, kept identical across hosts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ViewModel {
    pub prefs: ViewPrefs,
    /// Explorer over the open review's head trees (§5.5); empty otherwise.
    pub tree: TreeView,
    pub progress: Progress,
    /// The open file over its viewport, with comment overlays (§6.5).
    pub diff: Option<DiffView>,
    /// Every thread of the open review, oldest first.
    pub threads: Vec<ThreadView>,
    /// Review-level threads (the conversation panel).
    pub conversation: Vec<ThreadView>,
    pub stepper: Option<CommitStepper>,
    /// Where keys go (§6.4). Always valid for the current lists.
    pub focus: Focus,
    /// Primary bindings for the focused context, for the hint bar.
    pub hints: Vec<Hint>,
    /// The `?` overlay while open.
    pub help: Option<HelpView>,
    pub connection: ConnectionView,
    /// Last request error the daemon returned; cleared on (re)subscribe.
    pub last_error: Option<RpcError>,
    /// Workspaces and their repos, listed on subscribe; the review list is
    /// the union of every workspace's reviews.
    pub workspaces: Vec<Workspace>,
    pub reviews: Vec<Review>,
    /// Which of `reviews` is open, for headers (the review itself is in
    /// `review`, which is never pushed to the UI).
    pub open_review: Option<ReviewId>,
    pub review: Option<OpenReview>,
    pub draft: Option<Draft>,
    /// A working-tree refresh arrived while `draft` was open and is being
    /// held back (§5.4).
    pub pending_refresh: bool,
}
