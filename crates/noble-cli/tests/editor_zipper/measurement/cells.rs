//! The per-cell checks that do not need the editor commands: identity
//! reconstruction, documented navigation, the replacement's node tree, and the
//! refusal classes.
//!
//! Each of these fails with the first disagreement, so a receipt cell exists
//! only for a corpus entry the cursor actually satisfied.

use super::corpus::decode_node;
use super::navigation::document::encode;
use super::receipt::Tally;
use super::transport::{same_document, Editor};
use super::{attempt, focus, operation, Case, Document, Error, RefusalCell, Replacement, Sequence};
use noble_contracts::source::editor::Node;
use std::vec::Vec;

/// C1: every address reconstructs the original tree, and the cursor reports
/// the address it was given.
#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the sweep encodes trees and compares them, which
    // allocates.
)]
pub(super) fn identity_sweep(
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
pub(super) fn sequence_cell(
    tree: &[Node],
    case: &Case,
    sequence: &Sequence,
) -> Result<Document, Error> {
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
pub(super) fn replacement_cell(
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
pub(super) fn refusal_cell(
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
