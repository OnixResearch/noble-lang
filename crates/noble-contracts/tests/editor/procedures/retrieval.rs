#![allow(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; this retrieval fixture exercises the complete ordered-filter and budget contract in one deterministic case"
)]

use super::super::procedures::support::{
    add_record, add_seven_record, literal_record, revision, times_five_record,
};
use noble_contracts::source::procedures::{
    ProcedureLibrary, Producer, Query, QueryBudget, RefusalKind, RuntimeProducer, Status,
};
use noble_contracts::source::Session;

use noble_kernel::types::Ty;
use std::cell::Cell;
use std::vec;

#[path = "retrieval/generated.rs"]
mod generated;

#[test]
fn filters_are_ordered_identity_only_subsequences() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let ids = library
        .admit_batch(
            &namespace,
            None,
            vec![
                add_seven_record(Producer::Chunk).map_err(|error| format!("A: {error:?}"))?,
                times_five_record(Producer::Chunk).map_err(|error| format!("B: {error:?}"))?,
                add_record(Producer::Chunk).map_err(|error| format!("C: {error:?}"))?,
            ],
        )
        .map_err(|error| format!("batch: {error:?}"))?;
    let all = library
        .records()
        .iter()
        .map(|record| record.procedure_id)
        .collect::<Vec<_>>();
    let mut budget = QueryBudget::new();

    let exact = library
        .retrieve(
            &Query {
                exact_interface: Some((vec![Ty::I64], vec![Ty::I64])),
                composition_context: None,
                dependency_root: None,
                status: None,
            },
            &mut budget,
        )
        .map_err(|error| format!("exact: {error:?}"))?;
    assert_eq!(exact.examined_entries, 3);
    assert_eq!(exact.index_bytes, 96);
    let expected_exact = all
        .iter()
        .copied()
        .filter(|id| {
            library
                .records()
                .iter()
                .find(|record| record.procedure_id == *id)
                .is_some_and(|record| {
                    record.chunk.interface.stack_in == [Ty::I64]
                        && record.chunk.interface.stack_out == [Ty::I64]
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(exact.candidates, expected_exact);

    let compatible = library
        .retrieve(
            &Query {
                composition_context: Some((vec![Ty::Bool, Ty::I64], vec![Ty::Bool, Ty::I64])),
                exact_interface: None,
                dependency_root: None,
                status: None,
            },
            &mut budget,
        )
        .map_err(|error| format!("compatible context: {error:?}"))?;
    assert_eq!(compatible.candidates.len(), 2);
    assert!(compatible
        .candidates
        .iter()
        .all(|id| exact.candidates.contains(id)));

    let dependency = library
        .retrieve(
            &Query {
                dependency_root: Some(ids[0]),
                exact_interface: None,
                composition_context: None,
                status: None,
            },
            &mut budget,
        )
        .map_err(|error| format!("dependency root: {error:?}"))?;
    assert_eq!(dependency.candidates, [ids[0]]);

    let status = library
        .retrieve(
            &Query {
                status: Some(Status::Candidate),
                exact_interface: None,
                composition_context: None,
                dependency_root: None,
            },
            &mut budget,
        )
        .map_err(|error| format!("status: {error:?}"))?;
    assert_eq!(status.candidates, all);
    assert_eq!(budget.queries(), 4);
    assert_eq!(library.current_generation(), 1);
    Ok(())
}

#[test]
fn query_budget_counts_and_refuses_after_bound() -> Result<(), String> {
    let library = ProcedureLibrary::new(revision(7));
    let mut budget = QueryBudget::new();
    for _ in 0..32 {
        library
            .retrieve(
                &Query {
                    exact_interface: None,
                    composition_context: None,
                    dependency_root: None,
                    status: None,
                },
                &mut budget,
            )
            .map_err(|error| format!("query: {error:?}"))?;
    }
    assert_eq!(budget.queries(), 32);
    let refused = library
        .retrieve(
            &Query {
                exact_interface: None,
                composition_context: None,
                dependency_root: None,
                status: None,
            },
            &mut budget,
        )
        .expect_err("33rd query exceeds declared bound");
    assert_eq!(refused.kind, RefusalKind::Exhausted);
    assert_eq!(budget.queries(), 32);
    assert_eq!(budget.work(), 0);
    assert!(library.records().is_empty());
    assert_eq!(library.current_generation(), 0);
    Ok(())
}

#[test]
fn attempted_runtime_proposer_and_teacher_calls_refuse_before_invocation() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let source = literal_record(1, Producer::Chunk)
        .map_err(|error| format!("non-held-out source: {error:?}"))?;
    library
        .admit(&namespace, None, source.clone())
        .map_err(|error| format!("source admission: {error:?}"))?;
    let before = library
        .identity_manifest()
        .map_err(|error| format!("before identity: {error:?}"))?;
    let calls = Cell::new(0u32);
    let mut budget = QueryBudget::new();
    for producer in [RuntimeProducer::Proposer, RuntimeProducer::Teacher] {
        let refusal = library
            .request_runtime_producer(producer, &mut budget, || {
                calls.set(calls.get().saturating_add(1));
                source.clone()
            })
            .expect_err("the disabled runtime producer must refuse before calling the host");
        assert_eq!(refusal.kind, RefusalKind::Unsupported);
        assert_eq!(
            refusal.location,
            noble_contracts::source::chunks::Location::field("retrieval.runtime_producer")
        );
        assert_eq!(calls.get(), 0, "the host producer must never be called");
    }
    assert_eq!(budget.proposer_attempts(), 1);
    assert_eq!(budget.teacher_attempts(), 1);
    assert_eq!(budget.queries(), 0);
    assert_eq!(budget.work(), 0);
    assert_eq!(calls.get(), 0, "zero actual producer calls were recorded");
    assert_eq!(library.current_generation(), 1);
    assert_eq!(
        library
            .identity_manifest()
            .map_err(|error| format!("after identity: {error:?}"))?,
        before,
        "refused requests must not mutate retained source state"
    );
    Ok(())
}
