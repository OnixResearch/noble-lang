//! The corpus loop: one test that measures every frozen cell, then assembles
//! the receipt and (when asked) writes it.
//!
//! Nothing here decides a verdict. Each cell either compares what the editor
//! commands printed or asks the cursor for a class the corpus already states,
//! and the test fails on the first cell that disagrees.

use super::cells::{identity_sweep, refusal_cell, replacement_cell, sequence_cell};
use super::corpus::{collect_addresses, decode};
use super::paired::pair;
use super::receipt::{assemble, write, Tally};
use super::transport::{contains, repository_root, Editor};
use super::{
    focus, Case, Corpus, Document, Error, CORPUS_PATH, CRITERIA_PATH, CURSOR_MARKERS,
    IMPLEMENTATION, RECEIPT_VARIABLE, SHIPPED_CONTROL,
};
use noble_contracts::source::editor::Node;
use std::path::Path;
use std::vec::Vec;

const EXPECTED_CASES: usize = 12;

#[test]
fn cursor_corpus_cells() -> Result<(), Error> {
    let root = repository_root()?;
    let corpus: Corpus = serde_json::from_str(&std::fs::read_to_string(root.join(CORPUS_PATH))?)?;
    let criteria: Document =
        serde_json::from_str(&std::fs::read_to_string(root.join(CRITERIA_PATH))?)?;
    let editor = Editor::new()?;
    let mut tally = Tally {
        cases: corpus.cases.len(),
        ..Tally::default()
    };
    let mut cells: Vec<Document> = Vec::with_capacity(corpus.cases.len());
    cells.push(shipped_probe(&root)?);
    for case in &corpus.cases {
        let produced = case_cells(&editor, case, &mut tally)?;
        cells.extend(produced);
    }
    if corpus.cases.len() != EXPECTED_CASES {
        return Err(std::format!("the frozen corpus has {} cases", corpus.cases.len()).into());
    }
    if tally.refusals == 0 || tally.sequences == 0 || tally.replacements == 0 {
        return Err("the frozen corpus left a cell family unexercised".into());
    }
    if let Ok(destination) = std::env::var(RECEIPT_VARIABLE) {
        // Receipt metadata (git commit and tree, file digests) needs `git` and
        // `sha256sum`; the cells above are the measurement and run everywhere,
        // including the hermetic flake sandbox, which has no `git`.
        let receipt = assemble(&root, &criteria, &editor, &tally, cells)?;
        write(&root, &destination, &receipt)?;
    }
    editor.cleanup()?;
    Ok(())
}

/// C6 on the artifact itself: the shipped binary carries no cursor code, and
/// the control proves the search read the real editor CLI.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the probe reads two files and builds a report, which
    // allocates.
)]
fn shipped_probe(root: &Path) -> Result<Document, Error> {
    let shipped = std::fs::read(env!("CARGO_BIN_EXE_noble"))?;
    if !contains(&shipped, SHIPPED_CONTROL.as_bytes()) {
        return Err(
            "the searched artifact is not the shipped editor CLI: its own schema literal is absent"
                .into(),
        );
    }
    let module = std::fs::read_to_string(root.join(IMPLEMENTATION[0]))?;
    let mut markers = serde_json::Map::new();
    for marker in CURSOR_MARKERS {
        if !module.contains(marker) {
            return Err(std::format!("the cursor module does not spell {marker}").into());
        }
        if contains(&shipped, marker.as_bytes()) {
            return Err(
                std::format!("the shipped binary contains the cursor marker {marker}").into(),
            );
        }
        markers.insert(marker.to_owned(), serde_json::json!(0));
    }
    Ok(serde_json::json!({
        "cell": "default-off:shipped-binary",
        "declaration": "#[cfg(test)] mod zipper;",
        "markers": Document::Object(markers),
        "control": SHIPPED_CONTROL,
        "cursor_code_in_shipped_artifact": false,
    }))
}

/// Every cell of one corpus case, in the order the criteria name them.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the case runner reads the frozen documents, drives the
    // editor commands and collects report cells.
)]
fn case_cells(editor: &Editor, case: &Case, tally: &mut Tally) -> Result<Vec<Document>, Error> {
    let tree = decode(&case.tree)?;
    let addresses = collect_addresses(&tree)?;
    let mut cells: Vec<Document> = Vec::with_capacity(
        case.sequences
            .len()
            .saturating_add(case.replacements.len())
            .saturating_add(case.refusals.len())
            .saturating_add(2),
    );
    cells.push(identity_sweep(case, &tree, &addresses, tally)?);
    cells.push(pair(
        editor,
        &std::format!("original:{}", case.name),
        &original(&tree, &addresses),
        &case.tree.to_string(),
    )?);
    tally.pairs = tally.pairs.saturating_add(1);
    for sequence in &case.sequences {
        cells.push(sequence_cell(&tree, case, sequence)?);
        tally.sequences = tally.sequences.saturating_add(1);
    }
    for replacement in &case.replacements {
        let produced = replacement_cell(&tree, case, replacement)?;
        cells.push(pair(
            editor,
            &std::format!("replacement:{}:{}", case.name, replacement.name),
            &produced,
            &replacement.tree.to_string(),
        )?);
        tally.replacements = tally.replacements.saturating_add(1);
        tally.pairs = tally.pairs.saturating_add(1);
    }
    for refusal in &case.refusals {
        cells.push(refusal_cell(editor, &tree, case, refusal)?);
        tally.refusals = tally.refusals.saturating_add(1);
    }
    Ok(cells)
}

/// The tree the cursor reconstructs from the first root address, or the
/// decoded tree when the root list is empty and there is no node to focus.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the reconstruction clones the node list, which
    // allocates.
)]
fn original(tree: &[Node], addresses: &[Vec<u32>]) -> Vec<Node> {
    match addresses.first() {
        Some(first) => match focus(tree, first) {
            Ok(cursor) => cursor.reconstruct(),
            Err(_) => tree.to_vec(),
        },
        None => tree.to_vec(),
    }
}
