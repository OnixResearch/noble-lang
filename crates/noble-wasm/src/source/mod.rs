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

#[derive(Clone, Copy)]
pub(in crate::source) struct ProfileSelection {
    calculator: bool,
    checked_i64: bool,
    text_cursor: bool,
    declared_text: bool,
}

/// The environment row cap of a selected profile: its definition limit plus the
/// fixed rows only that profile installs (calculator 32, text cursor 27,
/// declared text 28, ordinary 26). Borrowed preflight, admission's environment
/// work accounting and the declared-shape check all use this one function so
/// the caps cannot drift apart.
pub(crate) const fn maximum_rows(
    calculator: bool,
    text_cursor: bool,
    declared_text: bool,
    checked_i64: bool,
) -> usize {
    if calculator {
        noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
    } else if checked_i64 {
        DEFINITION_LIMIT.saturating_add(30)
    } else if declared_text {
        DEFINITION_LIMIT.saturating_add(28)
    } else if text_cursor {
        DEFINITION_LIMIT.saturating_add(27)
    } else {
        DEFINITION_LIMIT.saturating_add(26)
    }
}
// Calculator source and compiler share a 896-row bound. The independently
// bounded named-body count and per-clone/fanout budgets are separate gates.
// Retain the complete E×B scans rather than discounting them. This 2b limit
// caps issued preparation accounting, NOT physical CPU instructions or
// resident bytes; preclone structural guards still reject amplification.
pub(super) const CALCULATOR_COMPILER_WORK_LIMIT: u64 = 2_000_000_000;

// Core live sessions reserve 24/25 for their two checked operations.
// Ordinary and declared sessions retain their original fixed prefixes.
fn builtin_count(env: &noble_kernel::contracts::Env) -> u32 {
    if env.has_checked_i64() {
        return 30;
    }
    if env.declared_modules {
        return declared_builtin_count(env);
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

/// The fixed prefix of a declared-module session.
const fn declared_builtin_count(env: &noble_kernel::contracts::Env) -> u32 {
    if env.calculator {
        return 32;
    }
    // Declared text extends the cursor prefix with signed I64 `<` at 27.
    if env.text_cursor {
        return 28;
    }
    23
}

pub(crate) struct Work {
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
    /// The ledger of ordinary, unmetered interning. It cannot refuse: with
    /// both calculator flags clear, `charge` only subtracts the visited
    /// amount from `remaining`, every `usize` count fits in `u64`, and one
    /// interning call visits fewer than `u64::MAX` entries in total, because
    /// each charge is bounded by the in-memory keys and bytes it actually
    /// visits.
    pub(crate) const fn unmetered() -> Self {
        Self {
            remaining: u64::MAX,
            issued: 0,
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        }
    }

    /// An ordinary ledger holding exactly `remaining` units, for tests that
    /// charge or exhaust a small budget.
    #[cfg(test)]
    pub(crate) const fn limited(remaining: u64) -> Self {
        Self {
            remaining,
            issued: remaining,
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        }
    }

    /// The units still unspent, for tests that observe what was charged.
    #[cfg(test)]
    pub(crate) const fn remaining(&self) -> u64 {
        self.remaining
    }

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

    pub(crate) fn entries(&mut self, count: usize) -> Result<(), crate::Diagnostic> {
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
    /// Finite known-sum case fusions performed by the experimental lowering.
    pub type_directed_specializations: u64,
    /// Ordinary operation entries retained on the baseline path by that lowering.
    pub type_directed_fallback_operations: u64,
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
    checked_i64: bool,
    worker_inputs: bool,
    /// Typed-worker profile: a host interruption point every eighth unit.
    worker_interruption: bool,
    /// Explicitly selected, finite type-directed ADT lowering experiment.
    type_directed_experimental: bool,
    /// Emit the trial-only cleanup export for either explicitly selected arm.
    experimental_cleanup: bool,
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

#[derive(Clone)]
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

// Equality compares recipes structurally, exactly as the derive did. A
// derived impl would recurse through `Rc<Self>`, so its generated trait
// instance would name itself before it exists; an explicit worklist of
// shallow node clones walks the two trees instead.
impl PartialEq for OriginFixedExpression {
    fn eq(&self, other: &Self) -> bool {
        recipes_equal(self, other)
    }
}

impl Eq for OriginFixedExpression {}

/// Whether two fixed recipes have equal shapes, literals, kinds and bounds.
fn recipes_equal(left: &OriginFixedExpression, right: &OriginFixedExpression) -> bool {
    let mut pending = RecipePairs {
        pairs: alloc::vec::Vec::with_capacity(8),
    };
    pending.pairs.push((left.clone(), right.clone()));
    let mut is_equal = true;
    while is_equal && !pending.pairs.is_empty() {
        is_equal = pending.compare_next();
    }
    is_equal
}

/// Recipe node pairs still to compare, innermost last.
struct RecipePairs {
    pairs: alloc::vec::Vec<(OriginFixedExpression, OriginFixedExpression)>,
}

impl RecipePairs {
    /// Compare the next pending pair and queue its children; `false` at a difference.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; the comparison mutates only its own private worklist of recipe pairs."
    )]
    fn compare_next(&mut self) -> bool {
        let Some(pair) = self.pairs.pop() else {
            return true;
        };
        match pair {
            (OriginFixedExpression::Literal(left), OriginFixedExpression::Literal(right)) => {
                left == right
            }
            (
                OriginFixedExpression::Arithmetic {
                    kind,
                    left,
                    right,
                    nodes,
                    depth,
                },
                OriginFixedExpression::Arithmetic {
                    kind: other_kind,
                    left: other_left,
                    right: other_right,
                    nodes: other_nodes,
                    depth: other_depth,
                },
            ) => {
                if kind != other_kind || nodes != other_nodes || depth != other_depth {
                    return false;
                }
                self.pairs
                    .push((right.as_ref().clone(), other_right.as_ref().clone()));
                self.pairs
                    .push((left.as_ref().clone(), other_left.as_ref().clone()));
                true
            }
            // A literal never equals an arithmetic node.
            _ => false,
        }
    }
}

#[derive(Clone)]
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

#[derive(Clone)]
pub enum OriginOperand {
    I64(OriginI64),
    Program(alloc::rc::Rc<OriginProgramValue>),
}

impl core::fmt::Debug for OriginOperand {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; Debug formatting writes only to the caller's Formatter, which is the operation"
    )]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        debug_origin(OriginRendering::Operand(self), f)
    }
}

impl core::fmt::Debug for OriginProgramValue {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; Debug formatting writes only to the caller's Formatter, which is the operation"
    )]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        debug_origin(OriginRendering::Value(self), f)
    }
}

impl core::fmt::Debug for OriginFixedExpression {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; Debug formatting writes only to the caller's Formatter, which is the operation"
    )]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        debug_origin(OriginRendering::Recipe(self), f)
    }
}

