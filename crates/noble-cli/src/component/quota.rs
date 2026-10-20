//! Wasmtime execution of a source-matched, import-free Noble-emitted Core module.
//! Engine fuel and the generated guest heap are independent budgets. The
//! private allocator diagnostic is trusted only after rebuilding the complete
//! Noble source recipe and comparing every emitted Core byte.

use anyhow::{bail, ensure, Context as _, Result};
use std::ffi::OsString;
use wasmtime::{Config, Engine, Instance, Module, Store, StoreLimitsBuilder, Trap};

use crate::workflow::encoding::{self, Json};

const MAX_CORE_BYTES: usize = 4_194_304;
const MEMORY_BYTES: usize = 1_048_576;
const HEAP_START: usize = 65_536;
const MAX_ALLOCATION: u32 = (MEMORY_BYTES - HEAP_START) as u32;
const MAX_FUEL: u64 = 10_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Export {
    Compute,
    Text,
}

fn selected_export(name: &str) -> Result<Export> {
    match name {
        "compute" => Ok(Export::Compute),
        "emit-fifty-six" | "emit-fifty-seven" | "emit-sixty-five" => Ok(Export::Text),
        _ => bail!("unapproved Noble quota export"),
    }
}

struct Metrics {
    allocations: u64,
    allocated_bytes: u64,
    copied_bytes: u64,
    live_bytes: u64,
    cleanups: u64,
    quota_exceeded: bool,
}

fn metrics(store: &mut Store<wasmtime::StoreLimits>, instance: &Instance) -> Result<Metrics> {
    let counter = |store: &mut Store<wasmtime::StoreLimits>, name| -> Result<u64> {
        let function = instance.get_typed_func::<(), i64>(&mut *store, name)?;
        Ok(u64::try_from(function.call(&mut *store, ())?)?)
    };
    let allocations = counter(store, "noble$allocation-count")?;
    let allocated_bytes = counter(store, "noble$allocated-bytes")?;
    let copied_bytes = counter(store, "noble$copied-bytes")?;
    let cleanups = counter(store, "noble$cleanup-count")?;
    let live = instance.get_typed_func::<(), i32>(&mut *store, "noble$live-bytes")?;
    let live_bytes = u64::try_from(live.call(&mut *store, ())?)?;
    let quota = instance.get_typed_func::<(), i32>(&mut *store, "noble$quota-exceeded")?;
    let flag = quota.call(&mut *store, ())?;
    ensure!(flag == 0 || flag == 1, "invalid allocator quota diagnostic");
    Ok(Metrics { allocations, allocated_bytes, copied_bytes, live_bytes, cleanups,
        quota_exceeded: flag == 1 })
}

fn text_result(store: &mut Store<wasmtime::StoreLimits>, instance: &Instance, area: i32) -> Result<Vec<u8>> {
    let memory = instance.get_memory(&mut *store, "memory").context("missing generated guest memory")?;
    let data = memory.data(&*store);
    let area = usize::try_from(area)?;
    let record = data.get(area..area.checked_add(8).context("result area overflow")?)
        .context("invalid result area")?;
    let pointer = u32::from_le_bytes(record[0..4].try_into()?) as usize;
    let length = u32::from_le_bytes(record[4..8].try_into()?) as usize;
    ensure!(pointer >= HEAP_START, "result is not in the generated heap");
    let text = data.get(pointer..pointer.checked_add(length).context("result length overflow")?)
        .context("invalid result text range")?;
    std::str::from_utf8(text).context("invalid generated UTF-8 result")?;
    Ok(text.to_vec())
}

#[derive(Clone, Copy)]
enum Returned {
    Integer(i64),
    Text(i32),
}

