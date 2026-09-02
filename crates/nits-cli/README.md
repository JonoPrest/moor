# nits

A daemon-backed code review tool. Nits are anchored to content (blobs), not to
diffs or line numbers — so they survive rebases, amends and force-pushes.

```console
$ cargo install nits nitsd
$ nits workspace add .
$ nits review create --base main --head HEAD
$ nits diff
```

- A workspace groups multiple git repos; a review spans any base vs any head
  across them, with commit stepping.
- Inline, file-level and review-level comments, with human and agent authorship
  recorded.
- Everything is served by one `nitsd` daemon per machine, reachable locally or
  over SSH; agents attach through `nits-mcp`. The CLI starts the daemon itself,
  so `nitsd` must be installed too — hence both crate names above, since cargo
  does not install binaries from dependency crates.

Other ways to install — Homebrew, `apt`, `dnf`, the AUR — and the full
documentation are at <https://github.com/JonoPrest/nits>.

## Licence

MIT.
