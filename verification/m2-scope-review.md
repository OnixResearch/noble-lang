# M2 scope review — feasibility milestone (updated 2026-09-18)

Supersedes the 2026-09-17 review: the proof milestone (tasks 4.3–4.5) and
the evidence records are now in scope. Everything below keeps the fragment
label **M2 fragment v0**; the omissions enumerated in the design's
V-MODEL-01 list remain omissions.

## Claimed

1. **Checker fragment (Rust).** A finite acceptance fragment of Noble's type
   system is implemented in `crates/noble-kernel` under the architecture
   catalog: untrusted candidates and witnesses, node derivations,
   instantiation verification, stack joins, effect propagation and inclusion,
   recursive eligibility, branch joins, declared limits with fail-closed
   accounting, and a five-way outcome domain. Rejection happens without
   execution. Coverage controls are enumerated in
   [the acceptance run](m2-acceptance-run.md).
2. **Extraction.** Every kernel function translates through the pinned
   Charon + Aeneas toolchain under the strict flags, and the generated
   `NobleKernel.lean` compiles in the offline proof root with zero errors and
   zero `sorry`, bound to the actual source, inventory, selection, and tool
   identities by the pinned extraction app (`extract exit=0`, probe 8 in
   [the probe ladder](m2-extraction-probe.md)). The split-file regeneration
   (probe 9) additionally makes the generated module *computable*: the
   external surface is implemented in `NobleKernel/FunsExternal.lean` and
   the extracted `acceptance.check` call graph evaluates.
3. **Reference model and soundness (task 4.2, PO-09).** `proofs/m2` is an
   independent Lean model of the fragment, repaired to agree with the
   kernel's decisions on all twelve fixture cases (divergence list in
   [the proof evidence](m2-proof-evidence.md) §4). `check_soundness` proves
   the acceptance-theorem shape (V-CHECK-03): `check … = accepted checked`
   implies well-formedness and a declarative `TypingDerivation`, with no
   `sorry` anywhere in the root.
4. **Termination and positive coverage (task 4.3, PO-10).**
   `foldBody_work_bounded` (every successful fold stays within its work
   budget and strictly spends from it on a nonempty body), `check_total`
   (every request on every candidate reaches one of the five outcomes), and
   `coverage_positive` (a concrete accepted fragment input carrying a
   `Derives` witness at its expected interface) are proved in
   `NobleM2/Termination.lean`.
5. **Refinement (task 4.4, PO-11).** The twelve kernel acceptance fixtures
   are restated in `NobleM2/Fixtures.lean` and proved, in
   `NobleM2/Refinement.lean`, to have the same projected outcome under the
   *extracted* `noble_kernel.acceptance.check` and under the reference
   `NobleM2.check` (the two diagnostic-provenance fixtures after
   `eraseProvenance`, see below). `accepted_via_refinement` composes the
   agreement with acceptance, handing extracted acceptance to the reference
   soundness story.
6. **The proof-required gate (task 4.5).** The checked-in control
   `verification/m2-proof-gate.sh` passes on the current tree and refuses
   each of the four mutated controls executed by
   `verification/m2-proof-gate-refusals.sh`: a `sorry` proof hole
   (`NO-SORRY`), a refinement restated about a hand-written same-named
   substitute module while the tree still builds green
   (`REFINEMENT-SUBJECT`, plus the constant-level subject binding), a
   deleted external-model implementation (`BUILD`), and the
   `coverage_positive` theorem file deleted with the tree kept building
   (`REQUIRED-THEOREMS`). The gate also enforces the axiom policy (Lean's
   trifecta for the reference theorems and the extracted checker; the
   per-declaration `native_decide` axioms additionally for the
   evaluation-closed theorems) and rejects any template axiom left in the
   external files (`EXTERNAL-MODELS`). Command and outputs:
   [the proof evidence](m2-proof-evidence.md) §1–2.

## Explicitly not claimed

- The bounded property harness and DX controls (5.1–5.3) remain open.
- The lifecycle: spec sync, archive, push, integration (6.3) remain open;
  the obligation-ledger entries for PO-09/PO-10/PO-11 move with the parent
  acceptance sequence.
- The per-function coverage classification gate (3.3 — separate
  extracted/proved/modeled/excepted/open statuses in the reviewed source
  inventory) remains open; the proof gate's theorem inventory is a
  proof-level control, not that classification.
- **Diagnostic provenance in the reference model.** For the
  `hidden_emit` and `resource_eligibility` fixtures the extracted checker
  locates the failing `node`/`def` site and the reference records `none`;
  their refinement theorems are stated after `eraseProvenance` (clears
  exactly those two fields). The reference model's provenance fields remain
  a disclosed gap, not a claim.
- Six `charon::opaque` assumptions (copying and formatting helpers listed in
  the acceptance run; implementations and their justification in
  [the proof evidence](m2-proof-evidence.md) §6) are disclosed, not proved;
  they carry no checker decisions.
- Octet lint-phase residue: 10 formatter-inherent findings (hand-written
  `Debug` impls; a derived `Debug` breaks the translation) plus the 2
  pre-existing `module_file_count` notes. The architecture catalog phase is
  clean (0 findings).
- Lean proof files over the 300-line style cap (`Judgment` 358,
  `Termination` 317, `CheckSoundness` 379, `Embed` 488) are an accepted
  deviation (irreducible proof case-trees / one symmetric translation
  layer), documented in [the proof evidence](m2-proof-evidence.md) §7.
- Every obligation outside the fragment scope in the ledger stays open.

## Evidence index

- Proof gate, refusal matrix, theorem inventory, repairs, decisions,
  disclosures: [m2-proof-evidence.md](m2-proof-evidence.md)
- Control run and rule coverage: [m2-acceptance-run.md](m2-acceptance-run.md)
- Extraction probe ladder: [m2-extraction-probe.md](m2-extraction-probe.md)
- Latest strict probe log: [m2-probe-latest.log](m2-probe-latest.log)
- Fragment definition: [m2-fragment.md](m2-fragment.md)
- Kernel commits: `7a33a3f` (substitution walk translates), `17a4adb`
  (generated module compiles), `94c5ff3` (bounded equality walks),
  `33a3372` (green probe recorded), `3384989` (control run recorded).
- Proof commits: `2d3cbe7` (reference model repaired to kernel agreement),
  `592e86f` (split-file external implementations, computable extraction),
  `5d7c964` (refinement theorems), `92b0081` (termination, coverage,
  check soundness).
