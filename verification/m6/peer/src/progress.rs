use crate::{adapter::Host, driver::Shared};
use anyhow::{Context, Result, bail, ensure};
use noble_kernel::async_tasks::Failure;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context as PollContext, Poll},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

const RUNNABLE: &str = r#"(component
 (core module $m (func (export "run") (loop $forever (br $forever))))
 (core instance $i (instantiate $m))
 (func (export "run") async (canon lift (core func $i "run") async)))"#;
const WORK_MS: u64 = 60;
const DEADLINE_MS: u64 = 3;
const FUEL: u64 = 10_000;
const QUANTUM: u64 = 100;
type Jobs = Arc<Mutex<Vec<std::thread::JoinHandle<Result<u128>>>>>;

struct Metered<F: Future> {
    future: Pin<Box<F>>,
    polls: Arc<AtomicU64>,
    longest: Arc<AtomicU64>,
}
impl<F: Future> Future for Metered<F> {
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut PollContext<'_>) -> Poll<Self::Output> {
        let start = Instant::now();
        let result = self.future.as_mut().poll(cx);
        self.polls.fetch_add(1, Ordering::SeqCst);
        self.longest.fetch_max(
            u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Ordering::SeqCst,
        );
        result
    }
}

fn start_native(
    host: &Shared,
    jobs: &Jobs,
    started: Instant,
) -> Result<oneshot::Receiver<Result<()>>> {
    ensure!(
        jobs.lock().is_empty(),
        "one native operation per progress probe"
    );
    let callback = host.lock().admit("first", 8, 64, Some(40))?;
    let (send, receive) = oneshot::channel();
    let retained = host.clone();
    jobs.lock().push(std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(WORK_MS));
        let result = (|| {
            let mut host = retained.lock();
            host.complete(callback, false, 8)?;
            host.settle(callback)?;
            Ok(())
        })();
        let error = result
            .as_ref()
            .err()
            .map(|e: &anyhow::Error| format!("{e:#}"));
        let _ = send.send(result);
        match error {
            Some(error) => bail!("{error}"),
            None => Ok(started.elapsed().as_millis()),
        }
    }));
    Ok(receive)
}

