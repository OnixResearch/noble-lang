use super::*;

pub(super) fn task_record(task: SourceTask, producer: Producer) -> Result<ProcedureRecord, String> {
    let nodes = task.nodes();
    let entry = (0..nodes.len()).map(|index| index as u32).collect();
    let mut chunk = super::envelope(&[Ty::I64, Ty::I64], &[Ty::I64], task.source(), nodes, entry);
    if task == SourceTask::Product {
        chunk.instantiations = vec![
            Instantiation {
                bindings: Vec::new(),
            },
            Instantiation {
                bindings: vec![Binding {
                    variable: 1,
                    kind: noble_kernel::words::VariableKind::Value,
                    ty: Ty::I64,
                }],
            },
        ];
        chunk.chunk_id = chunks::chunk_identity_of(&chunk);
    }
    let task_provenance = match producer {
        Producer::Synthesis => {
            let mut task_id = [0; 32];
            let task_name = task.name().as_bytes();
            task_id[..task_name.len()].copy_from_slice(task_name);
            Provenance {
                producer,
                producer_tool_revision: 2,
                task_id: Some(task_id),
                grammar_revision: Some(0),
            }
        }
        Producer::Chunk | Producer::Composition => super::provenance(producer),
    };
    let mut output = ProcedureRecord::candidate(chunk, task_provenance)
        .map_err(|error| format!("{} record: {error:?}", task.name()))?;
    if producer == Producer::Synthesis {
        let (class, outcome, claim) = task.evidence();
        output.evidence.push(EvidenceRecord {
            obligation: 1,
            class,
            outcome,
            toolchain: vec![String::from("synthesis-v2"), String::from("Lean 4.35.0")],
            claim: String::from(claim),
            subject_binding: output.procedure_id,
            policy_revision: 0,
            capability: None,
        });
    }
    Ok(output)
}

pub(super) fn admit_source(
    library: &mut ProcedureLibrary,
    namespace: &Session,
    task: SourceTask,
    producer: Producer,
    build_ns: &mut u64,
) -> Result<RetainedSource, String> {
    let record = task_record(task, producer)?;
    let started = Instant::now();
    let id = library
        .admit(namespace, None, record)
        .map_err(|error| format!("admit {}: {error:?}", task.name()))?;
    *build_ns = build_ns
        .checked_add(elapsed_nanoseconds(started)?)
        .ok_or_else(|| String::from("measured acquisition time exceeds the metrics counter"))?;
    Ok(RetainedSource { task, id })
}

pub(super) fn elapsed_nanoseconds(started: Instant) -> Result<u64, String> {
    u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| String::from("measured duration exceeds the metrics counter"))
}

pub(super) fn paired_sources<'a>(
    library: &'a ProcedureLibrary,
    sources: &[RetainedSource],
    order: [SourceTask; 2],
) -> Option<[&'a ProcedureRecord; 2]> {
    let first_id = sources.iter().find(|source| source.task == order[0])?.id;
    let second_id = sources.iter().find(|source| source.task == order[1])?.id;
    Some([
        library
            .records()
            .iter()
            .find(|record| record.procedure_id == first_id)?,
        library
            .records()
            .iter()
            .find(|record| record.procedure_id == second_id)?,
    ])
}
