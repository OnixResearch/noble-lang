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
in their respective receipt directories, but attest only those revisions.
The latest run for source commit `756ccc9` is retained in
[`receipts/756ccc9/campaign.json`](receipts/756ccc9/campaign.json): it reached
18/18 P1 and 24/24 P2 cells, killed and replayed all eight selected mutants,
refused historical expectations or executables for changed builds, and restored
the single mutation worktree cleanly.

## Optional Plausible P1 test arm

The Lean-community follow-up uses a separate executable driver under
`plausible/`, not a proof tactic in any accepted foundation theorem. Its
four-cell subset and matched 16-case-per-arm budget are frozen in
`plausible/matrix-v1.json`. The typed generator preserves cell-indexed
premises; the existing foundation differential path checks each generated
source against its independent BigInt reference, Lean-kernel-checked
expectation and both Wasm optimization modes. For the clean source commit
`fb63cb9`, the generated arm reached 4/4 selected cells, versus 0/4 for the
current seeded generator in those same cells. The measured decision in
`plausible/decision.md` adopts the optional stratified arm only for this
subset; it does not replace the current general generator. The retained
receipt, raw source and proof-gate negative control are indexed at
`plausible/receipts/fb63cb9/index.json`. The new generated inputs detected
test-side simulations of all four P1 bank faults. The real implementation
mutants were measured by the earlier, independently source-bound batch-5
campaign, not rerun by this pilot.

From a clean commit with populated pinned Lean packages, run:

```sh
verification/ci/lean-proofs.sh <new-proof-output-directory>
<selected-node> verification/differential-interactions/plausible/run.mjs \
  --foundation <new-proof-output-directory>/foundation/foundation-check.json \
  --output <new-pilot-output-directory> --target-dir <external-cargo-target>
<selected-node> verification/differential-interactions/plausible/run.mjs \
  --replay <new-pilot-output-directory>/pilot.json \
  --output <new-replay-output-directory> --target-dir <external-cargo-target>
```

Replay is bound to the exact clean source tree and built executable recorded
in the pilot receipt; to replay the retained historical receipt, check out
its recorded source commit. Passing generated cases and printed tactic
counterexamples remain finite test evidence, not proofs.

## Evidence boundary and ownership

All results are finite test evidence, not a theorem about Noble implementations. A cell counts as reached only when its required interaction is observed in an admitted execution and its declared property is evaluated; a generated template, unexecuted node, panic, or unrelated failure does not count. Reachability is separate from whether the property passed or all comparison legs agreed: those outcomes and mismatch classes remain explicit per cell. Refusal, exhaustion, and unsupported outcomes are reported separately, and an all-refused campaign cannot pass semantic acceptance. A mutant kill requires the intended property to fail on the same retained case and execution that reached the injected divergence, with the clean baseline target independently satisfying the property. Shrinking is bounded and does not establish a global minimum.

Each P2 matrix cell and fresh-process replay retain the selected execution limits: `/0` default graph admission, `Control::default()` (no fuel, cancellation callback, scratch instrumentation, or forward-scratch override), the effective default forward-scratch budget, no parameter snapshot, one vector per cell, and measured L3 node work against the frozen matrix work cap. These execution receipts are source-bound finite evidence, not resource-equivalence claims.

The P2 lane reuses #38's F32 observation relation and fixtures; it does not alter the pinned reference or promote the remaining correspondence claim. Certificate before/after endpoint and malformed-certificate checks remain owned by #42. Optimization quality and full resource/cost accounting remain owned by #64; this matrix makes no resource-equivalence claim.
