/// The compiler's own finite shape bounds precede the independently metered
/// kernel check. Semantic scope restrictions follow acceptance, not a guessed
/// interpretation of the untrusted witnesses.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; malformed input, kernel refusal and unsupported interfaces return distinct diagnostics before emission; assertions must not turn these expected boundary outcomes into panics."
)]
pub(crate) fn check(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<noble_kernel::untrusted::Checked, crate::Diagnostic> {
    attempt!(bounds(candidate));
    let environment = match noble_kernel::contracts::environment() {
        Ok(environment) => environment,
        Err(_) => return Err(crate::Diagnostic::Defective),
    };
    let checked = match noble_kernel::acceptance::check(&environment, request, candidate) {
        noble_kernel::untrusted::Outcome::Accepted(checked) => checked,
        noble_kernel::untrusted::Outcome::Invalid(_) => return Err(crate::Diagnostic::Invalid),
        noble_kernel::untrusted::Outcome::Exhausted(_) => return Err(crate::Diagnostic::Exhausted),
        noble_kernel::untrusted::Outcome::Unsupported(_) => {
            return Err(crate::Diagnostic::Unsupported)
        }
        noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err(crate::Diagnostic::Defective)
        }
    };
    if !request.expected.allowed_effects.is_empty()
        || !checked.interface.effects.is_empty()
        || !checked.interface.stack_in.is_empty()
    {
        return Err(crate::Diagnostic::Unsupported);
    }
    if checked.interface.stack_out.len() > crate::EXPORT_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(root_types(&checked.interface.stack_out));
    attempt!(root_quotations(candidate));
    if candidate.body.len() != checked.interface.stack_out.len() {
        return Err(crate::Diagnostic::Defective);
    }
    Ok(checked)
}

fn root_types(stack: &[noble_kernel::types::Ty]) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    while index < stack.len() {
        match &stack[index] {
            noble_kernel::types::Ty::Program(_, _, _) => {}
            _ => return Err(crate::Diagnostic::Unsupported),
        }
        index += 1;
    }
    Ok(())
}

fn root_quotations(
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    while index < candidate.body.len() {
        match node(candidate, candidate.body[index]) {
            Ok(noble_kernel::untrusted::Node::Quotation { .. }) => {}
            Ok(_) => return Err(crate::Diagnostic::Unsupported),
            Err(failure) => return Err(failure),
        }
        index += 1;
    }
    Ok(())
}

fn bounds(candidate: &noble_kernel::untrusted::Candidate) -> Result<(), crate::Diagnostic> {
    if candidate.nodes.len() > crate::NODE_LIMIT
        || candidate.body.len() > crate::BODY_OPERATION_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut bodies = 1usize;
    let mut index = 0usize;
    while index < candidate.nodes.len() {
        if let noble_kernel::untrusted::Node::Quotation { body, .. } = &candidate.nodes[index] {
            if body.len() > crate::BODY_OPERATION_LIMIT || bodies >= crate::BODY_LIMIT {
                return Err(crate::Diagnostic::Exhausted);
            }
            bodies += 1;
        }
        index += 1;
    }
    Ok(())
}

pub(crate) fn index(node: noble_kernel::untrusted::NodeId) -> Result<usize, crate::Diagnostic> {
    match usize::try_from(node.0) {
        Ok(index) => Ok(index),
        Err(_) => Err(crate::Diagnostic::Defective),
    }
}

pub(crate) fn node(
    candidate: &noble_kernel::untrusted::Candidate,
    id: noble_kernel::untrusted::NodeId,
) -> Result<&noble_kernel::untrusted::Node, crate::Diagnostic> {
    match candidate.nodes.get(attempt!(index(id))) {
        Some(node) => Ok(node),
        None => Err(crate::Diagnostic::Defective),
    }
}
