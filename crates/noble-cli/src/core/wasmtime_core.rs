//! In-process Wasmtime Core engine for opt-in `Core-Bootstrap` execution (`--engine wasmtime-core`).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use wasmparser::{
    DataKind, ElementKind, Encoding, ExternalKind, FuncType, Parser, Payload, RefType, TypeRef,
    ValType, Validator, WasmFeatures,
};
use wasmtime::{
    Caller, Config, Engine as WasmtimeEngine, Global, GlobalType, Instance, Linker, Memory,
    MemoryType, Module, Mutability, OptLevel, Ref, Store, StoreLimits, StoreLimitsBuilder, Table,
    TableType, Trap, Val, ValType as WasmValType,
};

use crate::workflow::encoding::hex;

const SELECTION_JSON: &str = include_str!("runtime/config.json");
#[cfg(test)]
const ABI_JSON: &str = include_str!("runtime/abi.json");

const MAX_FRAME: usize = 4 * 1024 * 1024;
const MAX_SOURCE: usize = 65_536;
const MAX_STACK: i32 = 128;
const MAX_TRACE: usize = 4096;
const MAX_CELLS: i32 = 4096;
const MAX_OBSERVATION_WORK: usize = 100_000;
const MAX_OBSERVATION_DEPTH: usize = 512;
const MAX_SIGNATURES: i32 = 16_384;
const MAX_FUEL: u64 = 1_000_000_000;
pub(crate) const MAX_STORE_INSTANCES: u64 = 1000;

const LIMITS_MAXIMUM: [i32; 6] = [196_608, 256, 256, 2048, 4096, 100_000];

const NAMES: [&str; 26] = [
    "dup",
    "drop",
    "swap",
    "dip",
    "i64.add",
    "i64.sub",
    "i64.mul",
    "i64.eq",
    "quote",
    "compose",
    "run",
    "reflect",
    "unit",
    "pair",
    "unpair",
    "inl",
    "inr",
    "case",
    "if",
    "nil",
    "cons",
    "list.case",
    "test.emit",
    "test.abort",
    "self.propose",
    "self.generation",
];

const EFFECTS: [&str; 5] = [
    "test.emit",
    "test.abort",
    "test.clock",
    "live.propose",
    "live.observe-generation",
];

const METRICS: [(i32, &str); 14] = [
    (0, "status"),
    (1, "quota_reason"),
    (2, "allocated_total_bytes"),
    (3, "released_total_bytes"),
    (4, "allocated_since_begin_bytes"),
    (5, "allocation_peak_bytes"),
    (6, "operand_peak_bytes"),
    (7, "continuation_peak_bytes"),
    (8, "execution_steps"),
    (9, "quote_invocations"),
    (10, "memory_bytes"),
    (11, "retained_before_begin_bytes"),
    (12, "stack_length"),
    (13, "reflection_steps"),
];

const CORE_EXPORTS: [(&str, &[ValType], &[ValType]); 31] = [
    (
        "begin",
        &[
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
        ],
        &[ValType::I32],
    ),
    ("submit", &[], &[ValType::I32]),
    ("push_i64", &[ValType::I64], &[ValType::I32]),
    ("push_bool", &[ValType::I32], &[ValType::I32]),
    ("push_unit", &[], &[ValType::I32]),
    ("push_program", &[ValType::I32], &[ValType::I32]),
    (
        "push_contract",
        &[ValType::I32, ValType::I64, ValType::I32, ValType::I32],
        &[ValType::I32],
    ),
    (
        "push_evidence",
        &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        &[ValType::I32],
    ),
    (
        "push_certified",
        &[ValType::I32, ValType::I32, ValType::I32, ValType::I64],
        &[ValType::I32],
    ),
    ("certified_subject", &[ValType::I32], &[ValType::I32]),
    ("stack_length", &[], &[ValType::I32]),
    ("stack_kind", &[ValType::I32], &[ValType::I32]),
    ("stack_value", &[ValType::I32], &[ValType::I64]),
    ("cell_kind", &[ValType::I32], &[ValType::I32]),
    ("cell_payload", &[ValType::I32], &[ValType::I64]),
    ("cell_a", &[ValType::I32], &[ValType::I32]),
    ("cell_b", &[ValType::I32], &[ValType::I32]),
    ("cell_c", &[ValType::I32], &[ValType::I32]),
    ("cell_x", &[ValType::I32], &[ValType::I32]),
    ("cell_y", &[ValType::I32], &[ValType::I32]),
    ("cell_z", &[ValType::I32], &[ValType::I32]),
    ("cell_w", &[ValType::I32], &[ValType::I32]),
    ("cell_n", &[ValType::I32], &[ValType::I32]),
    ("observe", &[ValType::I32], &[ValType::I32]),
    ("reflect", &[], &[ValType::I32]),
    ("recipe_length", &[], &[ValType::I32]),
    ("recipe_kind", &[ValType::I32], &[ValType::I32]),
    ("recipe_value", &[ValType::I32], &[ValType::I64]),
    ("metric", &[ValType::I32], &[ValType::I64]),
    ("signature_address", &[ValType::I32], &[ValType::I32]),
    ("signature_length", &[ValType::I32], &[ValType::I32]),
];

const ABI_GLOBALS_I32: [(&str, i32); 23] = [
    ("failure", 0),
    ("quota_reason", 0),
    ("phase", 0),
    ("heap_cursor", 0),
    ("heap_baseline", 0),
    ("allocation_limit", 196_608),
    ("recipe_limit", 256),
    ("depth_limit", 256),
    ("operand_limit", 2048),
    ("continuation_limit", 4096),
    ("step_limit", 100_000),
    ("allocation_peak", 0),
    ("operand_peak", 0),
    ("continuation_peak", 0),
    ("steps", 0),
    ("quote_invocations", 0),
    ("sp", 0),
    ("cp", 0),
    ("observed_program", 0),
    ("recipe_count", 0),
    ("reflection_steps", 0),
    ("rp", 0),
    ("generation", 0),
];

const ABI_GLOBALS_I64: [(&str, i64); 2] = [("allocated_total", 0), ("released_total", 0)];

struct PinnedTools {
    wasm_tools: PathBuf,
    wasm_opt: PathBuf,
    optimizer_flags: Vec<String>,
    record_json: Value,
}

struct HostState {
    limits: StoreLimits,
    memory: Option<Memory>,
    trace: Vec<String>,
    in_host_call: bool,
}

struct PendingModule {
    instance: Instance,
    record: Value,
    trace_start: usize,
}

pub(crate) struct Engine {
    engine: WasmtimeEngine,
    store: Store<HostState>,
    linker: Linker<HostState>,
    memory: Memory,
    sp_global: Global,
    heap_cursor_global: Global,
    tools: PinnedTools,
    emit_dir: Option<PathBuf>,
    optimized: bool,
    pending: Option<PendingModule>,
    poisoned: bool,
    compilations: u64,
    executions: u64,
    instances_retained: u64,
    saved_emit_files: HashSet<String>,
}

