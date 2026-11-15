use super::super::procedures::support::{
    add_record, envelope, literal_record, provenance, revision,
};
use noble_contracts::source::chunks;
use noble_contracts::source::chunks::{DefinitionRef, Node, Op};
use noble_contracts::source::procedures::{
    Alias, AliasCertificate, CapabilityMarker, ConsumerObligation, ConsumerPolicy, EvidenceClass,
    EvidenceOutcome, EvidenceRecord, LibraryDependency, ProcedureLibrary, ProcedureRecord,
    Producer, RefusalKind, Status,
};
use noble_contracts::source::Session;
use noble_kernel::types::{EffId, EffSet, Ty};
use std::string::String;
use std::vec;
use std::vec::Vec;

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
        certificate: AliasCertificate::LegacyDigest([0x92; 32]),
    });
    let refusal = library
        .admit(&namespace, None, record)
        .expect_err("a digest is not a #56 certificate");
    assert_eq!(refusal.kind, RefusalKind::Unsupported);
    assert_eq!(refusal.location.field, "alias_of");
    assert!(library.records().is_empty());
    Ok(())
}

fn polynomial_record(added: &[i64]) -> Result<ProcedureRecord, String> {
    let mut source = String::new();
    let mut nodes = Vec::new();
    let mut entry = Vec::new();
    for value in added {
        source.push_str(&format!("{value} + "));
        entry.push(u32::try_from(nodes.len()).map_err(|error| error.to_string())?);
        nodes.push(Node::I64(*value));
        entry.push(u32::try_from(nodes.len()).map_err(|error| error.to_string())?);
        nodes.push(Node::Operation {
            op: Op::Add,
            instantiation: 0,
        });
    }
    ProcedureRecord::candidate(
        envelope(&[Ty::I64], &[Ty::I64], source.as_bytes(), nodes, entry),
        provenance(Producer::Chunk),
    )
    .map_err(|error| format!("polynomial record: {error:?}"))
}

#[test]
fn exact_pair_alias_normalizes_distinct_bodies_and_survives_reload() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let target = polynomial_record(&[5])?;
    let mut candidate = polynomial_record(&[2, 3])?;
    assert_ne!(target.procedure_id, candidate.procedure_id);
    Alias::certify(&candidate, &target)
        .map_err(|error| format!("certificate: {error:?}"))?
        .attach(&mut candidate)
        .map_err(|error| format!("attach: {error:?}"))?;
    let AliasCertificate::ExactPair(pair) = &candidate
        .alias_of
        .as_ref()
        .expect("claim")
        .certificate
    else {
        return Err(String::from("a real alias requires a pair certificate"));
    };
    assert_eq!(pair.left, pair.right);
    assert_eq!(pair.left_closure, pair.right_closure);
    library
        .admit(&namespace, None, target)
        .map_err(|error| format!("target: {error:?}"))?;
    library
        .admit(&namespace, None, candidate.clone())
        .map_err(|error| format!("alias: {error:?}"))?;
    let encoded = library.encode().map_err(|error| format!("encode: {error:?}"))?;
    let loaded = ProcedureLibrary::load(&encoded, &namespace)
        .map_err(|error| format!("reload: {error:?}"))?;
    assert_eq!(
        loaded.encode().map_err(|error| format!("re-encode: {error:?}"))?,
        encoded
    );
    assert!(loaded.records().iter().any(|record| record == &candidate));
    Ok(())
}

