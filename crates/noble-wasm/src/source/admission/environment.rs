mod replay;
mod weight;

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
    let stack = noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0));
    [
        noble_kernel::words::Scheme {
            var_kinds: alloc::vec![noble_kernel::words::VariableKind::Stack],
            stack_in: alloc::vec![
                stack.clone(),
                noble_kernel::shapes::Pattern::I64,
                noble_kernel::shapes::Pattern::program(
                    alloc::vec![noble_kernel::shapes::Pattern::I64],
                    alloc::vec![noble_kernel::shapes::Pattern::I64],
                    alloc::vec::Vec::new()
                )
            ],
            stack_out: alloc::vec![stack.clone(), noble_kernel::shapes::Pattern::Unit],
            effects: alloc::vec![noble_kernel::shapes::EffectSlot::Effect(
                noble_kernel::types::EffId(3)
            )],
        },
        noble_kernel::words::Scheme {
            var_kinds: alloc::vec![noble_kernel::words::VariableKind::Stack],
            stack_in: alloc::vec![stack.clone()],
            stack_out: alloc::vec![stack, noble_kernel::shapes::Pattern::I64],
            effects: alloc::vec![noble_kernel::shapes::EffectSlot::Effect(
                noble_kernel::types::EffId(4)
            )],
        },
    ]
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every supplied environment row, dependency list and effect bound is untrusted and checked explicitly against the fixed bootstrap contract; mismatches return Invalid instead of asserting."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    live: bool,
    live_slots: bool,
    text_cursor: bool,
    calculator: bool,
    declared_text: bool,
) -> Result<(), crate::Diagnostic> {
    let env = &submission.environment;
    let definition_cap = if calculator {
        super::super::CALCULATOR_DEFINITION_LIMIT
    } else {
        super::super::DEFINITION_LIMIT
    };
    let maximum_rows = if calculator {
        noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
    } else if declared_text {
        definition_cap.saturating_add(28)
    } else {
        definition_cap.saturating_add(27)
    };
    if env.defs.len() > maximum_rows || submission.definitions.len() > definition_cap {
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
    let has_excess_definitions = env.nominals.len() > definition_cap
        || env.generic_variants.len() > definition_cap
        || env.bound_adapters.len() > definition_cap
        || env.live_resource_nominals.len() > super::super::DEFINITION_LIMIT
        || env.resource_kinds.len() > super::super::DEFINITION_LIMIT.saturating_add(1);
    if has_excess_definitions {
        return Err(crate::Diagnostic::Invalid);
    }
    let has_session_shape_mismatch = env.live_slots != live_slots
        || (env.live_test_hosts && !live_slots)
        || (live_slots && (live || text_cursor || !env.declared_modules))
        || env.text_cursor != text_cursor
        || (text_cursor && (live || env.declared_modules != declared_text))
        || (declared_text
            && (!text_cursor
                || calculator
                || !env.declared_modules
                || !env.bound_adapters.is_empty()
                || env.live_slots))
        || env.calculator != calculator
        || (calculator && (!env.declared_modules || !env.bound_adapters.is_empty()))
        || (!live_slots && !env.live_resource_nominals.is_empty());
    if has_session_shape_mismatch {
        return Err(crate::Diagnostic::Invalid);
    }
    let has_noncanonical_core_declarations = !env.declared_modules
        && (!env.nominals.is_empty()
            || !env.generic_variants.is_empty()
            || !env.bound_adapters.is_empty());
    let has_invalid_metadata = !env.schemas.is_empty()
        || env.caller_module.is_some()
        || (!live_slots && env.resource_kinds != [noble_kernel::contracts::FIXTURE_RESOURCE]);
    if has_invalid_metadata || has_noncanonical_core_declarations {
        return Err(crate::Diagnostic::Invalid);
    }
    let (fixed, count) = attempt!(canonical_prefix(
        env,
        live,
        live_slots,
        text_cursor,
        calculator,
        declared_text
    ));
    if !exact_effects(env, count, live_slots) {
        return Err(crate::Diagnostic::Invalid);
    }
    if live
        && (submission
            .request
            .expected
            .allowed_effects
            .as_slice()
            .iter()
            .chain(
                submission
                    .definitions
                    .iter()
                    .flat_map(|definition| definition.expected.allowed_effects.as_slice().iter()),
            )
            .any(|effect| !matches!(effect.0, 3 | 4)))
    {
        return Err(crate::Diagnostic::Invalid);
    }
    let expected = replay::ExpectedRows {
        prefix: count,
        named: submission.definitions.len(),
    };
    if live_slots && !env.live_resource_nominals.is_empty() {
        // A host resource is a type descriptor, not a guest constructor row.
        // Replay the ordinary declarations without these already-reconstructed
        // descriptors; replay still checks every actual definition in order.
        let mut ordinary = env.clone();
        ordinary
            .nominals
            .retain(|decl| !env.live_resource_nominals.contains(&decl.id));
        if ordinary
            .nominals
            .len()
            .saturating_add(env.live_resource_nominals.len())
            != env.nominals.len()
        {
            return Err(crate::Diagnostic::Invalid);
        }
        replay::check(&ordinary, fixed, expected)
    } else {
        replay::check(env, fixed, expected)
    }
}

#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
)]
fn exact_effects(env: &noble_kernel::contracts::Env, count: usize, live_slots: bool) -> bool {
    let has_clock = env
        .kinds
        .iter()
        .any(|kind| matches!(kind, noble_kernel::contracts::Behavior::BoundClock(_)));
    let has_code_export = env
        .kinds
        .iter()
        .any(|kind| matches!(kind, noble_kernel::contracts::Behavior::BoundCodeExport(_)));
    let has_send = env
        .kinds
        .iter()
        .any(|kind| matches!(kind, noble_kernel::contracts::Behavior::BoundSend(_)));
    if live_slots {
        // Clock registration may precede or follow the host's slot opt-in.
        // Both orderings are produced by the checked Env APIs, but no other
        // identity, duplication, or slot effect is ambiently available.
        return if has_clock {
            env.effects
                == [
                    noble_kernel::contracts::TEST_EMIT,
                    noble_kernel::contracts::LIVE_DISPATCH,
                    noble_kernel::contracts::TEST_CLOCK,
                ]
                || env.effects
                    == [
                        noble_kernel::contracts::TEST_EMIT,
                        noble_kernel::contracts::TEST_CLOCK,
                        noble_kernel::contracts::LIVE_DISPATCH,
                    ]
        } else {
            env.effects
                == [
                    noble_kernel::contracts::TEST_EMIT,
                    noble_kernel::contracts::LIVE_DISPATCH,
                ]
        };
    }
    let mut effects = if count == 23 || count == 27 || count == 28 || count == 32 {
        alloc::vec![noble_kernel::contracts::TEST_EMIT]
    } else {
        alloc::vec![
            noble_kernel::contracts::TEST_EMIT,
            noble_kernel::types::EffId(1)
        ]
    };
    if count == 26 {
        effects.push(noble_kernel::types::EffId(3));
        effects.push(noble_kernel::types::EffId(4));
    }
    // Each bound operation's effect is appended once, in registration order,
    // so the tail is exactly a permutation of the registered bound effects.
    let optional = [
        (has_clock, noble_kernel::contracts::TEST_CLOCK),
        (has_code_export, noble_kernel::contracts::TEST_CODE_EXPORT),
        (has_send, noble_kernel::contracts::TEST_SEND),
    ];
    let Some(tail) = env.effects.get(effects.len()..) else {
        return false;
    };
    env.effects.starts_with(&effects)
        && tail.len() == optional.iter().filter(|(present, _)| *present).count()
        && optional
            .iter()
            .all(|(present, effect)| tail.contains(effect) == *present)
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-wasm-maintainers; each fixed definition row owns a fresh empty dependency list, so one buffer cannot be moved into every row."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
fn canonical_prefix(
    env: &noble_kernel::contracts::Env,
    live: bool,
    live_slots: bool,
    text_cursor: bool,
    calculator: bool,
    declared_text: bool,
) -> Result<(noble_kernel::contracts::Env, usize), crate::Diagnostic> {
    if calculator {
        let fixed = attempt!(noble_kernel::contracts::calculator_environment()
            .map_err(|_| crate::Diagnostic::Defective));
        if env.defs.len() < 32 {
            return Err(crate::Diagnostic::Invalid);
        }
        let mut index = 0usize;
        while index < 32 {
            let has_definition_drift = !same_scheme(&env.defs[index], &fixed.defs[index])
                || env.kinds[index] != fixed.kinds[index]
                || !env.deps[index].is_empty()
                || env.definition_owners[index].is_some();
            if has_definition_drift {
                return Err(crate::Diagnostic::Invalid);
            }
            index += 1;
        }
        return Ok((fixed, 32));
    }
    if text_cursor {
        let fixed = attempt!((if declared_text {
            noble_kernel::contracts::declared_text_environment()
        } else {
            noble_kernel::contracts::text_cursor_environment()
        })
        .map_err(|_| crate::Diagnostic::Defective));
        let count = fixed.defs.len();
        if env.defs.len() < count {
            return Err(crate::Diagnostic::Invalid);
        }
        let mut index = 0usize;
        while index < count {
            let has_definition_drift = !same_scheme(&env.defs[index], &fixed.defs[index])
                || env.kinds[index] != fixed.kinds[index]
                || !env.deps[index].is_empty()
                || env.definition_owners[index].is_some();
            if has_definition_drift {
                return Err(crate::Diagnostic::Invalid);
            }
            index += 1;
        }
        return Ok((fixed, count));
    }
    let mut fixed = attempt!(bootstrap(env));
    attempt!(check_builtin_rows(env, &fixed));
    if live_slots {
        // Start with the immutable bootstrap and reconstruct only host
        // registrations via the kernel's validated, constructor-free API.
        // An unlisted catalog entry or altered resource kind cannot authorize
        // a candidate's nominal/resource interface.
        fixed = fixed.enable_live_slots();
        for id in &env.live_resource_nominals {
            let decl = env.nominal(*id).ok_or(crate::Diagnostic::Invalid)?;
            fixed = fixed
                .register_live_resource(decl.clone())
                .map_err(|_| crate::Diagnostic::Invalid)?;
        }
        if fixed.resource_kinds != env.resource_kinds
            || fixed.live_resource_nominals != env.live_resource_nominals
        {
            return Err(crate::Diagnostic::Invalid);
        }
        return Ok((fixed, 23));
    }
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
            let has_scheme_row_drift = !same_scheme(&env.defs[row], &scheme)
                || env.kinds[row] != noble_kernel::contracts::Behavior::Named
                || !env.deps[row].is_empty()
                || env.definition_owners[row].is_some();
            if has_scheme_row_drift {
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
    signed_owner_inst: Option<&noble_kernel::words::Inst>,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let scheme = match env.scheme(def) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Invalid),
    };
    // The one-time full environment scan and each independent kernel
    // acceptance reserve environment_work. This check substitutes only the
    // selected definition's scheme, not every row in the environment.
    attempt!(work.charge(attempt!(scheme_work(scheme))));
    let empty = noble_kernel::words::Inst {
        bindings: alloc::vec::Vec::new(),
    };
    let inst = match signed_owner_inst {
        Some(inst) => inst,
        None if scheme.var_kinds.is_empty() => &empty,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let input = match scheme.subst_stack(&scheme.stack_in, inst) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    let output = match scheme.subst_stack(&scheme.stack_out, inst) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    let effects = match scheme.subst_effects(&scheme.effects, inst) {
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

pub(super) fn contract_work(
    env: &noble_kernel::contracts::Env,
    def: noble_kernel::contracts::Definition,
    work: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let scheme = attempt!(env.scheme(def).ok_or(crate::Diagnostic::Invalid));
    weight::scheme(scheme, attempt!(lookup_rows(env)), work)
}

fn lookup_rows(env: &noble_kernel::contracts::Env) -> Result<usize, crate::Diagnostic> {
    env.nominals
        .len()
        .checked_add(env.generic_variants.len())
        .ok_or(crate::Diagnostic::Exhausted)
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; explicit row and component bounds precede saturating work accounting; oversized environments and unrepresentable costs must exhaust the shared budget rather than panic."
)]
pub(super) fn work(
    environment: &noble_kernel::contracts::Env,
    calculator: bool,
    meter: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let definition_cap = if calculator {
        super::super::CALCULATOR_DEFINITION_LIMIT
    } else {
        super::super::DEFINITION_LIMIT
    };
    let maximum_rows = if calculator {
        noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
    } else {
        definition_cap.saturating_add(27)
    };
    if environment.defs.len() > maximum_rows || environment.deps.len() > maximum_rows {
        return Err(crate::Diagnostic::Exhausted);
    }
    if environment.nominals.len() > definition_cap
        || environment.generic_variants.len() > definition_cap
        || environment.bound_adapters.len() > definition_cap
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut cost = 1u64;
    let selected_lookup_rows = if calculator {
        attempt!(lookup_rows(environment))
    } else {
        0
    };
    let mut index = 0usize;
    let mut failure = None;
    while index < environment.defs.len() {
        let row = if calculator {
            weight::scheme(&environment.defs[index], selected_lookup_rows, meter)
        } else {
            scheme_work(&environment.defs[index])
        };
        match row {
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

// Test harness only: scenario-named tests over checked fixtures.
#[allow(
    tigerstyle::non_trait_imports,
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; test harness: descriptive scenario names, fixture imports and long setup legs are the readable form."
)]
#[cfg(test)]
mod contract_meter_tests {
    use super::{exact_contract, scheme_work, work};
    use crate::{source::Work, Diagnostic};
    use noble_kernel::{
        contracts::Definition,
        types::{EffSet, Ty},
        untrusted::Expected,
    };

    fn budget(remaining: u64) -> Work {
        Work {
            remaining,
            issued: remaining,
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        }
    }

    #[test]
    fn local_contract_check_charges_its_scheme_without_rechecking_all_environment_rows() {
        let env = noble_kernel::contracts::environment().expect("fixed environment");
        let checked = Definition(0);
        let cost = scheme_work(env.scheme(checked).expect("fixed scheme"))
            .ok()
            .expect("bounded scheme");
        let mismatched = Expected {
            stack_in: alloc::vec![Ty::Text],
            stack_out: alloc::vec::Vec::new(),
            allowed_effects: EffSet::empty(),
        };
        let mut insufficient = budget(cost - 1);
        assert!(matches!(
            exact_contract(&env, checked, &mismatched, None, &mut insufficient),
            Err(Diagnostic::Exhausted)
        ));
        let mut sufficient = budget(cost);
        assert!(matches!(
            exact_contract(&env, checked, &mismatched, None, &mut sufficient),
            Err(Diagnostic::Invalid)
        ));
        assert_eq!(sufficient.remaining, 0);
        assert!(
            work(&env, false, &mut budget(u64::MAX))
                .ok()
                .expect("full environment budget")
                > cost
        );
    }

    #[test]
    fn oversized_scheme_still_exhausts_before_contract_or_kernel_checks() {
        let mut env = noble_kernel::contracts::environment().expect("fixed environment");
        let selected = env.defs.get_mut(0).expect("fixed scheme");
        selected.stack_out = (0..=crate::source::NODE_LIMIT)
            .map(|_| noble_kernel::shapes::Pattern::Unit)
            .collect();
        assert!(matches!(
            work(&env, false, &mut budget(u64::MAX)),
            Err(Diagnostic::Exhausted)
        ));
    }
}

// Test harness only: scenario-named tests over checked fixtures.
#[allow(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; test harness: descriptive scenario names, fixture imports and long setup legs are the readable form."
)]
#[cfg(test)]
mod declared_text_profile_tests {
    #[test]
    fn merged_prefix_rejects_mutations_even_under_selected_profile() {
        let limits = noble_contracts::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let retained = noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid retained source cap");
        let session =
            noble_contracts::source::ModuleSession::new_declared_text(&[], false, retained)
                .expect("selected source profile");
        let expression = session
            .prepare(b"\"7\" 0 text.byte", &[], limits)
            .expect("source-checked explicit Text cursor");
        let original = expression
            .declared_text_source_owner()
            .expect("checked source token")
            .rederive()
            .expect("source-checked expression");
        // (submission, live, live_slots, text_cursor, calculator, declared_text)
        assert!(super::check(&original, false, false, true, false, true).is_ok());
        for index in 22..28 {
            let mut forged = original.clone();
            forged.environment.kinds[index] = if (23..=25).contains(&index) {
                noble_kernel::contracts::Behavior::TextByte
            } else {
                noble_kernel::contracts::Behavior::Reserved
            };
            assert!(
                super::check(&forged, false, false, true, false, true).is_err(),
                "altered fixed row {index} passed merged admission"
            );
        }
        let mut forged = original.clone();
        forged.environment.deps[26].push(noble_kernel::contracts::Definition(0));
        assert!(super::check(&forged, false, false, true, false, true).is_err());
        let mut forged = original.clone();
        forged.environment.definition_owners[26] = Some(1);
        assert!(super::check(&forged, false, false, true, false, true).is_err());
        let mut forged = original.clone();
        forged.environment.defs[26].stack_in.clear();
        assert!(super::check(&forged, false, false, true, false, true).is_err());
        let mut forged = original.clone();
        forged.environment.text_cursor = false;
        assert!(super::check(&forged, false, false, true, false, true).is_err());
        let mut forged = original.clone();
        forged.environment.declared_modules = false;
        assert!(super::check(&forged, false, false, true, false, true).is_err());
        for (text_cursor, calculator, declared_text) in [
            (false, false, false),
            (true, false, false),
            (false, false, true),
            (true, true, true),
        ] {
            assert!(
                super::check(
                    &original,
                    false,
                    false,
                    text_cursor,
                    calculator,
                    declared_text
                )
                .is_err(),
                "declared-text source was admitted through another compiler profile"
            );
        }
    }
}
