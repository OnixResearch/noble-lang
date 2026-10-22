//! Persistent Core-Bootstrap compilation. All supplied payloads are untrusted.
//!
//! Preparation checks the exact environment, every concrete definition and the
//! root before emitting code. It never changes this compiler. Commit consumes
//! the preparation and rejects stale or different base allocation states.
//! Runtime instances must share the objects described in
//! `noble-cli/src/core/runtime/abi.json`; no recipes execute.

mod admission;
mod emit;
mod origin;
mod plan;
mod preflight;
mod reuse;
mod transaction;
mod types;

pub use reuse::{CanonicalEpoch, Reusable};

const TABLE_LIMIT: u32 = 16_384;
// Calculator table slots 0..=4 hold the generation-0 core entries; compiled
// code is allocated from here.
const CALCULATOR_FUNCTION_BASE: u32 = 5;
const TEXT_START: u32 = 262_144;
const CALCULATOR_TEXT_START: u32 = 524_288;
// Exclusive end of calculator static literal Text; signature keys are copied
// through the scratch region that starts here.
const CALCULATOR_TEXT_END: u32 = 983_040;
const MEMORY_BYTES: u32 = 1_048_576;
const NODE_LIMIT: usize = 4096;
const DEFINITION_LIMIT: usize = 256;
// The opt-in calculator's checked source closure can include numeric modules
// whose 321 reachable named bodies exceed the older 256-definition profile.
const CALCULATOR_DEFINITION_LIMIT: usize = 512;
// Calculator source and compiler share a 896-row bound. The independently
// bounded named-body count and per-clone/fanout budgets are separate gates.
// Retain the complete E×B scans rather than discounting them. This 2b limit
// caps issued preparation accounting, NOT physical CPU instructions or
// resident bytes; preclone structural guards still reject amplification.
pub(super) const CALCULATOR_COMPILER_WORK_LIMIT: u64 = 2_000_000_000;

// Core live sessions reserve 24/25 for their two checked operations.
// Ordinary and declared sessions retain their original fixed prefixes.
fn builtin_count(env: &noble_kernel::contracts::Env) -> u32 {
    if env.calculator && env.declared_modules {
        return 32;
    }
    // Declared text extends the cursor prefix with signed I64 `<` at 27.
    if env.text_cursor && env.declared_modules {
        return 28;
    }
    if env.declared_modules {
        return 23;
    }
    if env.text_cursor {
        return 27;
    }
    if env.effects.contains(&noble_kernel::types::EffId(3)) {
        return 26;
    }
    if env.effects.contains(&noble_kernel::types::EffId(1)) {
        return 24;
    }
    23
}

struct Work {
    // Spent is always issued - remaining; both the original caller budget
    // and every selected allowance stay in the same cumulative ledger.
    remaining: u64,
    issued: u64,
    // Only borrowed calculator preflight may charge its measured inspection
    // on top of the caller's still-reserved kernel acceptance budget.
    calculator_preflight: bool,
    calculator_plan: bool,
    // Observation only: the largest structural node count admitted against
    // the request's `type_size` by borrowed preflight. Never charged.
    max_type_nodes: u32,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the budget belongs to one private preparation and tracks consumed work without modifying the borrowed submission or published compiler."
)]
impl Work {
    const fn charge(&mut self, amount: u64) -> Result<(), crate::Diagnostic> {
        if self.calculator_preflight && amount != 0 {
            // No owned kernel clone can begin until the complete borrowed
            // preflight succeeds. Keep every actual inspection charge in the
            // same finite ledger without spending the caller's body budget.
            attempt!(self.grant_calculator(amount));
        } else if self.calculator_plan && amount > self.remaining {
            // The independent kernel, preflight and environment admission have
            // already finished. Issue only the shortfall for this measured
            // plan operation; never reset or discount previously charged work.
            attempt!(self.grant_calculator(amount - self.remaining));
        }
        self.remaining = match self.remaining.checked_sub(amount) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        Ok(())
    }

    /// Only the opt-in calculator grants measured structural compilation
    /// work. The cumulative issued amount, not just unspent work, is capped.
    const fn grant_calculator(&mut self, amount: u64) -> Result<(), crate::Diagnostic> {
        let Some(issued) = self.issued.checked_add(amount) else {
            return Err(crate::Diagnostic::Exhausted);
        };
        let Some(remaining) = self.remaining.checked_add(amount) else {
            return Err(crate::Diagnostic::Exhausted);
        };
        if issued > CALCULATOR_COMPILER_WORK_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        self.issued = issued;
        self.remaining = remaining;
        Ok(())
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; charging a platform-sized entry count uses non-const TryFrom and preserves Exhausted when the count cannot fit the u64 work meter."
    )]
    fn entries(&mut self, count: usize) -> Result<(), crate::Diagnostic> {
        match u64::try_from(count) {
            Ok(count) => self.charge(count),
            Err(_) => Err(crate::Diagnostic::Exhausted),
        }
    }

    const fn observe_type_size(&mut self, nodes: u32) {
        if nodes > self.max_type_nodes {
            self.max_type_nodes = nodes;
        }
    }
}

/// The selected calculator's fixed compiler ceilings, published beside
/// [`CompileMetrics`] so a headroom report compares against the code's own
/// constants rather than a copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CalculatorCaps {
    pub work_issued: u64,
    pub environment_rows: u64,
    pub named_bodies: u64,
    pub signature_keys: u64,
    pub table_entries: u32,
    pub wat_bytes: u64,
    pub text_start: u32,
    pub text_end: u32,
}

pub const CALCULATOR_CAPS: CalculatorCaps = CalculatorCaps {
    work_issued: CALCULATOR_COMPILER_WORK_LIMIT,
    environment_rows: noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT as u64,
    named_bodies: CALCULATOR_DEFINITION_LIMIT as u64,
    signature_keys: crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT as u64,
    table_entries: TABLE_LIMIT,
    wat_bytes: 4 * crate::OUTPUT_BYTE_LIMIT as u64,
    text_start: CALCULATOR_TEXT_START,
    text_end: CALCULATOR_TEXT_END,
};

/// Observed accounting of one successful preparation, for headroom reports.
/// Collecting it never changes admission, charging or the emitted bytes. The
/// `work_*` grants partition `work_issued - work_initial`; the session fields
/// (`signature_keys`, `table_end`, `text_end`) describe the prospective
/// compiler this preparation would commit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompileMetrics {
    /// The caller's preparation allowance the ledger starts from.
    pub work_initial: u64,
    /// Measured borrowed-preflight charges granted on top of the allowance.
    pub work_preflight_grant: u64,
    /// The calculator environment allowance, `environment_work × (named + 3)`.
    pub work_environment_grant: u64,
    /// Plan-lowering shortfalls issued after kernel acceptance.
    pub work_plan_grant: u64,
    /// Cumulative issued units, bounded by `CALCULATOR_CAPS.work_issued`.
    pub work_issued: u64,
    pub work_spent: u64,
    pub environment_rows: u64,
    /// One weighted environment scan; every stage pays it `named + 3` times.
    pub environment_work: u64,
    pub named_bodies: u64,
    /// Largest instantiated stack-entry or value type admitted by preflight.
    pub max_type_nodes: u32,
    /// The request's `type_size` ceiling that `max_type_nodes` was checked against.
    pub type_nodes_cap: u32,
    pub signature_keys: u64,
    /// Retained-key length charged before lowering; grows with the pool.
    pub signature_key_charge: u64,
    /// Retained identity-recipe length charged before lowering.
    pub identity_charge: u64,
    pub table_end: u32,
    pub text_end: u32,
    pub wat_bytes: u64,
}

/// Persistent semantic identities and disjoint compiled-code allocations.
/// A clone is an independent copy of the committed state, so a calculator
/// host can restore it after a failed submission.
#[derive(Clone, Debug)]
pub struct Compiler {
    live: bool,
    declared_live: bool,
    live_slots: bool,
    text_cursor: bool,
    declared_text: bool,
    calculator: bool,
    worker_inputs: bool,
    generation: u32,
    functions: u32,
    retention_revision: u64,
    free_code: alloc::vec::Vec<FreeCode>,
    installed_code: alloc::vec::Vec<CodeSpan>,
    text_end: u32,
    // Calculator only: each distinct static literal's first address. Like
    // `text_end` it is never rewound, so a recorded address stays valid.
    static_literals: alloc::collections::BTreeMap<alloc::vec::Vec<u8>, u32>,
    signatures: crate::signatures::Pool,
    descriptors: alloc::vec::Vec<(u32, u32)>,
    identities: alloc::vec::Vec<Identity>,
}

/// One installed module's physical table allocation. The generation identifies
/// its owner even after the same table indices have been reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeSpan {
    pub start: u32,
    pub slots: u32,
    pub generation: u32,
}

