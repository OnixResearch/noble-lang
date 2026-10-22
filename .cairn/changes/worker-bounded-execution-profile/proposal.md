# Worker-Typed bounded execution profile

## Why

MW1 requires a bounded execution profile. The local Worker-Typed engine charges a 4,096-step budget unit by unit, but the host cannot interrupt a running transition: there is no interruption point, no cancellation or deterministic deadline outcome, and every host-side exception reads as a guest trap. WORKER-09 asks for accounting units, interruption points at most eight units of guest work apart, and distinct outcomes. r[H-LIMIT-02]

## What Changes

- Define revision `worker-typed/step-4096/interrupt-8` for the typed worker. A new opt-in worker compiler mode emits a host interruption import and calls it after every eighth charged unit of execution or observation work; other profiles and Worker Program v2 keep byte-identical modules. r[H-LIMIT-02]
- The typed engine answers each check with continue, cancel or deadline and records the work between checks itself. Cancellation, a wall-clock host deadline at a check, a guest trap and a host internal failure become distinct outcomes beside deterministic exhaustion; no non-normal outcome exposes typed output. r[H-LIMIT-02]
- Reports carry the profile and the host's interruption observation. Host-selected cancel and fault controls exist only for in-process tests, never as CLI or candidate input. r[H-LIMIT-02]

## Impact

- **Files**: `crates/noble-wasm` worker emission (compiler mode, interruption import, accounting and observation ticks, `runtime/worker-interruption.wat`), the typed engine (`engine.mjs`, `protocol.mjs`, engine configuration), `crates/noble-cli/src/core/worker_typed.rs` and `worker.rs`.
- **Testing**: scoped typed-worker tests that run the real engine for each MW1 row of WORKER-09, the WORKER-02 gate, the existing worker suites, Cairn validation and the source-bound flake gates.
- **Non-claims**: the native-work and suspension rows of WORKER-09 stay MW2 obligations; Worker Program v2 keeps its Wasmtime fuel and epoch budgets; no WORKER case is promoted; WORKER-11 keeps `bounded-execution-profile` among its open gates.
