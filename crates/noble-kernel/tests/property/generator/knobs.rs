//! Witness-corruption knobs for the property lane (DX-PROPERTY-01).
//! Each mutation changes a claim, not the structures' decodability; both
//! checkers must independently re-decide the resulting candidate.

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; mutate_type follows one proper child of each owned pair/sum/list and changes the reached leaf or program; no external validity assumption needs an assertion."
)]
fn mutate_type(ty: &mut noble_kernel::types::Ty) {
    let mut current = ty;
    loop {
        match current {
            noble_kernel::types::Ty::Pair(left, _)
            | noble_kernel::types::Ty::Sum(left, _)
            | noble_kernel::types::Ty::List(left) => current = left,
            _ => {
                *current = match current {
                    noble_kernel::types::Ty::Unit => noble_kernel::types::Ty::Bool,
                    noble_kernel::types::Ty::Bool => noble_kernel::types::Ty::I64,
                    noble_kernel::types::Ty::I64 => noble_kernel::types::Ty::Text,
                    noble_kernel::types::Ty::Text => noble_kernel::types::Ty::Syntax,
                    noble_kernel::types::Ty::Syntax => noble_kernel::types::Ty::Unit,
                    noble_kernel::types::Ty::Program(_, _, _) => {
                        noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::I64))
                    }
                    _ => noble_kernel::types::Ty::I64,
                };
                return;
            }
        }
    }
}

struct Sites {
    stacks: Vec<(usize, usize)>,
    bindings: Vec<(usize, usize)>,
    quotes: Vec<usize>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; sites derives capacities and positions from the candidate and returns an error for an unaddressable binding count; empty site sets are valid knob inputs, not assertion failures."
)]
fn sites(candidate: &noble_kernel::untrusted::Candidate) -> Result<crate::knobs::Sites, String> {
    let binding_count = candidate.nodes.iter().try_fold(0usize, |count, node| {
        count
            .checked_add(crate::knobs::bindings(node).len())
            .ok_or_else(|| "knob binding-site capacity exceeds usize".to_owned())
    })?;
    let mut found = crate::knobs::Sites {
        stacks: Vec::with_capacity(binding_count),
        bindings: Vec::with_capacity(candidate.nodes.len()),
        quotes: Vec::with_capacity(candidate.nodes.len()),
    };
    for (index, node) in candidate.nodes.iter().enumerate() {
        if matches!(node, noble_kernel::untrusted::Node::Quotation { .. }) {
            found.quotes.push(index);
        }
        let bindings = crate::knobs::bindings(node);
        if !bindings.is_empty() {
            found.bindings.push((index, bindings.len()));
        }
        for (slot, binding) in bindings.iter().enumerate() {
            if matches!(binding, noble_kernel::words::Binding::Stack(segment) if !segment.is_empty())
            {
                found.stacks.push((index, slot));
            }
        }
    }
    Ok(found)
}

/// Apply exactly one knob, chosen by the seed.
pub fn apply(
    rng: &mut crate::rng::Stream,
    request: &mut noble_kernel::untrusted::Request,
    candidate: &mut noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    let found = crate::knobs::sites(candidate)?;
    match rng.below(9) {
        0 => crate::knobs::kind(rng, candidate, &found.bindings)?,
        choice @ 1..=2 => crate::knobs::arity(rng, candidate, &found.bindings, choice == 1)?,
        choice @ 3..=4 => crate::knobs::stack(rng, candidate, &found.stacks, choice == 3)?,
        5 => match request.expected.stack_out.last_mut() {
            Some(last) => crate::knobs::mutate_type(last),
            None => request
                .expected
                .stack_out
                .push(noble_kernel::types::Ty::I64),
        },
        6 => request.expected.allowed_effects = noble_kernel::types::EffSet::empty(),
        7 => crate::knobs::quotation(rng, candidate, &found.quotes)?,
        _ => crate::knobs::repoint(rng, candidate)?,
    }
    Ok(())
}