/// A compiler-bound dispatch site. The site ID is unique within this emitted
/// module; the source node ID remains scoped to its independently checked body.
#[derive(Clone, Debug)]
pub struct LiveSiteMetadata {
    pub site_id: u32,
    pub caller_program_index: usize,
    pub source_node: noble_kernel::untrusted::NodeId,
    pub selected_ref_logical_position: u32,
    pub forwarded_source_positions: alloc::vec::Vec<u32>,
    pub forwarded_target_positions: alloc::vec::Vec<u32>,
    pub target_input: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub target_output: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub effect_ceiling: noble_kernel::types::EffSet,
    /// Exact full-signature IDs shared by all modules of this compiler.
    pub input_signature: u32,
    pub output_signature: u32,
    pub effect_mask: u32,
}

/// The immutable checked target interface. Artifact bytes, value identity,
/// captures, evidence and host authorization are separately bound by the host.
#[derive(Clone, Debug)]
pub struct TargetMetadata {
    pub root_program_index: usize,
    pub input_signature: u32,
    pub output_signature: u32,
    pub effect_mask: u32,
    pub stack_in: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub stack_out: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub effects: noble_kernel::types::EffSet,
}

/// The independently checked type and immutable allocation of one emitted
/// `$sN` Program, including quotation bodies. Runtime-created `quote` and
/// `compose` values are not globals and must be checked by another route.
#[derive(Clone, Debug)]
pub struct ProgramMetadata {
    pub program_index: usize,
    pub entry: u32,
    pub input_signature: u32,
    pub output_signature: u32,
    pub effect_mask: u32,
    pub stack_in: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub stack_out: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub effects: noble_kernel::types::EffSet,
    /// `$program` installation emits zero runtime capture children at a/b;
    /// the c edge is its separately checked immutable recipe graph.
    pub capture_left: u32,
    pub capture_right: u32,
    pub recipe_depth: u32,
    pub recipe_leaves: u32,
    /// Closed, source-checked atom sequence for exact installed c-recipe
    /// correspondence. Only the Worker ingress retains these atoms.
    pub runtime_atoms: alloc::vec::Vec<ProgramAtomMetadata>,
}

#[derive(Clone, Debug)]
pub struct ProgramAtomMetadata {
    pub source_node: u32,
    pub tag: u32,
    pub payload: i64,
    pub child_program: Option<usize>,
    pub text: Option<(u32, u32)>,
    pub input: u32,
    pub output: u32,
    pub effects: u32,
    pub ordinal: u32,
}

/// D-neutral, source-checked lineage for a selected named caller's returned
/// dynamic Program. This describes no target DefinitionId or proof claim.
#[derive(Clone, Debug)]
pub struct SelectedOriginMetadata {
    pub definition_id: noble_contracts::source::proof::CheckedDefinitionId,
    pub source_generation: u64,
    pub named_program_index: usize,
    pub root_program_index: usize,
    pub programs: alloc::vec::Vec<OriginProgram>,
    pub outputs: alloc::vec::Vec<OriginOutput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-wasm-maintainers; the name matches the established vocabulary used by callers; renaming it would churn call sites without clarifying them."
)]
pub enum OriginSource {
    Definition,
    Selection,
}

#[derive(Clone, Debug)]
pub struct OriginProgram {
    pub source_artifact: OriginSource,
    pub program_index: usize,
    /// Source replay quotation NodeId; the named body and selection wrapper
    /// are identified separately and have no quotation occurrence here.
    pub body_node: Option<u32>,
    pub entry: u32,
    pub input_signature: u32,
    pub output_signature: u32,
    pub effect_mask: u32,
    pub operations: alloc::vec::Vec<OriginOperation>,
}

#[derive(Clone, Copy, Debug)]
pub enum OriginAction {
    I64(i64),
    Add,
    Sub,
    Mul,
    Dup,
    Swap,
    Drop,
    Dip,
    Run,
    StaticProgram(usize),
    Quote {
        input: u32,
        output: u32,
        witness: u32,
        program_operand: bool,
    },
    Compose,
    CallSelected(usize),
}

