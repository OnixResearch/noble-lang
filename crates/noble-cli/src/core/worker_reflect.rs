//! Reflected Worker Program recipes, for explicit host preparation
//! (P-ROUND-01).
//!
//! The runtime's own observation ABI (`observe`, `recipe_kind`,
//! `recipe_value`) yields a flat event stream per installed cell. `decode`
//! accepts exactly the pure source forms: I64 and Bool literals, Text, a
//! builtin or named call with its declared-witness events, the owner's own
//! scoped self call and nested quotations. `encode` spells the decoded recipe
//! in the checked-recipe grammar so the host can compare it with the
//! compiler's normalized recipe byte for byte. `render_module` spells it as
//! module source with synthesized names; a recursive owner's signature comes
//! from its retained checked scheme, which reflection does not carry.

use noble_kernel::shapes::{EffectSlot, Pattern};
use noble_kernel::words::{Scheme, VariableKind};

use super::worker_program::Refusal;

/// Recipe event kinds of the runtime reflection ABI.
const EVENT_I64: u32 = 1;
const EVENT_BUILTIN: u32 = 2;
const EVENT_QUOTATION: u32 = 3;
const EVENT_BOOL: u32 = 4;
const EVENT_END: u32 = 6;
const EVENT_TEXT_BYTE: u32 = 7;
const EVENT_TEXT: u32 = 8;
const EVENT_CALL: u32 = 15;
const EVENT_QUOTATION_INPUT: u32 = 17;
const EVENT_QUOTATION_OUTPUT: u32 = 18;
const EVENT_QUOTATION_EFFECTS: u32 = 19;
const EVENT_INVOCATION_INPUT: u32 = 20;
const EVENT_INVOCATION_OUTPUT: u32 = 21;
const EVENT_INVOCATION_EFFECTS: u32 = 22;
/// Synthesized module name; names are not part of any identity.
const MODULE_NAME: &str = "Reflected";

/// One element of a reflected recipe, in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Token {
    I64(i64),
    Bool(bool),
    Text(Vec<u8>),
    Builtin(u32),
    /// A named call by definition identity.
    Call(u64),
    /// The owner's own scoped recursive call.
    SelfCall,
    Open,
    Close,
}

/// One reflected closure definition and what rendering it needs.
pub(super) struct ReflectedDefinition {
    pub(super) ordinal: u32,
    pub(super) identity: u64,
    pub(super) signature: Option<Scheme>,
    pub(super) tokens: Vec<Token>,
}

/// Rendered module source and the synthesized name of its exported root.
pub(super) struct RenderedModule {
    pub(super) source: String,
    pub(super) root_name: String,
}

fn event(events: &[(u32, u64)], at: usize) -> Result<(u32, u64), Refusal> {
    events.get(at).copied().ok_or(Refusal::Invalid)
}

/// The declared-witness triple after a builtin or call: input and output
/// signature ids, then an empty effect mask (pure Programs only).
fn invocation_witness(events: &[(u32, u64)], at: usize) -> Result<(), Refusal> {
    if event(events, at)?.0 != EVENT_INVOCATION_INPUT
        || event(events, at + 1)?.0 != EVENT_INVOCATION_OUTPUT
        || event(events, at + 2)? != (EVENT_INVOCATION_EFFECTS, 0)
    {
        return Err(Refusal::Invalid);
    }
    Ok(())
}

fn text(events: &[(u32, u64)], mut at: usize) -> Result<(Vec<u8>, usize), Refusal> {
    let mut bytes = Vec::new();
    loop {
        match event(events, at)? {
            (EVENT_TEXT_BYTE, byte) => {
                bytes.push(u8::try_from(byte).map_err(|_| Refusal::Invalid)?);
            }
            (EVENT_END, 0) => break,
            _ => return Err(Refusal::Invalid),
        }
        at += 1;
    }
    std::str::from_utf8(&bytes).map_err(|_| Refusal::Invalid)?;
    Ok((bytes, at + 1))
}

