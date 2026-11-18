#[expect(
    tigerstyle::renamed_imports,
    reason = "Owner: noble-contracts; the local alias matches the domain vocabulary used throughout this module"
)]
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use crate::source::proof::{
    CheckedSelectedTarget, ProofContext, SourceCorrespondenceCandidate,
    SourceProofRefusal as Refusal,
};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use noble_kernel::{
    contracts::Definition,
    types::{EffSet, Ty},
    untrusted::{Node, NodeId},
};

impl super::ModuleSession {
    /// Compare the *full* checked source target to one host-retained verified
    /// source proof in this immutable namespace. This does not authenticate
    /// the host's Lean callback or attest an installed Wasm artifact; those
    /// independent host gates are prerequisites for any publication.
    #[expect(
        clippy::too_many_arguments,
        reason = "Owner: noble-maintainers; module, version, contract and proof names are independent namespace selectors that refuse separately; bundling them would change the public noble-contracts API and its reviewed source inventory."
    )]
    #[expect(
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-contracts; the parameter list is the internal call shape; bundling it would change the extracted subject"
    )]
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
    )]
    pub fn correspond_checked_target<'a>(
        &'a self,
        module_name: &str,
        version: u32,
        contract_name: &str,
        proof_name: &str,
        target: &'a CheckedSelectedTarget<'_>,
        context: ProofContext<'_>,
        limits: crate::Limits,
    ) -> Result<SourceCorrespondenceCandidate<'a>, Refusal> {
        // MC1 proves only a capture-free, pure I64-to-I64 builtin program.
        // Resource/Account interfaces and effectful live targets have no
        // matching theorem; interface fit alone must never imply proof.
        if is_unsupported_profile(target) {
            return Err(Refusal::UnsupportedProfile);
        }
        if context.module_generation != self.generation {
            return Err(Refusal::StaleContext);
        }
        // Each selector must name exactly one entry: the first match is
        // used, and any later match makes the selection ambiguous.
        let module = self.unique_module(module_name, version)?;
        let contract = unique_contract(module, contract_name)?;
        let proof = unique_published(module, proof_name)?;
        let goal = &contract.goal;
        check_retained_claim(module, goal, proof, proof_name, context)?;
        check_target_definition(goal, target, limits)?;
        Ok(SourceCorrespondenceCandidate {
            target_id: target.definition_id(),
            target_generation: target.source_generation(),
            module_generation: self.generation,
            goal,
            retained: &proof.checked,
            interface: &target.body().interface,
        })
    }

    /// The only module named `module_name` at `version`: none is missing
    /// evidence and a second one makes the selection ambiguous.
    fn unique_module(
        &self,
        module_name: &str,
        version: u32,
    ) -> Result<&super::super::Module, Refusal> {
        let is_module =
            |entry: &super::super::Module| entry.name == module_name && entry.version == version;
        let module_at =
            super::position_first(&self.modules, is_module).ok_or(Refusal::MissingEvidence)?;
        let later_modules = &self.modules[module_at.saturating_add(1)..];
        if super::position_first(later_modules, is_module).is_some() {
            return Err(Refusal::MismatchedSource);
        }
        Ok(&self.modules[module_at])
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether the target lies outside the MC1 profile: an I64-to-I64 pure
/// interface whose body and root have no live sites.
fn is_unsupported_profile(target: &CheckedSelectedTarget<'_>) -> bool {
    let body = target.body();
    body.interface.stack_in != [Ty::I64]
        || body.interface.stack_out != [Ty::I64]
        || body.interface.effects != EffSet::empty()
        || !body.live_sites.is_empty()
        || !target.root().live_sites.is_empty()
}

/// The only contract named `contract_name` in the module: none is missing
/// evidence and a second one makes the claim ambiguous.
fn unique_contract<'a>(
    module: &'a super::super::Module,
    contract_name: &str,
) -> Result<&'a super::super::LogicalContract, Refusal> {
    let is_contract = |entry: &super::super::LogicalContract| entry.name == contract_name;
    let contract_at =
        super::position_first(&module.contracts, is_contract).ok_or(Refusal::MissingEvidence)?;
    let later_contracts = &module.contracts[contract_at.saturating_add(1)..];
    if super::position_first(later_contracts, is_contract).is_some() {
        return Err(Refusal::MismatchedClaim);
    }
    Ok(&module.contracts[contract_at])
}

