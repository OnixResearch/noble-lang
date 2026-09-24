//! Independent Rust/Wasmtime Canonical ABI peer. No Noble binding or emitter code is used.
mod gc_control;
mod hosts;
mod values;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::Read;
use wasmtime::{
    component::{Component, Linker, Val},
    Config, Engine, Store, StoreLimitsBuilder,
};

fn execute(component_path: &str, function: &str, input: &Value) -> Result<Value> {
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, component_path)?;
    let mut linker = Linker::new(&engine);
    hosts::link(&engine, &component, &mut linker)?;
    let limits = StoreLimitsBuilder::new()
        .memory_size(4_194_304)
        .instances(64)
        .tables(64)
        .build();
    let mut host = hosts::Host::new(limits)?;
    host.allow_protected = !input["deny_protected"].as_bool().unwrap_or(false);
    host.counter_error = input["counter_error"].as_bool().unwrap_or(false);
    let mut store = Store::new(&engine, host);
    store.limiter(|host| &mut host.limits);
    store.set_fuel(10_000_000)?;
    let instance = linker.instantiate(&mut store, &component)?;
    let function = instance
        .get_func(&mut store, function)
        .context("missing component export")?;
    // Input conversion can acquire owners before a later argument fails.
    // Every conversion and execution failure therefore shares the cleanup path.
    let lifted = (|| {
        let arguments = if input.is_array() {
            input
        } else {
            &input["arguments"]
        };
        let args = arguments
            .as_array()
            .context("input must provide typed arguments")?
            .iter()
            .map(|value| hosts::decode_input(&mut store, value))
            .collect::<Result<Vec<_>>>()?;
        let mut results = vec![Val::Bool(false); function.ty(&store).results().len()];
        function.call(&mut store, &args, &mut results)?;
        let outputs = results
            .iter()
            .map(|value| hosts::encode_output(&mut store, value))
            .collect::<Result<Vec<_>>>()?;
        function.post_return(&mut store)?;
        Ok::<_, anyhow::Error>(outputs)
    })();
    store.data_mut().cleanup(lifted.is_err())?;
    let observation = match lifted {
        Ok(outputs) => json!({"outcome":"normal", "results":outputs, "post_return":true}),
        Err(error) => json!({"outcome":"trap", "error":format!("{error:#}"),
            "results":[], "partial_trusted_values_published":0, "post_return":false}),
    };
    let report = json!({"schema":"noble-m5-peer/v1", "engine":"wasmtime-40.0.2",
        "observation":observation, "host":store.data().report(),
        "remaining_fuel":store.get_fuel()?, "cleanup":"store-dropped-before-report"});
    drop(store);
    Ok(report)
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    anyhow::ensure!(args.len() == 3,
        "usage: noble-m5-peer COMPONENT EXPORT < TYPED_ARGUMENT_ARRAY | --resource-control cancellation-gc");
    if args[1] == "--resource-control" {
        let report = gc_control::execute(&args[2])?;
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    anyhow::ensure!(
        !args[1].starts_with("--"),
        "unknown peer command: {}",
        args[1]
    );
    let mut input = Vec::new();
    std::io::stdin().take(65_537).read_to_end(&mut input)?;
    anyhow::ensure!(input.len() <= 65_536, "input byte limit exceeded");
    let report = execute(&args[1], &args[2], &serde_json::from_slice(&input)?)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
