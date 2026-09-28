#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; this bounded worklist mutates only its fresh done vector and compiler-private plan, never the borrowed candidate; no mutable state is exposed by compile."
)]

/// A bounded Kahn worklist over checked quotation edges. Arena order is not
/// semantic order: later arena entries may be children of earlier entries.
pub(crate) fn order(
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &mut crate::lowering::Plan,
    root: crate::signatures::Interface,
) -> Result<(), crate::Diagnostic> {
    let (mut done, mut remaining) = initial_queue(&plan.nodes);
    let mut failure = None;
    while remaining != 0 {
        match schedule_step(candidate, plan, &mut done, remaining) {
            Ok(next) => remaining = next,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    finish_order(candidate, plan, root, &done)
}

fn finish_order(
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &mut crate::lowering::Plan,
    root: crate::signatures::Interface,
    done: &[bool],
) -> Result<(), crate::Diagnostic> {
    if !attempt!(ready(&candidate.body, plan, done)) {
        return Err(crate::Diagnostic::Defective);
    }
    append(plan, None, root, candidate.body.len())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; pinned March and August const trials report E0015 for schedule_round and E0658 for Option::ok_or; reassess when these scheduling APIs are const-capable."
)]
fn schedule_step(
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &mut crate::lowering::Plan,
    done: &mut [bool],
    remaining: usize,
) -> Result<usize, crate::Diagnostic> {
    let progressed = attempt!(schedule_round(candidate, plan, done));
    if progressed == 0 {
        // Successful kernel admission rules out quotation cycles. Never
        // replace an unexpected cycle with an empty or partial program.
        return Err(crate::Diagnostic::Defective);
    }
    remaining
        .checked_sub(progressed)
        .ok_or(crate::Diagnostic::Defective)
}

fn initial_queue(nodes: &[Option<crate::lowering::Operation>]) -> (alloc::vec::Vec<bool>, usize) {
    let mut done = alloc::vec::Vec::with_capacity(nodes.len());
    let mut remaining = 0usize;
    let mut index = 0usize;
    while index < nodes.len() {
        let is_program = is_quotation(nodes[index]);
        done.push(!is_program);
        if is_program {
            remaining += 1;
        }
        index += 1;
    }
    (done, remaining)
}

const fn is_quotation(operation: Option<crate::lowering::Operation>) -> bool {
    match operation {
        Some(crate::lowering::Operation::Program(_)) => true,
        Some(_) | None => false,
    }
}

fn schedule_round(
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &mut crate::lowering::Plan,
    done: &mut [bool],
) -> Result<usize, crate::Diagnostic> {
    let mut progressed = 0usize;
    let mut index = 0usize;
    let mut failure = None;
    while index < plan.nodes.len() {
        if !done[index] {
            if let Err(problem) = schedule_node(candidate, plan, done, index, &mut progressed) {
                failure = Some(problem);
                break;
            }
        }
        index += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(progressed),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; pinned March const trial reports E0277 and August reports E0658 for Vec Index, with E0015 for body, ready and append; reassess when these checked scheduling APIs are const-capable."
)]
fn schedule_node(
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &mut crate::lowering::Plan,
    done: &mut [bool],
    index: usize,
    progressed: &mut usize,
) -> Result<(), crate::Diagnostic> {
    let interface = match plan.nodes[index] {
        Some(crate::lowering::Operation::Program(interface)) => interface,
        Some(_) | None => return Err(crate::Diagnostic::Defective),
    };
    let owner = match u32::try_from(index) {
        Ok(owner) => owner,
        Err(_) => return Err(crate::Diagnostic::Defective),
    };
    let entries = attempt!(body(candidate, Some(owner)));
    if attempt!(ready(entries, plan, done)) {
        attempt!(append(plan, Some(owner), interface, entries.len()));
        done[index] = true;
        *progressed += 1;
    }
    Ok(())
}

fn ready(
    body: &[noble_kernel::untrusted::NodeId],
    plan: &crate::lowering::Plan,
    done: &[bool],
) -> Result<bool, crate::Diagnostic> {
    let mut index = 0usize;
    let mut result = Ok(true);
    while index < body.len() {
        match ready_node(body[index], plan, done) {
            Ok(true) => index += 1,
            outcome => {
                result = outcome;
                break;
            }
        }
    }
    result
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; pinned March and August const trials report E0015 for Plan::operation and admission::index and E0658 for slice::get; reassess when these checked lookup APIs are const-capable."
)]
fn ready_node(
    node: noble_kernel::untrusted::NodeId,
    plan: &crate::lowering::Plan,
    done: &[bool],
) -> Result<bool, crate::Diagnostic> {
    let operation = attempt!(plan.operation(node));
    if is_quotation(Some(operation)) {
        let child = attempt!(crate::admission::index(node));
        match done.get(child) {
            Some(true) => {}
            Some(false) => return Ok(false),
            None => return Err(crate::Diagnostic::Defective),
        }
    }
    Ok(true)
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; checked conversion, addition and declared limits precede growing the private program Vec, which cannot be const; quota failure remains a diagnostic rather than an assertion."
)]
fn append(
    plan: &mut crate::lowering::Plan,
    owner: Option<u32>,
    interface: crate::signatures::Interface,
    operations: usize,
) -> Result<(), crate::Diagnostic> {
    if plan.programs.len() >= crate::BODY_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    let count = match u32::try_from(operations) {
        Ok(count) => count,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    let end = match plan.functions.checked_add(count) {
        Some(end) => end,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    if end > crate::FUNCTION_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    plan.programs.push(crate::lowering::Program {
        owner,
        entry: if count == 0 { 0 } else { plan.functions },
        interface,
    });
    plan.functions = end;
    Ok(())
}

pub(crate) fn body(
    candidate: &noble_kernel::untrusted::Candidate,
    owner: Option<u32>,
) -> Result<&[noble_kernel::untrusted::NodeId], crate::Diagnostic> {
    match owner {
        None => Ok(&candidate.body),
        Some(owner) => match attempt!(crate::admission::node(
            candidate,
            noble_kernel::untrusted::NodeId(owner)
        )) {
            noble_kernel::untrusted::Node::Quotation { body, .. } => Ok(body),
            _ => Err(crate::Diagnostic::Defective),
        },
    }
}
