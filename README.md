# Nits

A daemon-backed code review tool. Nits are anchored to content (blobs), not to diffs or line numbers — so they survive rebases, amends and force-pushes.

- One always-running daemon (`nitsd`) per machine owns workspaces, reviews and comments in an append-only event log (redb).
- Clients — Tauri desktop (first), browser, TUI, CLI (`nits`), agents via MCP — attach over the same JSON protocol, locally or through an SSH tunnel.
- A workspace groups multiple git repos; a review spans any base vs any head across them, with commit stepping.
- GitHub-style diff review plus a file explorer over any ref; inline, file-level and review-level comments; human and agent authorship recorded.
- Clients cache everything by OID (memory + disk), apply mutations optimistically, and are keyboard-first.

## Install

```console
$ brew install jonoprest/nits/nits          # macOS, Linuxbrew
$ cargo install nits                        # any platform with a Rust toolchain
$ yay -S nits-bin                           # Arch
```

Debian/Ubuntu and Fedora/RHEL repositories, and tarballs for
`{x86_64,aarch64}-{apple-darwin,unknown-linux-gnu,unknown-linux-musl}`, are at
<https://jonoprest.github.io/nits/> and on the [releases page][releases].
Every package ships `nitsd` alongside the client, which starts it on demand.
There is no Windows build yet — the daemon's transport is unix-socket only.

Cutting a release: [`docs/RELEASING.md`](docs/RELEASING.md).

[releases]: https://github.com/JonoPrest/nits/releases

## Status

Milestone 1.1–1.2 done: Cargo workspace, CI, `nits-protocol` (all wire types + JSON fixtures), `nits-test-support` (real-git `RepoBuilder`). 1.3 done: redb event store with schema versioning. 1.4 done: git engine (gix + git CLI). **Milestone 1 complete** (protocol, store, git engine, render model, reviews, comments + anchoring, `Core` façade). Milestone 2.1–2.2 done: `nitsd` (length-prefixed JSON frames, version handshake, unix-socket/stdio server, subscriptions with gap replay, streamed `open_review`, async `nitsd::client`). 2.3 done: debounced file watcher (`TreeDelta` + re-resolve of working-tree reviews). 2.4 (WebSocket, `nitsd --ws 127.0.0.1:7677`) done. 2.5 done: `nits-mcp` stdio server (14 tools, agent provenance from `initialize`, `subscribe_events` long-poll). 2.6 done: `nits` CLI (`workspace add|list|attach`, `review create|list|show`, `files`, `diff`, `show`, `comment add|reply|resolve|list`, `events [--follow]`, `--json`, `--agent`). 2.7 done: lifecycle (`--data-dir`/`--socket`/`--stdio`, ctrl-c shutdown, stale-socket reclaim; kill -9 mid-burst test reopens consistent). **Milestone 2 complete.** Contexts + daemon lifecycle: `nits context add-ssh box user@host` / `nits -c box …` (no persisted current context; workspace/repo default from cwd), `nits daemon status|start|stop`, `nitsd --stdio` proxies to (and auto-starts) the one daemon per machine. 3.0 done: benchmark triggers (`cargo bench`, `docs/BENCHMARKS.md`; snapshot-after-edit and comment-burst triggers tripped, fixes pending). 3.1 done: `nits-client-core` sans-I/O state machine (`ClientCore::handle(Input) -> Result<Vec<Effect>, CoreError>`, connection `Disconnected | Connecting | Subscribed { last_seq }` with reconnect via `Since::After`, draft open/submit/discard with deferred refresh, proptest over random input sequences, wasm check in CI). Next: 3.2 cache — see `docs/HANDOVER.md`.

## Read

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — the design and all resolved decisions.
- [`docs/PLAN.md`](docs/PLAN.md) — four milestones with per-task test strategy. Start at **Milestone 1.1**.
- [`AGENTS.md`](AGENTS.md) — principles and conventions for anyone (human or agent) contributing.

## Naming

Project **Nits** · CLI `nits` · daemon `nitsd` · MCP shim `nits-mcp` · crates `nits-*` · data dir `~/.local/share/nits/`.
