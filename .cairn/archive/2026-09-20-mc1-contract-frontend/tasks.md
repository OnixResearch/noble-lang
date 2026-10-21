## Phase 1: Contract selection

- [x] [serial] Select experimental grammar, typed IR, companion representation, finite rule encoding, failure/budget and API boundaries r[VC-GATE-01].
- [x] [serial] Validate proposal/design/task deltas and retain scope excluding MC2 runtime delivery (VC-SCOPE-02). Verify: all three package gates report PASS. r[VC-GATE-01].

## Phase 2: Implementation

- [x] [parallel] Implement optional declarations linked to actual accepted programs without changing ordinary Program typing (VC-SCOPE-02). Verify: invalid optional predicates retain ordinary acceptance. r[VC-GATE-01].
- [x] [parallel] Resolve ordered input/output stacks, snapshots, logical definitions, parameters and claim kind into typed IR (VC-INPUT-01). Verify: stale/unbound references and initial/final confusion reject. r[VC-GATE-01].
- [x] [parallel] Implement scalar/structural predicates, Boolean connectives, quantified parameters and explicit invalid/unsupported diagnostics (VC-INPUT-02). Verify: representative predicates elaborate and malformed variants fail with spans. r[VC-GATE-01].
- [x] [parallel] Enforce erased logical data isolation and prohibit resources/host operations/tactics in preparation (VC-INPUT-04). Verify: ghost runtime references and resource declarations reject without proof execution. r[VC-GATE-01].
- [x] [serial] Export exact accepted subject and typed claim into Lean with wrapping arithmetic and normal-outcome semantics (VC-INPUT-03). Verify: changed body, postcondition, precondition or arithmetic cannot reuse the original theorem. r[VC-GATE-01].
- [x] [parallel] Prove primitive, sequencing, quotation, invocation, structural/branch and quantified family rules (VC-LIB-01). Verify: strict Lean theorem inventory contains no disallowed axioms. r[VC-GATE-01].
- [x] [serial] Require explicit intermediate implication, interface/context compatibility and preserved premises for composition (VC-LIB-02). Verify: theorem premise is necessary and the missing-implication example cannot certify composition. r[VC-GATE-01].
- [x] [parallel] Implement verification/explanation CLI with seven outcomes, exact theorem type and strict assumption inspection r[VC-GATE-01].

## Phase 3: Acceptance

- [x] [serial] Extract actual frontend/exporter and connect the named PO-15/19 statement-export fragment to the reviewed model (VC-INPUT-03). Verify: generated functions occur in compiled correspondence theorem types. r[VC-GATE-01].
- [x] [serial] Retain contract IR, emitted propositions, rule revision and exact theorem evidence separately from Rust refinement (VT-CONTRACT-01). Verify: application proof and exporter refinement statuses are independent. r[VC-GATE-01].
- [x] [serial] Execute actual increment/wrap/composition/family proof paths and invalid/type/unsupported/statement-mismatch/refutation controls (VC-LIB-01). Verify: actual CLI reports expected positive and negative outcomes. r[VC-GATE-01].
- [x] [serial] Run affected regression, quality and specification gates and update exact evidence r[VC-GATE-01].

## Lifecycle closeout

After the implementation checklist passes, inspect and execute the spec sync and archive plans, then run post-archive validation. Archive follows the checklist rather than being its own prerequisite.

The [MC1 evidence](../../../verification/mc1/evidence.json), [CLI matrix](../../../verification/mc1/acceptance.json), and [independent implementation receipt](../../../verification/mc1/implementation.json) retain exact subjects and trust boundaries. The strict rule/projection lanes and the six native-evaluated source equations remain separate; MC2, runtime execution, universal frontend/exporter correctness, and the broad PO-15/PO-19 obligations are not closed.
