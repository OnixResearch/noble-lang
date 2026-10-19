#![feature(register_tool)]
#![register_tool(tigerstyle)]

#[path = "component/eligibility.rs"]
mod eligibility;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

const ASYNC_WIT: &[u8] = br#"
package test:tasks@1.2.3;
interface service {
    resource ticket;
    bytes: async func() -> stream<u8>;
    number: async func() -> future<s64>;
    outcome: async func() -> future<result<s64, string>>;
    finish-bytes: async func(value: stream<u8>) -> s64;
    finish-number: async func(value: future<s64>) -> s64;
    finish-outcome: async func(value: future<result<s64, string>>) -> s64;
    open: async func() -> own<ticket>;
    close: async func(value: own<ticket>) -> s64;
}
world main {
    import service;
    import increment: async func(value: s64) -> s64;
    export run: async func() -> s64;
    export identity: async func(value: future<s64>) -> future<s64>;
}
"#;

fn world(wit: &[u8], name: &str) -> Result<noble_contracts::component::World, String> {
    noble_contracts::component::World::parse(wit, name, LIMITS)
        .map_err(|error| format!("{error:?}"))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the helper invokes the non-const source parser and checker and formats an owned test failure for an incorrectly accepted program."
)]
fn rejects_source(session: &noble_contracts::source::Session, source: &str) -> Result<(), String> {
    match session.prepare(source.as_bytes(), &[], LIMITS) {
        Err(error) => {
            assert_eq!(
                error.diagnostic().kind,
                noble_contracts::DiagnosticKind::Invalid
            );
            Ok(())
        }
        Ok(_) => Err(format!(
            "live async value escaped its ownership contract: {source}"
        )),
    }
}

#[test]
fn async_imports_preserve_direct_style_types_effects_and_owned_results() -> Result<(), String> {
    let world = world(ASYNC_WIT, "main")?;
    assert!(world.is_async());
    assert_eq!(world.profile(), noble_contracts::component::Profile::Async);
    let prepared = world
        .prepare_export(
            "run",
            b"service.bytes service.finish-bytes increment",
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let submission = prepared.submission().ok_or("missing checked export body")?;
    let environment = world.environment().map_err(|error| format!("{error:?}"))?;
    match noble_kernel::acceptance::check(
        &environment,
        &submission.request,
        &submission.body.candidate,
    ) {
        noble_kernel::untrusted::Outcome::Accepted(checked) => {
            assert_eq!(
                checked.interface.stack_out,
                vec![noble_kernel::types::Ty::I64]
            );
        }
        outcome => {
            return Err(format!(
                "regenerated environment rejected export: {outcome:?}"
            ))
        }
    }
    let mut expected_effects = Vec::with_capacity(3);
    for word in ["service.bytes", "service.finish-bytes", "increment"] {
        let operation = world
            .imports()
            .iter()
            .find(|operation| operation.word == word)
            .ok_or("missing direct-style import")?;
        assert!(operation.asynchronous);
        expected_effects.push(operation.effect.ok_or("missing request effect")?);
        assert!(world.check_import_effect(word, &[]).is_err());
        world
            .check_import_effect(word, &[&operation.identity])
            .map_err(|error| format!("{error:?}"))?;
    }
    assert_eq!(
        submission.request.expected.allowed_effects.as_slice(),
        expected_effects.as_slice()
    );

    let identity = world
        .prepare_export("identity", b"", LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let identity = identity.submission().ok_or("missing owner-transfer body")?;
    assert_eq!(
        identity.request.expected.stack_out,
        vec![noble_contracts::component::Type::FutureS64
            .noble()
            .ok_or("missing future mapping")?]
    );
    let owned = world
        .prepare_export("run", b"service.open service.close", LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let owned = owned.submission().ok_or("missing owned-resource body")?;
    assert_eq!(
        owned.request.expected.stack_out,
        vec![noble_kernel::types::Ty::I64]
    );

    rejects_async_finish_mismatches(&world)
}

#[test]
fn async_operations_reject_explicit_and_implicit_retained_borrows() -> Result<(), String> {
    for wit in [
        "package test:borrowed@1.0.0; interface handles { resource ticket; } world main { import handles; use handles.{ticket}; import wait: async func(value: borrow<ticket>) -> s64; }",
        "package test:borrowed@1.0.0; interface handles { resource ticket; wait: async func(value: borrow<ticket>) -> s64; } world main { import handles; }",
        "package test:borrowed@1.0.0; interface handles { resource ticket { wait: async func() -> s64; } } world main { import handles; }",
        "package test:borrowed@1.0.0; interface handles { resource ticket; } world main { import handles; use handles.{ticket}; export wait: async func(value: borrow<ticket>) -> s64; }",
    ] {
        let error = noble_contracts::component::World::parse(wit.as_bytes(), "main", LIMITS)
            .expect_err("async operation retained a borrow");
        assert_eq!(error.stage, noble_contracts::component::Stage::Wit);
        assert_eq!(error.diagnostic.kind, noble_contracts::DiagnosticKind::Unsupported);
    }

    let synchronous = world(
        b"package test:borrowed@1.0.0; interface handles { resource ticket { read: func() -> s64; } } world main { import handles; }",
        "main",
    )?;
    assert!(!synchronous.is_async());
    assert_eq!(
        synchronous.profile(),
        noble_contracts::component::Profile::Sync
    );
    let method = synchronous
        .imports()
        .first()
        .ok_or("missing borrowed method")?;
    let kind = synchronous
        .resources()
        .first()
        .ok_or("missing declared resource")?
        .kind;
    assert!(!method.asynchronous);
    assert_eq!(
        method.input_types(&synchronous),
        Some(vec![noble_kernel::types::Ty::Resource(kind)])
    );
    assert_eq!(
        method.output_types(&synchronous),
        Some(vec![
            noble_kernel::types::Ty::Resource(kind),
            noble_kernel::types::Ty::I64
        ])
    );
    Ok(())
}

#[test]
fn synthetic_resource_identities_are_disjoint_and_stable() -> Result<(), String> {
    let world = world(ASYNC_WIT, "main")?;
    let mut seen = vec![noble_kernel::authority::WITNESS_KIND];
    for resource in world.resources() {
        seen.push(resource.kind);
    }
    for ty in [
        noble_contracts::component::Type::StreamU8,
        noble_contracts::component::Type::FutureS64,
        noble_contracts::component::Type::FutureResultS64String,
    ] {
        let kind = ty.resource_kind().ok_or("live type has no owner kind")?;
        assert!(!seen.contains(&kind));
        assert!(
            usize::try_from(kind.0).map_err(|error| format!("{error:?}"))?
                > noble_contracts::component::MAX_RESOURCES
        );
        seen.push(kind);
        assert_eq!(
            ty.noble(),
            noble_contracts::component::Type::Own(kind).noble()
        );
        let producer = world
            .imports()
            .iter()
            .find(|operation| operation.results == [ty])
            .ok_or("missing live producer")?;
        let consumer = world
            .imports()
            .iter()
            .find(|operation| operation.parameters == [ty])
            .ok_or("missing matching live consumer")?;
        assert_eq!(producer.output_types(&world), consumer.input_types(&world));
    }
    Ok(())
}

#[path = "component/part2.rs"]
mod part2;

/// Async finish-outcome and finish-number mixes must be rejected before publication.
fn rejects_async_finish_mismatches(
    world: &noble_contracts::component::World,
) -> Result<(), String> {
    let session = world.session().map_err(|error| format!("{error:?}"))?;
    rejects_source(&session, "service.number service.finish-outcome")?;
    rejects_source(&session, "service.open service.finish-number")
}
