//! What the UI renders. Hosts read it after each `Effect::Render`, which
//! names the sections that changed.

use moor_protocol::{Anchor, Review, ReviewSnapshot, RpcError, ViewSection};
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

/// The review currently on screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenReview {
    pub snapshot: ReviewSnapshot,
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