impl Engine {
    pub(super) fn start(
        options: &super::arguments::Options,
    ) -> Result<Self, super::output::Failure> {
        if options.declared_modules
            || options.signed_self_recursion
            || options.size_change_evidence
            || options.text_byte_cursor
            || options.checked_i64
        {
            return Err(super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "unsupported",
                },
                "wasmtime-core supports only the Core-Bootstrap profile",
            ));
        }
        let tools = attempt!(load_and_verify_tools());
        let emit_dir = match &options.emit {
            Some(directory) => {
                let engine_dir = directory.join("engine");
                attempt!(crate::workflow::artifacts::create_directory(&engine_dir)
                    .map_err(internal_from_failure));
                let mut tools_bytes =
                    attempt!(serde_json::to_vec_pretty(&tools.record_json).map_err(internal_error));
                tools_bytes.push(b'\n');
                attempt!(super::framing::write_new(
                    &engine_dir.join("tools.json"),
                    &tools_bytes
                ));
                Some(engine_dir)
            }
            None => None,
        };
        let engine = attempt!(build_wasmtime_engine());
        let limits = StoreLimitsBuilder::new()
            .memory_size(1_048_576)
            .table_elements(16_384)
            .instances(usize::try_from(MAX_STORE_INSTANCES).unwrap_or(1000))
            .tables(1)
            .memories(1)
            .build();
        let mut store = Store::new(
            &engine,
            HostState {
                limits,
                memory: None,
                trace: Vec::new(),
                in_host_call: false,
            },
        );
        store.limiter(|host| &mut host.limits);
        store.set_epoch_deadline(u64::MAX);
        attempt!(store.set_fuel(MAX_FUEL).map_err(internal_error));

        let memory = attempt!(
            Memory::new(&mut store, MemoryType::new(16, Some(16))).map_err(internal_error)
        );
        store.data_mut().memory = Some(memory);
        let table = attempt!(Table::new(
            &mut store,
            TableType::new(wasmtime::RefType::FUNCREF, 16_384, Some(16_384)),
            Ref::Func(None),
        )
        .map_err(internal_error));

        let mut linker = Linker::new(&engine);
        attempt!(linker
            .define(&mut store, "noble", "memory", memory)
            .map_err(internal_error));
        attempt!(linker
            .define(&mut store, "noble", "table", table)
            .map_err(internal_error));

        let mut sp_global = None;
        let mut heap_cursor_global = None;
        for (name, initial) in ABI_GLOBALS_I32 {
            let global = attempt!(Global::new(
                &mut store,
                GlobalType::new(WasmValType::I32, Mutability::Var),
                Val::I32(initial),
            )
            .map_err(internal_error));
            if name == "sp" {
                sp_global = Some(global);
            } else if name == "heap_cursor" {
                heap_cursor_global = Some(global);
            }
            attempt!(linker
                .define(&mut store, "noble", name, global)
                .map_err(internal_error));
        }
        for (name, initial) in ABI_GLOBALS_I64 {
            let global = attempt!(Global::new(
                &mut store,
                GlobalType::new(WasmValType::I64, Mutability::Var),
                Val::I64(initial),
            )
            .map_err(internal_error));
            attempt!(linker
                .define(&mut store, "noble", name, global)
                .map_err(internal_error));
        }

        attempt!(linker
            .func_wrap(
                "noble",
                "test_emit",
                |mut caller: Caller<'_, HostState>,
                 offset: i32,
                 count: i32|
                 -> wasmtime::Result<i32> {
                    if caller.data().in_host_call {
                        return Err(wasmtime::Error::msg("reentrant test_emit call"));
                    }
                    caller.data_mut().in_host_call = true;
                    let result = handle_test_emit(&mut caller, offset, count);
                    caller.data_mut().in_host_call = false;
                    result
                },
            )
            .map_err(internal_error));
        attempt!(linker
            .func_wrap(
                "noble",
                "test_abort",
                |mut caller: Caller<'_, HostState>| -> wasmtime::Result<i32> {
                    if caller.data().in_host_call {
                        return Err(wasmtime::Error::msg("reentrant test_abort call"));
                    }
                    caller.data_mut().in_host_call = true;
                    if caller.data().trace.len() < MAX_TRACE {
                        caller.data_mut().trace.push(String::from("test.abort"));
                    }
                    caller.data_mut().in_host_call = false;
                    Ok(0)
                },
            )
            .map_err(internal_error));

        Ok(Self {
            engine,
            store,
            linker,
            memory,
            sp_global: attempt!(require_internal(sp_global, "missing sp global")),
            heap_cursor_global: attempt!(require_internal(
                heap_cursor_global,
                "missing heap_cursor global"
            )),
            tools,
            emit_dir,
            optimized: options.optimized,
            pending: None,
            poisoned: false,
            compilations: 0,
            executions: 0,
            instances_retained: 0,
            saved_emit_files: HashSet::new(),
        })
    }

    pub(super) fn prepare(
        &mut self,
        wat: &[u8],
        source: &[u8],
        submission: u64,
    ) -> Result<super::output::Report, super::output::Failure> {
        if self.poisoned || self.pending.is_some() {
            return Err(internal_failure("session is pending or poisoned"));
        }
        if self.instances_retained >= MAX_STORE_INSTANCES {
            self.poisoned = true;
            return Ok(store_limit_refusal(submission));
        }
        if wat.len() > MAX_FRAME || source.len() > MAX_SOURCE {
            return Err(internal_failure("preparation byte limit exceeded"));
        }

        let stem = format!("module-{submission}");
        attempt!(self.emit_file(&format!("{stem}.wat"), wat));
        attempt!(self.emit_file(&format!("{stem}.noble"), source));

        let (wasm_bytes, optimized_bytes) =
            attempt!(assemble_wat(wat, self.optimized, &self.tools));
        if wasm_bytes.len() > MAX_FRAME {
            return Err(internal_failure("module byte limit exceeded"));
        }
        if let Some(raw) = &optimized_bytes {
            attempt!(self.emit_file(&format!("{stem}.wasm"), raw));
            attempt!(self.emit_file(&format!("{stem}-optimized.wasm"), &wasm_bytes));
        } else {
            attempt!(self.emit_file(&format!("{stem}.wasm"), &wasm_bytes));
        }

        let imports = attempt!(validate_core_wasm(&wasm_bytes).map_err(unsupported_wasm));
        let module =
            attempt!(
                Module::new(&self.engine, &wasm_bytes).map_err(|error| unsupported_wasm(format!(
                    "wasmtime module compilation failed: {error}"
                )))
            );

        let trace_before = self.store.data().trace.len();
        let sp_before = attempt!(require_internal(
            self.sp_global.get(&mut self.store).i32(),
            "invalid sp global"
        ));

        attempt!(self.store.set_fuel(MAX_FUEL).map_err(internal_error));
        let instance = attempt!(self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|error| internal_failure(format!("wasmtime instantiation failed: {error}"))));
        self.instances_retained = self.instances_retained.saturating_add(1);

        let sp_after = attempt!(require_internal(
            self.sp_global.get(&mut self.store).i32(),
            "invalid sp global"
        ));
        if self.store.data().trace.len() != trace_before || sp_after != sp_before {
            self.poisoned = true;
            return Err(internal_failure("instantiation executed candidate body"));
        }

        self.compilations = self.compilations.saturating_add(1);
        let record = json!({
            "submission": submission,
            "wasm_sha256": crate::workflow::intrinsic::sha256(&wasm_bytes),
            "source_sha256": crate::workflow::intrinsic::sha256(source),
            "wat_sha256": crate::workflow::intrinsic::sha256(wat),
            "optimized": self.optimized,
            "compilations": self.compilations,
            "stem": stem,
            "imports": imports,
        });
        let ready = json!({
            "schema": "noble-core-report/v1",
            "profile": "Core-Bootstrap",
            "backend": "managed-linear-memory",
            "stage": "wasm",
            "outcome": "ready",
            "module": record,
            "guest_requests": 0,
            "protected_operations": 0,
            "candidate_prepare_requests": 0,
        });
        self.pending = Some(PendingModule {
            instance,
            record,
            trace_start: trace_before,
        });
        Ok(super::output::Report {
            outcome: String::from("ready"),
            json: ready.to_string(),
        })
    }

    pub(super) fn execute(&mut self) -> Result<super::output::Report, super::output::Failure> {
        let PendingModule {
            instance,
            record,
            trace_start,
        } = attempt!(require_internal(self.pending.take(), "no prepared module"));
        if self.poisoned {
            return Err(internal_failure("cannot execute a poisoned session"));
        }

        attempt!(self.store.set_fuel(MAX_FUEL).map_err(internal_error));

        let begin = attempt!(instance
            .get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(&mut self.store, "begin")
            .map_err(internal_error));
        let submit = attempt!(instance
            .get_typed_func::<(), i32>(&mut self.store, "submit")
            .map_err(internal_error));

        let mut native_trap: Option<String> = None;
        let mut fuel_exhausted = false;
        let mut status = match begin.call(
            &mut self.store,
            (
                LIMITS_MAXIMUM[0],
                LIMITS_MAXIMUM[1],
                LIMITS_MAXIMUM[2],
                LIMITS_MAXIMUM[3],
                LIMITS_MAXIMUM[4],
                LIMITS_MAXIMUM[5],
            ),
        ) {
            Ok(status) => status,
            Err(error) => {
                if error.downcast_ref::<Trap>().copied() == Some(Trap::OutOfFuel) {
                    fuel_exhausted = true;
                    native_trap = Some(String::from("wasmtime fuel exhausted"));
                    2
                } else {
                    native_trap = Some(error.to_string());
                    6
                }
            }
        };

        if status == 0 {
            self.executions = self.executions.saturating_add(1);
            match submit.call(&mut self.store, ()) {
                Ok(submit_status) => status = submit_status,
                Err(error) => {
                    if error.downcast_ref::<Trap>().copied() == Some(Trap::OutOfFuel) {
                        fuel_exhausted = true;
                        native_trap = Some(String::from("wasmtime fuel exhausted"));
                        status = 2;
                    } else {
                        native_trap = Some(error.to_string());
                        status = 6;
                    }
                }
            }
        }

        self.poisoned = status != 0;
        attempt!(self.store.set_fuel(MAX_FUEL).map_err(internal_error));

        let metrics = attempt!(read_metrics(&mut self.store, &instance));
        let stack = if status == 0 {
            attempt!(read_stack(
                &mut self.store,
                &instance,
                self.memory,
                self.heap_cursor_global
            ))
        } else {
            Vec::new()
        };

        let outcome = match status {
            0 => "normal",
            1 | 2 => "runtime-exhausted",
            _ => "trap",
        };
        let quota_reason = if fuel_exhausted {
            Some("wasmtime-fuel")
        } else {
            quota_reason_name(
                metrics
                    .get("quota_reason")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            )
        };

        let trace_slice: Vec<String> = self
            .store
            .data()
            .trace
            .get(trace_start..)
            .unwrap_or(&[])
            .to_vec();
        let guest_requests = trace_slice.len();
        let submission = record.get("submission").cloned().unwrap_or(Value::Null);
        let stem = record
            .get("stem")
            .and_then(Value::as_str)
            .map(str::to_owned);

        let report_value = json!({
            "schema": "noble-core-report/v1",
            "profile": "Core-Bootstrap",
            "backend": "managed-linear-memory",
            "submission": submission,
            "stage": "wasm",
            "outcome": outcome,
            "status": status,
            "quota_reason": quota_reason,
            "stack": stack,
            "request_trace": trace_slice,
            "guest_requests": guest_requests,
            "protected_operations": 0,
            "guest_executions": self.executions,
            "candidate_prepare_requests": 0,
            "runtime_prover_calls": 0,
            "native_trap": native_trap,
            "session_state": if self.poisoned {
                "terminated-after-runtime-failure"
            } else {
                "retained"
            },
            "metrics": metrics,
            "module": record,
        });
        if let Some(stem) = stem {
            let mut pretty =
                attempt!(serde_json::to_vec_pretty(&report_value).map_err(internal_error));
            pretty.push(b'\n');
            attempt!(self.emit_file(&format!("{stem}-execution.json"), &pretty));
        }

        Ok(super::output::Report {
            outcome: String::from(outcome),
            json: report_value.to_string(),
        })
    }

    fn emit_file(&mut self, name: &str, bytes: &[u8]) -> Result<(), super::output::Failure> {
        let Some(directory) = &self.emit_dir else {
            return Ok(());
        };
        if !self.saved_emit_files.insert(String::from(name)) {
            return Err(internal_failure(format!(
                "duplicate emitted artifact {name}"
            )));
        }
        super::framing::write_new(&directory.join(name), bytes)
    }
}

