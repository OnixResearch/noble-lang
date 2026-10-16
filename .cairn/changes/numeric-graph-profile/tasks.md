## Phase A: bounded design decision (this change)

- [x] [serial] Publish the operation and type whitelist, the exact disjoint `ngraph-i64/0` versus `ngraph-f32/0` boundary with no `I64` reinterpretation, and the binary32 reference contract. The contract covers type/ABI, exact literals, conversions, rounding order, no fusion or reassociation, signed zero, NaN class, infinities, subnormals, index rules and bitwise observation equality. Design D1–D3. r[NGRAPH-SCOPE-01] r[NGRAPH-F32-01]
- [x] [serial] Publish effect exclusions, finite budgets charged on the unpruned expansion, and stage-scoped unsupported diagnostics. Design D4–D6. r[NGRAPH-EXTRACT-01]
- [x] [serial] Specify admission, the deterministic symbolic stack-to-graph extraction over checked derivations, the independent graph checker, logical identity versus packed layout, and forward observations. Design D7–D8. r[NGRAPH-EXTRACT-01]
- [x] [serial] Specify checked fact provenance (derived, input-validated, #8-applicable evidence or a fail-closed guard). Design D9. r[NGRAPH-FACT-01]
- [x] [serial] Specify the named derivative policies with static default refusal of boolean and index paths, and the training contract with immutable parameter snapshots and reproducible provenance. Design D10–D11. r[NGRAPH-DERIV-01] r[NGRAPH-SNAP-01]
- [x] [serial] Specify the ownership and backend boundary and the mapping to kernel admission, source extraction, runtime ownership and the proposed open proof-to-artifact obligations NG-PO-A..H. Design D12–D13. r[NGRAPH-OWN-01] r[NGRAPH-PROOF-01] r[NGRAPH-BENCH-01]
- [x] [serial] Validate this change with Cairn `validate` and the proposal, design and tasks gates. These are document checks only: no implementation, execution, proof or performance evidence. r[NGRAPH-PROOF-01]

## Phase B: CPU-only feasibility experiment (approved on #14 on 2026-10-15)

Prerequisites P1–P6 are recorded in design "Phase B approval and decisions": B0(b), the isolated crate `crates/noble-ngraph-experiment/`, the `rustc_apfloat =0.2.3+llvm-462a31f5a5ab` oracle, evidence facts excluded, OWN-SCRATCH-01 proposed. Receipts: `verification/ngraph/README.md`.

- [ ] [serial] B1: Lower `I1`–`I4` (captures, composition, repeated inputs, a bounded `I64` transition) and `F1`–`F3` under the approved B0 option. r[NGRAPH-EXTRACT-01]
- [ ] [serial] B2: Run the differential legs L1 (ordinary Wasm/V8, `I64` only), L2 (independent BigInt and softfloat oracles, plus the B2-ORACLE `Math.fround` check), L3 (reference interpreter) and L4 (compiled evaluator) on the listed operand-order, overflow, rounding-barrier, FMA, signed-zero, nonfinite, subnormal, boundary-index and repeated-read cases. r[NGRAPH-F32-01]
- [ ] [serial] B3: Show that every false fact, bad index, effect, resource, dynamic run, recursion, malformed topology and exhausted budget control yields its D6 code before protected execution. r[NGRAPH-FACT-01] r[NGRAPH-EXTRACT-01]
- [x] [serial] B4: Check smooth-region gradients (G1 binary64 against central differences, G2 FP32 against binary64) with the stated step sizes and tolerances. Test the corner, boolean and index policies separately, and label surrogates. Evidence: `verification/ngraph/phase-b-core.json` record "B4 gradients (F1 + MSE)" and `crates/noble-ngraph-experiment/tests/b4_gradients.rs`. r[NGRAPH-DERIV-01]
- [x] [serial] B5: Train `F1` with `adam/0` and a recorded seed. Show that only parameter bytes change, that the graph identity is fixed, that old snapshots are stable and replay is bitwise, and that wrong or stale identity or evidence is refused. Evidence: `verification/ngraph/phase-b-core.json` record "B5 training F1 with adam/0" and `crates/noble-ngraph-experiment/tests/b5_training.rs`. r[NGRAPH-SNAP-01]
- [x] [serial] B6: Exercise retained snapshots after exit, both owner-release orders, and cleanup after guard failure, exhaustion and cancellation. Evidence: `verification/ngraph/phase-b-core.json` record "B6 ownership and cleanup" and `crates/noble-ngraph-experiment/tests/b6_ownership.rs`. r[NGRAPH-OWN-01]

## Phase C: measured continuation decision (approved on 2026-10-15; starts after Phase B evidence is complete)

- [ ] [serial] C1: Measure W1–W4 against the ordinary Wasm/V8 and host-native baselines, with every NGRAPH-BENCH-01 metric. r[NGRAPH-BENCH-01]
- [ ] [serial] C2: Pin source, tool, profile, oracle and hardware revisions. Report reference agreement, finite coverage, NG-PO-A..H theorem status and trusted boundaries separately. r[NGRAPH-PROOF-01]
- [ ] [serial] C3: Recommend stopping, CPU specialization or a separately scoped #2 GPU experiment under the stated (owner-adjustable) thresholds. r[NGRAPH-BENCH-01]
