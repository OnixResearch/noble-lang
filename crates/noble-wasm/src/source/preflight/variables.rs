//! Calculator-only borrowed pricing of the kernel's per-appearance owned nodes.
//!
//! The kernel copies a node every time a body names its NodeId, then copies
//! its witness again while validating and substituting the selected scheme.
//! Pricing just the unique arena nodes or environment rows misses this work.

fn add(left: u64, right: u64) -> Result<u64, crate::Diagnostic> {
    left.checked_add(right).ok_or(crate::Diagnostic::Exhausted)
}

fn mul(left: u64, right: u64) -> Result<u64, crate::Diagnostic> {
    left.checked_mul(right).ok_or(crate::Diagnostic::Exhausted)
}

#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the walk follows the kernel's checked node, type or pattern tree, and an explicit worklist would change the metered traversal order."
)]
/// A second generic constructor in one substituted path duplicates its
/// already-materialized argument twice again. Reject the exponential chain
/// while it is still a borrowed pattern, including in unselected env rows.
fn finite_pattern(
    value: &noble_kernel::shapes::Pattern,
    depth: usize,
    generic: bool,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(work.charge(1));
    if depth > super::PATH_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    match value {
        noble_kernel::shapes::Pattern::GenericNominal(_, arguments, _) => {
            if generic {
                return Err(crate::Diagnostic::Exhausted);
            }
            attempt!(finite_pattern(&arguments[0], depth + 1, true, work));
            finite_pattern(&arguments[1], depth + 1, true, work)
        }
        noble_kernel::shapes::Pattern::Pair(left, right)
        | noble_kernel::shapes::Pattern::Sum(left, right) => {
            attempt!(finite_pattern(left, depth + 1, generic, work));
            finite_pattern(right, depth + 1, generic, work)
        }
        noble_kernel::shapes::Pattern::List(item) => finite_pattern(item, depth + 1, generic, work),
        noble_kernel::shapes::Pattern::Program(input, output, _)
        | noble_kernel::shapes::Pattern::LiveRef(input, output, _) => {
            for item in input.iter().chain(output.iter()) {
                attempt!(finite_pattern(item, depth + 1, generic, work));
            }
            Ok(())
        }
        noble_kernel::shapes::Pattern::Nominal(_, _)
        | noble_kernel::shapes::Pattern::Var(_)
        | noble_kernel::shapes::Pattern::StackVar(_)
        | noble_kernel::shapes::Pattern::Unit
        | noble_kernel::shapes::Pattern::Bool
        | noble_kernel::shapes::Pattern::I64
        | noble_kernel::shapes::Pattern::Text
        | noble_kernel::shapes::Pattern::Syntax
        | noble_kernel::shapes::Pattern::Contract
        | noble_kernel::shapes::Pattern::Evidence
        | noble_kernel::shapes::Pattern::Certified
        | noble_kernel::shapes::Pattern::Resource(_) => Ok(()),
    }
}

