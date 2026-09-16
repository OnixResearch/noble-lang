//! The finite acceptance machine for the M2 fragment.
//!
//! The machine keeps an explicit frame stack (bounded by the declared depth
//! limit) instead of recursion, and threads its work meter and derivations as
//! returned state. Every conclusion is derived from the candidate's premises.

mod frames;
mod nodes;
mod parts;
mod preflight;

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

/// The machine between steps: its meter and frame stack, and, once the run
/// stops, its outcome.
struct Machine {
    state: State,
    frames: alloc::vec::Vec<Frame>,
    outcome: Option<Result<crate::untrusted::Interface, Fail>>,
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
    attempt!(preflight::check_request(env, request, candidate));
    let context = parts::Ctx { request, env };
    let mut machine = Machine {
        state: State {
            work: request.limits.work,
            derivations: alloc::vec::Vec::with_capacity(
                usize::try_from(request.limits.nodes.min(64)).unwrap_or(0),
            ),
        },
        frames: alloc::vec::Vec::with_capacity(
            usize::try_from(request.limits.depth.min(64)).unwrap_or(0),
        ),
        outcome: None,
    };
    machine.frames.push(frames::entry_frame(request, candidate));
    while machine.outcome.is_none() {
        machine = step(machine, candidate, &context);
    }
    match machine.outcome {
        Some(Ok(interface)) => Ok(crate::untrusted::Checked {
            interface,
            derivations: machine.state.derivations,
        }),
        Some(Err(problem)) => Err(problem),
        None => Err(Fail::Internal),
    }
}

/// Take one machine step: fold the current frame's next node.
fn step(
    mut machine: Machine,
    candidate: &crate::untrusted::Candidate,
    context: &parts::Ctx,
) -> Machine {
    let frame = match machine.frames.pop() {
        Some(frame) => frame,
        None => return halted(machine, Err(Fail::Internal)),
    };
    let node_id = match frame.body.get(frame.index) {
        Some(id) => *id,
        None => return close_frame(machine, frame, candidate, context),
    };
    let node = match nodes::node_of(candidate, node_id, context) {
        Ok(node) => node,
        Err(problem) => return halted(machine, Err(problem)),
    };
    match node {
        crate::untrusted::Node::Literal { .. } | crate::untrusted::Node::Invocation { .. } => {
            fold_current(machine, frame, node_id, &node, context)
        }
        crate::untrusted::Node::Quotation { .. } => {
            open_current(machine, frame, node_id, &node, context)
        }
    }
}

/// Stop the machine with this outcome.
fn halted(mut machine: Machine, outcome: Result<crate::untrusted::Interface, Fail>) -> Machine {
    machine.outcome = Some(outcome);
    machine
}

/// Close one exhausted frame, then stop the machine or continue it.
fn close_frame(
    mut machine: Machine,
    frame: Frame,
    candidate: &crate::untrusted::Candidate,
    context: &parts::Ctx,
) -> Machine {
    let parent = machine.frames.pop();
    let completion = match frames::complete_frame(frame, parent, candidate, context) {
        Ok(completion) => completion,
        Err(problem) => return halted(machine, Err(problem)),
    };
    match completion {
        frames::Completion::Entry(effects) => halted(
            machine,
            Ok(crate::untrusted::Interface {
                stack_in: context.request.expected.stack_in.clone(),
                stack_out: context.request.expected.stack_out.clone(),
                effects,
            }),
        ),
        frames::Completion::Step {
            cost,
            node,
            interface,
            joined,
        } => {
            let remaining = match parts::charge(machine.state.work, cost) {
                Ok(remaining) => remaining,
                Err(problem) => return halted(machine, Err(problem)),
            };
            machine.state.work = remaining;
            machine
                .state
                .derivations
                .push(crate::untrusted::Derivation { node, interface });
            machine.frames.push(frames::advance(joined));
            machine
        }
    }
}

/// Fold one literal or invocation node into the machine's counters.
fn fold_current(
    mut machine: Machine,
    frame: Frame,
    node_id: crate::untrusted::NodeId,
    node: &crate::untrusted::Node,
    context: &parts::Ctx,
) -> Machine {
    let (cost, joined, interface) = match nodes::fold_node(frame, node_id, node, context) {
        Ok(folded) => folded,
        Err(problem) => return halted(machine, Err(problem)),
    };
    let remaining = match parts::charge(machine.state.work, cost) {
        Ok(remaining) => remaining,
        Err(problem) => return halted(machine, Err(problem)),
    };
    machine.state.work = remaining;
    machine
        .state
        .derivations
        .push(crate::untrusted::Derivation {
            node: node_id,
            interface,
        });
    machine.frames.push(frames::advance(joined));
    machine
}

/// Open one quotation node into its parent and child frames.
fn open_current(
    mut machine: Machine,
    frame: Frame,
    node_id: crate::untrusted::NodeId,
    node: &crate::untrusted::Node,
    context: &parts::Ctx,
) -> Machine {
    let (cost, parent, child) = match nodes::open_quotation(frame, node_id, node, context) {
        Ok(opened) => opened,
        Err(problem) => return halted(machine, Err(problem)),
    };
    let remaining = match parts::charge(machine.state.work, cost) {
        Ok(remaining) => remaining,
        Err(problem) => return halted(machine, Err(problem)),
    };
    machine.state.work = remaining;
    machine.frames.push(parent);
    machine.frames.push(child);
    machine
}
