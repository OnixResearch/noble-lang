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
#![expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every public input is untrusted or host-retained and every invariant is checked with a typed refusal or explicit budget result; assertions would turn malformed requests, stale chunks and resource exhaustion into panics"
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
        match word {
            b"dup" => Some(Op::Dup),
            b"drop" => Some(Op::Drop),
            b"swap" => Some(Op::Swap),
            b"+" => Some(Op::Add),
            b"-" => Some(Op::Sub),
            b"*" => Some(Op::Mul),
            b"=" => Some(Op::Eq),
            b"run" => Some(Op::Run),
            _ => None,
        }
    }
}

/// One retained recipe node. References are indices into the recipe's own
/// node arena and are canonical only when a node refers to an earlier node.
#[octet::sealed_enum]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    I64(i64),
    Bool(bool),
    Capture { index: u16 },
    Operation { op: Op, instantiation: u16 },
    Invoke { dependency: u16, instantiation: u16 },
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
            out.set(resource, profile.limit(resource).min(self.limit(resource)));
        }
        out
    }
}

/// One exact refusal location: a stable field name and an optional ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub field: &'static str,
    pub index: Option<u32>,
}

impl Location {
    pub const fn field(field: &'static str) -> Self {
        Self { field, index: None }
    }
    pub const fn at(field: &'static str, index: u32) -> Self {
        Self {
            field,
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
        let resource_limit = self.limits.limit(resource);
        let mut refusal = Refusal::new(
            Class::Exhausted,
            at,
            "one shared budget counter reached its fixed limit",
        );
        refusal.resource = Some(resource);
        refusal.observed = observed;
        refusal.limit = resource_limit;
        refusal
    }

    fn refuse_unrepresentable_count(&mut self, resource: Resource, at: Location) -> Refusal {
        let max_value = self.limits.limit(resource);
        if let Some(slot) = self.charged.get_mut(resource.slot()) {
            *slot = max_value;
        }
        let mut refusal = self.refuse(resource, u32::MAX, at);
        refusal.message = alloc::string::String::from("budget count exceeds its u32 reporting range");
        refusal
    }

    /// Add `amount` to a cumulative resource and refuse the step that would
    /// exceed the limit. The observed count saturates at the limit so a report
    /// always names the boundary that stopped the work.
    pub fn charge(&mut self, resource: Resource, amount: u32, at: Location) -> Result<(), Refusal> {
        let current = self.observed(resource);
        let resource_limit = self.limits.limit(resource);
        let next = match current.checked_add(amount) {
            Some(next) => next,
            None => return Err(self.refuse(resource, u32::MAX, at)),
        };
        if next > resource_limit {
            if let Some(slot) = self.charged.get_mut(resource.slot()) {
                *slot = resource_limit;
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
        let resource_limit = self.limits.limit(resource);
        if value > resource_limit {
            if let Some(slot) = self.charged.get_mut(resource.slot()) {
                *slot = resource_limit;
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
    fn prepare_limits(&self, work: u32) -> crate::Limits {
        crate::Limits {
            bytes: MAX_SOURCE_BYTES,
            // The recipe expansion counter is charged separately. Bound the
            // whole completed source by its byte ceiling rather than imposing
            // an unreported 256-node cap on editor context around the hole.
            nodes: MAX_SOURCE_BYTES,
            depth: MAX_QUOTATION_DEPTH,
            work: work.min(INNER_WORK_CEILING),
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
fn identity_lane(envelope: &Envelope, lane: u64) -> u64 {
    let mut fold = crate::companion::digest::Fold::new(crate::companion::digest::DOMAIN_CHUNK);
    fold.absorb(lane);
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
        let lane_index = u64::try_from(lane).unwrap_or(u64::MAX);
        let value = identity_lane(envelope, lane_index);
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

/// One expansion frame: a node arena plus the reference list being walked.
struct Segment<'a> {
    envelope: &'a Envelope,
    body: &'a [u32],
    at: usize,
    close: bool,
}

/// Expand one envelope's recipe, inlining every invoked library entry, into the
/// flat element list that is emitted. A dependency edge, a quotation level and
/// an instantiation binding are each charged here; nothing is inferred from a
/// signature.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; one explicit frame loop expands a recipe and its dependencies; splitting it would separate each emitted element from the bound that refused it."
)]
fn expand_recipe(
    entries: &[Entry],
    envelope: &Envelope,
    ledger: &mut Ledger,
    at: Location,
) -> Result<Expansion, Refusal> {
    let mut frames: alloc::vec::Vec<Segment<'_>> =
        alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES);
    frames.push(Segment {
        envelope,
        body: &envelope.body.entry,
        at: 0,
        close: false,
    });
    let mut elements = alloc::vec::Vec::with_capacity(MAX_EXPANDED_NODE_CAPACITY);
    let mut depth = 0u32;
    let mut spent = 0u32;
    while let Some(top) = frames.len().checked_sub(1) {
        let Some(frame) = frames.get_mut(top) else {
            return Err(Refusal::new(
                Class::Internal,
                at,
                "expansion lost its frame",
            ));
        };
        if frame.at >= frame.body.len() {
            let should_close = frame.close;
            let _ = frames.pop();
            if should_close {
                depth = depth.saturating_sub(1);
                ledger.charge(Resource::ExpandedNodes, 1, at)?;
                elements.push(Element::Close);
                spent = spent.saturating_add(1);
            }
            continue;
        }
        let position = frame.at;
        frame.at = frame.at.saturating_add(1);
        let owner = frame.envelope;
        let body = frame.body;
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
        let node_at = Location::at("envelope.body.nodes", reference);
        if !matches!(node, Node::Invoke { .. }) {
            ledger.charge(Resource::ExpandedNodes, 1, at)?;
        }
        match node {
            Node::I64(value) => elements.push(Element::I64(*value)),
            Node::Bool(value) => elements.push(Element::Bool(*value)),
            Node::Capture { index } => {
                let Some(capture) = owner.captures.get(usize::from(*index)) else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "recipe capture index is outside the retained capture list",
                    ));
                };
                match capture.value {
                    Scalar::I64(value) => elements.push(Element::I64(value)),
                    Scalar::Bool(value) => elements.push(Element::Bool(value)),
                }
            }
            Node::Operation { op, instantiation } => {
                charge_instantiation(owner, *instantiation, ledger, at)?;
                let Some(witness) = owner.instantiations.get(usize::from(*instantiation)) else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "operation witness is outside the retained instantiation list",
                    ));
                };
                elements.push(Element::Op {
                    op: *op,
                    witness: Some(witness.clone()),
                });
            }
            Node::Invoke {
                dependency,
                instantiation,
            } => {
                charge_instantiation(owner, *instantiation, ledger, node_at)?;
                check_closed_instantiation(owner, *instantiation, node_at)?;
                ledger.charge(Resource::DependencySteps, 1, at)?;
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
                let Some(entry) = entries.get(usize::from(target)) else {
                    return Err(Refusal::new(
                        Class::Internal,
                        at,
                        "resolved dependency is not retained",
                    ));
                };
                frames.push(Segment {
                    envelope: &entry.envelope,
                    body: &entry.envelope.body.entry,
                    at: 0,
                    close: false,
                });
                continue;
            }
            Node::Quotation {
                body: quoted,
                instantiation,
            } => {
                charge_instantiation(owner, *instantiation, ledger, node_at)?;
                check_closed_instantiation(owner, *instantiation, node_at)?;
                depth = depth.saturating_add(1);
                ledger.observe(Resource::QuotationDepth, depth, at)?;
                elements.push(Element::Open);
                spent = spent.saturating_add(1);
                frames.push(Segment {
                    envelope: owner,
                    body: quoted,
                    at: 0,
                    close: true,
                });
                continue;
            }
        }
        spent = spent.saturating_add(1);
    }
    if !frames.is_empty() || depth != 0 {
        return Err(Refusal::new(
            Class::Internal,
            at,
            "recipe expansion ended mid-quotation",
        ));
    }
    Ok(Expansion {
        elements,
        expanded_nodes: spent,
    })
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
        u64::try_from(instantiation.bindings.len()).unwrap_or(u64::MAX),
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
    start
}

