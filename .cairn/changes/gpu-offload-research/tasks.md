## Phase 1: Research and design (this change; no implementation)

- [x] [serial] Define the `GPU-Map-Draft` eligible subset and the explicit `batch.map` reference semantics, grounded in the Core word table, type universe, effect rules and list representation. Compare explicit with inferred dispatch and defer inference. r[GPU-SCOPE-01]
- [x] [serial] Separate compile-time eligibility and kernel generation from runtime device selection. Specify device-independent precharged quota and observational equivalence of selection. r[GPU-DISPATCH-01]
- [x] [serial] Assess exact wrapping signed `I64` feasibility per device API (SPIR-V/Vulkan with and without `shaderInt64`, WGSL, PTX, OpenCL C, MSL, HIP) and specify the native and u32-pair lowerings without division or floating point. r[GPU-NUM-01]
- [x] [serial] Specify fallback, device failure, interruption and empty-batch behaviour, and write the end-to-end cost model with the measured evidence still needed. r[GPU-FALLBACK-01]
- [x] [serial] Write the unexecuted independent correctness plan: positive controls GPU-PLAN-01/02/03/04/05/10/11/12 and negative controls GPU-PLAN-06/07/08/09. Run the pinned Cairn `validate` and `gate proposal|design|tasks` document checks; these do not execute any kernel. r[GPU-GATE-01]

## Phase 2: Future evidence and implementation gates (not undertaken here)

- [ ] [serial] Measure the CPU-side cost terms (`c_disp`, `c_alloc`, `c_pack`, `c_unpack`) on the selected route and decide whether a packed `I64` batch representation and larger limits are prerequisites (coordinate #3/#5/#14). r[GPU-DISPATCH-01]
- [ ] [serial] Implement the independent eligibility checker and the SSA/kernel generator for one native Int64 and one u32-pair variant behind an explicit nonselected profile and an inventoried native binding boundary. Verify the static rejections of GPU-PLAN-07 under `require-gpu`. r[GPU-SCOPE-01] r[GPU-NUM-01]
- [ ] [serial] Execute GPU-PLAN-01…12 against the mathematical oracle on pinned hardware and drivers, including injected device failures (GPU-PLAN-08) and interruption (GPU-PLAN-09). Record per-variant results without promoting any Wasm or proof evidence. r[GPU-FALLBACK-01] r[GPU-GATE-01]
- [ ] [serial] Measure the device cost terms, compute `n*` for representative kernels and make any later selection decision only on these measurements. r[GPU-DISPATCH-01] r[GPU-GATE-01]
