## Context

This is a research and design record for issue #4. It selects no backend and authorizes no implementation. Claims are grounded in files at base `ffcce53`; anything not observed in source or a pinned tool document is labelled **[INFERENCE]**. External tool facts come from the pinned Wasmtime 40.0.2 source tree that `Cargo.toml` already references (`/nix/store/26a8xzacbx01hidm70lpyphyw926zhc5-source`), which contains Cranelift 0.127.2. Its `docs/security.md`, `docs/examples-pre-compiling-wasm.md` and `docs/stability-tiers.md` are the in-tree sources for the online pages at [docs.wasmtime.dev/security.html](https://docs.wasmtime.dev/security.html) and [docs.wasmtime.dev/examples-pre-compiling-wasm.html](https://docs.wasmtime.dev/examples-pre-compiling-wasm.html). The [cranelift.dev](https://cranelift.dev/) description, a CPU code generator used by Wasmtime for JIT and AOT, matches the pinned crates.

**Link drift.** The issue links `main`/`0ea10d3`; the campaign base is origin/main `44329dc` plus `d154e8b` (cargo fmt) and `ffcce53` (re-pin). Between `0ea10d3` and `ffcce53`:

- `config.json` and `.cairn/specs/language/spec.md` are unchanged. K-NUM-01 is at L557, the engine-role prose at L2214 and H-LIMIT-03 at L2855.
- `worker.rs` gained calculator launch paths. The V8 flags moved from L512 at `44329dc` to L537-L541 at `ffcce53`.
- `44329dc` added Wasmtime and `wat` only as `noble-wasm` dev-dependencies. They are used by `crates/noble-wasm/tests/calculator_collect.rs` and a test in `src/source/emit/metadata.rs` (L257) that executes emitted calculator WAT under Wasmtime.
- The production `noble-cli` Wasmtime dependency (`crates/noble-cli/Cargo.toml` L22) was already present at `0ea10d3`.

### Observed baseline per route

| Fact | Evidence |
|---|---|
| Selected Core target `wasm32-core-on-node-v8`; both representations, optimization off/on; Wasmtime listed in `unselected_tools` | `crates/noble-cli/src/core/runtime/config.json` L4-L6, L88 |
| Pinned Node 24.13.0, V8 13.6.233.17-node.37, wasm-tools 1.245.1, Binaryen 125; flags `--no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up --stack-size=1024 --max-old-space-size=256` | `config.json` L8-L27; launched env-cleared at `worker.rs` L529-L551 |
| Timing policy: 3 warmups, 7 samples, median/min/max/p95, first compile separate, repeated identical bytes may use the V8 process cache | `config.json` L29-L37 |
| Core limits: 64 KiB allocation, 256 recipe leaves, depth 256, 2048 operand bytes, 4096 continuation bytes, 100,000 dispatch steps; 48-byte cells, 4096-cell capacity | `config.json` L39-L49 |
| Per-module preparation in the worker: `wasm-tools parse` + `validate`, optional `wasm-opt` + re-validate, then `new WebAssembly.Module` | `crates/noble-cli/src/core/runtime/engine.mjs` L310-L361 (L340-L354) |
| Emitted module ABI: imported `noble.memory` (16/16 pages; 16/64 calculator; 20/20 live slots), imported `funcref` table of 16384, imported mutable globals, optional `test_emit`, `test_abort`, `test_emit_bound`, `test_clock_bound`, `resource_validate`, `live_*` | `crates/noble-wasm/src/source/emit/imports.rs` L10-L41 |
| Session state shared across modules through one JS memory/table/global set | `engine.mjs` L239-L240, L429-L436, L493, L756 |
| Execution budget is Noble-emitted: `$runtime_tick` charges every dispatch; allocations reserve 48-byte cells against cell and byte limits; V8 has no fuel | `crates/noble-wasm/runtime/accounting.wat` L10-L29; `stack.wat` L77-L88; BE-LIVE-01 |
| Program dispatch: explicit continuation stack and a loop issuing `call_indirect (type $entry)` over the fixed table | `crates/noble-wasm/runtime/stack.wat` L87 |
| I64 `+ - *` lower to Wasm `i64.add/sub/mul` after popping top (right) then deeper (left) | `crates/noble-wasm/runtime/operations.wat` L14-L25 |
| Production Wasmtime 40.0.2 (Cranelift 0.127.2) with `std, runtime, cranelift, component-model, gc, gc-drc`; no `cache` and no `parallel-compilation` features | `crates/noble-cli/Cargo.toml` L22; `Cargo.lock` (`wasmtime` 40.0.2, `cranelift-codegen` 0.127.2) |
| Production Wasmtime Core route: `noble component quota-core` runs a source-matched, import-free emitted Core module with `consume_fuel` and `StoreLimits`, and separates `Trap::OutOfFuel` from the generated allocator quota | `crates/noble-cli/src/component/quota.rs` L1-L4, L156-L173, L254-L260; active change `safety-runtime-quotas` |
| Component hosts (`authorized_fs`, `bounded_region`, `callback_owner`) on Wasmtime with fuel 10,000,000 and 4 MiB memory limits | `crates/noble-cli/src/component/authorized_fs.rs` L93-L97, L338-L346 (similar in the other two) |
| Selected `Component-Async-Bootstrap` pins Wasmtime 40.0.2 | `.cairn/specs/wit-wasi/spec.md` L10 |
| Workspace `unsafe_code = "forbid"` | `Cargo.toml` L11 |
| Wasmtime AOT load `Module::deserialize*` / `Component::deserialize*` are `unsafe fn`: arbitrary input "can trivially be used to execute arbitrary code" | pinned `crates/wasmtime/src/runtime/module.rs` L370-L404, L422-L485; `component/component.rs` L212-L251 |
| `Engine::precompile_module` produces artifacts accepted only by the same Wasmtime version | pinned `engine.rs` L665; `module.rs` L394-L403 |
| A `Store` never frees instances until it is dropped ("unsuitable for creating an unbounded number of instances") | pinned `runtime/store.rs` L149-L155 |
| Wasmtime Tier 1: x86_64 Linux/macOS/Windows; aarch64 Linux/macOS Tier 2; Wasm `gc` proposal Tier 2 ("production quality" missing); Pulley Tier 2 | pinned `docs/stability-tiers.md` L25-L27, L78-L79, L86, L89 |
| Cranelift `iadd`/`isub`/`imul` wrap modulo `2^B` regardless of signedness | pinned `cranelift/codegen/meta/src/shared/instructions.rs` L1796-L1856 |
| `cranelift-jit` returns finalized code as `*const u8`; calling it needs `unsafe`. `cranelift-jit`, `cranelift-module` and `cranelift-object` are absent from `Cargo.lock` | pinned `cranelift/jit/src/backend.rs` L266; `Cargo.lock` |

