//! The oracle's own type representation and set operations (DX-PROPERTY-01).
//!
//! Independent of `types::Ty`: a plain data mirror the decision function
//! works over, with its own `Data` predicate and effect-set unions written
//! from the fragment's documented rules.

/// The oracle's verdict on one candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The candidate derives at the expected interface inside the bound.
    Accept,
    /// The candidate does not derive (any rejection cause).
    Reject,
}

/// The oracle's own type representation, independent of `types::Ty`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Unit,
    Bool,
    I64,
    Text,
    Syntax,
    Pair(Box<Type>, Box<Type>),
    Sum(Box<Type>, Box<Type>),
    List(Box<Type>),
    /// Required stack, result stack, latent identities.
    Program(Vec<Type>, Vec<Type>, Vec<u32>),
    Resource,
}

/// Each pending destination is filled exactly once before the mirror is
/// returned. This explicit work stack avoids recursive host calls.
struct Mirror<'a> {
    pending: Vec<(&'a noble_kernel::types::Ty, &'a mut Type)>,
}

impl<'a> Mirror<'a> {
    fn visit(&mut self, source: &'a noble_kernel::types::Ty, destination: &'a mut Type) {
        match source {
            noble_kernel::types::Ty::Unit => *destination = Type::Unit,
            noble_kernel::types::Ty::Bool => *destination = Type::Bool,
            noble_kernel::types::Ty::I64 => *destination = Type::I64,
            noble_kernel::types::Ty::Text => *destination = Type::Text,
            noble_kernel::types::Ty::Syntax => *destination = Type::Syntax,
            noble_kernel::types::Ty::Resource(_) => *destination = Type::Resource,
            noble_kernel::types::Ty::Pair(left, right) => {
                *destination = Type::Pair(Box::new(Type::Unit), Box::new(Type::Unit));
                if let Type::Pair(first, second) = destination {
                    self.pending.reserve(2);
                    self.pending.push((right, second));
                    self.pending.push((left, first));
                }
            }
            noble_kernel::types::Ty::Sum(left, right) => {
                *destination = Type::Sum(Box::new(Type::Unit), Box::new(Type::Unit));
                if let Type::Sum(first, second) = destination {
                    self.pending.reserve(2);
                    self.pending.push((right, second));
                    self.pending.push((left, first));
                }
            }
            noble_kernel::types::Ty::List(item) => {
                *destination = Type::List(Box::new(Type::Unit));
                if let Type::List(element) = destination {
                    self.pending.reserve(1);
                    self.pending.push((item, element));
                }
            }
            noble_kernel::types::Ty::Program(input, output, effects) => {
                *destination = Type::Program(
                    vec![Type::Unit; input.len()],
                    vec![Type::Unit; output.len()],
                    effects.as_slice().iter().map(|id| id.0).collect(),
                );
                if let Type::Program(start, end, _) = destination {
                    #[expect(
                        tigerstyle::raw_arithmetic_overflow,
                        reason = "Owner: noble-maintainers; input and output are vectors of nonzero-sized Ty values, each length at most isize::MAX, so their combined child count fits usize; reassess if representation changes."
                    )]
                    self.pending.reserve(input.len() + output.len());
                    self.pending.extend(output.iter().zip(end.iter_mut()));
                    self.pending.extend(input.iter().zip(start.iter_mut()));
                }
            }
        }
    }
}

/// Mirror one kernel input type into the oracle's representation.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; oty is a total structural mirror of owned input types, filling every scheduled destination once; the independent oracle consumes the result without assuming input eligibility."
)]
pub fn oty(ty: &noble_kernel::types::Ty) -> Type {
    match ty {
        noble_kernel::types::Ty::Unit => return Type::Unit,
        noble_kernel::types::Ty::Bool => return Type::Bool,
        noble_kernel::types::Ty::I64 => return Type::I64,
        noble_kernel::types::Ty::Text => return Type::Text,
        noble_kernel::types::Ty::Syntax => return Type::Syntax,
        noble_kernel::types::Ty::Resource(_) => return Type::Resource,
        noble_kernel::types::Ty::Pair(..)
        | noble_kernel::types::Ty::Sum(..)
        | noble_kernel::types::Ty::List(_)
        | noble_kernel::types::Ty::Program(..) => {}
    }
    let mut mirrored = Type::Unit;
    let mut walk = Mirror {
        pending: vec![(ty, &mut mirrored)],
    };
    while let Some((source, destination)) = walk.pending.pop() {
        walk.visit(source, destination);
    }
    mirrored
}

/// Mirror one whole stack.
pub fn oty_stack(stack: &[noble_kernel::types::Ty]) -> Vec<Type> {
    stack.iter().map(oty).collect()
}

/// The documented `Data` predicate: a resource anywhere makes the value
/// ineligible, including through pair, sum, and list payloads. A program's
/// interface is not its captured value, so program types are eligible.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; is_data is the independent total predicate: resources reject, pair/sum/list children are traversed and program interfaces remain eligible; assertions would not strengthen that decision contract."
)]
pub fn is_data(ty: &Type) -> bool {
    match ty {
        Type::Resource => return false,
        Type::Unit | Type::Bool | Type::I64 | Type::Text | Type::Syntax | Type::Program(..) => {
            return true;
        }
        Type::Pair(..) | Type::Sum(..) | Type::List(_) => {}
    }
    let mut pending = vec![ty];
    while let Some(next) = pending.pop() {
        match next {
            Type::Resource => return false,
            Type::Pair(left, right) | Type::Sum(left, right) => {
                pending.reserve(2);
                pending.push(right);
                pending.push(left);
            }
            Type::List(item) => {
                pending.reserve(1);
                pending.push(item);
            }
            Type::Unit | Type::Bool | Type::I64 | Type::Text | Type::Syntax | Type::Program(..) => {
            }
        }
    }
    true
}

/// Sorted, deduplicated union of two sorted identity lists.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; union merges canonical identity lists with bounds-guarded cursors, consuming at least one input per step; its result is independently compared through oracle acceptance, not assertion padding."
)]
pub fn union(left: &[u32], right: &[u32]) -> Vec<u32> {
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; both inputs are u32 slices whose byte sizes are at most isize::MAX, so the sum of their element counts fits usize; reassess if element representation changes."
    )]
    let mut out = Vec::with_capacity(left.len() + right.len());
    let (mut i, mut j) = (0, 0);
    while i < left.len() || j < right.len() {
        let should_take_left = j == right.len() || (i < left.len() && left[i] < right[j]);
        let should_take_right = i == left.len() || (j < right.len() && right[j] < left[i]);
        if should_take_left {
            out.push(left[i]);
            i += 1;
        } else if should_take_right {
            out.push(right[j]);
            j += 1;
        } else {
            out.push(left[i]);
            i += 1;
            j += 1;
        }
    }
    out
}

/// Whether every identity of `small` is present in `large`.
pub fn is_subset(small: &[u32], large: &[u32]) -> bool {
    small.iter().all(|id| large.contains(id))
}
