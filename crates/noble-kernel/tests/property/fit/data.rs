//! Witnesses for product, sum, and list construction.

pub(super) fn pair(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let (a, b) = super::top_two(now)?;
    let joined = noble_kernel::types::Ty::Pair(
        std::boxed::Box::new(a.clone()),
        std::boxed::Box::new(b.clone()),
    );
    Some((
        vec![
            super::stack(super::under_two(now).to_vec()),
            super::value(a),
            super::value(b),
        ],
        super::push(super::under_two(now), joined),
        vec![],
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; unpair first requires a top entry and returns None unless it is a Pair; the successful branch derives the witness and output without asserting a randomly generated input."
)]
pub(super) fn unpair(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let joined = super::top(now)?;
    let (a, b) = match joined {
        noble_kernel::types::Ty::Pair(left, right) => ((*left).clone(), (*right).clone()),
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Sum(_, _)
        | noble_kernel::types::Ty::List(_)
        | noble_kernel::types::Ty::Program(_, _, _)
        | noble_kernel::types::Ty::Resource(_) => return None,
    };
    let next = [super::under_one(now), &[a.clone(), b.clone()][..]].concat();
    Some((
        vec![
            super::stack(super::under_one(now).to_vec()),
            super::value(a),
            super::value(b),
        ],
        next,
        vec![],
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; inject returns None on an empty stack and otherwise generates the opposite sum payload in the existing draw order; the independent oracle checks the resulting witness."
)]
pub(super) fn inject(
    rng: &mut super::super::rng::Stream,
    now: &[noble_kernel::types::Ty],
    def: u32,
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let upper = super::top(now)?;
    let other = super::super::gen::ty(rng, 0);
    let (a, b) = if def == 15 {
        (upper, other)
    } else {
        (other, upper)
    };
    let joined = noble_kernel::types::Ty::Sum(
        std::boxed::Box::new(a.clone()),
        std::boxed::Box::new(b.clone()),
    );
    Some((
        vec![
            super::stack(super::under_one(now).to_vec()),
            super::value(a),
            super::value(b),
        ],
        super::push(super::under_one(now), joined),
        vec![],
    ))
}

pub(super) fn empty_list(
    rng: &mut super::super::rng::Stream,
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let item = super::super::gen::ty(rng, 0);
    let joined = noble_kernel::types::Ty::List(std::boxed::Box::new(item.clone()));
    Some((
        vec![super::stack(now.to_vec()), super::value(item)],
        super::push(now, joined),
        vec![],
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; prepend rejects insufficient entries, non-lists and element mismatches through None; nonfitting word candidates are normal generator control flow."
)]
pub(super) fn prepend(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let (item, listed) = super::top_two(now)?;
    let inner = match listed {
        noble_kernel::types::Ty::List(element) => (*element).clone(),
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Pair(_, _)
        | noble_kernel::types::Ty::Sum(_, _)
        | noble_kernel::types::Ty::Program(_, _, _)
        | noble_kernel::types::Ty::Resource(_) => return None,
    };
    if item != inner {
        return None;
    }
    let joined = noble_kernel::types::Ty::List(std::boxed::Box::new(item.clone()));
    Some((
        vec![
            super::stack(super::under_two(now).to_vec()),
            super::value(item),
        ],
        super::push(super::under_two(now), joined),
        vec![],
    ))
}
