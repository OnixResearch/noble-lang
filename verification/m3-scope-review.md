# M3 scope review — the fragment-v1 claim set (2026-09-19)

Scope: the `m3-checker-coverage` change. Everything below keeps the
fragment label **bootstrap fragment v1** — exactly the 23-entry bootstrap
word table, the B-CHECK-02 explicit rejections (recursive definition
dependencies, user-declared recursive schemas), the B-CHECK-05 explicit
rejections (self- and mutually-referential substitution witnesses), the
B-CHECK-06 eliminator controls, witness verification per the M2
instantiation-verification decision, and the v1 gates with their refusal
matrices. The M2 claim set
([m2-scope-review.md](m2-scope-review.md)) stays in force for the v0
material it bounds; this review is the v1 superset and keeps the same
claim limits.

## Claimed

1. **Kernel fragment v1 (Rust).** `crates/noble-kernel` implements the
   fragment over the candidate revision `noble-candidate/v1`, with the
   `=` contract and the two new rejection walks
   (`acceptance/validate.rs`, `words/resolve.rs`) on top of the M2
   surface. Kernel-gate matrix, exact commands, and control enumeration:
   [m3-kernel-gates.md](m3-kernel-gates.md); the Dx controls:
   [the acceptance run](m3-acceptance-run.md) (DX-01 v1 family).
2. **Extraction and re-binding (task 4.1/4.2).** The pinned Charon +
   Aeneas pair re-runs on the extended kernel inside the confirmed
   subset: both exit 0, the emitted `Types.lean`/`Funs.lean` are
   byte-identical to the checked-in `proofs/m3/NobleKernel` module, and
   the 24 emitted template axioms are filled name-for-name by the
   checked-in externals ([m3-extraction-probe.md](m3-extraction-probe.md)).
   The ladder hit two refusal classes on the way; both are retained with
   their exact diagnostics and the repairs they forced.
3. **Per-function coverage classification (task 4.3).** Every generated
   function of the extraction subject is classified exactly once under
   the reviewed vocabulary (515 total: 491 extracted, 1 proved, 18
   modeled, 5 excepted, 0 open) and every classified constant binds in
   the elaborated environment with the citing theorems checked
   semantically ([the acceptance run](m3-acceptance-run.md), m3 coverage
   gate). The five excepted entries are exactly the crate's
   `#[charon::opaque]` disclosures.
4. **Reference model and the v1 theorem set (tasks 5.1–5.4, PO-09).**
   `proofs/m3` carries the fragment-v1 model layer, the soundness story,
   the v1 totality/termination bounds, the per-word coverage family, and
   the refinement matrix binding the extracted
   `noble_kernel.acceptance.check` to the reference checker on every
   table word, every rule representative, and every new rejection family
   — including the composed `outcomes_via_refinement_v1` and
   `accepted_via_refinement_v1`. Theorem inventory, axiom sets, and the
   disclosed debts: [m3-proof-evidence.md](m3-proof-evidence.md).
5. **Termination and positive coverage (tasks 5.3/5.4, PO-10).**
   `check_total_v1` reaches one of the five outcomes on every request and
   both validation walks terminate; `foldBody_work_bounded_v1`,
   `resolve_work_bounded`, `resolveList_work_bounded`,
   `resolveOne_work_bounded`, `followFrom_work_bounded`,
   `validateSchemas_ok_bounded` and `word_cost_positive` bound the
   charged measures; `coverage_positive_word` gives every table position
   an accepted, derivable exercising fixture and the eliminator exercise
   a composite one.
6. **Refinement of the Rust checker (tasks 5.2/5.4, PO-11).** The
   per-word and per-rule agreement rows plus the rejection agreements
   prove the extracted checker's decisions equal the reference model's on
   the frozen fixture matrix. Provenance: four rejection rows compare
   after `eraseProvenance` (the M2 disclosure, kept — see "not claimed").
7. **The v1 proof gate and its refusals (task 6.1/6.2).** The gate's
   required-theorem inventory is the theorem contract of
   [m3-proof-evidence.md](m3-proof-evidence.md) §6, enumerated in the
   gate itself, with the axiom policy per name and the semantic subject
   binding; the refusal matrix refuses every mutation at its named check
   (proof hole, missing per-word theorem, substituted subject,
   unexplained external, removed eliminator oracle arm,
   schema-revision regression). The coverage gate and its refusals
   (record/citation/stale/status mutations, plus unclassified and
   dangling-citation) are green on the same tree. Commands and outputs:
   [the acceptance run](m3-acceptance-run.md).
8. **Developer-experience controls.** The bounded property harness, the
   doc-example lane, and the DX-01 negative diagnostics extend to the v1
   rejection families; the oracle's contract table carries an arm for
   every pool word and the harness fails if one goes missing
   (`oracle_table_covers_every_pool_word`, the check behind the
   removed-eliminator-oracle-arm refusal).

## Explicitly not claimed

- **Whole-kernel verification.** Fragment v1 is a labelled subset. Not
  claimed: inference/unification, generalization, recursion support, live
  resources, evaluation and recipe semantics, Wasm lowering or the
  roadmap's `wasm-feasibility` milestone, host boundaries,
  conformance-ledger scenario execution. The enumerated omissions are in
  [m3-fragment.md](m3-fragment.md) "Omissions"; every result keeps its
  subset label (V-MODEL-01, V-GATE-05).
- **Diagnostic provenance in the reference model.** For the four
  provenance-carrying rejection rows the extracted checker locates the
  failing `node`/`def` site and the reference records `none`; those rows
  are stated after `eraseProvenance`. A disclosed gap, not a claim.
- **Six `charon::opaque` assumptions.** The copy/format helpers listed in
  [the proof evidence](m3-proof-evidence.md) §6 and disclosed in the
  coverage record's excepted entries are assumptions, not proofs; they
  carry no checker decisions.
- **The dependency walk's remaining-budget lemma.** Open and disclosed
  ([m3-proof-evidence.md](m3-proof-evidence.md) §5); the walk's totality
  is proven, its `w ≤ work` invariant is not. The schema scan's bound is
  proven (`validateSchemas_ok_bounded`).
- **Three task-2.3 kernel-level controls** (the `Sum<Resource,I64>`
  payload-exposure positive, the advertised-refinement negative, and the
  doc-example lanes) are exercised by the Rust suite only, not lifted
  into the Lean matrix — no proof obligation open in the tasks.
- **Lean style cap.** Proof files over the 300-line style cap remain an
  accepted deviation (irreducible proof case-trees).
- **Octet lint residue.** The architecture catalog phase is clean (0
  findings); the lint phase is warning-only (pre-existing production
  findings plus DX test-file style warnings, unchanged in kind from M2).

## Obligation ledger

`specs/verification/obligations.json` carries the fragment-v1 scope for
the three obligations this change touches — PO-09, PO-10, PO-11 — with
their evidence bound to the artifacts above
([the proof evidence](m3-proof-evidence.md), the gates, the extraction
probe, the coverage record). Every other obligation keeps its `open`
status and its spec-wide claim: fragment v1 moves three entries from
unscoped to fragment-v1-scoped, and claims nothing about the rest.

`specs/STATUS.json` moves with the ledger, as the design requires:
`proof_implementation_exists` is now `true`, which is exactly the claim
that a proof implementation exists for the labelled fragment. The flag
is what lets the three entries be `accepted`; the validator's
greenfield controls were pinned to their own fixture state in the same
commit so they keep testing the greenfield rule rather than the
bundle's current flags. `compiler_exists` and `runtime_exists` stay
`false` — the Rust compiler and runtime are not this change's claim.
