use noble_contracts::source::procedures::{ProcedureRecord, Query, Status};
use noble_kernel::types::Ty;
use std::vec;

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

pub(super) fn sample(mask: u8, state: &mut u64, records: &[ProcedureRecord]) -> Query {
    assert!(mask < super::QUERY_MASKS);
    assert!(!records.is_empty());
    let sample = usize::from(next(state).to_le_bytes()[0]);
    let selected_index = sample.checked_rem(records.len()).unwrap_or_default();
    let selected = &records[selected_index];
    let tail = if sample & 1 == 0 { Ty::Bool } else { Ty::I64 };
    let mut before = vec![tail.clone()];
    before.extend_from_slice(&selected.chunk.interface.stack_in);
    let mut after = vec![tail];
    after.extend_from_slice(&selected.chunk.interface.stack_out);
    let is_full_positive = mask == 15;
    let status = match mask & 3 {
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
            } else if sample.checked_rem(6) == Some(5) {
                [0xa5; 32]
            } else {
                records[selected_index].procedure_id
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
