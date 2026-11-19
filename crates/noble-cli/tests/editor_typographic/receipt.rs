//! Receipt writer for the typographic experiment.
//!
//! A receipt is produced only from a clean committed tree of this worktree, is
//! written outside the repository and is never overwritten, so every retained
//! receipt names the exact source commit it measured.

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Request {
    path: PathBuf,
    source_commit: String,
}

/// `None` unless the runner explicitly named a receipt output path.
pub fn request() -> Result<Option<Request>, String> {
    let Some(path) = std::env::var_os("NOBLE_EDITOR_TYPOGRAPHIC_RECEIPT") else {
        return Ok(None);
    };
    let source_commit = std::env::var("NOBLE_EDITOR_TYPOGRAPHIC_SOURCE_COMMIT")
        .map_err(|error| format!("source commit is not configured: {error}"))?;
    let source_commit = source_commit.trim().to_owned();
    Ok(Some(Request {
        path: PathBuf::from(path),
        source_commit,
    }))
}

impl Request {
    pub fn source_commit(&self) -> &str {
        &self.source_commit
    }

    pub fn write(&self, receipt: &serde_json::Value) -> Result<(), String> {
        self.verify_source_commit()?;
        let parent = self
            .path
            .parent()
            .ok_or("receipt output has no parent directory")?;
        let parent = std::fs::canonicalize(parent)
            .map_err(|error| format!("canonicalize receipt directory: {error}"))?;
        if parent.starts_with(worktree()?) {
            return Err(String::from("receipt output must be outside the worktree"));
        }
        if self.path.exists() {
            return Err(String::from(
                "a retained receipt is immutable; choose a new path",
            ));
        }
        let bytes = serde_json::to_vec_pretty(receipt)
            .map_err(|error| format!("encode receipt: {error}"))?;
        std::fs::write(&self.path, bytes).map_err(|error| format!("write receipt: {error}"))
    }

    fn verify_source_commit(&self) -> Result<(), String> {
        if self.source_commit.len() != 40
            || !self
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(String::from("source commit must be a full 40-hex SHA"));
        }
        let head = git_output(&["rev-parse", "HEAD"], "read HEAD")?;
        if head.trim() != self.source_commit {
            return Err(format!(
                "the worktree HEAD {} is not the declared source commit {}",
                head.trim(),
                self.source_commit
            ));
        }
        let status = git_output(&["status", "--porcelain"], "read the working tree")?;
        if !status.trim().is_empty() {
            return Err(format!(
                "the worktree is not clean, so no receipt can name this commit: {status}"
            ));
        }
        Ok(())
    }
}

fn worktree() -> Result<PathBuf, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("worktree root unavailable")?;
    std::fs::canonicalize(root).map_err(|error| format!("canonicalize worktree: {error}"))
}

fn git_output(arguments: &[&str], operation: &str) -> Result<String, String> {
    let worktree = worktree()?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&worktree)
        .args(arguments)
        .output()
        .map_err(|error| format!("{operation}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{operation} refused: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("{operation}: {error}"))
}
