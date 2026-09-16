//! The finite acceptance machine for the M2 fragment.
//!
//! The machine keeps an explicit frame stack (bounded by the declared depth
//! limit) instead of recursion, and threads its work meter and derivations as
//! returned state. Every conclusion is derived from the candidate's premises.

mod frames;
mod parts;

enum Fail {
    Invalid(crate::untrusted::Diagnostic),
    Unsupported(crate::untrusted::UnsupportedKind),
    Exhausted(crate::untrusted::LimitKind),
    Internal,
}

/// One frame: an in-progress body fold.
struct Frame {
    depth: u32,
    origin: Option<crate::untrusted::NodeId>,
    body: alloc::vec::Vec<crate::untrusted::NodeId>,
    index: usize,
    stack: alloc::vec::Vec<crate::types::Ty>,
    effects: crate::types::EffSet,
    claimed_out: alloc::vec::Vec<crate::types::Ty>,
    claimed_effects: crate::types::EffSet,
}

struct State {
    work: u32,
    derivations: alloc::vec::Vec<crate::untrusted::Derivation>,
}

/// Run the acceptance checker over one candidate and request.
pub fn check(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
    candidate: &crate::untrusted::Candidate,
) -> crate::untrusted::Outcome {
    match run(env, request, candidate) {
        Ok(checked) => crate::untrusted::Outcome::Accepted(checked),
        Err(Fail::Invalid(diagnostic)) => crate::untrusted::Outcome::Invalid(diagnostic),
        Err(Fail::Unsupported(kind)) => crate::untrusted::Outcome::Unsupported(kind),
        Err(Fail::Exhausted(limit)) => crate::untrusted::Outcome::Exhausted(limit),
        Err(Fail::Internal) => crate::untrusted::Outcome::InternalFailure,
    }
}

fn run(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
    candidate: &crate::untrusted::Candidate,
) -> Result<crate::untrusted::Checked, Fail> {
    preflight(env, request, candidate)?;
    let context = parts::Ctx { request, env };
    let mut state = State {
        work: request.limits.work,
        derivations: alloc::vec::Vec::with_capacity(
            usize::try_from(request.limits.nodes.min(64)).unwrap_or(0),
        ),
    };
    let mut frames =
        alloc::vec::Vec::with_capacity(usize::try_from(request.limits.depth.min(64)).unwrap_or(0));
    frames.push(frames::entry_frame(request, candidate));
    loop {
        let frame = match frames.pop() {
            Some(frame) => frame,
            None => return Err(Fail::Internal),
        };
        let node_id = match frame.body.get(frame.index) {
            Some(id) => *id,
            None => {
                let parent = frames.pop();
                match frames::complete_frame(frame, parent, candidate, &context)? {
                    frames::Completion::Entry(effects) => {
                        return Ok(crate::untrusted::Checked {
                            interface: crate::untrusted::Interface {
                                stack_in: request.expected.stack_in.clone(),
                                stack_out: request.expected.stack_out.clone(),
                                effects,
                            },
                            derivations: state.derivations,
                        })
                    }
                    frames::Completion::Step {
                        cost,
                        node,
                        interface,
                        joined,
                    } => {
                        state.work = parts::charge(state.work, cost)?;
                        state
                            .derivations
                            .push(crate::untrusted::Derivation { node, interface });
                        frames.push(frames::advance(joined));
                        continue;
                    }
                }
            }
        };
        let node = node_of(candidate, node_id, &context)?;
        match node {
            crate::untrusted::Node::Literal { .. } | crate::untrusted::Node::Invocation { .. } => {
                let (cost, joined, interface) = fold_node(frame, node_id, node, &context)?;
                state.work = parts::charge(state.work, cost)?;
                state.derivations.push(crate::untrusted::Derivation {
                    node: node_id,
                    interface,
                });
                frames.push(frames::advance(joined));
            }
            crate::untrusted::Node::Quotation { .. } => {
                let (cost, parent, child) = open_quotation(frame, node_id, node, &context)?;
                state.work = parts::charge(state.work, cost)?;
                frames.push(parent);
                frames.push(child);
            }
        }
    }
}

/// Fetch one node, rejecting a reference outside the finite arena.
fn node_of<'a>(
    candidate: &'a crate::untrusted::Candidate,
    node_id: crate::untrusted::NodeId,
    context: &parts::Ctx,
) -> Result<&'a crate::untrusted::Node, Fail> {
    match usize::try_from(node_id.0)
        .ok()
        .and_then(|index| candidate.nodes.get(index))
    {
        Some(node) => Ok(node),
        None => Err(parts::invalid(
            context,
            parts::site(Some(node_id), None),
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::MalformedReference(node_id),
        )),
    }
}

