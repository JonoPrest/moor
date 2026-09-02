# Releasing

One package at a time, triggered by hand. `nits`, `nitsd` and `nits-mcp` version
independently and each gets its own tag (`nits-v0.3.1`, `nitsd-v0.2.0`, …), so
shipping a CLI fix never forces a daemon release.

## Cutting a release

1. Bump the version. All three binaries inherit `workspace.package.version` from
   the root `Cargo.toml`, so bumping it there releases whichever package you
   then name. To version one independently, give that crate its own `version =`
   instead of `version.workspace = true`.
2. Rehearse locally:
   ```console
   $ cargo xtask release-plan --package nits   # version, tag, publish order
   $ cargo xtask publish --package nits --dry-run
   ```
3. Run the **release** workflow (Actions → release → Run workflow), choose the
   package, and untick any channel you want to skip. `dry_run` builds every
   artifact and checks every collision without tagging or publishing anything.

The run refuses to start only if the tag exists **on a different commit** —
that means the version was released from other code, so bump rather than
deleting the tag. A tag on the commit being released is a *resume*: a previous
run got that far and something after it failed, and rerunning must be able to
finish. The tag step is idempotent to match, and `plan` reports `tag_state` as
`absent`, `at_head` or `elsewhere`.

Being on crates.io deliberately does **not** block a release. A crate is
published whenever it appears in another package's dependency closure (a `nits`
release publishes `nitsd` along the way), and that must not then block `nitsd`'s
own release of the same version, which has produced no tag, tarball, formula,
AUR package, deb or rpm. Re-publishing is harmless because `publish()` skips
versions crates.io already has; the plan reports that state as
`crate_already_published` for information only.

## What the workflow does

| Job | Result |
| --- | --- |
| `plan` | Resolves the version, asserts no collision, prints the publish order |
| `build` | Static and glibc binaries for 6 targets, each with a `.sha256` |
| `linux-packages` | GPG-signed `.deb` and `.rpm` for amd64 and arm64 |
| `release` | Pushes the tag, creates the GitHub release with `SHA256SUMS` |
| `crates-io` | `cargo publish` over the dependency closure, in order |
| `homebrew` | Rewrites `Formula/<package>.rb` in the tap |
| `aur` | Pushes `PKGBUILD` + `.SRCINFO` to `<package>-bin` |
| `linux-repo` | Folds the packages into the APT/YUM repo on `gh-pages` |

Everything downstream of `release` reads the checksums off the uploaded
artifacts, so a formula or PKGBUILD can never disagree with what shipped.

RPMs are signed with `rpmsign` in `linux-packages`, before upload, so the
release artifact and the repo copy stay byte-identical. The published repo sets
`gpgcheck=1` (package signatures) as well as `repo_gpgcheck=1` (metadata) — dnf
treats those as separate checks, and unsigned RPMs fail the first.
`cargo-generate-rpm`'s `--signing-key` is not used: it takes a key file but has
no passphrase option.

crates.io rate-limits *new* crates: a small burst, then one per interval. A
first release of several new crates therefore gets partway and is refused with
429. `publish()` waits that out rather than failing, so this needs one run
instead of a re-run per interval.

`dry_run` renders and validates every channel — formula Ruby syntax and
per-platform checksums, PKGBUILD/.SRCINFO agreement, and the APT/YUM indexes
and their signatures — and skips only the steps that push. Bugs in those paths
surface in a dry run rather than mid-release.

The publish order comes from `cargo metadata`, not a hand-kept list — see
`xtask/src/release.rs`, which topologically sorts the closure and skips crates
crates.io already has. `publish = false` edges are not followed: cargo strips
those path dependencies when it packages, which is also what keeps the
`nits-client-core` ↔ `nits-test-support` dev-dependency cycle out of the walk.

## What each package ships

Every client-facing package contains **both** the client and `nitsd`. The
clients start the daemon themselves — `nitsd::launch::sibling_binary` looks for
`nitsd` next to the running executable, then on `PATH` — so a package with only
the client installs something that passes `--version` and fails on its first
real command. The set lives in `Releasable::binaries()` and every channel reads
it, so the tarball, formula, PKGBUILD and deb/rpm cannot disagree.

| Package | Binaries |
| --- | --- |
| `nits` | `nits`, `nitsd` |
| `nitsd` | `nitsd` |
| `nits-mcp` | `nits-mcp`, `nitsd` |

The Homebrew `test` block runs `nits workspace list`, which reaches
`ensure_daemon`; `--version` and `daemon status` both pass without `nitsd`
present, so neither would catch a client shipped without it.

