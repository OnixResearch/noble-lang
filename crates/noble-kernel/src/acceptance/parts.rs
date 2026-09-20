//! Pure helpers for the acceptance machine: charges, limits, joins, and
//! diagnostics. No helper recurses or mutates its inputs.

pub(crate) mod instantiate;

/// The request and environment a check runs against.
pub(crate) struct Ctx<'a> {
    pub(crate) request: &'a crate::untrusted::Request,
    pub(crate) env: &'a crate::contracts::Env,
}

/// The failing site: a node and, for invocations, its definition.
#[derive(Clone, Copy)]
pub(crate) struct Site {
    pub(crate) node: Option<crate::untrusted::NodeId>,
    pub(crate) def: Option<crate::contracts::Definition>,
}

pub(crate) const fn site(
    node: Option<crate::untrusted::NodeId>,
    def: Option<crate::contracts::Definition>,
) -> Site {
    Site { node, def }
}

/// Charge work, failing closed before the declared limit is exceeded.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; work and cost intentionally use the same work-unit domain, with checked subtraction defining remaining budget and Work exhaustion; wrappers would only restate this scalar contract."
)]
pub(crate) const fn charge(work: u32, cost: u32) -> Result<u32, super::Fail> {
    match work.checked_sub(cost) {
        Some(remaining) => Ok(remaining),
        None => Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work)),
    }
}

pub(crate) fn scheme_cost(scheme: &crate::words::Scheme) -> Result<u32, super::Fail> {
    let count = match u32::try_from(scheme.stack_in.len().saturating_add(scheme.stack_out.len())) {
        Ok(count) => count,
        Err(_) => return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work)),
    };
    Ok(count.saturating_add(1))
}

pub(crate) fn join_cost(interface: &crate::untrusted::Interface) -> Result<u32, super::Fail> {
    let count = match u32::try_from(
        interface
            .stack_in
            .len()
            .saturating_add(interface.stack_out.len()),
    ) {
        Ok(count) => count,
        Err(_) => return Err(super::Fail::Exhausted(crate::untrusted::LimitKind::Work)),
    };
    Ok(count.saturating_add(1))
}

/// Validate one stack against the declared height and type-size limits.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; limits_of rejects unrepresentable or excessive stack height before checking each type size, preserving typed StackHeight/TypeSize exhaustion instead of assertions."
)]
pub(crate) fn limits_of(stack: &[crate::types::Ty], ctx: &Ctx) -> Result<(), super::Fail> {
    let height = match u64::try_from(stack.len()) {
        Ok(height) => height,
        Err(_) => {
            return Err(super::Fail::Exhausted(
                crate::untrusted::LimitKind::StackHeight,
            ))
        }
    };
    if height > u64::from(ctx.request.limits.stack_height) {
        return Err(super::Fail::Exhausted(
            crate::untrusted::LimitKind::StackHeight,
        ));
    }
    match crate::words::bounds::check_sizes(stack, ctx.request.limits.type_size) {
        Ok(()) => Ok(()),
        Err(_) => Err(super::Fail::Exhausted(
            crate::untrusted::LimitKind::TypeSize,
        )),
    }
}

/// The failing definition of a node, when it is an invocation.
pub(crate) fn definition_of(
    candidate: &crate::untrusted::Candidate,
    node: crate::untrusted::NodeId,
) -> Option<crate::contracts::Definition> {
    let index = match usize::try_from(node.0) {
        Ok(index) => index,
        Err(_) => return None,
    };
    match candidate.nodes.get(index) {
        Some(crate::untrusted::Node::Invocation { def, .. }) => Some(*def),
        Some(crate::untrusted::Node::Literal { .. })
        | Some(crate::untrusted::Node::Quotation { .. })
        | None => None,
    }
}

/// Join one interface into a frame's running stack and effect bound.
pub(crate) fn join(
    frame: super::Frame,
    interface: &crate::untrusted::Interface,
    at: Site,
    ctx: &Ctx,
) -> Result<super::Frame, super::Fail> {
    let mut joined = frame;
    if let Some(constraint) = match_tail(&joined.stack, &interface.stack_in) {
        return Err(invalid(
            ctx,
            at,
            interface.stack_in.clone(),
            tail_copy(&joined.stack, interface.stack_in.len()),
            constraint,
        ));
    }
    let keep = joined.stack.len().saturating_sub(interface.stack_in.len());
    joined.stack.truncate(keep);
    joined.stack.extend_from_slice(&interface.stack_out);
    joined.effects = joined.effects.union(&interface.effects);
    attempt!(limits_of(&joined.stack, ctx));
    Ok(joined)
}

