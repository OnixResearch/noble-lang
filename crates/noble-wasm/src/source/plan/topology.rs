#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; lowering mutates only the unpublished prospective compiler, preparation-owned meter and fresh plan buffers; caller-owned submissions and checked interfaces remain immutable."
)]

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; lowering creates all quotation child indices in this owned program table; each operation checks its leaf increment before readiness, and only ready children contribute checked depth and leaf counts."
)]
const fn extend(
    programs: &[super::Program],
    done: &[bool],
    operation: &super::Operation,
    depth: &mut u32,
    leaves: &mut u32,
) -> Result<bool, crate::Diagnostic> {
    *leaves = match leaves.checked_add(1) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    if let super::Action::Program(child) = operation.action {
        if !done[child] {
            return Ok(false);
        }
        let child_depth = match programs[child].depth.checked_add(1) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        if *depth < child_depth {
            *depth = child_depth;
        }
        *leaves = match leaves.checked_add(programs[child].leaves) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
    }
    Ok(true)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the owned done table and program indices are aligned by order, compilation reserves this bounded scan, and only a fully ready operation sequence publishes checked depth and leaf counts."
)]
fn program(
    plan: &mut super::Layout,
    done: &mut [bool],
    index: usize,
) -> Result<(), crate::Diagnostic> {
    if done[index] {
        return Ok(());
    }
    let mut is_ready = true;
    let mut depth = 1u32;
    let mut leaves = 0u32;
    let mut at = 0usize;
    let mut failure = None;
    while at < plan.programs[index].operations.len() {
        match extend(
            &plan.programs,
            done,
            &plan.programs[index].operations[at],
            &mut depth,
            &mut leaves,
        ) {
            Ok(true) => at += 1,
            Ok(false) => {
                is_ready = false;
                break;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => {
            if is_ready {
                plan.programs[index].depth = depth;
                plan.programs[index].leaves = leaves;
                done[index] = true;
                plan.order.push(index);
            }
            Ok(())
        }
    }
}

fn round(plan: &mut super::Layout, done: &mut [bool]) -> Result<(), crate::Diagnostic> {
    let before = plan.order.len();
    let mut index = 0usize;
    let mut failure = None;
    while index < plan.programs.len() {
        match program(plan, done, index) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None if plan.order.len() == before => Err(crate::Diagnostic::Invalid),
        None => Ok(()),
    }
}

pub(super) fn order(plan: &mut super::Layout) -> Result<(), crate::Diagnostic> {
    plan.order.reserve(plan.programs.len());
    let mut done = alloc::vec![false; plan.programs.len()];
    let mut failure = None;
    while plan.order.len() < plan.programs.len() {
        match round(plan, &mut done) {
            Ok(()) => {}
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}
