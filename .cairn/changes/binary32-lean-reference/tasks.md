## Phase 1: Selected model and kernel-checked expectations

- [x] [serial] Select one explicit kernel-reducible binary32 model with a documented trusted base, instead of the opaque Lean 4.31.0 `Float32` or an unaudited pin change: raw encodings, classification, scaled exact interpretation, round-to-nearest/ties-to-even, gradual underflow, and add/subtract/multiply with signed-zero, infinity, NaN and overflow rules. r[NGV-F32-01]
- [x] [serial] State the proved model properties as named theorems and kernel-check all 26 retained expectations proposed by the external tool: ties in both directions, cancellation, both zero signs, subnormals, the normal boundary, underflow, overflow in both signs, infinities and NaNs. r[NGV-F32-02]
- [x] [serial] Establish that round-to-real reasoning cannot discharge binary32, and that reassociation and implicit contraction are observable. r[NGV-F32-01]
- [x] [serial] Retain the clean-source checker run with the Lean and Mathlib pins, the inventoried statements and the axiom audit, and document the claim/non-claim split in `proofs/foundation/README.md`. r[NGV-F32-03]
- [x] [serial] Run the foundation gates: `lake build NobleFoundation` with no warnings, `verification/foundation/check.mjs` on clean source, `verification/binary32/propose.py`, and Cairn/check-specs validation. r[NGV-F32-03]

## Phase 2: Correspondence and the rest of the slice (not selected)

- [x] [serial] Add the test-local mutation lane in `NobleFoundation/Binary32Mutations.lean`: an away-from-zero tie rule, collapsing every zero result to `+0`, flush-to-zero, a zero-sign-insensitive observation, a commutative subtraction and implicit contraction. Each variant is caught by a named retained expectation and `all_declared_mutations_are_caught` states that none escapes; the receipt is `verification/foundation/receipts/aa074e1/`. r[NGV-F32-02]
- [ ] [serial] Add bounded shrinking and replayable witnesses for a retained-expectation discrepancy, so a future mismatch becomes a minimized deterministic record rather than a bare failure. r[NGV-F32-02]
- [x] [serial] Replay all 26 retained expectations through the packed Rust graph reference (`noble_ngraph::sem::add/sub/mul`) with observation equality for NaN payloads and exact bytes for every other class, and retain the configuration: `verification/binary32/replay.json` records clean commit `418381971`, `rustc 1.96.0-nightly`, host `Linux 6.18.54 x86_64`, the lane and proposal digests, and states that this is tested-implementation evidence only. r[NGV-F32-03]
- [ ] [serial] Replay the same expectations through the compiled graph legs (`Compiled::new` + `run_legs`/`evaluate`) rather than the bare semantics, retaining APFloat as a secondary lane only, and keep a per-target receipt. r[NGV-F32-03]
- [ ] [serial] Prove or explicitly classify the ordered affine fold's per-operation rounding, extraction, training, snapshot and codec layers, guard/index rules and software helpers, and the optimizer slice. r[NGV-F32-01]
- [ ] [serial] Decide whether a correspondence claim to `crates/noble-ngraph/src/sem.rs` is provable or stays a reviewed trusted assumption, and record it separately from the model theorems. r[NGV-F32-03]

## Phase 3: Promotion (not selected)

- [ ] [serial] Sync the established verification requirements, register any conformance cases with their real states, and update the affected numeric-graph obligations only for the slice actually established. r[NGV-F32-03]
