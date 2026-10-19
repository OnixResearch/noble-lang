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
    assert_eq!(batch.obligations.len(), 1);
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal
    else {
        return Err("missing resolved contract".into());
    };
    noble_contracts::intrinsic::prepare_contract(contract, super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    noble_contracts::intrinsic::check_batch(batch, super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(contract.subject.definition_ordinal, 1);
    helpers::refuses_public_binding_mutations(contract)?;
    helpers::refuses_recipe_mutations(contract)?;
    helpers::refuses_evidence_mutations(contract)?;
    helpers::refuses_specialization_and_capture_mutations(contract)?;
    Ok(())
}
/// refuses public binding mutations must each be refused by contract preparation.

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
    refuses_named_runtime_as_v1_theorem()?;
    Ok(())
}

/// A named runtime call must not be accepted as an MC1-v1 theorem or proof.
fn refuses_named_runtime_as_v1_theorem() -> Result<(), String> {
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

#[path = "part5/helpers.rs"]
mod helpers;
