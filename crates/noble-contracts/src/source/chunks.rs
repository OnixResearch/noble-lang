//! Bounded checked typed-chunk completion (`typed-chunk/1`, `pure-i64-bool/1`).
//!
//! This module is the fixed-chunk baseline of issue #73 and `.cairn/changes/
//! typed-chunk-completion` (TCHUNK-01 through TCHUNK-04). It is host-owned and
//! opt-in: it adds no syntax, changes no ordinary source admission path and
//! publishes nothing (`Session::commit` is never called here).
//!
//! Three boundaries are kept separate and none is a credential for another:
//!
//! - An envelope is *input*. Its identity, source revision and ordered
//!   interface are recomputed from retained bytes, never trusted.
//! - A completed program is *fresh source*. It is emitted, re-parsed and
//!   prepared by [`super::Session::prepare`], and the resulting submission is
//!   independently re-checked by [`noble_kernel::acceptance::check`]. Only
//!   those two results can accept a proposal; fixture agreement, the envelope
//!   digest and this module's own ordered-stack replay cannot.
//! - A refusal class is *not* a completeness claim. `NoResult` means every
//!   proposal in the finite search was refused before a budget was exhausted;
//!   it never means no completion exists.
//!
//! The replay in this module is deliberately narrower than the language: a
//! quotation literal is typed at its construction site's concrete stack, so a
//! program that runs a quotation at a different ambient stack is refused here
//! (never accepted). Every emitted proposal is still checked by the ordinary
//! pipeline, so the replay can only under-approximate, never authorize.

#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; all mutation in this host-owned module is limited to the bounded Library snapshot, the per-request budget ledger, and fresh expansion/evaluation scratch; borrowed source, requests, recipes, namespace state and host state are never mutated"
)]
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the closed typed-chunk profile uses kernel Ty in its public interfaces and checked stack helpers, and qualifying every occurrence would obscure the ordered type signatures without changing ownership"
)]
use noble_kernel::types::Ty;

/// Envelope format revision carried by every chunk.
pub const ENVELOPE_FORMAT: u32 = 1;
/// Semantic revision every chunk targets.
pub const SEMANTIC_REVISION: u32 = 0;
/// The only admitted profile.
pub const PROFILE: &str = "pure-i64-bool/1";

/// Bytes in one chunk identity.
pub const IDENTITY_BYTES: usize = 16;
/// Bytes in one source revision.
pub const REVISION_BYTES: usize = 32;

/// Largest capture list one chunk may retain.
pub const MAX_CAPTURES: u32 = 8;
/// Largest binding list one witness may retain before the shared total cap.
pub const MAX_INSTANTIATION_BINDINGS: u32 = 16;

/// Fixed profile maxima. A host may lower, never raise, each of these.
pub const MAX_LIBRARY_BYTES: u32 = 262_144;
pub const MAX_LIBRARY_ENTRIES: u32 = 64;
pub const MAX_RECIPE_NODES: u32 = 64;
pub const MAX_STACK_HEIGHT: u32 = 32;
pub const MAX_DEPENDENCY_STEPS: u32 = 256;
pub const MAX_TOTAL_INSTANTIATION_BINDINGS: u32 = 64;
pub const MAX_QUOTATION_DEPTH: u32 = 8;
pub const MAX_EXPANDED_NODES: u32 = 256;
pub const MAX_COMPOSED_CHUNKS: u32 = 4;
pub const MAX_SOURCE_BYTES: u32 = 4_096;
pub const MAX_CANDIDATES: u32 = 128;
pub const MAX_DIAGNOSTICS: u32 = 64;
pub const MAX_CHECKING_WORK: u32 = 4_096;
pub const MAX_EVALUATION_STEPS: u32 = 256;
pub const MAX_RETAINED_BYTES: u32 = 262_144;

/// Inner compiler-work cap; the shared profile budget is always lower.
const INNER_WORK_CEILING: u32 = 2_000_000;
// Fixed-width mirrors used only for bounded host allocations; the public
// policy limits remain `u32` and are checked before these capacities are used.
const MAX_QUOTATION_FRAMES: usize = 9; // root frame plus the depth-8 profile limit
const MAX_EXPANDED_NODE_CAPACITY: usize = 256; // MAX_EXPANDED_NODES
const MAX_STACK_CAPACITY: usize = 33; // one over the profile limit to retain refusal state
const MAX_DEPENDENCY_PENDING: usize = 257; // selected entry plus 256 charged dependency steps
const IDENTITY_BYTES_U64: u64 = 16;
const REVISION_BYTES_U64: u64 = 32;

/// One immutable scalar capture or fixture value. The variant *is* the type,
/// so a capture cannot claim a type its value does not have.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    /// A wrapping 64-bit integer.
    I64(i64),
    /// A boolean.
    Bool(bool),
}

impl Scalar {
    /// The scalar's exact profile type.
    pub const fn ty(self) -> Ty {
        match self {
            Scalar::I64(_) => Ty::I64,
            Scalar::Bool(_) => Ty::Bool,
        }
    }
}

/// One retained scalar capture, indexed contiguously from zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capture {
    /// Position in the capture list; must equal the list index.
    pub index: u16,
    /// The immutable captured value.
    pub value: Scalar,
}

/// One exact type-variable binding retained by an instantiation witness.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    /// Stable variable ordinal inside the witness.
    pub variable: u16,
    /// Kind of the bound kernel variable.
    pub kind: noble_kernel::words::VariableKind,
    /// The concrete profile type this variable is bound to.
    pub ty: Ty,
}

/// An ordered witness list. It is retained, never regenerated from a signature.
#[derive(Clone, Debug, PartialEq)]
pub struct Instantiation {
    /// Bindings in increasing variable order.
    pub bindings: alloc::vec::Vec<Binding>,
}

/// A resolved dependency record. Only `identity`, `format`,
/// `semantic_revision` and `source_revision` are compared; no field is
/// admission authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefinitionRef {
    /// Lookup identity of the referenced library entry.
    pub identity: u64,
    /// Referenced envelope format.
    pub format: u32,
    /// Referenced semantic revision.
    pub semantic_revision: u32,
    /// Referenced emitted-source revision.
    pub source_revision: [u8; REVISION_BYTES],
}

/// The published whitelist: pure ordered-stack routing, `I64` arithmetic and
/// equality, and explicit quotation execution.
///
/// Each variant is a real bootstrap word. `quote`, `compose`, `dip`, `if` and
/// every non-pure word stay outside the profile, so the search surface cannot
/// grow into ordinary quotation composition (TCHUNK-02).
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Dup,
    Drop,
    Swap,
    Add,
    Sub,
    Mul,
    Eq,
    Run,
}

impl Op {
    /// The exact source spelling this operation emits and resolves.
    pub const fn spelling(self) -> &'static str {
        match self {
            Op::Dup => "dup",
            Op::Drop => "drop",
            Op::Swap => "swap",
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Eq => "=",
            Op::Run => "run",
        }
    }

    /// Resolve a spelling to a profile operation. A spelling outside this
    /// table is not a profile node, whatever the namespace defines.
    pub fn of(word: &[u8]) -> Option<Self> {
        if spells(word, b"dup") {
            Some(Op::Dup)
        } else if spells(word, b"drop") {
            Some(Op::Drop)
        } else if spells(word, b"swap") {
            Some(Op::Swap)
        } else if spells(word, b"+") {
            Some(Op::Add)
        } else if spells(word, b"-") {
            Some(Op::Sub)
        } else if spells(word, b"*") {
            Some(Op::Mul)
        } else if spells(word, b"=") {
            Some(Op::Eq)
        } else if spells(word, b"run") {
            Some(Op::Run)
        } else {
            None
        }
    }
}

/// Whether `word` is exactly the byte spelling `spelling`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; byte-spelling comparison uses slice equality, which is not a stable const trait"
)]
fn spells(word: &[u8], spelling: &[u8]) -> bool {
    *word == *spelling
}

/// One retained recipe node. References are indices into the recipe's own
/// node arena and are canonical only when a node refers to an earlier node.
#[octet::sealed_enum]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    I64(i64),
    Bool(bool),
    Capture {
        index: u16,
    },
    Operation {
        op: Op,
        instantiation: u16,
    },
    Invoke {
        dependency: u16,
        instantiation: u16,
    },
    Quotation {
        body: alloc::vec::Vec<u32>,
        instantiation: u16,
    },
}

/// A finite acyclic recipe: a node arena plus its entry references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub nodes: alloc::vec::Vec<Node>,
    pub entry: alloc::vec::Vec<u32>,
}

/// Exact ordered interface. The effect set is empty by profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Interface {
    pub stack_in: alloc::vec::Vec<Ty>,
    pub stack_out: alloc::vec::Vec<Ty>,
    pub effects: noble_kernel::types::EffSet,
}

/// The versioned chunk envelope. Every field is recomputed before retention.
#[derive(Clone, Debug, PartialEq)]
pub struct Envelope {
    pub format: u32,
    pub semantic_revision: u32,
    pub profile: alloc::string::String,
    pub chunk_id: [u8; IDENTITY_BYTES],
    pub source_revision: [u8; REVISION_BYTES],
    pub interface: Interface,
    pub captures: alloc::vec::Vec<Capture>,
    pub dependencies: alloc::vec::Vec<DefinitionRef>,
    pub instantiations: alloc::vec::Vec<Instantiation>,
    pub body: Recipe,
}

/// One admitted library entry, retaining the envelope, its canonical source and
/// the interface ordinary source admission derived for it.
#[derive(Clone, Debug)]
pub struct Entry {
    envelope: Envelope,
    #[charon::rename("source_bytes")]
    source: alloc::vec::Vec<u8>,
    interface: Interface,
    expanded_nodes: u32,
    identity: u64,
}

impl Entry {
    pub const fn envelope(&self) -> &Envelope {
        &self.envelope
    }
    pub fn source(&self) -> &[u8] {
        &self.source
    }
    /// The interface derived by [`super::Session::prepare`], not the declared one.
    pub const fn interface(&self) -> &Interface {
        &self.interface
    }
    pub const fn expanded_nodes(&self) -> u32 {
        self.expanded_nodes
    }
    pub const fn identity(&self) -> u64 {
        self.identity
    }
    pub const fn source_revision(&self) -> &[u8; REVISION_BYTES] {
        &self.envelope.source_revision
    }
}

/// One host-owned immutable library snapshot.
#[derive(Debug)]
pub struct Library {
    revision: [u8; REVISION_BYTES],
    budget: Budget,
    entries: alloc::vec::Vec<Entry>,
    retained: u32,
}

impl Library {
    /// Start an empty snapshot pinned to `revision`.
    pub fn new(revision: [u8; REVISION_BYTES]) -> Self {
        Self::new_with_budget(revision, Budget::PROFILE)
    }

    /// Start an empty snapshot with a host budget that can only lower the
    /// fixed profile limits.
    pub fn new_with_budget(revision: [u8; REVISION_BYTES], budget: Budget) -> Self {
        Self {
            revision,
            budget: budget.lowered(&Budget::PROFILE),
            entries: alloc::vec::Vec::new(),
            retained: 0,
        }
    }

    pub const fn revision(&self) -> &[u8; REVISION_BYTES] {
        &self.revision
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub const fn retained(&self) -> u32 {
        self.retained
    }

    /// The linear scan a dependency record is resolved through. A miss is the
    /// only answer; a partial match never resolves.
    fn find(&self, reference: &DefinitionRef) -> Option<u16> {
        let mut at = 0usize;
        while let Some(entry) = self.entries.get(at) {
            if entry_matches(entry, reference) {
                return u16::try_from(at).ok();
            }
            at = at.saturating_add(1);
        }
        None
    }
}

/// The fifteen shared budget resources.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    LibraryBytes,
    Entries,
    RecipeNodes,
    StackHeight,
    DependencySteps,
    InstantiationBindings,
    QuotationDepth,
    ExpandedNodes,
    ComposedChunks,
    SourceBytes,
    Candidates,
    Diagnostics,
    CheckingWork,
    EvaluationSteps,
    RetainedBytes,
}

impl Resource {
    /// Every resource, in the design table's order.
    pub const ALL: [Resource; Resource::COUNT] = [
        Resource::LibraryBytes,
        Resource::Entries,
        Resource::RecipeNodes,
        Resource::StackHeight,
        Resource::DependencySteps,
        Resource::InstantiationBindings,
        Resource::QuotationDepth,
        Resource::ExpandedNodes,
        Resource::ComposedChunks,
        Resource::SourceBytes,
        Resource::Candidates,
        Resource::Diagnostics,
        Resource::CheckingWork,
        Resource::EvaluationSteps,
        Resource::RetainedBytes,
    ];
    /// Number of distinct resources.
    pub const COUNT: usize = 15;

    /// The stable machine-facing name a report and a client key on.
    pub const fn name(self) -> &'static str {
        match self {
            Resource::LibraryBytes => "library-bytes",
            Resource::Entries => "entries",
            Resource::RecipeNodes => "recipe-nodes",
            Resource::StackHeight => "stack-height",
            Resource::DependencySteps => "dependency-steps",
            Resource::InstantiationBindings => "instantiation-bindings",
            Resource::QuotationDepth => "quotation-depth",
            Resource::ExpandedNodes => "expanded-nodes",
            Resource::ComposedChunks => "composed-chunks",
            Resource::SourceBytes => "source-bytes",
            Resource::Candidates => "candidates",
            Resource::Diagnostics => "diagnostics",
            Resource::CheckingWork => "checking-work",
            Resource::EvaluationSteps => "evaluation-steps",
            Resource::RetainedBytes => "retained-bytes",
        }
    }

    const fn slot(self) -> usize {
        match self {
            Resource::LibraryBytes => 0,
            Resource::Entries => 1,
            Resource::RecipeNodes => 2,
            Resource::StackHeight => 3,
            Resource::DependencySteps => 4,
            Resource::InstantiationBindings => 5,
            Resource::QuotationDepth => 6,
            Resource::ExpandedNodes => 7,
            Resource::ComposedChunks => 8,
            Resource::SourceBytes => 9,
            Resource::Candidates => 10,
            Resource::Diagnostics => 11,
            Resource::CheckingWork => 12,
            Resource::EvaluationSteps => 13,
            Resource::RetainedBytes => 14,
        }
    }
}

/// The fifteen profile maxima. A host lowers a copy; nothing raises one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub library_bytes: u32,
    pub entries: u32,
    pub recipe_nodes: u32,
    pub stack_height: u32,
    pub dependency_steps: u32,
    pub instantiation_bindings: u32,
    pub quotation_depth: u32,
    pub expanded_nodes: u32,
    pub composed_chunks: u32,
    pub source_bytes: u32,
    pub candidates: u32,
    pub diagnostics: u32,
    pub checking_work: u32,
    pub evaluation_steps: u32,
    pub retained_bytes: u32,
}

impl Budget {
    /// The fixed profile maxima.
    pub const PROFILE: Budget = Budget {
        library_bytes: MAX_LIBRARY_BYTES,
        entries: MAX_LIBRARY_ENTRIES,
        recipe_nodes: MAX_RECIPE_NODES,
        stack_height: MAX_STACK_HEIGHT,
        dependency_steps: MAX_DEPENDENCY_STEPS,
        instantiation_bindings: MAX_TOTAL_INSTANTIATION_BINDINGS,
        quotation_depth: MAX_QUOTATION_DEPTH,
        expanded_nodes: MAX_EXPANDED_NODES,
        composed_chunks: MAX_COMPOSED_CHUNKS,
        source_bytes: MAX_SOURCE_BYTES,
        candidates: MAX_CANDIDATES,
        diagnostics: MAX_DIAGNOSTICS,
        checking_work: MAX_CHECKING_WORK,
        evaluation_steps: MAX_EVALUATION_STEPS,
        retained_bytes: MAX_RETAINED_BYTES,
    };

    /// The limit this budget declares for one resource.
    pub const fn limit(&self, resource: Resource) -> u32 {
        match resource {
            Resource::LibraryBytes => self.library_bytes,
            Resource::Entries => self.entries,
            Resource::RecipeNodes => self.recipe_nodes,
            Resource::StackHeight => self.stack_height,
            Resource::DependencySteps => self.dependency_steps,
            Resource::InstantiationBindings => self.instantiation_bindings,
            Resource::QuotationDepth => self.quotation_depth,
            Resource::ExpandedNodes => self.expanded_nodes,
            Resource::ComposedChunks => self.composed_chunks,
            Resource::SourceBytes => self.source_bytes,
            Resource::Candidates => self.candidates,
            Resource::Diagnostics => self.diagnostics,
            Resource::CheckingWork => self.checking_work,
            Resource::EvaluationSteps => self.evaluation_steps,
            Resource::RetainedBytes => self.retained_bytes,
        }
    }

    const fn set(&mut self, resource: Resource, value: u32) {
        match resource {
            Resource::LibraryBytes => self.library_bytes = value,
            Resource::Entries => self.entries = value,
            Resource::RecipeNodes => self.recipe_nodes = value,
            Resource::StackHeight => self.stack_height = value,
            Resource::DependencySteps => self.dependency_steps = value,
            Resource::InstantiationBindings => self.instantiation_bindings = value,
            Resource::QuotationDepth => self.quotation_depth = value,
            Resource::ExpandedNodes => self.expanded_nodes = value,
            Resource::ComposedChunks => self.composed_chunks = value,
            Resource::SourceBytes => self.source_bytes = value,
            Resource::Candidates => self.candidates = value,
            Resource::Diagnostics => self.diagnostics = value,
            Resource::CheckingWork => self.checking_work = value,
            Resource::EvaluationSteps => self.evaluation_steps = value,
            Resource::RetainedBytes => self.retained_bytes = value,
        }
    }

    /// A host budget may only lower a profile value. Every raised field is
    /// clamped to the profile maximum rather than honoured.
    pub fn lowered(&self, profile: &Budget) -> Budget {
        let mut out = *profile;
        for resource in Resource::ALL {
            let profile_cap = profile.limit(resource);
            let requested_cap = self.limit(resource);
            let cap = if profile_cap < requested_cap {
                profile_cap
            } else {
                requested_cap
            };
            out.set(resource, cap);
        }
        out
    }
}

/// One exact refusal location: a stable field name and an optional ordinal.
/// The field name is owned: a `'static` borrow in a refusal type would keep
/// every `?` and loop over it out of the Aeneas extraction (issue #97).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub field: alloc::string::String,
    pub index: Option<u32>,
}

impl Location {
    pub fn field(field: &str) -> Self {
        Self {
            field: alloc::string::String::from(field),
            index: None,
        }
    }
    pub fn at(field: &str, index: u32) -> Self {
        Self {
            field: alloc::string::String::from(field),
            index: Some(index),
        }
    }
}

/// The four failure classes plus the fail-closed implementation class.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Invalid,
    Unsupported,
    Exhausted,
    Internal,
}

impl Class {
    pub const fn name(self) -> &'static str {
        match self {
            Class::Invalid => "invalid",
            Class::Unsupported => "unsupported",
            Class::Exhausted => "exhausted",
            Class::Internal => "internal",
        }
    }
}

/// One classified refusal with its exact location and counter values.
#[expect(
    tigerstyle::numeric_units,
    reason = "Owner: noble-maintainers; `limit` is paired with `resource`, whose enum selects the unit (bytes, entries, steps, or height), so one field cannot carry a fixed unit suffix"
)]
#[derive(Clone, Debug)]
pub struct Refusal {
    pub class: Class,
    pub location: Location,
    pub message: alloc::string::String,
    pub resource: Option<Resource>,
    pub observed: u32,
    pub limit: u32,
}

impl Refusal {
    pub fn new(class: Class, location: Location, message: &str) -> Self {
        Self {
            class,
            location,
            message: alloc::string::String::from(message),
            resource: None,
            observed: 0,
            limit: 0,
        }
    }

    pub const fn kind(&self) -> Class {
        self.class
    }

    /// The class name a report renders.
    pub const fn class_name(&self) -> &'static str {
        self.class.name()
    }
}

/// The shared fail-closed ledger. Cumulative resources add; per-item ceilings
/// keep the largest value observed. A refused charge never resets a counter and
/// never publishes partial state.
#[derive(Debug)]
pub struct Ledger {
    limits: Budget,
    charged: [u32; Resource::COUNT],
}

impl Ledger {
    pub const fn new(limits: Budget) -> Self {
        Self {
            limits,
            charged: [0; Resource::COUNT],
        }
    }

    pub const fn limits(&self) -> Budget {
        self.limits
    }

    /// The largest value observed, or the total charged, for one resource.
    pub fn observed(&self, resource: Resource) -> u32 {
        match self.charged.get(resource.slot()) {
            Some(value) => *value,
            None => 0,
        }
    }

    /// What a cumulative resource has left.
    pub fn remaining(&self, resource: Resource) -> u32 {
        self.limits
            .limit(resource)
            .saturating_sub(self.observed(resource))
    }

    fn refuse(&mut self, resource: Resource, observed: u32, at: Location) -> Refusal {
        let mut refusal = Refusal::new(
            Class::Exhausted,
            at,
            "one shared budget counter reached its fixed limit",
        );
        refusal.resource = Some(resource);
        refusal.observed = observed;
        refusal.limit = self.limits.limit(resource);
        refusal
    }

    fn refuse_unrepresentable_count(&mut self, resource: Resource, at: Location) -> Refusal {
        let max_value = self.limits.limit(resource);
        if let Some(slot) = self.charged.get_mut(resource.slot()) {
            *slot = max_value;
        }
        let mut refusal = self.refuse(resource, u32::MAX, at);
        refusal.message =
            alloc::string::String::from("budget count exceeds its u32 reporting range");
        refusal
    }

    /// Add `amount` to a cumulative resource and refuse the step that would
    /// exceed the limit. The observed count saturates at the limit so a report
    /// always names the boundary that stopped the work.
    pub fn charge(&mut self, resource: Resource, amount: u32, at: Location) -> Result<(), Refusal> {
        let current = self.observed(resource);
        let max_value = self.limits.limit(resource);
        let next = match current.checked_add(amount) {
            Some(next) => next,
            None => return Err(self.refuse(resource, u32::MAX, at)),
        };
        if next > max_value {
            if let Some(slot) = self.charged.get_mut(resource.slot()) {
                *slot = max_value;
            }
            return Err(self.refuse(resource, next, at));
        }
        if let Some(slot) = self.charged.get_mut(resource.slot()) {
            *slot = next;
        }
        Ok(())
    }

    /// Add a wide count without truncating it before checking the report width.
    pub fn charge_count(
        &mut self,
        resource: Resource,
        amount: u64,
        at: Location,
    ) -> Result<(), Refusal> {
        match u32::try_from(amount) {
            Ok(amount) => self.charge(resource, amount, at),
            Err(_) => Err(self.refuse_unrepresentable_count(resource, at)),
        }
    }

    /// Record a per-item ceiling. A smaller value than one already observed is
    /// kept out, so the ledger reports the worst case, never the last case.
    pub fn observe(&mut self, resource: Resource, value: u32, at: Location) -> Result<(), Refusal> {
        let max_value = self.limits.limit(resource);
        if value > max_value {
            if let Some(slot) = self.charged.get_mut(resource.slot()) {
                *slot = max_value;
            }
            return Err(self.refuse(resource, value, at));
        }
        if let Some(slot) = self.charged.get_mut(resource.slot()) {
            if *slot < value {
                *slot = value;
            }
        }
        Ok(())
    }

    /// Observe a wide value while retaining a resource-specific refusal if it
    /// is outside the report representation.
    pub fn observe_count(
        &mut self,
        resource: Resource,
        value: u64,
        at: Location,
    ) -> Result<(), Refusal> {
        match u32::try_from(value) {
            Ok(value) => self.observe(resource, value, at),
            Err(_) => Err(self.refuse_unrepresentable_count(resource, at)),
        }
    }

    /// The compiler-facing limits for one preparation. The caller supplies the
    /// remaining request work; no field ever exceeds the profile.
    const fn prepare_limits(&self, work: u32) -> crate::Limits {
        crate::Limits {
            bytes: MAX_SOURCE_BYTES,
            // The recipe expansion counter is charged separately. Bound the
            // whole completed source by its byte ceiling rather than imposing
            // an unreported 256-node cap on editor context around the hole.
            nodes: MAX_SOURCE_BYTES,
            depth: MAX_QUOTATION_DEPTH,
            work: if work < INNER_WORK_CEILING {
                work
            } else {
                INNER_WORK_CEILING
            },
        }
    }
}

/// One flat emitted element. `Open`/`Close` are the quotation brackets, so one
/// ordered list is both what gets emitted and what gets replayed.
#[octet::sealed_enum]
#[derive(Clone, Debug, PartialEq)]
pub enum Element {
    I64(i64),
    Bool(bool),
    Op {
        op: Op,
        witness: Option<Instantiation>,
    },
    Open,
    Close,
    /// The single editor hole. It never reaches emission or a kernel candidate.
    Hole,
}

/// Fold one canonical integer into `fold`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the digest fold mutates runtime identity state"
)]
fn absorb_ty(fold: &mut crate::companion::digest::Fold, ty: &Ty) {
    match ty {
        Ty::I64 => fold.absorb(1),
        Ty::Bool => fold.absorb(2),
        _ => fold.absorb(3),
    }
}

fn absorb_stack(fold: &mut crate::companion::digest::Fold, stack: &[Ty]) {
    fold.absorb_count(stack.len());
    for ty in stack {
        absorb_ty(fold, ty);
    }
}

fn absorb_bytes_lanes(domain: u64, bytes: &[u8], lanes: usize, out: &mut [u8]) {
    let mut lane = 0usize;
    while lane < lanes {
        let mut fold = crate::companion::digest::Fold::new(domain);
        fold.absorb_count(lane);
        fold.absorb_bytes(bytes);
        let value = fold.finish();
        let bytes_out = value.to_le_bytes();
        let base = lane.saturating_mul(8);
        let mut at = 0usize;
        while at < bytes_out.len() {
            if let Some(slot) = out.get_mut(base.saturating_add(at)) {
                *slot = bytes_out[at];
            }
            at = at.saturating_add(1);
        }
        lane = lane.saturating_add(1);
    }
}

/// The canonical source revision of one emitted program. An integrity index,
/// never admission authority (VC-ID-03): admission re-prepares the source.
pub fn canonical_revision(source: &[u8]) -> [u8; REVISION_BYTES] {
    let mut out = [0u8; REVISION_BYTES];
    absorb_bytes_lanes(
        crate::companion::digest::DOMAIN_CHUNK_SOURCE,
        source,
        REVISION_BYTES / 8,
        &mut out,
    );
    out
}

fn absorb_identity_interface(fold: &mut crate::companion::digest::Fold, envelope: &Envelope) {
    absorb_stack(fold, &envelope.interface.stack_in);
    absorb_stack(fold, &envelope.interface.stack_out);
    fold.absorb_count(envelope.interface.effects.as_slice().len());
    for effect in envelope.interface.effects.as_slice() {
        fold.absorb(u64::from(effect.0));
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; identity folding must hash even malformed public envelope inputs without panicking, leaving validation to typed refusal paths"
)]
fn absorb_identity_retained_values(fold: &mut crate::companion::digest::Fold, envelope: &Envelope) {
    fold.absorb_count(envelope.captures.len());
    for capture in &envelope.captures {
        fold.absorb(u64::from(capture.index));
        match capture.value {
            Scalar::I64(value) => {
                fold.absorb(1);
                fold.absorb(u64::from_le_bytes(value.to_le_bytes()));
            }
            Scalar::Bool(value) => {
                fold.absorb(2);
                fold.absorb(u64::from(value));
            }
        }
    }
    fold.absorb_count(envelope.dependencies.len());
    for dependency in &envelope.dependencies {
        fold.absorb(dependency.identity);
        fold.absorb(u64::from(dependency.format));
        fold.absorb(u64::from(dependency.semantic_revision));
        fold.absorb_bytes(&dependency.source_revision);
    }
    fold.absorb_count(envelope.instantiations.len());
    for instantiation in &envelope.instantiations {
        fold.absorb_count(instantiation.bindings.len());
        for binding in &instantiation.bindings {
            fold.absorb(u64::from(binding.variable));
            fold.absorb(match binding.kind {
                noble_kernel::words::VariableKind::Stack => 1,
                noble_kernel::words::VariableKind::Value => 2,
                noble_kernel::words::VariableKind::Effect => 3,
            });
            absorb_ty(fold, &binding.ty);
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; identity folding must hash even malformed public envelope inputs without panicking, leaving validation to typed refusal paths"
)]
fn absorb_identity_recipe(fold: &mut crate::companion::digest::Fold, envelope: &Envelope) {
    fold.absorb_count(envelope.body.nodes.len());
    for node in &envelope.body.nodes {
        match node {
            Node::I64(value) => {
                fold.absorb(1);
                fold.absorb(u64::from_le_bytes(value.to_le_bytes()));
            }
            Node::Bool(value) => {
                fold.absorb(2);
                fold.absorb(u64::from(*value));
            }
            Node::Capture { index } => {
                fold.absorb(3);
                fold.absorb(u64::from(*index));
            }
            Node::Operation { op, instantiation } => {
                fold.absorb(4);
                fold.absorb(u64::from(op.slot()));
                fold.absorb(u64::from(*instantiation));
            }
            Node::Invoke {
                dependency,
                instantiation,
            } => {
                fold.absorb(5);
                fold.absorb(u64::from(*dependency));
                fold.absorb(u64::from(*instantiation));
            }
            Node::Quotation {
                body,
                instantiation,
            } => {
                fold.absorb(6);
                fold.absorb(u64::from(*instantiation));
                fold.absorb_count(body.len());
                for reference in body {
                    fold.absorb(u64::from(*reference));
                }
            }
        }
    }
    fold.absorb_count(envelope.body.entry.len());
    for reference in &envelope.body.entry {
        fold.absorb(u64::from(*reference));
    }
}

/// One lane of the envelope identity fold over canonical body fields only.
fn identity_lane(envelope: &Envelope, lane: usize) -> u64 {
    let mut fold = crate::companion::digest::Fold::new(crate::companion::digest::DOMAIN_CHUNK);
    fold.absorb_count(lane);
    fold.absorb(u64::from(envelope.format));
    fold.absorb(u64::from(envelope.semantic_revision));
    fold.absorb_bytes(envelope.profile.as_bytes());
    absorb_identity_interface(&mut fold, envelope);
    absorb_identity_retained_values(&mut fold, envelope);
    absorb_identity_recipe(&mut fold, envelope);
    fold.finish()
}

/// The lookup identity of one envelope's canonical body. It is recomputed at
/// admission from retained bytes, so a forged identity is refused rather than
/// resolved.
pub fn chunk_identity_of(envelope: &Envelope) -> [u8; IDENTITY_BYTES] {
    let mut lanes_out = [0u8; IDENTITY_BYTES];
    let mut lane = 0usize;
    while lane < IDENTITY_BYTES / 8 {
        let value = identity_lane(envelope, lane);
        let bytes = value.to_le_bytes();
        let base = lane.saturating_mul(8);
        let mut at = 0usize;
        while at < bytes.len() {
            if let Some(slot) = lanes_out.get_mut(base.saturating_add(at)) {
                *slot = bytes[at];
            }
            at = at.saturating_add(1);
        }
        lane = lane.saturating_add(1);
    }
    lanes_out
}

impl Op {
    const fn slot(self) -> u32 {
        match self {
            Op::Dup => 0,
            Op::Drop => 1,
            Op::Swap => 2,
            Op::Add => 3,
            Op::Sub => 4,
            Op::Mul => 5,
            Op::Eq => 6,
            Op::Run => 7,
        }
    }
}

/// The expansion of one recipe into flat emitted elements.
struct Expansion {
    elements: alloc::vec::Vec<Element>,
    expanded_nodes: u32,
}

/// One expansion frame: the node arena it walks (by index, so the frame holds
/// no borrow), the reference list being walked and its progress.
struct Segment {
    /// `None` is the envelope being expanded; `Some(index)` is a library entry.
    owner: Option<u16>,
    /// `None` walks the owner's entry list; `Some(node)` that quotation's body.
    quotation: Option<usize>,
    at: usize,
    close: bool,
}

/// The mutable state of one recipe expansion.
struct ExpansionWalk {
    frames: alloc::vec::Vec<Segment>,
    elements: alloc::vec::Vec<Element>,
    depth: u32,
    spent: u32,
}

/// The envelope and reference list one expansion frame walks.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; segment lookups use the slice `get`, which is not a stable const fn"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; a missing owner entry or quotation node is returned as `None`, which the expansion turns into a typed refusal, so there is no invariant here to assert"
)]
fn segment_body<'a>(
    entries: &'a [Entry],
    root: &'a Envelope,
    segment: &Segment,
) -> Option<(&'a Envelope, &'a [u32])> {
    let owner = match segment.owner {
        None => root,
        Some(index) => match entries.get(usize::from(index)) {
            Some(entry) => &entry.envelope,
            None => return None,
        },
    };
    match segment.quotation {
        None => Some((owner, owner.body.entry.as_slice())),
        Some(node) => match owner.body.nodes.get(node) {
            Some(Node::Quotation { body, .. }) => Some((owner, body.as_slice())),
            Some(
                Node::I64(_)
                | Node::Bool(_)
                | Node::Capture { .. }
                | Node::Operation { .. }
                | Node::Invoke { .. },
            )
            | None => None,
        },
    }
}

