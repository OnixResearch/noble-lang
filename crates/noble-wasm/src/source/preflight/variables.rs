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

/// The `index`th item of `first` followed by `second`, in `first.iter().chain(second)` order.
fn chained<'a, T>(first: &'a [T], second: &'a [T], index: usize) -> &'a T {
    match first.get(index) {
        Some(item) => item,
        None => &second[index - first.len()],
    }
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
            attempt!(finite_patterns(input, 0, depth + 1, generic, work));
            finite_patterns(output, 0, depth + 1, generic, work)
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

// The calculator's environment clone budget, checked before any pricing walk,
// bounds every scheme pattern to depth 64, 512 nodes and stacks of 128, so the
// stack walks below recurse over a small bounded index instead of looping
// inside the recursive pattern walk.

/// Check the patterns of one Program stack from `index` on, in order,
/// stopping at the first failure.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the walk follows the kernel's checked pattern tree, whose stacks the environment clone budget bounds to 128 patterns, and an explicit worklist would change the metered traversal order."
)]
fn finite_patterns(
    patterns: &[noble_kernel::shapes::Pattern],
    index: usize,
    depth: usize,
    generic: bool,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let Some(item) = patterns.get(index) else {
        return Ok(());
    };
    attempt!(finite_pattern(item, depth, generic, work));
    finite_patterns(patterns, index + 1, depth, generic, work)
}

pub(super) fn schemes(
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let defs = &submission.environment.defs;
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < defs.len() {
        outcome = scheme_patterns(&defs[index], work);
        index += 1;
    }
    outcome
}

/// Check one environment scheme's input, then output, patterns in order.
fn scheme_patterns(
    scheme: &noble_kernel::words::Scheme,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < scheme.stack_in.len() + scheme.stack_out.len() {
        let item = chained(&scheme.stack_in, &scheme.stack_out, index);
        outcome = finite_pattern(item, 0, false, work);
        index += 1;
    }
    outcome
}

/// Add one nominal descriptor type's weighted volume and lookups to `total`.
fn descriptor_cost(
    ty: &noble_kernel::types::Ty,
    total: u64,
    lookup_rows: u64,
    work: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let (_, weighted, lookups) = attempt!(super::concrete::count(ty, 512, work));
    let total = attempt!(add(total, attempt!(mul(weighted, 16))));
    add(
        total,
        attempt!(mul(lookups, attempt!(mul(lookup_rows, 16)))),
    )
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
            let total = attempt!(mul(
                attempt!(add(effects.len() as u64, 1)),
                attempt!(mul(next, 16))
            ));
            let pricing = Pricing {
                sizes,
                depth: next,
                lookup_rows,
            };
            let total = attempt!(stack_patterns(input, 0, total, pricing, work));
            stack_patterns(output, 0, total, pricing, work)
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
                    total = attempt!(descriptor_cost(ty, total, lookup_rows, work));
                }
                noble_kernel::types::NominalShape::Variant(left, right) => {
                    total = attempt!(descriptor_cost(left, total, lookup_rows, work));
                    total = attempt!(descriptor_cost(right, total, lookup_rows, work));
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

/// The fixed inputs that price every pattern of one Program stack.
#[derive(Clone, Copy)]
struct Pricing<'a> {
    sizes: &'a [u64; 8],
    depth: u64,
    lookup_rows: u64,
}

/// Add the price of every pattern of one Program stack from `index` on to
/// `total`, in order, stopping at the first failure.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the walk follows the kernel's checked pattern tree, whose stacks the environment clone budget bounds to 128 patterns, and an explicit worklist would change the metered traversal order."
)]
fn stack_patterns(
    patterns: &[noble_kernel::shapes::Pattern],
    index: usize,
    total: u64,
    pricing: Pricing<'_>,
    work: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let Some(item) = patterns.get(index) else {
        return Ok(total);
    };
    let cost = attempt!(pattern(
        item,
        pricing.sizes,
        pricing.depth,
        pricing.lookup_rows,
        work
    ));
    let total = attempt!(add(total, cost));
    stack_patterns(patterns, index + 1, total, pricing, work)
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
    projected_from(
        patterns,
        0,
        Volume::default(),
        Projection { bindings, depth },
        work,
    )
}

