use super::super::monotonic_now_ns;
use super::support::*;
use noble_contracts::source::chunks as chunk;
use noble_contracts::source::chunks::{Binding, Instantiation};

#[test]
fn composition_preserves_a_retained_tail() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let (entries, swap_id, duplicate_id) = stack_tail_library(&session)?;
    let candidate = request(
        hole(),
        vec![0],
        vec![Ty::Bool, Ty::I64, Ty::Bool],
        vec![Ty::Bool, Ty::Bool, Ty::I64, Ty::I64],
    );
    let generation = session.generation();
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"swap dup ");
    assert_eq!(accepted.selected.len(), 2);
    assert_eq!(accepted.selected, vec![swap_id, duplicate_id]);
    assert_eq!(
        accepted.equation.stack_in,
        vec![Ty::Bool, Ty::I64, Ty::Bool]
    );
    assert_eq!(
        accepted.equation.stack_out,
        vec![Ty::Bool, Ty::Bool, Ty::I64, Ty::I64]
    );
    assert_eq!(accepted.equation.tail, 1);
    assert_eq!(report.candidates, 6);
    assert!(report.rejected >= 5);
    assert_eq!(session.generation(), generation);
    // The completion is fresh source: preparing it again against the same task
    // is what accepted it, and no program was committed.
    let prepared = session
        .prepare(&accepted.source, &candidate.task.stack_in, limits())
        .map_err(|error| format!("prepare: {error:?}"))?;
    assert_eq!(prepared.output(), [Ty::Bool, Ty::Bool, Ty::I64, Ty::I64]);
    Ok(())
}

#[test]
fn a_later_chunk_can_consume_the_initial_stack_tail() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let duplicate_body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Dup,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut duplicate = envelope(&[Ty::Bool], &[Ty::Bool, Ty::Bool], duplicate_body, b"dup ");
    duplicate.instantiations = vec![Instantiation {
        bindings: vec![Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::Bool,
        }],
    }];
    duplicate.chunk_id = chunk::chunk_identity_of(&duplicate);

    let drop_all = drop_all_chunk();
    let entries = admitted(&session, &[duplicate.clone(), drop_all.clone()])?;
    let candidate = request(hole(), vec![0], vec![Ty::I64, Ty::Bool], Vec::new());
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, .. } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"dup drop drop drop ");
    assert_eq!(
        accepted.selected,
        vec![duplicate.chunk_id, drop_all.chunk_id]
    );
    assert!(accepted.equation.stack_out.is_empty());
    assert_eq!(accepted.equation.tail, 0);
    Ok(())
}

fn drop_all_chunk() -> Envelope {
    let body = Recipe {
        nodes: vec![
            Node::Operation {
                op: Op::Drop,
                instantiation: 0,
            },
            Node::Operation {
                op: Op::Drop,
                instantiation: 0,
            },
            Node::Operation {
                op: Op::Drop,
                instantiation: 1,
            },
        ],
        entry: vec![0, 1, 2],
    };
    let mut chunk = envelope(
        &[Ty::I64, Ty::Bool, Ty::Bool],
        &[],
        body,
        b"drop drop drop ",
    );
    chunk.instantiations = vec![
        Instantiation {
            bindings: vec![Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::Bool,
            }],
        },
        Instantiation {
            bindings: vec![Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            }],
        },
    ];
    chunk.chunk_id = chunk::chunk_identity_of(&chunk);
    assert_eq!(chunk.interface.stack_in, [Ty::I64, Ty::Bool, Ty::Bool]);
    assert_eq!(chunk.body.entry, [0, 1, 2]);
    chunk
}

#[test]
fn fixed_baseline_measurement_reports_bounded_work() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let (entries, _, _) = stack_tail_library(&session)?;
    let candidate = request(
        hole(),
        vec![0],
        vec![Ty::Bool, Ty::I64, Ty::Bool],
        vec![Ty::Bool, Ty::Bool, Ty::I64, Ty::I64],
    );
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"swap dup ");
    assert_eq!(report.candidates, 6);
    assert!(report.checked_candidates > 0);
    assert!(report.expanded_nodes > 0);
    assert!(report.checking_work > 0);
    assert!(report.checking_work <= Budget::PROFILE.checking_work);
    assert!(report.source_bytes > 0);
    assert!(report.retained_bytes > 0);
    assert!(report.total_latency_ns > 0);
    maybe_write_measurement_receipt(
        &accepted,
        &report,
        MEASUREMENT_RECEIPT_PATH,
        MEASUREMENT_SOURCE_COMMIT,
    )?;
    Ok(())
}
