//! Independent Wasmtime 40 Canonical ABI linker/converter for two distinct
//! Noble-compiled components. Production dataspace decisions and Preserves
//! ingress are reused; no production component engine or guest interpreter is.
use anyhow::{anyhow, bail, ensure, Context, Result};
use noble_kernel::dataspace::{self, Change, Rights};
use noble_syndicate::{AdmissionRequest, Participant, Profile, SELECTED_LIMITS};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use wasmparser::{Parser, Payload};
use wasmtime::{component::{Component, Instance, Linker, Val}, Config, Engine, Module, Store, StoreLimits, StoreLimitsBuilder};

const IMPORT: &str = "noble:syndicate/dataspace@1.0.0";
const PUBLISH: Rights = Rights { publish: true, observe: false };
const OBSERVE: Rights = Rights { publish: false, observe: true };

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn checked<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| anyhow!("{error:?}"))
}

struct Host {
    profile: Profile,
    participant: Participant,
    test_trap: bool,
    requests: Vec<String>,
    roundtrips: Vec<Value>,
    limits: StoreLimits,
}

fn host_call(host: &mut Host, operation: &str, parameters: &[Val], output: &mut [Val]) -> Result<()> {
    ensure!(output.len() == 1, "invalid WIT result arity");
    host.requests.push(format!("{IMPORT}#{operation}"));
    let result = match (operation, parameters) {
        ("publish", [Val::String(name), Val::Bool(ready)]) => {
            let wire = checked(dataspace::encode_service(name,*ready))?;
            let receipt = checked(host.profile.publish_wire_receipt(&host.participant,wire.as_bytes()))?;
            ensure!(receipt.service.name == name && receipt.service.ready == *ready,
                "committed Preserves schema did not roundtrip WIT arguments");
            host.roundtrips.push(json!({ "operation":format!("{IMPORT}#publish"),
                "name":receipt.service.name,"ready":receipt.service.ready,
                "canonical_text":std::str::from_utf8(wire.as_bytes())?,
                "request_ordinal":host.requests.len(),"outcome":"roundtrip",
                "compiled_import":true }));
            receipt.accepted
        }
        ("observe", [Val::String(name), Val::Bool(ready)]) =>
            checked(host.profile.observe(&host.participant, name, *ready))?,
        ("retract", [Val::String(name)]) =>
            checked(host.profile.retract(&host.participant, name))?,
        ("fail", [Val::Bool(_)]) if host.test_trap => bail!("deliberate test-scope host import trap"),
        ("fail", [Val::Bool(_)]) => bail!("failure import unavailable outside test facet"),
        _ => bail!("invalid imported operation or typed arguments"),
    };
    output[0] = Val::Bool(result);
    Ok(())
}

struct Peer {
    store: Store<Host>,
    instance: Instance,
}

impl Peer {
    fn new(engine: &Engine, component: &Component, profile: &Profile, rights: Rights, test_trap: bool) -> Result<Self> {
        let participant = checked(profile.admit_participant(AdmissionRequest {
            shared_mutable_guest_memory: false, rights,
        }))?;
        let mut linker = Linker::new(engine);
        let mut interface = linker.instance(IMPORT)?;
        for operation in ["publish", "observe", "retract", "fail"] {
            interface.func_new(operation, move |mut store, _, parameters, output| {
                host_call(store.data_mut(), operation, parameters, output)
            })?;
        }
        let limits = StoreLimitsBuilder::new().memory_size(4_194_304).instances(4).tables(4).build();
        let mut store = Store::new(engine, Host {
            profile: profile.clone(), participant, test_trap, requests: Vec::new(),
            roundtrips: Vec::new(), limits,
        });
        store.limiter(|host| &mut host.limits);
        store.set_fuel(1_000_000)?;
        let instance = linker.instantiate(&mut store, component)?;
        Ok(Self { store, instance })
    }

