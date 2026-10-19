#[test]
fn direct_mc1_contract_rejects_inconsistent_public_binding_and_recipe() -> Result<(), String> {
    let source = b"module arithmetic@1 [
  def other [ 2 + ]
  def increment [ 1 + ]
  contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
    requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
  proof 1 p for law [ (export-unary-I64
    (intro (tail Stack) (intro (x I64)
      (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add))
        (bridge (by-exact-append-assoc)) (join (by-exact-result)))))
    (mc1-true-eq-wrap)) ]
]";
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(source, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared
        .proof_obligations()
        .ok_or("missing contract proof")?;
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal
    else {
        return Err("missing resolved contract".into());
    };
    noble_contracts::intrinsic::prepare_contract(contract, super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    noble_contracts::intrinsic::check_batch(batch, super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(contract.subject.definition_ordinal, 1);
    let mut changed = contract.clone();
    changed.subject.definition_source = b"def increment [ 2 + ]".to_vec();
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
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
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "source changed but accepted recipe stayed old"
    );
    let mut changed = contract.clone();
    changed.subject.source_span.start += 1;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "forged lexical span"
    );
    let mut changed = contract.clone();
    changed.subject.definition_ordinal = 0;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "forged source ordinal"
    );
    let mut changed = contract.clone();
    changed.subject.definition_owner += 1;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "forged owner"
    );
    let mut changed = contract.clone();
    changed.subject.source_dependencies[0].full_source[0] = b'x';
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "forged source dependency"
    );
    let mut changed = contract.clone();
    changed.subject.accepted_submission.environment.kinds[4] =
        noble_kernel::contracts::Behavior::Equals;
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
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
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "same interface but different accepted code"
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
    let noble_kernel::untrusted::Node::Literal { inst, .. } =
        &mut definition.body.candidate.nodes[first]
    else {
        return Err("missing literal".into());
    };
    inst.bindings.clear();
    assert!(
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
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
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "forged add instantiation"
    );
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
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
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
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
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
        noble_contracts::intrinsic::prepare_contract(&changed, super::super::LIMITS).is_err(),
        "forged root instantiation"
    );
    Ok(())
}

#[test]
fn source_bound_contract_preserves_formatting_and_refuses_named_v1() -> Result<(), String> {
    let formatted = b"module formatted@1 [
  export increment
  def increment # comment between name and bracket
    [ 1 + ]
  contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
    requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
]";
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(formatted, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        prepared.proof_obligations().is_none(),
        "proof-free contract remains inert metadata"
    );
    let (session, committed) = session.commit(prepared);
    committed.map_err(|error| format!("{error:?}"))?;
    let typed = session
        .prepare(
            b"formatted@1.increment",
            &[noble_kernel::types::Ty::I64],
            super::super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(typed.output(), &[noble_kernel::types::Ty::I64]);

    let two = b"module two@1 [
      def increment [ 2 + ]
      contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
        requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ]
      proof 1 p for law [ (export-unary-I64
        (intro (tail Stack) (intro (x I64)
          (pc-sequence (pc-exact (exec-literal 2)) (pc-exact (exec-add))
            (bridge (by-exact-append-assoc)) (join (by-exact-result)))))
        (mc1-true-eq-wrap)) ]
    ]";
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(two, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    noble_contracts::intrinsic::check_batch(
        prepared
            .proof_obligations()
            .ok_or("missing nonunit literal proof")?,
        super::super::LIMITS,
    )
    .map_err(|error| format!("{error:?}"))?;

    let named = b"module named@1 [ def one [ 1 ] export increment def increment [ one + ] ]";
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let session = super::super::commit(session, named)?;
    let ordinary = session
        .prepare(
            b"named@1.increment",
            &[noble_kernel::types::Ty::I64],
            super::super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(ordinary.output(), &[noble_kernel::types::Ty::I64]);
    let named_contract = b"module named_law@1 [
      def one [ 1 ] def increment [ one + ]
      contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
        requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
    ]";
    let fresh =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    assert!(
        fresh
            .prepare(named_contract, &[], super::super::LIMITS)
            .is_err(),
        "named ordinary runtime is not an MC1-v1 contract theorem"
    );
    let named_proof = b"module named_proof@1 [
      def one [ 1 ] def increment [ one + ]
      contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
        requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
      proof 1 p for law [ (export-unary-I64
        (intro (tail Stack) (intro (x I64)
          (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add))
            (bridge (by-exact-append-assoc)) (join (by-exact-result)))))
        (mc1-true-eq-wrap)) ]
    ]";
    assert!(
        fresh
            .prepare(named_proof, &[], super::super::LIMITS)
            .is_err(),
        "a named runtime call cannot be silently exported as a v1 proof"
    );
    Ok(())
}
