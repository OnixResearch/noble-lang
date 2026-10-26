## Phase 1: Checker, judgment and accounting theorems

- [x] [serial] Select the declared fragment: data and must-use resource payloads, recursive wrapping, `dup`/`drop`/`swap`/`quote`, composition, a finite two-arm branch and the ordinary error-return alternative, with an executable checker and an independent declarative judgment. r[VC-OWN-01]
- [x] [serial] Prove checker soundness, the eligibility equivalence at every wrapping depth, that a resource operand blocks every word, that dropping consumes only a duplicable operand, that duplication preserves both copies, that quoting retains, the opaque-segment conservation, the two-arm accounting rule and the empty-operand requirement for an error return. r[VC-OWN-01]
- [x] [serial] Inventory the eighteen new theorems and retain the clean-source kernel check: `verification/foundation/receipts/e7cc00993/` records 252 theorems, `clean_source: true` and axiom sets inside the allow-list. r[VC-OWN-03]

## Phase 2: Independent corpus, production comparison and mutations

- [x] [serial] Write `verification/ownership/cases.mjs` from the model and generate 104 cases over nine programmes and eleven operand shapes, classifying each case as an executable production control or as model-only with a stated reason. r[VC-OWN-02]
- [x] [serial] Drive the unary fragment through `noble_kernel::acceptance::check`: 15 accepted cases must derive exactly the model's stack and 6 rejected cases must surface the production rejection, with the category split reported. r[VC-OWN-02]
- [x] [serial] Compare a test-local shadow checker against every expectation and declare seven mutations, each caught by at least one retained expectation while the unmutated checker agrees everywhere. r[VC-OWN-02]
- [x] [serial] Retain `verification/ownership/differential.json`: clean commit, rustc pin, host, case-file, reference, model and lane digests, the coverage counts, the mutation line and the open Rust-to-model edge. r[VC-OWN-03]

## Phase 3: Promotion (not selected)

- [ ] [serial] Relate `crates/noble-kernel/src/acceptance` to the model through an extraction or refinement route; until then the differential stays finite evidence and the correspondence edge stays open. r[VC-OWN-03]