**Engine role reconciliation.** §13.4 names Wasmtime as the *intended* engine role and explicitly not a version claim. That intent is already realized for the WIT/component profiles and the quota-core route. It is not realized for Core-Bootstrap `run`/`session`/`compile`, the live REPL (BE-LIVE-01 pins Node/V8) or the calculator. Nothing here changes that selection. r[NATIVE-ENGINE-01]

**Precedent.** The active `c-backend` change (SPEC-BE002) already specifies an opt-in, nonselected native C11 AOT route with scope, admission, sandbox and differential-gate obligations (CB-SCOPE-01 … CB-GATE-01). Route 3 reuses those obligations and does not duplicate them. The differences are recorded in the decision "Direct Cranelift experiment: what differs from SPEC-BE002" below.

## Compatibility and host-integration matrix

Route 2 is split into JIT (2a) and precompiled/AOT (2b) because their trust differs. Route 3 is split into in-process JIT (3a) and object/AOT emission (3b).

| Dimension | 1. Node/V8 (selected) | 2a. Wasmtime/Cranelift JIT | 2b. Wasmtime precompiled (`.cwasm`) | 3a. Direct Cranelift JIT | 3b. Direct Cranelift object/AOT |
|---|---|---|---|---|---|
| Guest lowering | Existing checked plan → WAT + 2,314 lines of owned runtime WAT | **Byte-identical** emitted Wasm (same plan/WAT/assembly) | Same Wasm, compiled offline | **New** lowering from the checked plan (`noble_kernel::untrusted::Checked` consumed by `noble-wasm/src/source/plan`) to CLIF; runtime WAT re-expressed natively | Same as 3a, then emitted as an object file |
| Assembly/validation | `wasm-tools parse/validate`, optional `wasm-opt` + re-validate (engine.mjs L340-L351) | Same Wasm bytes; `Module::new` re-validates internally [INFERENCE: wasmparser validation inside Wasmtime] | Validation happens at precompile time; load-time checks are "lightly validated" only (module.rs L379-L381) | CLIF verifier only; it checks IR well-formedness, not Noble typing or memory safety | CLIF verifier, then linker |
| Required Wasm features | memory32, i32/i64, typed `call_indirect`, `funcref` table, imported mutable globals, passive bulk-memory (BE-LIVE-01). WasmGC only for the unselected `wasm-gc` representation | All Tier 1 core features; WasmGC is Tier 2 and must be excluded or reported separately | Same as 2a, fixed at precompile | None (no Wasm) | None |
| Imports/host adapters | JS host (`engine-host.mjs`, `engine.mjs`, `values.mjs`, `protocol.mjs`) provides `noble.*` imports and decodes values | **New Rust `Linker` host** reproducing every `noble.*` import with the same semantics, plus a Rust port of value decoding, metrics and the report protocol | Same as 2a | New native trampolines; host receives cell handles, not pointers | Same as 3a, plus a loader and an IPC shim when sandboxed (CB-HOST-01 pattern) |
| Shared session state | One persistent Node process; JS-owned memory/table/globals shared by all instances; old Program table slots retained (H-LIVE-02, BE-LIVE-01) | One `Store` owns the shared Memory/Table/Globals; Program handles are Store-local. **Instances are never freed before the Store drops** (store.rs L149-L155), so retained code grows monotonically per session | Same as 2a | Host-owned arena and append-only entry table; code retirement needs explicit refcounts | Process per artifact or long-lived worker; code fixed per load |
| Deterministic work quota | Noble-emitted dispatch-step and cell metering; no engine fuel | Same emitted metering, which must stay authoritative. Optional Wasmtime `consume_fuel` is a *second, separately named* accounting unit, as in quota.rs L254-L260 | Same; fuel instrumentation fixed at precompile [INFERENCE: fuel is a compile-time config] | Must emit its own charges at dispatch/backedges/allocations; no engine help | Same as 3a |
| Deadlines/cancellation | 120 s reply watchdog (worker.rs L399-L403) kills/poisons the Node session; not a quota | `epoch_interruption` deadlines (config.rs L753) or a watchdog; separate from fuel | Same | Watchdog/process kill only, unless polled flags are emitted | Process kill |
| Stack/recursion | `--stack-size=1024` plus Noble continuation limits | `max_wasm_stack` (config.rs L804) plus Noble limits | Same | No native recursion: explicit continuation stack; guard pages as crash-only backstop | Same |
| Traps | JS exceptions mapped to Noble outcomes | `wasmtime::Trap` variants (quota.rs L254) mapped identically | Same | Status-code ABI (no unwinding); faults are native termination, never Noble traps | Same; signal = native termination |
| Isolation | Wasm validation + V8 sandbox in a separate, env-cleared Node process (worker.rs L529-L551) | Wasm validation + Wasmtime sandbox (security.md L19-L46), **in-process** with the trusted CLI unless a worker process is added | Same as 2a **only if** the artifact is authentic; a forged `.cwasm` is arbitrary native code | **None intrinsic**: JIT code has full process authority | **None intrinsic**: an ELF can issue direct syscalls (CB-SANDBOX-01) |
| `unsafe_code = "forbid"` | Unaffected (external process) | Unaffected (safe Wasmtime API) | **Conflicts**: `deserialize*` is `unsafe fn` | **Conflicts**: calling finalized `*const u8` needs `unsafe` | Loader/exec in a separate runner; no in-process unsafe if loaded only by a sandbox runner |
| OS/CPU targets | Wherever the pinned Node runs; flake targets x86_64 Linux (README "Try it") | Tier 1 x86_64 Linux/macOS/Windows; aarch64 Tier 2 | Artifact bound to target triple and exact engine config | Each Cranelift ISA (x64, aarch64, riscv64, s390x) needs its own ABI/runtime port and evidence | Same as 3a plus linker/sysroot per target |
| Browser | Same Wasm bytes are browser-portable in principle; the host is Node-specific (spawned tools, fs) [INFERENCE: needs a separate browser host] | Not a browser engine; the guest Wasm stays browser-portable | Not browser | Not browser | Not browser |
| AOT | None; V8 process code cache only (config.json L35) | N/A | `Engine::precompile_module` (engine.rs L665); load is version- and config-bound and `unsafe` | Optional: `cranelift-object` | Object + linker + loader (CB-ADMIT-01 pattern) |
| Component boundary | Core only ("no-component-ABI-claim", config.json L86) | Already the selected component engine; a Core route can share the Engine config | Same | Must re-implement the Canonical ABI to interoperate; out of the initial profile | Same |
| Live REPL / slots | Selected; BE-LIVE-01 pins Node/V8 and forbids silent engine substitution | Needs its own live engine profile; not a drop-in | No (no reload) | Needs W^X code publication, active-frame pinning and retirement | No |
| Existing evidence | M4 acceptance, CORE cases, Mathlib I64 comparator (README) | Component M5–M8 gates; S-CASE-13 quota route (in progress). **None** for Core sessions, captures/recipes or the live REPL | None | None | None (C-route evidence does not transfer) |
| Removes | n/a | Node process, JS↔Rust JSON protocol, external `wasm-tools`/`wasm-opt` process spawns if the in-process `wat` 1.243.0 assembler is used [INFERENCE: needs byte-equality evidence against wasm-tools 1.245.1 output] | Engine compile from the online path | WAT emission, assembly, Wasm validation, V8 | Same as 3a |
| Adds | n/a | Rust host port, Store-retention policy, engine config pin and fuel/epoch policy | Authenticated artifact pipeline and a reviewed `unsafe` boundary | Native runtime (arena, stacks, dispatch, trap ABI), executable-memory management, sandbox, per-target ABI, new lowering correspondence, new `unsafe` boundary | Same as 3a, plus toolchain/loader admission |

