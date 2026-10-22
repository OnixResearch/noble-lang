//! Strict, bounded decoding of one untrusted acceptance candidate in the
//! `noble-candidate/v1` wire form, for the WORKER-07 consumer embedder.
//!
//! The document carries only the candidate: its format, semantic revision,
//! node arena and entry body. The consumer selects the expected interface,
//! environment and limits, so any other top-level field is refused rather than
//! read. Every witness is spelled with its kind (`stack`, `value`, `effect` or
//! a `ref` type equation between two variables of the same instantiation);
//! types have no variables, so a type that mentions one is malformed.
//! Decoding never consults the environment: the kernel checker resolves every
//! definition and witness against the consumer's environment.

use noble_kernel::types::{EffId, EffSet, Ty};
use noble_kernel::untrusted::{Candidate, Lit, Node, NodeId};
use noble_kernel::words::{Binding, Inst, Variable};
use serde_json::Value;

/// The candidate wire format this decoder understands.
pub(super) const FORMAT: &str = "noble-candidate/v1";
const FIELDS: [&str; 4] = ["body", "format", "nodes", "semantic_revision"];
const TYPE_DEPTH: usize = 32;

/// Why a candidate document was refused before any checker ran.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refused {
    Invalid(&'static str),
    Unsupported(&'static str),
    Exhausted(&'static str),
}

/// Decode a canonical candidate document. The bounded pre-decode scan runs
/// before serde allocates anything.
pub(super) fn decode(bytes: &[u8]) -> Result<Candidate, Refused> {
    super::worker_program::prescan(bytes).map_err(|(problem, why)| match problem {
        super::worker_program::Refusal::Exhausted => Refused::Exhausted(why),
        super::worker_program::Refusal::Invalid | super::worker_program::Refusal::Unsupported => {
            Refused::Invalid(why)
        }
    })?;
    let document: Value =
        serde_json::from_slice(bytes).map_err(|_| Refused::Invalid("candidate is not JSON"))?;
    if serde_json::to_vec(&document).map_err(|_| Refused::Invalid("candidate encoding failed"))?
        != bytes
    {
        return Err(Refused::Invalid("candidate is not canonical JSON"));
    }
    let fields = document
        .as_object()
        .ok_or(Refused::Invalid("candidate must be an object"))?;
    // Interface, environment, limits or acceptance claims are not candidate
    // fields: the consumer owns them (B-CHECK-07).
    if fields.len() != FIELDS.len() || !FIELDS.iter().all(|key| fields.contains_key(*key)) {
        return Err(Refused::Invalid(
            "candidate carries metadata outside noble-candidate/v1",
        ));
    }
    match document["format"].as_str() {
        Some(FORMAT) => {}
        Some(_) => {
            return Err(Refused::Unsupported(
                "candidate format is not noble-candidate/v1",
            ))
        }
        None => return Err(Refused::Invalid("candidate format must be a string")),
    }
    let revision = number(&document["semantic_revision"])?;
    let entries = document["nodes"]
        .as_array()
        .ok_or(Refused::Invalid("candidate nodes must be an array"))?;
    let mut nodes = Vec::with_capacity(entries.len());
    for entry in entries {
        nodes.push(node(entry)?);
    }
    Ok(Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision,
        nodes,
        body: ids(&document["body"])?,
    })
}

fn number(value: &Value) -> Result<u32, Refused> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .ok_or(Refused::Invalid("expected an unsigned 32-bit integer"))
}

fn ids(value: &Value) -> Result<Vec<NodeId>, Refused> {
    let entries = value
        .as_array()
        .ok_or(Refused::Invalid("a body must be an array of node indices"))?;
    let mut body = Vec::with_capacity(entries.len());
    for entry in entries {
        body.push(NodeId(number(entry)?));
    }
    Ok(body)
}

/// The one key and payload of a single-key object.
fn single(value: &Value) -> Result<(&str, &Value), Refused> {
    let fields = value
        .as_object()
        .ok_or(Refused::Invalid("expected a single-key object"))?;
    let mut entries = fields.iter();
    match (entries.next(), entries.next()) {
        (Some((key, payload)), None) => Ok((key.as_str(), payload)),
        _ => Err(Refused::Invalid("expected a single-key object")),
    }
}

fn exact(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|fields| {
        fields.len() == keys.len() && keys.iter().all(|key| fields.contains_key(*key))
    })
}

