//! The oracle's own type representation and set operations (DX-PROPERTY-01).
//!
//! Independent of `types::Ty`: a plain data mirror the decision function
//! works over, with its own `Data` predicate and effect-set unions written
//! from the fragment's documented rules.

use noble_kernel::types::Ty;

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
pub enum OTy {
    Unit,
    Bool,
    I64,
    Text,
    Syntax,
    Pair(Box<OTy>, Box<OTy>),
    Sum(Box<OTy>, Box<OTy>),
    List(Box<OTy>),
    /// Required stack, result stack, latent identities.
    Program(Vec<OTy>, Vec<OTy>, Vec<u32>),
    Resource,
}

/// Mirror one kernel input type into the oracle's representation.
pub fn oty(ty: &Ty) -> OTy {
    match ty {
        Ty::Unit => OTy::Unit,
        Ty::Bool => OTy::Bool,
        Ty::I64 => OTy::I64,
        Ty::Text => OTy::Text,
        Ty::Syntax => OTy::Syntax,
        Ty::Pair(left, right) => OTy::Pair(Box::new(oty(left)), Box::new(oty(right))),
        Ty::Sum(left, right) => OTy::Sum(Box::new(oty(left)), Box::new(oty(right))),
        Ty::List(item) => OTy::List(Box::new(oty(item))),
        Ty::Program(input, output, effects) => OTy::Program(
            input.iter().map(oty).collect(),
            output.iter().map(oty).collect(),
            effects.as_slice().iter().map(|id| id.0).collect(),
        ),
        Ty::Resource(_) => OTy::Resource,
    }
}

/// Mirror one whole stack.
pub fn oty_stack(stack: &[Ty]) -> Vec<OTy> {
    stack.iter().map(oty).collect()
}

/// The documented `Data` predicate: a resource anywhere makes the value
/// ineligible, including through pair, sum, and list payloads.
pub fn is_data(ty: &OTy) -> bool {
    match ty {
        OTy::Resource => false,
        OTy::Pair(left, right) | OTy::Sum(left, right) => is_data(left) && is_data(right),
        OTy::List(item) => is_data(item),
        OTy::Unit | OTy::Bool | OTy::I64 | OTy::Text | OTy::Syntax | OTy::Program(..) => true,
    }
}

/// Sorted, deduplicated union of two sorted identity lists.
pub fn union(left: &[u32], right: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(left.len() + right.len());
    let (mut i, mut j) = (0, 0);
    while i < left.len() || j < right.len() {
        let take_left = j == right.len() || (i < left.len() && left[i] < right[j]);
        let take_right = i == left.len() || (j < right.len() && right[j] < left[i]);
        if take_left {
            out.push(left[i]);
            i += 1;
        } else if take_right {
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
