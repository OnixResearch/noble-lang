use super::super::support::{
    add_record, add_seven_record, envelope, limits, provenance, revision, times_five_record,
};
use noble_contracts::source::chunks::{self, DefinitionRef, Node};
use noble_contracts::source::procedures::{
    CapabilityMarker, ComposeContext, EvidenceClass, EvidenceOutcome, EvidenceRecord,
    LibraryDependency, ProcedureId, ProcedureLibrary, ProcedureRecord, Producer, Query,
    QueryBudget, RefusalKind, Status,
};
use noble_contracts::source::Session;
use noble_kernel::types::{EffSet, Ty};
use std::string::String;
use std::vec;
use std::vec::Vec;

const SEEDS: [u64; 4] = [1, 7, 23, 101];
const QUERY_MASKS: u8 = 16;

fn dependent_on(parent: &ProcedureRecord) -> Result<ProcedureRecord, String> {
    let mut chunk = envelope(
        &[Ty::I64],
        &[Ty::I64],
        b"7 + ",
        vec![Node::Invoke {
            dependency: 0,
            instantiation: 0,
        }],
        vec![0],
    );
    let mut prefix = [0; 8];
    prefix.copy_from_slice(&parent.chunk.chunk_id[..8]);
    chunk.dependencies.push(DefinitionRef {
        identity: u64::from_be_bytes(prefix),
        format: parent.chunk.format,
        semantic_revision: parent.chunk.semantic_revision,
        source_revision: parent.chunk.source_revision,
    });
    chunk.chunk_id = chunks::chunk_identity_of(&chunk);
    ProcedureRecord::candidate_with_dependencies(
        chunk,
        provenance(Producer::Composition),
        vec![LibraryDependency {
            procedure_id: parent.procedure_id,
            chunk_id: parent.chunk.chunk_id,
            source_revision: parent.chunk.source_revision,
        }],
    )
    .map_err(|error| format!("dependent record: {error:?}"))
}