#[derive(Clone, Debug)]
pub struct OriginOperation {
    pub node: noble_kernel::untrusted::NodeId,
    /// Kernel/instruction indices are compacted; this is the separate exact
    /// parsed-and-lowered lexical occurrence, absent for selection wrapper.
    pub lexical_node: Option<u32>,
    /// Only authored source tokens have a byte span. Synthetic cleanup,
    /// projection, quote, and dip operations carry a replay witness instead.
    pub authored_span: Option<noble_contracts::Span>,
    pub generated: bool,
    pub generated_witness: Option<noble_contracts::source::proof::CheckedGeneratedWitness>,
    pub ordinal: u32,
    pub table_entry: u32,
    pub input_signature: u32,
    pub output_signature: u32,
    pub effect_mask: u32,
    pub action: OriginAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginI64 {
    RootInput(u32),
    Fixed {
        value: i64,
        recipe: alloc::rc::Rc<OriginFixedExpression>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginFixedArithmetic {
    Add,
    Sub,
    Mul,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginFixedExpression {
    Literal(i64),
    Arithmetic {
        kind: OriginFixedArithmetic,
        left: alloc::rc::Rc<Self>,
        right: alloc::rc::Rc<Self>,
        nodes: usize,
        depth: usize,
    },
}

#[derive(Clone, Debug)]
pub enum OriginProgramValue {
    Static {
        node: noble_kernel::untrusted::NodeId,
        program_index: usize,
    },
    Quote {
        node: noble_kernel::untrusted::NodeId,
        table_entry: u32,
        operand: OriginOperand,
    },
    Compose {
        node: noble_kernel::untrusted::NodeId,
        table_entry: u32,
        left: alloc::rc::Rc<Self>,
        right: alloc::rc::Rc<Self>,
    },
}

#[derive(Clone, Debug)]
pub enum OriginOperand {
    I64(OriginI64),
    Program(alloc::rc::Rc<OriginProgramValue>),
}

#[derive(Clone, Debug)]
pub struct OriginOutput {
    pub stack_position: usize,
    pub result: alloc::rc::Rc<OriginProgramValue>,
}

/// An independently rechecked, emitted named definition, not the submission's
/// wrapper root. A source receipt must separately bind the exact retained
/// definition identity/owner and source origin; runtime installation must
/// confirm the actual `$sN` cell and the exact assembled artifact bytes.
#[derive(Clone, Debug)]
pub struct NamedTargetMetadata {
    pub definition: noble_kernel::contracts::Definition,
    pub definition_identity: u64,
    pub program_index: usize,
    pub entry: u32,
    pub input_signature: u32,
    pub output_signature: u32,
    pub effect_mask: u32,
    pub stack_in: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub stack_out: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub effects: noble_kernel::types::EffSet,
    /// Canonical checked definition recipe, compared in full; a session-local
    /// numeric identity or a digest alone cannot authorize a different body.
    pub checked_recipe: alloc::vec::Vec<u8>,
}

/// Source-selected, independently checked named value in the exact WAT
/// produced by `prepare_checked_selected`. This is not a Lean receipt or an
/// installed ProgramValueId: the host must separately bind exact source and
/// WAT/binary SHA256 and inspect the actual installed Program/capture graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedTargetArtifact {
    pub definition_id: noble_contracts::source::proof::CheckedDefinitionId,
    pub source_generation: u64,
    pub named_program_index: usize,
}

/// Exact admitted nominal/resource descriptor, not its host provenance.
#[derive(Clone, Debug)]
pub struct ResourceCatalogEntry {
    pub declaration: noble_kernel::contracts::NominalDecl,
    pub kind: noble_kernel::types::ResourceKind,
}

#[derive(Clone, Copy, Debug)]
struct FreeCode {
    start: u32,
    slots: u32,
}

#[derive(Clone, Debug)]
struct Identity {
    id: u64,
    recipe: alloc::vec::Vec<u8>,
}

/// Complete checked WAT plus a private prospective compiler state.
pub struct Prepared {
    base: transaction::Base,
    base_live_slots: bool,
    base_retention_revision: u64,
    next: Compiler,
    code_span: CodeSpan,
    live_sites: alloc::vec::Vec<LiveSiteMetadata>,
    target_metadata: Option<TargetMetadata>,
    program_metadata: alloc::vec::Vec<ProgramMetadata>,
    named_targets: alloc::vec::Vec<NamedTargetMetadata>,
    selected_target: Option<SelectedTargetArtifact>,
    selected_origin: Option<SelectedOriginMetadata>,
    resource_catalog: alloc::vec::Vec<ResourceCatalogEntry>,
    wat: alloc::vec::Vec<u8>,
    metrics: CompileMetrics,
    /// Set only for a calculator Program-free preparation made at a
    /// canonical epoch; see `reuse`.
    epoch: Option<CanonicalEpoch>,
    work_issued: u64,
}

impl Prepared {
    /// The complete module. Instantiation does not execute its body.
    pub fn wat(&self) -> &[u8] {
        &self.wat
    }

    /// Observed accounting of this preparation; see [`CompileMetrics`].
    pub const fn metrics(&self) -> CompileMetrics {
        self.metrics
    }

    /// Source-free Core16 helper code over this checked prospective signature
    /// catalogue. It has no submission, active segment or host-effect import.
    pub fn runtime_bridge_wat(&self) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
        if !(self.next.live || self.next.declared_live) || self.next.live_slots {
            return Err(crate::Diagnostic::Invalid);
        }
        emit::runtime_bridge(&self.next.descriptors, self.next.declared_live)
    }

    /// Ordered id -> address, length, exact UTF-8 signature bytes for the
    /// shared arena. These bytes come only from this prospective compiler.
    pub fn runtime_signature_catalog(&self) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
        if !(self.next.live || self.next.declared_live)
            || self.next.descriptors.len() != self.next.signatures.keys.len()
            || self.next.descriptors.len() > crate::signatures::SOURCE_KEY_LIMIT
        {
            return Err(crate::Diagnostic::Invalid);
        }
        let mut bytes = alloc::vec::Vec::new();
        let count =
            u32::try_from(self.next.descriptors.len()).map_err(|_| crate::Diagnostic::Exhausted)?;
        bytes.extend_from_slice(&count.to_le_bytes());
        for ((address, length), signature) in
            self.next.descriptors.iter().zip(&self.next.signatures.keys)
        {
            if usize::try_from(*length).ok() != Some(signature.len())
                || *address < TEXT_START
                || address
                    .checked_add(*length)
                    .is_none_or(|end| end > MEMORY_BYTES)
            {
                return Err(crate::Diagnostic::Defective);
            }
            bytes.extend_from_slice(&address.to_le_bytes());
            bytes.extend_from_slice(&length.to_le_bytes());
            bytes.extend_from_slice(signature);
        }
        Ok(bytes)
    }

    /// The exact table span installed by this module's successful submission.
    /// An empty span installs no code and must not enter `retire_code`; its
    /// originating guest instance still needs separate module-root release.
    pub const fn code_span(&self) -> CodeSpan {
        self.code_span
    }

    /// End of the prospective static literal arena.
    pub const fn literal_end(&self) -> u32 {
        self.next.text_end
    }

    /// Worker modules expose their checked literal arena only in that profile.
    pub const fn worker_literal_end(&self) -> Option<u32> {
        if self.next.worker_inputs {
            Some(self.next.text_end)
        } else {
            None
        }
    }

    /// Checked site contracts bound to the exact bytes returned by `wat()`.
    pub fn live_sites(&self) -> &[LiveSiteMetadata] {
        &self.live_sites
    }

    /// Full ordered logical root interface, including borrowed inputs.
    pub const fn target_metadata(&self) -> Option<&TargetMetadata> {
        self.target_metadata.as_ref()
    }

    /// Every exact Program global, including saved quotation children whose
    /// interface differs from the module's entry wrapper.
    pub fn program_metadata(&self) -> &[ProgramMetadata] {
        &self.program_metadata
    }

    /// Exact named Program globals compiled from independently accepted bodies.
    /// A selected definition is `module_program_handle(program_index)`, never
    /// the wrapper handle returned by `install_target()`.
    pub fn named_targets(&self) -> &[NamedTargetMetadata] {
        &self.named_targets
    }

    /// The exact selected named value, if this preparation consumed a
    /// source-session checked selection rather than raw submission metadata.
    pub const fn selected_target(&self) -> Option<SelectedTargetArtifact> {
        self.selected_target
    }

    /// Only a complete, unambiguous checked linear lineage is exposed. A
    /// missing origin never changes ordinary Program execution or retention.
    pub const fn selected_origin(&self) -> Option<&SelectedOriginMetadata> {
        self.selected_origin.as_ref()
    }

    /// Compare these complete admitted descriptors against the independently
    /// registered host catalog before instantiating an opt-in module.
    pub fn resource_catalog(&self) -> &[ResourceCatalogEntry] {
        &self.resource_catalog
    }
}

impl Compiler {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; reserve_code updates only the unpublished prospective compiler table and preserves every refusal as a diagnostic."
    )]
    fn reserve_code(&mut self, slots: u32) -> Result<CodeSpan, crate::Diagnostic> {
        if slots == 0 {
            return Ok(CodeSpan {
                start: self.functions,
                slots: 0,
                generation: self.generation,
            });
        }
        let mut start = self.functions;
        if self.live_slots {
            if let Some(index) = self.free_code.iter().position(|free| free.slots >= slots) {
                let free = &mut self.free_code[index];
                start = free.start;
                free.start += slots;
                free.slots -= slots;
                if free.slots == 0 {
                    self.free_code.remove(index);
                }
            } else {
                self.functions = match self.functions.checked_add(slots) {
                    Some(end) if end <= TABLE_LIMIT => end,
                    Some(_) | None => return Err(crate::Diagnostic::Exhausted),
                };
            }
        } else {
            self.functions = match self.functions.checked_add(slots) {
                Some(end) if end <= TABLE_LIMIT => end,
                Some(_) | None => return Err(crate::Diagnostic::Exhausted),
            };
        }
        let span = CodeSpan {
            start,
            slots,
            generation: self.generation,
        };
        if self.live || self.declared_live || self.live_slots {
            self.installed_code.push(span);
        }
        Ok(span)
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; signature interning updates only the unpublished prospective compiler and its preparation-owned meter; caller type stacks remain immutable."
    )]
    fn signature(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        work: &mut Work,
    ) -> Result<u32, crate::Diagnostic> {
        let bytes = attempt!(admission::meter::stack(stack, work));
        attempt!(work.charge(bytes));
        if self.calculator {
            return self
                .signatures
                .intern_charged(stack, |visited| work.entries(visited));
        }
        let mut index = 0usize;
        let mut failure = None;
        while index < self.signatures.keys.len() {
            match work.entries(self.signatures.keys[index].len().saturating_add(1)) {
                Ok(()) => index += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        self.signatures.intern(stack)
    }

    /// A fresh compiler requires a fresh runtime import-object set.
    pub fn new() -> Self {
        Self {
            live: false,
            declared_live: false,
            live_slots: false,
            text_cursor: false,
            declared_text: false,
            calculator: false,
            worker_inputs: false,
            generation: 0,
            functions: 4,
            retention_revision: 0,
            free_code: alloc::vec::Vec::new(),
            installed_code: alloc::vec::Vec::new(),
            text_end: TEXT_START,
            static_literals: alloc::collections::BTreeMap::new(),
            signatures: crate::signatures::Pool::source(),
            descriptors: alloc::vec::Vec::new(),
            identities: alloc::vec::Vec::new(),
        }
    }

    /// The Core 16-page live profile has its own proposal/generation ingress
    /// and root-checked code span ledger. Neither ordinary Core nor the
    /// separate 20-page live-slot backend acquires these root exports.
    pub fn new_live() -> Self {
        Self {
            live: true,
            ..Self::new()
        }
    }

    /// The opt-in declared live worker uses the checked 16-page Program owner
    /// graph without acquiring Core's self-edit effect/profile.
    pub fn new_declared_live() -> Self {
        Self {
            declared_live: true,
            ..Self::new()
        }
    }

    /// Opt in to checked live references and generation-safe physical reuse.
    pub fn new_live_slots() -> Self {
        Self {
            live_slots: true,
            ..Self::new()
        }
    }

    /// The pure byte cursor is a separate compiler admission profile.
    pub fn new_text_cursor() -> Self {
        Self {
            text_cursor: true,
            ..Self::new()
        }
    }

    /// Distinct persistent code/identity pool for pure declared text modules.
    pub fn new_declared_text() -> Self {
        Self {
            text_cursor: true,
            declared_text: true,
            ..Self::new()
        }
    }

    /// Independent calculator admission with one reserved trampoline slot.
    pub fn new_calculator() -> Self {
        let mut compiler = Self::new();
        compiler.calculator = true;
        compiler.functions = CALCULATOR_FUNCTION_BASE;
        compiler.text_end = CALCULATOR_TEXT_START;
        compiler.signatures.select_calculator();
        compiler
    }

    /// Opt-in pure declared-module ingress with host-owned value constructors.
    pub fn new_worker() -> Self {
        Self {
            worker_inputs: true,
            ..Self::new()
        }
    }

    /// Independently accept and lower without changing existing code identities.
    pub fn prepare(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        // The merged cursor/declared profile has no raw Submission ingress.
        // A declared live compiler never accepts an ordinary submission.
        if self.declared_text || (self.declared_live && !submission.environment.declared_modules) {
            return Err(crate::Diagnostic::Unsupported);
        }
        self.prepare_from(submission, None, self.functions, false, false, false)
    }

    /// Rebuild a signed staged-source snapshot before authorizing `SignedSelf`.
    /// A public Submission or Env cannot supply the owner instance witness.
    #[expect(
        tigerstyle::compound_condition,
        reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
    )]
    pub fn prepare_signed(
        &self,
        source: &noble_contracts::source::SignedSourceOwner<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if self.live
            || self.live_slots
            || self.text_cursor
            || self.declared_text
            || self.calculator
            || self.worker_inputs
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        let submission = source.rederive().map_err(|_| crate::Diagnostic::Invalid)?;
        if !submission.environment.declared_modules
            || submission.environment.live_slots
            || submission.environment.text_cursor
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        self.prepare_from(&submission, None, self.functions, false, false, true)
    }

    /// Worker ingress for a pure Program with signed, scoped recursion. Like
    /// `prepare_signed`, it compiles only a rederivation from the private
    /// staged source after each signed owner is authenticated; no public
    /// Submission can authorize `SignedSelf` in a Worker module.
    #[expect(
        tigerstyle::compound_condition,
        reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
    )]
    pub fn prepare_worker_signed(
        &self,
        source: &noble_contracts::source::SignedSourceOwner<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.worker_inputs
            || self.live
            || self.live_slots
            || self.text_cursor
            || self.declared_text
            || self.calculator
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        let submission = source.rederive().map_err(|_| crate::Diagnostic::Invalid)?;
        self.prepare_from(&submission, None, self.functions, false, false, true)
    }

    /// The merged cursor/declared profile has no raw Submission ingress.
    /// Rebuild from the contracts-private source snapshot, including the
    /// signed owner and all quotations, before the fixed-prefix admission audit.
    #[expect(
        tigerstyle::compound_condition,
        reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
    )]
    pub fn prepare_declared_text(
        &self,
        source: &noble_contracts::source::DeclaredTextSourceOwner<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.declared_text
            || !self.text_cursor
            || self.live
            || self.declared_live
            || self.live_slots
            || self.calculator
            || self.worker_inputs
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        let submission = source.rederive().map_err(|_| crate::Diagnostic::Invalid)?;
        if !submission.environment.declared_modules || !submission.environment.text_cursor {
            return Err(crate::Diagnostic::Unsupported);
        }
        let is_signed = submission.definitions.iter().any(|definition| {
            definition
                .body
                .candidate
                .nodes
                .iter()
                .any(|node| matches!(node, noble_kernel::untrusted::Node::SignedSelf { .. }))
        });
        self.prepare_from(&submission, None, self.functions, false, false, is_signed)
    }

    /// Calculator-only preparation at a quiescent submission boundary: the
    /// runtime reported that no Program reachable from the operand stack runs
    /// compiled code. Every module re-lowers its own definition closure, so
    /// all committed compiled-code slots are then dead and this module's
    /// table initialization overwrites them from the fixed base. The
    /// definition-identity ledger likewise restarts empty: identities are
    /// session-stable ids checked against their recipes within one epoch, and
    /// no earlier epoch's compiled body can run again. Only the prospective
    /// state rewinds; commit publishes it, refusal changes nothing.
    pub fn prepare_quiescent(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.calculator {
            return Err(crate::Diagnostic::Invalid);
        }
        self.prepare_from(
            submission,
            None,
            CALCULATOR_FUNCTION_BASE,
            true,
            false,
            false,
        )
    }

    /// Calculator-only preparation at a Program-free submission boundary: the
    /// runtime reported that no Program, Syntax or recipe cell is reachable
    /// from the operand stack. Signature IDs are held only by those cells, so
    /// beyond the quiescent rewind the signature pool and its published
    /// descriptors restart empty too, and this module republishes its keys
    /// from ID 0. Static literal Text is never rewound.
    pub fn prepare_program_free(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.calculator {
            return Err(crate::Diagnostic::Invalid);
        }
        self.prepare_from(
            submission,
            None,
            CALCULATOR_FUNCTION_BASE,
            true,
            true,
            false,
        )
    }

    #[expect(
        tigerstyle::allocation_in_loop,
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-wasm-maintainers; each iteration's buffer is bounded by the checked input that drives the loop; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
    )]
    fn prepare_from(
        &self,
        submission: &noble_kernel::execution::Submission,
        selected: Option<&noble_contracts::source::proof::CheckedSelectedTarget<'_>>,
        functions: u32,
        quiescent: bool,
        program_free: bool,
        signed: bool,
    ) -> Result<Prepared, crate::Diagnostic> {
        if self.calculator
            && u64::from(submission.request.limits.work) > CALCULATOR_COMPILER_WORK_LIMIT
        {
            return Err(crate::Diagnostic::Exhausted);
        }
        let mut work = Work {
            remaining: u64::from(submission.request.limits.work),
            issued: u64::from(submission.request.limits.work),
            calculator_preflight: self.calculator,
            calculator_plan: false,
            max_type_nodes: 0,
        };
        attempt!(preflight::check(submission, self.calculator, &mut work));
        work.calculator_preflight = false;
        if self.worker_inputs
            && (!submission.environment.declared_modules
                || submission.environment.live_slots
                || submission.environment.text_cursor
                || self.live
                || self.live_slots
                || self.text_cursor
                || self.calculator)
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        let preflight_issued = work.issued;
        let checked = attempt!(admission::check(
            submission,
            self.live,
            self.live_slots,
            self.text_cursor,
            self.calculator,
            self.declared_text,
            signed,
            &mut work,
        ));
        let admission_issued = work.issued;
        // A Program-free epoch interns into a fresh calculator pool, so no
        // committed key is retained, copied or charged.
        let fresh_signatures = if program_free {
            let mut pool = crate::signatures::Pool::source();
            pool.select_calculator();
            Some(pool)
        } else {
            None
        };
        let signature_key_charge = attempt!(admission::meter::compilation(
            &checked,
            fresh_signatures.as_ref().unwrap_or(&self.signatures),
            &mut work
        ));
        // A quiescent epoch starts with an empty identity ledger: nothing of
        // the committed ledger is retained, so none of it is copied or charged.
        let retained = if quiescent { 0 } else { self.identities.len() };
        let mut identity = 0usize;
        let mut identity_charge = 0u64;
        let mut failure = None;
        while identity < retained {
            let entries = self.identities[identity].recipe.len();
            match work.entries(entries) {
                Ok(()) => {
                    identity_charge = identity_charge.saturating_add(entries as u64);
                    identity += 1;
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        // The literal table is copied into the prospective compiler.
        attempt!(work.entries(
            self.static_literals
                .keys()
                .fold(0usize, |total, key| total.saturating_add(key.len() + 1))
        ));
        let mut next = Self {
            live: self.live,
            declared_live: self.declared_live,
            live_slots: self.live_slots,
            text_cursor: self.text_cursor,
            declared_text: self.declared_text,
            calculator: self.calculator,
            worker_inputs: self.worker_inputs,
            generation: match self.generation.checked_add(1) {
                Some(value) => value,
                None => return Err(crate::Diagnostic::Exhausted),
            },
            functions,
            retention_revision: self.retention_revision,
            free_code: self.free_code.clone(),
            installed_code: self.installed_code.clone(),
            text_end: self.text_end,
            static_literals: self.static_literals.clone(),
            signatures: match fresh_signatures {
                Some(pool) => pool,
                None => self.signatures.clone(),
            },
            descriptors: if program_free {
                alloc::vec::Vec::new()
            } else {
                self.descriptors.clone()
            },
            identities: if quiescent {
                alloc::vec::Vec::new()
            } else {
                self.identities.clone()
            },
        };
        attempt!(admission::identity::check(
            submission,
            &mut next.identities,
            next.calculator,
            &mut work,
            signed,
        ));
        work.calculator_plan = self.calculator;
        let mut plan = attempt!(plan::lower(submission, &checked, &mut next, &mut work));
        // Worker literals share the scratch-region boundary used by calculator
        // signature expansion; neither profile may let static Text overlap it.
        if self.worker_inputs && next.text_end > CALCULATOR_TEXT_END {
            return Err(crate::Diagnostic::Exhausted);
        }
        // A calculator module publishes only the signatures new since the
        // committed compiler's descriptor table (all of its own after a
        // Program-free rewind); the trusted engine retains that published
        // prefix, so per-module metadata is history-independent.
        let published = if program_free {
            0
        } else {
            self.descriptors.len()
        };
        let wat = if next.calculator {
            let keys = attempt!(next
                .signatures
                .expanded_keys(published, |bytes| work.entries(bytes)));
            attempt!(emit::module(&plan, self.generation, published, &keys))
        } else {
            attempt!(emit::module(
                &plan,
                self.generation,
                0,
                &next.signatures.keys
            ))
        };
        let code_span = CodeSpan {
            start: plan.first_function,
            slots: plan.functions - plan.first_function,
            generation: next.generation,
        };
        let target_metadata = if self.live_slots || self.worker_inputs {
            let root = attempt!(plan
                .programs
                .get(plan.root)
                .ok_or(crate::Diagnostic::Invalid));
            Some(TargetMetadata {
                root_program_index: plan.root,
                input_signature: root.input,
                output_signature: root.output,
                effect_mask: root.effects,
                stack_in: submission.request.expected.stack_in.clone(),
                stack_out: submission.request.expected.stack_out.clone(),
                effects: checked.root.interface.effects.clone(),
            })
        } else {
            None
        };
        let mut named_targets = alloc::vec::Vec::new();
        if self.live_slots || self.worker_inputs {
            for (index, definition) in submission.definitions.iter().enumerate() {
                let checked_body = attempt!(checked
                    .definitions
                    .get(index)
                    .ok_or(crate::Diagnostic::Invalid));
                let program_index = attempt!(plan
                    .definition_roots
                    .get(index)
                    .copied()
                    .ok_or(crate::Diagnostic::Invalid));
                let program = attempt!(plan
                    .programs
                    .get(program_index)
                    .ok_or(crate::Diagnostic::Invalid));
                let recipe = attempt!(next
                    .identities
                    .iter()
                    .find(|identity| identity.id == definition.identity)
                    .ok_or(crate::Diagnostic::Invalid));
                named_targets.push(NamedTargetMetadata {
                    definition: definition.definition,
                    definition_identity: definition.identity,
                    program_index,
                    entry: program.entry,
                    input_signature: program.input,
                    output_signature: program.output,
                    effect_mask: program.effects,
                    stack_in: checked_body.interface.stack_in.clone(),
                    stack_out: checked_body.interface.stack_out.clone(),
                    effects: checked_body.interface.effects.clone(),
                    checked_recipe: recipe.recipe.clone(),
                });
            }
        }
        let selected_origin = selected.and_then(|selected| origin::build(selected, &plan));
        let mut program_metadata = alloc::vec::Vec::new();
        if self.live_slots || self.worker_inputs {
            program_metadata.reserve(plan.programs.len());
            let text_witness = plan.text_witness;
            for (program_index, program) in plan.programs.iter_mut().enumerate() {
                let logical = attempt!(program.logical.take().ok_or(crate::Diagnostic::Defective));
                let runtime_atoms = if self.worker_inputs {
                    program
                        .operations
                        .iter()
                        .map(|operation| plan::atom_metadata(operation, text_witness))
                        .collect()
                } else {
                    alloc::vec::Vec::new()
                };
                program_metadata.push(ProgramMetadata {
                    program_index,
                    entry: program.entry,
                    input_signature: program.input,
                    output_signature: program.output,
                    effect_mask: program.effects,
                    stack_in: logical.stack_in,
                    stack_out: logical.stack_out,
                    effects: logical.effects,
                    capture_left: 0,
                    capture_right: 0,
                    recipe_depth: program.depth,
                    recipe_leaves: program.leaves,
                    runtime_atoms,
                });
            }
        }
        let mut resource_catalog = alloc::vec::Vec::new();
        if self.live_slots {
            for id in &submission.environment.live_resource_nominals {
                let declaration = attempt!(submission
                    .environment
                    .nominal(*id)
                    .ok_or(crate::Diagnostic::Invalid));
                let noble_kernel::types::NominalShape::Opaque(value) = &declaration.shape else {
                    return Err(crate::Diagnostic::Invalid);
                };
                let noble_kernel::types::Ty::Resource(kind) = value.as_ref() else {
                    return Err(crate::Diagnostic::Invalid);
                };
                resource_catalog.push(ResourceCatalogEntry {
                    declaration: declaration.clone(),
                    kind: *kind,
                });
            }
        }
        let named_bodies = submission.definitions.len() as u64;
        let work_environment_grant = admission_issued - preflight_issued;
        let metrics = CompileMetrics {
            work_initial: u64::from(submission.request.limits.work),
            work_preflight_grant: preflight_issued - u64::from(submission.request.limits.work),
            work_environment_grant,
            work_plan_grant: work.issued - admission_issued,
            work_issued: work.issued,
            work_spent: work.issued - work.remaining,
            environment_rows: submission.environment.defs.len() as u64,
            // The allowance is exactly `environment_work × (named + 3)`.
            #[expect(
                tigerstyle::unchecked_division,
                reason = "Owner: noble-wasm-maintainers; the divisor is named_bodies.saturating_add(3), so it is never zero."
            )]
            environment_work: work_environment_grant / named_bodies.saturating_add(3),
            named_bodies,
            max_type_nodes: work.max_type_nodes,
            type_nodes_cap: submission.request.limits.type_size,
            signature_keys: next.signatures.len() as u64,
            signature_key_charge,
            identity_charge,
            table_end: plan.functions,
            text_end: next.text_end,
            wat_bytes: wat.len() as u64,
        };
        Ok(Prepared {
            base: transaction::Base::capture(
                self,
                retained,
                if program_free {
                    0
                } else {
                    self.signatures.len()
                },
                published,
            ),
            base_live_slots: self.live_slots,
            base_retention_revision: self.retention_revision,
            next,
            code_span,
            live_sites: plan.live_sites,
            target_metadata,
            program_metadata,
            named_targets,
            selected_target: None,
            selected_origin,
            resource_catalog,
            wat,
            metrics,
            epoch: if program_free {
                self.canonical_epoch()
            } else {
                None
            },
            work_issued: work.issued,
        })
    }

    /// Recompile the exact independently accepted source-selected target, and
    /// bind its retained definition ID to the emitted *definition* Program,
    /// not the root invocation wrapper. No arbitrary caller-supplied identity
    /// or previously compiled artifact can be substituted through this API.
    /// The host still authenticates its pinned checker and assembled bytes.
    pub fn prepare_checked_selected(
        &self,
        selected: &noble_contracts::source::proof::CheckedSelectedTarget<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.live_slots {
            return Err(crate::Diagnostic::Unsupported);
        }
        let mut prepared = attempt!(self.prepare_from(
            selected.submission(),
            Some(selected),
            self.functions,
            false,
            false,
            false,
        ));
        let id = selected.definition_id();
        let mut matches = prepared.named_targets.iter().filter(|named| {
            named.definition == selected.definition().definition
                && named.definition_identity == id.identity()
        });
        let Some(named) = matches.next() else {
            return Err(crate::Diagnostic::Invalid);
        };
        let has_interface_mismatch = matches.next().is_some()
            || named.stack_in != selected.body().interface.stack_in
            || named.stack_out != selected.body().interface.stack_out
            || named.effects != selected.body().interface.effects
            || !prepared.target_metadata.as_ref().is_some_and(|root| {
                root.stack_in == selected.root().interface.stack_in
                    && root.stack_out == selected.root().interface.stack_out
                    && root.effects == selected.root().interface.effects
            });
        if has_interface_mismatch {
            return Err(crate::Diagnostic::Invalid);
        }
        prepared.selected_target = Some(SelectedTargetArtifact {
            definition_id: id,
            source_generation: selected.source_generation(),
            named_program_index: named.program_index,
        });
        Ok(prepared)
    }

    /// Check publication provenance before allowing a live worker to install
    /// prospective bytes in a persistent arena. No compiler state is changed.
    pub fn can_commit(&self, prepared: &Prepared) -> bool {
        prepared.base_live_slots == self.live_slots
            && prepared.base_live_slots == prepared.next.live_slots
            && prepared.base_retention_revision == self.retention_revision
            && prepared.base.matches(self, &prepared.next)
    }

    /// Publish only after the shell has accepted/instantiated the complete module.
    /// A stale or different base allocation state is rejected without mutation.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; commit is the explicit compiler-state transition: it consumes the owned preparation, checks generation and immutable allocation provenance, then publishes only the matching prospective state."
    )]
    pub fn commit(&mut self, prepared: Prepared) -> Result<(), crate::Diagnostic> {
        if !self.can_commit(&prepared) {
            return Err(crate::Diagnostic::Invalid);
        }
        *self = prepared.next;
        Ok(())
    }

    /// The selected worker has verified that its idle signed/Text arena has
    /// no external or saved roots, cleared every source code entry and
    /// released every module global before tombstoning the complete heap.
    /// Source identities and generation remain monotone across this epoch.
    #[expect(
        tigerstyle::compound_condition,
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    pub fn retire_declared_text_epoch(
        &mut self,
        generation: u32,
        code_end: u32,
    ) -> Result<(), crate::Diagnostic> {
        if !self.declared_text
            || !self.text_cursor
            || self.live
            || self.declared_live
            || self.live_slots
            || self.calculator
            || self.worker_inputs
            || self.generation != generation
            || self.functions != code_end
            || self.retention_revision == u64::MAX
            || !self.free_code.is_empty()
            || !self.installed_code.is_empty()
            || !self.static_literals.is_empty()
        {
            return Err(crate::Diagnostic::Invalid);
        }
        self.functions = 4;
        self.text_end = TEXT_START;
        self.signatures = crate::signatures::Pool::source();
        self.descriptors.clear();
        self.retention_revision += 1;
        Ok(())
    }

    /// Current (generation, code end) of an exclusively declared-text compiler.
    pub fn declared_text_epoch_state(&self) -> Option<(u32, u32)> {
        (self.declared_text
            && self.text_cursor
            && !self.live
            && !self.declared_live
            && !self.live_slots
            && !self.calculator
            && !self.worker_inputs)
            .then_some((self.generation, self.functions))
    }

    /// Retire an installed live-slot or Core-live allocation only after the
    /// trusted runtime verifies zero backend Program owners and clears every
    /// entry of its physical shared table span. The callback must query the
    /// backend's `code_owners(start, length)` (negative means refusal), verify
    /// exact installed entries, invalidate Core-live stale Program handles,
    /// and clear the physical table span before returning success.
    /// Core-live saved stack/host roots, observed values, and adapter leases
    /// remain owners until their explicit release; an unknown host inventory
    /// must refuse retirement rather than report a zero count.
    /// Validation precedes the callback, so stale or partial spans cannot
    /// cause table side effects.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; successful retirement explicitly clears trusted runtime table entries and publishes a revised compiler allocation state."
    )]
    pub fn retire_code(
        &mut self,
        span: CodeSpan,
        verify_and_clear: impl FnOnce(CodeSpan) -> Result<(), crate::Diagnostic>,
    ) -> Result<(), crate::Diagnostic> {
        if !(self.live || self.declared_live || self.live_slots)
            || span.slots == 0
            || span.start < 4
        {
            return Err(crate::Diagnostic::Invalid);
        }
        let Some(index) = self.installed_code.iter().position(|owned| *owned == span) else {
            return Err(crate::Diagnostic::Invalid);
        };
        let Some(end) = span.start.checked_add(span.slots) else {
            return Err(crate::Diagnostic::Invalid);
        };
        if end > self.functions || self.retention_revision == u64::MAX {
            return Err(crate::Diagnostic::Invalid);
        }
        attempt!(verify_and_clear(span));
        self.installed_code.remove(index);
        let mut index = self
            .free_code
            .partition_point(|free| free.start < span.start);
        self.free_code.insert(
            index,
            FreeCode {
                start: span.start,
                slots: span.slots,
            },
        );
        if index > 0
            && self.free_code[index - 1].start + self.free_code[index - 1].slots
                == self.free_code[index].start
        {
            let right = self.free_code.remove(index);
            self.free_code[index - 1].slots += right.slots;
            index -= 1;
        }
        if index + 1 < self.free_code.len()
            && self.free_code[index].start + self.free_code[index].slots
                == self.free_code[index + 1].start
        {
            let right = self.free_code.remove(index + 1);
            self.free_code[index].slots += right.slots;
        }
        if self.free_code[index].start + self.free_code[index].slots == self.functions {
            self.functions = self.free_code[index].start;
            self.free_code.remove(index);
        }
        self.retention_revision += 1;
        Ok(())
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod retention_tests {
    #![allow(
        tigerstyle::bool_naming,
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-wasm-maintainers; these retention fixtures track terse flag names and import the compiler internals they exercise, so a policy prefix or qualified path would obscure the scenario."
    )]

    use super::{CodeSpan, Compiler, TABLE_LIMIT};
    use crate::Diagnostic;

    #[test]
    fn retired_spans_reuse_physical_table_without_invalidating_other_owners() {
        let mut compiler = Compiler::new_live_slots();
        compiler.generation = 1;
        let former = compiler.reserve_code(3).ok().expect("first span");
        compiler.generation = 2;
        let pinned = compiler.reserve_code(2).ok().expect("pinned span");
        assert_eq!((former.start, pinned.start), (4, 7));

        assert!(compiler.retire_code(former, |_| Ok(())).is_ok());
        compiler.generation = 3;
        let reused = compiler.reserve_code(2).ok().expect("reused span");
        assert_eq!((reused.start, reused.slots), (4, 2));
        assert_ne!(reused.generation, former.generation);
        assert!(matches!(
            compiler.retire_code(former, |_| Ok(())),
            Err(Diagnostic::Invalid)
        ));
        assert!(matches!(
            compiler.retire_code(CodeSpan { slots: 1, ..pinned }, |_| Ok(())),
            Err(Diagnostic::Invalid)
        ));
        compiler.generation = 4;
        assert_eq!(
            compiler.reserve_code(1).ok().expect("last free slot").start,
            6
        );
        assert_eq!(compiler.functions, 9);
        assert!(compiler.retire_code(pinned, |_| Ok(())).is_ok());
        assert_eq!(compiler.functions, 7);
        assert_eq!(compiler.installed_code.len(), 2);
    }

    #[test]
    fn contiguous_table_requirement_refuses_before_reserving_any_span() {
        let mut compiler = Compiler::new_live_slots();
        compiler.functions = TABLE_LIMIT - 1;
        let installed = compiler.reserve_code(1).ok().expect("table limit");
        assert!(matches!(
            compiler.reserve_code(1),
            Err(Diagnostic::Exhausted)
        ));
        assert_eq!(compiler.functions, TABLE_LIMIT);
        assert_eq!(compiler.installed_code.len(), 1);
        assert!(compiler.retire_code(installed, |_| Ok(())).is_ok());
        assert_eq!(compiler.functions, TABLE_LIMIT - 1);
        assert_eq!(
            compiler.reserve_code(1).ok().expect("reclaimed slot").start,
            installed.start
        );
    }

    #[test]
    fn refusal_preserves_owners_and_never_reuses_uncleared_code() {
        let mut compiler = Compiler::new_live_slots();
        compiler.generation = 1;
        let occupied = compiler.reserve_code(2).ok().expect("installed code");
        let next = compiler.functions;
        assert!(matches!(
            compiler.retire_code(occupied, |_| Err(Diagnostic::Invalid)),
            Err(Diagnostic::Invalid)
        ));
        assert_eq!(compiler.functions, next);
        assert_eq!(compiler.installed_code.as_slice(), &[occupied]);
        compiler.generation = 2;
        assert_eq!(
            compiler.reserve_code(1).ok().expect("fresh span").start,
            next
        );

        let mut called = false;
        assert!(matches!(
            compiler.retire_code(
                CodeSpan {
                    generation: 0,
                    ..occupied
                },
                |_| {
                    called = true;
                    Ok(())
                }
            ),
            Err(Diagnostic::Invalid)
        ));
        assert!(!called);
        assert_eq!(compiler.installed_code[0], occupied);
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        tigerstyle::compound_assertion,
        tigerstyle::function_length,
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-wasm-maintainers; the epoch fixtures keep one long setup per scenario and assert both halves of the restarted boundary together; the compiler internals under test are imported directly."
    )]

    use super::{
        transaction, Compiler, Identity, Work, CALCULATOR_COMPILER_WORK_LIMIT,
        CALCULATOR_DEFINITION_LIMIT,
    };
    use noble_kernel::types::{ResourceKind, Ty};

    #[test]
    fn borrowed_calculator_preflight_preserves_caller_work_without_escaping_its_ceiling() {
        let mut work = Work {
            remaining: 7,
            issued: CALCULATOR_COMPILER_WORK_LIMIT - 8,
            calculator_preflight: true,
            calculator_plan: false,
            max_type_nodes: 0,
        };
        assert!(work.charge(7).is_ok());
        assert_eq!(work.remaining, 7);
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT - 1);
        assert!(work.charge(1).is_ok());
        assert_eq!(work.remaining, 7);
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT);
        assert!(matches!(work.charge(1), Err(crate::Diagnostic::Exhausted)));
        assert_eq!(work.remaining, 7);
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT);

        work.calculator_preflight = false;
        assert!(work.charge(7).is_ok());
        assert_eq!(work.remaining, 0);
        assert!(matches!(work.charge(1), Err(crate::Diagnostic::Exhausted)));
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT);

        let mut ordinary = Work {
            remaining: 7,
            issued: 7,
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        };
        assert!(matches!(
            ordinary.charge(8),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert_eq!(ordinary.issued, 7);
        assert_eq!(ordinary.remaining, 7);
    }

    #[test]
    fn calculator_quiescent_epochs_restart_the_identity_ledger() {
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let mut session = noble_contracts::source::Session::new_calculator();
        let defined = session
            .prepare(b"def identity_epoch_word [ 1 drop ]", &[], limits)
            .unwrap_or_else(|_| panic!("calculator definition refused"));
        session
            .commit(defined)
            .unwrap_or_else(|_| panic!("calculator definition commit refused"));
        let input = session
            .prepare(b"identity_epoch_word", &[], limits)
            .unwrap_or_else(|_| panic!("calculator call refused"));
        let submission = input
            .submission()
            .unwrap_or_else(|| panic!("missing checked source submission"));
        assert!(!submission.definitions.is_empty());
        // A committed ledger already holding the limit of unrelated identities,
        // as after many earlier modules of one long session.
        let full = |count: usize| {
            let mut compiler = Compiler::new_calculator();
            compiler.identities = (0..count as u64)
                .map(|index| Identity {
                    id: u64::MAX - index,
                    recipe: alloc::vec![0],
                })
                .collect();
            compiler
        };

        let cumulative = full(CALCULATOR_DEFINITION_LIMIT);
        assert!(
            matches!(
                cumulative.prepare(submission),
                Err(crate::Diagnostic::Exhausted)
            ),
            "a non-quiescent preparation still refuses a new identity at the limit"
        );
        assert_eq!(cumulative.identities.len(), CALCULATOR_DEFINITION_LIMIT);

        let mut epochs = full(CALCULATOR_DEFINITION_LIMIT);
        let stale = transaction::Base::capture(
            &epochs,
            epochs.identities.len(),
            epochs.signatures.len(),
            epochs.descriptors.len(),
        );
        let prepared = epochs
            .prepare_quiescent(submission)
            .unwrap_or_else(|_| panic!("quiescent preparation refused past the cumulative limit"));
        assert!(epochs.can_commit(&prepared));
        assert!(
            !stale.matches(&epochs, &prepared.next),
            "a whole-ledger base cannot vouch for the restarted epoch"
        );
        epochs
            .commit(prepared)
            .unwrap_or_else(|_| panic!("quiescent commit refused"));
        assert_eq!(
            epochs.identities.len(),
            submission.definitions.len(),
            "the new epoch holds only this module's identities"
        );

        // The limit is still enforced within an epoch: an ordinary preparation
        // retains the epoch's ledger and refuses once it is full again.
        let missing = CALCULATOR_DEFINITION_LIMIT - epochs.identities.len();
        epochs
            .identities
            .extend((0..missing as u64).map(|index| Identity {
                id: u64::MAX - index,
                recipe: alloc::vec![0],
            }));
        let kept = epochs
            .prepare(submission)
            .unwrap_or_else(|_| panic!("known identities need no new ledger entry"));
        assert_eq!(kept.next.identities.len(), CALCULATOR_DEFINITION_LIMIT);
        assert!(epochs.can_commit(&kept));
    }

    #[test]
    fn calculator_program_free_epochs_restart_the_signature_pool() {
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let mut session = noble_contracts::source::Session::new_calculator();
        let defined = session
            .prepare(b"def signature_epoch_word [ 1 drop ]", &[], limits)
            .unwrap_or_else(|_| panic!("calculator definition refused"));
        session
            .commit(defined)
            .unwrap_or_else(|_| panic!("calculator definition commit refused"));
        let input = session
            .prepare(b"signature_epoch_word", &[], limits)
            .unwrap_or_else(|_| panic!("calculator call refused"));
        let submission = input
            .submission()
            .unwrap_or_else(|| panic!("missing checked source submission"));
        // A committed pool already holding the key limit of unrelated
        // signatures, as after many earlier modules of one long session.
        let full = || {
            let mut compiler = Compiler::new_calculator();
            for index in 0..crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT as u32 {
                assert_eq!(
                    compiler
                        .signatures
                        .intern(&[Ty::Resource(ResourceKind(index))])
                        .ok(),
                    Some(index)
                );
            }
            compiler
        };

        let cumulative = full();
        assert!(
            matches!(
                cumulative.prepare(submission),
                Err(crate::Diagnostic::Exhausted)
            ),
            "an ordinary preparation still refuses a new key at the limit"
        );
        assert!(
            matches!(
                cumulative.prepare_quiescent(submission),
                Err(crate::Diagnostic::Exhausted)
            ),
            "a merely quiescent preparation keeps every committed key"
        );
        assert_eq!(
            cumulative.signatures.len(),
            crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT
        );

        let mut epochs = full();
        let stale = transaction::Base::capture(
            &epochs,
            epochs.identities.len(),
            epochs.signatures.len(),
            epochs.descriptors.len(),
        );
        let prepared = epochs.prepare_program_free(submission).unwrap_or_else(|_| {
            panic!("Program-free preparation refused past the cumulative limit")
        });
        assert!(epochs.can_commit(&prepared));
        assert!(
            !stale.matches(&epochs, &prepared.next),
            "a whole-pool base cannot vouch for the restarted epoch"
        );
        let fresh = prepared.next.signatures.len();
        assert!(
            fresh > 0 && fresh < 64,
            "the new epoch holds only this module's keys"
        );
        assert_eq!(prepared.next.descriptors.len(), fresh);
        epochs
            .commit(prepared)
            .unwrap_or_else(|_| panic!("Program-free commit refused"));
        assert_eq!(epochs.signatures.len(), fresh);

        // The key limit still applies within an epoch, and an ordinary
        // preparation keeps and checks the epoch's whole pool.
        let kept = epochs
            .prepare(submission)
            .unwrap_or_else(|_| panic!("known keys need no new pool entry"));
        assert_eq!(kept.next.signatures.len(), fresh);
        assert!(epochs.can_commit(&kept));
        assert!(
            matches!(
                Compiler::new().prepare_program_free(submission),
                Err(crate::Diagnostic::Invalid)
            ),
            "only the calculator profile may restart signatures"
        );
    }

    #[test]
    fn calculator_many_signatures_cannot_exceed_cumulative_plan_ceiling() {
        let mut compiler = Compiler::new_calculator();
        let mut work = Work {
            remaining: 0,
            issued: CALCULATOR_COMPILER_WORK_LIMIT - 1_000_000,
            calculator_preflight: false,
            calculator_plan: true,
            max_type_nodes: 0,
        };
        for entries in 1..=64 {
            assert!(
                compiler
                    .signature(&alloc::vec![Ty::I64; entries], &mut work)
                    .is_ok(),
                "finite distinct structural signatures fit the remaining ceiling"
            );
        }
        assert_eq!(compiler.signatures.len(), 64);
        assert!(work.issued < CALCULATOR_COMPILER_WORK_LIMIT);

        // A repeated first key compares once; scanning and reserving all 64
        // possible keys would falsely refuse this valid near-ceiling lookup.
        work.issued = CALCULATOR_COMPILER_WORK_LIMIT - 4_096;
        work.remaining = 0;
        assert_eq!(compiler.signature(&[Ty::I64], &mut work).ok(), Some(0));
        assert!(work.issued <= CALCULATOR_COMPILER_WORK_LIMIT);
        assert_eq!(compiler.signatures.len(), 64);

        // The next distinct key must compare against the existing pool. Even
        // though just one unit can be issued, a longer comparison must fail
        // before subtraction or mutation wraps the cumulative accounting.
        work.issued = CALCULATOR_COMPILER_WORK_LIMIT - 1;
        work.remaining = 0;
        assert!(matches!(
            compiler.signature(&alloc::vec![Ty::Bool; 64], &mut work),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT - 1);
        assert_eq!(work.remaining, 0);
        assert_eq!(compiler.signatures.len(), 64);
        assert!(matches!(
            work.charge(u64::MAX),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT - 1);
        assert_eq!(work.remaining, 0);
    }

    #[test]
    fn calculator_module_metadata_does_not_grow_with_retained_signature_history() {
        let session = noble_contracts::source::ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator session refused"));
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let input = session
            .prepare(b"", &[], limits)
            .unwrap_or_else(|_| panic!("empty calculator source refused"));
        let submission = input
            .submission()
            .unwrap_or_else(|| panic!("missing checked source submission"));
        let step = |compiler: &mut Compiler| {
            let prepared = compiler
                .prepare(submission)
                .unwrap_or_else(|_| panic!("calculator prepare refused"));
            let wat = prepared.wat().to_vec();
            compiler
                .commit(prepared)
                .unwrap_or_else(|_| panic!("calculator commit refused"));
            wat
        };
        let published = |wat: &[u8]| {
            wat.split(|byte| *byte == b'\n')
                .filter(|line| line.starts_with(b"(data $sig"))
                .count()
        };

        let mut fresh = Compiler::new_calculator();
        step(&mut fresh);
        let fresh_steady = step(&mut fresh);

        // About 1.4 MB of unrelated retained keys, comparable to a long
        // calculator session, are published once by the next module.
        let mut retained = Compiler::new_calculator();
        for index in 0..3_000u32 {
            let stack = alloc::vec![Ty::Resource(ResourceKind(index)); 64];
            assert_eq!(retained.signatures.intern(&stack).ok(), Some(index));
        }
        let publishing = step(&mut retained);
        assert!(published(&publishing) > 3_000);
        assert!(publishing.len() > 1_000_000);
        let retained_steady = step(&mut retained);
        assert_eq!(retained.signatures.len(), fresh.signatures.len() + 3_000);

        // Later modules carry no retained key, branch or segment: they differ
        // from a fresh session's only in the decimal published-range bounds.
        assert_eq!(published(&fresh_steady), 0);
        assert_eq!(published(&retained_steady), 0);
        assert!(
            retained_steady.len() <= fresh_steady.len() + 16,
            "steady module grew from {} to {} bytes with retained history",
            fresh_steady.len(),
            retained_steady.len()
        );
    }
}

// Test harness only: scenario-named tests over checked fixtures.
#[allow(
    tigerstyle::function_length,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; test harness: descriptive scenario names, fixture imports and long setup legs are the readable form."
)]
#[cfg(test)]
mod declared_text_ingress_tests {
    use super::{admission, emit, plan, preflight, Compiler, Work};
    use noble_contracts::source::ModuleSession;