fn emit(elements: &[Element], at: Location) -> Result<alloc::vec::Vec<u8>, Refusal> {
    let mut out = alloc::vec::Vec::with_capacity(elements.len());
    let mut digits = [0u8; 20];
    for element in elements {
        match element {
            Element::I64(value) => {
                let start = emit_i64(*value, &mut digits);
                if let Some(encoded) = digits.get(start..) {
                    out.extend_from_slice(encoded);
                }
            }
            Element::Bool(true) => out.extend_from_slice(b"true"),
            Element::Bool(false) => out.extend_from_slice(b"false"),
            Element::Op { op, .. } => out.extend_from_slice(op.spelling().as_bytes()),
            Element::Open => out.push(b'['),
            Element::Close => out.push(b']'),
            Element::Hole => {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "an editor hole cannot be emitted as source",
                ))
            }
        }
        out.push(b' ');
        if u32::try_from(out.len()).map_or(true, |length| length > MAX_SOURCE_BYTES) {
            return Err(byte_budget_refusal(
                u64::try_from(out.len()).unwrap_or(u64::MAX),
                at,
            ));
        }
    }
    Ok(out)
}

fn byte_budget_refusal(observed: u64, at: Location) -> Refusal {
    let mut refusal = Refusal::new(
        Class::Exhausted,
        at,
        "emitted source exceeds the fixed byte bound",
    );
    refusal.resource = Some(Resource::SourceBytes);
    refusal.observed = u32::try_from(observed).unwrap_or(u32::MAX);
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

/// Lower-bound a completed source's bytes from its editor context, omitting
/// the single hole. No proposal can make this context smaller.
fn editor_context_bytes(elements: &[Element], at: Location) -> Result<u32, Refusal> {
    let mut bytes = 0u64;
    for element in elements {
        let token = match element {
            Element::I64(value) => integer_token_len(*value),
            Element::Bool(true) => 4,
            Element::Bool(false) => 5,
            Element::Op { op, .. } => encoded_len(op.spelling().len()),
            Element::Open | Element::Close => 1,
            Element::Hole => 0,
        };
        if token == 0 {
            continue;
        }
        bytes = bytes.saturating_add(token).saturating_add(1);
        if bytes > u64::from(MAX_SOURCE_BYTES) {
            return Err(byte_budget_refusal(bytes, at));
        }
    }
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

    fn push(&mut self, ty: Ty, ledger: &mut Ledger, at: Location) -> Result<(), Refusal> {
        self.stack.push(ty);
        ledger.observe_count(
            Resource::StackHeight,
            u64::try_from(self.stack.len()).unwrap_or(u64::MAX),
            at,
        )
    }

    fn pop(&mut self, at: Location) -> Result<Ty, Refusal> {
        match self.stack.pop() {
            Some(ty) => Ok(ty),
            None => Err(Refusal::new(
                Class::Invalid,
                at,
                "ordered stack underflow while replaying the profile",
            )),
        }
    }

    fn pop_i64(&mut self, ledger: &mut Ledger, at: Location) -> Result<(), Refusal> {
        let ty = self.pop(at)?;
        if ty != Ty::I64 {
            return Err(Refusal::new(
                Class::Invalid,
                at,
                "the profile operation requires an I64 operand",
            ));
        }
        ledger.observe_count(
            Resource::StackHeight,
            u64::try_from(self.height()).unwrap_or(u64::MAX),
            at,
        )
    }

    fn height(&self) -> usize {
        self.stack.len()
    }

    fn step(
        &mut self,
        element: &Element,
        ledger: &mut Ledger,
        context: Location,
        index: u32,
    ) -> Result<(), Refusal> {
        let at = Location::at(context.field, index);
        ledger.charge(Resource::CheckingWork, 1, at)?;
        match element {
            Element::I64(_) => self.push(Ty::I64, ledger, at),
            Element::Bool(_) => self.push(Ty::Bool, ledger, at),
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
                    at,
                )
            }
            Element::Op { op, witness } => self.operation(*op, witness.as_ref(), ledger, at),
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
        at: Location,
    ) -> Result<(), Refusal> {
        match op {
            Op::Dup => {
                let Some(value) = self.stack.last() else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "ordered stack underflow while replaying the profile",
                    ));
                };
                if !witness_matches(witness, &[(1, value.clone())]) {
                    return Err(invalid_witness(at));
                }
                let value = self.pop(at)?;
                self.push(value.clone(), ledger, at)?;
                self.push(value, ledger, at)
            }
            Op::Drop => {
                let Some(value) = self.stack.last() else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "ordered stack underflow while replaying the profile",
                    ));
                };
                if !witness_matches(witness, &[(1, value.clone())]) {
                    return Err(invalid_witness(at));
                }
                let _ = self.pop(at)?;
                Ok(())
            }
            Op::Swap => {
                let split = self.stack.len().saturating_sub(2);
                let expected = match (self.stack.get(split), self.stack.last()) {
                    (Some(lower), Some(top)) if self.stack.len() >= 2 => {
                        [(1, lower.clone()), (2, top.clone())]
                    }
                    _ => {
                        return Err(Refusal::new(
                            Class::Invalid,
                            at,
                            "ordered stack underflow while replaying the profile",
                        ))
                    }
                };
                if !witness_matches(witness, &expected) {
                    return Err(invalid_witness(at));
                }
                let top = self.pop(at)?;
                let next = self.pop(at)?;
                self.push(top, ledger, at)?;
                self.push(next, ledger, at)
            }
            Op::Add | Op::Sub | Op::Mul => {
                if !witness_matches(witness, &[]) {
                    return Err(invalid_witness(at));
                }
                self.pop_i64(ledger, at)?;
                self.pop_i64(ledger, at)?;
                self.push(Ty::I64, ledger, at)
            }
            Op::Eq => {
                if !witness_matches(witness, &[]) {
                    return Err(invalid_witness(at));
                }
                self.pop_i64(ledger, at)?;
                self.pop_i64(ledger, at)?;
                self.push(Ty::Bool, ledger, at)
            }
            Op::Run => {
                if !witness_matches(witness, &[]) {
                    return Err(invalid_witness(at));
                }
                let value = self.pop(at)?;
                let Ty::Program(required, produced, _) = value else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "run operand is not a quotation value",
                    ));
                };
                if self.stack.as_slice() != required.as_slice() {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "run operand does not match the ordered stack it executes on",
                    ));
                }
                self.stack = produced.as_ref().clone();
                ledger.observe_count(
                    Resource::StackHeight,
                    u64::try_from(self.height()).unwrap_or(u64::MAX),
                    at,
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
        for (index, element) in elements.iter().enumerate() {
            let position = index as u32;
            self.steps = self.steps.saturating_add(1);
            self.step(element, ledger, context, position)?;
            if boundary == Some(index.saturating_add(1)) {
                self.mark = Some(self.stack.clone());
            }
        }
        if !require_closed || self.ambient.is_empty() {
            Ok(())
        } else {
            Err(Refusal::new(
                Class::Invalid,
                Location::field(context.field),
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
    for (binding, (variable, ty)) in witness.bindings.iter().zip(expected) {
        if binding.variable != *variable
            || binding.kind != noble_kernel::words::VariableKind::Value
            || !binding.ty.eq(ty)
        {
            return false;
        }
    }
    true
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
    replay.run(elements, None, ledger, true, at)?;
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
            u64::try_from(self.entries.len()).unwrap_or(u64::MAX),
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
        let envelope_bytes = envelope_size(&envelope, body_at)?;
        ledger.charge(Resource::LibraryBytes, envelope_bytes, body_at)?;
        ledger.charge(Resource::RetainedBytes, envelope_bytes, body_at)?;
        let expansion = expand_recipe(&self.entries, &envelope, &mut ledger, body_at)?;
        replay_interface(
            &expansion.elements,
            &envelope.interface,
            &mut ledger,
            Location::field("envelope.interface"),
        )?;
        let source = emit(&expansion.elements, body_at)?;
        ledger.observe_count(
            Resource::SourceBytes,
            u64::try_from(source.len()).unwrap_or(u64::MAX),
            body_at,
        )?;
        if canonical_revision(&source) != envelope.source_revision {
            return Err(Refusal::new(
                Class::Invalid,
                Location::field("envelope.source_revision"),
                "declared source revision does not match the canonical emitted source",
            ));
        }
        let mut admission = AdmissionContext {
            namespace,
            ledger: &mut ledger,
            at: Location::field("envelope.interface"),
        };
        let (interface, _) = derived_interface(
            &mut admission,
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
        for (index, record) in envelope.dependencies.iter().enumerate() {
            if self.find(record).is_none() {
                let position = index as u32;
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::at("envelope.dependencies", position),
                    "dependency record does not resolve to an earlier retained entry with the exact revisions",
                ));
            }
        }
        Ok(())
    }
}

fn first_eight(bytes: &[u8; IDENTITY_BYTES]) -> [u8; 8] {
    let mut out = [0u8; 8];
    let mut at = 0usize;
    while at < out.len() {
        if let (Some(slot), Some(value)) = (out.get_mut(at), bytes.get(at)) {
            *slot = *value;
        }
        at = at.saturating_add(1);
    }
    out
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
        u64::try_from(envelope.body.nodes.len()).unwrap_or(u64::MAX),
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
    Ok(())
}

fn add_encoded_size(total: &mut u64, amount: u64, at: Location) -> Result<(), Refusal> {
    let next = total.checked_add(amount).ok_or_else(|| {
        let mut refusal = Refusal::new(
            Class::Exhausted,
            at,
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
            at,
            "canonical encoded size exceeds the finite retained-byte bound",
        );
        refusal.resource = Some(Resource::RetainedBytes);
        refusal.observed = match u32::try_from(next) {
            Ok(observed) => observed,
            Err(_) => u32::MAX,
        };
        refusal.limit = MAX_RETAINED_BYTES;
        return Err(refusal);
    }
    *total = next;
    Ok(())
}

fn encoded_len(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// Measure the exact retained envelope shape used by the typed-chunk profile.
/// The stable representation is fixed-width except for explicit vectors and
/// the profile spelling; source bytes are charged separately by `Library`.
fn envelope_size(envelope: &Envelope, at: Location) -> Result<u32, Refusal> {
    let mut encoded_bytes = 0u64;
    add_envelope_metadata_size(&mut encoded_bytes, envelope, at)?;
    add_recipe_size(&mut encoded_bytes, &envelope.body, at)?;
    u32::try_from(encoded_bytes).map_err(|_| {
        internal(
            at,
            "bounded envelope size did not fit the reporting counter",
        )
    })
}

fn add_envelope_metadata_size(
    encoded_bytes: &mut u64,
    envelope: &Envelope,
    at: Location,
) -> Result<(), Refusal> {
    add_encoded_size(
        encoded_bytes,
        12u64.saturating_add(encoded_len(envelope.profile.len())),
        at,
    )?;
    let identity_revision_bytes = IDENTITY_BYTES.saturating_add(REVISION_BYTES);
    let identity_revision_bytes = u64::from(
        u32::try_from(identity_revision_bytes)
            .map_err(|_| internal(at, "fixed identity widths exceed their bounded format"))?,
    );
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
    for capture in &envelope.captures {
        let capture_bytes = match capture.value {
            Scalar::I64(_) => 11,
            Scalar::Bool(_) => 4,
        };
        add_encoded_size(encoded_bytes, capture_bytes, at)?;
    }
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(envelope.dependencies.len()).saturating_mul(48),
        at,
    )?;
    add_encoded_size(encoded_bytes, 4, at)?;
    for instantiation in &envelope.instantiations {
        add_encoded_size(encoded_bytes, 4, at)?;
        add_encoded_size(
            encoded_bytes,
            encoded_len(instantiation.bindings.len()).saturating_mul(4),
            at,
        )?;
    }
    Ok(())
}

fn add_recipe_size(encoded_bytes: &mut u64, recipe: &Recipe, at: Location) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    for node in &recipe.nodes {
        let node_bytes = match node {
            Node::I64(_) => 9,
            Node::Bool(_) => 2,
            Node::Capture { .. } => 3,
            Node::Operation { .. } => 4,
            Node::Invoke { .. } => 5,
            Node::Quotation { body, .. } => {
                7u64.saturating_add(encoded_len(body.len()).saturating_mul(4))
            }
        };
        add_encoded_size(encoded_bytes, node_bytes, at)?;
    }
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(recipe.entry.len()).saturating_mul(4),
        at,
    )?;
    Ok(())
}

