//! Connection state (plan 3.1). Every transition is explicit in
//! `ClientCore`; this module only names the states.

use moor_protocol::Seq;
use strum::EnumDiscriminants;

/// Where the connection to the daemon is. `last_seq` survives disconnects so
/// a reconnect subscribes `Since::After` and misses nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumDiscriminants)]
#[strum_discriminants(name(ConnectionKind), derive(Hash))]
pub enum Connection {
    Disconnected {
        last_seq: Option<Seq>,
    },
    /// `Effect::Connect` issued. `hello_sent` flips when the transport comes
    /// up and the `Hello` goes out; `Welcome` then triggers the subscribe.
    Connecting {
        hello_sent: bool,
        last_seq: Option<Seq>,
    },
    /// Handshake done and subscribed; `last_seq` is the newest event seen.
    Subscribed {
        last_seq: Seq,
    },
}

impl Connection {
    #[must_use]
    pub fn kind(&self) -> ConnectionKind {
        ConnectionKind::from(self)
    }

    /// The newest sequence number this client has seen, if any.
    #[must_use]
    pub fn last_seq(&self) -> Option<Seq> {
        match *self {
            Connection::Disconnected { last_seq } | Connection::Connecting { last_seq, .. } => {
                last_seq
            }
            Connection::Subscribed { last_seq } => Some(last_seq),
        }
    }
}
