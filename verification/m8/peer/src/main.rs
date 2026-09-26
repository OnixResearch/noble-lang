//! Independent Wasmtime 40 typed linker for separately Noble-compiled service
//! components. All selected imports cross the production M8 monitor; no guest
//! interpreter, host-only positive substitute, or new WIT world is used.
use anyhow::{anyhow, bail, ensure, Context, Result};
use noble_kernel::dataspace::{Change, Limits, Rights};
use noble_syndicate::{choreography::{self, Action, Error as ChoreoError, Expected, Export,
    Import, ImportResult, InvocationOutcome, ParticipantToken, Role, Session, WitBinding},
    AdmissionRequest, Profile, SELECTED_LIMITS};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, sync::{Arc, atomic::{AtomicUsize, Ordering}}};
use wasmtime::{component::{types::ComponentItem, Component, Instance, Linker, Val}, Config, Engine, Store,
    StoreLimits, StoreLimitsBuilder};

const IMPORT_NAME: &str = "noble:syndicate/dataspace@1.0.0";
const PUBLISH: Rights = Rights { publish: true, observe: false };
fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn checked<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| anyhow!("{error:?}"))
}
fn input<'a>(workload: &'a Value, case: &str) -> Result<&'a Value> {
    workload["cases"][case]["input"].as_object()
        .map(|_| &workload["cases"][case]["input"])
        .with_context(|| format!("missing complete canonical input for {case}"))
}
fn declared<'a>(value: &'a Value, key: &str) -> Result<Vec<&'a str>> {
    value[key].as_array().with_context(|| format!("missing declared {key}"))?.iter()
        .map(|name| name.as_str().with_context(|| format!("invalid {key}")))
        .collect()
}
fn role_name(role: Role) -> &'static str {
    match role { Role::Publisher => "publisher", Role::Subscriber => "subscriber" }
}
fn action_row(plan: &choreography::Plan, action: &Action) -> Result<Value> {
    let name = plan.name(action.step()).context("projected action lacks a checked round name")?;
    let export = action.export().wit_name().context("unrecognized M8 export")?;
    let mut value = json!({ "step":action.step(), "round":action.round(),
        "role":role_name(action.role()), "export":export, "name":name });
    if let Some(ready) = action.ready() { value["ready"] = json!(ready); }
    value["result"] = match action.result() {
        Expected::Returned(result) => json!(result),
        Expected::TrapAfterPublication => json!("trap-after-publication"),
    };
    Ok(value)
}
fn events(session: &Session) -> Result<Vec<Value>> {
    let length = checked(session.snapshot())?.events_len;
    let mut entries = Vec::with_capacity(length);
    let indexed = u32::try_from(length).context("session event count exceeds monitor index")?;
    for index in 0..indexed {
        let item = checked(session.event(index))?.context("event vanished within a session snapshot")?;
        let name = item.name.as_str().context("invalid M8 event name")?;
        entries.push(json!({
            "kind":match item.change { Change::Added => "add", Change::Removed => "remove" },
            "name":name, "ready":item.ready,
            "observer":format!("{:?}", item.observer),
        }));
    }
    ensure!(checked(session.snapshot())?.events_len == length,
        "session event count changed during indexed event readout");
    Ok(entries)
}
fn state(session: &Session) -> Result<Value> {
    let snap = checked(session.snapshot())?;
    let publisher = checked(session.token(Role::Publisher))?;
    let subscriber = checked(session.token(Role::Subscriber))?;
    Ok(json!({ "cursor":snap.cursor, "pending":snap.pending,
        "guest_requests":snap.guest_requests, "refused_imports":snap.refused_requests,
        "protected_operations":snap.protected_operations,
        "dataspace":{"facets":snap.counts.0,"assertions":snap.counts.1,"interests":snap.counts.2},
        "events":events(session)?, "events_len":snap.events_len,
        "rights":{"publisher":{"facet":format!("{:?}",publisher.facet()),"publish":true,"observe":false},
                  "subscriber":{"facet":format!("{:?}",subscriber.facet()),"publish":false,"observe":true}} }))
}
fn new_session(bytes: &[u8]) -> Result<(Profile, Arc<Session>)> {
    let profile = checked(Profile::new(SELECTED_LIMITS))?;
    let session = Arc::new(checked(Session::admit(profile.clone(), bytes, WitBinding::SERVICE))?);
    Ok((profile, session))
}