fn check_stack(stack: &[Ty], ledger: &mut Ledger, field: &'static str) -> Result<(), Refusal> {
    let at = Location::field(field);
    ledger.observe_count(
        Resource::StackHeight,
        u64::try_from(stack.len()).unwrap_or(u64::MAX),
        at,
    )?;
    for ty in stack {
        if !profile_type(ty) {
            return Err(Refusal::new(
                Class::Unsupported,
                at,
                "interface holds a type outside the pure I64/Bool profile",
            ));
        }
    }
    Ok(())
}

/// The profile admits exactly `I64` and `Bool`.
pub fn profile_type(ty: &Ty) -> bool {
    matches!(ty, Ty::I64 | Ty::Bool)
}

fn check_captures(envelope: &Envelope) -> Result<(), Refusal> {
    let at = Location::field("envelope.captures");
    if u64::try_from(envelope.captures.len())
        .map_or(true, |count| count > u64::from(MAX_CAPTURES))
    {
        return Err(Refusal::new(
            Class::Invalid,
            at,
            "capture list exceeds the eight-capture bound",
        ));
    }
    for (position, capture) in envelope.captures.iter().enumerate() {
        let expected = match u16::try_from(position) {
            Ok(expected) => expected,
            Err(_) => {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "capture ordinal exceeds the finite format",
                ))
            }
        };
        if capture.index != expected || !profile_type(&capture.value.ty()) {
            return Err(Refusal::new(
                Class::Invalid,
                Location::at("envelope.captures", u32::from(expected)),
                "capture list must be contiguous from zero with profile scalars",
            ));
        }
    }
    Ok(())
}

