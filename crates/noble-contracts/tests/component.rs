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

fn retained() -> noble_contracts::source::RetainedSourceLimit {
    noble_contracts::source::RetainedSourceLimit::new(LIMITS.bytes)
        .expect("component test source limit")
}

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

#[test]
fn worker_test_emit_selection_requires_exact_checked_effect_and_world_shape() -> Result<(), String>
{
    use noble_contracts::component::{Type, World};
    use noble_contracts::source::{BoundOperation, ModuleSession};
    use noble_kernel::contracts::TEST_EMIT;
    use noble_kernel::shapes::{EffectSlot, Pattern};
    use noble_kernel::types::{EffSet, Ty};

    const WORKER_WIT: &[u8] = include_bytes!("../../noble-wasm/wit/worker-program.wit");
    let binding = BoundOperation {
        module_name: "ledger".into(),
        module_version: 1,
        operation: "test.emit".into(),
        adapter_identity: "worker-test-emit".into(),
        adapter_slot: 4,
        input: vec![Ty::Text],
        output: vec![],
        effects: vec![TEST_EMIT],
    };
    let session = ModuleSession::new(std::slice::from_ref(&binding), retained())
        .map_err(|error| format!("{error:?}"))?;
    let module = session.prepare(
        b"module ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]",
        &[], LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    let owner = module
        .checked_program_owner_for_binding("plus", &binding)
        .ok_or("bound source lost its exact checked owner")?;
    let (session, committed) = session.commit(module);
    committed.map_err(|error| format!("{error:?}"))?;
    let invocation = session
        .prepare(b"ledger@1.plus", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let checked = invocation
        .checked_program_interface_for_binding(&owner, &binding)
        .ok_or("bound source lost its exact checked invocation")?;

    let ordinary = world(WORKER_WIT, "guest")?;
    let kind = ordinary
        .resources()
        .first()
        .ok_or("missing WIT Program resource")?
        .kind;
    assert_eq!(
        ordinary.noble_type(Type::Own(kind)),
        Some(Ty::Resource(kind))
    );
    assert_eq!(
        ordinary.noble_type(Type::Borrow(kind)),
        Some(Ty::Resource(kind))
    );
    assert!(
        world(WORKER_WIT, "guest")?
            .select_worker_program(&checked)
            .is_err(),
        "the PURE ingress must not select an effectful Program"
    );

    let selected = world(WORKER_WIT, "guest")?
        .select_worker_program_for_test_emit(&checked)
        .map_err(|error| format!("{error:?}"))?;
    let program = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::from_ids(&[TEST_EMIT]));
    assert_eq!(selected.noble_type(Type::Own(kind)), Some(program));
    assert_eq!(selected.noble_type(Type::Borrow(kind)), None);
    assert_ne!(
        selected.build_context(),
        ordinary.build_context(),
        "the effect-bound type selection must have a distinct context"
    );
    let request = &selected.imports()[2];
    assert!(
        selected.check_import_effect(&request.word, &[]).is_err(),
        "selecting a Program type does not grant the host export operation"
    );
    let environment = selected
        .environment()
        .map_err(|error| format!("{error:?}"))?;
    let admit_id = selected.imports()[0]
        .definition
        .ok_or("missing WIT admit definition")?;
    let admit = environment
        .defs
        .get(admit_id.0 as usize)
        .ok_or("missing generated WIT admit contract")?;
    assert_eq!(
        admit.stack_out.last(),
        Some(&Pattern::program(
            vec![Pattern::I64],
            vec![Pattern::I64],
            vec![EffectSlot::Effect(TEST_EMIT)]
        )),
        "the generated import must carry the selected latent effect"
    );
    let guest = selected
        .prepare_export("compute", b"programs.admit swap programs.invoke", LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let expected = EffSet::from_ids(&[
        selected.imports()[0].effect.ok_or("missing admit effect")?,
        selected.imports()[1]
            .effect
            .ok_or("missing invoke effect")?,
    ]);
    assert_eq!(
        guest
            .submission()
            .ok_or("missing checked Worker guest")?
            .request
            .expected
            .allowed_effects,
        expected
    );
    let direct_run = selected
        .prepare_export("compute", b"programs.admit run", LIMITS)
        .err()
        .ok_or("a latent test.emit effect escaped the WIT callable set")?;
    assert_eq!(direct_run.stage, noble_contracts::component::Stage::Export);
    assert!(
        selected
            .prepare_export("compute", b"\"private\" emit", LIMITS)
            .is_err(),
        "the selected Program type is not a direct test.emit import"
    );

    let pure_session = ModuleSession::new(&[], retained()).map_err(|error| format!("{error:?}"))?;
    let pure_module = pure_session
        .prepare(
            b"module plain@1 [ export plus def plus [ 1 + ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let pure_owner = pure_module
        .checked_program_owner("plus")
        .ok_or("missing checked pure owner")?;
    let (pure_session, committed) = pure_session.commit(pure_module);
    committed.map_err(|error| format!("{error:?}"))?;
    let pure_invocation = pure_session
        .prepare(b"plain@1.plus", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let pure = pure_invocation
        .checked_program_interface(&pure_owner)
        .ok_or("missing checked pure invocation")?;
    assert!(
        world(WORKER_WIT, "guest")?
            .select_worker_program_for_test_emit(&pure)
            .is_err(),
        "a checked pure Program cannot select test.emit"
    );
    let pure_selected = world(WORKER_WIT, "guest")?
        .select_worker_program(&pure)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        pure_selected.noble_type(Type::Own(kind)),
        Some(Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty()))
    );
    assert_ne!(
        selected.build_context(),
        pure_selected.build_context(),
        "pure and test.emit Program mappings must not share an identity"
    );

    let changed = std::str::from_utf8(WORKER_WIT)
        .map_err(|error| error.to_string())?
        .replace(
            "request-code-export: func(owner: own<program>, destination: string) -> bool",
            "request-code-export: func(owner: own<program>, destination: string) -> s64",
        );
    assert!(
        World::parse(changed.as_bytes(), "guest", LIMITS)
            .map_err(|error| format!("{error:?}"))?
            .select_worker_program_for_test_emit(&checked)
            .is_err(),
        "the effectful selector must not accept a different host interface"
    );
    Ok(())
}

#[test]
fn worker_code_export_requires_exact_checked_effect_and_consumed_program_owner(
) -> Result<(), String> {
    use noble_contracts::component::{Type, World};
    use noble_contracts::source::{BoundOperation, ModuleSession};
    use noble_kernel::contracts::TEST_CODE_EXPORT;
    use noble_kernel::types::{EffSet, Ty};

    const WORKER_WIT: &[u8] = include_bytes!("../../noble-wasm/wit/worker-program.wit");
    let binding = BoundOperation {
        module_name: "ledger".into(),
        module_version: 1,
        operation: "test.code-export".into(),
        adapter_identity: "worker-test-code-export@1.0.0".into(),
        adapter_slot: 4,
        input: vec![Ty::Text, Ty::Text],
        output: vec![],
        effects: vec![TEST_CODE_EXPORT],
    };
    let session = ModuleSession::new(std::slice::from_ref(&binding), retained())
        .map_err(|error| format!("{error:?}"))?;
    let source = b"module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-capture\" \"local-destination\" code_export ] ]";
    let module = session
        .prepare(source, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let owner = module
        .checked_program_owner_for_binding("capture", &binding)
        .ok_or("missing checked private-capture Program")?;
    let (session, committed) = session.commit(module);
    committed.map_err(|error| format!("{error:?}"))?;
    let invocation = session
        .prepare(b"ledger@1.capture", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let checked = invocation
        .checked_program_interface_for_binding(&owner, &binding)
        .ok_or("missing exact checked code-export interface")?;
    assert!(world(WORKER_WIT, "guest")?
        .select_worker_program(&checked)
        .is_err());
    assert!(world(WORKER_WIT, "guest")?
        .select_worker_program_for_test_emit(&checked)
        .is_err());
    let selected = world(WORKER_WIT, "guest")?
        .select_worker_program_for_test_code_export(&checked)
        .map_err(|error| format!("{error:?}"))?;
    let kind = selected
        .resources()
        .first()
        .ok_or("missing WIT Program owner")?
        .kind;
    assert_eq!(
        selected.imports()[2].parameters,
        [Type::Own(kind), Type::String]
    );
    assert_eq!(selected.imports()[2].results, [Type::Boolean]);
    assert_eq!(
        selected.noble_type(Type::Own(kind)),
        Some(Ty::program(
            vec![Ty::I64],
            vec![Ty::I64],
            EffSet::from_ids(&[TEST_CODE_EXPORT])
        ))
    );
    assert!(
        selected
            .environment()
            .map_err(|error| format!("{error:?}"))?
            .knows_effect(TEST_CODE_EXPORT),
        "the selected latent effect must be known without granting a host import"
    );
    assert!(
        selected
            .check_import_effect(&selected.imports()[2].word, &[])
            .is_err(),
        "a checked effect witness never grants export destination/capture authority"
    );
    let guest = selected
        .prepare_export(
            "compute",
            b"programs.admit \"denied-destination\" programs.request-code-export drop",
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let expected = EffSet::from_ids(&[
        selected.imports()[0].effect.ok_or("missing admit effect")?,
        selected.imports()[2]
            .effect
            .ok_or("missing request effect")?,
    ]);
    assert_eq!(
        guest
            .submission()
            .ok_or("missing checked code-export guest")?
            .request
            .expected
            .allowed_effects,
        expected
    );
    let direct_run = selected
        .prepare_export("compute", b"programs.admit run", LIMITS)
        .err()
        .ok_or("a latent code-export effect escaped the WIT callable set")?;
    assert_eq!(direct_run.stage, noble_contracts::component::Stage::Export);
    assert!(
        selected
            .prepare_export(
                "compute",
                b"\"private\" \"denied-destination\" code_export",
                LIMITS
            )
            .is_err(),
        "the selected Program type is not a direct code-export import"
    );
    let changed = std::str::from_utf8(WORKER_WIT)
        .map_err(|error| error.to_string())?
        .replace(
            "request-code-export: func(owner: own<program>, destination: string) -> bool",
            "request-code-export: func(destination: string) -> bool",
        );
    assert!(
        World::parse(changed.as_bytes(), "guest", LIMITS)
            .map_err(|error| format!("{error:?}"))?
            .select_worker_program_for_test_code_export(&checked)
            .is_err(),
        "a string-only request cannot identify checked captured Program data"
    );
    Ok(())
}