/// The fixed inputs that project every pattern of one stack.
#[derive(Clone, Copy)]
struct Projection<'a> {
    bindings: &'a [Volume; 8],
    depth: usize,
}

/// Add the projected volume of every pattern from `index` on to `result`, in
/// order, stopping at the first failure. The environment clone budget bounds
/// each stack to 128 patterns, so the recursion stays shallow.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the walk follows the kernel's checked pattern tree, whose stacks the environment clone budget bounds to 128 patterns, and an explicit worklist would change the metered traversal order."
)]
fn projected_from(
    patterns: &[noble_kernel::shapes::Pattern],
    index: usize,
    result: Volume,
    projection: Projection<'_>,
    work: &mut super::super::Work,
) -> Result<Volume, crate::Diagnostic> {
    let Some(item) = patterns.get(index) else {
        return Ok(result);
    };
    let volume = attempt!(projected(item, projection.bindings, projection.depth, work));
    let result = attempt!(result.add(volume));
    projected_from(patterns, index + 1, result, projection, work)
}

/// The summed volume of a stack binding's types, in order.
fn stack_volume(
    stack: &[noble_kernel::types::Ty],
    work: &mut super::super::Work,
) -> Result<Volume, crate::Diagnostic> {
    let mut total = Volume::default();
    let mut index = 0usize;
    let mut failure = None;
    while index < stack.len() {
        match type_volume(&stack[index], work) {
            Ok(volume) => match total.add(volume) {
                Ok(sum) => {
                    total = sum;
                    index += 1;
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            },
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(total),
    }
}

/// The substituted volume of one witness binding.
fn binding_volume(
    binding: &noble_kernel::words::Binding,
    work: &mut super::super::Work,
) -> Result<Volume, crate::Diagnostic> {
    match binding {
        noble_kernel::words::Binding::Value(ty) => type_volume(ty, work),
        noble_kernel::words::Binding::Stack(stack) => stack_volume(stack, work),
        noble_kernel::words::Binding::Effect(effects) => Ok(Volume {
            height: 0,
            nodes: 0,
            weighted: effects.as_slice().len() as u64,
            lookups: 0,
        }),
        noble_kernel::words::Binding::Ref(_) => Ok(Volume::default()),
    }
}

/// One charged reference hop: the binding index it names, or `None` at a non-reference.
fn ref_hop(
    inst: &noble_kernel::words::Inst,
    current: usize,
    hops: usize,
    work: &mut super::super::Work,
) -> Result<Option<usize>, crate::Diagnostic> {
    attempt!(work.charge(1));
    if hops >= inst.bindings.len() {
        return Err(crate::Diagnostic::Invalid);
    }
    match &inst.bindings[current] {
        noble_kernel::words::Binding::Ref(var) => {
            let next = attempt!(usize::try_from(var.0).map_err(|_| crate::Diagnostic::Invalid));
            if next >= inst.bindings.len() {
                return Err(crate::Diagnostic::Invalid);
            }
            Ok(Some(next))
        }
        _ => Ok(None),
    }
}

/// Follow reference bindings from `index` to the binding they name, charging every hop.
fn resolved(
    inst: &noble_kernel::words::Inst,
    index: usize,
    work: &mut super::super::Work,
) -> Result<usize, crate::Diagnostic> {
    let mut current = index;
    let mut hops = 0usize;
    let mut failure = None;
    loop {
        match ref_hop(inst, current, hops, work) {
            Ok(Some(next)) => {
                current = next;
                hops += 1;
            }
            Ok(None) => break,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(current),
    }
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
    let mut index = 0usize;
    let mut failure = None;
    while index < inst.bindings.len() {
        match binding_volume(&inst.bindings[index], work) {
            Ok(volume) => volumes[index] = volume,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        match volumes[index].cost(lookup_rows) {
            Ok(size) => {
                sizes[index] = size;
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut total = 1u64;
    index = 0;
    while index < inst.bindings.len() {
        if matches!(inst.bindings[index], noble_kernel::words::Binding::Ref(_)) {
            match resolved(inst, index, work) {
                Ok(current) => {
                    volumes[index] = volumes[current];
                    sizes[index] = sizes[current];
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        match add(total, sizes[index]) {
            Ok(sum) => {
                total = sum;
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
        None => Ok((sizes, volumes, total)),
    }
}

#[derive(Clone, Copy)]
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

// Each node kind prices itself in its own function, so no pricing step
// follows a branch that two node kinds share.
fn node(
    node: &noble_kernel::untrusted::Node,
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<Cost, crate::Diagnostic> {
    match node {
        noble_kernel::untrusted::Node::Literal { inst, .. } => literal_cost(inst, submission, work),
        noble_kernel::untrusted::Node::Quotation { inst, body, .. } => {
            quotation_cost(inst, body.len(), submission, work)
        }
        noble_kernel::untrusted::Node::Invocation { inst, def, .. } => {
            invocation_cost(inst, *def, submission, work)
        }
        // The kernel refuses borrowed dispatch outside the live-slot profile.
        // The calculator profile never authorizes signed self recursion.
        noble_kernel::untrusted::Node::SlotInvoke { .. }
        | noble_kernel::untrusted::Node::SignedSelf { .. } => Err(crate::Diagnostic::Unsupported),
    }
}

/// The witness sizes and volumes every node prices from, its base clone cost
/// and the environment's nominal lookup rows.
struct NodeWitness {
    sizes: [u64; 8],
    volumes: [Volume; 8],
    cost: u64,
    lookup_rows: u64,
}

/// Price a node's instantiation witness.
fn node_witness(
    inst: &noble_kernel::words::Inst,
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<NodeWitness, crate::Diagnostic> {
    let lookup_rows = attempt!(u64::try_from(
        submission
            .environment
            .nominals
            .len()
            .saturating_add(submission.environment.generic_variants.len())
    )
    .map_err(|_| crate::Diagnostic::Exhausted));
    let (sizes, volumes, witness_size) = attempt!(witness(inst, lookup_rows, work));
    let cost = attempt!(mul(witness_size, 16));
    Ok(NodeWitness {
        sizes,
        volumes,
        cost,
        lookup_rows,
    })
}

/// A literal copies its witness and pushes one cell.
fn literal_cost(
    inst: &noble_kernel::words::Inst,
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<Cost, crate::Diagnostic> {
    let witness = attempt!(node_witness(inst, submission, work));
    let cost = attempt!(add(witness.cost, attempt!(mul(witness.sizes[0], 32))));
    let input = witness.sizes[0];
    let output = attempt!(add(input, 1));
    let input_height = witness.volumes[0].height;
    let output_height = attempt!(add(input_height, 1));
    Ok(Cost {
        clone: cost,
        input,
        output,
        quotation_input: 0,
        quotation_output: 0,
        input_height,
        output_height,
        quotation_input_height: 0,
        quotation_output_height: 0,
    })
}

/// A quotation copies its body and stacks and pushes one constructed Program.
fn quotation_cost(
    inst: &noble_kernel::words::Inst,
    body_len: usize,
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<Cost, crate::Diagnostic> {
    let witness = attempt!(node_witness(inst, submission, work));
    let mut cost = attempt!(add(witness.cost, attempt!(mul(body_len as u64, 16))));
    let stack = attempt!(add(
        attempt!(mul(witness.sizes[0], 2)),
        attempt!(add(witness.sizes[1], witness.sizes[2]))
    ));
    cost = attempt!(add(cost, attempt!(mul(stack, 16))));
    let constructed = attempt!(witness.volumes[1].add(witness.volumes[2])).constructed(1, false);
    let input = witness.sizes[0];
    let output = attempt!(add(
        input,
        attempt!(attempt!(constructed).cost(witness.lookup_rows))
    ));
    let input_height = witness.volumes[0].height;
    let output_height = attempt!(add(input_height, 1));
    Ok(Cost {
        clone: cost,
        input,
        output,
        quotation_input: witness.sizes[1],
        quotation_output: witness.sizes[2],
        input_height,
        output_height,
        quotation_input_height: witness.volumes[1].height,
        quotation_output_height: witness.volumes[2].height,
    })
}

/// An invocation prices its scheme's witness and every pattern of its scheme,
/// then projects its concrete input and output stacks.
fn invocation_cost(
    inst: &noble_kernel::words::Inst,
    def: noble_kernel::contracts::Definition,
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<Cost, crate::Diagnostic> {
    let witness = attempt!(node_witness(inst, submission, work));
    let scheme = attempt!(submission
        .environment
        .scheme(def)
        .ok_or(crate::Diagnostic::Invalid));
    let cost = attempt!(add(
        witness.cost,
        attempt!(super::super::admission::witness_scheme(
            submission, def, work
        ))
    ));
    let cost = attempt!(scheme_cost(
        scheme,
        &witness.sizes,
        witness.lookup_rows,
        cost,
        work
    ));
    let projected_input = attempt!(projected_stack(&scheme.stack_in, &witness.volumes, 0, work));
    let projected_output = attempt!(projected_stack(
        &scheme.stack_out,
        &witness.volumes,
        0,
        work
    ));
    let input = attempt!(projected_input.cost(witness.lookup_rows));
    let output = attempt!(projected_output.cost(witness.lookup_rows));
    Ok(Cost {
        clone: cost,
        input,
        output,
        quotation_input: 0,
        quotation_output: 0,
        input_height: projected_input.height,
        output_height: projected_output.height,
        quotation_input_height: 0,
        quotation_output_height: 0,
    })
}

/// Add every input, then output, pattern of a scheme to `cost`, stopping at
/// the first failure.
fn scheme_cost(
    scheme: &noble_kernel::words::Scheme,
    sizes: &[u64; 8],
    lookup_rows: u64,
    cost: u64,
    work: &mut super::super::Work,
) -> Result<u64, crate::Diagnostic> {
    let mut cost = cost;
    let mut index = 0usize;
    let mut failure = None;
    while index < scheme.stack_in.len() + scheme.stack_out.len() {
        let value = chained(&scheme.stack_in, &scheme.stack_out, index);
        match pattern(value, sizes, 0, lookup_rows, work) {
            Ok(size) => match add(cost, size) {
                Ok(sum) => {
                    cost = sum;
                    index += 1;
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            },
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(cost),
    }
}

/// The immutable inputs shared by every appearance visit.
#[derive(Clone, Copy)]
struct Walk<'a> {
    body: &'a noble_kernel::execution::Body,
    sizes: &'a [Cost],
    permitted_height: u32,
}

/// What one open level of the appearance walk iterates: the body's roots, or
/// the children of one quotation appearance together with its priced cost.
#[derive(Clone, Copy)]
enum Scope {
    Roots,
    Quotation(usize, Cost),
}

/// One open level of the appearance walk: what it iterates, the depth of its
/// children, the next child position and the frame its children carry.
#[derive(Clone, Copy)]
struct Level {
    scope: Scope,
    child_depth: usize,
    next: usize,
    frame: u64,
    frame_height: u64,
}

/// The appearance walk's state: its open levels, innermost last, and the
/// largest mismatch reserve seen so far.
struct Appearances {
    levels: alloc::vec::Vec<Level>,
    largest: u64,
}

// Process quotation children on every occurrence, not merely once for each
// unique quotation node. Cycles and depth past the kernel bound fail closed.
// One loop walks every appearance depth-first over an explicit level stack,
// so each appearance is entered, charged, closed and refused in exactly the
// order of a recursive walk, without a loop inside a recursive call.
/// Walk every appearance of a body against its carried frame and return the
/// frame after the last root.
fn appearances(
    walk: Walk<'_>,
    frame: u64,
    frame_height: u64,
    largest: &mut u64,
    work: &mut super::super::Work,
) -> Result<(u64, u64), crate::Diagnostic> {
    let mut levels = alloc::vec::Vec::with_capacity(super::PATH_LIMIT + 2);
    levels.push(Level {
        scope: Scope::Roots,
        child_depth: 0,
        next: 0,
        frame,
        frame_height,
    });
    let mut state = Appearances {
        levels,
        largest: *largest,
    };
    let mut is_walking = true;
    let mut failure = None;
    while is_walking {
        match appearance_step(walk, &mut state, work) {
            Ok(is_open) => is_walking = is_open,
            Err(problem) => {
                failure = Some(problem);
                is_walking = false;
            }
        }
    }
    *largest = state.largest;
    match failure {
        Some(problem) => Err(problem),
        None => Ok((state.levels[0].frame, state.levels[0].frame_height)),
    }
}

/// Advance the walk by one event: enter the innermost level's next child, or
/// close that level once its children are done. `Ok(false)` once every root
/// has been walked.
fn appearance_step(
    walk: Walk<'_>,
    state: &mut Appearances,
    work: &mut super::super::Work,
) -> Result<bool, crate::Diagnostic> {
    let top = state.levels.len() - 1;
    let level = state.levels[top];
    let Some(child) = next_child(walk, level) else {
        return close_level(walk, state);
    };
    state.levels[top].next += 1;
    attempt!(enter_appearance(
        walk,
        child,
        level.child_depth,
        state,
        work
    ));
    Ok(true)
}

/// The next child an open level iterates, if any remain.
fn next_child(walk: Walk<'_>, level: Level) -> Option<noble_kernel::untrusted::NodeId> {
    match level.scope {
        Scope::Roots => walk.body.candidate.body.get(level.next).copied(),
        Scope::Quotation(index, _) => quotation_child(walk, index, level.next),
    }
}

/// Child `position` of the quotation at arena `index`.
fn quotation_child(
    walk: Walk<'_>,
    index: usize,
    position: usize,
) -> Option<noble_kernel::untrusted::NodeId> {
    match walk.body.candidate.nodes.get(index) {
        Some(noble_kernel::untrusted::Node::Quotation { body: children, .. }) => {
            children.get(position).copied()
        }
        // Only a quotation appearance opens a level, so nothing else has children here.
        Some(_) | None => None,
    }
}

/// Enter one appearance at `depth`: charge it against the innermost level's
/// frame, then open its quotation body or carry its output into that frame.
fn enter_appearance(
    walk: Walk<'_>,
    id: noble_kernel::untrusted::NodeId,
    depth: usize,
    state: &mut Appearances,
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
    let cost = *attempt!(walk.sizes.get(index).ok_or(crate::Diagnostic::Invalid));
    let target = state.levels[state.levels.len() - 1];
    // match_tail compares this carried frame with this invocation's input,
    // never a later frame against an earlier interface. Keep charging each
    // successful join's entire carried frame separately below.
    state.largest = state.largest.max(attempt!(mismatch_reserve(
        target.frame,
        cost.input,
        target.frame_height,
        cost.input_height,
        walk.permitted_height
    )));
    attempt!(work.charge(attempt!(mul(target.frame, 16))));
    attempt!(work.charge(cost.clone));
    match node {
        noble_kernel::untrusted::Node::Quotation { .. } => {
            state.levels.push(Level {
                scope: Scope::Quotation(index, cost),
                child_depth: depth + 1,
                next: 0,
                frame: cost.quotation_input,
                frame_height: cost.quotation_input_height,
            });
            Ok(())
        }
        _ => finish_appearance(cost, state),
    }
}

/// Close the innermost level once its children are done. A quotation reserves
/// its claimed output mismatch, then finishes against its parent's frame; the
/// roots level ends the walk.
fn close_level(walk: Walk<'_>, state: &mut Appearances) -> Result<bool, crate::Diagnostic> {
    let top = state.levels.len() - 1;
    let level = state.levels[top];
    let Scope::Quotation(_, cost) = level.scope else {
        return Ok(false);
    };
    // Even an empty quotation body completes against its claimed Stack2.
    state.largest = state.largest.max(attempt!(mismatch_reserve(
        level.frame,
        cost.quotation_output,
        level.frame_height,
        cost.quotation_output_height,
        walk.permitted_height
    )));
    state.levels.truncate(top);
    attempt!(finish_appearance(cost, state));
    Ok(true)
}

/// Carry a finished appearance's output into the innermost level's frame.
fn finish_appearance(cost: Cost, state: &mut Appearances) -> Result<(), crate::Diagnostic> {
    let top = state.levels.len() - 1;
    let remainder = attempt!(state.levels[top]
        .frame
        .checked_sub(cost.input)
        .ok_or(crate::Diagnostic::Invalid));
    let remainder_height = attempt!(state.levels[top]
        .frame_height
        .checked_sub(cost.input_height)
        .ok_or(crate::Diagnostic::Invalid));
    state.levels[top].frame = attempt!(add(remainder, cost.output));
    state.levels[top].frame_height = attempt!(add(remainder_height, cost.output_height));
    Ok(())
}

/// Carry one input type's weighted volume and nominal lookups into the frame.
fn input_frame(
    ty: &noble_kernel::types::Ty,
    lookup_rows: u64,
    frame: u64,
    frame_height: u64,
    work: &mut super::super::Work,
) -> Result<(u64, u64), crate::Diagnostic> {
    let (_, weighted, lookups) = attempt!(super::concrete::count(ty, 512, work));
    let frame = attempt!(add(frame, weighted));
    let frame = attempt!(add(
        frame,
        attempt!(mul(lookups, attempt!(mul(lookup_rows, 16))))
    ));
    Ok((frame, attempt!(add(frame_height, 1))))
}

/// Add one claimed output type's volume cost to the claimed totals.
fn claimed_frame(
    ty: &noble_kernel::types::Ty,
    lookup_rows: u64,
    volume: u64,
    height: u64,
    work: &mut super::super::Work,
) -> Result<(u64, u64), crate::Diagnostic> {
    let volume = attempt!(add(
        volume,
        attempt!(attempt!(type_volume(ty, work)).cost(lookup_rows))
    ));
    Ok((volume, attempt!(add(height, 1))))
}

/// The frame volume and height carried in from a body's input types, in order.
fn input_frames(
    input: &[noble_kernel::types::Ty],
    lookup_rows: u64,
    work: &mut super::super::Work,
) -> Result<(u64, u64), crate::Diagnostic> {
    let mut frame = 0u64;
    let mut frame_height = 0u64;
    let mut index = 0usize;
    let mut failure = None;
    while index < input.len() {
        match input_frame(&input[index], lookup_rows, frame, frame_height, work) {
            Ok((next_frame, next_height)) => {
                frame = next_frame;
                frame_height = next_height;
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
        None => Ok((frame, frame_height)),
    }
}

/// The claimed output volume cost and height of a body, in order.
fn claimed_frames(
    claimed: &[noble_kernel::types::Ty],
    lookup_rows: u64,
    work: &mut super::super::Work,
) -> Result<(u64, u64), crate::Diagnostic> {
    let mut claimed_volume = 0u64;
    let mut claimed_height = 0u64;
    let mut index = 0usize;
    let mut failure = None;
    while index < claimed.len() {
        match claimed_frame(
            &claimed[index],
            lookup_rows,
            claimed_volume,
            claimed_height,
            work,
        ) {
            Ok((volume, height)) => {
                claimed_volume = volume;
                claimed_height = height;
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
        None => Ok((claimed_volume, claimed_height)),
    }
}

/// The priced cost of every arena node, in arena order.
fn node_costs(
    body: &noble_kernel::execution::Body,
    submission: &noble_kernel::execution::Submission,
    work: &mut super::super::Work,
) -> Result<alloc::vec::Vec<Cost>, crate::Diagnostic> {
    let mut sizes = alloc::vec::Vec::with_capacity(body.candidate.nodes.len());
    let mut index = 0usize;
    let mut failure = None;
    while index < body.candidate.nodes.len() {
        match node(&body.candidate.nodes[index], submission, work) {
            Ok(cost) => {
                sizes.push(cost);
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
        None => Ok(sizes),
    }
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
    let (frame, frame_height) = attempt!(input_frames(input, lookup_rows, work));
    let (claimed_volume, claimed_height) = attempt!(claimed_frames(claimed, lookup_rows, work));
    let sizes = attempt!(node_costs(body, submission, work));
    let walk = Walk {
        body,
        sizes: &sizes,
        permitted_height,
    };
    let (frame, frame_height) = attempt!(appearances(walk, frame, frame_height, largest, work));
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
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < submission.definitions.len() {
        let definition = &submission.definitions[index];
        outcome = body(
            &definition.body,
            &definition.expected.stack_in,
            &definition.expected.stack_out,
            submission,
            &mut largest,
            permitted_height,
            work,
        );
        index += 1;
    }
    attempt!(outcome);
    attempt!(work.charge(largest));
    Ok(())
}

// Test harness only: the recursive appearance walk that the explicit-stack
// walk replaced is kept verbatim as the differential oracle.
#[allow(
    clippy::too_many_arguments,
    tigerstyle::function_length,
    tigerstyle::no_recursion,
    tigerstyle::numeric_units,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; test harness: the oracle is the replaced recursive walk verbatim, and fixture setup legs are the readable form."
)]
#[cfg(test)]
mod appearance_tests {
    fn budget(remaining: u64) -> super::super::super::Work {
        super::super::super::Work {
            remaining,
            issued: remaining,
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        }
    }

    /// The replaced recursive walk of one appearance and its quotation body.
    fn recursive_visit(
        walk: super::Walk<'_>,
        id: noble_kernel::untrusted::NodeId,
        depth: usize,
        frame: &mut u64,
        frame_height: &mut u64,
        largest: &mut u64,
        work: &mut super::super::super::Work,
    ) -> Result<(), crate::Diagnostic> {
        if depth > super::super::PATH_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        work.charge(1)?;
        let index = usize::try_from(id.0).map_err(|_| crate::Diagnostic::Invalid)?;
        let node = walk
            .body
            .candidate
            .nodes
            .get(index)
            .ok_or(crate::Diagnostic::Invalid)?;
        let cost = walk.sizes.get(index).ok_or(crate::Diagnostic::Invalid)?;
        *largest = (*largest).max(super::mismatch_reserve(
            *frame,
            cost.input,
            *frame_height,
            cost.input_height,
            walk.permitted_height,
        )?);
        work.charge(super::mul(*frame, 16)?)?;
        work.charge(cost.clone)?;
        if let noble_kernel::untrusted::Node::Quotation { body: children, .. } = node {
            let mut child_frame = cost.quotation_input;
            let mut child_height = cost.quotation_input_height;
            for child in children {
                recursive_visit(
                    walk,
                    *child,
                    depth + 1,
                    &mut child_frame,
                    &mut child_height,
                    largest,
                    work,
                )?;
            }
            *largest = (*largest).max(super::mismatch_reserve(
                child_frame,
                cost.quotation_output,
                child_height,
                cost.quotation_output_height,
                walk.permitted_height,
            )?);
        }
        let remainder = frame
            .checked_sub(cost.input)
            .ok_or(crate::Diagnostic::Invalid)?;
        let remainder_height = frame_height
            .checked_sub(cost.input_height)
            .ok_or(crate::Diagnostic::Invalid)?;
        *frame = super::add(remainder, cost.output)?;
        *frame_height = super::add(remainder_height, cost.output_height)?;
        Ok(())
    }

    /// Every root appearance of a body through the recursive walk.
    fn recursive_appearances(
        walk: super::Walk<'_>,
        frame: u64,
        frame_height: u64,
        largest: &mut u64,
        work: &mut super::super::super::Work,
    ) -> Result<(u64, u64), crate::Diagnostic> {
        let mut frame = frame;
        let mut frame_height = frame_height;
        for root in &walk.body.candidate.body {
            recursive_visit(walk, *root, 0, &mut frame, &mut frame_height, largest, work)?;
        }
        Ok((frame, frame_height))
    }

    /// Both walks agree on the result, the largest reserve and the remaining
    /// work for every budget from exhaustion at the first charge to success.
    fn assert_walks_agree(
        submission: &noble_kernel::execution::Submission,
        body: &noble_kernel::execution::Body,
        input: &[noble_kernel::types::Ty],
    ) {
        let lookup_rows = u64::try_from(
            submission
                .environment
                .nominals
                .len()
                .saturating_add(submission.environment.generic_variants.len()),
        )
        .expect("lookup rows fit");
        let mut setup = budget(u64::MAX / 4);
        let (frame, frame_height) = super::input_frames(input, lookup_rows, &mut setup)
            .unwrap_or_else(|_| panic!("input frame"));
        let sizes = super::node_costs(body, submission, &mut setup)
            .unwrap_or_else(|_| panic!("node costs"));
        let walk = super::Walk {
            body,
            sizes: &sizes,
            permitted_height: submission.request.limits.stack_height,
        };
        let full = u64::MAX / 4;
        let mut probe = budget(full);
        let mut probe_largest = 0u64;
        assert!(
            recursive_appearances(walk, frame, frame_height, &mut probe_largest, &mut probe)
                .is_ok(),
            "the fixture walks to completion"
        );
        let spent = full - probe.remaining;
        assert!(spent > 0, "the fixture charges its appearances");
        for remaining in 0..=spent {
            let mut oracle_work = budget(remaining);
            let mut oracle_largest = 0u64;
            let expected = recursive_appearances(
                walk,
                frame,
                frame_height,
                &mut oracle_largest,
                &mut oracle_work,
            );
            let mut walk_work = budget(remaining);
            let mut walk_largest = 0u64;
            let actual =
                super::appearances(walk, frame, frame_height, &mut walk_largest, &mut walk_work);
            assert!(expected == actual, "result differs at budget {remaining}");
            assert_eq!(
                oracle_largest, walk_largest,
                "largest reserve differs at budget {remaining}"
            );
            assert_eq!(
                oracle_work.remaining, walk_work.remaining,
                "remaining work differs at budget {remaining}"
            );
        }
    }

    #[test]
    fn explicit_stack_walk_matches_the_recursive_walk_at_every_budget() {
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let mut calculator = noble_contracts::source::Session::new_calculator();
        let defined = calculator
            .prepare(
                b"def nest [ [ 1 [ 2 [ 3 ] drop ] drop ] drop [ ] drop ]",
                &[],
                limits,
            )
            .unwrap_or_else(|_| panic!("calculator definition refused"));
        calculator
            .commit(defined)
            .unwrap_or_else(|_| panic!("calculator definition commit refused"));
        let call = calculator
            .prepare(b"[ 4 [ 5 ] drop ] drop nest 6 nest drop", &[], limits)
            .unwrap_or_else(|_| panic!("calculator call refused"));
        let submission = call
            .submission()
            .unwrap_or_else(|| panic!("missing calculator submission"));
        assert!(
            !submission.definitions.is_empty(),
            "the call carries the nested definition body"
        );
        let has_nested_bodies = submission.definitions.iter().any(|definition| {
            definition.body.candidate.nodes.iter().any(|node| {
                matches!(node, noble_kernel::untrusted::Node::Quotation { body, .. } if !body.is_empty())
            })
        });
        assert!(
            has_nested_bodies,
            "the fixture walks non-empty quotation bodies"
        );
        assert_walks_agree(
            submission,
            &submission.body,
            &submission.request.expected.stack_in,
        );
        for definition in &submission.definitions {
            assert_walks_agree(submission, &definition.body, &definition.expected.stack_in);
        }
    }
}
