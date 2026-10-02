use noble_contracts::source::{ModuleKind, ModuleSession, Stage};
use noble_kernel::types::Ty;

const SOURCE: &[u8] = b"module arithmetic@1 [ export increment def unrelated [ 1 + ] def\tincrement# original lexical body\n[ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] export increment-law ]";

#[test]
fn direct_contract_uses_unique_lexical_origin_not_kernel_slot() -> Result<(), String> {
    let session = ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(SOURCE, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(prepared.kind(), ModuleKind::Module);
    assert!(prepared.proof_obligations().is_none());
    let (module_owner, module_name, version) = prepared.resolved_module()
        .ok_or("missing staged owner")?;
    assert_eq!((module_name.as_str(), version), ("arithmetic", 1));
    let goal = prepared.contract_goals().next().ok_or("missing typed contract")?;
    let subject = &goal.subject;
    assert_eq!(subject.definition, "increment");
    assert_eq!(subject.definition_ordinal, 1);
    assert_eq!(subject.definition_owner, module_owner);
    assert_eq!(subject.module_source.as_slice(), SOURCE);
    assert_eq!(subject.definition_source.as_slice(),
        b"def\tincrement# original lexical body\n[ 1 + ]");
    let start = usize::try_from(subject.source_span.start)
        .map_err(|_| "subject span start exceeds host address space")?;
    let end = usize::try_from(subject.source_span.end)
        .map_err(|_| "subject span end exceeds host address space")?;
    assert_eq!(SOURCE.get(start..end), Some(subject.definition_source.as_slice()));
    assert_eq!(subject.input_types, [Ty::I64]);
    assert_eq!(subject.output_types, [Ty::I64]);
    assert_eq!(subject.source_dependencies.len(), 1);
    assert_eq!(subject.source_dependencies[0].full_source.as_slice(), SOURCE);
    let root = subject.accepted_submission.body.candidate.body.first()
        .ok_or("missing selected subject invocation")?;
    let root = usize::try_from(root.0)
        .map_err(|_| "selected subject node exceeds host address space")?;
    let selected = subject.accepted_submission.body.candidate.nodes
        .get(root).ok_or("missing selected subject node")?;
    let noble_kernel::untrusted::Node::Invocation { def, .. } = selected else {
        return Err("subject is not a kernel-checked named invocation".into());
    };
    assert_ne!(def.0, subject.definition_ordinal,
        "lexical definition ordinal cannot be substituted for a specialization slot");
    let (session, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("{error:?}"))?;
    let executable = session.prepare(b"arithmetic@1.increment", &[Ty::I64], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(executable.output(), &[Ty::I64]);
    Ok(())
}

#[test]
fn imported_contract_retains_original_owner_source_and_ordinal() -> Result<(), String> {
    let session = ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let source = session.prepare(SOURCE, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let (session, outcome) = session.commit(source);
    outcome.map_err(|error| format!("{error:?}"))?;
    let original_owner = session.prepare(b"arithmetic@1.increment", &[Ty::I64], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?
        .resolved_module().ok_or("missing original module owner")?.0;
    let imported = session.prepare(b"import arithmetic@1 as a", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let (session, outcome) = session.commit(imported);
    outcome.map_err(|error| format!("{error:?}"))?;
    let pending = session.prepare(
        b"module consumer@1 [ proof 1 pending for a.increment-law [ (refl 0) ] ]",
        &[], super::LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    let batch = pending.proof_obligations().ok_or("missing imported contract goal")?;
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal else {
        return Err("import resolved to a non-contract claim".into());
    };
    assert_eq!((contract.subject.module.as_str(), contract.subject.version), ("arithmetic", 1));
    assert_eq!(contract.subject.definition_owner, original_owner);
    assert_eq!(contract.subject.definition_ordinal, 1);
    assert_eq!(contract.subject.module_source.as_slice(), SOURCE);
    assert_eq!(contract.subject.source_dependencies[0].full_source.as_slice(), SOURCE);
    let (session, outcome) = session.commit(pending);
    assert!(matches!(outcome, Err(error) if error.stage() == Stage::Acceptance));
    assert!(session.prepare(b"import consumer@1 as absent", &[], super::LIMITS).is_err());
    Ok(())
}

#[test]
fn named_subject_cannot_claim_a_direct_builtin_only_mc1_body() -> Result<(), String> {
    let session = ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let named = b"module named@1 [ def helper [ 1 + ] def increment [ helper ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] ]";
    assert!(matches!(session.prepare(named, &[], super::LIMITS), Err(error)
        if error.stage() == Stage::Check));
    assert_eq!(session.generation(), 0);
    assert!(session.prepare(b"import named@1 as absent", &[], super::LIMITS).is_err());
    Ok(())
}
