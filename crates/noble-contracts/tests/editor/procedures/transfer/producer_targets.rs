use super::*;
use noble_contracts::source::procedures::{RefusalKind, RuntimeProducer};
use std::cell::Cell;

const CRITERIA: &str = "verification/procedure-library/criteria-producer-targets-v2.json";
const TARGET_RECEIPT: &str = "verification/procedure-library/receipts/producer-targets-v2.json";
const TARGET_NAMES: [&str; 2] = ["arithComposeShifted", "arithComposeOffsetDifference"];
const TARGET_REFERENCES: [&str; 2] = ["(+ (- (+ x y) z) (+ 7 7))", "(+ (- x y) (+ z (+ 7 7)))"];
const TARGET_SOURCE_ORDERS: [[&str; 2]; 2] = [
    ["arithCommuted", "arithSubtract"],
    ["arithSubtract", "arithCommuted"],
];

pub(super) struct ProducerTargets {
    pub(super) set: TargetSet,
    pub(super) summary: serde_json::Value,
}

fn read_json(relative: &str) -> Result<serde_json::Value, String> {
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
    name: &str,
    reference: &str,
) -> Result<(String, serde_json::Value), String> {
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
            || evaluate_program(program, inputs)? != expected
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
    if criteria["schema"] != "noble-procedure-library-producer-targets-criteria/2" {
        return Err(String::from("producer target criterion schema drifted"));
    }
    for (index, name) in TARGET_NAMES.iter().copied().enumerate() {
        if criteria["producer"]["target_tasks"][index]["name"] != name
            || criteria["producer"]["target_tasks"][index]["reference"] != TARGET_REFERENCES[index]
            || criteria["producer"]["target_tasks"][index]["source_order"]
                != serde_json::json!(TARGET_SOURCE_ORDERS[index])
        {
            return Err(format!("{name}: frozen criterion task identity drifted"));
        }
    }
    if receipt["schema"] != "noble-synthesis-receipt/v1"
        || receipt["source"]["manifest"] != "verification/procedure-library/producer-targets-v2.txt"
        || receipt["source"]["clean"] != true
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
    let (first, first_summary) = target_program(&tasks[0], TARGET_NAMES[0], TARGET_REFERENCES[0])?;
    let (second, second_summary) =
        target_program(&tasks[1], TARGET_NAMES[1], TARGET_REFERENCES[1])?;
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
    };
    Ok(ProducerTargets {
        set,
        summary: serde_json::json!([first_summary, second_summary]),
    })
}

pub(super) fn evaluate_program(program: &str, inputs: [i64; 3]) -> Result<i64, String> {
    let words = program.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() || words.len() > 8 {
        return Err(String::from(
            "producer target exceeds the declared word bound",
        ));
    }
    let mut stack = Vec::with_capacity(4);
    stack.extend_from_slice(&inputs);
    for word in words {
        match word {
            "dup" => {
                let top = *stack.last().ok_or_else(|| String::from("dup underflow"))?;
                stack.push(top);
            }
            "swap" => {
                let length = stack.len();
                if length < 2 {
                    return Err(String::from("swap underflow"));
                }
                stack.swap(length - 2, length - 1);
            }
            "+" | "-" | "*" => {
                let right = stack
                    .pop()
                    .ok_or_else(|| String::from("operator underflow"))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| String::from("operator underflow"))?;
                let result = match word {
                    "+" => left.wrapping_add(right),
                    "-" => left.wrapping_sub(right),
                    "*" => left.wrapping_mul(right),
                    _ => return Err(String::from("unrecognized producer arithmetic word")),
                };
                stack.push(result);
            }
            "=" => return Err(String::from("Boolean target is outside the I64 criterion")),
            literal => {
                let value = literal
                    .parse::<i64>()
                    .map_err(|error| format!("unknown producer word {literal}: {error}"))?;
                if ![0, 1, 2, 3, 5, 7].contains(&value) {
                    return Err(format!(
                        "producer constant {literal} is outside the grammar"
                    ));
                }
                stack.push(value);
            }
        }
        if stack.len() > 4 {
            return Err(String::from("producer target exceeds max-stack 4"));
        }
    }
    match stack.as_slice() {
        [result] => Ok(*result),
        _ => Err(String::from(
            "producer target does not leave exactly one I64",
        )),
    }
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
    let identity_unchanged = library
        .identity_manifest()
        .map_err(|error| format!("probe final manifest: {error:?}"))?
        == original;
    if budget.proposer_attempts() != 1
        || budget.teacher_attempts() != 1
        || calls.get() != 0
        || !identity_unchanged
        || !library.records().is_empty()
    {
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
        "identity_unchanged": identity_unchanged,
        "records": library.records().len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonheldout_three_input_stack_evaluator_has_refusals() -> Result<(), String> {
        assert_eq!(evaluate_program("+ -", [5, -5, 7])?, 3);
        assert_eq!(evaluate_program("- 7 + 7 + +", [5, -5, 7])?, 7);
        assert!(evaluate_program("42 + +", [0, 0, 0]).is_err());
        assert!(evaluate_program("dup dup", [1, 2, 3]).is_err());
        assert!(evaluate_program("= +", [1, 2, 3]).is_err());
        Ok(())
    }
}
