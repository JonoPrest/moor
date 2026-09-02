//! Release tasks: work out what a given binary's release needs, and publish
//! the crates it is built from, in dependency order.
//!
//! `cargo xtask release-plan --package nits` prints the plan as JSON (consumed
//! by `.github/workflows/release.yml`); `cargo xtask publish --package nits`
//! carries it out. Both run locally, so a release can be rehearsed off CI.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use anyhow::{Context, bail};

/// A binary Nits ships to registries. Every release names one of these; adding
/// a target is a variant here and nowhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Releasable {
    /// The `nits` CLI — the flagship crate, named `nits` on crates.io.
    Cli,
    /// The `nitsd` daemon.
    Daemon,
    /// The `nits-mcp` MCP stdio shim.
    Mcp,
}

impl Releasable {
    /// The crate name, which is also the crates.io name and the binary name.
    pub fn crate_name(self) -> &'static str {
        match self {
            Self::Cli => "nits",
            Self::Daemon => "nitsd",
            Self::Mcp => "nits-mcp",
        }
    }

    /// Parse the `--package` argument. Untrusted input becomes a variant once,
    /// here, so every later match is exhaustive.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "nits" => Ok(Self::Cli),
            "nitsd" => Ok(Self::Daemon),
            "nits-mcp" => Ok(Self::Mcp),
            other => bail!("unknown package {other:?}; expected one of: nits, nitsd, nits-mcp"),
        }
    }
}

/// A workspace member as `cargo metadata` reports it.
#[derive(Debug)]
struct Member {
    name: String,
    version: String,
    /// The crate's directory, relative to the repo root. cargo-generate-rpm
    /// wants a path rather than a package name.
    dir: String,
    /// False for `publish = false` crates, which cargo strips from a published
    /// manifest and which therefore never need publishing themselves.
    publish: bool,
    /// Workspace crates this one depends on, in any kind (normal, build or
    /// dev). Dev-dependencies count: a path dev-dependency that carries a
    /// version stays in the published manifest and has to resolve.
    deps: BTreeSet<String>,
}

