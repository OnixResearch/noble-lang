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
mod transaction;
mod types;

const TABLE_LIMIT: u32 = 16_384;
const TEXT_START: u32 = 262_144;
const MEMORY_BYTES: u32 = 1_048_576;
const NODE_LIMIT: usize = 4096;
const DEFINITION_LIMIT: usize = 256;

// Core live sessions reserve 24/25 for their two checked operations.
// Ordinary and declared sessions retain their original fixed prefixes.
fn builtin_count(env: &noble_kernel::contracts::Env) -> u32 {
    if env.text_cursor && !env.declared_modules {
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
    remaining: u64,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the budget belongs to one private preparation and tracks consumed work without modifying the borrowed submission or published compiler."
)]
impl Work {
    const fn charge(&mut self, amount: u64) -> Result<(), crate::Diagnostic> {
        self.remaining = match self.remaining.checked_sub(amount) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
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
}

/// Persistent semantic identities and disjoint compiled-code allocations.
pub struct Compiler {
    live: bool,
    live_slots: bool,
    text_cursor: bool,
    generation: u32,
    functions: u32,
    retention_revision: u64,
    free_code: alloc::vec::Vec<FreeCode>,
    installed_code: alloc::vec::Vec<CodeSpan>,
    text_end: u32,
    signatures: crate::signatures::Pool,
    descriptors: alloc::vec::Vec<(u32, u32)>,
    identities: alloc::vec::Vec<Identity>,
}

/// One installed module's physical table allocation. The generation identifies
/// its owner even after the same table indices have been reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeSpan {
    pub start: u32,
    pub length: u32,
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
    QuoteI64 { input: u32, output: u32, witness: u32 },
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

#[derive(Clone, Copy)]
struct FreeCode {
    start: u32,
    length: u32,
}

#[derive(Clone)]
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
}