fn build_wasmtime_engine() -> Result<WasmtimeEngine, super::output::Failure> {
    let mut config = Config::new();
    config.cranelift_opt_level(OptLevel::Speed);
    config.consume_fuel(true);
    config.epoch_interruption(true);
    config.max_wasm_stack(1024 * 1024);
    config.wasm_gc(false);
    config.wasm_function_references(false);
    config.wasm_reference_types(true);
    config.wasm_bulk_memory(true);
    config.wasm_multi_value(true);
    config.wasm_simd(false);
    config.wasm_relaxed_simd(false);
    config.wasm_memory64(false);
    config.wasm_tail_call(false);
    config.wasm_exceptions(false);
    config.wasm_component_model(false);
    config.signals_based_traps(true);
    WasmtimeEngine::new(&config).map_err(internal_error)
}

fn store_limit_refusal(submission: u64) -> super::output::Report {
    let report = json!({
        "schema": "noble-core-report/v1",
        "profile": "Core-Bootstrap",
        "backend": "managed-linear-memory",
        "submission": submission,
        "stage": "wasm",
        "outcome": "runtime-exhausted",
        "status": 2,
        "quota_reason": "store-instance-limit",
        "diagnostic": format!("wasmtime-core Store instance retention limit ({MAX_STORE_INSTANCES}) reached"),
        "stack": [],
        "request_trace": [],
        "guest_requests": 0,
        "protected_operations": 0,
        "candidate_prepare_requests": 0,
        "prior_stack": "unchanged",
        "prior_namespace": "unchanged",
        "prior_session": "unchanged",
        "session_state": "terminated-after-store-limit",
    });
    super::output::Report {
        outcome: String::from("runtime-exhausted"),
        json: report.to_string(),
    }
}

