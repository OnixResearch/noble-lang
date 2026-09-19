//! Instantiation of one scheme into a concrete interface, with the
//! eligibility and effect-identity checks the fragment requires.

/// The literal scheme: `S -- S <literal type>`.
pub(crate) fn literal_scheme(lit: crate::untrusted::Lit) -> crate::words::Scheme {
    let pattern = match lit {
        crate::untrusted::Lit::I64(_) => crate::shapes::Pattern::I64,
        crate::untrusted::Lit::Bool(_) => crate::shapes::Pattern::Bool,
        crate::untrusted::Lit::Text => crate::shapes::Pattern::Text,
        crate::untrusted::Lit::Unit => crate::shapes::Pattern::Unit,
    };
    crate::words::Scheme {
        var_kinds: alloc::vec![crate::words::VariableKind::Stack],
        stack_in: alloc::vec![crate::shapes::Pattern::StackVar(crate::words::Variable(0))],
        stack_out: alloc::vec![
            crate::shapes::Pattern::StackVar(crate::words::Variable(0)),
            pattern,
        ],
        effects: alloc::vec![],
    }
}

/// The quotation scheme: `R -- R Program<A, C, e>`.
pub(crate) fn quotation_scheme() -> crate::words::Scheme {
    crate::words::Scheme {
        var_kinds: alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect
        ],
        stack_in: alloc::vec![crate::shapes::Pattern::StackVar(crate::words::Variable(0))],
        stack_out: alloc::vec![
            crate::shapes::Pattern::StackVar(crate::words::Variable(0)),
            crate::shapes::Pattern::program(
                alloc::vec![crate::shapes::Pattern::StackVar(crate::words::Variable(1))],
                alloc::vec![crate::shapes::Pattern::StackVar(crate::words::Variable(2))],
                alloc::vec![crate::shapes::EffectSlot::Var(crate::words::Variable(3))],
            ),
        ],
        effects: alloc::vec![],
    }
}

/// The value slot that requires `Data`, for behaviors that constrain one.
pub(crate) fn data_slot(
    behavior: Option<crate::contracts::Behavior>,
) -> Option<crate::words::Variable> {
    match behavior {
        Some(crate::contracts::Behavior::Dup)
        | Some(crate::contracts::Behavior::Drop)
        | Some(crate::contracts::Behavior::Quote) => Some(crate::words::Variable(1)),
        Some(crate::contracts::Behavior::Swap)
        | Some(crate::contracts::Behavior::Dip)
        | Some(crate::contracts::Behavior::Arith)
        | Some(crate::contracts::Behavior::Equals)
        | Some(crate::contracts::Behavior::Compose)
        | Some(crate::contracts::Behavior::Run)
        | Some(crate::contracts::Behavior::Reflect)
        | Some(crate::contracts::Behavior::Unit)
        | Some(crate::contracts::Behavior::Pair)
        | Some(crate::contracts::Behavior::Unpair)
        | Some(crate::contracts::Behavior::Inl)
        | Some(crate::contracts::Behavior::Inr)
        | Some(crate::contracts::Behavior::Case)
        | Some(crate::contracts::Behavior::If)
        | Some(crate::contracts::Behavior::Nil)
        | Some(crate::contracts::Behavior::Cons)
        | Some(crate::contracts::Behavior::ListCase)
        | Some(crate::contracts::Behavior::TestEmit)
        | Some(crate::contracts::Behavior::Named)
        | None => None,
    }
}

/// Instantiate one scheme into a concrete interface, validating the witness.
///
/// The witness's reference bindings are resolved first (B-CHECK-05): a
/// cyclic chain rejects as invalid, a chain that would outrun the declared
/// work limit rejects fail-closed, and the resolved witness returns with
/// the interface so callers can read its segments directly.
pub(crate) fn apply(
    scheme: &crate::words::Scheme,
    inst: &crate::words::Inst,
    data_var: Option<crate::words::Variable>,
    at: super::Site,
    ctx: &super::Ctx,
) -> Result<(crate::untrusted::Interface, crate::words::Inst), super::super::Fail> {
    attempt!(check_bounds(scheme, inst, at, ctx));
    let resolved = attempt!(resolve_witness(scheme, inst, at, ctx));
    let interface = attempt!(project(scheme, &resolved, data_var, at, ctx));
    Ok((interface, resolved))
}

