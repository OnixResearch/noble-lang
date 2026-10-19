/// refuses public binding mutations must each be refused by contract preparation.
pub(super) fn refuses_public_binding_mutations(
    contract: &noble_contracts::intrinsic::ContractGoal,
) -> Result<(), String> {
    let mut changed = contract.clone();
    changed.subject.definition_source = b"def increment [ 2 + ]".to_vec();
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged definition bytes"
    );
    let mut changed = contract.clone();
    let at = changed
        .subject
        .module_source
        .windows(b"def increment [ 1 + ]".len())
        .position(|window| window == b"def increment [ 1 + ]")
        .ok_or("missing source occurrence")?;
    let literal = at
        .checked_add(b"def increment [ ".len())
        .ok_or("source literal offset overflows")?;
    changed.subject.module_source[literal] = b'2';
    changed.subject.definition_source = b"def increment [ 2 + ]".to_vec();
    changed.subject.source_dependencies[0].full_source = changed.subject.module_source.clone();
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "source changed but accepted recipe stayed old"
    );
    let mut changed = contract.clone();
    changed.subject.source_span.start += 1;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged lexical span"
    );
    let mut changed = contract.clone();
    changed.subject.definition_ordinal = 0;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged source ordinal"
    );
    let mut changed = contract.clone();
    changed.subject.definition_owner += 1;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged owner"
    );
    Ok(())
}

/// refuses recipe mutations must each be refused by contract preparation.
pub(super) fn refuses_recipe_mutations(
    contract: &noble_contracts::intrinsic::ContractGoal,
) -> Result<(), String> {
    let mut changed = contract.clone();
    changed.subject.source_dependencies[0].full_source[0] = b'x';
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged source dependency"
    );
    let mut changed = contract.clone();
    changed.subject.accepted_submission.environment.kinds[4] =
        noble_kernel::contracts::Behavior::Equals;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged builtin behavior"
    );
    let mut changed = contract.clone();
    let definition = changed
        .subject
        .accepted_submission
        .definitions
        .iter_mut()
        .find(|definition| definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    let first = usize::try_from(definition.body.candidate.body[0].0)
        .map_err(|_| "accepted literal node exceeds host address space")?;
    let noble_kernel::untrusted::Node::Literal { lit, .. } =
        &mut definition.body.candidate.nodes[first]
    else {
        return Err("missing literal".into());
    };
    *lit = noble_kernel::untrusted::Lit::I64(2);
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "same interface but different accepted code"
    );
    Ok(())
}

/// refuses evidence mutations must each be refused by contract preparation.
pub(super) fn refuses_evidence_mutations(
    contract: &noble_contracts::intrinsic::ContractGoal,
) -> Result<(), String> {
    let mut changed = contract.clone();
    let definition = changed
        .subject
        .accepted_submission
        .definitions
        .iter_mut()
        .find(|definition| definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    let first = usize::try_from(definition.body.candidate.body[0].0)
        .map_err(|_| "accepted literal node exceeds host address space")?;
    let noble_kernel::untrusted::Node::Literal { inst, .. } =
        &mut definition.body.candidate.nodes[first]
    else {
        return Err("missing literal".into());
    };
    inst.bindings.clear();
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged literal instantiation"
    );
    let mut changed = contract.clone();
    let definition = changed
        .subject
        .accepted_submission
        .definitions
        .iter_mut()
        .find(|definition| definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    let second = usize::try_from(definition.body.candidate.body[1].0)
        .map_err(|_| "accepted add node exceeds host address space")?;
    let noble_kernel::untrusted::Node::Invocation { inst, .. } =
        &mut definition.body.candidate.nodes[second]
    else {
        return Err("missing add".into());
    };
    inst.bindings.clear();
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged add instantiation"
    );
    Ok(())
}

/// refuses specialization and capture mutations must each be refused by contract preparation.
pub(super) fn refuses_specialization_and_capture_mutations(
    contract: &noble_contracts::intrinsic::ContractGoal,
) -> Result<(), String> {
    let mut changed = contract.clone();
    let definition = changed
        .subject
        .accepted_submission
        .definitions
        .iter_mut()
        .find(|definition| definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    definition.body.candidate.body = vec![
        noble_kernel::untrusted::NodeId(0),
        noble_kernel::untrusted::NodeId(0),
    ];
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "wrong complete recipe"
    );
    let mut changed = contract.clone();
    let definition = changed
        .subject
        .accepted_submission
        .definitions
        .iter_mut()
        .find(|definition| definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    definition.definition.0 += 1;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "wrong specialization slot"
    );
    let mut changed = contract.clone();
    let root = usize::try_from(changed.subject.accepted_submission.body.candidate.body[0].0)
        .map_err(|_| "accepted root node exceeds host address space")?;
    let noble_kernel::untrusted::Node::Invocation { inst, .. } =
        &mut changed.subject.accepted_submission.body.candidate.nodes[root]
    else {
        return Err("missing root invocation".into());
    };
    inst.bindings
        .push(noble_kernel::words::Binding::Stack(vec![]));
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::super::LIMITS)
            .is_err(),
        "forged root instantiation"
    );
    Ok(())
}
