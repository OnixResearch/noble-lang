//! Bounded shrinking for the property harness (DX-PROPERTY-01).
//!
//! On a disagreement the harness minimizes the case while the failure
//! predicate — "the oracle's decision and the kernel's outcome class still
//! disagree" — stays true. Every step is bounded; the walk stops after
//! `MAX_STEPS` adopted mutations or when no single removal simplifies
//! further.

use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Candidate, Node, Request};
use noble_kernel::words::Binding;

/// The shrink bound.
pub const MAX_STEPS: u32 = 64;

/// Shrink one failing case. Returns the minimized request and candidate and
/// the number of adopted mutations.
pub fn shrink<F>(request: &Request, candidate: &Candidate, holds: &F) -> (Request, Candidate, u32)
where
    F: Fn(&Request, &Candidate) -> bool,
{
    let mut current_request = request.clone();
    let mut current = candidate.clone();
    let mut steps = 0;
    while steps < MAX_STEPS {
        let mut progress = false;
        if let Some((next_request, next)) = try_simplify(&current_request, &current, holds) {
            current_request = next_request;
            current = next;
            steps += 1;
            progress = true;
        }
        if !progress {
            break;
        }
    }
    (current_request, current, steps)
}

/// Try every single-step simplification in priority order; return the first
/// that keeps the failure predicate true.
fn try_simplify<F>(
    request: &Request,
    candidate: &Candidate,
    holds: &F,
) -> Option<(Request, Candidate)>
where
    F: Fn(&Request, &Candidate) -> bool,
{
    drop_body_entry(request, candidate, holds)
        .or_else(|| drop_quote_entry(request, candidate, holds))
        .or_else(|| shrink_stack_binding(request, candidate, holds))
        .or_else(|| flatten_binding_type(request, candidate, holds))
        .or_else(|| drop_expected_out(request, candidate, holds))
}

fn drop_body_entry<F>(
    request: &Request,
    candidate: &Candidate,
    holds: &F,
) -> Option<(Request, Candidate)>
where
    F: Fn(&Request, &Candidate) -> bool,
{
    let mut position = 0;
    while position < candidate.body.len() {
        let mut next = candidate.clone();
        next.body.remove(position);
        if holds(request, &next) {
            return Some((request.clone(), next));
        }
        position += 1;
    }
    None
}

fn drop_quote_entry<F>(
    request: &Request,
    candidate: &Candidate,
    holds: &F,
) -> Option<(Request, Candidate)>
where
    F: Fn(&Request, &Candidate) -> bool,
{
    let mut node_index = 0;
    while node_index < candidate.nodes.len() {
        if let Node::Quotation { body, .. } = &candidate.nodes[node_index] {
            let mut position = 0;
            while position < body.len() {
                let mut next = candidate.clone();
                if let Node::Quotation { body, .. } = &mut next.nodes[node_index] {
                    body.remove(position);
                }
                if holds(request, &next) {
                    return Some((request.clone(), next));
                }
                position += 1;
            }
        }
        node_index += 1;
    }
    None
}

fn each_stack_binding<F>(candidate: &Candidate, mut visit: F) -> bool
where
    F: FnMut(usize, usize) -> bool,
{
    let mut node_index = 0;
    while node_index < candidate.nodes.len() {
        let inst = inst_of(&candidate.nodes[node_index]);
        let mut slot = 0;
        while slot < inst.bindings.len() {
            if let Binding::Stack(segment) = &inst.bindings[slot] {
                let _ = segment;
                if visit(node_index, slot) {
                    return true;
                }
            }
            slot += 1;
        }
        node_index += 1;
    }
    false
}

fn shrink_stack_binding<F>(
    request: &Request,
    candidate: &Candidate,
    holds: &F,
) -> Option<(Request, Candidate)>
where
    F: Fn(&Request, &Candidate) -> bool,
{
    let mut result: Option<(Request, Candidate)> = None;
    each_stack_binding(candidate, |node_index, slot| {
        let inst = inst_of(&candidate.nodes[node_index]);
        if let Binding::Stack(segment) = &inst.bindings[slot] {
            let mut position = 0;
            while result.is_none() && position < segment.len() {
                let mut next = candidate.clone();
                if let Binding::Stack(truncated) =
                    &mut inst_of_mut(&mut next.nodes[node_index]).bindings[slot]
                {
                    truncated.remove(position);
                }
                if holds(request, &next) {
                    result = Some((request.clone(), next));
                }
                position += 1;
            }
        }
        result.is_some()
    });
    result
}

fn flatten_binding_type<F>(
    request: &Request,
    candidate: &Candidate,
    holds: &F,
) -> Option<(Request, Candidate)>
where
    F: Fn(&Request, &Candidate) -> bool,
{
    let mut result: Option<(Request, Candidate)> = None;
    each_stack_binding(candidate, |node_index, slot| {
        let inst = inst_of(&candidate.nodes[node_index]);
        if let Binding::Stack(segment) = &inst.bindings[slot] {
            let mut position = 0;
            while result.is_none() && position < segment.len() {
                if !matches!(segment[position], Ty::I64) {
                    let mut next = candidate.clone();
                    if let Binding::Stack(changed) =
                        &mut inst_of_mut(&mut next.nodes[node_index]).bindings[slot]
                    {
                        changed[position] = Ty::I64;
                    }
                    if holds(request, &next) {
                        result = Some((request.clone(), next));
                    }
                }
                position += 1;
            }
        }
        result.is_some()
    });
    result
}

fn drop_expected_out<F>(
    request: &Request,
    candidate: &Candidate,
    holds: &F,
) -> Option<(Request, Candidate)>
where
    F: Fn(&Request, &Candidate) -> bool,
{
    if request.expected.stack_out.is_empty() {
        return None;
    }
    let mut next_request = request.clone();
    next_request.expected.stack_out.pop();
    if holds(&next_request, candidate) {
        return Some((next_request, candidate.clone()));
    }
    None
}

fn inst_of(node: &Node) -> &noble_kernel::words::Inst {
    match node {
        Node::Literal { inst, .. }
        | Node::Invocation { inst, .. }
        | Node::Quotation { inst, .. } => inst,
    }
}

fn inst_of_mut(node: &mut Node) -> &mut noble_kernel::words::Inst {
    match node {
        Node::Literal { inst, .. }
        | Node::Invocation { inst, .. }
        | Node::Quotation { inst, .. } => inst,
    }
}
