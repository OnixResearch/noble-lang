mod host;
mod verified;

use anyhow::{Context, Result, bail, ensure};
use host::{Grant, Host, Mode};
use noble_kernel::async_tasks as task;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{future::Future, io::Read, os::unix::fs::OpenOptionsExt, path::Path,
    sync::Arc, task::{Context as PollContext, Poll}, time::Duration};
use wasmtime::component::{Accessor, Component, FutureConsumer, FutureReader, Linker,
    Source, Val, types::ComponentItem};
use wasmtime::{AsContextMut, Config, Engine, Store, StoreContextMut, StoreLimits,
    StoreLimitsBuilder};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../../crates/noble-wasm/wit/worker-send.wit",
        world: "worker-shell",
        exports: { default: async | task_exit },
    });
}

struct Selection {
    attempt:i64,
    text:String,
}

struct Runtime {
    limits:StoreLimits,
    host:Arc<Mutex<Host>>,
    selection:Selection,
    mode:Mode,
    live:Option<(Val,task::Callback)>,
    suspended:Option<tokio::sync::oneshot::Sender<()>>,
    resume:Option<tokio::sync::oneshot::Receiver<()>>,
    disposition:Option<&'static str>,
    delivered:Option<std::result::Result<i64,String>>,
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
    ensure!(*attempt==store.data().selection.attempt
        && text==&store.data().selection.text,
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
    store.data_mut().disposition=Some("cancelled");
    let cancellation=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
        store.data().host.lock().cancel(callback)))
        .map_err(|_|anyhow::anyhow!("host Send cancellation panicked"))
        .and_then(|result|result);
    // The guest has relinquished the future even when the native cancellation
    // fails or unwinds; do not leave a callback capable of delivering success.
    let closed=FutureReader::<std::result::Result<i64,String>>::from_val(&mut store,&live)
        .map(|mut future|future.close(&mut store));
    cancellation?;
    closed?;
    Ok(())
}

/// One-shot consumer of the guest's transferred admitted Send future. It only
/// forwards the realized value: the shell joins the native producer and
/// delivers outside this consumer, so no host lock is held across an await.
struct FinishSend {
    send:Option<tokio::sync::oneshot::Sender<anyhow::Result<std::result::Result<i64,String>>>>,
}
impl FutureConsumer<Runtime> for FinishSend {
    type Item=std::result::Result<i64,String>;
    fn poll_consume(
        mut self:std::pin::Pin<&mut Self>,
        _:&mut PollContext<'_>,
        store:StoreContextMut<Runtime>,
        mut source:Source<'_,Self::Item>,
        finish:bool,
    )->Poll<Result<()>> {
        if finish {return Poll::Ready(Ok(()));}
        let mut value=None;
        source.read(store,&mut value)?;
        let value=value.context("admitted Send future had no payload")?;
        if let Some(send)=self.send.take() {let _=send.send(Ok(value));}
        Poll::Ready(Ok(()))
    }
}
impl Drop for FinishSend {
    fn drop(&mut self) {
        if let Some(send)=self.send.take() {
            let _=send.send(Err(anyhow::anyhow!("Send future consumer vanished before delivery")));
        }
    }
}