/// The only published record named `proof_name` in the module: none is
/// missing evidence and a second one makes the claim ambiguous.
fn unique_published<'a>(
    module: &'a super::super::Module,
    proof_name: &str,
) -> Result<&'a super::super::PublishedProof, Refusal> {
    let is_proof = |entry: &super::super::PublishedProof| entry.reference.name == proof_name;
    let proof_at =
        super::position_first(&module.proofs, is_proof).ok_or(Refusal::MissingEvidence)?;
    let later_proofs = &module.proofs[proof_at.saturating_add(1)..];
    if super::position_first(later_proofs, is_proof).is_some() {
        return Err(Refusal::MismatchedClaim);
    }
    Ok(&module.proofs[proof_at])
}

/// The retained record must bind exactly this contract goal, at MC1
/// revisions, to the module's own source with no assumptions, as checked by
/// the expected model and checker revisions.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn check_retained_claim(
    module: &super::super::Module,
    goal: &crate::intrinsic::ContractGoal,
    proof: &super::super::PublishedProof,
    proof_name: &str,
    context: ProofContext<'_>,
) -> Result<(), Refusal> {
    let crate::intrinsic::PendingGoal::Contract { contract: bound } =
        &proof.reference.declaration.goal
    else {
        return Err(Refusal::MismatchedClaim);
    };
    if goal.revision != 1
        || proof.reference.declaration.revision != 1
        || proof.checked.kind != crate::intrinsic::ProofKind::Contract
    {
        return Err(Refusal::UnsupportedProfile);
    }
    if goal_differs(goal, bound) || binding_differs(module, proof, goal) {
        return Err(Refusal::MismatchedClaim);
    }
    if !proof.reference.dependencies.is_empty() {
        return Err(Refusal::Assumptions);
    }
    if checked_differs(proof, proof_name, context) {
        return Err(Refusal::MismatchedClaim);
    }
    Ok(())
}

