# Issue #82 decision record

## Design choice

Use the Talos interpreter pinned at `d7c84ac660757953b752bdfc9cb8dae3dab623ed` as an isolated,
verification-only Lake project under `verification/wasm-refinement-pilot/lean/`. Keep it outside
`proofs/`; do not import it into foundation proofs or production crates. Reuse only foundation
package caches whose manifest revisions match Talos; do not download or rebuild Mathlib. Keep
`wasm-tools` external and policy-pinned, and bind its printed WAT to the retained module with an
exact byte round trip. Refuse any module that `decodeForVerification` does not accept.

The alternative is a new in-Lean binary decoder paired with a Talos-style semantics. That is not
selected for this bounded pilot: it adds a second decoder implementation and changes the trust
question. Stopping at the inventory gap remains the conservative outcome if the adopted decoder
refuses the subject or the evidence cannot support the theorem.

## Frozen acceptance and trust criteria

A fixture is not semantically validated unless a kernel-checked theorem binds its exact module bytes,
execution, and source-side `NobleContracts.Exec` result for every admitted input. Normal return,
trap, and quota/fuel exhaustion must be distinct outcomes. The proof's transitive axioms must be a
subset of `{propext, Classical.choice, Quot.sound}` and must not use `native_decide`. A failed or
timed-out proof is inconclusive, never a miscompilation finding. Stale artifact bindings must be
refused. A correctly hashed wrong artifact must have a concrete, kernel-checked counterexample.

Talos's correspondence with the WebAssembly Core specification, the correctness of external
`wasm-tools` and Node/V8, and the initial imported memory/table/global store remain trusted unless a
separate retained proof discharges them. A digest establishes byte identity, not source
correspondence. The finite evidence applies only to the exact artifacts and test cases named in the
receipts; it does not verify the compiler or establish general Wasm conformance.

## Go/no-go

**NO-GO** for semantic acceptance of either the retained 10,474-byte artifact or the new dynamic
`x y *` artifact, and **NO-GO** for replacing TB+ trusted rebuild. Reopen only after the complete
digest-bound refinement theorem, trap/exhaustion separation, both artifact controls, pinned
Talos/V8 differential evidence, and the pinned official spec tests all pass. The measured proof effort
must be compared per fixture against the low fixed rebuild cost and remaining trust assumptions of
TB+; until then TB+ remains the usable admission route.

The evidence and per-unit status are recorded in `README.md` and the committed receipts under
`verification/wasm-refinement-pilot/receipts/`.