/// Expand one envelope's recipe, inlining every invoked library entry, into the
/// flat element list that is emitted. A dependency edge, a quotation level and
/// an instantiation binding are each charged here; nothing is inferred from a
/// signature.
fn expand_recipe(
    entries: &[Entry],
    envelope: &Envelope,
    ledger: &mut Ledger,
    at: Location,
) -> Result<Expansion, Refusal> {
    let mut walk = ExpansionWalk {
        frames: alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES),
        elements: alloc::vec::Vec::with_capacity(MAX_EXPANDED_NODE_CAPACITY),
        depth: 0,
        spent: 0,
    };
    walk.frames.push(Segment {
        owner: None,
        quotation: None,
        at: 0,
        close: false,
    });
    let mut outcome = Ok(());
    while outcome.is_ok() && !walk.frames.is_empty() {
        outcome = expand_step(entries, envelope, &mut walk, ledger, at.clone());
    }
    outcome?;
    if !walk.frames.is_empty() || walk.depth != 0 {
        return Err(Refusal::new(
            Class::Internal,
            at,
            "recipe expansion ended mid-quotation",
        ));
    }
    debug_assert_eq!(walk.depth, 0);
    debug_assert!(walk.spent <= MAX_EXPANDED_NODES);
    Ok(Expansion {
        elements: walk.elements,
        expanded_nodes: walk.spent,
    })
}

/// One expansion step: close the finished top frame or expand its next node.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; one explicit frame step expands a recipe node and its dependencies; splitting it would separate each emitted element from the bound that refused it."
)]
fn expand_step(
    entries: &[Entry],
    envelope: &Envelope,
    walk: &mut ExpansionWalk,
    ledger: &mut Ledger,
    at: Location,
) -> Result<(), Refusal> {
    debug_assert!(!walk.frames.is_empty());
    let Some(top) = walk.frames.len().checked_sub(1) else {
        return Ok(());
    };
    let Some(frame) = walk.frames.get(top) else {
        return Err(Refusal::new(
            Class::Internal,
            at,
            "expansion lost its frame",
        ));
    };
    let Some((owner, body)) = segment_body(entries, envelope, frame) else {
        return Err(Refusal::new(
            Class::Internal,
            at,
            "expansion lost its frame",
        ));
    };
    let owner_index = frame.owner;
    let position = frame.at;
    if position >= body.len() {
        let should_close = frame.close;
        let _ = walk.frames.pop();
        if should_close {
            walk.depth = walk.depth.saturating_sub(1);
            ledger.charge(Resource::ExpandedNodes, 1, at)?;
            walk.elements.push(Element::Close);
            walk.spent = walk.spent.saturating_add(1);
        }
        return Ok(());
    }
    if let Some(frame) = walk.frames.get_mut(top) {
        frame.at = position.saturating_add(1);
    }
    let Some(reference) = body.get(position).copied() else {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "recipe reference list ends unexpectedly",
        ));
    };
    let Some(node_index) = usize::try_from(reference).ok() else {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "recipe node reference exceeds the host address space",
        ));
    };
    let Some(node) = owner.body.nodes.get(node_index) else {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "recipe node reference points outside the finite arena",
        ));
    };
    if !matches!(node, Node::Invoke { .. }) {
        ledger.charge(Resource::ExpandedNodes, 1, at.clone())?;
    }
    match node {
        Node::I64(value) => walk.elements.push(Element::I64(*value)),
        Node::Bool(value) => walk.elements.push(Element::Bool(*value)),
        Node::Capture { index } => {
            let Some(capture) = owner.captures.get(usize::from(*index)) else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "recipe capture index is outside the retained capture list",
                ));
            };
            match capture.value {
                Scalar::I64(value) => walk.elements.push(Element::I64(value)),
                Scalar::Bool(value) => walk.elements.push(Element::Bool(value)),
            }
        }
        Node::Operation { op, instantiation } => {
            charge_instantiation(owner, *instantiation, ledger, at.clone())?;
            let Some(witness) = owner.instantiations.get(usize::from(*instantiation)) else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "operation witness is outside the retained instantiation list",
                ));
            };
            walk.elements.push(Element::Op {
                op: *op,
                witness: Some(witness.clone()),
            });
        }
        Node::Invoke {
            dependency,
            instantiation,
        } => {
            let node_at = Location::at("envelope.body.nodes", reference);
            charge_instantiation(owner, *instantiation, ledger, node_at.clone())?;
            check_closed_instantiation(owner, *instantiation, node_at)?;
            ledger.charge(Resource::DependencySteps, 1, at.clone())?;
            let Some(record) = owner.dependencies.get(usize::from(*dependency)) else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "recipe dependency index is outside the retained dependency list",
                ));
            };
            let Some(target) = find_entry(entries, record) else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "recipe dependency record does not resolve to a retained earlier entry",
                ));
            };
            if entries.get(usize::from(target)).is_none() {
                return Err(Refusal::new(
                    Class::Internal,
                    at,
                    "resolved dependency is not retained",
                ));
            }
            walk.frames.push(Segment {
                owner: Some(target),
                quotation: None,
                at: 0,
                close: false,
            });
            return Ok(());
        }
        Node::Quotation { instantiation, .. } => {
            let node_at = Location::at("envelope.body.nodes", reference);
            charge_instantiation(owner, *instantiation, ledger, node_at.clone())?;
            check_closed_instantiation(owner, *instantiation, node_at)?;
            walk.depth = walk.depth.saturating_add(1);
            ledger.observe(Resource::QuotationDepth, walk.depth, at)?;
            walk.elements.push(Element::Open);
            walk.spent = walk.spent.saturating_add(1);
            walk.frames.push(Segment {
                owner: owner_index,
                quotation: Some(node_index),
                at: 0,
                close: true,
            });
            return Ok(());
        }
    }
    walk.spent = walk.spent.saturating_add(1);
    debug_assert!(walk.spent > 0);
    Ok(())
}

fn charge_instantiation(
    envelope: &Envelope,
    index: u16,
    ledger: &mut Ledger,
    at: Location,
) -> Result<(), Refusal> {
    let Some(instantiation) = envelope.instantiations.get(usize::from(index)) else {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "recipe instantiation index is outside the retained witness list",
        ));
    };
    ledger.charge_count(
        Resource::InstantiationBindings,
        encoded_len(instantiation.bindings.len()),
        at,
    )
}

fn check_closed_instantiation(
    envelope: &Envelope,
    index: u16,
    at: Location,
) -> Result<(), Refusal> {
    let Some(instantiation) = envelope.instantiations.get(usize::from(index)) else {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "recipe instantiation index is outside the retained witness list",
        ));
    };
    if instantiation.bindings.is_empty() {
        Ok(())
    } else {
        Err(Refusal::new(
            Class::Invalid,
            at,
            "closed dependency and quotation nodes require an empty type witness",
        ))
    }
}

fn find_entry(entries: &[Entry], reference: &DefinitionRef) -> Option<u16> {
    let mut at = 0usize;
    while let Some(entry) = entries.get(at) {
        if entry_matches(entry, reference) {
            return u16::try_from(at).ok();
        }
        at = at.saturating_add(1);
    }
    None
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; identity comparison uses the non-const derived array equality"
)]
fn entry_matches(entry: &Entry, reference: &DefinitionRef) -> bool {
    if entry.identity != reference.identity {
        return false;
    }
    if entry.envelope.format != reference.format {
        return false;
    }
    if entry.envelope.semantic_revision != reference.semantic_revision {
        return false;
    }
    entry.envelope.source_revision == reference.source_revision
}

/// Emit the canonical source for one flat element list. Every token is a fixed
/// spelling or a decimal literal, so the bytes are canonical for the element
/// list they came from.
fn emit_i64(value: i64, digits: &mut [u8; 20]) -> usize {
    let mut start = digits.len();
    let mut magnitude = value.unsigned_abs();
    loop {
        start = start.saturating_sub(1);
        let digit = u8::try_from(magnitude % 10).unwrap_or(0);
        if let Some(slot) = digits.get_mut(start) {
            *slot = b'0'.saturating_add(digit);
        }
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    if value < 0 {
        start = start.saturating_sub(1);
        if let Some(slot) = digits.get_mut(start) {
            *slot = b'-';
        }
    }
    debug_assert!(start < digits.len());
    debug_assert!(magnitude == 0);
    start
}

/// The source bytes emitted so far plus the decimal scratch buffer.
struct Emission {
    out: alloc::vec::Vec<u8>,
    digits: [u8; 20],
}

/// Append one element's canonical spelling and its separator.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; emission appends tokens to the runtime output Vec"
)]
fn emit_element(emission: &mut Emission, element: &Element, at: &Location) -> Result<(), Refusal> {
    debug_assert!(encoded_len(emission.out.len()) <= u64::from(MAX_SOURCE_BYTES));
    match element {
        Element::I64(value) => {
            let start = emit_i64(*value, &mut emission.digits);
            if let Some(encoded) = emission.digits.get(start..) {
                emission.out.extend_from_slice(encoded);
            }
        }
        Element::Bool(true) => emission.out.extend_from_slice(b"true"),
        Element::Bool(false) => emission.out.extend_from_slice(b"false"),
        Element::Op { op, .. } => emission.out.extend_from_slice(op.spelling().as_bytes()),
        Element::Open => emission.out.push(b'['),
        Element::Close => emission.out.push(b']'),
        Element::Hole => {
            return Err(Refusal::new(
                Class::Invalid,
                at.clone(),
                "an editor hole cannot be emitted as source",
            ))
        }
    }
    emission.out.push(b' ');
    if u32::try_from(emission.out.len()).map_or(true, |length| length > MAX_SOURCE_BYTES) {
        return Err(byte_budget_refusal(
            encoded_len(emission.out.len()),
            at.clone(),
        ));
    }
    debug_assert!(!emission.out.is_empty());
    Ok(())
}

fn emit(elements: &[Element], at: Location) -> Result<alloc::vec::Vec<u8>, Refusal> {
    let mut emission = Emission {
        out: alloc::vec::Vec::with_capacity(elements.len()),
        digits: [0u8; 20],
    };
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < elements.len() {
        outcome = emit_element(&mut emission, &elements[index], &at);
        index = index.saturating_add(1);
    }
    outcome?;
    let out = emission.out;
    debug_assert!(out.len() >= elements.len());
    debug_assert!(u32::try_from(out.len()).is_ok_and(|length| length <= MAX_SOURCE_BYTES));
    Ok(out)
}

fn byte_budget_refusal(observed: u64, at: Location) -> Refusal {
    let mut refusal = Refusal::new(
        Class::Exhausted,
        at,
        "emitted source exceeds the fixed byte bound",
    );
    refusal.resource = Some(Resource::SourceBytes);
    let mut observed_bytes = u32::MAX;
    if let Ok(representable) = u32::try_from(observed) {
        observed_bytes = representable;
    }
    refusal.observed = observed_bytes;
    refusal.limit = MAX_SOURCE_BYTES;
    refusal
}

fn integer_token_len(value: i64) -> u64 {
    let mut magnitude = value.unsigned_abs();
    let mut digits = 1u64;
    while magnitude >= 10 {
        magnitude /= 10;
        digits = digits.saturating_add(1);
    }
    digits.saturating_add(u64::from(value < 0))
}

/// The canonical token bytes of one element; the hole contributes none.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; integer token widths come from the non-const digit counter"
)]
fn element_token_len(element: &Element) -> u64 {
    match element {
        Element::I64(value) => integer_token_len(*value),
        Element::Bool(true) => 4,
        Element::Bool(false) => 5,
        Element::Op { op, .. } => encoded_len(op.spelling().len()),
        Element::Open | Element::Close => 1,
        Element::Hole => 0,
    }
}

/// Lower-bound a completed source's bytes from its editor context, omitting
/// the single hole. No proposal can make this context smaller.
fn editor_context_bytes(elements: &[Element], at: Location) -> Result<u32, Refusal> {
    let mut bytes = 0u64;
    let mut index = 0usize;
    while bytes <= u64::from(MAX_SOURCE_BYTES) && index < elements.len() {
        let token = element_token_len(&elements[index]);
        if token != 0 {
            bytes = bytes.saturating_add(token).saturating_add(1);
        }
        index = index.saturating_add(1);
    }
    if bytes > u64::from(MAX_SOURCE_BYTES) {
        return Err(byte_budget_refusal(bytes, at));
    }
    debug_assert!(bytes <= u64::from(MAX_SOURCE_BYTES));
    let mut non_hole_count = 0usize;
    for element in elements {
        if !matches!(element, Element::Hole) {
            non_hole_count = non_hole_count.saturating_add(1);
        }
    }
    debug_assert!(bytes >= encoded_len(non_hole_count));
    u32::try_from(bytes).map_err(|_| byte_budget_refusal(u64::MAX, at))
}

/// The ordered-stack replay. It types exactly the profile's operations and
/// types a quotation literal at its construction site's concrete stack, so it
/// can only refuse work the ordinary pipeline would accept, never accept work
/// the pipeline refuses.
struct Replay {
    stack: alloc::vec::Vec<Ty>,
    ambient: alloc::vec::Vec<alloc::vec::Vec<Ty>>,
    depth: u32,
    steps: u32,
    /// The stack immediately after the recorded boundary step, when one was
    /// requested. It is the exact stack the hole's replacement produces.
    mark: Option<alloc::vec::Vec<Ty>>,
}

impl Replay {
    fn new(inputs: &[Ty]) -> Self {
        Self {
            stack: inputs.to_vec(),
            ambient: alloc::vec::Vec::new(),
            depth: 0,
            steps: 0,
            mark: None,
        }
    }

    /// Whether the recorded boundary stack is exactly `stack`.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the recorded stack is compared with slice equality, which is not a stable const trait"
    )]
    fn marks(&self, stack: &[Ty]) -> bool {
        match &self.mark {
            Some(mark) => *mark.as_slice() == *stack,
            None => false,
        }
    }

    fn push(&mut self, ty: Ty, ledger: &mut Ledger, at: &Location) -> Result<(), Refusal> {
        self.stack.push(ty);
        ledger.observe_count(
            Resource::StackHeight,
            encoded_len(self.stack.len()),
            at.clone(),
        )
    }

    fn pop(&mut self, at: &Location) -> Result<Ty, Refusal> {
        match self.stack.pop() {
            Some(ty) => Ok(ty),
            None => Err(Refusal::new(
                Class::Invalid,
                at.clone(),
                "ordered stack underflow while replaying the profile",
            )),
        }
    }

    fn pop_i64(&mut self, at: &Location) -> Result<(), Refusal> {
        let ty = self.pop(at)?;
        if ty != Ty::I64 {
            return Err(Refusal::new(
                Class::Invalid,
                at.clone(),
                "the profile operation requires an I64 operand",
            ));
        }
        Ok(())
    }

    fn height(&self) -> usize {
        self.stack.len()
    }

    fn step(
        &mut self,
        element: &Element,
        ledger: &mut Ledger,
        context: &Location,
        index: u32,
    ) -> Result<(), Refusal> {
        let at = Location::at(&context.field, index);
        ledger.charge(Resource::CheckingWork, 1, at.clone())?;
        match element {
            Element::I64(_) => self.push(Ty::I64, ledger, &at),
            Element::Bool(_) => self.push(Ty::Bool, ledger, &at),
            Element::Open => {
                self.depth = self.depth.saturating_add(1);
                ledger.observe(Resource::QuotationDepth, self.depth, at)?;
                self.ambient.push(self.stack.clone());
                Ok(())
            }
            Element::Close => {
                let Some(ambient) = self.ambient.pop() else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "quotation closes without an opening bracket",
                    ));
                };
                self.depth = self.depth.saturating_sub(1);
                let result = self.stack.clone();
                self.stack = ambient.clone();
                self.push(
                    Ty::Program(
                        alloc::boxed::Box::new(ambient),
                        alloc::boxed::Box::new(result),
                        noble_kernel::types::EffSet::empty(),
                    ),
                    ledger,
                    &at,
                )
            }
            Element::Op { op, witness } => self.operation(*op, witness.as_ref(), ledger, &at),
            Element::Hole => Err(Refusal::new(
                Class::Internal,
                at,
                "replay reached an unexpanded hole",
            )),
        }
    }

    fn operation(
        &mut self,
        op: Op,
        witness: Option<&Instantiation>,
        ledger: &mut Ledger,
        at: &Location,
    ) -> Result<(), Refusal> {
        match op {
            Op::Dup => {
                let Some(value) = self.stack.last() else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at.clone(),
                        "ordered stack underflow while replaying the profile",
                    ));
                };
                if !witness_matches(witness, &[(1, value.clone())]) {
                    return Err(invalid_witness(at.clone()));
                }
                let value = self.pop(at)?;
                self.push(value.clone(), ledger, at)?;
                self.push(value, ledger, at)
            }
            Op::Drop => {
                let Some(value) = self.stack.last() else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at.clone(),
                        "ordered stack underflow while replaying the profile",
                    ));
                };
                if !witness_matches(witness, &[(1, value.clone())]) {
                    return Err(invalid_witness(at.clone()));
                }
                let _ = self.pop(at)?;
                Ok(())
            }
            Op::Swap => {
                let split = self.stack.len().saturating_sub(2);
                let pair = if self.stack.len() >= 2 {
                    (self.stack.get(split), self.stack.last())
                } else {
                    (None, None)
                };
                let expected = match pair {
                    (Some(lower), Some(top)) => [(1, lower.clone()), (2, top.clone())],
                    _ => {
                        return Err(Refusal::new(
                            Class::Invalid,
                            at.clone(),
                            "ordered stack underflow while replaying the profile",
                        ))
                    }
                };
                if !witness_matches(witness, &expected) {
                    return Err(invalid_witness(at.clone()));
                }
                let top = self.pop(at)?;
                let next = self.pop(at)?;
                self.push(top, ledger, at)?;
                self.push(next, ledger, at)
            }
            Op::Add | Op::Sub | Op::Mul => {
                if !witness_matches(witness, &[]) {
                    return Err(invalid_witness(at.clone()));
                }
                self.pop_i64(at)?;
                self.pop_i64(at)?;
                self.push(Ty::I64, ledger, at)
            }
            Op::Eq => {
                if !witness_matches(witness, &[]) {
                    return Err(invalid_witness(at.clone()));
                }
                self.pop_i64(at)?;
                self.pop_i64(at)?;
                self.push(Ty::Bool, ledger, at)
            }
            Op::Run => {
                if !witness_matches(witness, &[]) {
                    return Err(invalid_witness(at.clone()));
                }
                let value = self.pop(at)?;
                let Ty::Program(required, produced, _) = value else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at.clone(),
                        "run operand is not a quotation value",
                    ));
                };
                if self.stack.as_slice() != required.as_slice() {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at.clone(),
                        "run operand does not match the ordered stack it executes on",
                    ));
                }
                self.stack = produced.as_ref().clone();
                ledger.observe_count(
                    Resource::StackHeight,
                    encoded_len(self.height()),
                    at.clone(),
                )
            }
        }
    }

    /// Replay one element list. The stack after the last element is the result;
    /// `boundary` additionally records the stack once that many steps ran.
    fn run(
        &mut self,
        elements: &[Element],
        boundary: Option<usize>,
        ledger: &mut Ledger,
        require_closed: bool,
        context: Location,
    ) -> Result<(), Refusal> {
        if boundary == Some(0) {
            self.mark = Some(self.stack.clone());
        }
        let mut index = 0usize;
        while index < elements.len() {
            let Some(element) = elements.get(index) else {
                return Err(internal(
                    context,
                    "replay index exceeds the checked element slice",
                ));
            };
            let position = index as u32;
            self.steps = self.steps.saturating_add(1);
            self.step(element, ledger, &context, position)?;
            if boundary == Some(index.saturating_add(1)) {
                self.mark = Some(self.stack.clone());
            }
            index = index.saturating_add(1);
        }
        if !require_closed || self.ambient.is_empty() {
            Ok(())
        } else {
            Err(Refusal::new(
                Class::Invalid,
                Location::field(&context.field),
                "replay ended inside an unclosed quotation",
            ))
        }
    }
}

fn witness_matches(witness: Option<&Instantiation>, expected: &[(u16, Ty)]) -> bool {
    let Some(witness) = witness else {
        return true;
    };
    if witness.bindings.len() != expected.len() {
        return false;
    }
    debug_assert_eq!(witness.bindings.len(), expected.len());
    let mut is_match = true;
    let mut index = 0usize;
    while is_match && index < expected.len() {
        is_match = binding_matches_at(&witness.bindings, expected, index);
        index = index.saturating_add(1);
    }
    debug_assert!(!is_match || index == expected.len());
    is_match
}

/// Whether the witness binding at `index` binds the expected value variable and type.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; witness lookups use the non-const slice `get` and derived type equality"
)]
fn binding_matches_at(bindings: &[Binding], expected: &[(u16, Ty)], index: usize) -> bool {
    match (bindings.get(index), expected.get(index)) {
        (Some(binding), Some((variable, ty))) => {
            binding.variable == *variable
                && binding.kind == noble_kernel::words::VariableKind::Value
                && binding.ty.eq(ty)
        }
        _ => false,
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; constructing a refusal allocates its message"
)]
fn invalid_witness(at: Location) -> Refusal {
    Refusal::new(
        Class::Invalid,
        at,
        "retained operation instantiation does not match its ordered stack types",
    )
}

fn replay_interface(
    elements: &[Element],
    interface: &Interface,
    ledger: &mut Ledger,
    at: Location,
) -> Result<(), Refusal> {
    let mut replay = Replay::new(&interface.stack_in);
    replay.run(elements, None, ledger, true, at.clone())?;
    if replay.stack != interface.stack_out {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "recipe replay differs from the declared ordered interface",
        ));
    }
    Ok(())
}

/// Type and effect requirement of one completion task. The host supplies it and
/// no completion may rewrite it.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub stack_in: alloc::vec::Vec<Ty>,
    pub stack_out: alloc::vec::Vec<Ty>,
    pub allowed_effects: noble_kernel::types::EffSet,
}

/// One host-retained expected behavior case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
    pub inputs: alloc::vec::Vec<Scalar>,
    pub outputs: alloc::vec::Vec<Scalar>,
}

/// A host-retained fixture set. Expected values come from the host, never from a
/// proposal or a chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureSet {
    pub id: [u8; IDENTITY_BYTES],
    pub cases: alloc::vec::Vec<Fixture>,
}

/// The fixed completion request.
#[derive(Clone, Debug)]
pub struct Request {
    pub format: u32,
    pub semantic_revision: u32,
    pub profile: alloc::string::String,
    pub task: Task,
    pub editor: super::editor::Candidate,
    pub hole_path: alloc::vec::Vec<u32>,
    pub library_revision: [u8; REVISION_BYTES],
    pub fixtures: Option<FixtureSet>,
    pub budget: Budget,
}

/// One member of the finite shortlex candidate queue. `ordinal` is the
/// original queue position and `sequence` contains indices into the exact
/// library snapshot supplied to the completion call.
#[derive(Clone, Debug, PartialEq)]
pub struct QueueCandidate {
    pub ordinal: u32,
    pub sequence: alloc::vec::Vec<u16>,
    ranking_position: u32,
    score: Option<f32>,
    identity: Option<CandidateId>,
    feature_extraction_ns: u64,
    scorer_elapsed_ns: u64,
}

impl QueueCandidate {
    fn unscored(ordinal: u32, sequence: alloc::vec::Vec<u16>) -> Self {
        Self {
            ordinal,
            sequence,
            ranking_position: ordinal,
            score: None,
            identity: None,
            feature_extraction_ns: 0,
            scorer_elapsed_ns: 0,
        }
    }
}

/// The requested number of canonical candidate entries to materialize.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateLimit(u32);

impl From<u32> for CandidateLimit {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// A bounded prefix of the canonical shortlex queue. `has_more` records
/// whether the candidate cap omitted a subsequent canonical candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateQueue {
    candidates: alloc::vec::Vec<QueueCandidate>,
    next_candidate: Option<QueueCandidate>,
}

impl CandidateQueue {
    /// The candidates in original shortlex order.
    pub fn candidates(&self) -> &[QueueCandidate] {
        &self.candidates
    }

    /// Whether at least one candidate follows this bounded prefix.
    pub const fn has_more(&self) -> bool {
        self.next_candidate.is_some()
    }
}

/// One attempted candidate outcome observed by a caller-owned collector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateOutcome {
    pub request_identity: [u8; REVISION_BYTES],
    pub candidate: CandidateId,
    pub ordinal: u32,
    pub sequence: alloc::vec::Vec<u16>,
    pub accepted: bool,
    pub refusal_class: Option<Class>,
    pub fixture_agreement: Option<bool>,
    pub cost: CandidateAttemptCost,
}

/// Per-attempt scorer and ordinary re-admission provenance. Resource deltas
/// follow [`Resource::ALL`]; elapsed fields are monotonic nanoseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateAttemptCost {
    pub feature_extraction_ns: u64,
    pub scorer_elapsed_ns: u64,
    pub re_admission_ns: u64,
    pub resource_deltas: [u32; Resource::COUNT],
}

/// Caller-owned sink for the bounded candidate attempt stream.
pub type CandidateOutcomeObserver<'a> = dyn FnMut(&CandidateOutcome) + 'a;

/// The caller's candidate-outcome observer as the search holds it.
///
/// Reviewed extraction exception (#97): Aeneas cannot represent `dyn`
/// closure types, so this host plug-in handle is an opaque type and its
/// `observe` is opaque; translated code only moves the handle and calls it.
#[charon::opaque]
struct ObserverHandle<'a>(&'a mut CandidateOutcomeObserver<'a>);

impl ObserverHandle<'_> {
    /// Deliver one attempted candidate outcome to the caller's observer.
    ///
    /// Reviewed extraction exception (#97): opaque because it calls through
    /// the trait object.
    #[charon::opaque]
    fn observe(&mut self, outcome: &CandidateOutcome) {
        (self.0)(outcome);
    }
}

/// Exact, opaque identity of one candidate in a bounded ranking request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateId {
    /// Original position in the canonical shortlex queue.
    pub ordinal: u32,
    /// Digest of the library identity and ordered chunk identities.
    pub digest: [u8; REVISION_BYTES],
}

/// Immutable trust-boundary identities bound to a scorer request and response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankingIdentity {
    /// Digest of every field in this identity, including the frozen model id.
    pub request: [u8; REVISION_BYTES],
    /// Ordered stack/effect contract identity.
    pub task_interface: [u8; REVISION_BYTES],
    /// Opaque digest of independently owned goal and fixture values.
    pub independent_goal: [u8; REVISION_BYTES],
    /// Digest of every candidate ID in this bounded queue and any overflow witness.
    pub candidate_set: [u8; REVISION_BYTES],
    /// Exact immutable chunk library revision.
    pub library_revision: [u8; REVISION_BYTES],
    /// Feature-vector schema revision.
    pub feature_revision: alloc::string::String,
    /// Source semantics used by #73.
    pub semantic_revision: u32,
    /// Policy, profile, request-envelope and caller-budget identity.
    pub policy: [u8; REVISION_BYTES],
    /// Exact editor tree and hole-path identity.
    pub editor_request: [u8; REVISION_BYTES],
    /// Caller-pinned scorer artifact identity, or `None` for an absent scorer.
    pub model_identity: Option<[u8; REVISION_BYTES]>,
    /// Exact caller-supplied budget (the profile still lowers it unchanged).
    pub budget: Budget,
}

/// A persisted ordering for one exact ranking request. Replay validates the
/// complete identity and candidate membership before it can affect search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredRanking {
    pub identity: RankingIdentity,
    pub ordered_candidates: alloc::vec::Vec<CandidateId>,
    pub invalid_scores: u32,
}

/// One candidate-scoring request. The scorer receives no source or expected result.
pub struct CandidateScoreRequest<'a> {
    /// Opaque token the adapter must echo; it is not a score feature.
    pub request_identity: [u8; REVISION_BYTES],
    /// Consumer-pinned model id, supplied only for response validation.
    pub model_identity: [u8; REVISION_BYTES],
    /// Current eligible candidate id, supplied only for response echo.
    pub candidate: CandidateId,
    /// The only scoring input.
    pub features: &'a CandidateFeatureVector,
}

/// Adapter reply bound to the exact request, model artifact and eligible candidate.
#[derive(Clone, Copy, Debug)]
pub struct CandidateScoreResponse {
    pub request_identity: [u8; REVISION_BYTES],
    pub model_identity: [u8; REVISION_BYTES],
    pub candidate: CandidateId,
    pub score: f32,
}