fn check_instantiations(envelope: &Envelope, ledger: &mut Ledger) -> Result<(), Refusal> {
    for (position, instantiation) in envelope.instantiations.iter().enumerate() {
        let at = Location::at("envelope.instantiations", position as u32);
        if u64::try_from(instantiation.bindings.len())
            .map_or(true, |count| count > u64::from(MAX_INSTANTIATION_BINDINGS))
        {
            return Err(Refusal::new(
                Class::Invalid,
                at,
                "instantiation retains more than the 16-binding profile bound",
            ));
        }
        ledger.charge_count(
            Resource::InstantiationBindings,
            u64::try_from(instantiation.bindings.len()).unwrap_or(u64::MAX),
            at,
        )?;
        let mut previous: Option<u16> = None;
        for binding in &instantiation.bindings {
            if !profile_type(&binding.ty) {
                return Err(Refusal::new(
                    Class::Unsupported,
                    at,
                    "instantiation binds a variable outside the profile",
                ));
            }
            if binding.kind != noble_kernel::words::VariableKind::Value {
                return Err(Refusal::new(
                    Class::Unsupported,
                    at,
                    "the pure I64/Bool profile supports only value-variable witnesses",
                ));
            }
            if let Some(seen) = previous {
                if binding.variable <= seen {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "instantiation bindings must be in increasing variable order",
                    ));
                }
            }
            previous = Some(binding.variable);
        }
    }
    Ok(())
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
    for (position, reference) in envelope.body.entry.iter().enumerate() {
        if !reference_precedes(*reference, envelope.body.nodes.len()) {
            let Ok(position) = u32::try_from(position) else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "recipe entry ordinal exceeds the finite format",
                ));
            };
            return Err(Refusal::new(
                Class::Invalid,
                Location::at("envelope.body.entry", position),
                "recipe entry references a node outside the finite arena",
            ));
        }
    }
    for (position, node) in envelope.body.nodes.iter().enumerate() {
        let own = match u32::try_from(position) {
            Ok(position) => position,
            Err(_) => {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "recipe node ordinal exceeds the finite format",
                ))
            }
        };
        check_recipe_node(envelope, node, own, position)?;
    }
    Ok(())
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
            for reference in body {
                if !reference_precedes(*reference, position) {
                    return Err(Refusal::new(
                        Class::Invalid,
                        Location::at("envelope.body.nodes", own),
                        "quotation body must reference strictly earlier nodes",
                    ));
                }
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
    let expected = expected_bootstrap_targets(elements, at)?;
    check_invocation_targets(candidate, &expected, at)
}

fn expected_bootstrap_targets(
    elements: &[Element],
    at: Location,
) -> Result<alloc::vec::Vec<noble_kernel::contracts::Definition>, Refusal> {
    let mut expected = alloc::vec::Vec::with_capacity(elements.len());
    for (index, element) in elements.iter().enumerate() {
        let Element::Op { op, .. } = element else {
            continue;
        };
        let Some(definition) = crate::program::bootstrap_word(op.spelling().as_bytes()) else {
            return Err(Refusal::new(
                Class::Unsupported,
                Location::at(at.field, index as u32),
                "profile operation has no canonical bootstrap definition",
            ));
        };
        expected.push(definition);
    }
    Ok(expected)
}

struct CandidateFrame<'a> {
    body: &'a [noble_kernel::untrusted::NodeId],
    at: usize,
}

