# M2 acceptance control run (2026-09-17)

Scope: the M2 feasibility milestone — the finite fragment checker, its
extraction, and the reference model proofs. Every gate was run against the
current `main` of this worktree at commit `94c5ff3` (plus the evidence
commit).

## Kernel gates (implementation worktree root)

| Gate | Command | Result |
|---|---|---|
| Build | `cargo build --offline --workspace --all-features` | 0 errors |
| Tests | `cargo test --offline --workspace --all-targets --all-features` | 6 suites, 14 tests, all pass (cli smoke 3, kernel acceptance 6, budget 2, fragment 3) |
| Clippy | `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings` | 0 errors, 0 warnings |
| Octet architecture | `rm -rf .octet/compiler-architecture/shards && cargo-octet check --workspace -- --all-targets --all-features` | Architecture findings: 0 (catalog clean). Lint phase: warning-only — 12 findings: 10 formatter-inherent `mutating_input_in_pure` sites (the hand-written `Debug` impls for `Ty`/`Pattern`; Rust's `fmt::Debug` requires `&mut Formatter`, and a derived `Debug` breaks the Lean translation — tested and reverted) plus the 2 pre-existing `module_file_count` notes on `acceptance.rs` (8 files in `src/` against a limit of 7). |

## Extraction gates (task 3.1/3.2)

Full probe `.pi/m2-extraction/toolrun.sh` (retained:
`verification/m2-probe-latest.log`): `extract exit=0` — the pinned offline
app rebuilt the proof root, collected the compiler-derived inventory
(490 production subjects), ran Charon and Aeneas with the strict flags
(`--error-on-warnings`, `-abort-on-error`, `-warnings-as-errors`), and
compiled the generated `NobleKernel.lean` inside the proof root
(`lake build NobleKernel`, 1679 jobs) and the explicit `-o …olean` step.

- Charon: exit 0. Aeneas: exit 0, every kernel function translated
  (no `Could not translate`).
- Generated module: zero Lean errors, zero `sorry`.
- Disclosed `charon::opaque` assumptions (Aeneas functionalizes their loops
  inside the trait-instance mutual block, which the pinned Lean 4.31 cannot
  prove monotone): `types/impls.rs` `clone_stack`, `debug_stack`;
  `shapes/impls.rs` `clone_parts`, `clone_slots`, `debug_parts`,
  `debug_slots`. All are copying or formatting steps, not checker decisions.
- Probe ladder (refusals answered by source changes):
  `verification/m2-extraction-probe.md`.

## Reference model gates (task 4.1/4.2)

- `proofs/m2` lake root: `lake build` green (11 jobs), `sorry` count 0.
- Soundness theorem `applyScheme_ok`
  (`proofs/m2/NobleM2/Soundness.lean`): an accepted scheme application
  exposes exactly the instantiated interface — the first lemma in the
  acceptance-theorem shape `check … = Accepted checked → …`.
- M1 reference check: the M1 generated module builds in its own evidence
  root (`.pi/m1-extraction/proof`, `lake build NobleKernel`, 1706 jobs).
  The worktree's `proofs/m1` requires the generated module to be copied in
  by an extraction run and is empty in a fresh clone; it is not a regression.

## Fragment rule coverage (task 1.3 controls)

| Fragment rule | Control |
|---|---|
| Node derivation, positive | `crates/noble-kernel/src/acceptance` unit suite (6 tests) |
| Instantiation arity/kind rejection | fragment suite + `Constraint::InstantiationArity/Kind` paths |
| Stack join, wrong order vs wrong shape | `Constraint::StackOrder` / `StackJoin` via `same_multiset` |
| Effect inclusion | acceptance suite inclusion negatives (`Constraint::EffectInclusion`) |
| Recursive eligibility | acceptance suite eligibility negatives (`Constraint::Eligibility`) |
| Branch joins (B-CHECK-06) | fragment suite branch-join cases |
| Limits (stack height, type size, work) | budget suite (`Fail::Exhausted`, fail-closed accounting) |
| Rejection without execution | CLI smoke suite (3 tests) |

## Open obligations (unchanged, kept open on purpose)

The full change package's remaining tasks (coverage classification 3.3,
termination and refinement 4.3-4.4, proof-required gate 4.5, DX controls
5.1-5.3, lifecycle 6.3) stay open; the feasibility milestone claims only the
fragment scope above. Every other obligation in the ledger stays open.
