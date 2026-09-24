use crate::{adapter::Host, transmit};
use anyhow::{Context, Result, bail, ensure};
use noble_kernel::async_tasks::{Callback, Failure};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use wasmtime::component::{
    Accessor, Component, Linker, Resource, ResourceAny, ResourceType, Val, types::ComponentItem,
};
use wasmtime::{
    AsContextMut, Config, Engine, Store, StoreContextMut, StoreLimits, StoreLimitsBuilder,
};

pub type Shared = Arc<Mutex<Host>>;
pub type Events = Arc<Mutex<Vec<String>>>;
pub const MAX_JOBS: usize = 32;
pub const MAX_LIVE_VALUES: usize = 8;
pub const FUEL: u64 = 100_000;
pub const YIELD_FUEL: u64 = 1_000;
pub const NATIVE_DELAY_MS: u64 = 30;

pub mod bindings {
    wasmtime::component::bindgen!({
        path: "../../../crates/noble-wasm/wit/async.wit",
        world: "bootstrap",
        exports: { default: async | task_exit },
    });
}

pub struct Counter;
pub struct Runtime {
    pub limits: StoreLimits,
    pub host: Shared,
    pub jobs: Arc<Mutex<Vec<std::thread::JoinHandle<Result<()>>>>>,
    pub events: Events,
    pub case: String,
    pub live: Vec<(Val, Callback, bool)>,
}

pub fn config() -> Config {
    let mut config = Config::new();
    config
        .async_support(true)
        .wasm_component_model(true)
        .wasm_component_model_async(true)
        .wasm_component_model_async_builtins(true)
        .wasm_component_model_async_stackful(true)
        .consume_fuel(true)
        .epoch_interruption(true);
    config
}

pub fn event(events: &Events, text: &str) {
    let mut events = events.lock();
    assert!(events.len() < 256, "bounded event observation overflow");
    events.push(text.to_owned());
}

pub fn native(
    runtime: &Runtime,
    callback: Callback,
    domain: bool,
    bytes: usize,
) -> Result<oneshot::Receiver<Result<()>>> {
    let mut jobs = runtime.jobs.lock();
    ensure!(
        jobs.len() < MAX_JOBS,
        "native job capacity was not reserved"
    );
    let host = runtime.host.clone();
    let events = runtime.events.clone();
    let (send, receive) = oneshot::channel();
    jobs.push(std::thread::spawn(move || {
        // Deliberately blocking work: never run this on the cooperative engine thread.
        std::thread::sleep(Duration::from_millis(NATIVE_DELAY_MS));
        let result = (|| {
            let mut host = host.lock();
            host.complete(callback, domain, bytes)?;
            host.settle(callback)?;
            event(&events, "native-complete");
            Ok(())
        })();
        let failure = result
            .as_ref()
            .err()
            .map(|error: &anyhow::Error| format!("{error:#}"));
        let _ = send.send(result);
        match failure {
            Some(error) => bail!("{error}"),
            None => Ok(()),
        }
    }));
    Ok(receive)
}

pub async fn wait_native(
    runtime: &Accessor<Runtime>,
    callback: Callback,
    receive: oneshot::Receiver<Result<()>>,
) -> Result<()> {
    let (case, host, events) = runtime.with(|mut access| {
        (
            access.get().case.clone(),
            access.get().host.clone(),
            access.get().events.clone(),
        )
    });
    if matches!(
        case.as_str(),
        "cancel-before-completion" | "late-completion"
    ) {
        tokio::time::sleep(Duration::from_millis(1)).await;
        host.lock().cancel(callback)?;
        event(&events, "cancel-ack");
        if case == "cancel-before-completion" {
            bail!("invocation cancelled before native completion");
        }
    }
    receive
        .await
        .context("native worker vanished without completion")??;
    if case == "ready-cancel" {
        host.lock().cancel(callback)?;
        event(&events, "cancel-ready-ack");
    }
    if matches!(
        case.as_str(),
        "cancel-before-completion" | "late-completion" | "ready-cancel"
    ) {
        bail!("invocation cancelled: no terminal payload delivered");
    }
    Ok(())
}

fn take_live(runtime: &mut Runtime, value: &Val) -> Result<(Callback, bool)> {
    let index = runtime
        .live
        .iter()
        .position(|(key, _, _)| match (key, value) {
            (Val::Future(left), Val::Future(right)) => left == right,
            (Val::Stream(left), Val::Stream(right)) => left == right,
            _ => false,
        })
        .context("foreign, duplicated or wrong-store live value")?;
    let (_, callback, numeric_future) = runtime.live.swap_remove(index);
    Ok((callback, numeric_future))
}