fn check_invocation_targets(
    candidate: &noble_kernel::untrusted::Candidate,
    expected: &[noble_kernel::contracts::Definition],
    at: Location,
) -> Result<(), Refusal> {
    let mut frames = alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES);
    frames.push(CandidateFrame {
        body: &candidate.body,
        at: 0,
    });
    let mut next = 0usize;
    while let Some(top) = frames.len().checked_sub(1) {
        let Some(frame) = frames.get_mut(top) else {
            return Err(internal(at, "candidate target walk lost its frame"));
        };
        if frame.at >= frame.body.len() {
            let _ = frames.pop();
            continue;
        }
        let position = frame.at;
        frame.at = frame.at.saturating_add(1);
        let Some(reference) = frame.body.get(position) else {
            return Err(internal(at, "candidate target walk lost a node reference"));
        };
        let Ok(node_index) = usize::try_from(reference.0) else {
            return Err(internal(
                at,
                "source parser emitted a node reference outside the host address space",
            ));
        };
        let Some(node) = candidate.nodes.get(node_index) else {
            return Err(internal(
                at,
                "source parser emitted an invalid candidate reference",
            ));
        };
        match node {
            noble_kernel::untrusted::Node::Invocation { def, .. } => {
                if expected.get(next) != Some(def) {
                    return Err(Refusal::new(
                        Class::Unsupported,
                        Location::at(at.field, next as u32),
                        "profile source spelling resolves outside its exact bootstrap operation",
                    ));
                }
                next = next.saturating_add(1);
            }
            noble_kernel::untrusted::Node::Quotation { body, .. } => {
                frames.push(CandidateFrame { body, at: 0 });
            }
            noble_kernel::untrusted::Node::Literal { .. } => {}
            noble_kernel::untrusted::Node::SignedSelf { .. }
            | noble_kernel::untrusted::Node::SlotInvoke { .. } => {
                return Err(Refusal::new(
                    Class::Unsupported,
                    at,
                    "profile source contains a signed or live invocation",
                ));
            }
        }
    }
    if next != expected.len() {
        return Err(Refusal::new(
            Class::Internal,
            at,
            "source parser and emitted operation sequence disagree",
        ));
    }
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; exhausting the shared ledger requires runtime mutation"
)]
fn exhaust_checking_work(ledger: &mut Ledger, at: Location) -> Refusal {
    let remaining = ledger.remaining(Resource::CheckingWork);
    match ledger.charge(Resource::CheckingWork, remaining.saturating_add(1), at) {
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
    ledger: &'a mut Ledger,
    at: Location,
}

/// Prepare one emitted program through ordinary source admission and
/// independently re-check the resulting submission in the kernel. The derived
/// interface is what returns; the declared one is only compared.
fn derived_interface(
    context: &mut AdmissionContext<'_>,
    source: &[u8],
    elements: &[Element],
    declared: &Interface,
) -> Result<(Interface, noble_kernel::untrusted::Candidate), Refusal> {
    let prepared = prepare_candidate(
        context.namespace,
        source,
        &declared.stack_in,
        context.ledger,
        context.at,
    )?;
    let derived = prepared_interface(&prepared, declared, context.at)?;
    let Some(submission) = prepared.submission() else {
        return Err(internal(context.at, "preparation retained no submission"));
    };
    check_bootstrap_targets(&submission.body.candidate, elements, context.at)?;
    let candidate = independently_accept(submission, &derived, context.ledger, context.at)?;
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
    match namespace.prepare(source, inputs, ledger.prepare_limits(remaining_work)) {
        Ok(prepared) => {
            ledger.charge(Resource::CheckingWork, prepared.work_used(), at)?;
            Ok(prepared)
        }
        Err(failure) => {
            ledger.charge(Resource::CheckingWork, failure.work_used(), at)?;
            let refusal = refusal_of_failure(&failure, at);
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
    let (outcome, work_used) = noble_kernel::acceptance::check_with_work(
        &submission.environment,
        &request,
        &submission.body.candidate,
    );
    ledger.charge(Resource::CheckingWork, work_used, at)?;
    match outcome {
        noble_kernel::untrusted::Outcome::Accepted(again) => {
            if again.interface.stack_in == derived.stack_in
                && again.interface.stack_out == derived.stack_out
                && again.interface.effects.is_empty()
            {
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

struct EditorWalkFrame<'a> {
    nodes: &'a [super::editor::Node],
    next: usize,
    trail: alloc::vec::Vec<u32>,
}

struct EditorWalk<'a> {
    frames: alloc::vec::Vec<EditorWalkFrame<'a>>,
    elements: alloc::vec::Vec<Element>,
    hole: Option<(usize, alloc::vec::Vec<u32>)>,
    depth: u32,
}

impl<'a> EditorWalk<'a> {
    fn new(nodes: &'a [super::editor::Node]) -> Self {
        let mut frames = alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES);
        frames.push(EditorWalkFrame {
            nodes,
            next: 0,
            trail: alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES),
        });
        Self {
            frames,
            elements: alloc::vec::Vec::with_capacity(nodes.len()),
            hole: None,
            depth: 0,
        }
    }

    fn advance(&mut self, ledger: &mut Ledger, at: Location) -> Result<bool, Refusal> {
        let Some(top) = self.frames.len().checked_sub(1) else {
            return Ok(false);
        };
        let (nodes, index, finished) = {
            let Some(frame) = self.frames.get_mut(top) else {
                return Err(internal(at, "editor walk lost its frame"));
            };
            let nodes: &'a [super::editor::Node] = frame.nodes;
            if frame.next >= frame.nodes.len() {
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
                at,
                "editor node reference is outside the finite tree",
            ));
        };
        match node {
            super::editor::Node::Hole | super::editor::Node::Quotation(_) => {
                let Some(frame) = self.frames.get(top) else {
                    return Err(internal(at, "editor walk lost its path frame"));
                };
                self.append_path_node(node, index, frame.trail.clone(), ledger, at)?;
            }
            _ => self.append_leaf(node, at)?,
        }
        Ok(true)
    }

    fn append_leaf(
        &mut self,
        node: &super::editor::Node,
        at: Location,
    ) -> Result<(), Refusal> {
        match node {
            super::editor::Node::Integer(value) => self.elements.push(Element::I64(*value)),
            super::editor::Node::Boolean(value) => self.elements.push(Element::Bool(*value)),
            super::editor::Node::Word(word) => match Op::of(word.as_bytes()) {
                Some(op) => self.elements.push(Element::Op { op, witness: None }),
                None => {
                    return Err(Refusal::new(
                        Class::Unsupported,
                        at,
                        "editor word is outside the published profile whitelist",
                    ))
                }
            },
            super::editor::Node::Hole | super::editor::Node::Quotation(_) => {
                return Err(internal(at, "editor path node was sent to the leaf handler"))
            }
        }
        Ok(())
    }

    fn append_path_node(
        &mut self,
        node: &'a super::editor::Node,
        index: usize,
        mut trail: alloc::vec::Vec<u32>,
        ledger: &mut Ledger,
        at: Location,
    ) -> Result<(), Refusal> {
        let position = u32::try_from(index).map_err(|_| {
            Refusal::new(
                Class::Invalid,
                at,
                "editor path position exceeds the path index width",
            )
        })?;
        trail.push(position);
        match node {
            super::editor::Node::Hole => {
                if self.hole.is_some() {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "editor tree holds more than one hole",
                    ));
                }
                self.hole = Some((self.elements.len(), trail));
                self.elements.push(Element::Hole);
            }
            super::editor::Node::Quotation(children) => {
                self.depth = self.depth.saturating_add(1);
                ledger.observe(Resource::QuotationDepth, self.depth, at)?;
                self.elements.push(Element::Open);
                self.frames.push(EditorWalkFrame {
                    nodes: children,
                    next: 0,
                    trail,
                });
            }
            _ => return Err(internal(at, "editor leaf was sent to the path handler")),
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
    while walk.advance(ledger, at)? {}
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
fn fill(
    elements: &[Element],
    hole: usize,
    proposal: &[Element],
) -> alloc::vec::Vec<Element> {
    let mut out = alloc::vec::Vec::with_capacity(
        elements
            .len()
            .saturating_add(proposal.len())
            .saturating_sub(1),
    );
    out.extend_from_slice(&elements[..hole.min(elements.len())]);
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
    if diagnostics
        .iter()
        .any(|retained| retained.class == refusal.class && retained.location == refusal.location)
    {
        return Ok(());
    }
    ledger.charge(Resource::Diagnostics, 1, refusal.location)?;
    ledger.charge(
        Resource::RetainedBytes,
        refusal_encoded_size(&refusal)?,
        refusal.location,
    )?;
    diagnostics.push(refusal);
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

/// One scalar or quotation value during fixture evaluation.
#[octet::sealed_enum]
#[derive(Clone, Debug)]
enum Value<'a> {
    I64(i64),
    Bool(bool),
    Program(&'a [Element]),
}

fn collect_quotation(program: &[Element], start: usize) -> Option<(&[Element], usize)> {
    let mut depth = 0u32;
    let mut at = start;
    while let Some(element) = program.get(at) {
        match element {
            Element::Open => depth = depth.saturating_add(1),
            Element::Close => {
                if depth == 0 {
                    return Some((program.get(start..at)?, at.saturating_add(1)));
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
fn eval_operation<'a>(
    op: Op,
    stack: &mut alloc::vec::Vec<Value<'a>>,
    frames: &mut alloc::vec::Vec<(&'a [Element], usize)>,
    at: Location,
) -> Result<(), Refusal> {
    match op {
        Op::Dup => {
            let Some(value) = stack.last().cloned() else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "fixture evaluation underflowed",
                ));
            };
            stack.push(value);
            Ok(())
        }
        Op::Drop => {
            if stack.pop().is_none() {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "fixture evaluation underflowed",
                ));
            }
            Ok(())
        }
        Op::Swap => {
            let (Some(top), Some(next)) = (stack.pop(), stack.pop()) else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "fixture evaluation underflowed",
                ));
            };
            stack.push(top);
            stack.push(next);
            Ok(())
        }
        Op::Add | Op::Sub | Op::Mul | Op::Eq => eval_binary(op, stack, at),
        Op::Run => {
            let value = pop_value(stack, at)?;
            let Value::Program(body) = value else {
                return Err(Refusal::new(
                    Class::Invalid,
                    at,
                    "fixture run operand is not a quotation value",
                ));
            };
            frames.push((body, 0));
            Ok(())
        }
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; binary fixture operations replace two stack values with one result"
)]
fn eval_binary<'a>(
    op: Op,
    stack: &mut alloc::vec::Vec<Value<'a>>,
    at: Location,
) -> Result<(), Refusal> {
    let (Value::I64(right), Value::I64(left)) = (pop_value(stack, at)?, pop_value(stack, at)?)
    else {
        return Err(Refusal::new(
            Class::Invalid,
            at,
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
                at,
                "fixture binary operation received a non-binary word",
            ));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; fixture stack pop transfers ownership of its top value"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; popping a runtime Vec cannot be evaluated during const evaluation"
)]
fn pop_value<'a>(
    stack: &mut alloc::vec::Vec<Value<'a>>,
    at: Location,
) -> Result<Value<'a>, Refusal> {
    match stack.pop() {
        Some(value) => Ok(value),
        None => Err(Refusal::new(
            Class::Invalid,
            at,
            "fixture evaluation underflowed",
        )),
    }
}

