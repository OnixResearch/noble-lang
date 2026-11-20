## Why

Noble's `I64` graphs already have a reference evaluator and a packed-register
implementation, but no independently checked sequential belt representation.
Issue #113 selects a small, target-neutral experiment: lower only total
`I64`/`Bool` graphs into age-addressed instructions and explicitly preserve
values that remain live after eviction.

r[NGRAPH-BELT-01] r[NGRAPH-BELT-02] r[NGRAPH-BELT-03] r[NGRAPH-BELT-04]

## What Changes

- **ADDED** `NGRAPH-BELT-01`: the admitted graph fragment and fixed resource
  bounds for an opt-in belt artifact.
- **ADDED** `NGRAPH-BELT-02`: newest-first age semantics, wrapping I64
  operations, explicit save/reload, and final ordered output reads.
- **ADDED** `NGRAPH-BELT-03`: canonical encoding, graph/revision/content
  binding, independent checking, and checked execution requirements.
- **ADDED** `NGRAPH-BELT-04`: separation of model proof, Rust correspondence,
  finite differential evidence, and cost measurements.

This package records the contract for the source implementation in
`crates/noble-ngraph/src/belt.rs`. It does not select an ISA, schedule, hardware
model, or production backend. #114's target-specialized schedules are described
separately in `belt-target-schedules`.

## Impact

- New source subject: `noble-ngraph::belt`, an opt-in experimental module.
- Affected native spec: `backend-experiments` (ADDED deltas only).
- Existing reference, `Compiled`, extraction, and ordinary program behavior stay
  unchanged. Guard/fact, F32, parameter, and unsupported graphs are refused.
- Lean soundness, finite Rust tests, extracted-source B2 comparisons, and cost
  reports remain separate claims; no ISA, Wasm, or hardware result is implied by
  this package.
