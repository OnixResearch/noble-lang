use super::*;

pub(crate) fn ty_name(ty: &Ty) -> &'static str {
    match ty {
        Ty::I64 => "I64",
        Ty::Bool => "Bool",
        Ty::Unit
        | Ty::Text
        | Ty::Syntax
        | Ty::Contract
        | Ty::Evidence
        | Ty::Certified
        | Ty::Pair(..)
        | Ty::Sum(..)
        | Ty::List(_)
        | Ty::Program(..)
        | Ty::LiveRef(..)
        | Ty::Resource(_)
        | Ty::Nominal(..)
        | Ty::GenericNominal(..) => "unsupported",
    }
}

pub(crate) fn scalar_json(value: Scalar) -> Value {
    match value {
        Scalar::I64(value) => json!({"I64": value}),
        Scalar::Bool(value) => json!({"Bool": value}),
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use core::fmt::Write;
        write!(&mut out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}

fn library_identity(revision: &[u8], entries: &[Envelope]) -> String {
    let mut bytes = Vec::with_capacity(
        revision.len().saturating_add(
            entries
                .len()
                .saturating_mul(chunks::IDENTITY_BYTES.saturating_add(chunks::REVISION_BYTES)),
        ),
    );
    bytes.extend_from_slice(revision);
    for entry in entries {
        bytes.extend_from_slice(&entry.chunk_id);
        bytes.extend_from_slice(&entry.source_revision);
    }
    hex(blake3::hash(&bytes).as_bytes())
}

fn library_entry_record(entry: &Envelope) -> Value {
    json!({
        "chunk_id": hex(&entry.chunk_id),
        "source_revision": hex(&entry.source_revision),
        "stack_in": entry.interface.stack_in.iter().map(ty_name).collect::<Vec<_>>(),
        "stack_out": entry.interface.stack_out.iter().map(ty_name).collect::<Vec<_>>(),
        "effects": "pure",
        "dependency_ids": entry.dependencies.iter().map(|dependency| dependency.identity).collect::<Vec<_>>(),
        "recipe_node_count": entry.body.nodes.len(),
        "recipe_entry_count": entry.body.entry.len()
    })
}

fn library_record(family: usize) -> Value {
    let revision = family_revision(family);
    let entries = family_envelopes(family);
    json!({
        "family": FAMILY_NAMES[family],
        "revision": hex(&revision),
        "identity": library_identity(&revision, &entries),
        "identity_rule": "blake3(revision || insertion-ordered-(chunk-identity,source-revision)-pairs)",
        "entries": entries.iter().map(library_entry_record).collect::<Vec<_>>()
    })
}

fn task_fixture_record(case: &TaskCase) -> Value {
    json!({
        "inputs": case.inputs.iter().copied().map(scalar_json).collect::<Vec<_>>(),
        "expected": case.outputs.iter().copied().map(scalar_json).collect::<Vec<_>>()
    })
}

fn task_target_kind(task: &TaskSpec) -> String {
    match task.target {
        Target::Arithmetic(op) => format!("arithmetic:{}", op.spelling()),
        Target::Swap => "ordered-stack-swap".to_string(),
        Target::Boolean(mode) => format!("boolean-mode:{mode}"),
        _ => integer_target_kind(task),
    }
}

fn integer_target_kind(task: &TaskSpec) -> String {
    let Target::Integer(value) = task.target else {
        return String::from("unsupported-target");
    };
    if task.family == 3 {
        format!("nested-integer:{value}")
    } else {
        format!("equal-signature-integer:{value}")
    }
}

fn task_provenance(task: &TaskSpec) -> Value {
    json!({
        "generator": "tchunk-ranking-corpus/1",
        "family_index": task.family,
        "block": task.block,
        "within_block": task.within_block,
        "split": task.split()
    })
}

fn task_contract(task: &TaskSpec) -> Value {
    json!({
        "stack_in": task.stack_in.iter().map(ty_name).collect::<Vec<_>>(),
        "stack_out": task.stack_out.iter().map(ty_name).collect::<Vec<_>>(),
        "effects": "pure"
    })
}

fn task_record(task: &TaskSpec, library: &Value) -> Value {
    json!({
        "id": task.id(),
        "family": FAMILY_NAMES[task.family],
        "chunk_library_identity": library["identity"].clone(),
        "provenance": task_provenance(task),
        "contract": task_contract(task),
        "primitive_reference_source": String::from_utf8_lossy(&task.primitive_source),
        "independent_fixtures": task.cases.iter().map(task_fixture_record).collect::<Vec<_>>(),
        "target_kind": task_target_kind(task)
    })
}

pub(crate) fn corpus() -> Value {
    let libraries: Vec<Value> = (0..FAMILY_NAMES.len()).map(library_record).collect();
    let records: Vec<Value> = super::tasks::corpus()
        .iter()
        .map(|task| task_record(task, &libraries[task.family]))
        .collect();
    assert_eq!(libraries.len(), FAMILY_NAMES.len(), "every family needs a library");
    assert_eq!(records.len(), 480, "every frozen task needs a record");
    json!({
        "schema": "tchunk-ranking-corpus/1",
        "task_count": records.len(),
        "family_count": FAMILY_NAMES.len(),
        "tasks_per_family": 96,
        "provenance_blocks_per_family": 12,
        "tasks_per_block": 8,
        "split": {"train": 280, "validation": 80, "test": 120, "unit": "family-provenance-block"},
        "deduplicate_by": ["canonical-source-bytes", "normalized-recipe-bytes", "ordered-interface", "independent-expected-trace", "generator-provenance", "chunk-library-identity"],
        "expected_values_owner": "independent-consumer-fixture",
        "expected_values_are_features": false,
        "libraries": libraries,
        "tasks": records
    })
}

#[test]
pub(crate) fn frozen_corpus_has_the_registered_split_and_valid_libraries() -> Result<(), String> {
    let manifest = corpus();
    let frozen: Value = serde_json::from_str(FROZEN_CORPUS)
        .map_err(|error| format!("frozen corpus JSON is invalid: {error}"))?;
    assert_eq!(manifest, frozen, "frozen corpus manifest drifted");
    let tasks = manifest["tasks"]
        .as_array()
        .ok_or_else(|| String::from("manifest task array missing"))?;
    assert_eq!(tasks.len(), 480);
    assert_eq!(manifest["libraries"].as_array().map(Vec::len), Some(5));

    check_task_splits(tasks)?;
    let session = noble_contracts::source::Session::without_test_hosts();
    let libraries = check_registered_libraries(&session)?;
    for spec in super::tasks::corpus() {
        let library = libraries
            .get(spec.family)
            .ok_or_else(|| format!("reference library missing for {}", spec.id()))?;
        check_primitive_reference(&session, library, &spec)?;
        check_fixture_reference(&spec, &primitive_envelope(&spec))?;
    }
    Ok(())
}

fn check_task_splits(tasks: &[Value]) -> Result<(), String> {
    let mut split_counts = [0usize; 3];
    let mut family_counts = [0usize; 5];
    let mut seen_ids = std::collections::BTreeSet::new();
    for task in tasks {
        let id = task["id"].as_str().ok_or("task ID missing")?;
        assert!(seen_ids.insert(id.to_owned()), "duplicate task ID {id}");
        let family = task["provenance"]["family_index"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .ok_or("family index missing")?;
        let block = task["provenance"]["block"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .ok_or("provenance block missing")?;
        let split = match block {
            0..=6 => 0,
            7..=8 => 1,
            _ => 2,
        };
        family_counts[family] = family_counts[family].saturating_add(1);
        split_counts[split] = split_counts[split].saturating_add(1);
        assert_eq!(
            task["independent_fixtures"].as_array().map(Vec::len),
            Some(FIXTURE_CASES)
        );
    }
    assert_eq!(family_counts, [96; 5]);
    assert_eq!(split_counts, [280, 80, 120]);
    Ok(())
}

fn check_registered_libraries(
    session: &noble_contracts::source::Session,
) -> Result<Vec<Library>, String> {
    let mut libraries = Vec::with_capacity(FAMILY_NAMES.len());
    for family in 0..FAMILY_NAMES.len() {
        let library = family_library(session, family)?;
        assert_eq!(library.entries().len(), 3);
        assert!(library
            .entries()
            .iter()
            .all(|entry| entry.interface().effects.is_empty()));
        libraries.push(library);
    }
    Ok(libraries)
}

fn check_primitive_reference(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
) -> Result<(), String> {
    let prepared = session
        .prepare(
            &spec.primitive_source,
            &spec.stack_in,
            source_limits(request(spec, library).budget),
        )
        .map_err(|error| format!("primitive reference {} refused: {error:?}", spec.id()))?;
    assert_eq!(
        prepared.output(),
        spec.stack_out,
        "typed reference {}",
        spec.id()
    );
    Ok(())
}

fn check_fixture_reference(spec: &TaskSpec, envelope: &Envelope) -> Result<(), String> {
    for case in &spec.cases {
        let expected = oracle_target(spec.target, &case.inputs)?;
        assert_eq!(expected, case.outputs, "independent fixture {}", spec.id());
        let actual = independent_recipe_output(envelope, &case.inputs)?;
        assert_eq!(actual, case.outputs, "primitive recipe {}", spec.id());
    }
    Ok(())
}