/// Evaluate one completed program over host-supplied inputs. The result is
/// fixture evidence only: it is reported beside checking, never as acceptance.
fn evaluate<'a>(
    elements: &'a [Element],
    inputs: &[Scalar],
    ledger: &mut Ledger,
) -> Result<Option<alloc::vec::Vec<Scalar>>, Refusal> {
    let at = Location::field("request.fixtures");
    let mut stack: alloc::vec::Vec<Value<'a>> =
        alloc::vec::Vec::with_capacity(MAX_STACK_CAPACITY);
    for input in inputs {
        stack.push(match input {
            Scalar::I64(value) => Value::I64(*value),
            Scalar::Bool(value) => Value::Bool(*value),
        });
    }
    let mut frames: alloc::vec::Vec<(&'a [Element], usize)> =
        alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES);
    frames.push((elements, 0));
    while let Some(top) = frames.len().checked_sub(1) {
        let is_finished = match frames.get(top) {
            Some((program, index)) => *index >= program.len(),
            None => return Ok(None),
        };
        if is_finished {
            let _ = frames.pop();
            continue;
        }
        let (program, position) = match frames.get(top) {
            Some((program, index)) => (*program, *index),
            None => return Ok(None),
        };
        let Some(element) = program.get(position) else {
            return Ok(None);
        };
        if let Some((_, index)) = frames.get_mut(top) {
            *index = index.saturating_add(1);
        }
        ledger.charge(Resource::EvaluationSteps, 1, at)?;
        match element {
            Element::I64(value) => stack.push(Value::I64(*value)),
            Element::Bool(value) => stack.push(Value::Bool(*value)),
            Element::Open => {
                let Some((body, after)) = collect_quotation(program, position.saturating_add(1))
                else {
                    return Ok(None);
                };
                if let Some((_, index)) = frames.get_mut(top) {
                    *index = after;
                } else {
                    return Ok(None);
                }
                stack.push(Value::Program(body));
            }
            Element::Op { op, .. } => eval_operation(*op, &mut stack, &mut frames, at)?,
            Element::Close | Element::Hole => return Ok(None),
        }
        ledger.observe_count(
            Resource::StackHeight,
            u64::try_from(stack.len()).unwrap_or(u64::MAX),
            at,
        )?;
    }
    let mut out = alloc::vec::Vec::with_capacity(stack.len());
    for value in stack {
        match value {
            Value::I64(value) => out.push(Scalar::I64(value)),
            Value::Bool(value) => out.push(Scalar::Bool(value)),
            Value::Program(_) => return Ok(None),
        }
    }
    Ok(Some(out))
}

