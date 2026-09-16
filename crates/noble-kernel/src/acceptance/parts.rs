//! Pure helpers for the acceptance machine: instantiation, joins, charges,
//! limits, and diagnostics. No helper recurses or mutates its inputs.

/// Charge work, failing closed before the declared limit is exceeded.
pub fn charge(work: u32, cost: u32) -> Result<u32, super::Fail> {
    match work.checked_sub(cost) {
        Some(remaining) => Ok(remaining),
        None => Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work)),
    }
}

/// The work charged for one node's instantiation.
pub fn scheme_cost(scheme: &crate::words::Scheme) -> u32 {
    u32::try_from(scheme.stack_in.len().saturating_add(scheme.stack_out.len()))
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

/// The work charged for one join.
pub fn join_cost(interface: &crate::untrusted::Interface) -> u32 {
    u32::try_from(
        interface
            .stack_in
            .len()
            .saturating_add(interface.stack_out.len()),
    )
    .unwrap_or(u32::MAX)
    .saturating_add(1)
}

/// Validate one stack against the declared height and type-size limits.
pub fn limits_of(
    stack: &[crate::types::Ty],
    request: &crate::untrusted::Request,
) -> Result<(), super::Fail> {
    if u64::try_from(stack.len()).unwrap_or(u64::MAX) > u64::from(request.limits.stack_height) {
        return Err(super::Fail::Exhausted(
            crate::untrusted::LimitKind::StackHeight,
        ));
    }
    for ty in stack {
        if ty.size() > request.limits.type_size {
            return Err(super::Fail::Exhausted(
                crate::untrusted::LimitKind::TypeSize,
            ));
        }
    }
    Ok(())
}

/// The failing definition of a node, when it is an invocation.
pub fn definition_of(
    candidate: &crate::untrusted::Candidate,
    node: crate::untrusted::NodeId,
) -> Option<crate::contracts::Definition> {
    match candidate
        .nodes
        .get(usize::try_from(node.0).unwrap_or(usize::MAX))
    {
        Some(crate::untrusted::Node::Invocation { def, .. }) => Some(*def),
        Some(crate::untrusted::Node::Literal { .. })
        | Some(crate::untrusted::Node::Quotation { .. })
        | None => None,
    }
}

/// The value slot that requires `Data`, for behaviors that constrain one.
pub fn data_slot(behavior: Option<crate::contracts::Behavior>) -> Option<crate::words::Variable> {
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

/// The literal scheme: `S -- S <literal type>`.
pub fn literal_scheme(lit: crate::untrusted::Lit) -> crate::words::Scheme {
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
pub fn quotation_scheme() -> crate::words::Scheme {
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
                },)
            )),
        ],
        effects: alloc::vec![],
    }
}

/// Instantiate one scheme into a concrete interface, validating the witness.
pub fn instantiate(
    scheme: &crate::words::Scheme,
    inst: &crate::words::Inst,
    data_var: Option<crate::words::Variable>,
    def: Option<crate::contracts::Definition>,
    node: crate::untrusted::NodeId,
    request: &crate::untrusted::Request,
    env: &crate::contracts::Env,
) -> Result<crate::untrusted::Interface, super::Fail> {
    let max_effects = env.len();
    match scheme.check_inst(
        inst,
        request.limits.stack_height,
        request.limits.type_size,
        max_effects,
    ) {
        Ok(()) => {}
        Err(crate::words::InstError::KindMismatch)
        | Err(crate::words::InstError::UnknownVariable) => {
            return Err(invalid(
                request,
                Some(node),
                def,
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::InstantiationKind,
            ))
        }
        Err(crate::words::InstError::ArityMismatch) => {
            return Err(invalid(
                request,
                Some(node),
                def,
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::InstantiationArity,
            ))
        }
        Err(crate::words::InstError::OversizedStack) => {
            return Err(super::Fail::Exhausted(
                crate::untrusted::LimitKind::StackHeight,
            ))
        }
        Err(crate::words::InstError::OversizedType)
        | Err(crate::words::InstError::OversizedEffects) => {
            return Err(super::Fail::Exhausted(
                crate::untrusted::LimitKind::TypeSize,
            ))
        }
    }
    let stack_in = match scheme.subst_stack(&scheme.stack_in, inst) {
        Ok(stack) => stack,
        Err(_) => return Err(instantiation_invalid(request, node, def)),
    };
    let stack_out = match scheme.subst_stack(&scheme.stack_out, inst) {
        Ok(stack) => stack,
        Err(_) => return Err(instantiation_invalid(request, node, def)),
    };
    let effects = match scheme.subst_effects(&scheme.effects, inst) {
        Ok(effects) => effects,
        Err(_) => return Err(instantiation_invalid(request, node, def)),
    };
    limits_of(&stack_in, request)?;
    limits_of(&stack_out, request)?;
    for id in effects.as_slice() {
        if !env.knows_effect(*id) {
            return Err(invalid(
                request,
                Some(node),
                def,
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::UnknownEffect(*id),
            ));
        }
    }
    if let Some(var) = data_var {
        match inst.value(var) {
            Some(ty) => {
                if !ty.is_data() {
                    let considered = ty.clone();
                    return Err(invalid(
                        request,
                        Some(node),
                        def,
                        alloc::vec::Vec::new(),
                        alloc::vec::Vec::new(),
                        crate::untrusted::Constraint::Eligibility(considered),
                    ));
                }
            }
            None => return Err(instantiation_invalid(request, node, def)),
        }
    }
    Ok(crate::untrusted::Interface {
        stack_in,
        stack_out,
        effects,
    })
}