    fn call(&mut self, operation: &str, arguments: &[Val]) -> Result<bool> {
        let function = self.instance.get_func(&mut self.store, operation)
            .context("missing component export")?;
        ensure!(function.ty(&self.store).results().len() == 1, "invalid component export result arity");
        let mut result = [Val::Bool(false)];
        let invocation = (|| {
            function.call(&mut self.store, arguments, &mut result)?;
            function.post_return(&mut self.store)?;
            Ok::<(), anyhow::Error>(())
        })();
        if invocation.is_err() {
            let host = self.store.data_mut();
            checked(host.profile.close(&mut host.participant))?;
        }
        invocation?;
        let [Val::Bool(value)] = result else { bail!("component export returned wrong WIT type") };
        Ok(value)
    }

    fn requests(&self) -> &[String] { &self.store.data().requests }

    fn close(&mut self) -> Result<()> {
        let host = self.store.data_mut();
        checked(host.profile.close(&mut host.participant))
    }
}

fn pair(name: &str, ready: bool) -> [Val; 2] {
    [Val::String(name.into()), Val::Bool(ready)]
}

fn drain(profile: &Profile, events: &mut Vec<Value>) -> Result<()> {
    for event in checked(profile.events())? {
        events.push(json!({"kind":match event.change { Change::Added => "add", Change::Removed => "remove" },
            "name":event.service.name,"ready":event.service.ready,
            "observer":format!("{:?}", event.observer)}));
    }
    checked(profile.clear_events())?;
    Ok(())
}

fn call_record(peer: &mut Peer, role: &str, operation: &str, name: &str, ready: Option<bool>,
               calls: &mut Vec<Value>, profile: &Profile, events: &mut Vec<Value>) -> Result<bool> {
    let inputs = if let Some(value) = ready { pair(name, value).to_vec() }
                 else { vec![Val::String(name.into())] };
    let result = peer.call(operation, &inputs)?;
    let mut row = json!({"role":role,"call":operation,"name":name});
    if let Some(ready) = ready { row["ready"] = json!(ready); }
    row[if operation == "observer" { "membership" } else { "result" }] = json!(result);
    calls.push(row);
    drain(profile, events)?;
    Ok(result)
}

fn compiled_scenario(engine: &Engine, publisher: &Component, subscriber: &Component)
    -> Result<(Value, Profile)> {
    let profile = checked(Profile::new(SELECTED_LIMITS))?;
    let mut producer = Peer::new(engine, publisher, &profile, PUBLISH, true)?;
    let mut observer = Peer::new(engine, subscriber, &profile, OBSERVE, false)?;
    let mut calls = Vec::new();
    let mut events = Vec::new();
    let matrix = [
        ("subscriber", "observer", "clock", Some(true), false),
        ("publisher", "publisher", "clock", Some(true), true),
        ("subscriber", "observer", "clock", Some(true), true),
        ("publisher", "withdraw", "clock", None, true),
        ("subscriber", "observer", "clock", Some(true), false),
        ("subscriber", "observer", "clock", Some(false), false),
        ("publisher", "publisher", "clock", Some(false), true),
        ("subscriber", "observer", "clock", Some(false), true),
        ("publisher", "withdraw", "clock", None, true),
        ("subscriber", "observer", "clock", Some(false), false),
        ("subscriber", "observer", "alarm", Some(false), false),
    ];
    for (role, operation, name, ready, expected) in matrix {
        let peer = if role == "publisher" { &mut producer } else { &mut observer };
        let actual = call_record(peer, role, operation, name, ready, &mut calls, &profile, &mut events)?;
        ensure!(actual == expected, "compiled {role} {operation} {name} membership/result mismatch");
    }
    let import_before = producer.requests().len();
    let trap = producer.call("publish-and-trap", &pair("alarm", false));
    let requests = &producer.requests()[import_before..];
    let published_before_trap = requests.iter().any(|name| name.ends_with("#publish"));
    let trap_import_started = requests.iter().any(|name| name.ends_with("#fail"));
    ensure!(trap.is_err() && published_before_trap && trap_import_started,
        "compiled publisher did not publish then trap");
    calls.push(json!({"role":"publisher","call":"publish-and-trap","name":"alarm",
        "ready":false,"outcome":"trap-after-publication"}));
    drain(&profile, &mut events)?;
    let after = call_record(&mut observer, "subscriber", "observer", "alarm", Some(false),
        &mut calls, &profile, &mut events)?;
    ensure!(!after && calls.len() == 13, "trap did not retract service");
    let subscriber_still_active_before_close = checked(profile.counts())?.0 == 1;
    let publisher_requests = producer.requests().to_vec();
    let subscriber_requests = observer.requests().to_vec();
    let roundtrip = producer.store.data().roundtrips.iter().filter(|row| row["name"]=="clock")
        .cloned().collect::<Vec<_>>();
    producer.close()?;
    observer.close()?;
    let remaining = checked(profile.counts())?;
    ensure!(remaining == (0, 0, 0), "component cleanup leaked conversational scope");
    let expected = [("add","clock",true),("remove","clock",true),
        ("add","clock",false),("remove","clock",false),
        ("add","alarm",false),("remove","alarm",false)];
    let actual = events.iter().map(|row| (row["kind"].as_str().unwrap_or(""),
        row["name"].as_str().unwrap_or(""),row["ready"].as_bool().unwrap_or(true)))
        .collect::<Vec<_>>();
    ensure!(actual == expected, "compiled observer event order mismatch: {actual:?}");
    Ok((json!({"calls":calls,"events":events,"publisher_requests":publisher_requests,
        "subscriber_requests":subscriber_requests,"publisher_trap_reported_success":false,
        "roundtrip":roundtrip,"publisher_calls":publisher_requests.len(),
        "observer_calls":subscriber_requests.len(),"trap_observed":true,
        "trap_import_started":trap_import_started,"published_before_trap":published_before_trap,
        "subscriber_still_active_before_close":subscriber_still_active_before_close,
        "separate_stores":true,"remaining":{"facets":remaining.0,"assertions":remaining.1,"interests":remaining.2}}),profile))
}

