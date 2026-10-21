#![feature(register_tool)]
#![register_tool(tigerstyle)]

use noble_wasm::source::{CodeSpan, Compiler};

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn diagnostic(problem: noble_wasm::Diagnostic) -> &'static str {
    match problem {
        noble_wasm::Diagnostic::Invalid => "invalid",
        noble_wasm::Diagnostic::Exhausted => "exhausted",
        noble_wasm::Diagnostic::Unsupported => "unsupported",
        noble_wasm::Diagnostic::Defective => "defective",
    }
}

#[test]
fn core_live_span_never_reuses_after_refused_retirement_or_accepts_forged_records(
) -> Result<(), String> {
    let mut session = noble_contracts::source::Session::new_live("addone");
    let definition = session
        .prepare(b"def addone [ 1 + ]", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    session
        .commit(definition)
        .map_err(|error| error.diagnostic().message.clone())?;
    let selected = session
        .prepare(b"7 addone", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let submission = selected
        .submission()
        .ok_or("missing checked Core live submission")?;
    let mut compiler = Compiler::new_live();
    let first = compiler
        .prepare(submission)
        .map_err(|problem| format!("first checked live compile: {}", diagnostic(problem)))?;
    let first_span = first.code_span();
    if first_span.slots == 0 || first_span.start < 4 {
        return Err("first checked live program installed no code".into());
    }
    compiler
        .commit(first)
        .map_err(|problem| format!("first publish: {}", diagnostic(problem)))?;
    let second = compiler
        .prepare(submission)
        .map_err(|problem| format!("second checked live compile: {}", diagnostic(problem)))?;
    let second_span = second.code_span();
    if second_span.start < first_span.start + first_span.slots
        || second_span.generation == first_span.generation
    {
        return Err("installed old span was reused before its owner was released".into());
    }
    compiler
        .commit(second)
        .map_err(|problem| format!("second publish: {}", diagnostic(problem)))?;
    if compiler
        .retire_code(first_span, |_| Err(noble_wasm::Diagnostic::Invalid))
        .is_ok()
    {
        return Err("refused backend-owner proof retired code".into());
    }
    let before_proof = compiler
        .prepare(submission)
        .map_err(|problem| format!("unretired allocation: {}", diagnostic(problem)))?;
    if before_proof.code_span().start == first_span.start {
        return Err("owner-refused span was reused".into());
    }
    let forged = CodeSpan {
        generation: first_span.generation.wrapping_add(1),
        ..first_span
    };
    if compiler.retire_code(forged, |_| panic!("forged generation reached host table"))
        != Err(noble_wasm::Diagnostic::Invalid)
    {
        return Err("forged generation reached backend retirement".into());
    }
    if Compiler::new().retire_code(first_span, |_| {
        panic!("ordinary Core reached retire callback")
    }) != Err(noble_wasm::Diagnostic::Invalid)
    {
        return Err("ordinary nonlive compiler acquired retirement authority".into());
    }
    Ok(())
}
