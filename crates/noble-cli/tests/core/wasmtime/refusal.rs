struct SamplePaths {
    source: String,
    manifest: String,
    authority: String,
}

fn assert_live_refusals(paths: &SamplePaths) -> Result<(), String> {
    for subcommand in ["repl", "watch"] {
        let (code, reports, _) = super::harness::run_noble(
            &[
                "live",
                subcommand,
                "--source",
                &paths.source,
                "--engine",
                "wasmtime-core",
            ],
            None,
        )?;
        assert_eq!(code, 2);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0]["schema"], "noble-live-report/v1");
        assert_eq!(reports[0]["stage"], "engine-selection");
        assert_eq!(reports[0]["outcome"], "wasmtime-core-unavailable");
        assert_eq!(reports[0]["guest_requests"], 0);
        assert_eq!(reports[0]["protected_operations"], 0);
    }

    let (code, reports, _) = super::harness::run_noble(
        &[
            "live",
            "slot",
            "--engine",
            "wasmtime-core",
            "--authority",
            &paths.authority,
        ],
        None,
    )?;
    assert_eq!(code, 2);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["schema"], "noble-live-slot-report/v1");
    assert_eq!(reports[0]["stage"], "selection");
    assert_eq!(reports[0]["outcome"], "refused");
    Ok(())
}

fn assert_standalone_profile_refusals() -> Result<(), String> {
    let (code, reports, stderr) =
        super::harness::run_noble(&["calculator", "--engine", "wasmtime-core"], None)?;
    assert_eq!(code, 2);
    assert!(reports.is_empty());
    assert!(
        stderr.contains("wasmtime-core is unsupported for calculator"),
        "unexpected calculator stderr: {stderr}"
    );

    let (code, reports, _) = super::harness::run_noble(
        &["wasm-experiment", "wasm-gc", "--engine", "wasmtime-core"],
        None,
    )?;
    assert_eq!(code, 2);
    assert!(reports.is_empty());
    Ok(())
}

fn assert_run_and_session_flag_refusals(paths: &SamplePaths) -> Result<(), String> {
    let (code, reports, _) = super::harness::run_noble(
        &[
            "run",
            &paths.source,
            "--engine",
            "wasmtime-core",
            "--declared-modules",
            "--bindings",
            &paths.manifest,
        ],
        None,
    )?;
    assert_eq!(code, 4);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["outcome"], "unsupported");
    assert_eq!(reports[0]["guest_requests"], 0);
    assert_eq!(reports[0]["protected_operations"], 0);

    for flag in ["--text-byte-cursor", "--checked-i64"] {
        let (code, reports, _) = super::harness::run_noble(
            &["session", "--engine", "wasmtime-core", flag],
            Some(b"1 drop\n"),
        )?;
        assert_eq!(code, 4);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0]["outcome"], "unsupported");
        assert_eq!(reports[0]["guest_requests"], 0);
        assert_eq!(reports[0]["protected_operations"], 0);
    }

    let (code, reports, _) = super::harness::run_noble(
        &[
            "run",
            &paths.source,
            "--engine",
            "wasmtime-core",
            "--wasm-gc",
        ],
        None,
    )?;
    assert_eq!(code, 2);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["stage"], "arguments");
    assert_eq!(reports[0]["outcome"], "invalid-input");
    assert_eq!(reports[0]["guest_requests"], 0);
    Ok(())
}

#[test]
fn native_plan_06_unsupported_profiles_refuse_without_fallback() -> Result<(), String> {
    let temp = super::harness::TempDir::new("np06")?;
    let source_path = temp.path().join("sample.noble");
    let manifest_path = temp.path().join("bindings.txt");
    let authority_path = temp.path().join("authority.json");
    std::fs::write(&source_path, "1 2 +").map_err(|error| format!("write sample: {error}"))?;
    std::fs::write(&manifest_path, "").map_err(|error| format!("write manifest: {error}"))?;
    std::fs::write(&authority_path, "{}").map_err(|error| format!("write authority: {error}"))?;
    let paths = SamplePaths {
        source: String::from(source_path.to_str().ok_or("non-utf8 path")?),
        manifest: String::from(manifest_path.to_str().ok_or("non-utf8 path")?),
        authority: String::from(authority_path.to_str().ok_or("non-utf8 path")?),
    };
    assert_live_refusals(&paths)?;
    assert_standalone_profile_refusals()?;
    assert_run_and_session_flag_refusals(&paths)
}