/// Whether the stack's top matches the expected segment.
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the length guard proves stack.len() >= expected.len(), and index < expected.len() proves tail_start + index < stack.len(); reassess if either guard changes."
)]
fn match_tail(
    stack: &[crate::types::Ty],
    expected: &[crate::types::Ty],
) -> Option<crate::untrusted::Constraint> {
    if stack.len() < expected.len() {
        return Some(crate::untrusted::Constraint::StackJoin);
    }
    let tail_start = stack.len() - expected.len();
    let mut mismatch: Option<crate::untrusted::Constraint> = None;
    let mut index = 0;
    while index < expected.len() {
        let ty = &expected[index];
        if &stack[tail_start + index] != ty {
            let actual = tail_copy(stack, expected.len());
            mismatch = Some(mismatch_constraint(expected, &actual));
            break;
        }
        index += 1;
    }
    mismatch
}

/// Classify a mismatch as wrong order or wrong shape.
pub(crate) fn mismatch_constraint(
    expected: &[crate::types::Ty],
    actual: &[crate::types::Ty],
) -> crate::untrusted::Constraint {
    if expected.len() == actual.len() && same_multiset(expected, actual) {
        crate::untrusted::Constraint::StackOrder
    } else {
        crate::untrusted::Constraint::StackJoin
    }
}

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; tail_copy subtracts needed only in the branch proving stack.len() >= needed; the other branch copies the entire shorter stack."
)]
pub(crate) fn tail_copy(
    stack: &[crate::types::Ty],
    needed: usize,
) -> alloc::vec::Vec<crate::types::Ty> {
    if stack.len() >= needed {
        stack[stack.len() - needed..].to_vec()
    } else {
        stack.to_vec()
    }
}

fn same_multiset(left: &[crate::types::Ty], right: &[crate::types::Ty]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut used = alloc::vec![false; right.len()];
    let mut is_matched = true;
    let mut item_index = 0;
    while is_matched && item_index < left.len() {
        match find_unused(right, &used, &left[item_index]) {
            Some(index) => {
                used[index] = true;
                item_index += 1;
            }
            None => is_matched = false,
        }
    }
    is_matched
}

/// The first unused index in `right` holding `item`, when one exists.
fn find_unused(
    right: &[crate::types::Ty],
    used: &[bool],
    item: &crate::types::Ty,
) -> Option<usize> {
    let mut found = None;
    let mut index = 0;
    while index < right.len() {
        if !used[index] && &right[index] == item {
            found = Some(index);
            break;
        }
        index += 1;
    }
    found
}

/// The first derived identity outside the allowed bound.
pub(crate) fn first_extra(
    derived: &crate::types::EffSet,
    allowed: &crate::types::EffSet,
) -> Option<crate::types::EffId> {
    let ids = derived.as_slice();
    let mut index = 0;
    let mut extra: Option<crate::types::EffId> = None;
    while index < ids.len() {
        if !allowed.contains(ids[index]) {
            extra = Some(ids[index]);
            break;
        }
        index += 1;
    }
    extra
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; invalid truncates expected then actual entries to the caller's diagnostic budget, including zero, and returns that diagnostic; assertions would turn reporting into another failure path."
)]
pub(crate) fn invalid(
    ctx: &Ctx,
    at: Site,
    expected: alloc::vec::Vec<crate::types::Ty>,
    actual: alloc::vec::Vec<crate::types::Ty>,
    constraint: crate::untrusted::Constraint,
) -> super::Fail {
    let budget = usize::try_from(ctx.request.limits.diagnostics).unwrap_or(0);
    let mut is_truncated = false;
    let mut expected = expected;
    let mut actual = actual;
    if expected.len().saturating_add(actual.len()) > budget {
        is_truncated = true;
        let keep_expected = budget.saturating_sub(actual.len());
        expected.truncate(keep_expected);
        if expected.len().saturating_add(actual.len()) > budget {
            let keep_actual = budget.saturating_sub(expected.len());
            actual.truncate(keep_actual);
        }
    }
    super::Fail::Invalid(crate::untrusted::Diagnostic {
        node: at.node,
        def: at.def,
        expected,
        actual,
        constraint,
        provenance_available: false,
        truncated: is_truncated,
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; first_unknown bounds both scans by their slice lengths and returns the first missing identity or None; absence is an ordinary diagnostic result, not an assertion failure."
)]
pub(crate) fn first_unknown(
    needed: &[crate::types::EffId],
    known: &[crate::types::EffId],
) -> Option<crate::types::EffId> {
    let mut index = 0;
    let mut found: Option<crate::types::EffId> = None;
    while index < needed.len() {
        let id = needed[index];
        let mut is_known = false;
        let mut known_index = 0;
        while known_index < known.len() {
            if known[known_index] == id {
                is_known = true;
                break;
            }
            known_index += 1;
        }
        if !is_known {
            found = Some(id);
            break;
        }
        index += 1;
    }
    found
}