pub(super) fn schemes(
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    for scheme in &submission.environment.defs {
        for item in scheme.stack_in.iter().chain(scheme.stack_out.iter()) {
            attempt!(finite_pattern(item, 0, false, work));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the walk follows the kernel's checked node, type or pattern tree, and an explicit worklist would change the metered traversal order."
)]
fn pattern(
    value: &noble_kernel::shapes::Pattern,
    sizes: &[u64; 8],
    depth: u64,
    lookup_rows: u64,
    work: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    attempt!(work.charge(1));
    let next = attempt!(add(depth, 1));
    if next > super::PATH_LIMIT as u64 {
        return Err(crate::Diagnostic::Exhausted);
    }
    match value {
        noble_kernel::shapes::Pattern::Var(var) | noble_kernel::shapes::Pattern::StackVar(var) => {
            let index = attempt!(usize::try_from(var.0).map_err(|_| crate::Diagnostic::Invalid));
            match sizes.get(index) {
                Some(size) => mul(*size, attempt!(mul(next, 16))),
                None => Err(crate::Diagnostic::Invalid),
            }
        }
        noble_kernel::shapes::Pattern::Pair(left, right)
        | noble_kernel::shapes::Pattern::Sum(left, right) => {
            let first = attempt!(pattern(left, sizes, next, lookup_rows, work));
            add(
                attempt!(add(
                    first,
                    attempt!(pattern(right, sizes, next, lookup_rows, work))
                )),
                attempt!(mul(next, 16)),
            )
        }
        noble_kernel::shapes::Pattern::List(item) => add(
            attempt!(pattern(item, sizes, next, lookup_rows, work)),
            attempt!(mul(next, 16)),
        ),
        noble_kernel::shapes::Pattern::Program(input, output, effects)
        | noble_kernel::shapes::Pattern::LiveRef(input, output, effects) => {
            attempt!(work.entries(effects.len()));
            let mut total = attempt!(mul(
                attempt!(add(effects.len() as u64, 1)),
                attempt!(mul(next, 16))
            ));
            for item in input.iter().chain(output.iter()) {
                total = attempt!(add(
                    total,
                    attempt!(pattern(item, sizes, next, lookup_rows, work))
                ));
            }
            Ok(total)
        }
        noble_kernel::shapes::Pattern::GenericNominal(_, arguments, _) => {
            let first = attempt!(pattern(&arguments[0], sizes, next, lookup_rows, work));
            let children = attempt!(add(
                first,
                attempt!(pattern(&arguments[1], sizes, next, lookup_rows, work))
            ));
            let constructor =
                attempt!(add(attempt!(mul(next, 16)), attempt!(mul(lookup_rows, 16))));
            add(attempt!(mul(children, 4)), constructor)
        }
        noble_kernel::shapes::Pattern::Nominal(_, shape) => {
            let mut total = attempt!(mul(next, 16));
            match shape.as_ref() {
                noble_kernel::types::NominalShape::Opaque(ty) => {
                    let (_, weighted, lookups) = attempt!(super::concrete::count(ty, 512, work));
                    total = attempt!(add(total, attempt!(mul(weighted, 16))));
                    total = attempt!(add(
                        total,
                        attempt!(mul(lookups, attempt!(mul(lookup_rows, 16))))
                    ));
                }
                noble_kernel::types::NominalShape::Variant(left, right) => {
                    for ty in [left, right] {
                        let (_, weighted, lookups) =
                            attempt!(super::concrete::count(ty, 512, work));
                        total = attempt!(add(total, attempt!(mul(weighted, 16))));
                        total = attempt!(add(
                            total,
                            attempt!(mul(lookups, attempt!(mul(lookup_rows, 16))))
                        ));
                    }
                }
            }
            add(total, attempt!(mul(lookup_rows, 16)))
        }
        noble_kernel::shapes::Pattern::Unit
        | noble_kernel::shapes::Pattern::Bool
        | noble_kernel::shapes::Pattern::I64
        | noble_kernel::shapes::Pattern::Text
        | noble_kernel::shapes::Pattern::Syntax
        | noble_kernel::shapes::Pattern::Contract
        | noble_kernel::shapes::Pattern::Evidence
        | noble_kernel::shapes::Pattern::Certified
        | noble_kernel::shapes::Pattern::Resource(_) => mul(next, 16),
    }
}

#[derive(Clone, Copy, Default)]
struct Volume {
    height: u64,
    nodes: u64,
    weighted: u64,
    lookups: u64,
}

impl Volume {
    fn add(self, other: Self) -> Result<Self, crate::Diagnostic> {
        Ok(Self {
            height: attempt!(add(self.height, other.height)),
            nodes: attempt!(add(self.nodes, other.nodes)),
            weighted: attempt!(add(self.weighted, other.weighted)),
            lookups: attempt!(add(self.lookups, other.lookups)),
        })
    }

    fn cost(self, lookup_rows: u64) -> Result<u64, crate::Diagnostic> {
        add(
            self.weighted,
            attempt!(mul(self.lookups, attempt!(mul(lookup_rows, 16)))),
        )
    }

    fn constructed(self, copies: u64, nominal: bool) -> Result<Self, crate::Diagnostic> {
        Ok(Self {
            height: 1,
            nodes: attempt!(add(1, attempt!(mul(self.nodes, copies)))),
            weighted: attempt!(add(
                1,
                attempt!(mul(attempt!(add(self.weighted, self.nodes)), copies))
            )),
            lookups: attempt!(add(u64::from(nominal), attempt!(mul(self.lookups, copies)))),
        })
    }
}

fn type_volume(
    ty: &noble_kernel::types::Ty,
    work: &mut super::super::Work,
) -> Result<Volume, crate::Diagnostic> {
    let (nodes, weighted, lookups) = attempt!(super::concrete::count(ty, 512, work));
    Ok(Volume {
        height: 1,
        nodes,
        weighted,
        lookups,
    })
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the walk follows the kernel's checked node, type or pattern tree, and an explicit worklist would change the metered traversal order."
)]
/// Project a rank-1 pattern's concrete stack volume without materializing
/// any substituted Ty. One generic family copies each resolved argument into
/// both the arguments and descriptor; nested families are rejected above.
fn projected(
    value: &noble_kernel::shapes::Pattern,
    bindings: &[Volume; 8],
    depth: usize,
    work: &mut super::super::Work,
) -> Result<Volume, crate::Diagnostic> {
    attempt!(work.charge(1));
    if depth > super::PATH_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    let next = depth + 1;
    match value {
        noble_kernel::shapes::Pattern::Var(var) | noble_kernel::shapes::Pattern::StackVar(var) => {
            let index = attempt!(usize::try_from(var.0).map_err(|_| crate::Diagnostic::Invalid));
            bindings
                .get(index)
                .copied()
                .ok_or(crate::Diagnostic::Invalid)
        }
        noble_kernel::shapes::Pattern::Pair(left, right)
        | noble_kernel::shapes::Pattern::Sum(left, right) => {
            let left = attempt!(projected(left, bindings, next, work));
            let right = attempt!(projected(right, bindings, next, work));
            if left.height != 1 || right.height != 1 {
                return Err(crate::Diagnostic::Invalid);
            }
            attempt!(left.add(right)).constructed(1, false)
        }
        noble_kernel::shapes::Pattern::List(item) => {
            let item = attempt!(projected(item, bindings, next, work));
            if item.height != 1 {
                return Err(crate::Diagnostic::Invalid);
            }
            item.constructed(1, false)
        }
        noble_kernel::shapes::Pattern::Program(input, output, effects)
        | noble_kernel::shapes::Pattern::LiveRef(input, output, effects) => {
            attempt!(work.entries(effects.len()));
            let input = attempt!(projected_stack(input, bindings, next, work));
            let output = attempt!(projected_stack(output, bindings, next, work));
            attempt!(input.add(output)).constructed(1, false)
        }
        noble_kernel::shapes::Pattern::Nominal(_, descriptor) => {
            let contents = match descriptor.as_ref() {
                noble_kernel::types::NominalShape::Opaque(ty) => attempt!(type_volume(ty, work)),
                noble_kernel::types::NominalShape::Variant(left, right) => {
                    let left = attempt!(type_volume(left, work));
                    attempt!(left.add(attempt!(type_volume(right, work))))
                }
            };
            contents.constructed(1, true)
        }
        noble_kernel::shapes::Pattern::GenericNominal(_, arguments, _) => {
            let first = attempt!(projected(&arguments[0], bindings, next, work));
            let second = attempt!(projected(&arguments[1], bindings, next, work));
            if first.height != 1 || second.height != 1 {
                return Err(crate::Diagnostic::Invalid);
            }
            attempt!(first.add(second)).constructed(2, true)
        }
        noble_kernel::shapes::Pattern::Unit
        | noble_kernel::shapes::Pattern::Bool
        | noble_kernel::shapes::Pattern::I64
        | noble_kernel::shapes::Pattern::Text
        | noble_kernel::shapes::Pattern::Syntax
        | noble_kernel::shapes::Pattern::Contract
        | noble_kernel::shapes::Pattern::Evidence
        | noble_kernel::shapes::Pattern::Certified
        | noble_kernel::shapes::Pattern::Resource(_) => Ok(Volume {
            height: 1,
            nodes: 1,
            weighted: 1,
            lookups: 0,
        }),
    }
}

