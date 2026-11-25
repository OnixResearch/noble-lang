struct WrappingCase {
    idx: usize,
    filename: &'static str,
    source: &'static str,
    expected: &'static str,
}

fn assert_wrapping_case(opt: super::harness::OptMode, case: &WrappingCase) -> Result<(), String> {
    let opt_flag = opt.as_flag();
    let temp = super::harness::TempDir::new(&format!("np01-{opt_flag}-{}", case.idx))?;
    let emit_v8 = temp.path().join("emit-v8");
    let emit_wt = temp.path().join("emit-wt");
    let emit_v8_str = emit_v8.to_str().ok_or("non-utf8 emit path")?;
    let emit_wt_str = emit_wt.to_str().ok_or("non-utf8 emit path")?;

    let (code_v8, rep_v8, _) = super::harness::run_source_file(super::harness::SourceRun {
        temp: &temp,
        name: &format!("v8-{}", case.filename),
        source: case.source,
        extra_args: &["--engine", "v8", "--opt", opt_flag, "--emit", emit_v8_str],
    })?;
    let (code_wt, rep_wt, _) = super::harness::run_source_file(super::harness::SourceRun {
        temp: &temp,
        name: &format!("wt-{}", case.filename),
        source: case.source,
        extra_args: &[
            "--engine",
            "wasmtime-core",
            "--opt",
            opt_flag,
            "--emit",
            emit_wt_str,
        ],
    })?;

    assert_eq!(code_v8, 0);
    assert_eq!(code_wt, 0);
    assert_eq!(rep_v8["stage"], "wasm");
    assert_eq!(rep_wt["stage"], "wasm");
    assert_eq!(rep_v8["outcome"], "normal");
    assert_eq!(rep_wt["outcome"], "normal");
    assert_eq!(rep_v8["status"], 0);
    assert_eq!(rep_wt["status"], 0);
    assert_eq!(rep_v8["stack"][0]["type"], "I64");
    assert_eq!(rep_v8["stack"][0]["value"], case.expected);
    assert_eq!(rep_wt["stack"][0]["type"], "I64");
    assert_eq!(rep_wt["stack"][0]["value"], case.expected);
    assert_eq!(
        rep_wt["module"]["wasm_sha256"], rep_v8["module"]["wasm_sha256"],
        "Wasm SHA-256 diverged for {} opt={opt_flag}",
        case.filename
    );
    assert_eq!(
        rep_wt["module"]["wat_sha256"], rep_v8["module"]["wat_sha256"],
        "WAT SHA-256 diverged for {} opt={opt_flag}",
        case.filename
    );

    let wasm_name = opt.wasm_filename();
    let bytes_v8 = std::fs::read(emit_v8.join("engine").join(wasm_name))
        .map_err(|error| format!("read v8 wasm: {error}"))?;
    let bytes_wt = std::fs::read(emit_wt.join("engine").join(wasm_name))
        .map_err(|error| format!("read wasmtime wasm: {error}"))?;
    assert_eq!(
        bytes_wt, bytes_v8,
        "Emitted Wasm bytes diverged for {} opt={opt_flag}",
        case.filename
    );
    Ok(())
}

#[test]
fn native_plan_01_wrapping_arithmetic_and_byte_identical_wasm() -> Result<(), String> {
    let cases = [
        WrappingCase {
            idx: 0,
            filename: "core14.noble",
            source: "9223372036854775807 1 +",
            expected: "-9223372036854775808",
        },
        WrappingCase {
            idx: 1,
            filename: "min_sub.noble",
            source: "-9223372036854775808 1 -",
            expected: "9223372036854775807",
        },
        WrappingCase {
            idx: 2,
            filename: "mul_wrap.noble",
            source: "4294967296 4294967296 *",
            expected: "0",
        },
    ];
    assert_eq!(cases.len(), 3);
    for opt in [super::harness::OptMode::OFF, super::harness::OptMode::ON] {
        for case in &cases {
            assert!(!case.source.is_empty());
            assert_wrapping_case(opt, case)?;
        }
    }
    Ok(())
}

fn assert_core03_captures() -> Result<(), String> {
    let submissions = [
        "def make_adder [ quote [ + ] compose ]",
        "2 make_adder",
        "40 swap run",
        "drop -1 make_adder",
        "40 swap run",
        "drop 9223372036854775807 make_adder",
        "40 swap run",
    ];
    for opt in [super::harness::OptMode::OFF, super::harness::OptMode::ON] {
        let (code_v8, reps_v8, err_v8) =
            super::harness::run_session(super::harness::Backend::V8, opt, &submissions)?;
        let (code_wt, reps_wt, err_wt) =
            super::harness::run_session(super::harness::Backend::WASMTIME_CORE, opt, &submissions)?;
        assert_eq!(code_v8, 0, "v8 stderr: {err_v8}");
        assert_eq!(code_wt, 0, "wt stderr: {err_wt}");
        assert_eq!(reps_v8.len(), submissions.len());
        assert_eq!(reps_wt.len(), submissions.len());
        for idx in [2_usize, 4, 6] {
            assert_eq!(reps_v8[idx]["outcome"], "normal");
            assert_eq!(reps_wt[idx]["outcome"], "normal");
            assert_eq!(reps_wt[idx]["stack"], reps_v8[idx]["stack"]);
            assert_eq!(reps_wt[idx]["candidate_prepare_requests"], 0);
        }
        assert_eq!(reps_wt[2]["stack"][0]["value"], "42");
        assert_eq!(reps_wt[4]["stack"][0]["value"], "39");
        assert_eq!(reps_wt[6]["stack"][0]["value"], "-9223372036854775769");
    }
    Ok(())
}