#[test]
fn exact_pair_alias_normalizes_distribution_and_wrapping_i64() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let left = ProcedureRecord::candidate(
        envelope(
            &[Ty::I64],
            &[Ty::I64],
            b"2 + 3 * ",
            vec![
                Node::I64(2),
                Node::Operation {
                    op: Op::Add,
                    instantiation: 0,
                },
                Node::I64(3),
                Node::Operation {
                    op: Op::Mul,
                    instantiation: 0,
                },
            ],
            vec![0, 1, 2, 3],
        ),
        provenance(Producer::Chunk),
    )
    .map_err(|error| format!("left distributive: {error:?}"))?;
    let right = ProcedureRecord::candidate(
        envelope(
            &[Ty::I64],
            &[Ty::I64],
            b"3 * 6 + ",
            vec![
                Node::I64(3),
                Node::Operation {
                    op: Op::Mul,
                    instantiation: 0,
                },
                Node::I64(6),
                Node::Operation {
                    op: Op::Add,
                    instantiation: 0,
                },
            ],
            vec![0, 1, 2, 3],
        ),
        provenance(Producer::Chunk),
    )
    .map_err(|error| format!("right distributive: {error:?}"))?;
    let mut aliased = left;
    Alias::certify(&aliased, &right)
        .map_err(|error| format!("distributive pair: {error:?}"))?
        .attach(&mut aliased)
        .map_err(|error| format!("attach distributive pair: {error:?}"))?;
    library
        .admit(&namespace, None, right)
        .map_err(|error| format!("right: {error:?}"))?;
    library
        .admit(&namespace, None, aliased)
        .map_err(|error| format!("distributed alias: {error:?}"))?;

    let wrapped = polynomial_record(&[i64::MAX, 1])?;
    let minimum = polynomial_record(&[i64::MIN])?;
    let mut wrapped_alias = wrapped;
    Alias::certify(&wrapped_alias, &minimum)
        .map_err(|error| format!("wrapping pair: {error:?}"))?
        .attach(&mut wrapped_alias)
        .map_err(|error| format!("attach wrapping pair: {error:?}"))?;
    library
        .admit(&namespace, None, minimum)
        .map_err(|error| format!("minimum: {error:?}"))?;
    library
        .admit(&namespace, None, wrapped_alias)
        .map_err(|error| format!("wrapping alias: {error:?}"))?;
    Ok(())
}

#[test]
fn unequal_polynomials_and_unsupported_alias_bodies_are_refused() -> Result<(), String> {
    let target = polynomial_record(&[5])?;
    let mut different = polynomial_record(&[6])?;
    let refused = Alias::certify(&different, &target)
        .expect_err("different polynomial must not produce a certificate");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    Alias::certify(&polynomial_record(&[2, 3])?, &target)
        .map_err(|error| format!("good pair: {error:?}"))?
        .attach(&mut different)
        .map_err(|error| format!("attach: {error:?}"))?;
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    library
        .admit(&namespace, None, target.clone())
        .map_err(|error| format!("target: {error:?}"))?;
    let refusal = library
        .admit(&namespace, None, different)
        .expect_err("a borrowed valid pair certificate cannot claim a different body");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    let mut boolean = literal_record(50, Producer::Chunk)
        .map_err(|error| format!("literal: {error:?}"))?;
    boolean.chunk.body.nodes.push(Node::Bool(true));
    boolean.chunk.body.entry.push(1);
    boolean.chunk.interface.stack_out.push(Ty::Bool);
    let refused = Alias::certify(&boolean, &target)
        .expect_err("Boolean body is outside the alias fragment");
    assert_eq!(refused.kind, RefusalKind::Unsupported);

    let mut dependent = polynomial_record(&[5])?;
    dependent.library_dependencies.push(LibraryDependency {
        procedure_id: target.procedure_id,
        chunk_id: target.chunk.chunk_id,
        source_revision: target.chunk.source_revision,
    });
    let refused = Alias::certify(&dependent, &target)
        .expect_err("dependency-bearing body is outside the verified fragment");
    assert_eq!(refused.kind, RefusalKind::Unsupported);
    Ok(())
}

