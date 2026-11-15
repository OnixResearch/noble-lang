use super::super::procedures::support::{
    add_record, envelope, literal_record, provenance, revision,
};
use noble_contracts::source::chunks;
use noble_contracts::source::chunks::{DefinitionRef, Node};
use noble_contracts::source::procedures::{
    Alias, CapabilityMarker, Certificate, ConsumerObligation, ConsumerPolicy, EvidenceClass,
    EvidenceOutcome, EvidenceRecord, LibraryDependency, ProcedureLibrary, ProcedureRecord,
    Producer, RefusalKind, Status,
};
use noble_contracts::source::Session;
use noble_kernel::types::{EffId, EffSet, Ty};
use std::string::String;
use std::vec;

#[path = "admission/alias/mod.rs"]
mod alias;
#[path = "admission/guards/criteria.rs"]
mod criteria;

fn evidence(
    record: &ProcedureRecord,
    class: EvidenceClass,
    outcome: EvidenceOutcome,
) -> EvidenceRecord {
    EvidenceRecord {
        obligation: 7,
        class,
        outcome,
        toolchain: vec![String::from("procedure-fixture-v1")],
        claim: String::from("exact subject evidence fixture"),
        subject_binding: record.procedure_id,
        policy_revision: 3,
        capability: None,
    }
}

#[test]
fn invalid_identity_contract_effect_and_capability_are_refused() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut forged_identity =
        literal_record(1, Producer::Chunk).map_err(|error| format!("record: {error:?}"))?;
    forged_identity.procedure_id = [0; 32];
    let refused = library
        .admit(&namespace, None, forged_identity)
        .expect_err("caller-selected identity must be refused");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    assert_eq!(refused.location.field, "procedure_id");

    let mut effectful =
        literal_record(2, Producer::Chunk).map_err(|error| format!("effectful: {error:?}"))?;
    effectful.chunk.interface.effects = EffSet::from_ids(&[EffId(17)]);
    let refused = library
        .admit(&namespace, None, effectful)
        .expect_err("effect escalation must be refused");
    assert_eq!(refused.kind, RefusalKind::Unsupported);
    assert_eq!(refused.location.field, "chunk.interface.effects");

    let mut capability =
        literal_record(3, Producer::Chunk).map_err(|error| format!("capability: {error:?}"))?;
    let mut record_evidence = evidence(&capability, EvidenceClass::Test, EvidenceOutcome::Passed);
    record_evidence.capability = Some(CapabilityMarker::LiveCapability);
    capability.evidence.push(record_evidence);
    let refused = library
        .admit(&namespace, None, capability)
        .expect_err("live capability must not be retained");
    assert_eq!(refused.kind, RefusalKind::LiveCapabilityInEvidence);
    assert_eq!(refused.location.field, "evidence.capability");

    let mut forged_proof =
        literal_record(4, Producer::Chunk).map_err(|error| format!("proof: {error:?}"))?;
    forged_proof.evidence.push(evidence(
        &forged_proof,
        EvidenceClass::Test,
        EvidenceOutcome::Proved,
    ));
    let refused = library
        .admit(&namespace, None, forged_proof)
        .expect_err("test class cannot claim a proof outcome");
    assert_eq!(refused.kind, RefusalKind::ForgedStatus);
    assert_eq!(refused.location.field, "evidence.outcome");
    assert_eq!(library.records().len(), 0);
    Ok(())
}

#[test]
fn evidence_classes_policy_and_dependency_edges_are_not_laundered() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut tested =
        literal_record(5, Producer::Chunk).map_err(|error| format!("tested: {error:?}"))?;
    tested.evidence.push(evidence(
        &tested,
        EvidenceClass::Test,
        EvidenceOutcome::Failed,
    ));
    tested.status = Status::Tested;
    tested.refresh_retrieval_signature();
    let refused = library
        .admit(&namespace, None, tested)
        .expect_err("failed test cannot support tested state");
    assert_eq!(refused.kind, RefusalKind::ForgedStatus);

    let mut promoted =
        literal_record(6, Producer::Chunk).map_err(|error| format!("promoted: {error:?}"))?;
    let mut proof = evidence(&promoted, EvidenceClass::Lean, EvidenceOutcome::Proved);
    proof.policy_revision = 3;
    promoted.evidence.push(proof);
    promoted.status = Status::Promoted;
    promoted.promotion = Some(noble_contracts::source::procedures::Promotion {
        policy_revision: 3,
        obligations: vec![7],
    });
    promoted.refresh_retrieval_signature();
    let policy = ConsumerPolicy {
        revision: 4,
        obligations: vec![ConsumerObligation {
            id: 7,
            allowed_classes: vec![EvidenceClass::Lean],
        }],
    };
    let refused = library
        .admit(&namespace, Some(&policy), promoted.clone())
        .expect_err("stale consumer policy must be refused");
    assert_eq!(refused.kind, RefusalKind::StaleContext);
    assert_eq!(refused.location.field, "promotion.policy_revision");

    let refused = library
        .admit(&namespace, None, promoted.clone())
        .expect_err("a promoted record without a selected consumer policy must be refused");
    assert_eq!(refused.kind, RefusalKind::StaleContext);
    assert_eq!(refused.location.field, "promotion.policy_revision");

    let current_policy = ConsumerPolicy {
        revision: 3,
        obligations: vec![ConsumerObligation {
            id: 7,
            allowed_classes: vec![EvidenceClass::Lean],
        }],
    };
    library
        .admit(&namespace, Some(&current_policy), promoted.clone())
        .map_err(|error| format!("current promotion: {error:?}"))?;
    let stale_candidate = literal_record(6, Producer::Chunk)
        .map_err(|error| format!("stale candidate: {error:?}"))?;
    let refusal = library
        .admit(&namespace, Some(&policy), stale_candidate)
        .expect_err("merge must not carry a promotion across policy revisions");
    assert_eq!(refusal.kind, RefusalKind::StaleContext);
    assert_eq!(refusal.location.field, "promotion.policy_revision");
    assert_eq!(library.records().len(), 1);
    assert_eq!(library.records()[0].status, Status::Promoted);
    Ok(())
}