/// One borrowed value of the recursive origin types to render.
#[derive(Clone, Copy)]
enum OriginRendering<'a> {
    Operand(&'a OriginOperand),
    Value(&'a OriginProgramValue),
    Recipe(&'a OriginFixedExpression),
}

/// Render the recursive origin types exactly as `#[derive(Debug)]` did.
///
/// Reviewed opaque extraction boundary: `OriginProgramValue`, `OriginOperand`
/// and `OriginFixedExpression` recurse through `Rc`, so rendering any nested
/// value formats it through its own `Debug` impl; no transparent body can
/// avoid the self- and mutually-referencing trait instances that the pinned
/// Aeneas refuses or emits before they are defined. Formatting layout is
/// already an abstracted trust boundary.
#[charon::opaque]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; Debug formatting writes only to the caller's Formatter, which is the operation"
)]
fn debug_origin(
    rendering: OriginRendering<'_>,
    f: &mut core::fmt::Formatter<'_>,
) -> core::fmt::Result {
    match rendering {
        OriginRendering::Operand(OriginOperand::I64(value)) => {
            f.debug_tuple("I64").field(value).finish()
        }
        OriginRendering::Operand(OriginOperand::Program(value)) => {
            f.debug_tuple("Program").field(value).finish()
        }
        OriginRendering::Value(OriginProgramValue::Static {
            node,
            program_index,
        }) => f
            .debug_struct("Static")
            .field("node", node)
            .field("program_index", program_index)
            .finish(),
        OriginRendering::Value(OriginProgramValue::Quote {
            node,
            table_entry,
            operand,
        }) => f
            .debug_struct("Quote")
            .field("node", node)
            .field("table_entry", table_entry)
            .field("operand", operand)
            .finish(),
        OriginRendering::Value(OriginProgramValue::Compose {
            node,
            table_entry,
            left,
            right,
        }) => f
            .debug_struct("Compose")
            .field("node", node)
            .field("table_entry", table_entry)
            .field("left", left)
            .field("right", right)
            .finish(),
        OriginRendering::Recipe(OriginFixedExpression::Literal(value)) => {
            f.debug_tuple("Literal").field(value).finish()
        }
        OriginRendering::Recipe(OriginFixedExpression::Arithmetic {
            kind,
            left,
            right,
            nodes,
            depth,
        }) => f
            .debug_struct("Arithmetic")
            .field("kind", kind)
            .field("left", left)
            .field("right", right)
            .field("nodes", nodes)
            .field("depth", depth)
            .finish(),
    }
}

#[derive(Clone, Debug)]
pub struct OriginOutput {
    pub stack_position: usize,
    pub result: alloc::rc::Rc<OriginProgramValue>,
    /// Exact checked template projection: ordered distinct root captures,
    /// logical source-binding classes and the recursive-owner classification.
    pub template: OriginTemplate,
}

/// The checked source binding of one template class, as a structural row. A
/// child class, a static quotation body or a fixed recipe is named by its
/// index into the same template's `classes`, `statics` or `recipes`, so equal
/// bindings have equal rows; no row holds a source span or a NodeId. The host
/// renders the canonical spelling (`q(r0)`, `c(…,…)`, …) from these rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginBinding {
    /// The enclosing named definition: a required lexical recursive owner.
    Named,
    /// A static definition quotation body, by content (a `statics` row).
    Static(u32),
    /// `quote` of the root input at this position.
    QuoteRoot(u32),
    /// `quote` of a fixed I64 recipe (a `recipes` row).
    QuoteFixed(u32),
    /// `quote` of a Program class.
    QuoteProgram(u32),
    /// `compose` of two Program classes.
    Compose { left: u32, right: u32 },
}

/// One operation of a static quotation body; a nested static quotation is
/// resolved by content to a `statics` row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginStaticOperation {
    I64(i64),
    Add,
    Sub,
    Mul,
    Dup,
    Drop,
    Swap,
    Dip,
    Run,
    Compose,
    Quote,
    Static(u32),
}

/// A static quotation body by content: its signature pair and operations.
#[derive(Clone, Debug)]
pub struct OriginStaticBody {
    pub input_signature: u32,
    pub output_signature: u32,
    pub operations: alloc::vec::Vec<OriginStaticOperation>,
}

/// A fixed I64 recipe: a literal, or checked arithmetic over two earlier rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginRecipe {
    Literal(i64),
    Arithmetic {
        kind: OriginFixedArithmetic,
        left: u32,
        right: u32,
    },
}

/// One verified logical source-binding class of a returned template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriginClass {
    pub binding: OriginBinding,
    /// Distinct retained VM cells that currently instantiate this binding. This
    /// count is observation data: physical sharing may change it without
    /// changing the binding, the captures or the target template identity.
    pub occurrences: u32,
}

/// Whether a returned template requires a semantic lexical recursive owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginOwner {
    /// Every reachable quotation is an acyclic static definition body and no
    /// reference resolves the enclosing named definition.
    Absent,
    /// A reachable quotation resolves the enclosing named definition. The
    /// bounded selected-origin subset does not classify this shape, so no
    /// target identity may be minted for it.
    SelfNamed,
}

/// Exact checked template projection of one returned Program. Identity-bearing
/// admission binds it to the actual retained graph and installed code; it is
/// not a `DefinitionId`. Rows appear in derivation order: every row a class,
/// body or recipe names comes before it.
#[derive(Clone, Debug)]
pub struct OriginTemplate {
    /// Ordered distinct root-input positions a reachable `quote` consumes.
    pub captures: alloc::vec::Vec<u32>,
    /// Distinct logical binding classes.
    pub classes: alloc::vec::Vec<OriginClass>,
    /// Distinct static quotation bodies the classes name.
    pub statics: alloc::vec::Vec<OriginStaticBody>,
    /// Distinct fixed recipe nodes the classes name.
    pub recipes: alloc::vec::Vec<OriginRecipe>,
    pub owner: OriginOwner,
}