/// Read the workspace graph. `--no-deps` keeps this to our own crates.
fn members() -> anyhow::Result<BTreeMap<String, Member>> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .context("running `cargo metadata`")?;
    if !out.status.success() {
        bail!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let root = meta
        .get("workspace_root")
        .and_then(serde_json::Value::as_str)
        .context("cargo metadata has no `workspace_root`")?
        .to_owned();
    let packages = meta
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .context("cargo metadata has no `packages` array")?;

    let names: BTreeSet<&str> = packages
        .iter()
        .filter_map(|p| p.get("name")?.as_str())
        .collect();

    let mut graph = BTreeMap::new();
    for p in packages {
        let name = p
            .get("name")
            .and_then(serde_json::Value::as_str)
            .context("cargo metadata reported a package without a name")?;
        let version = p
            .get("version")
            .and_then(serde_json::Value::as_str)
            .with_context(|| format!("{name} has no version"))?;
        // `publish` is null when unrestricted, and an array (empty, for
        // `publish = false`) when restricted.
        let publish = match p.get("publish") {
            None | Some(serde_json::Value::Null) => true,
            Some(serde_json::Value::Array(allowed)) => !allowed.is_empty(),
            Some(other) => bail!("{name} has an unexpected `publish` value: {other}"),
        };
        let manifest = p
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .with_context(|| format!("{name} has no manifest_path"))?;
        let dir = std::path::Path::new(manifest)
            .parent()
            .and_then(|d| d.strip_prefix(&root).ok())
            .map(|d| d.display().to_string())
            .with_context(|| format!("{name} is not under the workspace root"))?;
        let deps = p
            .get("dependencies")
            .and_then(serde_json::Value::as_array)
            .map(|ds| {
                ds.iter()
                    .filter_map(|d| d.get("name")?.as_str())
                    .filter(|d| names.contains(d))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        graph.insert(
            name.to_owned(),
            Member {
                name: name.to_owned(),
                version: version.to_owned(),
                dir,
                publish,
                deps,
            },
        );
    }
    Ok(graph)
}

/// Depth-first post-order over the dependency closure of `root`: a crate is
/// emitted only after everything it depends on, which is exactly the order
/// `cargo publish` needs.
///
/// Edges into `publish = false` crates are not followed. Cargo strips those
/// path dependencies when it packages, so they neither need publishing nor
/// pull anything else in — and not following them is what keeps the legitimate
/// dev-dependency cycles (`nits-client-core` ↔ `nits-test-support`) out of the
/// traversal, leaving the cycle check to catch only real ones.
fn publish_order(graph: &BTreeMap<String, Member>, root: &str) -> anyhow::Result<Vec<String>> {
    let mut order = Vec::new();
    let mut done = BTreeSet::new();
    let mut on_stack = BTreeSet::new();
    visit(graph, root, &mut order, &mut done, &mut on_stack)?;
    Ok(order)
}

fn visit(
    graph: &BTreeMap<String, Member>,
    name: &str,
    order: &mut Vec<String>,
    done: &mut BTreeSet<String>,
    on_stack: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    if done.contains(name) {
        return Ok(());
    }
    if !on_stack.insert(name.to_owned()) {
        bail!("dependency cycle through {name}");
    }
    let member = graph
        .get(name)
        .with_context(|| format!("{name} is not a workspace member"))?;
    for dep in &member.deps {
        if graph.get(dep).is_some_and(|d| d.publish) {
            visit(graph, dep, order, done, on_stack)?;
        }
    }
    on_stack.remove(name);
    done.insert(name.to_owned());
    if member.publish {
        order.push(member.name.clone());
    }
    Ok(())
}

/// Whether crates.io already serves this exact `name@version`. Shelling out to
/// `curl` keeps xtask free of an HTTP stack; crates.io requires a User-Agent.
fn is_published(name: &str, version: &str) -> anyhow::Result<bool> {
    let url = format!("https://crates.io/api/v1/crates/{name}/{version}");
    let out = Command::new("curl")
        .args([
            "--silent",
            "--location",
            "--user-agent",
            "nits-xtask (https://github.com/JonoPrest/nits)",
            "--output",
            "/dev/null",
            "--write-out",
            "%{http_code}",
            &url,
        ])
        .output()
        .context("running `curl` to query crates.io")?;
    if !out.status.success() {
        bail!("could not reach crates.io for {name}@{version}");
    }
    match String::from_utf8_lossy(&out.stdout).trim() {
        "200" => Ok(true),
        "404" => Ok(false),
        other => bail!("crates.io returned {other} for {name}@{version}"),
    }
}

/// The git tag a release claims. Per-package, so `nits` and `nitsd` version
/// independently.
fn tag_for(package: &str, version: &str) -> String {
    format!("{package}-v{version}")
}

/// Whether `tag` already exists in this repository.
fn tag_exists(tag: &str) -> anyhow::Result<bool> {
    let out = Command::new("git")
        .args(["tag", "--list", tag])
        .output()
        .context("running `git tag --list`")?;
    if !out.status.success() {
        bail!("`git tag --list` failed");
    }
    Ok(!String::from_utf8_lossy(&out.stdout).trim().is_empty())
}

/// Print the release plan as JSON. `release.yml` reads `version`, `tag` and
/// `collision` from this to decide whether the run may proceed.
pub fn plan(package: Releasable) -> anyhow::Result<()> {
    let graph = members()?;
    let root = package.crate_name();
    let member = graph
        .get(root)
        .with_context(|| format!("{root} is not a workspace member"))?;
    let version = member.version.clone();
    let dir = member.dir.clone();
    let tag = tag_for(root, &version);

    let mut crates = Vec::new();
    for name in publish_order(&graph, root)? {
        let member = graph
            .get(&name)
            .with_context(|| format!("{name} vanished from the graph"))?;
        crates.push(serde_json::json!({
            "name": member.name,
            "version": member.version,
            "already_published": is_published(&member.name, &member.version)?,
        }));
    }

    // The release crate itself already being on crates.io, or the tag already
    // existing, means this version was released: bump before re-running.
    let root_published = crates
        .iter()
        .find(|c| c["name"] == root)
        .and_then(|c| c["already_published"].as_bool())
        .unwrap_or(false);
    let tag_taken = tag_exists(&tag)?;

    let plan = serde_json::json!({
        "package": root,
        "version": version,
        "dir": dir,
        "tag": tag,
        "crates": crates,
        "collision": root_published || tag_taken,
        "collision_reason": match (root_published, tag_taken) {
            (true, true) => format!("{root}@{version} is on crates.io and tag {tag} exists"),
            (true, false) => format!("{root}@{version} is already on crates.io"),
            (false, true) => format!("tag {tag} already exists"),
            (false, false) => String::new(),
        },
    });
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}

/// Publish every crate in the closure that crates.io does not already have,
/// in dependency order. Idempotent: a re-run after a partial failure resumes.
///
/// A dry run cannot verify everything. Cargo resolves a package's dependency
/// versions against the registry index before it will even build the `.crate`,
/// so a crate whose workspace dependencies are not yet published is
/// unverifiable until they are — in a real run they are, one step earlier in
/// this same loop. Those crates are reported as deferred rather than failing
/// the run, which is the difference between a dry run that means something and
/// one that always goes red on a first release.
pub fn publish(package: Releasable, dry_run: bool) -> anyhow::Result<()> {
    let graph = members()?;
    let root = package.crate_name();
    for name in publish_order(&graph, root)? {
        let member = graph
            .get(&name)
            .with_context(|| format!("{name} vanished from the graph"))?;
        if is_published(&member.name, &member.version)? {
            println!(
                "skip {}@{} — already on crates.io",
                member.name, member.version
            );
            continue;
        }
        if dry_run && let Some(blocker) = unpublished_dep(&graph, member)? {
            println!(
                "defer {}@{} — cannot be verified until {blocker} is on crates.io",
                member.name, member.version
            );
            continue;
        }
        println!("publish {}@{}", member.name, member.version);
        let mut cmd = Command::new("cargo");
        cmd.args(["publish", "--package", &member.name, "--locked"]);
        if dry_run {
            cmd.arg("--dry-run");
        }
        let status = cmd
            .status()
            .with_context(|| format!("running `cargo publish -p {}`", member.name))?;
        if !status.success() {
            bail!("`cargo publish -p {}` failed", member.name);
        }
    }
    Ok(())
}

/// The first publishable workspace dependency of `member` that crates.io does
/// not yet have, if any.
fn unpublished_dep(
    graph: &BTreeMap<String, Member>,
    member: &Member,
) -> anyhow::Result<Option<String>> {
    for dep in &member.deps {
        let Some(dep) = graph.get(dep).filter(|d| d.publish) else {
            continue;
        };
        if !is_published(&dep.name, &dep.version)? {
            return Ok(Some(dep.name.clone()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, deps: &[&str], publish: bool) -> (String, Member) {
        (
            name.to_owned(),
            Member {
                name: name.to_owned(),
                version: "0.1.0".to_owned(),
                dir: format!("crates/{name}"),
                publish,
                deps: deps.iter().map(|d| (*d).to_owned()).collect(),
            },
        )
    }

    #[test]
    fn every_releasable_name_round_trips() {
        for r in [Releasable::Cli, Releasable::Daemon, Releasable::Mcp] {
            assert_eq!(Releasable::parse(r.crate_name()).expect("parses"), r);
        }
    }

    #[test]
    fn unknown_package_is_rejected() {
        assert!(Releasable::parse("moor").is_err());
    }

    #[test]
    fn dependencies_are_published_before_their_dependents() {
        let graph: BTreeMap<_, _> = [
            member("nits", &["nitsd", "nits-protocol"], true),
            member("nitsd", &["nits-protocol"], true),
            member("nits-protocol", &[], true),
        ]
        .into_iter()
        .collect();

        let order = publish_order(&graph, "nits").expect("orders");
        assert_eq!(order, ["nits-protocol", "nitsd", "nits"]);
    }

    #[test]
    fn unpublishable_crates_are_not_published_and_pull_in_nothing() {
        // Cargo strips a `publish = false` path dependency, so neither it nor
        // anything only it needs has to be on crates.io.
        let graph: BTreeMap<_, _> = [
            member("nits", &["nits-test-support", "nits-protocol"], true),
            member("nits-test-support", &["some-private-only-dep"], false),
            member("some-private-only-dep", &[], true),
            member("nits-protocol", &[], true),
        ]
        .into_iter()
        .collect();

        let order = publish_order(&graph, "nits").expect("orders");
        assert_eq!(order, ["nits-protocol", "nits"]);
    }

    #[test]
    fn a_dev_dependency_cycle_through_a_private_crate_is_not_a_cycle() {
        // The real shape: nits-client-core dev-depends on nits-test-support,
        // which depends back on nits-client-core. Cargo strips the edge.
        let graph: BTreeMap<_, _> = [
            member("nits-client-core", &["nits-test-support"], true),
            member("nits-test-support", &["nits-client-core"], false),
        ]
        .into_iter()
        .collect();

        let order = publish_order(&graph, "nits-client-core").expect("orders");
        assert_eq!(order, ["nits-client-core"]);
    }

    #[test]
    fn a_cycle_is_an_error_not_a_hang() {
        let graph: BTreeMap<_, _> = [member("a", &["b"], true), member("b", &["a"], true)]
            .into_iter()
            .collect();

        assert!(publish_order(&graph, "a").is_err());
    }

    #[test]
    fn tags_are_namespaced_per_package() {
        assert_eq!(tag_for("nits", "0.2.0"), "nits-v0.2.0");
        assert_eq!(tag_for("nitsd", "0.2.0"), "nitsd-v0.2.0");
    }
}
