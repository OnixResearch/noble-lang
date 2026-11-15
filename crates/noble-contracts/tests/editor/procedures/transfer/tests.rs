use super::*;

#[test]
#[ignore = "one-shot held-out transfer measurement; run only from the frozen, clean source commit"]
fn frozen_heldout_measurement_once() -> Result<(), String> {
    let selection_namespace = Session::without_test_hosts();
    let control = super::selection::choose_control_pair(&selection_namespace)?;
    let mut rows = Vec::with_capacity(TRANSFER_ROW_CAPACITY);
    let mut is_learned_passed = true;
    let mut has_runtime_agent_calls = false;
    for (seed_index, seed) in SEEDS.iter().copied().enumerate() {
        for arm in TRANSFER_ARMS {
            let (row, metrics, is_control_size_matched) =
                super::runner::run_arm_seed(arm, seed_index, &control, &FROZEN_TARGET_SET)?;
            has_runtime_agent_calls |= metrics.proposer_calls != 0 || metrics.teacher_calls != 0;
            if arm == "accumulated_learned_library" {
                is_learned_passed &= metrics.transfer_passed == 2;
            }
            if arm == "size_matched_nontransfer" && !is_control_size_matched {
                eprintln!("size-matched control unavailable at seed {seed}: selected pair exceeds the preregistered 10% byte bound");
            }
            rows.push(row);
        }
    }
    if has_runtime_agent_calls {
        return Err(String::from(
            "proposer or teacher calls occurred in a frozen transfer arm",
        ));
    }
    let mut output = String::from("{\"schema\":\"noble-procedure-library-transfer-run/1\",\"criteria\":\"verification/procedure-library/criteria-2026-11-13.json\",\"seeds\":[1,7,23,101],\"producer_receipt\":\"verification/synthesis/receipts/2026-11-13/synthesis-v2.json\",\"control_pool_selection_before_measurement\":{\"eligible_pair\":[\"");
    output.push_str(control.tasks[0].name());
    output.push_str("\",\"");
    output.push_str(control.tasks[1].name());
    output.push_str(&format!(
        "\"],\"target_bytes\":{},\"candidate_bytes\":{},\"within_ten_percent\":{}}},\"rows\":[",
        control.target_bytes, control.candidate_bytes, control.within_ten_percent,
    ));
    for (index, row) in rows.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(row);
    }
    output.push_str("]}");
    println!("PROCEDURE_TRANSFER_JSON={output}");
    if !is_learned_passed {
        return Err(String::from(
            "accumulated learned library did not meet the frozen 8/8 task-cell transfer threshold",
        ));
    }
    Ok(())
}

#[test]
#[ignore = "second one-shot producer-generated target measurement; run only from a clean frozen source commit"]
fn producer_generated_heldout_measurement_once() -> Result<(), String> {
    let selection_namespace = Session::without_test_hosts();
    let control = super::selection::choose_control_pair(&selection_namespace)?;
    let producer = super::producer_targets::load()?;
    let mut rows = Vec::with_capacity(TRANSFER_ROW_CAPACITY);
    let mut probes = Vec::with_capacity(SEEDS.len());
    let mut is_go = control.within_ten_percent;
    for (seed_index, seed) in SEEDS.iter().copied().enumerate() {
        for arm in TRANSFER_ARMS {
            let (row, metrics, is_matched) =
                super::runner::run_arm_seed(arm, seed_index, &control, &producer.set)?;
            is_go &= metrics.proposer_calls == 0
                && metrics.teacher_calls == 0
                && metrics.regression_failed == 0
                && metrics.restart_identity_match;
            if arm == "accumulated_learned_library" {
                is_go &= metrics.transfer_passed == 2;
            }
            if arm == "size_matched_nontransfer" {
                is_go &= is_matched;
            }
            rows.push(
                serde_json::from_str::<serde_json::Value>(&row)
                    .map_err(|error| format!("decode arm row: {error}"))?,
            );
        }
        probes.push(super::producer_targets::refusal_probe(seed)?);
    }
    let report = serde_json::json!({
        "schema": "noble-procedure-library-producer-transfer-run/1",
        "criteria": "verification/procedure-library/criteria-producer-targets-v2.json",
        "producer_receipt": "verification/procedure-library/receipts/producer-targets-v2.json",
        "target_producer": producer.summary,
        "paired_seeds": SEEDS,
        "control_pair": [control.tasks[0].name(), control.tasks[1].name()],
        "control_size_match": control.within_ten_percent,
        "rows": rows,
        "runtime_refusal_controls": probes,
        "decision": if is_go { "GO" } else { "NO-GO" },
    });
    println!("PRODUCER_TRANSFER_JSON={report}");
    if !is_go {
        return Err(String::from(
            "NO-GO: producer-target transfer missed its frozen success rule",
        ));
    }
    Ok(())
}

#[test]
fn producer_task_candidates_are_source_readmitted_and_fixture_bound() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    for task in SOURCE_TASKS {
        let record = super::producer::task_record(task, Producer::Synthesis)?;
        let mut library = chunks::Library::new(super::revision(7));
        library
            .admit(&namespace, record.chunk.clone())
            .map_err(|error| format!("{} typed-chunk admission: {error:?}", task.name()))?;
        let fixtures = FixtureSet {
            id: [0x71; chunks::IDENTITY_BYTES],
            cases: super::producer::source_reference_cases(task)?
                .iter()
                .map(|(inputs, expected)| Fixture {
                    inputs: inputs.iter().copied().map(Scalar::I64).collect(),
                    outputs: vec![Scalar::I64(*expected)],
                })
                .collect(),
        };
        let task_request = super::fixtures::request(&library, &[Ty::I64, Ty::I64], fixtures);
        let (_, passed, cases) =
            super::fixtures::fixture_report(&namespace, &library, &task_request);
        let passed_count = usize::try_from(passed)
            .map_err(|_| String::from("source task case count does not fit host index"))?;
        assert_eq!(
            passed_count,
            cases.len(),
            "{} source task regression",
            task.name()
        );
        assert!(
            !cases.is_empty(),
            "{} source task fixtures did not execute",
            task.name()
        );
    }
    Ok(())
}

#[test]
fn acquisition_references_refuse_a_mutated_producer_receipt() -> Result<(), String> {
    let mut receipt = super::producer::synthesis_receipt()?.clone();
    let task_index = receipt["tasks"]
        .as_array()
        .and_then(|tasks| {
            tasks
                .iter()
                .position(|item| item["name"] == SourceTask::Commuted.name())
        })
        .ok_or_else(|| String::from("producer source task missing"))?;
    receipt["tasks"][task_index]["heldout"]["entries"][0]["expected"] =
        serde_json::Value::String(String::from("8"));
    assert!(super::producer::source_reference_cases_in(&receipt, SourceTask::Commuted).is_err());
    receipt["tasks"][task_index]["heldout"]["entries"][0]["expected"] =
        serde_json::Value::String(String::from("7"));
    receipt["tasks"][task_index]["search"]["program"] =
        serde_json::Value::String(String::from("7 - +"));
    assert!(super::producer::source_reference_cases_in(&receipt, SourceTask::Commuted).is_err());
    Ok(())
}
