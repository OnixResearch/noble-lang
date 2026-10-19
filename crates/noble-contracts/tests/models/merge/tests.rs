#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only model modules import their sibling relation, identity and fixture types directly so the bounded model stays readable"
)]
use super::fixtures::{fixtures, inputs, MAX};
use super::identity::{check_evidence, subject_hash, Claim, Evidence};
use super::model::{check_relation, check_relation_over, program, MergeResult, Op, Precondition};
#[test]
fn model_fixtures_exercise_the_relation() {
    let inputs = inputs();
    let fixtures = fixtures();
    let mut verified = 0usize;
    let mut conflicts = 0usize;
    let mut counterexamples = 0usize;
    let mut incompatible = 0usize;
    let mut abnormal = 0usize;
    let mut at = 0usize;
    while at < fixtures.len() {
        let fixture = &fixtures[at];
        let result = check_relation_over(
            &fixture.base,
            &fixture.first,
            &fixture.second,
            &fixture.candidate,
            &inputs,
            fixture.output_len,
            Precondition::True,
        );
        assert_eq!(
            result, fixture.expected,
            "fixture {} produced {result:?}",
            fixture.name
        );
        match result {
            MergeResult::Verified => verified += 1,
            MergeResult::Conflict { .. } => conflicts += 1,
            MergeResult::Counterexample { .. } => counterexamples += 1,
            MergeResult::IncompatibleInterface => incompatible += 1,
            MergeResult::Abnormal => abnormal += 1,
            MergeResult::VacuousPrecondition => {}
        }
        at += 1;
    }
    std::println!(
        "NOBLE_SEMANTIC_MERGE_RECEIPT {{\"relation\":\"SMERGE/1-model\",\"fixtures\":{},\"verified\":{},\"conflicts\":{},\"counterexamples\":{},\"incompatible\":{},\"abnormal\":{},\"inputs\":{},\"decision\":\"no-go\",\"lean_obligations_open\":true}}",
        fixtures.len(),
        verified,
        conflicts,
        counterexamples,
        incompatible,
        abnormal,
        inputs.len()
    );
}

#[test]
fn wrapping_equality_is_bit_exact() {
    let base = program(Vec::new(), Vec::new(), 0);
    let first = program(vec![Op::Push(1), Op::Add], Vec::new(), 0);
    let candidate = program(vec![Op::Push(2), Op::Add], Vec::new(), 0);
    let result = check_relation(
        &base,
        &first,
        &base,
        &candidate,
        &[MAX, MAX],
        2,
        Precondition::True,
    );
    assert!(matches!(result, MergeResult::Counterexample { .. }));
}

#[test]
fn subject_identity_binds_captures_and_callees() {
    let first = program(vec![Op::PushCapture(0), Op::Add], vec![5], 7);
    let changed_capture = program(vec![Op::PushCapture(0), Op::Add], vec![6], 7);
    let changed_callee = program(vec![Op::PushCapture(0), Op::Add], vec![5], 8);
    assert_ne!(subject_hash(&first), subject_hash(&changed_capture));
    assert_ne!(subject_hash(&first), subject_hash(&changed_callee));
}

#[test]
fn evidence_refuses_wrong_claims_and_stale_revisions() {
    let base = program(Vec::new(), Vec::new(), 0);
    let first = program(vec![Op::Push(1), Op::Add], Vec::new(), 0);
    let selected = Claim {
        base: subject_hash(&base),
        first: subject_hash(&first),
        second: subject_hash(&base),
        candidate: subject_hash(&first),
        output_len: 2,
    };
    let wrong = Claim {
        candidate: subject_hash(&base),
        ..selected.clone()
    };
    let correct = Evidence {
        claim: selected.clone(),
        proof_claim: selected.clone(),
        revision: 3,
    };
    assert!(check_evidence(&correct, &selected, 3));
    let wrong_proof = Evidence {
        claim: selected.clone(),
        proof_claim: wrong,
        revision: 3,
    };
    assert!(!check_evidence(&wrong_proof, &selected, 3));
    let stale = Evidence {
        claim: selected.clone(),
        proof_claim: selected.clone(),
        revision: 2,
    };
    assert!(!check_evidence(&stale, &selected, 3));
}

#[test]
fn abnormal_and_vacuous_cases_do_not_verify() {
    let abnormal = program(vec![Op::Drop, Op::Drop, Op::Drop], Vec::new(), 0);
    let result = check_relation(
        &abnormal,
        &abnormal,
        &abnormal,
        &abnormal,
        &[0, 0],
        2,
        Precondition::True,
    );
    assert_eq!(result, MergeResult::Abnormal);
    let identity = program(Vec::new(), Vec::new(), 0);
    let vacuous = check_relation(
        &identity,
        &identity,
        &identity,
        &identity,
        &[0, 0],
        2,
        Precondition::False,
    );
    assert_eq!(vacuous, MergeResult::VacuousPrecondition);
}