#[test]
fn alias_closure_and_normal_forms_are_recomputed_on_admission_and_reload() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let target = polynomial_record(&[5])?;
    let mut candidate = polynomial_record(&[2, 3])?;
    let alias = Alias::certify(&candidate, &target)
        .map_err(|error| format!("certificate: {error:?}"))?;
    let mut library = ProcedureLibrary::new(revision(7));
    library
        .admit(&namespace, None, target.clone())
        .map_err(|error| format!("target: {error:?}"))?;

    let mut closure_forgery = alias.clone();
    if let AliasCertificate::ExactPair(pair) = &mut closure_forgery.certificate {
        pair.left_closure[0] ^= 1;
    }
    closure_forgery
        .attach(&mut candidate)
        .map_err(|error| format!("attach closure: {error:?}"))?;
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("forged closure digest must not be an authorization");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    let mut stale_form = alias.clone();
    if let AliasCertificate::ExactPair(pair) = &mut stale_form.certificate {
        pair.left[0].terms[0].coefficient ^= 1;
    }
    candidate.alias_of = Some(stale_form);
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("forged polynomial form must not be an authorization");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    let mut stale_target = alias.clone();
    stale_target.procedure_id[0] ^= 1;
    candidate.alias_of = Some(stale_target);
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("stale pair target is not retained");
    assert_eq!(refusal.kind, RefusalKind::StaleContext);
    assert_eq!(refusal.location.field, "alias_of");

    candidate.alias_of = Some(alias.clone());
    candidate.evidence.clear();
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("missing exact-pair evidence must be refused");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    candidate.evidence.push(evidence(
        &candidate,
        EvidenceClass::Validation,
        EvidenceOutcome::Passed,
    ));
    let refusal = library
        .admit(&namespace, None, candidate.clone())
        .expect_err("unrelated validation evidence must be refused");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "alias_of");

    let mut correctly_bound = candidate.clone();
    correctly_bound.evidence.clear();
    correctly_bound.alias_of = None;
    alias
        .clone()
        .attach(&mut correctly_bound)
        .map_err(|error| format!("attach evidence: {error:?}"))?;
    for case in 0..4 {
        let mut forged = correctly_bound.clone();
        let record = forged
            .evidence
            .first_mut()
            .ok_or_else(|| String::from("missing own evidence"))?;
        match case {
            0 => record.class = EvidenceClass::Review,
            1 => record.outcome = EvidenceOutcome::Failed,
            2 => record.toolchain = vec![String::from("unrelated-checker")],
            _ => record.claim.push_str(":other-pair"),
        }
        let refusal = library
            .admit(&namespace, None, forged)
            .expect_err("changed class/outcome/checker/scope cannot claim this alias");
        assert_eq!(refusal.kind, RefusalKind::Invalid);
        assert_eq!(refusal.location.field, "alias_of");
    }

    // A raw wire mutation also fails reload; the earlier admission forgeries
    // exercised pair recomputation with coherent newly computed state digests.
    candidate.alias_of = Some(alias);
    candidate.evidence.clear();
    let certificate = candidate.alias_of.take().expect("pair certificate");
    certificate
        .attach(&mut candidate)
        .map_err(|error| format!("attach good: {error:?}"))?;
    library
        .admit(&namespace, None, candidate)
        .map_err(|error| format!("good alias: {error:?}"))?;
    let mut bytes = library.encode().map_err(|error| format!("encode: {error:?}"))?;
    let alias = library
        .records()
        .iter()
        .find_map(|record| record.alias_of.as_ref())
        .ok_or_else(|| String::from("missing retained alias"))?;
    let AliasCertificate::ExactPair(pair) = &alias.certificate else {
        return Err(String::from("missing pair certificate"));
    };
    let marker: Vec<u8> = [vec![2], alias.procedure_id.to_vec(), pair.left_closure.to_vec()]
        .concat();
    let offset = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or_else(|| String::from("pair wire marker absent"))?;
    bytes[offset + 33] ^= 1;
    let refusal = ProcedureLibrary::load(&bytes, &namespace)
        .expect_err("tampered persisted closure cannot reload");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    Ok(())
}
