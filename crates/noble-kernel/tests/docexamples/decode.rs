//! Decoder for the executed documentation examples (DX-DOC-01).
//!
//! Decodes one fenced `noble-check` example, in the JSON shape documented in
//! [verification/m2-fragment.md] "Executed documentation examples", into the
//! kernel's request and candidate structures against the bootstrap
//! environment. Binding kinds are taken from each word's documented
//! variables, so a wrong-shaped witness is an error, not a guess.

use super::json::Json;
use noble_kernel::contracts::Definition;
use noble_kernel::types::{EffId, EffSet, Ty};
use noble_kernel::untrusted::{Candidate, Limits, Lit, Node, NodeId, Request};
use noble_kernel::words::{Binding, Inst, VariableKind};

/// The literal's documented variables: one stack.
const LITERAL: &[VariableKind] = &[VariableKind::Stack];

/// The quotation's documented variables: `R A C e` (also `reflect`'s).
fn quotation_kinds() -> &'static [VariableKind] {
    use VariableKind::{Effect, Stack};
    &[Stack, Stack, Stack, Effect]
}

/// One decoded example, ready for the actual checker.
pub struct Example {
    pub request: Request,
    pub candidate: Candidate,
}

/// The documented variable kinds of one word, in binding order.
fn def_of(name: &str) -> Result<(u32, &'static [VariableKind]), String> {
    use VariableKind::{Effect, Stack, Value};
    const STACK: &[VariableKind] = &[Stack];
    const STACK_VALUE: &[VariableKind] = &[Stack, Value];
    const STACK_TWO_VALUES: &[VariableKind] = &[Stack, Value, Value];
    const DIP: &[VariableKind] = &[Stack, Value, Stack, Effect];
    const QUOTE: &[VariableKind] = &[Stack, Value, Stack];
    const COMPOSE: &[VariableKind] = &[Stack, Stack, Stack, Stack, Effect, Effect];
    const RUN: &[VariableKind] = &[Stack, Stack, Effect];
    const BRANCH: &[VariableKind] = &[Stack, Stack, Effect, Effect];
    Ok(match name {
        "dup" => (0, STACK_VALUE),
        "drop" => (1, STACK_VALUE),
        "swap" => (2, STACK_TWO_VALUES),
        "dip" => (3, DIP),
        "+" => (4, STACK),
        "-" => (5, STACK),
        "*" => (6, STACK),
        "quote" => (7, QUOTE),
        "compose" => (8, COMPOSE),
        "run" => (9, RUN),
        "reflect" => (10, quotation_kinds()),
        "unit" => (11, STACK),
        "pair" => (12, STACK_TWO_VALUES),
        "unpair" => (13, STACK_TWO_VALUES),
        "inl" => (14, STACK_TWO_VALUES),
        "inr" => (15, STACK_TWO_VALUES),
        "case" => (16, &[]),
        "if" => (17, BRANCH),
        "nil" => (18, STACK_VALUE),
        "cons" => (19, STACK_VALUE),
        "list.case" => (20, &[]),
        "test.emit" => (21, STACK),
        _ => {
            return Err(format!(
                "unknown word `{name}` for the bootstrap environment"
            ))
        }
    })
}

fn single_key(value: &Json) -> Result<(&str, &Json), String> {
    match value {
        Json::Obj(entries) if entries.len() == 1 => Ok((&entries[0].0, &entries[0].1)),
        _ => Err("expected a one-field type object".to_string()),
    }
}

fn ty(value: &Json) -> Result<Ty, String> {
    let (key, payload) = single_key(value)?;
    match key {
        "unit" => Ok(Ty::Unit),
        "bool" => Ok(Ty::Bool),
        "i64" => Ok(Ty::I64),
        "text" => Ok(Ty::Text),
        "syntax" => Ok(Ty::Syntax),
        "pair" | "sum" => {
            let sides = payload.as_arr()?;
            if sides.len() != 2 {
                return Err(format!("`{key}` needs exactly two payloads"));
            }
            let (left, right) = (ty(&sides[0])?, ty(&sides[1])?);
            if key == "pair" {
                Ok(Ty::Pair(Box::new(left), Box::new(right)))
            } else {
                Ok(Ty::Sum(Box::new(left), Box::new(right)))
            }
        }
        "list" => Ok(Ty::List(Box::new(ty(payload)?))),
        "program" => {
            let input = stack(payload.field("in")?)?;
            let output = stack(payload.field("out")?)?;
            Ok(Ty::program(
                input,
                output,
                effect_set(payload.field("effects")?)?,
            ))
        }
        "resource" => match payload.as_str()? {
            "test.counter" => Ok(Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE)),
            other => Err(format!("unknown resource kind `{other}`")),
        },
        _ => Err(format!("unknown type key `{key}`")),
    }
}

fn stack(value: &Json) -> Result<Vec<Ty>, String> {
    value.as_arr()?.iter().map(ty).collect()
}

fn effect_id(value: &Json) -> Result<EffId, String> {
    match value {
        Json::Str(name) if name == "test.emit" => Ok(noble_kernel::contracts::TEST_EMIT),
        Json::Int(id) if *id >= 0 && *id <= u32::MAX as i64 => Ok(EffId(*id as u32)),
        _ => Err("effects are `test.emit` or small integers".to_string()),
    }
}

