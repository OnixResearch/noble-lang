//! Untrusted candidate data, the acceptance request, and the outcome domain.
//!
//! Nothing in this module is trusted: a `Candidate` is decoded input, and only
//! `Outcome::Accepted` carries a checked result.

/// Format revision every candidate must carry.
pub const CANDIDATE_FORMAT: u32 = 0;
/// Semantic revision the fragment targets (`0.1.0-draft.5`).
pub const SEMANTIC_REVISION: u32 = 0;

/// Index of a node inside the candidate arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(pub u32);

/// A literal's payload; text content is irrelevant to typing.
///
/// The extracted constructors carry a `Lit` suffix: `Bool` and `Unit` would
/// otherwise shadow Lean's own names inside every declaration named
/// `untrusted.Lit.*`, where this type's methods and instances live.
#[charon::variants_suffix("Lit")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lit {
    /// An `I64` literal.
    I64(i64),
    /// A `Bool` literal.
    Bool(bool),
    /// A `Text` literal.
    Text,
    /// The unit literal.
    Unit,
}

impl Lit {
    /// The literal's type.
    pub fn ty(&self) -> crate::types::Ty {
        match self {
            Lit::I64(_) => crate::types::Ty::I64,
            Lit::Bool(_) => crate::types::Ty::Bool,
            Lit::Text => crate::types::Ty::Text,
            Lit::Unit => crate::types::Ty::Unit,
        }
    }
}

/// The three fragment node forms; every node carries its instantiation
/// witness, and every conclusion is derived from those premises.
#[derive(Clone, Debug)]
pub enum Node {
    /// A literal with its construction-stack instantiation.
    Literal {
        /// The literal payload.
        lit: Lit,
        /// Instantiation of the literal's scheme.
        inst: crate::words::Inst,
    },
    /// A resolved invocation with a fresh instantiation.
    Invocation {
        /// The exact environment definition.
        def: crate::contracts::Definition,
        /// Instantiation of the definition's scheme.
        inst: crate::words::Inst,
    },
    /// A quotation literal over a finite body.
    Quotation {
        /// The body's node references.
        body: alloc::vec::Vec<NodeId>,
        /// Instantiation of the quotation scheme, binding the surrounding
        /// stack `R` and the body interface `A -- C ! e`.
        inst: crate::words::Inst,
    },
}

/// The untrusted candidate: a finite node arena plus the entry body.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Candidate format revision.
    pub format: u32,
    /// Semantic revision.
    pub revision: u32,
    /// The finite node arena.
    pub nodes: alloc::vec::Vec<Node>,
    /// The entry body's node references, in order.
    pub body: alloc::vec::Vec<NodeId>,
}

/// The independently supplied expected interface and allowed effect bound.
#[derive(Clone, Debug)]
pub struct Expected {
    /// The required entry stack, bottom-first.
    pub stack_in: alloc::vec::Vec<crate::types::Ty>,
    /// The required result stack, bottom-first.
    pub stack_out: alloc::vec::Vec<crate::types::Ty>,
    /// The allowed effect bound the derived bound must fit inside.
    pub allowed_effects: crate::types::EffSet,
}

/// Declared finite limits. Every stage charges before its next bounded step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Input bytes.
    pub bytes: u32,
    /// Decoded nodes.
    pub nodes: u32,
    /// Quotation nesting depth.
    pub depth: u32,
    /// One type's size.
    pub type_size: u32,
    /// One stack's height.
    pub stack_height: u32,
    /// Total checking work.
    pub work: u32,
    /// Diagnostic entries.
    pub diagnostics: u32,
}

/// One acceptance request.
#[derive(Clone, Debug)]
pub struct Request {
    /// Decoded input byte count, supplied by the caller.
    pub input_bytes: u32,
    /// The expected interface and allowed bound.
    pub expected: Expected,
    /// The declared limits.
    pub limits: Limits,
}

/// A derived node interface.
#[cfg_attr(test, derive(PartialEq, Eq))]
#[derive(Clone, Debug)]
pub struct Interface {
    /// Required invocation stack, bottom-first.
    pub stack_in: alloc::vec::Vec<crate::types::Ty>,
    /// Result stack, bottom-first.
    pub stack_out: alloc::vec::Vec<crate::types::Ty>,
    /// Latent effect bound.
    pub effects: crate::types::EffSet,
}

/// One node's retained derivation.
#[derive(Clone, Debug)]
pub struct Derivation {
    /// The node.
    pub node: NodeId,
    /// Its derived interface.
    pub interface: Interface,
}

/// The accepted result: the checked interface plus every node derivation.
#[derive(Clone, Debug)]
pub struct Checked {
    /// The accepted interface at the expected stacks.
    pub interface: Interface,
    /// One entry per checked node occurrence, in visit order.
    pub derivations: alloc::vec::Vec<Derivation>,
}

/// Which declared limit was exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind {
    /// Input bytes.
    Bytes,
    /// Decoded nodes.
    Nodes,
    /// Quotation nesting depth.
    Depth,
    /// One type's size.
    TypeSize,
    /// One stack's height.
    StackHeight,
    /// Total checking work.
    Work,
    /// Diagnostic entries.
    Diagnostics,
}

/// Out-of-fragment or foreign input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedKind {
    /// Unknown candidate format or semantic revision.
    FormatRevision,
    /// A node or type form outside the fragment.
    NodeForm,
    /// A malformed environment scheme.
    SchemeForm,
}

/// The violated constraint recorded by a rejection.
/// Equality needs no `Eq` marker in the kernel; the extraction renders the
/// marker impl with an unresolvable default.
#[derive(Clone, Debug, PartialEq)]
pub enum Constraint {
    /// A join's shapes do not match.
    StackJoin,
    /// The stack holds the right types in the wrong order.
    StackOrder,
    /// The derived bound contains an identity outside the allowed bound.
    EffectInclusion(crate::types::EffId),
    /// A `Data`-requiring word met a non-capturable type.
    Eligibility(crate::types::Ty),
    /// An effect identity the environment does not provide.
    UnknownEffect(crate::types::EffId),
    /// An instantiation binding's kind or variable is malformed.
    InstantiationKind,
    /// An instantiation's binding count is malformed.
    InstantiationArity,
    /// A node reference points outside the finite arena.
    MalformedReference(NodeId),
    /// A definition identity is not in the environment.
    UnknownDefinition(crate::contracts::Definition),
}

/// One rejection's diagnostic.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// The failing node, or `None` when the request's entry is at fault.
    pub node: Option<NodeId>,
    /// The failing word, when a word is at fault.
    pub def: Option<crate::contracts::Definition>,
    /// The expected stack shapes.
    pub expected: alloc::vec::Vec<crate::types::Ty>,
    /// The actual stack shapes.
    pub actual: alloc::vec::Vec<crate::types::Ty>,
    /// The violated constraint.
    pub constraint: Constraint,
    /// Whether value-origin provenance is available; v0 reports it as absent.
    pub provenance_available: bool,
    /// Whether the diagnostic stacks were cut to the diagnostic limit.
    pub truncated: bool,
}

/// The five-way outcome domain. Only acceptance carries a checked program.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// The candidate checked; the checked interface and derivations return.
    Accepted(Checked),
    /// The candidate is well-formed input but does not derive.
    Invalid(Diagnostic),
    /// The input is outside the fragment or names foreign identities.
    Unsupported(UnsupportedKind),
    /// A declared limit was exceeded before the bounded step.
    Exhausted(LimitKind),
    /// A reserved outcome the shell may report; v0 has no producing path.
    InternalFailure,
}
