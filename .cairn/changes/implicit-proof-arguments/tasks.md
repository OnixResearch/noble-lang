## Phase 1: Design only (this change)

- [x] [serial] Ground the explicit core at `44329dc`: `use`, binary `apply` with level rebasing, `refl`, `intro`, Eq-only `subst`, structural definitional equality, template recognizers and the lexer's atom alphabet. Record drift from `0ea10d3`. r[IMPL-ELAB-01]
- [x] [serial] Select fragment `implicit-args/v1` with its abstract grammar, constraint language, inference rules, ground-mgu uniqueness criterion, explicit fallback and shared budget model. r[IMPL-ELAB-01] r[IMPL-TRUST-01]
- [x] [serial] Design the inspect/copy-expanded elaboration record, determinism, revisioning and source mapping, coordinated with the `proof-automation-diagnostics` goal view. r[IMPL-EXPOSE-01]
- [x] [serial] Author the worked pairs P1 (polymorphic reflexivity), P2 (dependent rebasing) and P3 (reviewed `exec-literal` rule), the ambiguity, negative, determinism and budget scenario designs IMPL-CASE-01 to IMPL-CASE-20 (all absent/not-run/open/unassessed), and the evidence plan. Validate the documents with the pinned Cairn runner only. r[IMPL-ELAB-01] r[IMPL-TRUST-01] r[IMPL-EXPOSE-01]

## Phase 2: Future implementation and evidence gates (not undertaken by this change)

- [x] [serial] Select a non-colliding placeholder spelling and an opt-in elaboration revision in a reviewed grammar change. Implement the elaborator inside the shared proof meter, reusing the checker's rebasing substitution. r[IMPL-ELAB-01] r[IMPL-TRUST-01]
  Progress (2026-11-20): OWNER DECISION 2026-11-20 (decision 13) selects `(infer)`; the
  `IMPL-GRAMMAR-01` delta records the `proof 1 NAME infer :` opt-in. `intrinsic/elaborate.rs`
  implements A1-A3 (Robinson unification, occurs check, depth restriction, value-metacode rule,
  shared `Meter`) and runs at staging before the unchanged explicit checker; `tests/declared/implicit.rs`
  covers accepted positions, every other position by name, P1/P2, A3, underconstrained,
  premise candidates, wrong-code clash, under/over-application, budget exhaustion, determinism and
  the unchanged explicit refusal. The box stays open: the grammar change still needs the independent
  review owner decision 1 schedules, and no IMPL-CASE receipt exists yet (task 3).
  Ticked 2026-11-22 on the integrator review
  [`review-integrator-2026-11-22.md`](review-integrator-2026-11-22.md) (owner decision 28;
  ACCEPT-WITH-NOTES). Two gaps were closed first: the checker now resumes from the elaboration's
  work, so elaboration and checking share one request budget (F1,
  `elaboration_and_checking_share_one_request_budget`, red on the old checker), and the Octet
  deny-all findings were cleared. Note F4: substitution uses a metavariable-aware analogue of the
  checker's `replace_prop`, pinned by `instantiation_rebases_like_the_checker`; soundness does not
  depend on it because the unchanged checker and the pinned Lean consumer recheck every
  expansion. IMPL-CASE receipts stay with task 3.
- [x] [serial] Emit `noble-elaboration/v1` records, copy-expanded output and source maps. Feed the goal view's inserted-arguments row. r[IMPL-EXPOSE-01]
  Progress (2026-11-20): records, copy-expanded text and per-node source spans are implemented and
  tested (`ProofBatch::elaborations`, `verify-module`'s `elaboration` object with binder, position,
  head, solution, constraint spans, digests and explicit text). Feeding the goal view's
  inserted-arguments row remains open: `workflow/obligations.rs` still projects only prepared
  statements, so PAUTO must consume `ProofBatch::elaborations`.
  Ticked 2026-11-22 on the same review (F2, F3). The goal view and obligation map carry an
  `inserted` row per site from `ProofBatch::elaborations` (empty for an explicit proof;
  `goal_view_lists_the_arguments_the_elaborator_inserted`). The `verify-module` record names its
  schema `noble-elaboration/v1` and adds a pre-order source map from every explicit node to the
  abbreviated span that produced it (inserted nodes at their `(infer)` span), the checked claim's
  SHA-256, the model and checker revisions and the consumer policy
  (`elaboration_record_maps_every_explicit_node_to_the_abbreviated_source`, on a real staged and
  checked module).