/// Whether the retained goal differs from the bound one in its contract
/// header or its subject.
fn goal_differs(
    goal: &crate::intrinsic::ContractGoal,
    bound: &crate::intrinsic::ContractGoal,
) -> bool {
    header_differs(goal, bound) || subject_differs(&goal.subject, &bound.subject)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether two goals differ in contract name, span, inputs, outputs,
/// precondition or postcondition.
fn header_differs(
    goal: &crate::intrinsic::ContractGoal,
    bound: &crate::intrinsic::ContractGoal,
) -> bool {
    goal.contract_name != bound.contract_name
        || goal.contract_span != bound.contract_span
        || goal.inputs != bound.inputs
        || goal.outputs != bound.outputs
        || goal.requires != bound.requires
        || goal.ensures != bound.ensures
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether two subject bindings differ in the source, identity, owner, span,
/// ordinal, signature or effects of their definition.
fn subject_differs(
    subject: &crate::intrinsic::SubjectBinding,
    bound: &crate::intrinsic::SubjectBinding,
) -> bool {
    subject.module_source != bound.module_source
        || subject.definition_source != bound.definition_source
        || subject.definition_identity != bound.definition_identity
        || subject.definition_owner != bound.definition_owner
        || subject.source_span != bound.source_span
        || subject.definition_ordinal != bound.definition_ordinal
        || subject.input_types != bound.input_types
        || subject.output_types != bound.output_types
        || subject.effects != bound.effects
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether the retained record or its goal names another module, version or
/// module source than the selected module.
fn binding_differs(
    module: &super::super::Module,
    proof: &super::super::PublishedProof,
    goal: &crate::intrinsic::ContractGoal,
) -> bool {
    proof.reference.module != module.name
        || proof.reference.version != module.version
        || proof.reference.source != module.source
        || goal.subject.module != module.name
        || goal.subject.version != module.version
        || goal.subject.module_source != module.source
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether the checked result is not the MC1 claim of `proof_name`, or either
/// record names another model or checker revision than the context.
fn checked_differs(
    proof: &super::super::PublishedProof,
    proof_name: &str,
    context: ProofContext<'_>,
) -> bool {
    let checked = &proof.checked;
    checked.name != proof_name
        || checked.claim != "MC1Obligation.claim"
        || checked.model_revision != context.model_revision
        || checked.checker_revision != context.checker_revision
        || proof.reference.model_revision != context.model_revision
        || proof.reference.checker_revision != context.checker_revision
}

/// The retained goal must still prepare, name the target's source, and accept
/// exactly the target's definition at its interface.
fn check_target_definition(
    goal: &crate::intrinsic::ContractGoal,
    target: &CheckedSelectedTarget<'_>,
    limits: crate::Limits,
) -> Result<(), Refusal> {
    // The contract was prepared and source-checked on staging and again
    // before commit_verified. Recheck it from its retained, not public,
    // goal to reject a changed kernel/MC1 interpretation at this boundary.
    crate::intrinsic::prepare_contract(goal, limits)
        .map_err(|_| Refusal::IndependentCheckFailed)?;
    if goal.subject.definition_source != target.source() {
        return Err(Refusal::MismatchedSource);
    }
    let accepted = &goal.subject.accepted_submission;
    if accepted.definitions.len() != 1 {
        return Err(Refusal::MismatchedDefinition);
    }
    let defined = &accepted.definitions[0];
    if interface_differs(defined, goal, target) || definition_differs(defined, target) {
        return Err(Refusal::MismatchedDefinition);
    }
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether the accepted definition or the goal's signature differs from the
/// target body's checked interface.
fn interface_differs(
    defined: &noble_kernel::execution::Definition,
    goal: &crate::intrinsic::ContractGoal,
    target: &CheckedSelectedTarget<'_>,
) -> bool {
    let interface = &target.body().interface;
    defined.identity != goal.subject.definition_identity
        || goal.subject.input_types != interface.stack_in
        || goal.subject.output_types != interface.stack_out
        || goal.subject.effects != interface.effects
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
/// Whether the accepted definition differs from the target definition in its
/// expected stacks or effects, its environment is not the canonical wrapping
/// add, its candidate differs, or the target carries text payloads.
fn definition_differs(
    defined: &noble_kernel::execution::Definition,
    target: &CheckedSelectedTarget<'_>,
) -> bool {
    defined.expected.stack_in != target.definition().expected.stack_in
        || defined.expected.stack_out != target.definition().expected.stack_out
        || defined.expected.allowed_effects != target.definition().expected.allowed_effects
        || !crate::intrinsic::canonical_wrapping_add(&target.submission().environment)
        || !same_candidate(&defined.body.candidate, &target.definition().body.candidate)
        || !target.definition().body.texts.is_empty()
}

#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-contracts; the guard is the admission rule verbatim; splitting it would obscure the predicate it states"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn same_candidate(
    left: &noble_kernel::untrusted::Candidate,
    right: &noble_kernel::untrusted::Candidate,
) -> bool {
    if left.format != right.format
        || left.revision != right.revision
        || left.body.len() != 2
        || left.body != [NodeId(0), NodeId(1)]
        || right.body != left.body
        || left.nodes.len() != 2
        || right.nodes.len() != 2
    {
        return false;
    }
    let (
        Node::Literal {
            lit: noble_kernel::untrusted::Lit::I64(a),
            inst: ai,
        },
        Node::Literal {
            lit: noble_kernel::untrusted::Lit::I64(b),
            inst: bi,
        },
    ) = (&left.nodes[0], &right.nodes[0])
    else {
        return false;
    };
    if a != b || !crate::intrinsic::same_inst(ai, bi) {
        return false;
    }
    let (
        Node::Invocation {
            def: Definition(4),
            inst: ai,
        },
        Node::Invocation {
            def: Definition(4),
            inst: bi,
        },
    ) = (&left.nodes[1], &right.nodes[1])
    else {
        return false;
    };
    crate::intrinsic::same_inst(ai, bi)
}