pub async fn run(case: &str) -> Result<Value> {
    ensure!(
        matches!(
            case,
            "runnable-fuel" | "runnable-epoch" | "blocking-deadline"
        ),
        "unapproved progress probe"
    );
    let engine = Engine::new(&crate::driver::config())?;
    let component = Component::new(
        &engine,
        if case == "blocking-deadline" {
            include_str!("clock.wat")
        } else {
            RUNNABLE
        },
    )?;
    let host = Arc::new(Mutex::new(Host::new("success")?));
    let jobs: Jobs = Arc::new(Mutex::new(Vec::with_capacity(1)));
    let mut linker = Linker::new(&engine);
    let started = Instant::now();
    if case == "blocking-deadline" {
        let jobs = jobs.clone();
        let host = host.clone();
        linker
            .instance("wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16")?
            .func_new_concurrent("wait-for", move |_accessor, _ty, _params, _results| {
                let jobs = jobs.clone();
                let host = host.clone();
                Box::pin(async move {
                    start_native(&host, &jobs, started)?
                        .await
                        .context("worker lost completion")??;
                    Ok(())
                })
            })?;
    }
    // A live independently blocking native operation remains pinned while Wasm is continuously runnable.
    let retained_signal = if case != "blocking-deadline" {
        Some(start_native(&host, &jobs, started)?)
    } else {
        None
    };
    let mut store = Store::new(&engine, ());
    store.set_fuel(if case == "runnable-fuel" {
        FUEL
    } else {
        100_000_000
    })?;
    store.fuel_async_yield_interval(Some(QUANTUM))?;
    store.set_epoch_deadline(1);
    let instance = linker.instantiate_async(&mut store, &component).await?;
    let function = instance
        .get_func(&mut store, "run")
        .context("missing run export")?;
    let epoch = if case == "runnable-epoch" {
        let engine = engine.clone();
        Some(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(DEADLINE_MS));
            engine.increment_epoch();
        }))
    } else {
        None
    };
    let beats = Arc::new(AtomicU64::new(0));
    let stopped = Arc::new(AtomicBool::new(false));
    let heartbeat = {
        let beats = beats.clone();
        let stopped = stopped.clone();
        tokio::spawn(async move {
            while !stopped.load(Ordering::SeqCst) {
                beats.fetch_add(1, Ordering::SeqCst);
                tokio::task::yield_now().await;
            }
        })
    };
    let polls = Arc::new(AtomicU64::new(0));
    let longest = Arc::new(AtomicU64::new(0));
    let invocation = store.run_concurrent(async |accessor| {
        let task = function.call_concurrent(accessor, &[], &mut []).await?;
        task.block(accessor).await;
        anyhow::Ok(())
    });
    let measured = Metered {
        future: Box::pin(invocation),
        polls: polls.clone(),
        longest: longest.clone(),
    };
    let diagnostic = match tokio::time::timeout(
        Duration::from_millis(if case == "blocking-deadline" {
            DEADLINE_MS
        } else {
            1000
        }),
        measured,
    )
    .await
    {
        Err(_) if case == "blocking-deadline" => "driver deadline elapsed".to_owned(),
        Err(_) => bail!("continuously runnable guest failed interruption bound"),
        Ok(Ok(Err(error))) => format!("{error:#}"),
        Ok(Err(error)) => format!("{error:#}"),
        Ok(Ok(Ok(()))) => bail!("unbounded workload unexpectedly completed"),
    };
    let outcome = if case == "runnable-fuel" {
        "budget"
    } else {
        "deadline"
    };
    host.lock().fail_all(if outcome == "budget" {
        Failure::Budget
    } else {
        Failure::Deadline
    })?;
    let cancel_ack_ms = started.elapsed().as_millis();
    let pins_at_ack = host.lock().native_pins();
    let fuel_remaining = store.get_fuel()?;
    let at_ack = host.lock().report();
    // Wasmtime40 has no task cancellation; this profile owns one isolated invocation per Store.
    // Destroying it revokes guest access, never releases retained native ownership by assertion.
    drop(store);
    drop(retained_signal);
    stopped.store(true, Ordering::SeqCst);
    heartbeat.await?;
    if let Some(epoch) = epoch {
        epoch
            .join()
            .map_err(|_| anyhow::anyhow!("epoch ticker panic"))?;
    }
    let mut native_complete_ms = 0;
    for job in std::mem::take(&mut *jobs.lock()) {
        native_complete_ms = job.join().map_err(|_| anyhow::anyhow!("native panic"))??;
    }
    let pins_after_join = host.lock().native_pins();
    ensure!(
        pins_at_ack == 1 && pins_after_join == 0,
        "native pin lifetime violated"
    );
    ensure!(
        cancel_ack_ms < native_complete_ms && native_complete_ms >= u128::from(WORK_MS),
        "cancellation did not progress independently of blocking work"
    );
    ensure!(
        beats.load(Ordering::SeqCst) > 0,
        "cooperative driver made no unrelated progress"
    );
    ensure!(
        longest.load(Ordering::SeqCst) < 100_000_000,
        "observed runnable unit exceeded probe wall bound"
    );
    if case == "runnable-fuel" {
        ensure!(
            fuel_remaining == 0 && diagnostic.contains("fuel"),
            "not a fuel exhaustion"
        );
    }
    if case == "runnable-epoch" {
        ensure!(diagnostic.contains("interrupt"), "not an epoch interrupt");
    }
    Ok(
        json!({"schema":"noble-m6-peer/v1","engine":"wasmtime-40.0.2","progress":{"case":case,"outcome":outcome,"elapsed_ms":started.elapsed().as_millis(),"cancel_ack_ms":cancel_ack_ms,"native_complete_ms":native_complete_ms,"heartbeat_progress":beats.load(Ordering::SeqCst),"guest_budget":if case=="runnable-fuel"{FUEL}else{100_000_000},"guest_work_remaining":fuel_remaining,"native_pins_at_ack":pins_at_ack,"native_pins_after_join":pins_after_join,"fuel_quantum":QUANTUM,"poll_count":polls.load(Ordering::SeqCst),"longest_poll_ns":longest.load(Ordering::SeqCst),"requested_deadline_ms":DEADLINE_MS,"diagnostic":diagnostic},"at_ack":at_ack,"host":host.lock().report(),"native_jobs_joined":true,"host_retained_after_store_drop":true,"scope":"measured real native async engine interruption; configured fuel quantum, external epoch tick and admission-bounded blocking worker; host OS scheduling remains trusted"}),
    )
}
