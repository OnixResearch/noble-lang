//! Witnesses for program construction, composition, and invocation.

pub(super) fn dip(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let (a, program) = super::top_two(now)?;
    let (input, output, set) = super::as_program(&program)?;
    if input != super::under_two(now) {
        return None;
    }
    let ids = super::ids_of(&set);
    let next = [output.as_slice(), &[a.clone()][..]].concat();
    Some((
        vec![
            super::stack(input.clone()),
            super::value(a),
            super::stack(output.clone()),
            super::effects(&ids),
        ],
        next,
        ids,
    ))
}

pub(super) fn quote(
    rng: &mut super::super::rng::Stream,
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let a = super::top(now)?;
    let inner = super::super::gen::small_stack(rng);
    let mut body_out = inner.clone();
    body_out.push(a.clone());
    let program = super::program_of(&inner, &body_out, &noble_kernel::types::EffSet::empty());
    Some((
        vec![
            super::stack(super::under_one(now).to_vec()),
            super::value(a),
            super::stack(inner),
        ],
        super::push(super::under_one(now), program),
        vec![],
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; compose requires two program values and equal intermediate stacks through Option failures; noncomposable generator inputs must not become assertion panics."
)]
pub(super) fn compose(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let (deeper, upper) = super::top_two(now)?;
    let (a_input, mid, e_set) = super::as_program(&deeper)?;
    let (mid2, c_out, f_set) = super::as_program(&upper)?;
    if mid != mid2 {
        return None;
    }
    let union = e_set.union(&f_set);
    let program = super::program_of(&a_input, &c_out, &union);
    Some((
        vec![
            super::stack(super::under_two(now).to_vec()),
            super::stack(a_input),
            super::stack(mid),
            super::stack(c_out),
            super::effects(&super::ids_of(&e_set)),
            super::effects(&super::ids_of(&f_set)),
        ],
        super::push(super::under_two(now), program),
        vec![],
    ))
}

pub(super) fn apply(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let program = super::top(now)?;
    let (input, output, set) = super::as_program(&program)?;
    if input != super::under_one(now) {
        return None;
    }
    let ids = super::ids_of(&set);
    Some((
        vec![
            super::stack(input),
            super::stack(output.clone()),
            super::effects(&ids),
        ],
        output,
        ids,
    ))
}

pub(super) fn reflect(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let program = super::top(now)?;
    let (input, output, set) = super::as_program(&program)?;
    Some((
        vec![
            super::stack(super::under_one(now).to_vec()),
            super::stack(input),
            super::stack(output),
            super::effects(&super::ids_of(&set)),
        ],
        super::push(super::under_one(now), noble_kernel::types::Ty::Syntax),
        vec![],
    ))
}