fn projected_stack(
    patterns: &[noble_kernel::shapes::Pattern],
    bindings: &[Volume; 8],
    depth: usize,
    work: &mut super::super::Work,
) -> Result<Volume, crate::Diagnostic> {
    let mut result = Volume::default();
    for pattern in patterns {
        result = attempt!(result.add(attempt!(projected(pattern, bindings, depth, work))));
    }
    Ok(result)
}

fn witness(
    inst: &noble_kernel::words::Inst,
    lookup_rows: u64,
    work: &mut super::super::Work,
) -> Result<([u64; 8], [Volume; 8], u64), crate::Diagnostic> {
    let mut sizes = [0u64; 8];
    let mut volumes = [Volume::default(); 8];
    if inst.bindings.len() > sizes.len() {
        return Err(crate::Diagnostic::Exhausted);
    }
    for (index, binding) in inst.bindings.iter().enumerate() {
        volumes[index] = attempt!(match binding {
            noble_kernel::words::Binding::Value(ty) => type_volume(ty, work),
            noble_kernel::words::Binding::Stack(stack) => {
                let mut total = Volume::default();
                for ty in stack {
                    total = attempt!(total.add(attempt!(type_volume(ty, work))));
                }
                Ok(total)
            }
            noble_kernel::words::Binding::Effect(effects) => Ok(Volume {
                height: 0,
                nodes: 0,
                weighted: effects.as_slice().len() as u64,
                lookups: 0,
            }),
            noble_kernel::words::Binding::Ref(_) => Ok(Volume::default()),
        });
        sizes[index] = attempt!(volumes[index].cost(lookup_rows));
    }
    let mut total = 1u64;
    for index in 0..inst.bindings.len() {
        if matches!(inst.bindings[index], noble_kernel::words::Binding::Ref(_)) {
            let mut current = index;
            let mut hops = 0;
            loop {
                attempt!(work.charge(1));
                if hops >= inst.bindings.len() {
                    return Err(crate::Diagnostic::Invalid);
                }
                match &inst.bindings[current] {
                    noble_kernel::words::Binding::Ref(var) => {
                        current = attempt!(
                            usize::try_from(var.0).map_err(|_| crate::Diagnostic::Invalid)
                        );
                        if current >= inst.bindings.len() {
                            return Err(crate::Diagnostic::Invalid);
                        }
                        hops += 1;
                    }
                    _ => break,
                }
            }
            volumes[index] = volumes[current];
            sizes[index] = sizes[current];
        }
        total = attempt!(add(total, sizes[index]));
    }
    Ok((sizes, volumes, total))
}

