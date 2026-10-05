//! Source-bound, kernel-rechecked selection. A source target is not a proof of
//! its behavior or of any compiled artifact.

/// A selected definition's identity within one retained source session. Neither
/// field is portable across sessions; the borrowed target also carries its full
/// source and accepted recipe, which must be compared before publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedDefinitionId {
    index: u32,
    identity: u64,
}

impl CheckedDefinitionId {
    pub const fn index(self) -> u32 { self.index }
    pub const fn identity(self) -> u64 { self.identity }
}

/// Refusal of source selection or correspondence. `UnsupportedProfile` means
/// that the checked target lies outside the existing pure unary I64 theorem;
/// in particular this is not a generic Account/effect/capture proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceProofRefusal {
    UnsupportedProfile,
    StaleContext,
    MissingTarget,
    MissingEvidence,
    MismatchedSource,
    MismatchedDefinition,
    MismatchedInterface,
    MismatchedClaim,
    Assumptions,
    IndependentCheckFailed,
}

/// One source-owned named definition selected by a complete, independently
/// kernel-accepted root invocation. `submission` and `definition` retain the
/// actual checked body, instantiations, environment and source session identity;
/// no producer name or digest alone is an executable target.
pub struct CheckedSelectedTarget<'a> {
    id: CheckedDefinitionId,
    generation: u64,
    source: &'a [u8],
    submission: &'a noble_kernel::execution::Submission,
    definition: &'a noble_kernel::execution::Definition,
    root: noble_kernel::untrusted::Checked,
    body: noble_kernel::untrusted::Checked,
}

impl CheckedSelectedTarget<'_> {
    pub const fn definition_id(&self) -> CheckedDefinitionId { self.id }
    pub const fn source_generation(&self) -> u64 { self.generation }
    pub const fn source(&self) -> &[u8] { self.source }
    pub const fn submission(&self) -> &noble_kernel::execution::Submission { self.submission }
    pub const fn definition(&self) -> &noble_kernel::execution::Definition { self.definition }
    pub const fn root(&self) -> &noble_kernel::untrusted::Checked { &self.root }
    pub const fn body(&self) -> &noble_kernel::untrusted::Checked { &self.body }
}

/// Expected host context, not an attestation. The host must separately
/// authenticate its pinned checker and the installed artifact bytes.
#[derive(Clone, Copy)]
pub struct ProofContext<'a> {
    pub module_generation: u64,
    pub model_revision: &'a str,
    pub checker_revision: &'a str,
}

/// An exact source/definition/interface match against one retained proof
/// record. This is deliberately NOT a proof receipt or release authorization:
/// `commit_verified` trusts its host callback, and this no_std crate cannot
/// attest a Lean process, its assumptions, or installed Wasm bytes.
pub struct SourceCorrespondenceCandidate<'a> {
    pub(crate) target_id: CheckedDefinitionId,
    pub(crate) target_generation: u64,
    pub(crate) module_generation: u64,
    pub(crate) goal: &'a crate::intrinsic::ContractGoal,
    pub(crate) retained: &'a crate::intrinsic::CheckedProof,
    pub(crate) interface: &'a noble_kernel::untrusted::Interface,
}

impl SourceCorrespondenceCandidate<'_> {
    pub const fn definition_id(&self) -> CheckedDefinitionId { self.target_id }
    pub const fn target_generation(&self) -> u64 { self.target_generation }
    pub const fn module_generation(&self) -> u64 { self.module_generation }
    pub const fn goal(&self) -> &crate::intrinsic::ContractGoal { self.goal }
    pub const fn retained_proof(&self) -> &crate::intrinsic::CheckedProof { self.retained }
    pub const fn interface(&self) -> &noble_kernel::untrusted::Interface { self.interface }
    /// The MC1 theorem has no captured inputs or external premises. A nonempty
    /// capture or dependency is refused before this candidate can be returned.
    pub const fn captures(&self) -> &[crate::companion::CaptureBinding] { &[] }
    pub const fn assumptions(&self) -> &[crate::intrinsic::ProofDependency] { &[] }
}

impl super::Session {
    /// Select only the current retained definition from its complete accepted
    /// single-invocation submission. Recheck both root and definition body in
    /// the exact emitted environment; declarations alone have no authority.
    pub fn checked_selected_target<'a>(
        &'a self,
        prepared: &'a super::Prepared,
        name: &str,
    ) -> Result<CheckedSelectedTarget<'a>, SourceProofRefusal> {
        use noble_kernel::{acceptance, contracts::Behavior, untrusted::{Node, Outcome, Request}};
        use SourceProofRefusal as Refusal;
        if self.live_slots.is_none() {
            return Err(Refusal::UnsupportedProfile);
        }
        if !self.is_current_namespace(prepared) {
            return Err(Refusal::StaleContext);
        }
        let (index, retained) = self.definitions.iter().enumerate().rev()
            .find(|(_, definition)| definition.name == name)
            .ok_or(Refusal::MissingTarget)?;
        let source = self.definition_source(index).map_err(|_| Refusal::MismatchedSource)?;
        let submission = prepared.submission().ok_or(Refusal::MissingTarget)?;
        let [definition] = submission.definitions.as_slice() else {
            return Err(Refusal::MismatchedDefinition);
        };
        let index = u32::try_from(index).map_err(|_| Refusal::MismatchedDefinition)?;
        if prepared.selected_root != Some(index)
            || definition.identity != retained.identity
            || submission.environment.kind(definition.definition) != Some(Behavior::Named)
            || usize::try_from(definition.definition.0).ok()
                .and_then(|slot| submission.environment.definition_owners.get(slot)) != Some(&None)
            || !submission.body.texts.is_empty()
            || submission.body.candidate.body.len() != 1
            || submission.body.candidate.nodes.len() != 1
            || !matches!(submission.body.candidate.nodes.first(),
                Some(Node::Invocation { def, .. }) if *def == definition.definition)
            || submission.body.candidate.body[0] != noble_kernel::untrusted::NodeId(0)
        {
            return Err(Refusal::MismatchedDefinition);
        }
        let Outcome::Accepted(root) = acceptance::check(
            &submission.environment, &submission.request, &submission.body.candidate,
        ) else {
            return Err(Refusal::IndependentCheckFailed);
        };
        let request = Request {
            input_bytes: submission.request.input_bytes,
            expected: definition.expected.clone(),
            limits: submission.request.limits,
        };
        let Outcome::Accepted(body) = acceptance::check(
            &submission.environment, &request, &definition.body.candidate,
        ) else {
            return Err(Refusal::IndependentCheckFailed);
        };
        if root.interface.stack_in != body.interface.stack_in
            || root.interface.stack_out != body.interface.stack_out
            || root.interface.effects != body.interface.effects
            || root.interface.stack_in != submission.request.expected.stack_in
            || root.interface.stack_out != submission.request.expected.stack_out
        {
            return Err(Refusal::MismatchedInterface);
        }
        Ok(CheckedSelectedTarget {
            id: CheckedDefinitionId { index, identity: retained.identity },
            generation: self.generation,
            source, submission, definition, root, body,
        })
    }
}
