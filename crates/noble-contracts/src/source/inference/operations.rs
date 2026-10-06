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
        let origin = mismatched_literal_origin(frame, state, scheme);
        let message = state
            .arena
            .join_stacks(input, frame.stack, call.span, meter)
            .ok();
        return Err(contextualize(
            error,
            definition,
            environment.kind(definition),
            frame.caller,
            origin,
            message,
        ));
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

/// A ref is selected from the root's typed borrowed-input sidecar, never
/// constructed from a value or literal. The unqualified spelling chooses the
/// uniquely most-specific compatible interface; ambiguous peers require the
/// explicit borrowed formal ordinal (`slot.invoke.N`).
pub(super) fn slot_invoke(
    selected: Option<u32>,
    span: crate::Span,
    frame: &mut super::Frame,
    state: &mut super::State,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let actual = attempt!(state.arena.stack_value(frame.stack, span, meter));
    let mut choice: Option<(u32, alloc::vec::Vec<u32>, usize)> = None;
    let mut ambiguous = false;
    for borrowed in &state.borrows {
        attempt!(meter.charge(1, span));
        if selected.is_some_and(|ordinal| ordinal != borrowed.ordinal) {
            continue;
        }
        let noble_kernel::types::Ty::LiveRef(input, _, _) = &borrowed.ty else {
            return Err(crate::internal(span));
        };
        let value_count = input.iter()
            .filter(|ty| !matches!(ty, noble_kernel::types::Ty::LiveRef(..))).count();
        if value_count > actual.len()
            || !input.iter()
                .filter(|ty| !matches!(ty, noble_kernel::types::Ty::LiveRef(..)))
                .zip(&actual[actual.len() - value_count..])
                .all(|(left, right)| left == right)
        {
            continue;
        }
        let mut forwarded = alloc::vec::Vec::new();
        let mut compatible = true;
        for formal in input.iter().filter(|ty| matches!(ty, noble_kernel::types::Ty::LiveRef(..))) {
            let mut positions = state.borrows.iter()
                .filter(|candidate| &candidate.ty == formal)
                .map(|candidate| candidate.logical_position);
            match (positions.next(), positions.next()) {
                (Some(position), None) => forwarded.push(position),
                (None, _) | (Some(_), Some(_)) => {
                    compatible = false;
                    break;
                }
            }
        }
        if !compatible {
            continue;
        }
        let specificity = forwarded.len();
        if selected.is_some() || choice.as_ref().is_none_or(|(_, _, previous)| specificity > *previous) {
            choice = Some((borrowed.ordinal, forwarded, specificity));
            ambiguous = false;
        } else if choice.as_ref().is_some_and(|(_, _, previous)| specificity == *previous) {
            ambiguous = true;
        }
    }
    if ambiguous {
        return Err(crate::invalid(span,
            "ambiguous compatible borrowed references; select slot.invoke.N"));
    }
    let Some((ordinal, forwarded, _)) = choice else {
        return Err(crate::invalid(span,
            "slot.invoke requires a compatible host-issued borrowed input and exact forwarded refs"));
    };
    let noble_kernel::types::Ty::LiveRef(input, output, ceiling) =
        &state.borrows[ordinal as usize].ty else {
        return Err(crate::internal(span));
    };
    // Source preflight excludes refs from output; only values enter the stack.
    if output.iter().any(|ty| matches!(ty, noble_kernel::types::Ty::LiveRef(..))) {
        return Err(crate::invalid(span, "live reference cannot be returned"));
    }
    let value_count = input.iter()
        .filter(|ty| !matches!(ty, noble_kernel::types::Ty::LiveRef(..))).count();
    let prefix = &actual[..actual.len() - value_count];
    let mut resulting = alloc::vec::Vec::with_capacity(prefix.len().saturating_add(output.len()));
    resulting.extend_from_slice(prefix);
    resulting.extend_from_slice(output);
    let after = attempt!(state.arena.stack(&resulting, span, meter));
    let invoked = ceiling.union(&noble_kernel::types::EffSet::from_ids(
        &[noble_kernel::contracts::LIVE_DISPATCH],
    ));
    let effect = attempt!(state.arena.effect_constant(&invoked, span, meter));
    frame.effect = attempt!(state.arena.effect_union(frame.effect, effect, span, meter));
    let node = attempt!(super::add(frame.body, super::Draft {
        kind: super::DraftKind::SlotInvoke {
            ref_ordinal: ordinal,
            forwarded_ref_ordinals: forwarded,
        },
        variables: alloc::vec::Vec::new(),
        text: None,
        span,
    }, state, meter));
    frame.sequence.push(node);
    frame.stack = after;
    Ok(())
}

fn mismatched_literal_origin(
    frame: &super::Frame,
    state: &super::State,
    scheme: &noble_kernel::words::Scheme,
) -> Option<crate::Span> {
    let node = frame.sequence.last()?;
    let draft = state
        .bodies
        .get(frame.body)?
        .nodes
        .get(usize::try_from(node.0).ok()?)?;
    let super::DraftKind::Literal(literal) = &draft.kind else {
        return None;
    };
    let expected = scheme.stack_in.last()?;
    let matches = match (expected, literal) {
        (noble_kernel::shapes::Pattern::I64, noble_kernel::untrusted::Lit::I64(_))
        | (noble_kernel::shapes::Pattern::Bool, noble_kernel::untrusted::Lit::Bool(_))
        | (noble_kernel::shapes::Pattern::Text, noble_kernel::untrusted::Lit::Text)
        | (noble_kernel::shapes::Pattern::Unit, noble_kernel::untrusted::Lit::Unit) => true,
        (
            noble_kernel::shapes::Pattern::I64
            | noble_kernel::shapes::Pattern::Bool
            | noble_kernel::shapes::Pattern::Text
            | noble_kernel::shapes::Pattern::Unit,
            _,
        ) => false,
        _ => return None,
    };
    (!matches).then_some(draft.span)
}

fn contextualize(
    mut error: crate::Diagnostic,
    definition: noble_kernel::contracts::Definition,
    behavior: Option<noble_kernel::contracts::Behavior>,
    caller: Option<crate::Span>,
    origin: Option<crate::Span>,
    stacks: Option<(alloc::string::String, alloc::string::String)>,
) -> crate::Diagnostic {
    let (expected_stack, actual_stack) = match stacks {
        Some(stacks) => stacks,
        None => return error,
    };
    let mut message = alloc::string::String::from("stack/program join: expected ");
    message.push_str(&expected_stack);
    message.push_str("; actual ");
    message.push_str(&actual_stack);
    message.push_str("; word ");
    let word_start = message.len();
    crate::program::append_bootstrap_spelling(definition, &mut message);
    let word = alloc::string::String::from(&message[word_start..]);
    message.push_str("; ");
    message.push_str(&error.message);
    if let Some(caller) = caller {
        message.push_str("; while specializing a resolved definition");
        error.span = caller;
    }
    error.message = message;
    error.with_join(crate::JoinDiagnostic {
        word,
        expected_stack,
        actual_stack,
        constraint: if matches!(
            behavior,
            Some(
                noble_kernel::contracts::Behavior::If
                    | noble_kernel::contracts::Behavior::Case
                    | noble_kernel::contracts::Behavior::ListCase
            )
        ) {
            "branch-join"
        } else {
            "stack-type"
        },
        value_origin: if caller.is_some() { None } else { origin },
    })
}
