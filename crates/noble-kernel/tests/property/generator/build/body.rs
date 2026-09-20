//! Iterative depth-first body construction with suspended parent frames.

pub(super) struct Frame {
    pub ids: Vec<noble_kernel::untrusted::NodeId>,
    pub now: Vec<noble_kernel::types::Ty>,
    pub latent: Vec<u32>,
    depth: u64,
    remaining: u64,
}

struct Suspended {
    parent: crate::gen::body::Frame,
    start: Vec<noble_kernel::types::Ty>,
}

impl Frame {
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-maintainers; depth and steps are same-domain u64 generation counts, assigned directly to named depth/remaining fields; retaining this interface preserves the stream's count domain without wrapper conversions."
    )]
    fn new(now: Vec<noble_kernel::types::Ty>, depth: u64, steps: u64) -> Result<Self, String> {
        let count = usize::try_from(steps)
            .map_err(|error| format!("body step count exceeds usize: {error}"))?;
        let body_node_count = count
            .checked_mul(4)
            .ok_or_else(|| "body node-reference capacity exceeds usize".to_owned())?;
        Ok(Self {
            ids: Vec::with_capacity(body_node_count),
            now,
            // Every pool word and eliminator adds at most the single emit identity.
            latent: Vec::with_capacity(count),
            depth,
            remaining: steps,
        })
    }

    fn emit_word(
        &mut self,
        rng: &mut crate::rng::Stream,
        arena: &mut crate::gen::storage::Arena,
    ) -> Result<(), String> {
        for _ in 0..5 {
            let def = crate::fit::WORDS[rng.index(crate::fit::WORDS.len())?];
            if let Some((bindings, next, add)) = crate::fit::witness(rng, &self.now, def) {
                self.ids.push(arena.word(def, bindings)?);
                self.now = next;
                self.latent.extend(add);
                return Ok(());
            }
        }
        let lit = crate::gen::types::literal(rng)?;
        self.ids.push(arena.literal(lit, &self.now)?);
        self.now.reserve(1);
        self.now.push(lit.ty());
        Ok(())
    }

    fn finish_quote(
        &mut self,
        rng: &mut crate::rng::Stream,
        arena: &mut crate::gen::storage::Arena,
        start: Vec<noble_kernel::types::Ty>,
        child: crate::gen::body::Frame,
    ) -> Result<(), String> {
        // This draw occurs only after all child steps, never at suspension.
        let claimed = if !child.latent.is_empty() && rng.below(4) != 0 {
            child.latent
        } else {
            Vec::new()
        };
        let set = noble_kernel::types::EffSet::from_ids(
            &claimed
                .iter()
                .map(|id| noble_kernel::types::EffId(*id))
                .collect::<Vec<_>>(),
        );
        self.ids.push(arena.quotation(
            child.ids,
            crate::gen::storage::Claim {
                around: &self.now,
                start: &start,
                end: &child.now,
                effects: &claimed,
            },
        )?);
        self.now.reserve(1);
        self.now
            .push(noble_kernel::types::Ty::program(start, child.now, set));
        Ok(())
    }
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; depth and steps are u64 generation counts passed directly to the named Frame fields, not distinct identifier domains; reassess if callers stop passing explicit depth and step budgets."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; build propagates count conversion and arena-id failures and preserves depth-first random draws across suspended frames; the independent oracle checks generated bodies, not redundant constructor assertions."
)]
pub(super) fn build(
    rng: &mut crate::rng::Stream,
    arena: &mut crate::gen::storage::Arena,
    start: Vec<noble_kernel::types::Ty>,
    depth: u64,
    steps: u64,
) -> Result<crate::gen::body::Frame, String> {
    let frame_count = usize::try_from(depth)
        .map_err(|error| format!("body frame depth exceeds usize: {error}"))?;
    let mut pending: Vec<crate::gen::body::Suspended> = Vec::with_capacity(frame_count);
    let mut current = crate::gen::body::Frame::new(start, depth, steps)?;
    loop {
        if current.remaining == 0 {
            let Some(mut suspended) = pending.pop() else {
                return Ok(current);
            };
            suspended
                .parent
                .finish_quote(rng, arena, suspended.start, current)?;
            current = suspended.parent;
            continue;
        }
        current.remaining -= 1;
        if current.depth > 0 && rng.below(11) == 0 {
            let which = rng.below(3);
            let (ids, next, add) =
                crate::gen::eliminators::build(rng, arena, current.now.clone(), which)?;
            current.ids.extend(ids);
            current.now = next;
            current.latent.extend(add);
            continue;
        }
        if current.depth > 0 && rng.below(5) == 0 {
            let inner_start = crate::gen::small_stack(rng);
            let inner_steps = rng.below(4);
            #[expect(
                tigerstyle::raw_arithmetic_overflow,
                reason = "Owner: noble-maintainers; this quotation-descent branch is guarded by current.depth > 0, so reducing child depth by one cannot underflow."
            )]
            let child =
                crate::gen::body::Frame::new(inner_start.clone(), current.depth - 1, inner_steps)?;
            pending.push(crate::gen::body::Suspended {
                parent: current,
                start: inner_start,
            });
            current = child;
            continue;
        }
        current.emit_word(rng, arena)?;
    }
}
