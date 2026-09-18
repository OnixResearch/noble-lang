# M3 implementation tasks

All tasks are open. Package validation does not complete an implementation task.
Each phase starts after the previous phase passes its required checks.
All commands use the dedicated implementation worktree as the explicit root.
The [fragment v1 definition and audit](design.md) fix the exact scope; every control keeps its fragment label.

## Phase 1: Baseline, audit, and fragment v1

- [ ] [serial] 1.1 Run the document baseline and the four Cairn gates; record the exact commands, outcomes, and the independent M3 acceptance scope in `verification/m3-acceptance-run.md`. Verify: the recorded gate outputs exit 0. r[VT-M3-01] r[VT-M3-04]
- [ ] [serial] 1.2 Write `verification/m3-fragment.md`: the B-CHECK-01..07 audit table from the design, the 23-entry word table with per-word contract sources, the v1 control map (positive, boundary, exhausted, rejection per family), and the enumerated omissions. Verify: the document exists and every audited `In` row names its controls. r[VT-M3-01] r[VT-M3-02]
- [ ] [serial] 1.3 Map every scoped-in family (B-CHECK-02 recursion/schema rejection, B-CHECK-05 cyclic substitution, B-CHECK-06 eliminator depth) to its named fixtures and its theorem obligations, and freeze the per-word fixture list. Verify: the control map in `verification/m3-fragment.md` lists each fixture with its expected outcome. r[VT-M3-01]

## Phase 2: Kernel extension

- [ ] [serial] 2.1 Add the `=` contract to the bootstrap table and its behavior kind, keeping the table in `Definition` order with its length assertion updated. Verify: `cargo test --offline -p noble-kernel` table test passes with 23 entries. r[VT-M3-02]
- [ ] [serial] 2.2 Implement the rejection paths: recursive-definition dependency and user-declared recursive schema in environment data (unsupported, B-CHECK-02) and self- or mutually-referential substitution witnesses (invalid, B-CHECK-05), each as a bounded work-charged walk with its declared limit. Verify: the new `recursion-schema-cycle` test module passes with boundary and exhausted companions. r[VT-M3-01]
- [ ] [serial] 2.3 Add the kernel controls: per-word positive and rejection fixtures for all 23 entries, the duplication-shares-one-interface negative, the `Sum<Resource<R>,I64>` payload-exposure eliminator control, and the implicit-union-join and advertised-refinement negatives. Verify: the extended acceptance and fragment suites pass. r[VT-M3-01] r[VT-M3-02]
- [ ] [serial] 2.4 Run the full kernel gate matrix (build, tests, clippy `-D warnings`, fmt, octet architecture) on the extended kernel. Verify: recorded outputs with 0 errors and 0 architecture findings. r[VT-M3-01]

## Phase 3: Property harness, oracle, and documentation controls

- [ ] [serial] 3.1 Extend the generator pool to all 23 entries plus the resource-maker fixture and add independent oracle arms for `case` and `list.case` written from the documented contracts only; re-run the agreement and malformed lanes with recorded seeds and bounds. Verify: `cargo test --offline -p noble-kernel --test property` passes with 0 disagreements and 0 accepted malformed candidates. r[VT-M3-02] r[VT-M3-01]
- [ ] [serial] 3.2 Add per-word `noble-check` documentation examples (at least one rejection per constrained word) with the illustrative control retained. Verify: `cargo test --offline -p noble-kernel --test docexamples` passes and the executed count covers every word. r[VT-M3-02]
- [ ] [serial] 3.3 Extend the DX-01 diagnostic controls to the new rejection families (recursion, schema, cycle, eliminator joins) with provenance reported as unavailable where it is. Verify: `cargo test --offline -p noble-kernel --test dx01` passes with the extended case list recorded. r[VT-M3-01]

## Phase 4: Extraction re-binding and coverage

