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
        32
    } else if env.text_cursor && !env.declared_modules {
        27
    } else if !env.declared_modules && env.effects.contains(&noble_kernel::types::EffId(3)) {
        26
    } else if !env.declared_modules && env.effects.contains(&noble_kernel::types::EffId(1)) {
        24
    } else {
        23
    }
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
    live_slots: bool,
    text_cursor: bool,
    calculator: bool,
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
pub enum OriginSource {
    Definition,
    Selection,
}

#[derive(Clone, Debug)]
pub struct OriginProgram {
    pub source_artifact: OriginSource,
    pub program_index: usize,
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
    StaticProgram(usize),
    QuoteI64 {
        input: u32,
        output: u32,
        witness: u32,
    },
    Compose,
    CallSelected(usize),
}

#[derive(Clone, Debug)]
pub struct OriginOperation {
    pub node: noble_kernel::untrusted::NodeId,
    /// None exclusively for the independently checked selection wrapper.
    pub source_span: Option<noble_contracts::Span>,
    pub ordinal: u32,
    pub table_entry: u32,
    pub input_signature: u32,
    pub output_signature: u32,
    pub effect_mask: u32,
    pub action: OriginAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginI64 {
    RootInput(u32),
    Fixed(i64),
}

#[derive(Clone, Debug)]
pub enum OriginProgramValue {
    QuoteI64 {
        node: noble_kernel::untrusted::NodeId,
        table_entry: u32,
        operand: OriginI64,
    },
    Compose {
        node: noble_kernel::untrusted::NodeId,
        table_entry: u32,
        left: alloc::boxed::Box<Self>,
        right_node: noble_kernel::untrusted::NodeId,
        right_program_index: usize,
    },
}

#[derive(Clone, Debug)]
pub struct OriginOutput {
    pub stack_position: usize,
    pub result: OriginProgramValue,
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

    /// The exact table span installed by this module's successful submission.
    /// An empty span installs no code and must not be retired.
    pub const fn code_span(&self) -> CodeSpan {
        self.code_span
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
            slots: slots,
            generation: self.generation,
        };
        if self.live_slots {
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
            live_slots: false,
            text_cursor: false,
            calculator: false,
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

    /// A separate admission profile: the ordinary compiler never emits the
    /// proposal or generation imports.
    pub fn new_live() -> Self {
        Self {
            live: true,
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

    /// Independent calculator admission with one reserved trampoline slot.
    pub fn new_calculator() -> Self {
        let mut compiler = Self::new();
        compiler.calculator = true;
        compiler.functions = CALCULATOR_FUNCTION_BASE;
        compiler.text_end = CALCULATOR_TEXT_START;
        compiler.signatures.select_calculator();
        compiler
    }

    /// Independently accept and lower without changing existing code identities.
    pub fn prepare(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        self.prepare_from(submission, None, self.functions, false, false)
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
        self.prepare_from(submission, None, CALCULATOR_FUNCTION_BASE, true, false)
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
        self.prepare_from(submission, None, CALCULATOR_FUNCTION_BASE, true, true)
    }

    fn prepare_from(
        &self,
        submission: &noble_kernel::execution::Submission,
        selected: Option<&noble_contracts::source::proof::CheckedSelectedTarget<'_>>,
        functions: u32,
        quiescent: bool,
        program_free: bool,
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
        let preflight_issued = work.issued;
        let checked = attempt!(admission::check(
            submission,
            self.live,
            self.live_slots,
            self.text_cursor,
            self.calculator,
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
            live_slots: self.live_slots,
            text_cursor: self.text_cursor,
            calculator: self.calculator,
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
            &mut work
        ));
        work.calculator_plan = self.calculator;
        let mut plan = attempt!(plan::lower(submission, &checked, &mut next, &mut work));
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
        let target_metadata = if self.live_slots {
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
        if self.live_slots {
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
        if self.live_slots {
            program_metadata.reserve(plan.programs.len());
            for (program_index, program) in plan.programs.iter_mut().enumerate() {
                let logical = attempt!(program.logical.take().ok_or(crate::Diagnostic::Defective));
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
            false
        ));
        let id = selected.definition_id();
        let mut matches = prepared.named_targets.iter().filter(|named| {
            named.definition == selected.definition().definition
                && named.definition_identity == id.identity()
        });
        let Some(named) = matches.next() else {
            return Err(crate::Diagnostic::Invalid);
        };
        if matches.next().is_some()
            || named.stack_in != selected.body().interface.stack_in
            || named.stack_out != selected.body().interface.stack_out
            || named.effects != selected.body().interface.effects
            || !prepared.target_metadata.as_ref().is_some_and(|root| {
                root.stack_in == selected.root().interface.stack_in
                    && root.stack_out == selected.root().interface.stack_out
                    && root.effects == selected.root().interface.effects
            })
        {
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

    /// Retire an installed code allocation only after the trusted runtime
    /// verifies zero backend Program owners and clears every entry of its
    /// physical shared table span. The callback must query the backend's
    /// `code_owners(start, length)` (a negative result is a refusal), verify
    /// the installed entries and clear them before returning success.
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
        if !self.live_slots || span.slots == 0 || span.start < 4 {
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