fn sync_call(
    mut store: StoreContextMut<'_, Runtime>,
    name: &str,
    numeric_future: bool,
    params: &[Val],
    results: &mut [Val],
) -> Result<()> {
    event(&store.data().events, name);
    match (name, params) {
        ("make-future", []) | ("make-stream", []) => {
            ensure!(
                store.data().live.len() < MAX_LIVE_VALUES,
                "live-value capacity"
            );
            ensure!(
                store.data().jobs.lock().len() < MAX_JOBS,
                "native job capacity"
            );
            let callback = store.data().host.lock().admit(name, 0, 64, None)?;
            let domain = store.data().case == "domain-error";
            let receive = native(
                store.data(),
                callback,
                domain,
                if domain {
                    12
                } else if name == "make-stream" {
                    3
                } else {
                    8
                },
            )?;
            let value = if name == "make-future" {
                if numeric_future {
                    transmit::number(&mut store, receive, domain)
                } else {
                    transmit::future(&mut store, receive, callback, domain)
                }
            } else {
                transmit::stream(&mut store, receive, callback, domain)
            };
            store
                .data_mut()
                .live
                .push((value.clone(), callback, numeric_future)); // identity key only, never lowered again
            results[0] = value;
        }
        ("cancel-future", [value]) => {
            let (callback, numeric) = take_live(store.data_mut(), value)?;
            store.data().host.lock().cancel_task(callback)?;
            if numeric {
                wasmtime::component::FutureReader::<i64>::from_val(&mut store, value)?
                    .close(&mut store);
            } else {
                wasmtime::component::FutureReader::<Result<i64, String>>::from_val(
                    &mut store, value,
                )?
                .close(&mut store);
            }
        }
        ("close-stream", [value]) => {
            let (callback, _) = take_live(store.data_mut(), value)?;
            let mut reader = wasmtime::component::StreamReader::<u8>::from_val(&mut store, value)?;
            reader.close(&mut store);
            // Reader closure is not invocation cancellation; producer terminal work is drained below.
            event(
                &store.data().events,
                &format!("stream-read-end-closed:{}", callback.task.generation),
            );
        }
        _ => bail!("unapproved synchronous operation {name}"),
    }
    Ok(())
}

async fn async_call(
    accessor: &Accessor<Runtime>,
    name: &str,
    params: &[Val],
    results: &mut [Val],
) -> Result<()> {
    if matches!(name, "finish-future" | "drain-stream") {
        let value = params.first().context("missing live argument")?;
        let (callback, numeric) = accessor.with(|mut access| take_live(access.get(), value))?;
        return transmit::consume(accessor, name, value, callback, numeric, results).await;
    }
    event(
        &accessor.with(|mut access| access.get().events.clone()),
        &format!("enter-{name}"),
    );
    let (callback, receive) = accessor.with(|mut access| -> Result<_> {
        let runtime = access.get();
        ensure!(runtime.jobs.lock().len() < MAX_JOBS, "native job capacity");
        let argument = match params.last() {
            Some(Val::S64(value)) => *value,
            _ => 0,
        };
        let callback = match (name, params.first()) {
            ("transfer" | "consume-error", Some(Val::Resource(resource))) => {
                let resource = resource.try_into_resource::<Counter>(access.as_context_mut())?;
                ensure!(resource.owned(), "async input must be owned");
                access.get().host.lock().admit_resource(
                    resource.rep(),
                    argument,
                    name == "transfer",
                )?
            }
            ("first" | "second" | "sum5", _) => runtime.host.lock().admit(
                name,
                params.len() * 8,
                64,
                (name == "first").then_some(argument),
            )?,
            _ => bail!("unapproved asynchronous operation {name}"),
        };
        let receive = native(
            access.get(),
            callback,
            name == "consume-error",
            if name == "consume-error" { 12 } else { 8 },
        )?;
        Ok((callback, receive))
    })?;
    let events = accessor.with(|mut access| access.get().events.clone());
    event(&events, &format!("suspend-{name}"));
    wait_native(accessor, callback, receive).await?;
    event(&events, &format!("resume-{name}"));
    accessor.with(|mut access| -> Result<()> {
        let host = access.get().host.clone();
        if name == "transfer" {
            let rep = host.lock().deliver_resource(callback)?;
            results[0] = Val::Resource(ResourceAny::try_from_resource(
                Resource::<Counter>::new_own(rep),
                access.as_context_mut(),
            )?);
        } else {
            host.lock().deliver(callback)?;
            results[0] = if name == "consume-error" {
                Val::Result(Err(Some(Box::new(Val::String("domain-error".into())))))
            } else {
                let values = params
                    .iter()
                    .map(|value| match value {
                        Val::S64(value) => Ok(*value),
                        _ => bail!("expected s64"),
                    })
                    .collect::<Result<Vec<_>>>()?;
                Val::S64(if name == "sum5" {
                    values.iter().copied().sum()
                } else {
                    values[0].wrapping_add(1)
                })
            };
        }
        if access.get().case == "delivery-before-cancel" {
            host.lock().cancel(callback)?;
        }
        Ok(())
    })?;
    event(&events, &format!("complete-{name}"));
    Ok(())
}

