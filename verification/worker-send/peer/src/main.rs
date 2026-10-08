mod host;
mod verified;

use anyhow::{Context, Result, bail, ensure};
use host::{Grant, Host, Mode};
use noble_kernel::async_tasks as task;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc, time::Duration};
use wasmtime::component::{Component, FutureReader, Linker, Val, types::ComponentItem};
use wasmtime::{Config, Engine, Store, StoreContextMut, StoreLimits, StoreLimitsBuilder};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../../crates/noble-wasm/wit/worker-send.wit",
        world: "worker-shell",
        exports: { default: async | task_exit },
    });
}

struct Runtime {
    limits:StoreLimits,
    host:Arc<Mutex<Host>>,
    selected:verified::Verified,
    live:Option<(Val,task::Callback)>,
}

fn config()->Config {
    let mut config=Config::new();
    config.async_support(true);
    config.wasm_component_model(true);
    config.wasm_component_model_async(true);
    config.wasm_component_model_async_builtins(true);
    config.wasm_component_model_async_stackful(true);
    config.consume_fuel(true);
    config.epoch_interruption(true);
    config
}

fn begin(mut store:StoreContextMut<'_,Runtime>,params:&[Val],results:&mut [Val])->Result<()> {
    let [Val::S64(attempt),Val::String(text)]=params else {bail!("invalid typed WIT Send arguments")};
    ensure!(results.len()==1,"WIT begin-send result arity changed");
    ensure!(*attempt==store.data().selected.attempt
        && text==&store.data().selected.text,
        "component Send differs from checked nominal guest Action");
    ensure!(store.data().live.is_none(),"second live future cannot copy Send authority");
    let (callback,receive)=store.data().host.lock().begin(*attempt,text)?;
    let future=FutureReader::new(&mut store,async move {
        let result=receive.await.context("native Send vanished before terminal callback")?;
        Ok::<std::result::Result<i64,String>,anyhow::Error>(result)
    }).into_val();
    store.data_mut().live=Some((future.clone(),callback));
    results[0]=future;
    Ok(())
}

fn cancel(mut store:StoreContextMut<'_,Runtime>,params:&[Val],results:&mut [Val])->Result<()> {
    let [Val::Future(value)]=params else {bail!("invalid typed WIT future cancellation")};
    ensure!(results.is_empty(),"WIT cancel-send result arity changed");
    let (live,callback)=store.data_mut().live.take().context("unknown or duplicate Send future")?;
    if !matches!(&live,Val::Future(current) if current==value) {
        store.data_mut().live=Some((live,callback));
        bail!("foreign Send future cannot cancel retained task");
    }
    store.data().host.lock().cancel(callback)?;
    FutureReader::<std::result::Result<i64,String>>::from_val(&mut store,&live)?
        .close(&mut store);
    Ok(())
}

fn link(engine:&Engine,component:&Component,linker:&mut Linker<Runtime>)->Result<()> {
    let mut selected=false;
    for (name,item) in component.component_type().imports(engine) {
        ensure!(name=="noble-test:worker-send/host@1.0.0" && !selected,
            "component imported a different or duplicate authority interface");
        let ComponentItem::ComponentInstance(instance)=item else {
            bail!("component imported a non-interface authority")
        };
        selected=true;
        let mut begin_seen=false;
        let mut cancel_seen=false;
        let mut linked=linker.instance(name)?;
        for (operation,item) in instance.exports(engine) {
            match (operation,item) {
                ("begin-send",ComponentItem::ComponentFunc(ty)) if !ty.async_() && !begin_seen=>{
                    begin_seen=true;
                    linked.func_new(operation,|store,_,params,results|
                        begin(store,params,results))?;
                }
                ("cancel-send",ComponentItem::ComponentFunc(ty)) if !ty.async_() && !cancel_seen=>{
                    cancel_seen=true;
                    linked.func_new(operation,|store,_,params,results|
                        cancel(store,params,results))?;
                }
                ("begin-send" | "cancel-send",ComponentItem::Type(_))=>{},
                _=>bail!("unapproved Worker Send WIT import {operation}"),
            }
        }
        ensure!(begin_seen && cancel_seen,"selected WIT Send/cancel interface incomplete");
    }
    ensure!(selected,"component did not import selected Worker Send WIT host");
    Ok(())
}

