use super::*;
use noble_contracts::source::procedures::{RefusalKind, RuntimeProducer};
use std::cell::Cell;

const CRITERIA: &str = "verification/procedure-library/criteria-producer-targets-v3.json";
const TARGET_RECEIPT: &str = "verification/procedure-library/receipts/producer-targets-v3.json";
const TARGET_NAMES: [&str; 2] = ["arithCommutedThenSubtract", "arithSubtractThenCommuted"];
const TARGET_SOURCE_ORDERS: [[&str; 2]; 2] = [
    ["arithCommuted", "arithSubtract"],
    ["arithSubtract", "arithCommuted"],
];

pub(super) struct ProducerTargets {
    pub(super) set: TargetSet,
    pub(super) summary: serde_json::Value,
}

struct TargetDeclaration<'a> {
    name: &'a str,
    reference: &'a str,
}

pub(super) fn read_json(relative: &str) -> Result<serde_json::Value, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| String::from("worktree root unavailable"))?;
    let bytes =
        std::fs::read(root.join(relative)).map_err(|error| format!("read {relative}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("decode {relative}: {error}"))
}

fn target_program(
    receipt: &serde_json::Value,
    target: TargetDeclaration<'_>,
) -> Result<(String, serde_json::Value), String> {
    let TargetDeclaration { name, reference } = target;
    if receipt["name"] != name || receipt["reference"] != reference {
        return Err(format!("{name}: producer target name or reference drifted"));
    }
    if receipt["declaration"]["inputs"].as_u64() != Some(3)
        || receipt["declaration"]["bound"].as_u64() != Some(8)
        || receipt["search"]["grammar"] != "synth-i64/0"
    {
        return Err(format!("{name}: target declaration or grammar drifted"));
    }
    if receipt["status"] != "proved" && receipt["status"] != "unproven" {
        return Err(format!(
            "{name}: producer did not emit a checkable candidate"
        ));
    }
    let program = receipt["search"]["program"]
        .as_str()
        .filter(|program| !program.is_empty())
        .ok_or_else(|| format!("{name}: producer emitted no candidate"))?;
    if receipt["readmission"]["graph"]["outcome"] != "normal"
        || receipt["readmission"]["graph"]["evaluators_agree"] != true
    {
        return Err(format!(
            "{name}: generated body did not pass CLI re-admission"
        ));
    }
    let answers = receipt["solver_check"]["answers"]
        .as_array()
        .ok_or_else(|| format!("{name}: solver trace missing"))?;
    if answers.len() != 2 || answers.iter().any(|answer| answer["answer"] != "unsat") {
        return Err(format!(
            "{name}: solver did not reach two recorded UNSAT verdicts"
        ));
    }
    let rounds = receipt["rounds"]
        .as_array()
        .filter(|rounds| !rounds.is_empty())
        .ok_or_else(|| format!("{name}: producer search trace missing"))?;
    let heldout = &receipt["heldout"];
    let examples = heldout["entries"]
        .as_array()
        .filter(|examples| examples.len() == 3)
        .ok_or_else(|| format!("{name}: producer held-out examples missing"))?;
    if heldout["status"] != "pass" || heldout["passed"].as_u64() != Some(3) {
        return Err(format!("{name}: producer held-out checks failed"));
    }
    for example in examples {
        let inputs: [i64; 3] = example["inputs"]
            .as_array()
            .filter(|inputs| inputs.len() == 3)
            .ok_or_else(|| format!("{name}: producer input arity drifted"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| String::from("producer input is not a decimal string"))?
                    .parse::<i64>()
                    .map_err(|error| format!("producer input outside I64: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| String::from("producer input array length drifted"))?;
        let expected = example["expected"]
            .as_str()
            .ok_or_else(|| String::from("producer expected value missing"))?
            .parse::<i64>()
            .map_err(|error| format!("producer expected value outside I64: {error}"))?;
        let expected_text = expected.to_string();
        if example["agrees"] != true
            || example["observed"].as_str() != Some(expected_text.as_str())
            || evaluator::evaluate_program(program, inputs)? != expected
        {
            return Err(format!(
                "{name}: generated target disagrees on producer held-out input"
            ));
        }
    }
    let summary = serde_json::json!({
        "name": name,
        "reference": reference,
        "candidate": program,
        "status": receipt["status"],
        "search_rounds": rounds.len(),
        "counterexamples": receipt["examples"]["counterexamples"],
        "solver_answers": answers,
        "readmission": receipt["readmission"]["graph"],
        "heldout": heldout,
    });
    Ok((String::from(program), summary))
}

pub(super) fn load() -> Result<ProducerTargets, String> {
    let criteria = read_json(CRITERIA)?;
    let receipt = read_json(TARGET_RECEIPT)?;
    let manifest = super::target_reference::manifest_targets()?;
    if criteria["schema"] != "noble-procedure-library-producer-targets-criteria/3" {
        return Err(String::from("producer target criterion schema drifted"));
    }
    if criteria["producer"]["manifest"] != super::target_reference::MANIFEST {
        return Err(String::from("producer criterion manifest path drifted"));
    }
    for (index, name) in TARGET_NAMES.iter().copied().enumerate() {
        if criteria["producer"]["target_tasks"][index]["name"] != name || manifest[index].0 != name
        {
            return Err(format!("{name}: frozen criterion task identity drifted"));
        }
        if criteria["producer"]["target_tasks"][index]["reference"] != manifest[index].1
            || criteria["producer"]["target_tasks"][index]["source_order"]
                != serde_json::json!(TARGET_SOURCE_ORDERS[index])
        {
            return Err(format!("{name}: frozen criterion task identity drifted"));
        }
    }
    if receipt["schema"] != "noble-synthesis-receipt/v1"
        || receipt["source"]["manifest"] != "verification/procedure-library/producer-targets-v3.txt"
    {
        return Err(String::from("producer target source or budget drifted"));
    }
    if receipt["source"]["clean"] != true
        || receipt["budget"]["max_candidates"].as_u64() != Some(400_000)
    {
        return Err(String::from("producer target source or budget drifted"));
    }
    let tasks = receipt["tasks"]
        .as_array()
        .filter(|tasks| tasks.len() == 2)
        .ok_or_else(|| String::from("producer target task count drifted"))?;
    let mut inputs = [[[0i64; 3]; 3]; 4];
    for (index, seed) in SEEDS.iter().copied().enumerate() {
        inputs[index] =
            serde_json::from_value(criteria["held_out_case_inputs"][seed.to_string()].clone())
                .map_err(|error| format!("seed {seed} held-out input shape: {error}"))?;
    }
    let (first, first_summary) = target_program(
        &tasks[0],
        TargetDeclaration {
            name: TARGET_NAMES[0],
            reference: &manifest[0].1,
        },
    )?;
    let (second, second_summary) = target_program(
        &tasks[1],
        TargetDeclaration {
            name: TARGET_NAMES[1],
            reference: &manifest[1].1,
        },
    )?;
    let [(_, first_reference), (_, second_reference)] = manifest;
    let set = TargetSet {
        tasks: [
            HeldOutTask {
                name: TARGET_NAMES[0],
                kind: TargetKind::CommutedThenSubtract,
            },
            HeldOutTask {
                name: TARGET_NAMES[1],
                kind: TargetKind::SubtractThenCommuted,
            },
        ],
        inputs,
        programs: [Some(first), Some(second)],
        references: [Some(first_reference), Some(second_reference)],
    };
    Ok(ProducerTargets {
        set,
        summary: serde_json::json!([first_summary, second_summary]),
    })
}

pub(super) fn refusal_probe(seed: u64) -> Result<serde_json::Value, String> {
    let library = ProcedureLibrary::new(super::revision(7));
    let original = library
        .identity_manifest()
        .map_err(|error| format!("probe identity manifest: {error:?}"))?;
    let record = super::producer::task_record(SourceTask::Commuted, Producer::Chunk)?;
    let calls = Cell::new(0u32);
    let mut budget = QueryBudget::new();
    for producer in [RuntimeProducer::Proposer, RuntimeProducer::Teacher] {
        let refusal = library
            .request_runtime_producer(producer, &mut budget, || {
                calls.set(calls.get().saturating_add(1));
                record.clone()
            })
            .expect_err("runtime producer requests must refuse before invoking the host");
        if refusal.kind != RefusalKind::Unsupported {
            return Err(format!(
                "seed {seed}: runtime producer refusal changed: {refusal:?}"
            ));
        }
    }
    let is_identity_unchanged = library
        .identity_manifest()
        .map_err(|error| format!("probe final manifest: {error:?}"))?
        == original;
    if budget.proposer_attempts() != 1 || budget.teacher_attempts() != 1 {
        return Err(format!(
            "seed {seed}: attempted producer control was not fail-closed"
        ));
    }
    if calls.get() != 0 || !is_identity_unchanged || !library.records().is_empty() {
        return Err(format!(
            "seed {seed}: attempted producer control was not fail-closed"
        ));
    }
    Ok(serde_json::json!({
        "seed": seed,
        "proposer_attempts": budget.proposer_attempts(),
        "teacher_attempts": budget.teacher_attempts(),
        "proposer_calls": 0,
        "teacher_calls": calls.get(),
        "refusals": ["Unsupported", "Unsupported"],
        "identity_unchanged": is_identity_unchanged,
        "records": library.records().len(),
    }))
}

#[path = "targets/evaluator.rs"]
pub(crate) mod evaluator;
#[cfg(test)]
#[path = "targets/tests.rs"]
mod tests;
