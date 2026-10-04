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
    let prepared = selected.prepare_export("echo", b"", LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    noble_wasm::component::compile(&selected, core::slice::from_ref(&prepared))
        .map_err(|_| "selected checked-u64 world refused valid identity export")?;

    let changed = noble_contracts::component::World::parse_checked_u64(
        b"package test:unsigned@1.0.0; world main { export echo: func(value: u64) -> u64; }\n",
        "main", LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        noble_wasm::component::compile(&changed, core::slice::from_ref(&prepared)),
        Err(noble_wasm::Diagnostic::Invalid)
    ));
    Ok(())
}
