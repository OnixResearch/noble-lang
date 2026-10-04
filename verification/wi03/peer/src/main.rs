//! Independent Wasmtime component probe for the import-free, synchronous `echo: u64 -> u64`.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{io::Write, path::Path, process::{Command, Stdio}};
use wasmtime::{component::{Component, Linker, Type, Val}, Config, Engine, Store, StoreLimitsBuilder};

const VALUES: [u64; 5] = [0, 42, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX];

// Hash the very bytes passed to Wasmtime, not a separately reopened path.
fn sha256(bytes: &[u8]) -> Result<String> {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()
        .context("starting sha256sum")?;
    child.stdin.take().context("missing sha256sum stdin")?.write_all(bytes)?;
    let output = child.wait_with_output()?;
    anyhow::ensure!(output.status.success(), "sha256sum failed: {}", output.status);
    let output = std::str::from_utf8(&output.stdout)?;
    let hash = output.split_whitespace().next().context("missing SHA256")?;
    anyhow::ensure!(hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()), "invalid SHA256");
    Ok(hash.to_owned())
}

fn encoded(value: &Val) -> Value {
    match value {
        Val::U64(n) => json!({"type":"u64", "value":n.to_string()}),
        Val::S64(n) => json!({"type":"s64", "value":n.to_string()}),
        other => json!({"type":"unexpected", "debug":format!("{other:?}")}),
    }
}

fn probe(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let hash = sha256(&bytes)?;
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::from_binary(&engine, &bytes)?;
    let linker = Linker::<wasmtime::StoreLimits>::new(&engine);
    let mut observations = Vec::with_capacity(VALUES.len());
    for (index, input) in VALUES.into_iter().enumerate() {
        // A trap may poison an instance. Each value gets an independent, fresh guest.
        let limits = StoreLimitsBuilder::new().memory_size(4_194_304).instances(64).tables(64).build();
        let mut store = Store::new(&engine, limits);
        store.limiter(|limits| limits);
        store.set_fuel(10_000_000)?;
        let instance = linker.instantiate(&mut store, &component)
            .context("instantiating import-free component")?;
        let echo = instance.get_func(&mut store, "echo").context("missing echo export")?;
        if index == 0 {
            let ty = echo.ty(&store);
            let mut params = ty.params();
            let mut results = ty.results();
            if !matches!(params.next(), Some((_, Type::U64))) || params.next().is_some()
                || !matches!(results.next(), Some(Type::U64)) || results.next().is_some() {
                bail!("echo must have component type func(u64) -> u64");
            }
        }
        let mut outputs = [Val::Bool(false)];
        let outcome = match echo.call(&mut store, &[Val::U64(input)], &mut outputs) {
            Ok(()) => {
                let result = encoded(&outputs[0]);
                echo.post_return(&mut store)?;
                json!({"outcome":"normal", "result":result, "post_return":true})
            }
            Err(error) => json!({"outcome":"trap", "error":format!("{error:#}"), "post_return":false}),
        };
        observations.push(json!({"input":{"type":"u64", "value":input.to_string()}, "invocation":outcome}));
    }
    Ok(json!({"schema":"noble-wi03-peer/v1", "engine":"wasmtime-40.0.2",
        "component":{"path":path.display().to_string(), "sha256":hash},
        "export":"echo", "observations":observations}))
}

fn main() -> Result<()> {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_default();
    let path = args.next().context("usage: noble-wi03-peer COMPONENT_PATH")?;
    anyhow::ensure!(args.next().is_none(), "usage: {} COMPONENT_PATH", Path::new(&program).display());
    println!("{}", serde_json::to_string(&probe(Path::new(&path))?)?);
    Ok(())
}
