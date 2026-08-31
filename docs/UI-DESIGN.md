# UI design (settled 2026-08-31)

The product design for moor's review UI, agreed on the design canvas
(<https://claude.ai/code/artifact/f9e63a11-b6cd-4b8e-8ece-8a2d95f8c623>;
working artboards in `design/`). This is the reference the web UI is built
to and the TUI will follow. PLAN 4.4–4.6 items are superseded by this
where they differ.

## Principles

- **Keyboard-first, everywhere.** Every feature, toggle, tab, and pane is
  reachable by keyboard; a mouse click is an alias for a chord. Every
  button's hover tooltip shows its shortcut. Pane focus cycles with
  `tab`/`shift-tab`; if pane resizing is built it is keyboard-native
  (`⌥←/→` resize focused pane, `⌥0` reset), never drag-only.
- **One keymap, all shells.** The keymap lives in `moor-client-core` and
  drives web, Tauri, and the future TUI identically. Hints, the `?` help
  overlay, and button tooltips are all derived from it — never
  hand-written. A control without a binding is a bug (testable, like the
  CSS-class coverage test).
- **Terminal-safe canonical bindings.** Canonical chords use single
  printable keys and `ctrl` only, so the TUI is identical. GUI shells may
  add mac aliases (`⌘K` for `ctrl+k`, `⌘P`, `⌘⇧F`) and show them in
  tooltips, but the canonical spelling is the source of truth.
- **Minimal modality.** No vim-style modes. Text inputs (composer,
  search) capture keys until `Esc`/submit; everything else is single keys
  resolved by the focus context. Multi-key sequences stay rare (`] c`
  style navigation only).
- **User-configurable bindings.** `~/.config/moor/keys.toml` overrides
  per-context bindings. Commands are an enum; an unknown command or
  unparsable chord fails loudly at load. Everything derived from the
  keymap (hints, help, tooltips) re-derives from the loaded map.

## Layout

Claude-viewer anatomy, moor tokens (`ui/src/styles/app.css`; dark and
light both):

- **Header**: review title · scope control (below) · `base → head` chips
  (resolved branch names; a working-tree head shows `branch (worktree)`) ·
  layout + whitespace toggles · totals (`N files · +A −D`) · connection
  dot.
- **Tabs**: `Files changed` (with count) · `Conversation` (with thread
  count) · `Browse`. Keys `1/2/3`.
- **Left sidebar**: file tree with per-file `+A −D` and thread-count
  badges; commits list below (chronological, working tree on top).
- **Center**: all changed files stacked in one scroll, sticky per-file
  headers (`path · +A −D · viewed checkbox`), viewed files collapse.
- **Hint bar** footer: connection state, viewed progress, primary
  bindings for the focused context.

## Diff scope

- **All changes** (default): `base → branch tip`, plus a
  **`+ working tree`** toggle that folds uncommitted changes in.
  Off = committed changes only. Base defaults to whatever the review was
  created with (upstream → `origin/HEAD` → `main`).
- **By commit**: each commit diffs against its parent; the working tree
  is the final step. `p`/`n` steps, `2 of 4` position indicator, commit
  panel (subject, full body, author, relative+absolute time, parent).
  The sidebar commit list is the step list.

## Diff rendering

- Unified and **split (side-by-side)** layouts, `s` toggles, persisted.
  In split view an added line pairs with a filler cell; inline threads
  span both columns.
- **Hide whitespace** (`w`), persisted; re-keys renders.
- **Syntax highlighting** everywhere (daemon-rendered `SpanClass` spans;
  already implemented; falls back to plain past the size cap).
- **Context expanders** on every collapsed band (GitHub-style): `↥`/`↧`
  expand ±20 lines, clicking the band expands it fully, plus an
  "expand full file" affordance per file. Key `x` expands at the cursor.
- Word-level change highlights within modified lines (already rendered).

## Comments and threads

- **Inline threads are primary**: a thread renders under its anchored
  line, in the diff or in Browse. `c` opens the composer at the focused
  line/file, `r` replies, `⌘Enter`/`ctrl+enter` submits, `Esc` discards.
- **Conversation tab** aggregates every thread chronologically
  (GitHub-style): review-level comments, file-anchored threads quoting
  their diff lines with a "jump to diff" link, resolved threads collapsed
  to one line, a hide-resolved filter, and a review-level composer.
- **Every comment records the diff it was made on**
  (`Comment::context: Option<ChangeKind>`, wired end to end). Threads
  show a context chip (`main → worktree @<oid>` or `browse @<ref>`).
  Jumping to a comment whose diff has moved on opens the **original
  diff** read-only, with a banner ("Viewing the diff this comment was
  made on… Back to current diff (Esc)"). Outdated threads in the
  Conversation tab link "open original diff".
- **Browse comments** anchor to `file@ref` with no diff pair.

## Browse

A third tab for reading code without a diff: full file tree (every file,
not just changed), plain syntax-highlighted file view, comments on any
line. A `viewing: <ref> ▾` picker accepts any ref — branch, tag, commit,
working tree (the daemon's tree snapshots already take an arbitrary
`RefSpec`).

## Search

One palette (`ctrl+k`), three modes, `tab` cycles:

- **Files** — fuzzy file-name find (exists today; `ctrl+p` opens the
  palette in this mode).
- **Content** — keyword/regex across files, grouped by file with
  highlighted matching lines and line numbers; scope toggle
  `Changed files | All files @<ref>`; Enter jumps to the file at that
  line in the current tab. Needs a new daemon `Search` request.
- **`>` Actions** — every command by name (toggle split, hide
  whitespace, open review…), the discoverable face of the keymap.

`/` finds within the open file.

## Canonical bindings (defaults)

| Chord | Action |
| --- | --- |
| `ctrl+k` | palette (`>` actions, files, content) |
| `ctrl+p` | palette in Files mode |
| `/` | find in open file |
| `j`/`k` (and arrows, unadvertised) | next/prev row |
| `n`/`p` | next/prev hunk (next/prev commit in By-commit mode) |
| `]f` / `[f` | next/prev file |
| `c` / `r` | comment / reply |
| `v` | toggle viewed |
| `s` / `w` | split layout / hide whitespace |
| `x` | expand context at cursor |
| `1`/`2`/`3` | Files changed / Conversation / Browse |
| `tab`/`shift-tab` | cycle pane focus |
| `ctrl+enter` | submit comment |
| `Esc` | close/back (also leaves jump-to-context) |
| `?` | help overlay |

Arrow keys work wherever `j`/`k` do but are deliberately not listed in
the hint bar.

## Implementation notes

- UI-only: tabs, toggles + tooltip derivation, split view, arrow keys,
  syntax spans in remaining spots, palette shell.
- Core: scope switching (re-target the open review), by-commit stepping
  reusing `StepperCommit`, keys.toml loading in the hosts.
- Protocol/daemon: context expanders (render with more context or splice
  `BlobRender` rows), browse-mode comments already possible
  (`Anchor::File/Lines` need not be in the diff), content `Search`
  request.
