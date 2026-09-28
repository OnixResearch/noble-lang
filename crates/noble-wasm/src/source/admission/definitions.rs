#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each definition failure returns Diagnostic and totals above declared limits return Exhausted, not assertion panics; reassess when the heuristic understands fallible validators."
)]
pub(super) fn bounds(
    submission: &noble_kernel::execution::Submission,
    environment_work: u64,
    totals: &mut super::metadata::Totals,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.definitions.len() {
        match definition_bounds(submission, index, environment_work, totals, work) {
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
    Ok(())
}

fn definition_bounds(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    environment_work: u64,
    totals: &mut super::metadata::Totals,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let definition = &submission.definitions[index];
    attempt!(check_identity(submission, index, work));
    attempt!(work.charge(environment_work));
    attempt!(super::environment::exact_contract(
        &submission.environment,
        definition.definition,
        &definition.expected
    ));
    super::metadata::check(&definition.body, totals, work)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; builtin classification and metered duplicate lookup are non-const operations that reject forged definition identities before metadata scanning."
)]
fn check_identity(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let def = submission.definitions[index].definition;
    let env = &submission.environment;
    if def.0 < super::super::builtin_count(env)
        || env.kind(def) != Some(noble_kernel::contracts::Behavior::Named)
    {
        return Err(crate::Diagnostic::Invalid);
    }
    attempt!(work.entries(submission.definitions.len()));
    if attempt!(super::definition_index(submission, def)) != index {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the first kernel denial returns its Diagnostic instead of asserting on an untrusted definition; reassess when the heuristic understands fallible validators."
)]
pub(super) fn accept(
    submission: &noble_kernel::execution::Submission,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::super::Work,
) -> Result<alloc::vec::Vec<noble_kernel::untrusted::Checked>, crate::Diagnostic> {
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
    Ok(definitions)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; independent kernel acceptance and owned expected-interface cloning are non-const operations and remain mandatory for every executable definition."
)]
fn accept_definition(
    submission: &noble_kernel::execution::Submission,
    index: usize,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::super::Work,
) -> Result<noble_kernel::untrusted::Checked, crate::Diagnostic> {
    let definition = &submission.definitions[index];
    attempt!(work.charge(u64::from(limits.work).saturating_mul(2)));
    let request = noble_kernel::untrusted::Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits,
    };
    let env = attempt!(caller_environment(submission, definition.definition));
    if is_ambient_forbidden(&env, &definition.body) {
        return Err(crate::Diagnostic::Invalid);
    }
    let checked = attempt!(super::accept(&env, &request, &definition.body));
    if checked.interface.effects != definition.expected.allowed_effects {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(checked)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; pinned const trials fail on Vec indexing (March E0277, August E0658) and Env::clone (March E0277, August E0015); reassess when these operations become const-capable."
)]
fn caller_environment(
    submission: &noble_kernel::execution::Submission,
    def: noble_kernel::contracts::Definition,
) -> Result<noble_kernel::contracts::Env, crate::Diagnostic> {
    let index = match usize::try_from(def.0) {
        Ok(index) => index,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    if index >= submission.environment.definition_owners.len() {
        return Err(crate::Diagnostic::Invalid);
    }
    let owner = submission.environment.definition_owners[index];
    let mut env = submission.environment.clone();
    env.caller_module = owner;
    Ok(env)
}

fn is_ambient_forbidden(
    env: &noble_kernel::contracts::Env,
    body: &noble_kernel::execution::Body,
) -> bool {
    if env.caller_module.is_none() {
        return false;
    }
    let mut index = 0usize;
    while index < body.candidate.nodes.len() {
        match &body.candidate.nodes[index] {
            noble_kernel::untrusted::Node::Invocation { def, .. } if def.0 == 22 => return true,
            noble_kernel::untrusted::Node::Invocation { .. }
            | noble_kernel::untrusted::Node::Literal { .. }
            | noble_kernel::untrusted::Node::Quotation { .. } => {}
        }
        index += 1;
    }
    false
}
