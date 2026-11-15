use super::*;
use std::sync::OnceLock;

static SOURCE_RECEIPT: OnceLock<Result<serde_json::Value, String>> = OnceLock::new();

pub(super) fn synthesis_receipt() -> Result<&'static serde_json::Value, String> {
    let receipt = SOURCE_RECEIPT.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or_else(|| String::from("worktree root unavailable"))?;
        let path = root.join("verification/synthesis/receipts/2026-11-13/synthesis-v2.json");
        let bytes = std::fs::read_to_string(path)
            .map_err(|error| format!("read #116 producer receipt: {error}"))?;
        serde_json::from_str(&bytes)
            .map_err(|error| format!("decode #116 producer receipt: {error}"))
    });
    receipt.as_ref().map_err(Clone::clone)
}

pub(super) fn source_reference_cases(task: SourceTask) -> Result<Vec<([i64; 2], i64)>, String> {
    source_reference_cases_in(synthesis_receipt()?, task)
}

pub(super) fn source_reference_cases_in(
    receipt: &serde_json::Value,
    task: SourceTask,
) -> Result<Vec<([i64; 2], i64)>, String> {
    if receipt["schema"] != "noble-synthesis-receipt/v1"
        || receipt["grammar"]["i64"]["name"] != "synth-i64/0"
        || receipt["budget"]["max_candidates"].as_u64() != Some(400_000)
    {
        return Err(String::from(
            "#116 producer schema, grammar or budget drifted",
        ));
    }
    let tasks = receipt["tasks"]
        .as_array()
        .ok_or_else(|| String::from("#116 producer task list missing"))?;
    let mut matching = tasks.iter().filter(|entry| entry["name"] == task.name());
    let entry = matching
        .next()
        .ok_or_else(|| format!("{} missing in #116 producer receipt", task.name()))?;
    if matching.next().is_some() {
        return Err(format!(
            "{} is duplicated in #116 producer receipt",
            task.name()
        ));
    }
    let source = core::str::from_utf8(task.source())
        .map_err(|error| format!("source program utf8: {error}"))?;
    if entry["search"]["grammar"] != "synth-i64/0"
        || entry["search"]["program"] != source.trim()
        || entry["reference"].as_str().is_none_or(str::is_empty)
    {
        return Err(format!("{} source or reference drifted", task.name()));
    }
    let (class, outcome, claim) = task.evidence();
    let is_status_consistent = match (class, outcome) {
        (EvidenceClass::Lean, EvidenceOutcome::Proved) => {
            entry["status"] == "proved" && entry["certificate"]["theorem"] == claim
        }
        (EvidenceClass::Test, EvidenceOutcome::Passed) => entry["status"] == "unproven",
        _ => false,
    };
    if !is_status_consistent {
        return Err(format!("{} producer verifier status drifted", task.name()));
    }
    let heldout = &entry["heldout"];
    let examples = heldout["entries"]
        .as_array()
        .ok_or_else(|| format!("{} producer examples missing", task.name()))?;
    let example_count = u64::try_from(examples.len())
        .map_err(|_| format!("{} producer example count exceeds U64", task.name()))?;
    if heldout["status"] != "pass" {
        return Err(format!("{} producer example totals drifted", task.name()));
    }
    if heldout["passed"].as_u64() != Some(example_count) {
        return Err(format!("{} producer example totals drifted", task.name()));
    }
    if heldout["total"].as_u64() != Some(example_count) {
        return Err(format!("{} producer example totals drifted", task.name()));
    }
    if examples.len() != 2 {
        return Err(format!("{} producer example totals drifted", task.name()));
    }
    examples
        .iter()
        .map(|example| parse_source_example(example, task))
        .collect()
}

fn parse_source_example(
    example: &serde_json::Value,
    task: SourceTask,
) -> Result<([i64; 2], i64), String> {
    let inputs = example["inputs"]
        .as_array()
        .filter(|inputs| inputs.len() == 2)
        .ok_or_else(|| format!("{} producer input arity drifted", task.name()))?;
    let parse = |value: &serde_json::Value| {
        value
            .as_str()
            .ok_or_else(|| String::from("producer scalar is not a decimal string"))?
            .parse::<i64>()
            .map_err(|error| format!("producer scalar exceeds I64: {error}"))
    };
    let first = inputs
        .first()
        .ok_or_else(|| String::from("first producer input missing"))?;
    let second = inputs
        .get(1)
        .ok_or_else(|| String::from("second producer input missing"))?;
    let expected = parse(&example["expected"])?;
    if example["agrees"] != true || parse(&example["observed"])? != expected {
        return Err(format!("{} producer example does not agree", task.name()));
    }
    Ok(([parse(first)?, parse(second)?], expected))
}

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
