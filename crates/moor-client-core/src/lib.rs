//! Sans-I/O client core (plan milestone 3, `docs/ARCHITECTURE.md` §5).
//!
//! [`ClientCore`] is a pure state machine: the host feeds it every [`Input`]
//! (user actions, server frames, transport changes, stored values, clock
//! ticks) and acts on the returned [`Effect`]s. It never touches a socket, a
//! disk or a clock, so it runs unchanged in Tauri, a browser (wasm) and the
//! TUI, and tests drive it without mocks.
//!
//! Rules that hold for every input:
//! - An input is either applied (state may change, effects returned) or
//!   rejected with a typed [`CoreError`]; a rejected input leaves the core
//!   exactly as it was and produces no effects.
//! - Draft text never enters the core. `Action::DraftOpened` /
//!   `DraftSubmitted { body }` / `DraftDiscarded` are the only crossings.
//! - `Effect::Render` names only the [`ViewSection`]s that changed.

#![deny(clippy::wildcard_enum_match_arm)]

mod connection;
mod ids;
mod view;

use std::collections::BTreeMap;

use moor_protocol::{
    Anchor, Author, BuildInfo, ClientId, ClientMsg, ClientSeq, CommentKind, EventBody, Mutation,
    ProtocolVersion, Request, RequestId, Response, ReviewId, RpcError, Seq, ServerMsg, Since,
    StreamItem, SubscribeScope, ViewSection, WorkspaceId,
};
use strum::EnumDiscriminants;

pub use connection::{Connection, ConnectionKind};
pub use ids::IdSeed;
pub use view::{ConnectionView, Draft, OpenReview, ViewDelta, ViewModel};

pub use moor_protocol as protocol;

/// Milliseconds on the host's monotonic clock, delivered via `Input::Tick`.
pub type Millis = u64;

/// Key in the host's key-value store (`Effect::Persist` / `Effect::Load`).
pub type Key = String;

/// Something the host tells the core.
#[derive(Debug, Clone, PartialEq, Eq, EnumDiscriminants)]
#[strum_discriminants(name(InputKind), derive(Hash))]
pub enum Input {
    User(Action),
    Server(ServerMsg),
    Transport(TransportEvent),
    /// Answer to an `Effect::Load`; `None` when the key is absent.
    Stored {
        key: Key,
        value: Option<Vec<u8>>,
    },
    /// The host's clock advanced. Drives timeouts and id generation.
    Tick(Millis),
}

/// What the transport layer observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportEvent {
    /// The connection the core asked for in `Effect::Connect` is up.
    Connected,
    /// The connection dropped (or the dial failed).
    Disconnected,
}

/// A user intent, already resolved from keys or clicks by the host.
#[derive(Debug, Clone, PartialEq, Eq, EnumDiscriminants)]
#[strum_discriminants(name(ActionKind), derive(Hash))]
pub enum Action {
    Connect,
    Disconnect,
    ListReviews {
        workspace_id: WorkspaceId,
    },
    OpenReview {
        review_id: ReviewId,
    },
    CloseReview,
    /// The user started writing a comment at `anchor`. The editor is the
    /// host's; the core only records that one is open.
    DraftOpened {
        anchor: Anchor,
    },
    DraftSubmitted {
        body: String,
    },
    DraftDiscarded,
}

/// Something the host must do for the core.
#[derive(Debug, Clone, PartialEq, Eq, EnumDiscriminants)]
#[strum_discriminants(name(EffectKind), derive(Hash))]
pub enum Effect {
    /// Dial the daemon; report the outcome as `TransportEvent`.
    Connect,
    /// Close the connection. The host still reports `Disconnected`.
    Disconnect,
    Send(ClientMsg),
    Persist {
        key: Key,
        value: Vec<u8>,
    },
    Load {
        key: Key,
    },
    Render(ViewDelta),
}

