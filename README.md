# Moor

A daemon-backed code review tool. Comments are *moored* to content (blobs), not to diffs or line numbers.

- One always-running daemon (`moord`) per machine owns workspaces, reviews and comments in an append-only event log (redb).
- Clients — Tauri desktop (first), browser, TUI, CLI (`moor`), agents via MCP — attach over the same JSON protocol, locally or through an SSH tunnel.
- A workspace groups multiple git repos; a review spans any base vs any head across them, with commit stepping.
- GitHub-style diff review plus a file explorer over any ref; inline, file-level and review-level comments; human and agent authorship recorded.
- Clients cache everything by OID (memory + disk), apply mutations optimistically, and are keyboard-first.

## Status

Design phase. No code yet beyond a Cargo stub.

## Read

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — the design and all resolved decisions.
- [`docs/PLAN.md`](docs/PLAN.md) — four milestones with per-task test strategy. Start at **Milestone 1.1**.
- [`AGENTS.md`](AGENTS.md) — principles and conventions for anyone (human or agent) contributing.

## Naming

Project **Moor** · CLI `moor` · daemon `moord` · MCP shim `moor-mcp` · crates `moor-*` · data dir `~/.local/share/moor/`.