fn handle_test_emit(
    caller: &mut Caller<'_, HostState>,
    offset: i32,
    count: i32,
) -> wasmtime::Result<i32> {
    let memory = caller
        .data()
        .memory
        .ok_or_else(|| wasmtime::Error::msg("missing core memory"))?;
    let start = usize::try_from(offset).map_err(|_| wasmtime::Error::msg("invalid text offset"))?;
    let length = usize::try_from(count).map_err(|_| wasmtime::Error::msg("invalid text length"))?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| wasmtime::Error::msg("text range overflow"))?;
    let data = memory.data(&*caller);
    let slice = data
        .get(start..end)
        .ok_or_else(|| wasmtime::Error::msg("text range out of bounds"))?;
    let text = std::str::from_utf8(slice)
        .map_err(|_| wasmtime::Error::msg("invalid UTF-8 text in test_emit"))?;
    let text = String::from(text);
    if caller.data().trace.len() >= MAX_TRACE {
        return Ok(1);
    }
    caller.data_mut().trace.push(format!("test.emit:{text}"));
    Ok(0)
}

fn load_and_verify_tools() -> Result<PinnedTools, super::output::Failure> {
    let selection: Value = serde_json::from_str(SELECTION_JSON)
        .map_err(|error| unsupported_wasm(format!("invalid runtime config.json: {error}")))?;
    let tools = &selection["tools"];
    let wasm_tools_path = tool_str(&tools["wasm_tools"]["path"], "wasm_tools.path")?;
    let wasm_tools_version = tool_str(&tools["wasm_tools"]["version"], "wasm_tools.version")?;
    let wasm_opt_path = tool_str(&tools["wasm_opt"]["path"], "wasm_opt.path")?;
    let wasm_opt_version = tool_str(&tools["wasm_opt"]["version"], "wasm_opt.version")?;
    let flags_array = selection["optimizer_flags"]
        .as_array()
        .ok_or_else(|| unsupported_wasm("missing optimizer_flags in config.json"))?;
    let mut optimizer_flags = Vec::with_capacity(flags_array.len());
    for flag in flags_array {
        optimizer_flags.push(String::from(tool_str(flag, "optimizer_flags entry")?));
    }

    let wasm_tools = PathBuf::from(wasm_tools_path);
    let wasm_opt = PathBuf::from(wasm_opt_path);
    let deadline = attempt!(require_internal(
        crate::sandbox::now().checked_add(std::time::Duration::from_secs(30)),
        "tool check deadline overflow"
    ));
    let wt_ver = attempt!(run_pinned_tool(&wasm_tools, &["--version"], None, deadline));
    let expected_wt = format!("wasm-tools {wasm_tools_version}");
    if !wt_ver.trim().starts_with(&expected_wt) {
        return Err(unsupported_wasm(format!(
            "incompatible wasm-tools version: {}",
            wt_ver.trim()
        )));
    }
    let wo_ver = attempt!(run_pinned_tool(&wasm_opt, &["--version"], None, deadline));
    let expected_wo = format!("wasm-opt version {wasm_opt_version}");
    if !wo_ver.trim().starts_with(&expected_wo) {
        return Err(unsupported_wasm(format!(
            "incompatible wasm-opt version: {}",
            wo_ver.trim()
        )));
    }
    let wt_bytes = attempt!(crate::workflow::artifacts::read_bounded(
        &wasm_tools,
        64 * 1024 * 1024,
        "wasm-tools"
    )
    .map_err(unsupported_from_failure));
    let wo_bytes =
        attempt!(
            crate::workflow::artifacts::read_bounded(&wasm_opt, 64 * 1024 * 1024, "wasm-opt")
                .map_err(unsupported_from_failure)
        );

    let record_json = json!({
        "engine": "wasmtime-core",
        "wasmtime": "40.0.2",
        "cranelift": "0.127.2",
        "wasm_tools": {
            "path": wasm_tools_path,
            "version": wt_ver.trim(),
            "sha256": hex(blake3::hash(&wt_bytes).as_bytes()),
            "file_sha256": crate::workflow::intrinsic::sha256(&wt_bytes),
        },
        "wasm_opt": {
            "path": wasm_opt_path,
            "version": wo_ver.trim(),
            "sha256": hex(blake3::hash(&wo_bytes).as_bytes()),
            "file_sha256": crate::workflow::intrinsic::sha256(&wo_bytes),
        },
    });
    Ok(PinnedTools {
        wasm_tools,
        wasm_opt,
        optimizer_flags,
        record_json,
    })
}

fn tool_str<'a>(value: &'a Value, label: &str) -> Result<&'a str, super::output::Failure> {
    value
        .as_str()
        .ok_or_else(|| unsupported_wasm(format!("missing {label} in config.json")))
}

fn assemble_wat(
    wat: &[u8],
    optimized: bool,
    tools: &PinnedTools,
) -> Result<(Vec<u8>, Option<Vec<u8>>), super::output::Failure> {
    let raw_bytes = wat::parse_bytes(wat)
        .map(|cow| cow.into_owned())
        .map_err(|error| unsupported_wasm(format!("WAT assembly failed: {error}")))?;
    if raw_bytes.len() > MAX_FRAME {
        return Err(internal_failure("module byte limit exceeded"));
    }
    if !optimized {
        return Ok((raw_bytes, None));
    }

    let temporary =
        attempt!(crate::workflow::artifacts::Temporary::create().map_err(internal_from_failure));
    let deadline = attempt!(require_internal(
        crate::sandbox::now().checked_add(std::time::Duration::from_secs(60)),
        "assembly deadline overflow"
    ));
    let wasm_path = temporary.path.join("module.wasm");
    attempt!(
        crate::workflow::artifacts::write_source(&wasm_path, &raw_bytes)
            .map_err(internal_from_failure)
    );

    let opt_path = temporary.path.join("module-optimized.wasm");
    let mut opt_args: Vec<&str> = Vec::with_capacity(tools.optimizer_flags.len() + 3);
    opt_args.push("module.wasm");
    for flag in &tools.optimizer_flags {
        opt_args.push(flag.as_str());
    }
    opt_args.push("-o");
    opt_args.push("module-optimized.wasm");
    attempt!(run_pinned_tool(
        &tools.wasm_opt,
        &opt_args,
        Some(&temporary.path),
        deadline,
    ));
    attempt!(run_pinned_tool(
        &tools.wasm_tools,
        &["validate", "module-optimized.wasm"],
        Some(&temporary.path),
        deadline,
    ));
    let opt_bytes =
        attempt!(
            crate::workflow::artifacts::read_bounded(&opt_path, MAX_FRAME, "optimized-wasm",)
                .map_err(internal_from_failure)
        );
    Ok((opt_bytes, Some(raw_bytes)))
}