## Decisions

### Decision: Keep Node/V8 as the selected Core-Bootstrap engine

**Choice:** This change leaves `config.json`, its `unselected_tools` entry, BE-LIVE-01 and the M4 selection unchanged. Route 1 remains the baseline that both alternatives must match. r[NATIVE-GATE-01]

**Rationale:** Route 1 is the only CPU route with retained Core-Bootstrap acceptance evidence for runtime captures, recipes, traps, quotas and the live REPL. Neither alternative has comparable evidence today.

### Decision: The next experiment is an opt-in Wasmtime/Cranelift Core engine over identical bytes

**Choice:** If a follow-up implements anything, it should be a nonselected, explicitly requested engine profile (working name `wasm32-core-on-wasmtime`). It would run the **same emitted, validated Core Wasm bytes** as Route 1, in JIT mode first.

- The profile pins the Wasmtime version, cargo features and complete `Config`: Cranelift opt level, `consume_fuel`, `epoch_interruption`, `max_wasm_stack`, memory reservation/guard, enabled proposals and the `signals_based_traps` setting.
- It reproduces every `noble.*` import in a Rust `Linker`.
- It keeps Noble-emitted dispatch/allocation metering as the authoritative profile accounting. Engine fuel, if enabled, is a separately named backstop with its own accounting revision.
- It bounds Store-retained instances per session (refuse or end the session at the bound) because Wasmtime never frees instances before the Store drops.
- It refuses unsupported workloads (live REPL, `wasm-gc` representation, calculator, live slots) instead of falling back to V8, and the reverse also holds.