fn instantiation_invalid(
    request: &crate::untrusted::Request,
    node: crate::untrusted::NodeId,
    def: Option<crate::contracts::Definition>,
) -> super::Fail {
    invalid(
        request,
        Some(node),
        def,
        alloc::vec::Vec::new(),
        alloc::vec::Vec::new(),
        crate::untrusted::Constraint::InstantiationKind,
    )
}

/// Join one interface into a frame's running stack.
pub fn join(
    frame: super::Frame,
    interface: &crate::untrusted::Interface,
    node: crate::untrusted::NodeId,
    def: Option<crate::contracts::Definition>,
    request: &crate::untrusted::Request,
) -> Result<super::Frame, super::Fail> {
    let mut joined = frame;
    if let Some(constraint) = match_tail(&joined.stack, &interface.stack_in) {
        return Err(invalid(
            request,
            Some(node),
            def,
            interface.stack_in.clone(),
            tail_copy(&joined.stack, interface.stack_in.len()),
            constraint,
        ));
    }
    let keep = joined.stack.len().saturating_sub(interface.stack_in.len());
    joined.stack.truncate(keep);
    joined.stack.extend_from_slice(&interface.stack_out);
    joined.effects = joined.effects.union(&interface.effects);
    limits_of(&joined.stack, request)?;
    Ok(joined)
}

/// Whether the stack's top matches the expected segment.
fn match_tail(
    stack: &[crate::types::Ty],
    expected: &[crate::types::Ty],
) -> Option<crate::untrusted::Constraint> {
    if stack.len() < expected.len() {
        return Some(crate::untrusted::Constraint::StackJoin);
    }
    let offset = stack.len() - expected.len();
    for (index, ty) in expected.iter().enumerate() {
        if &stack[offset + index] != ty {
            let actual = tail_copy(stack, expected.len());
            return Some(mismatch_constraint(expected, &actual));
        }
    }
    None
}

/// Classify a mismatch as wrong order or wrong shape.
pub fn mismatch_constraint(
    expected: &[crate::types::Ty],
    actual: &[crate::types::Ty],
) -> crate::untrusted::Constraint {
    if expected.len() == actual.len() && same_multiset(expected, actual) {
        crate::untrusted::Constraint::StackOrder
    } else {
        crate::untrusted::Constraint::StackJoin
    }
}

/// The top `needed` entries, or the whole stack when it is shorter.
pub fn tail_copy(stack: &[crate::types::Ty], needed: usize) -> alloc::vec::Vec<crate::types::Ty> {
    if stack.len() >= needed {
        stack[stack.len() - needed..].to_vec()
    } else {
        stack.to_vec()
    }
}

fn same_multiset(left: &[crate::types::Ty], right: &[crate::types::Ty]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut used = alloc::vec![false; right.len()];
    for item in left {
        let mut found = false;
        for (index, other) in right.iter().enumerate() {
            if !used[index] && other == item {
                used[index] = true;
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

/// The first derived identity outside the allowed bound.
pub fn first_extra(
    derived: &crate::types::EffSet,
    allowed: &crate::types::EffSet,
) -> Option<crate::types::EffId> {
    derived
        .as_slice()
        .iter()
        .find(|id| !allowed.contains(**id))
        .copied()
}

/// Build one diagnostic under the declared diagnostic budget.
pub fn invalid(
    request: &crate::untrusted::Request,
    node: Option<crate::untrusted::NodeId>,
    def: Option<crate::contracts::Definition>,
    expected: alloc::vec::Vec<crate::types::Ty>,
    actual: alloc::vec::Vec<crate::types::Ty>,
    constraint: crate::untrusted::Constraint,
) -> super::Fail {
    let budget = usize::try_from(request.limits.diagnostics).unwrap_or(usize::MAX);
    let mut truncated = false;
    let mut expected = expected;
    let mut actual = actual;
    if expected.len().saturating_add(actual.len()) > budget {
        truncated = true;
        let keep_expected = budget.saturating_sub(actual.len());
        expected.truncate(keep_expected);
        if expected.len().saturating_add(actual.len()) > budget {
            let keep_actual = budget.saturating_sub(expected.len());
            actual.truncate(keep_actual);
        }
    }
    super::Fail::Invalid(crate::untrusted::Diagnostic {
        node,
        def,
        expected,
        actual,
        constraint,
        provenance_available: false,
        truncated,
    })
}
