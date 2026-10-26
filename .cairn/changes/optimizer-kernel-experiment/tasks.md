## Phase 1: Design

- [x] [serial] Publish the minimal typed candidate IR, shape/state interface, operation whitelist, budgets and explicit unsupported diagnostics. r[NGRAPH-OPT-01]
- [x] [serial] Resolve the numeric mismatch: select the `F32`-only SGD-like reference update first and record binary64 candidates as a distinct algorithm. r[NGRAPH-OPT-02]
- [x] [serial] Specify admission outcomes and the refusal classes, keeping Lean model theorems, checker obligations and finite Rust comparisons separate. r[NGRAPH-OPT-03]
- [x] [serial] Specify immutable snapshot publication binding and host-owned whole-trial rollback with fail-closed checkpoints and append-only attempts. r[NGRAPH-OPT-04]
- [x] [serial] Specify evaluation isolation, tampering controls and the preregistration discipline for any future performance comparison. r[NGRAPH-OPT-05]

## Phase 2: Reference experiment (future, not started)

- [ ] [serial] Implement the bounded candidate envelope and admission that refuses malformed, effectful, over-budget, shape-mismatched, aliasing, nonfinite and invalid-state-transition candidates. r[NGRAPH-OPT-01] r[NGRAPH-OPT-03]
- [ ] [serial] Provide Lean kernel-checked semantics for the selected `F32` reference update, reusing #38, with a named theorem inventory and a version-aware per-theorem axiom audit. r[NGRAPH-OPT-02]
- [ ] [serial] Add the fixed simple reference update plus the boundary/nonfinite and state-transition controls as finite Rust comparisons labeled implementation evidence. r[NGRAPH-OPT-02] r[NGRAPH-OPT-03]
- [ ] [serial] Publish fresh immutable parameter and optimizer-state snapshots and prove the parent snapshot byte-stability, binding optimizer code, parent identities, graph/loss/derivative identities, seed, data order, numeric semantics and tool revisions. r[NGRAPH-OPT-04]
- [ ] [serial] Implement host-owned whole-trial rollback with early and later fault injection, an unavailable-checkpoint refusal and append-only attempt records. r[NGRAPH-OPT-04]
- [ ] [serial] Implement evaluation isolation with tampering controls and record that proposer authority cannot reach held-out data, evaluator internals or secrets. r[NGRAPH-OPT-05]
- [ ] [serial] Preregister the tiny CPU comparison over at least three paired seeds with equal total budgets and report a stop/continue recommendation, kept separate from semantic validation. r[NGRAPH-OPT-05]
- [ ] [serial] Retain source/model/toolchain-bound receipts and leave every unproved compiler/executed-artifact correspondence explicit. r[NGRAPH-OPT-03]