impl Prepared {
    /// The complete module. Instantiation does not execute its body.
    pub fn wat(&self) -> &[u8] {
        &self.wat
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
    fn reserve_code(&mut self, length: u32) -> Result<CodeSpan, crate::Diagnostic> {
        if length == 0 {
            return Ok(CodeSpan {
                start: self.functions,
                length: 0,
                generation: self.generation,
            });
        }
        let mut start = self.functions;
        if self.live_slots {
            if let Some(index) = self.free_code.iter().position(|free| free.length >= length) {
                let free = &mut self.free_code[index];
                start = free.start;
                free.start += length;
                free.length -= length;
                if free.length == 0 {
                    self.free_code.remove(index);
                }
            } else {
                self.functions = match self.functions.checked_add(length) {
                    Some(end) if end <= TABLE_LIMIT => end,
                    Some(_) | None => return Err(crate::Diagnostic::Exhausted),
                };
            }
        } else {
            self.functions = match self.functions.checked_add(length) {
                Some(end) if end <= TABLE_LIMIT => end,
                Some(_) | None => return Err(crate::Diagnostic::Exhausted),
            };
        }
        let span = CodeSpan {
            start,
            length,
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
            generation: 0,
            functions: 4,
            retention_revision: 0,
            free_code: alloc::vec::Vec::new(),
            installed_code: alloc::vec::Vec::new(),
            text_end: TEXT_START,
            signatures: crate::signatures::Pool::source(),
            descriptors: alloc::vec::Vec::new(),
            identities: alloc::vec::Vec::new(),
        }
    }

    /// A separate admission profile: the ordinary compiler never emits the
    /// proposal or generation imports.
    pub fn new_live() -> Self {
        Self { live: true, ..Self::new() }
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
        Self { text_cursor: true, ..Self::new() }
    }

    /// Independently accept and lower without changing existing code identities.
    pub fn prepare(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        self.prepare_with_origin(submission, None)
    }

    fn prepare_with_origin(
        &self,
        submission: &noble_kernel::execution::Submission,
        selected: Option<&noble_contracts::source::proof::CheckedSelectedTarget<'_>>,
    ) -> Result<Prepared, crate::Diagnostic> {
        let mut work = Work {
            remaining: u64::from(submission.request.limits.work),
        };
        attempt!(preflight::check(submission, &mut work));
        let checked = attempt!(admission::check(
            submission,
            self.live,
            self.live_slots,
            self.text_cursor,
            &mut work
        ));
        attempt!(admission::meter::compilation(
            &checked,
            &self.signatures,
            &mut work
        ));
        let mut identity = 0usize;
        let mut failure = None;
        while identity < self.identities.len() {
            match work.entries(self.identities[identity].recipe.len()) {
                Ok(()) => identity += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        let mut next = Self {
            live: self.live,
            live_slots: self.live_slots,
            text_cursor: self.text_cursor,
            generation: match self.generation.checked_add(1) {
                Some(value) => value,
                None => return Err(crate::Diagnostic::Exhausted),
            },
            functions: self.functions,
            retention_revision: self.retention_revision,
            free_code: self.free_code.clone(),
            installed_code: self.installed_code.clone(),
            text_end: self.text_end,
            signatures: self.signatures.clone(),
            descriptors: self.descriptors.clone(),
            identities: self.identities.clone(),
        };
        attempt!(admission::identity::check(
            submission,
            &mut next.identities,
            &mut work
        ));
        let mut plan = attempt!(plan::lower(submission, &checked, &mut next, &mut work));
        let wat = attempt!(emit::module(&plan, self.generation));
        let code_span = CodeSpan {
            start: plan.first_function,
            length: plan.functions - plan.first_function,
            generation: next.generation,
        };
        let target_metadata = if self.live_slots {
            let root = attempt!(plan.programs.get(plan.root).ok_or(crate::Diagnostic::Invalid));
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
                let checked_body = attempt!(checked.definitions.get(index)
                    .ok_or(crate::Diagnostic::Invalid));
                let program_index = attempt!(plan.definition_roots.get(index).copied()
                    .ok_or(crate::Diagnostic::Invalid));
                let program = attempt!(plan.programs.get(program_index)
                    .ok_or(crate::Diagnostic::Invalid));
                let recipe = attempt!(next.identities.iter()
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
                let logical = attempt!(program.logical.take()
                    .ok_or(crate::Diagnostic::Defective));
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
                let declaration = attempt!(submission.environment.nominal(*id)
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
        Ok(Prepared {
            base: transaction::Base::capture(self),
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
        let mut prepared = attempt!(self.prepare_with_origin(selected.submission(), Some(selected)));
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
        if !self.live_slots || span.length == 0 || span.start < 4 {
            return Err(crate::Diagnostic::Invalid);
        }
        let Some(index) = self.installed_code.iter().position(|owned| *owned == span) else {
            return Err(crate::Diagnostic::Invalid);
        };
        let Some(end) = span.start.checked_add(span.length) else {
            return Err(crate::Diagnostic::Invalid);
        };
        if end > self.functions || self.retention_revision == u64::MAX {
            return Err(crate::Diagnostic::Invalid);
        }
        attempt!(verify_and_clear(span));
        self.installed_code.remove(index);
        let mut index = self.free_code.partition_point(|free| free.start < span.start);
        self.free_code.insert(
            index,
            FreeCode {
                start: span.start,
                length: span.length,
            },
        );
        if index > 0 && self.free_code[index - 1].start + self.free_code[index - 1].length
            == self.free_code[index].start
        {
            let right = self.free_code.remove(index);
            self.free_code[index - 1].length += right.length;
            index -= 1;
        }
        if index + 1 < self.free_code.len()
            && self.free_code[index].start + self.free_code[index].length == self.free_code[index + 1].start
        {
            let right = self.free_code.remove(index + 1);
            self.free_code[index].length += right.length;
        }
        if self.free_code[index].start + self.free_code[index].length == self.functions {
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
        assert_eq!((reused.start, reused.length), (4, 2));
        assert_ne!(reused.generation, former.generation);
        assert!(matches!(compiler.retire_code(former, |_| Ok(())), Err(Diagnostic::Invalid)));
        assert!(matches!(
            compiler.retire_code(CodeSpan {
                length: 1,
                ..pinned
            }, |_| Ok(())),
            Err(Diagnostic::Invalid)
        ));
        compiler.generation = 4;
        assert_eq!(compiler.reserve_code(1).ok().expect("last free slot").start, 6);
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
        assert_eq!(compiler.reserve_code(1).ok().expect("fresh span").start, next);

        let mut called = false;
        assert!(matches!(
            compiler.retire_code(CodeSpan { generation: 0, ..occupied }, |_| {
                called = true;
                Ok(())
            }),
            Err(Diagnostic::Invalid)
        ));
        assert!(!called);
        assert_eq!(compiler.installed_code[0], occupied);
    }
}
