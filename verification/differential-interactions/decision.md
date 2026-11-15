# Decision: bounded differential interaction matrix v1

**Decision:** run two separate, finite test profiles, and make no cross-profile coverage claim.

- **P1 `core-source-wasm`** extends `verification/foundation/differential.mjs`. Its frozen 18-cell matrix is quotation/composition shape (`plain`, `quoted`, `composed`) × boundary arithmetic (`min_max_wrap`, `zero_negative_one`, `small`) × output shape (`ordered_single`, `preserved_prefix_multiple`). The CLI reports successful source admission, emitted module identity and positive execution-step metrics, but has no per-operator trace. P1 therefore records the source operators and the output-discriminating execution witness separately; it does not claim direct operator tracing. The prefix/multiple-output cells additionally require at least two prefix values to remain ordered in the actual result. The existing BigInt reference and Lean-checked expectations remain independent from the source-to-Wasm observation. The current seeded generator and structured extension each receive 18 cases per optimization mode at seed 125. Per-case runtime fuel is explicitly unsupported by this CLI observer; the finite case-count budget is the frozen P1 workload bound.
- **P2 `ngraph-f32`** extends the numeric-graph experiment differential lane. Its frozen 24-cell matrix is the four selected interaction families (`guard_select`, `select_index`, `index_affine`, `guard_affine`) × six FP classes (`signed_zero`, `nan`, `subnormal_normal_boundary`, `overflow_infinity`, `cancellation`, `rounding_tie`). The L2 APFloat, binary64-per-operation, L3 reference and L4 compiled legs are retained. L3's evaluated-node count confirms the whole admitted graph ran; direct select branches and table indices are derived from the admitted input bits and graph operands. L4 has no per-node trace, so its cell witness is the selected input/output comparison, not an asserted internal trace. For the generator comparison, the current random graph stays under the structured arm's measured L3 node-work cap by reducing requested operations only as needed and spending the remaining cap on seeded vectors. P2 does not imply source-Wasm or Lean coverage.

The exact cell axes, witness rules, supported legs, unsupported runtime-fuel/parameter-snapshot features, outcome classes, compute budgets and kill rule are frozen in [`matrix-v1.json`](matrix-v1.json). Do not amend the matrix after seeing campaign results; a future expansion requires a new version and decision.

## Evidence boundary

This is empirical test qualification on finite cases, not a theorem about Noble's implementations. A cell is reached only by a runtime witness for the required operations/branch and an engaged property, never by a template label or an unexecuted node. Refused, exhausted and unsupported cases remain separate from accepted executions; an all-refused run cannot satisfy semantic acceptance. Mutant outcomes distinguish killed, divergence-unreached, executed-survived, invalid/equivalent and incomplete. Only an intended property failure on the same case and execution that reached the injected divergence is a kill.

A deliberate oracle weakness must be exposed by an independent control. Shrinking must retain the complete witness and recheck admission, interaction, expectation and mismatch identity for every candidate; stopping at the bound is not a proof of global minimality. Original and minimized failures are replayed from retained bytes in fresh processes. Changed candidate bytes require newly bound expectations and a fresh semantic execution; historical receipts remain historical.

## Ownership and reopen conditions

- #38 retains ownership of the pinned F32 reference and the promotion/correspondence boundary; this lane reuses its observation relation and fixtures without promoting the remaining claim.
- #42 retains certificate endpoint and malformed-certificate checks. This lane does not modify or claim them.
- #64 retains optimization quality and full resource/cost accounting. No resource equivalence follows from this matrix; any future such statement must use the existing explicit sufficient-budget relation.
- #87/#102 retain the source/WIT fuzz setup and exact-text export path; the extension reuses rather than replaces them.

Reopen this decision only with a versioned matrix change justified by an identified unsupported feature or a demonstrated mismatch between the selected matrix and the current supported profile. Keep new profiles and unsupported axes separate; do not relax witness or kill criteria to obtain a positive result.