fn link(engine: &Engine, component: &Component, linker: &mut Linker<Runtime>) -> Result<()> {
    for (name, item) in component.component_type().imports(engine) {
        let interface = match item {
            ComponentItem::ComponentInstance(interface) => interface,
            ComponentItem::Resource(_) => {
                ensure!(name == "counter", "unapproved root resource {name}");
                linker
                    .root()
                    .resource(name, ResourceType::host::<Counter>(), |store, rep| {
                        store.data().host.lock().drop_resource(rep)
                    })?;
                continue;
            }
            ComponentItem::Type(_) => continue,
            _ => bail!("unapproved root import {name}"),
        };
        ensure!(
            matches!(
                name,
                "noble-test:async-boundary/host@1.0.0"
                    | "noble-test:async-resources/counters@1.0.0"
                    | "noble-test:async-wide/host@1.0.0"
                    | "noble-test:async-eligibility/host@1.0.0"
            ),
            "unapproved interface {name}"
        );
        let mut instance = linker.instance(name)?;
        for (name, item) in interface.exports(engine) {
            match item {
                ComponentItem::Type(_) => {}
                ComponentItem::Resource(_) => {
                    ensure!(name == "counter", "unapproved resource");
                    instance.resource(name, ResourceType::host::<Counter>(), |store, rep| {
                        store.data().host.lock().drop_resource(rep)
                    })?;
                }
                ComponentItem::ComponentFunc(ty) if ty.async_() => {
                    let owned = name.to_owned();
                    instance.func_new_concurrent(name, move |accessor, _, params, results| {
                        let name = owned.clone();
                        Box::pin(async move { async_call(accessor, &name, params, results).await })
                    })?;
                }
                ComponentItem::ComponentFunc(ty) => {
                    let owned = name.to_owned();
                    let numeric = matches!(ty.results().next(), Some(wasmtime::component::Type::Future(future)) if future.ty() == Some(wasmtime::component::Type::S64));
                    instance.func_new(name, move |store, _, params, results| {
                        sync_call(store, &owned, numeric, params, results)
                    })?;
                }
                _ => bail!("unsupported import {name}"),
            }
        }
    }
    Ok(())
}

