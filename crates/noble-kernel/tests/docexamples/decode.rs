//! Decoder for the executed documentation examples (DX-DOC-01).
//!
//! Decodes one fenced `noble-check` example, in the documented JSON shape,
//! into the kernel's request and candidate structures against the bootstrap
//! environment. Binding kinds come from each word's documented variables,
//! so a wrong-shaped witness is an error, not a guess.

#[path = "shapes.rs"]
mod shapes;
#[path = "vocab.rs"]
mod vocab;

/// One decoded example, ready for the actual checker.
pub(super) struct Example {
    pub(super) request: noble_kernel::untrusted::Request,
    pub(super) candidate: noble_kernel::untrusted::Candidate,
}

fn binding(
    value: &crate::value::Json,
    kind: &noble_kernel::words::VariableKind,
) -> Result<noble_kernel::words::Binding, String> {
    match kind {
        noble_kernel::words::VariableKind::Stack => {
            Ok(noble_kernel::words::Binding::Stack(shapes::stack(value)?))
        }
        noble_kernel::words::VariableKind::Value => {
            Ok(noble_kernel::words::Binding::Value(shapes::ty(value)?))
        }
        noble_kernel::words::VariableKind::Effect => Ok(noble_kernel::words::Binding::Effect(
            shapes::effect_set(value)?,
        )),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; inst validates every declared binding and rejects missing, unknown or wrongly shaped witness entries with Result errors, not parser panics."
)]
fn inst(
    value: &crate::value::Json,
    kinds: &[noble_kernel::words::VariableKind],
) -> Result<noble_kernel::words::Inst, String> {
    let mut bindings = Vec::with_capacity(kinds.len());
    for (index, kind) in kinds.iter().enumerate() {
        let key = format!("v{index}");
        match value.optional(&key)? {
            Some(found) => bindings.push(binding(found, kind)?),
            None => return Err(format!("witness misses binding `{key}`")),
        }
    }
    let entries = match value {
        crate::value::Json::Obj(entries) => entries,
        crate::value::Json::Null
        | crate::value::Json::Bool(_)
        | crate::value::Json::Int(_)
        | crate::value::Json::Float(_)
        | crate::value::Json::Str(_)
        | crate::value::Json::Arr(_) => return Err("a witness must be an object".to_string()),
    };
    for (key, _) in entries {
        let is_known = key
            .strip_prefix('v')
            .and_then(|rest| rest.parse::<usize>().ok())
            .is_some_and(|position| position < kinds.len());
        if !is_known {
            return Err(format!("witness carries unknown binding `{key}`"));
        }
    }
    Ok(noble_kernel::words::Inst { bindings })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; lit returns errors for unknown literal tags and malformed payloads; asserting those input conditions would change the decoder's failure contract."
)]
fn lit(value: &crate::value::Json) -> Result<noble_kernel::untrusted::Lit, String> {
    let (key, payload) = shapes::single_key(value)?;
    match key {
        "i64" => Ok(noble_kernel::untrusted::Lit::I64(payload.as_int()?)),
        "bool" => match payload {
            crate::value::Json::Bool(flag) => Ok(noble_kernel::untrusted::Lit::Bool(*flag)),
            crate::value::Json::Null
            | crate::value::Json::Int(_)
            | crate::value::Json::Float(_)
            | crate::value::Json::Str(_)
            | crate::value::Json::Arr(_)
            | crate::value::Json::Obj(_) => Err("`bool` literal needs a JSON boolean".to_string()),
        },
        "text" => {
            payload.as_str()?;
            Ok(noble_kernel::untrusted::Lit::Text)
        }
        "unit" => Ok(noble_kernel::untrusted::Lit::Unit),
        _ => Err(format!("unknown literal key `{key}`")),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; node validates tagged documentation input with fallible field, vocabulary and witness decoders; malformed nodes must return errors rather than panic."
)]
fn node(value: &crate::value::Json) -> Result<noble_kernel::untrusted::Node, String> {
    let kind = value.field("kind")?.as_str()?;
    match kind {
        "literal" => Ok(noble_kernel::untrusted::Node::Literal {
            lit: lit(value.field("lit")?)?,
            inst: inst(value.field("inst")?, vocab::LITERAL)?,
        }),
        "invocation" => {
            let (def, kinds) = vocab::def_of(value.field("def")?.as_str()?)?;
            Ok(noble_kernel::untrusted::Node::Invocation {
                def: noble_kernel::contracts::Definition(def),
                inst: inst(value.field("inst")?, kinds)?,
            })
        }
        "quotation" => Ok(noble_kernel::untrusted::Node::Quotation {
            body: body_ids(value.field("body")?)?,
            inst: inst(value.field("inst")?, vocab::QUOTATION)?,
        }),
        _ => Err(format!("unknown node kind `{kind}`")),
    }
}

fn body_ids(value: &crate::value::Json) -> Result<Vec<noble_kernel::untrusted::NodeId>, String> {
    value
        .as_arr()?
        .iter()
        .map(|entry| {
            let id = entry.as_int()?;
            u32::try_from(id)
                .map(noble_kernel::untrusted::NodeId)
                .map_err(|problem| format!("node reference {id} is out of range: {problem}"))
        })
        .collect()
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; limits_from accepts omitted limits and reports malformed or out-of-u32 values through Result; extra assertions would alter the documentation-input contract."
)]
fn limits_from(
    value: Option<&crate::value::Json>,
) -> Result<noble_kernel::untrusted::Limits, String> {
    let mut limits = noble_kernel::untrusted::Limits {
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
    for (key, target) in [
        ("bytes", &mut limits.bytes),
        ("nodes", &mut limits.nodes),
        ("depth", &mut limits.depth),
        ("type_size", &mut limits.type_size),
        ("stack_height", &mut limits.stack_height),
        ("work", &mut limits.work),
        ("diagnostics", &mut limits.diagnostics),
    ] {
        if let Some(given) = found.optional(key)? {
            let number = given.as_int()?;
            *target = u32::try_from(number).map_err(|problem| {
                format!("limit `{key}` value {number} is out of range: {problem}")
            })?;
        }
    }
    Ok(limits)
}

/// Decode one whole example: request plus candidate.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; example composes fallible request and candidate decoders and preserves unknown formats for checker rejection; it must not assert that documentation input is valid."
)]
pub(super) fn example(value: &crate::value::Json) -> Result<Example, String> {
    let request_value = value.field("request")?;
    let expected = request_value.field("expected")?;
    let request = noble_kernel::untrusted::Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in: shapes::stack(expected.field("in")?)?,
            stack_out: shapes::stack(expected.field("out")?)?,
            allowed_effects: shapes::effect_set(expected.field("allowed_effects")?)?,
        },
        limits: limits_from(request_value.optional("limits")?)?,
    };
    let candidate_value = value.field("candidate")?;
    let format = match candidate_value.field("format")?.as_str()? {
        "noble-candidate/v1" => noble_kernel::untrusted::CANDIDATE_FORMAT,
        _ => 0,
    };
    let nodes = candidate_value
        .field("nodes")?
        .as_arr()?
        .iter()
        .map(node)
        .collect::<Result<_, _>>()?;
    let candidate = noble_kernel::untrusted::Candidate {
        format,
        revision: 0,
        nodes,
        body: body_ids(candidate_value.field("body")?)?,
    };
    Ok(Example { request, candidate })
}
