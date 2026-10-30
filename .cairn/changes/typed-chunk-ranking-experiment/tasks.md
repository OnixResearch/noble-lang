## Phase 1: Design (completed here)

- [x] [serial] Ground the package in #73 TCHUNK-01..04 and current `noble-ngraph` (`lib.rs`, `ir.rs`, `grad.rs`, `compiled.rs`) and editor boundaries, including the compiled-select limitation. r[TCRANK-01] r[TCRANK-02] r[TCRANK-05]
- [x] [serial] Freeze the 480-task, five-family corpus manifest shape, provenance-block split, deduplication and leakage controls. r[TCRANK-01]
- [x] [serial] Fix the 64-lane F32 feature vector, normalization, metadata exclusions and bounded extraction accounting. r[TCRANK-01]
- [x] [serial] Specify the fixed F32 scorer graph, trainable slots, target, loss, `grad::admit` policy, `adam/0` hyperparameters, seeds, snapshots and selection rule. r[TCRANK-02]
- [x] [serial] Constrain learned ordering to the finite #73 queue and define invalid/nonfinite, stale-identity, metadata and exhaustion behavior. r[TCRANK-02] r[TCRANK-04]
- [x] [serial] Pre-register primitive-only, fixed-chunk, learned and shuffled modes, all measurements, separate training cost, repetitions and seeds. r[TCRANK-03]
- [x] [serial] Freeze numerical go/no-go thresholds, controls, acceptance mapping and non-claims before held-out evaluation. r[TCRANK-04] r[TCRANK-05]

## Phase 2: Implementation and experiment (future; all unchecked)

Hard dependency: #73 Phase 2 MUST first implement and independently admit the exact typed-chunk envelope, immutable library, one-hole stack-equation solver, deterministic shortlex queue, ordinary source/kernel re-admission, budgets, outcomes and TCHUNK-04 fixtures. No task below may substitute a new chunk library or a signature-only queue. r[TCRANK-01] r[TCRANK-02] r[TCRANK-03] r[TCRANK-04]

- [ ] [serial] Materialize and freeze the `tchunk-ranking-corpus/1` manifest with 480 tasks, five families, twelve provenance blocks per family, the 280/80/120 train/validation/test split, exact expected contracts and independent fixture values; retain its digest and reject drift. r[TCRANK-01]
- [ ] [serial] Implement canonical deduplication and leakage checks over source, normalized recipe, interface, expected trace, generator provenance and library identity; prove no held-out answer, body or evaluator state reaches features, targets or model selection. r[TCRANK-01]
- [ ] [serial] Implement the exact 64-lane finite F32 feature encoder, its normalization and metadata checks, charge extraction in the #73 shared budget, and retain feature revision/provenance. r[TCRANK-01] r[TCRANK-04]
- [ ] [serial] Build/admit the fixed 64-input, 65-parameter F32 scorer graph, fixed MSE target/loss and default exact-real gradient policy; train only on training blocks with `adam/0`, the three declared seeds, 256 steps and batch size 32, retaining immutable parameter/state snapshots and separate cost. r[TCRANK-02] r[TCRANK-03]
- [ ] [serial] Select the lowest validation-loss snapshot by the frozen exact tie-break, freeze graph/feature/library/tool identities and parameter bytes, and reject stale or mismatched scorer provenance before held-out evaluation. r[TCRANK-02] r[TCRANK-04]
- [ ] [serial] Implement primitive-only, fixed-chunk, learned-ranking and splitmix64 shuffled modes with identical #73 eligibility, contracts, effects, checker/profile, caller budgets, total caps and ordinary re-admission; keep learned control strictly to finite queue permutation. r[TCRANK-02] r[TCRANK-03]
- [ ] [serial] Implement finite descending-score ordering, original-ordinal tie-breaks and deterministic invalid/nonfinite fallback; preserve distinct Invalid, Unsupported, Exhausted and NoResult outcomes and never publish a partial proposal. r[TCRANK-02] r[TCRANK-04]
- [ ] [serial] Execute all invalid-score, adversarial-ranking, stale-identity, incorrect-metadata, exhausted-budget and equal-signature/different-behavior controls, including body/capture/dependency/revision/interface/effect and forged-digest mutations. r[TCRANK-04]
- [ ] [serial] Record per-task structural acceptance, finite fixture success, independent replay outcomes, candidate/check/expansion/dependency/instantiation/work counts, source size, retained/scratch bytes, cold/warm model load and scorer-inclusive latency. r[TCRANK-03] r[TCRANK-05]
- [ ] [serial] Report offline training time, optimizer steps, rows, memory, parameter/state bytes and model-selection cost separately from inference, and state any permitted amortization count without claiming uncharged savings. r[TCRANK-03]
- [ ] [serial] Run every mode over all 120 held-out tasks for all ten declared paired seeds, retain family-level and task-level results, paired bootstrap intervals and no-cherry-picking manifests. r[TCRANK-03]
- [ ] [serial] Apply the frozen 15% work, 10% latency, confidence, success, memory, fallback and shuffled-control thresholds, record GO or NO-GO, and leave the fixed-chunk baseline in place on NO-GO. r[TCRANK-05]
- [ ] [serial] Verify the compiled evaluator's select/step path does not claim skipped branch computation or sparse-compute savings, and retain the scope check showing no topology, backend or conditional-execution rewrite. r[TCRANK-05]
- [ ] [serial] Retain source/model/toolchain-bound evidence and explicitly separate finite behavioral success from proof, completeness, equivalence and generalization claims. r[TCRANK-03] r[TCRANK-05]
