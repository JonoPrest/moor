//! What the UI renders. Hosts read it after each `Effect::Render`, which
//! names the sections that changed.

use moor_protocol::{
    Anchor, ClientSeq, EventBody, Review, ReviewSnapshot, RpcError, TreeOid, ViewSection,
};

use crate::cache::RenderKey;
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
#[strum_discriminants(name(ConnectionViewKind), derive(Hash))]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub anchor: Anchor,
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

/// Everything the UI needs, kept identical across hosts. 3.5 fills in the
/// tree, diff rows, thread list and stepper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ViewModel {
    pub connection: ConnectionView,
    /// Last request error the daemon returned; cleared on (re)subscribe.
    pub last_error: Option<RpcError>,
    pub reviews: Vec<Review>,
    pub review: Option<OpenReview>,
    pub draft: Option<Draft>,
    /// A working-tree refresh arrived while `draft` was open and is being
    /// held back (§5.4).
    pub pending_refresh: bool,
}