fn records() -> Result<Vec<ProcedureRecord>, String> {
    let first = add_seven_record(Producer::Chunk).map_err(|error| format!("first: {error:?}"))?;
    let mut tested =
        times_five_record(Producer::Chunk).map_err(|error| format!("tested: {error:?}"))?;
    tested.evidence.push(EvidenceRecord {
        obligation: 5,
        class: EvidenceClass::Test,
        outcome: EvidenceOutcome::Passed,
        toolchain: vec![String::from("generated-retrieval-fixture")],
        claim: String::from("a fixture test, not a proof"),
        subject_binding: tested.procedure_id,
        policy_revision: 1,
        capability: None,
    });
    tested.status = Status::Tested;
    tested.refresh_retrieval_signature();
    let binary = add_record(Producer::Chunk).map_err(|error| format!("binary: {error:?}"))?;
    let child = dependent_on(&first)?;
    let grandchild = dependent_on(&child)?;
    Ok(vec![first, tested, binary, child, grandchild])
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn generated_query(mask: u8, state: &mut u64, records: &[ProcedureRecord]) -> Query {
    let sample = next(state) as usize;
    let selected = &records[sample % records.len()];
    let tail = if sample & 1 == 0 { Ty::Bool } else { Ty::I64 };
    let mut before = vec![tail.clone()];
    before.extend_from_slice(&selected.chunk.interface.stack_in);
    let mut after = vec![tail];
    after.extend_from_slice(&selected.chunk.interface.stack_out);
    let is_full_positive = mask == QUERY_MASKS - 1;
    let status = match mask % 4 {
        0 => Status::Candidate,
        1 => Status::Tested,
        2 => Status::Promoted,
        _ => Status::Superseded,
    };
    Query {
        exact_interface: (mask & 1 != 0).then(|| {
            let record = if is_full_positive {
                &records[0]
            } else {
                selected
            };
            (
                record.chunk.interface.stack_in.clone(),
                record.chunk.interface.stack_out.clone(),
            )
        }),
        composition_context: (mask & 2 != 0).then(|| {
            if is_full_positive {
                (vec![Ty::Bool, Ty::I64], vec![Ty::Bool, Ty::I64])
            } else {
                (before, after)
            }
        }),
        dependency_root: (mask & 4 != 0).then(|| {
            if is_full_positive {
                records[0].procedure_id
            } else if sample % 6 == 5 {
                [0xa5; 32]
            } else {
                records[sample % records.len()].procedure_id
            }
        }),
        status: (mask & 8 != 0).then_some(if is_full_positive {
            Status::Candidate
        } else if mask == 8 {
            Status::Tested
        } else {
            status
        }),
    }
}

// Walk the record's declared parents, not the retrieval index's reverse-closure algorithm.
fn has_ancestor(records: &[ProcedureRecord], id: ProcedureId, root: ProcedureId) -> bool {
    let mut frontier = vec![id];
    for _ in 0..=records.len() {
        if frontier.contains(&root) {
            return true;
        }
        let mut next_frontier = Vec::new();
        for current in frontier {
            if let Some(record) = records.iter().find(|record| record.procedure_id == current) {
                next_frontier.extend(
                    record
                        .library_dependencies
                        .iter()
                        .map(|edge| edge.procedure_id),
                );
            }
        }
        frontier = next_frontier;
    }
    false
}

// Enumerate the only concrete context tails emitted by this generator, including the empty tail.
fn fits_context(record: &ProcedureRecord, before: &[Ty], after: &[Ty]) -> bool {
    [vec![], vec![Ty::Bool], vec![Ty::I64]]
        .into_iter()
        .any(|tail| {
            let mut input = tail.clone();
            input.extend_from_slice(&record.chunk.interface.stack_in);
            let mut output = tail;
            output.extend_from_slice(&record.chunk.interface.stack_out);
            input == before && output == after
        })
}

fn expected_ids(records: &[ProcedureRecord], query: &Query) -> Vec<ProcedureId> {
    records
        .iter()
        .filter(|record| {
            query.status.is_none_or(|status| record.status == status)
                && query
                    .exact_interface
                    .as_ref()
                    .is_none_or(|(input, output)| {
                        record.chunk.interface.stack_in == *input
                            && record.chunk.interface.stack_out == *output
                    })
                && query
                    .composition_context
                    .as_ref()
                    .is_none_or(|(before, after)| fits_context(record, before, after))
                && query
                    .dependency_root
                    .is_none_or(|root| has_ancestor(records, record.procedure_id, root))
        })
        .map(|record| record.procedure_id)
        .collect()
}

fn readmit_and_exercise(
    namespace: &Session,
    original: &[ProcedureRecord],
    selected: &[ProcedureId],
) -> Result<(), String> {
    if selected.is_empty() {
        return Ok(());
    }
    let mut fresh = ProcedureLibrary::new(revision(7));
    fresh
        .admit_batch(namespace, None, original.to_vec())
        .map_err(|error| format!("fresh ordinary chunk/source/kernel admission: {error:?}"))?;
    for id in selected {
        let record = fresh
            .records()
            .iter()
            .find(|record| record.procedure_id == *id)
            .ok_or_else(|| String::from("retrieved identity absent after fresh admission"))?;
        let composed = fresh
            .compose(
                namespace,
                &[*id],
                &ComposeContext {
                    prefix: Vec::new(),
                    suffix: Vec::new(),
                    stack_in: record.chunk.interface.stack_in.clone(),
                    stack_out: record.chunk.interface.stack_out.clone(),
                    allowed_effects: EffSet::empty(),
                    limits: limits(),
                },
            )
            .map_err(|error| format!("ordinary composed-source/kernel re-admission: {error:?}"))?;
        assert_eq!(composed.selected, [*id]);
        assert_eq!(
            composed.interface.stack_out,
            record.chunk.interface.stack_out
        );
        assert_eq!(composed.interface.effects, EffSet::empty());
    }
    Ok(())
}

#[test]
fn generated_queries_return_only_ids_and_require_readmission() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let fixture = records()?;
    library
        .admit_batch(&namespace, None, fixture)
        .map_err(|error| format!("initial admission: {error:?}"))?;
    let original = library.records().to_vec();
    assert_eq!(original.len(), 5);
    let mut is_nonempty = false;
    let mut is_empty = false;
    let mut is_tested = false;
    let mut retrieved_identity = None;
    for seed in SEEDS {
        let mut state = seed;
        let mut budget = QueryBudget::new();
        for mask in 0..QUERY_MASKS {
            let query = generated_query(mask, &mut state, &original);
            let result = library
                .retrieve(&query, &mut budget)
                .map_err(|error| format!("seed {seed}, mask {mask}: {error:?}"))?;
            let expected = expected_ids(&original, &query);
            assert_eq!(result.candidates, expected, "seed {seed}, mask {mask}");
            assert_eq!(result.examined_entries, 5);
            assert_eq!(result.index_bytes, 160);
            if mask == 0 {
                retrieved_identity = result.candidates.first().copied();
            }
            is_nonempty |= !expected.is_empty();
            is_empty |= expected.is_empty();
            is_tested |= expected.iter().any(|id| {
                original
                    .iter()
                    .any(|record| record.procedure_id == *id && record.status == Status::Tested)
            });
            readmit_and_exercise(&namespace, &original, &result.candidates)?;
        }
        for _ in 0..QUERY_MASKS {
            let all = library
                .retrieve(&Query::default(), &mut budget)
                .map_err(|error| format!("seed {seed}, budget fill: {error:?}"))?;
            assert_eq!(all.candidates.len(), original.len());
        }
        assert_eq!(budget.queries(), 32);
        assert!(budget.work() <= 4_096);
        let work = budget.work();
        let refused = library
            .retrieve(&Query::default(), &mut budget)
            .expect_err("33rd query on a nonempty library must refuse");
        assert_eq!(refused.kind, RefusalKind::Exhausted);
        assert_eq!(budget.queries(), 32);
        assert_eq!(budget.work(), work);
    }
    assert!(is_nonempty && is_empty && is_tested);
    assert_eq!(library.current_generation(), 1);
    assert_eq!(
        library.records(),
        original,
        "retrieval grants no status or authority"
    );

    // The returned ID is not proof evidence: even a correctly bound reference
    // cannot smuggle a live capability through ordinary admission.
    let retrieved_identity =
        retrieved_identity.ok_or_else(|| String::from("unfiltered query returned no identity"))?;
    let mut forged = original
        .iter()
        .find(|record| record.procedure_id == retrieved_identity)
        .ok_or_else(|| String::from("retrieved identity absent from records"))?
        .clone();
    forged.evidence.push(EvidenceRecord {
        obligation: 5,
        class: EvidenceClass::Lean,
        outcome: EvidenceOutcome::Proved,
        toolchain: vec![String::from("untrusted-proof-reference")],
        claim: String::from("not attested by an identity-only retrieval"),
        subject_binding: forged.procedure_id,
        policy_revision: 1,
        capability: Some(CapabilityMarker::LiveCapability),
    });
    let refusal = library
        .admit(&namespace, None, forged)
        .expect_err("retrieval identity must not authorize a forged capability");
    assert_eq!(refusal.kind, RefusalKind::LiveCapabilityInEvidence);
    assert_eq!(library.records(), original);
    Ok(())
}