impl PartialEq for CandidateScoreResponse {
    /// Field-wise equality exactly as derived; the score compares through
    /// `f32`'s `PartialEq`, which Aeneas translates (a raw float `==` it does not).
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; a `PartialEq` impl method cannot be const because `PartialEq` is not a stable const trait"
    )]
    fn eq(&self, other: &Self) -> bool {
        self.request_identity == other.request_identity
            && self.model_identity == other.model_identity
            && self.candidate == other.candidate
            && PartialEq::eq(&self.score, &other.score)
    }
}

/// Host-pinned model identity and one optional local scorer adapter.
pub struct RankerSession<'a> {
    ranker: RankerHandle<'a>,
    expected_model_identity: [u8; REVISION_BYTES],
}

impl<'a> RankerSession<'a> {
    /// Bind an adapter to the consumer-selected immutable model identity.
    ///
    /// Reviewed extraction exclusion (#97): the signature names a `dyn`
    /// trait object; the checked logic is `RankerHandle` and the completion
    /// functions that receive this session.
    #[charon::exclude]
    pub const fn new(
        ranker: &'a mut dyn CandidateRanker,
        expected_model_identity: [u8; REVISION_BYTES],
    ) -> Self {
        Self {
            ranker: RankerHandle(ranker),
            expected_model_identity,
        }
    }

    /// A session reborrowing this one's scorer for one completion call.
    ///
    /// Reviewed extraction exclusion (#97): the body reborrows the `dyn`
    /// scorer inside the opaque handle; only the excluded public completion
    /// adapters call it.
    #[charon::exclude]
    fn reborrow(&mut self) -> RankerSession<'_> {
        RankerSession {
            ranker: RankerHandle(&mut *self.ranker.0),
            expected_model_identity: self.expected_model_identity,
        }
    }
}

/// The host scorer adapter as a ranker session holds it.
///
/// Reviewed extraction exception (#97): Aeneas cannot represent trait
/// objects, so this host plug-in handle is an opaque type and its three
/// methods are opaque; translated code only reaches the scorer through them.
#[charon::opaque]
struct RankerHandle<'a>(&'a mut dyn CandidateRanker);

impl RankerHandle<'_> {
    /// The scorer's immutable model artifact identity.
    ///
    /// Reviewed extraction exception (#97): opaque because it calls through
    /// the trait object.
    #[charon::opaque]
    fn model_identity(&self) -> [u8; REVISION_BYTES] {
        self.0.model_identity()
    }

    /// Whether the scorer's lineage matches the bound ranking identity.
    ///
    /// Reviewed extraction exception (#97): opaque because it calls through
    /// the trait object.
    #[charon::opaque]
    fn provenance_matches(&self, identity: &RankingIdentity) -> bool {
        self.0.provenance_matches(identity)
    }

    /// Score one eligible candidate.
    ///
    /// Reviewed extraction exception (#97): opaque because it calls through
    /// the trait object.
    #[charon::opaque]
    fn score(
        &mut self,
        request: &CandidateScoreRequest<'_>,
    ) -> Result<CandidateScoreResponse, ScorerFailure> {
        self.0.score(request)
    }
}

/// Exact fixed-width feature record a candidate scorer may observe.
pub const FEATURE_LANES: usize = 64;
/// Revision of the request/queue/admitted-metadata feature layout.
pub const FEATURE_REVISION: &str = "tchunk-features/1";
/// One shared checking-work unit is charged for each encoded feature lane.
pub const FEATURE_EXTRACTION_WORK: u32 = 64;
/// One shared checking-work unit for binding the complete scorer request.
pub const RANKING_CONTEXT_WORK: u32 = 1;
/// One shared checking-work unit per candidate queue-management pass.
pub const RANKING_QUEUE_WORK: u32 = 5;
/// One shared checking-work unit for each candidate's opaque identity digest.
pub const RANKING_CANDIDATE_ID_WORK: u32 = 1;
/// One shared checking-work unit for each scorer invocation.
pub const RANKING_SCORE_WORK: u32 = 1;
pub type CandidateFeatureVector = [f32; FEATURE_LANES];

/// Why the optional untrusted ranking scorer could not be used.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScorerFailure {
    Unavailable,
    StaleProvenance,
    InvalidFeature,
    InvalidResponse,
    Evaluation,
}

/// A bounded scorer that can only assign a score to a candidate already in
/// the canonical queue. Implementations must bind their own model/feature
/// provenance; the completion engine still owns all candidate admission.
pub trait CandidateRanker {
    /// The immutable model artifact identity exposed by this adapter.
    fn model_identity(&self) -> [u8; REVISION_BYTES];

    /// Check graph, feature, training, tool, and library lineage against the request.
    fn provenance_matches(&self, identity: &RankingIdentity) -> bool;

    /// Score one eligible candidate. The response must echo all identity bindings;
    /// failure selects fixed shortlex and no source or expected result is exposed.
    fn score(
        &mut self,
        request: &CandidateScoreRequest<'_>,
    ) -> Result<CandidateScoreResponse, ScorerFailure>;
}

/// Outcome of applying a scorer to the bounded queue.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RankingDisposition {
    NotEvaluated,
    Applied,
    AppliedWithInvalidScores,
    FallbackStaleProvenance,
    FallbackScorerFailure,
    FallbackInvalidResponse,
    FallbackAllInvalidScores,
}

/// A ranked completion response plus non-authoritative scorer status.
#[derive(Clone, Debug)]
pub struct RankedResponse {
    pub response: Response,
    pub disposition: RankingDisposition,
    pub invalid_scores: u32,
    pub identity: Option<RankingIdentity>,
    /// IDs in the effective order, including shortlex fallbacks.
    pub ranked_candidates: alloc::vec::Vec<CandidateId>,
}

impl RankedResponse {
    /// Preserve a scorer-produced order for identity-checked replay.
    #[must_use]
    pub fn stored_ranking(&self) -> Option<StoredRanking> {
        if !matches!(
            self.disposition,
            RankingDisposition::Applied | RankingDisposition::AppliedWithInvalidScores
        ) {
            return None;
        }
        Some(StoredRanking {
            identity: self.identity.clone()?,
            ordered_candidates: self.ranked_candidates.clone(),
            invalid_scores: self.invalid_scores,
        })
    }
}

struct StoredRankingControl<'a> {
    ranking: &'a StoredRanking,
    expected_model_identity: [u8; REVISION_BYTES],
}

/// The host-supplied monotonic nanosecond clock.
///
/// Reviewed extraction exception (#97): Aeneas cannot represent `fn`
/// pointer types, so this host plug-in handle is an opaque type and its
/// `now_ns` is opaque; each read threads the clock state mutably, so two
/// reads are not assumed equal.
#[charon::opaque]
struct MonotonicClock(fn() -> u64);

impl MonotonicClock {
    /// Read the host clock.
    ///
    /// Reviewed extraction exception (#97): opaque because it calls the host
    /// function pointer.
    #[charon::opaque]
    fn now_ns(&mut self) -> u64 {
        (self.0)()
    }
}

struct CompletionControl<'session, 'callback, 'stored, 'clock> {
    ranker: Option<RankerSession<'session>>,
    observer: Option<ObserverHandle<'callback>>,
    stored: Option<StoredRankingControl<'stored>>,
    clock: &'clock mut MonotonicClock,
}

struct RankingStatus {
    disposition: RankingDisposition,
    invalid_scores: u32,
    identity: Option<RankingIdentity>,
    ranked_candidates: alloc::vec::Vec<CandidateId>,
}

impl RankingStatus {
    /// The status of a completion whose scorer has not run yet.
    const fn not_evaluated() -> Self {
        Self {
            disposition: RankingDisposition::NotEvaluated,
            invalid_scores: 0,
            identity: None,
            ranked_candidates: alloc::vec::Vec::new(),
        }
    }
}

/// The complete ordered stack equation one accepted proposal satisfies.
#[derive(Clone, Debug, PartialEq)]
pub struct Equation {
    /// The exact ordered stack at the hole, computed by replaying the prefix.
    pub stack_in: alloc::vec::Vec<Ty>,
    /// The exact ordered stack the selected chunks place there.
    pub stack_out: alloc::vec::Vec<Ty>,
    /// Positions below the selected chunks that the sequence leaves untouched.
    pub tail: u32,
}

/// One accepted completion: fresh source, its checked interface, the selected
/// chunk identities and the equation it satisfies.
#[derive(Clone, Debug)]
pub struct Accepted {
    #[charon::rename("source_bytes")]
    pub source: alloc::vec::Vec<u8>,
    pub candidate: noble_kernel::untrusted::Candidate,
    pub interface: Interface,
    pub selected: alloc::vec::Vec<[u8; IDENTITY_BYTES]>,
    pub equation: Equation,
}

/// One fixture case's reported agreement. It is behavioral evidence about this
/// proposal only, never a proof of the task contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureCaseReport {
    pub inputs: alloc::vec::Vec<Scalar>,
    pub expected: alloc::vec::Vec<Scalar>,
    pub actual: Option<alloc::vec::Vec<Scalar>>,
    pub agrees: bool,
}

/// Host fixture agreement for one accepted proposal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureReport {
    pub id: [u8; IDENTITY_BYTES],
    pub cases: alloc::vec::Vec<FixtureCaseReport>,
    pub agreement: bool,
    pub steps: u32,
}

/// What one completion attempt measured. Counts are work, not a speedup or a
/// completeness claim.
#[derive(Clone, Debug)]
pub struct Report {
    pub candidates: u32,
    pub checked_candidates: u32,
    pub accepted: u32,
    pub rejected: u32,
    pub diagnostics_dropped: u32,
    pub expanded_nodes: u32,
    pub dependency_steps: u32,
    pub instantiation_bindings: u32,
    pub checking_work: u32,
    pub evaluation_steps: u32,
    pub composed_chunks: u32,
    pub source_bytes: u32,
    pub stack_height: u32,
    pub quotation_depth: u32,
    pub retained_bytes: u32,
    /// Elapsed time measured by the host-supplied monotonic clock, in ns.
    pub total_latency_ns: u64,
    pub diagnostics: alloc::vec::Vec<Refusal>,
    pub fixtures: Option<FixtureReport>,
}

impl Report {
    fn of(ledger: &Ledger) -> Self {
        Self {
            candidates: ledger.observed(Resource::Candidates),
            checked_candidates: 0,
            accepted: 0,
            rejected: 0,
            diagnostics_dropped: 0,
            expanded_nodes: ledger.observed(Resource::ExpandedNodes),
            dependency_steps: ledger.observed(Resource::DependencySteps),
            instantiation_bindings: ledger.observed(Resource::InstantiationBindings),
            checking_work: ledger.observed(Resource::CheckingWork),
            evaluation_steps: ledger.observed(Resource::EvaluationSteps),
            composed_chunks: ledger.observed(Resource::ComposedChunks),
            source_bytes: ledger.observed(Resource::SourceBytes),
            stack_height: ledger.observed(Resource::StackHeight),
            quotation_depth: ledger.observed(Resource::QuotationDepth),
            retained_bytes: ledger.observed(Resource::RetainedBytes),
            total_latency_ns: 0,
            diagnostics: alloc::vec::Vec::new(),
            fixtures: None,
        }
    }
}

/// The completion response. `InternalFailure` is fail-closed and distinct from
/// `NoResult`.
#[octet::sealed_enum]
#[derive(Clone, Debug)]
pub enum Response {
    Accepted {
        accepted: alloc::boxed::Box<Accepted>,
        report: alloc::boxed::Box<Report>,
    },
    Invalid {
        refusal: Refusal,
        report: alloc::boxed::Box<Report>,
    },
    Unsupported {
        refusal: Refusal,
        report: alloc::boxed::Box<Report>,
    },
    Exhausted {
        refusal: Refusal,
        report: alloc::boxed::Box<Report>,
    },
    NoResult {
        report: alloc::boxed::Box<Report>,
    },
    InternalFailure {
        refusal: Refusal,
        report: alloc::boxed::Box<Report>,
    },
}

impl Response {
    /// The stable outcome name a client keys on.
    pub const fn outcome(&self) -> &'static str {
        match self {
            Response::Accepted { .. } => "accepted",
            Response::Invalid { .. } => "invalid",
            Response::Unsupported { .. } => "unsupported",
            Response::Exhausted { .. } => "exhausted",
            Response::NoResult { .. } => "no-result",
            Response::InternalFailure { .. } => "internal-failure",
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; this accessor mutably dereferences an owned Box"
    )]
    fn report_mut(&mut self) -> &mut Report {
        match self {
            Response::Accepted { report, .. }
            | Response::Invalid { report, .. }
            | Response::Unsupported { report, .. }
            | Response::Exhausted { report, .. }
            | Response::NoResult { report }
            | Response::InternalFailure { report, .. } => report,
        }
    }
}

impl Library {
    /// Admit one envelope into the snapshot.
    ///
    /// Retained bytes are recomputed from the envelope and the emitted source;
    /// the declared identity and source revision are compared against those
    /// recomputations, the emitted source is prepared by ordinary source
    /// admission, and the resulting submission is independently re-checked by
    /// [`noble_kernel::acceptance::check`]. Nothing here is published.
    pub fn admit(
        &mut self,
        namespace: &super::Session,
        envelope: Envelope,
    ) -> Result<u32, Refusal> {
        let mut ledger = Ledger::new(self.budget);
        ledger.charge_count(
            Resource::Entries,
            encoded_len(self.entries.len()),
            Location::field("library.entries"),
        )?;
        ledger.charge(
            Resource::LibraryBytes,
            self.retained,
            Location::field("library.bytes"),
        )?;
        ledger.charge(
            Resource::RetainedBytes,
            self.retained,
            Location::field("library.retained"),
        )?;
        ledger.charge(Resource::Entries, 1, Location::field("library.entries"))?;
        check_envelope(&envelope, &mut ledger)?;
        self.check_dependencies(&envelope)?;
        let body_at = Location::field("envelope.body");
        let envelope_bytes = envelope_size(&envelope, body_at.clone())?;
        ledger.charge(Resource::LibraryBytes, envelope_bytes, body_at.clone())?;
        ledger.charge(Resource::RetainedBytes, envelope_bytes, body_at.clone())?;
        let expansion = expand_recipe(&self.entries, &envelope, &mut ledger, body_at.clone())?;
        replay_interface(
            &expansion.elements,
            &envelope.interface,
            &mut ledger,
            Location::field("envelope.interface"),
        )?;
        let source = emit(&expansion.elements, body_at.clone())?;
        ledger.observe_count(
            Resource::SourceBytes,
            encoded_len(source.len()),
            body_at.clone(),
        )?;
        if canonical_revision(&source) != envelope.source_revision {
            return Err(Refusal::new(
                Class::Invalid,
                Location::field("envelope.source_revision"),
                "declared source revision does not match the canonical emitted source",
            ));
        }
        let admission = AdmissionContext {
            namespace,
            at: Location::field("envelope.interface"),
        };
        let (interface, _) = derived_interface(
            &admission,
            &mut ledger,
            &source,
            &expansion.elements,
            &envelope.interface,
        )?;
        let retained = match u32::try_from(source.len()) {
            Ok(bytes) => bytes,
            Err(_) => {
                return Err(Refusal::new(
                    Class::Exhausted,
                    body_at,
                    "retained source exceeds the finite byte bound",
                ))
            }
        };
        ledger.charge(
            Resource::LibraryBytes,
            retained,
            Location::field("library.bytes"),
        )?;
        ledger.charge(Resource::RetainedBytes, retained, body_at)?;
        let identity = envelope.chunk_id;
        let index = match u32::try_from(self.entries.len()) {
            Ok(index) => index,
            Err(_) => {
                return Err(Refusal::new(
                    Class::Exhausted,
                    Location::field("library.entries"),
                    "library ordinal exceeds the finite format",
                ))
            }
        };
        let expanded_nodes = expansion.expanded_nodes;
        self.entries.push(Entry {
            envelope,
            source,
            interface,
            expanded_nodes,
            identity: u64::from_be_bytes(first_eight(&identity)),
        });
        self.retained = self
            .retained
            .saturating_add(envelope_bytes)
            .saturating_add(retained);
        Ok(index)
    }

    fn check_dependencies(&self, envelope: &Envelope) -> Result<(), Refusal> {
        let mut index = 0usize;
        let mut failure = None;
        while failure.is_none() && index < envelope.dependencies.len() {
            failure = self.dependency_refusal(envelope, index);
            index = index.saturating_add(1);
        }
        match failure {
            Some(refusal) => Err(refusal),
            None => Ok(()),
        }
    }

    /// The refusal for dependency record `index` of `envelope`, if it does not resolve.
    fn dependency_refusal(&self, envelope: &Envelope, index: usize) -> Option<Refusal> {
        let Some(record) = envelope.dependencies.get(index) else {
            return Some(internal(
                Location::field("envelope.dependencies"),
                "dependency index exceeds the checked record slice",
            ));
        };
        if self.find(record).is_some() {
            return None;
        }
        let position = index as u32;
        Some(Refusal::new(
            Class::Invalid,
            Location::at("envelope.dependencies", position),
            "dependency record does not resolve to an earlier retained entry with the exact revisions",
        ))
    }
}

fn first_eight(bytes: &[u8; IDENTITY_BYTES]) -> [u8; 8] {
    let mut out = [0u8; 8];
    let mut at = 0usize;
    while at < out.len() {
        copy_identity_byte(&mut out, bytes, at);
        at = at.saturating_add(1);
    }
    out
}

/// Copy identity byte `at` into `out` when both positions exist.
fn copy_identity_byte(out: &mut [u8; 8], bytes: &[u8; IDENTITY_BYTES], at: usize) {
    if let (Some(slot), Some(value)) = (out.get_mut(at), bytes.get(at)) {
        *slot = *value;
    }
}

fn check_envelope(envelope: &Envelope, ledger: &mut Ledger) -> Result<(), Refusal> {
    if envelope.format != ENVELOPE_FORMAT {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("envelope.format"),
            "unsupported chunk envelope format",
        ));
    }
    if envelope.semantic_revision != SEMANTIC_REVISION {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("envelope.semantic_revision"),
            "unsupported chunk semantic revision",
        ));
    }
    if envelope.profile != PROFILE {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("envelope.profile"),
            "unsupported typed-chunk profile",
        ));
    }
    if !envelope.interface.effects.is_empty() {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("envelope.interface.effects"),
            "typed chunks require the empty effect set",
        ));
    }
    ledger.observe_count(
        Resource::RecipeNodes,
        encoded_len(envelope.body.nodes.len()),
        Location::field("envelope.body.nodes"),
    )?;
    if envelope.body.entry.len() > envelope.body.nodes.len()
        || envelope.dependencies.len() > envelope.body.nodes.len()
        || envelope.instantiations.len() > envelope.body.nodes.len()
    {
        return Err(Refusal::new(
            Class::Invalid,
            Location::field("envelope.body"),
            "recipe references and witness lists must be bounded by the recipe node arena",
        ));
    }
    check_stack(
        &envelope.interface.stack_in,
        ledger,
        "envelope.interface.stack_in",
    )?;
    check_stack(
        &envelope.interface.stack_out,
        ledger,
        "envelope.interface.stack_out",
    )?;
    check_captures(envelope)?;
    check_instantiations(envelope, ledger)?;
    check_recipe(envelope)?;
    if chunk_identity_of(envelope) != envelope.chunk_id {
        return Err(Refusal::new(
            Class::Invalid,
            Location::field("envelope.chunk_id"),
            "declared chunk identity is not the canonical body identity",
        ));
    }
    debug_assert_eq!(envelope.format, ENVELOPE_FORMAT);
    debug_assert!(envelope.interface.effects.is_empty());
    Ok(())
}

fn add_encoded_size(total: &mut u64, amount: u64, at: &Location) -> Result<(), Refusal> {
    let next = total.checked_add(amount).ok_or_else(|| {
        let mut refusal = Refusal::new(
            Class::Exhausted,
            at.clone(),
            "canonical encoded size exceeds the finite byte counter",
        );
        refusal.resource = Some(Resource::RetainedBytes);
        refusal.observed = u32::MAX;
        refusal.limit = MAX_RETAINED_BYTES;
        refusal
    })?;
    if next > u64::from(MAX_RETAINED_BYTES) {
        let mut refusal = Refusal::new(
            Class::Exhausted,
            at.clone(),
            "canonical encoded size exceeds the finite retained-byte bound",
        );
        refusal.resource = Some(Resource::RetainedBytes);
        let mut observed_retained_bytes = u32::MAX;
        if let Ok(representable) = u32::try_from(next) {
            observed_retained_bytes = representable;
        }
        refusal.observed = observed_retained_bytes;
        refusal.limit = MAX_RETAINED_BYTES;
        return Err(refusal);
    }
    *total = next;
    debug_assert!(next >= amount);
    debug_assert!(*total <= u64::from(MAX_RETAINED_BYTES));
    Ok(())
}

const fn encoded_len(count: usize) -> u64 {
    // Supported host and Wasm targets have at most 64-bit usize lengths.
    count as u64
}

/// Measure the exact retained envelope shape used by the typed-chunk profile.
/// The stable representation is fixed-width except for explicit vectors and
/// the profile spelling; source bytes are charged separately by `Library`.
fn envelope_size(envelope: &Envelope, at: Location) -> Result<u32, Refusal> {
    let mut encoded_bytes = 0u64;
    add_envelope_metadata_size(&mut encoded_bytes, envelope, &at)?;
    add_recipe_size(&mut encoded_bytes, &envelope.body, &at)?;
    u32::try_from(encoded_bytes).map_err(|_| {
        internal(
            at,
            "bounded envelope size did not fit the reporting counter",
        )
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; this size walker must return bounded refusals for malformed retained metadata rather than assert on caller-owned fields"
)]
fn add_envelope_metadata_size(
    encoded_bytes: &mut u64,
    envelope: &Envelope,
    at: &Location,
) -> Result<(), Refusal> {
    add_encoded_size(
        encoded_bytes,
        12u64.saturating_add(encoded_len(envelope.profile.len())),
        at,
    )?;
    let identity_revision_bytes = IDENTITY_BYTES.saturating_add(REVISION_BYTES);
    let identity_revision_bytes =
        u64::from(u32::try_from(identity_revision_bytes).map_err(|_| {
            internal(
                at.clone(),
                "fixed identity widths exceed their bounded format",
            )
        })?);
    add_encoded_size(encoded_bytes, identity_revision_bytes, at)?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(envelope.interface.stack_in.len()),
        at,
    )?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(envelope.interface.stack_out.len()),
        at,
    )?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(envelope.interface.effects.as_slice().len()).saturating_mul(4),
        at,
    )?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_capture_sizes(encoded_bytes, &envelope.captures, at)?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(envelope.dependencies.len()).saturating_mul(48),
        at,
    )?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_instantiation_sizes(encoded_bytes, &envelope.instantiations, at)
}

/// Add each retained capture's fixed scalar encoding, in capture order.
fn add_capture_sizes(
    encoded_bytes: &mut u64,
    captures: &[Capture],
    at: &Location,
) -> Result<(), Refusal> {
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < captures.len() {
        let capture_bytes = match captures[index].value {
            Scalar::I64(_) => 11,
            Scalar::Bool(_) => 4,
        };
        outcome = add_encoded_size(encoded_bytes, capture_bytes, at);
        index = index.saturating_add(1);
    }
    outcome
}

/// Add each instantiation witness's header and bindings, in witness order.
fn add_instantiation_sizes(
    encoded_bytes: &mut u64,
    instantiations: &[Instantiation],
    at: &Location,
) -> Result<(), Refusal> {
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < instantiations.len() {
        outcome = add_instantiation_size(encoded_bytes, &instantiations[index], at);
        index = index.saturating_add(1);
    }
    outcome
}

/// Add one instantiation witness's header and bindings.
fn add_instantiation_size(
    encoded_bytes: &mut u64,
    instantiation: &Instantiation,
    at: &Location,
) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(instantiation.bindings.len()).saturating_mul(4),
        at,
    )
}

fn add_recipe_size(encoded_bytes: &mut u64, recipe: &Recipe, at: &Location) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    add_recipe_node_sizes(encoded_bytes, &recipe.nodes, at)?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(recipe.entry.len()).saturating_mul(4),
        at,
    )?;
    Ok(())
}

/// Add each recipe node's fixed-width encoding, in arena order.
fn add_recipe_node_sizes(
    encoded_bytes: &mut u64,
    nodes: &[Node],
    at: &Location,
) -> Result<(), Refusal> {
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < nodes.len() {
        outcome = add_encoded_size(encoded_bytes, recipe_node_bytes(&nodes[index]), at);
        index = index.saturating_add(1);
    }
    outcome
}

fn check_stack(stack: &[Ty], ledger: &mut Ledger, field: &str) -> Result<(), Refusal> {
    let at = Location::field(field);
    ledger.observe_count(Resource::StackHeight, encoded_len(stack.len()), at.clone())?;
    let mut index = 0usize;
    let mut is_profile_stack = true;
    while is_profile_stack && index < stack.len() {
        is_profile_stack = profile_type(&stack[index]);
        index = index.saturating_add(1);
    }
    if !is_profile_stack {
        return Err(Refusal::new(
            Class::Unsupported,
            at,
            "interface holds a type outside the pure I64/Bool profile",
        ));
    }
    Ok(())
}

/// The profile admits exactly `I64` and `Bool`.
pub fn profile_type(ty: &Ty) -> bool {
    matches!(ty, Ty::I64 | Ty::Bool)
}

fn check_captures(envelope: &Envelope) -> Result<(), Refusal> {
    let at = Location::field("envelope.captures");
    if u64::try_from(envelope.captures.len()).map_or(true, |count| count > u64::from(MAX_CAPTURES))
    {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "capture list exceeds the eight-capture bound",
        ));
    }
    let mut position = 0usize;
    let mut failure = None;
    while failure.is_none() && position < envelope.captures.len() {
        failure = capture_refusal(envelope, position, &at);
        position = position.saturating_add(1);
    }
    if let Some(refusal) = failure {
        return Err(refusal);
    }
    debug_assert!(u32::try_from(envelope.captures.len()).is_ok_and(|count| count <= MAX_CAPTURES));
    debug_assert!(envelope.captures.last().is_none_or(|capture| {
        usize::from(capture.index).saturating_add(1) == envelope.captures.len()
    }));
    Ok(())
}

/// The refusal for one capture slot that is not contiguous with a profile scalar.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; capture lookups use the non-const slice `get` and refusals allocate their message"
)]
fn capture_refusal(envelope: &Envelope, position: usize, at: &Location) -> Option<Refusal> {
    debug_assert!(position < envelope.captures.len());
    debug_assert!(encoded_len(envelope.captures.len()) <= u64::from(MAX_CAPTURES));
    let Some(capture) = envelope.captures.get(position) else {
        return Some(internal(
            at.clone(),
            "capture index exceeds the checked capture slice",
        ));
    };
    let Ok(expected) = u16::try_from(position) else {
        return Some(Refusal::new(
            Class::Invalid,
            at.clone(),
            "capture ordinal exceeds the finite format",
        ));
    };
    if capture.index != expected || !profile_type(&capture.value.ty()) {
        return Some(Refusal::new(
            Class::Invalid,
            Location::at("envelope.captures", u32::from(expected)),
            "capture list must be contiguous from zero with profile scalars",
        ));
    }
    None
}

fn check_instantiations(envelope: &Envelope, ledger: &mut Ledger) -> Result<(), Refusal> {
    let mut position = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && position < envelope.instantiations.len() {
        outcome = check_instantiation(envelope, position, ledger);
        position = position.saturating_add(1);
    }
    outcome?;
    let mut has_bounded_instantiations = true;
    let mut has_supported_bindings = true;
    for instantiation in &envelope.instantiations {
        has_bounded_instantiations &= u32::try_from(instantiation.bindings.len())
            .is_ok_and(|count| count <= MAX_INSTANTIATION_BINDINGS);
        for binding in &instantiation.bindings {
            has_supported_bindings &= binding_is_profile(binding);
        }
    }
    debug_assert!(has_bounded_instantiations);
    debug_assert!(has_supported_bindings);
    Ok(())
}

/// Check one retained instantiation witness and charge its bindings.
fn check_instantiation(
    envelope: &Envelope,
    position: usize,
    ledger: &mut Ledger,
) -> Result<(), Refusal> {
    debug_assert!(position < envelope.instantiations.len());
    let at = Location::at("envelope.instantiations", position as u32);
    let Some(instantiation) = envelope.instantiations.get(position) else {
        return Err(internal(
            at,
            "instantiation index exceeds the checked slice",
        ));
    };
    if u64::try_from(instantiation.bindings.len())
        .map_or(true, |count| count > u64::from(MAX_INSTANTIATION_BINDINGS))
    {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "instantiation retains more than the 16-binding profile bound",
        ));
    }
    debug_assert!(
        encoded_len(instantiation.bindings.len()) <= u64::from(MAX_INSTANTIATION_BINDINGS)
    );
    ledger.charge_count(
        Resource::InstantiationBindings,
        encoded_len(instantiation.bindings.len()),
        at.clone(),
    )?;
    match bindings_refusal(&instantiation.bindings, &at) {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    }
}

/// The first profile, kind or ordering refusal among one witness's bindings.
fn bindings_refusal(bindings: &[Binding], at: &Location) -> Option<Refusal> {
    let mut previous: Option<u16> = None;
    let mut index = 0usize;
    let mut failure = None;
    while failure.is_none() && index < bindings.len() {
        let binding = &bindings[index];
        failure = binding_refusal(binding, previous, at);
        previous = Some(binding.variable);
        index = index.saturating_add(1);
    }
    failure
}

/// The refusal for one binding outside the profile, of the wrong kind or out of order.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; constructing a refusal allocates its message"
)]
fn binding_refusal(binding: &Binding, previous: Option<u16>, at: &Location) -> Option<Refusal> {
    if !profile_type(&binding.ty) {
        return Some(Refusal::new(
            Class::Unsupported,
            at.clone(),
            "instantiation binds a variable outside the profile",
        ));
    }
    if binding.kind != noble_kernel::words::VariableKind::Value {
        return Some(Refusal::new(
            Class::Unsupported,
            at.clone(),
            "the pure I64/Bool profile supports only value-variable witnesses",
        ));
    }
    debug_assert!(profile_type(&binding.ty));
    debug_assert!(binding.kind == noble_kernel::words::VariableKind::Value);
    if let Some(seen) = previous {
        if binding.variable <= seen {
            return Some(Refusal::new(
                Class::Invalid,
                at.clone(),
                "instantiation bindings must be in increasing variable order",
            ));
        }
    }
    None
}

fn check_recipe(envelope: &Envelope) -> Result<(), Refusal> {
    let at = Location::field("envelope.body");
    if envelope.body.entry.is_empty() {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "recipe entry list is empty",
        ));
    }
    let mut position = 0usize;
    let mut failure = None;
    while failure.is_none() && position < envelope.body.entry.len() {
        failure = recipe_entry_refusal(envelope, position, &at);
        position = position.saturating_add(1);
    }
    if let Some(refusal) = failure {
        return Err(refusal);
    }
    let mut position = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && position < envelope.body.nodes.len() {
        outcome = check_recipe_position(envelope, position, &at);
        position = position.saturating_add(1);
    }
    outcome?;
    debug_assert!(!envelope.body.entry.is_empty());
    let mut has_preceding_references = true;
    for reference in &envelope.body.entry {
        has_preceding_references &= reference_precedes(*reference, envelope.body.nodes.len());
    }
    debug_assert!(has_preceding_references);
    Ok(())
}

