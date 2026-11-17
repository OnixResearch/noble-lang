//! Source-authored rank-1 stack interfaces; annotations are checked against bodies.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use alloc::string::String;
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use alloc::vec::Vec;
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use noble_kernel::shapes::{EffectSlot, Pattern};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use noble_kernel::words::{Scheme, Variable, VariableKind};

pub(super) struct Families<'a> {
    generic: Vec<(String, noble_kernel::types::NominalTypeId, [u8; 2])>,
    modules: &'a [super::Module],
    local: &'a [super::Schema],
    identity: u64,
    environment: Option<&'a noble_kernel::contracts::Env>,
}

fn invalid(reason: &str) -> crate::source::Error {
    super::error(crate::source::Stage::Check, reason)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
pub(super) fn families<'a>(
    session: &'a super::ModuleSession,
    names: &'a [super::Schema],
    identity: u64,
) -> Families<'a> {
    let mut result = Vec::new();
    let mut ordinal = 0usize;
    let mut is_full = false;
    while !is_full && ordinal < names.len() {
        let id = noble_kernel::types::NominalTypeId {
            module: identity,
            ordinal: ordinal as u32,
        };
        if let Some(payload_params) = local_family(session, &names[ordinal], id) {
            // At most one family per local schema.
            if result.len() >= names.len() {
                is_full = true;
            } else {
                result.push((names[ordinal].name.clone(), id, payload_params));
            }
        }
        ordinal += 1;
    }
    let mut module_slot = 0usize;
    while module_slot < session.modules.len() {
        result = module_families(session, &session.modules[module_slot], result);
        module_slot += 1;
    }
    Families {
        generic: result,
        modules: &session.modules,
        local: names,
        identity,
        environment: session_environment(session),
    }
}

/// One named generic family with its nominal identity and payload parameters.
type Family = (String, noble_kernel::types::NominalTypeId, [u8; 2]);

/// The declared-source environment of `session`, if it has one. Selected by
/// match: the pinned Aeneas refuses a closure that returns a borrow.
const fn session_environment(
    session: &super::ModuleSession,
) -> Option<&noble_kernel::contracts::Env> {
    match &session.source.declared {
        Some(context) => Some(&context.environment),
        None => None,
    }
}

/// The payload parameters of the generic variant family declared by a local schema.
fn local_family(
    session: &super::ModuleSession,
    schema: &super::Schema,
    id: noble_kernel::types::NominalTypeId,
) -> Option<[u8; 2]> {
    let super::SchemaKind::Variant { parameters, .. } = &schema.kind else {
        return None;
    };
    if parameters.is_empty() {
        return None;
    }
    generic_payload_params(session, id)
}

/// The payload parameters of the registered generic variant family `id`.
fn generic_payload_params(
    session: &super::ModuleSession,
    id: noble_kernel::types::NominalTypeId,
) -> Option<[u8; 2]> {
    let Some(context) = &session.source.declared else {
        return None;
    };
    context
        .environment
        .generic_variant(id)
        .map(|declaration| declaration.payload_params)
}

/// Append the exported generic families of one module, at most one per export.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn module_families(
    session: &super::ModuleSession,
    module: &super::Module,
    mut result: Vec<Family>,
) -> Vec<Family> {
    let module_bound = result.len().saturating_add(module.exports.len());
    let mut entry_slot = 0usize;
    let mut is_full = false;
    while !is_full && entry_slot < module.exports.len() {
        if let Some(id) = module.exports[entry_slot].generic {
            if let Some(payload_params) = generic_payload_params(session, id) {
                if result.len() >= module_bound {
                    is_full = true;
                } else {
                    result.push((
                        exported_family_name(module, &module.exports[entry_slot]),
                        id,
                        payload_params,
                    ));
                }
            }
        }
        entry_slot += 1;
    }
    result
}

/// The qualified `module@version.Name` spelling of an exported family.
fn exported_family_name(module: &super::Module, export: &super::Export) -> String {
    alloc::format!("{}@{}.{}", module.name, module.version, export.name)
}

/// A copyable view of the signature families, passed by value through the
/// parser so no loop reads through a borrowed struct of borrows. The optional
/// environment is passed beside it: an optional borrow stored in a struct
/// that a loop carries is refused by the pinned Aeneas.
#[derive(Clone, Copy)]
struct FamilyView<'a> {
    generic: &'a [(String, noble_kernel::types::NominalTypeId, [u8; 2])],
    modules: &'a [super::Module],
    local: &'a [super::Schema],
    identity: u64,
}