/// Why an input was rejected. The core is unchanged after any of these.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error("{input:?} is not valid while the connection is {state:?}")]
    WrongConnectionState {
        input: InputKind,
        state: ConnectionKind,
    },
    #[error("server frame for unknown request id {0:?}")]
    UnknownRequest(RequestId),
    #[error("server answered request {id:?} with a {got} instead of a {expected}")]
    UnexpectedResponse {
        id: RequestId,
        expected: &'static str,
        got: &'static str,
    },
    #[error("no review is open")]
    NoOpenReview,
    #[error("a draft is already open")]
    DraftAlreadyOpen,
    #[error("no draft is open")]
    NoDraft,
    #[error("nothing was loaded under key {0:?}")]
    UnknownKey(Key),
    #[error("the daemon rejected the handshake: {0:?}")]
    Rejected(RpcError),
    /// An event at or before `last_seq`; the daemon only ever sends newer.
    #[error("event {seq} is not after the last seen {last_seq}")]
    StaleEvent { seq: Seq, last_seq: Seq },
}

/// What a `RequestId` is waiting for, so the reply can be routed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InFlight {
    Subscribe,
    ListReviews,
    ReviewSnapshot { review_id: ReviewId },
    Mutate { client_seq: ClientSeq },
}

/// Configuration fixed for the life of a `ClientCore`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub client_id: ClientId,
    pub client: BuildInfo,
    pub author: Author,
    /// Entropy for ids the core mints (comment ids). Hosts pass real random
    /// bits; tests pass a constant for reproducibility.
    pub id_seed: IdSeed,
}

/// The client state machine. See the crate docs.
#[derive(Debug)]
pub struct ClientCore {
    config: Config,
    connection: Connection,
    view: ViewModel,
    now: Millis,
    next_request: u64,
    next_client_seq: ClientSeq,
    in_flight: BTreeMap<RequestId, InFlight>,
    ids: ids::IdGen,
    /// `ReviewTargetsResolved` events held back while a draft is open (§5.4).
    deferred: Vec<EventBody>,
}

impl ClientCore {
    #[must_use]
    pub fn new(config: Config) -> Self {
        let ids = ids::IdGen::new(config.id_seed);
        Self {
            config,
            connection: Connection::Disconnected { last_seq: None },
            view: ViewModel::default(),
            now: 0,
            next_request: 1,
            next_client_seq: ClientSeq::new(1),
            in_flight: BTreeMap::new(),
            ids,
            deferred: Vec::new(),
        }
    }

