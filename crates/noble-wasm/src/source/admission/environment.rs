mod replay;

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

fn live_schemes() -> [noble_kernel::words::Scheme; 2] {
    use noble_kernel::shapes::{EffectSlot, Pattern};
    use noble_kernel::types::EffId;
    use noble_kernel::words::{Scheme, Variable, VariableKind};
    let stack = Pattern::StackVar(Variable(0));
    [
        Scheme {
            var_kinds: alloc::vec![VariableKind::Stack],
            stack_in: alloc::vec![stack.clone(), Pattern::I64,
                Pattern::program(alloc::vec![Pattern::I64], alloc::vec![Pattern::I64],
                    alloc::vec::Vec::new())],
            stack_out: alloc::vec![stack.clone(), Pattern::Unit],
            effects: alloc::vec![EffectSlot::Effect(EffId(3))],
        },
        Scheme {
            var_kinds: alloc::vec![VariableKind::Stack],
            stack_in: alloc::vec![stack.clone()],
            stack_out: alloc::vec![stack, Pattern::I64],
            effects: alloc::vec![EffectSlot::Effect(EffId(4))],
        },
    ]
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every supplied environment row, dependency list and effect bound is untrusted and checked explicitly against the fixed bootstrap contract; mismatches return Invalid instead of asserting."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    live: bool,
    text_cursor: bool,
) -> Result<(), crate::Diagnostic> {
    let env = &submission.environment;
    let maximum_rows = super::super::DEFINITION_LIMIT.saturating_add(27);
    if env.defs.len() > maximum_rows
        || submission.definitions.len() > super::super::DEFINITION_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    if env.defs.len() < 23 {
        return Err(crate::Diagnostic::Invalid);
    }
    if env.kinds.len() != env.defs.len() {
        return Err(crate::Diagnostic::Invalid);
    }
    if env.deps.len() != env.defs.len() {
        return Err(crate::Diagnostic::Invalid);
    }
    if env.definition_owners.len() != env.defs.len() {
        return Err(crate::Diagnostic::Invalid);
    }
    if env.nominals.len() > super::super::DEFINITION_LIMIT
        || env.generic_variants.len() > super::super::DEFINITION_LIMIT
        || env.bound_adapters.len() > super::super::DEFINITION_LIMIT
    {
        return Err(crate::Diagnostic::Invalid);
    }
    if env.text_cursor != text_cursor || (text_cursor && (live || env.declared_modules)) {
        return Err(crate::Diagnostic::Invalid);
    }
    let has_noncanonical_core_declarations = !env.declared_modules
        && (!env.nominals.is_empty()
            || !env.generic_variants.is_empty()
            || !env.bound_adapters.is_empty());
    let has_invalid_metadata = !env.schemas.is_empty()
        || env.caller_module.is_some()
        || env.resource_kinds != [noble_kernel::contracts::FIXTURE_RESOURCE];
    if has_invalid_metadata || has_noncanonical_core_declarations {
        return Err(crate::Diagnostic::Invalid);
    }
    let (fixed, count) = attempt!(canonical_prefix(env, live, text_cursor));
    let effects = if count == 23 || text_cursor {
        alloc::vec![noble_kernel::types::EffId(0)]
    } else {
        alloc::vec![noble_kernel::types::EffId(0), noble_kernel::types::EffId(1)]
    };
    let mut effects = effects;
    if env.kinds.iter().any(|kind| matches!(kind, noble_kernel::contracts::Behavior::BoundClock(_))) {
        effects.push(noble_kernel::contracts::TEST_CLOCK);
    }
    if count == 26 {
        effects.push(noble_kernel::types::EffId(3));
        effects.push(noble_kernel::types::EffId(4));
    }
    if env.effects != effects {
        return Err(crate::Diagnostic::Invalid);
    }
    if live && (submission.request.expected.allowed_effects.as_slice().iter()
        .chain(submission.definitions.iter().flat_map(|definition| {
            definition.expected.allowed_effects.as_slice().iter()
        }))
        .any(|effect| !matches!(effect.0, 3 | 4)))
    {
        return Err(crate::Diagnostic::Invalid);
    }
    replay::check(
        env,
        fixed,
        replay::ExpectedRows {
            prefix: count,
            named: submission.definitions.len(),
        },
    )
}

