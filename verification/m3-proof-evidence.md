# M3 proof evidence: gate floor, theorem inventory, axiom policy (2026-09-18)

Scope: tasks 5.1–5.4 of the `m3-checker-coverage` change — the fragment-v1
reference model and theorem set in `proofs/m3` — over the proof commits
`9d00d8d`..`f72876b`. Fragment label: **bootstrap fragment v1**. The proof
gate itself (`verification/m3-proof-gate.sh`) and its refusal matrix are
owned by the gates agent (phase 6); this record is the proof-side evidence:
the building root, the theorem inventory with axiom sets, and the v1
modeling decisions. Every command below ran this session against the
worktree exactly as committed at `f72876b`.

## 1. The building proof root

```console
$ PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
    (cd proofs/m3 && lake build NobleM2 NobleM2.Refinement NobleM3 NobleKernel)
Build completed successfully (1699 jobs).
```

- Toolchain: pinned Lean `4.31.0` (`lean-toolchain`, checked by the m2 gate
  floor and to be checked by the m3 gate's `TOOLCHAIN` step).
- Targets: `NobleM2` (the fragment-v1 reference model — v0 modules extended
  in place), `NobleM2.Refinement` (the twelve v0 fixture refinements, all
  still proving over the v1 checker), `NobleM3` (the v1 theorem modules),
  `NobleKernel` (the generated module, computable under `native_decide`).
- No `sorry`: `grep -rnw --include='*.lean' --exclude-dir=.lake sorry
  proofs/m3/{NobleM2,NobleM3}` → 0 hits.

## 2. What fragment v1 added to the reference model (task 5.1)

The v0 modules were extended in place (`proofs/m3/NobleM2/`), so the v0
theorem names keep proving over the v1 checker — no forked model:

- `Words.lean`: `Binding.ref` (a type equation between two variables) and
  the resolution walk `Inst.followFrom`/`resolveOne`/`resolveList`/`resolve`
  — charge-before-hop, pigeonhole cycle detection, kind check at the
  terminal binding, mirroring `words::resolve`.
- `Env.lean`: the kernel-ordered 23-entry word table (`wordTable`, `equals`
  at position 7, derived `bootstrapTable`/`bootstrapEnv` so the table and
  the named schemes cannot drift), `Env.deps`, `SchemaDecl {id, scheme,
  recursive}`, and the eliminators' table identities (`caseDef = 17`,
  `ifDef = 18`, `listCaseDef = 21`, each pinned by `definitionOf_*`).
- `Judgment.lean`: the three eliminator rules (`caseRule`/`ifRule`/
  `listCaseRule` — scrutinee, branch programs, common result stack,
  conservative union bound), the node rules restated over the *resolved*
  witness with the declarative `ResolvesTo` relation (`RefHop`, `RefReaches`,
  `ChainEnds`, `PositionResolves`), and the rejection judgments `DepCycle`,
  `DeclaresRecursiveSchema`, `WitnessCycle`.
- `Check.lean`: `Binding.kindOf` (a reference binding carries no direct
  kind), the B-CHECK-02 validation walks (`depWalk` — the three-color DFS
  with per-edge charging; `validateSchemas` — the recursive-schema scan),
  `applyScheme` resolving the witness first and returning it beside the
  interface, the fold's eligibility on the resolved witness, and the v1
  preflight order (request, schemes, dependencies, schemas, bounds,
  effects) matching `preflight.rs`.
- `Embed.lean`: the v1 embedding — reference bindings, dependency lists,
  and schema declarations all embed (`embedBinding`/`embedEnv`/
  `embedSchemaDecl`).

## 3. The theorem inventory with axiom sets (tasks 5.2–5.4)

Axiom sets were enumerated this session with the m2 gate's own mechanism
(`Lean.collectAxioms` over the built environment). Policy: the strict
theorems ⊆ Lean's trifecta `{propext, Classical.choice, Quot.sound}`; the
evaluation-closed theorems may additionally use the per-declaration
`native_decide` axioms Lean 4.31 emits (`._native.native_decide.ax_1_N` —
multi-premise declarations emit a numbered family, all native_decide
schemes; `sorryAx` and anything else reject). Zero violations.

**Resolution bridge (5.1/5.2)** — `NobleM2/Soundness.lean`:

| Theorem | Statement | Axioms |
|---|---|---|
| `applyScheme_ok` | an accepted application exposes the resolved witness's instantiation | propext, Quot.sound |
| `applyScheme_ok_resolves` | …and the resolved witness satisfies `ResolvesTo` | propext, Quot.sound |
| `followFrom_chain`, `resolveOne_position`, `resolveList_positions`, `resolve_resolvesTo` | the walk decides the declarative resolution, position by position | ⊆ trifecta |

**Per-rule soundness family (5.2)** — `NobleM3/Rules.lean` (V-CHECK-03):

