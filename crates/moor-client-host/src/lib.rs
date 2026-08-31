//! Host loop around [`ClientCore`] (plan 4.3, ARCHITECTURE §5): owns the
//! transport (unix socket, length-prefixed frames), the host KV store
//! (redb file or memory), the clock, and turns every `Effect::Render` into
//! [`ViewPatch`]es for a UI. Host-agnostic: the Tauri app and the TUI wrap
//! it; the integration test drives it against a real daemon.
//!
//! One task owns the core. Everything else (socket reader/writer, the
//! ticker, the UI) talks to it over channels, so the core never blocks and
//! is never shared.

pub mod keys_file;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use moor_client_core::{
    Action, CacheConfig, ClientCore, Config, Effect, IdSeed, Input, KeyChord, Keymap,
    TransportEvent, ViewPatch,
};
use moor_protocol::{Author, BuildInfo, ClientId, ClientMsg, Envelope, ServerMsg};
use moord::codec;
use redb::ReadableDatabase;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Who this client is on the wire.
#[derive(Debug, Clone)]
pub struct Identity {
    pub client_id: ClientId,
    pub client: BuildInfo,
    pub author: Author,
}

/// Where the host KV lives.
#[derive(Debug, Clone)]
pub enum KvConfig {
    /// Nothing survives the process; tests and the browser fallback.
    Memory,
    /// A redb file under the host's data dir.
    Redb(PathBuf),
}

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub socket: PathBuf,
    pub kv: KvConfig,
    pub identity: Identity,
    pub cache: CacheConfig,
    /// Entropy for the ids the core mints. Hosts pass random bits.
    pub id_seed: IdSeed,
    /// How often the core sees the clock.
    pub tick: Duration,
    /// `keys.toml` with keymap overrides (UI-DESIGN §bindings); read when
    /// the core loads its keymap. `None` disables the file.
    pub keys_file: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("kv store: {0}")]
    Kv(#[from] KvError),
}

