//! Source-authored rank-1 stack interfaces; annotations are checked against bodies.

use alloc::string::String;
use alloc::vec::Vec;
use noble_kernel::shapes::{EffectSlot, Pattern};
use noble_kernel::words::{Scheme, Variable, VariableKind};

pub(super) struct Families<'a> {
    generic: Vec<(String, noble_kernel::types::NominalTypeId, [u8; 2])>,
    session: &'a super::ModuleSession,
    local: &'a [super::Schema],
    identity: u64,
    environment: Option<&'a noble_kernel::contracts::Env>,
}

fn invalid(reason: &str) -> crate::source::Error {
    super::error(crate::source::Stage::Check, reason)
}

pub(super) fn families<'a>(
    session: &'a super::ModuleSession,
    names: &'a [super::Schema],
    identity: u64,
) -> Families<'a> {
    let mut result = Vec::new();
    for (ordinal, schema) in names.iter().enumerate() {
        if let super::SchemaKind::Variant { parameters, .. } = &schema.kind {
            if parameters.is_empty() {
                continue;
            }
            let id = noble_kernel::types::NominalTypeId {
                module: identity,
                ordinal: ordinal as u32,
            };
            if let Some(context) = &session.source.declared {
                if let Some(declaration) = context.environment.generic_variant(id) {
                    result.push((schema.name.clone(), id, declaration.payload_params));
                }
            }
        }
    }
    for module in &session.modules {
        for export in &module.exports {
            if let Some(id) = export.generic {
                if let Some(context) = &session.source.declared {
                    if let Some(declaration) = context.environment.generic_variant(id) {
                        result.push((
                            alloc::format!("{}@{}.{}", module.name, module.version, export.name),
                            id,
                            declaration.payload_params,
                        ));
                    }
                }
            }
        }
    }
    Families {
        generic: result,
        session,
        local: names,
        identity,
        environment: session
            .source
            .declared
            .as_ref()
            .map(|context| &context.environment),
    }
}

pub(super) fn parse(
    binders: &str,
    words: &[String],
    families: &Families<'_>,
) -> Result<Scheme, crate::source::Error> {
    let Some(binders) = binders
        .strip_prefix("forall<")
        .and_then(|text| text.strip_suffix('>'))
    else {
        return Err(invalid("invalid source signature binder list"));
    };
    let mut named = Vec::new();
    for binding in binders.split(',') {
        let Some((name, kind)) = binding.split_once(':') else {
            return Err(invalid("signature binder requires name and kind"));
        };
        if !super::parsing::name(name) || named.iter().any(|(known, _)| known == name) {
            return Err(invalid("duplicate or invalid signature binder"));
        }
        let kind = match kind {
            "stack" => VariableKind::Stack,
            "value" => VariableKind::Value,
            "effect" => VariableKind::Effect,
            _ => return Err(invalid("invalid signature binder kind")),
        };
        named.push((String::from(name), kind));
    }
    if named.is_empty() || named.len() > 32 {
        return Err(invalid("signature binder count exceeded"));
    }
    let Some(arrow) = words.iter().position(|word| word == "--") else {
        return Err(invalid(
            "signature requires ordered input and output stacks",
        ));
    };
    let Some(bang) = words.iter().position(|word| word == "!") else {
        return Err(invalid("signature requires latent effect bound"));
    };
    if arrow == 0 || bang <= arrow + 1 || bang + 2 != words.len() {
        return Err(invalid("malformed signature stack or effect bound"));
    }
    let input = attempt!(parse_stack(&words[..arrow], &named, families));
    let output = attempt!(parse_stack(&words[arrow + 1..bang], &named, families));
    let effects = attempt!(parse_effect(&words[bang + 1], &named));
    let scheme = Scheme {
        var_kinds: named.into_iter().map(|(_, kind)| kind).collect(),
        stack_in: input,
        stack_out: output,
        effects,
    };
    attempt!(scheme
        .validate()
        .map_err(|_| invalid("invalid source signature scheme")));
    Ok(scheme)
}

