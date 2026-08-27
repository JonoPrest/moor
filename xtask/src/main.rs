//! Repo maintenance tasks. `cargo xtask <task>`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

fn main() -> anyhow::Result<()> {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("fixtures") => fixtures(),
        Some(other) => bail!("unknown task {other:?}; available: fixtures"),
        None => bail!("usage: cargo xtask <fixtures>"),
    }
}

fn repo_root() -> anyhow::Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask must live one level below the repo root")
}

/// Write `fixtures/protocol/<Type>/<variant>.json` for every fixture and
/// delete files no fixture produces any more.
fn fixtures() -> anyhow::Result<()> {
    let dir = repo_root()?.join("fixtures").join("protocol");
    std::fs::create_dir_all(&dir)?;

    let mut wanted = BTreeSet::new();
    let mut written = 0usize;
    for f in moor_protocol_fixtures::all()? {
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
        "fixtures: {} total, {written} written, {removed} removed",
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
