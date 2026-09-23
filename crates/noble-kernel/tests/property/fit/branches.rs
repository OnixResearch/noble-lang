//! Witnesses for sum, boolean, and list branch combinators.

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; sum_cases returns None for insufficient inputs, non-sums, non-programs or incompatible branch interfaces; nonfitting generated words must not panic."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the initial now.len() < 3 guard returns before all two- and three-entry index subtractions."
)]
pub(super) fn sum_cases(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    if now.len() < 3 {
        return None;
    }
    let (a, b) = match &now[now.len() - 3] {
        noble_kernel::types::Ty::Sum(left, right) => ((**left).clone(), (**right).clone()),
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Pair(_, _)
        | noble_kernel::types::Ty::List(_)
        | noble_kernel::types::Ty::Program(_, _, _)
        | noble_kernel::types::Ty::Resource(_) => return None,
    };
    let (p1_in, p1_out, e_set) = super::as_program(&now[now.len() - 2])?;
    let (p2_in, p2_out, f_set) = super::as_program(&super::top(now)?)?;
    let under = &now[..now.len() - 3];
    let mut want1 = under.to_vec();
    want1.push(a.clone());
    if p1_in != want1 {
        return None;
    }
    let mut want2 = under.to_vec();
    want2.push(b.clone());
    if p2_in != want2 {
        return None;
    }
    if p1_out != p2_out {
        return None;
    }
    let union = e_set.union(&f_set);
    let next = [under, p1_out.as_slice()].concat();
    Some((
        vec![
            super::stack(under.to_vec()),
            super::value(a),
            super::value(b),
            super::stack(p1_out.clone()),
            super::effects(&super::ids_of(&e_set)),
            super::effects(&super::ids_of(&f_set)),
        ],
        next,
        super::ids_of(&union),
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; select returns None for a missing Bool or incompatible branch programs, preserving fallible word fitting instead of asserting generated stack shapes."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the initial three-entry length guard dominates both branch-input subtractions."
)]
pub(super) fn select(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    if now.len() < 3 {
        return None;
    }
    if !matches!(now[now.len() - 3], noble_kernel::types::Ty::Bool) {
        return None;
    }
    let (a_input, t_out, e_set) = super::as_program(&now[now.len() - 2])?;
    let (a2, t2, f_set) = super::as_program(&super::top(now)?)?;
    if a_input != a2 || t_out != t2 {
        return None;
    }
    let union = e_set.union(&f_set);
    let next = [super::under_two(now), t_out.as_slice()].concat();
    Some((
        vec![
            super::stack(a_input.clone()),
            super::stack(t_out.clone()),
            super::effects(&super::ids_of(&e_set)),
            super::effects(&super::ids_of(&f_set)),
        ],
        next,
        super::ids_of(&union),
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; list_cases returns None for missing list/program inputs and incompatible nil/cons interfaces; the generator must be able to try another word without panicking."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the initial length guard establishes at least three entries before every two- or three-entry subtraction."
)]
pub(super) fn list_cases(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    if now.len() < 3 {
        return None;
    }
    let a = match &now[now.len() - 3] {
        noble_kernel::types::Ty::List(item) => (**item).clone(),
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Pair(_, _)
        | noble_kernel::types::Ty::Sum(_, _)
        | noble_kernel::types::Ty::Program(_, _, _)
        | noble_kernel::types::Ty::Resource(_) => return None,
    };
    let (p1_in, p1_out, e_set) = super::as_program(&now[now.len() - 2])?;
    let (p2_in, p2_out, f_set) = super::as_program(&super::top(now)?)?;
    let under = &now[..now.len() - 3];
    if p1_in != under {
        return None;
    }
    let mut want2 = under.to_vec();
    want2.push(a.clone());
    want2.push(noble_kernel::types::Ty::List(std::boxed::Box::new(
        a.clone(),
    )));
    if p2_in != want2 {
        return None;
    }
    if p1_out != p2_out {
        return None;
    }
    let union = e_set.union(&f_set);
    let next = [under, p1_out.as_slice()].concat();
    Some((
        vec![
            super::stack(under.to_vec()),
            super::value(a),
            super::stack(p1_out.clone()),
            super::effects(&super::ids_of(&e_set)),
            super::effects(&super::ids_of(&f_set)),
        ],
        next,
        super::ids_of(&union),
    ))
}