fn node(value: &Value) -> Result<Node, Refused> {
    let kind = value["kind"]
        .as_str()
        .ok_or(Refused::Invalid("a node needs a string kind"))?;
    let shaped = match kind {
        "literal" => exact(value, &["inst", "kind", "lit"]),
        "invocation" | "signed-self" => exact(value, &["def", "inst", "kind"]),
        "quotation" => exact(value, &["body", "inst", "kind"]),
        _ => return Err(Refused::Unsupported("node form outside noble-candidate/v1")),
    };
    if !shaped {
        return Err(Refused::Invalid("node fields differ from its kind"));
    }
    let inst = inst(&value["inst"])?;
    Ok(match kind {
        "literal" => Node::Literal {
            lit: lit(&value["lit"])?,
            inst,
        },
        "invocation" => Node::Invocation {
            def: noble_kernel::contracts::Definition(number(&value["def"])?),
            inst,
        },
        "signed-self" => Node::SignedSelf {
            def: noble_kernel::contracts::Definition(number(&value["def"])?),
            inst,
        },
        _ => Node::Quotation {
            body: ids(&value["body"])?,
            inst,
        },
    })
}

fn lit(value: &Value) -> Result<Lit, Refused> {
    let (key, payload) = single(value)?;
    match key {
        "i64" => payload
            .as_i64()
            .map(Lit::I64)
            .ok_or(Refused::Invalid("an i64 literal needs an integer")),
        "bool" => payload
            .as_bool()
            .map(Lit::Bool)
            .ok_or(Refused::Invalid("a bool literal needs a boolean")),
        "text" => payload
            .as_str()
            .map(|_| Lit::Text)
            .ok_or(Refused::Invalid("a text literal needs a string")),
        "unit" if payload.as_object().is_some_and(serde_json::Map::is_empty) => Ok(Lit::Unit),
        _ => Err(Refused::Invalid("unknown literal form")),
    }
}

fn inst(value: &Value) -> Result<Inst, Refused> {
    let entries = value
        .as_array()
        .ok_or(Refused::Invalid("a witness must be an array of bindings"))?;
    let mut bindings = Vec::with_capacity(entries.len());
    for entry in entries {
        let (key, payload) = single(entry)?;
        bindings.push(match key {
            "stack" => Binding::Stack(stack(payload, 0)?),
            "value" => Binding::Value(ty(payload, 0)?),
            "effect" => Binding::Effect(effects(payload)?),
            // A type equation between two variables of this instantiation.
            "ref" => Binding::Ref(Variable(number(payload)?)),
            _ => return Err(Refused::Invalid("unknown witness binding kind")),
        });
    }
    Ok(Inst { bindings })
}

fn effects(value: &Value) -> Result<EffSet, Refused> {
    let entries = value
        .as_array()
        .ok_or(Refused::Invalid("an effect set must be an array"))?;
    let mut ids = Vec::with_capacity(entries.len());
    for entry in entries {
        ids.push(EffId(number(entry)?));
    }
    Ok(EffSet::from_ids(&ids))
}

fn stack(value: &Value, depth: usize) -> Result<Vec<Ty>, Refused> {
    let entries = value
        .as_array()
        .ok_or(Refused::Invalid("a stack must be an array of types"))?;
    let mut types = Vec::with_capacity(entries.len());
    for entry in entries {
        types.push(ty(entry, depth)?);
    }
    Ok(types)
}

#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-maintainers; a candidate type is a finite tree whose depth is bounded twice, by the pre-decode nesting scan and by TYPE_DEPTH here, so the recursion is bounded by the input."
)]
fn ty(value: &Value, depth: usize) -> Result<Ty, Refused> {
    if depth >= TYPE_DEPTH {
        return Err(Refused::Exhausted("candidate type nesting exceeds 32"));
    }
    let (key, payload) = single(value)?;
    let primitive = |ty: Ty| {
        if payload.as_object().is_some_and(serde_json::Map::is_empty) {
            Ok(ty)
        } else {
            Err(Refused::Invalid("a primitive type's payload must be {}"))
        }
    };
    match key {
        "unit" => primitive(Ty::Unit),
        "bool" => primitive(Ty::Bool),
        "i64" => primitive(Ty::I64),
        "text" => primitive(Ty::Text),
        "syntax" => primitive(Ty::Syntax),
        "pair" | "sum" => {
            let parts = payload
                .as_array()
                .filter(|parts| parts.len() == 2)
                .ok_or(Refused::Invalid("pair and sum need exactly two types"))?;
            let left = Box::new(ty(&parts[0], depth + 1)?);
            let right = Box::new(ty(&parts[1], depth + 1)?);
            Ok(if key == "pair" {
                Ty::Pair(left, right)
            } else {
                Ty::Sum(left, right)
            })
        }
        "list" => Ok(Ty::List(Box::new(ty(payload, depth + 1)?))),
        "program" if exact(payload, &["effects", "in", "out"]) => Ok(Ty::program(
            stack(&payload["in"], depth + 1)?,
            stack(&payload["out"], depth + 1)?,
            effects(&payload["effects"])?,
        )),
        // Types have no variables: a self-referential equation such as
        // `x = Pair<x, I64>` cannot be stated, so it is malformed input.
        _ => Err(Refused::Invalid("type form outside noble-candidate/v1")),
    }
}
