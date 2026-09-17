# M2 scope review — feasibility milestone (2026-09-17)

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
   identities by the pinned extraction app (`extract exit=0`,
   probe 8 in [the probe ladder](m2-extraction-probe.md)).
3. **Reference model and first soundness lemma.** `proofs/m2` is an
   independent Lean model of the fragment; `applyScheme_ok` proves the first
   acceptance-theorem-shaped link (accepted application ⇒ exactly the
   instantiated interface), with no `sorry` anywhere in the root.

## Explicitly not claimed

- Termination of the checker as a proved theorem (4.3), refinement of the
  *extracted* functions to the reference model (4.4), and the proof-required
  gate (4.5) remain open.
- The bounded property harness and DX controls (5.1-5.3) remain open.
- Six `charon::opaque` assumptions (copying and formatting helpers listed in
  the acceptance run) are disclosed, not proved; they carry no checker
  decisions.
- Octet lint-phase residue: 10 formatter-inherent findings (hand-written
  `Debug` impls; a derived `Debug` breaks the translation) plus the 2
  pre-existing `module_file_count` notes. The architecture catalog phase is
  clean (0 findings).
- Every obligation outside the fragment scope in the ledger stays open.

## Evidence index

- Control run and rule coverage: [m2-acceptance-run.md](m2-acceptance-run.md)
- Extraction probe ladder: [m2-extraction-probe.md](m2-extraction-probe.md)
- Latest strict probe log: [m2-probe-latest.log](m2-probe-latest.log)
- Fragment definition: [m2-fragment.md](m2-fragment.md)
- Kernel commits: `7a33a3f` (substitution walk translates), `17a4adb`
  (generated module compiles), `94c5ff3` (bounded equality walks),
  `33a3372` (green probe recorded), `3384989` (control run recorded).
