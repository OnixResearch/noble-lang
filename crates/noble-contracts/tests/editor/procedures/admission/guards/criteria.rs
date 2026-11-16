use super::*;

type Case = (&'static str, ProcedureRecord, RefusalKind, &'static str);

fn unsupported_proof(name: &'static str, record: ProcedureRecord) -> Case {
    (
        name,
        record,
        RefusalKind::ForgedStatus,
        "promotion.obligations",
    )
}

fn complete_record() -> Result<ProcedureRecord, String> {
    let mut record =
        literal_record(6, Producer::Chunk).map_err(|error| format!("promoted: {error:?}"))?;
    // A proof-class claim is not itself an independently checked proof; the
    // owner's verifier below decides which of these claims are verified.
    for obligation in 1..=6 {
        let mut proof = evidence(&record, EvidenceClass::Lean, EvidenceOutcome::Proved);
        proof.obligation = obligation;
        record.evidence.push(proof);
    }
    record.status = Status::Promoted;
    record.promotion = Some(noble_contracts::source::procedures::Promotion {
        policy_revision: 3,
        obligations: (1..=6).collect(),
    });
    record.refresh_retrieval_signature();
    Ok(record)
}

/// The owner independently checked every claim used below and observed what
/// each one asserts, so each refusal comes from the consumer policy rules,
/// not from a missing verification.
fn owner_ledger(promoted: &ProcedureRecord, cases: &[Case]) -> OwnerLedger {
    let mut ledger = OwnerLedger::empty();
    let case_claims = cases
        .iter()
        .flat_map(|(_, record, _, _)| record.evidence.iter());
    for claim in promoted.evidence.iter().chain(case_claims) {
        ledger.confirm(claim);
    }
    ledger
}

fn refusal_cases(promoted: &ProcedureRecord) -> [Case; 8] {
    assert_eq!(promoted.evidence.len(), 6);
    assert!(promoted.promotion.is_some());
    let mut omitted = promoted.clone();
    if let Some(promotion) = omitted.promotion.as_mut() {
        promotion.obligations.truncate(1);
    }
    let mut absent_proof = promoted.clone();
    absent_proof.evidence.truncate(5);
    let mut test_passed = promoted.clone();
    test_passed.evidence[5].class = EvidenceClass::Test;
    test_passed.evidence[5].outcome = EvidenceOutcome::Passed;
    let mut proof_class_passed = promoted.clone();
    proof_class_passed.evidence[5].outcome = EvidenceOutcome::Passed;
    let mut wrong_class = promoted.clone();
    wrong_class.evidence[5].class = EvidenceClass::Verus;
    let mut wrong_policy = promoted.clone();
    wrong_policy.evidence[5].policy_revision = 2;
    let mut wrong_subject = promoted.clone();
    wrong_subject.evidence[5].subject_binding = [0; 32];
    let mut forged_status = promoted.clone();
    forged_status.evidence[5].class = EvidenceClass::Test;
    [
        unsupported_proof(
            "omitted demanded obligations despite available proof",
            omitted,
        ),
        unsupported_proof("named obligation with no evidence", absent_proof),
        unsupported_proof("test passed is not proof", test_passed),
        unsupported_proof("proof class with passed outcome", proof_class_passed),
        unsupported_proof("proved evidence in a policy-disallowed class", wrong_class),
        unsupported_proof("proved evidence from another policy", wrong_policy),
        (
            "proved evidence for another subject",
            wrong_subject,
            RefusalKind::StaleContext,
            "evidence.subject_binding",
        ),
        (
            "test class claiming proved",
            forged_status,
            RefusalKind::ForgedStatus,
            "evidence.outcome",
        ),
    ]
}

