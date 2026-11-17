#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

mod definitions;
mod dependencies;
mod environment;
pub(super) mod identity;
mod metadata;
pub(super) mod meter;

/// Price the exact borrowed scheme selected by a calculator invocation
/// before kernel node_of copies its witness on each referenced occurrence.
pub(in crate::source) fn witness_scheme(
    submission: &noble_kernel::execution::Submission,
    def: noble_kernel::contracts::Definition,
    work: &mut super::Work,
) -> Result<u64, crate::Diagnostic> {
    environment::contract_work(&submission.environment, def, work)
}

pub(super) struct Accepted {
    pub(super) definitions: alloc::vec::Vec<noble_kernel::untrusted::Checked>,
    pub(super) root: noble_kernel::untrusted::Checked,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; kernel refusal and unreachable arena slots are untrusted-input diagnostics; the bounded visited table checks every accepted derivation without asserting on producer input."
)]
fn accept(
    env: &noble_kernel::contracts::Env,
    request: &noble_kernel::untrusted::Request,
    body: &noble_kernel::execution::Body,
    signed_owner: Option<(
        noble_kernel::contracts::Definition,
        &noble_kernel::words::Inst,
    )>,
) -> Result<noble_kernel::untrusted::Checked, crate::Diagnostic> {
    let outcome = match signed_owner {
        Some((owner, owner_inst)) => noble_kernel::acceptance::check_signed_structure(
            env,
            request,
            &body.candidate,
            owner,
            owner_inst,
        ),
        None => noble_kernel::acceptance::check(env, request, &body.candidate),
    };
    let checked = match outcome {
        noble_kernel::untrusted::Outcome::Accepted(checked) => checked,
        noble_kernel::untrusted::Outcome::Invalid(_) => return Err(crate::Diagnostic::Invalid),
        noble_kernel::untrusted::Outcome::Unsupported(_) => {
            return Err(crate::Diagnostic::Unsupported)
        }
        noble_kernel::untrusted::Outcome::Exhausted(_) => return Err(crate::Diagnostic::Exhausted),
        noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err(crate::Diagnostic::Defective)
        }
    };
    // No executable payload may hide in an unchecked, unreachable arena slot.
    let mut visited = alloc::vec![false; body.candidate.nodes.len()];
    let mut at = 0usize;
    let mut failure = None;
    while at < checked.derivations.len() {
        match mark_visited(&mut visited, checked.derivations[at].node) {
            Ok(()) => at += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if crate::scan::find_first(&visited, |slot| !*slot).is_some() {
        Err(crate::Diagnostic::Invalid)
    } else {
        attempt!(checked_interface(env, &request.expected, &checked));
        Ok(checked)
    }
}

/// Whether a definition body contains a signed self-reference node.
fn has_signed_self(body: &noble_kernel::execution::Body) -> bool {
    crate::scan::find_first(&body.candidate.nodes, |node| {
        matches!(node, noble_kernel::untrusted::Node::SignedSelf { .. })
    })
    .is_some()
}

/// Retain only kernel-derived interfaces. In slot mode the entry has a full
/// logical input (including borrowed references), whereas the individual
/// dispatch derivations operate on the physical value stack. A site must
/// resolve to that exact root reference, not to a candidate-supplied handle
/// or a similarly shaped but differently ordered nominal/resource signature.
fn checked_interface(
    env: &noble_kernel::contracts::Env,
    expected: &noble_kernel::untrusted::Expected,
    checked: &noble_kernel::untrusted::Checked,
) -> Result<(), crate::Diagnostic> {
    if checked.interface.stack_in != expected.stack_in
        || checked.interface.stack_out != expected.stack_out
        || !checked
            .interface
            .effects
            .is_subset_of(&expected.allowed_effects)
    {
        return Err(crate::Diagnostic::Invalid);
    }
    if !env.live_slots && !checked.live_sites.is_empty() {
        return Err(crate::Diagnostic::Invalid);
    }
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < checked.live_sites.len() {
        outcome = checked_live_site(expected, &checked.live_sites[index]);
        index += 1;
    }
    outcome
}

/// One dispatch site resolves to its exact root reference and forwards, in
/// order and exactly once each, the root inputs its borrowed formals name.
fn checked_live_site(
    expected: &noble_kernel::untrusted::Expected,
    site: &noble_kernel::untrusted::LiveSite,
) -> Result<(), crate::Diagnostic> {
    let position = usize::try_from(site.root_logical_input_position)
        .map_err(|_| crate::Diagnostic::Invalid)?;
    let Some(noble_kernel::types::Ty::LiveRef(input, output, ceiling)) =
        expected.stack_in.get(position)
    else {
        return Err(crate::Diagnostic::Invalid);
    };
    let preceding_refs = live_refs_before(&expected.stack_in, position);
    let is_ordinal_mismatch = match u32::try_from(preceding_refs) {
        Ok(refs) => refs != site.ref_ordinal,
        Err(_) => true,
    };
    let has_site_shape_mismatch = is_ordinal_mismatch
        || site.stack_in.as_slice() != input.as_slice()
        || site.stack_out.as_slice() != output.as_slice()
        || site.allowed_effects.as_slice() != ceiling.as_slice();
    if has_site_shape_mismatch
        || has_forwarded_mismatch(&expected.stack_in, input, &site.forwarded_ref_ordinals)
    {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(())
}

/// The number of borrowed live references among the first `position`
/// entries of `stack`.
fn live_refs_before(stack: &[noble_kernel::types::Ty], position: usize) -> usize {
    let mut refs = 0usize;
    let mut index = 0usize;
    while index < position {
        if matches!(stack[index], noble_kernel::types::Ty::LiveRef(_, _, _)) {
            refs += 1;
        }
        index += 1;
    }
    refs
}

/// Whether `forwarded` fails to name, in order, one root input equal to each
/// borrowed formal of `formals`, or names more inputs than there are formals.
fn has_forwarded_mismatch(
    stack_in: &[noble_kernel::types::Ty],
    formals: &[noble_kernel::types::Ty],
    forwarded: &[u32],
) -> bool {
    let mut next = 0usize;
    let mut index = 0usize;
    let mut has_mismatch = false;
    while !has_mismatch && index < formals.len() {
        let formal = &formals[index];
        if matches!(formal, noble_kernel::types::Ty::LiveRef(_, _, _)) {
            has_mismatch = is_forwarded_mismatch(stack_in, formal, forwarded, next);
            next += 1;
        }
        index += 1;
    }
    has_mismatch || next < forwarded.len()
}

/// Whether forwarded ordinal `next` is absent or names a root input that is
/// not exactly `formal`.
fn is_forwarded_mismatch(
    stack_in: &[noble_kernel::types::Ty],
    formal: &noble_kernel::types::Ty,
    forwarded: &[u32],
    next: usize,
) -> bool {
    let Some(ordinal) = forwarded.get(next) else {
        return true;
    };
    let Ok(position) = usize::try_from(*ordinal) else {
        return true;
    };
    match stack_in.get(position) {
        Some(root) => *root != *formal,
        None => true,
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; checked node conversion and mutable slice lookup are non-const operations; invalid derivations return Invalid before touching the private visited table."
)]
fn mark_visited(
    visited: &mut [bool],
    node: noble_kernel::untrusted::NodeId,
) -> Result<(), crate::Diagnostic> {
    let index = attempt!(crate::admission::index(node));
    match visited.get_mut(index) {
        Some(slot) => {
            *slot = true;
            Ok(())
        }
        None => Err(crate::Diagnostic::Invalid),
    }
}

pub(super) fn definition_index(
    submission: &noble_kernel::execution::Submission,
    definition: noble_kernel::contracts::Definition,
) -> Result<usize, crate::Diagnostic> {
    let mut index = 0usize;
    let mut found = None;
    while index < submission.definitions.len() {
        if submission.definitions[index].definition.0 == definition.0 {
            found = Some(index);
            break;
        }
        index += 1;
    }
    match found {
        Some(index) => Ok(index),
        None => Err(crate::Diagnostic::Invalid),
    }
}

fn bounds(
    submission: &noble_kernel::execution::Submission,
    calculator: bool,
    signed: bool,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut totals = metadata::Totals { nodes: 0, bytes: 0 };
    attempt!(metadata::check(&submission.body, &mut totals, work));
    attempt!(definitions::bounds(
        submission,
        calculator,
        signed,
        &mut totals,
        work
    ));
    dependencies::check(submission, signed, work)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; reserving the shared budget uses non-const TryFrom conversions to retain exhaustion diagnostics for unrepresentable body counts and work allowances."
)]
fn kernel_limits(
    submission: &noble_kernel::execution::Submission,
    environment_work: u64,
    work: &mut super::Work,
) -> Result<noble_kernel::untrusted::Limits, crate::Diagnostic> {
    // Reserve both kernel meters from this submission's one allowance; a
    // concrete specialization never receives a reset copy of the full budget.
    let bodies = match u64::try_from(submission.definitions.len().saturating_add(1)) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    attempt!(work.charge(environment_work.saturating_mul(bodies)));
    let per_body = match work.remaining.checked_div(bodies) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    let mut limits = submission.request.limits;
    limits.work = match u32::try_from(per_body / 4) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    Ok(limits)
}

fn calculator_environment_allowance(
    submission: &noble_kernel::execution::Submission,
    environment_work: u64,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    let bodies = attempt!(
        u64::try_from(submission.definitions.len().saturating_add(1))
            .map_err(|_| crate::Diagnostic::Exhausted)
    );
    // The already-checked whole environment is scanned once above, each
    // distinct named definition's own scheme is substituted once below
    // (their combined bounds cannot exceed one environment scan), and every
    // independent kernel body validation physically scans the environment.
    // Debit all of these scans as usual, but give this selected compiler a
    // separate finite structural-work allowance on top of the caller's body
    // work. Nothing is elided, and the absolute ceiling fails closed.
    let scans = attempt!(bodies
        .checked_add(2)
        .and_then(|count| environment_work.checked_mul(count))
        .ok_or(crate::Diagnostic::Exhausted));
    work.grant_calculator(scans)
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the call passes the complete checked input, plan and shared-budget coordinates, and an options struct would only rename the same borrowed fields."
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-wasm-maintainers; each selected compiler profile flag is a separate trusted admission input, and an options struct would only rename the same borrowed fields."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    live: bool,
    live_slots: bool,
    text_cursor: bool,
    calculator: bool,
    checked_i64: bool,
    declared_text: bool,
    signed: bool,
    work: &mut super::Work,
) -> Result<Accepted, crate::Diagnostic> {
    attempt!(work.charge(1));
    // Only the combined declared-text profile pairs declared modules with the
    // text cursor; live and the calculator never do.
    let is_declared_or_calculator_profile = submission.environment.declared_modules || calculator;
    let is_cursor_conflicting_with_calculator = text_cursor && calculator;
    let is_cursor_conflicting_with_non_declared_text = text_cursor && !declared_text;
    let is_cursor_profile_conflict = live
        || is_cursor_conflicting_with_calculator
        || is_cursor_conflicting_with_non_declared_text;
    let is_existing_profile_conflict =
        is_declared_or_calculator_profile && is_cursor_profile_conflict;
    let is_runtime_profile_conflict = live || live_slots || text_cursor;
    let is_checked_i64_runtime_conflict = checked_i64 && is_runtime_profile_conflict;
    let is_source_profile_conflict =
        calculator || declared_text || submission.environment.declared_modules;
    let is_checked_i64_source_conflict = checked_i64 && is_source_profile_conflict;
    let is_checked_i64_profile_conflict =
        is_checked_i64_runtime_conflict || is_checked_i64_source_conflict;
    if is_existing_profile_conflict || is_checked_i64_profile_conflict {
        return Err(crate::Diagnostic::Invalid);
    }
    let environment_work = attempt!(environment::work(
        &submission.environment,
        super::ProfileSelection {
            calculator,
            checked_i64,
            text_cursor,
            declared_text,
        },
        work
    ));
    attempt!(work.charge(environment_work));
    attempt!(environment::check(
        submission,
        live,
        live_slots,
        text_cursor,
        calculator,
        checked_i64,
        declared_text
    ));
    if calculator {
        attempt!(calculator_environment_allowance(
            submission,
            environment_work,
            work
        ));
    }
    attempt!(bounds(submission, calculator, signed, work));
    let limits = attempt!(kernel_limits(submission, environment_work, work));
    let definitions = attempt!(definitions::accept(submission, limits, signed, work));
    let root = attempt!(accept_root(submission, limits, work));
    Ok(Accepted { definitions, root })
}

fn accept_root(
    submission: &noble_kernel::execution::Submission,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<noble_kernel::untrusted::Checked, crate::Diagnostic> {
    attempt!(work.charge(u64::from(limits.work).saturating_mul(2)));
    let root_request = noble_kernel::untrusted::Request {
        input_bytes: submission.request.input_bytes,
        expected: submission.request.expected.clone(),
        limits,
    };
    accept(
        &submission.environment,
        &root_request,
        &submission.body,
        None,
    )
}

#[path = "admission/nodes.rs"]
pub(in crate::source) mod nodes;
