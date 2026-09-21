pub(super) struct Call {
    pub id: u32,
    pub span: crate::Span,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; text-byte accumulation is checked against the configured limit before cloning, and arena terms/drafts are node-charged and indexed fallibly."
)]
pub(super) fn literal(
    node: &crate::source::Node,
    frame: &mut super::Frame,
    state: &mut super::State,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let (lit, text) = match &node.kind {
        crate::source::Kind::Literal(lit) => (*lit, None),
        crate::source::Kind::Text(bytes) => {
            let count = attempt!(crate::index(bytes.len(), node.span));
            state.text_bytes = match state.text_bytes.checked_add(count) {
                Some(total) if total <= meter.limits.bytes => total,
                Some(_) | None => {
                    return Err(crate::source::exhausted(
                        node.span,
                        "specialized text payload byte limit exceeded",
                    ))
                }
            };
            attempt!(meter.charge(count, node.span));
            (noble_kernel::untrusted::Lit::Text, Some(bytes.clone()))
        }
        _ => return Err(crate::internal(node.span)),
    };
    let ty = attempt!(state.arena.ty(&lit.ty(), node.span, meter));
    let output = attempt!(state.arena.add(
        crate::inference::Term::Push(frame.stack, ty),
        node.span,
        meter
    ));
    let variables = alloc::vec![crate::inference::Variable::Stack(frame.stack)];
    let id = attempt!(super::add(
        frame.body,
        super::Draft {
            kind: super::DraftKind::Literal(lit),
            variables,
            text,
            span: node.span
        },
        state,
        meter
    ));
    frame.sequence.push(id);
    frame.stack = output;
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scheme lookup, fresh witness creation, stack unification, effect union, and draft insertion reject through typed diagnostics under the shared meter."
)]
pub(super) fn builtin(
    call: Call,
    frame: &mut super::Frame,
    state: &mut super::State,
    environment: &noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let definition = noble_kernel::contracts::Definition(call.id);
    let scheme = match environment.scheme(definition) {
        Some(scheme) => scheme,
        None => return Err(crate::internal(call.span)),
    };
    let variables = attempt!(state.arena.variables(&scheme.var_kinds, call.span, meter));
    let input = attempt!(state
        .arena
        .pattern_stack(&scheme.stack_in, &variables, call.span, meter));
    let output =
        attempt!(state
            .arena
            .pattern_stack(&scheme.stack_out, &variables, call.span, meter));
    let effect =
        attempt!(state
            .arena
            .effect_pattern(&scheme.effects, &variables, call.span, meter));
    if let Err(error) = state.arena.unify(frame.stack, input, call.span, meter) {
        if error.kind != crate::DiagnosticKind::Invalid {
            return Err(error);
        }
        let message = state
            .arena
            .join_message(input, frame.stack, call.span, meter)
            .ok();
        return Err(contextualize(error, definition, frame.caller, message));
    }
    frame.effect = attempt!(state
        .arena
        .effect_union(frame.effect, effect, call.span, meter));
    let id = attempt!(super::add(
        frame.body,
        super::Draft {
            kind: super::DraftKind::Invocation(definition),
            variables,
            text: None,
            span: call.span
        },
        state,
        meter
    ));
    frame.sequence.push(id);
    frame.stack = output;
    Ok(())
}

fn contextualize(
    mut error: crate::Diagnostic,
    definition: noble_kernel::contracts::Definition,
    caller: Option<crate::Span>,
    message: Option<alloc::string::String>,
) -> crate::Diagnostic {
    let mut message = match message {
        Some(message) => message,
        None => return error,
    };
    message.push_str("; word ");
    crate::program::append_bootstrap_spelling(definition, &mut message);
    message.push_str("; ");
    message.push_str(&error.message);
    if let Some(caller) = caller {
        message.push_str("; while specializing a resolved definition");
        error.span = caller;
    }
    error.message = message;
    error
}