// Authenticate the actual component's exact import identity before granting
// facets: Wasmtime's linker permits semver-compatible patch versions. Linking
// then checks the operation signatures and selected typed exports.
fn validate_binding(engine: &Engine, component: &Component) -> Result<()> {
    let component_type = component.component_type();
    let mut imports = component_type.imports(engine);
    let (identity, interface) = imports.next().context("missing versioned component import")?;
    ensure!(identity == IMPORT_NAME && imports.next().is_none(),
        "component import identity does not exactly match {IMPORT_NAME}");
    let ComponentItem::ComponentInstance(interface) = interface else {
        bail!("selected import is not a component interface")
    };
    let operations = ["publish", "observe", "retract", "fail"];
    ensure!(interface.exports(engine).len() == operations.len()
        && operations.iter().all(|name|
            matches!(interface.get_export(engine, name), Some(ComponentItem::ComponentFunc(_)))),
        "component interface has missing or additional operations");
    let exports = ["publisher", "observer", "withdraw", "publish-and-trap"];
    ensure!(component_type.exports(engine).len() == exports.len()
        && exports.iter().all(|name|
            matches!(component_type.get_export(engine, name), Some(ComponentItem::ComponentFunc(_)))),
        "component has missing or additional selected service exports");
    let invoked = Arc::new(AtomicUsize::new(0));
    let mut linker = Linker::<StoreLimits>::new(engine);
    let mut interface = linker.instance(IMPORT_NAME)?;
    for operation in ["publish", "observe", "retract", "fail"] {
        let called = invoked.clone();
        match operation {
            "publish" | "observe" => interface.func_wrap(operation,
                move |_, (_name, _ready): (String, bool)| -> Result<(bool,)> {
                    called.fetch_add(1, Ordering::SeqCst);
                    bail!("component called import during pre-facet signature admission")
                })?,
            "retract" => interface.func_wrap(operation, move |_, (_name,): (String,)| -> Result<(bool,)> {
                called.fetch_add(1, Ordering::SeqCst);
                bail!("component called import during pre-facet signature admission")
            })?,
            "fail" => interface.func_wrap(operation, move |_, (_value,): (bool,)| -> Result<(bool,)> {
                called.fetch_add(1, Ordering::SeqCst);
                bail!("component called import during pre-facet signature admission")
            })?,
            _ => unreachable!(),
        }
    }
    let limits = StoreLimitsBuilder::new().memory_size(4_194_304).instances(4).tables(4).build();
    let mut store = Store::new(engine, limits);
    store.limiter(|limits| limits);
    store.set_fuel(1_000_000)?;
    let instance = linker.instantiate(&mut store, component)?;
    ensure!(invoked.load(Ordering::SeqCst) == 0, "guest import started before facet admission");
    for name in ["publisher", "observer", "publish-and-trap"] {
        instance.get_typed_func::<(String, bool), (bool,)>(&mut store, name)
            .with_context(|| format!("wrong typed export signature {name}"))?;
    }
    instance.get_typed_func::<(String,), (bool,)>(&mut store, "withdraw")
        .context("wrong typed export signature withdraw")?;
    Ok(())
}