/// The refusal for one recipe entry reference outside the node arena.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; entry lookups use the non-const slice `get` and refusals allocate their message"
)]
fn recipe_entry_refusal(envelope: &Envelope, position: usize, at: &Location) -> Option<Refusal> {
    debug_assert!(position < envelope.body.entry.len());
    debug_assert!(!envelope.body.entry.is_empty());
    let Some(reference) = envelope.body.entry.get(position) else {
        return Some(internal(
            at.clone(),
            "recipe entry index exceeds the checked entry slice",
        ));
    };
    if reference_precedes(*reference, envelope.body.nodes.len()) {
        return None;
    }
    let Ok(position) = u32::try_from(position) else {
        return Some(Refusal::new(
            Class::Invalid,
            at.clone(),
            "recipe entry ordinal exceeds the finite format",
        ));
    };
    Some(Refusal::new(
        Class::Invalid,
        Location::at("envelope.body.entry", position),
        "recipe entry references a node outside the finite arena",
    ))
}

/// Check one recipe node's references against its own arena position.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; node lookups use the non-const slice `get` and refusals allocate their message"
)]
fn check_recipe_position(
    envelope: &Envelope,
    position: usize,
    at: &Location,
) -> Result<(), Refusal> {
    let Some(node) = envelope.body.nodes.get(position) else {
        return Err(internal(
            at.clone(),
            "recipe node index exceeds the checked node arena",
        ));
    };
    let own = match u32::try_from(position) {
        Ok(position) => position,
        Err(_) => {
            return Err(Refusal::new(
                Class::Invalid,
                at.clone(),
                "recipe node ordinal exceeds the finite format",
            ))
        }
    };
    check_recipe_node(envelope, node, own, position)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; cross-width reference conversion is checked at runtime"
)]
fn reference_precedes(reference: u32, boundary: usize) -> bool {
    match usize::try_from(reference) {
        Ok(index) => index < boundary,
        Err(_) => false,
    }
}

/// Whether every reference in a quotation body precedes `boundary`.
fn references_precede(references: &[u32], boundary: usize) -> bool {
    let mut index = 0usize;
    let mut is_preceding = true;
    while is_preceding && index < references.len() {
        is_preceding = reference_precedes(references[index], boundary);
        index = index.saturating_add(1);
    }
    is_preceding
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; recipe references are untrusted and each invalid ordinal must return its located refusal instead of panicking"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; constructing a refusal allocates its message"
)]
fn check_recipe_node(
    envelope: &Envelope,
    node: &Node,
    own: u32,
    position: usize,
) -> Result<(), Refusal> {
    match node {
        Node::Capture { index } => {
            if usize::from(*index) >= envelope.captures.len() {
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::at("envelope.body.nodes", own),
                    "capture node references an unretained capture",
                ));
            }
        }
        Node::Invoke {
            dependency,
            instantiation,
        } => {
            if usize::from(*dependency) >= envelope.dependencies.len()
                || usize::from(*instantiation) >= envelope.instantiations.len()
            {
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::at("envelope.body.nodes", own),
                    "invoke node references an unretained dependency or witness",
                ));
            }
        }
        Node::Quotation {
            body,
            instantiation,
        } => {
            if usize::from(*instantiation) >= envelope.instantiations.len() {
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::at("envelope.body.nodes", own),
                    "quotation node references an unretained witness",
                ));
            }
            if body.len() > position {
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::at("envelope.body.nodes", own),
                    "quotation references exceed the earlier recipe node prefix",
                ));
            }
            if !references_precede(body, position) {
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::at("envelope.body.nodes", own),
                    "quotation body must reference strictly earlier nodes",
                ));
            }
        }
        Node::Operation { instantiation, .. } => {
            if usize::from(*instantiation) >= envelope.instantiations.len() {
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::at("envelope.body.nodes", own),
                    "operation node references an unretained witness",
                ));
            }
        }
        Node::I64(_) | Node::Bool(_) => {}
    }
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; source diagnostics are dynamically borrowed and copied into refusals"
)]
fn refusal_of_failure(failure: &super::Error, at: Location) -> Refusal {
    let diagnostic = failure.diagnostic();
    let class = match diagnostic.kind {
        crate::DiagnosticKind::Invalid => Class::Invalid,
        crate::DiagnosticKind::Unsupported => Class::Unsupported,
        crate::DiagnosticKind::Exhausted => Class::Exhausted,
        crate::DiagnosticKind::Internal => Class::Internal,
    };
    Refusal::new(class, at, &diagnostic.message)
}

fn internal(at: Location, message: &str) -> Refusal {
    Refusal::new(Class::Internal, at, message)
}

/// Reject namespace shadowing: an accepted source spelling must resolve to
/// the exact bootstrap primitive selected by this profile, not merely to a
/// definition with a compatible interface.
fn check_bootstrap_targets(
    candidate: &noble_kernel::untrusted::Candidate,
    elements: &[Element],
    at: Location,
) -> Result<(), Refusal> {
    let expected = expected_bootstrap_targets(elements, &at)?;
    check_invocation_targets(candidate, &expected, at)
}

fn expected_bootstrap_targets(
    elements: &[Element],
    at: &Location,
) -> Result<alloc::vec::Vec<noble_kernel::contracts::Definition>, Refusal> {
    let mut expected = alloc::vec::Vec::with_capacity(elements.len());
    let mut index = 0usize;
    let mut failure = None;
    while failure.is_none() && index < elements.len() {
        match bootstrap_target(&elements[index], index, at) {
            Ok(Some(definition)) => expected.push(definition),
            Ok(None) => {}
            Err(refusal) => failure = Some(refusal),
        }
        index = index.saturating_add(1);
    }
    debug_assert!(failure.is_some() || index == elements.len());
    debug_assert!(expected.len() <= elements.len());
    match failure {
        Some(refusal) => Err(refusal),
        None => Ok(expected),
    }
}

/// The bootstrap definition one element must invoke, if it is a profile operation.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; bootstrap resolution calls the non-const program word table and refusals allocate their message"
)]
fn bootstrap_target(
    element: &Element,
    index: usize,
    at: &Location,
) -> Result<Option<noble_kernel::contracts::Definition>, Refusal> {
    let Element::Op { op, .. } = element else {
        return Ok(None);
    };
    match crate::program::bootstrap_word(op.spelling().as_bytes()) {
        Some(definition) => Ok(Some(definition)),
        None => Err(Refusal::new(
            Class::Unsupported,
            Location::at(&at.field, index as u32),
            "profile operation has no canonical bootstrap definition",
        )),
    }
}

/// One candidate target walk frame: the body it walks (by node index, so the
/// frame holds no borrow) and its progress.
struct CandidateFrame {
    /// `None` walks the candidate body; `Some(node)` that quotation's body.
    quotation: Option<usize>,
    at: usize,
}

/// The mutable state of one candidate target walk.
struct TargetWalk {
    frames: alloc::vec::Vec<CandidateFrame>,
    next: usize,
}

/// The node references one candidate target walk frame visits.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; quotation lookups use the slice `get`, which is not a stable const fn"
)]
fn candidate_frame_body<'a>(
    candidate: &'a noble_kernel::untrusted::Candidate,
    frame: &CandidateFrame,
) -> Option<&'a [noble_kernel::untrusted::NodeId]> {
    match frame.quotation {
        None => Some(candidate.body.as_slice()),
        Some(node) => match candidate.nodes.get(node) {
            Some(noble_kernel::untrusted::Node::Quotation { body, .. }) => Some(body.as_slice()),
            Some(
                noble_kernel::untrusted::Node::Literal { .. }
                | noble_kernel::untrusted::Node::Invocation { .. }
                | noble_kernel::untrusted::Node::SignedSelf { .. }
                | noble_kernel::untrusted::Node::SlotInvoke { .. },
            )
            | None => None,
        },
    }
}

fn check_invocation_targets(
    candidate: &noble_kernel::untrusted::Candidate,
    expected: &[noble_kernel::contracts::Definition],
    at: Location,
) -> Result<(), Refusal> {
    let mut walk = TargetWalk {
        frames: alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES),
        next: 0,
    };
    walk.frames.push(CandidateFrame {
        quotation: None,
        at: 0,
    });
    let mut outcome = Ok(());
    while outcome.is_ok() && !walk.frames.is_empty() {
        outcome = invocation_target_step(candidate, expected, &mut walk, &at);
    }
    outcome?;
    debug_assert!(walk.frames.is_empty());
    if walk.next != expected.len() {
        return Err(Refusal::new(
            Class::Internal,
            at,
            "source parser and emitted operation sequence disagree",
        ));
    }
    debug_assert_eq!(walk.next, expected.len());
    Ok(())
}

/// One target-walk step: close the finished top frame or check its next node.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; source preparation output is checked with fail-closed located refusals and must not panic on malformed or adversarial parser output"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the target walk mutates its runtime frame Vec and refusals allocate their message"
)]
fn invocation_target_step(
    candidate: &noble_kernel::untrusted::Candidate,
    expected: &[noble_kernel::contracts::Definition],
    walk: &mut TargetWalk,
    at: &Location,
) -> Result<(), Refusal> {
    let Some(top) = walk.frames.len().checked_sub(1) else {
        return Ok(());
    };
    let Some(frame) = walk.frames.get(top) else {
        return Err(internal(at.clone(), "candidate target walk lost its frame"));
    };
    let Some(body) = candidate_frame_body(candidate, frame) else {
        return Err(internal(at.clone(), "candidate target walk lost its frame"));
    };
    let position = frame.at;
    if position >= body.len() {
        let _ = walk.frames.pop();
        return Ok(());
    }
    if let Some(frame) = walk.frames.get_mut(top) {
        frame.at = position.saturating_add(1);
    }
    let Some(reference) = body.get(position) else {
        return Err(internal(
            at.clone(),
            "candidate target walk lost a node reference",
        ));
    };
    let Ok(node_index) = usize::try_from(reference.0) else {
        return Err(internal(
            at.clone(),
            "source parser emitted a node reference outside the host address space",
        ));
    };
    let Some(node) = candidate.nodes.get(node_index) else {
        return Err(internal(
            at.clone(),
            "source parser emitted an invalid candidate reference",
        ));
    };
    match node {
        noble_kernel::untrusted::Node::Invocation { def, .. } => {
            let is_expected = is_expected_target(expected, walk.next, def);
            if !is_expected {
                return Err(Refusal::new(
                    Class::Unsupported,
                    Location::at(&at.field, walk.next as u32),
                    "profile source spelling resolves outside its exact bootstrap operation",
                ));
            }
            walk.next = walk.next.saturating_add(1);
        }
        noble_kernel::untrusted::Node::Quotation { .. } => {
            walk.frames.push(CandidateFrame {
                quotation: Some(node_index),
                at: 0,
            });
        }
        noble_kernel::untrusted::Node::Literal { .. } => {}
        noble_kernel::untrusted::Node::SignedSelf { .. }
        | noble_kernel::untrusted::Node::SlotInvoke { .. } => {
            return Err(Refusal::new(
                Class::Unsupported,
                at.clone(),
                "profile source contains a signed or live invocation",
            ));
        }
    }
    Ok(())
}

/// Whether the `next` expected bootstrap definition is exactly `def`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; target lookups use the non-const slice `get` and derived definition equality"
)]
fn is_expected_target(
    expected: &[noble_kernel::contracts::Definition],
    next: usize,
    def: &noble_kernel::contracts::Definition,
) -> bool {
    match expected.get(next) {
        Some(target) => *target == *def,
        None => false,
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; exhausting the shared ledger requires runtime mutation"
)]
fn exhaust_checking_work(ledger: &mut Ledger, at: Location) -> Refusal {
    let remaining = ledger.remaining(Resource::CheckingWork);
    match ledger.charge(
        Resource::CheckingWork,
        remaining.saturating_add(1),
        at.clone(),
    ) {
        Err(refusal) => refusal,
        Ok(()) => internal(
            at,
            "checking-work exhaustion did not cross its ledger limit",
        ),
    }
}

/// Host-owned state shared by the admission steps for one emitted candidate.
struct AdmissionContext<'a> {
    namespace: &'a super::Session,
    at: Location,
}

/// Prepare one emitted program through ordinary source admission and
/// independently re-check the resulting submission in the kernel. The derived
/// interface is what returns; the declared one is only compared.
fn derived_interface(
    context: &AdmissionContext<'_>,
    ledger: &mut Ledger,
    source: &[u8],
    elements: &[Element],
    declared: &Interface,
) -> Result<(Interface, noble_kernel::untrusted::Candidate), Refusal> {
    let prepared = prepare_candidate(
        context.namespace,
        source,
        &declared.stack_in,
        ledger,
        context.at.clone(),
    )?;
    let derived = prepared_interface(&prepared, declared, context.at.clone())?;
    let Some(submission) = prepared.submission() else {
        return Err(internal(
            context.at.clone(),
            "preparation retained no submission",
        ));
    };
    check_bootstrap_targets(&submission.body.candidate, elements, context.at.clone())?;
    let candidate = independently_accept(submission, &derived, ledger, context.at.clone())?;
    debug_assert_eq!(derived.stack_in, declared.stack_in);
    debug_assert!(derived.effects.is_empty());
    Ok((derived, candidate))
}

fn prepare_candidate(
    namespace: &super::Session,
    source: &[u8],
    inputs: &[Ty],
    ledger: &mut Ledger,
    at: Location,
) -> Result<super::Prepared, Refusal> {
    let remaining_work = ledger.remaining(Resource::CheckingWork);
    if remaining_work == 0 {
        return Err(exhaust_checking_work(ledger, at));
    }
    debug_assert!(remaining_work > 0);
    match namespace.prepare(source, inputs, ledger.prepare_limits(remaining_work)) {
        Ok(prepared) => {
            debug_assert!(prepared.work_used() <= remaining_work);
            debug_assert!(prepared.checked().is_some());
            debug_assert!(prepared.submission().is_some());
            ledger.charge(Resource::CheckingWork, prepared.work_used(), at)?;
            Ok(prepared)
        }
        Err(failure) => {
            ledger.charge(Resource::CheckingWork, failure.work_used(), at.clone())?;
            let refusal = refusal_of_failure(&failure, at.clone());
            if refusal.class == Class::Exhausted {
                return Err(exhaust_checking_work(ledger, at));
            }
            Err(refusal)
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; preparation results contain runtime interfaces that must be cloned and compared"
)]
fn prepared_interface(
    prepared: &super::Prepared,
    declared: &Interface,
    at: Location,
) -> Result<Interface, Refusal> {
    let Some(checked) = prepared.checked() else {
        return Err(internal(at, "preparation retained no kernel result"));
    };
    let derived = Interface {
        stack_in: checked.interface.stack_in.clone(),
        stack_out: checked.interface.stack_out.clone(),
        effects: checked.interface.effects.clone(),
    };
    if !checked.interface.effects.is_empty() {
        return Err(Refusal::new(
            Class::Unsupported,
            at,
            "profile chunk derives a nonempty effect bound",
        ));
    }
    if derived != *declared {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "declared ordered interface differs from the interface source admission derived",
        ));
    }
    debug_assert!(derived.effects.is_empty());
    debug_assert_eq!(derived, *declared);
    Ok(derived)
}

fn independently_accept(
    submission: &noble_kernel::execution::Submission,
    derived: &Interface,
    ledger: &mut Ledger,
    at: Location,
) -> Result<noble_kernel::untrusted::Candidate, Refusal> {
    let mut request = submission.request.clone();
    let remaining_work = ledger.remaining(Resource::CheckingWork);
    if remaining_work == 0 {
        return Err(exhaust_checking_work(ledger, at));
    }
    request.limits.work = remaining_work;
    debug_assert!(request.limits.work <= ledger.remaining(Resource::CheckingWork));
    let (outcome, work_used) = noble_kernel::acceptance::check_with_work(
        &submission.environment,
        &request,
        &submission.body.candidate,
    );
    ledger.charge(Resource::CheckingWork, work_used, at.clone())?;
    match outcome {
        noble_kernel::untrusted::Outcome::Accepted(again) => {
            if again.interface.stack_in == derived.stack_in
                && again.interface.stack_out == derived.stack_out
                && again.interface.effects.is_empty()
            {
                debug_assert!(work_used <= remaining_work);
                debug_assert!(again.interface.effects.is_empty());
                debug_assert_eq!(again.interface.stack_in, derived.stack_in);
                Ok(submission.body.candidate.clone())
            } else {
                Err(internal(
                    at,
                    "independent kernel acceptance derived a different interface",
                ))
            }
        }
        noble_kernel::untrusted::Outcome::Exhausted(_) => Err(exhaust_checking_work(ledger, at)),
        noble_kernel::untrusted::Outcome::Invalid(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => Err(internal(
            at,
            "independent kernel acceptance refused a source-admitted submission",
        )),
    }
}

/// One flattened request editor tree.
struct FlatRequest {
    elements: alloc::vec::Vec<Element>,
    hole: usize,
}

/// The editor node list reached from `nodes` by descending through the
/// quotation at each position of `path`.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; each call descends one quotation level of an editor tree whose depth the request checks bound, and direct recursion keeps a borrowed work stack out of the pinned Aeneas extraction"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; editor node lookups use the slice `get`, which is not a stable const fn"
)]
fn editor_nodes_at<'a>(
    nodes: &'a [super::editor::Node],
    path: &[usize],
) -> Option<&'a [super::editor::Node]> {
    match path.split_first() {
        None => Some(nodes),
        Some((first, rest)) => match nodes.get(*first) {
            Some(super::editor::Node::Quotation(children)) => editor_nodes_at(children, rest),
            Some(
                super::editor::Node::Integer(_)
                | super::editor::Node::Boolean(_)
                | super::editor::Node::Word(_)
                | super::editor::Node::Hole,
            )
            | None => None,
        },
    }
}

/// Where one editor walk frame sits: the quotation positions from the root
/// node list (so the frame holds no borrow) and the reported path trail.
#[derive(Clone)]
struct EditorPath {
    positions: alloc::vec::Vec<usize>,
    trail: alloc::vec::Vec<u32>,
}

struct EditorWalkFrame {
    path: EditorPath,
    next: usize,
}

struct EditorWalk<'a> {
    root: &'a [super::editor::Node],
    frames: alloc::vec::Vec<EditorWalkFrame>,
    elements: alloc::vec::Vec<Element>,
    hole: Option<(usize, alloc::vec::Vec<u32>)>,
    depth: u32,
}

impl<'a> EditorWalk<'a> {
    fn new(nodes: &'a [super::editor::Node]) -> Self {
        let mut frames = alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES);
        frames.push(EditorWalkFrame {
            path: EditorPath {
                positions: alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES),
                trail: alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES),
            },
            next: 0,
        });
        Self {
            root: nodes,
            frames,
            elements: alloc::vec::Vec::with_capacity(nodes.len()),
            hole: None,
            depth: 0,
        }
    }

    fn advance(&mut self, ledger: &mut Ledger, at: &Location) -> Result<bool, Refusal> {
        let Some(top) = self.frames.len().checked_sub(1) else {
            return Ok(false);
        };
        let root: &'a [super::editor::Node] = self.root;
        let (nodes, index, finished) = {
            let Some(frame) = self.frames.get_mut(top) else {
                return Err(internal(at.clone(), "editor walk lost its frame"));
            };
            let Some(nodes) = editor_nodes_at(root, &frame.path.positions) else {
                return Err(internal(at.clone(), "editor walk lost its frame"));
            };
            if frame.next >= nodes.len() {
                (nodes, frame.next, true)
            } else {
                let index = frame.next;
                frame.next = frame.next.saturating_add(1);
                (nodes, index, false)
            }
        };
        if finished {
            let _ = self.frames.pop();
            if top > 0 {
                self.depth = self.depth.saturating_sub(1);
                self.elements.push(Element::Close);
            }
            return Ok(true);
        }
        let Some(node) = nodes.get(index) else {
            return Err(internal(
                at.clone(),
                "editor node reference is outside the finite tree",
            ));
        };
        match node {
            super::editor::Node::Hole | super::editor::Node::Quotation(_) => {
                let Some(frame) = self.frames.get(top) else {
                    return Err(internal(at.clone(), "editor walk lost its path frame"));
                };
                let path = frame.path.clone();
                self.append_path_node(node, index, path, ledger, at)?;
            }
            _ => self.append_leaf(node, at)?,
        }
        Ok(true)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; leaf handling mutates the runtime editor Vec and allocates typed refusal messages"
    )]
    fn append_leaf(&mut self, node: &super::editor::Node, at: &Location) -> Result<(), Refusal> {
        match node {
            super::editor::Node::Integer(value) => self.elements.push(Element::I64(*value)),
            super::editor::Node::Boolean(value) => self.elements.push(Element::Bool(*value)),
            super::editor::Node::Word(word) => match Op::of(word.as_bytes()) {
                Some(op) => self.elements.push(Element::Op { op, witness: None }),
                None => {
                    return Err(Refusal::new(
                        Class::Unsupported,
                        at.clone(),
                        "editor word is outside the published profile whitelist",
                    ))
                }
            },
            super::editor::Node::Hole | super::editor::Node::Quotation(_) => {
                return Err(internal(
                    at.clone(),
                    "editor path node was sent to the leaf handler",
                ))
            }
        }
        Ok(())
    }

    fn append_path_node(
        &mut self,
        node: &super::editor::Node,
        index: usize,
        mut path: EditorPath,
        ledger: &mut Ledger,
        at: &Location,
    ) -> Result<(), Refusal> {
        let position = u32::try_from(index).map_err(|_| {
            Refusal::new(
                Class::Invalid,
                at.clone(),
                "editor path position exceeds the path index width",
            )
        })?;
        path.trail.push(position);
        path.positions.push(index);
        match node {
            super::editor::Node::Hole => {
                if self.hole.is_some() {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at.clone(),
                        "editor tree holds more than one hole",
                    ));
                }
                self.hole = Some((self.elements.len(), path.trail));
                self.elements.push(Element::Hole);
            }
            super::editor::Node::Quotation(_) => {
                self.depth = self.depth.saturating_add(1);
                ledger.observe(Resource::QuotationDepth, self.depth, at.clone())?;
                self.elements.push(Element::Open);
                self.frames.push(EditorWalkFrame { path, next: 0 });
            }
            _ => {
                return Err(internal(
                    at.clone(),
                    "editor leaf was sent to the path handler",
                ))
            }
        }
        Ok(())
    }
}

/// Flatten and validate the request's editor tree. A word outside the profile is
/// `Unsupported`; a second hole, a malformed path or an unbalanced tree is
/// `Invalid`.
fn flatten_request(request: &Request, ledger: &mut Ledger) -> Result<FlatRequest, Refusal> {
    let at = Location::field("request.editor");
    let (elements, hole, trail) = flatten_editor_tree(&request.editor.nodes, ledger, at)?;
    if trail != request.hole_path {
        return Err(Refusal::new(
            Class::Invalid,
            Location::field("request.hole_path"),
            "hole path does not identify the single editor hole",
        ));
    }
    Ok(FlatRequest { elements, hole })
}

fn flatten_editor_tree(
    nodes: &[super::editor::Node],
    ledger: &mut Ledger,
    at: Location,
) -> Result<(alloc::vec::Vec<Element>, usize, alloc::vec::Vec<u32>), Refusal> {
    let mut walk = EditorWalk::new(nodes);
    let mut walking = Ok(true);
    while matches!(walking, Ok(true)) {
        walking = walk.advance(ledger, &at);
    }
    walking?;
    let Some((hole, trail)) = walk.hole else {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "editor tree holds no hole",
        ));
    };
    Ok((walk.elements, hole, trail))
}

/// Replace the hole with one proposal's elements.
fn fill(elements: &[Element], hole: usize, proposal: &[Element]) -> alloc::vec::Vec<Element> {
    let mut out = alloc::vec::Vec::with_capacity(
        elements
            .len()
            .saturating_add(proposal.len())
            .saturating_sub(1),
    );
    let prefix_end = if hole < elements.len() {
        hole
    } else {
        elements.len()
    };
    out.extend_from_slice(&elements[..prefix_end]);
    out.extend_from_slice(proposal);
    if let Some(rest) = elements.get(hole.saturating_add(1)..) {
        out.extend_from_slice(rest);
    }
    out
}

/// Retain the first refusal at each class and exact location. Repeated
/// refusals remain counted by the search but consume no more evidence space.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; this helper appends deduplicated refusals to a budget-bounded diagnostics Vec"
)]
fn record(
    ledger: &mut Ledger,
    diagnostics: &mut alloc::vec::Vec<Refusal>,
    refusal: Refusal,
) -> Result<(), Refusal> {
    if matches!(refusal.class, Class::Exhausted | Class::Internal) {
        return Err(refusal);
    }
    debug_assert!(!matches!(refusal.class, Class::Exhausted | Class::Internal));
    let is_recorded = crate::scan::find_first(diagnostics.as_slice(), |retained| {
        retained.class == refusal.class && retained.location == refusal.location
    })
    .is_some();
    if is_recorded {
        return Ok(());
    }
    ledger.charge(Resource::Diagnostics, 1, refusal.location.clone())?;
    ledger.charge(
        Resource::RetainedBytes,
        refusal_encoded_size(&refusal)?,
        refusal.location.clone(),
    )?;
    diagnostics.push(refusal);
    debug_assert!(!diagnostics.is_empty());
    Ok(())
}

fn response_of(refusal: Refusal, ledger: &Ledger) -> Response {
    let report = alloc::boxed::Box::new(Report::of(ledger));
    match refusal.class {
        Class::Invalid => Response::Invalid { refusal, report },
        Class::Unsupported => Response::Unsupported { refusal, report },
        Class::Exhausted => Response::Exhausted { refusal, report },
        Class::Internal => Response::InternalFailure { refusal, report },
    }
}

fn response_of_search(refusal: Refusal, search: &Search<'_>) -> Response {
    let mut response = response_of(refusal, &search.ledger);
    *response.report_mut() = search.report(0);
    response
}

/// One proposal's result: admitted, or a located refusal the search continues
/// past.
#[octet::sealed_enum]
enum Step {
    Admitted(alloc::boxed::Box<Accepted>),
    Refused(Refusal),
}

struct Composition {
    elements: alloc::vec::Vec<Element>,
    stack: alloc::vec::Vec<Ty>,
    untouched: usize,
}

/// One scalar or quotation value during fixture evaluation. A quotation is
/// the element range `start..end` of the evaluated program, so a value holds
/// no borrow.
#[octet::sealed_enum]
#[derive(Clone, Debug)]
enum Value {
    I64(i64),
    Bool(bool),
    Program { start: usize, end: usize },
}

/// One fixture evaluation frame: the program range it runs and its offset.
struct EvalFrame {
    start: usize,
    end: usize,
    index: usize,
}

/// The end of the quotation body that starts at `start` in `program` and the
/// position just past its closing bracket.
fn collect_quotation(program: &[Element], start: usize) -> Option<(usize, usize)> {
    let mut depth = 0u32;
    let mut at = start;
    while let Some(element) = program.get(at) {
        match element {
            Element::Open => depth = depth.saturating_add(1),
            Element::Close => {
                if depth == 0 {
                    return Some((at, at.saturating_add(1)));
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        at = at.saturating_add(1);
    }
    None
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; fixture operations must push and pop the bounded evaluation stack"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; fixture dispatch invokes runtime-only Vec mutation and bounded evaluation helpers"
)]
fn eval_operation(
    op: Op,
    stack: &mut alloc::vec::Vec<Value>,
    frames: &mut alloc::vec::Vec<EvalFrame>,
    at: &Location,
) -> Result<(), Refusal> {
    debug_assert!(stack.len() <= MAX_STACK_CAPACITY);
    let outcome = match op {
        Op::Dup => eval_duplicate(stack, at),
        Op::Drop => eval_drop(stack, at),
        Op::Swap => eval_swap(stack, at),
        Op::Add | Op::Sub | Op::Mul | Op::Eq => eval_binary(op, stack, at),
        Op::Run => eval_run(stack, frames, at),
    };
    if outcome.is_ok() {
        debug_assert!(stack.len() <= MAX_STACK_CAPACITY);
    }
    outcome
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; fixture duplication must push onto the bounded owned evaluation stack"
)]
fn eval_duplicate(stack: &mut alloc::vec::Vec<Value>, at: &Location) -> Result<(), Refusal> {
    let Some(value) = stack.last().cloned() else {
        return Err(Refusal::new(
            Class::Invalid,
            at.clone(),
            "fixture evaluation underflowed",
        ));
    };
    stack.push(value);
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; fixture drop consumes an owned value from the bounded evaluation stack"
)]
fn eval_drop(stack: &mut alloc::vec::Vec<Value>, at: &Location) -> Result<(), Refusal> {
    if stack.pop().is_none() {
        return Err(Refusal::new(
            Class::Invalid,
            at.clone(),
            "fixture evaluation underflowed",
        ));
    }
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; fixture swap transfers ownership of two values through the bounded evaluation stack"
)]
fn eval_swap(stack: &mut alloc::vec::Vec<Value>, at: &Location) -> Result<(), Refusal> {
    let (Some(top), Some(next)) = (stack.pop(), stack.pop()) else {
        return Err(Refusal::new(
            Class::Invalid,
            at.clone(),
            "fixture evaluation underflowed",
        ));
    };
    stack.push(top);
    stack.push(next);
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; fixture run transfers an owned quotation value into the bounded evaluation frame stack"
)]
fn eval_run(
    stack: &mut alloc::vec::Vec<Value>,
    frames: &mut alloc::vec::Vec<EvalFrame>,
    at: &Location,
) -> Result<(), Refusal> {
    let value = pop_value(stack, at)?;
    let Value::Program { start, end } = value else {
        return Err(Refusal::new(
            Class::Invalid,
            at.clone(),
            "fixture run operand is not a quotation value",
        ));
    };
    frames.push(EvalFrame {
        start,
        end,
        index: 0,
    });
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; binary fixture operations replace two stack values with one result"
)]
fn eval_binary(op: Op, stack: &mut alloc::vec::Vec<Value>, at: &Location) -> Result<(), Refusal> {
    let initial_len = stack.len();
    let (Value::I64(right), Value::I64(left)) = (pop_value(stack, at)?, pop_value(stack, at)?)
    else {
        return Err(Refusal::new(
            Class::Invalid,
            at.clone(),
            "fixture binary operation requires two I64 operands",
        ));
    };
    match op {
        Op::Add => stack.push(Value::I64(left.wrapping_add(right))),
        Op::Sub => stack.push(Value::I64(left.wrapping_sub(right))),
        Op::Mul => stack.push(Value::I64(left.wrapping_mul(right))),
        Op::Eq => stack.push(Value::Bool(left == right)),
        Op::Dup | Op::Drop | Op::Swap | Op::Run => {
            return Err(internal(
                at.clone(),
                "fixture binary operation received a non-binary word",
            ));
        }
    }
    debug_assert_eq!(stack.len().saturating_add(1), initial_len);
    debug_assert!(top_is_scalar(stack));
    Ok(())
}

