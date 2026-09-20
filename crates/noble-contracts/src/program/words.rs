pub(super) struct Call {
    pub def: noble_kernel::contracts::Definition,
    pub current: u32,
    pub explicit: Option<noble_kernel::words::Inst>,
    pub span: crate::Span,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scheme lookup, effect eligibility, explicit witnesses, and stack equations are checked with typed diagnostics."
)]
pub(super) fn apply(
    call: Call,
    arena: &mut crate::inference::Arena,
    env: &noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<(u32, super::Draft), crate::Diagnostic> {
    let scheme = match env.scheme(call.def) {
        Some(scheme) => scheme,
        None => return Err(crate::invalid(call.span, "unknown word definition")),
    };
    let variables = attempt!(arena.variables(&scheme.var_kinds, call.span, meter));
    attempt!(crate::inference::pure_effects(
        &scheme.effects,
        &variables,
        call.span,
        meter
    ));
    if let Some(inst) = &call.explicit {
        attempt!(super::witness::apply(
            super::witness::Explicit {
                inst,
                kinds: &scheme.var_kinds,
                variables: &variables,
                span: call.span,
            },
            arena,
            meter
        ));
    }
    let input = attempt!(arena.pattern_stack(&scheme.stack_in, &variables, call.span, meter));
    let output = attempt!(arena.pattern_stack(&scheme.stack_out, &variables, call.span, meter));
    attempt!(arena.unify(call.current, input, call.span, meter));
    Ok((
        output,
        super::Draft {
            kind: super::DraftKind::Invocation(call.def),
            variables,
            explicit: call.explicit,
            span: call.span,
        },
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this total token classifier explicitly rejects unknown and effectful words; arbitrary input has no panic precondition."
)]
pub(super) fn identity(
    word: &[u8],
    span: crate::Span,
) -> Result<noble_kernel::contracts::Definition, crate::Diagnostic> {
    let id = match word {
        _ if word == b"dup" => 0,
        _ if word == b"drop" => 1,
        _ if word == b"swap" => 2,
        _ if word == b"dip" => 3,
        _ if word == b"+" => 4,
        _ if word == b"-" => 5,
        _ if word == b"*" => 6,
        _ if word == b"=" => 7,
        _ if word == b"quote" => 8,
        _ if word == b"compose" => 9,
        _ if word == b"run" => 10,
        _ if word == b"reflect" => 11,
        _ if word == b"unit" => 12,
        _ if word == b"pair" => 13,
        _ if word == b"unpair" => 14,
        _ if word == b"inl" => 15,
        _ if word == b"inr" => 16,
        _ if word == b"case" => 17,
        _ if word == b"if" => 18,
        _ if word == b"nil" => 19,
        _ if word == b"cons" => 20,
        _ if word == b"list.case" => 21,
        _ if word == b"test.emit" || word == b"host" || word == b"call" => {
            return Err(crate::Diagnostic::new(
                crate::DiagnosticKind::Unsupported,
                span,
                "host and effectful words are outside the pure contract fragment",
            ))
        }
        _ => return Err(crate::invalid(span, "unknown program word")),
    };
    Ok(noble_kernel::contracts::Definition(id))
}
