## Why

PAA-1 compares an artifact with a selected trusted build. This establishes byte
identity, not the meaning of those bytes. O1, O3 and O7 in
[artifact-correspondence-extension](../artifact-correspondence-extension/design.md) remain
open. The [Wasm refinement pilot](../../../verification/wasm-refinement-pilot/README.md)
ran differential controls but did not prove that artifact execution refines MC1 `Exec`.

## What Changes

- Model the emitted linear-memory arithmetic subset with Mathlib-free Wasm steps,
  separating accepted returns from profile exhaustion and faults. r[WREF-STEP-01]
- Relate the actual extracted emitter to that model at a structured instruction
  boundary. Account for stack values, the preserved tail, and the recipe. r[WREF-EMIT-01]
- Keep encoding, loading and composition separate from structured lowering.
  Do not select a new backend or change release acceptance. r[WREF-CHAIN-01]

## Impact

- **Files:** a Mathlib-free Lean model and strict gate, scoped emitter/extraction
  changes, and source-bound verification for the selected cut.
- **Testing:** exact dependency and axiom audits, extracted-subject proofs,
  compiled differential controls and independently mismatched body/recipe inputs.