fn hostile_controls(profile: &Profile) -> Result<Value> {
    let mut publisher = checked(profile.admit_participant(AdmissionRequest {
        shared_mutable_guest_memory:false, rights:PUBLISH,
    }))?;
    let mut observer = checked(profile.admit_participant(AdmissionRequest {
        shared_mutable_guest_memory:false, rights:OBSERVE,
    }))?;
    let bounds = [
        ("invalid-tag", b"<other \"clock\" #t>".to_vec()),
        ("65-byte-input", vec![b'x';65]),
        ("depth-5", b"<<<<<service \"x\" #t>>>>>".to_vec()),
        ("invalid-utf8", b"<service \"cl\xffck\" #t>".to_vec()),
        ("noncanonical-spacing", b"<service  \"clock\" #t>".to_vec()),
        ("trailing-content", b"<service \"clock\" #t><service \"x\" #f>".to_vec()),
        ("extension-syntax", b"<service \"clock\" #:7>".to_vec()),
    ];
    let mut bound_rows = Vec::new();
    for (name, bytes) in bounds {
        let prior = checked(profile.counts())?;
        let result = profile.publish_wire(&publisher, &bytes);
        ensure!(result.is_err() && checked(profile.counts())? == prior,
            "hostile input mutated dataspace: {name}");
        bound_rows.push(json!({"name":name,"outcome":"reject","error":format!("{:?}", result.err()),
            "typed_protocol_values_created":0,"facet_rights_created":0}));
    }
    let schema = [
        ("ready-not-bool", b"<service \"store\" \"not-a-bool\">".to_vec()),
        ("empty-name", b"<service \"\" #t>".to_vec()),
        ("name-longer-than-32", format!("<service \"{}\" #t>", "a".repeat(33)).into_bytes()),
        ("name-outside-ascii-or-allowed-set", "<service \"café\" #t>".as_bytes().to_vec()),
    ];
    let mut schema_rows = Vec::new();
    for (name, bytes) in schema {
        let prior = checked(profile.counts())?;
        let result = profile.publish_wire(&publisher, &bytes);
        ensure!(result.is_err() && checked(profile.counts())? == prior, "schema mismatch published");
        schema_rows.push(json!({"name":name,"outcome":"reject",
            "typed_protocol_values_created":0,"assertions_published":0}));
    }
    let invalid_extension = b"<service \"Directory\" #t><resource 7>";
    let extension = profile.publish_wire(&publisher, invalid_extension);
    ensure!(extension.is_err(), "resource extension was admitted");
    let service = b"<service \"Directory\" #t>";
    let data_only = profile.publish_wire(&observer, service);
    ensure!(matches!(data_only, Err(noble_syndicate::Error::Kernel(dataspace::Error::Denied))),
        "wire value invented publish rights");
    let decoded = checked(dataspace::decode_service(service,4))?;
    ensure!(decoded.name == "Directory" && decoded.ready, "valid service was not inert data");
    let capability = json!({"outcome":"reject-capability-conversion","payload":{"resource_kind":"Directory","slot":7},
        "valid_service_remains_data_only":true,"invalid_extension_decoded":false,
        "facet_rights_created":0,"live_resources_created":0,"protected_operations":0,
        "attempted_publish_without_right":"denied"});
    checked(profile.close(&mut observer))?;
    checked(profile.close(&mut publisher))?;
    Ok(json!({"bounds":bound_rows,"schema":schema_rows,"capability":capability}))
}

