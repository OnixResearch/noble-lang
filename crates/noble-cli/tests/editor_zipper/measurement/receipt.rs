//! Assembling the receipt, and writing it only from a clean tree.
//!
//! The receipt records what the measurement observed and the bytes it ran: the
//! source commit and tree, the digests of the corpus, the criteria and the
//! implementation files, the toolchain, the command, and every cell.

use super::transport::{field, git, sha256, tool_version, Editor};
use super::{Document, Error, CORPUS_PATH, CRITERIA_PATH, IMPLEMENTATION, RECEIPT_VARIABLE};
use std::path::{Path, PathBuf};
use std::vec::Vec;

#[derive(Default)]
pub(super) struct Tally {
    pub(super) cases: usize,
    pub(super) addresses: usize,
    pub(super) sequences: usize,
    pub(super) replacements: usize,
    pub(super) refusals: usize,
    pub(super) pairs: usize,
}

/// The receipt body: every count comes from the measurement that just ran.
pub(super) fn assemble(
    root: &Path,
    criteria: &Document,
    editor: &Editor,
    tally: &Tally,
    cells: Vec<Document>,
) -> Result<Document, Error> {
    Ok(serde_json::json!({
        "schema": "noble-editor-zipper-receipt/v1",
        "issue": 142,
        "lane": "verification/editor-zipper",
        "criteria": {
            "path": CRITERIA_PATH,
            "sha256": sha256(&root.join(CRITERIA_PATH))?,
            "frozen": field(criteria, "frozen"),
            "go_rule": field(criteria, "go_rule"),
        },
        "corpus": {
            "path": CORPUS_PATH,
            "sha256": sha256(&root.join(CORPUS_PATH))?,
        },
        "implementation": sources(root)?,
        "source": {
            "commit": git(root, &["rev-parse", "HEAD"])?.trim().to_owned(),
            "tree": git(root, &["rev-parse", "HEAD^{tree}"])?.trim().to_owned(),
            "clean": git(root, &["status", "--porcelain"])?.trim().is_empty(),
        },
        "toolchain": {
            "rustc": tool_version("rustc"),
            "cargo": tool_version("cargo"),
        },
        "command": std::env::args().collect::<Vec<std::string::String>>(),
        "summary": {
            "cases": tally.cases,
            "addresses": tally.addresses,
            "sequences": tally.sequences,
            "replacements": tally.replacements,
            "refusals": tally.refusals,
            "verdict_pairs": tally.pairs,
            "editor_commands": editor.invocations(),
            "mismatches": 0,
        },
        "cells": cells,
        "non_claims": [
            "a passing corpus is not a theorem, a refinement or a model-to-implementation correspondence",
            "the cited paper's cost bounds, completeness results and timings are the authors' claims and are not measured or gated here",
            "the cursor is measured only over this frozen corpus of five node kinds; nothing here claims coverage of another editor tree",
            "the receipt compares verdicts of the unchanged editor transport; it does not establish anything about the kernel, the wasm backend or admission semantics beyond those comparisons",
        ],
    }))
}

/// The implementation files the receipt binds, each with its digest.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the listing hashes every file, which reads and allocates.
)]
fn sources(root: &Path) -> Result<Vec<Document>, Error> {
    let mut listed: Vec<Document> = Vec::with_capacity(IMPLEMENTATION.len());
    for path in IMPLEMENTATION {
        listed.push(serde_json::json!({
            "path": path,
            "sha256": sha256(&root.join(path))?,
        }));
    }
    Ok(listed)
}

/// Write the receipt where the caller asked, refusing anything but an
/// absolute path outside the repository and a clean source tree.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the writer reads the tree status, serializes the receipt
    // and writes a file.
)]
pub(super) fn write(root: &Path, destination: &str, receipt: &Document) -> Result<(), Error> {
    let path = PathBuf::from(destination);
    if !path.is_absolute() || path.starts_with(root) {
        return Err(std::format!(
            "{RECEIPT_VARIABLE} must be an absolute path outside the repository"
        )
        .into());
    }
    if !git(root, &["status", "--porcelain"])?.trim().is_empty() {
        return Err("a receipt must come from a clean commit".into());
    }
    let text = serde_json::to_string_pretty(receipt)?;
    std::fs::write(&path, std::format!("{text}\n"))?;
    Ok(())
}
