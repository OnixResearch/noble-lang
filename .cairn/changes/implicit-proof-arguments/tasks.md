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
  Progress (2026-10-31): the Lean half is proved at model level in
  `proofs/foundation/NobleFoundation/ImplicitUnification.lean`.
  - An answer `solved σ` is a most general unifier (`solved_mgu`).
  - Clash and occurs refusals hide no solution (`refused_no_unifier`).
  - Every problem with a unifier is answered under some budget (`solvable_iff`), and two budgets
    that both answer agree (`answers_agree`).
  - All unifiers agree at a meta exactly when the answer is ground there; otherwise two solutions
    differ (`unique_iff_ground`).
  - C's codes and values encode injectively into the tree algebra.

  Rule 7's level restriction and the value-meta exact-code rule are proved at model level in
  `proofs/foundation/NobleFoundation/ImplicitScope.lean`:
  - a ground binding that mentions a level at or beyond its meta's depth keeps every
    instantiation ill-scoped (`ill_scoped_persists`, `ill_scoped_has_no_repair`), so refusing it
    when the solver produces it is the only sound point;
  - a value meta solved by a level of the wrong code stays wrong-coded under every later
    substitution (`wrong_code_persists`);
  - an accepted premise argument is a ground explicit term, so no substitution retargets its
    witness identity (`Arg.source_bound_stable`).
  The design's propositions and sorts are encoded in
  `proofs/foundation/NobleFoundation/ImplicitPropositions.lean`:
  - structural identity of a proposition or sort is exactly equality of its encoded tree
    (`PropT.enc_injective`, `SortT.enc_injective`);
  - an encoded proposition is never a metavariable, so it cannot collide with a code or value
    meta (`PropT.enc_ne_mvar`, `PropT.head_enc`);
  - rebasal acts on exactly the level occurrences and composes (`PropT.levels_shift`,
    `PropT.shift_comp`, `PropT.shift_id`), which is the law IMPL-CASE-13's wrong order violates.

  The task stays open: no Rust elaborator exists to refine against the model, and no Rust
  representation of the encoded propositions exists either. Which premise positions the surface
  grammar may infer stays a policy decision, and source-to-Lean translation stays V-INTRINSIC-01.
- [ ] [serial] Register scenarios in conformance JSON and sync the requirement deltas only after independent review. Consider archive only after implementation and evidence gates. r[IMPL-EXPOSE-01]
