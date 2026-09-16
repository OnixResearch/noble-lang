//! Fragment v0 types: value, stack, and program types, effect sets, and the
//! recursive `Data` eligibility predicate.
//!
//! Stacks are ordered bottom-first: the last element is the stack top, which
//! matches the specification's "top on the right" notation.

use alloc::boxed::Box;
use alloc::vec::Vec;

/// Stable identity of a resource kind supplied by the environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceKind(pub u32);

/// Stable identity of one host-operation effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EffId(pub u32);

/// A finite set of effect identities, kept sorted and deduplicated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffSet(Vec<EffId>);

impl EffSet {
    /// The empty bound.
    pub fn empty() -> Self {
        EffSet(Vec::new())
    }

    /// Build from a list, sorting and deduplicating.
    pub fn from_ids(ids: &[EffId]) -> Self {
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        EffSet(sorted)
    }

    /// Membership.
    pub fn contains(&self, id: EffId) -> bool {
        self.0.binary_search(&id).is_ok()
    }

    /// Least upper bound. Union is associative, commutative, and idempotent.
    pub fn union(&self, other: &EffSet) -> EffSet {
        let mut out = Vec::with_capacity(self.0.len() + other.0.len());
        let (mut i, mut j) = (0usize, 0usize);
        while i < self.0.len() || j < other.0.len() {
            let take_left = j == other.0.len() || (i < self.0.len() && self.0[i] < other.0[j]);
            let take_right = i == self.0.len() || (j < other.0.len() && other.0[j] < self.0[i]);
            if take_left {
                out.push(self.0[i]);
                i += 1;
            } else if take_right {
                out.push(other.0[j]);
                j += 1;
            } else {
                out.push(self.0[i]);
                i += 1;
                j += 1;
            }
        }
        EffSet(out)
    }

    /// Inclusion: every identity here is present in `other`.
    pub fn is_subset_of(&self, other: &EffSet) -> bool {
        self.0.iter().all(|id| other.contains(*id))
    }

    /// The sorted identities.
    pub fn as_slice(&self) -> &[EffId] {
        &self.0
    }

    /// Number of identities.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A monomorphic program type: invocation stack `in`, result stack `out`, and
/// the latent effect bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramTy {
    /// Required invocation stack, bottom-first.
    pub stack_in: Vec<Ty>,
    /// Result stack, bottom-first.
    pub stack_out: Vec<Ty>,
    /// Latent host-operation bound.
    pub effects: EffSet,
}

/// A type in the fragment's finite universe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    /// The unit type.
    Unit,
    /// The boolean type.
    Bool,
    /// The wrapping 64-bit integer type.
    I64,
    /// The text type.
    Text,
    /// Inert syntax produced by `reflect`.
    Syntax,
    /// A product of two payloads.
    Pair(Box<Ty>, Box<Ty>),
    /// A sum of two payloads.
    Sum(Box<Ty>, Box<Ty>),
    /// A homogeneous list.
    List(Box<Ty>),
    /// A first-class program value.
    Program(Box<ProgramTy>),
    /// An opaque resource kind; never data-eligible.
    Resource(ResourceKind),
}

impl Ty {
    /// Build a program type.
    pub fn program(stack_in: Vec<Ty>, stack_out: Vec<Ty>, effects: EffSet) -> Ty {
        Ty::Program(Box::new(ProgramTy {
            stack_in,
            stack_out,
            effects,
        }))
    }

    /// The recursive `Data` eligibility predicate.
    ///
    /// A resource anywhere inside a payload makes the whole value ineligible,
    /// including through `Pair`, `Sum`, and `List` alternatives.
    pub fn is_data(&self) -> bool {
        match self {
            Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Syntax | Ty::Program(_) => true,
            Ty::Pair(a, b) | Ty::Sum(a, b) => a.is_data() && b.is_data(),
            Ty::List(item) => item.is_data(),
            Ty::Resource(_) => false,
        }
    }

    /// A size measure used by the declared type-size limit.
    pub fn size(&self) -> u32 {
        match self {
            Ty::Pair(a, b) | Ty::Sum(a, b) => {
                1u32.saturating_add(a.size()).saturating_add(b.size())
            }
            Ty::List(item) => 1u32.saturating_add(item.size()),
            Ty::Program(program) => {
                let mut total = 1u32;
                for ty in &program.stack_in {
                    total = total.saturating_add(ty.size());
                }
                for ty in &program.stack_out {
                    total = total.saturating_add(ty.size());
                }
                total
            }
            Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Syntax | Ty::Resource(_) => 1,
        }
    }
}

/// Whether every element of a stack is `Data`.
pub fn stack_is_data(stack: &[Ty]) -> bool {
    stack.iter().all(Ty::is_data)
}