/// Whether the fixture stack's top value is an `I64` or `Bool` scalar.
const fn top_is_scalar(stack: &[Value]) -> bool {
    matches!(stack.last(), Some(Value::I64(_)) | Some(Value::Bool(_)))
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; fixture stack pop transfers ownership of its top value"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; popping a runtime Vec cannot be evaluated during const evaluation"
)]
fn pop_value(stack: &mut alloc::vec::Vec<Value>, at: &Location) -> Result<Value, Refusal> {
    match stack.pop() {
        Some(value) => Ok(value),
        None => Err(Refusal::new(
            Class::Invalid,
            at.clone(),
            "fixture evaluation underflowed",
        )),
    }
}

/// The value and frame stacks of one fixture evaluation.
struct Evaluation {
    stack: alloc::vec::Vec<Value>,
    frames: alloc::vec::Vec<EvalFrame>,
}

/// Evaluate one completed program over host-supplied inputs. The result is
/// fixture evidence only: it is reported beside checking, never as acceptance.
fn evaluate(
    elements: &[Element],
    inputs: &[Scalar],
    ledger: &mut Ledger,
) -> Result<Option<alloc::vec::Vec<Scalar>>, Refusal> {
    let at = Location::field("request.fixtures");
    let mut evaluation = Evaluation {
        stack: alloc::vec::Vec::with_capacity(MAX_STACK_CAPACITY),
        frames: alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES),
    };
    for input in inputs {
        evaluation.stack.push(match input {
            Scalar::I64(value) => Value::I64(*value),
            Scalar::Bool(value) => Value::Bool(*value),
        });
    }
    evaluation.frames.push(EvalFrame {
        start: 0,
        end: elements.len(),
        index: 0,
    });
    let mut outcome = Ok(true);
    while matches!(outcome, Ok(true)) && !evaluation.frames.is_empty() {
        outcome = evaluate_step(elements, &mut evaluation, ledger, &at);
    }
    if !outcome? {
        return Ok(None);
    }
    debug_assert!(evaluation.stack.len() <= MAX_STACK_CAPACITY);
    let mut out = alloc::vec::Vec::with_capacity(evaluation.stack.len());
    let mut is_scalar_stack = true;
    let mut index = 0usize;
    while is_scalar_stack && index < evaluation.stack.len() {
        match &evaluation.stack[index] {
            Value::I64(value) => out.push(Scalar::I64(*value)),
            Value::Bool(value) => out.push(Scalar::Bool(*value)),
            Value::Program { .. } => is_scalar_stack = false,
        }
        index = index.saturating_add(1);
    }
    if !is_scalar_stack {
        return Ok(None);
    }
    debug_assert!(out.len() <= MAX_STACK_CAPACITY);
    Ok(Some(out))
}

/// One fixture-evaluation step: finish the top frame or run its next element.
/// `Ok(false)` ends the evaluation without a result.
fn evaluate_step(
    elements: &[Element],
    evaluation: &mut Evaluation,
    ledger: &mut Ledger,
    at: &Location,
) -> Result<bool, Refusal> {
    debug_assert!(!evaluation.frames.is_empty());
    debug_assert!(evaluation.stack.len() <= MAX_STACK_CAPACITY);
    let Some(top) = evaluation.frames.len().checked_sub(1) else {
        return Ok(true);
    };
    let Some(frame) = evaluation.frames.get(top) else {
        return Ok(false);
    };
    let start = frame.start;
    let position = frame.index;
    let Some(program) = elements.get(frame.start..frame.end) else {
        return Ok(false);
    };
    if position >= program.len() {
        let _ = evaluation.frames.pop();
        return Ok(true);
    }
    let Some(element) = program.get(position) else {
        return Ok(false);
    };
    if let Some(frame) = evaluation.frames.get_mut(top) {
        frame.index = frame.index.saturating_add(1);
    }
    ledger.charge(Resource::EvaluationSteps, 1, at.clone())?;
    match element {
        Element::I64(value) => evaluation.stack.push(Value::I64(*value)),
        Element::Bool(value) => evaluation.stack.push(Value::Bool(*value)),
        Element::Open => {
            let body_start = position.saturating_add(1);
            let Some((body_end, after)) = collect_quotation(program, body_start) else {
                return Ok(false);
            };
            if let Some(frame) = evaluation.frames.get_mut(top) {
                frame.index = after;
            } else {
                return Ok(false);
            }
            evaluation.stack.push(Value::Program {
                start: start.saturating_add(body_start),
                end: start.saturating_add(body_end),
            });
        }
        Element::Op { op, .. } => {
            eval_operation(*op, &mut evaluation.stack, &mut evaluation.frames, at)?
        }
        Element::Close | Element::Hole => return Ok(false),
    }
    ledger.observe_count(
        Resource::StackHeight,
        encoded_len(evaluation.stack.len()),
        at.clone(),
    )?;
    Ok(true)
}

/// One completion attempt: one shared ledger, one request, one hole equation.
struct Search<'a> {
    namespace: &'a super::Session,
    library: &'a Library,
    request: &'a Request,
    flat: FlatRequest,
    hole_context: HoleFeatureContext,
    stack_in: alloc::vec::Vec<Ty>,
    ledger: Ledger,
    diagnostics: alloc::vec::Vec<Refusal>,
    rejected: u32,
    checked_candidates: u32,
    fixtures: Option<FixtureReport>,
}

/// The worklist of one selected chunk's transitive revalidation.
struct Revalidation {
    pending: alloc::vec::Vec<u16>,
    selected: Option<Expansion>,
}

impl Search<'_> {
    fn report(&self, accepted: u32) -> Report {
        let mut report = Report::of(&self.ledger);
        report.accepted = accepted;
        report.rejected = self.rejected;
        report.checked_candidates = self.checked_candidates;
        report.diagnostics_dropped = self.rejected.saturating_sub(self.diagnostics.len() as u32);
        report.diagnostics = self.diagnostics.clone();
        report.fixtures = self.fixtures.clone();
        report
    }

    fn revalidate_entry(
        namespace: &super::Session,
        library: &Library,
        ledger: &mut Ledger,
        start: u16,
    ) -> Result<Expansion, Refusal> {
        let mut revalidation = Revalidation {
            pending: alloc::vec::Vec::with_capacity(MAX_DEPENDENCY_PENDING),
            selected: None,
        };
        revalidation.pending.push(start);
        let mut outcome = Ok(());
        while outcome.is_ok() && !revalidation.pending.is_empty() {
            outcome = Self::revalidate_step(namespace, library, ledger, start, &mut revalidation);
        }
        outcome?;
        revalidation.selected.ok_or_else(|| {
            internal(
                Location::field("proposal.revalidation"),
                "selected chunk was not revalidated",
            )
        })
    }

    /// Revalidate the next pending library entry and queue its dependencies.
    fn revalidate_step(
        namespace: &super::Session,
        library: &Library,
        ledger: &mut Ledger,
        start: u16,
        revalidation: &mut Revalidation,
    ) -> Result<(), Refusal> {
        let Some(index) = revalidation.pending.pop() else {
            return Ok(());
        };
        let Some(entry) = library.entries().get(usize::from(index)) else {
            return Err(internal(
                Location::field("proposal.sequence"),
                "selected dependency is not retained",
            ));
        };
        let envelope = entry.envelope();
        ledger.observe_count(
            Resource::RecipeNodes,
            encoded_len(envelope.body.nodes.len()),
            Location::field("proposal.revalidation"),
        )?;
        let expansion = expand_recipe(
            library.entries(),
            envelope,
            ledger,
            Location::field("proposal.revalidation"),
        )?;
        replay_interface(
            &expansion.elements,
            entry.interface(),
            ledger,
            Location::field("proposal.revalidation"),
        )?;
        let source = emit(
            &expansion.elements,
            Location::field("proposal.revalidation"),
        )?;
        ledger.observe_count(
            Resource::SourceBytes,
            encoded_len(source.len()),
            Location::field("proposal.revalidation"),
        )?;
        if source != entry.source || canonical_revision(&source) != envelope.source_revision {
            return Err(Refusal::new(
                Class::Invalid,
                Location::field("proposal.revalidation"),
                "retained chunk source no longer matches its immutable recipe revision",
            ));
        }
        let admission = AdmissionContext {
            namespace,
            at: Location::field("proposal.revalidation"),
        };
        let (interface, _) = derived_interface(
            &admission,
            ledger,
            &source,
            &expansion.elements,
            entry.interface(),
        )?;
        if interface != *entry.interface() {
            return Err(Refusal::new(
                Class::Invalid,
                Location::field("proposal.revalidation"),
                "re-admitted chunk interface differs from the immutable library entry",
            ));
        }
        Self::record_selected(revalidation, index == start, expansion);
        Self::queue_dependencies(library, envelope, ledger, revalidation)
    }

    /// Keep `expansion` as the selected chunk's expansion when it is the start entry.
    fn record_selected(revalidation: &mut Revalidation, is_start: bool, expansion: Expansion) {
        if is_start {
            revalidation.selected = Some(expansion);
        }
    }

    /// Queue every dependency of one revalidated entry, in record order.
    fn queue_dependencies(
        library: &Library,
        envelope: &Envelope,
        ledger: &mut Ledger,
        revalidation: &mut Revalidation,
    ) -> Result<(), Refusal> {
        let mut position = 0usize;
        let mut outcome = Ok(());
        while outcome.is_ok() && position < envelope.dependencies.len() {
            outcome = Self::queue_dependency(library, envelope, position, ledger, revalidation);
            position = position.saturating_add(1);
        }
        outcome
    }

    /// Charge one transitive dependency step and queue the entry it resolves to.
    fn queue_dependency(
        library: &Library,
        envelope: &Envelope,
        position: usize,
        ledger: &mut Ledger,
        revalidation: &mut Revalidation,
    ) -> Result<(), Refusal> {
        let Some(dependency) = envelope.dependencies.get(position) else {
            return Err(internal(
                Location::field("proposal.dependencies"),
                "dependency index exceeds the checked record slice",
            ));
        };
        let at = Location::at("proposal.dependencies", position as u32);
        ledger.charge(Resource::DependencySteps, 1, at.clone())?;
        let Some(target) = library.find(dependency) else {
            return Err(Refusal::new(
                Class::Invalid,
                at,
                "transitive dependency no longer resolves by its exact identity and revisions",
            ));
        };
        revalidation.pending.push(target);
        Ok(())
    }

    /// Expand one ordered chunk sequence and compute the ordered stack it
    /// produces at the hole. Every boundary must match exactly.
    fn compose(&mut self, sequence: &[u16]) -> Result<Result<Composition, Refusal>, Refusal> {
        let at = Location::field("proposal");
        let current = self.stack_in.clone();
        let untouched = current.len();
        let mut composition = Composition {
            elements: alloc::vec::Vec::with_capacity(MAX_EXPANDED_NODE_CAPACITY),
            stack: current,
            untouched,
        };
        let mut position = 0usize;
        let mut outcome = Ok(None);
        while matches!(outcome, Ok(None)) && position < sequence.len() {
            outcome = self.compose_step(sequence, position, &mut composition, &at);
            position = position.saturating_add(1);
        }
        match outcome? {
            Some(refusal) => Ok(Err(refusal)),
            None => Ok(Ok(composition)),
        }
    }

    /// Revalidate one selected chunk and apply it at the current composition
    /// boundary. `Ok(Some(..))` refuses this proposal; `Err` stops the search.
    fn compose_step(
        &mut self,
        sequence: &[u16],
        position: usize,
        composition: &mut Composition,
        at: &Location,
    ) -> Result<Option<Refusal>, Refusal> {
        let Some(index) = sequence.get(position) else {
            return Err(internal(
                at.clone(),
                "sequence index exceeds the checked slice",
            ));
        };
        let expansion =
            Self::revalidate_entry(self.namespace, self.library, &mut self.ledger, *index)?;
        // Copy the library reference out: the entry is borrowed while the
        // shared ledger is charged, and the two borrows are independent.
        let library: &Library = self.library;
        let Some(entry) = library.entries().get(usize::from(*index)) else {
            return Err(internal(at.clone(), "selected entry is not retained"));
        };
        composition.elements.extend(expansion.elements);
        let interface = entry.interface();
        if !composition.stack.ends_with(&interface.stack_in) {
            return Ok(Some(Refusal::new(
                Class::Invalid,
                Location::at("proposal.sequence", position as u32),
                "selected chunk does not fit the exact ordered stack at this composition boundary",
            )));
        }
        let above_untouched = composition
            .stack
            .len()
            .saturating_sub(composition.untouched);
        let consumed_from_untouched = interface.stack_in.len().saturating_sub(above_untouched);
        composition.untouched = composition
            .untouched
            .saturating_sub(consumed_from_untouched);
        composition.stack.truncate(
            composition
                .stack
                .len()
                .saturating_sub(interface.stack_in.len()),
        );
        composition.stack.extend_from_slice(&interface.stack_out);
        self.ledger.observe_count(
            Resource::StackHeight,
            encoded_len(composition.stack.len()),
            at.clone(),
        )?;
        Ok(None)
    }

    /// Prepare one emitted program against the original task contract.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; candidate admission mutates the request's shared budget ledger"
    )]
    fn prepare(
        &mut self,
        source: &[u8],
        elements: &[Element],
        declared: &Interface,
        at: Location,
    ) -> Result<Result<(Interface, noble_kernel::untrusted::Candidate), Refusal>, Refusal> {
        let admission = AdmissionContext {
            namespace: self.namespace,
            at,
        };
        match derived_interface(&admission, &mut self.ledger, source, elements, declared) {
            Ok(admitted) => Ok(Ok(admitted)),
            Err(refusal) => match refusal.class {
                Class::Invalid | Class::Unsupported => Ok(Err(refusal)),
                Class::Exhausted | Class::Internal => Err(refusal),
            },
        }
    }

    fn fixtures_for(
        &mut self,
        program: &[Element],
        set: &FixtureSet,
    ) -> Result<FixtureReport, Refusal> {
        let at = Location::field("request.fixtures");
        self.ledger
            .charge(Resource::RetainedBytes, fixture_report_size(set)?, at)?;
        let mut cases = alloc::vec::Vec::with_capacity(set.cases.len());
        let mut is_agreement = true;
        let mut index = 0usize;
        let mut failure = None;
        while failure.is_none() && index < set.cases.len() {
            match self.fixture_case(program, &set.cases[index]) {
                Ok(case) => {
                    is_agreement &= case.agrees;
                    cases.push(case);
                }
                Err(refusal) => failure = Some(refusal),
            }
            index = index.saturating_add(1);
        }
        if let Some(refusal) = failure {
            return Err(refusal);
        }
        Ok(FixtureReport {
            id: set.id,
            cases,
            agreement: is_agreement,
            steps: self.ledger.observed(Resource::EvaluationSteps),
        })
    }

    /// Charge and evaluate one host fixture case against the completed program.
    fn fixture_case(
        &mut self,
        program: &[Element],
        fixture: &Fixture,
    ) -> Result<FixtureCaseReport, Refusal> {
        self.ledger.charge(
            Resource::EvaluationSteps,
            1,
            Location::field("request.fixtures"),
        )?;
        let actual = evaluate(program, &fixture.inputs, &mut self.ledger)?;
        let is_agreed = match &actual {
            Some(values) => *values == fixture.outputs,
            None => false,
        };
        Ok(FixtureCaseReport {
            inputs: fixture.inputs.clone(),
            expected: fixture.outputs.clone(),
            actual,
            agrees: is_agreed,
        })
    }

    /// Attempt one shortlex proposal. `Ok(Step::Refused)` keeps the search
    /// going; `Err` is a request-level budget exhaustion and stops it.
    fn candidate(&mut self, sequence: &[u16]) -> Result<Step, Refusal> {
        let at = Location::field("proposal");
        self.ledger.charge(Resource::Candidates, 1, at.clone())?;
        self.ledger.observe_count(
            Resource::ComposedChunks,
            encoded_len(sequence.len()),
            at.clone(),
        )?;
        let Composition {
            elements,
            stack,
            untouched,
        } = match self.compose(sequence)? {
            Ok(composition) => composition,
            Err(refusal) => return Ok(Step::Refused(refusal)),
        };
        let filled = fill(&self.flat.elements, self.flat.hole, &elements);
        let mut replay = Replay::new(&self.request.task.stack_in);
        let boundary = self.flat.hole.saturating_add(elements.len());
        if let Err(refusal) =
            replay.run(&filled, Some(boundary), &mut self.ledger, true, at.clone())
        {
            return Ok(Step::Refused(refusal));
        }
        let is_marked_stack = replay.marks(&stack);
        if !is_marked_stack {
            return Err(internal(
                at,
                "the ordered equation and the emitted program disagree",
            ));
        }
        if replay.stack != self.request.task.stack_out {
            return Ok(Step::Refused(Refusal::new(
                Class::Invalid,
                at,
                "completed program does not produce the task's exact ordered output stack",
            )));
        }
        let source = match emit(&filled, at.clone()) {
            Ok(source) => source,
            Err(refusal) => return Ok(Step::Refused(refusal)),
        };
        self.ledger
            .observe_count(Resource::SourceBytes, encoded_len(source.len()), at.clone())?;
        let declared = Interface {
            stack_in: self.request.task.stack_in.clone(),
            stack_out: self.request.task.stack_out.clone(),
            effects: self.request.task.allowed_effects.clone(),
        };
        self.checked_candidates = self.checked_candidates.saturating_add(1);
        let (interface, candidate) = match self.prepare(&source, &filled, &declared, at.clone())? {
            Ok(admitted) => admitted,
            Err(refusal) => return Ok(Step::Refused(refusal)),
        };
        let tail = u32::try_from(untouched)
            .map_err(|_| internal(at.clone(), "stack tail exceeds the fixed report width"))?;
        let mut selected = alloc::vec::Vec::with_capacity(sequence.len());
        let mut position = 0usize;
        while position < sequence.len() {
            if let Some(chunk_id) = selected_chunk_id(self.library, sequence, position) {
                selected.push(chunk_id);
            }
            position = position.saturating_add(1);
        }
        let stack_lengths = [
            self.stack_in.len(),
            stack.len(),
            interface.stack_in.len(),
            interface.stack_out.len(),
        ];
        let mut stack_bytes = 0u64;
        for length in stack_lengths {
            stack_bytes = stack_bytes.saturating_add(encoded_len(length));
        }
        let output_bytes = encoded_len(source.len())
            .saturating_mul(2)
            .saturating_add(encoded_len(selected.len()).saturating_mul(IDENTITY_BYTES_U64))
            .saturating_add(stack_bytes);
        self.ledger
            .charge_count(Resource::RetainedBytes, output_bytes, at)?;
        if let Some(set) = &self.request.fixtures {
            self.fixtures = Some(self.fixtures_for(&filled, set)?);
        }
        Ok(Step::Admitted(alloc::boxed::Box::new(Accepted {
            source,
            candidate,
            interface,
            selected,
            equation: Equation {
                stack_in: self.stack_in.clone(),
                stack_out: stack,
                tail,
            },
        })))
    }
}

/// The chunk identity of the retained entry selected at `position`, if any.
fn selected_chunk_id(
    library: &Library,
    sequence: &[u16],
    position: usize,
) -> Option<[u8; IDENTITY_BYTES]> {
    let index = sequence.get(position)?;
    let entry = library.entries().get(usize::from(*index))?;
    Some(entry.envelope().chunk_id)
}

/// The canonical library order: `(stack_in, stack_out, effects, chunk_id)`.
/// Effects are empty by profile, so the identity is the only tie-break left.
/// The order refines equality, so distinct entries are never merged.
const fn rank(ty: &Ty) -> u8 {
    match ty {
        Ty::I64 => 0,
        Ty::Bool => 1,
        _ => 2,
    }
}

fn compare_stack(left: &[Ty], right: &[Ty]) -> core::cmp::Ordering {
    let mut at = 0usize;
    while let (Some(left_ty), Some(right_ty)) = (left.get(at), right.get(at)) {
        let order = rank(left_ty).cmp(&rank(right_ty));
        if order != core::cmp::Ordering::Equal {
            return order;
        }
        at = at.saturating_add(1);
    }
    left.len().cmp(&right.len())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; canonical ordering compares borrowed runtime stack slices"
)]
fn comes_before(left: &Entry, right: &Entry) -> core::cmp::Ordering {
    let order = compare_stack(&left.interface.stack_in, &right.interface.stack_in);
    if order != core::cmp::Ordering::Equal {
        return order;
    }
    let order = compare_stack(&left.interface.stack_out, &right.interface.stack_out);
    if order != core::cmp::Ordering::Equal {
        return order;
    }
    left.envelope.chunk_id.cmp(&right.envelope.chunk_id)
}

fn canonical_order(library: &Library) -> alloc::vec::Vec<u16> {
    let mut order: alloc::vec::Vec<u16> = alloc::vec::Vec::with_capacity(library.entries().len());
    let mut index = 0usize;
    while index < library.entries().len() {
        if let Ok(position) = u16::try_from(index) {
            order.push(position);
        }
        index = index.saturating_add(1);
    }
    order.sort_by(|left, right| {
        match (
            library.entries().get(usize::from(*left)),
            library.entries().get(usize::from(*right)),
        ) {
            (Some(left), Some(right)) => comes_before(left, right),
            (Some(_), None) => core::cmp::Ordering::Less,
            (None, Some(_)) => core::cmp::Ordering::Greater,
            (None, None) => core::cmp::Ordering::Equal,
        }
    });
    debug_assert!(order.len() <= library.entries().len());
    let mut is_ordered = true;
    let mut index = 0usize;
    while index < order.len().saturating_sub(1) {
        match order.get(index..index.saturating_add(2)) {
            Some(pair) => is_ordered &= canonical_pair_is_ordered(library, pair),
            None => {
                is_ordered = false;
                break;
            }
        }
        index = index.saturating_add(1);
    }
    debug_assert!(is_ordered);
    order
}

/// The canonical library identity order used by the shortlex search.
pub fn canonical_entry_order(library: &Library) -> alloc::vec::Vec<u16> {
    canonical_order(library)
}

/// Materialize at most the requested prefix of the canonical shortlex queue,
/// plus a boolean indicating that a later canonical candidate exists.
pub fn candidate_queue(library: &Library, candidate_limit: CandidateLimit) -> CandidateQueue {
    let capped_candidates = if candidate_limit.0 < MAX_CANDIDATES {
        candidate_limit.0
    } else {
        MAX_CANDIDATES
    };
    let bounded_candidate_count = usize::try_from(capped_candidates).unwrap_or(0);
    let order = canonical_entry_order(library);
    debug_assert!(u32::try_from(bounded_candidate_count).is_ok_and(|count| count <= MAX_CANDIDATES));
    debug_assert!(encoded_len(order.len()) <= u64::from(MAX_LIBRARY_ENTRIES));
    let mut build = QueueBuild {
        order,
        bounded_count: bounded_candidate_count,
        queue: CandidateQueue {
            candidates: alloc::vec::Vec::with_capacity(bounded_candidate_count),
            next_candidate: None,
        },
    };
    let mut chunk_count = 0u32;
    while build.queue.next_candidate.is_none() && chunk_count <= MAX_COMPOSED_CHUNKS {
        let count = usize::try_from(chunk_count).unwrap_or(0);
        if count > 0 && build.order.is_empty() {
            break;
        }
        queue_length(&mut build, count);
        chunk_count = chunk_count.saturating_add(1);
    }
    if build.queue.next_candidate.is_some() {
        return build.queue;
    }
    let candidates = build.queue.candidates;
    debug_assert!(candidates.len() <= bounded_candidate_count);
    let mut has_ordered_ordinals = true;
    let mut index = 0usize;
    while index < candidates.len().saturating_sub(1) {
        match (
            candidates.get(index),
            candidates.get(index.saturating_add(1)),
        ) {
            (Some(left), Some(right)) => has_ordered_ordinals &= left.ordinal < right.ordinal,
            _ => {
                has_ordered_ordinals = false;
                break;
            }
        }
        index = index.saturating_add(1);
    }
    debug_assert!(has_ordered_ordinals);
    CandidateQueue {
        candidates,
        next_candidate: None,
    }
}

/// The bounded shortlex candidate queue under construction.
struct QueueBuild {
    order: alloc::vec::Vec<u16>,
    bounded_count: usize,
    queue: CandidateQueue,
}

/// Queue every `count`-chunk sequence in shortlex order until the bound is
/// reached; the first sequence past the bound becomes the overflow witness.
fn queue_length(build: &mut QueueBuild, count: usize) {
    debug_assert!(build.queue.candidates.len() <= build.bounded_count);
    let mut digits: alloc::vec::Vec<u16> = alloc::vec![0; count];
    let mut has_more = true;
    while has_more && build.queue.next_candidate.is_none() {
        let mut sequence = alloc::vec::Vec::with_capacity(count);
        for digit in &digits {
            if let Some(index) = build.order.get(usize::from(*digit)) {
                sequence.push(*index);
            }
        }
        let ordinal = u32::try_from(build.queue.candidates.len()).unwrap_or(MAX_CANDIDATES);
        if build.queue.candidates.len() == build.bounded_count {
            debug_assert!(build.queue.candidates.len() <= build.bounded_count);
            build.queue.next_candidate = Some(QueueCandidate::unscored(ordinal, sequence));
        } else {
            build
                .queue
                .candidates
                .push(QueueCandidate::unscored(ordinal, sequence));
            has_more = advance(&mut digits, build.order.len());
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; this invariant checks runtime library entries through the non-const canonical comparator"
)]
fn canonical_pair_is_ordered(library: &Library, pair: &[u16]) -> bool {
    let (Some(left_index), Some(right_index)) = (pair.first(), pair.get(1)) else {
        return false;
    };
    let (Some(left), Some(right)) = (
        library.entries().get(usize::from(*left_index)),
        library.entries().get(usize::from(*right_index)),
    ) else {
        return false;
    };
    comes_before(left, right) != core::cmp::Ordering::Greater
}

fn check_task(task: &Task, ledger: &mut Ledger) -> Result<(), Refusal> {
    check_stack(&task.stack_in, ledger, "request.task.stack_in")?;
    check_stack(&task.stack_out, ledger, "request.task.stack_out")?;
    if !task.allowed_effects.is_empty() {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("request.task.allowed_effects"),
            "pure-i64-bool/1 requires an empty allowed effect set",
        ));
    }
    Ok(())
}

fn fixture_values_match_types(values: &[Scalar], expected: &[Ty]) -> bool {
    if values.len() != expected.len() {
        return false;
    }
    let mut index = 0usize;
    let mut is_match = true;
    while is_match && index < values.len() {
        is_match = scalar_has_type_at(values, expected, index);
        index = index.saturating_add(1);
    }
    is_match
}

/// Whether the scalar at `index` has exactly the expected profile type at `index`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; fixture lookups use the non-const slice `get` and derived type equality"
)]
fn scalar_has_type_at(values: &[Scalar], expected: &[Ty], index: usize) -> bool {
    match (values.get(index), expected.get(index)) {
        (Some(value), Some(ty)) => value.ty() == *ty,
        _ => false,
    }
}

fn check_fixtures(request: &Request) -> Result<(), Refusal> {
    let Some(set) = &request.fixtures else {
        return Ok(());
    };
    let mut position = 0usize;
    let mut failure = None;
    while failure.is_none() && position < set.cases.len() {
        failure = fixture_case_refusal(request, set, position);
        position = position.saturating_add(1);
    }
    if let Some(refusal) = failure {
        return Err(refusal);
    }
    let mut has_matching_fixture_inputs = true;
    let mut has_matching_fixture_outputs = true;
    for fixture in &set.cases {
        has_matching_fixture_inputs &= fixture.inputs.len() == request.task.stack_in.len();
        has_matching_fixture_outputs &= fixture.outputs.len() == request.task.stack_out.len();
    }
    debug_assert!(has_matching_fixture_inputs);
    debug_assert!(has_matching_fixture_outputs);
    Ok(())
}

/// The refusal for one fixture case whose values do not match the task interface.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; fixture case lookups use the non-const slice `get` and refusals allocate their message"
)]
fn fixture_case_refusal(request: &Request, set: &FixtureSet, position: usize) -> Option<Refusal> {
    let Some(fixture) = set.cases.get(position) else {
        return Some(internal(
            Location::field("request.fixtures.cases"),
            "fixture index exceeds the checked case slice",
        ));
    };
    let is_input_match = fixture_values_match_types(&fixture.inputs, &request.task.stack_in);
    let is_output_match = fixture_values_match_types(&fixture.outputs, &request.task.stack_out);
    if is_input_match && is_output_match {
        return None;
    }
    Some(Refusal::new(
        Class::Invalid,
        Location::at("request.fixtures.cases", position as u32),
        "fixture values do not match the original ordered task interface",
    ))
}

const fn scalar_encoded_size(value: Scalar) -> u64 {
    match value {
        Scalar::I64(_) => 9,
        Scalar::Bool(_) => 2,
    }
}

/// Size one retained refusal using the fixed report representation.
fn refusal_encoded_size(refusal: &Refusal) -> Result<u32, Refusal> {
    let mut encoded_bytes = 0u64;
    let at = &refusal.location;
    add_encoded_size(
        &mut encoded_bytes,
        4u64.saturating_add(encoded_len(refusal.location.field.len())),
        at,
    )?;
    add_encoded_size(
        &mut encoded_bytes,
        1u64.saturating_add(if refusal.location.index.is_some() {
            4
        } else {
            0
        }),
        at,
    )?;
    add_encoded_size(&mut encoded_bytes, 1, at)?;
    add_encoded_size(
        &mut encoded_bytes,
        1u64.saturating_add(if refusal.resource.is_some() { 4 } else { 0 }),
        at,
    )?;
    add_encoded_size(&mut encoded_bytes, 12, at)?;
    add_encoded_size(&mut encoded_bytes, encoded_len(refusal.message.len()), at)?;
    debug_assert!(encoded_bytes > 0);
    debug_assert!(encoded_bytes <= u64::from(MAX_RETAINED_BYTES));
    u32::try_from(encoded_bytes).map_err(|_| {
        let mut exhausted = Refusal::new(
            Class::Exhausted,
            at.clone(),
            "retained diagnostic exceeds the finite byte counter",
        );
        exhausted.resource = Some(Resource::RetainedBytes);
        exhausted.observed = u32::MAX;
        exhausted.limit = MAX_RETAINED_BYTES;
        exhausted
    })
}

