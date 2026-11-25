struct QuotaBoundaryCase {
    label: &'static str,
    expected_reason: &'static str,
    ok_subs: Vec<String>,
    over_subs: Vec<String>,
}

fn compose_empty_chunks(mut total: usize) -> Vec<String> {
    let chunk_count = total.div_ceil(25);
    let mut chunks = Vec::with_capacity(chunk_count);
    while total > 0 {
        assert!(
            chunks.len() < chunk_count,
            "chunk count exceeded div_ceil bound"
        );
        let count = total.min(25);
        chunks.push(vec!["[ ] compose"; count].join(" "));
        total = total.saturating_sub(count);
    }
    chunks
}

fn allocation_workload(runs: usize) -> Vec<String> {
    let p16 = format!(
        "[ 0 + 1 pair unpair drop ] {}",
        ["dup compose"; 4].join(" ")
    );
    let runner = ["dup [ run ] dip"; 20].join(" ");
    let m_alloc = format!("{p16} quote [ {runner} run ] compose");
    let final_sub = format!(
        "{}run drop",
        vec!["dup [ run ] dip "; runs.saturating_sub(1)].join("")
    );
    vec![String::from("0"), m_alloc, final_sub]
}

fn dispatch_step_workload(runs: usize) -> Vec<String> {
    let q = format!("[ 0 + ] {}", ["dup compose"; 6].join(" "));
    let runner = ["dup [ run ] dip"; 20].join(" ");
    let m_steps = format!("{q} quote [ {runner} run ] compose");
    let final_sub = format!(
        "{}run drop",
        vec!["dup [ run ] dip "; runs.saturating_sub(1)].join("")
    );
    vec![String::from("0"), m_steps, final_sub]
}

fn continuation_workload(tail_composes: usize) -> Vec<String> {
    let mut subs = vec![String::from(
        r#"[ false [ "x" test.emit test.abort ] [ ] if ]"#,
    )];
    subs.extend(compose_empty_chunks(200));
    subs.push(String::from("[ run ]"));
    subs.extend(compose_empty_chunks(200));
    subs.push(String::from("[ swap [ ] compose swap run ]"));
    subs.extend(compose_empty_chunks(tail_composes));
    subs.push(String::from("run"));
    subs
}

fn depth_workload(composes: usize) -> Vec<String> {
    let mut subs = vec![String::from("[ ]")];
    subs.extend(compose_empty_chunks(composes));
    subs.push(String::from("drop"));
    subs
}

fn assert_operand_bytes_parity() -> Result<(), String> {
    let op_ok: Vec<String> = vec![String::from("1 1 1 1"); 32];
    let mut op_over = op_ok.clone();
    op_over.push(String::from("1"));
    let op_ok_refs: Vec<&str> = op_ok.iter().map(String::as_str).collect();
    let op_over_refs: Vec<&str> = op_over.iter().map(String::as_str).collect();

    let (c_ok_v8, r_ok_v8, _) = super::harness::run_session(
        super::harness::Backend::V8,
        super::harness::OptMode::OFF,
        &op_ok_refs,
    )?;
    let (c_ok_wt, r_ok_wt, _) = super::harness::run_session(
        super::harness::Backend::WASMTIME_CORE,
        super::harness::OptMode::OFF,
        &op_ok_refs,
    )?;
    assert_eq!(c_ok_v8, 0);
    assert_eq!(c_ok_wt, 0);
    assert_eq!(r_ok_v8.last().ok_or("missing report")?["outcome"], "normal");
    assert_eq!(r_ok_wt.last().ok_or("missing report")?["outcome"], "normal");

    let (c_ov_v8, r_ov_v8, _) = super::harness::run_session(
        super::harness::Backend::V8,
        super::harness::OptMode::OFF,
        &op_over_refs,
    )?;
    let (c_ov_wt, r_ov_wt, _) = super::harness::run_session(
        super::harness::Backend::WASMTIME_CORE,
        super::harness::OptMode::OFF,
        &op_over_refs,
    )?;
    assert_eq!(c_ov_v8, 5);
    assert_eq!(c_ov_wt, 5);
    assert_eq!(
        r_ov_v8.last().ok_or("missing report")?["outcome"],
        r_ov_wt.last().ok_or("missing report")?["outcome"]
    );
    assert_eq!(
        r_ov_v8.last().ok_or("missing report")?["stage"],
        r_ov_wt.last().ok_or("missing report")?["stage"]
    );
    Ok(())
}