pub(super) fn exported(signature: &Scheme, environment: &noble_kernel::contracts::Env) -> bool {
    let mut pending: Vec<&Pattern> = signature
        .stack_in
        .iter()
        .chain(&signature.stack_out)
        .collect();
    let mut remaining = 256usize;
    while let Some(pattern) = pending.pop() {
        if remaining == 0 {
            return false;
        }
        remaining -= 1;
        match pattern {
            Pattern::Nominal(id, _) => {
                if !environment.nominal(*id).is_some_and(|decl| decl.exported) {
                    return false;
                }
            }
            Pattern::GenericNominal(id, args, _) => {
                if !environment
                    .generic_variant(*id)
                    .is_some_and(|decl| decl.exported)
                {
                    return false;
                }
                pending.push(&args[1]);
                pending.push(&args[0]);
            }
            Pattern::Pair(first, second) | Pattern::Sum(first, second) => {
                pending.push(second);
                pending.push(first);
            }
            Pattern::List(item) => pending.push(item),
            Pattern::Program(inputs, outputs, _) => {
                pending.extend(inputs.iter());
                pending.extend(outputs.iter());
            }
            Pattern::Unit
            | Pattern::Bool
            | Pattern::I64
            | Pattern::Text
            | Pattern::Syntax
            | Pattern::Contract
            | Pattern::Evidence
            | Pattern::Certified
            | Pattern::Resource(_)
            | Pattern::Var(_)
            | Pattern::StackVar(_) => {}
        }
    }
    true
}

fn parse_stack(
    words: &[String],
    binders: &[(String, VariableKind)],
    families: &Families<'_>,
) -> Result<Vec<Pattern>, crate::source::Error> {
    words
        .iter()
        .map(|word| pattern(word, binders, families, 0))
        .collect()
}

fn parse_effect(
    word: &str,
    binders: &[(String, VariableKind)],
) -> Result<Vec<EffectSlot>, crate::source::Error> {
    if word == "pure" {
        return Ok(Vec::new());
    }
    if word == "test.emit" {
        return Ok(alloc::vec![EffectSlot::Effect(
            noble_kernel::contracts::TEST_EMIT
        )]);
    }
    if word == "test.clock" {
        return Ok(alloc::vec![EffectSlot::Effect(
            noble_kernel::contracts::TEST_CLOCK
        )]);
    }
    let Some((index, _)) = binders
        .iter()
        .enumerate()
        .find(|(_, (name, kind))| name == word && *kind == VariableKind::Effect)
    else {
        return Err(invalid("unknown signature effect variable"));
    };
    Ok(alloc::vec![EffectSlot::Var(Variable(index as u32))])
}