fn run_pinned_tool(
    executable: &Path,
    arguments: &[&str],
    directory: Option<&Path>,
    deadline: std::time::Instant,
) -> Result<String, super::output::Failure> {
    if !executable.is_absolute() {
        return Err(unsupported_wasm("pinned tool path must be absolute"));
    }
    let mut command = std::process::Command::new(executable);
    command.env_clear().args(arguments);
    if let Some(dir) = directory {
        command.current_dir(dir);
    }
    let transcript = attempt!(
        crate::sandbox::process::capture(command, deadline).map_err(unsupported_from_failure)
    );
    if !transcript.status.success() {
        return Err(unsupported_wasm(format!(
            "pinned tool {} failed: {}",
            executable.display(),
            transcript.stderr.trim()
        )));
    }
    Ok(transcript.stdout)
}

pub(crate) fn validate_core_wasm(bytes: &[u8]) -> Result<Vec<Value>, String> {
    if bytes.len() < 8 || bytes.len() > MAX_FRAME || !bytes.starts_with(b"\0asm\x01\0\0\0") {
        return Err(String::from("invalid bounded Core Wasm module"));
    }
    let features = WasmFeatures::WASM1
        .difference(WasmFeatures::FLOATS)
        .difference(WasmFeatures::GC_TYPES)
        .union(WasmFeatures::BULK_MEMORY)
        .union(WasmFeatures::REFERENCE_TYPES)
        .union(WasmFeatures::MULTI_VALUE)
        .union(WasmFeatures::SIGN_EXTENSION);
    Validator::new_with_features(features)
        .validate_all(bytes)
        .map_err(|error| format!("unsupported or invalid core Wasm: {error}"))?;

    let mut types: Vec<FuncType> = Vec::new();
    let mut functions: Vec<u32> = Vec::new();
    let mut seen_imports: HashSet<&str> = HashSet::new();
    let mut seen_exports: HashSet<&str> = HashSet::new();
    let mut imports_report: Vec<Value> = Vec::new();
    let mut memory_imports = 0_usize;
    let mut table_imports = 0_usize;
    let mut global_imports = 0_usize;

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|error| format!("invalid core Wasm section: {error}"))? {
            Payload::Version {
                num: 1,
                encoding: Encoding::Module,
                ..
            } => {}
            Payload::Version { .. } => {
                return Err(String::from("expected a standard core Wasm v1 module"));
            }
            Payload::StartSection { .. } => {
                return Err(String::from("source module must not have a start function"));
            }
            Payload::TypeSection(section) => {
                for ty in section.into_iter_err_on_gc_types() {
                    types.push(ty.map_err(|error| format!("invalid core function type: {error}"))?);
                }
            }
            Payload::ImportSection(section) => {
                for import in section {
                    let import = import.map_err(|error| format!("invalid Wasm import: {error}"))?;
                    if import.module != "noble" || !seen_imports.insert(import.name) {
                        return Err(format!(
                            "unexpected or duplicate Wasm import: {}.{}",
                            import.module, import.name
                        ));
                    }
                    let kind = match import.ty {
                        TypeRef::Func(index) | TypeRef::FuncExact(index) => {
                            let expected: (&[ValType], &[ValType]) = match import.name {
                                "test_emit" => (&[ValType::I32, ValType::I32], &[ValType::I32]),
                                "test_abort" => (&[], &[ValType::I32]),
                                _ => {
                                    return Err(format!(
                                        "undeclared Core-Bootstrap host import: {}",
                                        import.name
                                    ));
                                }
                            };
                            let ty = types
                                .get(usize::try_from(index).unwrap_or(usize::MAX))
                                .ok_or_else(|| String::from("invalid import function type"))?;
                            if ty.params() != expected.0 || ty.results() != expected.1 {
                                return Err(format!("mistyped host import: {}", import.name));
                            }
                            functions.push(index);
                            "function"
                        }
                        TypeRef::Memory(mem) => {
                            if import.name != "memory"
                                || mem.memory64
                                || mem.shared
                                || mem.page_size_log2.is_some()
                                || mem.initial != 16
                                || mem.maximum != Some(16)
                            {
                                return Err(String::from("incompatible Wasm memory import"));
                            }
                            memory_imports += 1;
                            "memory"
                        }
                        TypeRef::Table(table) => {
                            if import.name != "table"
                                || table.table64
                                || table.shared
                                || table.element_type != RefType::FUNCREF
                                || table.initial != 16_384
                                || table.maximum != Some(16_384)
                            {
                                return Err(String::from("incompatible Wasm table import"));
                            }
                            table_imports += 1;
                            "table"
                        }
                        TypeRef::Global(global) => {
                            let expected = if ABI_GLOBALS_I32
                                .iter()
                                .any(|(name, _)| *name == import.name)
                            {
                                ValType::I32
                            } else if ABI_GLOBALS_I64.iter().any(|(name, _)| *name == import.name) {
                                ValType::I64
                            } else {
                                return Err(format!("undeclared global import: {}", import.name));
                            };
                            if !global.mutable || global.shared || global.content_type != expected {
                                return Err(format!("incompatible global import: {}", import.name));
                            }
                            global_imports += 1;
                            "global"
                        }
                        TypeRef::Tag(_) => {
                            return Err(String::from("unsupported Wasm tag import"));
                        }
                    };
                    imports_report.push(json!({
                        "module": import.module,
                        "name": import.name,
                        "kind": kind,
                    }));
                }
            }
            Payload::FunctionSection(section) => {
                for index in section {
                    functions
                        .push(index.map_err(|error| format!("invalid Wasm function: {error}"))?);
                }
            }
            Payload::MemorySection(section) if section.count() != 0 => {
                return Err(String::from("module must import its only memory"));
            }
            Payload::TableSection(section) if section.count() != 0 => {
                return Err(String::from("module must import its only table"));
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.map_err(|error| format!("invalid Wasm export: {error}"))?;
                    if !seen_exports.insert(export.name) {
                        return Err(format!("duplicate Wasm export: {}", export.name));
                    }
                    if export.name == "memory" {
                        if export.kind != ExternalKind::Memory || export.index != 0 {
                            return Err(String::from("incompatible Wasm memory export"));
                        }
                        continue;
                    }
                    let expected = CORE_EXPORTS
                        .iter()
                        .find(|(name, _, _)| *name == export.name)
                        .ok_or_else(|| format!("undeclared Wasm export: {}", export.name))?;
                    if export.kind != ExternalKind::Func {
                        return Err(format!("incompatible Wasm export kind: {}", export.name));
                    }
                    let type_index = functions
                        .get(usize::try_from(export.index).unwrap_or(usize::MAX))
                        .copied()
                        .ok_or_else(|| String::from("missing exported function index"))?;
                    let func_ty = types
                        .get(usize::try_from(type_index).unwrap_or(usize::MAX))
                        .ok_or_else(|| String::from("missing exported function type"))?;
                    if func_ty.params() != expected.1 || func_ty.results() != expected.2 {
                        return Err(format!(
                            "incompatible Wasm export signature: {}",
                            export.name
                        ));
                    }
                }
            }
            Payload::DataSection(section) => {
                for data in section {
                    if matches!(
                        data.map_err(|error| format!("invalid Wasm data segment: {error}"))?
                            .kind,
                        DataKind::Active { .. }
                    ) {
                        return Err(String::from("active Wasm data segment is forbidden"));
                    }
                }
            }
            Payload::ElementSection(section) => {
                for element in section {
                    if matches!(
                        element
                            .map_err(|error| format!("invalid Wasm element segment: {error}"))?
                            .kind,
                        ElementKind::Active { .. }
                    ) {
                        return Err(String::from("active Wasm element segment is forbidden"));
                    }
                }
            }
            _ => {}
        }
    }

    if memory_imports != 1
        || table_imports != 1
        || global_imports != ABI_GLOBALS_I32.len() + ABI_GLOBALS_I64.len()
        || seen_exports.len() != CORE_EXPORTS.len() + 1
    {
        return Err(String::from("incomplete Wasm arena ABI"));
    }
    Ok(imports_report)
}

