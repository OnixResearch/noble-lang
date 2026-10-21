## Phase 1: Selection

- [x] [serial] Map complete M3 scope, workload, negative controls and non-claims (BE-COMPARE-01). r[BE-CONFIG-01].
- [x] [serial] Select locked tools and execute an actual GC capture/optimization probe. Verify: pinned Node executes immutable GC captures before and after Binaryen optimization. r[BE-CONFIG-01].
- [x] [serial] Validate proposal, design and task gates before implementation. r[BE-CONFIG-01].

## Phase 2: Implementation

- [x] [parallel] Implement pure checked lowering and emitted compiled continuation functions (BE-DYNAMIC-01). Verify: no source/recipe interpreter or runtime compiler import. r[BE-CONFIG-01].
- [x] [parallel] Implement actual WasmGC immutable graph storage and region cleanup (BE-COMPARE-01). Verify: graph children are GC references, not a disguised linear heap. r[BE-CONFIG-01].
- [x] [parallel] Implement managed-linear-memory graph storage and checked growth (BE-COMPARE-01). Verify: failed allocation never publishes a cell. r[BE-CONFIG-01].
- [x] [serial] Preserve complete monomorphic interfaces and reject extra owner-slot adaptation before emission (BE-ARITY-01). r[BE-CONFIG-01].
- [x] [serial] Preserve exact semantic recipes through collections, returned aggregates and reflection-only reachability under optimization (BE-REFLECT-01). r[BE-CONFIG-01].
- [x] [serial] Implement construction/invocation quotas, tree depth and accounted cleanup without counting quota as success (BE-LIMIT-01). r[BE-CONFIG-01].
- [x] [parallel] Integrate reproducible host measurements, binary section accounting and explicit unknown metrics (BE-REPORT-01). r[BE-CONFIG-01] r[BE-REPORT-01].
- [x] [parallel] Extend existing Nix, policy, collection, extraction and source inventory lanes without target removal. r[BE-CONFIG-01].

## Phase 3: Acceptance

- [x] [serial] Execute CORE-03 captures, CORE-05 program-list selection and CORE-09 returned-program reflection with compiler service absent (BE-DYNAMIC-01, BE-REFLECT-01). r[BE-CONFIG-01].
- [x] [serial] Execute ADAPT-08 complete-interface rejection and ADAPT-09 left/right/balanced 255/256/257-leaf matrix plus exact allocation/depth/execution boundaries (BE-ARITY-01, BE-LIMIT-01). r[BE-CONFIG-01].
- [x] [serial] Execute ADAPT-10 optimized collection/return/reflection-only paths and ADAPT-12 selection-policy controls (BE-REFLECT-01, BE-REPORT-01). r[BE-CONFIG-01] r[BE-REPORT-01].
- [x] [serial] Record complete repeated measurements, failed cases, cleanup evidence, tool/source hashes and scoped representation selection (BE-COMPARE-01, BE-REPORT-01). r[BE-CONFIG-01] r[BE-REPORT-01].
- [x] [serial] Run affected regression, strict quality, extraction, inventory and specification gates; update conformance/roadmap evidence while preserving M4/M5 non-claims. r[BE-CONFIG-01] r[BE-REPORT-01].

## Lifecycle closeout

After every implementation and acceptance task passes, inspect and execute specification sync and archive, using the actual archive date, then run post-archive validation. Archive follows the checklist and is not its own prerequisite.