/// One completion attempt: one shared ledger, one request, one hole equation.
struct Search<'a> {
    namespace: &'a super::Session,
    library: &'a Library,
    request: &'a Request,
    flat: FlatRequest,
    stack_in: alloc::vec::Vec<Ty>,
    ledger: Ledger,
    diagnostics: alloc::vec::Vec<Refusal>,
    rejected: u32,
    checked_candidates: u32,
    fixtures: Option<FixtureReport>,
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
        let mut pending = alloc::vec::Vec::with_capacity(MAX_DEPENDENCY_PENDING);
        pending.push(start);
        let mut selected_expansion = None;
        while let Some(index) = pending.pop() {
            let Some(entry) = library.entries().get(usize::from(index)) else {
                return Err(internal(
                    Location::field("proposal.sequence"),
                    "selected dependency is not retained",
                ));
            };
            let envelope = entry.envelope();
            ledger.observe_count(
                Resource::RecipeNodes,
                u64::try_from(envelope.body.nodes.len()).unwrap_or(u64::MAX),
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
                u64::try_from(source.len()).unwrap_or(u64::MAX),
                Location::field("proposal.revalidation"),
            )?;
            if source != entry.source || canonical_revision(&source) != envelope.source_revision {
                return Err(Refusal::new(
                    Class::Invalid,
                    Location::field("proposal.revalidation"),
                    "retained chunk source no longer matches its immutable recipe revision",
                ));
            }
            let mut admission = AdmissionContext {
                namespace,
                ledger,
                at: Location::field("proposal.revalidation"),
            };
            let (interface, _) = derived_interface(
                &mut admission,
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
            if index == start {
                selected_expansion = Some(expansion);
            }
            for (position, dependency) in envelope.dependencies.iter().enumerate() {
                let at = Location::at("proposal.dependencies", position as u32);
                ledger.charge(Resource::DependencySteps, 1, at)?;
                let Some(target) = library.find(dependency) else {
                    return Err(Refusal::new(
                        Class::Invalid,
                        at,
                        "transitive dependency no longer resolves by its exact identity and revisions",
                    ));
                };
                pending.push(target);
            }
        }
        selected_expansion.ok_or_else(|| {
            internal(
                Location::field("proposal.revalidation"),
                "selected chunk was not revalidated",
            )
        })
    }

    /// Expand one ordered chunk sequence and compute the ordered stack it
    /// produces at the hole. Every boundary must match exactly.
    fn compose(&mut self, sequence: &[u16]) -> Result<Result<Composition, Refusal>, Refusal> {
        let at = Location::field("proposal");
        // Copy the library reference out first: the entries are borrowed while
        // the shared ledger is charged, and the two borrows are independent.
        let library: &Library = self.library;
        let entries = library.entries();
        let mut elements = alloc::vec::Vec::with_capacity(MAX_EXPANDED_NODE_CAPACITY);
        let mut current = self.stack_in.clone();
        let mut untouched = current.len();
        for (position, index) in sequence.iter().enumerate() {
            let expansion =
                Self::revalidate_entry(self.namespace, self.library, &mut self.ledger, *index)?;
            let Some(entry) = entries.get(usize::from(*index)) else {
                return Err(internal(at, "selected entry is not retained"));
            };
            elements.extend(expansion.elements);
            let interface = entry.interface();
            if !current.ends_with(&interface.stack_in) {
                return Ok(Err(Refusal::new(
                    Class::Invalid,
                    Location::at(
                        "proposal.sequence",
                        position as u32,
                    ),
                    "selected chunk does not fit the exact ordered stack at this composition boundary",
                )));
            }
            let above_untouched = current.len().saturating_sub(untouched);
            let consumed_from_untouched = interface.stack_in.len().saturating_sub(above_untouched);
            untouched = untouched.saturating_sub(consumed_from_untouched);
            current.truncate(current.len().saturating_sub(interface.stack_in.len()));
            current.extend(interface.stack_out.iter().cloned());
            self.ledger
                .observe_count(
                    Resource::StackHeight,
                    u64::try_from(current.len()).unwrap_or(u64::MAX),
                    at,
                )?;
        }
        Ok(Ok(Composition {
            elements,
            stack: current,
            untouched,
        }))
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
        let mut admission = AdmissionContext {
            namespace: self.namespace,
            ledger: &mut self.ledger,
            at,
        };
        match derived_interface(
            &mut admission,
            source,
            elements,
            declared,
        ) {
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
        for fixture in &set.cases {
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
            is_agreement &= is_agreed;
            cases.push(FixtureCaseReport {
                inputs: fixture.inputs.clone(),
                expected: fixture.outputs.clone(),
                actual,
                agrees: is_agreed,
            });
        }
        Ok(FixtureReport {
            id: set.id,
            cases,
            agreement: is_agreement,
            steps: self.ledger.observed(Resource::EvaluationSteps),
        })
    }

    /// Attempt one shortlex proposal. `Ok(Step::Refused)` keeps the search
    /// going; `Err` is a request-level budget exhaustion and stops it.
    fn candidate(&mut self, sequence: &[u16]) -> Result<Step, Refusal> {
        let at = Location::field("proposal");
        self.ledger.charge(Resource::Candidates, 1, at)?;
        self.ledger
            .observe_count(
                Resource::ComposedChunks,
                u64::try_from(sequence.len()).unwrap_or(u64::MAX),
                at,
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
        if let Err(refusal) = replay.run(&filled, Some(boundary), &mut self.ledger, true, at) {
            return Ok(Step::Refused(refusal));
        }
        if replay.mark.as_deref() != Some(stack.as_slice()) {
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
        let source = match emit(&filled, at) {
            Ok(source) => source,
            Err(refusal) => return Ok(Step::Refused(refusal)),
        };
        self.ledger
            .observe_count(
                Resource::SourceBytes,
                u64::try_from(source.len()).unwrap_or(u64::MAX),
                at,
            )?;
        let declared = Interface {
            stack_in: self.request.task.stack_in.clone(),
            stack_out: self.request.task.stack_out.clone(),
            effects: self.request.task.allowed_effects.clone(),
        };
        self.checked_candidates = self.checked_candidates.saturating_add(1);
        let (interface, candidate) = match self.prepare(&source, &filled, &declared, at)? {
            Ok(admitted) => admitted,
            Err(refusal) => return Ok(Step::Refused(refusal)),
        };
        let tail = u32::try_from(untouched)
            .map_err(|_| internal(at, "stack tail exceeds the fixed report width"))?;
        let selected: alloc::vec::Vec<[u8; IDENTITY_BYTES]> = sequence
            .iter()
            .filter_map(|index| self.library.entries().get(usize::from(*index)))
            .map(|entry| entry.envelope().chunk_id)
            .collect();
        let stack_bytes = [
            self.stack_in.len(),
            stack.len(),
            interface.stack_in.len(),
            interface.stack_out.len(),
        ]
        .iter()
        .fold(0u64, |total, length| {
            total.saturating_add(u64::try_from(*length).unwrap_or(u64::MAX))
        });
        let output_bytes = u64::try_from(source.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(2)
            .saturating_add(
                u64::try_from(selected.len())
                    .unwrap_or(u64::MAX)
                    .saturating_mul(IDENTITY_BYTES_U64),
            )
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
    let mut order: alloc::vec::Vec<u16> =
        alloc::vec::Vec::with_capacity(library.entries().len());
    for index in 0..library.entries().len() {
        if let Ok(index) = u16::try_from(index) {
            order.push(index);
        }
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
    order
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

fn check_fixtures(request: &Request) -> Result<(), Refusal> {
    let Some(set) = &request.fixtures else {
        return Ok(());
    };
    for (position, fixture) in set.cases.iter().enumerate() {
        let is_input_match = fixture.inputs.len() == request.task.stack_in.len()
            && fixture
                .inputs
                .iter()
                .zip(&request.task.stack_in)
                .all(|(value, ty)| value.ty() == *ty);
        let is_output_match = fixture.outputs.len() == request.task.stack_out.len()
            && fixture
                .outputs
                .iter()
                .zip(&request.task.stack_out)
                .all(|(value, ty)| value.ty() == *ty);
        if !is_input_match || !is_output_match {
            return Err(Refusal::new(
                Class::Invalid,
                Location::at("request.fixtures.cases", position as u32),
                "fixture values do not match the original ordered task interface",
            ));
        }
    }
    Ok(())
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
    let at = refusal.location;
    add_encoded_size(
        &mut encoded_bytes,
        4u64.saturating_add(encoded_len(refusal.location.field.len())),
        at,
    )?;
    add_encoded_size(
        &mut encoded_bytes,
        1u64.saturating_add(if refusal.location.index.is_some() { 4 } else { 0 }),
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
    u32::try_from(encoded_bytes).map_err(|_| {
        let mut exhausted = Refusal::new(
            Class::Exhausted,
            at,
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
        1u64
            .saturating_add(IDENTITY_BYTES_U64)
            .saturating_add(4)
            .saturating_add(1)
            .saturating_add(4),
        at,
    )?;
    for fixture in &set.cases {
        add_encoded_size(&mut encoded_bytes, 4, at)?;
        for value in &fixture.inputs {
            add_encoded_size(&mut encoded_bytes, scalar_encoded_size(*value), at)?;
        }
        add_encoded_size(&mut encoded_bytes, 4, at)?;
        for value in &fixture.outputs {
            add_encoded_size(&mut encoded_bytes, scalar_encoded_size(*value), at)?;
        }
        add_encoded_size(&mut encoded_bytes, 1 + 4, at)?;
        for value in &fixture.outputs {
            add_encoded_size(&mut encoded_bytes, scalar_encoded_size(*value), at)?;
        }
        add_encoded_size(&mut encoded_bytes, 1, at)?;
    }
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

/// Count the canonical request representation before walking its tree. This
/// makes the retained/request-byte limit apply to attacker-controlled words,
/// nested editor nodes, fixture values and metadata, not only emitted source.
fn request_size(request: &Request, ledger: &mut Ledger) -> Result<u32, Refusal> {
    let at = Location::field("request.retained");
    let mut encoded_bytes = 0u64;
    add_request_header_size(&mut encoded_bytes, request, at)?;
    add_editor_tree_size(&mut encoded_bytes, &request.editor.nodes, ledger, at)?;
    add_request_tail_size(&mut encoded_bytes, request, at)?;
    u32::try_from(encoded_bytes)
        .map_err(|_| internal(at, "bounded request size did not fit the reporting counter"))
}

fn add_request_header_size(
    encoded_bytes: &mut u64,
    request: &Request,
    at: Location,
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
    at: Location,
) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    let mut frames: alloc::vec::Vec<(&[super::editor::Node], usize, u32)> =
        alloc::vec::Vec::with_capacity(MAX_QUOTATION_FRAMES);
    frames.push((editor_nodes, 0, 0));
    add_encoded_size(encoded_bytes, 4, at)?;
    while let Some(top) = frames.len().checked_sub(1) {
        let Some((nodes, index, depth)) = frames.get_mut(top) else {
            return Err(internal(at, "request size walk lost its frame"));
        };
        if *index >= nodes.len() {
            let _ = frames.pop();
            continue;
        }
        let Some(node) = nodes.get(*index) else {
            return Err(internal(at, "request size walk lost an editor node"));
        };
        *index = index.saturating_add(1);
        let mut nested = None;
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
            super::editor::Node::Quotation(children) => {
                add_encoded_size(encoded_bytes, 5, at)?;
                let next_depth = depth.saturating_add(1);
                ledger.observe(Resource::QuotationDepth, next_depth, at)?;
                nested = Some((children.as_slice(), next_depth));
            }
        }
        if let Some((children, next_depth)) = nested {
            frames.push((children, 0, next_depth));
        }
    }
    Ok(())
}

fn add_request_tail_size(
    encoded_bytes: &mut u64,
    request: &Request,
    at: Location,
) -> Result<(), Refusal> {
    add_encoded_size(encoded_bytes, 4, at)?;
    add_encoded_size(
        encoded_bytes,
        encoded_len(request.hole_path.len()).saturating_mul(4),
        at,
    )?;
    add_encoded_size(encoded_bytes, REVISION_BYTES_U64, at)?;
    let budget_bytes = u64::try_from(Resource::COUNT)
        .unwrap_or(u64::MAX)
        .saturating_mul(4);
    add_encoded_size(encoded_bytes, budget_bytes, at)?;
    if let Some(fixtures) = &request.fixtures {
        let fixture_header_bytes = 21u64;
        add_encoded_size(encoded_bytes, fixture_header_bytes, at)?;
        for fixture in &fixtures.cases {
            add_encoded_size(encoded_bytes, 4, at)?;
            for value in &fixture.inputs {
                add_encoded_size(encoded_bytes, scalar_encoded_size(*value), at)?;
            }
            add_encoded_size(encoded_bytes, 4, at)?;
            for value in &fixture.outputs {
                add_encoded_size(encoded_bytes, scalar_encoded_size(*value), at)?;
            }
        }
    } else {
        add_encoded_size(encoded_bytes, 1, at)?;
    }
    Ok(())
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
        u64::try_from(library.entries().len()).unwrap_or(u64::MAX),
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
fn analyze_editor_context(
    namespace: &super::Session,
    request: &Request,
    ledger: &mut Ledger,
) -> Result<(), Refusal> {
    let at = Location::field("request.editor");
    let limits = crate::Limits {
        work: ledger.remaining(Resource::CheckingWork),
        ..crate::Limits::default()
    };
    let (analysis, work_used) =
        match namespace.analyze_editor_metered(&request.editor, &request.task.stack_in, limits) {
            Ok((analysis, work_used)) => (analysis, work_used),
            Err(failure) => {
                ledger.charge(Resource::CheckingWork, failure.work_used(), at)?;
                if failure.diagnostic().kind == crate::DiagnosticKind::Exhausted {
                    // `Candidate::build` can refuse before its meter is returned;
                    // reserve the remaining allowance on this fail-closed path.
                    let remaining = ledger.remaining(Resource::CheckingWork);
                    let observed = remaining.saturating_add(1);
                    return match ledger.charge(Resource::CheckingWork, observed, at) {
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
    ledger.charge(Resource::CheckingWork, work_used, at)?;
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
    Ok(())
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

/// Visit every sequence of one length in canonical shortlex order.
fn run_length(
    search: &mut Search<'_>,
    order: &[u16],
    chunk_count: u32,
) -> Result<(), alloc::boxed::Box<Response>> {
    let width = order.len();
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
    loop {
        sequence.clear();
        for digit in &digits {
            if let Some(index) = order.get(usize::from(*digit)) {
                sequence.push(*index);
            }
        }
        match search.candidate(&sequence) {
            Ok(Step::Admitted(accepted)) => {
                let report = search.report(1);
                return Err(alloc::boxed::Box::new(Response::Accepted {
                    accepted,
                    report: alloc::boxed::Box::new(report),
                }));
            }
            Ok(Step::Refused(refusal)) => {
                search.rejected = search.rejected.saturating_add(1);
                if let Err(exhausted) = record(&mut search.ledger, &mut search.diagnostics, refusal)
                {
                    return Err(alloc::boxed::Box::new(response_of_search(
                        exhausted, search,
                    )));
                }
            }
            Err(refusal) => {
                return Err(alloc::boxed::Box::new(response_of_search(refusal, search)))
            }
        }
        if !advance(&mut digits, width) {
            break;
        }
    }
    Ok(())
}

/// Run one bounded one-hole completion request.
///
/// The search is deterministic shortlex over the canonical library order. It
/// never commits, never publishes and never treats a signature, a digest or a
/// fixture result as acceptance authority.
pub fn complete(
    namespace: &super::Session,
    library: &Library,
    request: &Request,
    monotonic_now_ns: fn() -> u64,
) -> Response {
    let started = monotonic_now_ns();
    let mut response = complete_inner(namespace, library, request);
    response.report_mut().total_latency_ns = monotonic_now_ns().saturating_sub(started);
    response
}

fn complete_inner(namespace: &super::Session, library: &Library, request: &Request) -> Response {
    let mut ledger = Ledger::new(request.budget.lowered(&Budget::PROFILE));
    let flat = match check_request(request, library, &mut ledger) {
        Ok(flat) => flat,
        Err(refusal) => return response_of(refusal, &ledger),
    };
    if let Err(refusal) = analyze_editor_context(namespace, request, &mut ledger) {
        return response_of(refusal, &ledger);
    }
    let mut prefix = Replay::new(&request.task.stack_in);
    let up_to = match flat.elements.get(..flat.hole) {
        Some(slice) => slice,
        None => {
            return response_of(
                internal(
                    Location::field("request.editor"),
                    "hole index is outside the tree",
                ),
                &ledger,
            )
        }
    };
    if let Err(refusal) = prefix.run(up_to, None, &mut ledger, false, Location::field("editor")) {
        return response_of(refusal, &ledger);
    }
    let stack_in = prefix.stack.clone();
    let mut search = Search {
        namespace,
        library,
        request,
        flat,
        stack_in,
        ledger,
        diagnostics: alloc::vec::Vec::new(),
        rejected: 0,
        checked_candidates: 0,
        fixtures: None,
    };
    let order = canonical_order(library);
    let mut chunk_count = 0u32;
    while chunk_count <= MAX_COMPOSED_CHUNKS {
        if let Err(response) = run_length(&mut search, &order, chunk_count) {
            return *response;
        }
        chunk_count = chunk_count.saturating_add(1);
    }
    Response::NoResult {
        report: alloc::boxed::Box::new(search.report(0)),
    }
}