struct Cost {
    clone: u64,
    input: u64,
    output: u64,
    quotation_input: u64,
    quotation_output: u64,
    input_height: u64,
    output_height: u64,
    quotation_input_height: u64,
    quotation_output_height: u64,
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
fn node(
    node: &noble_kernel::untrusted::Node,
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<Cost, crate::Diagnostic> {
    let inst = match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. }
        | noble_kernel::untrusted::Node::Quotation { inst, .. } => inst,
        // The kernel refuses borrowed dispatch outside the live-slot profile.
        noble_kernel::untrusted::Node::SlotInvoke { .. } => {
            return Err(crate::Diagnostic::Unsupported)
        }
    };
    let lookup_rows = attempt!(u64::try_from(
        submission
            .environment
            .nominals
            .len()
            .saturating_add(submission.environment.generic_variants.len())
    )
    .map_err(|_| crate::Diagnostic::Exhausted));
    let (sizes, volumes, witness_size) = attempt!(witness(inst, lookup_rows, work));
    let mut cost = attempt!(mul(witness_size, 16));
    let input;
    let output;
    let quotation_input;
    let quotation_output;
    let input_height;
    let output_height;
    let quotation_input_height;
    let quotation_output_height;
    match node {
        noble_kernel::untrusted::Node::Literal { .. } => {
            cost = attempt!(add(cost, attempt!(mul(sizes[0], 32))));
            input = sizes[0];
            output = attempt!(add(input, 1));
            quotation_input = 0;
            quotation_output = 0;
            input_height = volumes[0].height;
            output_height = attempt!(add(input_height, 1));
            quotation_input_height = 0;
            quotation_output_height = 0;
        }
        noble_kernel::untrusted::Node::Quotation { body, .. } => {
            cost = attempt!(add(cost, attempt!(mul(body.len() as u64, 16))));
            let stack = attempt!(add(
                attempt!(mul(sizes[0], 2)),
                attempt!(add(sizes[1], sizes[2]))
            ));
            cost = attempt!(add(cost, attempt!(mul(stack, 16))));
            let constructed = attempt!(volumes[1].add(volumes[2])).constructed(1, false);
            input = sizes[0];
            output = attempt!(add(
                input,
                attempt!(attempt!(constructed).cost(lookup_rows))
            ));
            quotation_input = sizes[1];
            quotation_output = sizes[2];
            input_height = volumes[0].height;
            output_height = attempt!(add(input_height, 1));
            quotation_input_height = volumes[1].height;
            quotation_output_height = volumes[2].height;
        }
        noble_kernel::untrusted::Node::Invocation { def, .. } => {
            let scheme = attempt!(submission
                .environment
                .scheme(*def)
                .ok_or(crate::Diagnostic::Invalid));
            cost = attempt!(add(
                cost,
                attempt!(super::super::admission::witness_scheme(
                    submission, *def, work
                ))
            ));
            for value in scheme.stack_in.iter().chain(scheme.stack_out.iter()) {
                cost = attempt!(add(
                    cost,
                    attempt!(pattern(value, &sizes, 0, lookup_rows, work))
                ));
            }
            let projected_input = attempt!(projected_stack(&scheme.stack_in, &volumes, 0, work));
            let projected_output = attempt!(projected_stack(&scheme.stack_out, &volumes, 0, work));
            input = attempt!(projected_input.cost(lookup_rows));
            output = attempt!(projected_output.cost(lookup_rows));
            quotation_input = 0;
            quotation_output = 0;
            input_height = projected_input.height;
            output_height = projected_output.height;
            quotation_input_height = 0;
            quotation_output_height = 0;
        }
        noble_kernel::untrusted::Node::SlotInvoke { .. } => {
            return Err(crate::Diagnostic::Unsupported)
        }
    }
    Ok(Cost {
        clone: cost,
        input,
        output,
        quotation_input,
        quotation_output,
        input_height,
        output_height,
        quotation_input_height,
        quotation_output_height,
    })
}

