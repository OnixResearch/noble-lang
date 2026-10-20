//! Calculator-only borrowed structural bounds before kernel-owned scheme clones.
//!
//! Every embedded nominal descriptor is charged at each occurrence, and
//! deeper ancestors are charged repeatedly for the kernel's owned marker,
//! substitution and equality walks. The ordinary/cursor admission meter is
//! unchanged. All arithmetic fails closed before a producer-owned tree is
//! cloned by kernel validation.

fn add(left: u64, right: u64) -> Result<u64, crate::Diagnostic> {
    left.checked_add(right).ok_or(crate::Diagnostic::Exhausted)
}

fn step(depth: u64) -> Result<u64, crate::Diagnostic> {
    let next = attempt!(add(depth, 1));
    if next > 64 {
        Err(crate::Diagnostic::Exhausted)
    } else {
        Ok(next)
    }
}

fn shape(
    shape: &noble_kernel::types::NominalShape,
    depth: u64,
    lookups: &mut u64,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    match shape {
        noble_kernel::types::NominalShape::Opaque(value) => ty(value, depth, lookups, work),
        noble_kernel::types::NominalShape::Variant(left, right) => {
            let left = attempt!(ty(left, depth, lookups, work));
            add(left, attempt!(ty(right, depth, lookups, work)))
        }
    }
}

fn types(
    values: &[noble_kernel::types::Ty],
    depth: u64,
    lookups: &mut u64,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let mut total = 0;
    for value in values {
        total = attempt!(add(total, attempt!(ty(value, depth, lookups, work))));
    }
    Ok(total)
}

fn ty(
    value: &noble_kernel::types::Ty,
    depth: u64,
    lookups: &mut u64,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    attempt!(work.charge(1));
    let next = attempt!(step(depth));
    let children = match value {
        noble_kernel::types::Ty::Pair(left, right)
        | noble_kernel::types::Ty::Sum(left, right) => {
            let left = attempt!(ty(left, next, lookups, work));
            attempt!(add(left, attempt!(ty(right, next, lookups, work))))
        }
        noble_kernel::types::Ty::List(item) => attempt!(ty(item, next, lookups, work)),
        noble_kernel::types::Ty::Program(input, output, effects) => {
            attempt!(work.entries(effects.as_slice().len()));
            let input = attempt!(types(input, next, lookups, work));
            let stacks = attempt!(add(input, attempt!(types(output, next, lookups, work))));
            attempt!(add(stacks, effects.as_slice().len() as u64))
        }
        noble_kernel::types::Ty::Nominal(_, descriptor) => {
            *lookups = attempt!(add(*lookups, 1));
            attempt!(shape(descriptor, next, lookups, work))
        }
        noble_kernel::types::Ty::GenericNominal(_, arguments, descriptor) => {
            *lookups = attempt!(add(*lookups, 1));
            let first = attempt!(ty(&arguments[0], next, lookups, work));
            let args = attempt!(add(first, attempt!(ty(&arguments[1], next, lookups, work))));
            attempt!(add(args, attempt!(shape(descriptor, next, lookups, work))))
        }
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Resource(_) => 0,
    };
    add(next, children)
}

fn patterns(
    values: &[noble_kernel::shapes::Pattern],
    depth: u64,
    lookups: &mut u64,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let mut total = 0;
    for value in values {
        total = attempt!(add(total, attempt!(pattern(value, depth, lookups, work))));
    }
    Ok(total)
}

