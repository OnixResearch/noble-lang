//! Structural corruption for the malformed-candidate lane (DX-PROPERTY-02).
//! Every corruption must reject or bound, never accept or panic. The lane
//! runs with the workspace's debug overflow checks enabled.

#[path = "corruptions/witness.rs"]
mod witness;

/// The name of each corruption, reported with the lane summary.
pub fn label(choice: u64) -> &'static str {
    match choice {
        0 => "format-revision",
        1 => "semantic-revision",
        2 => "body-reference-out-of-range",
        3 => "reference-u32-max",
        4 => "unknown-definition",
        5 => "instantiation-arity-minus",
        6 => "instantiation-arity-plus",
        7 => "instantiation-kind",
        8 => "oversized-stack-binding",
        9 => "depth-chain",
        10 => "truncated-arena",
        12 => "cyclic-witness",
        _ => "raw-identifier-bits",
    }
}

pub const KINDS: u64 = 13;

/// Corrupt one freshly generated case in exactly one structural way.
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; CANDIDATE_FORMAT is 1 and both revision draws are below 1000, so the corrupted format is at most 1001 and revision at most 1000; reassess if the format or draw bound changes."
)]
pub fn corrupt(
    rng: &mut crate::rng::Stream,
    mut case: crate::gen::Case,
) -> Result<(crate::gen::Case, &'static str), String> {
    let choice = rng.below(crate::malform::KINDS);
    let label = crate::malform::label(choice);
    match choice {
        0 => {
            case.candidate.format =
                noble_kernel::untrusted::CANDIDATE_FORMAT + 1 + rng.below_u32(1000)?;
        }
        1 => case.candidate.revision = 1 + rng.below_u32(1000)?,
        2 => crate::malform::out_of_range(rng, &mut case.candidate)?,
        3 => crate::malform::maximum_reference(rng, &mut case.candidate)?,
        4 => crate::malform::unknown_definition(rng, &mut case.candidate)?,
        5..=8 | 12 => crate::malform::witness::apply(rng, &mut case.candidate, choice)?,
        9 => crate::malform::depth_chain(&mut case.candidate),
        10 => crate::malform::truncate(&mut case.candidate)?,
        _ => crate::malform::raw_identifiers(rng, &mut case.candidate)?,
    }
    Ok((case, label))
}

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the dangling id is 2000 plus a below-400 draw, hence at most 2399 in u32; reassess if this corruption's offset or draw bound changes."
)]
fn out_of_range(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    let last = candidate
        .body
        .last_mut()
        .ok_or_else(|| "out-of-range corruption requires a nonempty body".to_owned())?;
    *last = noble_kernel::untrusted::NodeId(2000 + rng.below_u32(400)?);
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; maximum_reference deliberately writes u32::MAX into a selected quotation or root body, returning errors for missing selection premises rather than asserting the malformed result."
)]
fn maximum_reference(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    let mut quotes = Vec::with_capacity(candidate.nodes.len());
    for (index, node) in candidate.nodes.iter().enumerate() {
        if matches!(node, noble_kernel::untrusted::Node::Quotation { .. }) {
            quotes.push(index);
        }
    }
    if quotes.is_empty() {
        let first = candidate
            .body
            .first_mut()
            .ok_or_else(|| "maximum-reference corruption requires a body entry".to_owned())?;
        *first = noble_kernel::untrusted::NodeId(u32::MAX);
        return Ok(());
    }
    let target = quotes[rng.index(quotes.len())?];
    let noble_kernel::untrusted::Node::Quotation { body, .. } = &mut candidate.nodes[target] else {
        return Err("maximum-reference corruption selected a non-quotation".to_owned());
    };
    match body.first_mut() {
        Some(first) => *first = noble_kernel::untrusted::NodeId(u32::MAX),
        None => body.push(noble_kernel::untrusted::NodeId(u32::MAX)),
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; unknown_definition deliberately replaces or appends a foreign invocation and propagates arena-id conversion failure; the malformed lane verifies rejection, not validity assertions here."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the foreign definition is 100 plus a below-200 draw, at most 299 in u32; reassess if the offset or draw bound changes."
)]
fn unknown_definition(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    let invocation = candidate
        .nodes
        .iter_mut()
        .find(|node| matches!(node, noble_kernel::untrusted::Node::Invocation { .. }));
    if let Some(noble_kernel::untrusted::Node::Invocation { def, .. }) = invocation {
        *def = noble_kernel::contracts::Definition(100 + rng.below_u32(200)?);
        return Ok(());
    }
    let index = u32::try_from(candidate.nodes.len())
        .map_err(|error| format!("unknown-definition arena index exceeds u32: {error}"))?;
    candidate
        .nodes
        .push(noble_kernel::untrusted::Node::Invocation {
            def: noble_kernel::contracts::Definition(u32::MAX),
            inst: noble_kernel::words::Inst {
                bindings: vec![noble_kernel::words::Binding::Stack(Vec::new())],
            },
        });
    candidate.body.push(noble_kernel::untrusted::NodeId(index));
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; depth_chain builds the fixed twelve-quotation corruption exceeding the generated depth limit of eight; the malformed lane checks nonacceptance without asserting this intentionally invalid fixture."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; level is drawn from 0..12u32, so each next-level index is at most twelve; reassess if the fixed chain range changes."
)]
#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-maintainers; each quotation owns its empty body/stack vectors, and Vec::new allocates no buffer; hoisting one vector cannot represent the independently retained node fields."
)]
fn depth_chain(candidate: &mut noble_kernel::untrusted::Candidate) {
    // Twelve nested quotations exceed the declared depth limit of eight.
    let mut nodes = Vec::with_capacity(12);
    for level in 0..12u32 {
        let body = if level + 1 < 12 {
            vec![noble_kernel::untrusted::NodeId(level + 1)]
        } else {
            Vec::new()
        };
        nodes.push(noble_kernel::untrusted::Node::Quotation {
            body,
            inst: noble_kernel::words::Inst {
                bindings: vec![
                    noble_kernel::words::Binding::Stack(Vec::new()),
                    noble_kernel::words::Binding::Stack(Vec::new()),
                    noble_kernel::words::Binding::Stack(Vec::new()),
                    noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty()),
                ],
            },
        });
    }
    candidate.nodes = nodes;
    candidate.body = vec![noble_kernel::untrusted::NodeId(0)];
}