async fn finish(accessor:&Accessor<Runtime>,_:u32,params:&[Val],
                results:&mut [Val])->Result<()> {
    let [Val::Future(value)]=params else {bail!("invalid typed WIT finish-send future")};
    ensure!(results.len()==1,"WIT finish-send result arity changed");
    let (callback,mode,host)=accessor.with(|mut access|->Result<(task::Callback,Mode,Arc<Mutex<Host>>)> {
        let data=access.data_mut();
        let (live,callback)=data.live.take().context("unknown or duplicate finish-send")?;
        ensure!(matches!(&live,Val::Future(current) if current==value),
            "foreign Send future cannot be finished");
        data.disposition=Some("finished");
        Ok((callback,data.mode,data.host.clone()))
    })?;
    // The guest future was transferred; only now may the native producer pass
    // its own gate. No guest borrow is held across the join below.
    host.lock().release_native(callback)?;
    let (send,receive)=tokio::sync::oneshot::channel();
    accessor.with(|mut access|->Result<()> {
        let reader=FutureReader::<std::result::Result<i64,String>>::from_val(
            access.as_context_mut(),&params[0])?;
        reader.pipe(access,FinishSend {send:Some(send)});
        Ok(())
    })?;
    let realized=receive.await.context("finish-send consumer vanished")??;
    if mode.suspends() {
        // The shell deliberately withholds the realized result so the guest is
        // parked on this explicit asynchronous finish. Signal the bounded
        // driver, let it retire the pending owner, then return an explicit
        // error: no result was delivered to this invocation.
        let signaled=accessor.with(|mut access|access.data_mut().suspended.take());
        if let Some(signaled)=signaled {let _=signaled.send(());}
        let resume=accessor.with(|mut access|access.data_mut().resume.take())
            .context("suspend resume channel missing")?;
        let _=resume.await;
        let diagnostic=match mode {
            Mode::SuspendAwaitingSend=>"Send suspended awaiting withheld result",
            Mode::DeadlineWithNativeOutstanding=>"host deadline retired the Send before delivery",
            _=>"Send result withheld",
        };
        results[0]=Val::Result(Err(Some(Box::new(Val::String(diagnostic.to_owned())))));
        return Ok(());
    }
    let joined=host.lock().take_native().context("delivery without a pending native producer")?;
    let native=joined.join().unwrap_or_else(|_|
        Err("native Send producer panicked".to_owned()));
    let expected=8+host.lock().grant().text.len();
    if mode.sends() {
        ensure!(matches!(&native,Ok(count) if *count==expected),
            "delivery path did not observe one complete native Send");
    } else {
        ensure!(matches!(&native,Ok(0)),"a withheld Send path issued native bytes");
    }
    host.lock().deliver(&realized)?;
    accessor.with(|mut access|access.data_mut().delivered=Some(realized.clone()));
    results[0]=match &realized {
        Ok(attempt)=>Val::Result(Ok(Some(Box::new(Val::S64(*attempt))))),
        Err(error)=>Val::Result(Err(Some(Box::new(Val::String(error.clone()))))),
    };
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
        let mut finish_seen=false;
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
                ("finish-send",ComponentItem::ComponentFunc(ty)) if ty.async_() && !finish_seen=>{
                    finish_seen=true;
                    linked.func_new_concurrent(operation,|accessor,_,params,results|
                        Box::pin(finish(accessor,0,params,results)))?;
                }
                ("begin-send" | "cancel-send" | "finish-send",ComponentItem::Type(_))=>{},
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

type GuestSummary=(Option<std::result::Result<i64,String>>,bool,Option<&'static str>);

fn accept(returned:Result<(Result<Option<std::result::Result<i64,String>>>,
        bool,Option<&'static str>)>,exit:&mut GuestExit,guest_error:&mut Option<String>,
          summary:&mut GuestSummary) {
    match returned {
        Ok((guest,consumed,disposition))=>{
            *summary=(guest.as_ref().ok().cloned().flatten(),consumed,disposition);
            match guest {
                Ok(value)=>*exit=GuestExit::Returned(value),
                Err(problem)=>*guest_error=Some(format!("{problem:#}")),
            }
        }
        Err(problem)=>*guest_error=Some(format!("{problem:#}")),
    }
}

const HOST_DEADLINE:Duration=Duration::from_secs(2);

enum GuestExit {
    Returned(Option<std::result::Result<i64,String>>),
    Suspended,
    Deadline,
}

async fn execute(arguments:&[String])->Result<Value> {
    let [_,component_path,cli_path,package_path,schema_path,state_path,submit_path,cancel_path,grant_path,mode]=arguments else {
        bail!("usage: noble-worker-send-peer COMPONENT NOBLE PACKAGE SCHEMA STATE SUBMIT CANCEL HOST_GRANT MODE")
    };
    let submitted=verified::submit(Path::new(cli_path),Path::new(package_path),Path::new(schema_path),
        Path::new(state_path),Path::new(submit_path),Path::new(cancel_path))?;
    let second=submitted.cancelled()?;
    let grant=Grant::selected(Path::new(grant_path))?;
    let mode=Mode::parse(mode)?;
    let (component_bytes,component_sha256)=read_component(Path::new(component_path))?;
    let engine=Engine::new(&config())?;
    let component=Component::new(&engine,&component_bytes)?;
    let host=Arc::new(Mutex::new(Host::new(grant,mode,&submitted.source_sha256)?));
    host.lock().record("submit-worker-event");
    let (suspended_tx,suspended_rx)=tokio::sync::oneshot::channel();
    let (resume_tx,resume_rx)=tokio::sync::oneshot::channel();
    let mut store=Store::new(&engine,Runtime {
        limits:StoreLimitsBuilder::new().memory_size(4_194_304).instances(16).tables(16).build(),
        host:host.clone(),
        selection:Selection {attempt:submitted.attempt,text:submitted.text.clone()},
        mode,live:None,suspended:Some(suspended_tx),resume:Some(resume_rx),
        disposition:None,delivered:None,
    });
    // Instantiation runs core start functions, which may already have begun a
    // native Send before trapping or failing typed export lookup. Keep all
    // guest and Store work in one bounded, unwind-safe phase: instantiate,
    // invoke the selected export, apply the bounded driver's suspension or
    // deadline decision while the Store still owns the guest, then release it.
    let phase=async {
        let checked=async {
            tokio::time::timeout(Duration::from_secs(10),async {
                store.limiter(|runtime|&mut runtime.limits);
                store.set_fuel(100_000)?;
                store.fuel_async_yield_interval(Some(1_000))?;
                store.set_epoch_deadline(1);
                let mut linker=Linker::new(&engine);
                link(&engine,&component,&mut linker)?;
                let instance=linker.instantiate_async(&mut store,&component).await?;
                let typed=bindings::WorkerShell::new(&mut store,&instance)?;
                let attempt=store.data().selection.attempt;
                let text=store.data().selection.text.clone();
                let delivery=store.data().mode.awaits_finish();
                let guest=store.run_concurrent(async |accessor| {
                    if delivery {
                        let (value,task)=typed.call_dispatch_result(accessor,attempt,text).await?;
                        task.block(accessor).await;
                        Ok::<Option<std::result::Result<i64,String>>,anyhow::Error>(Some(value))
                    } else {
                        let ((),task)=typed.call_dispatch(accessor,attempt,text).await?;
                        task.block(accessor).await;
                        Ok::<Option<std::result::Result<i64,String>>,anyhow::Error>(None)
                    }
                }).await;
                // The guest may have trapped or cancelled after relinquishing its
                // future; the handler disposition and consumption are still the
                // real retained state, so report them on both paths.
                let guest=match guest {Ok(inner)=>inner,Err(error)=>Err(error)};
                let consumed=store.data().live.is_none();
                let disposition=store.data().disposition;
                Ok::<_,anyhow::Error>((guest,consumed,disposition))
            }).await.map_err(anyhow::Error::from).and_then(|result|result)
        };
        let mut checked=std::pin::pin!(checked);
        let polled=std::future::poll_fn(|cx|->
            std::task::Poll<Result<(Result<Option<std::result::Result<i64,String>>>,
                bool,Option<&'static str>)>> {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(||checked.as_mut().poll(cx)))
                .unwrap_or_else(|_|std::task::Poll::Ready(Err(anyhow::anyhow!(
                    "component guest phase panicked"))))
        });
        let mut polled=std::pin::pin!(polled);
        let mut exit=GuestExit::Returned(None);
        let mut guest_error=None::<String>;
        let mut summary=(None::<std::result::Result<i64,String>>,false,None::<&'static str>);
        if mode.suspends() {
            if mode==Mode::DeadlineWithNativeOutstanding {
                tokio::select! {
                    _={
                        let suspended=suspended_rx;
                        async move {
                            if suspended.await.is_err() {std::future::pending::<()>().await;}
                            tokio::time::sleep(HOST_DEADLINE).await;
                        }
                    }=>exit=GuestExit::Deadline,
                    returned=&mut polled=>accept(returned,&mut exit,&mut guest_error,&mut summary),
                }
            } else {
                tokio::select! {
                    _=suspended_rx=>exit=GuestExit::Suspended,
                    returned=&mut polled=>accept(returned,&mut exit,&mut guest_error,&mut summary),
                }
            }
        } else {
            let returned=(&mut polled).await;
            accept(returned,&mut exit,&mut guest_error,&mut summary);
        }
        let mut driver_diagnostic=None::<String>;
        if mode.suspends() && !matches!(&exit,GuestExit::Returned(_)) {
            let callback=host.lock().pending_callback()
                .context("guest parked but the shell never admitted a Send")?;
            let retired=if matches!(&exit,GuestExit::Deadline) {
                host.lock().deadline(callback,
                    "bounded driver deadline elapsed with native work outstanding")
            } else {
                host.lock().suspend(callback)
            };
            // Release the parked guest: it observes the withheld finish and the
            // guest phase terminates with an explicit error, not a fabrication.
            let _=resume_tx.send(());
            let returned=(&mut polled).await;
            accept(returned,&mut exit,&mut guest_error,&mut summary);
            driver_diagnostic=retired.err().map(|problem|format!("{problem:#}"));
        }
        Ok::<_,anyhow::Error>((exit,summary.1,summary.2,guest_error,driver_diagnostic))
    };
    let (exit,future_consumed,disposition,guest_component_error,driver_diagnostic)=phase.await
        .unwrap_or_else(|problem|(GuestExit::Returned(None),false,None,
            Some(format!("{problem:#}")),None));
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
    let delivered_value=match &exit {
        GuestExit::Returned(value)=>value.clone(),
        _=>None,
    };
    let delivery_mode=mode.awaits_finish();
    let joined={host.lock().take_native()};
    let (native_completion_diagnostic,release_diagnostic)=if let Some(joined)=joined {
        // A producer panic ends native access but not the host's retained
        // task/resource obligations. Finish retirement even when no native
        // result can be trusted or delivered to the closed guest future.
        let native=joined.join().unwrap_or_else(|_|
            Err("native Send producer panicked".to_owned()));
        if delivery_mode && !mode.suspends() {
            // The finish consumer already validated and delivered the result;
            // only a second delivery would be reachable here.
            (None,None)
        } else {
            (host.lock().finish_native(native).err().map(|error|format!("{error:#}")),None)
        }
    } else if delivery_mode && !mode.suspends() {
        // The finish future already joined and released this delivery.
        (None,None)
    } else {
        (None,host.lock().release_unused().err().map(|error|format!("{error:#}")))
    };
    let invocation=delivered_value.as_ref().filter(|_|!mode.suspends())
        .and_then(|value|host.lock().finish_invocation(value).err())
        .map(|error|format!("{error:#}"));
    // A cancellation offered after one-shot delivery must be a duplicate; it
    // may not revoke the returned owner or restart retirement.
    if mode==Mode::DeadlineWithNativeOutstanding && driver_diagnostic.is_none() {
        host.lock().deadline_settled();
    }
    let duplicate=if delivery_mode && !mode.suspends() {
        host.lock().cancel_after_delivery().err().map(|error|format!("{error:#}"))
    } else {None};
    let observed=host.lock().observation();
    let cleanup_failed=observed["cleanup_failures"].as_array()
        .is_some_and(|failures|!failures.is_empty());
    let sequence=observed["lifecycle_sequence"].as_array().cloned().unwrap_or_default();
    let has=|event:&str|sequence.iter().any(|item|item.as_str()==Some(event));
    let retired_without_delivery=observed["late_completion_retired"]==true
        && observed["task_retired"]==1 && observed["resource_retired"]==1
        && observed["task_outstanding_pins"]==0 && observed["resource_native_pins"]==0
        && observed["task_reserved_bytes"]==0
        && observed["successful_result_deliveries"]==0;
    let outcome=if store_drop_diagnostic.is_some() || abort_diagnostic.is_some()
        || release_diagnostic.is_some() || driver_diagnostic.is_some()
        || invocation.is_some() || duplicate.is_some() || cleanup_failed
        || (delivery_mode && !mode.suspends() && delivered_value.is_none()) {"internal-failure"}
        else if observed["denied"]==true {"denied"}
        else if observed["quota_refusal"]=="byte-capacity" {"quota-denied"}
        else if has("deadline-retirement") {"host-deadline-retired"}
        else if has("suspension-retirement") {"suspended-retired"}
        else if let Some(delivered)=observed["delivered_result"].as_object() {
            if delivered.contains_key("ok") {"delivered-once"} else {"domain-error-delivered"}
        }
        else if native_completion_diagnostic.is_some() {
            if retired_without_delivery && future_consumed {
                "native-validation-failure-retired"
            } else {"internal-failure"}
        }
        else if observed["native_error"].as_str().is_some() {"native-failure-retired"}
        else if guest_component_error.is_none() && future_consumed
            && has("cancellation-commit") {"cancelled-retired"}
        else {"internal-failure"};
    let mut report=json!({"schema":"noble-worker-send-wit-integration/v1",
        "profile":"Worker-Send-WIT-Local", "stage":"component-retirement",
        "outcome":outcome, "mode":format!("{mode:?}"),
        "world":"noble-test:worker-send/worker-shell@1.0.0",
        "component_sha256":component_sha256,
        "source_sha256":submitted.source_sha256,"first_guest":submitted.first,"cancel_guest":second,
        "guest_component_error":guest_component_error,
        "future_consumed":future_consumed,"native_pins_at_guest_exit":pins_at_guest_exit,
        "handle_disposition":disposition,
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
    if let Some(diagnostic)=driver_diagnostic {
        report["driver_diagnostic"]=Value::String(diagnostic);
    }
    if let Some(diagnostic)=invocation {
        report["invocation_diagnostic"]=Value::String(diagnostic);
    }
    if let Some(diagnostic)=duplicate {
        report["duplicate_cancellation_diagnostic"]=Value::String(diagnostic);
    }
    Ok(report)
}

#[tokio::main(flavor="current_thread")]
async fn main() {
    let args:Vec<String>=std::env::args().collect();
    match execute(&args).await {
        Ok(report)=>{
            let outcome=report["outcome"].as_str().unwrap_or("internal-failure");
            let failed=!matches!(outcome,"delivered-once" | "domain-error-delivered"
                | "cancelled-retired" | "denied" | "suspended-retired" | "host-deadline-retired");
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
