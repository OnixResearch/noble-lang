#![allow(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; this retrieval fixture exercises the complete ordered-filter and budget contract in one deterministic case"
)]

use super::super::procedures::support::{
    add_record, add_seven_record, revision, times_five_record,
};
use noble_contracts::source::procedures::{
    ProcedureLibrary, Producer, Query, QueryBudget, RefusalKind, Status,
};
use noble_contracts::source::Session;

use noble_kernel::types::Ty;
use std::vec;

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
