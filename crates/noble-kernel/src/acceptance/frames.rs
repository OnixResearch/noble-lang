//! Frame completion: closing one body fold and returning its result upward.

/// What completing one frame produced for the machine.
pub(super) enum Completion {
    /// The entry body completed; its derived effect bound is the result.
    Entry(crate::types::EffSet),
    /// A nested body completed and its parent frame is joined and advanced.
    Step {
        /// Work charged for the parent join.
        cost: u32,
        /// The completed node.
        node: crate::untrusted::NodeId,
        /// Its rebuilt interface.
        interface: crate::untrusted::Interface,
        /// The advanced parent frame.
        joined: super::Frame,
    },
}

/// The entry frame for one request.
pub(super) fn entry_frame(
    request: &crate::untrusted::Request,
    candidate: &crate::untrusted::Candidate,
) -> super::Frame {
    super::Frame {
        depth: 0,
        origin: None,
        body: candidate.body.clone(),
        index: 0,
        stack: request.expected.stack_in.clone(),
        effects: crate::types::EffSet::empty(),
        claimed_out: request.expected.stack_out.clone(),
        claimed_effects: request.expected.allowed_effects.clone(),
    }
}

/// Advance a frame to its next body position.
pub(super) fn advance(mut frame: super::Frame) -> super::Frame {
    frame.index = frame.index.saturating_add(1);
    frame
}

/// Complete one frame: check its joins, then return its completion upward.
pub(super) fn complete_frame(
    frame: super::Frame,
    parent: Option<super::Frame>,
    candidate: &crate::untrusted::Candidate,
    ctx: &super::parts::Ctx,
) -> Result<Completion, super::Fail> {
    if frame.stack != frame.claimed_out {
        return Err(super::parts::invalid(
            ctx,
            super::parts::site(frame.origin, None),
            frame.claimed_out.clone(),
            frame.stack.clone(),
            super::parts::mismatch_constraint(&frame.claimed_out, &frame.stack),
        ));
    }
    if !frame.effects.is_subset_of(&frame.claimed_effects) {
        let id = super::parts::first_extra(&frame.effects, &frame.claimed_effects)
            .unwrap_or(crate::types::EffId(0));
        return Err(super::parts::invalid(
            ctx,
            super::parts::site(frame.origin, None),
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::EffectInclusion(id),
        ));
    }
    match frame.origin {
        None => Ok(Completion::Entry(frame.effects)),
        Some(node_id) => {
            let parent_frame = match parent {
                Some(parent_frame) => parent_frame,
                None => return Err(super::Fail::Internal),
            };
            let inst = match usize::try_from(node_id.0)
                .ok()
                .and_then(|index| candidate.nodes.get(index))
            {
                Some(crate::untrusted::Node::Quotation { inst, .. }) => inst.clone(),
                Some(crate::untrusted::Node::Literal { .. })
                | Some(crate::untrusted::Node::Invocation { .. })
                | None => return Err(super::Fail::Internal),
            };
            let scheme = super::parts::instantiate::quotation_scheme();
            let interface = attempt!(super::parts::instantiate::apply(
                &scheme,
                &inst,
                None,
                super::parts::site(Some(node_id), None),
                ctx,
            ));
            let cost = attempt!(super::parts::join_cost(&interface));
            let joined = attempt!(super::parts::join(
                parent_frame,
                &interface,
                super::parts::site(
                    Some(node_id),
                    super::parts::definition_of(candidate, node_id),
                ),
                ctx,
            ));
            Ok(Completion::Step {
                cost,
                node: node_id,
                interface,
                joined,
            })
        }
    }
}