/// Resolve one witness's reference bindings under the declared work limit.
fn resolve_witness(
    scheme: &crate::words::Scheme,
    inst: &crate::words::Inst,
    at: super::Site,
    ctx: &super::Ctx,
) -> Result<crate::words::Inst, super::super::Fail> {
    match crate::words::resolve::resolve(&scheme.var_kinds, inst, ctx.request.limits.work) {
        Ok((resolved, _spent)) => Ok(resolved),
        Err(crate::words::InstError::CyclicWitness) => Err(super::invalid(
            ctx,
            at,
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::CyclicWitness,
        )),
        Err(crate::words::InstError::WalkExhausted) => Err(super::super::Fail::Exhausted(
            crate::untrusted::LimitKind::Work,
        )),
        Err(crate::words::InstError::ArityMismatch) => Err(super::invalid(
            ctx,
            at,
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::InstantiationArity,
        )),
        Err(crate::words::InstError::OversizedStack) => Err(super::super::Fail::Exhausted(
            crate::untrusted::LimitKind::StackHeight,
        )),
        Err(crate::words::InstError::OversizedType)
        | Err(crate::words::InstError::OversizedEffects) => Err(super::super::Fail::Exhausted(
            crate::untrusted::LimitKind::TypeSize,
        )),
        Err(crate::words::InstError::KindMismatch)
        | Err(crate::words::InstError::UnknownVariable) => Err(instantiation_invalid(at, ctx)),
    }
}

fn check_bounds(
    scheme: &crate::words::Scheme,
    inst: &crate::words::Inst,
    at: super::Site,
    ctx: &super::Ctx,
) -> Result<(), super::super::Fail> {
    let max_effects = match ctx.env.len() {
        Some(count) => count,
        None => {
            return Err(super::super::Fail::Unsupported(
                crate::untrusted::UnsupportedKind::SchemeForm,
            ))
        }
    };
    match scheme.check_inst(
        inst,
        ctx.request.limits.stack_height,
        ctx.request.limits.type_size,
        max_effects,
    ) {
        Ok(()) => Ok(()),
        Err(crate::words::InstError::KindMismatch)
        | Err(crate::words::InstError::UnknownVariable)
        | Err(crate::words::InstError::CyclicWitness)
        | Err(crate::words::InstError::WalkExhausted) => Err(instantiation_invalid(at, ctx)),
        Err(crate::words::InstError::ArityMismatch) => Err(super::invalid(
            ctx,
            at,
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::InstantiationArity,
        )),
        Err(crate::words::InstError::OversizedStack) => Err(super::super::Fail::Exhausted(
            crate::untrusted::LimitKind::StackHeight,
        )),
        Err(crate::words::InstError::OversizedType)
        | Err(crate::words::InstError::OversizedEffects) => Err(super::super::Fail::Exhausted(
            crate::untrusted::LimitKind::TypeSize,
        )),
    }
}

fn project(
    scheme: &crate::words::Scheme,
    inst: &crate::words::Inst,
    data_var: Option<crate::words::Variable>,
    at: super::Site,
    ctx: &super::Ctx,
) -> Result<crate::untrusted::Interface, super::super::Fail> {
    // Read the eligible binding as an owned value first: a reference into the
    // instantiation cannot be carried across the substitutions below.
    let eligible: Option<crate::types::Ty> = match data_var {
        Some(var) => match inst.value(var) {
            Some(ty) => Some(ty.clone()),
            None => return Err(instantiation_invalid(at, ctx)),
        },
        None => None,
    };
    let stack_in = match scheme.subst_stack(&scheme.stack_in, inst) {
        Ok(stack) => stack,
        Err(_) => return Err(instantiation_invalid(at, ctx)),
    };
    let stack_out = match scheme.subst_stack(&scheme.stack_out, inst) {
        Ok(stack) => stack,
        Err(_) => return Err(instantiation_invalid(at, ctx)),
    };
    let effects = match scheme.subst_effects(&scheme.effects, inst) {
        Ok(effects) => effects,
        Err(_) => return Err(instantiation_invalid(at, ctx)),
    };
    attempt!(super::limits_of(&stack_in, ctx));
    attempt!(super::limits_of(&stack_out, ctx));
    let effect_ids: alloc::vec::Vec<crate::types::EffId> = effects.as_slice().to_vec();
    let known: alloc::vec::Vec<crate::types::EffId> = ctx.env.effects.to_vec();
    let unknown = super::first_unknown(&effect_ids, &known);
    if let Some(id) = unknown {
        return Err(super::invalid(
            ctx,
            at,
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::UnknownEffect(id),
        ));
    }
    if let Some(ty) = eligible {
        if !ty.is_data() {
            return Err(super::invalid(
                ctx,
                at,
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::Eligibility(ty),
            ));
        }
    }
    Ok(crate::untrusted::Interface {
        stack_in,
        stack_out,
        effects,
    })
}

fn instantiation_invalid(at: super::Site, ctx: &super::Ctx) -> super::super::Fail {
    super::invalid(
        ctx,
        at,
        alloc::vec::Vec::new(),
        alloc::vec::Vec::new(),
        crate::untrusted::Constraint::InstantiationKind,
    )
}
