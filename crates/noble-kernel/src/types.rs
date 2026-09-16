//! Fragment v0 value, stack, and program types.
//!
//! Trees are walked iteratively with explicit, bounded work stacks; no
//! operation recurses and no walk grows without a local bound. Stacks are
//! ordered bottom-first, matching the specification's "top on the right".

/// Local bound for one type walk; beyond it every predicate fails closed.
const WORK_CAP: usize = 512;

/// Stable identity of a resource kind supplied by the environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceKind(pub u32);

/// Stable identity of one host-operation effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EffId(pub u32);

/// A finite set of effect identities, kept sorted and deduplicated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffSet(alloc::vec::Vec<EffId>);

impl EffSet {
    /// The empty bound.
    pub fn empty() -> Self {
        EffSet(alloc::vec::Vec::new())
    }

    /// Build from a list, sorting and deduplicating.
    pub fn from_ids(ids: &[EffId]) -> Self {
        let mut sorted = alloc::vec::Vec::with_capacity(ids.len().max(4));
        sorted.extend_from_slice(ids);
        sorted.sort_unstable();
        sorted.dedup();
        EffSet(sorted)
    }

    /// Membership.
    pub fn contains(&self, id: EffId) -> bool {
        self.0.binary_search(&id).is_ok()
    }

    /// Least upper bound.
    pub fn union(&self, other: &EffSet) -> EffSet {
        let mut out = alloc::vec::Vec::with_capacity(self.0.len() + other.0.len());
        let mut left = 0usize;
        let mut right = 0usize;
        while left < self.0.len() || right < other.0.len() {
            let is_take_left =
                right == other.0.len() || (left < self.0.len() && self.0[left] < other.0[right]);
            let is_take_right =
                left == self.0.len() || (right < other.0.len() && other.0[right] < self.0[left]);
            if is_take_left {
                out.push(self.0[left]);
                left += 1;
            } else if is_take_right {
                out.push(other.0[right]);
                right += 1;
            } else {
                out.push(self.0[left]);
                left += 1;
                right += 1;
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

    /// Number of identities, or `None` when the count is not representable.
    pub fn len(&self) -> Option<u64> {
        u64::try_from(self.0.len()).ok()
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A monomorphic program type: invocation stack, result stack, latent bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramTy {
    /// Required invocation stack, bottom-first.
    pub stack_in: alloc::vec::Vec<Ty>,
    /// Result stack, bottom-first.
    pub stack_out: alloc::vec::Vec<Ty>,
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
    Pair(alloc::boxed::Box<Ty>, alloc::boxed::Box<Ty>),
    /// A sum of two payloads.
    Sum(alloc::boxed::Box<Ty>, alloc::boxed::Box<Ty>),
    /// A homogeneous list.
    List(alloc::boxed::Box<Ty>),
    /// A first-class program value.
    Program(alloc::boxed::Box<ProgramTy>),
    /// An opaque resource kind; never data-eligible.
    Resource(ResourceKind),
}

impl Ty {
    /// Build a program type.
    pub fn program(
        stack_in: alloc::vec::Vec<Ty>,
        stack_out: alloc::vec::Vec<Ty>,
        effects: EffSet,
    ) -> Ty {
        Ty::Program(alloc::boxed::Box::new(ProgramTy {
            stack_in,
            stack_out,
            effects,
        }))
    }

    /// The recursive `Data` eligibility predicate.
    ///
    /// A resource anywhere inside a payload makes the whole value ineligible,
    /// including through `Pair`, `Sum`, and `List` alternatives. A walk that
    /// exceeds the local bound fails closed.
    pub fn is_data(&self) -> bool {
        let mut work: alloc::vec::Vec<&Ty> = alloc::vec::Vec::with_capacity(8);
        work.push(self);
        while let Some(node) = work.pop() {
            if work.len() >= WORK_CAP {
                return false;
            }
            match node {
                Ty::Resource(_) => return false,
                Ty::Pair(left, right) | Ty::Sum(left, right) => {
                    work.push(left);
                    work.push(right);
                }
                Ty::List(item) => work.push(item),
                Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Syntax | Ty::Program(_) => {}
            }
        }
        true
    }

    /// A size measure used by the declared type-size limit.
    pub fn size(&self) -> Option<u32> {
        let mut todo: alloc::vec::Vec<(&Ty, bool)> = alloc::vec::Vec::with_capacity(8);
        let mut sizes: alloc::vec::Vec<u32> = alloc::vec::Vec::with_capacity(8);
        todo.push((self, false));
        while let Some((node, expanded)) = todo.pop() {
            if todo.len() >= WORK_CAP {
                return None;
            }
            if expanded {
                let mut total = 1u32;
                match node {
                    Ty::Pair(_, _) | Ty::Sum(_, _) => {
                        total = total.saturating_add(sizes.pop()?);
                        total = total.saturating_add(sizes.pop()?);
                    }
                    Ty::List(_) => {
                        total = total.saturating_add(sizes.pop()?);
                    }
                    Ty::Program(program) => {
                        let count = program.stack_in.len() + program.stack_out.len();
                        for _ in 0..count {
                            total = total.saturating_add(sizes.pop()?);
                        }
                    }
                    Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Syntax | Ty::Resource(_) => {}
                }
                sizes.push(total);
                continue;
            }
            match node {
                Ty::Pair(left, right) | Ty::Sum(left, right) => {
                    todo.push((node, true));
                    todo.push((left, false));
                    todo.push((right, false));
                }
                Ty::List(item) => {
                    todo.push((node, true));
                    todo.push((item, false));
                }
                Ty::Program(program) => {
                    todo.push((node, true));
                    for ty in program.stack_in.iter().chain(program.stack_out.iter()) {
                        todo.push((ty, false));
                    }
                }
                Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Syntax | Ty::Resource(_) => {
                    sizes.push(1)
                }
            }
        }
        sizes.pop()
    }
}

/// Whether every element of a stack is `Data`.
pub fn stack_is_data(stack: &[Ty]) -> bool {
    stack.iter().all(Ty::is_data)
}