/// The copyable view of `families`.
fn family_view<'a>(families: &'a Families<'_>) -> FamilyView<'a> {
    FamilyView {
        generic: &families.generic,
        modules: families.modules,
        local: families.local,
        identity: families.identity,
    }
}

pub(super) fn parse(
    binders: &str,
    words: &[String],
    families: &Families<'_>,
) -> Result<Scheme, crate::source::Error> {
    parse_scheme(binders, words, family_view(families), families.environment)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
/// Parse a `forall<..>` binder list and its interface words into a scheme.
fn parse_scheme(
    binders: &str,
    words: &[String],
    families: FamilyView<'_>,
    environment: Option<&noble_kernel::contracts::Env>,
) -> Result<Scheme, crate::source::Error> {
    let Some(binders) = binder_list(binders) else {
        return Err(invalid("invalid source signature binder list"));
    };
    let named = attempt!(parse_binders(binders));
    if named.is_empty() || named.len() > 32 {
        return Err(invalid("signature binder count exceeded"));
    }
    let Some(arrow) = super::position_first(words, |word| word == "--") else {
        return Err(invalid(
            "signature requires ordered input and output stacks",
        ));
    };
    let Some(bang) = super::position_first(words, |word| word == "!") else {
        return Err(invalid("signature requires latent effect bound"));
    };
    let (Some(output_start), Some(effect_index)) = (arrow.checked_add(1), bang.checked_add(1))
    else {
        return Err(invalid("malformed signature stack or effect bound"));
    };
    if arrow == 0 || bang <= output_start || effect_index.checked_add(1) != Some(words.len()) {
        return Err(invalid("malformed signature stack or effect bound"));
    }
    let input = attempt!(parse_stack(&words[..arrow], &named, families, environment));
    let output = attempt!(parse_stack(
        &words[output_start..bang],
        &named,
        families,
        environment,
    ));
    let effects = attempt!(parse_effect(&words[effect_index], &named));
    let mut var_kinds = Vec::with_capacity(named.len());
    for (_, kind) in named {
        var_kinds.push(kind);
    }
    let scheme = Scheme {
        var_kinds,
        stack_in: input,
        stack_out: output,
        effects,
    };
    attempt!(scheme
        .validate()
        .map_err(|_| invalid("invalid source signature scheme")));
    Ok(scheme)
}

/// The text inside `forall<` and the final `>`, as `strip_prefix` then `strip_suffix`.
fn binder_list(text: &str) -> Option<&str> {
    let inner = crate::scan::strip_prefix_str(text, "forall<")?;
    crate::scan::strip_suffix_str(inner, ">")
}

/// The binders of a `forall<..>` list, one per comma-separated piece as `str::split(',')`.
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; at never exceeds the text length, itself at most isize::MAX, so at + 1 cannot overflow"
)]
fn parse_binders(binders: &str) -> Result<Vec<(String, VariableKind)>, crate::source::Error> {
    let bytes = binders.as_bytes();
    let mut named = Ok(Vec::new());
    let mut start = 0usize;
    let mut at = 0usize;
    while named.is_ok() && at <= bytes.len() {
        if at == bytes.len() || bytes[at] == b',' {
            named = match named {
                Ok(known) => push_binder(&binders[start..at], binders.len(), known),
                Err(problem) => Err(problem),
            };
            start = at + 1;
        }
        at += 1;
    }
    named
}