struct Host {
    session: Arc<Session>,
    token: ParticipantToken,
    requests: Vec<Value>,
    limits: StoreLimits,
}
fn host_call(host: &mut Host, operation: &str, parameters: &[Val], output: &mut [Val]) -> Result<()> {
    ensure!(output.len() == 1, "wrong WIT result arity");
    let import = match (operation, parameters) {
        ("publish", [Val::String(name), Val::Bool(ready)]) => Import::Publish { name, ready:*ready },
        ("observe", [Val::String(name), Val::Bool(ready)]) => Import::Observe { name, ready:*ready },
        ("retract", [Val::String(name)]) => Import::Retract { name },
        ("fail", [Val::Bool(value)]) => Import::Fail { value:*value },
        _ => bail!("unexpected untyped or unselected component import"),
    };
    let result = host.session.import(&host.token, import);
    let mut row = json!({"operation":operation, "identity":format!("{IMPORT_NAME}#{operation}"),
        "role":role_name(host.token.role()), "monitored":true});
    match import {
        Import::Publish { name, ready } | Import::Observe { name, ready } => {
            row["name"] = json!(name); row["ready"] = json!(ready);
        }
        Import::Retract { name } => row["name"] = json!(name),
        Import::Fail { value } => row["value"] = json!(value),
    }
    match result {
        Ok(ImportResult::Boolean(value)) => {
            row["accepted"] = json!(true); row["result"] = json!(value);
            host.requests.push(row);
            output[0] = Val::Bool(value);
            Ok(())
        }
        Err(error) => {
            row["accepted"] = json!(false); row["error"] = json!(format!("{error:?}"));
            host.requests.push(row);
            bail!("production M8 monitor refused/trapped import: {error:?}")
        }
    }
}
struct Peer {
    store: Store<Host>,
    instance: Instance,
}
struct Invocation {
    result: std::result::Result<bool, String>,
    finalized: std::result::Result<(), ChoreoError>,
    imports: Vec<Value>,
    before_finish: Value,
}
impl Peer {
    fn new(engine: &Engine, component: &Component, session: Arc<Session>, role: Role) -> Result<Self> {
        let token = checked(session.token(role))?;
        let mut linker = Linker::new(engine);
        let mut interface = linker.instance(IMPORT_NAME)?;
        for operation in ["publish", "observe", "retract", "fail"] {
            interface.func_new(operation, move |mut store, _, parameters, output| {
                host_call(store.data_mut(), operation, parameters, output)
            })?;
        }
        let limits = StoreLimitsBuilder::new().memory_size(4_194_304).instances(4).tables(4).build();
        let mut store = Store::new(engine, Host { session, token, requests: Vec::new(), limits });
        store.limiter(|host| &mut host.limits);
        store.set_fuel(1_000_000)?;
        let instance = linker.instantiate(&mut store, component)?;
        Ok(Self { store, instance })
    }
    fn session(&self) -> &Session { &self.store.data().session }
    fn token(&self) -> &ParticipantToken { &self.store.data().token }
    // `begin` is host-owned pre-export admission. The guest is never invoked
    // after a refusal. `finish` runs after the actual call and post_return.
    fn invoke(&mut self, export: Export, name: &str, ready: Option<bool>) -> Result<Invocation> {
        let export_name = export.wit_name().context("unrecognized M8 export")?;
        checked(self.session().begin(self.token(), export, name, ready))?;
        ensure!(self.store.data().requests.is_empty(), "prior component imports were not accounted");
        let function = self.instance.get_func(&mut self.store, export_name)
            .context("admitted component export missing")?;
        let arguments = [Val::String(name.into()), Val::Bool(ready.unwrap_or(false))];
        let parameters = &arguments[..if ready.is_some() { 2 } else { 1 }];
        ensure!(function.ty(&self.store).results().len() == 1, "wrong export result type");
        let mut output = [Val::Bool(false)];
        let invocation = (|| {
            function.call(&mut self.store, parameters, &mut output)?;
            function.post_return(&mut self.store)?;
            let [Val::Bool(result)] = output else { bail!("non-Boolean export result") };
            Ok::<bool, anyhow::Error>(result)
        })();
        let before_finish = state(self.session())?;
        let outcome = match &invocation {
            Ok(value) => InvocationOutcome::Returned(*value),
            Err(_) => InvocationOutcome::Trapped,
        };
        let finalized = self.session().finish(self.token(), outcome);
        Ok(Invocation { result: invocation.map_err(|error| format!("{error:#}")), finalized,
            imports: std::mem::take(&mut self.store.data_mut().requests), before_finish })
    }
}
fn engine() -> Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true)
        .wasm_threads(false).shared_memory(false);
    Engine::new(&config)
}
fn compiled(engine: &Engine, publisher: &Component, subscriber: &Component, bytes: &[u8]) -> Result<Value> {
    validate_binding(engine, publisher)?;
    validate_binding(engine, subscriber)?;
    let (profile, session) = new_session(bytes)?;
    let mut producer = Peer::new(engine, publisher, session.clone(), Role::Publisher)?;
    let mut observer = Peer::new(engine, subscriber, session.clone(), Role::Subscriber)?;
    let plan = checked(choreography::project(bytes))?;
    let mut calls = Vec::new();
    let mut imports = Vec::new();
    let mut publisher_exports = Vec::new();
    let mut subscriber_exports = Vec::new();
    let mut observer_membership = Vec::new();
    let mut trap_import_sequence = Vec::new();
    let mut trap_publish_import_succeeded = false;
    let mut trap_fail_import_observed = false;
    let mut compiled_invocation_error_observed = false;
    let mut trap_guest_error = None;
    let mut subscriber_observes_absence_after_trap = false;
    for action in plan.global() {
        let name = plan.name(action.step()).context("missing projected name")?;
        let export_name = action.export().wit_name().context("unrecognized M8 export")?;
        let step = usize::try_from(action.step()).context("projected step exceeds host index")?;
        ensure!(step < plan.global().len(), "projected step exceeds plan");
        let peer = if action.role() == Role::Publisher { &mut producer } else { &mut observer };
        let before = checked(session.snapshot())?;
        ensure!(before.cursor == step && !before.pending, "pre-export monitor cursor drift");
        let outcome = peer.invoke(action.export(), name, action.ready())?;
        checked(outcome.finalized).with_context(|| format!("step {} production finalization", action.step()))?;
        let after = checked(session.snapshot())?;
        ensure!(after.cursor == step + 1 && !after.pending, "step did not advance exactly once");
        ensure!(!outcome.imports.is_empty(), "compiled export performed no monitored import");
        ensure!(outcome.imports.iter().all(|row| row["monitored"] == true), "import bypassed production monitor");
        for row in &outcome.imports { imports.push(json!({"step":action.step(), "call":row})); }
        let mut observed = json!({"step":action.step(), "round":action.round(),
            "role":role_name(action.role()), "export":export_name, "name":name});
        if let Some(ready) = action.ready() { observed["ready"] = json!(ready); }
        match action.result() {
            Expected::Returned(expected) => {
                let actual = *outcome.result.as_ref().map_err(|error| anyhow!(error.clone()))?;
                ensure!(actual == expected, "compiled step {} returned wrong result", action.step());
                observed["result"] = json!(actual);
                if action.export() == Export::Observer { observer_membership.push(actual); }
            }
            Expected::TrapAfterPublication => {
                ensure!(outcome.result.is_err(), "terminal guest export wrongly returned success");
                compiled_invocation_error_observed = true;
                trap_guest_error = outcome.result.as_ref().err().cloned();
                trap_import_sequence = outcome.imports.iter().map(|row| row["operation"].as_str().unwrap_or("").to_owned()).collect();
                trap_publish_import_succeeded = outcome.imports.first().is_some_and(|row|
                    row["operation"] == "publish" && row["accepted"] == true && row["result"] == true);
                trap_fail_import_observed = outcome.imports.get(1).is_some_and(|row|
                    row["operation"] == "fail" && row["error"] == "GuestFail");
                ensure!(trap_publish_import_succeeded && trap_fail_import_observed
                    && trap_import_sequence == ["publish", "fail"], "earlier refusal mistaken for terminal trap");
                observed["result"] = json!("trap-after-publication");
            }
        }
        if action.role() == Role::Publisher { publisher_exports.push(export_name); }
        else { subscriber_exports.push(export_name); }
        if step + 1 == plan.global().len() {
            subscriber_observes_absence_after_trap = action.role() == Role::Subscriber
                && action.export() == Export::Observer && outcome.result == Ok(false);
        }
        calls.push(observed);
    }
    ensure!(checked(session.complete())?, "host monitor did not complete projected plan");
    let before_close = checked(session.snapshot())?;
    ensure!(before_close.counts == (1, 0, 2), "publisher did not retire while subscriber survived");
    let typed_events = events(&session)?;
    ensure!(typed_events.len() == 4, "two rounds did not emit four distinct typed add/remove notifications");
    let compiled = json!({"calls":calls,"imports":imports,"publisher_exports":publisher_exports,
        "subscriber_exports":subscriber_exports,"observer_membership":observer_membership,
        "events":typed_events,"trap_import_sequence":trap_import_sequence,
        "trap_publish_import_succeeded":trap_publish_import_succeeded,
        "trap_fail_import_observed":trap_fail_import_observed,
        "compiled_invocation_error_observed":compiled_invocation_error_observed,
        "trap_guest_error":trap_guest_error,
        "compiled_trap_observed":compiled_invocation_error_observed,
        "publisher_trap_reported_success":false,
        "subscriber_observes_absence_after_trap":subscriber_observes_absence_after_trap,
        "subscriber_still_active":before_close.counts.0 == 1,
        "remaining_publisher_assertions":before_close.counts.1,
        "false_readiness_is_distinct_from_absence":observer_membership == [false,true,false,false,false],
        "all_imports_use_production_monitor":imports.iter().all(|row| row["call"]["monitored"] == true),
        "guest_facet_authority_from_descriptor":false,"new_wit_world_created":false,
        "complete_before_close":true});
    drop(producer); drop(observer); drop(session);
    ensure!(checked(profile.counts())? == (0,0,0), "session Drop failed to retire final observer");
    let mut complete = compiled;
    complete["remaining"] = json!({"facets":0,"assertions":0,"interests":0});
    Ok(complete)
}

