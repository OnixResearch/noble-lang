//! Witnesses for primitive stack and scalar words.

pub(super) fn copy_or_drop(
    now: &[noble_kernel::types::Ty],
    def: u32,
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let a = super::top(now)?;
    let next = if def == 0 {
        super::push(now, a.clone())
    } else {
        super::drop_one(now)?
    };
    Some((
        vec![super::stack(now.to_vec()), super::value(a)],
        next,
        vec![],
    ))
}

pub(super) fn swap(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let (a, b) = super::top_two(now)?;
    let next = [super::under_two(now), &[b.clone(), a.clone()][..]].concat();
    Some((
        vec![
            super::stack(super::under_two(now).to_vec()),
            super::value(a),
            super::value(b),
        ],
        next,
        vec![],
    ))
}

pub(super) fn integer_result(
    now: &[noble_kernel::types::Ty],
    output: noble_kernel::types::Ty,
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    let (a, b) = super::top_two(now)?;
    if !matches!(
        (a, b),
        (noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64)
    ) {
        return None;
    }
    Some((
        vec![super::stack(super::under_two(now).to_vec())],
        super::push(super::under_two(now), output),
        vec![],
    ))
}

pub(super) fn print(
    now: &[noble_kernel::types::Ty],
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    if !matches!(super::top(now)?, noble_kernel::types::Ty::Text) {
        return None;
    }
    Some((
        vec![super::stack(super::under_one(now).to_vec())],
        super::push(super::under_one(now), noble_kernel::types::Ty::Unit),
        vec![0],
    ))
}
