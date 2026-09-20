//! Word fitting for the candidate generator: derive one word's witness and
//! stack effect from the simulated stack, exactly as the documented contract
//! table prescribes. `None` means the word does not fit the stack now.

#[path = "fit/branches.rs"]
mod branches;
#[path = "fit/data.rs"]
mod data;
#[path = "fit/primitives.rs"]
mod primitives;
#[path = "fit/programs.rs"]
mod programs;

/// The word pool the generator emits, as definition ids.
pub const WORDS: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

pub fn stack(segment: std::vec::Vec<noble_kernel::types::Ty>) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Stack(segment)
}

pub fn value(ty: noble_kernel::types::Ty) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Value(ty)
}

pub fn effects(list: &[u32]) -> noble_kernel::words::Binding {
    let ids: std::vec::Vec<noble_kernel::types::EffId> = list
        .iter()
        .map(|id| noble_kernel::types::EffId(*id))
        .collect();
    noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::from_ids(&ids))
}

pub fn ids_of(set: &noble_kernel::types::EffSet) -> std::vec::Vec<u32> {
    set.as_slice().iter().map(|id| id.0).collect()
}

fn program_of(
    input: &[noble_kernel::types::Ty],
    output: &[noble_kernel::types::Ty],
    set: &noble_kernel::types::EffSet,
) -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::program(input.to_vec(), output.to_vec(), set.clone())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; as_program clones owned input/output vectors and the effect set for witness generation, whose Clone implementations are not const; reassess when owned cloning becomes const."
)]
fn as_program(
    ty: &noble_kernel::types::Ty,
) -> Option<(
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<noble_kernel::types::Ty>,
    noble_kernel::types::EffSet,
)> {
    match ty {
        noble_kernel::types::Ty::Program(input, output, set) => {
            Some((input.as_ref().clone(), output.as_ref().clone(), set.clone()))
        }
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Pair(_, _)
        | noble_kernel::types::Ty::Sum(_, _)
        | noble_kernel::types::Ty::List(_)
        | noble_kernel::types::Ty::Resource(_) => None,
    }
}

fn top(stack: &[noble_kernel::types::Ty]) -> Option<noble_kernel::types::Ty> {
    stack.last().cloned()
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; top_two returns cloned owned Ty values, including boxed/vector payloads, and Ty::clone is not const; reassess if the helper becomes a borrowed projection."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the initial length guard returns None below two entries, so both top-two index subtractions are bounded."
)]
fn top_two(
    stack: &[noble_kernel::types::Ty],
) -> Option<(noble_kernel::types::Ty, noble_kernel::types::Ty)> {
    if stack.len() < 2 {
        return None;
    }
    Some((
        stack[stack.len() - 2].clone(),
        stack[stack.len() - 1].clone(),
    ))
}

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; every caller first succeeds at top_two or guards for at least three branch inputs; reassess when adding a caller that has not established two entries."
)]
const fn under_two(stack: &[noble_kernel::types::Ty]) -> &[noble_kernel::types::Ty] {
    stack.split_at(stack.len() - 2).0
}

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; every caller first obtains the top entry through top(now)?, proving a nonempty stack; reassess if a new caller bypasses that proof."
)]
const fn under_one(stack: &[noble_kernel::types::Ty]) -> &[noble_kernel::types::Ty] {
    stack.split_at(stack.len() - 1).0
}

/// Fit one word: its witness bindings, the next simulated stack, and the
/// latent identities its invocation adds to the frame.
pub fn witness(
    rng: &mut super::rng::Stream,
    now: &[noble_kernel::types::Ty],
    def: u32,
) -> Option<(
    std::vec::Vec<noble_kernel::words::Binding>,
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<u32>,
)> {
    match def {
        0 | 1 => primitives::copy_or_drop(now, def),
        2 => primitives::swap(now),
        3 => programs::dip(now),
        4..=6 => primitives::integer_result(now, noble_kernel::types::Ty::I64),
        7 => primitives::integer_result(now, noble_kernel::types::Ty::Bool),
        8 => programs::quote(rng, now),
        9 => programs::compose(now),
        10 => programs::apply(now),
        11 => programs::reflect(now),
        12 => Some((
            vec![stack(now.to_vec())],
            push(now, noble_kernel::types::Ty::Unit),
            vec![],
        )),
        13 => data::pair(now),
        14 => data::unpair(now),
        15 | 16 => data::inject(rng, now, def),
        17 => branches::sum_cases(now),
        18 => branches::select(now),
        19 => data::empty_list(rng, now),
        20 => data::prepend(now),
        21 => branches::list_cases(now),
        22 => primitives::print(now),
        23 => {
            let resource =
                noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
            Some((vec![stack(now.to_vec())], push(now, resource), vec![]))
        }
        _ => None,
    }
}

fn push(
    stack: &[noble_kernel::types::Ty],
    tail: noble_kernel::types::Ty,
) -> std::vec::Vec<noble_kernel::types::Ty> {
    let mut next = stack.to_vec();
    next.push(tail);
    next
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; drop_one uses to_vec to clone the remaining owned types, which cannot run in const evaluation; reassess if its ownership contract changes."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the empty-stack arm returns None, so only a nonempty stack reaches the one-entry subtraction."
)]
fn drop_one(stack: &[noble_kernel::types::Ty]) -> Option<std::vec::Vec<noble_kernel::types::Ty>> {
    if stack.is_empty() {
        None
    } else {
        Some(stack[..stack.len() - 1].to_vec())
    }
}
