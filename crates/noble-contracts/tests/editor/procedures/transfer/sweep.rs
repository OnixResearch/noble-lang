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
    output.push_str("{\"task\":\"");
    output.push_str(task);
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

fn retrieve_step(
    library: &ProcedureLibrary,
    budget: &mut QueryBudget,
    counts: &mut Counts,
    before: Vec<Ty>,
    after: Vec<Ty>,
) -> Result<Vec<[u8; 32]>, String> {
    let query = Query {
        composition_context: Some((before, after)),
        exact_interface: None,
        dependency_root: None,
        status: None,
    };
    let started = Instant::now();
    let retrieved = library
        .retrieve(&query, budget)
        .map_err(|error| format!("retrieve source step: {error:?}"))?;
    counts.retrieval_ns = counts
        .retrieval_ns
        .checked_add(super::producer::elapsed_nanoseconds(started)?)
        .ok_or_else(|| String::from("measured retrieval time exceeds the metrics counter"))?;
    counts.retrieval_queries = counts.retrieval_queries.saturating_add(1);
    counts.retrieval_examined = counts
        .retrieval_examined
        .saturating_add(retrieved.examined_entries);
    counts.max_examined = counts.max_examined.max(retrieved.examined_entries);
    Ok(retrieved.candidates)
}

fn retrieve_source_steps(
    library: &ProcedureLibrary,
    budget: &mut QueryBudget,
    counts: &mut Counts,
) -> Result<[Vec<[u8; 32]>; 2], String> {
    // Each 2→1 source consumes one step of the 3→2→1 composition; a 3→1
    // whole-target context cannot match either single source.
    let first = retrieve_step(
        library,
        budget,
        counts,
        vec![Ty::I64, Ty::I64, Ty::I64],
        vec![Ty::I64, Ty::I64],
    )?;
    let second = retrieve_step(
        library,
        budget,
        counts,
        vec![Ty::I64, Ty::I64],
        vec![Ty::I64],
    )?;
    Ok([first, second])
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
        let retrieved_steps = retrieve_source_steps(library, &mut budget, &mut counts)?;
        counts.heldout_tasks = counts.heldout_tasks.saturating_add(1);
        let (fixtures, references) = super::fixtures::heldout_cases(task, seed_index);
        let reference_count = u32::try_from(references.len())
            .map_err(|_| String::from("held-out fixture count exceeds the metrics domain"))?;
        counts.heldout_examples = counts.heldout_examples.saturating_add(reference_count);
        let pair = super::producer::paired_sources(library, sources, order);
        let result = if let Some(records) = pair.filter(|records| {
            retrieved_steps
                .iter()
                .zip(records.iter())
                .all(|(candidates, record)| candidates.contains(&record.procedure_id))
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
                task: task.name,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonheldout_source_steps_retrieve_each_admitted_source() -> Result<(), String> {
        let namespace = Session::without_test_hosts();
        let mut library = ProcedureLibrary::new(super::super::revision(7));
        let mut ids = Vec::new();
        for task in [SourceTask::Scaled, SourceTask::Product] {
            let record = super::super::producer::task_record(task, Producer::Synthesis)?;
            ids.push(
                library
                    .admit(&namespace, None, record)
                    .map_err(|error| format!("non-held-out source admission: {error:?}"))?,
            );
        }
        let mut budget = QueryBudget::new();
        let mut counts = Counts::default();
        let steps = retrieve_source_steps(&library, &mut budget, &mut counts)?;
        assert_eq!(budget.queries(), 2);
        assert_eq!(counts.retrieval_queries, 2);
        assert_eq!(counts.retrieval_examined, 4);
        for candidates in steps {
            assert_eq!(candidates.len(), 2);
            for id in &ids {
                assert!(candidates.contains(id));
            }
        }
        Ok(())
    }

    #[test]
    fn nonheldout_task_rows_have_one_separator_and_parse() -> Result<(), String> {
        let references = [([113, 127, 131], 23)];
        let cases = [(Some(23), 23, true)];
        let mut rows = String::from("[");
        for (index, name) in ["nonheldout_source_a", "nonheldout_source_b"]
            .iter()
            .enumerate()
        {
            if index != 0 {
                rows.push(',');
            }
            task_json(
                &mut rows,
                TaskReport {
                    task: name,
                    order: [SourceTask::Scaled, SourceTask::Product],
                    outcome: "fixture-only",
                    passed: 1,
                    cases: &cases,
                    references: &references,
                },
            );
        }
        rows.push(']');
        let parsed: serde_json::Value =
            serde_json::from_str(&rows).map_err(|error| format!("task rows JSON: {error}"))?;
        let tasks = parsed
            .as_array()
            .ok_or_else(|| String::from("task rows are not a JSON array"))?;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0]["task"].as_str(), Some("nonheldout_source_a"));
        assert_eq!(tasks[1]["task"].as_str(), Some("nonheldout_source_b"));
        Ok(())
    }
}
