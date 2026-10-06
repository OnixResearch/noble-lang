//! Node lookup and literal, invocation, and quotation folding.

mod visibility;

/// Fetch one node, rejecting a reference outside the finite arena.
///
/// The node returns owned: a reference into the candidate arena cannot be
/// carried across the machine state the extraction interpreter tracks.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; node_of checks index representability and arena membership and reports MalformedReference for either failure; hostile references must not trigger assertions."
)]
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
) -> Result<(u32, super::Frame, crate::untrusted::Interface, Option<crate::untrusted::LiveSite>), super::Fail> {
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
            Ok((cost, joined, interface, None))
        }
        crate::untrusted::Node::Invocation { def, inst } => {
            attempt!(visibility::check(*def, node_id, context));
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
            Ok((cost, joined, interface, None))
        }
        crate::untrusted::Node::SlotInvoke {
            ref_ordinal,
            site_id,
            forwarded_ref_ordinals,
        } => fold_slot(
            frame,
            node_id,
            *ref_ordinal,
            *site_id,
            forwarded_ref_ordinals,
            context,
        ),
        crate::untrusted::Node::Quotation { .. } => Err(super::Fail::Internal),
    }
}

/// Derive a live invocation solely from the host-owned root input and the
/// checked forwarding positions. A raw ordinal is never type authority.
fn fold_slot(
    frame: super::Frame,
    node_id: crate::untrusted::NodeId,
    ref_ordinal: u32,
    site_id: u32,
    forwarded: &[u32],
    context: &super::parts::Ctx,
) -> Result<(u32, super::Frame, crate::untrusted::Interface, Option<crate::untrusted::LiveSite>), super::Fail> {
    let at = super::parts::site(Some(node_id), None);
    if !context.env.live_slots || frame.depth != 0 {
        return Err(super::parts::invalid_without_stacks(
            at,
            crate::untrusted::Constraint::BorrowProvenance,
        ));
    }
    if site_id != node_id.0 {
        return Err(super::parts::invalid_without_stacks(
            at,
            crate::untrusted::Constraint::SlotSite,
        ));
    }
    let logical = &context.request.expected.stack_in;
    let root_index = match usize::try_from(ref_ordinal) {
        Ok(index) => index,
        Err(_) => {
            return Err(super::parts::invalid_without_stacks(
                at,
                crate::untrusted::Constraint::BorrowProvenance,
            ));
        }
    };
    let Some(root) = frame.borrowed.get(root_index) else {
        return Err(super::parts::invalid_without_stacks(
            at,
            crate::untrusted::Constraint::BorrowProvenance,
        ));
    };
    let root_position = root.logical_position;
    let crate::types::Ty::LiveRef(stack_in, stack_out, ceiling) = &root.ty else {
        return Err(super::Fail::Internal);
    };
    let mut value_in = alloc::vec::Vec::with_capacity(stack_in.len());
    let mut next_forward = 0usize;
    let mut formal_index = 0;
    while formal_index < stack_in.len() {
        let formal = &stack_in[formal_index];
        if matches!(formal, crate::types::Ty::LiveRef(_, _, _)) {
            let Some(position) = forwarded.get(next_forward) else {
                return Err(super::parts::invalid_without_stacks(
                    at,
                    crate::untrusted::Constraint::BorrowProvenance,
                ));
            };
            let logical_position = match usize::try_from(*position) {
                Ok(position) => position,
                Err(_) => {
                    return Err(super::parts::invalid_without_stacks(
                        at,
                        crate::untrusted::Constraint::BorrowProvenance,
                    ));
                }
            };
            let Some(actual) = logical.get(logical_position) else {
                return Err(super::parts::invalid_without_stacks(
                    at,
                    crate::untrusted::Constraint::BorrowProvenance,
                ));
            };
            if !matches!(actual, crate::types::Ty::LiveRef(_, _, _)) {
                return Err(super::parts::invalid_without_stacks(
                    at,
                    crate::untrusted::Constraint::BorrowProvenance,
                ));
            }
            if actual != formal {
                return Err(super::parts::invalid(
                    context,
                    at,
                    alloc::vec![formal.clone()],
                    alloc::vec![actual.clone()],
                    crate::untrusted::Constraint::StackJoin,
                ));
            }
            next_forward += 1;
        } else {
            value_in.push(formal.clone());
        }
        formal_index += 1;
    }
    if next_forward != forwarded.len() {
        return Err(super::parts::invalid_without_stacks(
            at,
            crate::untrusted::Constraint::BorrowProvenance,
        ));
    }
    let interface = crate::untrusted::Interface {
        stack_in: value_in,
        stack_out: stack_out.as_ref().clone(),
        effects: ceiling.with_id(crate::contracts::LIVE_DISPATCH),
    };
    let extra = match u32::try_from(stack_in.len().saturating_add(forwarded.len())) {
        Ok(extra) => extra,
        Err(_) => return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work)),
    };
    let cost = match attempt!(super::parts::join_cost(&interface)).checked_add(extra) {
        Some(cost) => cost,
        None => return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work)),
    };
    let root_logical_input_position = match u32::try_from(root_position) {
        Ok(position) => position,
        Err(_) => return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::StackHeight)),
    };
    let joined = attempt!(super::parts::join(frame, &interface, at, context));
    let Some(super::BorrowedInput {
        ty: crate::types::Ty::LiveRef(site_in, site_out, site_ceiling),
        ..
    }) = joined.borrowed.get(root_index)
    else {
        return Err(super::Fail::Internal);
    };
    let site = crate::untrusted::LiveSite {
        site_id,
        ref_ordinal,
        root_logical_input_position,
        forwarded_ref_ordinals: forwarded.to_vec(),
        stack_in: site_in.as_ref().clone(),
        stack_out: site_out.as_ref().clone(),
        allowed_effects: site_ceiling.clone(),
    };
    Ok((
        cost,
        joined,
        interface,
        Some(site),
    ))
}

/// Validate one quotation node and build its parent and child frames.
///
/// Returns the work charged for the node's instantiation.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; open_quotation validates the witness and depth before reading its three required bindings, with typed rejection or Internal failure instead of panic paths."
)]
pub(super) fn open_quotation(
    frame: super::Frame,
    node_id: crate::untrusted::NodeId,
    node: &crate::untrusted::Node,
    context: &super::parts::Ctx,
) -> Result<(u32, super::Frame, super::Frame), super::Fail> {
    let (body, inst) = match node {
        crate::untrusted::Node::Quotation { body, inst } => (body, inst),
        crate::untrusted::Node::Literal { .. }
        | crate::untrusted::Node::Invocation { .. }
        | crate::untrusted::Node::SlotInvoke { .. } => {
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
        borrowed: alloc::vec::Vec::new(),
        effects: crate::types::EffSet::empty(),
        claimed_out,
        claimed_effects,
    };
    Ok((cost, frame, child))
}
