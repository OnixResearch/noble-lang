#[test]
fn imported_proof_dependency_budget_counts_each_obligation() -> Result<(), String> {
    let source = b"module proofs@1 [
      # The retained, immutable module source is copied with every imported proof.
      # This deliberately long original source makes the per-proof dependency
      # budget the first limit to exhaust, ahead of the aggregate module source
      # budget when two short importing proofs are checked at the boundary.
      proof 1 base : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ]
      export base
    ]";
    let module =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let staged = module
        .prepare(source, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let (published, result) = module.commit_verified(staged, |batch| {
        let mut checked = noble_contracts::intrinsic::check_batch(batch, super::super::LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        for proof in &mut checked {
            proof.model_revision = "a".repeat(64);
            proof.checker_revision = "b".repeat(64);
        }
        Ok(checked)
    });
    result.map_err(|error| format!("{error:?}"))?;
    let imported = super::super::commit(published, b"import proofs@1 as p")?;
    let dependent = b"module derived@1 [
      proof 1 first : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use p.base) x)) ]
      proof 1 second : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use p.base) x)) ]
    ]";
    let staged = imported
        .prepare(dependent, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = staged
        .proof_obligations()
        .ok_or("missing imported proof obligations")?;
    assert_eq!(batch.obligations.len(), 2);
    let [dependency] = batch.dependencies.as_slice() else {
        return Err("expected one deduplicated imported proof".into());
    };
    assert!(
        !dependency.source.is_empty(),
        "imported proof has no retained source"
    );
    let replicated = dependency
        .source
        .len()
        .checked_mul(batch.obligations.len())
        .ok_or("imported proof fixture overflows host byte count")?;
    let exact_bytes =
        u32::try_from(replicated).map_err(|_| "imported proof fixture exceeds u32 byte limit")?;
    let exact = noble_contracts::Limits {
        bytes: exact_bytes,
        ..super::super::LIMITS
    };
    imported
        .prepare(dependent, &[], exact)
        .map_err(|error| format!("{error:?}"))?;
    let short_bytes = exact_bytes
        .checked_sub(1)
        .ok_or("imported proof fixture has no byte below its exact budget")?;
    let short = noble_contracts::Limits {
        bytes: short_bytes,
        ..exact
    };
    let refused = imported
        .prepare(dependent, &[], short)
        .expect_err("one byte below the replicated dependency must fail");
    asserts_dependency_budget_refusal(&refused)
}

#[test]
fn exact_increment_contract_binds_checked_definition_not_text_only() -> Result<(), String> {
    let source = b"module arithmetic@1 [
  def increment [ 1 + ]
  contract 1 increment-law [ subject increment
    input [ x I64 ] output [ y I64 ]
    requires [ true ]
    ensures [ (eq (out y) (add (in x) 1)) ] ]
  proof 1 increment-correct for increment-law
    [ (export-unary-I64
        (intro (tail Stack) (intro (x I64)
          (pc-sequence (pc-exact (exec-literal 1))
            (pc-exact (exec-add))
            (bridge (by-exact-append-assoc))
            (join (by-exact-result)))))
        (mc1-true-eq-wrap)) ]
]
";
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(source, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared
        .proof_obligations()
        .ok_or("missing pending program proof")?;
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal
    else {
        return Err("missing resolved MC1 contract claim".into());
    };
    assert_eq!(contract.contract_name, "increment-law");
    assert_eq!(
        contract.subject.definition_source.as_slice(),
        b"def increment [ 1 + ]"
    );
    assert_eq!(contract.subject.module_source.as_slice(), source.as_slice());
    assert_eq!(contract.subject.input_types, [noble_kernel::types::Ty::I64]);
    assert_eq!(
        contract.subject.output_types,
        [noble_kernel::types::Ty::I64]
    );
    assert_eq!(contract.subject.accepted_submission.definitions.len(), 1);
    assert_eq!(contract.subject.source_dependencies.len(), 1);
    assert_eq!(
        contract.subject.source_dependencies[0]
            .full_source
            .as_slice(),
        source.as_slice()
    );
    assert_eq!(
        contract.subject.effects,
        noble_kernel::types::EffSet::empty()
    );
    let (session, result) = session.commit(prepared);
    assert!(
        matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance)
    );
    asserts_rejected_budget_left_no_generation(session)
}

/// A rejected dependency-budget overflow must not advance the session generation.
fn asserts_rejected_budget_left_no_generation(
    session: noble_contracts::source::ModuleSession,
) -> Result<(), String> {
    assert_eq!(session.generation(), 0);
    assert!(session
        .prepare(b"arithmetic@1.increment", &[], super::super::LIMITS)
        .is_err());
    Ok(())
}

/// One byte below the replicated dependency budget must fail with the exact reason.
fn asserts_dependency_budget_refusal(
    refused: &noble_contracts::source::Error,
) -> Result<(), String> {
    assert_eq!(refused.stage(), noble_contracts::source::Stage::Check);
    assert_eq!(
        refused.diagnostic().message,
        "proof dependency snapshot byte budget exhausted"
    );
    Ok(())
}