fn read_metrics(
    store: &mut Store<HostState>,
    instance: &Instance,
) -> Result<BTreeMap<String, Value>, super::output::Failure> {
    let metric = attempt!(instance
        .get_typed_func::<i32, i64>(&mut *store, "metric")
        .map_err(internal_error));
    let mut map = BTreeMap::new();
    for (id, name) in METRICS {
        let value = attempt!(metric.call(&mut *store, id).map_err(internal_error));
        map.insert(String::from(name), json!(value));
    }
    Ok(map)
}

struct Reader<'a> {
    store: &'a mut Store<HostState>,
    instance: &'a Instance,
    memory: Memory,
    heap_cursor: i32,
    observation_work: usize,
}

impl Reader<'_> {
    fn charge_observation(&mut self, depth: usize) -> Result<(), super::output::Failure> {
        self.observation_work = self.observation_work.saturating_add(1);
        if depth > MAX_OBSERVATION_DEPTH || self.observation_work > MAX_OBSERVATION_WORK {
            return Err(internal_failure("observation limit exhausted"));
        }
        Ok(())
    }

    fn call_i32_to_i32(&mut self, name: &str, arg: i32) -> Result<i32, super::output::Failure> {
        let func = attempt!(self
            .instance
            .get_typed_func::<i32, i32>(&mut *self.store, name)
            .map_err(internal_error));
        func.call(&mut *self.store, arg).map_err(internal_error)
    }

    fn call_i32_to_i64(&mut self, name: &str, arg: i32) -> Result<i64, super::output::Failure> {
        let func = attempt!(self
            .instance
            .get_typed_func::<i32, i64>(&mut *self.store, name)
            .map_err(internal_error));
        func.call(&mut *self.store, arg).map_err(internal_error)
    }

    fn read_text(&self, offset: i32, count: i32) -> Result<String, super::output::Failure> {
        let start = usize::try_from(offset).map_err(internal_error)?;
        let length = usize::try_from(count).map_err(internal_error)?;
        let end = require_internal(start.checked_add(length), "text length overflow")?;
        let data = self.memory.data(&*self.store);
        let slice = require_internal(data.get(start..end), "text range out of bounds")?;
        let text = std::str::from_utf8(slice).map_err(internal_error)?;
        Ok(String::from(text))
    }

    fn read_signature(&mut self, identity: i32) -> Result<String, super::output::Failure> {
        if !(0..=MAX_SIGNATURES).contains(&identity) {
            return Err(internal_failure("invalid signature identity"));
        }
        let address = attempt!(self.call_i32_to_i32("signature_address", identity));
        let length = attempt!(self.call_i32_to_i32("signature_length", identity));
        self.read_text(address, length)
    }

    fn read_boxed(&mut self, handle: i32, depth: usize) -> Result<Value, super::output::Failure> {
        if handle <= 0 || handle > MAX_CELLS || handle > self.heap_cursor {
            return Err(internal_failure("invalid live cell handle"));
        }
        let kind = attempt!(self.call_i32_to_i32("cell_kind", handle));
        let raw = if kind < 4 {
            attempt!(self.call_i32_to_i64("cell_payload", handle))
        } else {
            i64::from(handle)
        };
        let mut value = attempt!(self.read_value(kind, raw, depth));
        if let Value::Object(map) = &mut value {
            map.insert(String::from("handle"), json!(handle));
        }
        Ok(value)
    }

    fn read_value(
        &mut self,
        kind: i32,
        raw: i64,
        depth: usize,
    ) -> Result<Value, super::output::Failure> {
        attempt!(self.charge_observation(depth));
        if kind == 1 {
            return Ok(json!({ "type": "I64", "value": raw.to_string() }));
        }
        if kind == 2 {
            if raw != 0 && raw != 1 {
                return Err(internal_failure("invalid Bool value"));
            }
            return Ok(json!({ "type": "Bool", "value": raw == 1 }));
        }
        if kind == 3 {
            return Ok(json!({ "type": "Unit", "value": Value::Null }));
        }
        let handle = i32::try_from(raw)
            .ok()
            .filter(|&h| h > 0 && h <= self.heap_cursor)
            .ok_or_else(|| internal_failure("invalid value handle"))?;
        if attempt!(self.call_i32_to_i32("cell_kind", handle)) != kind {
            return Err(internal_failure("value tag disagrees with cell"));
        }
        match kind {
            4 => {
                let sig_in = attempt!(self.call_i32_to_i32("cell_y", handle));
                let sig_out = attempt!(self.call_i32_to_i32("cell_z", handle));
                let stack_in = attempt!(self.read_signature(sig_in));
                let stack_out = attempt!(self.read_signature(sig_out));
                let mask = attempt!(self.call_i32_to_i64("cell_payload", handle));
                let effects = decode_effects(mask);
                let recipe_root = attempt!(self.call_i32_to_i32("cell_c", handle));
                let (recipe, witnesses) = attempt!(self.read_recipe(recipe_root, depth + 1));
                Ok(json!({
                    "type": "Program",
                    "interface": {
                        "stack_in": stack_in,
                        "stack_out": stack_out,
                        "effects": effects,
                    },
                    "recipe": recipe,
                    "witnesses": witnesses,
                }))
            }
            5 => {
                let left = attempt!(self.call_i32_to_i32("cell_a", handle));
                let right = attempt!(self.call_i32_to_i32("cell_b", handle));
                let left_val = attempt!(self.read_boxed(left, depth + 1));
                let right_val = attempt!(self.read_boxed(right, depth + 1));
                Ok(json!({ "type": "Pair", "value": [left_val, right_val] }))
            }
            6 | 7 => {
                let mut items = Vec::new();
                let mut tail = handle;
                while attempt!(self.call_i32_to_i32("cell_kind", tail)) == 6 {
                    attempt!(self.charge_observation(depth));
                    let head = attempt!(self.call_i32_to_i32("cell_a", tail));
                    items.push(attempt!(self.read_boxed(head, depth + 1)));
                    let next = attempt!(self.call_i32_to_i32("cell_b", tail));
                    if next <= 0 || next >= tail {
                        return Err(internal_failure("cyclic/non-backward list"));
                    }
                    tail = next;
                }
                if attempt!(self.call_i32_to_i32("cell_kind", tail)) != 7 {
                    return Err(internal_failure("invalid list tail"));
                }
                Ok(json!({ "type": "List", "value": items }))
            }
            10 => {
                let root = attempt!(self.call_i32_to_i32("cell_a", handle));
                let (recipe, witnesses) = attempt!(self.read_recipe(root, depth + 1));
                Ok(json!({
                    "type": "Syntax",
                    "recipe": recipe,
                    "witnesses": witnesses,
                }))
            }
            11 => {
                let offset = attempt!(self.call_i32_to_i32("cell_x", handle));
                let length = attempt!(self.call_i32_to_i32("cell_y", handle));
                let text = attempt!(self.read_text(offset, length));
                Ok(json!({ "type": "Text", "value": text }))
            }
            12 | 13 => {
                let marker = attempt!(self.call_i32_to_i32("cell_w", handle));
                if marker != 0 {
                    return Err(internal_failure(
                        "nominal value is outside a declared module profile",
                    ));
                }
                let inner = attempt!(self.call_i32_to_i32("cell_a", handle));
                let value = attempt!(self.read_boxed(inner, depth + 1));
                Ok(json!({
                    "type": "Sum",
                    "variant": if kind == 12 { "left" } else { "right" },
                    "value": value,
                }))
            }
            14 => {
                let statement = attempt!(self.call_i32_to_i64("cell_payload", handle));
                let index = attempt!(self.call_i32_to_i32("cell_x", handle));
                let claim_kind = attempt!(self.call_i32_to_i32("cell_y", handle));
                let revision = attempt!(self.call_i32_to_i32("cell_z", handle));
                Ok(json!({
                    "type": "Contract",
                    "statement": u64::from_le_bytes(statement.to_le_bytes()).to_string(),
                    "index": index,
                    "claim_kind": claim_kind,
                    "revision": revision,
                }))
            }
            15 => {
                let index = attempt!(self.call_i32_to_i64("cell_payload", handle));
                let class = attempt!(self.call_i32_to_i32("cell_x", handle));
                let ruleset = attempt!(self.call_i32_to_i32("cell_y", handle));
                let contract_handle = attempt!(self.call_i32_to_i32("cell_a", handle));
                let contract = attempt!(self.read_boxed(contract_handle, depth + 1));
                Ok(json!({
                    "type": "Evidence",
                    "index": index,
                    "class": class,
                    "ruleset": ruleset,
                    "contract": contract,
                }))
            }
            16 => {
                let identity = attempt!(self.call_i32_to_i64("cell_payload", handle));
                let subject_h = attempt!(self.call_i32_to_i32("cell_a", handle));
                let contract_h = attempt!(self.call_i32_to_i32("cell_b", handle));
                let evidence_h = attempt!(self.call_i32_to_i32("cell_c", handle));
                let subject = attempt!(self.read_boxed(subject_h, depth + 1));
                let contract = attempt!(self.read_boxed(contract_h, depth + 1));
                let evidence = attempt!(self.read_boxed(evidence_h, depth + 1));
                Ok(json!({
                    "type": "Certified",
                    "identity": u64::from_le_bytes(identity.to_le_bytes()).to_string(),
                    "subject": subject,
                    "contract": contract,
                    "evidence": evidence,
                }))
            }
            _ => Err(internal_failure(format!(
                "unsupported observable value kind: {kind}"
            ))),
        }
    }

    fn read_recipe(
        &mut self,
        root: i32,
        depth: usize,
    ) -> Result<(Vec<Value>, Vec<Value>), super::output::Failure> {
        attempt!(self.charge_observation(depth));
        let mut result = Vec::new();
        let mut witnesses = Vec::new();
        let mut pending = if root == 0 { Vec::new() } else { vec![root] };
        while let Some(handle) = pending.pop() {
            attempt!(self.charge_observation(depth));
            if handle <= 0 || handle > MAX_CELLS || handle > self.heap_cursor {
                return Err(internal_failure("invalid recipe handle"));
            }
            let kind = attempt!(self.call_i32_to_i32("cell_kind", handle));
            if kind == 9 {
                let left = attempt!(self.call_i32_to_i32("cell_a", handle));
                let right = attempt!(self.call_i32_to_i32("cell_b", handle));
                if left <= 0 || right <= 0 || left >= handle || right >= handle {
                    return Err(internal_failure("non-backward recipe graph"));
                }
                pending.push(right);
                pending.push(left);
                continue;
            }
            if kind != 8 {
                return Err(internal_failure("invalid recipe node"));
            }
            let atom = attempt!(self.call_i32_to_i32("cell_x", handle));
            let value = attempt!(self.call_i32_to_i64("cell_payload", handle));
            let child = attempt!(self.call_i32_to_i32("cell_a", handle));
            match atom {
                1 => {
                    result.push(json!({ "literal": { "type": "I64", "value": value.to_string() } }))
                }
                4 => result.push(json!({ "literal": { "type": "Bool", "value": value != 0 } })),
                5 => result.push(json!({ "literal": { "type": "Unit", "value": Value::Null } })),
                2 => {
                    let id = usize::try_from(value)
                        .ok()
                        .filter(|&id| id < NAMES.len())
                        .ok_or_else(|| internal_failure("invalid builtin identity"))?;
                    let prefix = if id < 22 {
                        "builtin"
                    } else if id < 24 {
                        "host"
                    } else {
                        "live"
                    };
                    result.push(json!({ "invoke": format!("{prefix}:{}", NAMES[id]) }));
                }
                15 => {
                    let def_id = u64::from_le_bytes(value.to_le_bytes());
                    result.push(json!({ "invoke": format!("definition:{def_id}") }));
                }
                3 => {
                    let program = attempt!(self.read_boxed(child, depth + 1));
                    if program.get("type").and_then(Value::as_str) != Some("Program") {
                        return Err(internal_failure("quotation recipe has no program"));
                    }
                    result.push(json!({
                        "quotation": program["recipe"],
                        "interface": program["interface"],
                        "witnesses": program["witnesses"],
                    }));
                }
                8 => {
                    let mut boxed = attempt!(self.read_boxed(child, depth + 1));
                    let sig_id = attempt!(self.call_i32_to_i32("cell_y", handle));
                    let schema = attempt!(self.read_signature(sig_id));
                    if let Value::Object(map) = &mut boxed {
                        map.insert(String::from("schema"), json!(schema));
                    }
                    result.push(json!({ "literal": boxed }));
                }
                _ => return Err(internal_failure(format!("unsupported recipe atom: {atom}"))),
            }
            if atom == 2 || atom == 15 {
                let sig_in = attempt!(self.call_i32_to_i32("cell_y", handle));
                let sig_out = attempt!(self.call_i32_to_i32("cell_z", handle));
                let mask = attempt!(self.call_i32_to_i32("cell_w", handle));
                let stack_in = attempt!(self.read_signature(sig_in));
                let stack_out = attempt!(self.read_signature(sig_out));
                witnesses.push(json!({
                    "node": result.len() - 1,
                    "stack_in": stack_in,
                    "stack_out": stack_out,
                    "effects": decode_effects(i64::from(mask)),
                }));
            }
        }
        Ok((result, witnesses))
    }
}