/// Upper-bound the report copy of host fixtures before allocating its cases.
/// An accepted, well-typed program returns scalar outputs of the expected
/// shape; charging those bytes up front keeps fixture evidence inside the same
/// request/library retention budget.
fn fixture_report_size(set: &FixtureSet) -> Result<u32, Refusal> {
    let at = Location::field("report.fixtures");
    let mut encoded_bytes = 0u64;
    add_encoded_size(
        &mut encoded_bytes,
        1u64.saturating_add(IDENTITY_BYTES_U64)
            .saturating_add(4)
            .saturating_add(1)
            .saturating_add(4),
        &at,
    )?;
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < set.cases.len() {
        outcome = add_fixture_report_case_size(&mut encoded_bytes, &set.cases[index], &at);
        index = index.saturating_add(1);
    }
    outcome?;
    debug_assert!(encoded_bytes > 0);
    debug_assert!(encoded_bytes <= u64::from(MAX_RETAINED_BYTES));
    u32::try_from(encoded_bytes).map_err(|_| {
        let mut exhausted = Refusal::new(
            Class::Exhausted,
            at,
            "fixture report exceeds the finite retained-byte counter",
        );
        exhausted.resource = Some(Resource::RetainedBytes);
        exhausted.observed = u32::MAX;
        exhausted.limit = MAX_RETAINED_BYTES;
        exhausted
    })
}

/// Add one fixture case's report copy: inputs, expected and actual outputs.
fn add_fixture_report_case_size(
    encoded_bytes: &mut u64,
    fixture: &Fixture,
    at: &Location,
) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    add_scalar_sizes(encoded_bytes, &fixture.inputs, at)?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_scalar_sizes(encoded_bytes, &fixture.outputs, at)?;
    add_encoded_size(encoded_bytes, 1 + 4, at)?;
    add_scalar_sizes(encoded_bytes, &fixture.outputs, at)?;
    add_encoded_size(encoded_bytes, 1, at)
}

/// Add each scalar's fixed encoding, in order.
fn add_scalar_sizes(
    encoded_bytes: &mut u64,
    values: &[Scalar],
    at: &Location,
) -> Result<(), Refusal> {
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < values.len() {
        outcome = add_encoded_size(encoded_bytes, scalar_encoded_size(values[index]), at);
        index = index.saturating_add(1);
    }
    outcome
}

/// Count the canonical request representation before walking its tree. This
/// makes the retained/request-byte limit apply to attacker-controlled words,
/// nested editor nodes, fixture values and metadata, not only emitted source.
fn request_size(request: &Request, ledger: &mut Ledger) -> Result<u32, Refusal> {
    let at = Location::field("request.retained");
    let mut encoded_bytes = 0u64;
    add_request_header_size(&mut encoded_bytes, request, &at)?;
    add_editor_tree_size(&mut encoded_bytes, &request.editor.nodes, ledger, &at)?;
    add_request_tail_size(&mut encoded_bytes, request, &at)?;
    u32::try_from(encoded_bytes)
        .map_err(|_| internal(at, "bounded request size did not fit the reporting counter"))
}

fn add_request_header_size(
    encoded_bytes: &mut u64,
    request: &Request,
    at: &Location,
) -> Result<(), Refusal> {
    add_encoded_size(
        encoded_bytes,
        12u64.saturating_add(encoded_len(request.profile.len())),
        at,
    )?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(encoded_bytes, encoded_len(request.task.stack_in.len()), at)?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(encoded_bytes, encoded_len(request.task.stack_out.len()), at)?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(request.task.allowed_effects.as_slice().len()).saturating_mul(4),
        at,
    )?;
    add_encoded_size(encoded_bytes, 8, at)
}

fn add_editor_tree_size(
    encoded_bytes: &mut u64,
    editor_nodes: &[super::editor::Node],
    ledger: &mut Ledger,
    at: &Location,
) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    let mut walk = SizeWalk {
        frames: alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES),
    };
    walk.frames.push(SizeFrame {
        path: alloc::vec::Vec::new(),
        index: 0,
        depth: 0,
    });
    add_encoded_size(encoded_bytes, 4, at)?;
    let mut outcome = Ok(());
    while outcome.is_ok() && !walk.frames.is_empty() {
        outcome = editor_size_step(&mut walk, editor_nodes, encoded_bytes, ledger, at);
    }
    outcome
}

/// One request-size walk frame: the quotation positions from the root node
/// list (so the frame holds no borrow), the next node and its quotation depth.
struct SizeFrame {
    path: alloc::vec::Vec<usize>,
    index: usize,
    depth: u32,
}

/// The frame stack of the request-size editor walk.
struct SizeWalk {
    frames: alloc::vec::Vec<SizeFrame>,
}

/// One request-size walk step: finish the top frame or size its next node.
fn editor_size_step(
    walk: &mut SizeWalk,
    editor_nodes: &[super::editor::Node],
    encoded_bytes: &mut u64,
    ledger: &mut Ledger,
    at: &Location,
) -> Result<(), Refusal> {
    let Some(top) = walk.frames.len().checked_sub(1) else {
        return Ok(());
    };
    let Some(frame) = walk.frames.get(top) else {
        return Err(internal(at.clone(), "request size walk lost its frame"));
    };
    let Some(nodes) = editor_nodes_at(editor_nodes, &frame.path) else {
        return Err(internal(at.clone(), "request size walk lost its frame"));
    };
    let index = frame.index;
    let depth = frame.depth;
    if index >= nodes.len() {
        let _ = walk.frames.pop();
        return Ok(());
    }
    let Some(node) = nodes.get(index) else {
        return Err(internal(
            at.clone(),
            "request size walk lost an editor node",
        ));
    };
    if let Some(frame) = walk.frames.get_mut(top) {
        frame.index = index.saturating_add(1);
    }
    debug_assert!(index.saturating_add(1) <= nodes.len());
    match node {
        super::editor::Node::Integer(_) => add_encoded_size(encoded_bytes, 9, at)?,
        super::editor::Node::Boolean(_) => add_encoded_size(encoded_bytes, 2, at)?,
        super::editor::Node::Word(word) => {
            add_encoded_size(
                encoded_bytes,
                5u64.saturating_add(encoded_len(word.len())),
                at,
            )?;
        }
        super::editor::Node::Hole => add_encoded_size(encoded_bytes, 1, at)?,
        super::editor::Node::Quotation(_) => {
            add_encoded_size(encoded_bytes, 5, at)?;
            let next_depth = depth.saturating_add(1);
            ledger.observe(Resource::QuotationDepth, next_depth, at.clone())?;
            let Some(frame) = walk.frames.get(top) else {
                return Err(internal(at.clone(), "request size walk lost its frame"));
            };
            let mut path = frame.path.clone();
            path.push(index);
            walk.frames.push(SizeFrame {
                path,
                index: 0,
                depth: next_depth,
            });
            debug_assert!(walk.frames.len() <= MAX_QUOTATION_FRAMES);
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; request tail size is derived from hostile caller data and over-budget inputs must return typed refusals instead of assertion panics"
)]
fn add_request_tail_size(
    encoded_bytes: &mut u64,
    request: &Request,
    at: &Location,
) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(request.hole_path.len()).saturating_mul(4),
        at,
    )?;
    add_encoded_size(encoded_bytes, REVISION_BYTES_U64, at)?;
    let budget_bytes = encoded_len(Resource::COUNT).saturating_mul(4);
    add_encoded_size(encoded_bytes, budget_bytes, at)?;
    if let Some(fixtures) = &request.fixtures {
        let fixture_header_bytes = 21u64;
        add_encoded_size(encoded_bytes, fixture_header_bytes, at)?;
        let mut index = 0usize;
        let mut outcome = Ok(());
        while outcome.is_ok() && index < fixtures.cases.len() {
            outcome = add_request_fixture_size(encoded_bytes, &fixtures.cases[index], at);
            index = index.saturating_add(1);
        }
        outcome?;
    } else {
        add_encoded_size(encoded_bytes, 1, at)?;
    }
    Ok(())
}

/// Add one request fixture case: its input and expected output scalars.
fn add_request_fixture_size(
    encoded_bytes: &mut u64,
    fixture: &Fixture,
    at: &Location,
) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    add_scalar_sizes(encoded_bytes, &fixture.inputs, at)?;
    add_encoded_size(encoded_bytes, 4, at)?;
    add_scalar_sizes(encoded_bytes, &fixture.outputs, at)
}

/// Validate the request envelope, its task contract, its editor tree and its
/// library pin before any proposal is attempted.
fn check_request(
    request: &Request,
    library: &Library,
    ledger: &mut Ledger,
) -> Result<FlatRequest, Refusal> {
    check_request_versions(request)?;
    charge_request_retention(request, library, ledger)?;
    if request.profile.as_str() != PROFILE {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("request.profile"),
            "unsupported completion profile",
        ));
    }
    check_task(&request.task, ledger)?;
    check_fixtures(request)?;
    if request.editor.format != super::editor::FORMAT {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("request.editor.format"),
            "unsupported editor transport format",
        ));
    }
    let flat = flatten_request(request, ledger)?;
    let context_bytes = editor_context_bytes(&flat.elements, Location::field("request.editor"))?;
    ledger.observe(
        Resource::SourceBytes,
        context_bytes,
        Location::field("request.editor"),
    )?;
    if request.library_revision != *library.revision() {
        return Err(Refusal::new(
            Class::Invalid,
            Location::field("request.library_revision"),
            "request pins a library revision the snapshot does not hold",
        ));
    }
    debug_assert!(flat.hole < flat.elements.len());
    debug_assert_eq!(request.library_revision, *library.revision());
    Ok(flat)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; unsupported versions produce an allocated refusal message"
)]
fn check_request_versions(request: &Request) -> Result<(), Refusal> {
    if request.format != ENVELOPE_FORMAT {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("request.format"),
            "unsupported completion request format",
        ));
    }
    if request.semantic_revision != SEMANTIC_REVISION {
        return Err(Refusal::new(
            Class::Unsupported,
            Location::field("request.semantic_revision"),
            "unsupported completion request semantic revision",
        ));
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; request and library retention are charged through the shared bounded ledger, which must refuse oversized caller data rather than assert"
)]
fn charge_request_retention(
    request: &Request,
    library: &Library,
    ledger: &mut Ledger,
) -> Result<(), Refusal> {
    let request_bytes = request_size(request, ledger)?;
    ledger.charge(
        Resource::LibraryBytes,
        library.retained(),
        Location::field("request.library_bytes"),
    )?;
    ledger.charge_count(
        Resource::Entries,
        encoded_len(library.entries().len()),
        Location::field("request.library_entries"),
    )?;
    let total_retained = request_bytes
        .checked_add(library.retained())
        .ok_or_else(|| {
            internal(
                Location::field("request.retained"),
                "retained byte sum overflowed",
            )
        })?;
    ledger.charge(
        Resource::RetainedBytes,
        total_retained,
        Location::field("request.retained"),
    )?;
    Ok(())
}

/// Reuse the editor's source-aware analysis for context, charging its measured
/// work to the same request ledger as later replay and candidate admission.
#[derive(Clone, Copy)]
struct HoleFeatureContext {
    required_output_suffix_height: usize,
    known_effect_count: usize,
    unresolved_effect: bool,
}

fn analyze_editor_context(
    namespace: &super::Session,
    request: &Request,
    ledger: &mut Ledger,
) -> Result<HoleFeatureContext, Refusal> {
    let at = Location::field("request.editor");
    let limits = crate::Limits {
        work: ledger.remaining(Resource::CheckingWork),
        ..crate::Limits::default()
    };
    let (analysis, work_used) =
        match namespace.analyze_editor_metered(&request.editor, &request.task.stack_in, limits) {
            Ok((analysis, work_used)) => (analysis, work_used),
            Err(failure) => {
                ledger.charge(Resource::CheckingWork, failure.work_used(), at.clone())?;
                if failure.diagnostic().kind == crate::DiagnosticKind::Exhausted {
                    // `Candidate::build` can refuse before its meter is returned;
                    // reserve the remaining allowance on this fail-closed path.
                    let remaining = ledger.remaining(Resource::CheckingWork);
                    let observed = remaining.saturating_add(1);
                    return match ledger.charge(Resource::CheckingWork, observed, at.clone()) {
                        Err(refusal) => Err(refusal),
                        Ok(()) => Err(internal(
                            at,
                            "exhausted editor analysis did not exhaust shared checking work",
                        )),
                    };
                }
                return Err(refusal_of_failure(&failure, at));
            }
        };
    ledger.charge(Resource::CheckingWork, work_used, at.clone())?;
    if analysis.holes.len() != 1 {
        return Err(internal(
            at,
            "source editor analysis disagrees with the bounded one-hole transport",
        ));
    }
    // The hole itself has an unresolved effect by design. Known surrounding
    // effects are not permitted; final source admission rechecks the filled tree.
    if !analysis.known_effects.is_empty() {
        return Err(Refusal::new(
            Class::Unsupported,
            at,
            "completion editor context contains a known effect",
        ));
    }
    debug_assert_eq!(analysis.holes.len(), 1);
    debug_assert!(analysis.known_effects.is_empty());
    let Some(hole) = analysis.holes.first() else {
        return Err(internal(
            at,
            "source editor analysis lost the bounded one-hole constraint",
        ));
    };
    Ok(HoleFeatureContext {
        required_output_suffix_height: hole.required_output_suffix.len(),
        known_effect_count: hole.known_effects.len(),
        unresolved_effect: hole.unresolved_effect,
    })
}

/// Advance one shortlex odometer over `width` canonical positions. Returns
/// false once every sequence of this length has been visited.
fn advance(digits: &mut [u16], width: usize) -> bool {
    let mut at = digits.len();
    while at > 0 {
        at = at.saturating_sub(1);
        let Some(digit) = digits.get_mut(at) else {
            return false;
        };
        let next = usize::from(*digit).saturating_add(1);
        if next < width {
            *digit = next as u16;
            return true;
        }
        *digit = 0;
    }
    false
}

struct CandidateObservation<'a> {
    binding: Option<([u8; REVISION_BYTES], CandidateId)>,
    ordinal: u32,
    sequence: &'a [u16],
    accepted: bool,
    refusal_class: Option<Class>,
    fixture_agreement: Option<bool>,
    cost: CandidateAttemptCost,
}

struct CandidateCapture<'callback, 'clock> {
    observer: Option<ObserverHandle<'callback>>,
    identity: Option<RankingIdentity>,
    clock: &'clock mut MonotonicClock,
}

struct CandidateAttempt<'a> {
    ordinal: u32,
    sequence: &'a [u16],
    identity: Option<CandidateId>,
    feature_extraction_ns: u64,
    scorer_elapsed_ns: u64,
}

struct CandidateAttemptOutcome<'a> {
    ordinal: u32,
    sequence: &'a [u16],
    binding: Option<([u8; REVISION_BYTES], CandidateId)>,
    cost: CandidateAttemptCost,
}

struct CandidateAttemptMeasurement<'clock> {
    resources_before: [u32; Resource::COUNT],
    started_ns: u64,
    feature_extraction_ns: u64,
    scorer_elapsed_ns: u64,
    clock: &'clock mut MonotonicClock,
}

struct CandidateScoringControl<'clock> {
    request_identity: [u8; REVISION_BYTES],
    should_measure_candidate_time: bool,
    clock: &'clock mut MonotonicClock,
}

fn resource_snapshot(ledger: &Ledger) -> [u32; Resource::COUNT] {
    let mut snapshot = [0; Resource::COUNT];
    for resource in Resource::ALL {
        if let Some(value) = snapshot.get_mut(resource.slot()) {
            *value = ledger.observed(resource);
        }
    }
    snapshot
}

fn candidate_attempt_cost(
    ledger: &Ledger,
    measurement: CandidateAttemptMeasurement<'_>,
) -> CandidateAttemptCost {
    let resource_deltas = resource_deltas(ledger, &measurement.resources_before);
    CandidateAttemptCost {
        feature_extraction_ns: measurement.feature_extraction_ns,
        scorer_elapsed_ns: measurement.scorer_elapsed_ns,
        re_admission_ns: measurement
            .clock
            .now_ns()
            .saturating_sub(measurement.started_ns),
        resource_deltas,
    }
}

/// Each resource's ledger growth since `before`, in [`Resource::ALL`] order.
fn resource_deltas(ledger: &Ledger, before: &[u32; Resource::COUNT]) -> [u32; Resource::COUNT] {
    let mut deltas = [0; Resource::COUNT];
    for resource in Resource::ALL {
        if let (Some(previous), Some(delta)) =
            (before.get(resource.slot()), deltas.get_mut(resource.slot()))
        {
            *delta = ledger.observed(resource).saturating_sub(*previous);
        }
    }
    deltas
}

fn observe_candidate(
    capture: &mut CandidateCapture<'_, '_>,
    observation: CandidateObservation<'_>,
) {
    debug_assert!(encoded_len(observation.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    debug_assert_eq!(observation.accepted, observation.refusal_class.is_none());
    if let Some(observer) = &mut capture.observer {
        let Some((request_identity, candidate)) = observation.binding else {
            debug_assert!(
                false,
                "observed candidates must have a bound request identity"
            );
            return;
        };
        let outcome = CandidateOutcome {
            request_identity,
            candidate,
            ordinal: observation.ordinal,
            sequence: observation.sequence.to_vec(),
            accepted: observation.accepted,
            refusal_class: observation.refusal_class,
            fixture_agreement: observation.fixture_agreement,
            cost: observation.cost,
        };
        observer.observe(&outcome);
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; binding an observed candidate digests its runtime identity and refusals allocate their message"
)]
fn candidate_binding(
    search: &Search<'_>,
    attempt: &CandidateAttempt<'_>,
    is_observed: bool,
    identity: Option<&RankingIdentity>,
) -> Result<Option<([u8; REVISION_BYTES], CandidateId)>, Refusal> {
    debug_assert!(attempt.ordinal <= MAX_CANDIDATES);
    debug_assert!(encoded_len(attempt.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    if !is_observed {
        return Ok(None);
    }
    let Some(identity) = identity else {
        return Err(internal(
            Location::field("ranking.identity"),
            "an observation request has no immutable identity",
        ));
    };
    let candidate = match attempt.identity {
        Some(candidate) => candidate,
        None => candidate_identity(search.library, attempt.ordinal, attempt.sequence),
    };
    Ok(Some((identity.request, candidate)))
}

fn observe_refusal(
    attempt: &CandidateAttemptOutcome<'_>,
    refusal: &Refusal,
    capture: &mut CandidateCapture<'_, '_>,
) {
    let refusal_class = refusal.class;
    observe_candidate(
        capture,
        CandidateObservation {
            binding: attempt.binding,
            ordinal: attempt.ordinal,
            sequence: attempt.sequence,
            accepted: false,
            refusal_class: Some(refusal_class),
            fixture_agreement: None,
            cost: attempt.cost,
        },
    );
}

fn accept_candidate(
    search: &mut Search<'_>,
    attempt: &CandidateAttemptOutcome<'_>,
    accepted: alloc::boxed::Box<Accepted>,
    capture: &mut CandidateCapture<'_, '_>,
) -> Result<(), alloc::boxed::Box<Response>> {
    debug_assert!(attempt.ordinal <= MAX_CANDIDATES);
    debug_assert_eq!(accepted.selected.len(), attempt.sequence.len());
    let fixture_agreement = search.fixtures.as_ref().map(|fixtures| fixtures.agreement);
    observe_candidate(
        capture,
        CandidateObservation {
            binding: attempt.binding,
            ordinal: attempt.ordinal,
            sequence: attempt.sequence,
            accepted: true,
            refusal_class: None,
            fixture_agreement,
            cost: attempt.cost,
        },
    );
    let report = search.report(1);
    Err(alloc::boxed::Box::new(Response::Accepted {
        accepted,
        report: alloc::boxed::Box::new(report),
    }))
}

fn run_candidate_attempt(
    search: &mut Search<'_>,
    attempt: CandidateAttempt<'_>,
    capture: &mut CandidateCapture<'_, '_>,
) -> Result<(), alloc::boxed::Box<Response>> {
    debug_assert!(attempt.ordinal <= MAX_CANDIDATES);
    debug_assert!(encoded_len(attempt.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    let binding = match candidate_binding(
        search,
        &attempt,
        capture.observer.is_some(),
        capture.identity.as_ref(),
    ) {
        Ok(binding) => binding,
        Err(refusal) => return Err(alloc::boxed::Box::new(response_of_search(refusal, search))),
    };
    let resources_before = if capture.observer.is_some() {
        Some(resource_snapshot(&search.ledger))
    } else {
        None
    };
    let started_ns = if resources_before.is_some() {
        capture.clock.now_ns()
    } else {
        0
    };
    let result = search.candidate(attempt.sequence);
    let cost = match resources_before {
        Some(resources_before) => candidate_attempt_cost(
            &search.ledger,
            CandidateAttemptMeasurement {
                resources_before,
                started_ns,
                feature_extraction_ns: attempt.feature_extraction_ns,
                scorer_elapsed_ns: attempt.scorer_elapsed_ns,
                clock: capture.clock,
            },
        ),
        None => CandidateAttemptCost::default(),
    };
    let outcome = CandidateAttemptOutcome {
        ordinal: attempt.ordinal,
        sequence: attempt.sequence,
        binding,
        cost,
    };
    match result {
        Ok(Step::Admitted(accepted)) => accept_candidate(search, &outcome, accepted, capture),
        Ok(Step::Refused(refusal)) => {
            observe_refusal(&outcome, &refusal, capture);
            search.rejected = search.rejected.saturating_add(1);
            if let Err(exhausted) = record(&mut search.ledger, &mut search.diagnostics, refusal) {
                return Err(alloc::boxed::Box::new(response_of_search(
                    exhausted, search,
                )));
            }
            Ok(())
        }
        Err(refusal) => {
            observe_refusal(&outcome, &refusal, capture);
            Err(alloc::boxed::Box::new(response_of_search(refusal, search)))
        }
    }
}

/// Visit every sequence of one length in canonical shortlex order.
fn run_length(
    search: &mut Search<'_>,
    order: &[u16],
    chunk_count: u32,
    next_ordinal: &mut u32,
    capture: &mut CandidateCapture<'_, '_>,
) -> Result<(), alloc::boxed::Box<Response>> {
    let width = order.len();
    debug_assert!(u32::try_from(width).is_ok_and(|count| count <= MAX_LIBRARY_ENTRIES));
    debug_assert!(chunk_count <= MAX_COMPOSED_CHUNKS);
    debug_assert!(*next_ordinal <= MAX_CANDIDATES);
    if capture.observer.is_some() && capture.identity.is_none() {
        let refusal = internal(
            Location::field("ranking.identity"),
            "an observation request has no immutable identity",
        );
        return Err(alloc::boxed::Box::new(response_of_search(refusal, search)));
    }
    if chunk_count > 0 && width == 0 {
        return Ok(());
    }
    let count = match usize::try_from(chunk_count) {
        Ok(count) => count,
        Err(_) => {
            let refusal = internal(
                Location::field("request.budget.max_composed_chunks"),
                "fixed chunk count exceeds the host address space",
            );
            return Err(alloc::boxed::Box::new(response_of_search(refusal, search)));
        }
    };
    let mut digits: alloc::vec::Vec<u16> = alloc::vec![0u16; count];
    let mut sequence: alloc::vec::Vec<u16> = alloc::vec::Vec::with_capacity(count);
    let mut outcome = Ok(());
    let mut has_next = true;
    while outcome.is_ok() && has_next {
        sequence.clear();
        for digit in &digits {
            if let Some(index) = order_index(order, *digit) {
                sequence.push(index);
            }
        }
        debug_assert_eq!(sequence.len(), digits.len());
        let ordinal = *next_ordinal;
        *next_ordinal = (*next_ordinal).saturating_add(1);
        outcome = run_candidate_attempt(
            search,
            CandidateAttempt {
                ordinal,
                sequence: &sequence,
                identity: None,
                feature_extraction_ns: 0,
                scorer_elapsed_ns: 0,
            },
            capture,
        );
        has_next = outcome.is_ok() && advance(&mut digits, width);
    }
    outcome
}

/// The library entry index that `digit` selects in the canonical order.
fn order_index(order: &[u16], digit: u16) -> Option<u16> {
    order.get(usize::from(digit)).copied()
}

/// `min(value, maximum) / maximum` as `f32`, or zero for a zero maximum.
/// Float literals, integer-to-float casts and the float `/` operator are
/// outside the pinned Aeneas, so the ratio is built from exact conversions
/// and the `Div` trait; the result is bit-identical to the cast quotient.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the ratio is built from `From` conversions and the `Div` trait, which are not stable const traits"
)]
fn feature_ratio(value: u64, maximum: u32) -> f32 {
    if maximum == 0 {
        f32::from(0_u8)
    } else {
        let bounded = match u32::try_from(value) {
            Ok(count) => {
                if count < maximum {
                    count
                } else {
                    maximum
                }
            }
            Err(_) => maximum,
        };
        core::ops::Div::div(count_as_f32(bounded), count_as_f32(maximum))
    }
}

/// The exact `value as f32`: both 16-bit halves convert losslessly and the
/// 2^16 scaling is exact, so the one rounding of the final sum is the
/// round-to-nearest-even rounding the cast performs.
fn count_as_f32(value: u32) -> f32 {
    let bytes = value.to_le_bytes();
    let low = f32::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    let high = f32::from(u16::from_le_bytes([bytes[2], bytes[3]]));
    let half_scale = f32::from(256_u16);
    let scale = core::ops::Mul::mul(half_scale, half_scale);
    core::ops::Add::add(core::ops::Mul::mul(high, scale), low)
}

/// One for `true` and zero for `false`, as `f32`.
fn feature_bit(value: bool) -> f32 {
    f32::from(value)
}

fn feature_type_count(stack: &[Ty], ty: Ty) -> u64 {
    let mut count = 0usize;
    let mut index = 0usize;
    while index < stack.len() {
        if is_same_type(&stack[index], &ty) {
            count = count.saturating_add(1);
        }
        index = index.saturating_add(1);
    }
    encoded_len(count)
}

/// Whether two kernel types are equal.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; identity comparison uses the non-const derived type equality"
)]
fn is_same_type(left: &Ty, right: &Ty) -> bool {
    *left == *right
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; applying an admitted interface must truncate and append to the owned working stack"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; applying an interface mutates the bounded runtime stack"
)]
fn feature_apply_interface(stack: &mut alloc::vec::Vec<Ty>, interface: &Interface) -> bool {
    let input_len = interface.stack_in.len();
    if stack.len() < input_len {
        return false;
    }
    let is_matching_suffix = stack_ends_with(stack, &interface.stack_in);
    if !is_matching_suffix {
        return false;
    }
    stack.truncate(stack.len().saturating_sub(input_len));
    stack.extend_from_slice(&interface.stack_out);
    match usize::try_from(MAX_STACK_HEIGHT) {
        Ok(maximum) => stack.len() <= maximum,
        Err(_) => false,
    }
}

/// Whether `stack` ends with exactly the interface inputs `inputs`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the suffix lookup uses the non-const slice `get` and slice equality"
)]
fn stack_ends_with(stack: &[Ty], inputs: &[Ty]) -> bool {
    match stack.get(stack.len().saturating_sub(inputs.len())..) {
        Some(suffix) => *suffix == *inputs,
        None => false,
    }
}

const fn recipe_node_bytes(node: &Node) -> u64 {
    match node {
        Node::I64(_) => 9,
        Node::Bool(_) => 2,
        Node::Capture { .. } => 3,
        Node::Operation { .. } => 4,
        Node::Invoke { .. } => 5,
        Node::Quotation { body, .. } => {
            7u64.saturating_add(encoded_len(body.len()).saturating_mul(4))
        }
    }
}

fn recipe_quotation_depth(recipe: &Recipe) -> Option<u32> {
    debug_assert!(encoded_len(recipe.nodes.len()) <= u64::from(MAX_RECIPE_NODES));
    debug_assert!(encoded_len(recipe.entry.len()) <= u64::from(MAX_RECIPE_NODES));
    let mut node_depths = [0u32; 64];
    let mut node_index = 0usize;
    let mut is_valid = true;
    while is_valid && node_index < recipe.nodes.len() {
        is_valid = record_node_depth(recipe, node_index, &mut node_depths);
        node_index = node_index.saturating_add(1);
    }
    let mut deepest = 0u32;
    let mut entry_index = 0usize;
    while is_valid && entry_index < recipe.entry.len() {
        match entry_depth(&node_depths, recipe.entry[entry_index]) {
            Some(depth) => {
                if depth > deepest {
                    deepest = depth;
                }
            }
            None => is_valid = false,
        }
        entry_index = entry_index.saturating_add(1);
    }
    if is_valid {
        Some(deepest)
    } else {
        None
    }
}

/// Record one recipe node's quotation depth. False when the node is outside
/// the depth table or a quotation references a node that is not earlier.
fn record_node_depth(recipe: &Recipe, node_index: usize, node_depths: &mut [u32; 64]) -> bool {
    let Some(node) = recipe.nodes.get(node_index) else {
        return false;
    };
    if node_index >= node_depths.len() {
        return false;
    }
    if let Node::Quotation { body, .. } = node {
        let Some(nested_depth) = deepest_child(body, node_index, node_depths) else {
            return false;
        };
        node_depths[node_index] = nested_depth.saturating_add(1);
    }
    true
}

/// The deepest recorded depth among a quotation body's references, each of
/// which must point to a node before `node_index`.
fn deepest_child(body: &[u32], node_index: usize, node_depths: &[u32; 64]) -> Option<u32> {
    debug_assert!(node_index < node_depths.len());
    let mut nested_depth = 0u32;
    let mut index = 0usize;
    let mut is_valid = true;
    while is_valid && index < body.len() {
        match child_depth(body[index], node_index, node_depths) {
            Some(depth) => {
                if depth > nested_depth {
                    nested_depth = depth;
                }
            }
            None => is_valid = false,
        }
        index = index.saturating_add(1);
    }
    debug_assert!(!is_valid || index == body.len());
    if is_valid {
        Some(nested_depth)
    } else {
        None
    }
}

/// The recorded depth of a strictly earlier referenced node.
fn child_depth(reference: u32, node_index: usize, node_depths: &[u32; 64]) -> Option<u32> {
    let child_index = usize::try_from(reference).ok()?;
    if child_index >= node_index {
        return None;
    }
    Some(node_depths[child_index])
}

/// The recorded depth of one recipe entry reference.
fn entry_depth(node_depths: &[u32; 64], reference: u32) -> Option<u32> {
    let index = usize::try_from(reference).ok()?;
    node_depths.get(index).copied()
}

#[derive(Default)]
struct CandidateFeatureCounts {
    stack_after: alloc::vec::Vec<Ty>,
    expanded_nodes: u64,
    capture_count: u64,
    dependency_count: u64,
    instantiation_count: u64,
    source_bytes: u64,
    recipe_bytes: u64,
    literals: u64,
    routing: u64,
    arithmetic: u64,
    boolean_ops: u64,
    quotations: u64,
    runs: u64,
    quote_depth: u64,
    stack_matches: bool,
    effects_match: bool,
}

