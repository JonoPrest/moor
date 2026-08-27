//! Test helpers: build real git repositories in temp dirs.
//!
//! No mocked git — everything runs the `git` binary so the engine is tested
//! against the same objects it will read in production.

pub mod sim;

pub use sim::{Divergence, Peer, Sim};

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use tempfile::TempDir;

/// A real git repository in a temporary directory, deleted on drop.
#[derive(Debug)]
pub struct TestRepo {
    dir: TempDir,
}

/// Fluent builder: `RepoBuilder::new().commit("a", &[("f.txt", "x")]).branch("x").build()`.
#[derive(Debug, Default)]
pub struct RepoBuilder {
    steps: Vec<Step>,
}

#[derive(Debug)]
enum Step {
    Commit {
        message: String,
        files: Vec<(String, Vec<u8>)>,
        remove: Vec<String>,
    },
    Branch(String),
    Checkout(String),
    Tag(String),
    /// Write files without committing (working tree changes).
    Write(Vec<(String, Vec<u8>)>),
    Remove(Vec<String>),
}

/// Build a `&[(path, content)]` list for [`RepoBuilder::commit`].
#[macro_export]
macro_rules! files {
    ($($path:expr => $content:expr),* $(,)?) => {
        &[$(($path, $content.as_ref() as &[u8])),*][..]
    };
}

impl RepoBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add/overwrite `files` and commit with `message`.
    #[must_use]
    pub fn commit(mut self, message: &str, files: &[(&str, &[u8])]) -> Self {
        self.steps.push(Step::Commit {
            message: message.to_owned(),
            files: owned(files),
            remove: vec![],
        });
        self
    }

    /// Remove `paths` and commit with `message`.
    #[must_use]
    pub fn commit_removing(mut self, message: &str, paths: &[&str]) -> Self {
        self.steps.push(Step::Commit {
            message: message.to_owned(),
            files: vec![],
            remove: paths.iter().map(|p| (*p).to_owned()).collect(),
        });
        self
    }

    /// Create a branch at the current HEAD and check it out.
    #[must_use]
    pub fn branch(mut self, name: &str) -> Self {
        self.steps.push(Step::Branch(name.to_owned()));
        self
    }

    #[must_use]
    pub fn checkout(mut self, name: &str) -> Self {
        self.steps.push(Step::Checkout(name.to_owned()));
        self
    }

    #[must_use]
    pub fn tag(mut self, name: &str) -> Self {
        self.steps.push(Step::Tag(name.to_owned()));
        self
    }

    /// Write files to the working tree without staging or committing.
    #[must_use]
    pub fn write(mut self, files: &[(&str, &[u8])]) -> Self {
        self.steps.push(Step::Write(owned(files)));
        self
    }

    /// Delete files from the working tree without committing.
    #[must_use]
    pub fn remove(mut self, paths: &[&str]) -> Self {
        self.steps.push(Step::Remove(
            paths.iter().map(|p| (*p).to_owned()).collect(),
        ));
        self
    }

    pub fn build(self) -> anyhow::Result<TestRepo> {
        let repo = TestRepo::init()?;
        for step in self.steps {
            repo.apply(step)?;
        }
        Ok(repo)
    }
}

fn owned(files: &[(&str, &[u8])]) -> Vec<(String, Vec<u8>)> {
    files
        .iter()
        .map(|(p, c)| ((*p).to_owned(), c.to_vec()))
        .collect()
}

impl TestRepo {
    /// An empty repository with deterministic author/committer identity.
    pub fn init() -> anyhow::Result<Self> {
        let dir = tempfile::Builder::new().prefix("moor-test-").tempdir()?;
        let repo = Self { dir };
        repo.git(&["init", "-q", "-b", "main"])?;
        repo.git(&["config", "user.name", "Test User"])?;
        repo.git(&["config", "user.email", "test@example.com"])?;
        repo.git(&["config", "commit.gpgsign", "false"])?;
        Ok(repo)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Run `git` in the repo and return trimmed stdout.
    pub fn git(&self, args: &[&str]) -> anyhow::Result<String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_DATE", "2024-01-01T00:00:00+00:00")
            .env("GIT_COMMITTER_DATE", "2024-01-01T00:00:00+00:00")
            .output()
            .with_context(|| format!("running git {args:?}"))?;
        if !out.status.success() {
            bail!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8(out.stdout)?.trim_end().to_owned())
    }

    /// Full SHA of `rev`.
    pub fn rev_parse(&self, rev: &str) -> anyhow::Result<String> {
        self.git(&["rev-parse", "--verify", &format!("{rev}^{{commit}}")])
    }

    pub fn write_file(&self, rel: &str, content: &[u8]) -> anyhow::Result<PathBuf> {
        let p = self.path().join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, content)?;
        Ok(p)
    }

    fn apply(&self, step: Step) -> anyhow::Result<()> {
        match step {
            Step::Commit {
                message,
                files,
                remove,
            } => {
                for (p, c) in &files {
                    self.write_file(p, c)?;
                }
                for p in &remove {
                    self.git(&["rm", "-q", p])?;
                }
                if !files.is_empty() {
                    self.git(&["add", "-A"])?;
                }
                self.git(&["commit", "-q", "--allow-empty", "-m", &message])?;
            }
            Step::Branch(name) => {
                self.git(&["checkout", "-q", "-b", &name])?;
            }
            Step::Checkout(name) => {
                self.git(&["checkout", "-q", &name])?;
            }
            Step::Tag(name) => {
                self.git(&["tag", &name])?;
            }
            Step::Write(files) => {
                for (p, c) in &files {
                    self.write_file(p, c)?;
                }
            }
            Step::Remove(paths) => {
                for p in &paths {
                    std::fs::remove_file(self.path().join(p))?;
                }
            }
        }
        Ok(())
    }
}

/// Synthetic repository for benchmarks: `files` small text files spread over
/// `files / 100` directories, committed once on `main`. Paths look like
/// `dir_017/file_0421.txt`.
pub fn synthetic_repo(files: usize) -> anyhow::Result<TestRepo> {
    let repo = TestRepo::init()?;
    for (path, body) in &synthetic_files(files) {
        repo.write_file(path, body.as_bytes())?;
    }
    repo.git(&["add", "-A"])?;
    repo.git(&["commit", "-q", "-m", "synthetic"])?;
    Ok(repo)
}

/// The paths and bodies `synthetic_repo` writes, in order.
#[must_use]
pub fn synthetic_files(files: usize) -> Vec<(String, String)> {
    let dirs = (files / 100).max(1);
    (0..files)
        .map(|i| {
            let path = format!("dir_{:03}/file_{i:05}.txt", i % dirs);
            let body = format!("file {i}\nline two\nline three\n");
            (path, body)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_history_with_branches_and_dirty_tree() {
        let repo = RepoBuilder::new()
            .commit(
                "one",
                files!["a.txt" => "a\n", "src/lib.rs" => "fn main() {}\n"],
            )
            .branch("feature")
            .commit("two", files!["a.txt" => "b\n"])
            .tag("v1")
            .write(files!["untracked.txt" => "u\n"])
            .build()
            .unwrap();

        assert_ne!(
            repo.rev_parse("main").unwrap(),
            repo.rev_parse("feature").unwrap()
        );
        assert_eq!(
            repo.rev_parse("v1").unwrap(),
            repo.rev_parse("feature").unwrap()
        );
        assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]).unwrap(), "2");
        assert_eq!(
            repo.git(&["status", "--porcelain"]).unwrap(),
            "?? untracked.txt"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("a.txt")).unwrap(),
            "b\n"
        );
    }
}