fn inspect_shared_memory(module_bytes: &[u8]) -> Result<bool> {
    let mut config = Config::new();
    config.wasm_threads(true);
    let engine = Engine::new(&config)?;
    Module::new(&engine, module_bytes).context("shared-memory fixture failed Wasmtime validation")?;
    let mut found = None;
    for payload in Parser::new(0).parse_all(module_bytes) {
        if let Payload::MemorySection(section) = payload? {
            for memory in section {
                let memory = memory?;
                ensure!(found.is_none(), "ambiguous multi-memory admission fixture");
                found = Some(memory.shared);
            }
        }
    }
    found.context("admission fixture has no inspected core memory")
}

fn admission_control(engine: &Engine, bytes: &[u8]) -> Result<Value> {
    let shared = inspect_shared_memory(bytes)?;
    ensure!(shared, "gate-supplied module does not request shared memory");
    let loader_refusal = Module::new(engine, bytes);
    ensure!(loader_refusal.is_err(), "selected participant loader admitted shared memory");
    let profile = checked(Profile::new(SELECTED_LIMITS))?;
    let before = checked(profile.counts())?;
    let denied = profile.admit_participant(AdmissionRequest {
        shared_mutable_guest_memory:shared, rights:PUBLISH,
    });
    ensure!(matches!(denied,Err(noble_syndicate::Error::UnsupportedSharedGuestMemory)),
        "real shared memory request was admitted");
    ensure!(before == checked(profile.counts())?, "admission granted a facet");
    Ok(json!({"stage":"profile-admission","outcome":"unsupported",
        "input_sha256":sha256(bytes),"inspected_shared_memory":shared,
        "selected_engine_threads_enabled":false,"selected_engine_shared_memory_enabled":false,
        "loader_rejected":true,"loader_error":format!("{:#}",loader_refusal.unwrap_err()),
        "requested_shared_mutable_noble_memory":true,"guest_requests":0,
        "protected_operations":0,"facet_rights_created":0,"counts_before":before,"counts_after":before}))
}