pub async fn execute(path: &str, export: &str, case: &str) -> Result<Value> {
    let engine = Engine::new(&config())?;
    let component = Component::from_file(&engine, path)?;
    let host = Arc::new(Mutex::new(Host::new(case)?));
    let events = Arc::new(Mutex::new(Vec::with_capacity(256)));
    let jobs = Arc::new(Mutex::new(Vec::with_capacity(MAX_JOBS)));
    let mut store = Store::new(
        &engine,
        Runtime {
            limits: StoreLimitsBuilder::new()
                .memory_size(4_194_304)
                .instances(64)
                .tables(64)
                .build(),
            host: host.clone(),
            events: events.clone(),
            jobs: jobs.clone(),
            case: case.to_owned(),
            live: Vec::with_capacity(MAX_LIVE_VALUES),
        },
    );
    store.limiter(|runtime| &mut runtime.limits);
    store.set_fuel(FUEL)?;
    store.fuel_async_yield_interval(Some(YIELD_FUEL))?;
    store.set_epoch_deadline(1);
    let mut linker = Linker::new(&engine);
    link(&engine, &component, &mut linker)?;
    let instance = linker.instantiate_async(&mut store, &component).await?;
    let started = Instant::now();
    let result = if [
        "order",
        "future-result",
        "stream-result",
        "future-cancel",
        "stream-close",
    ]
    .contains(&export)
    {
        let peer = bindings::Bootstrap::new(&mut store, &instance)?;
        store.run_concurrent(async |accessor| -> Result<Vec<Value>> {
            match export {
                "order" => { let (value, task) = peer.call_order(accessor, 40).await?; task.block(accessor).await; Ok(vec![integer(value)]) }
                "future-result" => { let (value, task) = peer.call_future_result(accessor).await?; task.block(accessor).await; Ok(vec![sum(value.map(integer))]) }
                "stream-result" => { let (value, task) = peer.call_stream_result(accessor).await?; task.block(accessor).await; Ok(vec![sum(value.map(|bytes| json!({"type":"List","items":bytes.into_iter().map(|byte| integer(i64::from(byte))).collect::<Vec<_>>()})))]) }
                "future-cancel" => { let ((), task) = peer.call_future_cancel(accessor).await?; task.block(accessor).await; Ok(vec![]) }
                "stream-close" => { let ((), task) = peer.call_stream_close(accessor).await?; task.block(accessor).await; Ok(vec![]) }
                _ => unreachable!(),
            }
        }).await.and_then(|value| value)
    } else {
        let function = instance
            .get_func(&mut store, export)
            .context("missing export")?;
        let args = if export == "sum5" {
            vec![
                Val::S64(1),
                Val::S64(2),
                Val::S64(3),
                Val::S64(4),
                Val::S64(5),
            ]
        } else if export == "check" {
            vec![]
        } else {
            let rep = host.lock().create_resource(41)?;
            vec![
                Val::Resource(ResourceAny::try_from_resource(
                    Resource::<Counter>::new_own(rep),
                    &mut store,
                )?),
                Val::S64(1),
            ]
        };
        store.run_concurrent(async |accessor| {
            let mut results = [Val::S64(0)];
            let task = function.call_concurrent(accessor, &args, &mut results).await?;
            task.block(accessor).await;
            accessor.with(|mut access| match &results[0] {
                Val::S64(value) => Ok(vec![integer(*value)]),
                Val::Result(Err(Some(error))) => match &**error { Val::String(error) => Ok(vec![sum(Err(error.clone()))]), _ => bail!("wrong domain error") },
                Val::Resource(resource) => {
                    let returned = resource.try_into_resource::<Counter>(access.as_context_mut())?;
                    let rep = returned.rep();
                    let value = host.lock().resource_value(rep)?;
                    host.lock().drop_resource(rep)?;
                    Ok(vec![json!({"type":"Resource","value":value.to_string(),"disposition":"peer-released"})])
                }
                _ => bail!("unsupported peer output"),
            })
        }).await.and_then(|value| value)
    };
    let outcome = if result.is_ok() {
        "normal"
    } else if matches!(
        case,
        "cancel-before-completion" | "late-completion" | "ready-cancel"
    ) {
        "cancelled"
    } else {
        "trap"
    };
    if result.is_err() {
        host.lock().fail_all(if outcome == "cancelled" {
            Failure::Cancelled
        } else {
            Failure::Trap
        })?;
    }
    let fuel = store.get_fuel()?;
    let guest_stop_ms = started.elapsed().as_millis();
    let native_pins_at_store_drop = host.lock().native_pins();
    drop(store); // guest lifetime ends; host table and native jobs remain alive
    for job in std::mem::take(&mut *jobs.lock()) {
        job.join()
            .map_err(|_| anyhow::anyhow!("native worker panicked"))??;
    }
    host.lock().retire_guest_resources()?;
    // Closing a stream only closes its read end; settle the producer's terminal outcome independently.
    if result.is_ok() {
        host.lock().close_streams()?;
        host.lock().finish_invocation()?;
    }
    let report = host.lock().report();
    let error = result.as_ref().err().map(|error| format!("{error:#}"));
    Ok(
        json!({"schema":"noble-m6-peer/v1","engine":"wasmtime-40.0.2","binding_generator":"wasmtime-wit-bindgen-40.0.2","observation":{"outcome":outcome,"results":result.unwrap_or_default(),"events":*events.lock(),"error":error,"task_exit_observed":outcome=="normal"},"host":report,"case":case,"elapsed_ms":started.elapsed().as_millis(),"guest_stop_ms":guest_stop_ms,"native_pins_at_store_drop":native_pins_at_store_drop,"remaining_fuel":fuel,"native_jobs_joined":true,"host_retained_after_store_drop":true}),
    )
}

pub fn integer(value: i64) -> Value {
    json!({"type":"I64","value":value.to_string()})
}
pub fn sum(value: Result<Value, String>) -> Value {
    match value {
        Ok(value) => json!({"type":"Sum","tag":"left","value":value}),
        Err(error) => json!({"type":"Sum","tag":"right","value":{"type":"Text","value":error}}),
    }
}
