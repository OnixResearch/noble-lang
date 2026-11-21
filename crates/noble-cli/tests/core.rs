#![feature(register_tool)]
#![register_tool(tigerstyle)]
//! Declared structured CORE-15/16 harnesses. These never instantiate a Wasm engine.

#[path = "core/eligibility.rs"]
mod eligibility;
#[path = "core/mutations.rs"]
mod mutations;

const fn source_limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    }
}

fn candidate(text: &[u8]) -> Result<noble_kernel::execution::Submission, String> {
    let source = noble_contracts::source::Session::new();
    let prepared = source
        .prepare(text, &[], source_limits())
        .map_err(|error| error.diagnostic().message.clone())?;
    prepared
        .submission()
        .cloned()
        .ok_or_else(|| "expression did not produce a candidate".into())
}

#[test]
fn source_preparation_rejection_preserves_definition_namespace() -> Result<(), String> {
    let mut source = noble_contracts::source::Session::new();
    let definition = source
        .prepare(b"def increment [ 1 + ]", &[], source_limits())
        .map_err(|error| error.diagnostic().message.clone())?;
    source
        .commit(definition)
        .map_err(|error| error.diagnostic().message.clone())?;
    let bad = source.prepare(b"def increment [ \"wrong\" 1 + ]", &[], source_limits());
    assert!(bad.is_err(), "ill-typed replacement must not install");
    let good = source
        .prepare(b"41 increment", &[], source_limits())
        .map_err(|error| error.diagnostic().message.clone())?;
    let candidate = good.submission().ok_or("missing expression")?;
    assert_eq!(
        candidate.request.expected.stack_out,
        vec![noble_kernel::types::Ty::I64]
    );
    assert!(
        noble_wasm::source::Compiler::new()
            .prepare(candidate)
            .is_ok(),
        "old checked definition must remain compilable"
    );
    Ok(())
}

#[test]
fn source_limits_accept_exact_work_and_reject_the_next_step() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let source = b"40 2 quote [ + ] compose run";
    for key in ["bytes", "nodes", "depth", "work"] {
        let baseline = source_limits();
        let mut low = 0_u32;
        let mut high = match key {
            "bytes" => baseline.bytes,
            "nodes" => baseline.nodes,
            "depth" => baseline.depth,
            "work" => baseline.work,
            _ => return Err("unknown preparation limit".into()),
        };
        while low < high {
            let middle = low.midpoint(high);
            let mut limits = baseline;
            set_limit(&mut limits, key, middle)?;
            match session.prepare(source, &[], limits) {
                Ok(_) => high = middle,
                Err(error) => {
                    assert_eq!(
                        error.diagnostic().kind,
                        noble_contracts::DiagnosticKind::Exhausted
                    );
                    low = middle.checked_add(1).ok_or("limit search overflow")?;
                }
            }
        }
        assert!(low > 0, "the workload must consume {key}");
        let mut exact = baseline;
        set_limit(&mut exact, key, low)?;
        let prepared = session.prepare(source, &[], exact).map_err(|error| {
            format!("exact {key} limit refused: {}", error.diagnostic().message)
        })?;
        assert_eq!(prepared.output(), &[noble_kernel::types::Ty::I64]);
        set_limit(
            &mut exact,
            key,
            low.checked_sub(1).ok_or("zero exact limit")?,
        )?;
        match session.prepare(source, &[], exact) {
            Ok(_) => return Err(format!("exceeded {key} was accepted")),
            Err(error) => assert_eq!(
                error.diagnostic().kind,
                noble_contracts::DiagnosticKind::Exhausted
            ),
        }
    }
    Ok(())
}

const fn set_limit(
    limits: &mut noble_contracts::Limits,
    key: &str,
    value: u32,
) -> Result<(), &'static str> {
    match key.as_bytes() {
        b"bytes" => limits.bytes = value,
        b"nodes" => limits.nodes = value,
        b"depth" => limits.depth = value,
        b"work" => limits.work = value,
        _ => return Err("unknown preparation limit"),
    }
    Ok(())
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the test asserts Invalid for the foreign commit, then requires the already-prepared local commit to succeed. That successful continuation is the observable unchanged-state check; extra assertions would only pin fixture internals."
)]
fn backend_rejects_foreign_same_generation_commit_without_changing_state() -> Result<(), String> {
    let mut first = noble_wasm::source::Compiler::new();
    let mut second = noble_wasm::source::Compiler::new();
    let initial = first
        .prepare(&candidate(b"1")?)
        .map_err(|_| "first compiler preparation failed")?;
    first
        .commit(initial)
        .map_err(|_| "first compiler commit failed")?;
    let initial = second
        .prepare(&candidate(b"1 2")?)
        .map_err(|_| "second compiler preparation failed")?;
    second
        .commit(initial)
        .map_err(|_| "second compiler commit failed")?;
    let foreign = first
        .prepare(&candidate(b"3")?)
        .map_err(|_| "foreign preparation failed")?;
    let local = second
        .prepare(&candidate(b"4")?)
        .map_err(|_| "local preparation failed")?;
    assert!(matches!(
        second.commit(foreign),
        Err(noble_wasm::Diagnostic::Invalid)
    ));
    second
        .commit(local)
        .map_err(|_| "rejection changed the second compiler state".into())
}