fn effect_set(value: &Json) -> Result<EffSet, String> {
    let ids: Vec<EffId> = value
        .as_arr()?
        .iter()
        .map(effect_id)
        .collect::<Result<_, _>>()?;
    Ok(EffSet::from_ids(&ids))
}

fn binding(value: &Json, kind: &VariableKind) -> Result<Binding, String> {
    match kind {
        VariableKind::Stack => Ok(Binding::Stack(stack(value)?)),
        VariableKind::Value => Ok(Binding::Value(ty(value)?)),
        VariableKind::Effect => Ok(Binding::Effect(effect_set(value)?)),
    }
}

fn inst(value: &Json, kinds: &[VariableKind]) -> Result<Inst, String> {
    let mut bindings: Vec<Binding> = Vec::with_capacity(kinds.len());
    let mut index = 0;
    while index < kinds.len() {
        let key = format!("v{index}");
        match value.optional(&key)? {
            Some(found) => bindings.push(binding(found, &kinds[index])?),
            None => return Err(format!("witness misses binding `{key}`")),
        }
        index += 1;
    }
    let entries = match value {
        Json::Obj(entries) => entries,
        _ => return Err("a witness must be an object".to_string()),
    };
    for (key, _) in entries {
        let known = key
            .strip_prefix('v')
            .and_then(|rest| rest.parse::<usize>().ok())
            .is_some_and(|position| position < kinds.len());
        if !known {
            return Err(format!("witness carries unknown binding `{key}`"));
        }
    }
    Ok(Inst { bindings })
}

fn lit(value: &Json) -> Result<Lit, String> {
    let (key, payload) = single_key(value)?;
    match key {
        "i64" => Ok(Lit::I64(payload.as_int()?)),
        "bool" => match payload {
            Json::Bool(flag) => Ok(Lit::Bool(*flag)),
            _ => Err("`bool` literal needs a JSON boolean".to_string()),
        },
        "text" => {
            payload.as_str()?;
            Ok(Lit::Text)
        }
        "unit" => Ok(Lit::Unit),
        _ => Err(format!("unknown literal key `{key}`")),
    }
}

fn node(value: &Json) -> Result<Node, String> {
    let kind = value.field("kind")?.as_str()?;
    match kind {
        "literal" => Ok(Node::Literal {
            lit: lit(value.field("lit")?)?,
            inst: inst(value.field("inst")?, LITERAL)?,
        }),
        "invocation" => {
            let (def, kinds) = def_of(value.field("def")?.as_str()?)?;
            Ok(Node::Invocation {
                def: Definition(def),
                inst: inst(value.field("inst")?, kinds)?,
            })
        }
        "quotation" => Ok(Node::Quotation {
            body: body_ids(value.field("body")?)?,
            inst: inst(value.field("inst")?, quotation_kinds())?,
        }),
        _ => Err(format!("unknown node kind `{kind}`")),
    }
}

fn body_ids(value: &Json) -> Result<Vec<NodeId>, String> {
    value
        .as_arr()?
        .iter()
        .map(|entry| {
            let id = entry.as_int()?;
            if id >= 0 && id <= u32::MAX as i64 {
                Ok(NodeId(id as u32))
            } else {
                Err("node references are small integers".to_string())
            }
        })
        .collect()
}

fn limits_from(value: Option<&Json>) -> Result<Limits, String> {
    let mut limits = Limits {
        bytes: 1 << 16,
        nodes: 256,
        depth: 32,
        type_size: 64,
        stack_height: 16,
        work: 100_000,
        diagnostics: 64,
    };
    let Some(found) = value else {
        return Ok(limits);
    };
    for key in [
        "bytes",
        "nodes",
        "depth",
        "type_size",
        "stack_height",
        "work",
        "diagnostics",
    ] {
        if let Some(given) = found.optional(key)? {
            let value = given.as_int()?;
            if value < 0 || value > u32::MAX as i64 {
                return Err(format!("limit `{key}` is out of range"));
            }
            let small = value as u32;
            match key {
                "bytes" => limits.bytes = small,
                "nodes" => limits.nodes = small,
                "depth" => limits.depth = small,
                "type_size" => limits.type_size = small,
                "stack_height" => limits.stack_height = small,
                "work" => limits.work = small,
                _ => limits.diagnostics = small,
            }
        }
    }
    Ok(limits)
}

/// Decode one whole example: request plus candidate.
pub fn decode(value: &Json) -> Result<Example, String> {
    let request_value = value.field("request")?;
    let expected = request_value.field("expected")?;
    let request = Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in: stack(expected.field("in")?)?,
            stack_out: stack(expected.field("out")?)?,
            allowed_effects: effect_set(expected.field("allowed_effects")?)?,
        },
        limits: limits_from(request_value.optional("limits")?)?,
    };
    let candidate_value = value.field("candidate")?;
    let format = match candidate_value.field("format")?.as_str()? {
        "noble-candidate/v0" => 0,
        _ => 1,
    };
    let nodes: Vec<Node> = candidate_value
        .field("nodes")?
        .as_arr()?
        .iter()
        .map(node)
        .collect::<Result<_, _>>()?;
    let candidate = Candidate {
        format,
        revision: 0,
        nodes,
        body: body_ids(candidate_value.field("body")?)?,
    };
    Ok(Example { request, candidate })
}
