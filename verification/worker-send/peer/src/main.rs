mod host;
mod verified;

use anyhow::{Context, Result, bail, ensure};
use host::{Grant, Host, Mode};
use noble_kernel::async_tasks as task;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{future::Future, io::Read, os::unix::fs::OpenOptionsExt, path::Path,
    sync::Arc, task::Poll, time::Duration};
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
    let cancellation=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
        store.data().host.lock().cancel(callback)))
        .map_err(|_|anyhow::anyhow!("host Send cancellation panicked"))
        .and_then(|result|result);
    // The guest has relinquished the future even when the native cancellation
    // fails or unwinds; do not leave a callback capable of delivering success.
    let closed=FutureReader::<std::result::Result<i64,String>>::from_val(&mut store,&live)
        .map(|future|future.close(&mut store));
    cancellation?;
    closed?;
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

fn read_component_file(mut file:std::fs::File)->Result<(Vec<u8>,String)> {
    let metadata=file.metadata()?;
    ensure!(metadata.is_file() && metadata.len()<=4_194_304,
        "selected component must be a bounded regular file");
    let mut bytes=vec![0;usize::try_from(metadata.len())?];
    file.read_exact(&mut bytes).context("reading selected component")?;
    let mut extra=[0];
    ensure!(file.read(&mut extra)?==0,"selected component grew beyond its byte bound");
    let sha256=format!("{:x}",Sha256::digest(&bytes));
    Ok((bytes,sha256))
}

fn read_component(path:&Path)->Result<(Vec<u8>,String)> {
    let file=std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK)
        .open(path).context("selected component unavailable")?;
    read_component_file(file)
}