    #[must_use]
    pub fn view(&self) -> &ViewModel {
        &self.view
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Apply one input. `Err` means nothing changed and nothing is to be done.
    pub fn handle(&mut self, input: Input) -> Result<Vec<Effect>, CoreError> {
        match input {
            Input::User(action) => self.user(action),
            Input::Server(msg) => self.server(msg),
            Input::Transport(ev) => Ok(self.transport(ev)),
            Input::Stored { key, .. } => Err(CoreError::UnknownKey(key)),
            Input::Tick(ms) => {
                self.now = self.now.max(ms);
                Ok(Vec::new())
            }
        }
    }

    fn wrong_state(&self, input: InputKind) -> CoreError {
        CoreError::WrongConnectionState {
            input,
            state: self.connection.kind(),
        }
    }

    fn user(&mut self, action: Action) -> Result<Vec<Effect>, CoreError> {
        match action {
            Action::Connect => match self.connection {
                Connection::Disconnected { last_seq } => {
                    self.connection = Connection::Connecting {
                        hello_sent: false,
                        last_seq,
                    };
                    self.view.connection = ConnectionView::Connecting;
                    Ok(vec![Effect::Connect, render(&[ViewSection::Connection])])
                }
                Connection::Connecting { .. } | Connection::Subscribed { .. } => {
                    Err(self.wrong_state(InputKind::User))
                }
            },
            Action::Disconnect => match self.connection {
                Connection::Disconnected { .. } => Err(self.wrong_state(InputKind::User)),
                Connection::Connecting { .. } | Connection::Subscribed { .. } => {
                    Ok(vec![Effect::Disconnect])
                }
            },
            Action::ListReviews { workspace_id } => {
                self.require_subscribed()?;
                Ok(vec![self.request(
                    Request::ListReviews { workspace_id },
                    InFlight::ListReviews,
                )])
            }
            Action::OpenReview { review_id } => {
                self.require_subscribed()?;
                Ok(vec![self.request(
                    Request::ReviewSnapshot { review_id },
                    InFlight::ReviewSnapshot { review_id },
                )])
            }
            Action::CloseReview => {
                if self.view.review.is_none() {
                    return Err(CoreError::NoOpenReview);
                }
                self.view.review = None;
                self.view.draft = None;
                self.view.pending_refresh = false;
                self.deferred.clear();
                Ok(vec![render(&[
                    ViewSection::Diff,
                    ViewSection::Threads,
                    ViewSection::Draft,
                ])])
            }
            Action::DraftOpened { anchor } => {
                if self.view.review.is_none() {
                    return Err(CoreError::NoOpenReview);
                }
                if self.view.draft.is_some() {
                    return Err(CoreError::DraftAlreadyOpen);
                }
                self.view.draft = Some(Draft { anchor });
                Ok(vec![render(&[ViewSection::Draft])])
            }
            Action::DraftSubmitted { body } => {
                let Some(review) = &self.view.review else {
                    return Err(CoreError::NoOpenReview);
                };
                let Some(draft) = &self.view.draft else {
                    return Err(CoreError::NoDraft);
                };
                self.require_subscribed()?;
                let review_id = review.snapshot.review.id;
                let anchor = draft.anchor.clone();
                let comment_id = self.ids.comment_id(self.now);
                let client_seq = self.next_client_seq;
                self.next_client_seq = client_seq.next();
                let send = self.request(
                    Request::Mutate {
                        client_seq,
                        mutation: Mutation::AddComment {
                            review_id,
                            comment_id,
                            kind: CommentKind::Note,
                            anchor,
                            body,
                        },
                    },
                    InFlight::Mutate { client_seq },
                );
                self.view.draft = None;
                let mut effects = vec![send];
                effects.push(self.drain_deferred());
                Ok(effects)
            }
            Action::DraftDiscarded => {
                if self.view.draft.is_none() {
                    return Err(CoreError::NoDraft);
                }
                self.view.draft = None;
                Ok(vec![self.drain_deferred()])
            }
        }
    }

    fn require_subscribed(&self) -> Result<(), CoreError> {
        match self.connection {
            Connection::Subscribed { .. } => Ok(()),
            Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                Err(self.wrong_state(InputKind::User))
            }
        }
    }

    fn request(&mut self, request: Request, waiting: InFlight) -> Effect {
        let id = RequestId::new(self.next_request);
        self.next_request += 1;
        self.in_flight.insert(id, waiting);
        Effect::Send(ClientMsg::Request { id, request })
    }

    fn transport(&mut self, ev: TransportEvent) -> Vec<Effect> {
        match ev {
            TransportEvent::Connected => match self.connection {
                Connection::Connecting {
                    hello_sent: false,
                    last_seq,
                } => {
                    self.connection = Connection::Connecting {
                        hello_sent: true,
                        last_seq,
                    };
                    vec![Effect::Send(ClientMsg::Hello {
                        client_id: self.config.client_id,
                        protocol: ProtocolVersion::CURRENT,
                        client: self.config.client.clone(),
                        author: self.config.author.clone(),
                    })]
                }
                // A duplicate or unsolicited "connected" changes nothing.
                Connection::Connecting {
                    hello_sent: true, ..
                }
                | Connection::Disconnected { .. }
                | Connection::Subscribed { .. } => Vec::new(),
            },
            TransportEvent::Disconnected => {
                let last_seq = self.connection.last_seq();
                let was_down = matches!(self.connection, Connection::Disconnected { .. });
                self.connection = Connection::Disconnected { last_seq };
                self.in_flight.clear();
                if was_down {
                    return Vec::new();
                }
                self.view.connection = ConnectionView::Disconnected;
                vec![render(&[ViewSection::Connection])]
            }
        }
    }