    fn budget(limit: u32) -> Work {
        Work {
            remaining: u64::from(limit),
            issued: u64::from(limit),
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        }
    }

    #[test]
    fn signed_owner_mutations_fail_selected_admission() {
        let limits = noble_contracts::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let session = ModuleSession::new_declared_text(
            &[],
            true,
            noble_contracts::source::RetainedSourceLimit::new(65_536)
                .expect("valid legacy retained source cap"),
        )
        .expect("pure declared text");
        let result = session
            .prepare(
                include_bytes!("../../../noble-contracts/src/fixtures/result.noble"),
                &[],
                limits,
            )
            .expect("nominal Result declaration");
        let (session, accepted) = session.commit(result);
        accepted.expect("Result installed");
        let digit = session
            .prepare(
                include_bytes!("../../../../verification/calculator-precursor/decimal-digit.noble"),
                &[],
                limits,
            )
            .expect("ASCII digit helper");
        let (session, accepted) = session.commit(digit);
        accepted.expect("digit helper installed");
        let fold = session
            .prepare(
                include_bytes!("../../../../verification/calculator-precursor/decimal-fold.noble"),
                &[],
                noble_contracts::Limits {
                    nodes: 8_192,
                    ..limits
                },
            )
            .expect("signed digit fold within reduced source-node budget");
        let (session, accepted) = session.commit(fold);
        accepted.expect("fold installed");
        let prepared = session
            .prepare(b"\"123\" 0 0 decimal_fold@1.fold", &[], limits)
            .expect("checked Text cursor caller");
        let owner = prepared.declared_text_source_owner().expect("source token");
        let original = owner.rederive().expect("source rederivation");
        let compiler = Compiler::new_declared_text();
        let mut work = budget(original.request.limits.work);
        assert!(
            preflight::check(&original, false, &mut work).is_ok(),
            "whole-source preflight exhausted before declared-text admission"
        );
        let checked = admission::check(&original, false, false, true, false, true, true, &mut work)
            .ok()
            .expect("declared-text whole-submission admission exhausted");
        assert!(
            admission::meter::compilation(&checked, &compiler.signatures, &mut work).is_ok(),
            "checked derivation compilation meter exhausted"
        );
        let mut next = Compiler::new_declared_text();
        next.generation = 1;
        assert!(
            admission::identity::check(&original, &mut next.identities, false, &mut work, true)
                .is_ok(),
            "source identity checks exhausted"
        );
        let plan = plan::lower(&original, &checked, &mut next, &mut work)
            .ok()
            .expect("checked program lowering exhausted");
        assert!(
            emit::module(&plan, 0, 0, &next.signatures.keys).is_ok(),
            "source Wasm emission exhausted"
        );
        assert!(compiler.prepare_declared_text(&owner).is_ok());
        assert!(compiler
            .prepare_from(&original, None, compiler.functions, false, false, true)
            .is_ok());
        let signed_owner = prepared.signed_source_owner().expect("signed owner source");
        assert!(
            Compiler::new().prepare_signed(&signed_owner).is_err(),
            "ordinary signed compiler must not import the selected cursor"
        );

        let mut changed = original.clone();
        let definition = changed
            .definitions
            .iter_mut()
            .find(|definition| definition.owner_inst.is_some())
            .expect("signed fold specialization");
        definition
            .owner_inst
            .as_mut()
            .expect("rigid owner binder")
            .bindings[0] =
            noble_kernel::words::Binding::Stack(alloc::vec![noble_kernel::types::Ty::Bool]);
        assert!(
            compiler
                .prepare_from(&changed, None, compiler.functions, false, false, true)
                .is_err(),
            "fresh or foreign self Inst cannot dispatch to checked owner"
        );
    }

