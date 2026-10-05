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
) -> Result<u32, super::Fail> {
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
    // The validation stages and body consume one request-local work allowance.
    // Do not restart it when crossing a preflight or machine boundary.
    let remaining = attempt!(super::validate::dependencies(env, request.limits.work));
    let remaining = attempt!(super::validate::schemas(env, remaining));
    let remaining = attempt!(validate_nominals(env, request, remaining));
    attempt!(reject_ambient_emit(env, candidate));
    if !env.live_slots {
        let mut index = 0;
        while index < candidate.nodes.len() {
            if matches!(candidate.nodes[index], crate::untrusted::Node::SlotInvoke { .. }) {
                return Err(super::Fail::Unsupported(
                    crate::untrusted::UnsupportedKind::NodeForm,
                ));
            }
            index += 1;
        }
    }
    attempt!(super::schemes::validate(env, request));
    let context = super::parts::Ctx { request, env };
    attempt!(super::parts::limits_of_entry(
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
        None => Ok(remaining),
    }
}

/// No candidate arena node, including an unreachable one, may smuggle the
/// historical fixed host operation outside the explicitly selected test-host
/// profile. Runtime authorization remains the independent host's obligation.
fn reject_ambient_emit(
    env: &crate::contracts::Env,
    candidate: &crate::untrusted::Candidate,
) -> Result<(), super::Fail> {
    if env.ambient_test_emit_visible() {
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
    remaining: u32,
) -> Result<u32, super::Fail> {
    let remaining = attempt!(nominal_budget(env, request, remaining));
    if let Some(failure) = first_nominal_failure(env, &request.limits) {
        return Err(nominal_failure(failure));
    }
    if !env.valid_generic_declarations() {
        return Err(nominal_failure(NominalFailure {
            is_invalid_type: true,
        }));
    }
    if !env.validate_contracts() {
        return Err(super::parts::invalid_without_stacks(
            super::parts::site(None, None),
            crate::untrusted::Constraint::InvalidContract,
        ));
    }
    Ok(remaining)
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

/// New definitions keep their separate node bound; their work charge is
/// carried into the nominal scan and the candidate body.
fn nominal_budget(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
    remaining: u32,
) -> Result<u32, super::Fail> {
    let remaining = if env.declared_modules {
        attempt!(declared_definition_budget(env, request, remaining))
    } else {
        remaining
    };
    let total = env
        .nominals
        .len()
        .saturating_add(env.generic_variants.len())
        .saturating_add(env.bound_adapters.len());
    let Ok(count) = u64::try_from(total) else {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work));
    };
    let Ok(cost) = u32::try_from(count) else {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work));
    };
    super::parts::charge(remaining, cost)
}

fn declared_definition_budget(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
    remaining: u32,
) -> Result<u32, super::Fail> {
    let Ok(additions) = u64::try_from(env.defs.len().saturating_sub(23)) else {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Nodes));
    };
    if additions > u64::from(request.limits.nodes) {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Nodes));
    }
    let Ok(cost) = u32::try_from(additions) else {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work));
    };
    super::parts::charge(remaining, cost)
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
