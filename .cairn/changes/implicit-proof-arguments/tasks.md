## Phase 1: Design only (this change)

- [x] [serial] Ground the explicit core at `44329dc`: `use`, binary `apply` with level rebasing, `refl`, `intro`, Eq-only `subst`, structural definitional equality, template recognizers and the lexer's atom alphabet. Record drift from `0ea10d3`. r[IMPL-ELAB-01]
- [x] [serial] Select fragment `implicit-args/v1` with its abstract grammar, constraint language, inference rules, ground-mgu uniqueness criterion, explicit fallback and shared budget model. r[IMPL-ELAB-01] r[IMPL-TRUST-01]
- [x] [serial] Design the inspect/copy-expanded elaboration record, determinism, revisioning and source mapping, coordinated with the `proof-automation-diagnostics` goal view. r[IMPL-EXPOSE-01]
- [x] [serial] Author the worked pairs P1 (polymorphic reflexivity), P2 (dependent rebasing) and P3 (reviewed `exec-literal` rule), the ambiguity, negative, determinism and budget scenario designs IMPL-CASE-01 to IMPL-CASE-20 (all absent/not-run/open/unassessed), and the evidence plan. Validate the documents with the pinned Cairn runner only. r[IMPL-ELAB-01] r[IMPL-TRUST-01] r[IMPL-EXPOSE-01]

## Phase 2: Future implementation and evidence gates (not undertaken by this change)

- [ ] [serial] Select a non-colliding placeholder spelling and an opt-in elaboration revision in a reviewed grammar change. Implement the elaborator inside the shared proof meter, reusing the checker's rebasing substitution. r[IMPL-ELAB-01] r[IMPL-TRUST-01]
- [ ] [serial] Emit `noble-elaboration/v1` records, copy-expanded output and source maps. Feed the goal view's inserted-arguments row. r[IMPL-EXPOSE-01]
- [ ] [serial] Execute IMPL-CASE-01 to IMPL-CASE-20 with source-bound receipts, checking explicit expansions through the source checker and the pinned Lean consumer. r[IMPL-ELAB-01] r[IMPL-TRUST-01] r[IMPL-EXPOSE-01]
- [ ] [serial] Prove first-order unification uniqueness and fragment completeness for the constraint algebra in reviewed Lean. Pursue Rust elaborator refinement under the existing Charon/Aeneas policy. Keep source-to-Lean translation correctness as its separate open obligation. r[IMPL-ELAB-01]
- [ ] [serial] Register scenarios in conformance JSON and sync the requirement deltas only after independent review. Consider archive only after implementation and evidence gates. r[IMPL-EXPOSE-01]