fn assert_runtime_quota_case(case: &QuotaBoundaryCase) -> Result<(), String> {
    let label = case.label;
    let ok_refs: Vec<&str> = case.ok_subs.iter().map(String::as_str).collect();
    let over_refs: Vec<&str> = case.over_subs.iter().map(String::as_str).collect();

    let (c_ok_v8, r_ok_v8, e_ok_v8) = super::harness::run_session(
        super::harness::Backend::V8,
        super::harness::OptMode::OFF,
        &ok_refs,
    )?;
    let (c_ok_wt, r_ok_wt, e_ok_wt) = super::harness::run_session(
        super::harness::Backend::WASMTIME_CORE,
        super::harness::OptMode::OFF,
        &ok_refs,
    )?;
    assert_eq!(c_ok_v8, 0, "{label} ok v8 failed: {e_ok_v8}");
    assert_eq!(c_ok_wt, 0, "{label} ok wt failed: {e_ok_wt}");
    assert_eq!(r_ok_v8.last().ok_or("missing report")?["outcome"], "normal");
    assert_eq!(r_ok_wt.last().ok_or("missing report")?["outcome"], "normal");

    let (c_ov_v8, r_ov_v8, e_ov_v8) = super::harness::run_session(
        super::harness::Backend::V8,
        super::harness::OptMode::OFF,
        &over_refs,
    )?;
    let (c_ov_wt, r_ov_wt, e_ov_wt) = super::harness::run_session(
        super::harness::Backend::WASMTIME_CORE,
        super::harness::OptMode::OFF,
        &over_refs,
    )?;
    assert_eq!(c_ov_v8, 5, "{label} over v8 exit: {e_ov_v8}");
    assert_eq!(c_ov_wt, 5, "{label} over wt exit: {e_ov_wt}");
    let last_v8 = r_ov_v8.last().ok_or("missing v8 over report")?;
    let last_wt = r_ov_wt.last().ok_or("missing wt over report")?;
    for rep in [last_v8, last_wt] {
        assert_eq!(rep["stage"], "wasm", "{label}: {rep}");
        assert_eq!(rep["outcome"], "runtime-exhausted", "{label}: {rep}");
        assert_eq!(rep["status"], 2, "{label}: {rep}");
        assert_eq!(rep["quota_reason"], case.expected_reason, "{label}: {rep}");
        assert_eq!(rep["stack"], serde_json::json!([]), "{label}: {rep}");
        assert_eq!(
            rep["session_state"], "terminated-after-runtime-failure",
            "{label}: {rep}"
        );
    }
    Ok(())
}

fn assert_abort_trap_parity() -> Result<(), String> {
    let (c_trap_v8, r_trap_v8, _) = super::harness::run_session(
        super::harness::Backend::V8,
        super::harness::OptMode::OFF,
        &["test.abort"],
    )?;
    let (c_trap_wt, r_trap_wt, _) = super::harness::run_session(
        super::harness::Backend::WASMTIME_CORE,
        super::harness::OptMode::OFF,
        &["test.abort"],
    )?;
    assert_eq!(c_trap_v8, 1);
    assert_eq!(c_trap_wt, 1);
    assert_eq!(r_trap_v8[0]["outcome"], "trap");
    assert_eq!(r_trap_wt[0]["outcome"], "trap");
    assert_eq!(r_trap_v8[0]["status"], 6);
    assert_eq!(r_trap_wt[0]["status"], 6);
    assert_eq!(r_trap_wt[0]["request_trace"], r_trap_v8[0]["request_trace"]);
    Ok(())
}

#[test]
fn native_plan_04_quota_boundaries_and_trap_parity() -> Result<(), String> {
    assert_operand_bytes_parity()?;
    let leaves_ok = format!("[ 1 drop ] {} drop", ["dup compose"; 7].join(" "));
    let leaves_over = format!(
        "[ 1 drop ] {} [ 1 drop ] compose drop",
        ["dup compose"; 7].join(" ")
    );
    let cases = [
        QuotaBoundaryCase {
            label: "allocation",
            expected_reason: "allocation",
            ok_subs: allocation_workload(3),
            over_subs: allocation_workload(4),
        },
        QuotaBoundaryCase {
            label: "recipe leaves",
            expected_reason: "recipe leaves",
            ok_subs: vec![leaves_ok],
            over_subs: vec![leaves_over],
        },
        QuotaBoundaryCase {
            label: "program graph depth",
            expected_reason: "program graph depth",
            ok_subs: depth_workload(255),
            over_subs: depth_workload(256),
        },
        QuotaBoundaryCase {
            label: "continuation bytes",
            expected_reason: "continuation bytes",
            ok_subs: continuation_workload(110),
            over_subs: continuation_workload(111),
        },
        QuotaBoundaryCase {
            label: "execution work",
            expected_reason: "execution work",
            ok_subs: dispatch_step_workload(24),
            over_subs: dispatch_step_workload(25),
        },
    ];
    assert_eq!(cases.len(), 5);
    for case in &cases {
        assert!(!case.ok_subs.is_empty());
        assert_runtime_quota_case(case)?;
    }
    assert_abort_trap_parity()
}

#[test]
fn native_plan_05_bounded_store_retention_1000_submissions() -> Result<(), String> {
    let submissions: Vec<&str> = vec!["1 drop"; 1001];
    let (code, reports, stderr) = super::harness::run_session(
        super::harness::Backend::WASMTIME_CORE,
        super::harness::OptMode::OFF,
        &submissions,
    )?;
    assert_eq!(
        code, 5,
        "expected exit 5 at 1001st submission, stderr: {stderr}"
    );
    assert_eq!(reports.len(), 1001, "expected 1001 reports");

    for (idx, report) in reports[..1000].iter().enumerate() {
        assert_eq!(
            report["outcome"],
            "normal",
            "submission {} failed: {report}",
            idx.saturating_add(1)
        );
        assert_eq!(report["status"], 0);
        assert_eq!(report["session_state"], "retained");
    }

    let bound_report = &reports[1000];
    assert_eq!(bound_report["submission"], 1001);
    assert_eq!(bound_report["stage"], "wasm");
    assert_eq!(bound_report["outcome"], "runtime-exhausted");
    assert_eq!(bound_report["quota_reason"], "store-instance-limit");
    assert_eq!(bound_report["guest_requests"], 0);
    assert_eq!(bound_report["protected_operations"], 0);
    assert_eq!(bound_report["candidate_prepare_requests"], 0);
    assert_eq!(bound_report["prior_stack"], "unchanged");
    assert_eq!(bound_report["prior_session"], "unchanged");
    assert_eq!(
        bound_report["session_state"],
        "terminated-after-store-limit"
    );
    Ok(())
}
