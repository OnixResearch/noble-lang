# M3 proof evidence: fragment-v1 theorem inventory (2026-09-18)

Scope: tasks 5.1–5.4 of the `m3-checker-coverage` change — the fragment-v1
reference model and theorem set in `proofs/m3`, over the commits `e108519`
(v1 reference model + judgment), `3390267` (per-word coverage), `ff00084`
(per-rule family + correspondences), `5a0de31`/`f72876b` (refinement matrix
+ termination), `8fa4c80` (tree hygiene). Fragment label: **bootstrap
fragment v1**. Every output below was produced this session against the
worktree's proof tree exactly as committed; the axiom inventory was
enumerated with the gate's own mechanism (`Lean.collectAxioms` over the
built environment, both import roots).

## 1. The proof root and its build

```console
$ PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
    (cd proofs/m3 && lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel)
Build completed successfully (1699 jobs).

$ grep -rnw --include='*.lean' --exclude-dir=.lake sorry proofs/m3
(no output — zero proof holes)
```

`NobleM3` is the v1 root: `Words` (table refinements), `Inv` (the three
eliminator shape inversions), `Coverage`/`CoverageWords` (per-word positive
coverage and the eliminator exercise), `Rules` (the per-rule family and the
reference correspondences), `RefinementWords` (per-word instantiation
refinements), `Refinement` (the checker-level matrix). `NobleM2` is the v0
floor extended in place to fragment v1 — the environment's external data
(`deps`, `SchemaDecl`), `Binding.ref` with the charge-before-hop resolution
walk, the resolving scheme application, the B-CHECK-02 preflight walks, and
the judgment's three eliminator rules plus rejection judgments — with the
m2-floor theorem *statements* unchanged and re-proven over the extended
checker (`check_soundness`, `check_total`, `foldBody_work_bounded`,
`coverage_positive`, the twelve fixture refinements in `NobleM2.Refinement`).

## 2. Axiom policy

Reference-model theorems carry only Lean's trifecta
`{propext, Classical.choice, Quot.sound}` (most carry strictly less).
Evaluation-closed theorems additionally carry the per-declaration
`native_decide` axioms Lean 4.31 emits, name family
`<theorem>._native.native_decide.ax_<n>_<n>` — one per distinct
`native_decide` occurrence in that proof (M2 saw only `ax_1_1`; the v1
multi-use theorems continue the family through `ax_1_10`). `sorryAx` and
anything else reject. **Gate note (M3DxGates):** the m2 predicate matches
the literal suffix `ax_1_1`; the v1 policy must match the family
(`._native.native_decide.ax_` prefix), or twelve sound theorems below will
be refused by accident.

## 3. Theorem inventory with axiom sets (V-CHECK-03/04/05, PO-09/10/11)

Enumerated with `Lean.collectAxioms`; "n.d." = the theorem's own
per-declaration native_decide axioms besides the listed base set.

### 3.1 The v1 model layer: judgment, resolution, rejection (task 5.1)

| Theorem | Axioms |
|---|---|
| `NobleM2.applyScheme_ok` | propext, Quot.sound |
| `NobleM2.applyScheme_ok_resolves` | propext, Quot.sound |
| `NobleM2.resolve_resolvesTo` | propext, Quot.sound |
| `NobleM2.followFrom_chain` | propext, Quot.sound |
| `NobleM2.resolveOne_position` | propext, Quot.sound |
| `NobleM2.resolveList_positions` | propext, Quot.sound |
| `NobleM2.check_soundness` | trifecta |
| `NobleM2.foldBody_derives` | trifecta |
| `NobleM2.foldBody_entry_derives` | trifecta |
| `NobleM2.check_total` | trifecta |
| `NobleM2.check_total_v1` | trifecta |
| `NobleM2.foldBody_work_bounded` | trifecta |
| `NobleM2.foldBody_work_bounded_v1` | trifecta |
| `NobleM2.resolve_work_bounded` | propext, Quot.sound |
| `NobleM2.word_cost_positive` | propext, Quot.sound |
| `NobleM2.quotationDerives_bound` | propext, Quot.sound |
| `NobleM2.derives_cons_inv` | propext, Quot.sound |
| `NobleM2.invocation_word` | propext, Quot.sound |
| `NobleM2.rule_empty_sound` | trifecta |
| `NobleM2.rule_literal_sound` | trifecta |
| `NobleM2.rule_sequence_sound` | trifecta |
| `NobleM2.rule_quotation_sound` | trifecta |
| `NobleM2.rule_word_sound` | trifecta |
| `NobleM2.caseScheme_inv` | propext, Quot.sound |
| `NobleM2.ifScheme_inv` | propext, Quot.sound |

### 3.2 Soundness, the per-rule family, and the correspondences (task 5.2)

