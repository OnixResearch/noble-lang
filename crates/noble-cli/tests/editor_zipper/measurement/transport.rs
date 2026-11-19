//! The editor transport as a child process, with a counter that proves a
//! refused move never reaches it.
//!
//! Everything compared in this lane is what these two commands print or write:
//! reports come from their stdout, and the admitted source is the
//! `engine/module-1.noble` file `--emit` writes.

use super::{Document, Error};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) struct Editor {
    binary: PathBuf,
    temporary: PathBuf,
    invocations: std::cell::Cell<usize>,
    next: std::cell::Cell<usize>,
}

pub(super) struct Outcome {
    pub(super) exit: i32,
    pub(super) report: Document,
    pub(super) emitted: Option<Vec<u8>>,
}

impl Editor {
    pub(super) fn new() -> Result<Self, Error> {
        let temporary =
            std::env::temp_dir().join(std::format!("noble-editor-zipper-{}", std::process::id()));
        if temporary.exists() {
            std::fs::remove_dir_all(&temporary)?;
        }
        std::fs::create_dir_all(&temporary)?;
        Ok(Self {
            binary: PathBuf::from(env!("CARGO_BIN_EXE_noble")),
            temporary,
            invocations: std::cell::Cell::new(0),
            next: std::cell::Cell::new(0),
        })
    }

    pub(super) fn invocations(&self) -> usize {
        self.invocations.get()
    }

    pub(super) fn cleanup(&self) -> Result<(), Error> {
        std::fs::remove_dir_all(&self.temporary)?;
        Ok(())
    }

    pub(super) fn run(&self, action: &str, document: &str, write: bool) -> Result<Outcome, Error> {
        let number = self.next.get();
        self.next.set(number.saturating_add(1));
        let input = self.temporary.join(std::format!("input-{number}.json"));
        std::fs::write(&input, document)?;
        let directory = self.temporary.join(std::format!("emit-{number}"));
        let mut command = Command::new(&self.binary);
        command.args(["editor", action]).arg(&input);
        if write {
            command.arg("--emit").arg(&directory);
        }
        let output = command.output()?;
        self.invocations
            .set(self.invocations.get().saturating_add(1));
        std::fs::remove_file(&input)?;
        let text = std::str::from_utf8(&output.stdout)?.trim();
        let report: Document = if text.is_empty() {
            Document::Null
        } else {
            serde_json::from_str(text)?
        };
        let emitted = if write {
            let path = directory.join("engine").join("module-1.noble");
            if path.exists() {
                Some(std::fs::read(path)?)
            } else {
                None
            }
        } else {
            None
        };
        Ok(Outcome {
            exit: output.status.code().unwrap_or(-1),
            report,
            emitted,
        })
    }
}

#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the readers clone a parsed report field, which allocates.
)]
pub(super) fn field(report: &Document, name: &str) -> Document {
    report.get(name).cloned().unwrap_or(Document::Null)
}

#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the reader clones a parsed failure field, which allocates.
)]
pub(super) fn failure_field(report: &Document, name: &str) -> Document {
    report
        .get("failure")
        .and_then(|failure| failure.get(name))
        .cloned()
        .unwrap_or(Document::Null)
}

pub(super) fn verdict(outcome: &Outcome) -> Document {
    serde_json::json!({
        "exit": outcome.exit,
        "stage": field(&outcome.report, "stage"),
        "outcome": field(&outcome.report, "outcome"),
        "category": failure_field(&outcome.report, "category"),
    })
}

pub(super) fn digest(report: &Document, name: &str) -> Document {
    report
        .get("module")
        .and_then(|module| module.get(name))
        .cloned()
        .unwrap_or(Document::Null)
}

#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the comparison formats both documents into its refusal.
)]
pub(super) fn same_document(left: &Document, right: &Document, what: &str) -> Result<(), Error> {
    if left == right {
        return Ok(());
    }
    Err(std::format!("{what} differs: cursor {left} edit {right}").into())
}

pub(super) fn repository_root() -> Result<PathBuf, Error> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(root.canonicalize()?)
}

pub(super) fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

pub(super) fn sha256(path: &Path) -> Result<String, Error> {
    let output = Command::new("sha256sum").arg(path).output()?;
    let text = std::str::from_utf8(&output.stdout)?;
    let digest = text
        .split_whitespace()
        .next()
        .ok_or("sha256sum printed no digest")?;
    if digest.len() != 64 {
        return Err(std::format!("sha256sum printed no digest for {}", path.display()).into());
    }
    Ok(digest.to_owned())
}

pub(super) fn git(root: &Path, arguments: &[&str]) -> Result<String, Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(std::format!("git {arguments:?} failed").into());
    }
    Ok(std::str::from_utf8(&output.stdout)?.to_owned())
}

#[allow(
    tigerstyle::foreign_enum_catch_all,
    // Justification: a missing tool version is recorded as an absent value
    // rather than failing a measurement that does not depend on it.
)]
pub(super) fn tool_version(tool: &str) -> Document {
    match Command::new(tool).arg("-V").output() {
        Ok(output) if output.status.success() => match std::str::from_utf8(&output.stdout) {
            Ok(text) => Document::String(text.trim().to_owned()),
            Err(_) => Document::Null,
        },
        _ => Document::Null,
    }
}
