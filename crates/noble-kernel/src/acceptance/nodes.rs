//! Node lookup and literal, invocation, and quotation folding.

/// Fetch one node, rejecting a reference outside the finite arena.
///
/// The node returns owned: a reference into the candidate arena cannot be
/// carried across the machine state the extraction interpreter tracks.
pub(super) fn node_of(
    candidate: &crate::untrusted::Candidate,
    node_id: crate::untrusted::NodeId,
    context: &super::parts::Ctx,
) -> Result<crate::untrusted::Node, super::Fail> {
    let index = match usize::try_from(node_id.0) {
        Ok(index) => index,
        Err(_) => {
            return Err(super::parts::invalid(
                context,
                super::parts::site(Some(node_id), None),
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::MalformedReference(node_id),
            ))
        }
    };
    match candidate.nodes.get(index) {
        Some(node) => Ok(node.clone()),
        None => Err(super::parts::invalid(
            context,
            super::parts::site(Some(node_id), None),
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::MalformedReference(node_id),
        )),
    }
}

/// Fold one literal or invocation node into its frame.
///
/// Returns the work charged for the node's instantiation and join.
pub(super) fn fold_node(
    frame: super::Frame,
    node_id: crate::untrusted::NodeId,
    node: &crate::untrusted::Node,
    context: &super::parts::Ctx,
) -> Result<(u32, super::Frame, crate::untrusted::Interface), super::Fail> {
    match node {
        crate::untrusted::Node::Literal { lit, inst } => {
            let scheme = super::parts::instantiate::literal_scheme(*lit);
            let at = super::parts::site(Some(node_id), None);
            let cost = attempt!(super::parts::scheme_cost(&scheme));
            let (interface, _resolved) = attempt!(super::parts::instantiate::apply(
                &scheme, inst, None, at, context
            ));
            let cost = cost.saturating_add(attempt!(super::parts::join_cost(&interface)));
            let joined = attempt!(super::parts::join(frame, &interface, at, context));
            Ok((cost, joined, interface))
        }
        crate::untrusted::Node::Invocation { def, inst } => {
            let scheme = match context.env.scheme(*def) {
                Some(scheme) => scheme.clone(),
                None => {
                    return Err(super::parts::invalid(
                        context,
                        super::parts::site(Some(node_id), Some(*def)),
                        alloc::vec::Vec::new(),
                        alloc::vec::Vec::new(),
                        crate::untrusted::Constraint::UnknownDefinition(*def),
                    ))
                }
            };
            let at = super::parts::site(Some(node_id), Some(*def));
            let data_var = super::parts::instantiate::data_slot(context.env.kind(*def));
            let cost = attempt!(super::parts::scheme_cost(&scheme));
            let (interface, _resolved) = attempt!(super::parts::instantiate::apply(
                &scheme, inst, data_var, at, context
            ));
            let cost = cost.saturating_add(attempt!(super::parts::join_cost(&interface)));
            let joined = attempt!(super::parts::join(frame, &interface, at, context));
            Ok((cost, joined, interface))
        }
        crate::untrusted::Node::Quotation { .. } => Err(super::Fail::Internal),
    }
}

/// Validate one quotation node and build its parent and child frames.
///
/// Returns the work charged for the node's instantiation.
pub(super) fn open_quotation(
    frame: super::Frame,
    node_id: crate::untrusted::NodeId,
    node: &crate::untrusted::Node,
    context: &super::parts::Ctx,
) -> Result<(u32, super::Frame, super::Frame), super::Fail> {
    let (body, inst) = match node {
        crate::untrusted::Node::Quotation { body, inst } => (body, inst),
        crate::untrusted::Node::Literal { .. } | crate::untrusted::Node::Invocation { .. } => {
            return Err(super::Fail::Internal)
        }
    };
    let scheme = super::parts::instantiate::quotation_scheme();
    let cost = attempt!(super::parts::scheme_cost(&scheme));
    attempt!(super::parts::instantiate::apply(
        &scheme,
        inst,
        None,
        super::parts::site(Some(node_id), None),
        context,
    ));
    if frame.depth.saturating_add(1) > context.request.limits.depth {
        return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Depth));
    }
    let start = match inst.stack(crate::words::Variable(1)) {
        Some(segment) => segment.to_vec(),
        None => return Err(super::Fail::Internal),
    };
    let claimed_out = match inst.stack(crate::words::Variable(2)) {
        Some(segment) => segment.to_vec(),
        None => return Err(super::Fail::Internal),
    };
    let claimed_effects = match inst.effects(crate::words::Variable(3)) {
        Some(set) => set.clone(),
        None => return Err(super::Fail::Internal),
    };
    let depth = frame.depth.saturating_add(1);
    let child = super::Frame {
        depth,
        origin: Some(node_id),
        body: body.clone(),
        index: 0,
        stack: start,
        effects: crate::types::EffSet::empty(),
        claimed_out,
        claimed_effects,
    };
    Ok((cost, frame, child))
}