/// Fold one literal or invocation node into its frame.
///
/// Returns the work charged for the node's instantiation and join.
fn fold_node(
    frame: Frame,
    node_id: crate::untrusted::NodeId,
    node: &crate::untrusted::Node,
    context: &parts::Ctx,
) -> Result<(u32, Frame, crate::untrusted::Interface), Fail> {
    match node {
        crate::untrusted::Node::Literal { lit, inst } => {
            let scheme = parts::instantiate::literal_scheme(*lit);
            let at = parts::site(Some(node_id), None);
            let cost = parts::scheme_cost(&scheme)?;
            let interface = parts::instantiate::apply(&scheme, inst, None, at, context)?;
            let cost = cost.saturating_add(parts::join_cost(&interface)?);
            let joined = parts::join(frame, &interface, at, context)?;
            Ok((cost, joined, interface))
        }
        crate::untrusted::Node::Invocation { def, inst } => {
            let scheme = match context.env.scheme(*def) {
                Some(scheme) => scheme.clone(),
                None => {
                    return Err(parts::invalid(
                        context,
                        parts::site(Some(node_id), Some(*def)),
                        alloc::vec::Vec::new(),
                        alloc::vec::Vec::new(),
                        crate::untrusted::Constraint::UnknownDefinition(*def),
                    ))
                }
            };
            let at = parts::site(Some(node_id), Some(*def));
            let data_var = parts::instantiate::data_slot(context.env.kind(*def));
            let cost = parts::scheme_cost(&scheme)?;
            let interface = parts::instantiate::apply(&scheme, inst, data_var, at, context)?;
            let cost = cost.saturating_add(parts::join_cost(&interface)?);
            let joined = parts::join(frame, &interface, at, context)?;
            Ok((cost, joined, interface))
        }
        crate::untrusted::Node::Quotation { .. } => Err(Fail::Internal),
    }
}

/// Validate one quotation node and build its parent and child frames.
///
/// Returns the work charged for the node's instantiation.
fn open_quotation(
    frame: Frame,
    node_id: crate::untrusted::NodeId,
    node: &crate::untrusted::Node,
    context: &parts::Ctx,
) -> Result<(u32, Frame, Frame), Fail> {
    let (body, inst) = match node {
        crate::untrusted::Node::Quotation { body, inst } => (body, inst),
        crate::untrusted::Node::Literal { .. } | crate::untrusted::Node::Invocation { .. } => {
            return Err(Fail::Internal)
        }
    };
    let scheme = parts::instantiate::quotation_scheme();
    let cost = parts::scheme_cost(&scheme)?;
    parts::instantiate::apply(
        &scheme,
        inst,
        None,
        parts::site(Some(node_id), None),
        context,
    )?;
    if frame.depth.saturating_add(1) > context.request.limits.depth {
        return Err(Fail::Exhausted(crate::untrusted::LimitKind::Depth));
    }
    let start = match inst.stack(crate::words::Variable(1)) {
        Some(segment) => segment.to_vec(),
        None => return Err(Fail::Internal),
    };
    let claimed_out = match inst.stack(crate::words::Variable(2)) {
        Some(segment) => segment.to_vec(),
        None => return Err(Fail::Internal),
    };
    let claimed_effects = match inst.effects(crate::words::Variable(3)) {
        Some(set) => set.clone(),
        None => return Err(Fail::Internal),
    };
    let depth = frame.depth.saturating_add(1);
    let child = Frame {
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

fn preflight(
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
    candidate: &crate::untrusted::Candidate,
) -> Result<(), Fail> {
    if candidate.format != crate::untrusted::CANDIDATE_FORMAT
        || candidate.revision != crate::untrusted::SEMANTIC_REVISION
    {
        return Err(Fail::Unsupported(
            crate::untrusted::UnsupportedKind::FormatRevision,
        ));
    }
    if request.input_bytes > request.limits.bytes {
        return Err(Fail::Exhausted(crate::untrusted::LimitKind::Bytes));
    }
    let node_count = match u64::try_from(candidate.nodes.len()) {
        Ok(count) => count,
        Err(_) => return Err(Fail::Exhausted(crate::untrusted::LimitKind::Nodes)),
    };
    if node_count > u64::from(request.limits.nodes) {
        return Err(Fail::Exhausted(crate::untrusted::LimitKind::Nodes));
    }
    for scheme in &env.defs {
        if scheme.validate().is_err() {
            return Err(Fail::Unsupported(
                crate::untrusted::UnsupportedKind::SchemeForm,
            ));
        }
    }
    let context = parts::Ctx { request, env };
    parts::limits_of(&request.expected.stack_in, &context)?;
    parts::limits_of(&request.expected.stack_out, &context)?;
    for id in request.expected.allowed_effects.as_slice() {
        if !env.knows_effect(*id) {
            return Err(parts::invalid(
                &context,
                parts::site(None, None),
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::UnknownEffect(*id),
            ));
        }
    }
    Ok(())
}