fn cleanup_control(engine: &Engine, publisher: &Component, subscriber: &Component) -> Result<Value> {
    let profile = checked(Profile::new(SELECTED_LIMITS))?;
    let both = Rights { publish:true, observe:true };
    let mut root = Peer::new(engine, publisher, &profile, both, true)?;
    let mut watcher = Peer::new(engine, subscriber, &profile, OBSERVE, false)?;
    let mut child = checked(profile.child(&root.store.data().participant,PUBLISH))?;
    ensure!(root.call("publisher",&pair("alpha",true))?, "first owned assertion failed");
    ensure!(root.call("publisher",&pair("beta",false))?, "second owned assertion failed");
    ensure!(!checked(profile.observe(&root.store.data().participant,"gamma",true))?,
        "unexpected root interest membership");
    ensure!(watcher.call("observer",&pair("alpha",true))?, "watcher did not observe assertion");
    checked(profile.clear_events())?;
    let before = checked(profile.counts())?;
    ensure!(before == (3,2,2), "unexpected owned cleanup fixture state");
    let trap = root.call("publish-and-trap",&pair("alpha",true));
    ensure!(trap.is_err(), "compiled root failed to trap");
    let after = checked(profile.counts())?;
    ensure!(after == (1,0,1), "root/child assertion or interest survived trap");
    let notifications = checked(profile.events())?;
    ensure!(notifications.len() == 1 && notifications[0].change == Change::Removed
        && notifications[0].service.name == "alpha", "surviving observer missed removal");
    checked(profile.close(&mut child))?;
    checked(profile.close(&mut child))?;
    let subscriber_still_active = !watcher.call("observer",&pair("alpha",true))?;
    root.close()?;
    watcher.close()?;
    let remaining = checked(profile.counts())?;
    ensure!(remaining == (0,0,0), "cleanup control left scope state");
    Ok(json!({"outcome":"retired","actor_trap":true,"compiled_publish_and_trap":true,
        "owned_assertions_before":2,"owned_interests_before":1,"child_scopes_before":1,
        "remaining_scope_assertions":0,"remaining_scope_interests":0,"live_child_scopes":0,
        "duplicate_retractions":0,"surviving_observer_removal":true,
        "subscriber_still_active_before_close":subscriber_still_active,"remaining":remaining}))
}

fn transition_control() -> Result<Value> {
    let profile = checked(Profile::new(SELECTED_LIMITS))?;
    let mut publisher = checked(profile.admit_participant(AdmissionRequest {
        shared_mutable_guest_memory:false,rights:PUBLISH,
    }))?;
    let mut observer = checked(profile.admit_participant(AdmissionRequest {
        shared_mutable_guest_memory:false,rights:OBSERVE,
    }))?;
    checked(profile.observe(&observer,"clock",true))?;
    checked(profile.observe(&observer,"clock",false))?;
    checked(profile.publish(&publisher,"clock",true))?;
    checked(profile.publish(&publisher,"clock",false))?;
    let events = checked(profile.events())?.iter().map(|row| json!({"name":row.service.name,
        "ready":row.service.ready,"kind":match row.change { Change::Added=>"add",Change::Removed=>"remove" }})).collect::<Vec<_>>();
    ensure!(events == vec![json!({"name":"clock","ready":true,"kind":"add"}),
        json!({"name":"clock","ready":true,"kind":"remove"}),
        json!({"name":"clock","ready":false,"kind":"add"})], "replacement not ordered");
    checked(profile.publish(&publisher,"clock",false))?;
    ensure!(checked(profile.events())?.len()==3, "idempotent publish produced an event");
    checked(profile.close(&mut publisher))?;
    checked(profile.close(&mut observer))?;
    Ok(json!({"outcome":"replace-atomic","events":events,"idempotent_replay_events":0,
        "remaining":checked(profile.counts())?}))
}

