## Phase 1: Design only (this change)

- [x] [serial] Re-verify the contract/evidence baseline at `de1044b`: normal-return `PC`, prefix safety and total correctness (`program-contracts/spec.md:110,167-174`), rewrite/identity rules (`:369,381,408,441`), evidence binding/consumer selection (`:467,492,534`), bounded limits and diagnostic kinds (`lib.rs:50,75`), prepared subjects (`lib.rs:203,253`), `export_lean` (`lib.rs:47`), the isolated Lean consumer (`workflow/intrinsic.rs:591`) and the README verification boundary (`README.md:1021-1046`). r[SMERGE-SCOPE-01]
- [x] [serial] Select `SMERGE/1`: four exact subjects, identical complete ordered stacks, first-order observable types, structural wrapping-`I64` equality, per-observable three-way merge, conflict/absent/abnormal outcomes, `true` precondition and non-vacuity. r[SMERGE-REL-01]
- [x] [serial] Select and justify the terminating fragment from the static definition/quotation DAG and total builtins; exclude recursion, host effects, resources, live dispatch, async, runtime-generated Programs and installed quota behavior. r[SMERGE-SCOPE-01]
- [x] [serial] Specify `noble-merge-claim/v1`, independent consumer regeneration, isolated pinned Lean checking, optional bounded counterexample search, the outcome lattice and `noble-merge-evidence/v1` exact-subject/capture/dependency binding. r[SMERGE-CHECK-01] r[SMERGE-EVIDENCE-01]
- [x] [serial] Author the positive/negative fixture plan, the SM-1–SM-8 proof obligations, the experimental plan, the go/no-go criteria, coordination and non-claims. Validate only the change documents with the pinned Cairn runner. r[SMERGE-GATE-01]

## Phase 2: Future proof, prototype and evidence (not undertaken by this change)

- [ ] [serial] Encode `MergeRel` and the terminating-fragment theorem in the pinned Lean model; review the statements and prove SM-1/SM-2 before encoding any claim generator. r[SMERGE-CHECK-01]
- [ ] [serial] Implement the untrusted claim generator and the consumer's independent regeneration; reject producer-supplied subjects, claims, assumptions, hashes and trust flags. r[SMERGE-CHECK-01] r[SMERGE-EVIDENCE-01]
- [ ] [serial] Implement the isolated pinned Lean check and the optional bounded counterexample search with reproducible seeds, raw observations and source-bound receipts. r[SMERGE-CHECK-01]
- [ ] [serial] Execute the positive and negative fixtures against frozen subjects; retain every claim, proof, observation and tool identity; verify stale-subject and wrong-claim refusals. r[SMERGE-GATE-01]
- [ ] [serial] Discharge or narrow SM-3–SM-6, document the SM-7 installed-correspondence boundary and the SM-8 no-automatic-merge boundary, then make the go/no-go decision. r[SMERGE-GATE-01]
