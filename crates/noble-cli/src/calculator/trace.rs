//! Opt-in per-module calculator trace (`NOBLE_CALCULATOR_TRACE=<file>`).
//!
//! Observation only: when the variable is unset or empty no record is built,
//! the worker is not asked for timings and every output is unchanged. When it
//! is set, one JSON object per executed guest module is appended to the file
//! as a single line (schema `noble-calculator-trace/1`). Module records
//! (`"record": "module"`) carry:
//!
//! - `schema`: `"noble-calculator-trace/1"`.
//! - `session`: process-local session number (the calculator runs one
//!   committed session with one worker process); `line`: 1-based submitted
//!   line (0 = session startup); `phase`: `"session-init"` (the State
//!   initialization module) or `"line"` (a module of a submitted line, which
//!   runs directly on the committed session); `module`: session serial. A
//!   failed line is restored, so its serials are issued again by the next
//!   line: `(line, module)` is unique, `module` alone is not.
//! - `source`, `source_sha256`, `source_bytes`: the exact guest module source
//!   (a host-composed stage; a merged probe consumer is part of it).
//! - `input_types`, `output_types`: operand stack heights before and after.
//! - `epoch`: compiler preparation boundary, `"normal"`, `"quiescent"` or
//!   `"program_free"`; `first`: the generation-0 module of its worker.
//! - `cache`: exact module reuse `{"host": h, "worker": w}`. `h` is `"off"`
//!   (`NOBLE_CALCULATOR_MODULE_CACHE=off`), `"bypass"` (not a canonical
//!   Program-free preparation), `"miss"` or `"hit"` (source check and backend
//!   preparation replaced by an exact recorded artifact); `w` is `"miss"` or
//!   `"hit"` (assembly and V8 compilation replaced by the worker's exact-WAT
//!   module).
//! - Times in integer microseconds: `check_us` (namespace source check),
//!   `backend_us` (compiler preparation, or reuse validation on a host hit),
//!   `prepare_us` (worker round trip for assembly, V8 compilation and
//!   instantiation), worker-measured `rehash_us` (assembler identity re-hash),
//!   `assemble_us` (assembler process), `compile_us` (`new WebAssembly.Module`),
//!   `instantiate_us`, host-measured `execute_us` (worker round trip for
//!   collect, begin, injection, submit and report), `commit_us`, `total_us`.
//! - `wat_bytes`, `wasm_bytes`, `wat_sha256`, `wasm_sha256`; `report_sha256`
//!   (the worker's execution report bytes); `outcome` (worker outcome).
//! - `work_issued`: cumulative backend Work issued by the preparation (on a
//!   host hit, by the recorded preparation).
//! - `host_cache_bytes`: accounted bytes retained by the host module cache
//!   after this module (null when it is off).
//!
//! Session records (`"record": "session"`) carry `session`, `line`,
//! `namespace_us` (namespace module registration) and `worker_start_us`.
//! Additional per-compile metrics are added as extra top-level keys of the
//! module record (for example `compile`); consumers must ignore unknown keys.

use std::io::Write;

pub(super) const VARIABLE: &str = "NOBLE_CALCULATOR_TRACE";
const SCHEMA: &str = "noble-calculator-trace/1";

static SINK: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> =
    std::sync::OnceLock::new();
static SESSIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Open the trace file once per process. An unset or empty variable disables
/// tracing; an unwritable path is a startup error, never a silent drop.
pub(super) fn open() -> Result<(), String> {
    let sink = match std::env::var_os(VARIABLE) {
        Some(path) if !path.is_empty() => Some(std::sync::Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|error| format!("cannot open {VARIABLE} file: {error}"))?,
        )),
        _ => None,
    };
    let _ = SINK.set(sink);
    Ok(())
}

pub(super) fn enabled() -> bool {
    SINK.get().is_some_and(Option::is_some)
}

pub(super) fn next_session() -> u64 {
    SESSIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

pub(super) fn micros(start: std::time::Instant) -> u64 {
    u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX)
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    crate::workflow::intrinsic::sha256(bytes)
}

/// Append one complete record line; a write failure ends the run.
pub(super) fn write(mut record: serde_json::Value) -> Result<(), String> {
    let Some(Some(sink)) = SINK.get() else {
        return Ok(());
    };
    record["schema"] = serde_json::Value::from(SCHEMA);
    let mut line = serde_json::to_vec(&record).map_err(|error| error.to_string())?;
    line.push(b'\n');
    let mut file = sink
        .lock()
        .map_err(|_| "calculator trace lock poisoned".to_string())?;
    file.write_all(&line)
        .map_err(|error| format!("calculator trace write failed: {error}"))
}