fn pre_session(workload: &Value, baseline: &[u8]) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    let input = input(workload,"S-CASE-19")?;
    let variants = workload["variants"].as_array().context("missing gate-supplied hostile descriptor bytes")?;
    for name in declared(input, "pre_session_mutations")? {
        let profile = checked(Profile::new(SELECTED_LIMITS))?;
        let fixture = if name == "nonempty-dataspace" {
            None
        } else {
            let row = variants.iter().find(|row| row["name"] == name).with_context(|| format!("missing hostile bytes {name}"))?;
            let file = row["file"].as_str().context("missing hostile byte path")?;
            let bytes = fs::read(file)?;
            ensure!(sha(&bytes) == row["sha256"], "gate-supplied hostile descriptor changed");
            Some(bytes)
        };
        let mut existing = if fixture.is_none() { Some(checked(profile.admit_participant(AdmissionRequest {
            shared_mutable_guest_memory:false, rights:PUBLISH }))?) } else { None };
        let before = checked(profile.counts())?;
        let before_events = checked(profile.events())?;
        let result = Session::admit(profile.clone(), fixture.as_deref().unwrap_or(baseline), WitBinding::SERVICE);
        ensure!(result.is_err(), "hostile descriptor/session admitted: {name}");
        let error = result.err().context("expected admission refusal")?;
        let stage = match error {
            ChoreoError::ByteLimit => "pre-parse-length",
            ChoreoError::DepthLimit => "pre-parse-depth",
            ChoreoError::Utf8 => "pre-parse-utf8",
            ChoreoError::ParserEnd => "parser-end",
            ChoreoError::Descriptor => "typed-deserialize",
            ChoreoError::Protocol => "version",
            ChoreoError::Nonempty => "pre-session",
            _ => bail!("unexpected pre-session refusal for {name}: {error:?}"),
        };
        ensure!(before == checked(profile.counts())? && before_events == checked(profile.events())?,
            "pre-session refusal changed the M7 dataspace");
        let digest = fixture.as_ref().map(|bytes| sha(bytes));
        rows.push(json!({"name":name,"outcome":"reject","stage":stage,
            "input_sha256":digest,"error":format!("{error:?}"),
            "typed_descriptors_created":0,"facet_rights_created":0,"guest_requests":0,
            "protected_operations":0,"before":before,"after":checked(profile.counts())?}));
        if let Some(participant) = existing.as_mut() { checked(profile.close(participant))?; }
    }
    Ok(rows)
}
fn strict_json_edges(workload: &Value) -> Result<Vec<Value>> {
    let variants = workload["strict_json_edges"].as_array()
        .context("missing gate-supplied strict JSON edge fixtures")?;
    let mut rows = Vec::new();
    for variant in variants {
        let name = variant["name"].as_str().context("missing strict JSON edge name")?;
        let file = variant["file"].as_str().context("missing strict JSON edge byte path")?;
        let bytes = fs::read(file)?;
        ensure!(sha(&bytes) == variant["sha256"], "strict JSON edge changed after gate freeze: {name}");
        let profile = checked(Profile::new(SELECTED_LIMITS))?;
        let before_counts = checked(profile.counts())?;
        let before_events = checked(profile.events())?;
        ensure!(before_counts == (0, 0, 0) && before_events.is_empty(),
            "strict JSON edge lacks a clean pre-session profile");
        let projected = choreography::project(&bytes);
        let (outcome, error, actions) = match variant["outcome"].as_str() {
            Some("admit") => {
                let plan = checked(projected)?;
                let actions = plan.global().iter().map(|action| action_row(&plan, action))
                    .collect::<Result<Vec<_>>>()?;
                let session = checked(Session::admit(profile.clone(), &bytes, WitBinding::SERVICE))?;
                ensure!(checked(profile.counts())? == (2, 0, 0),
                    "valid escaped JSON did not admit both participant facets");
                drop(session);
                ("admit", None, Some(actions))
            }
            Some("reject") => {
                let error = projected.err().with_context(|| format!("invalid strict JSON edge admitted: {name}"))?;
                let refused = Session::admit(profile.clone(), &bytes, WitBinding::SERVICE);
                ensure!(matches!(refused, Err(actual) if actual == error),
                    "projection and pre-facet admission disagreed on strict JSON edge: {name}");
                ("reject", Some(format!("{error:?}")), None)
            }
            _ => bail!("unreviewed strict JSON edge verdict: {name}"),
        };
        let after_counts = checked(profile.counts())?;
        let after_events = checked(profile.events())?;
        ensure!(after_counts == before_counts && after_events == before_events,
            "strict JSON edge leaked facets/assertions/events: {name}");
        rows.push(json!({"name":name,"outcome":outcome,"error":error,
            "input_sha256":sha(&bytes),"facet_rights_created":if outcome == "admit" { 2 } else { 0 },
            "facet_rights_remaining":0,
            "global_actions":actions,
            "before":{"counts":before_counts,"events":before_events.len()},
            "after":{"counts":after_counts,"events":after_events.len()}}));
    }
    Ok(rows)
}
fn one_round_reservations(workload: &Value) -> Result<Vec<Value>> {
    let fixture = &workload["one_round"];
    let bytes = fs::read(fixture["file"].as_str().context("missing one-round descriptor")?)?;
    ensure!(sha(&bytes) == fixture["sha256"], "one-round descriptor changed after gate freeze");
    let plan = checked(choreography::project(&bytes))?;
    ensure!(plan.global().len() == 5, "one-round descriptor did not project one withdrawal");
    let mut rows = Vec::new();
    for (name, interests, events, admissible) in [
        ("one-round-interests-one", 1, 4, false),
        ("one-round-events-two", 2, 2, false),
        ("one-round-full-reservation", 2, 4, true),
    ] {
        let limits = Limits { interests, events, ..SELECTED_LIMITS };
        let profile = checked(Profile::new(limits))?;
        let before = checked(profile.counts())?;
        ensure!(before == (0, 0, 0) && checked(profile.events())?.is_empty(),
            "one-round reservation did not start with a clean dataspace");
        let (outcome, error, admitted) = match Session::admit(profile.clone(), &bytes, WitBinding::SERVICE) {
            Ok(session) if admissible => {
                let admitted = checked(profile.counts())?.0;
                ensure!(admitted == 2, "adequate one-round reservation did not grant two facets");
                drop(session);
                ("admit", None, admitted)
            }
            Err(ChoreoError::Capacity) if !admissible => ("reject", Some("Capacity"), 0),
            Ok(_) => bail!("under-reserved one-round descriptor admitted: {name}"),
            Err(error) => bail!("one-round reservation refused at wrong boundary: {name}: {error:?}"),
        };
        let after = checked(profile.counts())?;
        ensure!(after == before && checked(profile.events())?.is_empty(),
            "one-round reservation leaked authority or notifications: {name}");
        rows.push(json!({"name":name,"descriptor_sha256":sha(&bytes),
            "projected_actions":plan.global().len(),
            "limits":{"facets":limits.facets,"assertions":limits.assertions,
                "interests":limits.interests,"events":limits.events},
            "outcome":outcome,
            "error":error,"facets_admitted_before_cleanup":admitted,
            "before":before,"after":after}));
    }
    Ok(rows)
}