    #[test]
    fn declared_text_compiler_accepts_second_checked_expression_in_one_session() {
        let limits = noble_contracts::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let mut session = ModuleSession::new_declared_text(
            &[],
            true,
            noble_contracts::source::RetainedSourceLimit::new(65_536)
                .expect("valid legacy retained source cap"),
        )
        .expect("pure declared text");
        for module in [
            include_bytes!("../../../noble-contracts/src/fixtures/result.noble").as_slice(),
            include_bytes!("../../../../verification/calculator-precursor/decimal-digit.noble")
                .as_slice(),
            include_bytes!("../../../../verification/calculator-precursor/decimal-fold.noble")
                .as_slice(),
        ] {
            let prepared = session
                .prepare(module, &[], limits)
                .expect("checked module");
            let (next, accepted) = session.commit(prepared);
            accepted.expect("registered immutable module");
            session = next;
        }
        let mut compiler = Compiler::new_declared_text();
        let mut stack = alloc::vec::Vec::new();
        for source in [
            b"\"0\" 0 0 decimal_fold@1.fold".as_slice(),
            b"drop \"9\" 0 0 decimal_fold@1.fold".as_slice(),
            b"drop \"1a\" 0 0 decimal_fold@1.fold".as_slice(),
            b"drop \"12\" 0 0 decimal_fold@1.fold".as_slice(),
        ] {
            let prepared = session
                .prepare(source, &stack, limits)
                .expect("checked expression");
            stack = prepared.output().to_vec();
            let owner = prepared
                .declared_text_source_owner()
                .expect("source-bound declared text");
            let compiled = compiler
                .prepare_declared_text(&owner)
                .ok()
                .expect("one-session compiler exhausted valid fold");
            compiler
                .commit(compiled)
                .ok()
                .expect("matching prepared state");
            let (next, accepted) = session.commit(prepared);
            accepted.expect("checked expression installed");
            session = next;
        }
    }
}

