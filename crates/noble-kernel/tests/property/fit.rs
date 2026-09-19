//! Word fitting for the candidate generator: derive one word's witness and
//! stack effect from the simulated stack, exactly as the documented contract
//! table prescribes. `None` means the word does not fit the stack now.

use noble_kernel::types::{EffId, EffSet, Ty};
use noble_kernel::words::Binding;

use super::gen::{small_stack, ty};
use super::rng::Rng;

/// The word pool the generator emits, as definition ids.
pub const WORDS: &[u32] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

pub fn stack(segment: Vec<Ty>) -> Binding {
    Binding::Stack(segment)
}

pub fn value(ty: Ty) -> Binding {
    Binding::Value(ty)
}

pub fn effects(list: &[u32]) -> Binding {
    let ids: Vec<EffId> = list.iter().map(|id| EffId(*id)).collect();
    Binding::Effect(EffSet::from_ids(&ids))
}

pub fn ids_of(set: &EffSet) -> Vec<u32> {
    set.as_slice().iter().map(|id| id.0).collect()
}

fn program_of(input: &[Ty], output: &[Ty], set: &EffSet) -> Ty {
    Ty::program(input.to_vec(), output.to_vec(), set.clone())
}

fn as_program(ty: &Ty) -> Option<(Vec<Ty>, Vec<Ty>, EffSet)> {
    match ty {
        Ty::Program(input, output, set) => {
            Some((input.as_ref().clone(), output.as_ref().clone(), set.clone()))
        }
        _ => None,
    }
}

fn top(stack: &[Ty]) -> Option<Ty> {
    stack.last().cloned()
}

fn top_two(stack: &[Ty]) -> Option<(Ty, Ty)> {
    if stack.len() < 2 {
        return None;
    }
    Some((
        stack[stack.len() - 2].clone(),
        stack[stack.len() - 1].clone(),
    ))
}

fn under_two(stack: &[Ty]) -> &[Ty] {
    &stack[..stack.len() - 2]
}

fn under_one(stack: &[Ty]) -> &[Ty] {
    &stack[..stack.len() - 1]
}