fn drive(engine: &Engine, publisher: &Component, subscriber: &Component, descriptor: &[u8], through: usize)
    -> Result<(Profile, Arc<Session>, Peer, Peer)> {
    let (profile, session) = new_session(descriptor)?;
    let mut producer = Peer::new(engine, publisher, session.clone(), Role::Publisher)?;
    let mut observer = Peer::new(engine, subscriber, session.clone(), Role::Subscriber)?;
    let plan = checked(choreography::project(descriptor))?;
    for action in plan.global().iter().take(through) {
        let name = plan.name(action.step()).context("missing projected name")?;
        let peer = if action.role() == Role::Publisher { &mut producer } else { &mut observer };
        let result = peer.invoke(action.export(), name, action.ready())?;
        checked(result.finalized)?;
        match action.result() {
            Expected::Returned(expected) => ensure!(result.result == Ok(expected), "control prelude diverged"),
            Expected::TrapAfterPublication => ensure!(result.result.is_err(), "control did not trap"),
        }
    }
    Ok((profile, session, producer, observer))
}

fn refused_begin(session: &Session, token: &ParticipantToken, export: Export,
    name: &str, ready: Option<bool>, label: &str) -> Result<Value> {
    let before = state(session)?;
    let result = session.begin(token, export, name, ready);
    ensure!(result.is_err(), "pre-export mutation admitted: {label}");
    let after = state(session)?;
    ensure!(before == after, "pre-export denial changed session state: {label}");
    let export_name = export.wit_name().context("unrecognized M8 export")?;
    Ok(json!({"name":label,"outcome":"reject","guest_invoked":false,
        "stage":"pre-export","error":format!("{:?}",result.err()),
        "attempt":{"role":role_name(token.role()),"export":export_name,
            "name":name,"ready":ready,"facet":format!("{:?}",token.facet())},
        "before":before,"after":after}))
}
fn pre_export(engine: &Engine, publisher: &Component, subscriber: &Component, descriptor: &[u8],
    names: &[&str]) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for &name in names {
        let through = match name {
            "replayed-step" | "forged-rights" | "replayed-export" => 1,
            "wrong-export-publisher-instead-of-publish-and-trap" | "wrong-export-at-trap-step" => 6,
            "false-ready-mistaken-for-absence" | "false-ready-pair-confused-with-absence" => 2,
            "retired-facet" => 7,
            _ => 0,
        };
        let (_profile, session, producer, observer) = drive(engine,publisher,subscriber,descriptor,through)?;
        let publisher_token = checked(session.token(Role::Publisher))?;
        let subscriber_token = checked(session.token(Role::Subscriber))?;
        let row = match name {
            "wrong-role" | "wrong-component-role" => refused_begin(&session,&publisher_token,Export::Observer,"clock",Some(false),name)?,
            "out-of-order" => refused_begin(&session,&publisher_token,Export::Publisher,"clock",Some(false),name)?,
            "replayed-step" | "replayed-export" => refused_begin(&session,&subscriber_token,Export::Observer,"clock",Some(false),name)?,
            "wrong-export-publisher-instead-of-publish-and-trap" | "wrong-export-at-trap-step" =>
                refused_begin(&session,&publisher_token,Export::Publisher,"alarm",Some(true),name)?,
            "wrong-name" => refused_begin(&session,&subscriber_token,Export::Observer,"alarm",Some(false),name)?,
            "wrong-ready" => refused_begin(&session,&subscriber_token,Export::Observer,"clock",Some(true),name)?,
            "foreign-facet" => {
                let (_foreign_profile, foreign) = new_session(descriptor)?;
                let token = checked(foreign.token(Role::Subscriber))?;
                refused_begin(&session,&token,Export::Observer,"clock",Some(false),name)?
            }
            "retired-facet" => refused_begin(&session,&publisher_token,Export::Publisher,"alarm",Some(true),name)?,
            "forged-rights" => refused_begin(&session,&subscriber_token,Export::Publisher,"clock",Some(false),name)?,
            "false-ready-mistaken-for-absence" | "false-ready-pair-confused-with-absence" => {
                let snapshot = checked(session.snapshot())?;
                ensure!(snapshot.counts.1 == 1, "false-ready control lacks a real assertion");
                refused_begin(&session,&subscriber_token,Export::Observer,"clock",Some(true),name)?
            }
            _ => bail!("unreviewed pre-export mutation: {name}"),
        };
        drop(producer); drop(observer); drop(session);
        rows.push(row);
    }
    Ok(rows)
}
fn link_refusal(engine: &Engine, component: &Component, label: &str,
    digest: &str) -> Result<Value> {
    let profile = checked(Profile::new(SELECTED_LIMITS))?;
    let before = checked(profile.counts())?;
    let failure = validate_binding(engine, component);
    ensure!(failure.is_err(), "hostile actual component linked as selected world: {label}");
    ensure!(checked(profile.counts())? == before, "link refusal granted rights");
    // No Session::admit call is permitted before validating actual submitted WIT.
    // Bind the inspected malformed artifact's bytes independently of its label.
    Ok(json!({"name":label,"outcome":"reject","stage":"link","guest_invoked":false,
        "component_sha256":digest,"facet_rights_created":0,"error":format!("{:#}",failure.unwrap_err()),
        "before":{"cursor":0,"pending":false,"dataspace":{"facets":0,"assertions":0,"interests":0},
            "events":[],"rights":{},"guest_requests":0,"protected_operations":0},
        "after":{"cursor":0,"pending":false,"dataspace":{"facets":0,"assertions":0,"interests":0},
            "events":[],"rights":{},"guest_requests":0,"protected_operations":0}}))
}
fn in_flight(engine: &Engine, component: &Component, subscriber: &Component,
    descriptor: &[u8], name: &str) -> Result<Value> {
    let through = if name == "missing-fail-after-trap-publication"
        || name == "missing-fail-after-publish-and-trap-publication" { 6 } else { 3 };
    let (_profile, session, mut producer, observer) = drive(engine,component,subscriber,descriptor,through)?;
    let before = state(&session)?;
    let (export, target, ready) = if through == 6 {
        (Export::PublishAndTrap,"alarm",Some(true))
    } else { (Export::Withdraw,"clock",None) };
    // The guest export itself is genuinely Noble-compiled; the hostile body
    // invokes a mismatched import/argument or omits fail after a successful
    // publication. Host callback always delegates to Session::import.
    let invocation = producer.invoke(export,target,ready)?;
    ensure!(invocation.finalized == Err(ChoreoError::Invocation),
        "production monitor accepted hostile in-flight component: {name}");
    ensure!(!checked(session.complete())?, "hostile in-flight session completed");
    let after = state(&session)?;
    ensure!(before["cursor"] == after["cursor"], "refused in-flight call advanced cursor");
    ensure!(after["dataspace"]["assertions"] == 0 && after["dataspace"]["facets"] == 1,
        "in-flight refusal did not retire publisher and owned assertion");
    let authorized_publications = u64::from(through == 6);
    let preceding_operations = before["protected_operations"].as_u64().context("missing protected operation counter")?;
    let current_operations = after["protected_operations"].as_u64().context("missing final operation counter")?;
    ensure!(current_operations == preceding_operations + authorized_publications,
        "in-flight denial committed an unauthorized M7 operation");
    let preceding_requests = before["guest_requests"].as_u64().context("missing guest request counter")?;
    let current_requests = after["guest_requests"].as_u64().context("missing final guest request counter")?;
    ensure!(current_requests == preceding_requests + authorized_publications,
        "in-flight denial accepted an unauthorized guest request");
    let preceding_refusals = before["refused_imports"].as_u64().context("missing import refusal counter")?;
    let current_refusals = after["refused_imports"].as_u64().context("missing final import refusal counter")?;
    ensure!(current_refusals == preceding_refusals + u64::from(through != 6),
        "in-flight import refusal accounting diverged");
    let before_events = before["events"].as_array().context("missing prior events")?;
    let after_events = after["events"].as_array().context("missing cleanup events")?;
    let expected_name = if through == 6 { "alarm" } else { "clock" };
    let expected_ready = through == 6;
    let new_events = &after_events[before_events.len()..];
    ensure!(new_events.len() == if through == 6 { 2 } else { 1 },
        "in-flight denial emitted an unexpected M7 event");
    if through == 6 {
        ensure!(new_events[0]["kind"] == "add" && new_events[0]["name"] == "alarm"
            && new_events[0]["ready"] == true, "missing-fail variant did not publish alarm");
    }
    let removal = new_events.iter().find(|entry|
        entry["kind"] == "remove" && entry["name"] == expected_name
            && entry["ready"] == expected_ready)
        .context("surviving subscriber missed exact typed mandatory removal")?.clone();
    let owned_before = before["dataspace"]["assertions"] == 1
        || invocation.before_finish["dataspace"]["assertions"] == 1;
    ensure!(owned_before, "no previously owned assertion tested in-flight");
    let rejected = invocation.imports.iter().any(|entry| entry["error"] == "ImportRefused");
    if through == 6 {
        ensure!(invocation.result == Ok(true), "missing-fail guest did not return after real publication");
        ensure!(invocation.imports.len() == 1 && invocation.imports[0]["accepted"] == true
            && invocation.imports[0]["operation"] == "publish", "missing-fail fixture did not publish");
    } else {
        ensure!(invocation.result.is_err() && rejected,
            "mismatched imported operation did not trigger Wasmtime trap");
    }
    drop(producer); drop(observer); drop(session);
    Ok(json!({"name":name,"outcome":"reject","owned_assertion_before":owned_before,
        "unauthorized_m7_operations":0,"unauthorized_protected_operations":0,
        "publisher_retired":true,"previously_owned_assertions_retracted":true,
        "typed_removal":removal,"reported_success":false,"choreography_completed":false,
        "before":before,"after":after,"before_finish":invocation.before_finish,
        "imports":invocation.imports,"guest_invocation_error":invocation.result.err(),
        "monitor_finalization_error":format!("{:?}",invocation.finalized.err())}))
}
fn bypass_control(descriptor: &[u8]) -> Result<bool> {
    let (profile, session) = new_session(descriptor)?;
    let before = checked(session.snapshot())?;
    let direct = profile.admit_participant(AdmissionRequest {
        shared_mutable_guest_memory:false, rights:PUBLISH,
    });
    ensure!(matches!(direct,Err(noble_syndicate::Error::ChoreographyActive)),
        "legacy Profile participant admitted during M8 session");
    ensure!(checked(session.snapshot())? == before, "legacy admission mutated active M8 session");
    drop(session);
    Ok(false)
}
fn run(pub_bytes: &[u8], sub_bytes: &[u8], workload_bytes: &[u8]) -> Result<Value> {
    ensure!(pub_bytes != sub_bytes, "two participant artifacts must be compiled separately");
    let workload: Value = serde_json::from_slice(workload_bytes)?;
    ensure!(workload["schema"] == "noble-m8-peer-workload/v1", "wrong gate workload schema");
    let descriptor_file = workload["descriptor"].as_str().context("missing gate-supplied descriptor")?;
    let descriptor = fs::read(descriptor_file)?;
    ensure!(sha(&descriptor) == workload["descriptor_sha256"], "descriptor changed after gate freeze");
    let engine = engine()?;
    let publisher = Component::new(&engine,pub_bytes)?;
    let subscriber = Component::new(&engine,sub_bytes)?;
    // Only after actual Wasmtime typed linking of both submitted artifacts may
    // the trusted host supply WitBinding::SERVICE to production Session::admit.
    validate_binding(&engine,&publisher)?;
    validate_binding(&engine,&subscriber)?;
    let plan = checked(choreography::project(&descriptor))?;
    let projection = json!({"descriptor_sha256":sha(&descriptor), "stage":"projection", "outcome":"admitted",
        "guest_requests":0,"protected_operations":0,
        "global_actions":plan.global().iter().map(|action| action_row(&plan,action)).collect::<Result<Vec<_>>>()?,
        "publisher_steps":plan.publisher_steps(),"subscriber_steps":plan.subscriber_steps(),
        "descriptor_grants_authority":false,"new_wit_world_created":false});
    let compiled = compiled(&engine,&publisher,&subscriber,&descriptor)?;
    let controls = {
        let hostile = input(&workload,"S-CASE-19")?;
        let boundary = input(&workload,"WI-20")?;
        let pre_session = pre_session(&workload,&descriptor)?;
        let strict_json_edges = strict_json_edges(&workload)?;
        let one_round_reservations = one_round_reservations(&workload)?;
        let pre_export_rows = pre_export(&engine,&publisher,&subscriber,&descriptor,
            &declared(hostile,"pre_export_mutations")?)?;
        let mut boundary_pre_export = Vec::new();
        for name in declared(boundary,"pre_export_mutations")? {
            if name == "wrong-wit-version" || name == "wrong-wit-signature" {
                let variant = &workload["components"][name];
                let file = variant["artifact"].as_str().context("missing malformed component artifact")?;
                let bytes = fs::read(file)?;
                ensure!(sha(&bytes) == variant["sha256"], "malformed component changed");
                let component = Component::new(&engine,&bytes)?;
                boundary_pre_export.push(link_refusal(&engine,&component,name,&sha(&bytes))?);
            } else {
                boundary_pre_export.extend(pre_export(&engine,&publisher,&subscriber,&descriptor,&[name])?);
            }
        }
        let mut in_flight_rows = Vec::new();
        for name in declared(hostile,"in_flight_mutations")? {
            let variant = &workload["components"][name];
            let bytes = fs::read(variant["artifact"].as_str().context("missing hostile component")?)?;
            ensure!(sha(&bytes) == variant["sha256"], "hostile component changed");
            let component = Component::new(&engine,&bytes)?;
            validate_binding(&engine,&component)?;
            in_flight_rows.push(in_flight(&engine,&component,&subscriber,&descriptor,name)?);
        }
        let mut boundary_post_start = Vec::new();
        for name in declared(boundary,"post_start_mutations")? {
            let mapped = match name {
                "wrong-import-after-admitted-export" => "wrong-import-identity",
                "missing-fail-after-publish-and-trap-publication" => "missing-fail-after-trap-publication",
                _ => bail!("unreviewed hostile boundary variant: {name}"),
            };
            let variant = &workload["components"][mapped];
            let bytes = fs::read(variant["artifact"].as_str().context("missing hostile boundary component")?)?;
            ensure!(sha(&bytes) == variant["sha256"], "hostile boundary component changed");
            let component = Component::new(&engine,&bytes)?;
            validate_binding(&engine,&component)?;
            let mut observation = in_flight(&engine,&component,&subscriber,&descriptor,mapped)?;
            observation["name"] = json!(name);
            boundary_post_start.push(observation);
        }
        json!({"pre_session":pre_session,"strict_json_edges":strict_json_edges,
            "one_round_reservations":one_round_reservations,
            "pre_export":pre_export_rows,"in_flight":in_flight_rows,
            "boundary_pre_export":boundary_pre_export,"boundary_post_start":boundary_post_start,
            "direct_m7_profile_bypass":bypass_control(&descriptor)?})
    };
    Ok(json!({"schema":"noble-m8-peer/v1","outcome":"passed","engine":"wasmtime-40.0.2",
        "workload_sha256":sha(workload_bytes),
        "components":{"publisher_sha256":sha(pub_bytes),"subscriber_sha256":sha(sub_bytes),
            "distinct":pub_bytes != sub_bytes,"separate_stores":true,
            "threads_enabled":false,"shared_memory_enabled":false},
        "projection":projection,"compiled":compiled,"controls":controls,
        "scope":"finite-local-synchronous-projection; no general choreography or transport claim"}))
}
fn main() -> Result<()> {
    let usage = "usage: noble-m8-peer PUBLISHER_COMPONENT.wasm SUBSCRIBER_COMPONENT.wasm scenario WORKLOAD.json";
    let mut args = std::env::args().skip(1);
    let publisher_file = args.next().context(usage)?;
    let subscriber_file = args.next().context(usage)?;
    let scenario = args.next().context(usage)?;
    let workload_file = args.next().context(usage)?;
    ensure!(scenario == "scenario" && args.next().is_none(), "{usage}");
    let publisher = fs::read(publisher_file)?;
    let subscriber = fs::read(subscriber_file)?;
    let workload = fs::read(workload_file)?;
    println!("{}",run(&publisher,&subscriber,&workload)?);
    Ok(())
}