fn canonical_prefix(
    env: &noble_kernel::contracts::Env,
    live: bool,
    text_cursor: bool,
) -> Result<(noble_kernel::contracts::Env, usize), crate::Diagnostic> {
    if text_cursor {
        let fixed = attempt!(noble_kernel::contracts::text_cursor_environment()
            .map_err(|_| crate::Diagnostic::Defective));
        if env.defs.len() < 27 {
            return Err(crate::Diagnostic::Invalid);
        }
        let mut index = 0usize;
        while index < 27 {
            if !same_scheme(&env.defs[index], &fixed.defs[index])
                || env.kinds[index] != fixed.kinds[index]
                || !env.deps[index].is_empty()
                || env.definition_owners[index].is_some()
            {
                return Err(crate::Diagnostic::Invalid);
            }
            index += 1;
        }
        return Ok((fixed, 27));
    }
    let mut fixed = attempt!(bootstrap(env));
    attempt!(check_builtin_rows(env, &fixed));
    let has_abort = env.defs.len() > 23
        && env.kinds[23] == noble_kernel::contracts::Behavior::Named
        && same_scheme(&env.defs[23], &abort_scheme())
        && env.definition_owners[23].is_none()
        && env.deps[23].is_empty();
    if has_abort {
        fixed.defs.push(abort_scheme());
        fixed.kinds.push(noble_kernel::contracts::Behavior::Named);
        fixed.deps.push(alloc::vec::Vec::new());
        fixed.definition_owners.push(None);
    }
    if live {
        if env.declared_modules || !has_abort || env.defs.len() < 26 {
            return Err(crate::Diagnostic::Invalid);
        }
        for (index, scheme) in live_schemes().into_iter().enumerate() {
            let row = index + 24;
            if !same_scheme(&env.defs[row], &scheme)
                || env.kinds[row] != noble_kernel::contracts::Behavior::Named
                || !env.deps[row].is_empty()
                || env.definition_owners[row].is_some()
            {
                return Err(crate::Diagnostic::Invalid);
            }
            fixed.defs.push(scheme);
            fixed.kinds.push(noble_kernel::contracts::Behavior::Named);
            fixed.deps.push(alloc::vec::Vec::new());
            fixed.definition_owners.push(None);
        }
        return Ok((fixed, 26));
    }
    Ok((fixed, if has_abort { 24 } else { 23 }))
}

fn bootstrap(
    env: &noble_kernel::contracts::Env,
) -> Result<noble_kernel::contracts::Env, crate::Diagnostic> {
    let mut fixed =
        attempt!(noble_kernel::contracts::environment().map_err(|_| crate::Diagnostic::Defective));
    if env.declared_modules {
        fixed.declared_modules = true;
    } else {
        // Core source sessions retain the historical omission of the fixed
        // host operation's Unit result. Declared sessions retain the kernel's
        // canonical contract, but cannot invoke that ambient operation.
        fixed.defs[22].stack_out = alloc::vec![noble_kernel::shapes::Pattern::StackVar(
            noble_kernel::words::Variable(0)
        )];
    }
    Ok(fixed)
}

fn check_builtin_rows(
    env: &noble_kernel::contracts::Env,
    fixed: &noble_kernel::contracts::Env,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut result = Ok(());
    while index < 23 {
        let has_incorrect_contract = !same_scheme(&env.defs[index], &fixed.defs[index])
            || env.kinds[index] != fixed.kinds[index];
        let has_supplied_metadata =
            !env.deps[index].is_empty() || env.definition_owners[index].is_some();
        if has_incorrect_contract || has_supplied_metadata {
            result = Err(crate::Diagnostic::Invalid);
            break;
        }
        index += 1;
    }
    result
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
    let maximum_rows = super::super::DEFINITION_LIMIT.saturating_add(27);
    if environment.defs.len() > maximum_rows || environment.deps.len() > maximum_rows {
        return Err(crate::Diagnostic::Exhausted);
    }
    if environment.nominals.len() > super::super::DEFINITION_LIMIT
        || environment.generic_variants.len() > super::super::DEFINITION_LIMIT
        || environment.bound_adapters.len() > super::super::DEFINITION_LIMIT
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
    let mut row_index = 0usize;
    while row_index < environment.bound_adapters.len() {
        let row = &environment.bound_adapters[row_index];
        if row.adapter_identity.len() > 65_536 {
            failure = Some(crate::Diagnostic::Exhausted);
            break;
        }
        cost = cost
            .saturating_add(row.adapter_identity.len() as u64)
            .saturating_add(1024);
        row_index += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    cost = cost.saturating_add(
        (environment
            .nominals
            .len()
            .saturating_add(environment.generic_variants.len()) as u64)
            .saturating_mul(2048),
    );
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
