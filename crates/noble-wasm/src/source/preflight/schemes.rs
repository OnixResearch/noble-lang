#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

const fn pattern_children(
    pattern: &noble_kernel::shapes::Pattern,
) -> Result<usize, crate::Diagnostic> {
    match pattern {
        noble_kernel::shapes::Pattern::Pair(_, _) | noble_kernel::shapes::Pattern::Sum(_, _) => {
            Ok(2)
        }
        noble_kernel::shapes::Pattern::List(_) => Ok(1),
        noble_kernel::shapes::Pattern::Program(input, output, effects) => {
            if input.len() > super::STACK_LIMIT
                || output.len() > super::STACK_LIMIT
                || effects.len() > 8
            {
                return Err(crate::Diagnostic::Exhausted);
            }
            Ok(input.len().saturating_add(output.len()))
        }
        noble_kernel::shapes::Pattern::Unit
        | noble_kernel::shapes::Pattern::Bool
        | noble_kernel::shapes::Pattern::I64
        | noble_kernel::shapes::Pattern::Text
        | noble_kernel::shapes::Pattern::Syntax
        | noble_kernel::shapes::Pattern::Resource(_)
        | noble_kernel::shapes::Pattern::Var(_)
        | noble_kernel::shapes::Pattern::StackVar(_) => Ok(0),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; selecting a borrowed structural child uses non-const boxed dereferencing and vector slice access without cloning the producer pattern; invalid paths return diagnostics rather than asserting on input."
)]
fn pattern_child(
    pattern: &noble_kernel::shapes::Pattern,
    child: usize,
) -> Result<&noble_kernel::shapes::Pattern, crate::Diagnostic> {
    match pattern {
        noble_kernel::shapes::Pattern::Pair(left, right)
        | noble_kernel::shapes::Pattern::Sum(left, right) => {
            if child == 0 {
                Ok(left)
            } else {
                Ok(right)
            }
        }
        noble_kernel::shapes::Pattern::List(item) => Ok(item),
        noble_kernel::shapes::Pattern::Program(input, output, _) => {
            let selected = if child < input.len() {
                input.get(child)
            } else {
                output.get(child.saturating_sub(input.len()))
            };
            match selected {
                Some(pattern) => Ok(pattern),
                None => Err(crate::Diagnostic::Invalid),
            }
        }
        _ => Err(crate::Diagnostic::Invalid),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; only the private bounded traversal constructs these child paths; checked Program lookup and non-container rejection return Invalid without asserting on a borrowed producer pattern."
)]
fn pattern_at<'a>(
    root: &'a noble_kernel::shapes::Pattern,
    path: &[usize],
) -> Result<&'a noble_kernel::shapes::Pattern, crate::Diagnostic> {
    let mut pattern = root;
    let mut index = 0usize;
    let mut failure = None;
    while index < path.len() {
        match pattern_child(pattern, path[index]) {
            Ok(child) => {
                pattern = child;
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
        None => Ok(pattern),
    }
}

fn pattern_child_count(
    root: &noble_kernel::shapes::Pattern,
    path: &[usize],
    work: &mut super::super::Work,
) -> Result<usize, crate::Diagnostic> {
    attempt!(work.entries(path.len().saturating_add(1)));
    pattern_children(attempt!(pattern_at(root, path)))
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the private reserved path requires Vec push/pop for bounded descent and ascent; each climb charges the borrowed ancestor walk before selecting a sibling, and malformed producer patterns return the first diagnostic."
)]
fn pattern_step(
    root: &noble_kernel::shapes::Pattern,
    path: &mut alloc::vec::Vec<usize>,
    work: &mut super::super::Work,
) -> Result<bool, crate::Diagnostic> {
    let children = attempt!(pattern_child_count(root, path, work));
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
        match pattern_child_count(root, path, work) {
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
    reason = "Owner: noble-maintainers; the path-only worklist checks both node and depth limits and charges every root-to-child walk before traversal; oversized patterns return Exhausted before the kernel's owned clone boundary."
)]
fn pattern(
    root: &noble_kernel::shapes::Pattern,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(work.charge(1));
    let mut path = alloc::vec::Vec::with_capacity(super::PATH_LIMIT);
    let mut nodes = 0u32;
    let mut failure = None;
    loop {
        if nodes >= super::TYPE_LIMIT {
            failure = Some(crate::Diagnostic::Exhausted);
            break;
        }
        nodes += 1;
        match pattern_step(root, &mut path, work) {
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
    reason = "Owner: noble-maintainers; scheme arities and every borrowed input/output pattern are bounded before kernel substitution; invalid resource demands exhaust the shared meter instead of becoming assertion failures."
)]
pub(super) fn check(
    scheme: &noble_kernel::words::Scheme,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    if scheme.var_kinds.len() > 8
        || scheme.stack_in.len() > super::STACK_LIMIT
        || scheme.stack_out.len() > super::STACK_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    if scheme.effects.len() > 8 {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut index = 0usize;
    let mut failure = None;
    while index < scheme.stack_in.len() {
        match pattern(&scheme.stack_in[index], work) {
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
    while index < scheme.stack_out.len() {
        match pattern(&scheme.stack_out[index], work) {
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