fn assert_core05_program_lists() -> Result<(), String> {
    let core05_a = "20 nil [ 1 + ] swap cons [ ] [ drop run ] list.case";
    let core05_b = "20 nil [ 2 * ] swap cons [ ] [ drop run ] list.case";
    for (source, expected) in [(core05_a, "21"), (core05_b, "40")] {
        let (code_v8, reps_v8, _) = super::harness::run_session(
            super::harness::Backend::V8,
            super::harness::OptMode::OFF,
            &[source],
        )?;
        let (code_wt, reps_wt, _) = super::harness::run_session(
            super::harness::Backend::WASMTIME_CORE,
            super::harness::OptMode::OFF,
            &[source],
        )?;
        assert_eq!(code_v8, 0);
        assert_eq!(code_wt, 0);
        assert_eq!(reps_v8[0]["stack"][0]["value"], expected);
        assert_eq!(reps_wt[0]["stack"][0]["value"], expected);
        assert_eq!(reps_wt[0]["candidate_prepare_requests"], 0);
    }
    Ok(())
}

fn assert_core09_reflected_recipe() -> Result<(), String> {
    let core09 = "2 quote [ + ] compose dup [ 40 swap run ] dip swap pair";
    let mut off_recipe: Option<serde_json::Value> = None;
    for opt in [super::harness::OptMode::OFF, super::harness::OptMode::ON] {
        let (code_v8, reps_v8, err_v8) =
            super::harness::run_session(super::harness::Backend::V8, opt, &[core09])?;
        let (code_wt, reps_wt, err_wt) =
            super::harness::run_session(super::harness::Backend::WASMTIME_CORE, opt, &[core09])?;
        assert_eq!(code_v8, 0, "v8 stderr: {err_v8}");
        assert_eq!(code_wt, 0, "wt stderr: {err_wt}");
        assert_eq!(reps_wt[0]["candidate_prepare_requests"], 0);
        assert_eq!(reps_wt[0]["stack"], reps_v8[0]["stack"]);

        let pair_val = &reps_wt[0]["stack"][0]["value"];
        assert_eq!(pair_val[0]["type"], "Program");
        assert_eq!(pair_val[1]["type"], "I64");
        assert_eq!(pair_val[1]["value"], "42");
        let recipe = &pair_val[0]["recipe"];
        assert_eq!(
            *recipe,
            serde_json::json!([
                {"literal": {"type": "I64", "value": "2"}},
                {"invoke": "builtin:i64.add"}
            ])
        );
        if let Some(prev) = &off_recipe {
            assert_eq!(recipe, prev, "CORE-09 recipe changed across opt off/on");
        } else {
            off_recipe = Some(recipe.clone());
        }
    }
    Ok(())
}

#[test]
fn native_plan_02_captures_programs_and_recipes() -> Result<(), String> {
    assert_core03_captures()?;
    assert_core05_program_lists()?;
    assert_core09_reflected_recipe()?;
    Ok(())
}

#[test]
fn native_plan_03_ordered_host_trap_core_17() -> Result<(), String> {
    let source = r#"[ "first" test.emit test.abort ] [ "second" test.emit ] compose run"#;
    for opt in [super::harness::OptMode::OFF, super::harness::OptMode::ON] {
        let opt_flag = opt.as_flag();
        let temp = super::harness::TempDir::new(&format!("np03-{opt_flag}"))?;
        let (code_v8, rep_v8, _) = super::harness::run_source_file(super::harness::SourceRun {
            temp: &temp,
            name: "v8.noble",
            source,
            extra_args: &["--engine", "v8", "--opt", opt_flag],
        })?;
        let (code_wt, rep_wt, _) = super::harness::run_source_file(super::harness::SourceRun {
            temp: &temp,
            name: "wt.noble",
            source,
            extra_args: &["--engine", "wasmtime-core", "--opt", opt_flag],
        })?;
        assert_eq!(code_v8, 1);
        assert_eq!(code_wt, 1);
        for rep in [&rep_v8, &rep_wt] {
            assert_eq!(rep["stage"], "wasm");
            assert_eq!(rep["outcome"], "trap");
            assert_eq!(rep["status"], 6);
            assert_eq!(rep["session_state"], "terminated-after-runtime-failure");
            assert_eq!(
                rep["request_trace"],
                serde_json::json!(["test.emit:first", "test.abort"])
            );
            assert_eq!(rep["guest_requests"], 2);
            assert_eq!(rep["protected_operations"], 0);
        }
    }
    Ok(())
}
