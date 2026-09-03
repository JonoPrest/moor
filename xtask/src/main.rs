//! Repo maintenance tasks. `cargo xtask <task>`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

mod release;

use release::Releasable;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fixtures") => fixtures(),
        Some("release-plan") => release::plan(package_arg(&args[1..])?),
        Some("publish") => release::publish(
            package_arg(&args[1..])?,
            args[1..].iter().any(|a| a == "--dry-run"),
        ),
        Some(other) => bail!("unknown task {other:?}; available: fixtures, release-plan, publish"),
        None => bail!("usage: cargo xtask <fixtures|release-plan|publish>"),
    }
}

/// Read the required `--package <name>` flag shared by the release tasks.
fn package_arg(args: &[String]) -> anyhow::Result<Releasable> {
    let name = args
        .iter()
        .position(|a| a == "--package" || a == "-p")
        .and_then(|i| args.get(i + 1))
        .context("missing `--package nits`")?;
    Releasable::parse(name)
}

fn repo_root() -> anyhow::Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask must live one level below the repo root")
}

/// Write `fixtures/protocol/` and `fixtures/client/` and delete files no
/// fixture produces any more.
fn fixtures() -> anyhow::Result<()> {
    write_fixtures("protocol", nits_protocol_fixtures::all()?)?;
    write_fixtures("client", nits_client_core_fixtures::all()?)
}

/// Write `fixtures/<set>/<Type>/<variant>.json` for every fixture in `all`.
fn write_fixtures(set: &str, all: Vec<nits_protocol_fixtures::Fixture>) -> anyhow::Result<()> {
    let dir = repo_root()?.join("fixtures").join(set);
    std::fs::create_dir_all(&dir)?;

    let mut wanted = BTreeSet::new();
    let mut written = 0usize;
    for f in all {
        let path = dir.join(f.rel_path());
        wanted.insert(path.clone());
        let parent = path
            .parent()
            .with_context(|| format!("{} has no parent directory", path.display()))?;
        std::fs::create_dir_all(parent)?;
        let mut json = serde_json::to_string_pretty(&f.value)?;
        json.push('\n');
        let current = std::fs::read_to_string(&path).ok();
        if current.as_deref() != Some(json.as_str()) {
            std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
            written += 1;
        }
    }

    let mut removed = 0usize;
    for entry in walk(&dir)? {
        if entry.extension().is_some_and(|e| e == "json") && !wanted.contains(&entry) {
            std::fs::remove_file(&entry)?;
            removed += 1;
        }
    }
    for entry in std::fs::read_dir(&dir)? {
        let p = entry?.path();
        if p.is_dir() && std::fs::read_dir(&p)?.next().is_none() {
            std::fs::remove_dir(&p)?;
        }
    }

    println!(
        "fixtures/{set}: {} total, {written} written, {removed} removed",
        wanted.len()
    );
    Ok(())
}

fn walk(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let p = entry?.path();
        if p.is_dir() {
            out.extend(walk(&p)?);
        } else {
            out.push(p);
        }
    }
    Ok(out)
}
