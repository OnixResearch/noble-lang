#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; inspecting a Program effect bound calls the kernel's non-const EffSet::as_slice accessor; all child counts remain borrowed and explicitly bounded."
)]
fn type_children(ty: &noble_kernel::types::Ty) -> Result<usize, crate::Diagnostic> {
    match ty {
        noble_kernel::types::Ty::Pair(_, _) | noble_kernel::types::Ty::Sum(_, _) => Ok(2),
        noble_kernel::types::Ty::List(_) => Ok(1),
        noble_kernel::types::Ty::Program(input, output, effects) => {
            if input.len() > super::STACK_LIMIT
                || output.len() > super::STACK_LIMIT
                || effects.as_slice().len() > 2
            {
                return Err(crate::Diagnostic::Exhausted);
            }
            Ok(input.len().saturating_add(output.len()))
        }
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Resource(_) => Ok(0),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; selecting a borrowed structural child uses non-const boxed dereferencing and vector slice access without cloning the producer type; invalid paths return diagnostics rather than asserting on input."
)]
fn type_child(
    ty: &noble_kernel::types::Ty,
    child: usize,
) -> Result<&noble_kernel::types::Ty, crate::Diagnostic> {
    match ty {
        noble_kernel::types::Ty::Pair(left, right) | noble_kernel::types::Ty::Sum(left, right) => {
            if child == 0 {
                Ok(left)
            } else {
                Ok(right)
            }
        }
        noble_kernel::types::Ty::List(item) => Ok(item),
        noble_kernel::types::Ty::Program(input, output, _) => {
            let selected = if child < input.len() {
                input.get(child)
            } else {
                output.get(child.saturating_sub(input.len()))
            };
            match selected {
                Some(ty) => Ok(ty),
                None => Err(crate::Diagnostic::Invalid),
            }
        }
        _ => Err(crate::Diagnostic::Invalid),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; only the private bounded traversal constructs these child paths; checked Program lookup and non-container rejection return Invalid without asserting on a borrowed producer tree."
)]
fn type_at<'a>(
    root: &'a noble_kernel::types::Ty,
    path: &[usize],
) -> Result<&'a noble_kernel::types::Ty, crate::Diagnostic> {
    let mut ty = root;
    let mut index = 0usize;
    let mut failure = None;
    while index < path.len() {
        match type_child(ty, path[index]) {
            Ok(child) => {
                ty = child;
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
        None => Ok(ty),
    }
}

fn type_child_count(
    root: &noble_kernel::types::Ty,
    path: &[usize],
    work: &mut super::super::Work,
) -> Result<usize, crate::Diagnostic> {
    attempt!(work.entries(path.len().saturating_add(1)));
    type_children(attempt!(type_at(root, path)))
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the private reserved path requires Vec push/pop for bounded descent and ascent; each climb charges the borrowed ancestor walk before selecting a sibling, and malformed producer trees return the first diagnostic."
)]
fn type_step(
    root: &noble_kernel::types::Ty,
    path: &mut alloc::vec::Vec<usize>,
    work: &mut super::super::Work,
) -> Result<bool, crate::Diagnostic> {
    let children = attempt!(type_child_count(root, path, work));
    if children != 0 {
        if path.len() >= super::PATH_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        path.push(0usize);
        return Ok(true);
    }
    let mut sibling = None;
    let mut failure = None;
    while let Some(child) = path.pop() {
        match type_child_count(root, path, work) {
            Ok(count) => {
                let next_child = child.saturating_add(1);
                if next_child < count {
                    sibling = Some(next_child);
                    break;
                }
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => match sibling {
            Some(child) => {
                path.push(child);
                Ok(true)
            }
            None => Ok(false),
        },
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the path-only worklist checks both node and depth limits and charges every root-to-child walk before traversal; exhaustion is an input diagnostic and no recursive clone or assertion is needed."
)]
pub(super) fn ty(
    root: &noble_kernel::types::Ty,
    max_node_count: u32,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(work.charge(1));
    let mut path = alloc::vec::Vec::with_capacity(super::PATH_LIMIT);
    let mut nodes = 0u32;
    let mut failure = None;
    loop {
        if nodes >= max_node_count.min(super::TYPE_LIMIT) {
            failure = Some(crate::Diagnostic::Exhausted);
            break;
        }
        nodes += 1;
        match type_step(root, &mut path, work) {
            Ok(true) => {}
            Ok(false) => break,
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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the borrowed stack bound and each metered type walk fail through diagnostics before owned cloning; the first failure stops the bounded scan without input assertions."
)]
pub(super) fn stack(
    stack: &[noble_kernel::types::Ty],
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let max_entry_count = match usize::try_from(limits.stack_height) {
        Ok(value) => value,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    if stack.len() > max_entry_count.min(super::STACK_LIMIT) {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut index = 0usize;
    let mut failure = None;
    while index < stack.len() {
        match ty(&stack[index], limits.type_size, work) {
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