Archives, the formula and the AUR package **bundle** the daemon, because none of
those formats can express a dependency on another of our artifacts. deb and rpm
**depend** on it: the release builds one system package per binary, and the
client packages carry `Depends: nitsd` / `Requires: nitsd`, so the two clients
are co-installable and there is only ever one `/usr/bin/nitsd`.

`cargo install` can do neither, so it is documented as `cargo install nits
nitsd` — cargo does not install binaries from dependency crates, and a plain
`cargo install nits` yields a client that fails at `ensure_daemon`.

Note for deb metadata: cargo-deb interpolates only `$auto`, so a literal
`$version` in `depends`/`provides` reaches the control file verbatim and
`dpkg` rejects it. Keep those relationships unversioned.

## Platforms

Built: `{x86_64,aarch64}-{apple-darwin,unknown-linux-gnu,unknown-linux-musl}`,
each on a native runner so there is no cross toolchain to maintain.

Runner labels move: `macos-13` was retired on 2025-12-04 (`macos-15-intel` is
the last x86_64 macOS image, through Aug 2027), and `ubuntu-22.04`/`-arm` begin
brownouts on 2026-09-17. The matrix is on `ubuntu-24.04`/`-arm` and
`macos-15`/`macos-15-intel`; the glibc floor is therefore 2.39, and the musl
targets are the static option for older systems.

There is no Windows build. `nitsd` and `nits-client-host` use
`std::os::unix::net` unconditionally; Windows needs a real named-pipe transport
first, not a build target.

## Secrets

| Secret | Used by | How to get it |
| --- | --- | --- |
| `CARGO_REGISTRY_TOKEN` | `crates-io` | crates.io → Account Settings → API Tokens, scoped to `publish-new` + `publish-update` |
| `HOMEBREW_TAP_TOKEN` | `homebrew` | A fine-grained PAT with Contents: read/write on `JonoPrest/homebrew-nits` |
| `AUR_SSH_PRIVATE_KEY` | `aur` | The private half of a key registered on your AUR account |
| `GPG_PRIVATE_KEY` | `linux-packages`, `linux-repo` | `gpg --armor --export-secret-keys <id>` |
| `GPG_PASSPHRASE` | `linux-packages`, `linux-repo` | That key's passphrase |

## One-time setup

- **Homebrew tap** — create `JonoPrest/homebrew-nits` (public, empty). The
  workflow creates `Formula/` on first run. Users then get
  `brew install jonoprest/nits/nits`.
- **AUR** — not set up, and the `aur` input therefore defaults to **off**.
  To enable it, register an SSH key at <https://aur.archlinux.org/account> and
  add `AUR_SSH_PRIVATE_KEY`; the first push creates `nits-bin`, and the
  workflow handles the empty-repo case. The job still renders and validates
  `PKGBUILD`/`.SRCINFO` in a dry run when ticked, so the code path does not rot
  while the channel is unused.
- **Signing key** — generate a dedicated key, no expiry:
  ```console
  $ gpg --batch --passphrase '<passphrase>' --quick-generate-key 'Nits <jjprest@gmail.com>' rsa4096 sign never
  ```
  Publish the fingerprint in the README so people can verify it out of band.
- **gh-pages** — enable Pages for the repo, branch `gh-pages`, root. The
  workflow creates the branch on first run.

## Not yet published

**npm.** `nits` on npm is taken by a dormant `0.0.1` package from 2022, so the
name has to be resolved with its owner (or npm's dispute process) before there
is anything worth wiring up. When it is, the shape is the esbuild/biome one: a
thin JS shim plus one package per platform holding the binary, selected via
`optionalDependencies` with `os`/`cpu` fields — not napi, which would only earn
its keep if we wanted `nits-client-core` as a JavaScript *library*.

**homebrew/core**, which is what `brew install nits` without the tap prefix
would mean. Two things gate it, and the second is not just a matter of waiting:

- Notability — at least 30 forks, 30 watchers or 75 stars on the canonical
  repository, which must also be at least 30 days old.
- Core formulae must **build from source**; ours downloads prebuilt binaries.
  A core formula would be a different formula: the GitHub *source* tarball (not
  the crates.io `.crate`, which contains only the `nits` package and so cannot
  build `nitsd`), `depends_on "rust" => :build`, and a `cargo install` per
  binary.

So this is not "the same formula, later" — budget for writing a second one.

**Debian and Fedora proper.** Not a matter of patience either. Debian forbids
vendoring: a package must build from other Debian packages with no network
access, so every dependency crate needs to exist in the archive as
`librust-*-dev` first. `nits` currently pulls in about 200 crates. That plus a
sponsoring Debian Developer, an ITP bug and the NEW queue makes this a
months-to-years project, not a release chore. The APT/YUM repo above is the
normal answer for software in this position — Docker, the GitHub CLI and
Tailscale all ship the same way.
