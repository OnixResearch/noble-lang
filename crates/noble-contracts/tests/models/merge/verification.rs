#![allow(
    tigerstyle::assertion_density,
    tigerstyle::unused_self,
    reason = "Owner: noble-contracts-maintainers; this test-only claim/consumer lane builds finite selections and asserts each outcome, so long setup legs and per-fixture assertions are the readable form."
)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only verification module imports its sibling claim, consumer, Lean-driver, fixture and model items directly so the lane stays readable"
)]
//! Phase 3 checks for the test-only `SMERGE/1` claim lane: producer
//! regeneration, consumer refusal of differing producer fields, bounded
//! counterexample replay, outcome visibility and the isolated pinned Lean
//! check (`--ignored`; the lane runs it with the pinned toolchain).

use super::claim::{
    admit, checked, regenerate, sha256, Evidence, Observable, Outcome, Preparation, Refusal,
    Selection, EVIDENCE_SCHEMA, RELATION_REVISION, SEMANTIC_REVISION,
};
use super::consumer::{
    evaluate, has_witness, replay, search, Counterexample, Search, Witness, MAX_VECTORS,
};
use super::fixtures::MAX;
use super::lean::{self, Bounds};
use super::model::{program, Op, Precondition, Program};

const SEED: u64 = 0x5eed_2026_1119;

fn identity() -> Program {
    program(Vec::new(), Vec::new(), 0)
}

fn increment_top() -> Program {
    program(vec![Op::Push(1), Op::Add], Vec::new(), 0)
}

fn increment_two() -> Program {
    program(vec![Op::Push(2), Op::Add], Vec::new(), 0)
}

fn increment_bottom() -> Program {
    program(
        vec![Op::Swap, Op::Push(1), Op::Add, Op::Swap],
        Vec::new(),
        0,
    )
}

/// The design's SMERGE-CASE-01 candidate `[ swap 1 + swap 1 + ]`.
fn increment_both() -> Program {
    program(
        vec![
            Op::Swap,
            Op::Push(1),
            Op::Add,
            Op::Swap,
            Op::Push(1),
            Op::Add,
        ],
        Vec::new(),
        0,
    )
}

fn i64_pair() -> Vec<Observable> {
    std::vec![Observable::I64, Observable::I64]
}

fn preparation(program: Program) -> Preparation {
    Preparation {
        program,
        input: i64_pair(),
        output: i64_pair(),
    }
}

fn selection(base: Program, first: Program, second: Program, candidate: Program) -> Selection {
    Selection {
        base: preparation(base),
        first: preparation(first),
        second: preparation(second),
        candidate: preparation(candidate),
        input: i64_pair(),
        output: i64_pair(),
        precondition: Precondition::True,
    }
}

/// The design's SMERGE-CASE-01 subject set.
fn case01() -> Selection {
    selection(
        identity(),
        increment_top(),
        increment_bottom(),
        increment_both(),
    )
}

/// The design's SMERGE-CASE-04 subject set.
fn case04() -> Selection {
    selection(
        identity(),
        increment_top(),
        increment_top(),
        increment_top(),
    )
}

/// The literal MC1 code of the case-04 candidate.
const CASE04_CODE: &str = "[Op.lit (Value.i64 (BitVec.ofNat 64 1)), Op.word 4]";

/// The case-01 candidate lowers to `[ swap 1 + swap 1 + ]`.
const CASE01_CANDIDATE_CODE: &str = "[Op.word 2, Op.lit (Value.i64 (BitVec.ofNat 64 1)), Op.word 4, Op.word 2, Op.lit (Value.i64 (BitVec.ofNat 64 1)), Op.word 4]";

fn case04_claim_text() -> String {
    format!(
        "MergeRel [Ty.i64, Ty.i64] [Ty.i64, Ty.i64] [] {code} {code} {code}",
        code = CASE04_CODE
    )
}

fn case01_claim_text() -> String {
    format!(
        "MergeRel [Ty.i64, Ty.i64] [Ty.i64, Ty.i64] [] {first} {second} {candidate}",
        first = CASE04_CODE,
        second = "[Op.word 2, Op.lit (Value.i64 (BitVec.ofNat 64 1)), Op.word 4, Op.word 2]",
        candidate = CASE01_CANDIDATE_CODE
    )
}