/// Re-derive every retained template projection from the immutable origin
/// values and require it to equal the stored metadata. The host calls this on
/// retained worker metadata; `origin::build` also calls it before returning.
pub fn verify_selected_origin_template(origin: &SelectedOriginMetadata) -> bool {
    origin::verified_templates(origin)
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
        let mut index = 0usize;
        let mut failure = None;
        while index < self.next.descriptors.len() {
            let (address, length) = self.next.descriptors[index];
            let signature = &self.next.signatures.keys[index];
            if is_defective_descriptor(address, length, signature) {
                failure = Some(crate::Diagnostic::Defective);
                break;
            }
            bytes.extend_from_slice(&address.to_le_bytes());
            bytes.extend_from_slice(&length.to_le_bytes());
            bytes.extend_from_slice(signature);
            index += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(bytes),
        }
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
        let start = attempt!(self.code_start(slots));
        let span = CodeSpan {
            start,
            slots,
            generation: self.generation,
        };
        if self.owns_code() {
            self.installed_code.push(span);
        }
        Ok(span)
    }

    /// The first slot of `slots` new code slots: the first free span that fits
    /// under live slots, else the table's end, which then grows by `slots`.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; reserve_code updates only the unpublished prospective compiler table and preserves every refusal as a diagnostic."
    )]
    fn code_start(&mut self, slots: u32) -> Result<u32, crate::Diagnostic> {
        if let Some(index) = self.fitting_free_code(slots) {
            return Ok(self.take_free_code(index, slots));
        }
        let start = self.functions;
        self.functions = attempt!(extended_table(self.functions, slots));
        Ok(start)
    }

    /// The first free span holding `slots` slots; only live slots reuse code.
    fn fitting_free_code(&self, slots: u32) -> Option<usize> {
        if !self.live_slots {
            return None;
        }
        position_of(&self.free_code, |free| free.slots >= slots)
    }

    /// Take `slots` slots from the front of free span `index`, dropping it once empty.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; reserve_code updates only the unpublished prospective compiler table and preserves every refusal as a diagnostic."
    )]
    fn take_free_code(&mut self, index: usize, slots: u32) -> u32 {
        let free = &mut self.free_code[index];
        let start = free.start;
        free.start += slots;
        free.slots -= slots;
        if free.slots == 0 {
            self.free_code.remove(index);
        }
        start
    }

    /// Whether a live or declared-live profile owns its installed code spans.
    const fn owns_code(&self) -> bool {
        self.live || self.declared_live || self.live_slots
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
            return self.signatures.intern_charged(stack, work);
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
            checked_i64: false,
            worker_inputs: false,
            worker_interruption: false,
            type_directed_experimental: false,
            experimental_cleanup: false,
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

    /// Opt in to the bounded type-directed finite-Sum lowering experiment.
    /// Unsupported operation shapes retain ordinary baseline lowering.
    pub fn new_type_directed_experimental() -> Self {
        Self {
            type_directed_experimental: true,
            experimental_cleanup: true,
            ..Self::new()
        }
    }

    /// Select the baseline lowering with the explicit trial cleanup export.
    pub fn new_experimental_baseline() -> Self {
        Self {
            experimental_cleanup: true,
            ..Self::new()
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

    /// The checked-I64-v1 operation table is selected independently of Core.
    pub fn new_checked_i64() -> Self {
        Self {
            checked_i64: true,
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

    /// The typed-worker execution profile: worker ingress whose every eighth
    /// charged unit of execution or observation work calls the imported
    /// `noble/worker_interrupt_check` with the total charged work.
    pub fn new_interruptible_worker() -> Self {
        Self {
            worker_inputs: true,
            worker_interruption: true,
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
        if submission.environment.has_checked_i64() != self.checked_i64 {
            return Err(crate::Diagnostic::Invalid);
        }
        if self.type_directed_experimental
            && (self.live
                || self.declared_live
                || self.live_slots
                || self.text_cursor
                || self.declared_text
                || self.calculator
                || self.worker_inputs
                || self.worker_interruption
                || self.checked_i64
                || submission.environment.declared_modules
                || submission.environment.has_checked_i64())
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        self.prepare_from(submission, self.functions, false, false, false)
    }

    /// Rebuild a signed staged-source snapshot before authorizing `SignedSelf`.
    /// A public Submission or Env cannot supply the owner instance witness.
    #[expect(
        tigerstyle::compound_condition,
        reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
    )]
    pub fn prepare_signed(
        &self,
        owner: &noble_contracts::source::SignedSourceOwner<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if self.live
            || self.live_slots
            || self.text_cursor
            || self.declared_text
            || self.calculator
            || self.checked_i64
            || self.worker_inputs
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        let submission = owner.rederive().map_err(|_| crate::Diagnostic::Invalid)?;
        if !submission.environment.declared_modules
            || submission.environment.live_slots
            || submission.environment.text_cursor
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        self.prepare_from(&submission, self.functions, false, false, true)
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
        owner: &noble_contracts::source::SignedSourceOwner<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.worker_inputs
            || self.live
            || self.live_slots
            || self.text_cursor
            || self.declared_text
            || self.calculator
            || self.checked_i64
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        let submission = owner.rederive().map_err(|_| crate::Diagnostic::Invalid)?;
        self.prepare_from(&submission, self.functions, false, false, true)
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
        owner: &noble_contracts::source::DeclaredTextSourceOwner<'_>,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.declared_text
            || !self.text_cursor
            || self.live
            || self.declared_live
            || self.live_slots
            || self.calculator
            || self.checked_i64
            || self.worker_inputs
        {
            return Err(crate::Diagnostic::Unsupported);
        }
        let submission = owner.rederive().map_err(|_| crate::Diagnostic::Invalid)?;
        if !submission.environment.declared_modules || !submission.environment.text_cursor {
            return Err(crate::Diagnostic::Unsupported);
        }
        let is_signed = crate::scan::find_first(&submission.definitions, |definition| {
            crate::scan::find_first(&definition.body.candidate.nodes, |node| {
                matches!(node, noble_kernel::untrusted::Node::SignedSelf { .. })
            })
            .is_some()
        })
        .is_some();
        self.prepare_from(&submission, self.functions, false, false, is_signed)
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
        self.prepare_from(submission, CALCULATOR_FUNCTION_BASE, true, false, false)
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
        self.prepare_from(submission, CALCULATOR_FUNCTION_BASE, true, true, false)
    }

    /// Independently accept and lower a submission into a preparation.
    fn prepare_from(
        &self,
        submission: &noble_kernel::execution::Submission,
        functions: u32,
        quiescent: bool,
        program_free: bool,
        signed: bool,
    ) -> Result<Prepared, crate::Diagnostic> {
        let (prepared, _) =
            attempt!(self.prepare_layout(submission, functions, quiescent, program_free, signed));
        Ok(prepared)
    }

    /// Charge borrowed preflight and independent admission of a submission.
    fn admitted(
        &self,
        submission: &noble_kernel::execution::Submission,
        signed: bool,
    ) -> Result<Admitted, crate::Diagnostic> {
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
        attempt!(preflight::check(
            submission,
            ProfileSelection {
                calculator: self.calculator,
                checked_i64: self.checked_i64,
                text_cursor: self.text_cursor,
                declared_text: self.declared_text,
            },
            &mut work
        ));
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
            self.checked_i64,
            self.declared_text,
            signed,
            &mut work,
        ));
        let admission_issued = work.issued;
        Ok(Admitted {
            work,
            checked,
            preflight_issued,
            admission_issued,
        })
    }

    /// Independently accept and lower a submission. The unpublished plan is
    /// returned beside the preparation, so a selected preparation derives the
    /// returned target's lineage from it and no borrowed source state flows
    /// through this preparation.
    fn prepare_layout(
        &self,
        submission: &noble_kernel::execution::Submission,
        functions: u32,
        quiescent: bool,
        program_free: bool,
        signed: bool,
    ) -> Result<(Prepared, plan::Layout), crate::Diagnostic> {
        let Admitted {
            mut work,
            checked,
            preflight_issued,
            admission_issued,
        } = attempt!(self.admitted(submission, signed));
        // A Program-free epoch interns into a fresh calculator pool, so no
        // committed key is retained, copied or charged.
        let fresh_signatures = fresh_signature_pool(program_free);
        let signature_key_charge = attempt!(admission::meter::compilation(
            &checked,
            fresh_signatures.as_ref().unwrap_or(&self.signatures),
            &mut work
        ));
        // A quiescent epoch starts with an empty identity ledger: nothing of
        // the committed ledger is retained, so none of it is copied or charged.
        let retained = self.retained_identities(quiescent);
        let identity_charge = attempt!(identity_copy_charge(&self.identities, retained, &mut work));
        // The literal table is copied into the prospective compiler.
        attempt!(work.entries(literal_copy_entries(&self.static_literals)));
        let mut next =
            attempt!(self.prospective(functions, quiescent, program_free, fresh_signatures));
        attempt!(admission::identity::check(
            submission,
            &mut next.identities,
            next.calculator,
            &mut work,
            signed,
        ));
        work.calculator_plan = self.calculator;
        let mut plan = attempt!(plan::lower(submission, &checked, &mut next, &mut work));
        let (published, wat) = attempt!(self.emitted(&plan, &next, program_free, &mut work));
        let code_span = CodeSpan {
            start: plan.first_function,
            slots: plan.functions - plan.first_function,
            generation: next.generation,
        };
        let target_metadata = attempt!(self.root_metadata(submission, &checked, &plan));
        let named_targets =
            attempt!(self.named_metadata(submission, &checked, &plan, &next.identities));
        let program_metadata = attempt!(self.program_metadata(&mut plan));
        let resource_catalog = attempt!(self.resource_metadata(&submission.environment));
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
            type_directed_specializations: plan.type_directed_specializations,
            type_directed_fallback_operations: plan.type_directed_fallback_operations,
        };
        let signature_base = self.retained_signatures(program_free);
        let epoch = self.program_free_epoch(program_free);
        let live_sites = core::mem::take(&mut plan.live_sites);
        let prepared = Prepared {
            base: transaction::Base::capture(self, retained, signature_base, published),
            base_live_slots: self.live_slots,
            base_retention_revision: self.retention_revision,
            next,
            code_span,
            live_sites,
            target_metadata,
            program_metadata,
            named_targets,
            selected_target: None,
            selected_origin: None,
            resource_catalog,
            wat,
            metrics,
            epoch,
            work_issued: work.issued,
        };
        Ok((prepared, plan))
    }

    /// The committed identities a preparation retains: none at a quiescent boundary.
    fn retained_identities(&self, quiescent: bool) -> usize {
        if quiescent {
            0
        } else {
            self.identities.len()
        }
    }

    /// The committed signature prefix a preparation retains: none at a Program-free boundary.
    fn retained_signatures(&self, program_free: bool) -> usize {
        if program_free {
            0
        } else {
            self.signatures.len()
        }
    }

    /// The canonical epoch recorded by a Program-free preparation, if any.
    fn program_free_epoch(&self, program_free: bool) -> Option<CanonicalEpoch> {
        if program_free {
            self.canonical_epoch()
        } else {
            None
        }
    }

    /// The prospective compiler a preparation lowers into: the committed state
    /// at the next generation, rewound where the epoch restarts it.
    fn prospective(
        &self,
        functions: u32,
        quiescent: bool,
        program_free: bool,
        fresh_signatures: Option<crate::signatures::Pool>,
    ) -> Result<Self, crate::Diagnostic> {
        let generation = match self.generation.checked_add(1) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        let free_code = self.free_code.clone();
        let installed_code = self.installed_code.clone();
        let static_literals = self.static_literals.clone();
        let signatures = fresh_signatures.unwrap_or_else(|| self.signatures.clone());
        let descriptors = if program_free {
            alloc::vec::Vec::new()
        } else {
            self.descriptors.clone()
        };
        let identities = if quiescent {
            alloc::vec::Vec::new()
        } else {
            self.identities.clone()
        };
        Ok(Self {
            live: self.live,
            declared_live: self.declared_live,
            live_slots: self.live_slots,
            text_cursor: self.text_cursor,
            declared_text: self.declared_text,
            calculator: self.calculator,
            checked_i64: self.checked_i64,
            worker_inputs: self.worker_inputs,
            worker_interruption: self.worker_interruption,
            type_directed_experimental: self.type_directed_experimental,
            experimental_cleanup: self.experimental_cleanup,
            generation,
            functions,
            retention_revision: self.retention_revision,
            free_code,
            installed_code,
            text_end: self.text_end,
            static_literals,
            signatures,
            descriptors,
            identities,
        })
    }

    /// Emit the module and report the committed descriptor prefix it publishes after.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; the budget belongs to one private preparation and tracks consumed work without modifying the borrowed submission or published compiler."
    )]
    fn emitted(
        &self,
        plan: &plan::Layout,
        next: &Self,
        program_free: bool,
        work: &mut Work,
    ) -> Result<(usize, alloc::vec::Vec<u8>), crate::Diagnostic> {
        // Worker literals share the scratch-region boundary used by calculator
        // signature expansion; neither profile may let static Text overlap it.
        if self.worker_inputs && next.text_end > CALCULATOR_TEXT_END {
            return Err(crate::Diagnostic::Exhausted);
        }
        // A calculator module publishes only the signatures new since the
        // committed compiler's descriptor table (all of its own after a
        // Program-free rewind); the trusted engine retains that published
        // prefix, so per-module metadata is history-independent.
        let published = self.retained_descriptors(program_free);
        if next.calculator {
            let keys = attempt!(next.signatures.expanded_keys(published, work));
            let wat = attempt!(emit::module(plan, self.generation, published, &keys));
            return Ok((published, wat));
        }
        let wat = attempt!(emit::module(
            plan,
            self.generation,
            0,
            &next.signatures.keys
        ));
        Ok((published, wat))
    }

    /// The committed descriptor prefix a preparation retains: none at a Program-free boundary.
    fn retained_descriptors(&self, program_free: bool) -> usize {
        if program_free {
            0
        } else {
            self.descriptors.len()
        }
    }

    /// The checked root interface the live-slot and Worker profiles publish.
    fn root_metadata(
        &self,
        submission: &noble_kernel::execution::Submission,
        checked: &admission::Accepted,
        plan: &plan::Layout,
    ) -> Result<Option<TargetMetadata>, crate::Diagnostic> {
        if !(self.live_slots || self.worker_inputs) {
            return Ok(None);
        }
        let root = attempt!(plan
            .programs
            .get(plan.root)
            .ok_or(crate::Diagnostic::Invalid));
        Ok(Some(TargetMetadata {
            root_program_index: plan.root,
            input_signature: root.input,
            output_signature: root.output,
            effect_mask: root.effects,
            stack_in: submission.request.expected.stack_in.clone(),
            stack_out: submission.request.expected.stack_out.clone(),
            effects: checked.root.interface.effects.clone(),
        }))
    }

    /// The named targets the live-slot and Worker profiles publish.
    fn named_metadata(
        &self,
        submission: &noble_kernel::execution::Submission,
        checked: &admission::Accepted,
        plan: &plan::Layout,
        identities: &[Identity],
    ) -> Result<alloc::vec::Vec<NamedTargetMetadata>, crate::Diagnostic> {
        if !(self.live_slots || self.worker_inputs) {
            return Ok(alloc::vec::Vec::new());
        }
        checked_named_targets(submission, checked, plan, identities)
    }

    /// The program metadata the live-slot and Worker profiles publish.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; metadata publication moves each logical interface out of the unpublished plan that this preparation owns; the borrowed submission stays immutable."
    )]
    fn program_metadata(
        &self,
        plan: &mut plan::Layout,
    ) -> Result<alloc::vec::Vec<ProgramMetadata>, crate::Diagnostic> {
        if !(self.live_slots || self.worker_inputs) {
            return Ok(alloc::vec::Vec::new());
        }
        checked_program_metadata(plan, self.worker_inputs)
    }

    /// The resource catalog the live-slot profile publishes.
    fn resource_metadata(
        &self,
        environment: &noble_kernel::contracts::Env,
    ) -> Result<alloc::vec::Vec<ResourceCatalogEntry>, crate::Diagnostic> {
        if !self.live_slots {
            return Ok(alloc::vec::Vec::new());
        }
        admitted_resource_catalog(environment)
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
        let (mut prepared, plan) = attempt!(self.prepare_layout(
            selected.submission(),
            self.functions,
            false,
            false,
            false,
        ));
        prepared.selected_origin = origin::build(selected, &plan);
        let id = selected.definition_id();
        let (first, has_duplicate) = selected_named_target(
            &prepared.named_targets,
            selected.definition().definition,
            id.identity(),
        );
        let Some(index) = first else {
            return Err(crate::Diagnostic::Invalid);
        };
        let named_program_index = prepared.named_targets[index].program_index;
        if selection_mismatch(
            &prepared.named_targets[index],
            has_duplicate,
            &selected.body().interface,
            prepared.target_metadata.as_ref(),
            &selected.root().interface,
        ) {
            return Err(crate::Diagnostic::Invalid);
        }
        prepared.selected_target = Some(SelectedTargetArtifact {
            definition_id: id,
            source_generation: selected.source_generation(),
            named_program_index,
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
            || self.checked_i64
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
        if !self.owns_code() || span.slots == 0 || span.start < 4 {
            return Err(crate::Diagnostic::Invalid);
        }
        let Some(index) = position_of(&self.installed_code, |owned| *owned == span) else {
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
        let index = self
            .free_code
            .partition_point(|free| free.start < span.start);
        self.free_code.insert(
            index,
            FreeCode {
                start: span.start,
                slots: span.slots,
            },
        );
        let index = self.coalesce_left(index);
        self.coalesce_right(index);
        self.release_table_end(index);
        self.retention_revision += 1;
        Ok(())
    }

    /// Merge free span `index` into its left neighbour when they touch,
    /// returning the index of the span that now holds it.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; successful retirement explicitly clears trusted runtime table entries and publishes a revised compiler allocation state."
    )]
    fn coalesce_left(&mut self, index: usize) -> usize {
        if !self.touches_left(index) {
            return index;
        }
        let right = self.free_code.remove(index);
        self.free_code[index - 1].slots += right.slots;
        index - 1
    }

    /// Whether free span `index` starts where its left neighbour ends.
    fn touches_left(&self, index: usize) -> bool {
        index > 0
            && self.free_code[index - 1].start + self.free_code[index - 1].slots
                == self.free_code[index].start
    }

    /// Merge free span `index`'s right neighbour into it when they touch.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; successful retirement explicitly clears trusted runtime table entries and publishes a revised compiler allocation state."
    )]
    fn coalesce_right(&mut self, index: usize) {
        if !self.touches_right(index) {
            return;
        }
        let right = self.free_code.remove(index + 1);
        self.free_code[index].slots += right.slots;
    }

    /// Whether free span `index`'s right neighbour starts where it ends.
    fn touches_right(&self, index: usize) -> bool {
        index + 1 < self.free_code.len()
            && self.free_code[index].start + self.free_code[index].slots
                == self.free_code[index + 1].start
    }

    /// Shrink the table when free span `index` ends exactly at the table's end.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; successful retirement explicitly clears trusted runtime table entries and publishes a revised compiler allocation state."
    )]
    fn release_table_end(&mut self, index: usize) {
        if self.free_code[index].start + self.free_code[index].slots != self.functions {
            return;
        }
        self.functions = self.free_code[index].start;
        self.free_code.remove(index);
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// The index of the first element satisfying `predicate`, as `items.iter().position(..)`.
fn position_of<T>(items: &[T], mut predicate: impl FnMut(&T) -> bool) -> Option<usize> {
    let mut index = 0usize;
    let mut found = None;
    while found.is_none() && index < items.len() {
        if predicate(&items[index]) {
            found = Some(index);
        }
        index += 1;
    }
    found
}

/// The table end after `slots` more functions, refused past the table limit.
fn extended_table(functions: u32, slots: u32) -> Result<u32, crate::Diagnostic> {
    match functions.checked_add(slots) {
        Some(end) if end <= TABLE_LIMIT => Ok(end),
        Some(_) | None => Err(crate::Diagnostic::Exhausted),
    }
}

/// The work budget and checked submission after borrowed preflight and
/// independent admission, with the issued work recorded at each stage.
struct Admitted {
    work: Work,
    checked: admission::Accepted,
    preflight_issued: u64,
    admission_issued: u64,
}

/// A fresh calculator signature pool for a Program-free epoch.
fn fresh_signature_pool(program_free: bool) -> Option<crate::signatures::Pool> {
    if program_free {
        let mut pool = crate::signatures::Pool::source();
        pool.select_calculator();
        Some(pool)
    } else {
        None
    }
}

/// Whether the selected named target or the root disagrees with the checked selection.
fn selection_mismatch(
    named: &NamedTargetMetadata,
    has_duplicate: bool,
    body: &noble_kernel::untrusted::Interface,
    target: Option<&TargetMetadata>,
    root: &noble_kernel::untrusted::Interface,
) -> bool {
    has_duplicate
        || named.stack_in != body.stack_in
        || named.stack_out != body.stack_out
        || named.effects != body.effects
        || !target.is_some_and(|target| {
            target.stack_in == root.stack_in
                && target.stack_out == root.stack_out
                && target.effects == root.effects
        })
}

/// Whether a published descriptor fails to name exactly its key's bytes in memory.
fn is_defective_descriptor(address: u32, length_bytes: u32, signature: &[u8]) -> bool {
    usize::try_from(length_bytes).ok() != Some(signature.len())
        || address < TEXT_START
        || address
            .checked_add(length_bytes)
            .is_none_or(|end| end > MEMORY_BYTES)
}

/// Charge copying the first `retained` identity recipes, returning the charged entries.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the budget belongs to one private preparation and tracks consumed work without modifying the borrowed submission or published compiler."
)]
fn identity_copy_charge(
    identities: &[Identity],
    retained: usize,
    work: &mut Work,
) -> Result<u64, crate::Diagnostic> {
    let mut identity = 0usize;
    let mut identity_charge = 0u64;
    let mut failure = None;
    while identity < retained {
        let entries = identities[identity].recipe.len();
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
    match failure {
        Some(problem) => Err(problem),
        None => Ok(identity_charge),
    }
}

/// The entries of copying the static literal table: every key plus one.
fn literal_copy_entries(
    static_literals: &alloc::collections::BTreeMap<alloc::vec::Vec<u8>, u32>,
) -> usize {
    let mut total = 0usize;
    for key in static_literals.keys() {
        total = total.saturating_add(key.len() + 1);
    }
    total
}

/// The named target of every accepted definition, in definition order.
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-wasm-maintainers; growth is bounded by the checked input or admission limits that drive the loop."
)]
fn checked_named_targets(
    submission: &noble_kernel::execution::Submission,
    checked: &admission::Accepted,
    plan: &plan::Layout,
    identities: &[Identity],
) -> Result<alloc::vec::Vec<NamedTargetMetadata>, crate::Diagnostic> {
    let mut targets = alloc::vec::Vec::new();
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.definitions.len() {
        match checked_named_target(submission, checked, plan, identities, index) {
            Ok(target) => {
                targets.push(target);
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(targets),
    }
}

/// The emitted program and checked recipe of accepted definition `index`.
fn checked_named_target(
    submission: &noble_kernel::execution::Submission,
    checked: &admission::Accepted,
    plan: &plan::Layout,
    identities: &[Identity],
    index: usize,
) -> Result<NamedTargetMetadata, crate::Diagnostic> {
    let definition = &submission.definitions[index];
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
    let identity = definition.identity;
    let recipe = attempt!(
        crate::scan::find_first(identities, |row| row.id == identity)
            .ok_or(crate::Diagnostic::Invalid)
    );
    Ok(NamedTargetMetadata {
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
    })
}

/// Every emitted program's metadata, moving out each program's logical interface.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; metadata publication moves each logical interface out of the unpublished plan that this preparation owns; the borrowed submission stays immutable."
)]
fn checked_program_metadata(
    plan: &mut plan::Layout,
    worker_inputs: bool,
) -> Result<alloc::vec::Vec<ProgramMetadata>, crate::Diagnostic> {
    let mut metadata = alloc::vec::Vec::with_capacity(plan.programs.len());
    let text_witness = plan.text_witness;
    let mut index = 0usize;
    let mut failure = None;
    while index < plan.programs.len() {
        match program_entry(
            &mut plan.programs[index],
            index,
            text_witness,
            worker_inputs,
        ) {
            Ok(entry) => {
                metadata.push(entry);
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(metadata),
    }
}

/// One program's metadata; a worker module also retains its runtime atoms.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; metadata publication moves each logical interface out of the unpublished plan that this preparation owns; the borrowed submission stays immutable."
)]
fn program_entry(
    program: &mut plan::Program,
    program_index: usize,
    text_witness: u32,
    worker_inputs: bool,
) -> Result<ProgramMetadata, crate::Diagnostic> {
    let logical = attempt!(program.logical.take().ok_or(crate::Diagnostic::Defective));
    let runtime_atoms = if worker_inputs {
        runtime_atoms(&program.operations, text_witness)
    } else {
        alloc::vec::Vec::new()
    };
    Ok(ProgramMetadata {
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
    })
}

