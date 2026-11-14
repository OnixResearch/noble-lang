use super::*;

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending each task row requires a growable String buffer, which cannot be represented as &mut str"
)]
fn task_json(output: &mut String, report: TaskReport<'_>) {
    let TaskReport {
        task,
        order,
        outcome,
        passed,
        cases,
        references,
    } = report;
    if !output.ends_with('{') && !output.ends_with('[') {
        output.push(',');
    }
    output.push_str("{\"task\":\"");
    output.push_str(task.name);
    output.push_str("\",\"selected_sources\":[\"");
    output.push_str(order[0].name());
    output.push_str("\",\"");
    output.push_str(order[1].name());
    output.push_str("\"],\"outcome\":\"");
    output.push_str(outcome);
    output.push_str("\",\"passed\":");
    output.push_str(&passed.to_string());
    output.push_str(",\"total\":");
    output.push_str(&references.len().to_string());
    output.push_str(",\"cases\":[");
    for (index, (inputs, expected)) in references.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        let case = cases.get(index);
        let actual = case
            .and_then(|(actual, _, _)| *actual)
            .map_or_else(|| String::from("null"), |value| value.to_string());
        let is_agreed = case.is_some_and(|(_, _, agrees)| *agrees);
        output.push_str(&format!(
            "{{\"inputs\":[{},{},{}],\"expected\":{},\"actual\":{},\"agrees\":{}}}",
            inputs[0], inputs[1], inputs[2], expected, actual, is_agreed,
        ));
    }
    output.push_str("]}");
}

pub(super) fn evaluate(input: SweepInput<'_>) -> Result<(String, Counts), String> {
    let SweepInput {
        namespace,
        arm,
        seed_index,
        library,
        sources,
        label,
        control,
    } = input;
    let mut output = format!(
        "{{\"sweep\":\"{label}\",\"generation\":{},\"records\":{},\"tasks\":[",
        library.current_generation(),
        library.records().len(),
    );
    let mut counts = Counts::default();
    let mut budget = QueryBudget::new();
    let context = ComposeContext {
        prefix: Vec::new(),
        suffix: Vec::new(),
        stack_in: vec![Ty::I64, Ty::I64, Ty::I64],
        stack_out: vec![Ty::I64],
        allowed_effects: EffSet::empty(),
        limits: super::limits(),
    };
    let empty_cases = Vec::new();
    for (task_index, task) in HELD_OUT_TASKS.iter().copied().enumerate() {
        if task_index != 0 {
            output.push(',');
        }
        let order = super::fixtures::target_order(task, arm, control);
        let query = Query {
            composition_context: Some((vec![Ty::I64, Ty::I64, Ty::I64], vec![Ty::I64])),
            exact_interface: None,
            dependency_root: None,
            status: None,
        };
        let started = Instant::now();
        let retrieved = library
            .retrieve(&query, &mut budget)
            .map_err(|error| format!("retrieve {}: {error:?}", task.name))?;
        counts.retrieval_ns = counts
            .retrieval_ns
            .checked_add(super::producer::elapsed_nanoseconds(started)?)
            .ok_or_else(|| String::from("measured retrieval time exceeds the metrics counter"))?;
        counts.retrieval_queries = counts.retrieval_queries.saturating_add(1);
        counts.retrieval_examined = counts
            .retrieval_examined
            .saturating_add(retrieved.examined_entries);
        counts.max_examined = counts.max_examined.max(retrieved.examined_entries);
        counts.heldout_tasks = counts.heldout_tasks.saturating_add(1);
        let (fixtures, references) = super::fixtures::heldout_cases(task, seed_index);
        let reference_count = u32::try_from(references.len())
            .map_err(|_| String::from("held-out fixture count exceeds the metrics domain"))?;
        counts.heldout_examples = counts.heldout_examples.saturating_add(reference_count);
        let pair = super::producer::paired_sources(library, sources, order);
        let result = if let Some(records) = pair.filter(|records| {
            records
                .iter()
                .all(|record| retrieved.candidates.contains(&record.procedure_id))
        }) {
            let composition = library
                .compose(
                    namespace,
                    &[records[0].procedure_id, records[1].procedure_id],
                    &context,
                )
                .map_err(|error| format!("compose {}: {error:?}", task.name))?;
            let fixture_set = FixtureSet {
                id: [0x51; chunks::IDENTITY_BYTES],
                cases: fixtures,
            };
            super::fixtures::compose_and_check(
                namespace,
                library,
                records,
                &composition.source,
                fixture_set,
            )?
        } else {
            let reason = if pair.is_none() {
                "source-pair-unavailable"
            } else {
                "retrieval-excluded-source"
            };
            (reason, 0, empty_cases.clone())
        };
        counts.heldout_examples_passed = counts.heldout_examples_passed.saturating_add(result.1);
        if result.1 == reference_count {
            counts.transfer_passed = counts.transfer_passed.saturating_add(1);
        }
        task_json(
            &mut output,
            TaskReport {
                task,
                order,
                outcome: result.0,
                passed: result.1,
                cases: &result.2,
                references: &references,
            },
        );
    }
    let (earlier_tasks, regression_failed) =
        super::fixtures::earlier_regression(namespace, library, sources)?;
    counts.earlier_tasks = earlier_tasks;
    counts.regression_failed = regression_failed;
    output.push_str(&format!(
        "],\"earlier_tasks\":{earlier_tasks},\"regression_failed\":{regression_failed}}}"
    ));
    Ok((output, counts))
}