fn pattern(
    word: &str,
    binders: &[(String, VariableKind)],
    families: &Families<'_>,
    depth: u32,
) -> Result<Pattern, crate::source::Error> {
    if depth >= 32 {
        return Err(invalid("signature type nesting limit exceeded"));
    }
    if let Some((index, (_, kind))) = binders
        .iter()
        .enumerate()
        .find(|(_, (name, _))| name == word)
    {
        return Ok(match kind {
            VariableKind::Stack => Pattern::StackVar(Variable(index as u32)),
            VariableKind::Value => Pattern::Var(Variable(index as u32)),
            VariableKind::Effect => return Err(invalid("effect binder used as value type")),
        });
    }
    let primitive = match word {
        "Unit" => Some(Pattern::Unit),
        "Bool" => Some(Pattern::Bool),
        "I64" => Some(Pattern::I64),
        "Text" => Some(Pattern::Text),
        "Syntax" => Some(Pattern::Syntax),
        _ => None,
    };
    if let Some(primitive) = primitive {
        return Ok(primitive);
    }
    if let Some((ordinal, _)) = families
        .local
        .iter()
        .enumerate()
        .find(|(_, schema)| schema.name == word)
    {
        let id = noble_kernel::types::NominalTypeId {
            module: families.identity,
            ordinal: ordinal as u32,
        };
        if let Some(declaration) = families.environment.and_then(|env| env.nominal(id)) {
            return Ok(Pattern::Nominal(
                id,
                alloc::boxed::Box::new(declaration.shape.clone()),
            ));
        }
    }
    if let Some(nominal) = exported_nominal(word, families) {
        return Ok(nominal);
    }
    let Some((constructor, inner)) = word.split_once('<') else {
        return Err(invalid("unknown signature value type"));
    };
    let Some(inner) = inner.strip_suffix('>') else {
        return Err(invalid("unclosed signature type arguments"));
    };
    let next = depth + 1;
    if constructor == "Resource" {
        return if inner == "test.counter" {
            Ok(Pattern::Resource(noble_kernel::contracts::FIXTURE_RESOURCE))
        } else {
            Err(invalid("unknown signature resource kind"))
        };
    }
    if constructor == "List" {
        return Ok(Pattern::List(alloc::boxed::Box::new(attempt!(pattern(
            inner, binders, families, next,
        )))));
    }
    let parts = attempt!(split_arguments(inner));
    if constructor == "Program" {
        if parts.len() != 3 {
            return Err(invalid(
                "Program signature requires input, output and effects",
            ));
        }
        let input = attempt!(program_stack(parts[0], binders, families, next));
        let output = attempt!(program_stack(parts[1], binders, families, next));
        let effects = attempt!(parse_effect(parts[2], binders));
        return Ok(Pattern::program(input, output, effects));
    }
    if parts.len() != 2 {
        return Err(invalid("signature type requires two arguments"));
    }
    let first = attempt!(pattern(parts[0], binders, families, next));
    let second = attempt!(pattern(parts[1], binders, families, next));
    match constructor {
        "Pair" => Ok(Pattern::Pair(
            alloc::boxed::Box::new(first),
            alloc::boxed::Box::new(second),
        )),
        "Sum" => Ok(Pattern::Sum(
            alloc::boxed::Box::new(first),
            alloc::boxed::Box::new(second),
        )),
        _ => {
            let Some((_, id, payload_params)) =
                families.generic.iter().find(|(name, _, _)| name == constructor)
            else {
                return Err(invalid("unknown or unexported generic signature family"));
            };
            Ok(Pattern::GenericNominal(
                *id,
                alloc::boxed::Box::new([first, second]),
                *payload_params,
            ))
        }
    }
}

fn exported_nominal(word: &str, families: &Families<'_>) -> Option<Pattern> {
    let (qualified_module, export_name) = word.rsplit_once('.')?;
    let (module_name, version) = qualified_module.rsplit_once('@')?;
    if version.as_bytes().first() == Some(&b'0') {
        return None;
    }
    let version = version.parse::<u32>().ok()?;
    let module = families.session.modules.iter()
        .find(|module| module.name == module_name && module.version == version)?;
    let ty = module.exports.iter().find(|export| export.name == export_name)?
        .ty.as_ref()?;
    let noble_kernel::types::Ty::Nominal(id, shape) = ty else {
        return None;
    };
    families.environment?.nominal(*id).filter(|declaration| declaration.exported)?;
    Some(Pattern::Nominal(*id, shape.clone()))
}

fn program_stack(
    word: &str,
    binders: &[(String, VariableKind)],
    families: &Families<'_>,
    depth: u32,
) -> Result<Vec<Pattern>, crate::source::Error> {
    if word == "empty" {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for component in attempt!(split_delimited(word, b'+')) {
        result.push(attempt!(pattern(component, binders, families, depth)));
    }
    Ok(result)
}

fn split_arguments(word: &str) -> Result<Vec<&str>, crate::source::Error> {
    split_delimited(word, b',')
}

fn split_delimited(word: &str, separator: u8) -> Result<Vec<&str>, crate::source::Error> {
    let mut result = Vec::new();
    let mut depth = 0u32;
    let mut start = 0usize;
    for (at, byte) in word.bytes().enumerate() {
        match byte {
            b'<' => {
                depth = attempt!(depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("signature nesting overflow")))
            }
            b'>' => {
                depth = attempt!(depth
                    .checked_sub(1)
                    .ok_or_else(|| invalid("unbalanced signature type")))
            }
            byte if byte == separator && depth == 0 => {
                result.push(&word[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(invalid("unbalanced signature type arguments"));
    }
    result.push(&word[start..]);
    if result.iter().any(|part| part.is_empty()) {
        return Err(invalid("empty signature type argument"));
    }
    Ok(result)
}