async fn execute(arguments:&[String])->Result<Value> {
    let [_,component_path,cli_path,package_path,schema_path,state_path,submit_path,cancel_path,grant_path,mode]=arguments else {
        bail!("usage: noble-worker-send-peer COMPONENT NOBLE PACKAGE SCHEMA STATE SUBMIT CANCEL HOST_GRANT send-then-cancel|cancel-before-send")
    };
    let selected=verified::run(Path::new(cli_path),Path::new(package_path),Path::new(schema_path),
        Path::new(state_path),Path::new(submit_path),Path::new(cancel_path))?;
    let grant=Grant::selected(Path::new(grant_path))?;
    let mode=Mode::parse(mode)?;
    let meta=std::fs::metadata(component_path)?;
    ensure!(meta.is_file() && meta.len()<=4_194_304,"selected component exceeds bounded input");
    let engine=Engine::new(&config())?;
    let component=Component::from_file(&engine,component_path)?;
    let host=Arc::new(Mutex::new(Host::new(grant,mode,&selected.source_sha256)?));
    let mut store=Store::new(&engine,Runtime {
        limits:StoreLimitsBuilder::new().memory_size(4_194_304).instances(16).tables(16).build(),
        host:host.clone(),selected,live:None,
    });
    store.limiter(|runtime|&mut runtime.limits);
    store.set_fuel(100_000)?;
    store.fuel_async_yield_interval(Some(1_000))?;
    store.set_epoch_deadline(1);
    let mut linker=Linker::new(&engine);
    link(&engine,&component,&mut linker)?;
    let instance=linker.instantiate_async(&mut store,&component).await?;
    let typed=bindings::WorkerShell::new(&mut store,&instance)?;
    let attempt=store.data().selected.attempt;
    let text=store.data().selected.text.clone();
    let call=tokio::time::timeout(Duration::from_secs(10),store.run_concurrent(async |accessor| {
        let ((),task)=typed.call_dispatch(accessor,attempt,text).await?;
        task.block(accessor).await;
        Ok::<(),anyhow::Error>(())
    })).await.map_err(anyhow::Error::from).and_then(|value|value).and_then(|value|value);
    let selected=&mut store.data_mut().selected;
    let source_sha256=std::mem::take(&mut selected.source_sha256);
    let checked_first=std::mem::take(&mut selected.first);
    let checked_second=std::mem::take(&mut selected.second);
    let future_consumed=store.data().live.is_none();
    let pins_at_guest_exit=host.lock().outstanding_pins();
    drop(store);
    // Store disposal is not native-stop. Host table/callback and producer job
    // remain alive until explicit join, completion, resource cleanup and pin settlement.
    host.lock().abort_unconsumed()?;
    let joined={host.lock().take_native()};
    if let Some(joined)=joined {
        let native=joined.join().map_err(|_|anyhow::anyhow!("native Send producer panicked"))?;
        host.lock().finish_native(native)?;
    } else {
        host.lock().release_unused()?;
    }
    let observed=host.lock().observation();
    let outcome=if observed["denied"]==true {"denied"}
        else if observed["native_error"].as_str().is_some() {"native-failure-retired"}
        else if call.is_ok() && future_consumed {"cancelled-retired"}
        else {"internal-failure"};
    Ok(json!({"schema":"noble-worker-send-wit-integration/v1",
        "profile":"Worker-Send-WIT-Local", "stage":"component-retirement",
        "outcome":outcome, "world":"noble-test:worker-send/worker-shell@1.0.0",
        "source_sha256":source_sha256,"first_guest":checked_first,"cancel_guest":checked_second,
        "guest_component_error":call.err().map(|error|format!("{error:#}")),
        "future_consumed":future_consumed,"native_pins_at_guest_exit":pins_at_guest_exit,
        "host":observed}))
}

#[tokio::main(flavor="current_thread")]
async fn main() {
    let args:Vec<String>=std::env::args().collect();
    match execute(&args).await {
        Ok(report)=>{
            let failed=report["outcome"]!="cancelled-retired" && report["outcome"]!="denied";
            println!("{report}");
            if failed {std::process::exit(2)}
        }
        Err(error)=>{
            println!("{}",json!({"schema":"noble-worker-send-wit-integration/v1",
                "outcome":"internal-failure","diagnostic":format!("{error:#}")}));
            std::process::exit(2)
        }
    }
}