fn feature_recipe_metrics(recipe: &Recipe, counts: &mut CandidateFeatureCounts) {
    debug_assert!(encoded_len(recipe.nodes.len()) <= u64::from(MAX_RECIPE_NODES));
    debug_assert!(encoded_len(recipe.entry.len()) <= u64::from(MAX_RECIPE_NODES));
    counts.recipe_bytes = counts.recipe_bytes.saturating_add(8);
    for node in &recipe.nodes {
        counts.recipe_bytes = counts.recipe_bytes.saturating_add(recipe_node_bytes(node));
        match node {
            Node::I64(_) | Node::Bool(_) => counts.literals = counts.literals.saturating_add(1),
            Node::Operation { op, .. } => match op {
                Op::Dup | Op::Drop | Op::Swap => counts.routing = counts.routing.saturating_add(1),
                Op::Add | Op::Sub | Op::Mul => {
                    counts.arithmetic = counts.arithmetic.saturating_add(1)
                }
                Op::Eq => counts.boolean_ops = counts.boolean_ops.saturating_add(1),
                Op::Run => counts.runs = counts.runs.saturating_add(1),
            },
            Node::Quotation { .. } => counts.quotations = counts.quotations.saturating_add(1),
            Node::Capture { .. } | Node::Invoke { .. } => {}
        }
    }
    counts.recipe_bytes = counts
        .recipe_bytes
        .saturating_add(encoded_len(recipe.entry.len()).saturating_mul(4));
}

#[derive(Default)]
struct FeatureMetadataMatches {
    body: bool,
    dependencies: bool,
    instantiations: bool,
}

fn feature_metadata_matches(
    library: &Library,
    candidate: &QueueCandidate,
) -> FeatureMetadataMatches {
    debug_assert!(encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    debug_assert!(encoded_len(library.entries().len()) <= u64::from(MAX_LIBRARY_ENTRIES));
    let mut matches = FeatureMetadataMatches {
        body: true,
        dependencies: true,
        instantiations: true,
    };
    let mut position = 0usize;
    let mut is_resolved = true;
    while is_resolved && position < candidate.sequence.len() {
        is_resolved = fold_selected_metadata(library, candidate, position, &mut matches);
        position = position.saturating_add(1);
    }
    matches
}

/// Fold the selected entry at `position` into `matches`; false when it is not retained.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; entry lookups use the non-const slice `get` and recompute runtime identities"
)]
fn fold_selected_metadata(
    library: &Library,
    candidate: &QueueCandidate,
    position: usize,
    matches: &mut FeatureMetadataMatches,
) -> bool {
    match library
        .entries()
        .get(usize::from(candidate.sequence[position]))
    {
        Some(entry) => {
            fold_entry_metadata(library, entry, matches);
            true
        }
        None => {
            matches.body = false;
            matches.dependencies = false;
            matches.instantiations = false;
            false
        }
    }
}

/// Fold one selected entry's recomputed identities and witness shapes into `matches`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; identity lanes invoke canonical source and envelope digest calculations at runtime"
)]
fn fold_entry_metadata(library: &Library, entry: &Entry, matches: &mut FeatureMetadataMatches) {
    let envelope = entry.envelope();
    let is_dependencies_match = dependencies_resolve(library, &envelope.dependencies);
    let is_instantiations_match = instantiations_are_profile(&envelope.instantiations);
    matches.body &= envelope.chunk_id == chunk_identity_of(envelope)
        && envelope.source_revision == canonical_revision(entry.source());
    matches.dependencies &= is_dependencies_match;
    matches.instantiations &= is_instantiations_match;
}

/// Whether every dependency record still resolves in the library snapshot.
fn dependencies_resolve(library: &Library, dependencies: &[DefinitionRef]) -> bool {
    let mut index = 0usize;
    let mut is_resolved = true;
    while is_resolved && index < dependencies.len() {
        is_resolved = library.find(&dependencies[index]).is_some();
        index = index.saturating_add(1);
    }
    is_resolved
}

/// Whether every witness binds strictly increasing value variables to
/// profile types.
fn instantiations_are_profile(instantiations: &[Instantiation]) -> bool {
    let mut index = 0usize;
    let mut is_match = true;
    while is_match && index < instantiations.len() {
        let bindings = &instantiations[index].bindings;
        is_match = bindings_are_ordered(bindings) && bindings_are_profile(bindings);
        index = index.saturating_add(1);
    }
    is_match
}

/// Whether a witness's bindings name strictly increasing variables.
fn bindings_are_ordered(bindings: &[Binding]) -> bool {
    let mut index = 0usize;
    let mut is_ordered = true;
    while is_ordered && index < bindings.len().saturating_sub(1) {
        is_ordered = binding_precedes_next(bindings, index);
        index = index.saturating_add(1);
    }
    is_ordered
}

/// Whether the binding at `index` names a smaller variable than the next binding.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; binding lookups use the slice `get`, which is not a stable const fn"
)]
fn binding_precedes_next(bindings: &[Binding], index: usize) -> bool {
    match (bindings.get(index), bindings.get(index.saturating_add(1))) {
        (Some(left), Some(right)) => left.variable < right.variable,
        _ => false,
    }
}

/// Whether every binding is a value variable bound to a profile type.
fn bindings_are_profile(bindings: &[Binding]) -> bool {
    let mut index = 0usize;
    let mut is_profile = true;
    while is_profile && index < bindings.len() {
        is_profile = binding_is_profile(&bindings[index]);
        index = index.saturating_add(1);
    }
    is_profile
}

/// Whether one binding is a value variable bound to a profile type.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the kind check uses the non-const derived variable-kind equality"
)]
fn binding_is_profile(binding: &Binding) -> bool {
    binding.kind == noble_kernel::words::VariableKind::Value && profile_type(&binding.ty)
}

fn candidate_features(
    search: &Search<'_>,
    candidate: &QueueCandidate,
) -> Result<CandidateFeatureVector, ScorerFailure> {
    let mut features = [f32::from(0_u8); FEATURE_LANES];
    feature_request_lanes(&mut features, search)?;
    let counts = collect_candidate_features(
        &mut features,
        search.library,
        search.request,
        &search.stack_in,
        candidate,
    )?;
    feature_candidate_lanes(&mut features, search.request, candidate, &counts);
    feature_budget_lanes(&mut features, search, candidate);
    feature_identity_lanes(&mut features, search, candidate)?;
    if features_are_finite(&features) {
        Ok(features)
    } else {
        Err(ScorerFailure::InvalidFeature)
    }
}

/// Whether every feature lane is finite.
fn features_are_finite(features: &CandidateFeatureVector) -> bool {
    let mut index = 0usize;
    let mut is_finite = true;
    while is_finite && index < features.len() {
        is_finite = features[index].is_finite();
        index = index.saturating_add(1);
    }
    is_finite
}

fn feature_request_lanes(
    features: &mut CandidateFeatureVector,
    search: &Search<'_>,
) -> Result<(), ScorerFailure> {
    let request = search.request;
    let input_height = encoded_len(request.task.stack_in.len());
    let output_height = encoded_len(request.task.stack_out.len());
    let required_output_suffix_height =
        encoded_len(search.hole_context.required_output_suffix_height);
    let known_effect_count = encoded_len(search.hole_context.known_effect_count);
    debug_assert!(input_height <= u64::from(MAX_STACK_HEIGHT));
    debug_assert!(output_height <= u64::from(MAX_STACK_HEIGHT));
    let stack_max = MAX_STACK_HEIGHT;
    features[0] = feature_ratio(input_height, stack_max);
    features[1] = feature_ratio(output_height, stack_max);
    features[2] = feature_ratio(encoded_len(request.hole_path.len()), MAX_QUOTATION_DEPTH);
    features[3] = feature_ratio(encoded_len(search.flat.hole), MAX_SOURCE_BYTES);
    features[4] = feature_ratio(
        feature_type_count(&request.task.stack_in, Ty::I64),
        stack_max,
    );
    features[5] = feature_ratio(
        feature_type_count(&request.task.stack_in, Ty::Bool),
        stack_max,
    );
    features[6] = feature_ratio(
        feature_type_count(&request.task.stack_out, Ty::I64),
        stack_max,
    );
    features[7] = feature_ratio(
        feature_type_count(&request.task.stack_out, Ty::Bool),
        stack_max,
    );
    features[8] = feature_ratio(encoded_len(search.stack_in.len()), stack_max);
    features[9] = feature_ratio(required_output_suffix_height, stack_max);
    features[10] = feature_ratio(known_effect_count, MAX_DIAGNOSTICS);
    features[11] = feature_bit(search.hole_context.unresolved_effect);
    Ok(())
}

fn collect_candidate_features(
    features: &mut CandidateFeatureVector,
    library: &Library,
    request: &Request,
    stack_in: &[Ty],
    candidate: &QueueCandidate,
) -> Result<CandidateFeatureCounts, ScorerFailure> {
    debug_assert!(encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    debug_assert!(encoded_len(library.entries().len()) <= u64::from(MAX_LIBRARY_ENTRIES));
    let mut counts = CandidateFeatureCounts {
        stack_after: stack_in.to_vec(),
        stack_matches: true,
        effects_match: true,
        ..CandidateFeatureCounts::default()
    };
    let mut chunk_at = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && chunk_at < candidate.sequence.len() {
        outcome = collect_chunk_features(
            features,
            library
                .entries()
                .get(usize::from(candidate.sequence[chunk_at])),
            request,
            chunk_at,
            &mut counts,
        );
        chunk_at = chunk_at.saturating_add(1);
    }
    outcome?;
    Ok(counts)
}

/// Fold one selected chunk into the running counts and its four per-chunk
/// lanes; a chunk that is not retained is an invalid feature.
fn collect_chunk_features(
    features: &mut CandidateFeatureVector,
    entry: Option<&Entry>,
    request: &Request,
    chunk_at: usize,
    counts: &mut CandidateFeatureCounts,
) -> Result<(), ScorerFailure> {
    let Some(entry) = entry else {
        return Err(ScorerFailure::InvalidFeature);
    };
    debug_assert!(encoded_len(chunk_at) < u64::from(MAX_COMPOSED_CHUNKS));
    let envelope = entry.envelope();
    let interface = entry.interface();
    counts.stack_matches &= feature_apply_interface(&mut counts.stack_after, interface);
    counts.effects_match &= interface.effects == request.task.allowed_effects;
    counts.expanded_nodes = counts
        .expanded_nodes
        .saturating_add(u64::from(entry.expanded_nodes()));
    counts.capture_count = counts
        .capture_count
        .saturating_add(encoded_len(envelope.captures.len()));
    counts.dependency_count = counts
        .dependency_count
        .saturating_add(encoded_len(envelope.dependencies.len()));
    counts.instantiation_count = counts
        .instantiation_count
        .saturating_add(encoded_len(envelope.instantiations.len()));
    counts.source_bytes = counts
        .source_bytes
        .saturating_add(encoded_len(entry.source().len()));
    feature_recipe_metrics(&envelope.body, counts);
    let quote_depth =
        u64::from(recipe_quotation_depth(&envelope.body).ok_or(ScorerFailure::InvalidFeature)?);
    if quote_depth > counts.quote_depth {
        counts.quote_depth = quote_depth;
    }
    let base = 32usize.saturating_add(chunk_at.saturating_mul(4));
    debug_assert!(base.saturating_add(3) < FEATURE_LANES);
    let input = &interface.stack_in;
    let output = &interface.stack_out;
    let stack_max = MAX_STACK_HEIGHT;
    features[base] = feature_ratio(encoded_len(input.len()), stack_max);
    features[base.saturating_add(1)] = feature_ratio(encoded_len(output.len()), stack_max);
    features[base.saturating_add(2)] = feature_ratio(
        feature_type_count(input, Ty::I64).saturating_add(feature_type_count(output, Ty::I64)),
        stack_max,
    );
    features[base.saturating_add(3)] = feature_ratio(
        feature_type_count(input, Ty::Bool).saturating_add(feature_type_count(output, Ty::Bool)),
        stack_max,
    );
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; exact typed stack equality and request fields are checked at runtime"
)]
fn feature_candidate_lanes(
    features: &mut CandidateFeatureVector,
    request: &Request,
    candidate: &QueueCandidate,
    counts: &CandidateFeatureCounts,
) {
    debug_assert!(encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    // A sequence can overflow the legal stack before re-admission refuses it.
    // The feature scratch still has a bound: the initial stack plus four
    // admitted chunk outputs, each at most one profile stack high.
    debug_assert!(
        encoded_len(counts.stack_after.len())
            <= u64::from(MAX_STACK_HEIGHT)
                .saturating_mul(u64::from(MAX_COMPOSED_CHUNKS).saturating_add(1))
    );
    let current_stack_height = encoded_len(counts.stack_after.len());
    let output_height = encoded_len(request.task.stack_out.len());
    let stack_max = MAX_STACK_HEIGHT;
    features[12] = feature_ratio(encoded_len(candidate.sequence.len()), MAX_COMPOSED_CHUNKS);
    features[13] = feature_ratio(u64::from(candidate.ordinal), MAX_CANDIDATES);
    features[14] = feature_ratio(current_stack_height, stack_max);
    features[15] = feature_ratio(
        output_height.saturating_sub(current_stack_height),
        stack_max,
    );
    features[16] = feature_ratio(encoded_len(candidate.sequence.len()), MAX_COMPOSED_CHUNKS);
    features[17] = feature_ratio(counts.expanded_nodes, MAX_EXPANDED_NODES);
    features[18] = feature_ratio(counts.quote_depth, MAX_QUOTATION_DEPTH);
    features[19] = feature_ratio(counts.capture_count, MAX_CAPTURES.saturating_mul(4));
    features[20] = feature_ratio(counts.dependency_count, MAX_DEPENDENCY_STEPS);
    features[21] = feature_ratio(counts.instantiation_count, MAX_TOTAL_INSTANTIATION_BINDINGS);
    features[22] = feature_ratio(counts.source_bytes, MAX_SOURCE_BYTES);
    features[23] = feature_ratio(counts.recipe_bytes, MAX_LIBRARY_BYTES);
    let max_recipe_nodes = MAX_RECIPE_NODES.saturating_mul(4);
    features[24] = feature_ratio(counts.literals, max_recipe_nodes);
    features[25] = feature_ratio(counts.routing, max_recipe_nodes);
    features[26] = feature_ratio(counts.arithmetic, max_recipe_nodes);
    features[27] = feature_ratio(counts.boolean_ops, max_recipe_nodes);
    features[28] = feature_ratio(counts.quotations, max_recipe_nodes);
    features[29] = feature_ratio(counts.runs, max_recipe_nodes);
    features[30] =
        feature_bit(counts.stack_matches && counts.stack_after == request.task.stack_out);
    features[31] = feature_bit(counts.effects_match);
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; feature extraction reads the request's runtime shared budget ledger"
)]
fn feature_budget_lanes(
    features: &mut CandidateFeatureVector,
    search: &Search<'_>,
    candidate: &QueueCandidate,
) {
    let limits = search.ledger.limits();
    debug_assert!(limits.candidates <= MAX_CANDIDATES);
    debug_assert!(candidate.ordinal < MAX_CANDIDATES);
    let remaining_candidates = limits
        .candidates
        .saturating_sub(search.ledger.observed(Resource::Candidates))
        .saturating_sub(candidate.ordinal);
    features[48] = feature_ratio(u64::from(remaining_candidates), MAX_CANDIDATES);
    features[49] = feature_ratio(
        u64::from(search.ledger.remaining(Resource::ExpandedNodes)),
        MAX_EXPANDED_NODES,
    );
    features[50] = feature_ratio(
        u64::from(search.ledger.remaining(Resource::CheckingWork)),
        MAX_CHECKING_WORK,
    );
    features[51] = feature_ratio(
        u64::from(search.ledger.remaining(Resource::SourceBytes)),
        MAX_SOURCE_BYTES,
    );
    features[52] = feature_ratio(
        u64::from(search.ledger.remaining(Resource::EvaluationSteps)),
        MAX_EVALUATION_STEPS,
    );
    features[53] = feature_ratio(
        u64::from(search.ledger.remaining(Resource::Diagnostics)),
        MAX_DIAGNOSTICS,
    );
    features[54] = feature_ratio(
        u64::from(search.ledger.remaining(Resource::RetainedBytes)),
        MAX_RETAINED_BYTES,
    );
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; identity lanes invoke canonical source and envelope digest calculations at runtime"
)]
fn feature_identity_lanes(
    features: &mut CandidateFeatureVector,
    search: &Search<'_>,
    candidate: &QueueCandidate,
) -> Result<(), ScorerFailure> {
    features[55] = feature_bit(search.request.library_revision == *search.library.revision());
    features[56] = feature_bit(search.request.profile == PROFILE);
    features[57] = feature_bit(search.request.format == ENVELOPE_FORMAT);
    features[58] = feature_bit(search.request.semantic_revision == SEMANTIC_REVISION);
    let metadata_matches = feature_metadata_matches(search.library, candidate);
    if !metadata_matches.body || !metadata_matches.dependencies || !metadata_matches.instantiations
    {
        return Err(ScorerFailure::InvalidFeature);
    }
    features[59] = feature_bit(metadata_matches.body);
    features[60] = feature_bit(metadata_matches.dependencies);
    features[61] = feature_bit(metadata_matches.instantiations);
    features[62] = f32::from(1_u8);
    features[63] = f32::from(0_u8);
    Ok(())
}

const DOMAIN_RANKING_INTERFACE: u64 = 0x7463_7261_6e6b_0001;
const DOMAIN_RANKING_GOAL: u64 = 0x7463_7261_6e6b_0002;
const DOMAIN_RANKING_CANDIDATE: u64 = 0x7463_7261_6e6b_0003;
const DOMAIN_RANKING_CANDIDATES: u64 = 0x7463_7261_6e6b_0004;
const DOMAIN_RANKING_POLICY: u64 = 0x7463_7261_6e6b_0005;
const DOMAIN_RANKING_EDITOR: u64 = 0x7463_7261_6e6b_0006;
const DOMAIN_RANKING_REQUEST: u64 = 0x7463_7261_6e6b_0007;

#[derive(Default)]
struct IdentityBytes(alloc::vec::Vec<u8>);

impl IdentityBytes {
    fn with_capacity(capacity_bytes: usize) -> Self {
        Self(alloc::vec::Vec::with_capacity(capacity_bytes))
    }

    fn push(&mut self, byte: u8) {
        self.0.push(byte);
    }

    fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the digest fold mutates runtime identity state"
)]
fn ranking_digest(domain: u64, input: &[u8]) -> [u8; REVISION_BYTES] {
    let mut digest = [0u8; REVISION_BYTES];
    absorb_bytes_lanes(domain, input, REVISION_BYTES / 8, &mut digest);
    digest
}

fn identity_push_count(bytes: &mut IdentityBytes, count: usize) {
    bytes.extend_from_slice(&encoded_len(count).to_le_bytes());
}

/// Append the counted hole path, one little-endian part at a time.
fn identity_push_hole_path(bytes: &mut IdentityBytes, hole_path: &[u32]) {
    identity_push_count(bytes, hole_path.len());
    for part in hole_path {
        bytes.extend_from_slice(&part.to_le_bytes());
    }
}

fn identity_push_type(bytes: &mut IdentityBytes, ty: &Ty) {
    bytes.push(match ty {
        Ty::I64 => 1,
        Ty::Bool => 2,
        _ => 255,
    });
}

fn identity_push_stack(bytes: &mut IdentityBytes, stack: &[Ty]) {
    identity_push_count(bytes, stack.len());
    for ty in stack {
        identity_push_type(bytes, ty);
    }
}

fn identity_push_budget(bytes: &mut IdentityBytes, budget: Budget) {
    for limit in [
        budget.library_bytes,
        budget.entries,
        budget.recipe_nodes,
        budget.stack_height,
        budget.dependency_steps,
        budget.instantiation_bindings,
        budget.quotation_depth,
        budget.expanded_nodes,
        budget.composed_chunks,
        budget.source_bytes,
        budget.candidates,
        budget.diagnostics,
        budget.checking_work,
        budget.evaluation_steps,
        budget.retained_bytes,
    ] {
        bytes.extend_from_slice(&limit.to_le_bytes());
    }
}

fn identity_push_editor_nodes(bytes: &mut IdentityBytes, nodes: &[super::editor::Node]) {
    identity_push_count(bytes, nodes.len());
    let mut walk = IdentityWalk {
        frames: alloc::vec::Vec::with_capacity(1),
    };
    walk.frames.push(IdentityFrame {
        path: alloc::vec::Vec::new(),
        next: 0,
    });
    let mut is_walking = true;
    while is_walking && !walk.frames.is_empty() {
        debug_assert!(u64::try_from(walk.frames.len())
            .is_ok_and(|depth| depth <= u64::from(MAX_QUOTATION_DEPTH).saturating_add(1)));
        is_walking = identity_editor_step(bytes, nodes, &mut walk);
    }
}

/// One editor-identity walk frame: the quotation positions from the root
/// node list (so the frame holds no borrow) and the next node.
struct IdentityFrame {
    path: alloc::vec::Vec<usize>,
    next: usize,
}

/// The frame stack of the editor-identity walk.
struct IdentityWalk {
    frames: alloc::vec::Vec<IdentityFrame>,
}

/// One editor-identity step: close the finished top frame or encode its
/// next node. False stops the walk after a lost node.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the identity walk mutates its runtime frame Vec and appends identity bytes"
)]
fn identity_editor_step(
    bytes: &mut IdentityBytes,
    root: &[super::editor::Node],
    walk: &mut IdentityWalk,
) -> bool {
    debug_assert!(!walk.frames.is_empty());
    let Some(top) = walk.frames.len().checked_sub(1) else {
        return false;
    };
    let Some(frame) = walk.frames.get(top) else {
        return false;
    };
    let next = frame.next;
    let Some(nodes) = editor_nodes_at(root, &frame.path) else {
        bytes.push(255);
        return false;
    };
    debug_assert!(next <= nodes.len());
    if next == nodes.len() {
        bytes.push(0);
        let _ = walk.frames.pop();
        return true;
    }
    let Some(node) = nodes.get(next) else {
        bytes.push(255);
        return false;
    };
    if let Some(frame) = walk.frames.get_mut(top) {
        frame.next = next.saturating_add(1);
    }
    match node {
        super::editor::Node::Integer(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        super::editor::Node::Boolean(value) => {
            bytes.push(2);
            bytes.push(u8::from(*value));
        }
        super::editor::Node::Word(word) => {
            bytes.push(3);
            identity_push_count(bytes, word.len());
            bytes.extend_from_slice(word.as_bytes());
        }
        super::editor::Node::Quotation(children) => {
            bytes.push(4);
            identity_push_count(bytes, children.len());
            if let Some(frame) = walk.frames.get(top) {
                let mut path = frame.path.clone();
                path.push(next);
                walk.frames.push(IdentityFrame { path, next: 0 });
            }
        }
        super::editor::Node::Hole => bytes.push(5),
    }
    true
}

fn task_interface_identity(request: &Request) -> [u8; REVISION_BYTES] {
    let mut bytes = IdentityBytes::default();
    identity_push_stack(&mut bytes, &request.task.stack_in);
    identity_push_stack(&mut bytes, &request.task.stack_out);
    identity_push_count(&mut bytes, request.task.allowed_effects.as_slice().len());
    for effect in request.task.allowed_effects.as_slice() {
        bytes.extend_from_slice(&effect.0.to_le_bytes());
    }
    ranking_digest(DOMAIN_RANKING_INTERFACE, bytes.as_slice())
}

fn independent_goal_identity(request: &Request) -> [u8; REVISION_BYTES] {
    let mut bytes = IdentityBytes::default();
    if let Some(fixtures) = &request.fixtures {
        bytes.push(1);
        bytes.extend_from_slice(&fixtures.id);
        identity_push_count(&mut bytes, fixtures.cases.len());
        for case in &fixtures.cases {
            identity_push_count(&mut bytes, case.inputs.len());
            for value in &case.inputs {
                identity_push_scalar(&mut bytes, *value);
            }
            identity_push_count(&mut bytes, case.outputs.len());
            for value in &case.outputs {
                identity_push_scalar(&mut bytes, *value);
            }
        }
    } else {
        bytes.push(0);
    }
    debug_assert!(!bytes.as_slice().is_empty());
    debug_assert_eq!(
        bytes.as_slice().first().copied(),
        Some(u8::from(request.fixtures.is_some()))
    );
    ranking_digest(DOMAIN_RANKING_GOAL, bytes.as_slice())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; scalar identity appends runtime bytes to its bounded digest input"
)]
fn identity_push_scalar(bytes: &mut IdentityBytes, value: Scalar) {
    match value {
        Scalar::I64(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        Scalar::Bool(value) => {
            bytes.push(2);
            bytes.push(u8::from(value));
        }
    }
}

fn candidate_identity(library: &Library, ordinal: u32, sequence: &[u16]) -> CandidateId {
    debug_assert!(ordinal <= MAX_CANDIDATES);
    debug_assert!(encoded_len(sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    let mut bytes = IdentityBytes::with_capacity(
        REVISION_BYTES
            .saturating_add(core::mem::size_of::<u32>())
            .saturating_add(core::mem::size_of::<u64>())
            .saturating_add(
                sequence.len().saturating_mul(
                    core::mem::size_of::<u16>()
                        .saturating_add(1)
                        .saturating_add(IDENTITY_BYTES)
                        .saturating_add(REVISION_BYTES),
                ),
            ),
    );
    bytes.extend_from_slice(library.revision());
    bytes.extend_from_slice(&ordinal.to_le_bytes());
    identity_push_count(&mut bytes, sequence.len());
    for chunk_index in sequence {
        bytes.extend_from_slice(&chunk_index.to_le_bytes());
        match library.entries().get(usize::from(*chunk_index)) {
            Some(entry) => {
                bytes.push(1);
                bytes.extend_from_slice(&entry.envelope().chunk_id);
                bytes.extend_from_slice(&entry.envelope().source_revision);
            }
            None => bytes.push(0),
        }
    }
    CandidateId {
        ordinal,
        digest: ranking_digest(DOMAIN_RANKING_CANDIDATE, bytes.as_slice()),
    }
}

fn candidate_set_identity(library: &Library, queue: &CandidateQueue) -> [u8; REVISION_BYTES] {
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    debug_assert!(queue.next_candidate.as_ref().is_none_or(|candidate| {
        candidate.ordinal <= MAX_CANDIDATES
            && encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS)
    }));
    let mut bytes = IdentityBytes::with_capacity(
        REVISION_BYTES
            .saturating_add(core::mem::size_of::<u64>())
            .saturating_add(1)
            .saturating_add(
                queue
                    .candidates
                    .len()
                    .saturating_add(usize::from(queue.next_candidate.is_some()))
                    .saturating_mul(REVISION_BYTES),
            ),
    );
    bytes.extend_from_slice(library.revision());
    identity_push_count(&mut bytes, queue.candidates.len());
    for candidate in &queue.candidates {
        bytes.extend_from_slice(
            &candidate_identity(library, candidate.ordinal, &candidate.sequence).digest,
        );
    }
    match &queue.next_candidate {
        Some(candidate) => {
            bytes.push(1);
            bytes.extend_from_slice(
                &candidate_identity(library, candidate.ordinal, &candidate.sequence).digest,
            );
        }
        None => bytes.push(0),
    }
    ranking_digest(DOMAIN_RANKING_CANDIDATES, bytes.as_slice())
}

fn ranking_identity(
    request: &Request,
    library: &Library,
    queue: &CandidateQueue,
    model_identity: Option<[u8; REVISION_BYTES]>,
) -> RankingIdentity {
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    // The caller's declared budget may exceed the profile; the ledger clamps it.
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(request.budget.candidates));
    let mut editor_bytes = IdentityBytes::default();
    editor_bytes.extend_from_slice(&request.editor.format.to_le_bytes());
    identity_push_hole_path(&mut editor_bytes, &request.hole_path);
    identity_push_editor_nodes(&mut editor_bytes, &request.editor.nodes);
    let editor_request = ranking_digest(DOMAIN_RANKING_EDITOR, editor_bytes.as_slice());
    let task_interface = task_interface_identity(request);
    let independent_goal = independent_goal_identity(request);
    let candidate_set = candidate_set_identity(library, queue);
    let mut policy_bytes = IdentityBytes::default();
    policy_bytes.extend_from_slice(&request.format.to_le_bytes());
    policy_bytes.extend_from_slice(&request.semantic_revision.to_le_bytes());
    identity_push_count(&mut policy_bytes, request.profile.len());
    policy_bytes.extend_from_slice(request.profile.as_bytes());
    identity_push_budget(&mut policy_bytes, request.budget);
    let policy = ranking_digest(DOMAIN_RANKING_POLICY, policy_bytes.as_slice());
    let mut request_bytes = IdentityBytes::default();
    request_bytes.extend_from_slice(&task_interface);
    request_bytes.extend_from_slice(&independent_goal);
    request_bytes.extend_from_slice(&candidate_set);
    request_bytes.extend_from_slice(library.revision());
    identity_push_feature_revision(&mut request_bytes);
    request_bytes.extend_from_slice(&request.semantic_revision.to_le_bytes());
    request_bytes.extend_from_slice(&policy);
    request_bytes.extend_from_slice(&editor_request);
    identity_push_budget(&mut request_bytes, request.budget);
    identity_push_model(&mut request_bytes, model_identity);
    RankingIdentity {
        request: ranking_digest(DOMAIN_RANKING_REQUEST, request_bytes.as_slice()),
        task_interface,
        independent_goal,
        candidate_set,
        library_revision: *library.revision(),
        feature_revision: feature_revision_string(),
        semantic_revision: request.semantic_revision,
        policy,
        editor_request,
        model_identity,
        budget: request.budget,
    }
}

/// Append the counted feature-layout revision spelling.
fn identity_push_feature_revision(bytes: &mut IdentityBytes) {
    identity_push_count(bytes, FEATURE_REVISION.len());
    bytes.extend_from_slice(FEATURE_REVISION.as_bytes());
}

/// Append the frozen model identity behind a presence byte.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the model identity is appended to the runtime identity byte buffer"
)]
fn identity_push_model(bytes: &mut IdentityBytes, model_identity: Option<[u8; REVISION_BYTES]>) {
    match model_identity {
        Some(identity) => {
            bytes.push(1);
            bytes.extend_from_slice(&identity);
        }
        None => bytes.push(0),
    }
}

/// The owned feature-layout revision recorded in a ranking identity.
fn feature_revision_string() -> alloc::string::String {
    alloc::string::String::from(FEATURE_REVISION)
}

fn candidate_sequences_are_bounded(queue: &CandidateQueue) -> bool {
    let mut index = 0usize;
    while index < queue.candidates.len() {
        let Some(candidate) = queue.candidates.get(index) else {
            return false;
        };
        if encoded_len(candidate.sequence.len()) > u64::from(MAX_COMPOSED_CHUNKS) {
            return false;
        }
        index = index.saturating_add(1);
    }
    true
}

