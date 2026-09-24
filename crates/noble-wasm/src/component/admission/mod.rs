mod body;

#[derive(Clone, Copy)]
struct Context<'a> {
    world: &'a noble_contracts::component::World,
    world_context: &'a [u8],
    environment: &'a noble_kernel::contracts::Env,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; exact world binding, export uniqueness and aggregate arena limits are untrusted-input checks; the fresh visited table is bounded by the export count and every refusal returns a diagnostic rather than asserting on producer data."
)]
pub(super) fn check(
    world: &noble_contracts::component::World,
    exports: &[noble_contracts::component::CheckedExport],
) -> Result<usize, crate::Diagnostic> {
    if exports.len() != world.exports().len() {
        return Err(crate::Diagnostic::Invalid);
    }
    if exports.len() > noble_contracts::component::MAX_OPERATIONS {
        return Err(crate::Diagnostic::Invalid);
    }
    let world_context = world.build_context();
    let environment = match world.environment() {
        Ok(environment) => environment,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    let mut seen = alloc::vec![false; exports.len()];
    let mut at = 0usize;
    let mut total_nodes = 0usize;
    let mut text_count = 0usize;
    let mut failure = None;
    let context = Context {
        world,
        world_context: &world_context,
        environment: &environment,
    };
    let count = exports.len();
    while at < count && failure.is_none() {
        match export(context, &exports[at], &mut seen, total_nodes) {
            Ok((nodes, texts)) => {
                total_nodes = nodes;
                text_count = text_count.saturating_add(texts);
            }
            Err(error) => failure = Some(error),
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(text_count),
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; export admission invokes non-const independent acceptance and mutates only the fresh bounded compiler-owned visited table; the regenerated world, supplied export and environment remain immutable."
)]
fn export(
    context: Context<'_>,
    export: &noble_contracts::component::CheckedExport,
    seen: &mut [bool],
    total_nodes: usize,
) -> Result<(usize, usize), crate::Diagnostic> {
    if export.world_context() != context.world_context {
        return Err(crate::Diagnostic::Invalid);
    }
    attempt!(visit(seen, export.index()));
    let submission = match export.submission() {
        Some(submission) => submission,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let total_nodes = total_nodes.saturating_add(submission.body.candidate.nodes.len());
    if total_nodes > super::NODE_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(one(context.world, export, context.environment));
    Ok((total_nodes, submission.body.texts.len()))
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; non-const SliceIndex::get_mut validates the index before changing only compiler-owned scratch visitation bits, never the borrowed producer arena."
)]
fn visit(seen: &mut [bool], index: usize) -> Result<(), crate::Diagnostic> {
    let slot = match seen.get_mut(index) {
        Some(slot) => slot,
        None => return Err(crate::Diagnostic::Invalid),
    };
    if *slot {
        return Err(crate::Diagnostic::Invalid);
    }
    *slot = true;
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every export must undergo non-const independent kernel acceptance using the regenerated environment after exact request and shape checks; refusal and effect mismatches remain input diagnostics, never assertions or producer acceptance flags."
)]
fn one(
    world: &noble_contracts::component::World,
    export: &noble_contracts::component::CheckedExport,
    environment: &noble_kernel::contracts::Env,
) -> Result<(), crate::Diagnostic> {
    let operation = match world.exports().get(export.index()) {
        Some(operation) => operation,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let submission = match export.submission() {
        Some(submission) => submission,
        None => return Err(crate::Diagnostic::Invalid),
    };
    if !submission.definitions.is_empty() {
        return Err(crate::Diagnostic::Unsupported);
    }
    attempt!(same_environment(&submission.environment, environment));
    if submission.request.expected.stack_in != operation.input_types() {
        return Err(crate::Diagnostic::Invalid);
    }
    if submission.request.expected.stack_out != operation.output_types() {
        return Err(crate::Diagnostic::Invalid);
    }
    let candidate = &submission.body.candidate;
    if candidate.nodes.len() > super::NODE_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    if candidate.body.len() != candidate.nodes.len() {
        return Err(crate::Diagnostic::Exhausted);
    }
    if submission.body.texts.len() > candidate.nodes.len() {
        return Err(crate::Diagnostic::Exhausted);
    }
    let checked = match noble_kernel::acceptance::check(environment, &submission.request, candidate)
    {
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
    if checked.interface.effects != submission.request.expected.allowed_effects {
        return Err(crate::Diagnostic::Invalid);
    }
    attempt!(body::nodes(candidate));
    body::validate_texts(&submission.body)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the supplied environment is untrusted and every complete scheme, effect, dependency and behavior mismatch returns Invalid before kernel rechecking."
)]
fn same_environment(
    supplied: &noble_kernel::contracts::Env,
    expected: &noble_kernel::contracts::Env,
) -> Result<(), crate::Diagnostic> {
    if supplied.defs.len() != expected.defs.len() {
        return Err(crate::Diagnostic::Invalid);
    }
    if supplied.kinds != expected.kinds {
        return Err(crate::Diagnostic::Invalid);
    }
    if supplied.deps != expected.deps {
        return Err(crate::Diagnostic::Invalid);
    }
    if supplied.effects != expected.effects {
        return Err(crate::Diagnostic::Invalid);
    }
    if !supplied.schemas.is_empty() {
        return Err(crate::Diagnostic::Invalid);
    }
    let mut at = 0usize;
    let mut failure = None;
    let count = supplied.defs.len();
    while at < count && failure.is_none() {
        if let Err(error) = same_definition(&supplied.defs[at], expected, at) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; complete scheme comparison invokes non-const Vec and polymorphic type equality after checked runtime definition lookup."
)]
fn same_definition(
    left: &noble_kernel::words::Scheme,
    expected: &noble_kernel::contracts::Env,
    at: usize,
) -> Result<(), crate::Diagnostic> {
    let right = match expected.defs.get(at) {
        Some(right) => right,
        None => return Err(crate::Diagnostic::Invalid),
    };
    if left.var_kinds != right.var_kinds || left.stack_in != right.stack_in {
        return Err(crate::Diagnostic::Invalid);
    }
    if left.stack_out != right.stack_out || left.effects != right.effects {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(())
}