fn boundary_controls(engine: &Engine, component: &Component) -> Result<Vec<Value>> {
    let called = Arc::new(AtomicUsize::new(0));
    let mut wrong_version = Linker::<()>::new(engine);
    let mut foreign = wrong_version.instance("noble:syndicate/dataspace@1.0.1")?;
    foreign.func_wrap("publish", |_, (_name, _ready): (String, bool)| Ok((true,)))?;
    let mut store = Store::new(engine, ());
    let version = wrong_version.instantiate(&mut store, component);
    ensure!(version.is_err(), "wrong WIT package version linked");

    let mut wrong_signature = Linker::<()>::new(engine);
    let mut imported = wrong_signature.instance(IMPORT)?;
    let invoked = called.clone();
    imported.func_wrap("publish", move |_, (_name, _ready): (i64, bool)| {
        invoked.fetch_add(1, Ordering::SeqCst);
        Ok((true,))
    })?;
    imported.func_wrap("observe", |_, (_name, _ready): (String, bool)| Ok((false,)))?;
    imported.func_wrap("retract", |_, (_name,): (String,)| Ok((false,)))?;
    imported.func_wrap("fail", |_, (_value,): (bool,)| Ok((false,)))?;
    let signature = wrong_signature.instantiate(&mut store, component);
    ensure!(signature.is_err() && called.load(Ordering::SeqCst) == 0,
        "wrong WIT signature linked or invoked host");

    let profile = checked(Profile::new(SELECTED_LIMITS))?;
    let mut publisher = checked(profile.admit_participant(AdmissionRequest {
        shared_mutable_guest_memory:false,rights:PUBLISH,
    }))?;
    let variants = [
        ("invalid-preserves-tag", b"<other \"clock\" #t>".as_slice(), "decode"),
        ("invalid-preserves-schema", b"<service \"clock\" \"not-a-bool\">".as_slice(), "schema"),
        ("resource-shaped-payload", b"<service \"Directory\" #t><resource 7>".as_slice(), "protocol-adapter"),
    ];
    let mut report = vec![
        json!({"name":"wrong-wit-version","outcome":"reject","stage":"link",
            "error":format!("{:#}",version.unwrap_err()),"guest_requests":0,
            "protected_operations":0,"facet_rights_created":0}),
        json!({"name":"wrong-signature","outcome":"reject","stage":"link",
            "error":format!("{:#}",signature.unwrap_err()),"guest_requests":0,
            "protected_operations":0,"facet_rights_created":0}),
    ];
    for (name, input, stage) in variants {
        let before = checked(profile.counts())?;
        let result = profile.publish_wire(&publisher,input);
        ensure!(result.is_err() && before == checked(profile.counts())?,
            "hostile boundary payload mutated scope: {name}");
        report.push(json!({"name":name,"outcome":"reject","stage":stage,
            "error":format!("{:?}",result.err()),"guest_requests":0,
            "protected_operations":0,"facet_rights_created":0,
            "typed_protocol_values_created":0}));
    }
    checked(profile.close(&mut publisher))?;
    Ok(report)
}

fn run(pub_bytes: &[u8],sub_bytes: &[u8], shared_module: &[u8]) -> Result<Value> {
    ensure!(pub_bytes != sub_bytes,"publisher/subscriber compiled binaries must differ");
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true)
        .wasm_threads(false).shared_memory(false);
    let engine = Engine::new(&config)?;
    let publisher = Component::new(&engine,pub_bytes)?;
    let subscriber = Component::new(&engine,sub_bytes)?;
    let (compiled,profile) = compiled_scenario(&engine,&publisher,&subscriber)?;
    let mut controls = hostile_controls(&profile)?;
    controls["roundtrip"] = compiled["roundtrip"].clone();
    controls["admission"] = admission_control(&engine,shared_module)?;
    controls["cleanup"] = cleanup_control(&engine,&publisher,&subscriber)?;
    controls["transition"] = transition_control()?;
    controls["boundary"] = json!(boundary_controls(&engine,&publisher)?);
    ensure!(checked(profile.counts())?==(0,0,0), "hostile controls leaked state");
    Ok(json!({"schema":"noble-m7-peer/v1","outcome":"passed","engine":"wasmtime-40.0.2",
        "components":{"publisher_sha256":sha256(pub_bytes),"subscriber_sha256":sha256(sub_bytes),
            "distinct":pub_bytes != sub_bytes,"threads_enabled":false,
            "shared_memory_enabled":false},"compiled":compiled,"controls":controls,
        "remaining":{"facets":0,"assertions":0,"interests":0},
        "scope":"selected-local-sync-service; no fairness, availability, network, or general Syndicate claim"}))
}

fn main() -> Result<()> {
    let args=std::env::args().collect::<Vec<_>>();
    ensure!(args.len()==5 && args[3]=="scenario",
        "usage: noble-m7-peer PUBLISHER_COMPONENT.wasm OBSERVER_COMPONENT.wasm scenario SHARED_CORE.wasm");
    let publisher=fs::read(&args[1])?;
    let subscriber=fs::read(&args[2])?;
    let shared=fs::read(&args[4])?;
    println!("{}",run(&publisher,&subscriber,&shared)?);
    Ok(())
}