fn truncate(candidate: &mut noble_kernel::untrusted::Candidate) -> Result<(), String> {
    // Always point one past the truncated arena, never at a surviving node.
    let keep = candidate.nodes.len() / 2;
    candidate.nodes.truncate(keep.max(1));
    let dangling = u32::try_from(candidate.nodes.len())
        .map_err(|error| format!("truncated arena length exceeds u32: {error}"))?;
    candidate.body = vec![noble_kernel::untrusted::NodeId(dangling)];
    Ok(())
}

fn raw_identifiers(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
) -> Result<(), String> {
    // Keep every original draw's low four bytes; the format remains foreign.
    candidate.format = rng.next_u32() | 0x8000_0000;
    candidate.revision = rng.next_u32();
    for node in &mut candidate.nodes {
        if let noble_kernel::untrusted::Node::Invocation { def, .. } = node {
            *def = noble_kernel::contracts::Definition(rng.next_u32() | 0x8000_0000);
        }
    }
    let last = candidate
        .body
        .last_mut()
        .ok_or_else(|| "raw-identifier corruption requires a nonempty body".to_owned())?;
    *last = noble_kernel::untrusted::NodeId(rng.next_u32());
    Ok(())
}

const fn inst_of(node: &mut noble_kernel::untrusted::Node) -> &mut noble_kernel::words::Inst {
    match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. }
        | noble_kernel::untrusted::Node::Quotation { inst, .. } => inst,
    }
}
