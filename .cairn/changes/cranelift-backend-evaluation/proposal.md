# Evaluate Cranelift through Wasmtime versus a direct native backend (issue #4)

## Why

Issue [#4](https://github.com/OnixResearch/noble-lang/issues/4) asks whether Cranelift should become a CPU execution route for Noble, and insists on distinguishing a Wasm *engine* change from a new *native backend*. No recommendation, compatibility matrix, trust model or pre-registered measurement plan exists today.

The baseline is more mixed than "Wasmtime is unselected". At base `ffcce53` (origin/main `44329dc` plus a formatting and re-pin commit; issue links pin `0ea10d3`), the Core-Bootstrap `run`/`session`/live REPL target is `wasm32-core-on-node-v8`. It pins Node 24.13.0/V8 13.6.233.17-node.37, runs with `--no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up`, and lists Wasmtime under `unselected_tools` (`crates/noble-cli/src/core/runtime/config.json` L4, L27, L88; flags launched at `crates/noble-cli/src/core/worker.rs` L537-L541). Production `noble-cli` already links Wasmtime 40.0.2 with Cranelift 0.127.2 (`crates/noble-cli/Cargo.toml` L22; `Cargo.lock`) for its component routes and the `quota-core` route, which runs import-free emitted Core modules under fuel (`crates/noble-cli/src/component/quota.rs` L156-L173). Commit `44329dc` only adds Wasmtime and `wat` as `noble-wasm` *dev*-dependencies (`crates/noble-wasm/Cargo.toml` L11-L13) so calculator tests can execute emitted runtime WAT. The language specification lists Wasmtime as the *intended* engine role, not as a version-compatibility claim (`.cairn/specs/language/spec.md` §13.4, L2214). Each route therefore needs a per-profile reconciliation, not a blanket engine swap. r[NATIVE-ENGINE-01]

A direct Cranelift backend would bypass the isolation that Wasm validation supplies and Wasmtime enforces. It would also introduce `unsafe` loading or calling boundaries into a workspace that sets `unsafe_code = "forbid"` (`Cargo.toml` L11). Both routes therefore need explicit trust obligations before anyone implements them. r[NATIVE-AOT-01] r[NATIVE-CLIF-01]

## What Changes

- Add a research/design record (`design.md`) with: a compatibility and host-integration matrix for (1) the selected Node/V8 route, (2) Wasmtime/Cranelift over the *same* emitted Wasm in JIT and precompiled forms, and (3) a direct Noble → Cranelift IR → native route; a bounded direct-native ABI/runtime sketch with sandbox and trust obligations; a reproducible benchmark and conformance plan with pre-registered decision criteria; and a recommendation that names the measured evidence still missing. r[NATIVE-ENGINE-01] r[NATIVE-CLIF-01] r[NATIVE-GATE-01]
- Add four `backend-experiments` requirements. They apply only *if* a later change pursues either route:
  - An opt-in Wasmtime Core engine must reuse byte-identical emitted Core modules, use a declared accounting revision and refuse rather than fall back silently. r[NATIVE-ENGINE-01]
  - Precompiled artifacts must be loaded only from authenticated, exactly matched trusted builds behind an inventoried `unsafe` boundary. r[NATIVE-AOT-01]
  - A direct Cranelift experiment must be separately scoped, checked before lowering and sandboxed. r[NATIVE-CLIF-01]
  - No route is selected without pinned differential evidence. r[NATIVE-GATE-01]
- Recommendation (design only): keep Node/V8 selected. Make an opt-in, nonselected Wasmtime/Cranelift Core engine experiment the next step. Defer a direct Cranelift backend to a separately scoped experiment that is admissible only after that measurement and after native-sandbox prerequisites shared with SPEC-BE002 exist.

## Impact

- **Files**: `.cairn/changes/cranelift-backend-evaluation/{proposal,design,tasks}.md`, `metadata.json` and the ADDED delta `specs/backend-experiments/spec.md`. This change does not edit canonical `.cairn/specs/`, `specs/conformance/*.json`, the family manifest, roadmap/status, generated views, the ledger or source code.
- **Testing**: Pinned Cairn `validate` and the `gate proposal|design|tasks cranelift-backend-evaluation` gates. These are structural document checks. They do not execute Noble, Wasmtime or Cranelift and do not measure anything. The benchmark and conformance plan in `design.md` is unexecuted.
- **Coordination**: #3 (saved-Program/code lifetime, Store and code retirement), #5 (allocator layer for a native runtime), #16 (per-module compile latency; Route 2 removes only the engine/assembler share), #14 (numeric-graph CPU reference; not a Cranelift consumer here) and #2 (GPU dispatch reuses this CPU route analysis).

## Out of Scope

Implementing either route; selecting or replacing the backend; changing `config.json`, `unselected_tools`, the live REPL engine, WIT/component hosts, Cargo dependencies or vendored sources; running benchmarks; GPU offload (#2); allocator selection (#5); ownership/lifetime redesign (#3); live-reference semantics (#1); and any proof, receipt or conformance-state promotion.
