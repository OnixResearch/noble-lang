## Phase 1: Design

- [x] [serial] Declare the two-child bound, the host-issued scope identity, retained ordered membership, attachment only through the scope's own move-only admitted value, and refusal of duplicate, foreign, stale, post-close and fabricated membership. r[RA-GROUP-01]
- [x] [serial] Declare incremental admission forced by the exclusive `Admission` borrow, preservation of a committed first child and of caller inputs on refusal, no rollback claim, and once-per-child capacity accounting. r[RA-GROUP-02]
- [x] [serial] Declare attachment-ordered results, delivery-gated reporting, the sticky first abnormal outcome, cancellation acknowledgement kept apart from result and settlement, and exactly-once membership release. r[RA-GROUP-03]
- [x] [serial] Declare per-undelivered-child retirement with native request, native-stop observation, pin settlement and cleanup separate, an unsettled child keeping the scope unsettled, and retirement inherited by the host registry. r[RA-GROUP-04]
- [x] [serial] Declare close requiring every member to finalize and accounting to settle, idempotent repeated close, and no manufactured completion. r[RA-GROUP-05]
- [x] [serial] Declare the finite two-child reference model over #46, the adapter comparison against the real M6 table API, and the separate labelling of scope policy, transition composition and adapter correspondence. r[RA-GROUP-06]

## Phase 2: Foundation model

- [x] [serial] Add `proofs/foundation/NobleFoundation/TaskScope.lean` importing `TaskInterleaving`, register it in `NobleFoundation.lean`, `inventory.json` and the README, and prove the membership, result-order, refusal, cancellation, settlement and capacity obligations at the declared bound. r[RA-GROUP-01] r[RA-GROUP-03] r[RA-GROUP-04] r[RA-GROUP-02]

## Phase 3: Adapter pilot

- [x] [serial] Add the host scope adapter and its reference traces to the existing `m6-interleaving` target by `#[path]`, replay the model traces over the real table, compare results, refusals, accounting deltas and `observation()`, and check input ownership through cleanup. r[RA-GROUP-01] r[RA-GROUP-02] r[RA-GROUP-03] r[RA-GROUP-04] r[RA-GROUP-05]
- [x] [serial] Emit the generated scope corpus from the adapter and check the committed file matches the emission, so the model's declared traces cannot drift from the adapter's. r[RA-GROUP-06]

## Phase 4: Evidence

- [ ] [serial] Retain `verification/task-scope/` with a README and a `run.mjs` receipt binding the source commit, change revision, foundation module and declared bounds, keeping model proofs, runtime tests and host assumptions separate. r[RA-GROUP-06]
- [ ] [serial] Rerun the existing M6 lifecycle and single-invocation `ExecutionScope` regressions and record them in the receipt. r[RA-GROUP-05]
