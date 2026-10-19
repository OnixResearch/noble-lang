#[test]
fn selected_schema_and_abi_mode_bind_preparations_without_changing_sync_mapping(
) -> Result<(), String> {
    let wit = b"package test:profiles@1.0.0; world sync { import increment: func(value: s64) -> s64; export run: func(value: s64) -> s64; } world native { import increment: async func(value: s64) -> s64; export run: async func(value: s64) -> s64; } world live { export identity: func(value: stream<u8>) -> stream<u8>; }";
    let synchronous = super::world(wit, "sync")?;
    let asynchronous = super::world(wit, "native")?;
    let live_only = super::world(wit, "live")?;
    assert!(!synchronous.is_async());
    assert!(asynchronous.is_async());
    assert!(live_only.is_async());
    for selected in [&synchronous, &asynchronous] {
        let import = selected.imports().first().ok_or("missing increment")?;
        assert_eq!(import.core_module, "$root");
        assert_eq!(import.core_name, "increment");
        assert_eq!(selected.exports()[0].export_name, "run");
    }
    let source_session = synchronous
        .session()
        .map_err(|error| format!("{error:?}"))?;
    let prepared = source_session
        .prepare(b"def saved [ increment ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let mut target_session = asynchronous
        .session()
        .map_err(|error| format!("{error:?}"))?;
    let error = target_session
        .commit(prepared)
        .expect_err("foreign ABI preparation committed");
    assert!(matches!(
        error.stage(),
        noble_contracts::source::Stage::Acceptance
    ));

    let original = super::world(super::ASYNC_WIT, "main")?;
    let altered_wit = [
        super::ASYNC_WIT,
        b"\n// exact WIT bytes remain build-bound\n",
    ]
    .concat();
    let altered = super::world(&altered_wit, "main")?;
    let session = original.session().map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(b"def saved [ service.number ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let mut target = altered.session().map_err(|error| format!("{error:?}"))?;
    let error = target
        .commit(prepared)
        .expect_err("foreign WIT preparation committed");
    assert!(matches!(
        error.stage(),
        noble_contracts::source::Stage::Acceptance
    ));
    Ok(())
}

#[test]
fn unsupported_async_payloads_do_not_widen_to_untyped_handles() {
    for payload in [
        "stream<s64>",
        "future<string>",
        "future<stream<u8>>",
        "future<result<s64, list<u8>>>",
        "future<result<list<u8>, string>>",
        "stream",
        "future",
        "list<future<s64>>",
    ] {
        let wit = format!(
            "package test:unsupported@1.0.0; world main {{ import get: async func() -> {payload}; }}"
        );
        let error = noble_contracts::component::World::parse(wit.as_bytes(), "main", super::LIMITS)
            .expect_err("unsupported live payload was admitted");
        assert_eq!(error.stage, noble_contracts::component::Stage::Wit);
        assert!(matches!(
            error.diagnostic.kind,
            noble_contracts::DiagnosticKind::Invalid | noble_contracts::DiagnosticKind::Unsupported
        ));
    }
}

#[test]
fn unsigned_wit_requires_a_selected_checked_boundary() -> Result<(), String> {
    const WIT: &[u8] =
        b"package test:unsigned@1.0.0; world main { export echo: func(value: u64) -> u64; }";
    let implicit = noble_contracts::component::World::parse(WIT, "main", super::LIMITS)
        .expect_err("implicit unsigned-to-signed binding must refuse");
    assert_eq!(implicit.stage, noble_contracts::component::Stage::Binding);
    assert_eq!(
        implicit.diagnostic.kind,
        noble_contracts::DiagnosticKind::Invalid
    );
    assert_eq!(noble_contracts::component::Type::CheckedU64.noble(), None);

    let selected = noble_contracts::component::World::parse_checked_u64(WIT, "main", super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_ne!(
        selected.build_context(),
        super::world(
            b"package test:unsigned@1.0.0; world main { export echo: func(value: s64) -> s64; }",
            "main"
        )?
        .build_context()
    );
    let operation = selected
        .exports()
        .first()
        .ok_or("missing unsigned export")?;
    assert_eq!(
        operation.parameters,
        [noble_contracts::component::Type::CheckedU64]
    );
    assert_eq!(
        operation.input_types(&selected),
        Some(vec![noble_kernel::types::Ty::I64])
    );
    let accepted = selected
        .prepare_export("echo", b"", super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        accepted
            .submission()
            .ok_or("missing unsigned body")?
            .request
            .expected
            .stack_out,
        vec![noble_kernel::types::Ty::I64]
    );
    Ok(())
}

#[test]
fn checked_unsigned_selector_requires_import_free_exact_scalar_export() -> Result<(), String> {
    for wit in [
        b"package test:unsigned@1.0.0; world main { import signed: func(value: s64) -> s64; export echo: func(value: u64) -> u64; }".as_slice(),
        b"package test:unsigned@1.0.0; world main { import unsigned: func(value: u64) -> u64; export echo: func(value: u64) -> u64; }",
        b"package test:unsigned@1.0.0; world main { import unsigned: func(value: u64) -> u64; }",
        b"package test:unsigned@1.0.0; world main { export echo: func() -> u64; }",
        b"package test:unsigned@1.0.0; world main { export echo: func(value: u64); }",
        b"package test:unsigned@1.0.0; world main { export echo: func(value: s64) -> u64; }",
        b"package test:unsigned@1.0.0; world main { export echo: func(value: u64) -> s64; }",
        b"package test:unsigned@1.0.0; world main { export echo: async func(value: u64) -> u64; }",
    ] {
        let error = noble_contracts::component::World::parse_checked_u64(wit, "main", super::LIMITS)
            .expect_err("selected checked export must be import-free and exactly u64 -> u64");
        assert_eq!(error.stage, noble_contracts::component::Stage::Binding);
        assert_eq!(error.diagnostic.kind, noble_contracts::DiagnosticKind::Unsupported);
    }
    let signed = b"package test:signed@1.0.0; world main { import signed: func(value: s64) -> s64; export echo: func(value: s64) -> s64; }";
    let world = noble_contracts::component::World::parse(signed, "main", super::LIMITS).map_err(
        |error| format!("normal signed import/export world must remain supported: {error:?}"),
    )?;
    assert_eq!(world.imports().len(), 1);
    assert_eq!(world.exports().len(), 1);
    Ok(())
}
