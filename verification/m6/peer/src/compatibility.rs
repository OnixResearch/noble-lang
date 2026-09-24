use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::time::Instant;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Engine, Store};
use wasmtime_wasi::clocks::{WasiClocksCtx, WasiClocksCtxView, WasiClocksView};

#[derive(Default)]
struct Clocks {
    clocks: WasiClocksCtx,
    table: ResourceTable,
}
impl WasiClocksView for Clocks {
    fn clocks(&mut self) -> WasiClocksCtxView<'_> {
        WasiClocksCtxView {
            ctx: &mut self.clocks,
            table: &mut self.table,
        }
    }
}

pub async fn run(component_path: &str, mode: &str) -> Result<Value> {
    if matches!(mode, "wasi-p3-clock" | "wasi-stable-version-rejected") {
        return clock(mode).await;
    }
    let mut config = crate::driver::config();
    match mode {
        "native-enabled" => {}
        "async-disabled" => {
            config
                .wasm_component_model_async(false)
                .wasm_component_model_async_builtins(false)
                .wasm_component_model_async_stackful(false);
        }
        "stackful-disabled" => {
            config.wasm_component_model_async_stackful(false);
        }
        _ => bail!("unapproved compatibility mode {mode}"),
    }
    let engine = Engine::new(&config)?;
    let result = Component::from_file(&engine, component_path);
    let diagnostic = result.as_ref().err().map(|error| format!("{error:#}"));
    ensure!(
        result.is_ok() == (mode == "native-enabled"),
        "compatibility outcome disagrees with selected feature contract"
    );
    Ok(
        json!({"schema":"noble-m6-peer/v1","compatibility":{"mode":mode,"stage":"engine-component-admission","outcome":if result.is_ok(){"accepted"}else{"rejected"},"diagnostic":diagnostic,"imports_started":0},"observed_native_execution":false}),
    )
}

async fn clock(mode: &str) -> Result<Value> {
    let engine = Engine::new(&crate::driver::config())?;
    let source = include_str!("clock.wat");
    let source = if mode == "wasi-stable-version-rejected" {
        source.replace("0.3.0-rc-2025-09-16", "0.3.0")
    } else {
        source.to_owned()
    };
    let component = Component::new(&engine, &source)?;
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p3::clocks::add_to_linker(&mut linker)?;
    let mut store = Store::new(&engine, Clocks::default());
    store.set_fuel(100_000)?;
    store.set_epoch_deadline(1);
    let instance = linker.instantiate_async(&mut store, &component).await;
    if mode == "wasi-stable-version-rejected" {
        let error = instance
            .err()
            .context("unpinned stable WASI interface unexpectedly linked")?;
        return Ok(
            json!({"schema":"noble-m6-peer/v1","compatibility":{"mode":mode,"stage":"link","outcome":"rejected","diagnostic":format!("{error:#}"),"imports_started":0},"wasi_version":"0.3.0-rc-2025-09-16","requested_wasi_version":"0.3.0"}),
        );
    }
    let instance = instance?;
    let function = instance
        .get_func(&mut store, "run")
        .context("missing clock component export")?;
    let started = Instant::now();
    store
        .run_concurrent(async |accessor| {
            let task = function.call_concurrent(accessor, &[], &mut []).await?;
            task.block(accessor).await;
            anyhow::Ok(())
        })
        .await??;
    let elapsed = started.elapsed();
    ensure!(
        elapsed.as_nanos() >= 1_000_000,
        "WASI wait-for returned before requested duration"
    );
    Ok(
        json!({"schema":"noble-m6-peer/v1","compatibility":{"mode":mode,"stage":"native-component-execution","outcome":"normal","imports_started":1},"wasi_version":"0.3.0-rc-2025-09-16","operation":"wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for","requested_ns":1000000,"elapsed_ns":elapsed.as_nanos(),"task_exit_observed":true,"non_claims":["stable WASI0.3","all WASI interfaces","Noble u64 surface support"]}),
    )
}