/// Admit one `name:kind` binder after the binders named before it.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn push_binder(
    binding: &str,
    text_len: usize,
    mut named: Vec<(String, VariableKind)>,
) -> Result<Vec<(String, VariableKind)>, crate::source::Error> {
    let Some((name, kind)) = crate::scan::split_once_byte(binding, b':') else {
        return Err(invalid("signature binder requires name and kind"));
    };
    if !super::parsing::name(name)
        || super::position_first(&named, |(known, _)| known == name).is_some()
    {
        return Err(invalid("duplicate or invalid signature binder"));
    }
    let kind = match kind {
        "stack" => VariableKind::Stack,
        "value" => VariableKind::Value,
        "effect" => VariableKind::Effect,
        _ => return Err(invalid("invalid signature binder kind")),
    };
    // A `name:kind` binder spans several bytes, so the count stays below the text length.
    if named.len() >= text_len {
        return Err(invalid("signature binder count exceeded"));
    }
    named.push((String::from(name), kind));
    Ok(named)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
pub(super) fn exported(signature: &Scheme, environment: &noble_kernel::contracts::Env) -> bool {
    // An owned copy of the interface is consumed instead of a worklist of
    // borrows; the visit order and the work bound are unchanged.
    let mut pending: Vec<Pattern> = Vec::with_capacity(
        signature
            .stack_in
            .len()
            .saturating_add(signature.stack_out.len()),
    );
    pending.extend_from_slice(&signature.stack_in);
    pending.extend_from_slice(&signature.stack_out);
    let mut state = Some(pending);
    let mut remaining = 256usize;
    let mut has_work = true;
    while has_work {
        state = match state {
            Some(mut current) => match current.pop() {
                Some(_) if remaining == 0 => {
                    has_work = false;
                    None
                }
                Some(pattern) => {
                    remaining -= 1;
                    exported_step(pattern, environment, current)
                }
                None => {
                    has_work = false;
                    Some(current)
                }
            },
            None => {
                has_work = false;
                None
            }
        };
    }
    state.is_some()
}

/// One exported-interface check: reject a hidden pattern or queue its children.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec push/append, Box moves and environment lookups are not const on the pinned compilers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn exported_step(
    pattern: Pattern,
    environment: &noble_kernel::contracts::Env,
    mut pending: Vec<Pattern>,
) -> Option<Vec<Pattern>> {
    match pattern {
        Pattern::Nominal(id, _) => {
            if !environment.nominal(id).is_some_and(|decl| decl.exported) {
                return None;
            }
        }
        Pattern::GenericNominal(id, mut args, _) => {
            if !environment
                .generic_variant(id)
                .is_some_and(|decl| decl.exported)
            {
                return None;
            }
            // Move each argument out behind a cheap placeholder.
            let second = core::mem::replace(&mut args[1], Pattern::Unit);
            let first = core::mem::replace(&mut args[0], Pattern::Unit);
            pending.push(second);
            pending.push(first);
        }
        Pattern::Pair(first, second) | Pattern::Sum(first, second) => {
            pending.push(*second);
            pending.push(*first);
        }
        Pattern::List(item) => pending.push(*item),
        Pattern::Program(mut inputs, mut outputs, _) => {
            pending.append(&mut *inputs);
            pending.append(&mut *outputs);
        }
        Pattern::LiveRef(..) => return None,
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
    Some(pending)
}

#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; the loop pushes at most one pattern per interface word, so the stack is bounded by words.len()"
)]
fn parse_stack(
    words: &[String],
    binders: &[(String, VariableKind)],
    families: FamilyView<'_>,
    environment: Option<&noble_kernel::contracts::Env>,
) -> Result<Vec<Pattern>, crate::source::Error> {
    let mut stack = Vec::new();
    let mut failure = None;
    let mut at = 0usize;
    while failure.is_none() && at < words.len() {
        match pattern(&words[at], binders, families, environment) {
            Ok(value) => stack.push(value),
            Err(problem) => failure = Some(problem),
        }
        at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(stack),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
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
    if word == "test.code-export" {
        return Ok(alloc::vec![EffectSlot::Effect(
            noble_kernel::contracts::TEST_CODE_EXPORT
        )]);
    }
    if word == "test.send" {
        return Ok(alloc::vec![EffectSlot::Effect(
            noble_kernel::contracts::TEST_SEND
        )]);
    }
    let Some(index) = super::position_first(binders, |(name, kind)| {
        name == word && *kind == VariableKind::Effect
    }) else {
        return Err(invalid("unknown signature effect variable"));
    };
    Ok(alloc::vec![EffectSlot::Var(Variable(index as u32))])
}

/// One pending action of the signature pattern parser. Words are owned
/// copies of pieces of the signature text, not borrows into it.
enum PatternStep {
    Parse(String, u32),
    List,
    Binary(String),
    ProgramInput(usize, String, String, u32),
    Program(usize, usize, String),
}

/// The signature pattern parser's work list, parsed values and text-length bound.
struct PatternState {
    pending: Vec<PatternStep>,
    values: Vec<Pattern>,
    text_len: usize,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn pattern(
    word: &str,
    binders: &[(String, VariableKind)],
    families: FamilyView<'_>,
    environment: Option<&noble_kernel::contracts::Env>,
) -> Result<Pattern, crate::source::Error> {
    let mut state = Ok(PatternState {
        pending: alloc::vec![PatternStep::Parse(String::from(word), 0)],
        values: Vec::new(),
        text_len: word.len(),
    });
    let mut has_work = true;
    while has_work {
        state = match state {
            Ok(mut current) => match current.pending.pop() {
                Some(step) => pattern_step(step, binders, families, environment, current),
                None => {
                    has_work = false;
                    Ok(current)
                }
            },
            Err(problem) => {
                has_work = false;
                Err(problem)
            }
        };
    }
    let mut state = attempt!(state);
    match state.values.pop() {
        Some(value) if state.values.is_empty() => Ok(value),
        Some(_) | None => Err(invalid("invalid source signature scheme")),
    }
}

#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::nested_conditionals,
    reason = "Owner: noble-contracts; the guard chain states the admission rule verbatim; extracting it would obscure the predicate it states"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass pattern parser is kept linear so the grammar stays readable beside it"
)]
/// One step of the signature pattern parser: parse, combine or queue values.
fn pattern_step(
    step: PatternStep,
    binders: &[(String, VariableKind)],
    families: FamilyView<'_>,
    environment: Option<&noble_kernel::contracts::Env>,
    mut state: PatternState,
) -> Result<PatternState, crate::source::Error> {
    let value = match step {
        PatternStep::Parse(word, depth) => {
            let word = word.as_str();
            if depth >= 32 {
                return Err(invalid("signature type nesting limit exceeded"));
            }
            if let Some(index) = super::position_first(binders, |(name, _)| name == word) {
                Some(match binders[index].1 {
                    VariableKind::Stack => Pattern::StackVar(Variable(index as u32)),
                    VariableKind::Value => Pattern::Var(Variable(index as u32)),
                    VariableKind::Effect => {
                        return Err(invalid("effect binder used as value type"))
                    }
                })
            } else {
                match word {
                    "Unit" => Some(Pattern::Unit),
                    "Bool" => Some(Pattern::Bool),
                    "I64" => Some(Pattern::I64),
                    "Text" => Some(Pattern::Text),
                    "Syntax" => Some(Pattern::Syntax),
                    _ => match named_nominal(word, families, environment) {
                        Some(nominal) => Some(nominal),
                        None => {
                            let Some((constructor, inner)) =
                                crate::scan::split_once_byte(word, b'<')
                            else {
                                return Err(invalid("unknown signature value type"));
                            };
                            let Some(inner) = crate::scan::strip_suffix_str(inner, ">") else {
                                return Err(invalid("unclosed signature type arguments"));
                            };
                            let next = attempt!(depth
                                .checked_add(1)
                                .ok_or_else(|| invalid("signature type nesting limit exceeded")));
                            if constructor == "Resource" {
                                if inner != "test.counter" {
                                    return Err(invalid("unknown signature resource kind"));
                                }
                                Some(Pattern::Resource(noble_kernel::contracts::FIXTURE_RESOURCE))
                            } else if constructor == "List" {
                                state.pending.push(PatternStep::List);
                                state
                                    .pending
                                    .push(PatternStep::Parse(String::from(inner), next));
                                None
                            } else {
                                let mut parts = attempt!(split_arguments(inner));
                                if constructor == "Program" {
                                    if parts.len() != 3 {
                                        return Err(invalid(
                                            "Program signature requires input, output and effects",
                                        ));
                                    }
                                    let input = attempt!((parts[0] != "empty")
                                        .then(|| split_delimited(&parts[0], b'+'))
                                        .transpose());
                                    let (count, mut components) = match input {
                                        Some(components) => (components.len(), components),
                                        None => (0, Vec::new()),
                                    };
                                    state.pending.push(PatternStep::ProgramInput(
                                        count,
                                        core::mem::take(&mut parts[1]),
                                        core::mem::take(&mut parts[2]),
                                        next,
                                    ));
                                    // Last component first, so the first is parsed first.
                                    while let Some(component) = components.pop() {
                                        state.pending.push(PatternStep::Parse(component, next));
                                    }
                                } else {
                                    if parts.len() != 2 {
                                        return Err(invalid(
                                            "signature type requires two arguments",
                                        ));
                                    }
                                    state
                                        .pending
                                        .push(PatternStep::Binary(String::from(constructor)));
                                    state.pending.push(PatternStep::Parse(
                                        core::mem::take(&mut parts[1]),
                                        next,
                                    ));
                                    state.pending.push(PatternStep::Parse(
                                        core::mem::take(&mut parts[0]),
                                        next,
                                    ));
                                }
                                None
                            }
                        }
                    },
                }
            }
        }
        PatternStep::List => {
            let Some(item) = state.values.pop() else {
                return Err(invalid("invalid source signature scheme"));
            };
            Some(Pattern::List(alloc::boxed::Box::new(item)))
        }
        PatternStep::Binary(constructor) => {
            let (Some(second), Some(first)) = (state.values.pop(), state.values.pop()) else {
                return Err(invalid("invalid source signature scheme"));
            };
            Some(match constructor.as_str() {
                "Pair" => Pattern::Pair(
                    alloc::boxed::Box::new(first),
                    alloc::boxed::Box::new(second),
                ),
                "Sum" => Pattern::Sum(
                    alloc::boxed::Box::new(first),
                    alloc::boxed::Box::new(second),
                ),
                _ => {
                    let Some((_, id, payload_params)) =
                        crate::scan::find_first(families.generic, |(name, _, _)| {
                            *name == constructor
                        })
                    else {
                        return Err(invalid("unknown or unexported generic signature family"));
                    };
                    Pattern::GenericNominal(
                        *id,
                        alloc::boxed::Box::new([first, second]),
                        *payload_params,
                    )
                }
            })
        }
        PatternStep::ProgramInput(inputs, output_word, effect, depth) => {
            let output = attempt!((output_word != "empty")
                .then(|| split_delimited(&output_word, b'+'))
                .transpose());
            let (count, mut components) = match output {
                Some(components) => (components.len(), components),
                None => (0, Vec::new()),
            };
            state
                .pending
                .push(PatternStep::Program(inputs, count, effect));
            // Last component first, so the first is parsed first.
            while let Some(component) = components.pop() {
                state.pending.push(PatternStep::Parse(component, depth));
            }
            None
        }
        PatternStep::Program(inputs, outputs, effect) => {
            let Some(total) = inputs.checked_add(outputs) else {
                return Err(invalid("signature program stack exceeds its components"));
            };
            let Some(input_start) = state.values.len().checked_sub(total) else {
                return Err(invalid("invalid source signature scheme"));
            };
            let effects = attempt!(parse_effect(&effect, binders));
            let Some(output_start) = input_start.checked_add(inputs) else {
                return Err(invalid("signature program stack exceeds its components"));
            };
            let output = state.values.split_off(output_start);
            let input = state.values.split_off(input_start);
            Some(Pattern::program(input, output, effects))
        }
    };
    if let Some(value) = value {
        if state.values.len() >= state.text_len {
            return Err(invalid("signature type exceeds its text"));
        }
        state.values.push(value);
    }
    Ok(state)
}

