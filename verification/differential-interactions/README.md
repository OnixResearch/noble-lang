# Differential interaction matrix v1

This lane adds bounded feature-interaction cases to the existing source/Wasm differential runner and F32 graph experiment; it does not replace either harness. `matrix-v1.json` freezes the two profiles, selected cells, witness rules, supported comparison legs, budgets, and unsupported axes. `decision.md` records the rationale and reopen conditions. Do not merge P1 and P2 coverage or use the P2 graph results as source-to-Wasm or Lean evidence.

## Reproduction

Run from a clean, committed revision. First produce a fresh foundation-check receipt for that revision, then export the existing minimized source corpus with `verification/fuzz/export-differential.mjs`. The P1 baseline command is:

```sh
<selected-node> verification/foundation/differential.mjs \
  --seed 125 --count 18 \
  --foundation <foundation-check-receipt.json> \
  --output <new-directory-outside-the-repository> \
  --import <exported-source-cases.json> \
  --target-dir <one-external-target-directory> \
  --skip-checker-lanes
```

The P2 baseline is the `issue125_structured_interaction_matrix` test in the existing `b2_differential` target:

```sh
cargo test --locked --offline -p noble-ngraph-experiment \
  --test b2_differential issue125_structured_interaction_matrix -- --nocapture
```

and replay results. See the runner's usage comment for the required options.
The successful runs for source commits `7ed66b5d` and `675ab62f` are retained
in their respective receipt directories, but attest only those revisions. The
latest run for source commit `c9fa57d` is retained in
[`receipts/c9fa57d/campaign.json`](receipts/c9fa57d/campaign.json): it reached
18/18 P1 and 24/24 P2 cells, killed and replayed all eight selected mutants,
and refused historical expectations or executables for changed builds.

## Evidence boundary and ownership

All results are finite test evidence, not a theorem about Noble implementations. A cell counts as reached only when its required interaction is observed in an admitted execution and its declared property is evaluated; a generated template, unexecuted node, panic, or unrelated failure does not count. Reachability is separate from whether the property passed or all comparison legs agreed: those outcomes and mismatch classes remain explicit per cell. Refusal, exhaustion, and unsupported outcomes are reported separately, and an all-refused campaign cannot pass semantic acceptance. A mutant kill requires the intended property to fail on the same retained case and execution that reached the injected divergence, with the clean baseline target independently satisfying the property. Shrinking is bounded and does not establish a global minimum.

Each P2 matrix cell and fresh-process replay retain the selected execution limits: `/0` default graph admission, `Control::default()` (no fuel, cancellation callback, scratch instrumentation, or forward-scratch override), the effective default forward-scratch budget, no parameter snapshot, one vector per cell, and measured L3 node work against the frozen matrix work cap. These execution receipts are source-bound finite evidence, not resource-equivalence claims.

The P2 lane reuses #38's F32 observation relation and fixtures; it does not alter the pinned reference or promote the remaining correspondence claim. Certificate before/after endpoint and malformed-certificate checks remain owned by #42. Optimization quality and full resource/cost accounting remain owned by #64; this matrix makes no resource-equivalence claim.
