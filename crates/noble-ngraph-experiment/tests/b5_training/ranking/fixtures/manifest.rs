use super::*;

pub(crate) fn ty_name(ty: &Ty) -> &'static str {
    match ty {
        Ty::I64 => "I64",
        Ty::Bool => "Bool",
        _ => "unsupported",
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
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

pub(crate) fn corpus_manifest() -> Value {
    let tasks = corpus_tasks();
    let libraries: Vec<Value> = (0..FAMILY_NAMES.len())
        .map(|family| {
            let revision = family_revision(family);
            let entries = family_envelopes(family);
            let mut identity_bytes = Vec::with_capacity(
                revision
                    .len()
                    .saturating_add(entries.len().saturating_mul(
                        chunks::IDENTITY_BYTES.saturating_add(chunks::REVISION_BYTES),
                    )),
            );
            identity_bytes.extend_from_slice(&revision);
            for entry in &entries {
                identity_bytes.extend_from_slice(&entry.chunk_id);
                identity_bytes.extend_from_slice(&entry.source_revision);
            }
            let identity = blake3::hash(&identity_bytes);
            json!({
                "family": FAMILY_NAMES[family],
                "revision": hex(&revision),
                "identity": hex(identity.as_bytes()),
                "identity_rule": "blake3(revision || insertion-ordered-(chunk-identity,source-revision)-pairs)",
                "entries": entries.iter().map(|entry| json!({
                    "chunk_id": hex(&entry.chunk_id),
                    "source_revision": hex(&entry.source_revision),
                    "stack_in": entry.interface.stack_in.iter().map(ty_name).collect::<Vec<_>>(),
                    "stack_out": entry.interface.stack_out.iter().map(ty_name).collect::<Vec<_>>(),
                    "effects": "pure",
                    "dependency_ids": entry.dependencies.iter().map(|dependency| dependency.identity).collect::<Vec<_>>(),
                    "recipe_node_count": entry.body.nodes.len(),
                    "recipe_entry_count": entry.body.entry.len()
                })).collect::<Vec<_>>()
            })
        })
        .collect();
    let records: Vec<Value> = tasks
        .iter()
        .map(|task| {
            json!({
                "id": task.id(),
                "family": FAMILY_NAMES[task.family],
                "chunk_library_identity": libraries[task.family]["identity"].clone(),
                "provenance": {
                    "generator": "tchunk-ranking-corpus/1",
                    "family_index": task.family,
                    "block": task.block,
                    "within_block": task.within_block,
                    "split": task.split()
                },
                "contract": {
                    "stack_in": task.stack_in.iter().map(ty_name).collect::<Vec<_>>(),
                    "stack_out": task.stack_out.iter().map(ty_name).collect::<Vec<_>>(),
                    "effects": "pure"
                },
                "primitive_reference_source": String::from_utf8_lossy(&task.primitive_source),
                "independent_fixtures": task.cases.iter().map(|case| json!({
                    "inputs": case.inputs.iter().copied().map(scalar_json).collect::<Vec<_>>(),
                    "expected": case.outputs.iter().copied().map(scalar_json).collect::<Vec<_>>()
                })).collect::<Vec<_>>(),
                "target_kind": match task.target {
                    Target::Arithmetic(op) => format!("arithmetic:{}", op.spelling()),
                    Target::Swap => "ordered-stack-swap".to_string(),
                    Target::Boolean(mode) => format!("boolean-mode:{mode}"),
                    Target::Integer(value) if task.family == 3 => format!("nested-integer:{value}"),
                    Target::Integer(value) => format!("equal-signature-integer:{value}"),
                }
            })
        })
        .collect();
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
    let manifest = corpus_manifest();
    let frozen: Value = serde_json::from_str(FROZEN_CORPUS)
        .map_err(|error| format!("frozen corpus JSON is invalid: {error}"))?;
    assert_eq!(manifest, frozen, "frozen corpus manifest drifted");
    let tasks = manifest["tasks"]
        .as_array()
        .ok_or_else(|| String::from("manifest task array missing"))?;
    assert_eq!(tasks.len(), 480);
    assert_eq!(manifest["libraries"].as_array().map(Vec::len), Some(5));

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

    let session = noble_contracts::source::Session::without_test_hosts();
    let mut libraries = Vec::with_capacity(FAMILY_NAMES.len());
    for family in 0..FAMILY_NAMES.len() {
        let library = family_library(&session, family)?;
        assert_eq!(library.entries().len(), 3);
        assert!(library
            .entries()
            .iter()
            .all(|entry| entry.interface().effects.is_empty()));
        libraries.push(library);
    }
    for spec in corpus_tasks() {
        let library = libraries
            .get(spec.family)
            .ok_or_else(|| format!("reference library missing for {}", spec.id()))?;
        let prepared = session
            .prepare(
                &spec.primitive_source,
                &spec.stack_in,
                source_limits(request(&spec, library).budget),
            )
            .map_err(|error| format!("primitive reference {} refused: {error:?}", spec.id()))?;
        assert_eq!(
            prepared.output(),
            spec.stack_out,
            "typed reference {}",
            spec.id()
        );
        let envelope = primitive_envelope(&spec);
        for case in &spec.cases {
            let expected = oracle_target(spec.target, &case.inputs)?;
            assert_eq!(expected, case.outputs, "independent fixture {}", spec.id());
            let actual = independent_recipe_output(&envelope, &case.inputs)?;
            assert_eq!(actual, case.outputs, "primitive recipe {}", spec.id());
        }
    }
    Ok(())
}
