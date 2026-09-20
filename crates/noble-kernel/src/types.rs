//! Fragment v0 value, stack, and program types.
//!
//! Trees are walked iteratively with explicit, bounded work stacks; no
//! operation recurses and no walk grows without a local bound. Stacks are
//! ordered bottom-first, matching the specification's "top on the right".

mod impls;
mod size;

/// Local bound for one type walk; beyond it every predicate fails closed.
const WORK_CAP: usize = 512;

/// Stable identity of a resource kind supplied by the environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceKind(pub u32);

/// Stable identity of one host-operation effect.
///
/// The identity is ordered by its payload, compared explicitly at each use.
/// A derived `PartialOrd`/`Ord` would have Aeneas emit a `PartialOrd` instance
/// whose `lt` and `gt` fields are the trait's own default methods, and the
/// Lean backend renders those defaults by handing them the whole instance
/// where the model expects the comparison function. Comparing the payloads
/// keeps ordering inside the scalar operations the backend already models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    ///
    /// The insertion walk is explicit: slice sort helpers draw core iterator
    /// code into the extraction subject, which Aeneas cannot translate. Order
    /// is compared on the payloads, so no `PartialOrd` instance is involved.
    pub fn from_ids(ids: &[EffId]) -> Self {
        let mut sorted: alloc::vec::Vec<EffId> =
            alloc::vec::Vec::with_capacity(crate::capacity::at_least(ids.len(), 4));
        let mut index = 0;
        while index < ids.len() {
            let id = ids[index];
            let mut position = 0;
            let mut is_placed = false;
            while position < sorted.len() && !is_placed {
                if sorted[position] == id {
                    is_placed = true;
                } else if sorted[position].0 > id.0 {
                    sorted.insert(position, id);
                    is_placed = true;
                } else {
                    position += 1;
                }
            }
            if !is_placed {
                sorted.push(id);
            }
            index += 1;
        }
        EffSet(sorted)
    }

    /// Membership.
    pub fn contains(&self, id: EffId) -> bool {
        let mut index = 0;
        let mut is_found = false;
        while index < self.0.len() {
            if self.0[index] == id {
                is_found = true;
                break;
            }
            index += 1;
        }
        is_found
    }

    /// Least upper bound.
    pub fn union(&self, other: &EffSet) -> EffSet {
        #[expect(
            tigerstyle::raw_arithmetic_overflow,
            reason = "Owner: noble-maintainers; each Vec<EffId> is bounded by isize::MAX bytes and EffId is nonzero-sized, so the sum of two lengths is at most 2 * isize::MAX < usize::MAX; reassess if storage becomes zero-sized."
        )]
        let mut out = alloc::vec::Vec::with_capacity(self.0.len() + other.0.len());
        let mut left = 0usize;
        let mut right = 0usize;
        while left < self.0.len() || right < other.0.len() {
            let is_take_left = right == other.0.len()
                || (left < self.0.len() && self.0[left].0 < other.0[right].0);
            let is_take_right = left == self.0.len()
                || (right < other.0.len() && other.0[right].0 < self.0[left].0);
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
        let mut index = 0;
        let mut is_subset = true;
        while index < self.0.len() {
            if !other.contains(self.0[index]) {
                is_subset = false;
                break;
            }
            index += 1;
        }
        is_subset
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
        self.0.len() == 0
    }
}

/// A type in the fragment's finite universe.
///
/// The program case carries its stacks and bound directly, so the type family
/// is self-recursive: mutually recursive types would leave Aeneas' dependency
/// analysis with mixed declaration groups it refuses to translate.
///
/// The extracted constructors carry a `Type` suffix: `Unit` and `Bool` would
/// otherwise shadow Lean's own `Unit` and `Bool` inside every declaration
/// named `types.Ty.*`, which is where this type's own methods and instances
/// live. The Rust names are unchanged.
///
/// `Clone`, `Debug`, and equality are hand-written in `impls`: a derived body
/// would reference this type.s own instance, which the Lean backend renders
/// after the method that feeds it.
#[charon::variants_suffix("Type")]
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
    /// A first-class program type: required stack, result stack, latent bound.
    Program(
        alloc::boxed::Box<alloc::vec::Vec<Ty>>,
        alloc::boxed::Box<alloc::vec::Vec<Ty>>,
        EffSet,
    ),
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
        Ty::Program(
            alloc::boxed::Box::new(stack_in),
            alloc::boxed::Box::new(stack_out),
            effects,
        )
    }

    /// The recursive `Data` eligibility predicate.
    ///
    /// A resource anywhere inside a payload makes the whole value ineligible,
    /// including through `Pair`, `Sum`, and `List` alternatives. A walk that
    /// exceeds the local bound fails closed.
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; each Ty constructor must explicitly declare whether Data eligibility inspects children, accepts directly or rejects resources; new types must not inherit a fallback eligibility rule."
    )]
    pub fn is_data(&self) -> bool {
        let mut work: alloc::vec::Vec<Ty> = alloc::vec::Vec::with_capacity(8);
        work.push(self.clone());
        let mut is_data = true;
        while let Some(node) = work.pop() {
            if work.len() >= WORK_CAP {
                is_data = false;
                break;
            }
            match node {
                Ty::Resource(_) => {
                    is_data = false;
                    break;
                }
                Ty::Pair(left, right) | Ty::Sum(left, right) => {
                    work.push(*left);
                    work.push(*right);
                }
                Ty::List(item) => work.push(*item),
                Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Syntax | Ty::Program(_, _, _) => {}
            }
        }
        is_data
    }

    /// A size measure used by the declared type-size limit.
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-maintainers; every size-walk Step must map explicitly to failure or a computed size; a new state must require review of the fail-closed size contract."
    )]
    pub fn size(&self) -> Option<u32> {
        let mut walk = size::Walk {
            todo: alloc::vec::Vec::with_capacity(8),
            sizes: alloc::vec::Vec::with_capacity(8),
        };
        walk.todo.push((self.clone(), false));
        let mut outcome = size::Step::Continue;
        while matches!(outcome, size::Step::Continue) {
            let (next, step) = size::walk_step(walk);
            walk = next;
            outcome = step;
        }
        match outcome {
            size::Step::Failed => None,
            size::Step::Continue | size::Step::Done => walk.sizes.pop(),
        }
    }
}

pub fn stack_is_data(stack: &[Ty]) -> bool {
    let mut index = 0;
    let mut is_data = true;
    while index < stack.len() {
        if !stack[index].is_data() {
            is_data = false;
            break;
        }
        index += 1;
    }
    is_data
}
