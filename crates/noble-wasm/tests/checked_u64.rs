#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

#[test]
fn checked_unsigned_export_requires_its_exact_world_recipe() -> Result<(), String> {
    let wit = b"package test:unsigned@1.0.0; world main { export echo: func(value: u64) -> u64; }";
    let selected = noble_contracts::component::World::parse_checked_u64(wit, "main", LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let prepared = selected
        .prepare_export("echo", b"", LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    noble_wasm::component::compile(&selected, core::slice::from_ref(&prepared))
        .map_err(|_| "selected checked-u64 world refused valid identity export")?;

    let changed = noble_contracts::component::World::parse_checked_u64(
        b"package test:unsigned@1.0.0; world main { export echo: func(value: u64) -> u64; }\n",
        "main",
        LIMITS,
    )
    .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        noble_wasm::component::compile(&changed, core::slice::from_ref(&prepared)),
        Err(noble_wasm::Diagnostic::Invalid)
    ));
    Ok(())
}

#[test]
fn checked_i64_source_submission_is_accepted_by_the_checked_compiler() -> Result<(), String> {
    let frontend = noble_contracts::source::Session::new_checked_i64();
    let prepared = frontend
        .prepare(b"1 2 i64.checked-add", &[], LIMITS)
        .map_err(|error| format!("front end refused checked-I64 source: {error:?}"))?;
    let submission = prepared
        .submission()
        .ok_or("checked-I64 source was not an expression")?;
    let outcome = noble_kernel::acceptance::check(
        &submission.environment,
        &submission.request,
        &submission.body.candidate,
    );
    if !matches!(outcome, noble_kernel::untrusted::Outcome::Accepted(_)) {
        return Err(format!(
            "checked-I64 kernel acceptance refused source: {outcome:?}"
        ));
    }
    match noble_wasm::source::Compiler::new_checked_i64().prepare(submission) {
        Ok(_) => Ok(()),
        Err(noble_wasm::Diagnostic::Invalid) => Err("compiler returned Invalid".into()),
        Err(noble_wasm::Diagnostic::Exhausted) => Err("compiler returned Exhausted".into()),
        Err(noble_wasm::Diagnostic::Unsupported) => Err("compiler returned Unsupported".into()),
        Err(noble_wasm::Diagnostic::Defective) => Err("compiler returned Defective".into()),
    }
}
