## Phase 1: Research design

- [x] [serial] Select the Mathlib-free structured-emission route, enumerate the first cut and excluded proof edges, and record the Talos pilot's actual limits. Decision 56; baseline `95626e2b`, `verification/wasm-refinement-pilot/README.md`, `NobleContracts.Rules`, and `crates/noble-wasm/runtime/operations.wat`. r[WREF-STEP-01] r[WREF-EMIT-01] r[WREF-CHAIN-01]

## Phase 2: Proof and source custody

- [ ] [serial] Mechanize the Wasm steps for the selected body and reached helpers; prove preserved tails, ordered arithmetic, quota/fault separation and non-vacuity under the strict axiom policy. r[WREF-STEP-01]
- [ ] [serial] Build and extract actual structured emitter code, and prove its checked bounded outputs simulate `Exec` and preserve the reflected recipe with a complete premise inventory. r[WREF-EMIT-01]
- [ ] [serial] Run source and subject binding, independent positive and mismatch controls, and retain the machine receipt and axiom audit. r[WREF-STEP-01] r[WREF-EMIT-01]
- [ ] [serial] Review go/no-go and the encoding, loading and composition gaps before any sync or archive. Keep PO-17/18 and dependent obligations open without applicable correspondence. r[WREF-CHAIN-01]