/// A bare local schema name or an exact exported `module@version.Name`
/// resolves only to an already registered nominal declaration.
fn named_nominal(
    word: &str,
    families: FamilyView<'_>,
    environment: Option<&noble_kernel::contracts::Env>,
) -> Option<Pattern> {
    if let Some(ordinal) = super::position_first(families.local, |schema| schema.name == word) {
        let id = noble_kernel::types::NominalTypeId {
            module: families.identity,
            ordinal: ordinal as u32,
        };
        let declaration = match environment {
            Some(environment) => environment.nominal(id),
            None => None,
        };
        if let Some(declaration) = declaration {
            return Some(Pattern::Nominal(
                id,
                alloc::boxed::Box::new(declaration.shape.clone()),
            ));
        }
    }
    exported_nominal(word, families, environment)
}

fn exported_nominal(
    word: &str,
    families: FamilyView<'_>,
    environment: Option<&noble_kernel::contracts::Env>,
) -> Option<Pattern> {
    let (qualified_module, export_name) = crate::scan::rsplit_once_byte(word, b'.')?;
    let (module_name, version) = crate::scan::rsplit_once_byte(qualified_module, b'@')?;
    let version = super::canonical_version(version)?;
    let module = crate::scan::find_first(families.modules, |module| {
        module.name == module_name && module.version == version
    })?;
    let ty = crate::scan::find_first(&module.exports, |export| export.name == export_name)?
        .ty
        .as_ref()?;
    let noble_kernel::types::Ty::Nominal(id, shape) = ty else {
        return None;
    };
    environment?
        .nominal(*id)
        .filter(|declaration| declaration.exported)?;
    Some(Pattern::Nominal(*id, shape.clone()))
}

