#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; exact scheme identity uses non-const structural PartialEq on owned vectors; retaining complete comparisons is required for environment admission."
)]
fn same_scheme(left: &noble_kernel::words::Scheme, right: &noble_kernel::words::Scheme) -> bool {
    left.var_kinds == right.var_kinds
        && left.stack_in == right.stack_in
        && left.stack_out == right.stack_out
        && left.effects == right.effects
}

fn abort_scheme() -> noble_kernel::words::Scheme {
    noble_kernel::words::Scheme {
        var_kinds: alloc::vec![noble_kernel::words::VariableKind::Stack],
        stack_in: alloc::vec![noble_kernel::shapes::Pattern::StackVar(
            noble_kernel::words::Variable(0)
        )],
        stack_out: alloc::vec![noble_kernel::shapes::Pattern::StackVar(
            noble_kernel::words::Variable(0)
        )],
        effects: alloc::vec![noble_kernel::shapes::EffectSlot::Effect(
            noble_kernel::types::EffId(1)
        )],
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every supplied environment row, dependency list and effect bound is untrusted and checked explicitly against the fixed bootstrap contract; mismatches return Invalid instead of asserting."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
) -> Result<(), crate::Diagnostic> {
    let env = &submission.environment;
    if env.defs.len() > super::super::DEFINITION_LIMIT.saturating_add(24)
        || submission.definitions.len() > super::super::DEFINITION_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    if env.defs.len() < 23 || env.kinds.len() != env.defs.len() || env.deps.len() != env.defs.len()
    {
        return Err(crate::Diagnostic::Invalid);
    }
    if !env.schemas.is_empty() {
        return Err(crate::Diagnostic::Invalid);
    }
    let mut fixed = match noble_kernel::contracts::environment() {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Defective),
    };
    fixed.defs[22].stack_out = alloc::vec![noble_kernel::shapes::Pattern::StackVar(
        noble_kernel::words::Variable(0)
    )];
    let mut index = 0usize;
    let mut is_matching = true;
    while index < 23 {
        if !same_scheme(&env.defs[index], &fixed.defs[index])
            || env.kinds[index] != fixed.kinds[index]
            || !env.deps[index].is_empty()
        {
            is_matching = false;
            break;
        }
        index += 1;
    }
    if !is_matching {
        return Err(crate::Diagnostic::Invalid);
    }
    let count = if env.defs.len() == 23 { 23 } else { 24 };
    if count == 24
        && (!same_scheme(&env.defs[23], &abort_scheme())
            || env.kinds[23] != noble_kernel::contracts::Behavior::Named
            || !env.deps[23].is_empty())
    {
        return Err(crate::Diagnostic::Invalid);
    }
    let effects = if count == 23 {
        alloc::vec![noble_kernel::types::EffId(0)]
    } else {
        alloc::vec![noble_kernel::types::EffId(0), noble_kernel::types::EffId(1)]
    };
    if env.effects != effects
        || submission.definitions.len() != env.defs.len().saturating_sub(count)
    {
        return Err(crate::Diagnostic::Invalid);
    }
    while index < env.defs.len() {
        if index >= count && env.kinds[index] != noble_kernel::contracts::Behavior::Named {
            is_matching = false;
            break;
        }
        index += 1;
    }
    if is_matching {
        Ok(())
    } else {
        Err(crate::Diagnostic::Invalid)
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; absent or polymorphic schemes, substitution failures and complete interface mismatches reject untrusted definitions with Invalid; none is a panic invariant."
)]
pub(super) fn exact_contract(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
    expected: &noble_kernel::untrusted::Expected,
) -> Result<(), crate::Diagnostic> {
    let scheme = match env.scheme(def) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Invalid),
    };
    if !scheme.var_kinds.is_empty() {
        return Err(crate::Diagnostic::Invalid);
    }
    let inst = noble_kernel::words::Inst {
        bindings: alloc::vec::Vec::new(),
    };
    let input = match scheme.subst_stack(&scheme.stack_in, &inst) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    let output = match scheme.subst_stack(&scheme.stack_out, &inst) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    let effects = match scheme.subst_effects(&scheme.effects, &inst) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    if input != expected.stack_in
        || output != expected.stack_out
        || effects != expected.allowed_effects
    {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; explicit row and component bounds precede saturating work accounting; oversized environments and unrepresentable costs must exhaust the shared budget rather than panic."
)]
pub(super) fn work(environment: &noble_kernel::contracts::Env) -> Result<u64, crate::Diagnostic> {
    if environment.defs.len() > super::super::DEFINITION_LIMIT.saturating_add(24)
        || environment.deps.len() > super::super::DEFINITION_LIMIT.saturating_add(24)
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut cost = 1u64;
    let mut index = 0usize;
    let mut failure = None;
    while index < environment.defs.len() {
        match scheme_work(&environment.defs[index]) {
            Ok(amount) => {
                cost = cost.saturating_add(amount);
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    index = 0;
    while index < environment.deps.len() {
        if environment.deps[index].len() > environment.defs.len() {
            failure = Some(crate::Diagnostic::Exhausted);
            break;
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(cost),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; explicit component bounds and non-const TryFrom retain exhaustion diagnostics for unrepresentable work; the fixed 512-node kernel bound reserves complete structural scans instead of asserting on supplied schemes."
)]
fn scheme_work(scheme: &noble_kernel::words::Scheme) -> Result<u64, crate::Diagnostic> {
    if scheme.stack_in.len() > super::super::NODE_LIMIT
        || scheme.stack_out.len() > super::super::NODE_LIMIT
        || scheme.var_kinds.len() > super::super::NODE_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    if scheme.effects.len() > super::super::NODE_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    // One pattern's kernel worklist is capped at 512 structural nodes.
    let parts = scheme
        .stack_in
        .len()
        .saturating_add(scheme.stack_out.len())
        .saturating_add(scheme.var_kinds.len())
        .saturating_add(scheme.effects.len())
        .saturating_add(1);
    let parts = match u64::try_from(parts) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    Ok(parts.saturating_mul(512))
}