#[test]
fn dangling_dependency_is_refused_without_changing_library() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut dangling =
        add_record(Producer::Chunk).map_err(|error| format!("dangling: {error:?}"))?;
    dangling.library_dependencies.push(LibraryDependency {
        procedure_id: [0x71; 32],
        chunk_id: [0x72; chunks::IDENTITY_BYTES],
        source_revision: [0x73; chunks::REVISION_BYTES],
    });
    dangling.procedure_id = dangling
        .identity_digest()
        .map_err(|error| format!("identity: {error:?}"))?;
    let refused = library
        .admit(&namespace, None, dangling)
        .expect_err("dependency absent from #73 envelope must be refused");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    assert_eq!(refused.location.field, "library_dependencies");
    assert!(library.records().is_empty());
    Ok(())
}

#[test]
fn forward_dependency_is_only_admitted_as_an_ordered_batch() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let future =
        literal_record(11, Producer::Chunk).map_err(|error| format!("future: {error:?}"))?;
    let mut chunk = envelope(
        &[],
        &[Ty::I64],
        b"11 ",
        vec![Node::Invoke {
            dependency: 0,
            instantiation: 0,
        }],
        vec![0],
    );
    let mut identity = [0; 8];
    identity.copy_from_slice(&future.chunk.chunk_id[..8]);
    chunk.dependencies.push(DefinitionRef {
        identity: u64::from_be_bytes(identity),
        format: future.chunk.format,
        semantic_revision: future.chunk.semantic_revision,
        source_revision: future.chunk.source_revision,
    });
    chunk.chunk_id = chunks::chunk_identity_of(&chunk);
    let parent = ProcedureRecord::candidate_with_dependencies(
        chunk,
        provenance(Producer::Composition),
        vec![LibraryDependency {
            procedure_id: future.procedure_id,
            chunk_id: future.chunk.chunk_id,
            source_revision: future.chunk.source_revision,
        }],
    )
    .map_err(|error| format!("parent: {error:?}"))?;

    let refused = library
        .admit(&namespace, None, parent.clone())
        .expect_err("single admission cannot resolve a forward edge");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    assert_eq!(refused.location.field, "library_dependencies");
    let ids = library
        .admit_batch(&namespace, None, vec![parent, future])
        .map_err(|error| format!("ordered batch: {error:?}"))?;
    assert_eq!(ids.len(), 2);
    assert_eq!(library.current_generation(), 1);
    assert_eq!(library.records().len(), 2);
    Ok(())
}

#[test]
fn status_and_retrieval_projections_are_recomputed() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut record =
        literal_record(8, Producer::Chunk).map_err(|error| format!("record: {error:?}"))?;
    record.retrieval.cost_key[0] = 1;
    let refused = library
        .admit(&namespace, None, record)
        .expect_err("caller cannot forge retrieval projection");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    assert_eq!(refused.location.field, "retrieval");
    assert!(library.records().is_empty());
    Ok(())
}

#[test]
fn effectful_caller_allowance_does_not_expand_pure_record_effects() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let record =
        literal_record(9, Producer::Chunk).map_err(|error| format!("record: {error:?}"))?;
    library
        .admit(&namespace, None, record)
        .map_err(|error| format!("admit: {error:?}"))?;
    assert_eq!(
        library.records()[0].chunk.interface.effects,
        EffSet::empty()
    );
    assert_eq!(library.records()[0].chunk.interface.stack_out, [Ty::I64]);
    Ok(())
}

#[test]
fn an_alias_digest_cannot_replace_a_verified_exact_pair_certificate() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut record =
        literal_record(10, Producer::Chunk).map_err(|error| format!("record: {error:?}"))?;
    record.alias_of = Some(Alias {
        procedure_id: [0x91; 32],
        certificate: Certificate::LegacyDigest([0x92; 32]),
    });
    let refusal = library
        .admit(&namespace, None, record)
        .expect_err("a digest is not a #56 certificate");
    assert_eq!(refusal.kind, RefusalKind::Unsupported);
    assert_eq!(refusal.location.field, "alias_of");
    assert!(library.records().is_empty());
    Ok(())
}
