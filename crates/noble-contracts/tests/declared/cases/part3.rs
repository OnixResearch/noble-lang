#[test]
fn exact_intrinsic_source_proofs_stage_without_ordinary_publication() -> Result<(), String> {
    let logic = b"module logic@1 [
  proof 1 polymorphic-reflexivity :
    [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ]
    [ (intro (A Type0) (intro (x A) (refl x))) ]
]
";
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(logic, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared
        .proof_obligations()
        .ok_or("missing pending source proof")?;
    assert_eq!(batch.module, "logic");
    assert_eq!(batch.version, 1);
    assert_eq!(batch.source.as_slice(), logic.as_slice());
    assert_eq!(batch.obligations[0].name, "polymorphic-reflexivity");
    assert!(matches!(
        &batch.obligations[0].goal,
        noble_contracts::intrinsic::PendingGoal::Pure { .. }
    ));
    let (session, result) = session.commit(prepared);
    assert!(
        matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance)
    );
    assert_eq!(session.generation(), 0);
    let source_checked = session
        .prepare(logic, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let (session, result) = session.commit_verified(source_checked, |batch| {
        noble_contracts::intrinsic::check_batch(batch, super::super::LIMITS)
            .map_err(|error| format!("{error:?}"))
    });
    assert!(
        matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance),
        "source-only checking without a host model/checker revision must not publish a proof"
    );
    assert_eq!(session.generation(), 0);
    let ordinary = session
        .prepare(
            b"module ordinary@1 [ export proof def proof [ 1 ] ]",
            &[],
            super::super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(ordinary.proof_obligations().is_none());
    let (session, result) = session.commit(ordinary);
    result.map_err(|error| format!("{error:?}"))?;
    let callable = session
        .prepare(b"ordinary@1.proof", &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(callable.output(), &[noble_kernel::types::Ty::I64]);
    let mixed = session.prepare(
        b"module mixed@1 [ export proof def proof [ 1 ] proof 1 theorem : [ (Eq I64 0 0) ] [ (refl 0) ] ]",
        &[], super::super::LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        mixed
            .proof_obligations()
            .ok_or("mixed module lost logical body")?
            .obligations[0]
            .name,
        "theorem"
    );
    Ok(())
}

#[test]
fn callback_cannot_publish_invalid_source_or_substituted_claim() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    session = super::super::commit(session, b"module anchor@1 [ export alive def alive [ 1 ] ]")?;
    let prior = session.generation();
    let invalid = b"module bad@1 [ proof 1 unsound : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl missing)) ] ]";
    let prepared = session
        .prepare(invalid, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let mut was_invoked = false;
    let (next, result) = session.commit_verified(prepared, |batch| {
        was_invoked = true;
        Ok(vec![noble_contracts::intrinsic::CheckedProof {
            name: batch.obligations[0].name.clone(),
            kind: noble_contracts::intrinsic::ProofKind::Pure,
            claim: "True".into(),
            lean_term: "True.intro".into(),
            model_revision: "a".repeat(64),
            checker_revision: "b".repeat(64),
        }])
    });
    assert!(
        !was_invoked,
        "source-invalid proof must reject before invoking host"
    );
    assert!(
        matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance)
    );
    session = next;
    assert_eq!(session.generation(), prior);

    let unsupported = b"module identity@1 [ def id [ ] contract 1 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (in x)) ] ] proof 1 forged for id-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";
    let prepared = session
        .prepare(unsupported, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let mut was_invoked = false;
    let (next, result) = session.commit_verified(prepared, |batch| {
        was_invoked = true;
        Ok(vec![noble_contracts::intrinsic::CheckedProof {
            name: batch.obligations[0].name.clone(),
            kind: noble_contracts::intrinsic::ProofKind::Contract,
            claim: "True".into(),
            lean_term: "True.intro".into(),
            model_revision: "a".repeat(64),
            checker_revision: "b".repeat(64),
        }])
    });
    assert!(
        !was_invoked,
        "unsupported program proof must reject before invoking host"
    );
    assert!(
        matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance)
    );
    session = next;
    assert_eq!(session.generation(), prior);

    let valid = b"module logic@1 [ proof 1 refl : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] ]";
    for change_claim in [true, false] {
        let prepared = session
            .prepare(valid, &[], super::super::LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        let (next, result) = session.commit_verified(prepared, |batch| {
            let mut checked = noble_contracts::intrinsic::check_batch(batch, super::super::LIMITS)
                .map_err(|error| format!("{error:?}"))?;
            let accepted = checked.first_mut().ok_or("missing source-checked proof")?;
            // Deliberately forged syntactically valid host revisions are not
            // sufficient to replace either the actual claim or Lean term.
            accepted.model_revision = "a".repeat(64);
            accepted.checker_revision = "b".repeat(64);
            if change_claim {
                accepted.claim = "True".into();
            } else {
                // One more pair of parentheses is still a well-typed Lean
                // expression, but not the exact lowering of the source term.
                accepted.lean_term = format!("({})", accepted.lean_term);
            }
            Ok(checked)
        });
        assert!(
            matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance)
        );
        session = next;
        assert_eq!(session.generation(), prior);
    }
    assert!(session
        .prepare(b"import bad@1 as absent", &[], super::super::LIMITS)
        .is_err());
    assert!(session
        .prepare(b"import identity@1 as absent", &[], super::super::LIMITS)
        .is_err());
    assert!(session
        .prepare(b"import logic@1 as absent", &[], super::super::LIMITS)
        .is_err());
    let preserved = session
        .prepare(b"anchor@1.alive", &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(preserved.output(), &[noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn retained_proof_dependency_budget_rejects_atomically() -> Result<(), String> {
    let source = b"module logic@1 [ proof 1 base : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] proof 1 derived : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use base) x)) ] export derived ]";
    let replicated = source
        .len()
        .checked_mul(3)
        .ok_or("proof dependency fixture overflows host byte count")?;
    let exact_bytes =
        u32::try_from(replicated).map_err(|_| "proof dependency fixture exceeds u32 byte limit")?;
    let short_bytes = exact_bytes
        .checked_sub(1)
        .ok_or("proof dependency fixture has no byte below its exact budget")?;
    let limits = noble_contracts::Limits {
        bytes: short_bytes,
        ..super::super::LIMITS
    };
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    session = super::super::commit(session, b"module anchor@1 [ export alive def alive [ 1 ] ]")?;
    let prior = session.generation();
    let prepared = session
        .prepare(source, &[], limits)
        .map_err(|error| format!("{error:?}"))?;
    let (session, outcome) = session.commit_verified(prepared, |batch| {
        let mut checked = noble_contracts::intrinsic::check_batch(batch, limits)
            .map_err(|error| format!("{error:?}"))?;
        for proof in &mut checked {
            // An adversarial host can syntactically forge revision fields,
            // but cannot exceed the immutable dependency publication budget.
            proof.model_revision = "a".repeat(64);
            proof.checker_revision = "b".repeat(64);
        }
        Ok(checked)
    });
    assert!(matches!(outcome, Err(error) if
        error.stage() == noble_contracts::source::Stage::Acceptance &&
        error.diagnostic().message == "proof publication dependency budget exhausted"));
    assert_eq!(session.generation(), prior);
    assert!(session
        .prepare(b"import logic@1 as absent", &[], super::super::LIMITS)
        .is_err());
    let retained = session
        .prepare(b"anchor@1.alive", &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(retained.output(), &[noble_kernel::types::Ty::I64]);
    let exact = noble_contracts::Limits {
        bytes: exact_bytes,
        ..super::super::LIMITS
    };
    let prepared = session
        .prepare(source, &[], exact)
        .map_err(|error| format!("{error:?}"))?;
    let (published, result) = session.commit_verified(prepared, |batch| {
        let mut checked = noble_contracts::intrinsic::check_batch(batch, exact)
            .map_err(|error| format!("{error:?}"))?;
        for proof in &mut checked {
            proof.model_revision = "a".repeat(64);
            proof.checker_revision = "b".repeat(64);
        }
        Ok(checked)
    });
    result.map_err(|error| format!("{error:?}"))?;
    let next_generation = prior
        .checked_add(1)
        .ok_or("fixture generation cannot advance")?;
    assert_eq!(published.generation(), next_generation);
    let usable = published
        .prepare(b"import logic@1 as accepted", &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(usable.proof_obligations().is_none());
    Ok(())
}