/// Decode one cell's event stream. A call naming `owner` is its scoped self
/// call: unsigned and mutual recursion never reach an admitted Program.
pub(super) fn decode(events: &[(u32, u64)], owner: u64) -> Result<Vec<Token>, Refusal> {
    let mut tokens = Vec::with_capacity(events.len());
    let mut depth = 0usize;
    let mut at = 0usize;
    while at < events.len() {
        let (kind, value) = event(events, at)?;
        at += 1;
        match kind {
            EVENT_I64 => tokens.push(Token::I64(i64::from_le_bytes(value.to_le_bytes()))),
            EVENT_BOOL if value <= 1 => tokens.push(Token::Bool(value == 1)),
            EVENT_TEXT if value == 0 => {
                let (bytes, next) = text(events, at)?;
                tokens.push(Token::Text(bytes));
                at = next;
            }
            EVENT_BUILTIN | EVENT_CALL => {
                invocation_witness(events, at)?;
                at += 3;
                tokens.push(match kind {
                    EVENT_BUILTIN => {
                        Token::Builtin(u32::try_from(value).map_err(|_| Refusal::Invalid)?)
                    }
                    _ if value == owner => Token::SelfCall,
                    _ => Token::Call(value),
                });
            }
            EVENT_QUOTATION if value == 0 => {
                if event(events, at)?.0 != EVENT_QUOTATION_INPUT
                    || event(events, at + 1)?.0 != EVENT_QUOTATION_OUTPUT
                    || event(events, at + 2)? != (EVENT_QUOTATION_EFFECTS, 0)
                {
                    return Err(Refusal::Invalid);
                }
                at += 3;
                depth += 1;
                tokens.push(Token::Open);
            }
            EVENT_END if value == 0 && depth > 0 => {
                depth -= 1;
                tokens.push(Token::Close);
            }
            // Unit, captured non-Text data, nominal and bound atoms have no
            // pure source spelling here.
            _ => return Err(Refusal::Unsupported),
        }
    }
    if depth != 0 {
        return Err(Refusal::Invalid);
    }
    Ok(tokens)
}

/// The checked-recipe grammar of `noble_wasm` admission identity.
pub(super) fn encode(tokens: &[Token], owner: u64) -> Vec<u8> {
    let mut out = Vec::new();
    for token in tokens {
        match token {
            Token::I64(value) => out.extend_from_slice(format!("i(i64.const {value})").as_bytes()),
            Token::Bool(true) => out.push(b't'),
            Token::Bool(false) => out.push(b'f'),
            Token::Text(bytes) => {
                out.extend_from_slice(format!("s{}:", bytes.len()).as_bytes());
                out.extend_from_slice(bytes);
            }
            Token::Builtin(id) => out.extend_from_slice(format!("b{id};").as_bytes()),
            Token::Call(identity) => out.extend_from_slice(format!("d{identity};").as_bytes()),
            Token::SelfCall => out.extend_from_slice(format!("S{owner};").as_bytes()),
            Token::Open => out.push(b'['),
            Token::Close => out.push(b']'),
        }
    }
    out
}

fn definition_name(ordinal: u32) -> String {
    format!("r{ordinal}")
}

/// A Text literal the source lexer reads back as exactly `bytes`.
fn text_literal(bytes: &[u8]) -> Result<String, Refusal> {
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Invalid)?;
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{85}' | '\u{2028}' | '\u{2029}' => {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(character)));
            }
            control if control.is_control() => {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(control)));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    Ok(out)
}

fn binder(scheme: &Scheme, variable: u32, expected: VariableKind) -> Result<String, Refusal> {
    let kind = scheme
        .var_kinds
        .get(usize::try_from(variable).map_err(|_| Refusal::Invalid)?)
        .copied()
        .ok_or(Refusal::Invalid)?;
    if kind != expected {
        return Err(Refusal::Invalid);
    }
    Ok(match kind {
        VariableKind::Stack => format!("S{variable}"),
        VariableKind::Value => format!("V{variable}"),
        VariableKind::Effect => format!("E{variable}"),
    })
}

