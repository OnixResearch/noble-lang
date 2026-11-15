use super::*;

fn complete_record() -> Result<ProcedureRecord, String> {
    let mut record =
        literal_record(6, Producer::Chunk).map_err(|error| format!("promoted: {error:?}"))?;
    // A proof-class record is not itself an independently checked proof.
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

fn refusal_cases(
    promoted: &ProcedureRecord,
) -> [(&'static str, ProcedureRecord, RefusalKind, &'static str); 8] {
    let mut omitted = promoted.clone();
    omitted
        .promotion
        .as_mut()
        .expect("promotion")
        .obligations
        .truncate(1);
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
        (
            "omitted demanded obligations despite available proof",
            omitted,
            RefusalKind::ForgedStatus,
            "promotion.obligations",
        ),
        (
            "named obligation with no evidence",
            absent_proof,
            RefusalKind::ForgedStatus,
            "promotion.obligations",
        ),
        (
            "test passed is not proof",
            test_passed,
            RefusalKind::ForgedStatus,
            "promotion.obligations",
        ),
        (
            "proof class with passed outcome",
            proof_class_passed,
            RefusalKind::ForgedStatus,
            "promotion.obligations",
        ),
        (
            "proved evidence in a policy-disallowed class",
            wrong_class,
            RefusalKind::ForgedStatus,
            "promotion.obligations",
        ),
        (
            "proved evidence from another policy",
            wrong_policy,
            RefusalKind::ForgedStatus,
            "promotion.obligations",
        ),
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

#[test]
fn proof_demanding_promotion_requires_every_selected_obligation() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
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
    let promoted = complete_record()?;
    for (case, record, kind, field) in refusal_cases(&promoted) {
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
    // Model O1/O3 are distinct from implementation refinement, independently
    // demanded as ID 7. No obligation ID is privileged in production code.
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
    missing_refinement
        .promotion
        .as_mut()
        .expect("promotion")
        .obligations = vec![1, 3];
    missing_refinement
        .evidence
        .retain(|proof| proof.obligation == 1 || proof.obligation == 3);
    let refused = library
        .admit(&namespace, Some(&refinement_policy), missing_refinement)
        .expect_err("model-only O1/O3 evidence cannot omit demanded Rust encoder refinement");
    assert_eq!(refused.kind, RefusalKind::ForgedStatus);
    assert_eq!(refused.location.field, "promotion.obligations");
    assert_eq!(library.current_generation(), generation);
    assert_eq!(
        library
            .generation_records()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
        generations
    );
    assert_eq!(library.records(), records.as_slice());

    library
        .admit(&namespace, Some(&policy), promoted)
        .map_err(|error| format!("all six obligations proved: {error:?}"))?;
    assert_eq!(library.current_generation(), generation + 1);
    assert_eq!(library.records().len(), records.len() + 1);
    Ok(())
}
