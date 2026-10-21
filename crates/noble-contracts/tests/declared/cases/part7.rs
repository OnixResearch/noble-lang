//! Legacy declared-module cases retained from the signed, code-export and
//! retained-source integration lineage.

#[test]
fn signed_pure_list_branch_with_dip_preserves_effect_ceiling() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    let session = super::super::commit(
        session,
        b"module calc_once@1 [ signature first forall<S:stack> [ S List<I64> -- S I64 ! pure ] export first def first [ 0 swap [ ] [ [ + ] dip drop ] list.case ] ]",
    )?;
    let call = session
        .prepare(b"3 nil cons calc_once@1.first", &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(call.output(), &[noble_kernel::types::Ty::I64]);

    let bound = super::super::binding(super::super::Binding {
        version: 1,
        slot: 3,
        adapter: "version-A",
    });
    let effectful = noble_contracts::source::ModuleSession::new(&[bound], super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    let accounted = b"module ledger@1 [ require emit Text -- ! test.emit signature first forall<S:stack> [ S List<I64> -- S I64 ! test.emit ] export first def first [ 0 swap [ ] [ [ \"event\" emit + ] dip drop ] list.case ] ]";
    effectful
        .prepare(accounted, &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let leaking = b"module ledger@1 [ require emit Text -- ! test.emit signature first forall<S:stack> [ S List<I64> -- S I64 ! pure ] export first def first [ 0 swap [ ] [ [ \"event\" emit + ] dip drop ] list.case ] ]";
    assert!(matches!(
        effectful.prepare(leaking, &[], super::super::LIMITS),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    Ok(())
}

#[test]
fn code_export_binding_checks_capture_destination_and_owner_effect() -> Result<(), String> {
    use noble_contracts::source::{BoundOperation, ModuleSession, Stage};
    use noble_kernel::contracts::TEST_CODE_EXPORT;
    use noble_kernel::types::Ty;
    let binding = BoundOperation {
        module_name: "ledger".into(),
        module_version: 1,
        operation: "test.code-export".into(),
        adapter_identity: "worker-test-code-export@1.0.0".into(),
        adapter_slot: 9,
        input: vec![Ty::Text, Ty::Text],
        output: vec![],
        effects: vec![TEST_CODE_EXPORT],
    };
    let source = b"module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-capture\" \"local-destination\" code_export ] ]";
    let session = ModuleSession::new(core::slice::from_ref(&binding), super::super::retained())
        .map_err(|e| format!("{e:?}"))?;
    let prepared = session
        .prepare(source, &[], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let owner = prepared
        .checked_program_owner_for_binding("capture", &binding)
        .ok_or("missing bound checked Program owner")?;
    let (session, linked) = session.commit(prepared);
    linked.map_err(|e| format!("{e:?}"))?;
    let invoked = session
        .prepare(b"ledger@1.capture", &[Ty::I64], super::super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let interface = invoked
        .checked_program_interface_for_binding(&owner, &binding)
        .ok_or("missing source-checked code-export interface")?;
    assert!(interface.is_test_code_export_i64_endomorphism());
    assert!(!interface.is_test_emit_i64_endomorphism());
    let mut wrong = binding.clone();
    wrong.input.pop();
    assert!(
        matches!(ModuleSession::new(&[wrong], super::super::retained()).and_then(|session|
        session.prepare(source, &[], super::super::LIMITS)), Err(error)
        if error.stage() == Stage::Link)
    );
    let mut wrong = binding;
    wrong.effects.clear();
    assert!(
        matches!(ModuleSession::new(&[wrong], super::super::retained()).and_then(|session|
        session.prepare(source, &[], super::super::LIMITS)), Err(error)
        if error.stage() == Stage::Link)
    );
    Ok(())
}

fn padded_retained_module(name: &str, value: i64, size: usize) -> Result<String, String> {
    let mut source = format!(
        "module {name}@1 [ opaque Item I64 public export Item export value def value [ {value} ] # "
    );
    let suffix = "\n ]";
    let padding = size
        .checked_sub(source.len())
        .and_then(|rest| rest.checked_sub(suffix.len()))
        .ok_or_else(|| "module source padding exceeds the requested length".to_string())?;
    source.extend(std::iter::repeat_n('x', padding));
    source.push_str(suffix);
    Ok(source)
}

#[test]
fn explicitly_retained_modules_admit_exact_boundary_and_refuse_one_extra_byte_atomically(
) -> Result<(), String> {
    use noble_contracts::source::{ModuleSession, RetainedSourceLimit, Stage};

    let max = usize::try_from(RetainedSourceLimit::MAX_BYTES)
        .map_err(|_| "retained source bound exceeds address space".to_string())?;
    let per_input = usize::try_from(super::super::LIMITS.bytes)
        .map_err(|_| "per-input bound exceeds address space".to_string())?;
    let historical = RetainedSourceLimit::new(super::super::LIMITS.bytes)
        .ok_or("missing historical source quota")?;
    let opted_in = RetainedSourceLimit::new(RetainedSourceLimit::MAX_BYTES)
        .ok_or("missing opt-in retained source quota")?;
    let first = padded_retained_module("first", 1, per_input)?;
    let second = padded_retained_module("second", 2, per_input)?;
    let third = padded_retained_module("third", 3, per_input)?;
    let fourth = padded_retained_module("fourth", 4, per_input)?;
    let fifth = b"module fifth@1 [ export value def value [ 5 ] ]";
    let short_fourth = padded_retained_module("fourth", 4, per_input + 1 - fifth.len())?;

    let historical_session =
        ModuleSession::new(&[], historical).map_err(|error| format!("{error:?}"))?;
    let historical_session = super::super::commit(historical_session, first.as_bytes())?;
    let old_generation = historical_session.generation();
    let old_refusal = historical_session.prepare(second.as_bytes(), &[], super::super::LIMITS);
    assert!(matches!(old_refusal, Err(error) if error.stage() == Stage::Check));
    assert_eq!(historical_session.generation(), old_generation);

    let mut session = ModuleSession::new(&[], opted_in).map_err(|error| format!("{error:?}"))?;
    let mut oversized = first.clone();
    oversized.push(' ');
    assert_eq!(oversized.len(), per_input + 1);
    assert!(
        matches!(session.prepare(oversized.as_bytes(), &[], super::super::LIMITS),
        Err(error) if error.stage() == Stage::Parse)
    );
    let oversized_host_budget = noble_contracts::Limits {
        bytes: RetainedSourceLimit::MAX_BYTES,
        ..super::super::LIMITS
    };
    assert!(
        matches!(session.prepare(oversized.as_bytes(), &[], oversized_host_budget),
        Err(error) if error.stage() == Stage::Parse)
    );
    assert_eq!(session.generation(), 0);
    for module in [&first, &second, &third] {
        session = super::super::commit(session, module.as_bytes())?;
    }
    let original_type = session
        .resolve_type("first@1.Item")
        .map_err(|error| format!("{error:?}"))?;
    let exact = super::super::commit(session.snapshot(), fourth.as_bytes())?;
    assert_eq!(3 * per_input + fourth.len(), max);
    assert_eq!(exact.generation(), 4);
    assert_eq!(
        exact
            .resolve_type("first@1.Item")
            .map_err(|error| format!("{error:?}"))?,
        original_type
    );

    session = super::super::commit(session, short_fourth.as_bytes())?;
    assert_eq!(3 * per_input + short_fourth.len() + fifth.len(), max + 1);
    let generation = session.generation();
    let refusal = session.prepare(fifth, &[], super::super::LIMITS);
    assert!(
        matches!(refusal, Err(error) if error.stage() == Stage::Check
        && error.diagnostic().message.contains("retained module and definition byte limit"))
    );
    assert_eq!(session.generation(), generation);
    assert_eq!(
        session
            .resolve_type("first@1.Item")
            .map_err(|error| format!("{error:?}"))?,
        original_type
    );
    let usable = session
        .prepare(b"first@1.value", &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(usable.output(), &[noble_kernel::types::Ty::I64]);
    assert!(usable.submission().is_some());
    Ok(())
}

#[test]
fn transitive_alias_rebind_charges_each_exact_outside_source_and_header() -> Result<(), String> {
    use noble_contracts::source::{ModuleSession, RetainedSourceLimit, Stage};

    let module_one = padded_retained_module("m", 1, 65_536)?;
    let module_two =
        b"module m@2 [ opaque Item I64 public export Item export value def value [ 2 ] ]";
    let once = b"def once [ m.value ]";
    let twice = b"def twice [ once once ]";
    let history_per_generation = once.len() + 4 + twice.len() + 4;
    let exact = module_one.len() + module_two.len() + 2 * history_per_generation;
    let exact = u32::try_from(exact).map_err(|_| "retained quota exceeds u32".to_string())?;
    assert!(exact > super::super::LIMITS.bytes && exact < RetainedSourceLimit::MAX_BYTES);

    for allowance in [exact - 1, exact] {
        let quota = RetainedSourceLimit::new(allowance).ok_or("invalid retained quota")?;
        let mut session = ModuleSession::new_declared_text(&[], false, quota)
            .map_err(|error| format!("{error:?}"))?;
        session = super::super::commit(session, module_one.as_bytes())?;
        session = super::super::commit(session, module_two)?;
        session = super::super::commit(session, b"import m@1 as m")?;
        session = super::super::commit(session, once)?;
        session = super::super::commit(session, twice)?;
        let old_owner = session
            .resolve_type("m.Item")
            .map_err(|error| format!("{error:?}"))?;
        let generation = session.generation();
        let rebind = session.prepare(b"import m@2 as m", &[], super::super::LIMITS);
        if allowance < exact {
            assert!(matches!(rebind, Err(error) if error.stage() == Stage::Check
                && error.diagnostic().message.contains("retained module and definition byte limit")));
            assert_eq!(session.generation(), generation);
            assert_eq!(
                session
                    .resolve_type("m.Item")
                    .map_err(|error| format!("{error:?}"))?,
                old_owner
            );
            assert_eq!(
                session
                    .prepare(b"once twice", &[], super::super::LIMITS)
                    .map_err(|error| format!("{error:?}"))?
                    .output(),
                &[
                    noble_kernel::types::Ty::I64,
                    noble_kernel::types::Ty::I64,
                    noble_kernel::types::Ty::I64
                ]
            );
        } else {
            let rebind = rebind.map_err(|error| format!("{error:?}"))?;
            let rebuilt = rebind
                .rebuilt_import_definitions()
                .ok_or("missing checked transitive replay")?;
            assert_eq!(
                rebuilt.iter().map(|entry| entry.name()).collect::<Vec<_>>(),
                vec!["once", "twice"]
            );
            let (next, outcome) = session.commit(rebind);
            outcome.map_err(|error| format!("{error:?}"))?;
            assert_eq!(next.generation(), generation + 1);
            assert_ne!(
                next.resolve_type("m.Item")
                    .map_err(|error| format!("{error:?}"))?,
                old_owner
            );
        }
    }
    Ok(())
}
