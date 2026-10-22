#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::compound_assertion,
    tigerstyle::excessive_file_length,
    tigerstyle::function_length,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
#[path = "declared/bindings.rs"]
mod bindings;
#[path = "declared/cases.rs"]
mod cases;
#[path = "declared/named.rs"]
mod named;
#[path = "declared/origin.rs"]
mod origin;

#[path = "declared/signatures.rs"]
mod signatures;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn retained() -> noble_contracts::source::RetainedSourceLimit {
    noble_contracts::source::RetainedSourceLimit::new(LIMITS.bytes)
        .expect("declared test source limit")
}

struct Binding<'a> {
    version: u32,
    slot: u32,
    adapter: &'a str,
}

fn binding(spec: Binding<'_>) -> noble_contracts::source::BoundOperation {
    noble_contracts::source::BoundOperation {
        module_name: "ledger".into(),
        module_version: spec.version,
        operation: "test.emit".into(),
        adapter_identity: spec.adapter.into(),
        adapter_slot: spec.slot,
        input: vec![noble_kernel::types::Ty::Text],
        output: vec![],
        effects: vec![noble_kernel::contracts::TEST_EMIT],
    }
}

fn commit(
    session: noble_contracts::source::ModuleSession,
    text: &[u8],
) -> Result<noble_contracts::source::ModuleSession, String> {
    let prepared = session
        .prepare(text, &[], LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let (session, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("{error:?}"))?;
    Ok(session)
}

#[test]
fn calculator_retains_ten_individually_bounded_module_sources() -> Result<(), String> {
    let limits = noble_contracts::Limits {
        bytes: 262_144,
        ..LIMITS
    };
    let mut calculator = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    // DX-MODULE-05: an ordinary declared input stays within 65,536 bytes; its
    // aggregate retention is the explicit opt-in maximum, 262,144 bytes.
    let ordinary_limits = noble_contracts::Limits {
        bytes: 65_536,
        ..LIMITS
    };
    let mut ordinary = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(
            noble_contracts::source::RetainedSourceLimit::MAX_BYTES,
        )
        .ok_or("maximum retained source limit")?,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut aggregate = 0usize;
    for index in 0..10 {
        let source = format!(
            "module calc{index}@1 [ export value def value [ {index} ] ] #{}",
            "x".repeat(26_700)
        );
        aggregate += source.len();
        assert!(source.len() < usize::try_from(limits.bytes).map_err(|e| e.to_string())?);
        let prepared = calculator
            .prepare(source.as_bytes(), &[], limits)
            .map_err(|error| format!("{error:?}"))?;
        let (next, outcome) = calculator.commit(prepared);
        outcome.map_err(|error| format!("{error:?}"))?;
        calculator = next;
        if index < 9 {
            let prepared = ordinary
                .prepare(source.as_bytes(), &[], ordinary_limits)
                .map_err(|error| format!("{error:?}"))?;
            let (next, outcome) = ordinary.commit(prepared);
            outcome.map_err(|error| format!("{error:?}"))?;
            ordinary = next;
        } else {
            assert!(matches!(
                ordinary.prepare(source.as_bytes(), &[], ordinary_limits),
                Err(error) if error.stage() == noble_contracts::source::Stage::Check
            ));
        }
    }
    assert!(aggregate > 262_144);
    assert!(aggregate < 524_288);
    assert_eq!(calculator.generation(), 10);
    assert_eq!(ordinary.generation(), 9);
    Ok(())
}

#[test]
fn calculator_aggregate_preflight_preserves_candidate_byte_cap() -> Result<(), String> {
    let limits = noble_contracts::Limits {
        bytes: 262_144,
        ..LIMITS
    };
    let mut session = noble_contracts::source::ModuleSession::new_calculator()
        .map_err(|error| format!("{error:?}"))?;
    for index in 0..2 {
        let source = format!(
            "module retained{index}@1 [ export value def value [ {index} ] ] #{}",
            "x".repeat(220_000)
        );
        let prepared = session
            .prepare(source.as_bytes(), &[], limits)
            .map_err(|error| format!("{error:?}"))?;
        let (next, outcome) = session.commit(prepared);
        outcome.map_err(|error| format!("{error:?}"))?;
        session = next;
    }
    let generation = session.generation();
    // The missing closing bracket would fail parsing; the aggregate must refuse
    // this module before parsing or cloning the retained sources.
    let unparseable = format!("module too_large@1 [ #{}", "x".repeat(100_000));
    assert!(unparseable.len() < 262_144);
    assert!(matches!(
        session.prepare(unparseable.as_bytes(), &[], limits),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    assert!(matches!(
        session.prepare(&vec![b'x'; 262_145], &[], limits),
        Err(error) if error.stage() == noble_contracts::source::Stage::Parse
    ));
    assert_eq!(session.generation(), generation);
    Ok(())
}

fn assert_pure_candidate(session: &noble_contracts::source::ModuleSession) -> Result<(), String> {
    let prepared = session
        .prepare(b"1", &[], LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert!(
        prepared
            .submission()
            .ok_or("pure submission missing")?
            .environment
            .declared_modules
    );
    Ok(())
}

#[test]
fn worker_getters_expose_closed_checked_owner_not_source_guessed_rows() -> Result<(), String> {
    use noble_contracts::source::{pure_builtin_spelling, ModuleSession, ProgramClosureError};
    assert_eq!(pure_builtin_spelling(0), Some("dup"));
    assert_eq!(pure_builtin_spelling(21), Some("list.case"));
    for forbidden in [22, 23, 26, 27] {
        assert_eq!(pure_builtin_spelling(forbidden), None);
    }
    let session = ModuleSession::new_declared_text(&[], false, retained())
        .map_err(|error| format!("{error:?}"))?;
    let package = b"module package@1 [ variant Choice Good I64 public Bad Text public export Choice export Choice.Good export helper def helper [ 1 + ] export root def root [ let value [ value helper ] run ] ]";
    let prepared = session
        .prepare(package, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let owner = prepared
        .resolved_module()
        .ok_or("new package lost resolved module")?;
    let nominal = prepared
        .checked_nominals()
        .ok_or("new package lost checked nominals")?
        .collect::<Vec<_>>();
    assert_eq!(nominal.len(), 1);
    assert_eq!(nominal[0].module_identity(), owner.0);
    assert_eq!(nominal[0].module_version(), 1);
    assert_eq!(nominal[0].declaration().id.module, owner.0);
    assert!(prepared.resolve_type("package@1.Choice").is_ok());
    assert!(prepared.resolve_type("package@2.Choice").is_err());
    let root = prepared
        .checked_program_owner("root")
        .ok_or("exported root lost source identity")?;
    assert_eq!(root.module_identity(), owner.0);
    assert_eq!(root.module_version(), 1);
    assert_eq!(root.definition_ordinal(), 1);
    assert!(prepared.checked_program_owner("missing").is_none());
    let closure = prepared
        .checked_program_closure("root")
        .map_err(|error| format!("closed package refused: {error:?}"))?;
    assert_eq!(
        closure
            .iter()
            .map(|entry| entry.definition_ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(closure[1].definition_identity(), root.definition_identity());
    assert_eq!(closure[1].definition_slot(), root.definition_slot());
    assert_ne!(closure[0].definition_slot(), closure[1].definition_slot());
    let decoy = b"module decoy@1 [ export helper def helper [ 1 + ] export root def root [ 1 helper ] def unused [ 0 ] ]";
    let decoy = session
        .prepare(decoy, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(decoy.checked_program_owner("root").is_some());
    assert!(
        decoy.checked_program_closure("root").err() == Some(ProgramClosureError::Invalid),
        "an uncalled source definition cannot enter a closed worker package"
    );
    let external = commit(
        session,
        b"module external@1 [ export bump def bump [ 1 + ] ]",
    )?;
    let foreign = external
        .prepare(
            b"module foreign@1 [ export root def root [ 1 external@1.bump ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(foreign.checked_program_owner("root").is_some());
    assert!(
        foreign.checked_program_closure("root").err() == Some(ProgramClosureError::Invalid),
        "a foreign checked source owner is not inside this worker package"
    );
    let signed = ModuleSession::new_declared_text(&[], true, retained())
        .map_err(|error| format!("{error:?}"))?;
    let unchecked_self = signed.prepare(
        b"module spin@1 [ signature forever forall<S:stack> [ S -- S ! pure ] export forever def forever [ forever ] ]",
        &[], LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert!(unchecked_self.checked_program_owner("forever").is_none());
    assert!(
        unchecked_self.checked_program_closure("forever").err()
            == Some(ProgramClosureError::Invalid),
        "an unverified signed self cycle cannot enter a closed worker package"
    );
    let bound = ModuleSession::new(
        &[binding(Binding {
            version: 1,
            slot: 4,
            adapter: "version-A",
        })],
        retained(),
    )
    .map_err(|error| format!("{error:?}"))?;
    let host_module = bound.prepare(
        b"module ledger@1 [ opaque UserId I64 private require emit Text -- ! test.emit export UserId export handle def handle [ \"audit\" emit ] ]",
        &[], LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert!(host_module.linked_binding().is_some());
    assert!(host_module.resolve_type("ledger@1.UserId").is_err());
    assert!(host_module.checked_nominals().is_none());
    assert!(host_module.checked_program_owner("handle").is_none());
    assert!(
        host_module.checked_program_closure("handle").err() == Some(ProgramClosureError::Invalid),
        "an independently linked host adapter must not enter a pure worker package"
    );
    Ok(())
}

#[test]
fn bound_program_provenance_requires_exact_host_selection_and_invocation() -> Result<(), String> {
    use noble_contracts::source::{BoundOperation, ModuleSession, ProgramClosureError};
    use noble_kernel::types::{EffSet, Ty};

    let selected = binding(Binding {
        version: 1,
        slot: 4,
        adapter: "version-A",
    });
    let session = ModuleSession::new(core::slice::from_ref(&selected), retained())
        .map_err(|error| format!("{error:?}"))?;
    let source = b"module ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
    let module = session
        .prepare(source, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(module.checked_program_owner("plus").is_none());
    assert_eq!(
        module.checked_program_closure("plus").err(),
        Some(ProgramClosureError::Invalid)
    );
    let owner = module
        .checked_program_owner_for_binding("plus", &selected)
        .ok_or("bound export lost checked source owner")?;
    let closure = module
        .checked_program_closure_for_binding("plus", &selected)
        .map_err(|error| format!("bound export lost closed source graph: {error:?}"))?;
    assert_eq!(
        closure
            .iter()
            .map(|entry| entry.definition_slot())
            .collect::<Vec<_>>(),
        vec![owner.definition_slot()]
    );

    let altered = [
        BoundOperation {
            module_name: "other".into(),
            ..selected.clone()
        },
        BoundOperation {
            module_version: 2,
            ..selected.clone()
        },
        BoundOperation {
            operation: "test.clock".into(),
            ..selected.clone()
        },
        BoundOperation {
            adapter_identity: "version-B".into(),
            ..selected.clone()
        },
        BoundOperation {
            adapter_slot: 5,
            ..selected.clone()
        },
        BoundOperation {
            input: vec![Ty::I64],
            ..selected.clone()
        },
        BoundOperation {
            output: vec![Ty::I64],
            ..selected.clone()
        },
        BoundOperation {
            effects: vec![],
            ..selected.clone()
        },
    ];
    for wrong in &altered {
        assert!(module
            .checked_program_owner_for_binding("plus", wrong)
            .is_none());
        assert_eq!(
            module
                .checked_program_closure_for_binding("plus", wrong)
                .err(),
            Some(ProgramClosureError::Invalid)
        );
    }
    let extra = binding(Binding {
        version: 2,
        slot: 5,
        adapter: "version-B",
    });
    let multiple = ModuleSession::new(&[selected.clone(), extra], retained())
        .map_err(|error| format!("{error:?}"))?;
    let multiple_module = multiple
        .prepare(source, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(multiple_module
        .checked_program_owner_for_binding("plus", &selected)
        .is_none());
    assert!(
        multiple_module
            .checked_program_closure_for_binding("plus", &selected)
            .err()
            == Some(ProgramClosureError::Invalid),
        "one matching row among several staged bindings is not an exact host selection"
    );
    let clock = BoundOperation {
        operation: "test.clock".into(),
        input: vec![],
        output: vec![Ty::I64],
        effects: vec![noble_kernel::contracts::TEST_CLOCK],
        ..selected.clone()
    };
    let clock_session =
        ModuleSession::new(&[clock], retained()).map_err(|error| format!("{error:?}"))?;
    let clock_staged = clock_session
        .prepare(
            b"module ledger@1 [ export plus def plus [ 1 + ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(clock_staged.checked_program_owner("plus").is_none());
    assert!(
        clock_staged.checked_program_closure("plus").err() == Some(ProgramClosureError::Invalid),
        "pure getters must reject even an unused non-emit host binding"
    );
    let inert = session
        .prepare(
            b"module ledger@1 [ require emit Text -- ! test.emit export plus def plus [ 1 + ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        inert
            .checked_program_owner_for_binding("plus", &selected)
            .is_none(),
        "declaring a bound operation without invoking it is not an effectful Program"
    );
    let partial = session.prepare(
        b"module ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] def unused [ 0 ] ]",
        &[], LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert!(
        partial
            .checked_program_closure_for_binding("plus", &selected)
            .err()
            == Some(ProgramClosureError::Invalid),
        "an unused source definition cannot enter a bound Worker package"
    );

    let (session, committed) = session.commit(module);
    committed.map_err(|error| format!("{error:?}"))?;
    let invocation = session
        .prepare(b"ledger@1.plus", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(invocation.checked_program_interface(&owner).is_none());
    let interface = invocation
        .checked_program_interface_for_binding(&owner, &selected)
        .ok_or("exact bound invocation lost checked interface")?;
    assert!(interface.is_test_emit_i64_endomorphism());
    assert!(!interface.is_pure_i64_endomorphism());
    assert_eq!(
        interface.ty(),
        Ty::program(
            vec![Ty::I64],
            vec![Ty::I64],
            EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT])
        )
    );
    for wrong in &altered {
        assert!(invocation
            .checked_program_interface_for_binding(&owner, wrong)
            .is_none());
    }
    let extended = session
        .prepare(b"ledger@1.plus 1 drop", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        extended
            .checked_program_interface_for_binding(&owner, &selected)
            .is_none(),
        "an expression extending the invocation is not the named bound Program"
    );
    Ok(())
}

#[test]
fn link_rejects_bad_contract_and_ambient_emit() -> Result<(), String> {
    let source=b"module ledger@1 [ require emit Text -- ! test.emit export handle def handle [ \"audit\" emit ] ]";
    let missing = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(&missing.prepare(source,&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    let unused = noble_contracts::source::ModuleSession::new(
        &[binding(Binding {
            version: 1,
            slot: 9,
            adapter: "unused",
        })],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|e| format!("{e:?}"))?;
    let inert = unused
        .prepare(
            b"module ledger@1 [ export handle def handle [ 1 ] ]",
            &[],
            LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert!(inert.linked_binding().is_none());
    let mut wrong = binding(Binding {
        version: 1,
        slot: 1,
        adapter: "wrong-input",
    });
    wrong.input = vec![noble_kernel::types::Ty::I64];
    let session = noble_contracts::source::ModuleSession::new(
        &[wrong],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(&session.prepare(source,&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    let mut wrong = binding(Binding {
        version: 1,
        slot: 2,
        adapter: "wrong-effect",
    });
    wrong.effects.clear();
    let session = noble_contracts::source::ModuleSession::new(
        &[wrong],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(&session.prepare(source,&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    Ok(())
}

#[test]
fn invalid_initializers_and_imports_leave_generation_unchanged() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|e| format!("{e:?}"))?;
    let generation = session.generation();
    assert!(
        matches!(&session.prepare(b"import ledger@99 as missing",&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    assert!(session
        .prepare(
            b"module ledger@1 [ export handle def handle [ \"audit\" test.emit ] ]",
            &[],
            LIMITS
        )
        .is_err());
    assert!(session
        .prepare(b"module ledger@1 [ \"initializer\" ]", &[], LIMITS)
        .is_err());
    assert!(session
        .prepare(b"module ledger@1 [ export missing ]", &[], LIMITS)
        .is_err());
    assert!(session
        .prepare("module café@1 [ ]".as_bytes(), &[], LIMITS)
        .is_err());
    assert!(session
        .prepare(b"\"audit\" test.emit", &[], LIMITS)
        .is_err());
    assert_pure_candidate(&session)?;
    assert_eq!(session.generation(), generation);
    Ok(())
}

// r[verify DX-DIAG-01]
#[test]
fn declared_source_utf8_errors_locate_bad_bytes_and_preserve_valid_text() -> Result<(), String> {
    use noble_contracts::source::{ModuleSession, Stage};

    let session = ModuleSession::new_declared_text(&[], false, retained())
        .map_err(|error| format!("{error:?}"))?;
    let generation = session.generation();
    for (source, end) in [(b"1 \xff 2".as_slice(), 3), (b"1 \xe2\x82".as_slice(), 4)] {
        let error = session
            .prepare(source, &[], LIMITS)
            .expect_err("invalid UTF-8 must not produce a declared candidate");
        assert_eq!(error.stage(), Stage::Parse);
        assert_eq!(
            error.diagnostic().kind,
            noble_contracts::DiagnosticKind::Invalid
        );
        assert_eq!(
            error.diagnostic().span,
            noble_contracts::Span { start: 2, end },
        );
    }
    assert_eq!(session.generation(), generation);

    let accepted = session
        .prepare(b"\"A\xc3\xa9\" 1", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        accepted.output(),
        &[noble_kernel::types::Ty::Text, noble_kernel::types::Ty::I64],
    );
    let text = accepted
        .submission()
        .ok_or("valid text expression has no submission")?
        .body
        .texts
        .first()
        .ok_or("valid text literal was not retained")?;
    assert_eq!(text.bytes, b"A\xc3\xa9");
    Ok(())
}

#[test]
fn incompatible_alias_rebind_and_stale_commit_keep_previous_candidate() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|e| format!("{e:?}"))?;
    session = commit(session, b"module ledger@1 [ export get def get [ 10 ] ]")?;
    session = commit(session, b"module ledger@2 [ export get def get [ true ] ]")?;
    session = commit(session, b"import ledger@1 as account")?;
    let old = session
        .prepare(b"account.get", &[], LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(old.output(), &[noble_kernel::types::Ty::I64]);
    session = commit(session, b"def saved [ account.get ]")?;
    let stale = session
        .prepare(b"import ledger@1 as account", &[], LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert!(stale.rebuilt_import_definitions().is_none());
    let incompatible = session
        .prepare(b"import ledger@2 as account", &[], LIMITS)
        .err()
        .ok_or("incompatible dependent alias rebind was accepted")?;
    assert!(matches!(
        incompatible.stage(),
        noble_contracts::source::Stage::Check | noble_contracts::source::Stage::Acceptance
    ));
    let retained = session
        .prepare(b"saved account.get", &[], LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        retained.output(),
        &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64]
    );
    assert_eq!(old.output(), &[noble_kernel::types::Ty::I64]);
    session = commit(session, b"1")?;
    let (session, outcome) = session.commit(stale);
    let error = outcome.err().ok_or("accepted stale module import")?;
    assert_eq!(error.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(
        session
            .prepare(b"account.get", &[], LIMITS)
            .map_err(|e| format!("{e:?}"))?
            .output(),
        &[noble_kernel::types::Ty::I64]
    );
    Ok(())
}

#[test]
fn alias_rebind_replays_exact_current_dependents_but_preserves_versioned_owners(
) -> Result<(), String> {
    use noble_contracts::source::ModuleSession;
    let mut session = ModuleSession::new_declared_text(
        &[],
        false,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| format!("{error:?}"))?;
    session = commit(session, b"module m@1 [ export addone def addone [ 1 + ] ]")?;
    session = commit(session, b"module m@2 [ export addone def addone [ 2 + ] ]")?;
    let first = session
        .prepare(b"import m@1 as m", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(first.resolved_import_alias(), Some("m"));
    assert!(first.rebuilt_import_definitions().is_none());
    let (next, outcome) = session.commit(first);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    session = commit(session, b"import m@1 as other")?;
    session = commit(session, b"def pinned [ m@1.addone ]")?;
    session = commit(session, b"def once [ let x x m.addone ]")?;
    session = commit(session, b"def twice [ once once ]")?;
    let captured = session
        .prepare(b"[ once ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(captured.submission().is_some());
    let before = session.generation();
    let rebound = session
        .prepare(b"import m@2 as m", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(rebound.resolved_import_alias(), Some("m"));
    assert_eq!(rebound.resolved_module().map(|owner| owner.2), Some(2));
    let rebuilt = rebound
        .rebuilt_import_definitions()
        .ok_or("missing checked replay")?;
    assert_eq!(
        rebuilt.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
        vec!["once", "twice"]
    );
    assert!(rebuilt
        .iter()
        .all(|entry| entry.previous_identity() != entry.new_identity()));
    let (next, outcome) = session.commit(rebound);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    assert_eq!(session.generation(), before + 1);
    assert_eq!(
        session
            .prepare(b"20 twice 20 pinned 20 other.addone", &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?
            .output(),
        &[
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::I64
        ]
    );

    let untouched = ModuleSession::new_declared_text(
        &[],
        false,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| format!("{error:?}"))?;
    let untouched = commit(
        untouched,
        b"module m@1 [ export addone def addone [ 1 + ] ]",
    )?;
    let untouched = commit(
        untouched,
        b"module m@2 [ export addone def addone [ 2 + ] ]",
    )?;
    let untouched = commit(untouched, b"import m@1 as m")?;
    let untouched = commit(untouched, b"def pinned [ m@1.addone ]")?;
    let rebind = untouched
        .prepare(b"import m@2 as m", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        rebind.rebuilt_import_definitions().map(|items| items.len()),
        Some(0),
        "a version-pinned callee is not an outside alias dependent"
    );
    Ok(())
}

#[test]
fn failed_registration_and_alias_rebind_never_reissue_a_module_owner() -> Result<(), String> {
    use noble_contracts::source::{ModuleSession, RetainedSourceLimit};
    let retained = RetainedSourceLimit::new(65_536).ok_or("invalid retained source limit")?;
    let mut session = ModuleSession::new_declared_text(&[], false, retained)
        .map_err(|error| format!("{error:?}"))?;
    let first = session
        .prepare(
            b"module m@1 [ export addone def addone [ 1 + ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let first_owner = first
        .resolved_module()
        .ok_or("first module lost checked owner")?
        .0;
    let (next, outcome) = session.commit(first);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    session = commit(session, b"import m@1 as m")?;
    assert!(session
        .prepare(
            b"module m@2 [ export addone def addone [ true + ] ]",
            &[],
            LIMITS
        )
        .is_err());

    let second = session
        .prepare(
            b"module m@2 [ export addone def addone [ 2 + ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let second_owner = second
        .resolved_module()
        .ok_or("second module lost checked owner")?
        .0;
    assert!(second_owner > first_owner);
    let (next, outcome) = session.commit(second);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    session = commit(session, b"def once [ m.addone ]")?;
    session = commit(session, b"def twice [ once once ]")?;
    session = commit(session, b"import m@2 as m")?;

    let third = session
        .prepare(
            b"module m@3 [ export addone def addone [ 3 + ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let third_owner = third
        .resolved_module()
        .ok_or("third module lost checked owner")?
        .0;
    assert!(third_owner > second_owner && third_owner != first_owner);
    let (next, outcome) = session.commit(third);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    session = commit(session, b"import m@3 as m")?;
    assert_eq!(
        session
            .prepare(b"20 twice", &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?
            .output(),
        &[noble_kernel::types::Ty::I64]
    );
    Ok(())
}

#[test]
fn alias_rebind_refuses_a_current_body_capturing_a_shadowed_old_alias_owner() -> Result<(), String>
{
    let session = noble_contracts::source::ModuleSession::new_declared_text(
        &[],
        false,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| format!("{error:?}"))?;
    let session = commit(session, b"module m@1 [ export addone def addone [ 1 + ] ]")?;
    let session = commit(session, b"module m@2 [ export addone def addone [ 2 + ] ]")?;
    let session = commit(session, b"import m@1 as m")?;
    let session = commit(session, b"def once [ m.addone ]")?;
    let session = commit(session, b"def twice [ once once ]")?;
    let session = commit(session, b"def once [ 2 + ]")?;
    let before = session.generation();
    let error = session
        .prepare(b"import m@2 as m", &[], LIMITS)
        .err()
        .ok_or("accepted stale call to a shadowed alias-dependent owner")?;
    assert_eq!(error.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(session.generation(), before);
    assert_eq!(
        session
            .prepare(b"20 twice", &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?
            .output(),
        &[noble_kernel::types::Ty::I64]
    );
    Ok(())
}

#[test]
fn alias_rebind_refuses_a_new_host_effect_above_the_previous_ceiling() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(
        &[binding(Binding {
            version: 2,
            slot: 7,
            adapter: "ledger-v2-emit",
        })],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| format!("{error:?}"))?;
    let session = commit(
        session,
        b"module ledger@1 [ export touch def touch [ 1 drop ] ]",
    )?;
    let session = commit(session, b"module ledger@2 [ require emit Text -- ! test.emit export touch def touch [ \"event\" emit ] ]")?;
    let session = commit(session, b"import ledger@1 as account")?;
    let session = commit(session, b"def once [ account.touch ]")?;
    let before = session.generation();
    let error = session
        .prepare(b"import ledger@2 as account", &[], LIMITS)
        .err()
        .ok_or("accepted an effectful replacement of a pure dependent")?;
    assert!(matches!(
        error.stage(),
        noble_contracts::source::Stage::Check | noble_contracts::source::Stage::Acceptance
    ));
    assert_eq!(session.generation(), before);
    Ok(())
}

#[test]
fn signed_lexical_owner_rederives_its_checked_tail_from_original_source() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new_declared_text(
        &[],
        true,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| format!("{error:?}"))?;
    let session = commit(session,
        b"module countdown@1 [ signature go forall<S:stack> [ S I64 -- S I64 ! pure ] export go def go [ dup [ let n n 0 = ] dip swap [ ] [ 1 - go ] if ] ]")?;
    let prepared = session
        .prepare(b"3 countdown@1.go", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(prepared.output(), &[noble_kernel::types::Ty::I64]);
    let rederived = prepared
        .signed_source_owner()
        .ok_or("signed lexical source owner missing")?
        .rederive()
        .map_err(|error| format!("{error:?}"))?;
    assert!(rederived.environment.declared_modules);
    Ok(())
}

#[test]
fn nominal_resource_fixture_rejects_dup_drop_and_capture() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|e| format!("{e:?}"))?;
    session = commit(session,b"module ledger@1 [ opaque CounterOwner Pair<I64,Resource<test.counter>> private variant State Ready I64 public Busy List<Resource<test.counter>> private export CounterOwner export State ]")?;
    for spelling in ["ledger@1.CounterOwner", "ledger@1.State"] {
        let ty = session
            .resolve_type(spelling)
            .map_err(|e| format!("{e:?}"))?;
        for word in [b"dup".as_slice(), b"drop", b"quote"] {
            let result = session.prepare(word, core::slice::from_ref(&ty), LIMITS);
            assert!(
                matches!(&result,Err(e) if e.stage()==noble_contracts::source::Stage::Acceptance),
                "unexpected fixture outcome: {result:?}"
            );
        }
    }
    Ok(())
}
#[path = "declared/owners.rs"]
mod owners;
