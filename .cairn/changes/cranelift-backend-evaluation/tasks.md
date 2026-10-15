## Phase 1: Research and design (this change; no implementation)

- [x] [serial] Reconcile the intended Wasmtime engine role (§13.4) with the selected `wasm32-core-on-node-v8` configuration, the production Wasmtime 40.0.2 component/quota-core routes and the `44329dc` dev-dependency drift. Record the compatibility and host-integration matrix for Node/V8, Wasmtime JIT/AOT and direct Cranelift JIT/object. r[NATIVE-ENGINE-01]
- [x] [serial] Specify the precompiled-artifact trust boundary: `unsafe` `deserialize`, artifact key binding, authentication and separate offline/online cost reporting. r[NATIVE-AOT-01]
- [x] [serial] Sketch the bounded `Native-Clif-Draft` ABI/runtime (values, stacks, calling convention, dispatch, captures, host calls, allocation, code retirement, traps, quotas, reentrancy) and its sandbox and `unsafe` obligations. Record precisely what differs from SPEC-BE002. r[NATIVE-CLIF-01]
- [x] [serial] Write the unexecuted benchmark and conformance plan with pinned configurations, phase split, NB-W1…W5 workloads, independent oracles, positive controls NATIVE-PLAN-01/02/03/05, negative controls NATIVE-PLAN-04/06/07/08, and pre-registered decision criteria. Record the recommendation and the missing measured evidence. r[NATIVE-GATE-01]
- [x] [serial] Run the pinned Cairn `validate` and `gate proposal|design|tasks` checks for this change. These are document checks only; they do not run Noble, Wasmtime or Cranelift. r[NATIVE-GATE-01]

## Phase 2: Future evidence and implementation gates (not undertaken here)

- [ ] [serial] Implement the opt-in Wasmtime Core engine profile over byte-identical emitted modules with a Rust `Linker` host and a bounded Store-retention policy. Verify the positive controls NATIVE-PLAN-01/02/03 and the negative controls NATIVE-PLAN-04/05/06, which must refuse with no V8 fallback. r[NATIVE-ENGINE-01]
- [ ] [serial] Measure NB-W1…W5 on routes 1 and 2a under the pinned plan, with raw samples retained, and evaluate them against the pre-registered criteria without retuning thresholds. r[NATIVE-GATE-01]
- [ ] [serial] Only if 2a passes: design the authenticated precompile pipeline and an inventoried `unsafe` deserialization crate. Run the NATIVE-PLAN-07 forged/foreign/mutated artifact refusals before any positive AOT load. r[NATIVE-AOT-01]
- [ ] [serial] Only if criterion 4 holds and the SPEC-BE002 sandbox exists: open a separately scoped direct-Cranelift change with dependency adoption records. Run NATIVE-PLAN-08 adversaries and the full differential gate. r[NATIVE-CLIF-01] r[NATIVE-GATE-01]
