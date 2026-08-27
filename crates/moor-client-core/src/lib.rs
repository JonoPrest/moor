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
//! - `Effect::Render` names only the [`ViewSection`]s that changed, and
//!   there is at most one per input, after every other effect.
//! - Mutations are optimistic (§5.2): the view shows `committed + pending`;
//!   a foreign event re-applies the pending list on top; the daemon's own
//!   echo (matched by `client_id`/`client_seq`) retires a pending entry.
//! - Content (trees, render headers, chunks) is fetched through one path in
//!   `content.rs`: memory → disk (`Load`) → daemon (`Send`).

#![deny(clippy::wildcard_enum_match_arm)]

mod cache;
mod connection;
mod content;
mod diff;
mod events;
mod explorer;
mod focus;
mod ids;
mod keymap;
mod patch;
mod view;

use std::collections::BTreeMap;

use moor_protocol::{
    Anchor, Author, BuildInfo, ClientId, ClientMsg, ClientSeq, CommentId, CommentKind, Event,
    EventBody, Mutation, NonEmpty, ProtocolVersion, RenderTarget, RepoId, RepoPath, Request,
    RequestId, Response, ReviewId, ReviewSnapshot, ReviewTarget, RpcError, Seq, ServerMsg, Since,
    StreamItem, SubscribeScope, ThreadId, Timestamp, ViewSection, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use strum::EnumDiscriminants;

pub use cache::{Bytes, CacheKey, CacheValue, ContentCache, Evicted, RenderKey};
pub use connection::{Connection, ConnectionKind};
pub use content::{CacheConfig, DiskTier, DiskTierKind, FileRef, PREFETCH_RADIUS};
pub use diff::{
    CommitStepper, DiffRow, DiffView, PendingIds, StepperCommit, ThreadPlace, ThreadPlaceKind,
    ThreadView, conversation, threads,
};
pub use events::{
    EventMeta, MutationError, MutationErrorKind, apply_body, local_event, thread_id_of,
};
pub use explorer::{
    MAX_HITS, Progress, SearchHit, SearchView, TreeNode, TreeNodeKind, TreeView, ViewedState,
    viewed_state,
};
pub use focus::{Focus, FocusKind, NoTarget, PAGE_ROWS, clamp as clamp_focus, visible_nodes};
pub use ids::IdSeed;
pub use keymap::{
    Binding, Command, Conflict, Context, HelpEntry, HelpGroup, HelpView, Hint, KeyChord, KeyCode,
    KeyCodeKind, KeyParseError, KeySeq, Keymap, Lookup, Modifiers, NamedKey, Override, Overrides,
    label,
};
pub use patch::{ViewPatch, ViewPatchKind};
pub use view::{
    ConnectionView, ConnectionViewKind, Draft, Layout, OpenFile, OpenReview, PendingEvent,
    ViewDelta, ViewModel, ViewPrefs,
};

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
    /// A key press, resolved against the keymap and the focus (§6.4).
    Key(KeyChord),
}

/// After this long without a further chord, a pending sequence (`g` of
/// `g g`) is dropped.
pub const SEQ_TIMEOUT_MS: Millis = 800;

/// What the transport layer observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportEvent {
    /// The connection the core asked for in `Effect::Connect` is up.
    Connected,
    /// The connection dropped (or the dial failed).
    Disconnected,
}

