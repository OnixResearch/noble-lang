//! Reading the frozen corpus documents into editor nodes.
//!
//! This is the harness's own reading of the corpus JSON, never a second
//! ingress: if it read a tree differently from the transport, the cursor-side
//! document would differ and the report comparison would fail. The address
//! walk and the first-hole address are what the corpus states and what a
//! refusal report must name.

use super::{Document, Error};
use noble_contracts::source::editor::Node;
use std::vec::Vec;

// Justification: the reader walks a runtime JSON document whose shape the
// corpus pins; none of these steps can be `const`.
#[allow(tigerstyle::missing_const_fn)]
pub(super) fn decode(document: &Document) -> Result<Vec<Node>, Error> {
    document
        .get("nodes")
        .and_then(Document::as_array)
        .ok_or_else(|| Error::from("a corpus document needs a nodes array"))
        .and_then(|nodes| decode_list(nodes))
}

#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the list reader allocates the decoded nodes it returns.
)]
fn decode_list(nodes: &[Document]) -> Result<Vec<Node>, Error> {
    let mut decoded = Vec::with_capacity(nodes.len());
    for node in nodes {
        decoded.push(decode_node(node)?);
    }
    Ok(decoded)
}

#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the node reader parses text, allocates a word and can
    // build a nested list, none of which is const, and it is deliberately
    // exhaustive over the five transport kinds so an unknown kind refuses.
)]
pub(super) fn decode_node(value: &Document) -> Result<Node, Error> {
    let object = value
        .as_object()
        .ok_or("a corpus node must be a JSON object")?;
    let kind = object
        .get("kind")
        .and_then(Document::as_str)
        .ok_or("a corpus node needs a string kind")?;
    match kind {
        "integer" => {
            required_fields(object, &["kind", "value"])?;
            let number = object
                .get("value")
                .and_then(Document::as_i64)
                .ok_or("a corpus integer needs an I64 value")?;
            Ok(Node::Integer(number))
        }
        "boolean" => {
            required_fields(object, &["kind", "value"])?;
            let is_true = object
                .get("value")
                .and_then(Document::as_bool)
                .ok_or("a corpus boolean needs a boolean value")?;
            Ok(Node::Boolean(is_true))
        }
        "word" => {
            required_fields(object, &["kind", "value"])?;
            let text = object
                .get("value")
                .and_then(Document::as_str)
                .ok_or("a corpus word needs a text value")?;
            Ok(Node::Word(text.into()))
        }
        "hole" => {
            required_fields(object, &["kind"])?;
            Ok(Node::Hole)
        }
        "quotation" => {
            required_fields(object, &["kind", "nodes"])?;
            let children = object
                .get("nodes")
                .and_then(Document::as_array)
                .ok_or("a corpus quotation needs a nodes array")?;
            Ok(Node::Quotation(decode_list(children)?))
        }
        other => Err(std::format!("unsupported corpus node kind {other}").into()),
    }
}

#[allow(
    tigerstyle::missing_const_fn,
    // Justification: the check compares a runtime JSON field map and formats a
    // refusal, so it is not const.
)]
fn required_fields(
    object: &serde_json::Map<String, Document>,
    expected: &[&str],
) -> Result<(), Error> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("a corpus node has missing or unexpected fields".into());
    }
    Ok(())
}

/// Every address in the tree, in depth-first order.
#[allow(
    tigerstyle::borrowed_argument_types,
    // Justification: the address list grows once per node of the corpus tree,
    // so it cannot be a borrowed slice.
)]
#[allow(
    tigerstyle::no_recursion,
    // Justification: the walk descends one level per quotation and the ingress
    // depth limit bounds that.
)]
fn descend(tree: &[Node], prefix: &[u32], into: &mut Vec<Vec<u32>>) -> Result<(), Error> {
    for (at, node) in tree.iter().enumerate() {
        let mut address = Vec::with_capacity(prefix.len().saturating_add(1));
        address.extend_from_slice(prefix);
        address.push(u32::try_from(at)?);
        into.push(address.clone());
        if let Node::Quotation(children) = node {
            descend(children, &address, into)?;
        }
    }
    Ok(())
}

pub(super) fn collect_addresses(tree: &[Node]) -> Result<Vec<Vec<u32>>, Error> {
    let mut into: Vec<Vec<u32>> = Vec::with_capacity(tree.len());
    descend(tree, &[], &mut into)?;
    Ok(into)
}

/// The `node_path` of the first hole in source order, as the transport names
/// the first hole of a refused tree.
#[allow(
    tigerstyle::no_recursion,
    // Justification: the search descends one level per quotation and the
    // ingress depth limit bounds that.
)]
fn search_hole(tree: &[Node], prefix: &[u32], into: &mut Option<Vec<u32>>) -> Result<(), Error> {
    for (at, node) in tree.iter().enumerate() {
        let mut address = Vec::with_capacity(prefix.len().saturating_add(1));
        address.extend_from_slice(prefix);
        address.push(u32::try_from(at)?);
        match node {
            Node::Hole => {
                if into.is_none() {
                    *into = Some(address);
                }
            }
            Node::Quotation(children) => search_hole(children, &address, into)?,
            Node::Integer(_) | Node::Boolean(_) | Node::Word(_) => {}
        }
    }
    Ok(())
}

pub(super) fn first_hole_address(tree: &[Node]) -> Result<Option<Vec<u32>>, Error> {
    let mut found = None;
    search_hole(tree, &[], &mut found)?;
    Ok(found)
}

pub(super) fn contains_hole(tree: &[Node]) -> Result<bool, Error> {
    Ok(first_hole_address(tree)?.is_some())
}