r[NATIVE-ENGINE-01]

**Rationale:**

- This route keeps the checker boundary and the guest lowering untouched, so semantic preservation reduces to engine conformance for a fixed byte string.
- Wasmtime 40.0.2 is already pinned, vendored and trusted by production component routes. Its marginal trust is therefore the new Rust host adapters, not a new compiler.
- Running in-process with `wat` 1.243.0, which is already a production dependency, could remove external tool spawns and the V8 compile from the per-module path that #16 measures. Whether this actually helps is unmeasured [INFERENCE].
- Store retention is the main new lifetime hazard. It must be bounded before long sessions are claimed (coordinate #3/#5).

### Decision: Precompiled (AOT) Wasmtime artifacts only behind an authenticated, inventoried unsafe boundary

**Choice:** Leave 2b out of the first experiment. A later AOT profile MUST:

- produce artifacts only with `Engine::precompile_module` from bytes it has independently re-derived from the accepted source;
- bind the artifact key to the Wasm bytes, Wasmtime version, cargo features, full `Config` and target triple;
- load only artifacts whose bytes equal the host's own trusted precompile output or come from a separately reviewed authenticated build route;
- confine `Module::deserialize*` to one minimal crate with an explicit `unsafe_code` exemption and a VT-NATIVE-01 safety argument, never by relaxing the workspace `forbid`;
- report offline compile cost and online load/instantiate cost separately.

r[NATIVE-AOT-01]

**Rationale:** Wasmtime documents that deserialized bytes are only lightly validated and that arbitrary input can execute arbitrary code (module.rs L373-L385). Wasm's sandbox therefore does not cover a forged artifact. A digest or flag supplied by the candidate is not authentication (compare CB-ADMIT-01).

### Decision: Direct Cranelift is a separately scoped experiment, not a backend replacement

**Choice:** Do not pursue Route 3 until two conditions hold:

1. Route 2 measurements show that engine compile/instantiate, or a representation Wasm cannot express, still dominates after the frontend and lowering are excluded.
2. The native-sandbox prerequisites of SPEC-BE002 exist.

If Route 3 is pursued, it uses the bounded profile and ABI in "Direct-native ABI and runtime sketch" below, with independent checking before lowering and out-of-process execution for untrusted Programs. r[NATIVE-CLIF-01]

**Rationale:** Cranelift code generation does not reproduce Wasm validation, memory isolation, checked control transfer or import-only authority. It also requires new `unsafe` call/load boundaries, a native runtime, per-target ABIs and a new lowering correspondence. The issue's cost decomposition, roughly 40% source check, 40% backend and 20% worker in #16, suggests that removing Wasm would at most address part of the worker share [INFERENCE: pre-port tree figures, not measured here].

### Decision: Direct Cranelift experiment: what differs from SPEC-BE002

**Choice:** Reuse CB-SCOPE-01 (scope, refusal), CB-IDENTITY-01 (build vs semantic identity), CB-ORDER-01 (explicit sequencing), CB-LIMIT-01 (limits), CB-HOST-01 (mediated `test.emit`), CB-SANDBOX-01 and CB-ADMIT-01 unchanged in spirit for any native artifact. Cranelift adds the following:

1. **In-process JIT** (3a), which C does not have. It needs W^X code publication, executable-memory accounting, retirement and an in-process `unsafe` call boundary. For *untrusted* Programs, 3a is admissible only inside an out-of-process sandboxed worker that owns the JIT, never in the trusted CLI process.
2. **No C undefined-behaviour layer.** Cranelift `iadd/isub/imul` are defined modulo `2^B`, so CB-NUM-01's unsigned-only workaround is unnecessary. Sequencing is explicit SSA, so CB-ORDER-01's temporaries are natural.
3. **CLIF verification** replaces C11 syntax validation. Neither is a correctness proof.
4. **No external C compiler/linker** for 3a. Admission compares re-derived CLIF/machine code from the accepted subject instead of a reproducible ELF build (3b keeps the CB-ADMIT-01 linker obligation).
5. **New dependencies** (`cranelift-frontend`, `cranelift-module`, `cranelift-jit`/`cranelift-object`) need VT-NATIVE-03 adoption records. They are absent from `Cargo.lock` today.

r[NATIVE-CLIF-01]

**Rationale:** Restating the C obligations would create two diverging copies of the same contract. Only these deltas are Cranelift-specific.

### Decision: Selection requires pinned, pre-registered differential evidence

**Choice:** The decision criteria below are fixed before any measurement. No route changes selection without passing every correctness gate, and unsupported workloads are reported as unsupported, never as zero cost. r[NATIVE-GATE-01]

**Rationale:** BE-REPORT-01 and BE-COMPARE-01 already forbid treating blocked candidates or failed correctness as performance wins. Pre-registration prevents thresholds from being tuned after the fact.

## Direct-native ABI and runtime sketch (bounded profile `Native-Clif-Draft`)

**Scope.** Independently accepted, resource-free, nonrecursive Core-Bootstrap candidates over `Unit`, `Bool`, `I64`, UTF-8 `Text`, inert `Syntax`, homogeneous `List`, `Pair`, `Sum` and `Program`. The only host words are `test.emit` and `test.abort`. Declared modules, components, live slots/REPL, the calculator profile, resources, recursion, FFI and other targets refuse explicitly. Initial target: `x86_64-unknown-linux-gnu`, matching the flake.

**Checker boundary.** Lowering consumes only an independently accepted candidate: the same `noble_kernel` acceptance that precedes `noble-wasm/src/source/plan`. Neither CLIF verification nor successful compilation substitutes for kernel type/effect/capture/ownership acceptance or host authorization.

- **Values.** 16-byte stack slot `{kind: u32, reserved: u32, payload: u64}`, mirroring the `slot_kind`/`slot_value` split used in `operations.wat`. Aggregates are 48-byte cells in one host-owned arena and are addressed by **u32 cell handles, never raw pointers**. Every access is checked against `cell_limit` (as in `accounting.wat`), so guest code cannot name host memory (K-SAFE-03, S-MEM-01).
- **Operand and continuation stacks.** Bounded arrays inside the arena (2048 operand bytes, 4096 continuation bytes per `config.json`), indexed by `sp`/`cp`. There is no native recursion. Dispatch is a single loop, mirroring `stack.wat` L77-L88.
- **Calling convention.** Every compiled operation or entry has the one signature `extern fn(ctx: *mut Ctx, env: u32) -> u32` using the target's default call convention. The `u32` status is normal, trap code or quota reason. There is no unwinding through Rust or JIT frames; Rust panics never cross the boundary.
- **Program values and dispatch.** A Program is a cell `{entry: u32, env: u32, recipe: u32}`. `entry` indexes a host-owned, **append-only** entry table of code addresses that all share the single signature, so the dispatcher's indirect call cannot reach a mistyped target. The index is bounds-checked before each indirect call. `compose` builds a recipe cell; `run` pushes continuations; `reflect` reads the normalized recipe. No source compiler is reachable from `run` (W-EXEC-02).
- **Captures.** Immutable environment cells written once at `quote`. Captured Programs keep their `entry` index and recipe, so rebinding a display name cannot change them (K-DEF-01/02).
- **Host calls.** CLIF calls a host trampoline `host_emit(ctx, text_cell) -> u32`. The trusted side validates the handle, bounds and UTF-8, copies the bytes into host memory and then checks current authorization. In sandboxed execution this becomes the CB-HOST-01 framed IPC. Host trampolines never re-enter the guest; a busy flag in `Ctx` enforces non-reentrancy.
- **Allocation and reclamation.** Bump arena with mark/rollback, charging before growth, like `$reserve`/`$rollback` in `accounting.wat`. Allocator choice and reclamation belong to #5; ownership/lifetime contracts belong to #3.
- **Code loading and retirement.** One JIT module per accepted submission, published W^X. A generation is retired only when no Program cell, active frame, saved Program or captured recipe references any of its entry indices. Saved old definitions pin their generation (H-LIVE-02 behaviour). Retained code bytes are charged to a declared quota; exhaustion refuses new submissions rather than evicting live code.
- **Traps.** Arithmetic cannot trap (wrapping). Malformed tags, out-of-range handles, `test.abort` and quota return status codes. A hardware fault or guard-page hit is native termination, reported separately and never as a Noble trap or success.
- **Quotas and interruption.** Charges are emitted at dispatch, allocation, composition and text growth. A deadline or cancellation is a watchdog/process action, reported separately from deterministic quota exhaustion. The accounting revision is recorded in every report (H-LIMIT-02/03).
- **Trust and sandbox obligations.** The JIT and all generated code run in a sandboxed worker meeting CB-SANDBOX-01: closed inherited FDs except private IPC, no ambient credentials, confined filesystem, no network, exec or clock, and CPU/memory/wall limits. `unsafe` code (executable memory, calling code pointers) is confined to one crate with an explicit lint exemption and a VT-NATIVE-01 safety argument; the workspace `forbid` stays. Cranelift, the native runtime and the sandbox are new trusted subjects. Existing M4 Wasm evidence, Mathlib I64 comparisons and PO-17/18 do not transfer.

**Work removed versus added.** Removed: WAT text emission, runtime WAT assets, `wasm-tools`/`wasm-opt`, the Node process, V8 compilation and the JSON value protocol. Added: CLIF lowering and its correspondence evidence, a native runtime, executable-memory lifecycle, the sandboxed worker and IPC, per-target ABI ports, dependency adoption and the `unsafe` boundary.

## Benchmark and conformance plan (unexecuted)

**Pins recorded per run (BE-CONFIG-01):**

- source tree hash and Cargo/flake locks;
- `config.json` hash; Node/V8, wasm-tools, Binaryen, Wasmtime/Cranelift versions and features, plus the full Wasmtime `Config` dump;
- representation (`managed-linear-memory`; `wasm-gc` reported separately or as unsupported) and `wasm-opt` off/on × Cranelift `OptLevel` none/speed, crossed explicitly and never conflated;
- the V8 flags as pinned;
- hardware (CPU model and microcode, cores pinned with `taskset`, SMT state, governor, kernel, RAM) and cache state (cold process, warm process, warm instance, V8 process cache permitted per config).

**Phases measured separately:** source parse/check; kernel acceptance; plan/lowering; WAT emission; assembly (wasm-tools vs in-process `wat`); `wasm-opt`; engine compile (`WebAssembly.Module` vs `Module::new` vs CLIF define+finalize); artifact load (`deserialize`); instantiation; first result; warm steady state. Memory metrics are peak RSS (`VmHWM`), guest logical allocation (runtime metrics), retained instances/code bytes per session and artifact size. Host-crossing cost is measured as `test.emit` round-trips.

**Workloads:**

| ID | Workload | Inputs |
|---|---|---|
| NB-W1 | Pure numeric | CORE-14 boundary; long add/sub/mul chains; equality ladders |
| NB-W2 | Repeated short invocations | N-submission sessions (N = 1, 10, 100, 1000); the #16 calculator module stream replayed as recorded module bytes |
| NB-W3 | Host-effect heavy | CORE-17 ordered trap; composed chains of 1–256 `test.emit` |
| NB-W4 | Runtime Programs | CORE-03/05/09; ADAPT-08/09/10/12 left/right/balanced trees at depth 1–256; reused captures; saved old definitions after rebinding |
| NB-W5 | Limits | `dispatch_steps`, `allocation_bytes`, `composition_depth`, `continuation_bytes` at bound and one over; `test.abort` traps |

**Sampling:** 3 warmups and 7 samples per in-process configuration (config.json policy), plus at least 10 cold-process repetitions. Report median, min, max and p95, and keep all raw samples.

**Correctness oracles (independent of every engine):** the `expected` fields of cases.json and adaptation-cases.json; the Lean/Mathlib I64 model (`verification/mathlib-i64`) for arithmetic expectations; differential comparison of ordered stack values, signed I64 text, normalized recipes, request/effect prefixes, trap and quota classifications across routes. Any discrepancy fails that workload for that route.

**Conformance scenario designs (unexecuted; not registered in `specs/conformance`):**

| ID | Given | When | Then |
|---|---|---|---|
| NATIVE-PLAN-01 | CORE-14 and `-9223372036854775808 1 -`, `4294967296 4294967296 *` | Run on routes 1/2a (and 3 if built), opt off/on | All yield the K-NUM-01 values; route-2 Wasm bytes are identical to route 1 |
| NATIVE-PLAN-02 | CORE-03/05/09 with captures supplied after compilation | Run on each route | Identical stacks and normalized recipes; no recompilation per tree |
| NATIVE-PLAN-03 | CORE-17 | Run on each route | Request trace `test.emit:first`, `test.abort`; second Program never starts |
| NATIVE-PLAN-04 | NB-W5 one-over inputs | Run on each route | Same quota *class* and stage per declared profile; fuel exhaustion (if enabled) is reported as a distinct unit; no partial success |
| NATIVE-PLAN-05 | 1000-submission session on 2a | Run until the declared instance bound | Refusal or explicit session end at the bound; no unbounded Store growth |
| NATIVE-PLAN-06 | Live REPL, calculator, live-slot and `wasm-gc` requests on 2a | Request the engine | Explicit refusal; no V8 fallback and no partial session mutation |
| NATIVE-PLAN-07 | Mutated, foreign-version, foreign-config and wrong-target `.cwasm` | Load on 2b | Refused before `deserialize` by key/byte authentication; zero guest execution |
| NATIVE-PLAN-08 | Direct-syscall and out-of-range-handle adversaries (3a/3b) | Execute in the sandbox | Sandbox denial or native termination, reported separately; zero protected actions |

**Pre-registered decision criteria:**

1. **Correctness gate (all routes).** 100% agreement with independent expectations on NATIVE-PLAN-01…06 and every NB workload the route claims. Unsupported workloads are listed explicitly. Any discrepancy blocks the route regardless of performance.
2. **Route 2a becomes a candidate for a later selection proposal** when the correctness gate passes and, on NB-W2 and the #16 stream, the median cold first-result time improves by at least 25% versus route 1 with non-overlapping min/max ranges. NB-W1/W4 warm steady-state must not regress by more than 10% at median, peak RSS must not exceed route 1 by more than 25%, and Store retention must be bounded (NATIVE-PLAN-05).
3. **Route 2b is considered** only if 2a passes and the online load+instantiate time saves at least 50% of 2a's compile time on NB-W2. Offline cost and artifact size are reported, and the NATIVE-PLAN-07 authentication controls must pass.
4. **Route 3 is justified** only if, after 2a, engine compile + instantiate is still at least 30% of per-module wall time with frontend/lowering excluded, *or* a required representation cannot be expressed in Wasm. The SPEC-BE002 sandbox prerequisites must also exist.
5. **Retain V8** if none of the above holds. Retaining V8 is the default outcome and requires no new evidence.

These thresholds are proposals fixed now. Changing them requires a recorded revision before data collection.

## Rejected alternatives

- **Replacing V8 with Wasmtime for all Core profiles at once.** Rejected: there is no Core session, live REPL or calculator evidence on Wasmtime, BE-LIVE-01 pins V8, and Store retention is unbounded.
- **Treating Wasmtime fuel as the profile's work quota.** Rejected: fuel is an engine-instruction unit that changes with optimization and engine version. Noble's emitted dispatch accounting stays authoritative; fuel is a separately named backstop (H-LIMIT-03).
- **Loading cached `.cwasm` artifacts keyed only by a hash file.** Rejected: `deserialize` trusts its input; a candidate-supplied hash is not authentication.
- **Running direct Cranelift JIT in the trusted CLI process.** Rejected for untrusted Programs: generated code has full process authority.
- **Using CLIF verification as an admission check.** Rejected: it checks IR well-formedness, not Noble semantics or authority.
- **Inheriting M4/Mathlib/C-backend evidence for a new route.** Rejected by K-VERIFY and CB-GATE-01 precedent.

## Tests

This change runs only structural Cairn gates and validation. The benchmark and conformance plan above is unexecuted. Positive controls (NATIVE-PLAN-01…03, 05) and negative controls (NATIVE-PLAN-04, 06–08) are designed, not run.

## Claim boundaries

- No performance claim. Every speed or memory effect stated above is [INFERENCE] until NB workloads run.
- No correctness claim for any engine or lowering. No proof, conformance state or receipt changes.
- Wasmtime's sandbox is an engine property that holds only for validated Wasm and authentic artifacts. Cranelift-generated native code does not inherit it.
- Wasmtime facts are for the pinned 40.0.2 source and may differ in other versions.

## Risks / Trade-offs

- A Rust host port of `engine.mjs` and `engine-host.mjs` can silently diverge in value decoding, metrics or protocol. Mitigation: byte-identical module input plus differential traces (NATIVE-PLAN-01…04).
- Store-local Program handles cannot migrate between Stores. Bounding retention may force session restarts that V8 does not need; this needs #3 coordination.
- Removing `wasm-tools`/`wasm-opt` spawns changes the assembler identity. `wat` 1.243.0 output must be shown byte-identical to, or separately admitted from, wasm-tools 1.245.1 output.
- `gc` is Tier 2 in Wasmtime 40.0.2, so `wasm-gc` results on Wasmtime are not production-tier evidence.
- A native route doubles the backend-correctness surface and adds a sandbox dependency that may not exist on every host.

## Acceptance mapping

| Issue #4 outcome checkbox | Section |
|---|---|
| Compatibility and host-integration matrix for the three routes | [Compatibility and host-integration matrix](#compatibility-and-host-integration-matrix); "Observed baseline per route" and "Engine role reconciliation" in [Context](#context) |
| Bounded direct-native ABI/runtime sketch and explicit sandbox/trust obligations | [Direct-native ABI and runtime sketch](#direct-native-abi-and-runtime-sketch-bounded-profile-native-clif-draft); Decisions "Direct Cranelift is a separately scoped experiment" and "what differs from SPEC-BE002"; r[NATIVE-CLIF-01] r[NATIVE-AOT-01] |
| Reproducible benchmark and conformance plan with decision criteria | [Benchmark and conformance plan](#benchmark-and-conformance-plan-unexecuted); r[NATIVE-GATE-01] |
| Recommendation: retain V8, add Wasmtime/Cranelift, or a separately scoped native experiment, with missing measured evidence identified | [Decisions](#decisions) (retain V8 now; opt-in Wasmtime experiment next; native only after criteria 4) plus pre-registered criteria 1–5; missing evidence: every NB workload and NATIVE-PLAN case on routes 2a/2b/3, the Store-retention bound, `wat` vs `wasm-tools` byte equality, and the per-phase split for the #16 stream |