- [x] [serial] Execute IMPL-CASE-01 to IMPL-CASE-20 with source-bound receipts, checking explicit expansions through the source checker and the pinned Lean consumer. r[IMPL-ELAB-01] r[IMPL-TRUST-01] r[IMPL-EXPOSE-01]
  Progress (2026-11-22): every case input was first run through the production `verify-module`
  (and, for IMPL-CASE-20, `session --declared-modules`) to check the designs before writing the
  gate. That run found three gaps against the requirements, now closed in the elaborator:
  refusals did not name the unresolved binder, head identity and originating constraints
  (IMPL-ELAB-01; only `underconstrained` did), diagnostics had no byte caps (a clash between two
  5.6 KiB codes produced an 11 KiB refusal; IMPL-EXPOSE-01), and the elaborator ignored the requested
  `--normalization-work`/`--substitution-work` budgets, so exhaustion below its own need was
  reported by the checker afterwards (IMPL-TRUST-01). See the design's implementation notes;
  controls: `wrong_value_code_is_invalid_constraint` (exact message),
  `constraint_spans_are_capped_with_a_count`, `rendered_terms_are_capped_with_a_count`,
  `premise_placeholder_lists_candidates_without_choosing` (binder spans),
  `app_spines_refuse_under_and_over_application`, `shared_proof_budgets_bound_elaboration` and
  the in-crate `exhaustion_names_the_exceeded_budget_and_caps_count_cut_bytes`. Design inputs
  that cannot run as written are listed for amendment: IMPL-CASE-18's 200 nested binders exceed
  the frontend depth limit (64; the deepest elaborable head has 20 `Type0` and 20 value binders,
  whose explicit apply chain fits the elaboration depth bound), IMPL-CASE-13's off-by-two term is
  refused at the application (`apply value has wrong code`) before any claim comparison,
  IMPL-CASE-14's module-version change is a different head identity (`logic@2/poly-refl`), not a
  changed revision, and IMPL-CASE-16's `?` holes are tokenized as atoms and refused where they
  stand.
  Ticked 2026-11-22 (Cairn batch 5). The seven amendments are recorded in the case file
  (integrator decision 37, accepted by the owner as decision 38). `verification/implicit-cases/gate.mjs`
  ran at commit `b4c63a1e` with 100 determinism runs per pair
  ([`runs/20261122-b5`](../../../verification/implicit-cases/runs/20261122-b5/acceptance.json),
  sha256 `1641c70c…`, source revision `sha256:ed0941eb…`, binary `38d1e33d…` before and after):
  verdict `all-passed`, IMPL-CASE-01..20 promotable. Every case is promoted to
  implemented/passed with proof open and explicit trust; the batch-5 review is the integrator
  self-review `review-integrator-2026-11-22-batch5.md` under owner decision 38 (owner veto at the
  push).
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
- [x] [serial] Register scenarios in conformance JSON and sync the requirement deltas only after independent review. Consider archive only after implementation and evidence gates. r[IMPL-EXPOSE-01]
  Done 2026-11-22 (owner decision 44): the IMPL-CASE-01..20 designs were registered in
  `specs/conformance/implicit-cases.json` and promoted in batch 5 from the IMPL gate receipt
  (`verification/implicit-cases/runs/20261122-b5`). The owner accepted their veto as the review of the
  integrator-authored deltas (as in decision 38) and asked for the sync now: `cairn sync
  implicit-proof-arguments --execute` merged IMPL-ELAB-01, IMPL-TRUST-01, IMPL-EXPOSE-01 and
  IMPL-GRAMMAR-01 into `.cairn/specs/program-contracts` and `.cairn/specs/developer-experience`, and
  `tools/cairn-specs.mjs --write-views` regenerated the views and scenario links. The change stays
  active: task 4 (Rust elaborator refinement) is open by the same decision, so it is not archived.