fn run_candidate_queue(
    search: &mut Search<'_>,
    queue: &CandidateQueue,
    capture: &mut CandidateCapture<'_, '_>,
) -> Result<(), alloc::boxed::Box<Response>> {
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    debug_assert!(candidate_sequences_are_bounded(queue));
    if let Some(refusal) = ranked_identity_refusal(
        capture.identity.as_ref(),
        capture.observer.is_some(),
        search.library,
    ) {
        return Err(alloc::boxed::Box::new(response_of_search(refusal, search)));
    }
    run_ranked_candidates(search, queue, capture)?;
    if let Some(candidate) = &queue.next_candidate {
        return run_overflow_candidate(search, candidate, capture);
    }
    Ok(())
}

/// Run the queued candidates in ranked order until one stops the search.
fn run_ranked_candidates(
    search: &mut Search<'_>,
    queue: &CandidateQueue,
    capture: &mut CandidateCapture<'_, '_>,
) -> Result<(), alloc::boxed::Box<Response>> {
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < queue.candidates.len() {
        outcome = run_ranked_candidate(search, &queue.candidates[index], capture);
        index = index.saturating_add(1);
    }
    outcome
}

/// The refusal for a ranked queue that carries no immutable identity.
fn ranked_identity_refusal(
    identity: Option<&RankingIdentity>,
    is_observed: bool,
    library: &Library,
) -> Option<Refusal> {
    let Some(identity) = identity else {
        return Some(internal(
            Location::field("ranking.identity"),
            "a ranked queue has no immutable identity",
        ));
    };
    debug_assert_eq!(identity.library_revision, *library.revision());
    debug_assert!(!is_observed || identity.model_identity.is_some());
    None
}

fn run_overflow_candidate(
    search: &mut Search<'_>,
    candidate: &QueueCandidate,
    capture: &mut CandidateCapture<'_, '_>,
) -> Result<(), alloc::boxed::Box<Response>> {
    debug_assert!(candidate.ordinal <= MAX_CANDIDATES);
    debug_assert!(encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    let attempt = CandidateAttempt {
        ordinal: candidate.ordinal,
        sequence: &candidate.sequence,
        identity: candidate.identity,
        feature_extraction_ns: candidate.feature_extraction_ns,
        scorer_elapsed_ns: candidate.scorer_elapsed_ns,
    };
    let binding = match candidate_binding(
        search,
        &attempt,
        capture.observer.is_some(),
        capture.identity.as_ref(),
    ) {
        Ok(binding) => binding,
        Err(refusal) => return Err(alloc::boxed::Box::new(response_of_search(refusal, search))),
    };
    let resources_before = if capture.observer.is_some() {
        Some(resource_snapshot(&search.ledger))
    } else {
        None
    };
    let started_ns = if resources_before.is_some() {
        capture.clock.now_ns()
    } else {
        0
    };
    let result = search.candidate(attempt.sequence);
    let cost = match resources_before {
        Some(resources_before) => candidate_attempt_cost(
            &search.ledger,
            CandidateAttemptMeasurement {
                resources_before,
                started_ns,
                feature_extraction_ns: attempt.feature_extraction_ns,
                scorer_elapsed_ns: attempt.scorer_elapsed_ns,
                clock: capture.clock,
            },
        ),
        None => CandidateAttemptCost::default(),
    };
    match result {
        Err(refusal) => {
            observe_refusal(
                &CandidateAttemptOutcome {
                    ordinal: attempt.ordinal,
                    sequence: attempt.sequence,
                    binding,
                    cost,
                },
                &refusal,
                capture,
            );
            Err(alloc::boxed::Box::new(response_of_search(refusal, search)))
        }
        Ok(_) => {
            let refusal = internal(
                Location::field("request.budget.candidates"),
                "the bounded queue overflow was not refused by the shared candidate budget",
            );
            Err(alloc::boxed::Box::new(response_of_search(refusal, search)))
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; ranked candidate execution mutates the request ledger and invokes the caller's runtime observer"
)]
fn run_ranked_candidate(
    search: &mut Search<'_>,
    candidate: &QueueCandidate,
    capture: &mut CandidateCapture<'_, '_>,
) -> Result<(), alloc::boxed::Box<Response>> {
    debug_assert!(candidate.ordinal < MAX_CANDIDATES);
    debug_assert!(encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    run_candidate_attempt(
        search,
        CandidateAttempt {
            ordinal: candidate.ordinal,
            sequence: &candidate.sequence,
            identity: candidate.identity,
            feature_extraction_ns: candidate.feature_extraction_ns,
            scorer_elapsed_ns: candidate.scorer_elapsed_ns,
        },
        capture,
    )
}

/// Run one bounded one-hole completion request.
///
/// The search is deterministic shortlex over the canonical library order. It
/// never commits, never publishes and never treats a signature, a digest or a
/// fixture result as acceptance authority.
///
/// Reviewed extraction exclusion (#97): the signature names a host `fn`
/// clock; the checked logic is `complete_fixed`.
#[charon::exclude]
pub fn complete(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    monotonic_now_ns: fn() -> u64,
) -> Response {
    let mut clock = MonotonicClock(monotonic_now_ns);
    complete_fixed(
        namespace,
        library,
        request,
        CompletionControl {
            ranker: None,
            observer: None,
            stored: None,
            clock: &mut clock,
        },
    )
}

/// Run the deterministic shortlex completion while streaming attempted
/// candidate outcomes to a caller-owned observer.
///
/// Reviewed extraction exclusion (#97): the signature names a `dyn`
/// observer and a host `fn` clock; the checked logic is `complete_fixed`.
#[charon::exclude]
pub fn complete_observed(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    observer: &mut CandidateOutcomeObserver<'_>,
    monotonic_now_ns: fn() -> u64,
) -> Response {
    let mut clock = MonotonicClock(monotonic_now_ns);
    complete_fixed(
        namespace,
        library,
        request,
        CompletionControl {
            ranker: None,
            observer: Some(ObserverHandle(observer)),
            stored: None,
            clock: &mut clock,
        },
    )
}

/// Run completion with a bounded untrusted scorer that can only reorder the
/// canonical finite queue. Every selected candidate follows the ordinary
/// #73 re-admission path; scorer failures use fixed shortlex.
///
/// Reviewed extraction exclusion (#97): the signature names a host `fn`
/// clock; the checked logic is `complete_scored`.
#[charon::exclude]
pub fn complete_ranked(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    ranker: &mut RankerSession<'_>,
    monotonic_now_ns: fn() -> u64,
) -> RankedResponse {
    let mut clock = MonotonicClock(monotonic_now_ns);
    complete_scored(
        namespace,
        library,
        request,
        CompletionControl {
            ranker: Some(ranker.reborrow()),
            observer: None,
            stored: None,
            clock: &mut clock,
        },
    )
}

/// Ranked completion with the same candidate-outcome stream as
/// [`complete_observed`].
///
/// Reviewed extraction exclusion (#97): the signature names a `dyn`
/// observer and a host `fn` clock; the checked logic is `complete_scored`.
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; this one-shot public API explicitly binds namespace, immutable library/request, scorer, consumer observer and monotonic clock trust boundaries"
)]
#[charon::exclude]
pub fn complete_ranked_observed(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    ranker: &mut RankerSession<'_>,
    observer: &mut CandidateOutcomeObserver<'_>,
    monotonic_now_ns: fn() -> u64,
) -> RankedResponse {
    let mut clock = MonotonicClock(monotonic_now_ns);
    complete_scored(
        namespace,
        library,
        request,
        CompletionControl {
            ranker: Some(ranker.reborrow()),
            observer: Some(ObserverHandle(observer)),
            stored: None,
            clock: &mut clock,
        },
    )
}

/// Replay a stored candidate-ID ordering without invoking a model. A stale
/// request or malformed candidate set falls back to the canonical shortlex
/// queue; every attempted proposal still uses ordinary #73 re-admission.
///
/// Reviewed extraction exclusion (#97): the signature names a `dyn`
/// observer and a host `fn` clock; the checked logic is `complete_stored`.
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; this replay entry point binds the request, immutable library, persisted ranking, current model identity, optional consumer observer and monotonic clock"
)]
#[charon::exclude]
pub fn complete_replayed<'observer>(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    stored_ranking: &StoredRanking,
    expected_model_identity: [u8; REVISION_BYTES],
    observer: Option<&'observer mut CandidateOutcomeObserver<'observer>>,
    monotonic_now_ns: fn() -> u64,
) -> RankedResponse {
    let mut clock = MonotonicClock(monotonic_now_ns);
    complete_stored(
        namespace,
        library,
        request,
        CompletionControl {
            ranker: None,
            observer: observer.map(ObserverHandle),
            stored: Some(StoredRankingControl {
                ranking: stored_ranking,
                expected_model_identity,
            }),
            clock: &mut clock,
        },
    )
}

/// Fixed shortlex completion, optionally observed, timed on the host clock.
fn complete_fixed(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    control: CompletionControl<'_, '_, '_, '_>,
) -> Response {
    let mut ranking = RankingStatus::not_evaluated();
    complete_timed(namespace, library, request, control, &mut ranking)
}

/// Scorer-ranked completion, timed on the host clock, with its scorer status.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; completion dispatch initializes runtime search state and reads the host clock"
)]
fn complete_scored(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    control: CompletionControl<'_, '_, '_, '_>,
) -> RankedResponse {
    let mut ranking = RankingStatus::not_evaluated();
    let response = complete_timed(namespace, library, request, control, &mut ranking);
    debug_assert!(ranking.invalid_scores <= MAX_CANDIDATES);
    debug_assert!(
        ranking.disposition != RankingDisposition::Applied || ranking.invalid_scores == 0
    );
    RankedResponse {
        response,
        disposition: ranking.disposition,
        invalid_scores: ranking.invalid_scores,
        identity: ranking.identity,
        ranked_candidates: ranking.ranked_candidates,
    }
}

/// Stored-ranking replay completion, timed on the host clock, with its status.
fn complete_stored(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    control: CompletionControl<'_, '_, '_, '_>,
) -> RankedResponse {
    let mut ranking = RankingStatus::not_evaluated();
    let response = complete_timed(namespace, library, request, control, &mut ranking);
    RankedResponse {
        response,
        disposition: ranking.disposition,
        invalid_scores: ranking.invalid_scores,
        identity: ranking.identity,
        ranked_candidates: ranking.ranked_candidates,
    }
}

/// Run one completion, reading the host clock before and after it to record
/// the total latency.
fn complete_timed(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    control: CompletionControl<'_, '_, '_, '_>,
    ranking: &mut RankingStatus,
) -> Response {
    let CompletionControl {
        ranker,
        observer,
        stored,
        clock,
    } = control;
    let started = clock.now_ns();
    let mut response = complete_inner(
        namespace,
        library,
        request,
        CompletionControl {
            ranker,
            observer,
            stored,
            clock: &mut *clock,
        },
        ranking,
    );
    let finished = clock.now_ns();
    let total_latency_ns = finished.saturating_sub(started);
    debug_assert_eq!(
        started.saturating_add(total_latency_ns),
        finished.max(started)
    );
    response.report_mut().total_latency_ns = total_latency_ns;
    debug_assert_eq!(response.report_mut().total_latency_ns, total_latency_ns);
    response
}

fn charge_ranking_queue(search: &mut Search<'_>, queue: &CandidateQueue) -> Result<(), Refusal> {
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    debug_assert!(queue.next_candidate.as_ref().is_none_or(|candidate| {
        candidate.ordinal <= MAX_CANDIDATES
            && encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS)
    }));
    let bounded_candidate_count = queue
        .candidates
        .len()
        .saturating_add(usize::from(queue.next_candidate.is_some()));
    let queue_work_count = u32::try_from(bounded_candidate_count)
        .map_err(|_| {
            internal(
                Location::field("ranking.queue"),
                "candidate queue exceeds profile",
            )
        })?
        .saturating_mul(RANKING_QUEUE_WORK);
    search.ledger.charge(
        Resource::CheckingWork,
        RANKING_CONTEXT_WORK,
        Location::field("ranking.identity"),
    )?;
    search.ledger.charge(
        Resource::CheckingWork,
        queue_work_count,
        Location::field("ranking.queue"),
    )
}

fn ranked_candidate_ids(queue: &CandidateQueue, library: &Library) -> alloc::vec::Vec<CandidateId> {
    let mut identities = alloc::vec::Vec::with_capacity(queue.candidates.len());
    let mut index = 0usize;
    while index < queue.candidates.len() {
        if let Some(identity) = ranked_candidate_id_at(queue, library, index) {
            identities.push(identity);
        }
        index = index.saturating_add(1);
    }
    identities
}

/// The identity of the queued candidate at `index`, recomputed when not yet bound.
fn ranked_candidate_id_at(
    queue: &CandidateQueue,
    library: &Library,
    index: usize,
) -> Option<CandidateId> {
    let candidate = queue.candidates.get(index)?;
    Some(match candidate.identity {
        Some(identity) => identity,
        None => candidate_identity(library, candidate.ordinal, &candidate.sequence),
    })
}

fn replay_candidate_queue(
    search: &mut Search<'_>,
    stored: &StoredRankingControl<'_>,
) -> Result<(CandidateQueue, RankingDisposition, RankingIdentity), Refusal> {
    let candidate_limit = CandidateLimit::from(search.ledger.limits().candidates);
    let mut queue = candidate_queue(search.library, candidate_limit);
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    debug_assert!(candidate_sequences_are_bounded(&queue));
    let identity = ranking_identity(
        search.request,
        search.library,
        &queue,
        Some(stored.expected_model_identity),
    );
    charge_ranking_queue(search, &queue)?;
    let bounded_candidate_count = queue
        .candidates
        .len()
        .saturating_add(usize::from(queue.next_candidate.is_some()));
    let candidate_id_work = u32::try_from(bounded_candidate_count)
        .map_err(|_| {
            internal(
                Location::field("ranking.queue"),
                "candidate queue exceeds profile",
            )
        })?
        .saturating_mul(RANKING_CANDIDATE_ID_WORK);
    search.ledger.charge(
        Resource::CheckingWork,
        candidate_id_work,
        Location::field("ranking.candidate_identity"),
    )?;
    if identity != stored.ranking.identity {
        return Ok((queue, RankingDisposition::FallbackStaleProvenance, identity));
    }
    if let Err(disposition) =
        validate_and_apply_stored_order(&mut queue, stored.ranking, search.library)
    {
        return Ok((queue, disposition, identity));
    }
    Ok((queue, RankingDisposition::Applied, identity))
}

fn validate_and_apply_stored_order(
    queue: &mut CandidateQueue,
    stored: &StoredRanking,
    library: &Library,
) -> Result<(), RankingDisposition> {
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    debug_assert!(candidate_sequences_are_bounded(queue));
    if stored.ordered_candidates.len() != queue.candidates.len() {
        return Err(RankingDisposition::FallbackInvalidResponse);
    }
    let candidate_count = match u32::try_from(queue.candidates.len()) {
        Ok(count) => count,
        Err(_) => return Err(RankingDisposition::FallbackInvalidResponse),
    };
    if stored.invalid_scores > candidate_count
        || (candidate_count > 0 && stored.invalid_scores == candidate_count)
    {
        return Err(RankingDisposition::FallbackInvalidResponse);
    }
    let mut seen = alloc::vec![false; queue.candidates.len()];
    let mut position = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && position < stored.ordered_candidates.len() {
        outcome = apply_stored_position(queue, stored, library, &mut seen, position);
        position = position.saturating_add(1);
    }
    outcome?;
    if seen.contains(&false) {
        return Err(RankingDisposition::FallbackInvalidResponse);
    }
    queue
        .candidates
        .sort_unstable_by_key(|candidate| candidate.ranking_position);
    Ok(())
}

/// Validate one stored candidate identity and give it its stored position.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; stored positions are looked up and applied through non-const slice accessors"
)]
fn apply_stored_position(
    queue: &mut CandidateQueue,
    stored: &StoredRanking,
    library: &Library,
    seen: &mut [bool],
    position: usize,
) -> Result<(), RankingDisposition> {
    debug_assert!(position < stored.ordered_candidates.len());
    debug_assert_eq!(seen.len(), queue.candidates.len());
    let Some(candidate_id) = stored.ordered_candidates.get(position) else {
        return Err(RankingDisposition::FallbackInvalidResponse);
    };
    let Some(index) = usize::try_from(candidate_id.ordinal).ok() else {
        return Err(RankingDisposition::FallbackInvalidResponse);
    };
    let Some(candidate) = queue.candidates.get_mut(index) else {
        return Err(RankingDisposition::FallbackInvalidResponse);
    };
    let Some(seen_slot) = seen.get_mut(index) else {
        return Err(RankingDisposition::FallbackInvalidResponse);
    };
    let ranking_position = match u32::try_from(position) {
        Ok(position) => position,
        Err(_) => return Err(RankingDisposition::FallbackInvalidResponse),
    };
    if *seen_slot
        || candidate.ordinal != candidate_id.ordinal
        || candidate_identity(library, candidate.ordinal, &candidate.sequence) != *candidate_id
    {
        return Err(RankingDisposition::FallbackInvalidResponse);
    }
    *seen_slot = true;
    candidate.identity = Some(*candidate_id);
    candidate.ranking_position = ranking_position;
    Ok(())
}

fn rank_candidate_queue(
    search: &mut Search<'_>,
    session: &mut RankerSession<'_>,
    should_measure_candidate_time: bool,
    clock: &mut MonotonicClock,
) -> Result<(CandidateQueue, RankingDisposition, u32, RankingIdentity), Refusal> {
    let candidate_limit = CandidateLimit::from(search.ledger.limits().candidates);
    debug_assert!(candidate_limit.0 <= MAX_CANDIDATES);
    let mut queue = candidate_queue(search.library, candidate_limit);
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    let identity = ranking_identity(
        search.request,
        search.library,
        &queue,
        Some(session.expected_model_identity),
    );
    charge_ranking_queue(search, &queue)?;
    if session.ranker.model_identity() != session.expected_model_identity
        || !session.ranker.provenance_matches(&identity)
    {
        return Ok((
            queue,
            RankingDisposition::FallbackStaleProvenance,
            0,
            identity,
        ));
    }
    let (invalid_scores, fallback) = score_candidate_queue(
        search,
        &mut queue,
        session,
        CandidateScoringControl {
            request_identity: identity.request,
            should_measure_candidate_time,
            clock,
        },
    )?;
    if let Some(disposition) = fallback {
        return Ok((queue, disposition, invalid_scores, identity));
    }
    let disposition = sort_ranked_candidates(&mut queue, invalid_scores);
    Ok((queue, disposition, invalid_scores, identity))
}

fn score_candidate_queue(
    search: &mut Search<'_>,
    queue: &mut CandidateQueue,
    session: &mut RankerSession<'_>,
    mut control: CandidateScoringControl<'_>,
) -> Result<(u32, Option<RankingDisposition>), Refusal> {
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    debug_assert!(candidate_sequences_are_bounded(queue));
    let mut invalid_scores = 0u32;
    let mut fallback = None;
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && fallback.is_none() && index < queue.candidates.len() {
        let candidate = &mut queue.candidates[index];
        match score_candidate(search, candidate, session, &mut control) {
            Ok(Ok(score)) => {
                if !score.is_finite() {
                    invalid_scores = invalid_scores.saturating_add(1);
                }
                candidate.score = Some(score);
            }
            Ok(Err(disposition)) => fallback = Some(disposition),
            Err(refusal) => outcome = Err(refusal),
        }
        index = index.saturating_add(1);
    }
    outcome?;
    Ok((invalid_scores, fallback))
}

fn score_candidate(
    search: &mut Search<'_>,
    candidate: &mut QueueCandidate,
    session: &mut RankerSession<'_>,
    control: &mut CandidateScoringControl<'_>,
) -> Result<Result<f32, RankingDisposition>, Refusal> {
    debug_assert!(candidate.ordinal < MAX_CANDIDATES);
    debug_assert!(encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    let candidate_id = candidate_identity(search.library, candidate.ordinal, &candidate.sequence);
    candidate.identity = Some(candidate_id);
    search.ledger.charge(
        Resource::CheckingWork,
        RANKING_CANDIDATE_ID_WORK,
        Location::at("ranking.candidate_identity", candidate.ordinal),
    )?;
    let features = match extract_candidate_features(search, candidate, control)? {
        Ok(features) => features,
        Err(disposition) => return Ok(Err(disposition)),
    };
    search.ledger.charge(
        Resource::CheckingWork,
        RANKING_SCORE_WORK,
        Location::at("ranking.score", candidate.ordinal),
    )?;
    let score_request = CandidateScoreRequest {
        request_identity: control.request_identity,
        model_identity: session.expected_model_identity,
        candidate: candidate_id,
        features: &features,
    };
    let score_started_ns = if control.should_measure_candidate_time {
        control.clock.now_ns()
    } else {
        0
    };
    let score_result = session.ranker.score(&score_request);
    if control.should_measure_candidate_time {
        candidate.scorer_elapsed_ns = control.clock.now_ns().saturating_sub(score_started_ns);
    }
    match score_result {
        Ok(response) => {
            if response.request_identity == control.request_identity
                && response.model_identity == session.expected_model_identity
                && response.candidate == candidate_id
            {
                Ok(Ok(response.score))
            } else if response.model_identity != session.expected_model_identity {
                Ok(Err(RankingDisposition::FallbackStaleProvenance))
            } else {
                Ok(Err(RankingDisposition::FallbackInvalidResponse))
            }
        }
        Err(failure) => {
            let disposition = match failure {
                ScorerFailure::StaleProvenance => RankingDisposition::FallbackStaleProvenance,
                ScorerFailure::InvalidResponse => RankingDisposition::FallbackInvalidResponse,
                ScorerFailure::Unavailable
                | ScorerFailure::InvalidFeature
                | ScorerFailure::Evaluation => RankingDisposition::FallbackScorerFailure,
            };
            Ok(Err(disposition))
        }
    }
}

fn extract_candidate_features(
    search: &mut Search<'_>,
    candidate: &mut QueueCandidate,
    control: &mut CandidateScoringControl<'_>,
) -> Result<Result<CandidateFeatureVector, RankingDisposition>, Refusal> {
    debug_assert!(candidate.ordinal < MAX_CANDIDATES);
    debug_assert!(encoded_len(candidate.sequence.len()) <= u64::from(MAX_COMPOSED_CHUNKS));
    search.ledger.charge(
        Resource::CheckingWork,
        FEATURE_EXTRACTION_WORK,
        Location::at("ranking.features", candidate.ordinal),
    )?;
    let feature_started_ns = if control.should_measure_candidate_time {
        control.clock.now_ns()
    } else {
        0
    };
    let features = candidate_features(search, candidate);
    if control.should_measure_candidate_time {
        candidate.feature_extraction_ns = control.clock.now_ns().saturating_sub(feature_started_ns);
    }
    match features {
        Ok(features) => Ok(Ok(features)),
        Err(ScorerFailure::StaleProvenance) => Ok(Err(RankingDisposition::FallbackStaleProvenance)),
        Err(_) => Ok(Err(RankingDisposition::FallbackScorerFailure)),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; candidate ordering compares the runtime F32 scorer results"
)]
fn sort_ranked_candidates(queue: &mut CandidateQueue, invalid_scores: u32) -> RankingDisposition {
    debug_assert!(encoded_len(queue.candidates.len()) <= u64::from(MAX_CANDIDATES));
    debug_assert!(u32::try_from(queue.candidates.len()).is_ok_and(|count| invalid_scores <= count));
    if !queue.candidates.is_empty()
        && u32::try_from(queue.candidates.len()).is_ok_and(|count| invalid_scores == count)
    {
        return RankingDisposition::FallbackAllInvalidScores;
    }
    let disposition = if invalid_scores == 0 {
        RankingDisposition::Applied
    } else {
        RankingDisposition::AppliedWithInvalidScores
    };
    queue.candidates.sort_unstable_by(|left, right| {
        let left_score = left.score.unwrap_or(f32::NAN);
        let right_score = right.score.unwrap_or(f32::NAN);
        match (left_score.is_finite(), right_score.is_finite()) {
            (true, true) => right_score
                .partial_cmp(&left_score)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then_with(|| left.ordinal.cmp(&right.ordinal)),
            (true, false) => core::cmp::Ordering::Less,
            (false, true) => core::cmp::Ordering::Greater,
            (false, false) => left.ordinal.cmp(&right.ordinal),
        }
    });
    disposition
}

fn initialize_search<'a>(
    namespace: &'a super::Session,
    library: &'a Library,
    request: &'a Request,
) -> Result<Search<'a>, Response> {
    let mut ledger = Ledger::new(request.budget.lowered(&Budget::PROFILE));
    debug_assert!(ledger.remaining(Resource::CheckingWork) <= MAX_CHECKING_WORK);
    let flat = match check_request(request, library, &mut ledger) {
        Ok(flat) => flat,
        Err(refusal) => return Err(response_of(refusal, &ledger)),
    };
    let hole_context = match analyze_editor_context(namespace, request, &mut ledger) {
        Ok(context) => context,
        Err(refusal) => return Err(response_of(refusal, &ledger)),
    };
    let mut prefix = Replay::new(&request.task.stack_in);
    let up_to = match flat.elements.get(..flat.hole) {
        Some(slice) => slice,
        None => {
            return Err(response_of(
                internal(
                    Location::field("request.editor"),
                    "hole index is outside the tree",
                ),
                &ledger,
            ));
        }
    };
    if let Err(refusal) = prefix.run(up_to, None, &mut ledger, false, Location::field("editor")) {
        return Err(response_of(refusal, &ledger));
    }
    let stack_in = prefix.stack.clone();
    debug_assert!(usize::try_from(MAX_STACK_HEIGHT).is_ok_and(|maximum| stack_in.len() <= maximum));
    debug_assert!(flat.hole <= flat.elements.len());
    Ok(Search {
        namespace,
        library,
        request,
        flat,
        hole_context,
        stack_in,
        ledger,
        diagnostics: alloc::vec::Vec::new(),
        rejected: 0,
        checked_candidates: 0,
        fixtures: None,
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; completion dispatch initializes runtime search state and routes the caller's trust-boundary controls"
)]
fn complete_inner(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    control: CompletionControl<'_, '_, '_, '_>,
    ranking: &mut RankingStatus,
) -> Response {
    let search = match initialize_search(namespace, library, request) {
        Ok(search) => search,
        Err(response) => return response,
    };
    debug_assert!(search.ledger.limits().candidates <= MAX_CANDIDATES);
    debug_assert!(control.ranker.is_none() || control.stored.is_none());
    let CompletionControl {
        ranker,
        observer,
        stored,
        clock,
    } = control;
    let capture = CandidateCapture {
        observer,
        identity: None,
        clock,
    };
    if let Some(ranker) = ranker {
        return complete_with_ranker(search, library, ranker, capture, ranking);
    }
    if let Some(stored) = stored {
        return complete_with_stored_ranking(search, library, stored, capture, ranking);
    }
    complete_shortlex(search, library, request, capture)
}

fn complete_with_ranker(
    mut search: Search<'_>,
    library: &Library,
    mut ranker: RankerSession<'_>,
    capture: CandidateCapture<'_, '_>,
    ranking: &mut RankingStatus,
) -> Response {
    debug_assert!(search.ledger.limits().candidates <= MAX_CANDIDATES);
    debug_assert!(capture.identity.is_none());
    let should_measure_candidate_time = capture.observer.is_some();
    let CandidateCapture {
        observer, clock, ..
    } = capture;
    let (queue, status, invalid, identity) = match rank_candidate_queue(
        &mut search,
        &mut ranker,
        should_measure_candidate_time,
        clock,
    ) {
        Ok(ranked) => ranked,
        Err(refusal) => return response_of_search(refusal, &search),
    };
    ranking.disposition = status;
    ranking.invalid_scores = invalid;
    ranking.identity = Some(identity.clone());
    ranking.ranked_candidates = ranked_candidate_ids(&queue, library);
    debug_assert!(ranking.invalid_scores <= MAX_CANDIDATES);
    debug_assert_eq!(identity.library_revision, *search.library.revision());
    let mut ranked_capture = CandidateCapture {
        observer,
        identity: Some(identity),
        clock,
    };
    if let Err(response) = run_candidate_queue(&mut search, &queue, &mut ranked_capture) {
        return *response;
    }
    Response::NoResult {
        report: alloc::boxed::Box::new(search.report(0)),
    }
}

fn complete_with_stored_ranking(
    mut search: Search<'_>,
    library: &Library,
    stored: StoredRankingControl<'_>,
    capture: CandidateCapture<'_, '_>,
    ranking: &mut RankingStatus,
) -> Response {
    debug_assert!(search.ledger.limits().candidates <= MAX_CANDIDATES);
    debug_assert!(capture.identity.is_none());
    let CandidateCapture {
        observer, clock, ..
    } = capture;
    let (queue, disposition, identity) = match replay_candidate_queue(&mut search, &stored) {
        Ok(replayed) => replayed,
        Err(refusal) => return response_of_search(refusal, &search),
    };
    ranking.disposition = disposition;
    if disposition == RankingDisposition::Applied {
        ranking.invalid_scores = stored.ranking.invalid_scores;
    }
    ranking.identity = Some(identity.clone());
    ranking.ranked_candidates = ranked_candidate_ids(&queue, library);
    debug_assert_eq!(identity.library_revision, *search.library.revision());
    debug_assert!(ranking.invalid_scores <= MAX_CANDIDATES);
    let mut replay_capture = CandidateCapture {
        observer,
        identity: Some(identity),
        clock,
    };
    if let Err(response) = run_candidate_queue(&mut search, &queue, &mut replay_capture) {
        return *response;
    }
    Response::NoResult {
        report: alloc::boxed::Box::new(search.report(0)),
    }
}

fn complete_shortlex(
    mut search: Search<'_>,
    library: &Library,
    request: &Request,
    capture: CandidateCapture<'_, '_>,
) -> Response {
    debug_assert!(search.ledger.limits().candidates <= MAX_CANDIDATES);
    debug_assert!(capture.identity.is_none());
    let observation_identity = if capture.observer.is_some() {
        let queue = candidate_queue(
            library,
            CandidateLimit::from(search.ledger.limits().candidates),
        );
        Some(ranking_identity(request, library, &queue, None))
    } else {
        None
    };
    let CandidateCapture {
        observer, clock, ..
    } = capture;
    let mut shortlex_capture = CandidateCapture {
        observer,
        identity: observation_identity,
        clock,
    };
    let order = canonical_order(library);
    debug_assert!(encoded_len(order.len()) <= u64::from(MAX_LIBRARY_ENTRIES));
    let mut chunk_count = 0u32;
    let mut next_ordinal = 0u32;
    let mut outcome = Ok(());
    while outcome.is_ok() && chunk_count <= MAX_COMPOSED_CHUNKS {
        outcome = run_length(
            &mut search,
            &order,
            chunk_count,
            &mut next_ordinal,
            &mut shortlex_capture,
        );
        chunk_count = chunk_count.saturating_add(1);
    }
    if let Err(response) = outcome {
        return *response;
    }
    debug_assert!(chunk_count > MAX_COMPOSED_CHUNKS);
    Response::NoResult {
        report: alloc::boxed::Box::new(search.report(0)),
    }
}