fn read_stack(
    store: &mut Store<HostState>,
    instance: &Instance,
    memory: Memory,
    heap_cursor_global: Global,
) -> Result<Vec<Value>, super::output::Failure> {
    let stack_len_fn = attempt!(instance
        .get_typed_func::<(), i32>(&mut *store, "stack_length")
        .map_err(internal_error));
    let count = attempt!(stack_len_fn.call(&mut *store, ()).map_err(internal_error));
    if !(0..=MAX_STACK).contains(&count) {
        return Err(internal_failure("invalid stack height"));
    }
    let heap_cursor = attempt!(require_internal(
        heap_cursor_global.get(&mut *store).i32(),
        "invalid heap_cursor global"
    ));
    let mut reader = Reader {
        store,
        instance,
        memory,
        heap_cursor,
        observation_work: 0,
    };
    let mut stack = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    for at in 0..count {
        let kind = attempt!(reader.call_i32_to_i32("stack_kind", at));
        let raw = attempt!(reader.call_i32_to_i64("stack_value", at));
        let handle = if kind < 4 {
            Value::Null
        } else {
            json!(u64::from_le_bytes(raw.to_le_bytes()))
        };
        let mut value = attempt!(reader.read_value(kind, raw, 0));
        if let Value::Object(map) = &mut value {
            map.insert(String::from("handle"), handle);
        }
        stack.push(value);
    }
    Ok(stack)
}

