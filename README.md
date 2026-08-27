# Moor

A daemon-backed code review tool. Comments are *moored* to content (blobs), not to diffs or line numbers.

- One always-running daemon (`moord`) per machine owns workspaces, reviews and comments in an append-only event log (redb).
- Clients — Tauri desktop (first), browser, TUI, CLI (`moor`), agents via MCP — attach over the same JSON protocol, locally or through an SSH tunnel.
- A workspace groups multiple git repos; a review spans any base vs any head across them, with commit stepping.
- GitHub-style diff review plus a file explorer over any ref; inline, file-level and review-level comments; human and agent authorship recorded.
- Clients cache everything by OID (memory + disk), apply mutations optimistically, and are keyboard-first.

## Status

Milestone 1.1–1.2 done: Cargo workspace, CI, `moor-protocol` (all wire types + JSON fixtures), `moor-test-support` (real-git `RepoBuilder`). 1.3 done: redb event store with schema versioning. 1.4 done: git engine (gix + git CLI). **Milestone 1 complete** (protocol, store, git engine, render model, reviews, comments + anchoring, `Core` façade). Milestone 2.1–2.2 done: `moord` (length-prefixed JSON frames, version handshake, unix-socket/stdio server, subscriptions with gap replay, streamed `open_review`, async `moord::client`). 2.3 done: debounced file watcher (`TreeDelta` + re-resolve of working-tree reviews). 2.4 (WebSocket, `moord --ws 127.0.0.1:7677`) done. 2.5 done: `moor-mcp` stdio server (14 tools, agent provenance from `initialize`, `subscribe_events` long-poll). 2.6 done: `moor` CLI (`workspace add|list|attach`, `review create|list|show`, `files`, `diff`, `show`, `comment add|reply|resolve|list`, `events [--follow]`, `--json`, `--agent`). 2.7 done: lifecycle (`--data-dir`/`--socket`/`--stdio`, ctrl-c shutdown, stale-socket reclaim; kill -9 mid-burst test reopens consistent). **Milestone 2 complete.** Contexts + daemon lifecycle: `moor context add-ssh box user@host` / `moor -c box …`, `moor daemon status|start|stop`, `moord --stdio` proxies to (and auto-starts) the one daemon per machine. 3.0 done: benchmark triggers (`cargo bench`, `docs/BENCHMARKS.md`; snapshot-after-edit and comment-burst triggers tripped, fixes pending). Next: 3.1 `moor-client-core`.

## Read

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — the design and all resolved decisions.
- [`docs/PLAN.md`](docs/PLAN.md) — four milestones with per-task test strategy. Start at **Milestone 1.1**.
- [`AGENTS.md`](AGENTS.md) — principles and conventions for anyone (human or agent) contributing.

## Naming

Project **Moor** · CLI `moor` · daemon `moord` · MCP shim `moor-mcp` · crates `moor-*` · data dir `~/.local/share/moor/`.