/// A user intent, already resolved from keys or clicks by the host. Crosses
/// the host ↔ UI boundary (Tauri `dispatch`), hence serde.
#[derive(Debug, Clone, PartialEq, Eq, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(name(ActionKind), derive(Hash, PartialOrd, Ord, strum::EnumIter))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Action {
    Connect,
    Disconnect,
    /// Refresh the workspace list (and, on its answer, every review list).
    /// Done automatically on subscribe.
    ListWorkspaces,
    ListReviews {
        workspace_id: WorkspaceId,
    },
    /// Create a review; the id is minted by the core. Not optimistic: the
    /// `ReviewCreated` event adds it to the list.
    CreateReview {
        workspace_id: WorkspaceId,
        title: String,
        targets: NonEmpty<ReviewTarget>,
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
    /// The user started writing a reply in `thread_id`.
    ReplyOpened {
        thread_id: ThreadId,
    },
    SetFocus {
        focus: Focus,
    },
    ToggleHelp,
    /// Reply in an existing thread of the open review.
    Reply {
        thread_id: ThreadId,
        body: String,
    },
    EditComment {
        comment_id: CommentId,
        body: String,
    },
    DeleteComment {
        comment_id: CommentId,
    },
    ResolveThread {
        thread_id: ThreadId,
    },
    UnresolveThread {
        thread_id: ThreadId,
    },
    /// Write a suggestion comment's patch to the working tree. Not
    /// optimistic: the daemon reports the result as `SuggestionApplied`.
    ApplySuggestion {
        comment_id: CommentId,
    },
    /// The host shows rows `first_row..=last_row` of `file`. Opens the file
    /// if it was not; drives chunk (pre)fetching.
    Viewport {
        file: FileRef,
        first_row: u32,
        last_row: u32,
    },
    CloseFile,
    /// Expand or collapse a directory of the explorer (`None` = repo root).
    ToggleDir {
        repo_id: RepoId,
        path: Option<RepoPath>,
    },
    /// Open (`Some`) or close (`None`) the fuzzy file search.
    FileSearch {
        query: Option<String>,
    },
    SetLayout {
        layout: Layout,
    },
    /// Change the render options; re-keys every render, so the open
    /// review's headers are fetched again.
    SetRenderOpts {
        ignore_whitespace: bool,
        context_lines: u32,
    },
    MarkViewed {
        file: FileRef,
    },
    UnmarkViewed {
        file: FileRef,
    },
    /// Fetch the commits of one repo of the open review for the stepper.
    ListCommits {
        repo_id: RepoId,
    },
    /// Move the stepper cursor; `None` shows the whole range.
    StepCommit {
        selected: Option<usize>,
    },
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
    /// Delete a key from the host store (disk-tier trimming).
    Remove {
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
    #[error("{0:?} is not a file of the open review")]
    UnknownFile(FileRef),
    #[error("no file is open")]
    NoOpenFile,
    #[error("a draft is already open")]
    DraftAlreadyOpen,
    #[error("no draft is open")]
    NoDraft,
    /// The mutation would be rejected by the daemon; nothing was sent.
    #[error(transparent)]
    Mutation(#[from] MutationError),
    /// Only humans mark files viewed (`Mutation::MarkViewed` is
    /// `Forbidden` for agents).
    #[error("only a human viewer can mark files viewed")]
    NotHuman,
    #[error("no commit list to step through")]
    NoStepper,
    #[error("no thread {0}")]
    UnknownThread(ThreadId),
    #[error("{0:?} indexes past the end of its list")]
    FocusOutOfRange(Focus),
    #[error("{0} is not bound in this context")]
    UnboundKey(String),
    #[error(transparent)]
    NoTarget(#[from] NoTarget),
    #[error("commit index {0} is out of range")]
    CommitOutOfRange(usize),
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
pub(crate) enum InFlight {
    Subscribe,
    ListWorkspaces,
    ListReviews {
        workspace_id: WorkspaceId,
    },
    /// Streamed open (local daemon): snapshot, trees, headers, first chunks.
    OpenReview {
        review_id: ReviewId,
    },
    /// Piecewise open (disk tier on): snapshot only, the rest by key.
    ReviewSnapshot {
        review_id: ReviewId,
    },
    ListFiles {
        review_id: ReviewId,
    },
    ListCommits {
        repo_id: RepoId,
    },
    TreeSnapshot {
        root: moor_protocol::TreeOid,
    },
    FileRender {
        render: RenderKey,
        stop_after: moor_protocol::ChunkIndex,
    },
    RenderChunk {
        key: CacheKey,
    },
    Mutate {
        client_seq: ClientSeq,
    },
}

impl InFlight {
    /// Whether this request counts against `CacheConfig::max_in_flight`.
    fn is_content(&self) -> bool {
        match self {
            InFlight::TreeSnapshot { .. }
            | InFlight::FileRender { .. }
            | InFlight::RenderChunk { .. } => true,
            InFlight::Subscribe
            | InFlight::ListWorkspaces
            | InFlight::ListReviews { .. }
            | InFlight::OpenReview { .. }
            | InFlight::ReviewSnapshot { .. }
            | InFlight::ListFiles { .. }
            | InFlight::ListCommits { .. }
            | InFlight::Mutate { .. } => false,
        }
    }

    /// The cache key this request fills, if it is a single-key fetch.
    fn key(&self) -> Option<CacheKey> {
        match self {
            InFlight::TreeSnapshot { root } => Some(CacheKey::Tree { root: *root }),
            InFlight::FileRender { render, .. } => Some(CacheKey::Header {
                render: render.clone(),
            }),
            InFlight::RenderChunk { key } => Some(key.clone()),
            InFlight::Subscribe
            | InFlight::ListWorkspaces
            | InFlight::ListReviews { .. }
            | InFlight::OpenReview { .. }
            | InFlight::ReviewSnapshot { .. }
            | InFlight::ListFiles { .. }
            | InFlight::ListCommits { .. }
            | InFlight::Mutate { .. } => None,
        }
    }
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
    pub cache: CacheConfig,
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
    deferred: Vec<Event>,
    content: content::Content,
    /// The open review as the daemon last confirmed it; `view.review.snapshot`
    /// is this plus `pending`.
    committed: Option<ReviewSnapshot>,
    /// Mutations sent and not yet echoed by the daemon, in send order.
    pending: Vec<Pending>,
    explorer: explorer::ExplorerState,
    stepper: Option<CommitStepper>,
    /// `Effect::Load` for the prefs and keymap issued; answered or not.
    prefs_loaded: bool,
    keymap: Keymap,
    /// Chords of a sequence in progress, and when it started.
    chords: Vec<KeyChord>,
    chord_started: Millis,
    help_open: bool,
    /// Focus to return to when the composer or help closes.
    focus_return: Option<Focus>,
}

/// A mutation applied locally and awaiting the daemon's echo.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    client_seq: ClientSeq,
    /// When it was applied locally; the optimistic event's timestamp.
    ts: Timestamp,
    mutation: Mutation,
    body: EventBody,
    /// `false` after a disconnect until it is re-sent on resubscribe.
    sent: bool,
}

impl ClientCore {
    #[must_use]
    pub fn new(config: Config) -> Self {
        let ids = ids::IdGen::new(config.id_seed);
        let content = content::Content::new(config.cache);
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
            content,
            committed: None,
            pending: Vec::new(),
            explorer: explorer::ExplorerState::default(),
            stepper: None,
            prefs_loaded: false,
            keymap: Keymap::default_table(),
            chords: Vec::new(),
            chord_started: 0,
            help_open: false,
            focus_return: None,
        }
    }

    #[must_use]
    pub fn client_id(&self) -> ClientId {
        self.config.client_id
    }

    #[must_use]
    pub fn author(&self) -> &Author {
        &self.config.author
    }

    #[must_use]
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Chords of the sequence in progress (`g` while waiting for `g g`).
    #[must_use]
    pub fn pending_chords(&self) -> &[KeyChord] {
        &self.chords
    }

    /// Mutations awaiting the daemon, oldest first.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
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
        let mut effects = match input {
            Input::User(action) => self.user(action)?,
            Input::Server(msg) => self.server(msg)?,
            Input::Transport(ev) => self.transport(ev),
            Input::Stored { key, value } if key == ViewPrefs::KEY => self.prefs_stored(value),
            Input::Stored { key, value } if key == Keymap::KEY => self.keymap_stored(value),
            Input::Stored { key, value } => self.stored(key, value)?,
            Input::Tick(ms) => {
                self.now = self.now.max(ms);
                if !self.chords.is_empty()
                    && self.now.saturating_sub(self.chord_started) >= SEQ_TIMEOUT_MS
                {
                    self.chords.clear();
                }
                Vec::new()
            }
            Input::Key(chord) => self.key(chord)?,
        };
        // One `Render` per input: the union of every section touched, in
        // first-touched order, after every other effect.
        let mut sections: Vec<ViewSection> = Vec::new();
        effects.retain(|e| match e {
            Effect::Render(delta) => {
                sections.extend(delta.sections.iter().copied());
                false
            }
            Effect::Connect
            | Effect::Disconnect
            | Effect::Send(_)
            | Effect::Persist { .. }
            | Effect::Load { .. }
            | Effect::Remove { .. } => true,
        });
        sections.extend(self.derive());
        if !sections.is_empty() {
            effects.push(render(&sections));
        }
        Ok(effects)
    }

    /// Recompute the parts of the view that are functions of core state
    /// (explorer, progress) and report which changed.
    // One block per derived panel; splitting would hide what is derived.
    #[allow(clippy::too_many_lines)]
    fn derive(&mut self) -> Vec<ViewSection> {
        let mut sections = Vec::new();
        let (tree, progress) = match (&self.view.review, &self.committed) {
            (Some(open), Some(_)) => {
                let heads: Vec<moor_protocol::TreeOid> = open
                    .snapshot
                    .resolved
                    .as_ref()
                    .map(|r| r.iter().map(|t| t.head.tree).collect())
                    .unwrap_or_default();
                let trees: Vec<&moor_protocol::TreeSnapshot> = heads
                    .iter()
                    .filter_map(|root| {
                        match self.content.cache.peek(&CacheKey::Tree { root: *root }) {
                            Some(CacheValue::Tree { snapshot }) => Some(snapshot),
                            Some(CacheValue::Header { .. } | CacheValue::Chunk { .. }) | None => {
                                None
                            }
                        }
                    })
                    .collect();
                let open_file = open.open_file.as_ref().map(|f| FileRef {
                    repo_id: f.render.repo_id,
                    path: f.render.path.clone(),
                });
                let repo_names: Vec<(RepoId, String)> = self
                    .view
                    .workspaces
                    .iter()
                    .flat_map(|w| w.repos.iter().map(|r| (r.id, r.display_name.clone())))
                    .collect();
                let inputs = explorer::ExplorerInputs {
                    snapshot: &open.snapshot,
                    repo_names: &repo_names,
                    trees,
                    files: &open.files,
                    open_file: open_file.as_ref(),
                    viewer: &self.config.author,
                    state: &self.explorer,
                };
                (
                    explorer::build(&inputs),
                    explorer::progress(&open.snapshot, &self.config.author, &open.files),
                )
            }
            (None, _) | (Some(_), None) => (TreeView::default(), Progress::default()),
        };
        if tree != self.view.tree {
            self.view.tree = tree;
            sections.push(ViewSection::Tree);
        }
        if progress != self.view.progress {
            self.view.progress = progress;
            sections.push(ViewSection::Progress);
        }
        let (diff, threads) = match &self.view.review {
            Some(open) => {
                let pending = self.pending_ids();
                let threads = diff::threads(&open.snapshot, &pending);
                let diff = open.open_file.as_ref().and_then(|f| {
                    diff::diff_view(
                        &self.content.cache,
                        &open.snapshot,
                        &self.config.author,
                        &f.render,
                        f.first_row,
                        f.last_row,
                    )
                });
                (diff, threads)
            }
            None => (None, Vec::new()),
        };
        if diff != self.view.diff {
            self.view.diff = diff;
            sections.push(ViewSection::Diff);
        }
        if threads != self.view.threads {
            let conversation = diff::conversation(&threads);
            if conversation != self.view.conversation {
                self.view.conversation = conversation;
                sections.push(ViewSection::Conversation);
            }
            self.view.threads = threads;
            sections.push(ViewSection::Threads);
        }
        if self.stepper != self.view.stepper {
            self.view.stepper.clone_from(&self.stepper);
            sections.push(ViewSection::CommitStepper);
        }
        let focus = focus::clamp(&self.view, self.view.focus);
        if focus != self.view.focus {
            self.view.focus = focus;
            sections.push(ViewSection::Focus);
        }
        let hints = self.keymap.hints(focus.context());
        if hints != self.view.hints {
            self.view.hints = hints;
            sections.push(ViewSection::Hints);
        }
        let help = self.help_open.then(|| self.keymap.help(focus.context()));
        if help != self.view.help {
            self.view.help = help;
            sections.push(ViewSection::Help);
        }
        sections
    }

    /// Comment and thread ids the pending mutations touch.
    fn pending_ids(&self) -> PendingIds {
        let mut ids = PendingIds::default();
        for p in &self.pending {
            match &p.body {
                EventBody::CommentCreated { comment } => ids.comments.push(comment.id),
                EventBody::CommentEdited { comment_id, .. }
                | EventBody::CommentDeleted { comment_id, .. }
                | EventBody::CommentReanchored { comment_id, .. } => ids.comments.push(*comment_id),
                EventBody::ThreadResolved { thread_id, .. }
                | EventBody::ThreadUnresolved { thread_id, .. } => ids.threads.push(*thread_id),
                EventBody::ReviewCreated { .. }
                | EventBody::ReviewUpdated { .. }
                | EventBody::ReviewDeleted { .. }
                | EventBody::ReviewTargetsResolved { .. }
                | EventBody::FileViewed { .. }
                | EventBody::FileUnviewed { .. }
                | EventBody::ReviewRequested { .. }
                | EventBody::SuggestionApplied { .. }
                | EventBody::WorkspaceCreated { .. }
                | EventBody::WorkspaceUpdated { .. }
                | EventBody::RepoAttached { .. }
                | EventBody::RepoDetached { .. } => {}
            }
        }
        ids
    }

    /// Resolve a key press. The chord buffer is the one piece of state a
    /// rejected input may change: an unbound or unresolvable sequence
    /// clears it, so the next key starts fresh.
    fn key(&mut self, chord: KeyChord) -> Result<Vec<Effect>, CoreError> {
        let context = self.view.focus.context();
        let mut pressed = self.chords.clone();
        pressed.push(chord);
        match self.keymap.lookup(context, &pressed) {
            Lookup::Prefix => {
                if self.chords.is_empty() {
                    self.chord_started = self.now;
                }
                self.chords = pressed;
                Ok(Vec::new())
            }
            Lookup::Command(command) => {
                self.chords.clear();
                let action = focus::resolve(self, command)?;
                self.user(action)
            }
            Lookup::None => {
                self.chords.clear();
                if context == Context::Composer {
                    // Text for the host's editor, not a command.
                    Ok(Vec::new())
                } else {
                    let seq =
                        KeySeq::new(pressed).map_or_else(|_| chord.to_string(), |s| s.to_string());
                    Err(CoreError::UnboundKey(seq))
                }
            }
        }
    }

    /// The stored keymap overrides arrived (or were absent / unreadable:
    /// the defaults stay).
    fn keymap_stored(&mut self, value: Option<Vec<u8>>) -> Vec<Effect> {
        let Some(overrides) = value.and_then(|b| serde_json::from_slice::<Overrides>(&b).ok())
        else {
            return Vec::new();
        };
        self.keymap = Keymap::with_overrides(&overrides);
        // Hints and help are derived after this returns.
        Vec::new()
    }

    /// The stored preferences arrived (or were absent).
    fn prefs_stored(&mut self, value: Option<Vec<u8>>) -> Vec<Effect> {
        self.prefs_loaded = true;
        let Some(prefs) = value.and_then(|b| serde_json::from_slice::<ViewPrefs>(&b).ok()) else {
            return Vec::new();
        };
        if prefs == self.view.prefs {
            return Vec::new();
        }
        self.apply_prefs(prefs, false)
    }

    /// Install `prefs`; persist when `save`. A render-option change re-keys
    /// every render, so the open review's file list is fetched again.
    fn apply_prefs(&mut self, prefs: ViewPrefs, save: bool) -> Vec<Effect> {
        let before = self.view.prefs;
        self.view.prefs = prefs;
        let mut effects = Vec::new();
        if save {
            effects.push(Effect::Persist {
                key: ViewPrefs::KEY.to_owned(),
                value: serde_json::to_vec(&prefs).unwrap_or_default(),
            });
        }
        let sections = vec![ViewSection::Diff];
        if prefs.render_opts() != before.render_opts() {
            self.content.config.render_opts = prefs.render_opts();
            if let Some(open) = &mut self.view.review {
                let review_id = open.snapshot.review.id;
                open.files.clear();
                open.open_file = None;
                if let Connection::Subscribed { .. } = self.connection {
                    effects.push(self.request(
                        Request::ListFiles { review_id },
                        InFlight::ListFiles { review_id },
                    ));
                }
            }
        }
        effects.push(render(&sections));
        effects
    }

    fn wrong_state(&self, input: InputKind) -> CoreError {
        CoreError::WrongConnectionState {
            input,
            state: self.connection.kind(),
        }
    }

    // One arm per variant; splitting would hide the exhaustive match.
    #[allow(clippy::too_many_lines)]
    fn user(&mut self, action: Action) -> Result<Vec<Effect>, CoreError> {
        match action {
            Action::Connect => match self.connection {
                Connection::Disconnected { last_seq } => {
                    self.connection = Connection::Connecting {
                        hello_sent: false,
                        last_seq,
                    };
                    self.view.connection = ConnectionView::Connecting;
                    let mut effects = vec![Effect::Connect, render(&[ViewSection::Connection])];
                    if !self.prefs_loaded {
                        // Once per core: the host answers with `Input::Stored`.
                        self.prefs_loaded = true;
                        effects.push(Effect::Load {
                            key: ViewPrefs::KEY.to_owned(),
                        });
                        effects.push(Effect::Load {
                            key: Keymap::KEY.to_owned(),
                        });
                    }
                    Ok(effects)
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
            Action::ListWorkspaces => {
                self.require_subscribed()?;
                Ok(vec![self.request(
                    Request::ListWorkspaces,
                    InFlight::ListWorkspaces,
                )])
            }
            Action::ListReviews { workspace_id } => {
                self.require_subscribed()?;
                Ok(vec![self.request(
                    Request::ListReviews { workspace_id },
                    InFlight::ListReviews { workspace_id },
                )])
            }
            Action::CreateReview {
                workspace_id,
                title,
                targets,
            } => {
                self.require_subscribed()?;
                let review_id = self.ids.review_id(self.now);
                let client_seq = self.next_client_seq;
                self.next_client_seq = client_seq.next();
                Ok(vec![self.request(
                    Request::Mutate {
                        client_seq,
                        mutation: Mutation::CreateReview {
                            review_id,
                            workspace_id,
                            title,
                            targets,
                        },
                    },
                    InFlight::Mutate { client_seq },
                )])
            }
            Action::OpenReview { review_id } => {
                self.require_subscribed()?;
                let opts = self.content.config.render_opts;
                let (request, waiting) = match self.content.config.disk {
                    DiskTier::Disabled => (
                        Request::OpenReview { review_id, opts },
                        InFlight::OpenReview { review_id },
                    ),
                    DiskTier::Enabled { .. } => (
                        Request::ReviewSnapshot { review_id },
                        InFlight::ReviewSnapshot { review_id },
                    ),
                };
                Ok(vec![self.request(request, waiting)])
            }
            Action::CloseReview => {
                if self.view.review.is_none() {
                    return Err(CoreError::NoOpenReview);
                }
                let mut effects = Vec::new();
                self.close_review(&mut effects);
                // Diff, threads, tree, progress: derived after this returns.
                effects.push(render(&[ViewSection::Draft]));
                Ok(effects)
            }
            Action::Viewport {
                file,
                first_row,
                last_row,
            } => self.viewport(file, first_row, last_row),
            Action::CloseFile => self.close_file(),
            Action::ToggleDir { repo_id, path } => {
                if self.view.review.is_none() {
                    return Err(CoreError::NoOpenReview);
                }
                let key = (repo_id, path);
                if !self.explorer.expanded.remove(&key) {
                    self.explorer.expanded.insert(key);
                }
                // The tree itself is derived after this returns.
                Ok(Vec::new())
            }
            Action::FileSearch { query } => {
                if self.view.review.is_none() {
                    return Err(CoreError::NoOpenReview);
                }
                self.explorer.search = query;
                Ok(Vec::new())
            }
            Action::SetLayout { layout } => {
                let prefs = ViewPrefs {
                    layout,
                    ..self.view.prefs
                };
                Ok(self.apply_prefs(prefs, true))
            }
            Action::SetRenderOpts {
                ignore_whitespace,
                context_lines,
            } => {
                let prefs = ViewPrefs {
                    ignore_whitespace,
                    context_lines,
                    ..self.view.prefs
                };
                Ok(self.apply_prefs(prefs, true))
            }
            Action::MarkViewed { file } => self.mark_viewed(file, true),
            Action::UnmarkViewed { file } => self.mark_viewed(file, false),
            Action::ListCommits { repo_id } => {
                let review_id = self.open_review_id()?;
                self.require_subscribed()?;
                Ok(vec![self.request(
                    Request::ListCommits { review_id, repo_id },
                    InFlight::ListCommits { repo_id },
                )])
            }
            Action::StepCommit { selected } => {
                let Some(stepper) = &mut self.stepper else {
                    return Err(CoreError::NoStepper);
                };
                if let Some(i) = selected
                    && i >= stepper.commits.len()
                {
                    return Err(CoreError::CommitOutOfRange(i));
                }
                stepper.selected = selected;
                Ok(Vec::new())
            }
            Action::DraftOpened { anchor } => {
                if self.view.review.is_none() {
                    return Err(CoreError::NoOpenReview);
                }
                if self.view.draft.is_some() {
                    return Err(CoreError::DraftAlreadyOpen);
                }
                self.view.draft = Some(Draft {
                    anchor,
                    reply_to: None,
                });
                self.enter(Focus::Composer);
                Ok(vec![render(&[ViewSection::Draft, ViewSection::Focus])])
            }
            Action::ReplyOpened { thread_id } => {
                let Some(open) = &self.view.review else {
                    return Err(CoreError::NoOpenReview);
                };
                if self.view.draft.is_some() {
                    return Err(CoreError::DraftAlreadyOpen);
                }
                let root = open
                    .snapshot
                    .threads
                    .iter()
                    .find(|t| t.id == thread_id)
                    .and_then(|t| open.snapshot.comments.iter().find(|c| c.id == t.root))
                    .ok_or(CoreError::UnknownThread(thread_id))?;
                self.view.draft = Some(Draft {
                    anchor: root.anchor.clone(),
                    reply_to: Some(thread_id),
                });
                self.enter(Focus::Composer);
                Ok(vec![render(&[ViewSection::Draft, ViewSection::Focus])])
            }
            Action::SetFocus { focus } => {
                if focus::clamp(&self.view, focus) != focus {
                    return Err(CoreError::FocusOutOfRange(focus));
                }
                self.view.focus = focus;
                let mut effects = Vec::new();
                // A focused row outside the viewport scrolls the viewport.
                if let Focus::Diff { row } = focus
                    && let Some(open) = &self.view.review
                    && let Some(f) = &open.open_file
                    && (row < f.first_row || row > f.last_row)
                {
                    let file = FileRef {
                        repo_id: f.render.repo_id,
                        path: f.render.path.clone(),
                    };
                    let first_row = row.saturating_sub(PAGE_ROWS / 2);
                    effects.extend(self.viewport(file, first_row, first_row + PAGE_ROWS - 1)?);
                }
                effects.push(render(&[ViewSection::Focus]));
                Ok(effects)
            }
            Action::ToggleHelp => {
                if self.help_open {
                    self.help_open = false;
                    self.leave();
                } else {
                    self.help_open = true;
                    self.enter(Focus::Help);
                }
                Ok(vec![render(&[ViewSection::Help, ViewSection::Focus])])
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
                let reply_to = draft.reply_to;
                let comment_id = self.ids.comment_id(self.now);
                let mutation = match reply_to {
                    Some(thread_id) => Mutation::Reply {
                        review_id,
                        thread_id,
                        comment_id,
                        kind: CommentKind::Note,
                        body,
                    },
                    None => Mutation::AddComment {
                        review_id,
                        comment_id,
                        kind: CommentKind::Note,
                        anchor,
                        body,
                    },
                };
                let mut effects = self.mutate(mutation)?;
                self.view.draft = None;
                self.leave();
                effects.extend(self.drain_deferred());
                Ok(effects)
            }
            Action::Reply { thread_id, body } => {
                let review_id = self.open_review_id()?;
                let comment_id = self.ids.comment_id(self.now);
                self.mutate(Mutation::Reply {
                    review_id,
                    thread_id,
                    comment_id,
                    kind: CommentKind::Note,
                    body,
                })
            }
            Action::EditComment { comment_id, body } => {
                let review_id = self.open_review_id()?;
                self.mutate(Mutation::EditComment {
                    review_id,
                    comment_id,
                    body,
                })
            }
            Action::DeleteComment { comment_id } => {
                let review_id = self.open_review_id()?;
                self.mutate(Mutation::DeleteComment {
                    review_id,
                    comment_id,
                })
            }
            Action::ResolveThread { thread_id } => {
                let review_id = self.open_review_id()?;
                self.mutate(Mutation::ResolveThread {
                    review_id,
                    thread_id,
                })
            }
            Action::UnresolveThread { thread_id } => {
                let review_id = self.open_review_id()?;
                self.mutate(Mutation::UnresolveThread {
                    review_id,
                    thread_id,
                })
            }
            Action::ApplySuggestion { comment_id } => {
                let review_id = self.open_review_id()?;
                self.require_subscribed()?;
                let Some(open) = &self.view.review else {
                    return Err(CoreError::NoOpenReview);
                };
                let is_suggestion = open.snapshot.comments.iter().any(|c| {
                    c.id == comment_id && matches!(c.kind, CommentKind::Suggestion { .. })
                });
                if !is_suggestion {
                    return Err(CoreError::Mutation(MutationError::UnknownComment(
                        comment_id,
                    )));
                }
                let client_seq = self.next_client_seq;
                self.next_client_seq = client_seq.next();
                Ok(vec![self.request(
                    Request::Mutate {
                        client_seq,
                        mutation: Mutation::ApplySuggestion {
                            review_id,
                            comment_id,
                        },
                    },
                    InFlight::Mutate { client_seq },
                )])
            }
            Action::DraftDiscarded => {
                if self.view.draft.is_none() {
                    return Err(CoreError::NoDraft);
                }
                self.view.draft = None;
                self.leave();
                Ok(self.drain_deferred())
            }
        }
    }

    /// Move focus into a modal panel (composer, help), remembering where
    /// to come back to.
    fn enter(&mut self, focus: Focus) {
        if self.focus_return.is_none() {
            self.focus_return = Some(self.view.focus);
        }
        self.view.focus = focus;
    }

    /// Leave the modal panel; `derive` clamps the restored focus.
    fn leave(&mut self) {
        if let Some(f) = self.focus_return.take() {
            self.view.focus = f;
        }
    }

    fn open_review_id(&self) -> Result<ReviewId, CoreError> {
        self.view
            .review
            .as_ref()
            .map(|r| r.snapshot.review.id)
            .ok_or(CoreError::NoOpenReview)
    }

    /// What this client's own events carry, at the core's current time.
    fn meta_now(&self) -> EventMeta {
        EventMeta {
            author: self.config.author.clone(),
            ts: Timestamp::from_millis(i64::try_from(self.now).unwrap_or(i64::MAX)),
        }
    }

    /// Mark or unmark `file` viewed at its current head blob. The event body
    /// needs the blob, which only the file list knows, so it is built here
    /// rather than by `local_event`.
    fn mark_viewed(&mut self, file: FileRef, viewed: bool) -> Result<Vec<Effect>, CoreError> {
        self.require_subscribed()?;
        let Some(open) = &self.view.review else {
            return Err(CoreError::NoOpenReview);
        };
        let review_id = open.snapshot.review.id;
        let Some(human) = self.config.author.as_human() else {
            return Err(CoreError::NotHuman);
        };
        let Some(render) = open
            .files
            .iter()
            .find(|k| k.repo_id == file.repo_id && k.path == file.path)
        else {
            return Err(CoreError::UnknownFile(file));
        };
        let blob_oid = match &render.target {
            RenderTarget::Diff { change } => change.new_blob(),
            RenderTarget::Blob { oid } => Some(*oid),
        };
        let (mutation, body) = if viewed {
            (
                Mutation::MarkViewed {
                    review_id,
                    repo_id: file.repo_id,
                    path: file.path.clone(),
                },
                EventBody::FileViewed {
                    review_id,
                    repo_id: file.repo_id,
                    path: file.path,
                    viewer: human,
                    blob_oid,
                },
            )
        } else {
            (
                Mutation::UnmarkViewed {
                    review_id,
                    repo_id: file.repo_id,
                    path: file.path.clone(),
                },
                EventBody::FileUnviewed {
                    review_id,
                    repo_id: file.repo_id,
                    path: file.path,
                    viewer: human,
                },
            )
        };
        Ok(self.mutate_with(mutation, body))
    }

    /// Apply `mutation` optimistically and send it. Rejected (nothing sent,
    /// nothing shown) when the daemon would reject it against the current
    /// view, pending mutations included.
    fn mutate(&mut self, mutation: Mutation) -> Result<Vec<Effect>, CoreError> {
        self.require_subscribed()?;
        let Some(open) = &self.view.review else {
            return Err(CoreError::NoOpenReview);
        };
        let meta = self.meta_now();
        let body = local_event(&open.snapshot, &meta, &mutation)?;
        Ok(self.mutate_with(mutation, body))
    }

    /// `mutate` with the optimistic event already built and validated.
    fn mutate_with(&mut self, mutation: Mutation, body: EventBody) -> Vec<Effect> {
        let meta = self.meta_now();
        let client_seq = self.next_client_seq;
        self.next_client_seq = client_seq.next();
        self.pending.push(Pending {
            client_seq,
            ts: meta.ts,
            mutation: mutation.clone(),
            body,
            sent: true,
        });
        let send = self.request(
            Request::Mutate {
                client_seq,
                mutation,
            },
            InFlight::Mutate { client_seq },
        );
        let sections = self.rebase();
        vec![send, render(&sections)]
    }

    /// Rebuild the shown snapshot as `committed` plus every pending event,
    /// and mirror the pending list into the view. Returns the sections the
    /// pending events touch (`Threads` when the list just emptied, so the
    /// pending marks clear).
    fn rebase(&mut self) -> Vec<ViewSection> {
        let (Some(committed), Some(open)) = (&self.committed, &mut self.view.review) else {
            return Vec::new();
        };
        let mut shown = committed.clone();
        let mut sections = Vec::new();
        let author = self.config.author.clone();
        for p in &self.pending {
            let meta = EventMeta {
                author: author.clone(),
                ts: p.ts,
            };
            sections.extend(apply_body(&mut shown, &meta, &p.body));
        }
        let was_pending = !open.pending.is_empty();
        open.snapshot = shown;
        open.pending = self
            .pending
            .iter()
            .map(|p| PendingEvent {
                client_seq: p.client_seq,
                body: p.body.clone(),
            })
            .collect();
        if was_pending && open.pending.is_empty() {
            sections.push(ViewSection::Threads);
        }
        sections
    }

    /// Retire the pending entry the daemon has answered (by echo or error).
    fn retire_pending(&mut self, client_seq: ClientSeq) -> bool {
        let before = self.pending.len();
        self.pending.retain(|p| p.client_seq != client_seq);
        self.pending.len() != before
    }

    /// Drop the open review and everything content-side it pinned.
    fn close_review(&mut self, effects: &mut Vec<Effect>) {
        self.view.review = None;
        self.view.draft = None;
        self.view.pending_refresh = false;
        self.deferred.clear();
        self.committed = None;
        self.pending.clear();
        self.stepper = None;
        self.focus_return = None;
        self.help_open = false;
        self.review_closed(effects);
    }

    /// A review snapshot arrived (streamed or single): it becomes the open
    /// review, replacing any other. Pending mutations for the same review
    /// survive (they are still on their way to the daemon).
    fn install_snapshot(&mut self, snapshot: ReviewSnapshot, effects: &mut Vec<Effect>) {
        let same = self
            .committed
            .as_ref()
            .is_some_and(|c| c.review.id == snapshot.review.id);
        let pending = if same {
            std::mem::take(&mut self.pending)
        } else {
            Vec::new()
        };
        self.close_review(effects);
        self.committed = Some(snapshot.clone());
        self.view.review = Some(OpenReview::new(snapshot));
        self.pending = pending;
        self.rebase();
        // Keys go to the explorer of the review that just opened.
        self.view.focus = Focus::Tree { index: 0 };
    }

    fn require_subscribed(&self) -> Result<(), CoreError> {
        match self.connection {
            Connection::Subscribed { .. } => Ok(()),
            Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                Err(self.wrong_state(InputKind::User))
            }
        }
    }

    pub(crate) fn request(&mut self, request: Request, waiting: InFlight) -> Effect {
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
                self.clear_in_flight();
                if was_down {
                    return Vec::new();
                }
                self.view.connection = ConnectionView::Disconnected;
                vec![render(&[ViewSection::Connection])]
            }
        }
    }

    // One arm per variant; splitting would hide the exhaustive match.
    #[allow(clippy::too_many_lines)]
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
                    self.clear_in_flight();
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
            ServerMsg::StreamItem { id, item } => self.stream_item(id, item),
            ServerMsg::StreamEnd { id } => {
                let Some(waiting) = self.in_flight.remove(&id) else {
                    return Err(CoreError::UnknownRequest(id));
                };
                let mut effects = Vec::new();
                match waiting {
                    InFlight::OpenReview { .. } => {
                        if self.view.review.is_some() {
                            effects.push(render(&[ViewSection::Diff]));
                        }
                    }
                    InFlight::FileRender { render, .. } => {
                        // The header may never have come (cancelled early or
                        // errored); clear the pending mark so it can be retried.
                        self.content_failed(&CacheKey::Header { render });
                        self.content_done(&mut effects);
                    }
                    InFlight::Subscribe
                    | InFlight::ListWorkspaces
                    | InFlight::ListReviews { .. }
                    | InFlight::ReviewSnapshot { .. }
                    | InFlight::ListFiles { .. }
                    | InFlight::ListCommits { .. }
                    | InFlight::TreeSnapshot { .. }
                    | InFlight::RenderChunk { .. }
                    | InFlight::Mutate { .. } => {
                        self.in_flight.insert(id, waiting);
                        return Err(CoreError::UnexpectedResponse {
                            id,
                            expected: "Response",
                            got: "StreamEnd",
                        });
                    }
                }
                Ok(effects)
            }
            ServerMsg::Error { id, error } => {
                let Some(waiting) = self.in_flight.remove(&id) else {
                    return Err(CoreError::UnknownRequest(id));
                };
                self.view.last_error = Some(error);
                let mut effects = Vec::new();
                let mut sections = vec![ViewSection::Connection];
                if let InFlight::Subscribe = waiting {
                    // Subscription failed: stay connected but not subscribed;
                    // the host may retry with `Connect` after disconnecting.
                    self.view.connection = ConnectionView::Connecting;
                }
                if let InFlight::Mutate { client_seq } = waiting {
                    // Rejected: the optimistic change is undone.
                    self.retire_pending(client_seq);
                    sections.extend(self.rebase());
                    sections.push(ViewSection::Threads);
                }
                if let Some(key) = waiting.key() {
                    self.content_failed(&key);
                }
                if waiting.is_content() {
                    self.content_done(&mut effects);
                }
                effects.push(render(&sections));
                Ok(effects)
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
                    Ok(self.apply_event(event))
                }
                // The daemon replays the gap after `Since::After` *before*
                // answering the subscribe, so events are valid once the
                // `Subscribe` request is out.
                Connection::Connecting {
                    hello_sent: true,
                    last_seq: Some(last_seq),
                } if event.seq <= last_seq => Err(CoreError::StaleEvent {
                    seq: event.seq,
                    last_seq,
                }),
                Connection::Connecting {
                    hello_sent: true,
                    last_seq,
                } if last_seq.is_some() => {
                    self.connection = Connection::Connecting {
                        hello_sent: true,
                        last_seq: Some(event.seq),
                    };
                    Ok(self.apply_event(event))
                }
                Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                    Err(self.wrong_state(InputKind::Server))
                }
            },
            ServerMsg::TreeDelta { delta } => match self.connection {
                Connection::Subscribed { .. } => Ok(self.tree_delta(&delta)),
                Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                    Err(self.wrong_state(InputKind::Server))
                }
            },
        }
    }

    fn clear_in_flight(&mut self) {
        let keys: Vec<CacheKey> = self.in_flight.values().filter_map(InFlight::key).collect();
        for k in &keys {
            self.content_failed(k);
        }
        self.in_flight.clear();
        self.content_reset_in_flight();
        for p in &mut self.pending {
            p.sent = false;
        }
    }

    // One arm per variant; splitting would hide the exhaustive match.
    #[allow(clippy::too_many_lines)]
    fn stream_item(&mut self, id: RequestId, item: StreamItem) -> Result<Vec<Effect>, CoreError> {
        let Some(waiting) = self.in_flight.get(&id).cloned() else {
            return Err(CoreError::UnknownRequest(id));
        };
        let got = stream_item_name(&item);
        let unexpected = |expected| CoreError::UnexpectedResponse { id, expected, got };
        let mut effects = Vec::new();
        match (waiting, item) {
            (InFlight::OpenReview { review_id }, StreamItem::ReviewSnapshot { snapshot }) => {
                if snapshot.review.id != review_id {
                    return Err(unexpected("ReviewSnapshot for the requested review"));
                }
                self.install_snapshot(snapshot, &mut effects);
                self.expect_streamed_trees(&mut effects);
                effects.push(render(&[
                    ViewSection::Diff,
                    ViewSection::Threads,
                    ViewSection::Conversation,
                    ViewSection::Draft,
                    ViewSection::Focus,
                ]));
            }
            (InFlight::OpenReview { .. }, StreamItem::TreeSnapshot { snapshot }) => {
                let key = CacheKey::Tree {
                    root: snapshot.root_oid,
                };
                self.arrived(
                    key,
                    CacheValue::Tree { snapshot },
                    content::Arrival::Stream,
                    &mut effects,
                );
            }
            (InFlight::OpenReview { .. }, StreamItem::Header { header }) => {
                let key = CacheKey::Header {
                    render: RenderKey::of_header(&header),
                };
                self.arrived(
                    key,
                    CacheValue::Header { header },
                    content::Arrival::Stream,
                    &mut effects,
                );
            }
            (
                InFlight::OpenReview { .. },
                StreamItem::Chunk {
                    repo_id,
                    path,
                    chunk,
                },
            ) => {
                let Some(render) = self.view.review.as_ref().and_then(|r| {
                    r.files
                        .iter()
                        .find(|k| k.repo_id == repo_id && k.path == path)
                        .cloned()
                }) else {
                    return Err(unexpected("Chunk of a file whose header was streamed"));
                };
                let key = CacheKey::Chunk {
                    render,
                    index: chunk.index,
                };
                self.arrived(
                    key,
                    CacheValue::Chunk { chunk },
                    content::Arrival::Stream,
                    &mut effects,
                );
            }
            (InFlight::FileRender { render, .. }, StreamItem::Header { header }) => {
                if RenderKey::of_header(&header) != render {
                    return Err(unexpected("Header of the requested file"));
                }
                let key = CacheKey::Header { render };
                self.arrived(
                    key,
                    CacheValue::Header { header },
                    content::Arrival::Stream,
                    &mut effects,
                );
            }
            (
                InFlight::FileRender { render, stop_after },
                StreamItem::Chunk {
                    repo_id,
                    path,
                    chunk,
                },
            ) => {
                if repo_id != render.repo_id || path != render.path {
                    return Err(unexpected("Chunk of the requested file"));
                }
                self.stream_chunk(id, &render, stop_after, chunk, &mut effects);
            }
            (
                InFlight::FileRender { .. },
                StreamItem::ReviewSnapshot { .. } | StreamItem::TreeSnapshot { .. },
            ) => return Err(unexpected("Header or Chunk")),
            (
                InFlight::Subscribe
                | InFlight::ListWorkspaces
                | InFlight::ListReviews { .. }
                | InFlight::ReviewSnapshot { .. }
                | InFlight::ListFiles { .. }
                | InFlight::ListCommits { .. }
                | InFlight::TreeSnapshot { .. }
                | InFlight::RenderChunk { .. }
                | InFlight::Mutate { .. },
                StreamItem::ReviewSnapshot { .. }
                | StreamItem::TreeSnapshot { .. }
                | StreamItem::Header { .. }
                | StreamItem::Chunk { .. },
            ) => return Err(unexpected("Response")),
        }
        Ok(effects)
    }

    // One arm per variant; splitting would hide the exhaustive match.
    #[allow(clippy::too_many_lines)]
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
                // Pending mutations lost with the connection go out again,
                // once each, with their original `client_seq` so the daemon
                // (and this core, on the echo) can match them.
                let resend: Vec<(ClientSeq, Mutation)> = self
                    .pending
                    .iter_mut()
                    .filter(|p| !p.sent)
                    .map(|p| {
                        p.sent = true;
                        (p.client_seq, p.mutation.clone())
                    })
                    .collect();
                let mut effects: Vec<Effect> = resend
                    .into_iter()
                    .map(|(client_seq, mutation)| {
                        self.request(
                            Request::Mutate {
                                client_seq,
                                mutation,
                            },
                            InFlight::Mutate { client_seq },
                        )
                    })
                    .collect();
                // The review list is the union of every workspace's reviews;
                // start with the workspaces.
                effects.push(self.request(Request::ListWorkspaces, InFlight::ListWorkspaces));
                effects.push(render(&[ViewSection::Connection]));
                effects
            }
            (InFlight::ListWorkspaces, Response::Workspaces { workspaces }) => {
                let ids: Vec<WorkspaceId> = workspaces.iter().map(|w| w.id).collect();
                self.view.workspaces = workspaces;
                let mut effects: Vec<Effect> = ids
                    .into_iter()
                    .map(|workspace_id| {
                        self.request(
                            Request::ListReviews { workspace_id },
                            InFlight::ListReviews { workspace_id },
                        )
                    })
                    .collect();
                effects.push(render(&[ViewSection::ReviewList]));
                effects
            }
            (InFlight::ListReviews { workspace_id }, Response::Reviews { reviews }) => {
                // Replace this workspace's reviews, keep the others'.
                self.view.reviews.retain(|r| r.workspace_id != workspace_id);
                self.view.reviews.extend(reviews);
                self.view.reviews.sort_by_key(|r| r.created);
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
                let mut effects = Vec::new();
                self.install_snapshot(snapshot, &mut effects);
                self.review_opened_piecewise(review_id, &mut effects);
                effects.push(render(&[
                    ViewSection::Diff,
                    ViewSection::Threads,
                    ViewSection::Conversation,
                    ViewSection::Draft,
                    ViewSection::Focus,
                ]));
                effects
            }
            (InFlight::ListFiles { review_id }, Response::Files { files }) => {
                let mut effects = Vec::new();
                if self.open_mut(review_id).is_some() {
                    self.review_files(review_id, files, &mut effects);
                }
                effects
            }
            (InFlight::ListCommits { repo_id }, Response::Commits { commits }) => {
                if self.view.review.is_some() {
                    self.stepper = Some(CommitStepper::from_commits(repo_id, &commits));
                }
                Vec::new()
            }
            (InFlight::TreeSnapshot { root }, Response::TreeSnapshot { snapshot }) => {
                if snapshot.root_oid != root {
                    return Err(CoreError::UnexpectedResponse {
                        id,
                        expected: "TreeSnapshot of the requested root",
                        got: "TreeSnapshot of another root",
                    });
                }
                let mut effects = Vec::new();
                self.arrived(
                    CacheKey::Tree { root },
                    CacheValue::Tree { snapshot },
                    content::Arrival::Response,
                    &mut effects,
                );
                self.content_done(&mut effects);
                effects
            }
            (InFlight::RenderChunk { key }, Response::RenderChunk { chunk }) => {
                let CacheKey::Chunk { index, .. } = &key else {
                    return Err(CoreError::UnexpectedResponse {
                        id,
                        expected: "RenderChunk",
                        got: "RenderChunk for a non-chunk key",
                    });
                };
                if chunk.index != *index {
                    return Err(CoreError::UnexpectedResponse {
                        id,
                        expected: "RenderChunk with the requested index",
                        got: "RenderChunk with another index",
                    });
                }
                let mut effects = Vec::new();
                self.arrived(
                    key,
                    CacheValue::Chunk { chunk },
                    content::Arrival::Response,
                    &mut effects,
                );
                self.content_done(&mut effects);
                effects
            }
            (InFlight::Mutate { client_seq }, Response::Committed { event }) => {
                // The same event is also broadcast; whichever arrives first
                // applies it, the other only retires the pending entry.
                match self.connection {
                    Connection::Subscribed { last_seq } if last_seq >= event.seq => {
                        if self.retire_pending(client_seq) {
                            let sections = self.rebase();
                            vec![render(&sections)]
                        } else {
                            Vec::new()
                        }
                    }
                    Connection::Subscribed { .. } => {
                        self.connection = Connection::Subscribed {
                            last_seq: event.seq,
                        };
                        self.apply_event(event)
                    }
                    Connection::Disconnected { .. } | Connection::Connecting { .. } => {
                        return Err(self.wrong_state(InputKind::Server));
                    }
                }
            }
            (waiting, _) => {
                let expected = match waiting {
                    InFlight::Subscribe => "Subscribed",
                    InFlight::ListWorkspaces => "Workspaces",
                    InFlight::ListReviews { .. } => "Reviews",
                    InFlight::OpenReview { .. } | InFlight::FileRender { .. } => "StreamItem",
                    InFlight::ReviewSnapshot { .. } => "ReviewSnapshot",
                    InFlight::ListFiles { .. } => "Files",
                    InFlight::ListCommits { .. } => "Commits",
                    InFlight::TreeSnapshot { .. } => "TreeSnapshot",
                    InFlight::RenderChunk { .. } => "RenderChunk",
                    InFlight::Mutate { .. } => "Committed",
                };
                return Err(CoreError::UnexpectedResponse { id, expected, got });
            }
        };
        self.in_flight.remove(&id);
        Ok(effects)
    }

    /// Fold a committed event into the view: the review list first, then the
    /// open review's committed snapshot, then the pending list on top.
    // One arm per event; splitting would hide the exhaustive match.
    #[allow(clippy::too_many_lines)]
    fn apply_event(&mut self, event: Event) -> Vec<Effect> {
        let mut sections = Vec::new();
        let mut effects = Vec::new();
        if event.client_id == self.config.client_id {
            // Our own mutation came back: it is committed now.
            self.retire_pending(event.client_seq);
        }
        match &event.body {
            EventBody::ReviewCreated { review } => {
                self.view.reviews.retain(|r| r.id != review.id);
                self.view.reviews.push(review.clone());
                sections.push(ViewSection::ReviewList);
            }
            EventBody::ReviewUpdated {
                review_id,
                title,
                status,
            } => {
                if let Some(r) = self.view.reviews.iter_mut().find(|r| r.id == *review_id) {
                    r.title.clone_from(title);
                    r.status = *status;
                    sections.push(ViewSection::ReviewList);
                }
            }
            EventBody::ReviewDeleted { review_id } => {
                let before = self.view.reviews.len();
                self.view.reviews.retain(|r| r.id != *review_id);
                if self.view.reviews.len() != before {
                    sections.push(ViewSection::ReviewList);
                }
                if self.open_mut(*review_id).is_some() {
                    self.close_review(&mut effects);
                    sections.extend([ViewSection::Diff, ViewSection::Threads, ViewSection::Draft]);
                }
            }
            EventBody::ReviewTargetsResolved { review_id, .. }
                if self.open_mut(*review_id).is_some() && self.view.draft.is_some() =>
            {
                // Held back until the draft closes (§5.4).
                self.deferred.push(event);
                if !self.view.pending_refresh {
                    self.view.pending_refresh = true;
                    sections.push(ViewSection::Draft);
                }
                return if sections.is_empty() {
                    Vec::new()
                } else {
                    vec![render(&sections)]
                };
            }
            EventBody::WorkspaceCreated { workspace } => {
                self.view.workspaces.retain(|w| w.id != workspace.id);
                self.view.workspaces.push(workspace.clone());
                sections.push(ViewSection::ReviewList);
            }
            EventBody::WorkspaceUpdated { workspace_id, name } => {
                if let Some(w) = self
                    .view
                    .workspaces
                    .iter_mut()
                    .find(|w| w.id == *workspace_id)
                {
                    w.name.clone_from(name);
                    sections.push(ViewSection::ReviewList);
                }
            }
            EventBody::RepoAttached { workspace_id, repo } => {
                if let Some(w) = self
                    .view
                    .workspaces
                    .iter_mut()
                    .find(|w| w.id == *workspace_id)
                {
                    w.repos.retain(|r| r.id != repo.id);
                    w.repos.push(repo.clone());
                    sections.push(ViewSection::ReviewList);
                }
            }
            EventBody::RepoDetached {
                workspace_id,
                repo_id,
            } => {
                if let Some(w) = self
                    .view
                    .workspaces
                    .iter_mut()
                    .find(|w| w.id == *workspace_id)
                {
                    w.repos.retain(|r| r.id != *repo_id);
                    sections.push(ViewSection::ReviewList);
                }
            }
            EventBody::ReviewTargetsResolved { .. }
            | EventBody::CommentCreated { .. }
            | EventBody::CommentEdited { .. }
            | EventBody::CommentDeleted { .. }
            | EventBody::CommentReanchored { .. }
            | EventBody::ThreadResolved { .. }
            | EventBody::ThreadUnresolved { .. }
            | EventBody::FileViewed { .. }
            | EventBody::FileUnviewed { .. }
            | EventBody::ReviewRequested { .. }
            | EventBody::SuggestionApplied { .. } => {}
        }
        let concerns_open = event
            .body
            .review_id()
            .is_some_and(|id| self.committed.as_ref().is_some_and(|c| c.review.id == id));
        if concerns_open {
            let meta = EventMeta {
                author: event.author.clone(),
                ts: event.ts,
            };
            if let Some(committed) = &mut self.committed {
                sections.extend(apply_body(committed, &meta, &event.body));
            }
            sections.extend(self.rebase());
            if let EventBody::ReviewTargetsResolved { review_id, .. } = event.body {
                // New heads mean new trees and renders: refetch the trees
                // and the file list; headers re-key by blob.
                self.want_review_trees(&mut effects);
                effects.push(self.request(
                    Request::ListFiles { review_id },
                    InFlight::ListFiles { review_id },
                ));
            }
        }
        if !sections.is_empty() {
            effects.push(render(&sections));
        }
        effects
    }

    fn open_mut(&mut self, review_id: ReviewId) -> Option<&mut OpenReview> {
        self.view
            .review
            .as_mut()
            .filter(|r| r.snapshot.review.id == review_id)
    }

    /// Apply refreshes held back during a draft; always renders `Draft`
    /// (the draft just closed) plus whatever the refreshes touched, after
    /// any fetches the refreshes issued.
    fn drain_deferred(&mut self) -> Vec<Effect> {
        let deferred = std::mem::take(&mut self.deferred);
        self.view.pending_refresh = false;
        let mut sections = vec![ViewSection::Draft, ViewSection::Focus];
        let mut effects = Vec::new();
        for event in deferred {
            for effect in self.apply_event(event) {
                match effect {
                    Effect::Render(delta) => sections.extend(delta.sections),
                    Effect::Connect
                    | Effect::Disconnect
                    | Effect::Send(_)
                    | Effect::Persist { .. }
                    | Effect::Load { .. }
                    | Effect::Remove { .. } => effects.push(effect),
                }
            }
        }
        effects.push(render(&sections));
        effects
    }
}

/// What `command` would do right now, without doing it. Hosts use this
/// for menus and tests use it to prove every action is reachable.
pub fn resolve_command(core: &ClientCore, command: Command) -> Result<Action, NoTarget> {
    focus::resolve(core, command)
}

pub(crate) fn render(sections: &[ViewSection]) -> Effect {
    Effect::Render(ViewDelta::new(sections))
}

fn stream_item_name(item: &StreamItem) -> &'static str {
    match item {
        StreamItem::ReviewSnapshot { .. } => "ReviewSnapshot",
        StreamItem::TreeSnapshot { .. } => "TreeSnapshot",
        StreamItem::Header { .. } => "Header",
        StreamItem::Chunk { .. } => "Chunk",
    }
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