/// Fit one word: its witness bindings, the next simulated stack, and the
/// latent identities its invocation adds to the frame.
pub fn fit_word(rng: &mut Rng, now: &[Ty], def: u32) -> Option<(Vec<Binding>, Vec<Ty>, Vec<u32>)> {
    match def {
        0 | 1 => {
            let a = top(now)?;
            let next = if def == 0 {
                push(now, a.clone())
            } else {
                drop_one(now)?
            };
            Some((vec![stack(now.to_vec()), value(a)], next, vec![]))
        }
        2 => {
            let (a, b) = top_two(now)?;
            let next = [under_two(now), &[b.clone(), a.clone()][..]].concat();
            Some((
                vec![stack(under_two(now).to_vec()), value(a), value(b)],
                next,
                vec![],
            ))
        }
        3 => {
            let (a, program) = top_two(now)?;
            let (input, output, set) = as_program(&program)?;
            if input != under_two(now) {
                return None;
            }
            let ids = ids_of(&set);
            let next = [output.as_slice(), &[a.clone()][..]].concat();
            Some((
                vec![
                    stack(input.clone()),
                    value(a),
                    stack(output.clone()),
                    effects(&ids),
                ],
                next,
                ids,
            ))
        }
        4..=6 => {
            let (a, b) = top_two(now)?;
            if !matches!((a, b), (Ty::I64, Ty::I64)) {
                return None;
            }
            Some((
                vec![stack(under_two(now).to_vec())],
                push(under_two(now), Ty::I64),
                vec![],
            ))
        }
        7 => {
            let (a, b) = top_two(now)?;
            if !matches!((a, b), (Ty::I64, Ty::I64)) {
                return None;
            }
            Some((
                vec![stack(under_two(now).to_vec())],
                push(under_two(now), Ty::Bool),
                vec![],
            ))
        }
        8 => {
            let a = top(now)?;
            let inner = small_stack(rng);
            let mut body_out = inner.clone();
            body_out.push(a.clone());
            let program = program_of(&inner, &body_out, &EffSet::empty());
            Some((
                vec![stack(under_one(now).to_vec()), value(a), stack(inner)],
                push(under_one(now), program),
                vec![],
            ))
        }
        9 => {
            let (deeper, upper) = top_two(now)?;
            let (a_input, mid, e_set) = as_program(&deeper)?;
            let (mid2, c_out, f_set) = as_program(&upper)?;
            if mid != mid2 {
                return None;
            }
            let union = e_set.union(&f_set);
            let program = program_of(&a_input, &c_out, &union);
            Some((
                vec![
                    stack(under_two(now).to_vec()),
                    stack(a_input),
                    stack(mid),
                    stack(c_out),
                    effects(&ids_of(&e_set)),
                    effects(&ids_of(&f_set)),
                ],
                push(under_two(now), program),
                vec![],
            ))
        }
        10 => {
            let program = top(now)?;
            let (input, output, set) = as_program(&program)?;
            if input != under_one(now) {
                return None;
            }
            let ids = ids_of(&set);
            Some((
                vec![stack(input), stack(output.clone()), effects(&ids)],
                output,
                ids,
            ))
        }
        11 => {
            let program = top(now)?;
            let (input, output, set) = as_program(&program)?;
            Some((
                vec![
                    stack(under_one(now).to_vec()),
                    stack(input),
                    stack(output),
                    effects(&ids_of(&set)),
                ],
                push(under_one(now), Ty::Syntax),
                vec![],
            ))
        }
        12 => Some((vec![stack(now.to_vec())], push(now, Ty::Unit), vec![])),
        13 => {
            let (a, b) = top_two(now)?;
            let joined = Ty::Pair(Box::new(a.clone()), Box::new(b.clone()));
            Some((
                vec![stack(under_two(now).to_vec()), value(a), value(b)],
                push(under_two(now), joined),
                vec![],
            ))
        }
        14 => {
            let joined = top(now)?;
            let (a, b) = match joined {
                Ty::Pair(left, right) => ((*left).clone(), (*right).clone()),
                _ => return None,
            };
            let next = [under_one(now), &[a.clone(), b.clone()][..]].concat();
            Some((
                vec![stack(under_one(now).to_vec()), value(a), value(b)],
                next,
                vec![],
            ))
        }
        15 | 16 => {
            let upper = top(now)?;
            let other = ty(rng, 0);
            let (a, b) = if def == 15 {
                (upper, other)
            } else {
                (other, upper)
            };
            let joined = Ty::Sum(Box::new(a.clone()), Box::new(b.clone()));
            Some((
                vec![stack(under_one(now).to_vec()), value(a), value(b)],
                push(under_one(now), joined),
                vec![],
            ))
        }
        17 => {
            if now.len() < 3 {
                return None;
            }
            let (a, b) = match &now[now.len() - 3] {
                Ty::Sum(left, right) => ((**left).clone(), (**right).clone()),
                _ => return None,
            };
            let (p1_in, p1_out, e_set) = as_program(&now[now.len() - 2])?;
            let (p2_in, p2_out, f_set) = as_program(&top(now)?)?;
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
                    stack(under.to_vec()),
                    value(a),
                    value(b),
                    stack(p1_out.clone()),
                    effects(&ids_of(&e_set)),
                    effects(&ids_of(&f_set)),
                ],
                next,
                ids_of(&union),
            ))
        }
        18 => {
            if now.len() < 3 {
                return None;
            }
            if !matches!(now[now.len() - 3], Ty::Bool) {
                return None;
            }
            let (a_input, t_out, e_set) = as_program(&now[now.len() - 2])?;
            let (a2, t2, f_set) = as_program(&top(now)?)?;
            if a_input != a2 || t_out != t2 {
                return None;
            }
            let union = e_set.union(&f_set);
            let next = [under_two(now), t_out.as_slice()].concat();
            Some((
                vec![
                    stack(a_input.clone()),
                    stack(t_out.clone()),
                    effects(&ids_of(&e_set)),
                    effects(&ids_of(&f_set)),
                ],
                next,
                ids_of(&union),
            ))
        }
        19 => {
            let item = ty(rng, 0);
            let joined = Ty::List(Box::new(item.clone()));
            Some((
                vec![stack(now.to_vec()), value(item)],
                push(now, joined),
                vec![],
            ))
        }
        20 => {
            let (item, listed) = top_two(now)?;
            let inner = match listed {
                Ty::List(element) => (*element).clone(),
                _ => return None,
            };
            if item != inner {
                return None;
            }
            let joined = Ty::List(Box::new(item.clone()));
            Some((
                vec![stack(under_two(now).to_vec()), value(item)],
                push(under_two(now), joined),
                vec![],
            ))
        }
        21 => {
            if now.len() < 3 {
                return None;
            }
            let a = match &now[now.len() - 3] {
                Ty::List(item) => (**item).clone(),
                _ => return None,
            };
            let (p1_in, p1_out, e_set) = as_program(&now[now.len() - 2])?;
            let (p2_in, p2_out, f_set) = as_program(&top(now)?)?;
            let under = &now[..now.len() - 3];
            if p1_in != under {
                return None;
            }
            let mut want2 = under.to_vec();
            want2.push(a.clone());
            want2.push(Ty::List(Box::new(a.clone())));
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
                    stack(under.to_vec()),
                    value(a),
                    stack(p1_out.clone()),
                    effects(&ids_of(&e_set)),
                    effects(&ids_of(&f_set)),
                ],
                next,
                ids_of(&union),
            ))
        }
        22 => {
            if !matches!(top(now)?, Ty::Text) {
                return None;
            }
            Some((
                vec![stack(under_one(now).to_vec())],
                push(under_one(now), Ty::Unit),
                vec![0],
            ))
        }
        23 => {
            let resource = Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
            Some((vec![stack(now.to_vec())], push(now, resource), vec![]))
        }
        _ => None,
    }
}

fn push(stack: &[Ty], tail: Ty) -> Vec<Ty> {
    let mut next = stack.to_vec();
    next.push(tail);
    next
}

fn drop_one(stack: &[Ty]) -> Option<Vec<Ty>> {
    if stack.is_empty() {
        None
    } else {
        Some(stack[..stack.len() - 1].to_vec())
    }
}