#[derive(Debug, thiserror::Error)]
pub enum KvError {
    #[error(transparent)]
    Database(#[from] redb::DatabaseError),
    #[error(transparent)]
    Transaction(#[from] redb::TransactionError),
    #[error(transparent)]
    Table(#[from] redb::TableError),
    #[error(transparent)]
    Storage(#[from] redb::StorageError),
    #[error(transparent)]
    Commit(#[from] redb::CommitError),
}

const KV_TABLE: redb::TableDefinition<'_, &str, &[u8]> = redb::TableDefinition::new("kv");

/// The host KV behind `Effect::Persist` / `Load` / `Remove`.
enum Kv {
    Memory(HashMap<String, Vec<u8>>),
    Redb(redb::Database),
}

impl Kv {
    fn open(config: &KvConfig) -> Result<Self, KvError> {
        Ok(match config {
            KvConfig::Memory => Kv::Memory(HashMap::new()),
            KvConfig::Redb(path) => {
                let db = redb::Database::create(path)?;
                // Make sure the table exists so reads never fail on a fresh file.
                let tx = db.begin_write()?;
                tx.open_table(KV_TABLE)?;
                tx.commit()?;
                Kv::Redb(db)
            }
        })
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, KvError> {
        match self {
            Kv::Memory(m) => Ok(m.get(key).cloned()),
            Kv::Redb(db) => {
                let tx = db.begin_read()?;
                let table = tx.open_table(KV_TABLE)?;
                Ok(table.get(key)?.map(|v| v.value().to_vec()))
            }
        }
    }

    fn put(&mut self, key: &str, value: &[u8]) -> Result<(), KvError> {
        match self {
            Kv::Memory(m) => {
                m.insert(key.to_owned(), value.to_vec());
                Ok(())
            }
            Kv::Redb(db) => {
                let tx = db.begin_write()?;
                tx.open_table(KV_TABLE)?.insert(key, value)?;
                tx.commit()?;
                Ok(())
            }
        }
    }

    fn remove(&mut self, key: &str) -> Result<(), KvError> {
        match self {
            Kv::Memory(m) => {
                m.remove(key);
                Ok(())
            }
            Kv::Redb(db) => {
                let tx = db.begin_write()?;
                tx.open_table(KV_TABLE)?.remove(key)?;
                tx.commit()?;
                Ok(())
            }
        }
    }
}

/// What the UI side holds: send actions in, receive patches out.
#[derive(Debug, Clone)]
pub struct Handle {
    actions: mpsc::UnboundedSender<Command>,
}

/// Commands the UI side sends to the host task.
#[derive(Debug)]
enum Command {
    Dispatch(Action),
    /// A key chord outside a text input; the core resolves it (§6.4).
    Key(KeyChord),
    /// Emit every section (a UI that just attached).
    Attach,
}

impl Handle {
    /// Queue an action for the core. Returns `false` once the host is gone.
    pub fn dispatch(&self, action: Action) -> bool {
        self.actions.send(Command::Dispatch(action)).is_ok()
    }

    /// Queue a key chord for the core's keymap.
    pub fn key(&self, chord: KeyChord) -> bool {
        self.actions.send(Command::Key(chord)).is_ok()
    }

    /// Ask for every section to be emitted.
    pub fn attach(&self) -> bool {
        self.actions.send(Command::Attach).is_ok()
    }
}

/// Something the transport tasks report to the host task.
#[derive(Debug)]
enum Incoming {
    Connected(mpsc::UnboundedSender<ClientMsg>),
    Msg(ServerMsg),
    Disconnected,
}

/// Start the host on the current runtime. Patches for the UI arrive on the
/// returned receiver, one `Vec` per render; the task ends when `shutdown`
/// is cancelled or the receiver is dropped.
pub fn spawn(
    config: HostConfig,
    shutdown: CancellationToken,
) -> Result<(Handle, mpsc::UnboundedReceiver<Vec<ViewPatch>>), HostError> {
    let HostConfig {
        socket,
        kv,
        identity,
        cache,
        id_seed,
        tick,
        keys_file,
    } = config;
    let kv = Kv::open(&kv)?;
    let (actions_tx, actions_rx) = mpsc::unbounded_channel();
    let (patches_tx, patches_rx) = mpsc::unbounded_channel();
    let host = Host {
        core: ClientCore::new(Config {
            client_id: identity.client_id,
            client: identity.client,
            author: identity.author,
            id_seed,
            cache,
        }),
        socket,
        kv,
        tick,
        keys_file,
        writer: None,
        patches: patches_tx,
    };
    tokio::spawn(host.run(actions_rx, shutdown));
    Ok((
        Handle {
            actions: actions_tx,
        },
        patches_rx,
    ))
}

struct Host {
    core: ClientCore,
    socket: PathBuf,
    kv: Kv,
    tick: Duration,
    keys_file: Option<PathBuf>,
    /// Outbound frames while connected.
    writer: Option<mpsc::UnboundedSender<ClientMsg>>,
    patches: mpsc::UnboundedSender<Vec<ViewPatch>>,
}

impl Host {
    /// Unix time in ms: the core stamps ids and pending events with it.
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }

    async fn run(
        mut self,
        mut actions: mpsc::UnboundedReceiver<Command>,
        shutdown: CancellationToken,
    ) {
        let (incoming_tx, mut incoming) = mpsc::unbounded_channel::<Incoming>();
        let mut ticker = tokio::time::interval(self.tick);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                cmd = actions.recv() => match cmd {
                    Some(Command::Dispatch(action)) => self.feed(Input::User(action), &incoming_tx),
                    Some(Command::Key(chord)) => self.feed(Input::Key(chord), &incoming_tx),
                    Some(Command::Attach) => {
                        if self.patches.send(self.core.view().full_patches()).is_err() {
                            break;
                        }
                    }
                    None => break,
                },
                Some(msg) = incoming.recv() => match msg {
                    Incoming::Connected(writer) => {
                        self.writer = Some(writer);
                        self.feed(Input::Transport(TransportEvent::Connected), &incoming_tx);
                    }
                    Incoming::Msg(msg) => self.feed(Input::Server(msg), &incoming_tx),
                    Incoming::Disconnected => {
                        self.writer = None;
                        self.feed(Input::Transport(TransportEvent::Disconnected), &incoming_tx);
                    }
                },
                _ = ticker.tick() => {
                    let now = Self::now_ms();
                    self.feed(Input::Tick(now), &incoming_tx);
                }
            }
        }
    }

    /// Run one input through the core and act on its effects. Rejections
    /// are ordinary (an unbound key, a stale event): logged, nothing else.
    fn feed(&mut self, input: Input, incoming: &mpsc::UnboundedSender<Incoming>) {
        match self.core.handle(input) {
            Ok(effects) => {
                for e in effects {
                    self.effect(e, incoming);
                }
            }
            Err(err) => tracing::debug!(%err, "input rejected"),
        }
    }

    fn effect(&mut self, effect: Effect, incoming: &mpsc::UnboundedSender<Incoming>) {
        match effect {
            Effect::Connect => {
                tokio::spawn(connect(self.socket.clone(), incoming.clone()));
            }
            Effect::Disconnect => {
                // Dropping the sender ends the writer task, which closes the
                // socket; the reader then reports `Disconnected`.
                self.writer = None;
            }
            Effect::Send(msg) => {
                if let Some(w) = &self.writer
                    && w.send(msg).is_err()
                {
                    self.writer = None;
                }
            }
            Effect::Persist { key, value } => {
                if let Err(err) = self.kv.put(&key, &value) {
                    tracing::warn!(%err, key, "persist failed");
                }
            }
            Effect::Remove { key } => {
                if let Err(err) = self.kv.remove(&key) {
                    tracing::warn!(%err, key, "remove failed");
                }
            }
            Effect::Load { key } => {
                let value = if key == Keymap::KEY {
                    // keys.toml outranks anything stored: it is the user's
                    // file; a bad file is rejected loudly and whole.
                    self.keymap_overrides()
                } else {
                    match self.kv.get(&key) {
                        Ok(v) => v,
                        Err(err) => {
                            tracing::warn!(%err, key, "load failed");
                            None
                        }
                    }
                };
                // The answer is an input like any other; its effects may
                // load more keys, so recurse through `feed`.
                self.feed(Input::Stored { key, value }, incoming);
            }
            Effect::Render(delta) => {
                let patches = self.core.view().patches(&delta);
                // A closed receiver means the UI is gone; the loop notices
                // on the next command.
                let _ = self.patches.send(patches);
            }
        }
    }
}

impl Host {
    /// The keymap overrides from `keys.toml`, serialised the way the core
    /// reads them; `None` when there is no file (or it is broken, which is
    /// reported and leaves the defaults).
    fn keymap_overrides(&self) -> Option<Vec<u8>> {
        let path = self.keys_file.as_ref()?;
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
            Err(err) => {
                tracing::error!(%err, path = %path.display(), "keys.toml unreadable; keeping default bindings");
                return None;
            }
        };
        match keys_file::parse(&text) {
            Ok(overrides) => serde_json::to_vec(&overrides).ok(),
            Err(err) => {
                tracing::error!(%err, path = %path.display(), "keys.toml rejected; keeping default bindings");
                None
            }
        }
    }
}

/// Dial the daemon; on success run reader and writer tasks until either
/// side closes. Every outcome reaches the host as an `Incoming`.
async fn connect(socket: PathBuf, incoming: mpsc::UnboundedSender<Incoming>) {
    match tokio::net::UnixStream::connect(&socket).await {
        Ok(stream) => serve_stream(stream, incoming).await,
        Err(err) => {
            tracing::warn!(%err, socket = %socket.display(), "connect failed");
            let _ = incoming.send(Incoming::Disconnected);
        }
    }
}

async fn serve_stream<S>(stream: S, incoming: mpsc::UnboundedSender<Incoming>)
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (mut rd, mut wr) = moord::transport::byte_stream(stream);
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ClientMsg>();
    if incoming.send(Incoming::Connected(out_tx)).is_err() {
        return;
    }
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if codec::write_msg(&mut wr, &Envelope::current(msg))
                .await
                .is_err()
            {
                break;
            }
        }
        // Sender dropped or write failed: close our side.
        let _ = tokio::io::AsyncWriteExt::shutdown(&mut wr).await;
    });
    loop {
        match codec::read_msg::<_, ServerMsg>(&mut rd).await {
            Ok(Some(env)) => {
                if incoming.send(Incoming::Msg(env.msg)).is_err() {
                    break;
                }
            }
            Ok(None) => break,
            Err(err) => {
                tracing::debug!(%err, "read failed");
                break;
            }
        }
    }
    writer.abort();
    let _ = incoming.send(Incoming::Disconnected);
}

/// Sensible defaults for a local daemon: memory-only cache tier, 100 ms
/// ticks.
#[must_use]
pub fn local_config(
    socket: &Path,
    identity: Identity,
    id_seed: IdSeed,
    kv: KvConfig,
) -> HostConfig {
    HostConfig {
        socket: socket.to_path_buf(),
        kv,
        identity,
        cache: CacheConfig::default(),
        id_seed,
        tick: Duration::from_millis(100),
        keys_file: keys_file::default_keys_path(),
    }
}
