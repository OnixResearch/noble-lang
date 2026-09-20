//! Seeded generator of fragment candidates for the bounded property harness
//! (DX-PROPERTY-01). Deterministic from its seed; no time or environment
//! input. Candidates stay inside generous declared limits, so a correct
//! checker decides each one `accepted` or `invalid` — never exhausted.

#[path = "build/body.rs"]
mod body;
#[path = "build/eliminators.rs"]
mod eliminators;
#[path = "build/storage.rs"]
mod storage;
#[path = "build/types.rs"]
mod types;

/// One generated acceptance case.
pub struct Case {
    pub request: noble_kernel::untrusted::Request,
    pub candidate: noble_kernel::untrusted::Candidate,
}

/// Firing evidence for `case`, `if`, and `list.case`, respectively.
pub static ELIMINATORS: [std::sync::atomic::AtomicU64; 3] = [
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
];

/// Bootstrap plus fixture definition 23, producing the opaque resource.
pub fn environment() -> Result<noble_kernel::contracts::Env, String> {
    let mut env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    env.defs.push(noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![noble_kernel::shapes::Pattern::StackVar(
            noble_kernel::words::Variable(0),
        )],
        stack_out: vec![
            noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0)),
            noble_kernel::shapes::Pattern::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
        ],
        effects: vec![],
    });
    env.kinds.push(noble_kernel::contracts::Behavior::Named);
    env.deps.push(Vec::new());
    Ok(env)
}

/// A random type, evaluating constructor children in left-to-right DFS order.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; ty constructs owned types by a strictly descending depth and single-use continuations while preserving draw order; generated type semantics are checked by the independent oracle."
)]
pub fn ty(rng: &mut crate::rng::Stream, depth: u64) -> noble_kernel::types::Ty {
    let mut pending = Vec::new();
    let mut remaining = depth;
    'generate: loop {
        let mut value = match rng.below(if remaining == 0 { 5 } else { 11 }) {
            choice @ 0..=4 => crate::gen::types::base(choice),
            choice @ 5..=7 => {
                remaining -= 1;
                // Each descent needs exactly one continuation slot.
                pending.reserve(1);
                pending.push(match choice {
                    5 => crate::gen::types::Frame::PairRight(remaining),
                    6 => crate::gen::types::Frame::SumRight(remaining),
                    _ => crate::gen::types::Frame::List,
                });
                continue;
            }
            _ => noble_kernel::types::Ty::program(
                crate::gen::small_stack(rng),
                crate::gen::small_stack(rng),
                crate::gen::types::empty_or_emit(rng),
            ),
        };
        // Each pop finishes a continuation or starts its final child at a
        // strictly smaller depth. No continuation can restart a child twice.
        while let Some(frame) = pending.pop() {
            value = match frame {
                crate::gen::types::Frame::PairRight(child_depth) => {
                    pending.push(crate::gen::types::Frame::PairFinish(value));
                    remaining = child_depth;
                    continue 'generate;
                }
                crate::gen::types::Frame::SumRight(child_depth) => {
                    pending.push(crate::gen::types::Frame::SumFinish(value));
                    remaining = child_depth;
                    continue 'generate;
                }
                crate::gen::types::Frame::PairFinish(left) => {
                    noble_kernel::types::Ty::Pair(Box::new(left), Box::new(value))
                }
                crate::gen::types::Frame::SumFinish(left) => {
                    noble_kernel::types::Ty::Sum(Box::new(left), Box::new(value))
                }
                crate::gen::types::Frame::List => noble_kernel::types::Ty::List(Box::new(value)),
            };
        }
        return value;
    }
}

pub fn small_stack(rng: &mut crate::rng::Stream) -> Vec<noble_kernel::types::Ty> {
    (0..rng.below(3))
        .map(|_| crate::gen::types::base(rng.below(5)))
        .collect()
}

/// Probability of a post-generation witness or expectation corruption.
pub const KNOB_CHANCE: u64 = 45;

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; case composes fallible seeded body generation and optional claim corruption; the 1000-case differential lane checks its output, and malformed-lane generation intentionally remains fallible."
)]
pub fn case(rng: &mut crate::rng::Stream) -> Result<crate::gen::Case, String> {
    let mut arena = crate::gen::storage::Arena::new();
    let entry: Vec<noble_kernel::types::Ty> =
        (0..rng.below(3)).map(|_| crate::gen::ty(rng, 1)).collect();
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; Stream::below(6) is a modulo-bounded draw in 0..6, so adding one yields at most six u64 steps."
    )]
    let steps = 1 + rng.below(6);
    let frame = crate::gen::body::build(rng, &mut arena, entry.clone(), 2, steps)?;
    let allowed = crate::gen::allowed_effects(rng, &frame.latent);
    let mut request = noble_kernel::untrusted::Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in: entry,
            stack_out: frame.now,
            allowed_effects: noble_kernel::types::EffSet::from_ids(
                &allowed
                    .iter()
                    .map(|id| noble_kernel::types::EffId(*id))
                    .collect::<Vec<_>>(),
            ),
        },
        limits: crate::gen::limits(),
    };
    let mut candidate = noble_kernel::untrusted::Candidate {
        format: noble_kernel::untrusted::CANDIDATE_FORMAT,
        revision: noble_kernel::untrusted::SEMANTIC_REVISION,
        nodes: arena.nodes,
        body: frame.ids,
    };
    if rng.below(100) < crate::gen::KNOB_CHANCE {
        crate::knobs::apply(rng, &mut request, &mut candidate)?;
    }
    Ok(crate::gen::Case { request, candidate })
}

fn allowed_effects(rng: &mut crate::rng::Stream, latent: &[u32]) -> Vec<u32> {
    if latent.is_empty() {
        return if rng.bit() { vec![0] } else { Vec::new() };
    }
    if rng.below(10) < 7 {
        latent.to_vec()
    } else {
        Vec::new()
    }
}

pub fn limits() -> noble_kernel::untrusted::Limits {
    noble_kernel::untrusted::Limits {
        bytes: 1 << 16,
        nodes: 256,
        depth: 8,
        type_size: 64,
        stack_height: 16,
        work: 100_000,
        diagnostics: 64,
    }
}
