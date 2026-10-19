## Phase 1: Design (this change)

- [x] [serial] Re-verify the contract/evidence baseline at `de1044b`: normal-return `PC`, prefix safety and total correctness (`program-contracts/spec.md:110,167-174`), rewrite/identity rules (`:369,381,408,441`), evidence binding/consumer selection (`:467,492,534`), bounded limits and diagnostic kinds (`lib.rs:50,75`), prepared subjects (`lib.rs:203,253`), `export_lean` (`lib.rs:47`), the isolated Lean consumer (`workflow/intrinsic.rs:591`) and the README verification boundary (`README.md:1021-1046`). r[SMERGE-SCOPE-01]
- [x] [serial] Select `SMERGE/1`: four exact subjects, identical complete ordered stacks, first-order observable types, structural wrapping-`I64` equality, per-observable three-way merge, conflict/absent/abnormal outcomes, `true` precondition and non-vacuity. r[SMERGE-REL-01]
- [x] [serial] Select and justify the terminating fragment from the static definition/quotation DAG and total builtins; exclude recursion, host effects, resources, live dispatch, async, runtime-generated Programs and installed quota behavior. r[SMERGE-SCOPE-01]
- [x] [serial] Specify `noble-merge-claim/v1`, independent consumer regeneration, isolated pinned Lean checking, optional bounded counterexample search, the outcome lattice and `noble-merge-evidence/v1` exact-subject/capture/dependency binding. r[SMERGE-CHECK-01] r[SMERGE-EVIDENCE-01]
- [x] [serial] Author the positive/negative fixture plan, the SM-1–SM-8 proof obligations, the experimental plan, the go/no-go criteria, coordination and non-claims. Validate only the change documents with the pinned Cairn runner. r[SMERGE-GATE-01]

## Phase 2: Model prototype and decision (this change)

- [x] [serial] Implement the bounded test-only stack model and the positive/negative fixture lane; cover independent/equivalent/one-sided/identical changes, conflict, lost change, wrapping `I64`, missing output, changed capture, abnormal and vacuous behavior. r[SMERGE-GATE-01]
- [x] [serial] Verify subject identity binds captures and callees, wrong claims and stale revisions are refused, and nonverified outcomes remain nonverified. r[SMERGE-EVIDENCE-01] r[SMERGE-CHECK-01]
- [x] [serial] Record the source-bound receipt and the explicit `no-go` decision because SM-1 through SM-6 remain open; do not integrate, merge, publish or admit a live slot. r[SMERGE-GATE-01]

## Phase 3: Future proof and integration (not selected by the no-go decision)

- [ ] [serial] Encode `MergeRel` and the terminating-fragment theorem in the pinned Lean model and prove SM-1/SM-2. r[SMERGE-CHECK-01]
- [ ] [serial] Implement the untrusted claim generator, independent consumer regeneration and isolated pinned Lean check; reject producer-supplied subjects, claims, assumptions, hashes and trust flags. r[SMERGE-CHECK-01] r[SMERGE-EVIDENCE-01]
- [ ] [serial] Discharge or narrow SM-3–SM-6, document SM-7/SM-8 and produce a separately reviewed integration plan. r[SMERGE-GATE-01]
