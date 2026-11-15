use super::*;

pub(super) fn case_15() -> TestResult {
    let scratch = Scratch::new("pe-case15")?;
    let source = scratch.write("active-pair.noble", "1 +")?;
    let active_shape = "Pair<I64,Pair<Unit,I64>> I64";
    assert_eq!(case_row(15)?["input"]["active_shape"], active_shape);
    let active = format!("{active_shape} --");
    let tails = [
        ("Text", "Text", "\"audit\""),
        ("Bool", "Bool", "true"),
        ("Unit", "Unit", "unit"),
        ("Pair", "Pair<I64,Bool>", "7 false pair"),
    ];
    let maps = [
        ("none", "", "2 3 4"),
        ("all", "1=2,2=3,3=4", ""),
        ("mixed", "1=2,3=4", "3"),
        ("non-prefix", "2=3", "2 4"),
    ];
    let mut runs = Vec::new();
    let mut observed_tail_types = Vec::new();
    let mut observed_static_maps = std::collections::BTreeSet::new();
    for (tail_name, tail_type, tail_value) in tails {
        observed_tail_types.push(tail_name);
        let interface = format!("{tail_type} {active}");
        let initial = format!("{tail_value} 2 unit 3 pair pair 4");
        let original = program_session(false, &[&initial, "1 +"])?;
        let expected = observed_stack(&original[1])?;
        assert_eq!(expected.len(), 3, "tail, nested pair and scalar output");
        for (map_name, static_map, dynamic) in maps {
            observed_static_maps.insert(map_name);
            let name = format!("{tail_name}/{map_name}");
            let output = scratch.path(&format!("{tail_name}-{map_name}.noble"));
            let command = specialize_program_source(&source, &interface, "1", static_map, &output);
            let run = scratch.run(&command)?;
            let report = expect_normal(&run, "specialize")?;
            assert_eq!(report["tail"], 1, "{name}: {report}");
            assert_eq!(report["original_closure"], serde_json::json!([]));
            assert_eq!(report["residual_closure"], serde_json::json!([]));
            assert_published(&output);
            let emitted = std::fs::read_to_string(&output)?;
            assert!(
                emitted.contains("pair\n"),
                "nested output must be rebuilt: {emitted}"
            );
            assert!(
                emitted.contains("unit\n"),
                "Unit inside pair must be rebuilt: {emitted}"
            );
            let residual_initial = if dynamic.is_empty() {
                tail_value.to_owned()
            } else {
                format!("{tail_value} {dynamic}")
            };
            let residual = program_session(true, &[&residual_initial, &emitted])?;
            assert_eq!(observed_stack(&residual[1])?, expected, "{name}: {report}");
            assert_eq!(residual[1]["guest_requests"], 0);
            runs.push(cli_observation(&name, &run, &output)?);
        }
    }
    let listed_maps: Vec<_> = maps.iter().map(|(name, _, _)| *name).collect();
    let listed_types: Vec<_> = tails.iter().map(|(name, _, _)| *name).collect();
    assert_eq!(observed_tail_types, listed_types);
    assert_eq!(observed_static_maps.len(), listed_maps.len());
    assert!(listed_maps
        .iter()
        .all(|name| observed_static_maps.contains(name)));
    case_observation(
        15,
        runs,
        serde_json::json!({
            "tested_tail_types":observed_tail_types,
            "tested_static_maps":listed_maps,
            "ordered_typed_stacks_agree":true,
            "opaque_tail_positions_unchanged":true,
            "ordinary_paths_selectable":true,
        }),
    )
}

pub(super) fn case_16() -> TestResult {
    let scratch = Scratch::new("pe-case16")?;
    let subjects = [
        (
            "checked-root-with-reached-definitions",
            "def twice [ dup + ]\ntwice",
            1usize,
            &["def twice [ dup + ]", "\"audit\" 5", "twice"][..],
            "10",
        ),
        (
            "saved-old-definition-after-rebinding",
            "def step [ 1 + ]\ndef use [ step ]\ndef step [ 9 + ]\nuse",
            2usize,
            &[
                "def step [ 1 + ]",
                "def use [ step ]",
                "def step [ 9 + ]",
                "\"audit\" 5",
                "use",
            ][..],
            "6",
        ),
    ];
    let mut runs = Vec::new();
    let mut old_id = None;
    for (name, body, closure_len, original_sources, expected_scalar) in subjects {
        let source = scratch.write(&format!("{name}.noble"), body)?;
        let output = scratch.path(&format!("{name}.residual"));
        let command = specialize_program_source(&source, "Text I64 --", "1", "1=5", &output);
        let run = scratch.run(&command)?;
        let report = expect_normal(&run, "specialize")?;
        assert_eq!(
            report["original_closure"]
                .as_array()
                .ok_or("original closure")?
                .len(),
            closure_len
        );
        assert_eq!(report["residual_closure"], serde_json::json!([]));
        assert_ne!(report["original_subject"], report["residual_subject"]);
        let original = program_session(false, original_sources)?;
        let expected = observed_stack(original.last().ok_or("ordinary source report")?)?;
        assert_eq!(expected[0].1, "audit");
        assert_eq!(expected[1].1, expected_scalar);
        let emitted = std::fs::read_to_string(&output)?;
        let residual = program_session(true, &["\"audit\"", &emitted])?;
        assert_eq!(observed_stack(&residual[1])?, expected, "{name}: {report}");
        let pinned = scratch.path(&format!("{name}.pinned"));
        let verified = scratch.run(
            &specialize_program_source(&source, "Text I64 --", "1", "1=5", &pinned)
                .arg("--subject-id")
                .arg(report["original_subject"].as_str().ok_or("original id")?),
        )?;
        let verified_report = expect_normal(&verified, "specialize")?;
        assert_eq!(
            verified_report["residual_subject"],
            report["residual_subject"]
        );
        if name == "saved-old-definition-after-rebinding" {
            old_id = Some(
                report["original_subject"]
                    .as_str()
                    .ok_or("original id")?
                    .to_owned(),
            );
        }
        runs.push(cli_observation(name, &run, &output)?);
    }
    let changed = scratch.write("changed.noble", "def step [ 2 + ]\ndef use [ step ]\nuse")?;
    let stale_out = scratch.path("changed.residual");
    let stale = scratch.run(
        &specialize_program_source(&changed, "Text I64 --", "1", "1=5", &stale_out)
            .arg("--subject-id")
            .arg(old_id.as_deref().ok_or("retained original id")?),
    )?;
    expect_code(&stale, "specialize", "E-SUBJECT")?;
    assert_no_publication(&stale_out);
    runs.push(cli_observation(
        "stale-changed-definition",
        &stale,
        &stale_out,
    )?);
    case_observation(
        16,
        runs,
        serde_json::json!({
            "exact_original_closure":true, "empty_residual_closure":true,
            "fresh_identity":true, "changed_definition_or_stale_pin_publishes":false,
        }),
    )
}