fn producer(selection: &Selection, proof: &str) -> Evidence {
    Evidence::render(selection, proof.into()).expect("producer rendering")
}

#[test]
fn claim_regeneration_is_exact_and_canonical() {
    let claim = regenerate(&case04()).expect("case-04 regenerates");
    assert_eq!(claim.claim_text, case04_claim_text());
    assert_eq!(claim.relation, RELATION_REVISION);
    assert_eq!(claim.semantic_revision, SEMANTIC_REVISION);
    let again = regenerate(&case04()).expect("regeneration is deterministic");
    assert_eq!(claim.digest(), again.digest());
    assert_eq!(claim.subject_digests, again.subject_digests);
    let other = regenerate(&case01()).expect("case-01 regenerates");
    assert_eq!(other.claim_text, case01_claim_text());
    assert_ne!(claim.digest(), other.digest());
    // The candidate's own projection is the literal MC1 code, so the Lean
    // claim is over the exact subject and not over a recipe digest.
    assert_eq!(case04().candidate.lean_code(), CASE04_CODE);
    assert_eq!(case01().candidate.lean_code(), CASE01_CANDIDATE_CODE);
    // Length-delimited identity: a changed callee or capture is a different
    // subject even when the code coincides.
    let mut changed = case04();
    changed.candidate.program.callee = 9;
    assert_ne!(
        changed.candidate.subject_digest(),
        case04().candidate.subject_digest()
    );
    let mut captured = case04();
    captured.candidate.program = program(vec![Op::PushCapture(0), Op::Add], vec![1], 0);
    assert_ne!(
        captured.candidate.subject_digest(),
        case04().candidate.subject_digest()
    );
    // SHA-256 is exercised by fixed vectors so an evidence digest cannot be
    // silently replaced by a weaker index.
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn consumer_refuses_every_differing_producer_field() {
    let selected = case04();
    let valid = producer(&selected, "NobleContracts.mergeRel_identical_increment");
    assert!(admit(&selected, &valid).is_ok());

    let mut stale = valid.clone();
    stale.subjects[3] = sha256(b"another subject");
    assert_eq!(admit(&selected, &stale), Err(Refusal::StaleSubject));

    let mut wrong_claim = valid.clone();
    wrong_claim.claim = case01_claim_text();
    assert_eq!(admit(&selected, &wrong_claim), Err(Refusal::WrongClaim));

    let mut assumptions = valid.clone();
    assumptions.assumptions.push("requires false".into());
    assert_eq!(
        admit(&selected, &assumptions),
        Err(Refusal::WrongAssumptions)
    );

    let mut wrong_hash = valid.clone();
    wrong_hash.claim_hash = sha256(b"not the claim");
    assert_eq!(admit(&selected, &wrong_hash), Err(Refusal::WrongHash));

    let mut trusted = valid.clone();
    trusted.trusted = true;
    assert_eq!(admit(&selected, &trusted), Err(Refusal::TrustFlag));

    let mut revision = valid.clone();
    revision.revision = "SMERGE/0".into();
    assert_eq!(admit(&selected, &revision), Err(Refusal::WrongRevision));

    let mut schema = valid.clone();
    schema.schema = "noble-merge-evidence/v2".into();
    assert_eq!(admit(&selected, &schema), Err(Refusal::WrongSchema));

    let mut premarked = valid.clone();
    premarked.claimed_outcome = Outcome::Verified;
    assert_eq!(admit(&selected, &premarked), Err(Refusal::WrongOutcome));

    // Every refusal stays visibly nonverified.
    for refusal in [
        Refusal::StaleSubject,
        Refusal::WrongClaim,
        Refusal::WrongAssumptions,
        Refusal::WrongHash,
        Refusal::TrustFlag,
        Refusal::WrongRevision,
        Refusal::WrongSchema,
        Refusal::WrongOutcome,
        Refusal::IncompatibleInterface,
        Refusal::VacuousPrecondition,
    ] {
        assert!(!refusal.outcome().is_verified(), "{refusal:?}");
    }
    assert_eq!(EVIDENCE_SCHEMA, "noble-merge-evidence/v1");
}

#[test]
fn interface_observable_and_precondition_refusals() {
    let mut missing = selection(identity(), identity(), identity(), identity());
    missing.candidate.output = std::vec![Observable::I64];
    assert_eq!(regenerate(&missing), Err(Refusal::IncompatibleInterface));

    let mut vacuous = case04();
    vacuous.precondition = Precondition::False;
    assert_eq!(regenerate(&vacuous), Err(Refusal::VacuousPrecondition));
    assert!(!has_witness(&vacuous));

    let mut empty = case04();
    empty.input = Vec::new();
    assert_eq!(regenerate(&empty), Err(Refusal::VacuousPrecondition));

    // The producer cannot introduce an interface the consumer did not select:
    // its claim text is compared with the regeneration, never parsed.
    let selected = case04();
    let mut forged = producer(&selected, "NobleContracts.mergeRel_identical_increment");
    forged.claim = forged
        .claim
        .replace("[Ty.i64, Ty.i64] [Ty.i64, Ty.i64]", "[Ty.i64] [Ty.i64]");
    assert_eq!(admit(&selected, &forged), Err(Refusal::WrongClaim));
    assert!(has_witness(&selected));
}

#[test]
fn bounded_search_validates_witnesses_and_never_certifies() {
    // SMERGE-CASE-06 lost change: a validated counterexample.
    let lost = selection(identity(), increment_top(), identity(), identity());
    let found = search(&lost, SEED, MAX_VECTORS);
    assert_eq!(found.outcome, Outcome::ValidatedCounterexample);
    assert_eq!(found.seed, SEED);
    let witness = found.counterexample.expect("witness retained");
    assert_eq!(witness.position, 1);
    assert_eq!(witness.expected, witness.actual.wrapping_add(1));
    assert_eq!(
        replay(&lost, &witness.input, SEED),
        Witness::Differs(witness.clone())
    );

    // SMERGE-CASE-07 incompatible edits: a conflict, not a candidate choice.
    let incompatible = selection(
        identity(),
        increment_top(),
        increment_two(),
        increment_top(),
    );
    let conflict = search(&incompatible, SEED, MAX_VECTORS);
    assert_eq!(conflict.outcome, Outcome::Conflict);

    // SMERGE-CASE-08 wrapping boundary: at `y = MAX` the candidate `[2 +]`
    // differs from the merge `[1 +]` because `MAX + 1` wraps to `0`.
    let wrapping = selection(identity(), increment_top(), identity(), increment_two());
    let boundary = replay(&wrapping, &[0, MAX], SEED);
    match boundary {
        Witness::Differs(counterexample) => {
            assert_eq!(counterexample.input, std::vec![0, MAX]);
            assert_eq!(counterexample.position, 1);
            assert_eq!(counterexample.expected, 0);
            assert_eq!(counterexample.actual, 1);
        }
        other => panic!("wrapping boundary is not a counterexample: {other:?}"),
    }

    // A passing corpus is never a proof: the search stays `unknown` and a
    // truncated search is marked as such.
    let passing = search(&case01(), SEED, MAX_VECTORS);
    assert_eq!(passing.outcome, Outcome::Unknown);
    assert!(!passing.truncated);
    assert_eq!(passing.examined, MAX_VECTORS);
    let truncated = search(&case01(), SEED, MAX_VECTORS + 10);
    assert!(truncated.truncated);
    assert_eq!(truncated.outcome, Outcome::Unknown);

    // A selected observable the lane's corpus cannot generate is `unsupported`
    // rather than a silent pass, for every first-order type outside `I64`.
    for ty in [Observable::Unit, Observable::Bool, Observable::Text] {
        let mut typed = case01();
        typed.input = std::vec![ty];
        typed.base.input = std::vec![ty];
        typed.first.input = std::vec![ty];
        typed.second.input = std::vec![ty];
        typed.candidate.input = std::vec![ty];
        assert_eq!(
            replay(&typed, &[1], SEED),
            Witness::Refused(Refusal::UnsupportedObservable)
        );
        assert!(!ty.lean().is_empty());
    }

    // The generated corpus is bounded by the declared vector budget.
    assert_eq!(found.examined, 1);
    assert_eq!(passing.examined, MAX_VECTORS);
}

#[test]
fn outcome_discipline_is_total_and_visible() {
    let claim = regenerate(&case01()).expect("claim");
    let mut bound = claim.clone();
    bound.proof_digest = sha256(b"NobleContracts.mergeRel_independent_changes");
    assert_eq!(
        checked(&bound, true, &["propext".into(), "Quot.sound".into()]),
        Outcome::Verified
    );
    // An unadmitted claim (no bound proof bytes) is never verified.
    assert_eq!(checked(&claim, true, &[]), Outcome::Unknown);
    // A disallowed axiom is never verified.
    assert_eq!(
        checked(&bound, true, &["Lean.ofReduceBool".into()]),
        Outcome::Unknown
    );
    // A rejected proof is never verified.
    assert_eq!(checked(&bound, false, &[]), Outcome::Unknown);

    let all = [
        Outcome::Verified,
        Outcome::Conflict,
        Outcome::ValidatedCounterexample,
        Outcome::Unknown,
        Outcome::Unsupported,
        Outcome::Exhausted,
        Outcome::InternalFailure,
    ];
    let mut verified = 0usize;
    for outcome in all {
        if outcome.is_verified() {
            verified += 1;
            assert_eq!(outcome.name(), "verified");
        } else {
            assert_ne!(outcome.name(), "verified");
        }
    }
    assert_eq!(verified, 1);

    let selected = case01();
    let evidence = producer(&selected, "NobleContracts.mergeRel_independent_changes");
    // A validated refutation dominates an accepting proof check, and the
    // checker is never consulted.
    let refuted = search(
        &selection(identity(), increment_top(), identity(), identity()),
        SEED,
        MAX_VECTORS,
    );
    let outcome = evaluate(&selected, &evidence, &refuted, |_, _| {
        panic!("the checker must not run after a validated refutation")
    });
    assert_eq!(outcome, Outcome::ValidatedCounterexample);

    // Without a refutation the isolated check decides, and acceptance is the
    // only path to `verified`.
    let accepted = evaluate(
        &selected,
        &evidence,
        &search(&selected, SEED, 1),
        |claim, proof| {
            assert_eq!(claim, case01_claim_text());
            assert_eq!(proof, "NobleContracts.mergeRel_independent_changes");
            (true, std::vec!["propext".into(), "Quot.sound".into()])
        },
    );
    assert_eq!(accepted, Outcome::Verified);
    let rejected = evaluate(&selected, &evidence, &search(&selected, SEED, 1), |_, _| {
        (false, Vec::new())
    });
    assert_eq!(rejected, Outcome::Unknown);

    let names: Vec<String> = all.iter().map(|outcome| outcome.name().into()).collect();
    std::println!(
        "NOBLE_SEMANTIC_MERGE_VERIFICATION {{\"relation\":\"{}\",\"semantic_revision\":\"{}\",\"case04_claim\":{},\"case04_digest\":\"{}\",\"case01_claim\":{},\"case01_digest\":\"{}\",\"search_seed\":{},\"search_vectors\":{},\"search_outcome\":\"{}\",\"subject_candidate\":\"{}\",\"outcomes\":{}}}",
        RELATION_REVISION,
        SEMANTIC_REVISION,
        json_string(&case04_claim_text()),
        regenerate(&case04()).expect("claim").digest(),
        json_string(&case01_claim_text()),
        regenerate(&case01()).expect("claim").digest(),
        SEED,
        MAX_VECTORS,
        search(&case01(), SEED, MAX_VECTORS).outcome.name(),
        case01().candidate.subject_digest(),
        json_array(&names),
    );
}

/// Bounded JSON string rendering: the receipt line contains only ASCII claim
/// text, so escaping quotes and backslashes is exact.
fn json_string(text: &str) -> String {
    let mut rendered = String::with_capacity(text.len() + 2);
    rendered.push('"');
    for character in text.chars() {
        match character {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            other => rendered.push(other),
        }
    }
    rendered.push('"');
    rendered
}

/// The isolated pinned Lean check.  `#[ignore]`d so the ordinary sandbox run
/// reports it as ignored; the Phase 3 lane runs it with the pinned toolchain
/// and a built `proofs/mc1` library.
#[test]
#[ignore = "requires NOBLE_LEAN (pinned Lean 4.35.0) and a built proofs/mc1 library"]
fn semantic_merge_lean_check() {
    let bounds = Bounds::lane();
    let case04_claim = regenerate(&case04()).expect("claim");
    let case01_claim = regenerate(&case01()).expect("claim");
    let case04 = lean::check(
        &case04_claim.claim_text,
        "NobleContracts.mergeRel_identical_increment",
        bounds,
    );
    assert_eq!(
        case04.outcome,
        Outcome::Verified,
        "case-04 rejected: {}",
        case04.diagnostic
    );
    let case01 = lean::check(
        &case01_claim.claim_text,
        "NobleContracts.mergeRel_independent_changes",
        bounds,
    );
    assert_eq!(
        case01.outcome,
        Outcome::Verified,
        "case-01 rejected: {}",
        case01.diagnostic
    );
    // SMERGE-CASE-12: a valid proof of a different claim does not verify this
    // one, and the submission stays nonverified.
    let wrong = lean::check(
        &case04_claim.claim_text,
        "NobleContracts.mergeRel_independent_changes",
        bounds,
    );
    assert_eq!(wrong.outcome, Outcome::Unknown);
    assert!(!wrong.accepted);
    // A proof submission may not smuggle a command.
    let command = lean::check(
        &case04_claim.claim_text,
        "by trivial\nimport NobleContracts.Merge",
        bounds,
    );
    assert_eq!(command.outcome, Outcome::Unknown);
    let allowance = ["propext", "Classical.choice", "Quot.sound"];
    for axiom in &case04.axioms {
        assert!(allowance.iter().any(|name| axiom == name), "{axiom}");
    }
    let proof04 = "NobleContracts.mergeRel_identical_increment";
    let proof01 = "NobleContracts.mergeRel_independent_changes";
    std::println!(
        "NOBLE_SEMANTIC_MERGE_LEAN {{\"case04\":\"{}\",\"case01\":\"{}\",\"wrong_proof\":\"{}\",\"command_submission\":\"{}\",\"axioms\":{},\"proof_case04\":{},\"proof_case04_sha256\":\"{}\",\"proof_case01\":{},\"proof_case01_sha256\":\"{}\",\"library_digest\":\"{}\",\"library_root\":{},\"lean\":{}}}",
        case04.outcome.name(),
        case01.outcome.name(),
        wrong.outcome.name(),
        command.outcome.name(),
        json_array(&case04.axioms),
        json_string(proof04),
        sha256(proof04.as_bytes()),
        json_string(proof01),
        sha256(proof01.as_bytes()),
        case04.library_digest,
        json_string(&case04.library),
        json_string(&case04.lean),
    );
}

fn json_array(values: &[String]) -> String {
    let mut rendered = String::from("[");
    let mut at = 0usize;
    while at < values.len() {
        if at != 0 {
            rendered.push(',');
        }
        rendered.push_str(&json_string(&values[at]));
        at += 1;
    }
    rendered.push(']');
    rendered
}

/// The search result must keep its witness and bound when it is retained.
#[test]
fn search_receipts_retain_the_witness() {
    let lost = selection(identity(), increment_top(), identity(), identity());
    let found: Search = search(&lost, SEED, MAX_VECTORS);
    let witness: Counterexample = found.counterexample.expect("witness");
    assert_eq!(witness.seed, SEED);
    assert_eq!(found.examined, 1);
    let mut claim = regenerate(&lost).expect("claim");
    claim.proof_digest = sha256(b"proof");
    assert_eq!(checked(&claim, false, &[]), Outcome::Unknown);
    let empty: Vec<String> = Vec::new();
    assert!(json_array(&empty) == "[]");
}