async fn execute(arguments:&[String])->Result<Value> {
    let [_,component_path,cli_path,package_path,schema_path,state_path,submit_path,cancel_path,grant_path,mode]=arguments else {
        bail!("usage: noble-worker-send-peer COMPONENT NOBLE PACKAGE SCHEMA STATE SUBMIT CANCEL HOST_GRANT send-then-cancel|cancel-before-send|inject-receive-error|inject-receive-corruption|inject-producer-panic|inject-cancel-error|inject-cancel-panic|inject-fallback-authority-error|buffer-quota-preflight")
    };
    let selected=verified::run(Path::new(cli_path),Path::new(package_path),Path::new(schema_path),
        Path::new(state_path),Path::new(submit_path),Path::new(cancel_path))?;
    let grant=Grant::selected(Path::new(grant_path))?;
    let mode=Mode::parse(mode)?;
    let (component_bytes,component_sha256)=read_component(Path::new(component_path))?;
    let engine=Engine::new(&config())?;
    let component=Component::new(&engine,&component_bytes)?;
    let host=Arc::new(Mutex::new(Host::new(grant,mode,&selected.source_sha256)?));
    let mut store=Store::new(&engine,Runtime {
        limits:StoreLimitsBuilder::new().memory_size(4_194_304).instances(16).tables(16).build(),
        host:host.clone(),selected,live:None,
    });
    // Instantiation runs core start functions, which may already have begun
    // a native Send before trapping or failing typed export lookup. Keep all
    // work after Store construction in one bounded, unwind-safe guest phase;
    // even an error here must proceed through the shared retirement path.
    let call=tokio::time::timeout(Duration::from_secs(10),async {
        let phase=async {
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
            store.run_concurrent(async |accessor| {
                let ((),task)=typed.call_dispatch(accessor,attempt,text).await?;
                task.block(accessor).await;
                Ok::<(),anyhow::Error>(())
            }).await??;
            Ok::<(),anyhow::Error>(())
        };
        let mut phase=std::pin::pin!(phase);
        std::future::poll_fn(|cx|
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                phase.as_mut().poll(cx)))
                .unwrap_or_else(|_|Poll::Ready(Err(anyhow::anyhow!(
                    "component guest phase panicked"))))
        ).await
    }).await.map_err(anyhow::Error::from).and_then(|result|result);
    let selected=&mut store.data_mut().selected;
    let source_sha256=std::mem::take(&mut selected.source_sha256);
    let checked_first=std::mem::take(&mut selected.first);
    let checked_second=std::mem::take(&mut selected.second);
    let future_consumed=store.data().live.is_none();
    let pins_at_guest_exit=host.lock().outstanding_pins();
    let store_drop_diagnostic=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
        drop(store)))
        .err().map(|_|"component Store disposal panicked".to_owned());
    // Store disposal is not native-stop. Host table/callback and producer job
    // remain alive until explicit join, completion, resource cleanup and pin settlement.
    let abort_diagnostic=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
        host.lock().abort_unconsumed()))
        .map_err(|_|anyhow::anyhow!("host Send abort panicked"))
        .and_then(|result|result).err().map(|error|format!("{error:#}"));
    let joined={host.lock().take_native()};
    let (native_completion_diagnostic,release_diagnostic)=if let Some(joined)=joined {
        // A producer panic ends native access but not the host's retained
        // task/resource obligations. Finish retirement even when no native
        // result can be trusted or delivered to the closed guest future.
        let native=joined.join().unwrap_or_else(|_|
            Err("native Send producer panicked".to_owned()));
        (host.lock().finish_native(native).err().map(|error|format!("{error:#}")),None)
    } else {
        (None,host.lock().release_unused().err().map(|error|format!("{error:#}")))
    };
    let observed=host.lock().observation();
    let cleanup_failed=observed["cleanup_failures"].as_array()
        .is_some_and(|failures|!failures.is_empty());
    let outcome=if store_drop_diagnostic.is_some() || abort_diagnostic.is_some()
        || release_diagnostic.is_some() || cleanup_failed {"internal-failure"}
        else if observed["denied"]==true {"denied"}
        else if observed["quota_refusal"]=="byte-capacity" {"quota-denied"}
        else if native_completion_diagnostic.is_some() {
            let retired_without_delivery=observed["late_completion_retired"]==true
                && observed["task_retired"]==1 && observed["resource_retired"]==1
                && observed["task_outstanding_pins"]==0 && observed["resource_native_pins"]==0
                && observed["task_reserved_bytes"]==0
                && observed["successful_result_deliveries"]==0;
            if retired_without_delivery && call.is_ok() && future_consumed {
                "native-validation-failure-retired"
            } else {"internal-failure"}
        }
        else if observed["native_error"].as_str().is_some() {"native-failure-retired"}
        else if call.is_ok() && future_consumed {"cancelled-retired"}
        else {"internal-failure"};
    let mut report=json!({"schema":"noble-worker-send-wit-integration/v1",
        "profile":"Worker-Send-WIT-Local", "stage":"component-retirement",
        "outcome":outcome, "world":"noble-test:worker-send/worker-shell@1.0.0",
        "component_sha256":component_sha256,
        "source_sha256":source_sha256,"first_guest":checked_first,"cancel_guest":checked_second,
        "guest_component_error":call.err().map(|error|format!("{error:#}")),
        "future_consumed":future_consumed,"native_pins_at_guest_exit":pins_at_guest_exit,
        "host":observed});
    if let Some(diagnostic)=native_completion_diagnostic {
        report["native_completion_diagnostic"]=Value::String(diagnostic);
    }
    if let Some(diagnostic)=abort_diagnostic {
        report["abort_diagnostic"]=Value::String(diagnostic);
    }
    if let Some(diagnostic)=store_drop_diagnostic {
        report["store_drop_diagnostic"]=Value::String(diagnostic);
    }
    if let Some(diagnostic)=release_diagnostic {
        report["release_diagnostic"]=Value::String(diagnostic);
    }
    Ok(report)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf,
        sync::atomic::{AtomicU64, Ordering}};

    static NEXT_TEST:AtomicU64=AtomicU64::new(1);
    struct PrivateDir(PathBuf);
    impl Drop for PrivateDir {
        fn drop(&mut self) { let _=fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn replaced_component_path_cannot_change_opened_bytes_or_compiled_component() {
        let id=NEXT_TEST.fetch_add(1,Ordering::Relaxed);
        let dir=std::env::temp_dir().join(format!("noble-worker-send-component-{}-{id}",
            std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let private=PrivateDir(dir);
        let path=private.0.join("selected.wasm");
        let replacement=private.0.join("replacement.wasm");
        fs::write(&path,b"(component)").unwrap();
        fs::write(&replacement,b"not a component").unwrap();
        let opened=fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK)
            .open(&path).unwrap();
        fs::rename(&replacement,&path).unwrap();
        let (bytes,sha256)=read_component_file(opened).unwrap();
        assert_eq!(bytes,b"(component)");
        assert_eq!(sha256,"fa69abd0e355ea21aeaf9cd3b36e1bf6a4944ee9df0be50d1c764b43eaa1dce6");
        assert_eq!(fs::read(&path).unwrap(),b"not a component");
        let engine=Engine::new(&config()).unwrap();
        Component::new(&engine,&bytes).unwrap();
    }
}