/// The source spelling of one scheme pattern, for the selected pure types.
fn pattern(scheme: &Scheme, item: &Pattern) -> Result<String, Refusal> {
    Ok(match item {
        Pattern::StackVar(variable) => binder(scheme, variable.0, VariableKind::Stack)?,
        Pattern::Var(variable) => binder(scheme, variable.0, VariableKind::Value)?,
        Pattern::Unit => "Unit".into(),
        Pattern::Bool => "Bool".into(),
        Pattern::I64 => "I64".into(),
        Pattern::Text => "Text".into(),
        Pattern::Syntax => "Syntax".into(),
        Pattern::List(element) => format!("List<{}>", pattern(scheme, element)?),
        Pattern::Pair(left, right) => {
            format!(
                "Pair<{},{}>",
                pattern(scheme, left)?,
                pattern(scheme, right)?
            )
        }
        Pattern::Sum(left, right) => {
            format!(
                "Sum<{},{}>",
                pattern(scheme, left)?,
                pattern(scheme, right)?
            )
        }
        _ => return Err(Refusal::Unsupported),
    })
}

/// `forall<...> [ IN -- OUT ! EFFECT ]` for one retained scheme.
pub(super) fn signature(scheme: &Scheme) -> Result<String, Refusal> {
    if scheme.var_kinds.is_empty() || scheme.var_kinds.len() > 32 || scheme.stack_in.is_empty() {
        return Err(Refusal::Unsupported);
    }
    let mut binders = Vec::with_capacity(scheme.var_kinds.len());
    for (index, kind) in scheme.var_kinds.iter().enumerate() {
        let variable = u32::try_from(index).map_err(|_| Refusal::Invalid)?;
        let spelling = match kind {
            VariableKind::Stack => "stack",
            VariableKind::Value => "value",
            VariableKind::Effect => "effect",
        };
        binders.push(format!("{}:{spelling}", binder(scheme, variable, *kind)?));
    }
    let input = scheme
        .stack_in
        .iter()
        .map(|item| pattern(scheme, item))
        .collect::<Result<Vec<_>, _>>()?;
    let output = scheme
        .stack_out
        .iter()
        .map(|item| pattern(scheme, item))
        .collect::<Result<Vec<_>, _>>()?;
    let effect = match scheme.effects.as_slice() {
        [] => "pure".to_owned(),
        [EffectSlot::Var(variable)] => binder(scheme, variable.0, VariableKind::Effect)?,
        _ => return Err(Refusal::Unsupported),
    };
    Ok(format!(
        "forall<{}> [ {} -- {} ! {effect} ]",
        binders.join(","),
        input.join(" "),
        output.join(" ")
    ))
}

