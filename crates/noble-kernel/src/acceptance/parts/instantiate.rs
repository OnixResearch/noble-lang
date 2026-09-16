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
        stack_in: alloc::vec![crate::shapes::StackPart::Stack(crate::words::Variable(0))],
        stack_out: alloc::vec![
            crate::shapes::StackPart::Stack(crate::words::Variable(0)),
            crate::shapes::StackPart::Pattern(pattern),
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
        stack_in: alloc::vec![crate::shapes::StackPart::Stack(crate::words::Variable(0))],
        stack_out: alloc::vec![
            crate::shapes::StackPart::Stack(crate::words::Variable(0)),
            crate::shapes::StackPart::Pattern(crate::shapes::Pattern::Program(
                alloc::boxed::Box::new(crate::shapes::Signature {
                    stack_in: alloc::vec![crate::shapes::StackPart::Stack(crate::words::Variable(
                        1
                    ))],
                    stack_out: alloc::vec![crate::shapes::StackPart::Stack(
                        crate::words::Variable(2)
                    )],
                    effects: alloc::vec![crate::shapes::EffectSlot::Var(crate::words::Variable(3))],
                }),
            )),
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
pub(crate) fn apply(
    scheme: &crate::words::Scheme,
    inst: &crate::words::Inst,
    data_var: Option<crate::words::Variable>,
    at: super::Site,
    ctx: &super::Ctx,
) -> Result<crate::untrusted::Interface, super::super::Fail> {
    attempt!(check_bounds(scheme, inst, at, ctx));
    project(scheme, inst, data_var, at, ctx)
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
        | Err(crate::words::InstError::UnknownVariable) => Err(instantiation_invalid(at, ctx)),
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
    let effect_ids = effects.as_slice();
    let mut index = 0;
    while index < effect_ids.len() {
        let id = effect_ids[index];
        if !ctx.env.knows_effect(id) {
            return Err(super::invalid(
                ctx,
                at,
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::UnknownEffect(id),
            ));
        }
        index += 1;
    }
    if let Some(var) = data_var {
        match inst.value(var) {
            Some(ty) => {
                if !ty.is_data() {
                    let considered = ty.clone();
                    return Err(super::invalid(
                        ctx,
                        at,
                        alloc::vec::Vec::new(),
                        alloc::vec::Vec::new(),
                        crate::untrusted::Constraint::Eligibility(considered),
                    ));
                }
            }
            None => return Err(instantiation_invalid(at, ctx)),
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
