use super::*;

fn record_position(records: &[ProcedureRecord], procedure_id: &ProcedureId) -> Option<usize> {
    let mut index = 0;
    let mut found = None;
    while index < records.len() && found.is_none() {
        if &records[index].procedure_id == procedure_id {
            found = Some(index);
        }
        index += 1;
    }
    found
}

fn validate_unique_records(records: &[ProcedureRecord]) -> Result<(), Refusal> {
    let mut index = 0;
    let mut result = Ok(());
    while index < records.len() && result.is_ok() {
        if record_position(&records[..index], &records[index].procedure_id).is_some() {
            result = Err(Refusal::at(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "procedure_id",
                    message: "a generation cannot retain the same procedure identity twice",
                },
                index as u32,
            ));
        }
        index += 1;
    }
    result
}

fn validate_retained_record(
    retained: &ProcedureRecord,
    records: &[ProcedureRecord],
) -> Result<(), Refusal> {
    let record = match record_position(records, &retained.procedure_id) {
        Some(index) => &records[index],
        None => {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "generation.closure",
                    message: "a publication must retain every subject from its parent generation",
                },
            ))
        }
    };
    if retained.supersedes.is_some() && record.supersedes != retained.supersedes {
        return Err(Refusal::new(
            RefusalKind::Invalid,
            Diagnostic {
                field: "supersedes",
                message: "a publication cannot rewrite a retained replacement predecessor",
            },
        ));
    }
    Ok(())
}

fn validate_publish(parent: &Snapshot, snapshot: &Snapshot) -> Result<(), Refusal> {
    let mut index = 0;
    let mut result = Ok(());
    while index < parent.records.len() && result.is_ok() {
        result = validate_retained_record(&parent.records[index], &snapshot.records);
        index += 1;
    }
    result
}

pub(super) fn validate(generations: &[Snapshot], snapshot: &Snapshot) -> Result<(), Refusal> {
    validate_unique_records(&snapshot.records)?;
    let Some(parent_generation) = snapshot.record.parent_generation else {
        return Ok(());
    };
    let parent = match usize::try_from(parent_generation) {
        Ok(index) => generations.get(index),
        Err(_) => None,
    };
    let parent = match parent {
        Some(parent) => parent,
        None => {
            return Err(Refusal::new(
                RefusalKind::Invalid,
                Diagnostic {
                    field: "generation.parent",
                    message: "generation parent is not retained",
                },
            ))
        }
    };
    match snapshot.record.kind {
        GenerationKind::Publish => validate_publish(parent, snapshot),
        GenerationKind::Rollback => {
            // Each snapshot's identity and state bindings are checked by the
            // caller. Compare the full retained records, not just their indexes.
            if snapshot.records != parent.records {
                return Err(Refusal::new(
                    RefusalKind::Invalid,
                    Diagnostic {
                        field: "generation.closure",
                        message: "rollback must reproduce the exact records of its named parent",
                    },
                ));
            }
            Ok(())
        }
    }
}
