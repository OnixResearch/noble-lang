#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every borrowed type reserves its bounded structural walk before inspection and refunds only unused work; first-failure metering returns Exhausted rather than asserting on interface input."
)]
pub(in crate::source) fn stack(
    stack: &[noble_kernel::types::Ty],
    work: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let mut bytes = 2u64;
    attempt!(work.entries(stack.len()));
    let mut index = 0usize;
    let mut failure = None;
    while index < stack.len() {
        match type_bytes(&stack[index], work) {
            Ok(amount) => {
                bytes = bytes.saturating_add(amount);
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(bytes),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; Ty::size performs the kernel's non-const structural traversal only after its full allowance is reserved; the unused portion is refunded without changing the caller's original charge sequence."
)]
fn type_bytes(
    ty: &noble_kernel::types::Ty,
    work: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    // Reserve the bounded structural walk before entering it, then return
    // unused allowance. Accepted types have at most 512 structural nodes.
    attempt!(work.charge(2048));
    let node_count = match ty.size() {
        Some(node_count) if node_count <= 512 => u64::from(node_count),
        Some(_) | None => return Err(crate::Diagnostic::Exhausted),
    };
    work.remaining = work
        .remaining
        .saturating_add(2048u64.saturating_sub(node_count.saturating_mul(4)));
    Ok(node_count.saturating_mul(32).saturating_add(1))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; checked derivation scans are reserved before complete interface traversal, then each stack uses the same bounded meter; exhaustion stops at the first failed charge without assertions or resetting allowance."
)]
fn checked_work(
    checked: &noble_kernel::untrusted::Checked,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let count = match u64::try_from(checked.derivations.len()) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    // Derivation lookup, quotation ordering and type-registry comparisons are
    // finite scans, not hidden unmetered work per executable occurrence.
    attempt!(work.charge(count.saturating_mul(count).saturating_mul(8)));
    attempt!(interface_work(&checked.interface, work));
    let mut index = 0usize;
    let mut failure = None;
    while index < checked.derivations.len() {
        match interface_work(&checked.derivations[index].interface, work) {
            Ok(()) => index += 1,
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

fn interface_work(
    interface: &noble_kernel::untrusted::Interface,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(stack(&interface.stack_in, work));
    attempt!(stack(&interface.stack_out, work));
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every actual retained signature key and accepted interface is charged in order before lowering; first-failure loops retain exhaustion diagnostics without assertions or hypothetical duplicate-pool costs."
)]
pub(in crate::source) fn compilation(
    checked: &super::Accepted,
    pool: &crate::signatures::Pool,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < pool.keys.len() {
        match work.entries(pool.keys[index].len()) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    index = 0;
    while index < checked.definitions.len() {
        match checked_work(&checked.definitions[index], work) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    checked_work(&checked.root, work)
}