/// Module source for a closure reflected from one module: definitions in
/// ordinal order, each named by its ordinal, the root exported and every
/// recursive owner preceded by its signature.
pub(super) fn render_module(
    version: u32,
    root: u64,
    definitions: &[ReflectedDefinition],
) -> Result<RenderedModule, Refusal> {
    let mut ordered: Vec<&ReflectedDefinition> = definitions.iter().collect();
    ordered.sort_by_key(|definition| definition.ordinal);
    if ordered
        .iter()
        .enumerate()
        .any(|(at, definition)| usize::try_from(definition.ordinal).ok() != Some(at))
    {
        return Err(Refusal::Unsupported);
    }
    let mut source = format!("module {MODULE_NAME}@{version} [");
    let mut root_name = None;
    for definition in &ordered {
        let name = definition_name(definition.ordinal);
        if let Some(scheme) = &definition.signature {
            source.push_str(&format!(" signature {name} {}", signature(scheme)?));
        }
        if definition.identity == root {
            source.push_str(&format!(" export {name}"));
            root_name = Some(name.clone());
        }
        source.push_str(&format!(" def {name} ["));
        for token in &definition.tokens {
            source.push(' ');
            match token {
                Token::I64(value) => source.push_str(&value.to_string()),
                Token::Bool(value) => source.push_str(if *value { "true" } else { "false" }),
                Token::Text(bytes) => source.push_str(&text_literal(bytes)?),
                Token::Builtin(id) => source.push_str(
                    noble_contracts::source::pure_builtin_spelling(*id)
                        .ok_or(Refusal::Unsupported)?,
                ),
                Token::Call(identity) => {
                    let callee = ordered
                        .iter()
                        .find(|candidate| candidate.identity == *identity)
                        .ok_or(Refusal::Invalid)?;
                    source.push_str(&definition_name(callee.ordinal));
                }
                Token::SelfCall => source.push_str(&name),
                Token::Open => source.push('['),
                Token::Close => source.push(']'),
            }
        }
        source.push_str(" ]");
    }
    source.push_str(" ]");
    Ok(RenderedModule {
        source,
        root_name: root_name.ok_or(Refusal::Invalid)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `dec`'s reflected events in the WORKER-04 fixture.
    const DEC: [(u32, u64); 14] = [
        (8, 0),
        (7, 116),
        (7, 97),
        (7, 103),
        (6, 0),
        (2, 1),
        (20, 3),
        (21, 1),
        (22, 0),
        (1, 1),
        (2, 5),
        (20, 4),
        (21, 1),
        (22, 0),
    ];

    #[test]
    fn decode_refuses_malformed_or_unspellable_events() {
        assert_eq!(
            encode(&decode(&DEC, 1).unwrap(), 1),
            b"s3:tagb1;i(i64.const 1)b5;".to_vec()
        );
        let mut missing_witness = DEC.to_vec();
        missing_witness.remove(6);
        assert_eq!(decode(&missing_witness, 1), Err(Refusal::Invalid));
        let mut effectful = DEC.to_vec();
        effectful[8] = (22, 1);
        assert_eq!(decode(&effectful, 1), Err(Refusal::Invalid));
        assert_eq!(decode(&[(6, 0)], 1), Err(Refusal::Unsupported));
        assert_eq!(
            decode(&[(3, 0), (17, 1), (18, 1), (19, 0)], 1),
            Err(Refusal::Invalid)
        );
        assert_eq!(decode(&[(5, 0)], 1), Err(Refusal::Unsupported));
        assert_eq!(
            decode(&[(8, 0), (7, 0xff), (6, 0)], 1),
            Err(Refusal::Invalid)
        );
    }

    #[test]
    fn rendered_text_and_signature_are_read_back_by_the_checker() {
        let text = "q\"\\\n\r\t\u{85}\u{2028}\u{1}é";
        let scheme = Scheme {
            var_kinds: vec![VariableKind::Stack, VariableKind::Value],
            stack_in: vec![
                Pattern::StackVar(noble_kernel::words::Variable(0)),
                Pattern::Var(noble_kernel::words::Variable(1)),
                Pattern::List(Box::new(Pattern::Pair(
                    Box::new(Pattern::I64),
                    Box::new(Pattern::Text),
                ))),
            ],
            stack_out: vec![
                Pattern::StackVar(noble_kernel::words::Variable(0)),
                Pattern::Var(noble_kernel::words::Variable(1)),
            ],
            effects: vec![],
        };
        let source = format!(
            "module Probe@1 [ signature keep {} export keep def keep [ drop ] export word def word [ {} drop ] ]",
            signature(&scheme).unwrap(),
            text_literal(text.as_bytes()).unwrap()
        );
        let retained = noble_contracts::source::RetainedSourceLimit::new(65_536).unwrap();
        let session =
            noble_contracts::source::ModuleSession::new_signed_self(&[], retained).unwrap();
        let checked = session
            .prepare(
                source.as_bytes(),
                &[],
                super::super::worker_program::SOURCE_LIMITS,
            )
            .unwrap();
        let (session, committed) = session.commit(checked);
        committed.unwrap();
        let invocation = session
            .prepare(
                b"Probe@1.word",
                &[],
                super::super::worker_program::SOURCE_LIMITS,
            )
            .unwrap();
        assert!(invocation
            .submission()
            .unwrap()
            .definitions
            .iter()
            .flat_map(|definition| definition.body.texts.iter())
            .any(|literal| literal.bytes == text.as_bytes()));
    }
}
