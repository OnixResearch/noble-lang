## Phase 1: Design

- [x] [serial] Publish the minimal typed candidate IR, shape/state interface, operation whitelist, budgets and explicit unsupported diagnostics. r[NGRAPH-OPT-01]
- [x] [serial] Resolve the numeric mismatch: select the `F32`-only SGD-like reference update first and record binary64 candidates as a distinct algorithm. r[NGRAPH-OPT-02]
- [x] [serial] Specify admission outcomes and the refusal classes, keeping Lean model theorems, checker obligations and finite Rust comparisons separate. r[NGRAPH-OPT-03]
  - Note (2026-10-29): the concrete IR, exact whitelist, budgets and refusal envelope are now published in `design.md` under “Minimal candidate IR” and in the NGRAPH-OPT-01/02/03 scenarios in `specs/backend-experiments/spec.md`.
- [x] [serial] Specify immutable snapshot publication binding and host-owned whole-trial rollback with fail-closed checkpoints and append-only attempts. r[NGRAPH-OPT-04]
- [x] [serial] Specify evaluation isolation, tampering controls and the preregistration discipline for any future performance comparison. r[NGRAPH-OPT-05]

## Phase 2: Reference experiment

- [x] [serial] Implement the bounded candidate envelope and admission that refuses malformed, effectful, over-budget, shape-mismatched, aliasing, nonfinite and invalid-state-transition candidates. r[NGRAPH-OPT-01] r[NGRAPH-OPT-03]
- Note (2026-11-13): `noble-ngraph-experiment/src/opt/` strictly parses `ngraph-opt/1`, rejects duplicate JSON keys and unsupported capabilities, measures its fixed budgets, and evaluates admitted programs with `noble_ngraph::sem::{mul,sub}`. `tests/support/opt_kernel/corpus.rs` emits the Lean comparison corpus; `tests/support/opt_kernel.rs` (registered in the existing `b3_rejections` target) covers refusal locations and finite behavior.
- [x] [serial] Provide Lean kernel-checked semantics for the selected `F32` reference update, reusing #38, with a named theorem inventory and a version-aware per-theorem axiom audit. r[NGRAPH-OPT-02]
  Complete at model level (2026-10-29): `proofs/foundation/NobleFoundation/SgdReference.lean` defines
  the two-rounding update over the #38 `Binary32` model and the published admission rule, with
  universal shape, per-lane and nonfinite-refusal theorems, kernel-checked boundary expectations and
  a fused-multiply-add control. Its theorems are in the foundation inventory, and the clean-source
  foundation receipt (Lean 4.31.0, per-theorem `#print axioms`) is retained with it. The class
  precedence among refusals is the model's reading, not a published contract.
- [x] [serial] Add the fixed simple reference update plus the boundary/nonfinite and state-transition controls as finite Rust comparisons labeled implementation evidence. r[NGRAPH-OPT-02] r[NGRAPH-OPT-03]
- [x] [serial] Publish fresh immutable parameter and optimizer-state snapshots and prove the parent snapshot byte-stability, binding optimizer code, parent identities, graph/loss/derivative identities, seed, data order, numeric semantics and tool revisions. r[NGRAPH-OPT-04]
- [x] [serial] Implement host-owned whole-trial rollback with early and later fault injection, an unavailable-checkpoint refusal and append-only attempt records. r[NGRAPH-OPT-04]
- [x] [serial] Implement evaluation isolation with tampering controls and record that proposer authority cannot reach held-out data, evaluator internals or secrets. r[NGRAPH-OPT-05]
- [x] [serial] Preregister the conditional tiny CPU comparison over at least three paired seeds and report a stop/continue recommendation, kept separate from semantic validation. r[NGRAPH-OPT-05]
  - Note (2026-11-13): feasibility did not pass; the conditional comparison was not activated. `verification/opt-kernel/decision.md` freezes the paired seeds and equal work/wall-clock budget accounting, records STOP for `ngraph-opt/1`, and defines the separately versioned `trace-monomial/1` reopen route. No timing result is claimed.
- [ ] [serial] Retain source/model/toolchain-bound receipts and leave every unproved compiler/executed-artifact correspondence explicit. r[NGRAPH-OPT-03]