| Theorem | Axioms |
|---|---|
| `NobleM2.listCaseScheme_inv` | propext, Quot.sound |
| `NobleM2.dataOkAt_iff` | propext, Quot.sound |
| `NobleM2.subsetOf_subset` | propext, Quot.sound |
| `NobleM2.subsetOf_iff` | propext, Quot.sound |
| `NobleM2.duplication_shares_one_interface` | trifecta |
| `NobleM2.resolvesTo_refl` | propext, Quot.sound |
| `NobleM2.WordCoverage.ee_accepted` | propext,+own n.d. axioms (0) |
| `NobleM2.coverage_positive` | propext,+own n.d. axioms (0) |
| `NobleM2.Coverage.accepted` | propext,+own n.d. axioms (0) |
| `NobleM2.Coverage.derives` | propext,+own n.d. axioms (0) |
| `NobleM2.Regression.ce_accepted` | propext,+own n.d. axioms (0) |
| `NobleM2.Regression.ce_derives` | propext,+own n.d. axioms (0) |
| `NobleM2.WordRefinement.word_refinements` | propext,+own n.d. axioms (0) |
| `NobleM2.WordRefinement.word_refinement_dup` | propext,+own n.d. axioms (0) |
| `NobleM2.WordRefinement.word_refinement_equals` | propext,+own n.d. axioms (0) |
| `NobleM2.WordRefinement.word_refinement_case` | propext,+own n.d. axioms (0) |
| `NobleM2.WordRefinement.word_refinement_test_emit` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_words` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_empty` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_literal` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_word` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_sequence` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_quotation` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_case` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_if` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_rule_listcase` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_recursive_dependency_self` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_recursive_dependency_mutual` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_dependency_boundary` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_dependency_exhausted` | propext,+own n.d. axioms (0) |

### 3.3 The per-word instantiation refinements (task 5.2, design item 1)

`NobleM2.WordRefinement.word_refinement_<word>` for all 23 entries
(`dup drop swap dip add sub mul equals quote compose run reflect unit pair
unpair inl inr case if nil cons list_case test_emit`), each binding the
extracted instantiation path `acceptance.parts.instantiate.apply` (bounds,
reference-witness resolution, substitution, eligibility) to the reference
instantiation's interface on the canonical witness, plus the aggregate
`word_refinements` over all 23 positions. All are evaluation-closed:
trifecta + own n.d. axioms.

### 3.4 Termination and coverage v1 (task 5.3)

| Theorem | Axioms |
|---|---|
| `NobleM3.Refine.refinement_dependency_before_body` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_recursive_schema` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_schema_boundary` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_schema_exhausted` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_cyclic_witness_self` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_cyclic_witness_mutual` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_resolvable_chain` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_kind_crossed_reference` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_duplication_negative` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.refinement_implicit_union_negative` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.inclusion_extracted` | propext,+own n.d. axioms (0) |
| `NobleM3.Refine.outcomes_via_refinement_v1` | trifecta |
| `NobleM3.Refine.accepted_via_refinement_v1` | trifecta |
| `noble_kernel.acceptance.check` | trifecta |

The per-word coverage family `NobleM2.WordCoverage.coverage_<word>` (23
entries, same names as §3.3) and `coverage_positive_word` (every table
position has an accepted, derivable exercising fixture) are
evaluation-closed per word (trifecta + own n.d.); the eliminator exercise
`coverage_eliminator_exercise` (eight-node composite: two literals, `inl`,
two literal-then-`quote` pairs producing the branch programs, `case`
consuming them) carries `ee_accepted`/`ee_derives` with own n.d. axioms.

### 3.5 The refinement matrix (task 5.4)

All in `NobleM3.Refine` (namespace prefix omitted), every subject binding
the generated `noble_kernel.acceptance.check` (gate-verifiable by the
semantic subject check; the extracted checker itself is trifecta):

- **Per-word checker agreements** `refinement_word_<word>` (23, §3.3 names)
  + aggregate `refinement_words` — evaluation-closed.
- **Per-rule representatives** `refinement_rule_{empty,literal,word,
  sequence,quotation,case,if,listcase}` (8) — evaluation-closed.
- **B-CHECK-02 rejection agreements**: `refinement_recursive_dependency_self`
  and `_mutual` (naming definition 23), `refinement_dependency_boundary`
  (accepted at one unit of declared work), `_exhausted` (fail-closed one
  below), `refinement_dependency_before_body` (external data validated
  before the body's dangling reference would reject),
  `refinement_recursive_schema` (naming declaration 7),
  `refinement_schema_boundary`, `_exhausted`.
- **B-CHECK-05 rejection agreements**: `refinement_cyclic_witness_self`,
  `_mutual` (invalid, cyclic-witness constraint), `refinement_resolvable_chain`
  (accepted), `refinement_kind_crossed_reference` (instantiation-kind).
  The four provenance-carrying rows compare after `eraseProvenance`
  (the extracted diagnostic locates the failing site; §6).
- **B-CHECK-06 negatives**: `refinement_duplication_negative` (a duplicated
  program value's second run cannot claim an independent instantiation —
  stack-join rejection) and `refinement_implicit_union_negative` (an `if`
  whose left branch carries the latent `test.emit` bound rejects against
  the empty allowance — no implicit join to the quiet branch).
- **Decision-path rows**: `eligibility_dup`, `eligibility_drop`,
  `eligibility_quote` (the extracted path rejects a resource witness with
  the eligibility constraint exactly where the reference guard declines
  `DataOk`; the reference-side iff is `NobleM2.dataOkAt_iff`, aliased as
  `NobleM3.Refine.eligibility_iff`), `inclusion_extracted` (the extracted
  `is_subset_of` decides the reference `subsetOf` over the pair matrix;
  the iff is `NobleM2.subsetOf_iff`, aliased `NobleM3.Refine.inclusion_iff`).
- **The generalized composition**: `outcomes_via_refinement_v1` (trifecta —
  one agreement equation yields the full five-way outcome correspondence)
  and `accepted_via_refinement_v1` (trifecta — the v0 statement over the
  v1 checker, derived from the five-way).

## 4. The kernel-level controls that stay kernel-level

Three task-2.3 controls remain exercised by the Rust suite only, not by the
Lean matrix: the `Sum<Resource,I64>` payload-exposure positive, the
advertised-refinement negative, and the doc-example lanes. Their decisions
are single fixtures of the same shape as the rows above; lifting them is
follow-up work with no proof obligation open in the tasks.

## 5. Disclosed proof debts

- **Dependency-walk remaining-budget invariant.** The schema scan's
  remaining-budget bound is proven (`validateSchemas_ok_bounded`); the
  dependency walk's analogous `w ≤ work` invariant is *not*: the walk's
  four-mode case tree is proven total (fuel-bounded, carried by
  `check_total_v1`) and mirrors the kernel's per-edge charging, but the
  remaining-budget lemma is left open with this disclosure. The
  load-bearing work measures (resolution chain, binding, pass, whole
  resolution; the fold; the schema scan) are proven.
- **eraseProvenance.** The v1 matrix keeps the M2 disclosure unchanged for
  the four provenance-carrying rejection rows; the acceptance rows are all
  exact.

## 6. File map (added or rewritten under `proofs/m3`)

- `NobleM2/`: `Env` (kernel-ordered 23-entry named-scheme table, `deps`,
  `SchemaDecl`), `Words` (`Binding.ref` + the resolution walk), `Judgment`
  (eliminator + rejection rules, `ResolvesTo`), `Check` (resolving
  application, B-CHECK-02 walks, v1 preflight), `Soundness` (resolution
  bridge lemmas), `CheckSoundness` (v1 ports), `Termination` (v1 work
  measure), `Candidate` (DecidableEq for fixtures), plus the carried v0
  modules.
- `NobleM3/`: `Refine` (table + application refinements), `Inv`, `Coverage`,
  `CoverageWords`, `Rules`, `RefinementWords`, `Refinement`.

The required-theorem name list for the gate (the contract with M3DxGates):

- strict (trifecta only): `NobleM2.{applyScheme_ok, applyScheme_ok_resolves,
  resolve_resolvesTo, followFrom_chain, resolveOne_position,
  resolveList_positions, check_soundness, foldBody_derives,
  foldBody_entry_derives, check_total, check_total_v1,
  foldBody_work_bounded, foldBody_work_bounded_v1, resolve_work_bounded,
  followFrom_work_bounded, resolveOne_work_bounded,
  resolveList_work_bounded, validateSchemas_ok_bounded,
  word_cost_positive, quotationDerives_bound, derives_cons_inv,
  invocation_word, rule_empty_sound, rule_literal_sound,
  rule_sequence_sound, rule_quotation_sound, rule_word_sound,
  rule_case_sound, rule_if_sound, rule_listcase_sound, caseScheme_inv,
  ifScheme_inv, listCaseScheme_inv, dataOkAt_iff, subsetOf_subset,
  subsetOf_iff, duplication_shares_one_interface, fresh_instantiation,
  resolvesTo_refl, outcomes_via_refinement_v1, accepted_via_refinement_v1}`
  and `noble_kernel.acceptance.check`.
- evaluation-closed (trifecta + own n.d. family): `NobleM2.{coverage_positive,
  Coverage.accepted, Coverage.derives, Regression.ce_accepted,
  Regression.ce_derives, WordCoverage.coverage_<word> (23),
  WordCoverage.coverage_positive_word,
  WordCoverage.coverage_eliminator_exercise, WordCoverage.ee_accepted,
  WordCoverage.ee_derives}`, `NobleM2.WordRefinement.{word_refinement_<word>
  (23), word_refinements}`, `NobleM3.Refine.{refinement_word_<word> (23),
  refinement_words, refinement_rule_empty, refinement_rule_literal,
  refinement_rule_word, refinement_rule_sequence,
  refinement_rule_quotation, refinement_rule_case, refinement_rule_if,
  refinement_rule_listcase, refinement_recursive_dependency_self,
  refinement_recursive_dependency_mutual, refinement_dependency_boundary,
  refinement_dependency_exhausted, refinement_dependency_before_body,
  refinement_recursive_schema, refinement_schema_boundary,
  refinement_schema_exhausted, refinement_cyclic_witness_self,
  refinement_cyclic_witness_mutual, refinement_resolvable_chain,
  refinement_kind_crossed_reference, refinement_duplication_negative,
  refinement_implicit_union_negative, eligibility_dup, eligibility_drop,
  eligibility_quote, inclusion_extracted}`.
