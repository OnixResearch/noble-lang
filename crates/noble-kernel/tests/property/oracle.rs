//! Independent acceptance oracle for the M2 fragment (DX-PROPERTY-01).
//!
//! This decision function is written separately from the kernel checker,
//! from the fragment's documented rule table ([verification/m2-fragment.md]
//! "Environment contracts" and "Rules"): its own type representation
//! (`otypes`), its own contract table (`table`), its own tail-join, and its
//! own frame machine below. It reads the kernel's public *input* structures
//! only — it calls no kernel decision code. The harness asserts the kernel
//! checker and this oracle agree on every generated candidate.
//!
//! The harness environment provides one effect identity (`test.emit`, id 0)
//! and one resource kind, so the oracle collapses resource kinds to one
//! constructor; no kernel decision distinguishes resource kinds.

use noble_kernel::untrusted::{Candidate, Node, NodeId, Request};
use noble_kernel::words::Inst;

use super::otypes::{is_data, is_subset, oty_stack, union, Decision, OTy};
use super::table::{
    append, effects_at, exact_arity, requires_data, stack_at, value_at, word_face, OFace,
};
use noble_kernel::untrusted::Lit;

/// The interface of one node under the oracle's rules: literals push their
/// type, quotations expose their claimed program, and invocations go through
/// the word table in `table`.
fn face(node: &Node) -> Option<OFace> {
    match node {
        Node::Literal { lit, inst } => {
            exact_arity(inst, 1)?;
            let under = stack_at(inst, 0)?;
            let lit_ty = match lit {
                Lit::I64(_) => OTy::I64,
                Lit::Bool(_) => OTy::Bool,
                Lit::Text => OTy::Text,
                Lit::Unit => OTy::Unit,
            };
            Some(OFace {
                input: under.clone(),
                output: append(under, &[lit_ty]),
                latent: vec![],
            })
        }
        Node::Invocation { def, inst } => word_face(def.0, inst),
        Node::Quotation { inst, .. } => {
            exact_arity(inst, 4)?;
            let (around, start, end, claimed) = (
                stack_at(inst, 0)?,
                stack_at(inst, 1)?,
                stack_at(inst, 2)?,
                effects_at(inst, 3)?,
            );
            let mut output = around.clone();
            output.push(OTy::Program(start, end, claimed));
            Some(OFace {
                input: around,
                output,
                latent: vec![],
            })
        }
    }
}

/// One in-progress body fold. `origin` names the quotation node a child
/// frame was opened for; the entry frame has none.
struct OFrame {
    body: Vec<NodeId>,
    index: usize,
    stack: Vec<super::otypes::OTy>,
    effects: Vec<u32>,
    claimed_out: Vec<super::otypes::OTy>,
    claimed_effects: Vec<u32>,
    origin: Option<NodeId>,
}

/// Decide one candidate against one request under the oracle's rules.
pub fn decide(request: &Request, candidate: &Candidate) -> Decision {
    if candidate.format != 0 || candidate.revision != 0 {
        return Decision::Reject;
    }
    if candidate.nodes.len() > request.limits.nodes as usize {
        return Decision::Reject;
    }
    if request
        .expected
        .allowed_effects
        .as_slice()
        .iter()
        .any(|id| id.0 != 0)
    {
        return Decision::Reject;
    }
    let mut frames = vec![OFrame {
        body: candidate.body.clone(),
        index: 0,
        stack: oty_stack(&request.expected.stack_in),
        effects: vec![],
        claimed_out: oty_stack(&request.expected.stack_out),
        claimed_effects: request
            .expected
            .allowed_effects
            .as_slice()
            .iter()
            .map(|id| id.0)
            .collect(),
        origin: None,
    }];
    while let Some(frame) = frames.pop() {
        let node_id = match frame.body.get(frame.index) {
            Some(id) => *id,
            None => match close(frame, &mut frames, candidate) {
                Some(decision) => return decision,
                None => continue,
            },
        };
        let index = usize::try_from(node_id.0).unwrap_or(usize::MAX);
        let node = match candidate.nodes.get(index) {
            Some(node) => node,
            None => return Decision::Reject,
        };
        let quotation = match node {
            Node::Quotation { body, inst } => Some((body.clone(), inst.clone())),
            _ => None,
        };
        match quotation {
            Some((body, inst)) => {
                let opened = match (stack_at(&inst, 1), stack_at(&inst, 2), effects_at(&inst, 3)) {
                    (Some(a), Some(c), Some(e)) => (a, c, e),
                    _ => return Decision::Reject,
                };
                let child = OFrame {
                    body,
                    index: 0,
                    stack: opened.0,
                    effects: vec![],
                    claimed_out: opened.1,
                    claimed_effects: opened.2,
                    origin: Some(node_id),
                };
                frames.push(frame);
                frames.push(child);
            }
            None => {
                let interface = match face(node) {
                    Some(found) => found,
                    None => return Decision::Reject,
                };
                if let Node::Invocation { def, .. } = node {
                    if requires_data(def.0) {
                        match value_at(inst_of(node), 1) {
                            Some(ty) if !is_data(&ty) => return Decision::Reject,
                            None => return Decision::Reject,
                            Some(_) => {}
                        }
                    }
                }
                // Every latent identity must be one the environment
                // provides; the harness environment provides id 0 only.
                if interface.latent.iter().any(|id| *id != 0) {
                    return Decision::Reject;
                }
                let mut next = frame;
                let input = interface.input;
                if next.stack.len() < input.len()
                    || next.stack[next.stack.len() - input.len()..] != input[..]
                {
                    return Decision::Reject;
                }
                next.stack.truncate(next.stack.len() - input.len());
                next.stack.extend(interface.output);
                next.effects = union(&next.effects, &interface.latent);
                next.index += 1;
                frames.push(next);
            }
        }
    }
    Decision::Reject
}

fn inst_of(node: &Node) -> &Inst {
    match node {
        Node::Literal { inst, .. }
        | Node::Invocation { inst, .. }
        | Node::Quotation { inst, .. } => inst,
    }
}

/// Complete one frame: check its joins, then return its completion upward.
/// `Some(decision)` ends the run; `None` resumes the parent frame.
fn close(frame: OFrame, frames: &mut Vec<OFrame>, candidate: &Candidate) -> Option<Decision> {
    if frame.stack != frame.claimed_out || !is_subset(&frame.effects, &frame.claimed_effects) {
        return Some(Decision::Reject);
    }
    let origin = match frame.origin {
        Some(origin) => origin,
        None => return Some(Decision::Accept),
    };
    let mut parent = match frames.pop() {
        Some(parent) => parent,
        None => return Some(Decision::Reject),
    };
    let index = usize::try_from(origin.0).unwrap_or(usize::MAX);
    let interface = match candidate.nodes.get(index) {
        None => return Some(Decision::Reject),
        Some(node) => match face(node) {
            Some(found) => found,
            None => return Some(Decision::Reject),
        },
    };
    let input = interface.input;
    if parent.stack.len() < input.len()
        || parent.stack[parent.stack.len() - input.len()..] != input[..]
    {
        return Some(Decision::Reject);
    }
    parent.stack.truncate(parent.stack.len() - input.len());
    parent.stack.extend(interface.output);
    parent.effects = union(&parent.effects, &interface.latent);
    parent.index += 1;
    frames.push(parent);
    None
}