    fn server(&mut self, msg: ServerMsg) -> Result<Vec<Effect>, CoreError> {
        match msg {
            ServerMsg::Welcome { .. } => match self.connection {
                Connection::Connecting {
                    hello_sent: true,
                    last_seq,
                } => {
                    let since = match last_seq {
                        Some(seq) => Since::After { seq },
                        None => Since::Now,
                    };
                    Ok(vec![self.request(
                        Request::Subscribe {
                            scope: SubscribeScope::All,
                            since,
                        },
                        InFlight::Subscribe,
                    )])
                }
                Connection::Connecting {
                    hello_sent: false, ..
                }
                | Connection::Disconnected { .. }
                | Connection::Subscribed { .. } => Err(self.wrong_state(InputKind::Server)),
            },
            ServerMsg::Rejected { error } => match self.connection {
                Connection::Connecting { .. } => {
                    // The daemon closes the connection; we go down now so
                    // the view says why before the transport event lands.
                    self.connection = Connection::Disconnected {
                        last_seq: self.connection.last_seq(),
                    };
                    self.in_flight.clear();
                    self.view.connection = ConnectionView::Rejected {
                        error: error.clone(),
                    };
                    Err(CoreError::Rejected(error))
                }
                Connection::Disconnected { .. } | Connection::Subscribed { .. } => {
                    Err(self.wrong_state(InputKind::Server))
                }
            },
            ServerMsg::Response { id, response } => self.response(id, response),
            ServerMsg::StreamItem { id, item } => {
                self.require_in_flight(id)?;
                // No streaming request is issued yet (3.2 adds OpenReview).
                let got = match item {
                    StreamItem::ReviewSnapshot { .. } => "ReviewSnapshot",
                    StreamItem::TreeSnapshot { .. } => "TreeSnapshot",
                    StreamItem::Header { .. } => "Header",
                    StreamItem::Chunk { .. } => "Chunk",
                };
                Err(CoreError::UnexpectedResponse {
                    id,
                    expected: "Response",
                    got,
                })
            }
            ServerMsg::StreamEnd { id } => {
                self.require_in_flight(id)?;
                Err(CoreError::UnexpectedResponse {
                    id,
                    expected: "Response",
                    got: "StreamEnd",
                })
            }
            ServerMsg::Error { id, error } => {
                let Some(waiting) = self.in_flight.remove(&id) else {
                    return Err(CoreError::UnknownRequest(id));
                };
                self.view.last_error = Some(error);
                let mut sections = vec![ViewSection::Connection];
                if let InFlight::Subscribe = waiting {
                    // Subscription failed: stay connected but not subscribed;
                    // the host may retry with `Connect` after disconnecting.
                    self.view.connection = ConnectionView::Connecting;
                }
                if let InFlight::Mutate { .. } = waiting {
                    sections.push(ViewSection::Threads);
                }
                Ok(vec![render(&sections)])
            }
            ServerMsg::Event { event } => match self.connection {
                Connection::Subscribed { last_seq } if event.seq <= last_seq => {
                    Err(CoreError::StaleEvent {
                        seq: event.seq,
                        last_seq,
                    })
                }
                Connection::Subscribed { .. } => {
                    self.connection = Connection::Subscribed {
                        last_seq: event.seq,
                    };
                    Ok(self.apply_event(event.body))
                }
                Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                    Err(self.wrong_state(InputKind::Server))
                }
            },
            ServerMsg::TreeDelta { .. } => match self.connection {
                // Consumed by the explorer cache in 3.2; nothing to show yet.
                Connection::Subscribed { .. } => Ok(Vec::new()),
                Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                    Err(self.wrong_state(InputKind::Server))
                }
            },
        }
    }

    fn require_in_flight(&self, id: RequestId) -> Result<(), CoreError> {
        if self.in_flight.contains_key(&id) {
            Ok(())
        } else {
            Err(CoreError::UnknownRequest(id))
        }
    }

    fn response(&mut self, id: RequestId, response: Response) -> Result<Vec<Effect>, CoreError> {
        let Some(waiting) = self.in_flight.get(&id).cloned() else {
            return Err(CoreError::UnknownRequest(id));
        };
        let got = response_name(&response);
        let effects = match (waiting, response) {
            (InFlight::Subscribe, Response::Subscribed { seq }) => {
                let Connection::Connecting { .. } = self.connection else {
                    return Err(self.wrong_state(InputKind::Server));
                };
                let last_seq = self.connection.last_seq().map_or(seq, |s| s.max(seq));
                self.connection = Connection::Subscribed { last_seq };
                self.view.connection = ConnectionView::Subscribed;
                self.view.last_error = None;
                vec![render(&[ViewSection::Connection])]
            }
            (InFlight::ListReviews, Response::Reviews { reviews }) => {
                self.view.reviews = reviews;
                vec![render(&[ViewSection::ReviewList])]
            }
            (InFlight::ReviewSnapshot { review_id }, Response::ReviewSnapshot { snapshot }) => {
                if snapshot.review.id != review_id {
                    return Err(CoreError::UnexpectedResponse {
                        id,
                        expected: "ReviewSnapshot for the requested review",
                        got: "ReviewSnapshot for another review",
                    });
                }
                self.view.review = Some(OpenReview { snapshot });
                self.view.draft = None;
                self.view.pending_refresh = false;
                self.deferred.clear();
                vec![render(&[
                    ViewSection::Diff,
                    ViewSection::Threads,
                    ViewSection::Conversation,
                    ViewSection::Draft,
                ])]
            }
            (InFlight::Mutate { .. }, Response::Committed { event }) => {
                // The same event is also broadcast; applying here covers a
                // subscription that starts after the mutation.
                match self.connection {
                    Connection::Subscribed { last_seq } if last_seq >= event.seq => Vec::new(),
                    Connection::Subscribed { .. } => {
                        self.connection = Connection::Subscribed {
                            last_seq: event.seq,
                        };
                        self.apply_event(event.body)
                    }
                    Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                        return Err(self.wrong_state(InputKind::Server));
                    }
                }
            }
            (waiting, _) => {
                let expected = match waiting {
                    InFlight::Subscribe => "Subscribed",
                    InFlight::ListReviews => "Reviews",
                    InFlight::ReviewSnapshot { .. } => "ReviewSnapshot",
                    InFlight::Mutate { .. } => "Committed",
                };
                return Err(CoreError::UnexpectedResponse { id, expected, got });
            }
        };
        self.in_flight.remove(&id);
        Ok(effects)
    }

    /// Fold a committed event into the view. Events for reviews other than
    /// the open one only touch the review list.
    // One flat arm per `EventBody` variant, deliberately not split so the
    // exhaustive match stays readable when variants are added.
    #[allow(clippy::too_many_lines)]
    fn apply_event(&mut self, body: EventBody) -> Vec<Effect> {
        let mut sections = Vec::new();
        match body {
            EventBody::ReviewCreated { review } => {
                self.view.reviews.retain(|r| r.id != review.id);
                self.view.reviews.push(review);
                sections.push(ViewSection::ReviewList);
            }
            EventBody::ReviewUpdated {
                review_id,
                title,
                status,
            } => {
                if let Some(r) = self.view.reviews.iter_mut().find(|r| r.id == review_id) {
                    r.title.clone_from(&title);
                    r.status = status;
                    sections.push(ViewSection::ReviewList);
                }
                if let Some(open) = self.open_mut(review_id) {
                    open.snapshot.review.title = title;
                    open.snapshot.review.status = status;
                    sections.push(ViewSection::Conversation);
                }
            }
            EventBody::ReviewDeleted { review_id } => {
                let before = self.view.reviews.len();
                self.view.reviews.retain(|r| r.id != review_id);
                if self.view.reviews.len() != before {
                    sections.push(ViewSection::ReviewList);
                }
                if self.open_mut(review_id).is_some() {
                    self.view.review = None;
                    self.view.draft = None;
                    self.view.pending_refresh = false;
                    self.deferred.clear();
                    sections.extend([ViewSection::Diff, ViewSection::Threads, ViewSection::Draft]);
                }
            }
            EventBody::ReviewTargetsResolved { review_id, targets } => {
                if self.open_mut(review_id).is_some() {
                    if self.view.draft.is_some() {
                        self.deferred
                            .push(EventBody::ReviewTargetsResolved { review_id, targets });
                        if !self.view.pending_refresh {
                            self.view.pending_refresh = true;
                            sections.push(ViewSection::Draft);
                        }
                    } else if let Some(open) = self.open_mut(review_id) {
                        open.snapshot.resolved = Some(targets);
                        sections.push(ViewSection::Diff);
                    }
                }
            }
            EventBody::CommentCreated { comment } => {
                if let Some(open) = self.open_mut(comment.review_id) {
                    open.snapshot.comments.retain(|c| c.id != comment.id);
                    open.snapshot.comments.push(comment);
                    sections.push(ViewSection::Threads);
                }
            }
            EventBody::CommentEdited {
                review_id,
                comment_id,
                body,
            } => {
                if let Some(open) = self.open_mut(review_id)
                    && let Some(c) = open
                        .snapshot
                        .comments
                        .iter_mut()
                        .find(|c| c.id == comment_id)
                {
                    c.body = body;
                    sections.push(ViewSection::Threads);
                }
            }
            EventBody::CommentDeleted {
                review_id,
                comment_id,
            } => {
                if let Some(open) = self.open_mut(review_id) {
                    open.snapshot.comments.retain(|c| c.id != comment_id);
                    sections.push(ViewSection::Threads);
                }
            }
            EventBody::CommentReanchored {
                review_id,
                comment_id,
                anchor,
                state,
            } => {
                if let Some(open) = self.open_mut(review_id)
                    && let Some(c) = open
                        .snapshot
                        .comments
                        .iter_mut()
                        .find(|c| c.id == comment_id)
                {
                    c.anchor = anchor;
                    c.state = state;
                    sections.push(ViewSection::Threads);
                }
            }
            EventBody::ThreadResolved {
                review_id,
                thread_id,
                ..
            }
            | EventBody::ThreadUnresolved {
                review_id,
                thread_id,
                ..
            } => {
                let _ = (review_id, thread_id);
                if self.open_mut(review_id).is_some() {
                    sections.push(ViewSection::Threads);
                }
            }
            EventBody::FileViewed { review_id, .. } | EventBody::FileUnviewed { review_id, .. } => {
                if self.open_mut(review_id).is_some() {
                    sections.push(ViewSection::Progress);
                }
            }
            EventBody::ReviewRequested { review_id, .. }
            | EventBody::SuggestionApplied { review_id, .. } => {
                if self.open_mut(review_id).is_some() {
                    sections.push(ViewSection::Conversation);
                }
            }
            EventBody::WorkspaceCreated { .. }
            | EventBody::WorkspaceUpdated { .. }
            | EventBody::RepoAttached { .. }
            | EventBody::RepoDetached { .. } => {}
        }
        if sections.is_empty() {
            Vec::new()
        } else {
            vec![render(&sections)]
        }
    }

    fn open_mut(&mut self, review_id: ReviewId) -> Option<&mut OpenReview> {
        self.view
            .review
            .as_mut()
            .filter(|r| r.snapshot.review.id == review_id)
    }

    /// Apply refreshes held back during a draft; always renders `Draft`
    /// (the draft just closed) plus whatever the refreshes touched.
    fn drain_deferred(&mut self) -> Effect {
        let deferred = std::mem::take(&mut self.deferred);
        self.view.pending_refresh = false;
        let mut sections = vec![ViewSection::Draft];
        for body in deferred {
            for effect in self.apply_event(body) {
                if let Effect::Render(delta) = effect {
                    sections.extend(delta.sections);
                }
            }
        }
        render(&sections)
    }
}

fn render(sections: &[ViewSection]) -> Effect {
    Effect::Render(ViewDelta::new(sections))
}

fn response_name(r: &Response) -> &'static str {
    match r {
        Response::Workspaces { .. } => "Workspaces",
        Response::Reviews { .. } => "Reviews",
        Response::Review { .. } => "Review",
        Response::ReviewSnapshot { .. } => "ReviewSnapshot",
        Response::Files { .. } => "Files",
        Response::Resolved { .. } => "Resolved",
        Response::Commits { .. } => "Commits",
        Response::TreeSnapshot { .. } => "TreeSnapshot",
        Response::RenderChunk { .. } => "RenderChunk",
        Response::Subscribed { .. } => "Subscribed",
        Response::Unsubscribed => "Unsubscribed",
        Response::Committed { .. } => "Committed",
        Response::ShuttingDown => "ShuttingDown",
    }
}
