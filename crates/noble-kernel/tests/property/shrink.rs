//! Bounded shrinking for the property harness (DX-PROPERTY-01).
//!
//! On a disagreement the harness minimizes the case while the failure
//! predicate — "the oracle's decision and the kernel's outcome class still
//! disagree" — stays true. Every step is bounded; the walk stops after
//! `MAX_STEPS` adopted mutations or when no single removal simplifies
//! further.

/// The shrink bound.
pub const MAX_STEPS: u32 = 64;

/// Shrink one failing case. Returns the minimized request and candidate and
/// the number of adopted mutations.
pub fn minimize<F>(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
    holds: &F,
) -> (
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
    u32,
)
where
    F: Fn(&noble_kernel::untrusted::Request, &noble_kernel::untrusted::Candidate) -> bool,
{
    let mut current_request = request.clone();
    let mut current = candidate.clone();
    let mut steps = 0;
    while steps < MAX_STEPS {
        match try_simplify(&current_request, &current, holds) {
            Some((next_request, next)) => {
                current_request = next_request;
                current = next;
                steps += 1;
            }
            None => break,
        }
    }
    (current_request, current, steps)
}

/// Try every single-step simplification in priority order; return the first
/// that keeps the failure predicate true.
fn try_simplify<F>(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
    holds: &F,
) -> Option<(
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
)>
where
    F: Fn(&noble_kernel::untrusted::Request, &noble_kernel::untrusted::Candidate) -> bool,
{
    drop_body_entry(request, candidate, holds)
        .or_else(|| drop_quote_entry(request, candidate, holds))
        .or_else(|| drop_stack_entry(request, candidate, holds))
        .or_else(|| flatten_binding_type(request, candidate, holds))
        .or_else(|| drop_expected_out(request, candidate, holds))
}

fn drop_body_entry<F>(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
    holds: &F,
) -> Option<(
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
)>
where
    F: Fn(&noble_kernel::untrusted::Request, &noble_kernel::untrusted::Candidate) -> bool,
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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; drop_quote_entry checks holds on each candidate clone and returns only the first predicate-preserving quotation removal; failed probes are normal shrinking decisions, not assertion failures."
)]
fn drop_quote_entry<F>(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
    holds: &F,
) -> Option<(
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
)>
where
    F: Fn(&noble_kernel::untrusted::Request, &noble_kernel::untrusted::Candidate) -> bool,
{
    let mut node_index = 0;
    while node_index < candidate.nodes.len() {
        if let noble_kernel::untrusted::Node::Quotation { body, .. } = &candidate.nodes[node_index]
        {
            let mut position = 0;
            while position < body.len() {
                let mut next = candidate.clone();
                if let noble_kernel::untrusted::Node::Quotation { body, .. } =
                    &mut next.nodes[node_index]
                {
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

fn each_stack_binding<F>(candidate: &noble_kernel::untrusted::Candidate, mut visit: F) -> bool
where
    F: FnMut(usize, usize) -> bool,
{
    let mut node_index = 0;
    while node_index < candidate.nodes.len() {
        let inst = inst_of(&candidate.nodes[node_index]);
        let mut slot = 0;
        while slot < inst.bindings.len() {
            if matches!(&inst.bindings[slot], noble_kernel::words::Binding::Stack(_))
                && visit(node_index, slot)
            {
                return true;
            }
            slot += 1;
        }
        node_index += 1;
    }
    false
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; drop_stack_entry preserves traversal priority and adopts a single removal only when holds succeeds; assertions would duplicate the caller-supplied disagreement predicate."
)]
fn drop_stack_entry<F>(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
    holds: &F,
) -> Option<(
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
)>
where
    F: Fn(&noble_kernel::untrusted::Request, &noble_kernel::untrusted::Candidate) -> bool,
{
    let mut result = None;
    each_stack_binding(candidate, |node_index, slot| {
        let inst = inst_of(&candidate.nodes[node_index]);
        if let noble_kernel::words::Binding::Stack(segment) = &inst.bindings[slot] {
            let mut position = 0;
            while result.is_none() && position < segment.len() {
                let mut next = candidate.clone();
                if let noble_kernel::words::Binding::Stack(truncated) =
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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; flatten_binding_type changes one non-I64 stack entry and adopts it only if holds remains true, preserving first-match order and the outer shrink bound without assertion padding."
)]
fn flatten_binding_type<F>(
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
    holds: &F,
) -> Option<(
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
)>
where
    F: Fn(&noble_kernel::untrusted::Request, &noble_kernel::untrusted::Candidate) -> bool,
{
    let mut result = None;
    each_stack_binding(candidate, |node_index, slot| {
        let inst = inst_of(&candidate.nodes[node_index]);
        if let noble_kernel::words::Binding::Stack(segment) = &inst.bindings[slot] {
            let mut position = 0;
            while result.is_none() && position < segment.len() {
                if !matches!(segment[position], noble_kernel::types::Ty::I64) {
                    let mut next = candidate.clone();
                    if let noble_kernel::words::Binding::Stack(changed) =
                        &mut inst_of_mut(&mut next.nodes[node_index]).bindings[slot]
                    {
                        changed[position] = noble_kernel::types::Ty::I64;
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
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
    holds: &F,
) -> Option<(
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
)>
where
    F: Fn(&noble_kernel::untrusted::Request, &noble_kernel::untrusted::Candidate) -> bool,
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

const fn inst_of(node: &noble_kernel::untrusted::Node) -> &noble_kernel::words::Inst {
    match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. }
        | noble_kernel::untrusted::Node::Quotation { inst, .. } => inst,
    }
}

const fn inst_of_mut(node: &mut noble_kernel::untrusted::Node) -> &mut noble_kernel::words::Inst {
    match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. }
        | noble_kernel::untrusted::Node::Quotation { inst, .. } => inst,
    }
}
