//! Complete discriminator/quotation/eliminator patterns, in arena order.

struct Pattern {
    under: Vec<noble_kernel::types::Ty>,
    left: noble_kernel::types::Ty,
    right: noble_kernel::types::Ty,
    which: u64,
}

pub(super) type Fragment = (
    Vec<noble_kernel::untrusted::NodeId>,
    Vec<noble_kernel::types::Ty>,
    Vec<u32>,
);

/// Both quotations retain their original claimed interface and effect bounds.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; build validates the eliminator selector and propagates arena/branch failures while constructing both claimed quotation interfaces; the independent oracle checks their semantics."
)]
pub(super) fn build(
    rng: &mut crate::rng::Stream,
    arena: &mut crate::gen::storage::Arena,
    now: Vec<noble_kernel::types::Ty>,
    which: u64,
) -> Result<Fragment, String> {
    let index = usize::try_from(which)
        .map_err(|error| format!("eliminator index exceeds usize: {error}"))?;
    let counter = crate::gen::ELIMINATORS
        .get(index)
        .ok_or_else(|| format!("unknown generated eliminator {which}"))?;
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let pattern = crate::gen::eliminators::Pattern {
        under: now,
        left: crate::gen::types::base(rng.below(4)),
        right: crate::gen::types::base(rng.below(4)),
        which,
    };
    let final_stack =
        crate::gen::eliminators::stack_with(&pattern.under, [noble_kernel::types::Ty::Unit]);
    let (disc, start1, start2) = pattern.discriminator(rng, arena)?;
    let around = crate::gen::eliminators::stack_with(&pattern.under, [pattern.discriminant()]);
    let body1 = crate::gen::eliminators::emitting(arena, &start1, which == 0)?;
    let drops = match which {
        0 => 1,
        1 => 0,
        _ => 2,
    };
    let body2 = crate::gen::eliminators::quiet(arena, &start2, drops)?;
    let quote1 = arena.quotation(
        body1,
        crate::gen::storage::Claim {
            around: &around,
            start: &start1,
            end: &final_stack,
            effects: &[0],
        },
    )?;
    let quote2 = arena.quotation(
        body2,
        crate::gen::storage::Claim {
            around: &around,
            start: &start2,
            end: &final_stack,
            effects: &[],
        },
    )?;
    let (word, bindings) = pattern.invocation(&final_stack);
    let invocation = arena.word(word, bindings)?;
    Ok((vec![disc, quote1, quote2, invocation], final_stack, vec![0]))
}

impl Pattern {
    fn discriminator(
        &self,
        rng: &mut crate::rng::Stream,
        arena: &mut crate::gen::storage::Arena,
    ) -> Result<
        (
            noble_kernel::untrusted::NodeId,
            Vec<noble_kernel::types::Ty>,
            Vec<noble_kernel::types::Ty>,
        ),
        String,
    > {
        match self.which {
            0 => {
                let disc = arena.word(
                    15,
                    vec![
                        crate::fit::stack(self.under.clone()),
                        crate::fit::value(self.left.clone()),
                        crate::fit::value(self.right.clone()),
                    ],
                )?;
                Ok((
                    disc,
                    crate::gen::eliminators::stack_with(&self.under, [self.left.clone()]),
                    crate::gen::eliminators::stack_with(&self.under, [self.right.clone()]),
                ))
            }
            1 => {
                let disc =
                    arena.literal(noble_kernel::untrusted::Lit::Bool(rng.bit()), &self.under)?;
                Ok((disc, self.under.clone(), self.under.clone()))
            }
            _ => {
                let disc = arena.word(
                    19,
                    vec![
                        crate::fit::stack(self.under.clone()),
                        crate::fit::value(self.left.clone()),
                    ],
                )?;
                let right = crate::gen::eliminators::stack_with(
                    &self.under,
                    [
                        self.left.clone(),
                        noble_kernel::types::Ty::List(Box::new(self.left.clone())),
                    ],
                );
                Ok((disc, self.under.clone(), right))
            }
        }
    }

    fn discriminant(&self) -> noble_kernel::types::Ty {
        match self.which {
            0 => noble_kernel::types::Ty::Sum(
                Box::new(self.left.clone()),
                Box::new(self.right.clone()),
            ),
            1 => noble_kernel::types::Ty::Bool,
            _ => noble_kernel::types::Ty::List(Box::new(self.left.clone())),
        }
    }

    fn invocation(
        &self,
        final_stack: &[noble_kernel::types::Ty],
    ) -> (u32, Vec<noble_kernel::words::Binding>) {
        let (word, count) = match self.which {
            0 => (17, 6),
            1 => (18, 4),
            _ => (21, 5),
        };
        let mut bindings = Vec::with_capacity(count);
        bindings.push(crate::fit::stack(self.under.clone()));
        if self.which != 1 {
            bindings.push(crate::fit::value(self.left.clone()));
        }
        if self.which == 0 {
            bindings.push(crate::fit::value(self.right.clone()));
        }
        bindings.push(crate::fit::stack(final_stack.to_vec()));
        bindings.push(crate::fit::effects(&[0]));
        bindings.push(crate::fit::effects(&[]));
        (word, bindings)
    }
}

fn stack_with<const COUNT: usize>(
    under: &[noble_kernel::types::Ty],
    extra: [noble_kernel::types::Ty; COUNT],
) -> Vec<noble_kernel::types::Ty> {
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; under is a slice of nonzero-sized Ty values and every extra array has one or two entries, so under.len() + COUNT fits usize; reassess if array counts or representation change."
    )]
    let mut stack = Vec::with_capacity(under.len() + COUNT);
    stack.extend_from_slice(under);
    stack.extend(extra);
    stack
}

fn emitting(
    arena: &mut crate::gen::storage::Arena,
    start: &[noble_kernel::types::Ty],
    has_payload: bool,
) -> Result<Vec<noble_kernel::untrusted::NodeId>, String> {
    let mut body = Vec::with_capacity(if has_payload { 3 } else { 2 });
    let mut cursor = start.to_vec();
    if has_payload {
        let top = cursor
            .pop()
            .ok_or_else(|| "case emitting branch is missing its payload".to_owned())?;
        body.push(arena.word(
            1,
            vec![crate::fit::stack(cursor.clone()), crate::fit::value(top)],
        )?);
    }
    body.push(arena.literal(noble_kernel::untrusted::Lit::Text, &cursor)?);
    cursor.reserve(1);
    cursor.push(noble_kernel::types::Ty::Text);
    body.push(arena.word(22, vec![crate::fit::stack(cursor)])?);
    Ok(body)
}

fn quiet(
    arena: &mut crate::gen::storage::Arena,
    start: &[noble_kernel::types::Ty],
    drops: usize,
) -> Result<Vec<noble_kernel::untrusted::NodeId>, String> {
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; the sole caller selects drops as 0, 1 or 2 from the eliminator kind, so one final literal requires at most three body entries; reassess if that caller changes."
    )]
    let mut body = Vec::with_capacity(drops + 1);
    let mut cursor = start.to_vec();
    for _ in 0..drops {
        let top = cursor
            .pop()
            .ok_or_else(|| "quiet eliminator branch is missing its payload".to_owned())?;
        body.push(arena.word(
            1,
            vec![crate::fit::stack(cursor.clone()), crate::fit::value(top)],
        )?);
    }
    body.push(arena.literal(noble_kernel::untrusted::Lit::Unit, &cursor)?);
    Ok(body)
}
