#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

mod dependencies;
mod environment;
pub(super) mod identity;
mod metadata;
pub(super) mod meter;

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
) -> Result<noble_kernel::untrusted::Checked, crate::Diagnostic> {
    let checked = match noble_kernel::acceptance::check(env, request, &body.candidate) {
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
    at = 0;
    while at < visited.len() {
        if !visited[at] {
            failure = Some(crate::Diagnostic::Invalid);
            break;
        }
        at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(checked),
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
        if submission.definitions[index].definition == definition {
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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; metadata, duplicate definitions, exact contracts and dependency edges are checked with the shared meter; malformed input and exhausted aggregate limits must return diagnostics rather than panic."
)]
fn bounds(
    submission: &noble_kernel::execution::Submission,
    environment_work: u64,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut totals = metadata::Totals { nodes: 0, bytes: 0 };
    attempt!(metadata::check(&submission.body, &mut totals, work));
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.definitions.len() {
        match definition_bounds(submission, index, environment_work, &mut totals, work) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if totals.nodes > usize::try_from(submission.request.limits.nodes).unwrap_or(0)
        || totals.bytes > usize::try_from(submission.request.limits.bytes).unwrap_or(0)
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    dependencies::check(submission, work)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this helper invokes non-const metered definition lookup, complete contract comparison and text validation before accepting producer metadata."
)]
fn definition_bounds(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    environment_work: u64,
    totals: &mut metadata::Totals,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    let definition = &submission.definitions[index];
    if definition.definition.0 < 24 {
        return Err(crate::Diagnostic::Invalid);
    }
    attempt!(work.entries(submission.definitions.len()));
    if attempt!(definition_index(submission, definition.definition)) != index {
        return Err(crate::Diagnostic::Invalid);
    }
    attempt!(work.charge(environment_work));
    attempt!(environment::exact_contract(
        &submission.environment,
        definition.definition,
        &definition.expected
    ));
    metadata::check(&definition.body, totals, work)
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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every definition and root is independently kernel-checked after shared-budget reservation; refusal and effect mismatches remain diagnostics, not assertions on untrusted submissions."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    work: &mut super::Work,
) -> Result<Accepted, crate::Diagnostic> {
    attempt!(work.charge(1));
    let environment_work = attempt!(environment::work(&submission.environment));
    attempt!(work.charge(environment_work));
    attempt!(environment::check(submission));
    attempt!(bounds(submission, environment_work, work));
    let limits = attempt!(kernel_limits(submission, environment_work, work));
    let mut definitions = alloc::vec::Vec::with_capacity(submission.definitions.len());
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.definitions.len() {
        match accept_definition(submission, index, limits, work) {
            Ok(checked) => {
                definitions.push(checked);
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
    attempt!(work.charge(u64::from(limits.work).saturating_mul(2)));
    let root_request = noble_kernel::untrusted::Request {
        input_bytes: submission.request.input_bytes,
        expected: submission.request.expected.clone(),
        limits,
    };
    let root = attempt!(accept(
        &submission.environment,
        &root_request,
        &submission.body
    ));
    Ok(Accepted { definitions, root })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; independent kernel acceptance and owned expected-interface cloning are non-const operations and remain mandatory for every executable definition."
)]
fn accept_definition(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<noble_kernel::untrusted::Checked, crate::Diagnostic> {
    let definition = &submission.definitions[index];
    attempt!(work.charge(u64::from(limits.work).saturating_mul(2)));
    let request = noble_kernel::untrusted::Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits,
    };
    let checked = attempt!(accept(&submission.environment, &request, &definition.body));
    if checked.interface.effects != definition.expected.allowed_effects {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(checked)
}

pub(super) fn text(
    body: &noble_kernel::execution::Body,
    node: noble_kernel::untrusted::NodeId,
) -> Result<&[u8], crate::Diagnostic> {
    let mut index = 0usize;
    let mut found = None;
    while index < body.texts.len() {
        if body.texts[index].node == node {
            found = Some(body.texts[index].bytes.as_slice());
            break;
        }
        index += 1;
    }
    match found {
        Some(bytes) => Ok(bytes),
        None => Err(crate::Diagnostic::Invalid),
    }
}

pub(super) fn node(
    candidate: &noble_kernel::untrusted::Candidate,
    id: noble_kernel::untrusted::NodeId,
) -> Result<&noble_kernel::untrusted::Node, crate::Diagnostic> {
    match candidate.nodes.get(attempt!(crate::admission::index(id))) {
        Some(node) => Ok(node),
        None => Err(crate::Diagnostic::Invalid),
    }
}