fn execute(arguments: &[OsString]) -> Result<Json> {
    let [_, _, path, name, fuel, allocation, wit_path, world_name, sources @ ..] = arguments else {
        bail!("usage: noble component quota-core CORE_WASM EXPORT FUEL ALLOCATION_BYTES WIT WORLD EXPORT=SOURCE ...")
    };
    ensure!(!sources.is_empty(), "complete Noble source recipe required");
    let name = name.to_str().context("export must be UTF-8")?;
    let kind = selected_export(name)?;
    let fuel = fuel.to_str().context("fuel must be UTF-8")?.parse::<u64>()?;
    let allocation = allocation.to_str().context("allocation limit must be UTF-8")?.parse::<u32>()?;
    ensure!((1..=MAX_FUEL).contains(&fuel), "fuel outside host budget");
    ensure!(allocation <= MAX_ALLOCATION, "allocation outside generated heap budget");
    let core = crate::workflow::read_bounded(std::path::Path::new(path), MAX_CORE_BYTES,
        "quota-core-artifact").map_err(|error| anyhow::anyhow!("core artifact: {}", error.json().encode()))?;
    let wit = crate::workflow::read_bounded(std::path::Path::new(wit_path), 65_536,
        "component-wit-input").map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let world_name = world_name.to_str().context("world name must be UTF-8")?;
    let world = noble_contracts::component::World::parse(
        &wit, world_name, crate::core::SOURCE_LIMITS
    ).map_err(super::report::diagnostic)
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let sources = super::sources(sources)
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let prepared = sources.iter().map(|source| {
        world.prepare_export(&source.name, &source.bytes, crate::core::SOURCE_LIMITS)
            .map_err(super::report::diagnostic)
    }).collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let artifact = noble_wasm::component::compile(&world, &prepared)
        .map_err(super::report::backend)
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let rebuilt = super::artifacts::assemble(&world, artifact.wat())
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    ensure!(core == rebuilt.core, "Core artifact does not match the independently compiled Noble source recipe");
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let module = Module::new(&engine, core)?;
    ensure!(module.imports().next().is_none(), "quota Core module requires a host import");
    let mut store = Store::new(&engine, StoreLimitsBuilder::new().memory_size(4_194_304)
        .instances(16).tables(16).build());
    store.limiter(|limits| limits);
    store.set_fuel(MAX_FUEL)?;
    let instance = Instance::new(&mut store, &module, &[])?;
    let memory = instance.get_memory(&mut store, "memory").context("missing generated guest memory")?;
    ensure!(memory.data_size(&store) == MEMORY_BYTES, "unexpected initial guest memory");
    let setter = instance.get_typed_func::<i32, ()>(&mut store, "noble$set-allocation-limit")?;
    setter.call(&mut store, i32::try_from(allocation)?)?;
    ensure!(!metrics(&mut store, &instance)?.quota_exceeded, "allocator failed before execution");
    let export = std::format!("cm32p2||{name}");
    let scalar = if kind == Export::Compute {
        Some(instance.get_typed_func::<(), i64>(&mut store, &export)?)
    } else { None };
    let text = if kind == Export::Text {
        Some(instance.get_typed_func::<(), i32>(&mut store, &export)?)
    } else { None };
    // Instantiation, quota setup, export lookup and diagnostics have already
    // completed. Exactly this budget applies to the selected guest invocation.
    store.set_fuel(fuel)?;
    let called = match kind {
        Export::Compute => scalar.context("missing integer export")?.call(&mut store, ()).map(Returned::Integer),
        Export::Text => text.context("missing text export")?.call(&mut store, ()).map(Returned::Text),
    };
    let fuel_remaining = store.get_fuel()?;
    let trap = called.as_ref().err().and_then(|error| error.downcast_ref::<Trap>()).copied();
    // Private diagnostic calls and cleanup must not be charged to the target
    // call's one-unit fuel budget. Preserve the measured remaining fuel first.
    store.set_fuel(MAX_FUEL)?;
    let before = metrics(&mut store, &instance)?;
    let (outcome, quota_reason, returned_integer, returned_hex, reported_success) = match called {
        Ok(value) => {
            ensure!(!before.quota_exceeded, "allocator flagged quota after success");
            let (integer, bytes) = match value {
                Returned::Integer(integer) => (Some(integer), None),
                Returned::Text(area) => (None, Some(text_result(&mut store, &instance, area)?)),
            };
            let post = std::format!("cm32p2||{name}_post");
            match value {
                Returned::Integer(integer) => instance.get_typed_func::<i64, ()>(&mut store, &post)?.call(&mut store, integer)?,
                Returned::Text(area) => instance.get_typed_func::<i32, ()>(&mut store, &post)?.call(&mut store, area)?,
            }
            ("normal", None, integer, bytes.map(|bytes| encoding::hex(&bytes)), true)
        }
        Err(_) => {
            instance.get_typed_func::<(), ()>(&mut store, "noble$cleanup")?.call(&mut store, ())?;
            match trap {
                Some(Trap::OutOfFuel) if fuel_remaining == 0 && !before.quota_exceeded =>
                    ("specified-quota-failure", Some("fuel"), None, None, false),
                Some(Trap::UnreachableCodeReached) if before.quota_exceeded =>
                    ("specified-quota-failure", Some("allocation"), None, None, false),
                _ => ("guest-trap", None, None, None, false),
            }
        }
    };
    let after = metrics(&mut store, &instance)?;
    ensure!(after.live_bytes == 0 && after.cleanups == before.cleanups + 1,
        "guest cleanup did not discharge generated heap");
    Ok(encoding::object([
        ("schema", encoding::string("noble-runtime-quota/v1")),
        ("stage", encoding::string("execution")),
        ("outcome", encoding::string(outcome)),
        ("reported_success", Json::Bool(reported_success)),
        ("quota_reason", encoding::optional_string(quota_reason)),
        ("export", encoding::string(name)),
        ("fuel", Json::Number(fuel)),
        ("fuel_remaining_at_return", Json::Number(fuel_remaining)),
        ("allocation_limit_bytes", Json::Number(u64::from(allocation))),
        ("wasm_memory_bytes", Json::Number(MEMORY_BYTES as u64)),
        ("guest_callbacks", Json::Number(0)),
        ("returned_i64", returned_integer.map_or(Json::Null, |value| encoding::string(value.to_string()))),
        ("returned_hex", returned_hex.map_or(Json::Null, encoding::string)),
        ("wasmtime_trap", trap.map_or(Json::Null, |value| encoding::string(std::format!("{value:?}")))),
        ("quota_exceeded", Json::Bool(before.quota_exceeded)),
        ("allocations_before_cleanup", Json::Number(before.allocations)),
        ("allocated_bytes_before_cleanup", Json::Number(before.allocated_bytes)),
        ("copied_bytes_before_cleanup", Json::Number(before.copied_bytes)),
        ("live_bytes_before_cleanup", Json::Number(before.live_bytes)),
        ("live_bytes_after_cleanup", Json::Number(after.live_bytes)),
        ("cleanups", Json::Number(after.cleanups)),
    ]))
}

pub(super) fn run(arguments: &[OsString]) -> std::process::ExitCode {
    match execute(arguments) {
        Ok(report) => { println!("{}", report.encode()); std::process::ExitCode::SUCCESS }
        Err(error) => {
            eprintln!("quota-core: {error:#}");
            std::process::ExitCode::from(2)
        }
    }
}
