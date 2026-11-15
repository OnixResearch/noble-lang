use super::*;

fn definition(record: &ProcedureRecord) -> DefinitionRef {
    let mut identity = [0; 8];
    identity.copy_from_slice(&record.chunk.chunk_id[..8]);
    DefinitionRef {
        identity: u64::from_be_bytes(identity),
        format: record.chunk.format,
        semantic_revision: record.chunk.semantic_revision,
        source_revision: record.chunk.source_revision,
    }
}

pub(super) fn target_order(
    task: HeldOutTask,
    arm: &str,
    control: &ControlSelection,
) -> [SourceTask; 2] {
    if arm == "size_matched_nontransfer" {
        return match task.kind {
            TargetKind::CommutedThenSubtract => control.tasks,
            TargetKind::SubtractThenCommuted => [control.tasks[1], control.tasks[0]],
        };
    }
    match task.kind {
        TargetKind::CommutedThenSubtract => [SourceTask::Commuted, SourceTask::Subtract],
        TargetKind::SubtractThenCommuted => [SourceTask::Subtract, SourceTask::Commuted],
    }
}

pub(super) fn heldout_cases(
    task: HeldOutTask,
    task_index: usize,
    seed_index: usize,
    target_set: &TargetSet,
) -> Result<(Vec<Fixture>, Vec<([i64; 3], i64)>), String> {
    let inputs = target_set.inputs[seed_index];
    let mut fixtures = Vec::with_capacity(inputs.len());
    let mut references = Vec::with_capacity(inputs.len());
    for values in inputs {
        let formula = match task.kind {
            TargetKind::CommutedThenSubtract => {
                values[0].wrapping_sub(values[1]).wrapping_sub(values[2])
            }
            TargetKind::SubtractThenCommuted => values[0]
                .wrapping_add(values[1])
                .wrapping_sub(values[2])
                .wrapping_add(14),
        };
        let expected = if let Some(program) = &target_set.programs[task_index] {
            let generated = super::producer_targets::evaluate_program(program, values)?;
            if generated != formula {
                return Err(format!(
                    "{} generated candidate disagrees with declared reference at {values:?}",
                    task.name
                ));
            }
            generated
        } else {
            formula
        };
        fixtures.push(Fixture {
            inputs: values.iter().copied().map(Scalar::I64).collect(),
            outputs: vec![Scalar::I64(expected)],
        });
        references.push((values, expected));
    }
    Ok((fixtures, references))
}

pub(super) fn request(library: &chunks::Library, stack_in: &[Ty], fixtures: FixtureSet) -> Request {
    Request {
        format: chunks::ENVELOPE_FORMAT,
        semantic_revision: chunks::SEMANTIC_REVISION,
        profile: String::from(chunks::PROFILE),
        task: Task {
            stack_in: stack_in.to_vec(),
            stack_out: vec![Ty::I64],
            allowed_effects: EffSet::empty(),
        },
        editor: Candidate {
            format: noble_contracts::source::editor::FORMAT,
            nodes: vec![EditorNode::Hole],
        },
        hole_path: vec![0],
        library_revision: *library.revision(),
        fixtures: Some(fixtures),
        budget: Budget::PROFILE,
    }
}

pub(super) fn fixture_report(
    namespace: &Session,
    library: &chunks::Library,
    request: &Request,
) -> FixtureOutcome {
    match chunks::complete(namespace, library, request, || 0) {
        Response::Accepted { report, .. } => {
            let Some(fixtures) = report.fixtures.as_ref() else {
                return ("accepted-without-fixtures", 0, Vec::new());
            };
            let details = fixtures
                .cases
                .iter()
                .map(|case| {
                    let expected = case
                        .expected
                        .first()
                        .and_then(|value| match value {
                            Scalar::I64(value) => Some(*value),
                            Scalar::Bool(_) => None,
                        })
                        .unwrap_or(0);
                    let actual = case
                        .actual
                        .as_ref()
                        .and_then(|values| values.first())
                        .and_then(|value| match value {
                            Scalar::I64(value) => Some(*value),
                            Scalar::Bool(_) => None,
                        });
                    (actual, expected, case.agrees)
                })
                .collect::<Vec<_>>();
            let passed = details
                .iter()
                .filter(|(_, _, agrees)| *agrees)
                .fold(0u32, |count, _| count.saturating_add(1));
            ("accepted", passed, details)
        }
        response => (response.outcome(), 0, Vec::new()),
    }
}

pub(super) fn compose_and_check(
    namespace: &Session,
    procedure_library: &ProcedureLibrary,
    selected: [&ProcedureRecord; 2],
    source: &[u8],
    fixtures: FixtureSet,
) -> Result<FixtureOutcome, String> {
    let mut checker = chunks::Library::new(*procedure_library.chunk_library_revision());
    for record in selected {
        checker
            .admit(namespace, record.chunk.clone())
            .map_err(|error| format!("re-admit dependency: {error:?}"))?;
    }
    let mut composite = super::envelope(
        &[Ty::I64, Ty::I64, Ty::I64],
        &[Ty::I64],
        source,
        vec![
            Node::Invoke {
                dependency: 0,
                instantiation: 0,
            },
            Node::Invoke {
                dependency: 1,
                instantiation: 0,
            },
        ],
        vec![0, 1],
    );
    composite.dependencies = selected.iter().map(|record| definition(record)).collect();
    composite.chunk_id = chunks::chunk_identity_of(&composite);
    checker
        .admit(namespace, composite)
        .map_err(|error| format!("re-admit composition candidate: {error:?}"))?;
    let emitted = checker
        .entries()
        .last()
        .ok_or("composition chunk was not retained by typed-chunk checker")?;
    if emitted.source() != source {
        return Err(String::from(
            "typed-chunk emitted source differs from fresh procedure composition",
        ));
    }
    let request = request(&checker, &[Ty::I64, Ty::I64, Ty::I64], fixtures);
    Ok(fixture_report(namespace, &checker, &request))
}

pub(super) fn earlier_regression(
    namespace: &Session,
    library: &ProcedureLibrary,
    sources: &[RetainedSource],
) -> Result<(u32, u32), String> {
    let mut checked = 0u32;
    let mut failed = 0u32;
    for source in sources {
        let Some(record) = library
            .records()
            .iter()
            .find(|record| record.procedure_id == source.id)
        else {
            continue;
        };
        let mut checker = chunks::Library::new(*library.chunk_library_revision());
        checker
            .admit(namespace, record.chunk.clone())
            .map_err(|error| format!("regression re-admit {}: {error:?}", source.task.name()))?;
        let cases = super::producer::source_reference_cases(source.task)?;
        let fixture_set = FixtureSet {
            id: [0x61; chunks::IDENTITY_BYTES],
            cases: cases
                .iter()
                .map(|(inputs, expected)| Fixture {
                    inputs: inputs.iter().copied().map(Scalar::I64).collect(),
                    outputs: vec![Scalar::I64(*expected)],
                })
                .collect(),
        };
        let request = request(&checker, &[Ty::I64, Ty::I64], fixture_set);
        let (_, passed, details) = fixture_report(namespace, &checker, &request);
        checked = checked.saturating_add(1);
        let detail_count = u32::try_from(details.len())
            .map_err(|_| String::from("regression fixture count exceeds the metrics domain"))?;
        if passed != detail_count || details.is_empty() {
            failed = failed.saturating_add(1);
        }
    }
    Ok((checked, failed))
}