fn kind(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
    sites: &[(usize, usize)],
) -> Result<(), String> {
    if sites.is_empty() {
        return Ok(());
    }
    let (node_index, _) = sites[rng.index(sites.len())?];
    let inst = crate::knobs::inst_of(&mut candidate.nodes[node_index]);
    let binding = inst
        .bindings
        .first_mut()
        .ok_or_else(|| "kind knob selected a node without bindings".to_owned())?;
    *binding = match binding {
        noble_kernel::words::Binding::Stack(_) => {
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Unit)
        }
        _ => noble_kernel::words::Binding::Stack(Vec::new()),
    };
    Ok(())
}

fn arity(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
    sites: &[(usize, usize)],
    is_drop: bool,
) -> Result<(), String> {
    if sites.is_empty() {
        return Ok(());
    }
    let (node_index, count) = sites[rng.index(sites.len())?];
    let inst = crate::knobs::inst_of(&mut candidate.nodes[node_index]);
    if is_drop {
        let remaining = count
            .checked_sub(1)
            .ok_or_else(|| "arity knob selected a node without bindings".to_owned())?;
        inst.bindings.truncate(remaining);
    } else {
        inst.bindings
            .push(noble_kernel::words::Binding::Stack(Vec::new()));
    }
    Ok(())
}

fn stack(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
    sites: &[(usize, usize)],
    is_mutation: bool,
) -> Result<(), String> {
    if sites.is_empty() {
        return Ok(());
    }
    let (node_index, slot) = sites[rng.index(sites.len())?];
    let inst = crate::knobs::inst_of(&mut candidate.nodes[node_index]);
    let noble_kernel::words::Binding::Stack(segment) = &mut inst.bindings[slot] else {
        return Err("stack knob selected a non-stack binding".to_owned());
    };
    let first = segment
        .first_mut()
        .ok_or_else(|| "stack knob selected an empty stack".to_owned())?;
    if is_mutation {
        crate::knobs::mutate_type(first);
    } else {
        segment.remove(0);
    }
    Ok(())
}

fn quotation(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
    sites: &[usize],
) -> Result<(), String> {
    if sites.is_empty() {
        return Ok(());
    }
    let index = sites[rng.index(sites.len())?];
    let inst = crate::knobs::inst_of(&mut candidate.nodes[index]);
    let binding = inst
        .bindings
        .get_mut(2)
        .ok_or_else(|| "quotation knob selected a missing output claim".to_owned())?;
    let noble_kernel::words::Binding::Stack(segment) = binding else {
        return Err("quotation knob selected a non-stack output claim".to_owned());
    };
    match segment.last_mut() {
        Some(last) => crate::knobs::mutate_type(last),
        None => segment.push(noble_kernel::types::Ty::Bool),
    }
    Ok(())
}

fn repoint(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    if candidate.nodes.len() <= 1 || candidate.body.is_empty() {
        return Ok(());
    }
    let position = rng.index(candidate.body.len())?;
    let arena = u32::try_from(candidate.nodes.len())
        .map_err(|error| format!("repointing arena size exceeds u32: {error}"))?;
    let mut target = rng.below_u32(arena)?;
    if target == candidate.body[position].0 {
        #[expect(
            tigerstyle::raw_arithmetic_overflow,
            reason = "Owner: noble-maintainers; the entry guard proves arena > 1 and below_u32(arena) gives target < arena; the increment is taken only when target < arena - 1."
        )]
        let next = if target == arena - 1 { 0 } else { target + 1 };
        target = next;
    }
    candidate.body[position] = noble_kernel::untrusted::NodeId(target);
    Ok(())
}

const fn bindings(node: &noble_kernel::untrusted::Node) -> &[noble_kernel::words::Binding] {
    match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. }
        | noble_kernel::untrusted::Node::Quotation { inst, .. } => inst.bindings.as_slice(),
    }
}

const fn inst_of(node: &mut noble_kernel::untrusted::Node) -> &mut noble_kernel::words::Inst {
    match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. }
        | noble_kernel::untrusted::Node::Quotation { inst, .. } => inst,
    }
}