fn pattern(
    value: &noble_kernel::shapes::Pattern,
    depth: u64,
    lookups: &mut u64,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    attempt!(work.charge(1));
    let next = attempt!(step(depth));
    let children = match value {
        noble_kernel::shapes::Pattern::Pair(left, right)
        | noble_kernel::shapes::Pattern::Sum(left, right) => {
            let left = attempt!(pattern(left, next, lookups, work));
            attempt!(add(left, attempt!(pattern(right, next, lookups, work))))
        }
        noble_kernel::shapes::Pattern::List(item) => {
            attempt!(pattern(item, next, lookups, work))
        }
        noble_kernel::shapes::Pattern::Program(input, output, effects) => {
            attempt!(work.entries(effects.len()));
            let input = attempt!(patterns(input, next, lookups, work));
            let stacks = attempt!(add(input, attempt!(patterns(output, next, lookups, work))));
            attempt!(add(stacks, effects.len() as u64))
        }
        noble_kernel::shapes::Pattern::Nominal(_, descriptor) => {
            *lookups = attempt!(add(*lookups, 1));
            attempt!(shape(descriptor, next, lookups, work))
        }
        noble_kernel::shapes::Pattern::GenericNominal(_, arguments, _) => {
            *lookups = attempt!(add(*lookups, 1));
            let first = attempt!(pattern(&arguments[0], next, lookups, work));
            attempt!(add(first, attempt!(pattern(&arguments[1], next, lookups, work))))
        }
        noble_kernel::shapes::Pattern::Unit
        | noble_kernel::shapes::Pattern::Bool
        | noble_kernel::shapes::Pattern::I64
        | noble_kernel::shapes::Pattern::Text
        | noble_kernel::shapes::Pattern::Syntax
        | noble_kernel::shapes::Pattern::Contract
        | noble_kernel::shapes::Pattern::Evidence
        | noble_kernel::shapes::Pattern::Certified
        | noble_kernel::shapes::Pattern::Resource(_)
        | noble_kernel::shapes::Pattern::StackVar(_) => 0,
        // Kernel substitution clones the bound value once per occurrence
        // before checking the expanded interface. Borrowed preflight caps a
        // concrete binding at 512 Ty constructors, not the total fan-out.
        noble_kernel::shapes::Pattern::Var(_) => 512,
    };
    add(next, children)
}

fn root(
    value: &noble_kernel::shapes::Pattern,
    lookup_rows: usize,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let mut lookups = 0;
    let weighted = attempt!(pattern(value, 0, &mut lookups, work));
    // The same accepted scheme is validated, copied into an owned kernel
    // environment, compared against nominal descriptors, and substituted.
    // Each pass may copy ancestor subtrees; W counts their descendant sizes.
    let scans = attempt!(weighted.checked_mul(16).ok_or(crate::Diagnostic::Exhausted));
    let rows = attempt!(u64::try_from(lookup_rows)
        .map_err(|_| crate::Diagnostic::Exhausted));
    let lookup_cost = attempt!(lookups
        .checked_mul(rows)
        .and_then(|count| count.checked_mul(2))
        .ok_or(crate::Diagnostic::Exhausted));
    Ok(attempt!(add(scans, lookup_cost)).max(512))
}

pub(super) fn scheme(
    scheme: &noble_kernel::words::Scheme,
    lookup_rows: usize,
    work: &mut super::super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    if scheme.stack_in.len() > super::super::super::NODE_LIMIT
        || scheme.stack_out.len() > super::super::super::NODE_LIMIT
        || scheme.var_kinds.len() > super::super::super::NODE_LIMIT
        || scheme.effects.len() > super::super::super::NODE_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    let scalar = attempt!(u64::try_from(scheme.var_kinds.len()
        .saturating_add(scheme.effects.len()).saturating_add(1))
        .map_err(|_| crate::Diagnostic::Exhausted));
    attempt!(work.entries(scheme.var_kinds.len().saturating_add(scheme.effects.len())));
    let mut cost = attempt!(scalar.checked_mul(512).ok_or(crate::Diagnostic::Exhausted));
    for value in &scheme.stack_in {
        cost = attempt!(add(cost, attempt!(root(value, lookup_rows, work))));
    }
    for value in &scheme.stack_out {
        cost = attempt!(add(cost, attempt!(root(value, lookup_rows, work))));
    }
    Ok(cost)
}