fn reject_model_only_refinement(
    library: &mut ProcedureLibrary,
    namespace: &Session,
    promoted: &ProcedureRecord,
) -> Result<(), String> {
    let before = library
        .encode()
        .map_err(|error| format!("before refusal: {error:?}"))?;
    // A model theorem for O1/O3 does not establish Rust encoder refinement.
    // The consumer demands it separately; no obligation ID is privileged.
    let refinement_policy = ConsumerPolicy {
        revision: 3,
        obligations: [1, 3, 7]
            .into_iter()
            .map(|id| ConsumerObligation {
                id,
                allowed_classes: vec![EvidenceClass::Lean],
            })
            .collect(),
    };
    let mut missing_refinement = promoted.clone();
    if let Some(promotion) = missing_refinement.promotion.as_mut() {
        promotion.obligations = vec![1, 3];
    }
    missing_refinement
        .evidence
        .retain(|proof| proof.obligation == 1 || proof.obligation == 3);
    assert_eq!(missing_refinement.evidence.len(), 2);
    let refused = library
        .admit(namespace, Some(&refinement_policy), missing_refinement)
        .expect_err("model-only evidence cannot omit demanded Rust encoder refinement");
    assert_eq!(refused.kind, RefusalKind::ForgedStatus);
    assert_eq!(refused.location.field, "promotion.obligations");
    assert_eq!(
        library
            .encode()
            .map_err(|error| format!("after refusal: {error:?}"))?,
        before
    );
    Ok(())
}

/// A demanded obligation the owner never verified, or verified with another
/// outcome than the claim asserts, cannot complete the promotion.
fn reject_unverified_obligation(
    namespace: &Session,
    policy: &ConsumerPolicy,
    promoted: &ProcedureRecord,
) -> Result<(), String> {
    for observed in [None, Some(EvidenceOutcome::Unknown)] {
        let mut ledger = OwnerLedger::empty();
        for claim in &promoted.evidence[..5] {
            ledger.confirm(claim);
        }
        if let Some(outcome) = observed {
            ledger.observe(&promoted.evidence[5], outcome);
        }
        let mut library = ProcedureLibrary::with_verifier(revision(7), ledger.boxed());
        let refused = library
            .admit(namespace, Some(policy), promoted.clone())
            .expect_err("an unverified demanded obligation must not promote");
        assert_eq!(refused.kind, RefusalKind::ForgedStatus, "{observed:?}");
        assert_eq!(refused.location.field, "promotion.obligations");
        assert_eq!(refused.location.index, Some(5));
        assert!(library.records().is_empty());
    }
    Ok(())
}

#[test]
fn proof_demanding_promotion_requires_every_selected_obligation() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let promoted = complete_record()?;
    let cases = refusal_cases(&promoted);
    let mut library =
        ProcedureLibrary::with_verifier(revision(7), owner_ledger(&promoted, &cases).boxed());
    let seed = literal_record(9, Producer::Chunk).map_err(|error| format!("seed: {error:?}"))?;
    library
        .admit(&namespace, None, seed)
        .map_err(|error| format!("seed admission: {error:?}"))?;
    let generation = library.current_generation();
    let generations: Vec<_> = library.generation_records().into_iter().cloned().collect();
    let records = library.records().to_vec();
    let policy = ConsumerPolicy {
        revision: 3,
        obligations: (1..=6)
            .map(|id| ConsumerObligation {
                id,
                allowed_classes: vec![EvidenceClass::Lean],
            })
            .collect(),
    };
    for (case, record, kind, field) in cases {
        let refused = library
            .admit(&namespace, Some(&policy), record)
            .expect_err(case);
        assert_eq!(refused.kind, kind, "{case}");
        assert_eq!(refused.location.field, field, "{case}");
        assert_eq!(library.current_generation(), generation, "{case}");
        assert_eq!(
            library
                .generation_records()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>(),
            generations,
            "{case}"
        );
        assert_eq!(library.records(), records.as_slice(), "{case}");
    }
    reject_model_only_refinement(&mut library, &namespace, &promoted)?;
    reject_unverified_obligation(&namespace, &policy, &promoted)?;

    library
        .admit(&namespace, Some(&policy), promoted)
        .map_err(|error| format!("all six obligations verified as proved: {error:?}"))?;
    assert_eq!(
        library.current_generation(),
        generation.checked_add(1).ok_or("generation overflow")?
    );
    assert_eq!(
        library.records().len(),
        records
            .len()
            .checked_add(1)
            .ok_or("record count overflow")?
    );
    Ok(())
}