/// The closed runtime atom of every operation, in order.
fn runtime_atoms(
    operations: &[plan::Operation],
    text_witness: u32,
) -> alloc::vec::Vec<ProgramAtomMetadata> {
    let mut atoms = alloc::vec::Vec::with_capacity(operations.len());
    for operation in operations {
        atoms.push(plan::atom_metadata(operation, text_witness));
    }
    atoms
}

/// Every admitted live resource descriptor, in environment order.
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-wasm-maintainers; growth is bounded by the checked input or admission limits that drive the loop."
)]
fn admitted_resource_catalog(
    environment: &noble_kernel::contracts::Env,
) -> Result<alloc::vec::Vec<ResourceCatalogEntry>, crate::Diagnostic> {
    let mut catalog = alloc::vec::Vec::new();
    let mut index = 0usize;
    let mut failure = None;
    while index < environment.live_resource_nominals.len() {
        match resource_entry(environment, environment.live_resource_nominals[index]) {
            Ok(entry) => {
                catalog.push(entry);
                index += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(catalog),
    }
}

/// The admitted descriptor of one opaque live resource nominal.
fn resource_entry(
    environment: &noble_kernel::contracts::Env,
    id: noble_kernel::types::NominalTypeId,
) -> Result<ResourceCatalogEntry, crate::Diagnostic> {
    let declaration = attempt!(environment.nominal(id).ok_or(crate::Diagnostic::Invalid));
    let noble_kernel::types::NominalShape::Opaque(value) = &declaration.shape else {
        return Err(crate::Diagnostic::Invalid);
    };
    let noble_kernel::types::Ty::Resource(kind) = value.as_ref() else {
        return Err(crate::Diagnostic::Invalid);
    };
    Ok(ResourceCatalogEntry {
        declaration: declaration.clone(),
        kind: *kind,
    })
}

/// The first named target compiled for the selection, and whether another follows it.
fn selected_named_target(
    named_targets: &[NamedTargetMetadata],
    definition: noble_kernel::contracts::Definition,
    identity: u64,
) -> (Option<usize>, bool) {
    let mut first = None;
    let mut has_duplicate = false;
    let mut index = 0usize;
    while !has_duplicate && index < named_targets.len() {
        if is_named_selection(&named_targets[index], definition, identity) {
            if first.is_none() {
                first = Some(index);
            } else {
                has_duplicate = true;
            }
        }
        index += 1;
    }
    (first, has_duplicate)
}

/// Whether a named target was compiled for `definition` with `identity`.
fn is_named_selection(
    named: &NamedTargetMetadata,
    definition: noble_kernel::contracts::Definition,
    identity: u64,
) -> bool {
    named.definition == definition && named.definition_identity == identity
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

    // LREF-CASE-02's code-retention counter in Live-Slot-Design (integrator decision 47): the
    // install generation that tags every installed code span refuses at its maximum instead of
    // wrapping, and the refused preparation leaves the compiler's state as it was.
    #[test]
    fn install_generation_refuses_at_its_maximum_without_wrapping() {
        let mut compiler = Compiler::new_live_slots();
        compiler.generation = u32::MAX;
        let functions = compiler.functions;
        assert!(matches!(
            compiler.prospective(functions, false, false, None),
            Err(Diagnostic::Exhausted)
        ));
        assert_eq!(
            (compiler.generation, compiler.functions),
            (u32::MAX, functions)
        );
    }

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
    fn experimental_lowering_fuses_only_a_checked_known_sum_case() {
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let session = noble_contracts::source::Session::new();
        let prepared = session
            .prepare(b"10 inl [ 1 + ] [ 2 + ] case", &[], limits)
            .unwrap_or_else(|_| panic!("checked known-sum source refused"));
        let submission = prepared
            .submission()
            .unwrap_or_else(|| panic!("missing checked known-sum submission"));
        let baseline = Compiler::new()
            .prepare(submission)
            .unwrap_or_else(|_| panic!("baseline known-sum compile refused"));
        let explicit_baseline = Compiler::new_experimental_baseline()
            .prepare(submission)
            .unwrap_or_else(|_| panic!("explicit trial baseline compile refused"));
        let experimental = Compiler::new_type_directed_experimental()
            .prepare(submission)
            .unwrap_or_else(|_| panic!("experimental known-sum compile refused"));

        assert_eq!(baseline.metrics().type_directed_specializations, 0);
        assert_eq!(experimental.metrics().type_directed_specializations, 1);
        assert!(core::str::from_utf8(experimental.wat()).is_ok_and(|wat| wat
            .contains("noble-type-directed-report:v1 specialized-known-sum-cases=1")
            && wat.contains("(call $op_known_case_left")));
        assert!(!core::str::from_utf8(baseline.wat())
            .is_ok_and(|wat| wat.contains("(call $op_known_case_left")));
        assert!(!core::str::from_utf8(baseline.wat())
            .is_ok_and(|wat| wat.contains("(export \"cleanup\"")));
        assert!(core::str::from_utf8(explicit_baseline.wat())
            .is_ok_and(|wat| wat.contains("(export \"cleanup\"")));
    }

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

    /// FNV-1a of an emitted module, enough to pin its exact bytes in a fixture.
    fn wat_digest(wat: &[u8]) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in wat {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        hash
    }

    fn fixture_limits() -> noble_contracts::Limits {
        noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        }
    }

    // Recorded from the lowering before its loops were restructured for the
    // pinned Aeneas extraction (issue #97): every charge, grant, allocation and
    // emitted byte of these ordinary, experimental, calculator and nominal
    // fixtures must stay identical.
    #[test]
    fn restructured_lowering_keeps_its_recorded_accounting_and_bytes() {
        let ordinary_session = noble_contracts::source::Session::new();
        let known_sum = ordinary_session
            .prepare(b"10 inl [ 1 + ] [ 2 + ] case", &[], fixture_limits())
            .unwrap_or_else(|_| panic!("known-sum source refused"));
        let submission = known_sum
            .submission()
            .unwrap_or_else(|| panic!("missing known-sum submission"));
        let ordinary = Compiler::new()
            .prepare(submission)
            .unwrap_or_else(|_| panic!("ordinary compile refused"));
        let ordinary_metrics = super::CompileMetrics {
            work_initial: 512_000_000,
            work_preflight_grant: 0,
            work_environment_grant: 0,
            work_plan_grant: 0,
            work_issued: 512_000_000,
            work_spent: 256_109_871,
            environment_rows: 24,
            environment_work: 0,
            named_bodies: 0,
            max_type_nodes: 3,
            type_nodes_cap: 256,
            signature_keys: 7,
            signature_key_charge: 0,
            identity_charge: 0,
            table_end: 13,
            text_end: 262_274,
            wat_bytes: 62_439,
            type_directed_specializations: 0,
            type_directed_fallback_operations: 0,
        };
        assert_eq!(ordinary.metrics(), ordinary_metrics);
        assert_eq!(wat_digest(ordinary.wat()), 0x4db9_0ea5_8a6a_6d3d);
        let experimental = Compiler::new_type_directed_experimental()
            .prepare(submission)
            .unwrap_or_else(|_| panic!("experimental compile refused"));
        assert_eq!(
            experimental.metrics(),
            super::CompileMetrics {
                work_spent: 256_109_876,
                wat_bytes: 63_045,
                type_directed_specializations: 1,
                type_directed_fallback_operations: 4,
                ..ordinary_metrics
            }
        );
        assert_eq!(wat_digest(experimental.wat()), 0x01dc_dfc9_8cfc_9c28);

        let mut calculator = noble_contracts::source::Session::new_calculator();
        let defined = calculator
            .prepare(b"def identity_epoch_word [ 1 drop ]", &[], fixture_limits())
            .unwrap_or_else(|_| panic!("calculator definition refused"));
        calculator
            .commit(defined)
            .unwrap_or_else(|_| panic!("calculator definition commit refused"));
        let call = calculator
            .prepare(b"identity_epoch_word", &[], fixture_limits())
            .unwrap_or_else(|_| panic!("calculator call refused"));
        let submission = call
            .submission()
            .unwrap_or_else(|| panic!("missing calculator submission"));
        let calculator_metrics = super::CompileMetrics {
            work_initial: 512_000_000,
            work_preflight_grant: 37_325,
            work_environment_grant: 1_790_468,
            work_plan_grant: 0,
            work_issued: 513_827_793,
            work_spent: 257_604_957,
            environment_rows: 33,
            environment_work: 447_617,
            named_bodies: 1,
            max_type_nodes: 1,
            type_nodes_cap: 256,
            signature_keys: 3,
            signature_key_charge: 0,
            identity_charge: 0,
            table_end: 7,
            text_end: 524_288,
            wat_bytes: 80_934,
            type_directed_specializations: 0,
            type_directed_fallback_operations: 0,
        };
        for prepared in [
            Compiler::new_calculator().prepare(submission),
            Compiler::new_calculator().prepare_quiescent(submission),
            Compiler::new_calculator().prepare_program_free(submission),
        ] {
            let prepared = prepared.unwrap_or_else(|_| panic!("calculator compile refused"));
            assert_eq!(prepared.metrics(), calculator_metrics);
            assert_eq!(wat_digest(prepared.wat()), 0x0e66_33b3_71d5_56e0);
        }

        let module = b"module capsule@1 [
            opaque Secret Pair<I64,I64> private
            export Secret
            signature make forall<S:stack> [ S -- S Secret ! pure ]
            export make
            def make [ 7 11 pair Secret.new ]
            signature sum forall<S:stack> [ S Secret -- S I64 ! pure ]
            export sum
            def sum [ Secret.into unpair + ]
        ]";
        let session = noble_contracts::source::ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator module session refused"));
        let pending = session
            .prepare(module, &[], fixture_limits())
            .unwrap_or_else(|_| panic!("nominal module refused"));
        let (session, installed) = session.commit(pending);
        installed.unwrap_or_else(|_| panic!("nominal module not installed"));
        let make = session
            .prepare(b"capsule@1.make", &[], fixture_limits())
            .unwrap_or_else(|_| panic!("nominal creator refused"));
        let carrier = make.output().to_vec();
        let mut compiler = Compiler::new_calculator();
        let created = compiler
            .prepare(
                make.submission()
                    .unwrap_or_else(|| panic!("missing creator submission")),
            )
            .unwrap_or_else(|_| panic!("nominal creator compile refused"));
        let make_metrics = super::CompileMetrics {
            work_preflight_grant: 70_444,
            work_environment_grant: 1_825_284,
            work_issued: 513_895_728,
            work_spent: 257_677_711,
            environment_rows: 35,
            environment_work: 456_321,
            max_type_nodes: 4,
            signature_keys: 5,
            wat_bytes: 89_382,
            ..calculator_metrics
        };
        assert_eq!(created.metrics(), make_metrics);
        assert_eq!(wat_digest(created.wat()), 0xb780_9508_aaf8_180b);
        compiler
            .commit(created)
            .unwrap_or_else(|_| panic!("nominal creator commit refused"));
        let (session, committed) = session.commit(make);
        committed.unwrap_or_else(|_| panic!("nominal creator not committed"));
        let sum = session
            .prepare(b"capsule@1.sum", &carrier, fixture_limits())
            .unwrap_or_else(|_| panic!("nominal consumer refused"));
        let consumed = compiler
            .prepare(
                sum.submission()
                    .unwrap_or_else(|| panic!("missing consumer submission")),
            )
            .unwrap_or_else(|_| panic!("nominal consumer compile refused"));
        assert_eq!(
            consumed.metrics(),
            super::CompileMetrics {
                work_preflight_grant: 75_416,
                work_environment_grant: 1_827_332,
                work_issued: 513_902_748,
                work_spent: 257_687_918,
                environment_work: 456_833,
                signature_keys: 6,
                signature_key_charge: 36,
                identity_charge: 38,
                table_end: 9,
                wat_bytes: 87_789,
                ..make_metrics
            }
        );
        assert_eq!(wat_digest(consumed.wat()), 0x4220_b858_9924_b696);
    }

    #[test]
    fn runtime_signature_catalog_lists_descriptors_until_the_first_defective_one() {
        let session = noble_contracts::source::Session::new();
        let known_sum = session
            .prepare(b"10 inl [ 1 + ] [ 2 + ] case", &[], fixture_limits())
            .unwrap_or_else(|_| panic!("known-sum source refused"));
        let submission = known_sum
            .submission()
            .unwrap_or_else(|| panic!("missing known-sum submission"));
        let mut prepared = Compiler::new()
            .prepare(submission)
            .unwrap_or_else(|_| panic!("ordinary compile refused"));
        assert!(matches!(
            prepared.runtime_signature_catalog(),
            Err(crate::Diagnostic::Invalid)
        ));
        prepared.next.live = true;
        let descriptors = prepared.next.descriptors.clone();
        assert!(descriptors.len() >= 2);
        let count = u32::try_from(descriptors.len())
            .unwrap_or_else(|_| panic!("descriptor count exceeds u32"));
        let mut expected = count.to_le_bytes().to_vec();
        for (index, (address, length)) in descriptors.iter().enumerate() {
            expected.extend_from_slice(&address.to_le_bytes());
            expected.extend_from_slice(&length.to_le_bytes());
            expected.extend_from_slice(&prepared.next.signatures.keys[index]);
        }
        assert_eq!(prepared.runtime_signature_catalog().ok(), Some(expected));

        // A defect after a valid first descriptor refuses the whole catalogue.
        prepared.next.descriptors[1].1 += 1;
        assert!(matches!(
            prepared.runtime_signature_catalog(),
            Err(crate::Diagnostic::Defective)
        ));
        prepared.next.descriptors[1] = descriptors[1];
        prepared.next.descriptors[1].0 = super::TEXT_START - 1;
        assert!(matches!(
            prepared.runtime_signature_catalog(),
            Err(crate::Diagnostic::Defective)
        ));
        prepared.next.descriptors[1].0 = super::MEMORY_BYTES;
        prepared.next.descriptors[1].1 = 1;
        prepared.next.signatures.keys[1] = alloc::vec![0];
        assert!(matches!(
            prepared.runtime_signature_catalog(),
            Err(crate::Diagnostic::Defective)
        ));
    }

    #[test]
    fn reusable_retained_bytes_counts_every_identity_and_literal_record() {
        let mut calculator = noble_contracts::source::Session::new_calculator();
        let defined = calculator
            .prepare(b"def retained_word [ 1 drop ]", &[], fixture_limits())
            .unwrap_or_else(|_| panic!("calculator definition refused"));
        calculator
            .commit(defined)
            .unwrap_or_else(|_| panic!("calculator definition commit refused"));
        let call = calculator
            .prepare(
                b"\"abc\" text.length \"de\" text.length retained_word",
                &[],
                fixture_limits(),
            )
            .unwrap_or_else(|_| panic!("calculator call refused"));
        let submission = call
            .submission()
            .unwrap_or_else(|| panic!("missing calculator submission"));
        let prepared = Compiler::new_calculator()
            .prepare_program_free(submission)
            .unwrap_or_else(|_| panic!("program-free compile refused"));
        let reusable = prepared
            .reusable()
            .unwrap_or_else(|| panic!("program-free preparation is reusable"));
        assert!(!prepared.next.identities.is_empty());
        assert_eq!(prepared.next.static_literals.len(), 2);
        let mut expected = core::mem::size_of::<super::Reusable>()
            + reusable.wat().len()
            + prepared.next.signatures.retained_bytes()
            + prepared.next.descriptors.len() * 8;
        for key in prepared.next.static_literals.keys() {
            expected += key.len() + 64;
        }
        for identity in &prepared.next.identities {
            expected += identity.recipe.len() + 64;
        }
        assert_eq!(reusable.retained_bytes(), expected);
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
            preflight::check(
                &original,
                super::ProfileSelection {
                    calculator: false,
                    checked_i64: false,
                    text_cursor: true,
                    declared_text: true,
                },
                &mut work,
            )
            .is_ok(),
            "whole-source preflight exhausted before declared-text admission"
        );
        let checked = admission::check(
            &original, false, false, true, false, false, true, true, &mut work,
        )
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
            .prepare_from(&original, compiler.functions, false, false, true)
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
                .prepare_from(&changed, compiler.functions, false, false, true)
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
        compiler.prepare_from(submission, compiler.functions, false, false, true)
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

#[cfg(test)]
mod origin_operand_debug_tests {
    //! The recursive origin types' hand-written `Debug` must render exactly as
    //! the derives they replace, and recipe equality must agree with the
    //! derived `PartialEq`; `mirror` keeps those derives over mirrored trees.
    #![allow(
        tigerstyle::non_trait_imports,
        tigerstyle::no_recursion,
        tigerstyle::path_segment_repetition,
        reason = "Owner: noble-wasm-maintainers; the origin types under test are imported directly, the mirror conversions follow the test trees recursively, and the mirror and test names repeat the module's origin/debug subject on purpose."
    )]
    use super::{
        OriginFixedArithmetic, OriginFixedExpression, OriginI64, OriginOperand, OriginProgramValue,
    };
    use alloc::{format, rc::Rc, vec, vec::Vec};

    mod mirror {
        use super::super::OriginFixedArithmetic;
        use alloc::rc::Rc;

        #[derive(Debug, PartialEq, Eq)]
        pub enum OriginFixedExpression {
            Literal(i64),
            Arithmetic {
                kind: OriginFixedArithmetic,
                left: Rc<Self>,
                right: Rc<Self>,
                nodes: usize,
                depth: usize,
            },
        }

        #[derive(Debug)]
        #[expect(
            dead_code,
            reason = "Owner: noble-wasm-maintainers; the mirror's payloads are read only by its derived Debug rendering under test."
        )]
        pub enum OriginI64 {
            RootInput(u32),
            Fixed {
                value: i64,
                recipe: Rc<OriginFixedExpression>,
            },
        }

        #[derive(Debug)]
        #[expect(
            dead_code,
            reason = "Owner: noble-wasm-maintainers; the mirror's payloads are read only by its derived Debug rendering under test."
        )]
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
                left: Rc<Self>,
                right: Rc<Self>,
            },
        }

        #[derive(Debug)]
        #[expect(
            dead_code,
            reason = "Owner: noble-wasm-maintainers; the mirror's payloads are read only by its derived Debug rendering under test."
        )]
        pub enum OriginOperand {
            I64(OriginI64),
            Program(Rc<OriginProgramValue>),
        }
    }

    fn mirrored_recipe(recipe: &OriginFixedExpression) -> Rc<mirror::OriginFixedExpression> {
        Rc::new(match recipe {
            OriginFixedExpression::Literal(value) => mirror::OriginFixedExpression::Literal(*value),
            OriginFixedExpression::Arithmetic {
                kind,
                left,
                right,
                nodes,
                depth,
            } => mirror::OriginFixedExpression::Arithmetic {
                kind: *kind,
                left: mirrored_recipe(left),
                right: mirrored_recipe(right),
                nodes: *nodes,
                depth: *depth,
            },
        })
    }

    fn mirrored_i64(value: &OriginI64) -> mirror::OriginI64 {
        match value {
            OriginI64::RootInput(input) => mirror::OriginI64::RootInput(*input),
            OriginI64::Fixed { value, recipe } => mirror::OriginI64::Fixed {
                value: *value,
                recipe: mirrored_recipe(recipe),
            },
        }
    }

    fn mirrored_value(value: &OriginProgramValue) -> Rc<mirror::OriginProgramValue> {
        Rc::new(match value {
            OriginProgramValue::Static {
                node,
                program_index,
            } => mirror::OriginProgramValue::Static {
                node: *node,
                program_index: *program_index,
            },
            OriginProgramValue::Quote {
                node,
                table_entry,
                operand,
            } => mirror::OriginProgramValue::Quote {
                node: *node,
                table_entry: *table_entry,
                operand: mirrored(operand),
            },
            OriginProgramValue::Compose {
                node,
                table_entry,
                left,
                right,
            } => mirror::OriginProgramValue::Compose {
                node: *node,
                table_entry: *table_entry,
                left: mirrored_value(left),
                right: mirrored_value(right),
            },
        })
    }

    fn mirrored(operand: &OriginOperand) -> mirror::OriginOperand {
        match operand {
            OriginOperand::I64(value) => mirror::OriginOperand::I64(mirrored_i64(value)),
            OriginOperand::Program(value) => mirror::OriginOperand::Program(mirrored_value(value)),
        }
    }

    fn literal(value: i64) -> Rc<OriginFixedExpression> {
        Rc::new(OriginFixedExpression::Literal(value))
    }

    fn arithmetic(
        kind: OriginFixedArithmetic,
        left: Rc<OriginFixedExpression>,
        right: Rc<OriginFixedExpression>,
        nodes: usize,
        depth: usize,
    ) -> Rc<OriginFixedExpression> {
        Rc::new(OriginFixedExpression::Arithmetic {
            kind,
            left,
            right,
            nodes,
            depth,
        })
    }

    /// Recipes covering every variant and field, with value-equal trees in
    /// separate allocations and a shared subtree.
    fn recipes() -> Vec<Rc<OriginFixedExpression>> {
        let shared = arithmetic(OriginFixedArithmetic::Add, literal(1), literal(2), 3, 2);
        vec![
            literal(-5),
            literal(-5),
            literal(3),
            Rc::clone(&shared),
            arithmetic(OriginFixedArithmetic::Add, literal(1), literal(2), 3, 2),
            arithmetic(OriginFixedArithmetic::Sub, literal(1), literal(2), 3, 2),
            arithmetic(OriginFixedArithmetic::Add, literal(1), literal(2), 4, 2),
            arithmetic(OriginFixedArithmetic::Add, literal(1), literal(2), 3, 3),
            arithmetic(OriginFixedArithmetic::Add, literal(1), literal(9), 3, 2),
            arithmetic(
                OriginFixedArithmetic::Mul,
                Rc::clone(&shared),
                literal(7),
                5,
                3,
            ),
            arithmetic(
                OriginFixedArithmetic::Mul,
                arithmetic(OriginFixedArithmetic::Add, literal(1), literal(2), 3, 2),
                literal(7),
                5,
                3,
            ),
            arithmetic(
                OriginFixedArithmetic::Mul,
                arithmetic(OriginFixedArithmetic::Add, literal(1), literal(8), 3, 2),
                literal(7),
                5,
                3,
            ),
            arithmetic(
                OriginFixedArithmetic::Mul,
                literal(7),
                Rc::clone(&shared),
                5,
                3,
            ),
        ]
    }

    fn samples() -> Vec<OriginOperand> {
        let node = noble_kernel::untrusted::NodeId(7);
        let leaf = Rc::new(OriginProgramValue::Static {
            node,
            program_index: 3,
        });
        let quoted = Rc::new(OriginProgramValue::Quote {
            node,
            table_entry: 2,
            operand: OriginOperand::I64(OriginI64::RootInput(1)),
        });
        let composed = Rc::new(OriginProgramValue::Compose {
            node,
            table_entry: 4,
            left: Rc::clone(&leaf),
            right: Rc::clone(&quoted),
        });
        let fixed = Rc::new(OriginProgramValue::Quote {
            node,
            table_entry: 5,
            operand: OriginOperand::I64(OriginI64::Fixed {
                value: 21,
                recipe: arithmetic(OriginFixedArithmetic::Mul, literal(3), literal(7), 3, 2),
            }),
        });
        let nested = Rc::new(OriginProgramValue::Quote {
            node,
            table_entry: 6,
            operand: OriginOperand::Program(Rc::clone(&composed)),
        });
        let mut operands = vec![
            OriginOperand::I64(OriginI64::RootInput(0)),
            OriginOperand::I64(OriginI64::Fixed {
                value: -5,
                recipe: Rc::new(OriginFixedExpression::Literal(-5)),
            }),
            OriginOperand::Program(leaf),
            OriginOperand::Program(quoted),
            OriginOperand::Program(composed),
            OriginOperand::Program(fixed),
            OriginOperand::Program(nested),
        ];
        for recipe in recipes() {
            operands.push(OriginOperand::I64(OriginI64::Fixed { value: 0, recipe }));
        }
        operands
    }

    #[test]
    fn debug_matches_the_derived_rendering() {
        for operand in samples() {
            let expected = mirrored(&operand);
            assert_eq!(format!("{operand:?}"), format!("{expected:?}"));
            assert_eq!(format!("{operand:#?}"), format!("{expected:#?}"));
            assert_eq!(format!("{operand:4?}"), format!("{expected:4?}"));
            if let OriginOperand::Program(value) = &operand {
                let expected = mirrored_value(value);
                assert_eq!(format!("{value:?}"), format!("{expected:?}"));
                assert_eq!(format!("{value:#?}"), format!("{expected:#?}"));
            }
        }
        for recipe in recipes() {
            let expected = mirrored_recipe(&recipe);
            assert_eq!(format!("{recipe:?}"), format!("{expected:?}"));
            assert_eq!(format!("{recipe:#?}"), format!("{expected:#?}"));
        }
    }

    #[test]
    fn recipe_equality_matches_the_derived_equality() {
        let recipes = recipes();
        for left in &recipes {
            for right in &recipes {
                let is_expected = mirrored_recipe(left) == mirrored_recipe(right);
                assert_eq!(**left == **right, is_expected, "{left:?} == {right:?}");
                assert_eq!(left == right, is_expected, "Rc {left:?} == {right:?}");
                assert_eq!(**left != **right, !is_expected, "{left:?} != {right:?}");
            }
        }
        assert!(
            recipes[0] == recipes[1],
            "value-equal literals in separate allocations"
        );
        assert!(
            recipes[3] == recipes[4],
            "value-equal trees in separate allocations"
        );
        assert!(
            recipes[9] == recipes[10],
            "a shared subtree equals its separate copy"
        );
        assert!(
            recipes[10] != recipes[11],
            "a difference deep in the left subtree"
        );
    }
}