/// The immutable inputs shared by every recursive `appearance` visit.
struct Walk<'a> {
    body: &'a noble_kernel::execution::Body,
    sizes: &'a [Cost],
    permitted_height: u32,
}

// Process quotation children on every occurrence, not merely once for each
// unique quotation node. Cycles and depth past the kernel bound fail closed.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the walk follows the kernel's checked node, type or pattern tree, and an explicit worklist would change the metered traversal order."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the call passes the complete checked input, plan and shared-budget coordinates, and an options struct would only rename the same borrowed fields."
)]
fn appearance(
    walk: &Walk<'_>,
    id: noble_kernel::untrusted::NodeId,
    depth: usize,
    frame: &mut u64,
    frame_height: &mut u64,
    largest: &mut u64,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    if depth > super::PATH_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(work.charge(1));
    let index = attempt!(usize::try_from(id.0).map_err(|_| crate::Diagnostic::Invalid));
    let node = attempt!(walk
        .body
        .candidate
        .nodes
        .get(index)
        .ok_or(crate::Diagnostic::Invalid));
    let cost = attempt!(walk.sizes.get(index).ok_or(crate::Diagnostic::Invalid));
    // match_tail compares this carried frame with this invocation's input,
    // never a later frame against an earlier interface. Keep charging each
    // successful join's entire carried frame separately below.
    *largest = (*largest).max(attempt!(mismatch_reserve(
        *frame,
        cost.input,
        *frame_height,
        cost.input_height,
        walk.permitted_height
    )));
    attempt!(work.charge(attempt!(mul(*frame, 16))));
    attempt!(work.charge(cost.clone));
    if let noble_kernel::untrusted::Node::Quotation { body: children, .. } = node {
        let mut child_frame = cost.quotation_input;
        let mut child_height = cost.quotation_input_height;
        for child in children {
            attempt!(appearance(
                walk,
                *child,
                depth + 1,
                &mut child_frame,
                &mut child_height,
                largest,
                work
            ));
        }
        // Even an empty quotation body completes against its claimed Stack2.
        *largest = (*largest).max(attempt!(mismatch_reserve(
            child_frame,
            cost.quotation_output,
            child_height,
            cost.quotation_output_height,
            walk.permitted_height
        )));
    }
    let remainder = attempt!(frame
        .checked_sub(cost.input)
        .ok_or(crate::Diagnostic::Invalid));
    let remainder_height = attempt!(frame_height
        .checked_sub(cost.input_height)
        .ok_or(crate::Diagnostic::Invalid));
    *frame = attempt!(add(remainder, cost.output));
    *frame_height = attempt!(add(remainder_height, cost.output_height));
    Ok(())
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the call passes the complete checked input, plan and shared-budget coordinates, and an options struct would only rename the same borrowed fields."
)]
fn body(
    body: &noble_kernel::execution::Body,
    input: &[noble_kernel::types::Ty],
    claimed: &[noble_kernel::types::Ty],
    submission: &noble_kernel::execution::Submission,
    largest: &mut u64,
    permitted_height: u32,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let lookup_rows = attempt!(u64::try_from(
        submission
            .environment
            .nominals
            .len()
            .saturating_add(submission.environment.generic_variants.len())
    )
    .map_err(|_| crate::Diagnostic::Exhausted));
    let mut frame = 0u64;
    let mut frame_height = 0u64;
    for ty in input {
        let (_, weighted, lookups) = attempt!(super::concrete::count(ty, 512, work));
        frame = attempt!(add(frame, weighted));
        frame = attempt!(add(
            frame,
            attempt!(mul(lookups, attempt!(mul(lookup_rows, 16))))
        ));
        frame_height = attempt!(add(frame_height, 1));
    }
    let mut claimed_volume = 0u64;
    let mut claimed_height = 0u64;
    for ty in claimed {
        claimed_volume = attempt!(add(
            claimed_volume,
            attempt!(attempt!(type_volume(ty, work)).cost(lookup_rows))
        ));
        claimed_height = attempt!(add(claimed_height, 1));
    }
    let mut sizes = alloc::vec::Vec::with_capacity(body.candidate.nodes.len());
    for item in &body.candidate.nodes {
        sizes.push(attempt!(node(item, submission, work)));
    }
    let walk = Walk {
        body,
        sizes: &sizes,
        permitted_height,
    };
    for id in &body.candidate.body {
        attempt!(appearance(
            &walk,
            *id,
            0,
            &mut frame,
            &mut frame_height,
            largest,
            work
        ));
    }
    // A named definition or root can mismatch at completion with zero nodes.
    *largest = (*largest).max(attempt!(mismatch_reserve(
        frame,
        claimed_volume,
        frame_height,
        claimed_height,
        permitted_height
    )));
    Ok(())
}

fn mismatch_reserve(
    frame: u64,
    interface: u64,
    frame_height: u64,
    interface_height: u64,
    permitted_height: u32,
) -> Result<u64, crate::Diagnostic> {
    let height = frame_height.max(interface_height);
    if height > u64::from(permitted_height.min(128)) {
        return Err(crate::Diagnostic::Exhausted);
    }
    let entries = attempt!(add(frame, interface));
    mul(attempt!(mul(entries, height)), 16)
}

pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    // Acceptance halts on its first mismatch across all bodies and nested
    // quotations. Reserve the largest actual comparison site, not impossible
    // combinations of different bodies or different joins within one body.
    // Borrowed scans, successful joins and witness walks remain cumulative.
    let permitted_height = submission.request.limits.stack_height;
    let mut largest = 0u64;
    attempt!(body(
        &submission.body,
        &submission.request.expected.stack_in,
        &submission.request.expected.stack_out,
        submission,
        &mut largest,
        permitted_height,
        work
    ));
    for definition in &submission.definitions {
        attempt!(body(
            &definition.body,
            &definition.expected.stack_in,
            &definition.expected.stack_out,
            submission,
            &mut largest,
            permitted_height,
            work
        ));
    }
    attempt!(work.charge(largest));
    Ok(())
}
