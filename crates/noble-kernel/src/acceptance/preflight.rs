//! Request preflight: format, limits, environment schemes, and effect
//! identities, plus the external-environment-data validation walks
//! (B-CHECK-02). Every check completes before the machine starts.

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; check_request preserves format, byte/node, scheme, dependency, schema, stack and effect rejection order; invalid requests are typed outcomes, not assertion failures."
)]
pub(super) fn check_request(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
    candidate: &crate::untrusted::Candidate,
) -> Result<(), super::Fail> {
    if candidate.format != crate::untrusted::CANDIDATE_FORMAT
        || candidate.revision != crate::untrusted::SEMANTIC_REVISION
    {
        return Err(super::Fail::Unsupported(
            crate::untrusted::UnsupportedKind::FormatRevision,
        ));
    }
    if request.input_bytes > request.limits.bytes {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Bytes));
    }
    let node_count = match u64::try_from(candidate.nodes.len()) {
        Ok(count) => count,
        Err(_) => return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Nodes)),
    };
    if node_count > u64::from(request.limits.nodes) {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Nodes));
    }
    attempt!(super::validate::dependencies(env, &request.limits));
    attempt!(super::validate::schemas(env, &request.limits));
    attempt!(validate_nominals(env, request));
    attempt!(reject_ambient_emit(env, candidate));
    attempt!(super::schemes::validate(env, request));
    let context = super::parts::Ctx { request, env };
    attempt!(super::parts::limits_of(
        &request.expected.stack_in,
        &context
    ));
    attempt!(super::parts::limits_of(
        &request.expected.stack_out,
        &context
    ));
    let unknown = super::parts::effects::first_unknown(
        request.expected.allowed_effects.as_slice(),
        &env.effects,
    );
    match unknown {
        Some(id) => Err(super::parts::invalid_without_stacks(
            super::parts::site(None, None),
            crate::untrusted::Constraint::UnknownEffect(id),
        )),
        None => Ok(()),
    }
}

/// No candidate arena node, including an unreachable one, may smuggle the
/// historical fixed host operation into an opt-in module transaction.
fn reject_ambient_emit(
    env: &crate::contracts::Env,
    candidate: &crate::untrusted::Candidate,
) -> Result<(), super::Fail> {
    if !env.declared_modules && env.nominals.is_empty() && env.bound_adapters.is_empty() {
        return Ok(());
    }
    let Some((index, def)) = ambient_emit(env, candidate) else {
        return Ok(());
    };
    let Ok(index) = u32::try_from(index) else {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Nodes));
    };
    let node_id = crate::untrusted::NodeId(index);
    Err(super::parts::invalid_without_stacks(
        super::parts::site(Some(node_id), Some(def)),
        crate::untrusted::Constraint::PrivateDefinition(def),
    ))
}

/// Find the first forbidden invocation, including unreachable arena nodes.
fn ambient_emit(
    env: &crate::contracts::Env,
    candidate: &crate::untrusted::Candidate,
) -> Option<(usize, crate::contracts::Definition)> {
    let mut index = 0;
    let mut first = None;
    while index < candidate.nodes.len() {
        if let crate::untrusted::Node::Invocation { def, .. } = &candidate.nodes[index] {
            if env.kind(*def) == Some(crate::contracts::Behavior::TestEmit) {
                first = Some((index, *def));
                break;
            }
        }
        index += 1;
    }
    first
}

fn validate_nominals(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
) -> Result<(), super::Fail> {
    attempt!(nominal_budget(env, request));
    if let Some(failure) = first_nominal_failure(env, &request.limits) {
        return Err(nominal_failure(failure));
    }
    if !env.validate_contracts() {
        return Err(super::parts::invalid_without_stacks(
            super::parts::site(None, None),
            crate::untrusted::Constraint::InvalidContract,
        ));
    }
    Ok(())
}

/// Preserve the first invalid declaration before checking contract coherence.
struct NominalFailure {
    is_invalid_type: bool,
}

const fn nominal_failure(failure: NominalFailure) -> super::Fail {
    if failure.is_invalid_type {
        return super::parts::invalid_without_stacks(
            super::parts::site(None, None),
            crate::untrusted::Constraint::InvalidType,
        );
    }
    super::Fail::Exhausted(crate::untrusted::LimitKind::TypeSize)
}

fn first_nominal_failure(
    env: &crate::contracts::Env,
    limits: &crate::untrusted::Limits,
) -> Option<NominalFailure> {
    let mut index = 0;
    while index < env.nominals.len() {
        if let Some(problem) = nominal_failure_at(env, limits, index) {
            return Some(problem);
        }
        index += 1;
    }
    None
}

/// The node bound applies to new definitions only; both paths charge work.
struct NominalBudget {
    count: u64,
    node_bound: Option<u64>,
    work_bound: u64,
}

const fn exhausted_nominal_budget(budget: &NominalBudget) -> Option<crate::untrusted::LimitKind> {
    if let Some(nodes) = budget.node_bound {
        if budget.count > nodes {
            return Some(crate::untrusted::LimitKind::Nodes);
        }
    }
    if budget.count > budget.work_bound {
        return Some(crate::untrusted::LimitKind::Work);
    }
    None
}

fn nominal_budget(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
) -> Result<(), super::Fail> {
    if env.declared_modules {
        attempt!(declared_definition_budget(env, request));
    }
    let total = env.nominals.len().saturating_add(env.bound_adapters.len());
    let Ok(count) = u64::try_from(total) else {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work));
    };
    if let Some(kind) = exhausted_nominal_budget(&NominalBudget {
        count,
        node_bound: None,
        work_bound: u64::from(request.limits.work),
    }) {
        return Err(super::Fail::Exhausted(kind));
    }
    Ok(())
}

fn declared_definition_budget(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
) -> Result<(), super::Fail> {
    let Ok(additions) = u64::try_from(env.defs.len().saturating_sub(23)) else {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Nodes));
    };
    if let Some(kind) = exhausted_nominal_budget(&NominalBudget {
        count: additions,
        node_bound: Some(u64::from(request.limits.nodes)),
        work_bound: u64::from(request.limits.work),
    }) {
        return Err(super::Fail::Exhausted(kind));
    }
    Ok(())
}

/// Validate one declaration's representation before checking its size.
fn nominal_failure_at(
    env: &crate::contracts::Env,
    limits: &crate::untrusted::Limits,
    index: usize,
) -> Option<NominalFailure> {
    if !env.validate_decl(index, 512) {
        return Some(NominalFailure {
            is_invalid_type: true,
        });
    }
    if !nominal_size_within_limit(nominal_payload_size(&env.nominals[index].shape), limits) {
        return Some(NominalFailure {
            is_invalid_type: false,
        });
    }
    None
}

fn nominal_payload_size(shape: &crate::types::NominalShape) -> Option<u32> {
    if let crate::types::NominalShape::Opaque(representation) = shape {
        return representation.size();
    }
    if let crate::types::NominalShape::Variant(left, right) = shape {
        return left
            .size()
            .zip(right.size())
            .and_then(|(left, right)| left.checked_add(right));
    }
    None
}

/// A nominal descriptor adds one node for its wrapper, even when the payload
/// cannot be represented or exceeds the request's structural type bound.
const fn nominal_size_within_limit(
    payload_node_count: Option<u32>,
    limits: &crate::untrusted::Limits,
) -> bool {
    let Some(payload_node_count) = payload_node_count else {
        return false;
    };
    let Some(total_node_count) = payload_node_count.checked_add(1) else {
        return false;
    };
    total_node_count <= limits.type_size
}
