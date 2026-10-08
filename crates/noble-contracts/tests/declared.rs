#[path = "declared/bindings.rs"]
mod bindings;
#[path = "declared/cases.rs"]
mod cases;
#[path = "declared/named.rs"]
mod named;
#[path = "declared/origin.rs"]
mod origin;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

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
    use noble_contracts::source::{pure_builtin_spelling, ModuleSession};
    assert_eq!(pure_builtin_spelling(0), Some("dup"));
    assert_eq!(pure_builtin_spelling(21), Some("list.case"));
    for forbidden in [22, 23, 26, 27] {
        assert_eq!(pure_builtin_spelling(forbidden), None);
    }
    let session = ModuleSession::new_declared_text(&[], false)
        .map_err(|error| format!("{error:?}"))?;
    let package = b"module package@1 [ variant Choice Good I64 public Bad Text public export Choice export Choice.Good export helper def helper [ 1 + ] export root def root [ let value [ value helper ] run ] ]";
    let prepared = session.prepare(package, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let owner = prepared.resolved_module().ok_or("new package lost resolved module")?;
    let nominal = prepared.checked_nominals().ok_or("new package lost checked nominals")?
        .collect::<Vec<_>>();
    assert_eq!(nominal.len(), 1);
    assert_eq!(nominal[0].module_identity(), owner.0);
    assert_eq!(nominal[0].module_version(), 1);
    assert_eq!(nominal[0].declaration().id.module, owner.0);
    assert!(prepared.resolve_type("package@1.Choice").is_ok());
    assert!(prepared.resolve_type("package@2.Choice").is_err());
    let root = prepared.checked_program_owner("root").ok_or("exported root lost source identity")?;
    assert_eq!(root.module_identity(), owner.0);
    assert_eq!(root.module_version(), 1);
    assert_eq!(root.definition_ordinal(), 1);
    assert!(prepared.checked_program_owner("missing").is_none());
    let closure = prepared.checked_program_closure("root").ok_or("closed package refused")?;
    assert_eq!(closure.iter().map(|entry| entry.definition_ordinal()).collect::<Vec<_>>(),
        vec![0, 1]);
    assert_eq!(closure[1].definition_identity(), root.definition_identity());
    assert_eq!(closure[1].definition_slot(), root.definition_slot());
    assert_ne!(closure[0].definition_slot(), closure[1].definition_slot());
    let decoy = b"module decoy@1 [ export helper def helper [ 1 + ] export root def root [ 1 helper ] def unused [ 0 ] ]";
    let decoy = session.prepare(decoy, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(decoy.checked_program_owner("root").is_some());
    assert!(decoy.checked_program_closure("root").is_none(),
        "an uncalled source definition cannot enter a closed worker package");
    let external = commit(session,
        b"module external@1 [ export bump def bump [ 1 + ] ]")?;
    let foreign = external.prepare(
        b"module foreign@1 [ export root def root [ 1 external@1.bump ] ]",
        &[], LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert!(foreign.checked_program_owner("root").is_some());
    assert!(foreign.checked_program_closure("root").is_none(),
        "a foreign checked source owner is not inside this worker package");
    let signed = ModuleSession::new_declared_text(&[], true)
        .map_err(|error| format!("{error:?}"))?;
    let unchecked_self = signed.prepare(
        b"module spin@1 [ signature forever forall<S:stack> [ S -- S ! pure ] export forever def forever [ forever ] ]",
        &[], LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert!(unchecked_self.checked_program_owner("forever").is_none());
    assert!(unchecked_self.checked_program_closure("forever").is_none(),
        "an unverified signed self cycle cannot enter a closed worker package");
    Ok(())
}

#[test]
fn link_rejects_bad_contract_and_ambient_emit() -> Result<(), String> {
    let source=b"module ledger@1 [ require emit Text -- ! test.emit export handle def handle [ \"audit\" emit ] ]";
    let missing = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(&missing.prepare(source,&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    let unused = noble_contracts::source::ModuleSession::new(&[binding(Binding {
        version: 1,
        slot: 9,
        adapter: "unused",
    })])
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
    let session =
        noble_contracts::source::ModuleSession::new(&[wrong]).map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(&session.prepare(source,&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    let mut wrong = binding(Binding {
        version: 1,
        slot: 2,
        adapter: "wrong-effect",
    });
    wrong.effects.clear();
    let session =
        noble_contracts::source::ModuleSession::new(&[wrong]).map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(&session.prepare(source,&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    Ok(())
}

#[test]
fn invalid_initializers_and_imports_leave_generation_unchanged() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
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

#[test]
fn alias_rebinding_and_stale_commit_keep_previous_candidate() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
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
    session = commit(session, b"import ledger@2 as account")?;
    let new = session
        .prepare(b"saved account.get", &[], LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        new.output(),
        &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool]
    );
    assert_eq!(old.output(), &[noble_kernel::types::Ty::I64]);
    let (session, outcome) = session.commit(stale);
    let error = outcome.err().ok_or("accepted stale module import")?;
    assert_eq!(error.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(
        session
            .prepare(b"account.get", &[], LIMITS)
            .map_err(|e| format!("{e:?}"))?
            .output(),
        &[noble_kernel::types::Ty::Bool]
    );
    Ok(())
}

#[test]
fn nominal_resource_fixture_rejects_dup_drop_and_capture() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
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

#[test]
fn private_arm_is_owner_only_but_owner_eliminator_is_callable() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = commit(session,b"module ledger@1 [ variant Status Ready I64 public Failed Text private export Status export Status.Ready export make export inspect def make [ \"err\" Status.Failed ] def inspect [ [ drop 1 ] [ drop 0 ] Status.match ] ]")?;
    let accepted = session
        .prepare(b"ledger@1.make ledger@1.inspect", &[], LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    assert!(accepted.submission().is_some());
    assert!(session
        .prepare(b"ledger@1.Status.Failed", &[], LIMITS)
        .is_err());
    assert!(session
        .prepare(
            b"ledger@1.make [ drop 1 ] [ drop 0 ] ledger@1.Status.match",
            &[],
            LIMITS
        )
        .is_err());
    let missing_arm=b"module ledger@2 [ variant Status Ready I64 public Failed Text private export bad def bad [ 1 Status.Ready [ drop 1 ] Status.match ] ]";
    assert!(
        matches!(&session.prepare(missing_arm,&[],LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Check)
    );
    Ok(())
}
