//! The corpus loop: one test that measures every frozen cell, then assembles
//! the receipt and (when asked) writes it.
//!
//! Nothing here decides a verdict. Each cell either compares what the editor
//! commands printed or asks the cursor for a class the corpus already states,
//! and the test fails on the first cell that disagrees.

use super::corpus::{collect_addresses, decode, decode_node};
use super::navigation::document::encode;
use super::paired::pair;
use super::receipt::{assemble, write, Tally};
use super::transport::{contains, repository_root, same_document, Editor};
use super::{
    attempt, focus, operation, Case, Corpus, Document, Error, RefusalCell, Replacement, Sequence,
    CORPUS_PATH, CRITERIA_PATH, CURSOR_MARKERS, IMPLEMENTATION, RECEIPT_VARIABLE, SHIPPED_CONTROL,
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
    let receipt = assemble(&root, &criteria, &editor, &tally, cells)?;
    if let Ok(destination) = std::env::var(RECEIPT_VARIABLE) {
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
    cells.push(identity_cells(case, &tree, &addresses, tally)?);
    let original = match addresses.first() {
        Some(first) => focus(&tree, first)?.reconstruct(),
        None => tree.clone(),
    };
    cells.push(pair(
        editor,
        &std::format!("original:{}", case.name),
        &original,
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

/// C1: every address reconstructs the original tree, and the cursor reports
/// the address it was given.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the sweep enumerates and encodes trees, which allocates.
)]
fn identity_cells(
    case: &Case,
    tree: &[Node],
    addresses: &[Vec<u32>],
    tally: &mut Tally,
) -> Result<Document, Error> {
    let original = encode(tree);
    for address in addresses {
        let at = focus(tree, address)?;
        if at.node_path() != *address {
            return Err(std::format!(
                "{}: the cursor reported {:?} for {address:?}",
                case.name,
                at.node_path()
            )
            .into());
        }
        if encode(&at.reconstruct()) != original {
            return Err(std::format!(
                "{}: reconstruction at {address:?} is not the original tree",
                case.name
            )
            .into());
        }
        if encode(&at.candidate().nodes) != original {
            return Err(std::format!(
                "{}: the candidate at {address:?} is not the original tree",
                case.name
            )
            .into());
        }
    }
    tally.addresses = tally.addresses.saturating_add(addresses.len());
    Ok(serde_json::json!({
        "cell": std::format!("identity:{}", case.name),
        "origin": case.origin,
        "addresses": addresses.len(),
        "reconstruction": "original",
    }))
}

/// C2: each step reaches its documented address, and the sequence reconstructs
/// the documented tree.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the walk encodes the reconstruction and parses the
    // expected document.
)]
fn sequence_cell(tree: &[Node], case: &Case, sequence: &Sequence) -> Result<Document, Error> {
    if sequence.steps.len() != sequence.addresses.len() {
        return Err(std::format!(
            "{}:{}: steps and addresses differ in length",
            case.name,
            sequence.name
        )
        .into());
    }
    let mut at = focus(tree, &sequence.at)?;
    for (step, address) in sequence.steps.iter().zip(sequence.addresses.iter()) {
        at = attempt(tree, &at.node_path(), operation(&step.op)?).map_err(|refusal| {
            std::format!(
                "{}:{}: {} refused with {}",
                case.name,
                sequence.name,
                step.op,
                refusal.class()
            )
        })?;
        if at.node_path() != *address {
            return Err(std::format!(
                "{}:{}: {} reached {:?}, not {address:?}",
                case.name,
                sequence.name,
                step.op,
                at.node_path()
            )
            .into());
        }
    }
    let expected: Document = match &sequence.tree {
        Some(document) => document.clone(),
        None => serde_json::from_str(&encode(tree))?,
    };
    let reconstructed: Document = serde_json::from_str(&encode(&at.reconstruct()))?;
    same_document(&reconstructed, &expected, &sequence.name)?;
    Ok(serde_json::json!({
        "cell": std::format!("sequence:{}:{}", case.name, sequence.name),
        "steps": sequence.steps.len(),
        "addresses": sequence.addresses,
        "tree": "reconstructed",
    }))
}

/// C3 without the transport half: the replacement equals the independent JSON
/// edit, at the address the corpus states.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the comparison decodes a node, replaces the focus and
    // encodes the result.
)]
fn replacement_cell(
    tree: &[Node],
    case: &Case,
    replacement: &Replacement,
) -> Result<Vec<Node>, Error> {
    let at = focus(tree, &replacement.at)?;
    let replaced = at.replace(decode_node(&replacement.node)?);
    if replaced.node_path() != replacement.at {
        return Err(std::format!(
            "{}:{}: the replacement moved the focus to {:?}",
            case.name,
            replacement.name,
            replaced.node_path()
        )
        .into());
    }
    let produced = replaced.reconstruct();
    let document: Document = serde_json::from_str(&encode(&produced))?;
    same_document(&document, &replacement.tree, &replacement.name)?;
    Ok(produced)
}

/// C5: the documented class, and no editor command while the refusal is
/// produced.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the refusal check accounts editor invocations and formats
    // its diagnostics.
)]
fn refusal_cell(
    editor: &Editor,
    tree: &[Node],
    case: &Case,
    refusal: &RefusalCell,
) -> Result<Document, Error> {
    let before = editor.invocations();
    let outcome = attempt(tree, &refusal.at, operation(&refusal.op)?);
    if editor.invocations() != before {
        return Err(std::format!(
            "{}:{}: a refused move reached an editor command",
            case.name,
            refusal.name
        )
        .into());
    }
    let class = match outcome {
        Ok(_) => {
            return Err(std::format!(
                "{}:{}: {} was not refused",
                case.name,
                refusal.name,
                refusal.op
            )
            .into())
        }
        Err(class) => class,
    };
    if class.class() != refusal.class {
        return Err(std::format!(
            "{}:{}: refused with {}, not {}",
            case.name,
            refusal.name,
            class.class(),
            refusal.class
        )
        .into());
    }
    Ok(serde_json::json!({
        "cell": std::format!("refusal:{}:{}", case.name, refusal.name),
        "class": class.class(),
        "editor_commands": 0,
    }))
}