| Theorem | Axioms |
|---|---|
| `rule_empty_sound` | trifecta |
| `rule_literal_sound` | trifecta |
| `rule_sequence_sound` | trifecta |
| `rule_quotation_sound` | trifecta |
| `rule_word_sound` | trifecta |
| `rule_case_sound` / `rule_if_sound` / `rule_listcase_sound` | trifecta + own `native_decide` family (the table lookups) |
| `derives_cons_inv`, `invocation_word`, `quotationDerives_bound` | ⊆ trifecta |
| `caseScheme_inv` / `ifScheme_inv` / `listCaseScheme_inv` (`NobleM3/Inv.lean`) | ⊆ trifecta |

**Decision-path correspondences (5.2)** — `Rules.lean`, `NobleM3/Refinement.lean`:

| Theorem | Axioms |
|---|---|
| `eligibility_iff` (reference side, from `dataOkAt_iff`) | ⊆ trifecta |
| `eligibility_dup` / `eligibility_drop` / `eligibility_quote` (extracted rows) | own `native_decide` |
| `inclusion_iff` (= `subsetOf_iff`, symbolic) | propext, Quot.sound |
| `inclusion_extracted` (the six boundary pairs) | own `native_decide` |
| `duplication_shares_one_interface` | trifecta |
| `fresh_instantiation` | trifecta + own `native_decide` family |

**Termination and coverage (5.3)** — `NobleM2/Termination.lean`, `NobleM3/Coverage*.lean`:

| Theorem | Statement | Axioms |
|---|---|---|
| `followFrom_work_bounded` / `resolveOne_work_bounded` / `resolveList_work_bounded` / `resolve_work_bounded` | the resolution walk stays inside its hop budget (charge-before-hop) | ⊆ trifecta |
| `check_total_v1` | totality over v1 incl. both validation walks' termination | trifecta |
| `word_cost_positive` | every table entry's application charges ≥ 1 unit | propext, Quot.sound |
| `foldBody_work_bounded_v1` | the fold's work bound restated over the extended checker | trifecta |
| `coverage_dup` … `coverage_test_emit` (23) | per word: accepted and derivable at its table position | trifecta + own `native_decide` |
| `coverage_positive_word` | all 23 positions aggregated | trifecta + the family's `native_decide` axioms |
| `coverage_eliminator_exercise` | quote-produced program values feed the `case` join (B-CHECK-06) | trifecta + own `native_decide` family |

**Per-word scheme refinements (5.2)** — `NobleM3/RefinementWords.lean`:
`word_refinement_dup` … `word_refinement_test_emit` (23, each binding the
extracted instantiation path `acceptance.parts.instantiate.apply` to the
reference `applyScheme` on the word's scheme and canonical witness) and the
aggregate `word_refinements` — own `native_decide` axioms, subject the
generated constant (the call is through `acceptance.parts.instantiate.apply`
of `NobleKernel.Funs`).

**Refinement matrix (5.4)** — `NobleM3/Refinement.lean`, subject the
generated `noble_kernel.acceptance.check`:

| Family | Theorems | Axioms |
|---|---|---|
| per-word checker agreements | `refinement_word_dup` … `refinement_word_test_emit` (23) + `refinement_words` | own `native_decide` |
| per-rule representatives | `refinement_rule_empty/literal/word/sequence/quotation/case/if/listcase` (8) | own `native_decide` |
| dependency rejections | `refinement_recursive_dependency_self/mutual`, `refinement_dependency_boundary/exhausted/before_body` | own `native_decide` |
| schema rejections | `refinement_recursive_schema`, `refinement_schema_boundary/exhausted` | own `native_decide` |
| witness rejections | `refinement_cyclic_witness_self/mutual`, `refinement_resolvable_chain`, `refinement_kind_crossed_reference` | own `native_decide` (the three provenance-carrying rows after `eraseProvenance`) |
| B-CHECK-06 negatives | `refinement_duplication_negative`, `refinement_implicit_union_negative` | own `native_decide` (after `eraseProvenance`) |
| outcome correspondence | `outcomes_via_refinement_v1` (five-way), `accepted_via_refinement_v1` | **trifecta** |

The twelve v0 fixture refinements (`NobleM2.Refinement`) still prove over
the v1 checker unchanged (task 6.1's floor); the m2 agreement harness
(`NobleM2.Fixtures` `#eval`) prints 12/12 `true` at build time.

## 4. Disclosures (honest scope)

- The dep-walk *completeness* (a `DepCycle` judgment always being found by
  the bounded walk) and the symbolic `WitnessCycle → cyclic rejection`
  direction are stated as judgments + evaluated rejection agreements, not
  proven symbolically end-to-end; the walk's *decisions* on the control
  matrix are proved equal to the extracted checker's (the rejection rows
  above). The ∀-versions remain M4 targets, consistent with the M2 note on
  `applyScheme`'s ∀-refinement.
- The kernel's payload-exposure and advertised-refinement controls are
  kernel-level (task 2.3, `m3-kernel-gates.md`); the proof matrix covers the
  duplication and implicit-union negatives for B-CHECK-06.
- Files over 300 lines: `NobleM2/Env.lean` (the 23 named schemes), the
  `Judgment.lean` rules, `Rules.lean` (case-trees), and the two refinement
  modules — the M2 precedent for evaluation-closed families applies.
