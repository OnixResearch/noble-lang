## Phase 1: Selected model and kernel-checked expectations

- [x] [serial] Select one explicit kernel-reducible binary32 model with a documented trusted base, instead of the opaque Lean 4.31.0 `Float32` or an unaudited pin change: raw encodings, classification, scaled exact interpretation, round-to-nearest/ties-to-even, gradual underflow, and add/subtract/multiply with signed-zero, infinity, NaN and overflow rules. r[NGV-F32-01]
- [x] [serial] State the proved model properties as named theorems and kernel-check all 26 retained expectations proposed by the external tool: ties in both directions, cancellation, both zero signs, subnormals, the normal boundary, underflow, overflow in both signs, infinities and NaNs. r[NGV-F32-02]
- [x] [serial] Establish that round-to-real reasoning cannot discharge binary32, and that reassociation and implicit contraction are observable. r[NGV-F32-01]
- [x] [serial] Retain the clean-source checker run with the Lean and Mathlib pins, the inventoried statements and the axiom audit, and document the claim/non-claim split in `proofs/foundation/README.md`. r[NGV-F32-03]
- [x] [serial] Run the foundation gates: `lake build NobleFoundation` with no warnings, `verification/foundation/check.mjs` on clean source, `verification/binary32/propose.py`, and Cairn/check-specs validation. r[NGV-F32-03]

## Phase 2: Correspondence and the rest of the slice (not selected)

- [ ] [serial] Add a wrong-rounding, signed-zero-collapse, NaN-policy, operand-reversal and stale-model mutation lane with bounded shrinking and replayable witnesses, and retain which retained expectations are proved versus externally tested. r[NGV-F32-02]
- [ ] [serial] Replay the checked expectations through the packed Rust evaluator and the graph reference, retaining APFloat only as a secondary lane, and record the target/FP configuration. r[NGV-F32-03]
- [ ] [serial] Prove or explicitly classify the ordered affine fold's per-operation rounding, extraction, training, snapshot and codec layers, guard/index rules and software helpers, and the optimizer slice. r[NGV-F32-01]
- [ ] [serial] Decide whether a correspondence claim to `crates/noble-ngraph/src/sem.rs` is provable or stays a reviewed trusted assumption, and record it separately from the model theorems. r[NGV-F32-03]

## Phase 3: Promotion (not selected)

- [ ] [serial] Sync the established verification requirements, register any conformance cases with their real states, and update the affected numeric-graph obligations only for the slice actually established. r[NGV-F32-03]
