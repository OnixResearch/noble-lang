## Phase 1: Selected model and kernel-checked theorems

- [x] [serial] Specify the admitted value, the exact byte grammar, the name alphabet, the 64-byte wire budget, the 32-byte name budget and the \`1..=4\` configured depth in \`proofs/foundation/NobleFoundation/ServiceWire.lean\`, and prove value round trip and canonicality as named theorems with full-input consumption structural in the parser. r[VC-REF-01]
- [x] [serial] Prove the exact bounds and the decision precedence: the byte budget before the depth configuration, the depth configuration before the depth scan, and the depth scan before the closed-schema match, with the name size and alphabet, the readiness tag, the empty name and the trailing-byte cases separated as semantic classes. r[VC-REF-01]
- [x] [serial] Inventory all 22 new theorems and retain the clean-source kernel check: \`verification/foundation/receipts/b25314b1/\` records 234 theorems, \`clean_source: true\` and axiom sets inside the allow-list. r[VC-REF-03]

## Phase 2: Independent comparison and receipts

- [x] [serial] Write \`verification/service-ref/cases.mjs\` as an independent decoder from the Lean model and generate the case file: 269 cases covering the alphabet edges, name lengths 0/1/32/33, wire lengths 64/65, configured depths 0/1/4/5, truncation at every offset, appended bytes, one-byte substitutions and seeded generated values. r[VC-REF-02]
- [x] [serial] Compare the Rust codec against that case file in \`crates/noble-kernel/tests/service_ref_differential.rs\`, asserting the one documented bracket-scan divergence rather than pinning it as agreement. r[VC-REF-02]
- [x] [serial] Retain \`verification/service-ref/differential.json\`: clean commit, \`rustc 1.96.0-nightly\`, host \`Linux 6.18.54 x86_64\`, the case-file and lane digests, the coverage counts and the open Rust-to-model edge. r[VC-REF-03]

- [x] [serial] Correct the model's depth scan to a balanced one after a finite Lean-generated corpus comparison disagreed with the codec on one stray-opener input, realign \`verification/service-ref/cases.mjs\` and replace the recorded divergence with three \`control-\` cases that a counting-only scan would decide differently; receipt \`verification/foundation/receipts/134b215c5/\` records 454 theorems with \`clean_source: true\`. r[VC-REF-02]

- [x] [serial] Run nine seeded single-rule decoder faults through the same 271 cases in \`service_ref_corpus_detects_each_declared_decoder_mutation\`, each simulated from the codec's own decisions; every fault disagrees with at least one expectation (four with exactly one), and eight applied as real \`wire.rs\` edits reproduce the simulated counts. Receipt \`verification/service-ref/differential.json\` at \`97b8bdf9\` records the counts under \`coverage.mutations\`. r[VC-REF-02]

- [x] [serial] Add a whitespace family to \`verification/service-ref/cases.mjs\`: a space, tab, line feed or carriage return inserted at every offset of the canonical record and each of its two spaces deleted, 86 cases, every one refused by the model decoder and by \`wire.rs\` (357 cases in total). r[VC-REF-02]

## Phase 3: Promotion (not selected)

- [ ] [serial] Relate \`crates/noble-kernel/src/dataspace/wire.rs\` to the model through an extraction or refinement route; until then the differential stays finite evidence and the correspondence edge stays open. r[VC-REF-03]