fn decode_effects(mask: i64) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (bit, name) in EFFECTS.iter().enumerate() {
        if (mask & (1_i64 << bit)) != 0 {
            out.push(*name);
        }
    }
    out
}

const fn quota_reason_name(code: i64) -> Option<&'static str> {
    match code {
        1 => Some("allocation"),
        2 => Some("recipe leaves"),
        3 => Some("program graph depth"),
        4 => Some("operand bytes"),
        5 => Some("continuation bytes"),
        6 => Some("execution work"),
        7 => Some("reflection/type-observation work"),
        _ => None,
    }
}

fn unsupported_wasm(message: impl Into<String>) -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "wasm",
            outcome: "unsupported",
        },
        message,
    )
}

fn internal_failure(message: impl Into<String>) -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "wasm",
            outcome: "internal-failure",
        },
        message,
    )
}

fn internal_error<E: std::fmt::Display>(error: E) -> super::output::Failure {
    internal_failure(error.to_string())
}

fn internal_from_failure(failure: crate::workflow::output::Failure) -> super::output::Failure {
    internal_failure(failure.message)
}

fn unsupported_from_failure(failure: crate::workflow::output::Failure) -> super::output::Failure {
    unsupported_wasm(failure.message)
}

fn require_internal<T>(value: Option<T>, message: &str) -> Result<T, super::output::Failure> {
    match value {
        Some(item) => Ok(item),
        None => Err(internal_failure(message)),
    }
}

#[cfg(test)]
mod tests {
    fn to_err(error: impl std::fmt::Display) -> String {
        error.to_string()
    }

    #[test]
    fn abi_json_matches_expected_core_exports_and_globals() -> Result<(), String> {
        let abi: serde_json::Value = serde_json::from_str(super::ABI_JSON).map_err(to_err)?;
        let exports = abi["exports"]
            .as_object()
            .ok_or("missing abi.json exports")?;
        assert_eq!(exports.len(), super::CORE_EXPORTS.len());
        for (name, _, _) in super::CORE_EXPORTS {
            assert!(exports.contains_key(name), "missing export {name}");
        }
        let globals = abi["globals"]
            .as_array()
            .ok_or("missing abi.json globals")?;
        assert_eq!(
            globals.len(),
            super::ABI_GLOBALS_I32.len() + super::ABI_GLOBALS_I64.len()
        );
        Ok(())
    }

    #[test]
    fn wasmtime_core_refuses_wasm_gc_start_section_and_bad_imports() -> Result<(), String> {
        let engine = super::build_wasmtime_engine().map_err(|failure| failure.message)?;
        let gc_wat = r#"(module (type $s (struct (field i32))) (func (export "f") (result i32) (i32.const 1)))"#;
        let gc_bytes = wat::parse_str(gc_wat).map_err(to_err)?;
        assert!(
            super::validate_core_wasm(&gc_bytes).is_err(),
            "wasmparser Core validator must reject WasmGC modules"
        );
        assert!(
            wasmtime::Module::new(&engine, &gc_bytes).is_err(),
            "Wasmtime Config must reject WasmGC modules (wasm_gc=false)"
        );

        let start_wat = r#"(module (func $init) (start $init))"#;
        let start_bytes = wat::parse_str(start_wat).map_err(to_err)?;
        assert!(
            super::validate_core_wasm(&start_bytes).is_err(),
            "Core validator must reject start sections"
        );

        let bad_import_wat = r#"(module (import "foreign" "mem" (memory 16 16)))"#;
        let bad_import_bytes = wat::parse_str(bad_import_wat).map_err(to_err)?;
        assert!(
            super::validate_core_wasm(&bad_import_bytes).is_err(),
            "Core validator must reject foreign imports"
        );

        let local_mem_wat = r#"(module (memory 16 16) (export "memory" (memory 0)))"#;
        let local_mem_bytes = wat::parse_str(local_mem_wat).map_err(to_err)?;
        assert!(
            super::validate_core_wasm(&local_mem_bytes).is_err(),
            "Core validator must reject locally defined memory"
        );
        Ok(())
    }

    #[test]
    fn store_limits_blocks_1001st_instance_in_one_store() -> Result<(), String> {
        let engine = super::build_wasmtime_engine().map_err(|failure| failure.message)?;
        let bytes = wat::parse_str("(module)").map_err(to_err)?;
        let module = wasmtime::Module::new(&engine, &bytes).map_err(to_err)?;
        let limits = wasmtime::StoreLimitsBuilder::new().instances(1000).build();
        let mut store = wasmtime::Store::new(&engine, limits);
        store.limiter(|state| state);
        store.set_fuel(1_000_000).map_err(to_err)?;
        for idx in 0..1000 {
            wasmtime::Instance::new(&mut store, &module, &[])
                .map_err(|error| format!("instance {idx} failed: {error}"))?;
        }
        assert!(
            wasmtime::Instance::new(&mut store, &module, &[]).is_err(),
            "1001st instance in one Store must be refused by StoreLimits"
        );
        Ok(())
    }
}
