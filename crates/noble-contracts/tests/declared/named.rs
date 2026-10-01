use noble_contracts::source::{ModuleSession, Stage};
use noble_kernel::types::Ty;

const DONOR: &[u8] = b"module donor@1 [ def other [ 1 + ] export step def step# original step body\n[ 1 + ] ]";
const TWICE: &[u8] = b"module chain@1 [ export twice def twice [ d.step d.step ] contract 2 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] export twice-law ]";

fn imported_donor() -> Result<ModuleSession, String> {
    let session = ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let session = super::commit(session, DONOR)?;
    super::commit(session, b"import donor@1 as d")
}

#[test]
fn two_named_calls_preserve_distinct_specializations_and_exact_original_occurrences() -> Result<(), String> {
    let session = imported_donor()?;
    let prepared = session.prepare(TWICE, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(prepared.proof_obligations().is_none());
    let goal = prepared.contract_goals().next().ok_or("missing v2 contract")?;
    assert_eq!(goal.revision, 2);
    let subject = &goal.subject;
    let uses = &subject.named_uses;
    assert_eq!(uses.len(), 3, "root and two fresh per-use step specializations");
    let root = &uses[0];
    assert_eq!(root.definition_name, "twice");
    assert_eq!(root.definition_ordinal, 0);
    assert_eq!(root.caller_definition, None);
    assert_eq!(root.source_node, None);
    assert_eq!(root.source_span, None);
    assert_eq!(subject.accepted_submission.definitions.len(), 3);
    assert_eq!(subject.definition_owner, root.definition_owner);
    assert_eq!(subject.definition_ordinal, root.definition_ordinal);
    assert_eq!(subject.source_span, root.definition_span);
    assert_eq!(subject.named_imports.len(), 1);
    assert_eq!(subject.named_imports[0].alias, "d");
    assert_eq!(subject.named_imports[0].owner, uses[1].definition_owner);
    assert_eq!(subject.named_imports[0].module_index, uses[1].module_index);
    for occurrence in &uses[1..] {
        assert_eq!(occurrence.definition_name, "step");
        assert_eq!(occurrence.definition_ordinal, 1,
            "same-body `other` must not be mistaken for original `step`");
        assert_eq!(occurrence.caller_definition, Some(root.definition));
        let source = occurrence.source_span.ok_or("missing original named call span")?;
        let start = usize::try_from(source.start).map_err(|_| "source start exceeds address space")?;
        let end = usize::try_from(source.end).map_err(|_| "source end exceeds address space")?;
        assert_eq!(TWICE.get(start..end), Some(b"d.step".as_slice()));
        let owner = &subject.source_dependencies[occurrence.module_index];
        assert_eq!((owner.module.as_str(), owner.version), ("donor", 1));
        assert_eq!(owner.owner, occurrence.definition_owner);
        assert_eq!(owner.full_source.as_slice(), DONOR);
        let body_start = usize::try_from(occurrence.body_span.start)
            .map_err(|_| "definition body start exceeds address space")?;
        let body_end = usize::try_from(occurrence.body_span.end)
            .map_err(|_| "definition body end exceeds address space")?;
        assert_eq!(DONOR.get(body_start..body_end), Some(b"[ 1 + ]".as_slice()));
        let def_start = usize::try_from(occurrence.definition_span.start)
            .map_err(|_| "definition start exceeds address space")?;
        let def_end = usize::try_from(occurrence.definition_span.end)
            .map_err(|_| "definition end exceeds address space")?;
        assert_eq!(DONOR.get(def_start..def_end),
            Some(b"def step# original step body\n[ 1 + ]".as_slice()));
        let row = subject.accepted_submission.definitions.iter()
            .find(|row| row.definition == occurrence.definition)
            .ok_or("accepted graph lost named per-use row")?;
        assert_eq!(row.identity, occurrence.definition_identity);
        assert_eq!(row.expected.stack_in, [Ty::I64]);
        assert_eq!(row.expected.stack_out, [Ty::I64]);
    }
    assert_ne!(uses[1].definition, uses[2].definition,
        "two `d.step` calls must not share one dynamic specialization slot");
    assert_ne!(uses[1].source_node, uses[2].source_node);
    assert_ne!(uses[1].source_span, uses[2].source_span);
    assert_ne!(uses[1].candidate_node, uses[2].candidate_node);
    let (session, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("{error:?}"))?;
    let typed = session.prepare(b"chain@1.twice", &[Ty::I64], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(typed.output(), &[Ty::I64]);
    Ok(())
}

#[test]
fn rebound_import_cannot_rewrite_an_original_named_subject_or_its_proof() -> Result<(), String> {
    let session = imported_donor()?;
    let session = super::commit(session, TWICE)?;
    let second = b"module donor@2 [ export step def step [ 1 + ] ]";
    let session = super::commit(session, second)?;
    let session = super::commit(session, b"import donor@2 as d")?;
    let belated = b"module belated@1 [ contract 2 old-law [ subject chain@1.twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] ]";
    let old = session.prepare(belated, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let old_goal = old.contract_goals().next().ok_or("missing original qualified subject")?;
    assert_eq!(old_goal.subject.named_imports.len(), 1);
    assert_eq!(old_goal.subject.named_imports[0].alias, "d");
    let old_module = &old_goal.subject.source_dependencies[old_goal.subject.named_imports[0].module_index];
    assert_eq!(old_module.owner, old_goal.subject.named_imports[0].owner);
    assert_eq!((old_module.module.as_str(), old_module.version), ("donor", 1));
    assert_eq!(old_module.full_source.as_slice(), DONOR);
    let fresh = b"module fresh@1 [ def twice [ d.step d.step ] contract 2 fresh-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] ]";
    let new = session.prepare(fresh, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let new_goal = new.contract_goals().next().ok_or("missing newly bound subject")?;
    assert_eq!(new_goal.subject.named_imports.len(), 1);
    let new_module = &new_goal.subject.source_dependencies[new_goal.subject.named_imports[0].module_index];
    assert_eq!(new_module.owner, new_goal.subject.named_imports[0].owner);
    assert_eq!((new_module.module.as_str(), new_module.version), ("donor", 2));
    assert_eq!(new_module.full_source.as_slice(), second);
    assert_ne!(old_module.owner, new_module.owner);

    let third = b"module donor@3 [ export step def step [ 2 + ] ]";
    let session = super::commit(session, third)?;
    let session = super::commit(session, b"import donor@3 as d")?;
    assert!(matches!(session.prepare(fresh, &[], super::LIMITS), Err(error)
        if error.stage() == Stage::Check),
        "new source may not silently claim x+2 after rebinding to a two-increment step");
    session.prepare(belated, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let original_proof = b"module old_proof@1 [ proof 2 p for chain@1.twice-law [ (export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap)) ] ]";
    let prepared = session.prepare(original_proof, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared.proof_obligations().ok_or("missing frozen original proof")?;
    noble_contracts::intrinsic::check_batch(batch, super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}

#[test]
fn revision_two_proof_targets_only_revision_two_contract() -> Result<(), String> {
    let session = imported_donor()?;
    let session = super::commit(session, TWICE)?;
    let session = super::commit(session, b"import chain@1 as c")?;
    let valid = b"module claim@1 [ proof 2 twice-correct for c.twice-law [ (export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap)) ] ]";
    let prepared = session.prepare(valid, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared.proof_obligations().ok_or("missing v2 proof obligation")?;
    assert_eq!(batch.obligations.len(), 1);
    assert_eq!(batch.obligations[0].revision, 2);
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal else {
        return Err("v2 proof did not resolve original contract".into());
    };
    assert_eq!(contract.revision, 2);
    assert_eq!(contract.subject.named_uses.len(), 3);
    assert_eq!(contract.subject.source_dependencies[contract.subject.named_uses[1].module_index]
        .full_source.as_slice(), DONOR);
    let checked = noble_contracts::intrinsic::check_batch(batch, super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(checked.len(), 1);
    assert_eq!(checked[0].kind, noble_contracts::intrinsic::ProofKind::NamedContract);
    assert_eq!(checked[0].claim, "NamedV2Obligation.claim");

    let marker = b"module claim@1 [ proof 2 twice-correct for c.twice-law [ (named-two-call-I64) ] ]";
    let unproved = session.prepare(marker, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let marker_batch = unproved.proof_obligations().ok_or("missing source marker proof")?;
    assert!(noble_contracts::intrinsic::check_batch(marker_batch, super::LIMITS).is_err(),
        "bare source marker must not claim the named graph theorem");
    let (next, outcome) = session.commit(unproved);
    assert!(matches!(outcome, Err(error) if error.stage() == Stage::Acceptance));
    assert!(next.prepare(b"import claim@1 as missing", &[], super::LIMITS).is_err());
    let incompatible = b"module incompatible@1 [ proof 1 wrong for c.twice-law [ (refl 0) ] ]";
    assert!(matches!(next.prepare(incompatible, &[], super::LIMITS), Err(error)
        if error.stage() == Stage::Check));

    let direct = b"module direct@1 [ def inc [ 1 + ] contract 1 inc-law [ subject inc input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] export inc-law ]";
    let session = super::commit(next, direct)?;
    let session = super::commit(session, b"import direct@1 as direct")?;
    let incompatible = b"module incompatible@1 [ proof 2 wrong for direct.inc-law [ (refl 0) ] ]";
    assert!(matches!(session.prepare(incompatible, &[], super::LIMITS), Err(error)
        if error.stage() == Stage::Check));
    Ok(())
}

#[test]
fn revision_two_rejects_pure_proofs_noncanonical_revisions_and_new_field_syntax() -> Result<(), String> {
    let session = ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let bad = [
        b"module syntax@1 [ proof 2 p : [ true ] [ (refl 0) ] ]".as_slice(),
        b"module syntax@1 [ proof 3 p : [ true ] [ (refl 0) ] ]".as_slice(),
        b"module syntax@1 [ proof 02 p : [ true ] [ (refl 0) ] ]".as_slice(),
        b"module syntax@1 [ def inc [ 1 + ] contract 02 law [ subject inc input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ true ] ] ]".as_slice(),
        b"module syntax@1 [ def inc [ 1 + ] contract 3 law [ subject inc input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ true ] ] ]".as_slice(),
        b"module syntax@1 [ def inc [ 1 + ] contract 2 law [ subject inc params [ ] input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ true ] ] ]".as_slice(),
    ];
    for source in bad {
        assert!(matches!(session.prepare(source, &[], super::LIMITS), Err(error)
            if error.stage() == Stage::Parse));
    }
    assert_eq!(session.generation(), 0);
    Ok(())
}