// Test harness only: scenario-named tests over checked fixtures.
#[allow(
    tigerstyle::non_trait_imports,
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; test harness: descriptive scenario names, fixture imports and long setup legs are the readable form."
)]
#[cfg(test)]
mod signed_dispatch_tests {
    use super::Compiler;
    use crate::Diagnostic;
    use noble_contracts::source::ModuleSession;
    use noble_kernel::{
        types::{EffSet, Ty},
        untrusted::{Node, Outcome},
    };

    const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    };

    /// Signed ingress over an already rederived submission.
    fn signed(
        submission: &noble_kernel::execution::Submission,
    ) -> Result<super::Prepared, Diagnostic> {
        let compiler = Compiler::new();
        compiler.prepare_from(submission, None, compiler.functions, false, false, true)
    }

    fn prepared(
        module: &[u8],
        expression: &[u8],
        input: &[Ty],
    ) -> noble_kernel::execution::Submission {
        let session = ModuleSession::new_signed_self(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(65_536)
                .expect("valid legacy retained source cap"),
        )
        .expect("signed source");
        let staged = session
            .prepare(module, &[], LIMITS)
            .expect("original signed module");
        let (session, linked) = session.commit(staged);
        linked.expect("module link");
        let expression = session
            .prepare(expression, input, LIMITS)
            .expect("checked concrete owner expression");
        expression
            .signed_source_owner()
            .expect("sealed source owner")
            .rederive()
            .expect("source-bound concrete submission")
    }

    #[test]
    fn an_otherwise_kernel_valid_fresh_generic_type_call_cannot_dispatch_a_different_compiled_owner(
    ) {
        let mut submission = prepared(
            b"module binder@1 [ signature identity forall<S:stack,A:value> [ S A -- S A ! pure ] export identity def identity [ identity ] ]",
            b"binder@1.identity",
            &[Ty::I64],
        );
        assert!(signed(&submission).is_ok());
        let Some(Node::Invocation { inst, .. }) = submission.body.candidate.nodes.first_mut()
        else {
            panic!("missing root owner invocation");
        };
        *inst.bindings.get_mut(1).expect("rank-1 value binder") =
            noble_kernel::words::Binding::Value(Ty::Bool);
        submission.request.expected.stack_in = alloc::vec![Ty::Bool];
        submission.request.expected.stack_out = alloc::vec![Ty::Bool];
        assert!(
            matches!(
                noble_kernel::acceptance::check(
                    &submission.environment,
                    &submission.request,
                    &submission.body.candidate,
                ),
                Outcome::Accepted(_)
            ),
            "the alternate rank-1 call itself is well typed"
        );
        assert!(
            matches!(signed(&submission), Err(Diagnostic::Invalid)),
            "a valid generic Bool call cannot invoke the I64 body"
        );
    }

    #[test]
    fn an_otherwise_kernel_valid_fresh_generic_effect_call_cannot_dispatch_a_different_compiled_owner(
    ) {
        let mut submission = prepared(
            b"module binder@1 [ signature loop forall<S:stack,E:effect> [ S -- S ! E ] export loop def loop [ loop ] ]",
            b"binder@1.loop",
            &[],
        );
        assert!(signed(&submission).is_ok());
        let original = submission
            .definitions
            .iter()
            .find_map(|definition| definition.owner_inst.as_ref())
            .and_then(|inst| inst.bindings.get(1))
            .expect("source owner effect binder");
        let alternative = match original {
            noble_kernel::words::Binding::Effect(effect) if effect.as_slice().is_empty() => {
                EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT])
            }
            noble_kernel::words::Binding::Effect(_) => EffSet::empty(),
            _ => panic!("source owner's second binder is not an effect"),
        };
        let Some(Node::Invocation { inst, .. }) = submission.body.candidate.nodes.first_mut()
        else {
            panic!("missing root owner invocation");
        };
        *inst.bindings.get_mut(1).expect("rank-1 effect binder") =
            noble_kernel::words::Binding::Effect(alternative.clone());
        submission.request.expected.allowed_effects = alternative;
        assert!(
            matches!(
                noble_kernel::acceptance::check(
                    &submission.environment,
                    &submission.request,
                    &submission.body.candidate,
                ),
                Outcome::Accepted(_)
            ),
            "the alternate rank-1 effect call itself is well typed"
        );
        assert!(
            matches!(signed(&submission), Err(Diagnostic::Invalid)),
            "a valid generic alternate effect cannot invoke the owner's checked ceiling"
        );
    }
}
