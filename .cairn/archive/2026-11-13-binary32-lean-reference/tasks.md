## Phase 1: Selected model and kernel-checked expectations

- [x] [serial] Select one explicit kernel-reducible binary32 model with a documented trusted base, instead of the opaque Lean 4.31.0 `Float32` or an unaudited pin change: raw encodings, classification, scaled exact interpretation, round-to-nearest/ties-to-even, gradual underflow, and add/subtract/multiply with signed-zero, infinity, NaN and overflow rules. r[NGV-F32-01]
- [x] [serial] State the proved model properties as named theorems and kernel-check all 26 retained expectations proposed by the external tool: ties in both directions, cancellation, both zero signs, subnormals, the normal boundary, underflow, overflow in both signs, infinities and NaNs. r[NGV-F32-02]
- [x] [serial] Establish that round-to-real reasoning cannot discharge binary32, and that reassociation and implicit contraction are observable. r[NGV-F32-01]
- [x] [serial] Retain the clean-source checker run with the Lean and Mathlib pins, the inventoried statements and the axiom audit, and document the claim/non-claim split in `proofs/foundation/README.md`. r[NGV-F32-03]
- [x] [serial] Run the foundation gates: `lake build NobleFoundation` with no warnings, `verification/foundation/check.mjs` on clean source, `verification/binary32/propose.py`, and Cairn/check-specs validation. r[NGV-F32-03]

## Phase 2: Correspondence and the rest of the slice (not selected)

- [x] [serial] Add the test-local mutation lane in `NobleFoundation/Binary32Mutations.lean`: an away-from-zero tie rule, collapsing every zero result to `+0`, flush-to-zero, a zero-sign-insensitive observation, a commutative subtraction and implicit contraction. Each variant is caught by a named retained expectation and `all_declared_mutations_are_caught` states that none escapes; the receipt is `verification/foundation/receipts/aa074e1/`. r[NGV-F32-02]
- [x] [serial] Add a bounded, deterministic shrinker with replayable witnesses for one binary32 operation discrepancy: it walks the documented simplification list, accepts only strictly simpler pairs under the documented order, and reports a LOCAL minimum (never a global one) inside a documented attempt budget. The flush-to-zero lane asserts determinism, replayability and termination, and `verification/binary32/shrinker.json` records the witness (flush-to-zero on `0x00800000 * 0x3f000000`), the clean commit, the toolchain and the host. A pair that needs a simplification outside the list is reported unchanged rather than invented, so the lane claims a documented local minimum and no reachability beyond it. r[NGV-F32-02]
- [x] [serial] Drive the same shrinker from a retained-expectation discrepancy (model expectation versus tested implementation) instead of a bare operation discrepancy, so a future expectation failure becomes a minimized deterministic record rather than a bare failure. `retained_expectation_discrepancies_shrink_and_preserve_their_classes` walks the retained packet, skips every expectation the candidate still reproduces, shrinks each remaining pair and pins replay, determinism, an unexhausted budget and observation-class preservation per witness; the additive/subtractive expectations are stated as outside this multiplication-only lane rather than panicking. Measured: two of eight retained multiplication expectations disagree with the flush-to-zero candidate (packet indices 12 and 14) and each shrinks in 36 attempts to a stable subnormal witness. `verification/binary32/shrinker.json` moves to schema `noble-binary32-shrinker/v2` with `witnesses`, `expectation_witnesses` and `carried_expectations`. r[NGV-F32-02]
- [x] [serial] Replay all 26 retained expectations through the packed Rust graph reference (`noble_ngraph::sem::add/sub/mul`) with observation equality for NaN payloads and exact bytes for every other class, and retain the configuration: `verification/binary32/replay.json` records clean commit `9845a9e9d`, `rustc 1.96.0-nightly`, host `Linux 6.18.54 x86_64`, the lane and proposal digests, and states that this is tested-implementation evidence only. r[NGV-F32-03]
- [x] [serial] Replay the same expectations through the compiled graph legs (`Compiled::new` + `run_legs`) rather than the bare semantics: every retained expectation builds a two-input F32 graph, all four legs agree, and L2 APFloat, L3 reference and L4 compiled each match the expectation (NaN by observation equality, every other class byte-exactly). `verification/binary32/replay.json` now records both lanes at clean commit `9845a9e9d` on this target. r[NGV-F32-03]
- [x] [serial] Prove or explicitly classify the ordered affine fold's per-operation rounding, extraction, training, snapshot and codec layers, guard/index rules and software helpers, and the optimizer slice. r[NGV-F32-01]
  - The fold is proved (`fold`, `fold_append`, `fold_order_is_load_bearing`, two signed-zero fixtures); every remaining layer is classified **not modelled** with its owner issue in the README table, and none of them is claimed here.
- [x] [serial] Decide whether a correspondence claim to `crates/noble-ngraph/src/sem.rs` is provable or stays a reviewed trusted assumption, and record it separately from the model theorems. r[NGV-F32-03]
  Decided 2026-11-13: it stays a **reviewed trusted, tested assumption** on
  `x86_64-unknown-linux-gnu`, recorded as its own decision in `design.md` apart
  from the model theorems. It cannot be proved now because `sem.rs` computes with
  the Rust `f32` operators themselves (`sem.rs:90`, `:96`, `:102`) and Aeneas
  does not translate floats: the upstream support is the open draft
  AeneasVerif/aeneas#1073, tracked as #92. The evidence the assumption rests on
  is named with its limits (26 replayed expectations in
  `verification/binary32/replay.json`, the shrinker, 36 decimal agreements, the
  APFloat lane, the ignored 10^7 soft-float tier), and none of it is reported as
  proof. Reopen when #92 lands or a reviewed float-capable extraction is
  selected.

## Phase 3: Promotion

- [x] [serial] Sync the established verification requirements, register any conformance cases with their real states, and update the affected numeric-graph obligations only for the slice actually established. r[NGV-F32-03]
  Promoted 2026-11-13. NGV-F32-01, NGV-F32-02 and NGV-F32-03 are in the canonical
  `.cairn/specs/verification/spec.md` (and its generated `specs/VERIFICATION.md` view), with
  `NGV` added to the evidence-route table in `tools/check-specs.mjs`. NGV-F32-CASE-01/02/03 are
  registered in `specs/conformance/verification-cases.json` with their real states: the two model
  cases are implemented, executed and kernel-proved against the clean-source receipt
  `verification/foundation/receipts/f8555afa3/`, and the receipt-scope case is a passed review with
  proof `not-applicable`. `specs/STATUS.json` `numeric_graph.proof_slices` records the NG-PO-D
  slice — proved kernel-reducible model, tested replay, reviewed trusted correspondence assumption
  on `x86_64-unknown-linux-gnu` — and NG-PO-D **stays** in `proof_obligations_open` with
  `ng_po_discharged` empty. Views regenerated with `bun tools/cairn-specs.mjs --write-views`, the
  requirements ledger with `bun tools/check-specs.mjs --refresh-ledger`; both document self-tests
  and `cairn validate` pass.