- [ ] [serial] 4.1 Re-run the extraction probe ladder on the extended kernel inside the confirmed subset, retaining any failed probe with its diagnostic. Verify: `verification/m3-extraction-probe.md` records the ladder with the final probe green. r[VT-M3-02]
- [ ] [serial] 4.2 Regenerate the Lean module under the pinned Charon/Aeneas pair with `-split-files` externals, re-bind the record to the actual source, inventory, selection, and tool identities, and keep the generated module computable. Verify: charon and aeneas exit 0 and `lake build NobleKernel` succeeds under the pinned backend. r[VT-M3-02]
- [ ] [serial] 4.3 Re-derive `verification/m3-coverage.json` from the regenerated inventory with separate extracted, proved, modeled, excepted, and open statuses; the acceptance-path decision functions are not silently open. Verify: the coverage join reports every generated function classified exactly once. r[VT-M3-04]

## Phase 5: Reference model and the v1 theorem set

- [ ] [serial] 5.1 Add the `proofs/m3` Lake root with the reference model extended to fragment v1: the `=` scheme, the three eliminator judgment rules, the rejection judgments for recursion, schema, and cycle, and the v1 embedding — independent of the extracted implementation. Verify: `lake build NobleM3` succeeds with no `sorry`. r[VT-M3-03]
- [ ] [serial] 5.2 Prove the per-word scheme refinements for all 23 entries, the per-rule soundness family (EMPTY, LITERAL, WORD, SEQUENCE, QUOTATION, CASE, IF, LISTCASE), the eligibility and inclusion correspondences, and the duplication/freshness theorem. Verify: the theorem targets build and are enumerated in `verification/m3-proof-evidence.md` with axiom inventories. r[VT-M3-03]
- [ ] [serial] 5.3 Re-establish termination and positive coverage over fragment v1: `check_total_v1`, `foldBody_work_bounded_v1` including the new rejection walks, per-word cost positivity, and `coverage_positive_word` for every table entry. Verify: the targets build and each word's coverage witness is recorded. r[VT-M3-03]
- [ ] [serial] 5.4 Prove the refinement matrix: per-word and per-rule representative-input agreements between the extracted checker and the reference, the rejection agreements for the new negatives, and the generalized `accepted_via_refinement`. Verify: every refinement subject binds a generated constant and the proof root builds with no proof holes. r[VT-M3-03]

## Phase 6: Gate extension and refusals

- [ ] [serial] 6.1 Add `verification/m3-proof-gate.sh` and `verification/m3-coverage-gate.sh` succeeding the M2 gates: the required-theorem list covers the v1 set, the axiom policy and subject bindings are enforced, and the per-function classification is checked. Verify: both gates pass on the current tree with outputs retained. r[VT-M3-04]
- [ ] [serial] 6.2 Extend the refusal matrix with the four new mutations (missing per-word theorem, missing per-word coverage case, removed eliminator oracle arm, schema-revision regression) alongside the M2 mutations, each on a scratch copy. Verify: `verification/m3-proof-gate-refusals.sh` (and the coverage refusals) reject every mutation with the tree kept green on the positive baseline. r[VT-M3-04]

## Phase 7: Acceptance and lifecycle

- [ ] [serial] 7.1 Run the full v1 control matrix end to end, retain evidence for every family, control, gate, and refusal, and keep the M1/M2 reference checks green. Verify: `verification/m3-acceptance-run.md` records every command with its outcome. r[VT-M3-04]
- [ ] [serial] 7.2 Update the scope review and the evidence records; move the PO-09/PO-10/PO-11 ledger entries to the exact fragment v1 claims with bound evidence; re-disclose or close the M2 `eraseProvenance` and opaque disclosures. Verify: `verification/m3-scope-review.md` and `specs/verification/obligations.json` agree with the built evidence. r[VT-M3-03] r[VT-M3-04]
- [ ] [serial] 7.3 Execute the spec sync, archive with an explicit date, commit, push, and integrate without a force-push after acceptance. Verify: Cairn validation passes on the archived tree. r[VT-M3-04]

## After Task Completion

These lifecycle actions follow the completed implementation checklist. They are not prerequisites for marking their own completion.

1. Run the archive dry-run and inspect its plan.
2. If the plan is unblocked, execute archive with an explicit archive date.
3. Run Cairn validation and commit the completed change.
4. Push the verified branch and integrate it into `origin/main` without a force-push.
5. Before worktree removal, run the drain-evidence guard and preserve any owner-only evidence outside the worktree.
