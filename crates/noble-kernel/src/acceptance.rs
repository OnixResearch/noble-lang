//! The finite acceptance machine for the M2 fragment.
//!
//! The machine keeps an explicit frame stack (bounded by the declared depth
//! limit) instead of recursion, and threads its work meter and derivations as
//! returned state. Every conclusion is derived from the candidate's premises.

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

enum Finished {
    Entry(crate::types::EffSet),
    Nested(Taken),
}

struct Taken {
    node: crate::untrusted::NodeId,
    interface: crate::untrusted::Interface,
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
    if u64::try_from(candidate.nodes.len()).unwrap_or(u64::MAX) > u64::from(request.limits.nodes) {
        return Err(Fail::Exhausted(crate::untrusted::LimitKind::Nodes));
    }
    for scheme in &env.defs {
        if scheme.validate().is_err() {
            return Err(Fail::Unsupported(
                crate::untrusted::UnsupportedKind::SchemeForm,
            ));
        }
    }
    parts::limits_of(&request.expected.stack_in, request)?;
    parts::limits_of(&request.expected.stack_out, request)?;
    for id in request.expected.allowed_effects.as_slice() {
        if !env.knows_effect(*id) {
            return Err(parts::invalid(
                request,
                None,
                None,
                alloc::vec::Vec::new(),
                alloc::vec::Vec::new(),
                crate::untrusted::Constraint::UnknownEffect(*id),
            ));
        }
    }
    let mut state = State {
        work: request.limits.work,
        derivations: alloc::vec::Vec::with_capacity(
            usize::try_from(request.limits.nodes.min(64)).unwrap_or(usize::MAX),
        ),
    };
    let mut frames = alloc::vec::Vec::with_capacity(
        usize::try_from(request.limits.depth.min(64)).unwrap_or(usize::MAX),
    );
    frames.push(Frame {
        depth: 0,
        origin: None,
        body: candidate.body.clone(),
        index: 0,
        stack: request.expected.stack_in.clone(),
        effects: crate::types::EffSet::empty(),
        claimed_out: request.expected.stack_out.clone(),
        claimed_effects: request.expected.allowed_effects.clone(),
    });
    loop {
        let frame = match frames.pop() {
            Some(frame) => frame,
            None => return Err(Fail::Internal),
        };
        let node_id = match frame.body.get(frame.index) {
            Some(id) => *id,
            None => match finish_frame(frame, candidate, env, request)? {
                Finished::Entry(effects) => {
                    return Ok(crate::untrusted::Checked {
                        interface: crate::untrusted::Interface {
                            stack_in: request.expected.stack_in.clone(),
                            stack_out: request.expected.stack_out.clone(),
                            effects,
                        },
                        derivations: state.derivations,
                    })
                }
                Finished::Nested(taken) => {
                    let parent = match frames.pop() {
                        Some(parent) => parent,
                        None => return Err(Fail::Internal),
                    };
                    state.work = parts::charge(state.work, parts::join_cost(&taken.interface)?)?;
                    let joined = parts::join(
                        parent,
                        &taken.interface,
                        taken.node,
                        parts::definition_of(candidate, taken.node),
                        request,
                    )?;
                    state.derivations.push(crate::untrusted::Derivation {
                        node: taken.node,
                        interface: taken.interface,
                    });
                    frames.push(advance(joined));
                    continue;
                }
            },
        };
        match candidate
            .nodes
            .get(usize::try_from(node_id.0).unwrap_or(usize::MAX))
        {
            None => {
                return Err(parts::invalid(
                    request,
                    Some(node_id),
                    None,
                    alloc::vec::Vec::new(),
                    alloc::vec::Vec::new(),
                    crate::untrusted::Constraint::MalformedReference(node_id),
                ))
            }
            Some(node) => match node {
                crate::untrusted::Node::Literal { lit, inst } => {
                    let scheme = parts::literal_scheme(*lit);
                    state.work = parts::charge(state.work, parts::scheme_cost(&scheme)?)?;
                    let interface =
                        parts::instantiate(&scheme, inst, None, None, node_id, request, env)?;
                    state.work = parts::charge(state.work, parts::join_cost(&interface)?)?;
                    let joined = parts::join(frame, &interface, node_id, None, request)?;
                    state.derivations.push(crate::untrusted::Derivation {
                        node: node_id,
                        interface,
                    });
                    frames.push(advance(joined));
                }
                crate::untrusted::Node::Invocation { def, inst } => {
                    let scheme = match env.scheme(*def) {
                        Some(scheme) => scheme.clone(),
                        None => {
                            return Err(parts::invalid(
                                request,
                                Some(node_id),
                                Some(*def),
                                alloc::vec::Vec::new(),
                                alloc::vec::Vec::new(),
                                crate::untrusted::Constraint::UnknownDefinition(*def),
                            ))
                        }
                    };
                    state.work = parts::charge(state.work, parts::scheme_cost(&scheme)?)?;
                    let data_var = parts::data_slot(env.kind(*def));
                    let interface = parts::instantiate(
                        &scheme,
                        inst,
                        data_var,
                        Some(*def),
                        node_id,
                        request,
                        env,
                    )?;
                    state.work = parts::charge(state.work, parts::join_cost(&interface)?)?;
                    let joined = parts::join(frame, &interface, node_id, Some(*def), request)?;
                    state.derivations.push(crate::untrusted::Derivation {
                        node: node_id,
                        interface,
                    });
                    frames.push(advance(joined));
                }
                crate::untrusted::Node::Quotation { body, inst } => {
                    let scheme = parts::quotation_scheme();
                    state.work = parts::charge(state.work, parts::scheme_cost(&scheme)?)?;
                    parts::instantiate(&scheme, inst, None, None, node_id, request, env)?;
                    if frame.depth.saturating_add(1) > request.limits.depth {
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
                    frames.push(frame);
                    frames.push(Frame {
                        depth: frame_ref_depth(&frames),
                        origin: Some(node_id),
                        body: body.clone(),
                        index: 0,
                        stack: start,
                        effects: crate::types::EffSet::empty(),
                        claimed_out,
                        claimed_effects,
                    });
                }
            },
        }
    }
}

fn frame_ref_depth(frames: &[Frame]) -> u32 {
    match frames.last() {
        Some(frame) => frame.depth.saturating_add(1),
        None => 1,
    }
}

fn advance(mut frame: Frame) -> Frame {
    frame.index = frame.index.saturating_add(1);
    frame
}

fn finish_frame(
    frame: Frame,
    candidate: &crate::untrusted::Candidate,
    env: &crate::contracts::Env,
    request: &crate::untrusted::Request,
) -> Result<Finished, Fail> {
    if frame.stack != frame.claimed_out {
        return Err(parts::invalid(
            request,
            frame.origin,
            None,
            frame.claimed_out.clone(),
            frame.stack.clone(),
            parts::mismatch_constraint(&frame.claimed_out, &frame.stack),
        ));
    }
    if !frame.effects.is_subset_of(&frame.claimed_effects) {
        let id = parts::first_extra(&frame.effects, &frame.claimed_effects)
            .unwrap_or(crate::types::EffId(0));
        return Err(parts::invalid(
            request,
            frame.origin,
            None,
            alloc::vec::Vec::new(),
            alloc::vec::Vec::new(),
            crate::untrusted::Constraint::EffectInclusion(id),
        ));
    }
    match frame.origin {
        None => Ok(Finished::Entry(frame.effects)),
        Some(node_id) => {
            let inst = match candidate
                .nodes
                .get(usize::try_from(node_id.0).unwrap_or(usize::MAX))
            {
                Some(crate::untrusted::Node::Quotation { inst, .. }) => inst.clone(),
                Some(crate::untrusted::Node::Literal { .. })
                | Some(crate::untrusted::Node::Invocation { .. })
                | None => return Err(Fail::Internal),
            };
            let scheme = parts::quotation_scheme();
            let interface = parts::instantiate(&scheme, &inst, None, None, node_id, request, env)?;
            Ok(Finished::Nested(Taken {
                node: node_id,
                interface,
            }))
        }
    }
}