fn split_arguments(word: &str) -> Result<Vec<String>, crate::source::Error> {
    split_delimited(word, b',')
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
/// The top-level pieces of `word` between `separator` bytes, as owned copies.
fn split_delimited(word: &str, separator: u8) -> Result<Vec<String>, crate::source::Error> {
    let mut split = Ok(Delimited {
        parts: Vec::new(),
        depth: 0,
        start: 0,
    });
    let mut at = 0usize;
    while split.is_ok() && at < word.len() {
        split = match split {
            Ok(current) => delimited_step(word, separator, at, current),
            Err(problem) => Err(problem),
        };
        at += 1;
    }
    let Delimited {
        parts: mut result,
        depth,
        start,
    } = attempt!(split);
    if depth != 0 {
        return Err(invalid("unbalanced signature type arguments"));
    }
    result.push(String::from(&word[start..]));
    if super::has_empty_part(&result) {
        return Err(invalid("empty signature type argument"));
    }
    Ok(result)
}

/// The parts, nesting depth and current part start of a delimited split.
struct Delimited {
    parts: Vec<String>,
    depth: u32,
    start: usize,
}

/// Classify the byte of `word` at `at`: track nesting or cut a top-level part.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn delimited_step(
    word: &str,
    separator: u8,
    at: usize,
    mut split: Delimited,
) -> Result<Delimited, crate::source::Error> {
    match word.as_bytes()[at] {
        b'<' => {
            split.depth = attempt!(split
                .depth
                .checked_add(1)
                .ok_or_else(|| invalid("signature nesting overflow")))
        }
        b'>' => {
            split.depth = attempt!(split
                .depth
                .checked_sub(1)
                .ok_or_else(|| invalid("unbalanced signature type")))
        }
        byte if byte == separator && split.depth == 0 => {
            // Parts end at distinct separator bytes, so this bound is never reached.
            if split.parts.len() >= word.len() {
                return Err(invalid("signature type argument count exceeds its text"));
            }
            split.parts.push(String::from(&word[split.start..at]));
            split.start = attempt!(at
                .checked_add(1)
                .ok_or_else(|| invalid("signature argument position overflow")));
        }
        _ => {}
    }
    Ok(split)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_children_parse_in_order_and_nesting_is_bounded() {
        let binders = Vec::new();
        let families = Families {
            generic: Vec::new(),
            modules: &[],
            local: &[],
            identity: 0,
            environment: None,
        };
        let families = family_view(&families);
        let value = pattern(
            "Program<Pair<I64,Bool>+List<Text>,Sum<Bool,I64>,pure>",
            &binders,
            families,
            None,
        )
        .expect("nested program signature");
        let Pattern::Program(input, output, _) = value else {
            panic!("expected program");
        };
        assert!(matches!(
            input.as_slice(),
            [Pattern::Pair(_, _), Pattern::List(_)]
        ));
        assert!(matches!(output.as_slice(), [Pattern::Sum(_, _)]));

        let error = pattern(
            "Program<Unknown,Resource<unknown>,pure>",
            &binders,
            families,
            None,
        )
        .expect_err("input error takes precedence");
        assert_eq!(error.diagnostic().message, "unknown signature value type");

        let deep = alloc::format!("{}I64{}", "List<".repeat(32), ">".repeat(32));
        let error = pattern(&deep, &binders, families, None).expect_err("type depth bound");
        assert_eq!(
            error.diagnostic().message,
            "signature type nesting limit exceeded"
        );
    }

    fn empty_families() -> Families<'static> {
        Families {
            generic: Vec::new(),
            modules: &[],
            local: &[],
            identity: 0,
            environment: None,
        }
    }

    fn binder_error(binders: &str) -> String {
        let words: Vec<String> = ["S", "--", "S", "!", "pure"]
            .iter()
            .map(|word| String::from(*word))
            .collect();
        let error = parse(binders, &words, &empty_families()).expect_err("binder refusal");
        String::from(error.diagnostic().message.as_str())
    }

    #[test]
    fn binder_list_splits_at_every_comma_and_stops_at_the_first_failure() {
        let words: Vec<String> = ["S", "--", "S", "!", "pure"]
            .iter()
            .map(|word| String::from(*word))
            .collect();
        let scheme = parse("forall<S:stack>", &words, &empty_families()).expect("one binder");
        assert_eq!(scheme.var_kinds, alloc::vec![VariableKind::Stack]);
        // Empty pieces before, between and after commas are binders too.
        for binders in [
            "forall<S:stack,>",
            "forall<,S:stack>",
            "forall<S:stack,,T:value>",
        ] {
            assert_eq!(
                binder_error(binders),
                "signature binder requires name and kind"
            );
        }
        // The first refused binder decides the diagnostic.
        assert_eq!(
            binder_error("forall<S:bad,T>"),
            "invalid signature binder kind"
        );
        assert_eq!(
            binder_error("forall<S:stack,S:value>"),
            "duplicate or invalid signature binder"
        );
        assert_eq!(
            binder_error("forall<S:stack"),
            "invalid source signature binder list"
        );
    }

    #[test]
    fn delimited_split_keeps_nested_arguments_and_stops_at_the_first_failure() {
        let parts = split_delimited("Pair<I64,Bool>,List<Text>", b',').expect("two parts");
        assert_eq!(parts, alloc::vec!["Pair<I64,Bool>", "List<Text>"]);
        let error = split_delimited("I64,,Bool", b',').expect_err("empty part");
        assert_eq!(error.diagnostic().message, "empty signature type argument");
        let error = split_delimited("I64>,Bool<", b',').expect_err("early close");
        assert_eq!(error.diagnostic().message, "unbalanced signature type");
        let error = split_delimited("Pair<I64", b',').expect_err("unclosed");
        assert_eq!(
            error.diagnostic().message,
            "unbalanced signature type arguments"
        );
    }

    #[test]
    fn callable_type_components_report_the_first_failure() {
        let span = crate::Span { start: 0, end: 1 };
        let parse_type =
            |word: &str| super::super::types::parse_with_families(word, &[], &[], None, span);
        let parsed = parse_type("Program<I64+Bool,Text,pure>").expect("callable type");
        assert_eq!(parsed.nodes, 4);
        let error = parse_type("Program<I64+Unknown+Missing,Text,pure>")
            .err()
            .expect("unknown component");
        assert_eq!(error.message, "unknown or cyclic nominal type");
        let error = parse_type("Program<I64,,pure>").err().expect("empty field");
        assert_eq!(error.message, "empty type argument");
        let error = parse_type("Program<I64,Text,bogus+test.emit>")
            .err()
            .expect("unknown effect first");
        assert_eq!(error.message, "unknown callable effect bound");
        let error = parse_type("Program<I64,Text,test.emit+bogus>")
            .err()
            .expect("unregistered effect first");
        assert_eq!(error.message, "unregistered or duplicate callable effect");
    }

    fn parse_type(word: &str) -> Result<super::super::types::ParsedType, crate::Diagnostic> {
        let span = crate::Span { start: 0, end: 1 };
        super::super::types::parse_with_families(word, &[], &[], None, span)
    }

    fn refusal(word: &str) -> String {
        parse_type(word)
            .err()
            .map(|error| error.message)
            .expect("refused type")
    }

    #[test]
    fn nested_callable_types_resume_their_parent_in_order() {
        let parsed = parse_type("Program<Program<I64,Bool,pure>+Text,empty,pure>").expect("nested");
        assert_eq!(parsed.nodes, 5);
        let noble_kernel::types::Ty::Program(inputs, outputs, _) = parsed.ty else {
            panic!("expected a program");
        };
        assert!(matches!(
            inputs.as_slice(),
            [
                noble_kernel::types::Ty::Program(..),
                noble_kernel::types::Ty::Text
            ]
        ));
        assert!(outputs.is_empty());
        assert_eq!(
            parse_type("List<Program<I64,empty,pure>>")
                .expect("list")
                .nodes,
            3
        );
        let pair = parse_type("Pair<Program<I64,empty,pure>,Bool>").expect("pair");
        assert_eq!(pair.nodes, 4);
        assert!(matches!(pair.ty, noble_kernel::types::Ty::Pair(..)));
        let grouped = parse_type("Program<Pair<I64,Bool>,List<Text>,pure>").expect("grouped");
        assert_eq!(grouped.nodes, 6);
        assert_eq!(
            refusal("Program<I64,Text,pure>x"),
            "trailing characters in type word"
        );
    }

    #[test]
    fn callable_type_refusals_keep_their_order_and_limits() {
        assert_eq!(
            refusal("Program<I64,Unknown,pure>"),
            "unknown or cyclic nominal type"
        );
        assert_eq!(
            refusal("Program<Unknown,Text,bogus>"),
            "unknown or cyclic nominal type"
        );
        assert_eq!(
            refusal("LiveRef<I64,empty,pure>"),
            "LiveRef requires the live-slot profile"
        );
        assert_eq!(refusal("Program<I64,Text,pure"), "unclosed Program type");
        assert_eq!(
            refusal("Program<I64,Text>"),
            "Program requires input, output and effects"
        );
        let nest = |levels: usize| {
            alloc::format!(
                "{}I64{}",
                "Program<".repeat(levels),
                ",empty,pure>".repeat(levels)
            )
        };
        assert_eq!(parse_type(&nest(31)).expect("31 levels").nodes, 32);
        assert_eq!(
            refusal(&nest(32)),
            "type constructor nesting limit exceeded"
        );
        let stack = |height: usize| {
            alloc::format!("Program<{},empty,pure>", ["I64"; 300][..height].join("+"))
        };
        assert_eq!(refusal(&stack(257)), "Program stack height limit exceeded");
        assert_eq!(refusal(&stack(256)), "type constructor limit exceeded");
        assert_eq!(parse_type(&stack(255)).expect("255 inputs").nodes, 256);
    }

    #[expect(
        tigerstyle::unbounded_collection_growth,
        reason = "Owner: noble-contracts; the test helper collects one span per definition of a fixed literal module"
    )]
    fn definition_spans(source: &[u8]) -> Vec<crate::Span> {
        let parsed = super::super::parsing::parse(source, crate::Limits::default(), None)
            .expect("module source");
        let super::super::parsing::ParsedUnit::Module { members, .. } = parsed else {
            panic!("expected a module");
        };
        let mut spans = Vec::new();
        for member in members {
            if let super::super::parsing::Member::Definition { span, .. } = member {
                spans.push(span);
            }
        }
        spans
    }

    #[test]
    fn named_export_reparse_stops_at_the_first_conflicting_member() {
        let limits = crate::Limits::default();
        let verify = |source: &[u8], ordinal: u32, span: crate::Span| {
            super::super::verify_named_export(source, "m", 1, "f", ordinal, span, limits)
        };
        let source = b"module m@1 [ def g [ ] export f def f [ ] ]";
        let spans = definition_spans(source);
        verify(source, 1, spans[1]).expect("unique exported definition");
        let error = verify(source, 0, spans[1]).expect_err("wrong ordinal");
        assert_eq!(
            error.message,
            "named source export has a different lexical definition"
        );
        // The duplicate export precedes the definition, so it is reported.
        let source = b"module m@1 [ export f export f def f [ ] def f [ ] ]";
        let spans = definition_spans(source);
        let error = verify(source, 0, spans[0]).expect_err("duplicate export");
        assert_eq!(error.message, "duplicate named source export");
        let source = b"module m@1 [ export f def f [ ] def f [ ] ]";
        let spans = definition_spans(source);
        let error = verify(source, 0, spans[0]).expect_err("second definition");
        assert_eq!(
            error.message,
            "named source export has a different lexical definition"
        );
    }

    #[test]
    #[expect(
        tigerstyle::platform_dependent_cast,
        reason = "Owner: noble-contracts; usize is at least 32 bits on every supported target, so the u32-to-usize widening is lossless."
    )]
    fn definition_origin_reparse_binds_one_definition_and_refuses_repeats() {
        let limits = crate::Limits::default();
        let source: &[u8] = b"module m@1 [ def f [ 1 ] def g [ 2 ] def g [ 3 ] ]";
        let spans = definition_spans(source);
        let origin = |ordinal: u32, span: crate::Span| super::super::DefinitionOrigin {
            module_source: source,
            module: "m",
            version: 1,
            definition: "f",
            ordinal,
            span,
        };
        let bytes = |span: crate::Span| &source[span.start as usize..span.end as usize];
        let body =
            super::super::verify_definition_origin(origin(0, spans[0]), bytes(spans[0]), limits)
                .expect("unique definition");
        assert_eq!(body.end, spans[0].end);
        assert!(body.start > spans[0].start);
        let error =
            super::super::verify_definition_origin(origin(1, spans[0]), bytes(spans[0]), limits)
                .expect_err("wrong ordinal");
        assert_eq!(
            error.message,
            "ambiguous or mismatched lexical subject definition"
        );
        let repeated = super::super::DefinitionOrigin {
            definition: "g",
            ordinal: 1,
            span: spans[1],
            ..origin(0, spans[0])
        };
        let error = super::super::verify_definition_origin(repeated, bytes(spans[1]), limits)
            .expect_err("repeated definition");
        assert_eq!(
            error.message,
            "ambiguous or mismatched lexical subject definition"
        );
    }

    #[test]
    fn signature_and_binding_blocks_report_their_own_failures() {
        let parse_module = |source: &[u8]| {
            super::super::parsing::parse(source, crate::Limits::default(), None).map(|_| ())
        };
        parse_module(b"module m@1 [ signature f forall<S:stack> [ S -- S ! pure ] ]")
            .expect("closed signature");
        let error = parse_module(b"module m@1 [ signature f forall<S:stack [ S -- S ! pure ] ]")
            .expect_err("binder list");
        assert_eq!(error.message, "invalid signature binder list");
        let error = parse_module(
            b"module m@1 [ contract 1 c [ subject f input [ x I64 x Bool ] output [ y I64 ] requires [ true ] ensures [ true ] ] ]",
        )
        .expect_err("duplicate binding");
        assert_eq!(error.message, "duplicate stack binding");
        let error = parse_module(
            b"module m@1 [ contract 1 c [ subject f input [ x I64 ] output [ y I64 y ] requires [ true ] ensures [ true ] ] ]",
        )
        .expect_err("incomplete binding");
        assert_eq!(error.message, "expected declaration word");
    }
}
