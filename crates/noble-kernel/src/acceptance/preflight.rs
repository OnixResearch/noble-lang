//! Request preflight: format, limits, environment schemes, and effect
//! identities. Every check completes before the machine starts.

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
    attempt!(validate_schemes(env));
    let context = super::parts::Ctx { request, env };
    attempt!(super::parts::limits_of(
        &request.expected.stack_in,
        &context
    ));
    attempt!(super::parts::limits_of(
        &request.expected.stack_out,
        &context
    ));
    check_allowed_effects(&context)
}

/// Reject any environment definition whose scheme is not in the accepted form.
fn validate_schemes(env: &crate::contracts::Env) -> Result<(), super::Fail> {
    let mut index = 0;
    let mut failure: Option<super::Fail> = None;
    while index < env.defs.len() {
        let scheme = &env.defs[index];
        if scheme.validate().is_err() {
            failure = Some(super::Fail::Unsupported(
                crate::untrusted::UnsupportedKind::SchemeForm,
            ));
            break;
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// Reject any allowed effect identity the environment does not know.
fn check_allowed_effects(context: &super::parts::Ctx) -> Result<(), super::Fail> {
    let allowed_effects: alloc::vec::Vec<crate::types::EffId> =
        context.request.expected.allowed_effects.as_slice().to_vec();
    let mut index = 0;
    let mut unknown: Option<crate::types::EffId> = None;
    while index < allowed_effects.len() {
        let id = allowed_effects[index];
        if !context.env.knows_effect(id) {
            unknown = Some(id);
            break;
        }
        index += 1;
    }
    match unknown {
        Some(id) => Err(super::parts::invalid(
            context,
            super::parts::site(None, None),
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::UnknownEffect(id),
        )),
        None => Ok(()),
    }
}
