//! Source-authored rank-1 stack interfaces; annotations are checked against bodies.

use alloc::string::String;
use alloc::vec::Vec;
use noble_kernel::shapes::{EffectSlot, Pattern};
use noble_kernel::words::{Scheme, Variable, VariableKind};

type Families = Vec<(String, noble_kernel::types::NominalTypeId, [u8; 2])>;

fn invalid(reason: &str) -> crate::source::Error {
    super::error(crate::source::Stage::Check, reason)
}

pub(super) fn families(
    session: &super::ModuleSession,
    names: &[super::Schema],
    identity: u64,
) -> Families {
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
                    // At most one family per local schema.
                    if result.len() >= names.len() {
                        break;
                    }
                    result.push((schema.name.clone(), id, declaration.payload_params));
                }
            }
        }
    }
    for module in &session.modules {
        // At most one family per exported item of this module.
        let module_bound = result.len().saturating_add(module.exports.len());
        for export in &module.exports {
            if let Some(id) = export.generic {
                if let Some(context) = &session.source.declared {
                    if let Some(declaration) = context.environment.generic_variant(id) {
                        if result.len() >= module_bound {
                            break;
                        }
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
    result
}

pub(super) fn parse(
    binders: &str,
    words: &[String],
    families: &Families,
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
        // A `name:kind` binder spans several bytes, so the count stays below the text length.
        if named.len() >= binders.len() {
            return Err(invalid("signature binder count exceeded"));
        }
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
    let (Some(output_start), Some(effect_index)) = (arrow.checked_add(1), bang.checked_add(1)) else {
        return Err(invalid("malformed signature stack or effect bound"));
    };
    if arrow == 0 || bang <= output_start || effect_index.checked_add(1) != Some(words.len()) {
        return Err(invalid("malformed signature stack or effect bound"));
    }
    let input = attempt!(parse_stack(&words[..arrow], &named, families));
    let output = attempt!(parse_stack(&words[output_start..bang], &named, families));
    let effects = attempt!(parse_effect(&words[effect_index], &named));
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
            Pattern::LiveRef(..) => return false,
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
    families: &Families,
) -> Result<Vec<Pattern>, crate::source::Error> {
    words
        .iter()
        .map(|word| pattern(word, binders, families))
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
    families: &Families,
) -> Result<Pattern, crate::source::Error> {
    enum Step<'a> {
        Parse(&'a str, u32),
        List,
        Binary(&'a str),
        ProgramInput(usize, &'a str, &'a str, u32),
        Program(usize, usize, &'a str),
    }
    let mut pending = alloc::vec![Step::Parse(word, 0)];
    let mut values = Vec::new();
    while let Some(step) = pending.pop() {
        let value = match step {
            Step::Parse(word, depth) => {
                if depth >= 32 {
                    return Err(invalid("signature type nesting limit exceeded"));
                }
                if let Some((index, (_, kind))) = binders
                    .iter()
                    .enumerate()
                    .find(|(_, (name, _))| name == word)
                {
                    Some(match kind {
                        VariableKind::Stack => Pattern::StackVar(Variable(index as u32)),
                        VariableKind::Value => Pattern::Var(Variable(index as u32)),
                        VariableKind::Effect => return Err(invalid("effect binder used as value type")),
                    })
                } else {
                    match word {
                        "Unit" => Some(Pattern::Unit),
                        "Bool" => Some(Pattern::Bool),
                        "I64" => Some(Pattern::I64),
                        "Text" => Some(Pattern::Text),
                        "Syntax" => Some(Pattern::Syntax),
                        _ => {
                            let Some((constructor, inner)) = word.split_once('<') else {
                                return Err(invalid("unknown signature value type"));
                            };
                            let Some(inner) = inner.strip_suffix('>') else {
                                return Err(invalid("unclosed signature type arguments"));
                            };
                            let next = attempt!(depth.checked_add(1)
                                .ok_or_else(|| invalid("signature type nesting limit exceeded")));
                            if constructor == "Resource" {
                                if inner != "test.counter" {
                                    return Err(invalid("unknown signature resource kind"));
                                }
                                Some(Pattern::Resource(noble_kernel::contracts::FIXTURE_RESOURCE))
                            } else if constructor == "List" {
                                pending.push(Step::List);
                                pending.push(Step::Parse(inner, next));
                                None
                            } else {
                                let parts = attempt!(split_arguments(inner));
                                if constructor == "Program" {
                                    if parts.len() != 3 {
                                        return Err(invalid("Program signature requires input, output and effects"));
                                    }
                                    let input = attempt!((parts[0] != "empty")
                                        .then(|| split_delimited(parts[0], b'+')).transpose());
                                    pending.push(Step::ProgramInput(
                                        input.as_ref().map_or(0, Vec::len), parts[1], parts[2], next,
                                    ));
                                    for component in input.into_iter().flatten().rev() {
                                        pending.push(Step::Parse(component, next));
                                    }
                                } else {
                                    if parts.len() != 2 {
                                        return Err(invalid("signature type requires two arguments"));
                                    }
                                    pending.push(Step::Binary(constructor));
                                    pending.push(Step::Parse(parts[1], next));
                                    pending.push(Step::Parse(parts[0], next));
                                }
                                None
                            }
                        }
                    }
                }
            }
            Step::List => {
                let Some(item) = values.pop() else {
                    return Err(invalid("invalid source signature scheme"));
                };
                Some(Pattern::List(alloc::boxed::Box::new(item)))
            }
            Step::Binary(constructor) => {
                let (Some(second), Some(first)) = (values.pop(), values.pop()) else {
                    return Err(invalid("invalid source signature scheme"));
                };
                Some(match constructor {
                    "Pair" => Pattern::Pair(alloc::boxed::Box::new(first), alloc::boxed::Box::new(second)),
                    "Sum" => Pattern::Sum(alloc::boxed::Box::new(first), alloc::boxed::Box::new(second)),
                    _ => {
                        let Some((_, id, payload_params)) =
                            families.iter().find(|(name, _, _)| name == constructor)
                        else {
                            return Err(invalid("unknown or unexported generic signature family"));
                        };
                        Pattern::GenericNominal(*id, alloc::boxed::Box::new([first, second]), *payload_params)
                    }
                })
            }
            Step::ProgramInput(inputs, output_word, effect, depth) => {
                let output = attempt!((output_word != "empty")
                    .then(|| split_delimited(output_word, b'+')).transpose());
                pending.push(Step::Program(inputs, output.as_ref().map_or(0, Vec::len), effect));
                for component in output.into_iter().flatten().rev() {
                    pending.push(Step::Parse(component, depth));
                }
                None
            }
            Step::Program(inputs, outputs, effect) => {
                let Some(total) = inputs.checked_add(outputs) else {
                    return Err(invalid("signature program stack exceeds its components"));
                };
                let Some(input_start) = values.len().checked_sub(total) else {
                    return Err(invalid("invalid source signature scheme"));
                };
                let effects = attempt!(parse_effect(effect, binders));
                let Some(output_start) = input_start.checked_add(inputs) else {
                    return Err(invalid("signature program stack exceeds its components"));
                };
                let output = values.split_off(output_start);
                let input = values.split_off(input_start);
                Some(Pattern::program(input, output, effects))
            }
        };
        if let Some(value) = value {
            if values.len() >= word.len() {
                return Err(invalid("signature type exceeds its text"));
            }
            values.push(value);
        }
    }
    match values.pop() {
        Some(value) if values.is_empty() => Ok(value),
        _ => Err(invalid("invalid source signature scheme")),
    }
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
                // Parts end at distinct separator bytes, so this bound is never reached.
                if result.len() >= word.len() {
                    return Err(invalid("signature type argument count exceeds its text"));
                }
                result.push(&word[start..at]);
                start = attempt!(at
                    .checked_add(1)
                    .ok_or_else(|| invalid("signature argument position overflow")));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_children_parse_in_order_and_nesting_is_bounded() {
        let binders = Vec::new();
        let families = Vec::new();
        let value = pattern("Program<Pair<I64,Bool>+List<Text>,Sum<Bool,I64>,pure>", &binders, &families)
            .expect("nested program signature");
        let Pattern::Program(input, output, _) = value else {
            panic!("expected program");
        };
        assert!(matches!(input.as_slice(), [Pattern::Pair(_, _), Pattern::List(_)]));
        assert!(matches!(output.as_slice(), [Pattern::Sum(_, _)]));

        let error = pattern("Program<Unknown,Resource<unknown>,pure>", &binders, &families)
            .expect_err("input error takes precedence");
        assert_eq!(error.diagnostic().message, "unknown signature value type");

        let deep = alloc::format!("{}I64{}", "List<".repeat(32), ">".repeat(32));
        let error = pattern(&deep, &binders, &families).expect_err("type depth bound");
        assert_eq!(error.diagnostic().message, "signature type nesting limit exceeded");
    }
}
