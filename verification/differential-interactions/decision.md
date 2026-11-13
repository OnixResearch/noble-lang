# Decision: bounded differential interaction matrix v1

**Decision:** run two separate, finite test profiles, and make no cross-profile coverage claim.

- **P1 `core-source-wasm`** extends `verification/foundation/differential.mjs`. Its frozen 18-cell matrix is quotation/composition shape (`plain`, `quoted`, `composed`) × boundary arithmetic (`min_max_wrap`, `zero_negative_one`, `small`) × output shape (`ordered_single`, `preserved_prefix_multiple`). A P1 cell is reached only when an admitted execution performs the intended arithmetic and the relevant quotation/composition path; the prefix/multiple-output cells additionally require at least two prefix values to remain ordered in the actual result. The existing BigInt reference and Lean-checked expectations remain independent from the source-to-Wasm observation.
- **P2 `ngraph-f32`** extends the numeric-graph experiment differential lane. Its frozen 24-cell matrix is the four selected interaction families (`guard_select`, `select_index`, `index_affine`, `guard_affine`) × six FP classes (`signed_zero`, `nan`, `subnormal_normal_boundary`, `overflow_infinity`, `cancellation`, `rounding_tie`). The L2 APFloat, binary64-per-operation, L3 reference and L4 compiled legs are retained. P2 does not imply source-Wasm or Lean coverage.

The exact cell axes, witness rules, supported legs, unsupported parameter-snapshot feature, outcome classes and kill rule are frozen in [`matrix-v1.json`](matrix-v1.json). Do not amend the matrix after seeing campaign results; a future expansion requires a new version and decision.

## Evidence boundary

This is empirical test qualification on finite cases, not a theorem about Noble's implementations. A cell is reached only by a runtime witness for the required operations/branch and an engaged property, never by a template label or an unexecuted node. Refused, exhausted and unsupported cases remain separate from accepted executions; an all-refused run cannot satisfy semantic acceptance. Mutant outcomes distinguish killed, divergence-unreached, executed-survived, invalid/equivalent and incomplete. Only an intended property failure on the same case and execution that reached the injected divergence is a kill.

A deliberate oracle weakness must be exposed by an independent control. Shrinking must retain the complete witness and recheck admission, interaction, expectation and mismatch identity for every candidate; stopping at the bound is not a proof of global minimality. Original and minimized failures are replayed from retained bytes in fresh processes. Changed candidate bytes require newly bound expectations and a fresh semantic execution; historical receipts remain historical.

## Ownership and reopen conditions

- #38 retains ownership of the pinned F32 reference and the promotion/correspondence boundary; this lane reuses its observation relation and fixtures without promoting the remaining claim.
- #42 retains certificate endpoint and malformed-certificate checks. This lane does not modify or claim them.
- #64 retains optimization quality and full resource/cost accounting. No resource equivalence follows from this matrix; any future such statement must use the existing explicit sufficient-budget relation.
- #87/#102 retain the source/WIT fuzz setup and exact-text export path; the extension reuses rather than replaces them.

Reopen this decision only with a versioned matrix change justified by an identified unsupported feature or a demonstrated mismatch between the selected matrix and the current supported profile. Keep new profiles and unsupported axes separate; do not relax witness or kill criteria to obtain a positive result.
