# M3 acceptance control run (final, 2026-09-19)

Scope: the `m3-checker-coverage` change — bootstrap fragment v1 (the
23-entry word table, the B-CHECK-02 recursion/schema rejections, the
B-CHECK-05 cyclic-substitution rejections, and the extended controls).
This file records the **full end-to-end control matrix** (task 7.1):
every family, control, gate, and refusal, with the command and its
outcome. Baseline tree at first recording: `c668dac` (kernel v1
`53f2fcf`, re-extraction `be7d986`, package `50807a9`, proof-root
commits through `c668dac`); the matrix below was re-run on the final
tree with the v1 proof root building (strict tier) and the v1 refusal
matrices in place.
Kernel-gate evidence for tasks 2.1–2.4 lives in
[m3-kernel-gates.md](m3-kernel-gates.md); every row below marked
"re-run" was executed again on this tree while writing this document.

## Document baseline and Cairn gates

Run from the implementation-worktree root. The `bun` tool self-tests and
the family's document validation (`tools/check-specs.mjs`) are the
document baseline; the four Cairn gates follow.

```console
$ bun tools/cairn-specs.mjs --self-test
  "result": "passed", "specs": 12, "scenarios": 149,
  "self_tests_passed": 26, "lane": "document-conversion-only"

$ bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs
  100 pass, 0 fail

$ bun tools/check-specs.mjs --self-test
  "result": "passed", ... "self_tests_passed": 226

$ bun tools/cairn.mjs validate --root .           # exit 0
$ bun tools/cairn.mjs gate proposal m3-checker-coverage --root .   # exit 0, valid: true
$ bun tools/cairn.mjs gate design   m3-checker-coverage --root .   # exit 0, valid: true
$ bun tools/cairn.mjs gate tasks    m3-checker-coverage --root .   # exit 0, valid: true
```

(A recording defect in the 2.1 checkbox — a `../../` link from
`tasks.md` that pointed inside `.cairn/` — was the only document
baseline failure; fixed in the same commit that added this file.)

## Kernel gates (re-run, exit 0 each)

Environment: `PATH=/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21/bin`,
`CARGO_HOME=/tmp/cargo-home`, `CARGO_TARGET_DIR=<repo>/target`;
octet at `/nix/store/vj0sg5rn291z78im7gd2cqzqzd0bq3zy-cargo-octet-0.1.0/bin`.

| Gate | Command | Result (re-run 2026-09-18; kernel test counts updated 2026-09-19) |
|---|---|---|
| Tests | `cargo test --offline -p noble-kernel` | 9 suites all ok: acceptance 11, budget 2, docexamples 2, dx01 9, fragment 6, property 3, recursion-schema-cycle 11 (+ lib/doctests 0) |
| Clippy | `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings` | exit 0, 0 errors, 0 warnings |
| rustfmt | `cargo fmt --check` | clean |
| Octet architecture | `rm -rf .octet/compiler-architecture/shards && cargo-octet check --workspace -- --all-targets --all-features` | **Architecture findings: 0** (receipt `b38205ce2178ef089acd6a1125be5387d612173d4c50d8e28c6a681b042de11a`); lint phase warning-only (pre-existing production findings and DX test-file style warnings only) |

Build evidence: the re-run tests/clippy compile the whole workspace
all-targets (exit 0); the dedicated build command and its outcome are in
[m3-kernel-gates.md](m3-kernel-gates.md) (exit 0, 0 errors).

## Proof floor on the regenerated v1 kernel (re-run)

The M2 gates are the floor until the v1 gates supersede them at archive
time. The regenerated tree (re-extraction under the pinned pair, task
2.4's regeneration duty) keeps them green:

```console
$ verification/m2-proof-gate.sh      # pinned lean 4.31.0 on PATH
[m2-gate] PASS TOOLCHAIN … PASS (all checks)
$ verification/m2-coverage-gate.sh
[m2-coverage] PASS COVERAGE-JOIN (total=515 classified=515 extracted=491
  proved=1 modeled=18 excepted=5 open=0) … PASS (all checks)
```

The twelve M2 refinement theorems bind the regenerated v1
`noble_kernel.acceptance.check` — the kernel extension is
behavior-preserving on the whole v0 fixture matrix
([m3-kernel-gates.md](m3-kernel-gates.md) "Regeneration re-binding").

## Independent M3 acceptance scope

What the M3 acceptance sequence claims, and what it does not:

- **Claimed:** the named `bootstrap fragment v1` — exactly the 23-entry
  word table (contracts per [m3-fragment.md](m3-fragment.md)), the
  B-CHECK-02 explicit rejections (recursive definition dependencies,
  user-declared recursive schemas), the B-CHECK-05 explicit rejections
  (self- and mutually-referential substitution witnesses), the
  B-CHECK-06 eliminator controls (payload exposure, explicit union
  joins, branch joins), witness verification per the M2
  instantiation-verification decision, and the extended gates and
  refusal matrix.
- **Not claimed:** inference/unification, generalization, recursion
  support, live resources, evaluation and recipe semantics, Wasm
  lowering or the roadmap's `wasm-feasibility` milestone, host
  boundaries, conformance-ledger scenario execution, and whole-kernel
  verification. Every result about fragment v1 MUST keep its exact
  subset label (V-MODEL-01, V-GATE-05); the enumerated omissions are in
  [m3-fragment.md](m3-fragment.md) "Omissions".
- **Proof depth:** fragment v1's theorem set is owned by the `proofs/m3`
  root (tasks 5.1–5.4) and recorded in
  [m3-proof-evidence.md](m3-proof-evidence.md). The claims below stand
  on that theorem set, on the v1 gates and refusal matrices, and on the
  M2 floor, which stays green. The `proofs/m3` root builds with no
  `sorry`, its theorem inventory is the gate's required-theorem list,
  and each name's axiom set is checked against its class.

## Gate-extension status (updated as tasks complete)

| Gate / matrix | Status |
|---|---|
| `verification/m3-proof-gate.sh` | done (task 6.1, evidence below) |
| `verification/m3-coverage-gate.sh` | done (task 6.1, evidence below) |
| `verification/m3-proof-gate-refusals.sh` | done, all six mutations refused (task 6.2, evidence below) |
| `verification/m3-coverage-refusals.sh` | done, all six mutations refused (task 6.2, evidence below) |
| per-word doc-example coverage assertion | done (task 3.2, evidence below) |
| DX-01 rejection-family controls | done (task 3.3, evidence below) |

## Task 6.1 evidence: the m3 gates on the final tree (strict tier)

`verification/m3-proof-gate.sh` and `verification/m3-coverage-gate.sh`
succeed the M2 gates (never weakening them). Each has two tiers: the
strict tier runs the full battery against `proofs/m3` (the v1
required-theorem inventory — the theorem contract of
[m3-proof-evidence.md](m3-proof-evidence.md) §6, enumerated in the gate
itself — the per-name axiom policy, semantic subject binding;
the coverage battery over `verification/m3-coverage.json` with the
`m3-coverage-join.mjs` join tool, whose citation partition routes each
citation namespace to the checker whose import surface can see it); the
handoff tier remains as the M2-floor mode for a root that does not
build, and is not engaged on this tree.

Both gates pass in the strict tier on the final tree (pinned lean
4.31.0), each with the whole battery green:

```console
$ verification/m3-proof-gate.sh
[m3-gate] PASS TOOLCHAIN (lean 4.31.0)
[m3-gate] PASS BUILD (lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel)
[m3-gate] PASS NO-SORRY (no 'sorry' outside .lake)
[m3-gate] PASS GENERATED-KERNEL (Aeneas entry point, Funs.lean, acceptance.check)
[m3-gate] PASS REFINEMENT-SUBJECT (NobleM3/Refine.lean and NobleM2/Embed.lean import NobleKernel)
[m3-gate] PASS REQUIRED-THEOREMS (the v1 theorem inventory of m3-proof-evidence.md §6, axiom policy, subject binding)
[m3-gate] PASS EXTERNAL-MODELS (no template axioms in FunsExternal/TypesExternal)
[m3-gate] PASS (strict tier, all checks) — proof root: …/proofs/m3

$ verification/m3-coverage-gate.sh
[m3-coverage] PASS SETUP (bun + lean 4.31.0)
[m3-coverage] PASS BUILD (lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel)
[m3-coverage] PASS COVERAGE-DERIVE (Funs.lean + FunsExternal.lean defs with rust names and source spans; keyword parity holds)
[m3-coverage] PASS COVERAGE-JOIN (total=515 classified=515 extracted=491
  proved=1 modeled=18 excepted=5 open=0)
[m3-coverage] PASS EXCEPTED-DISCLOSURE (5 excepted entries == #[charon::opaque] declarations in the crate)
[m3-coverage] PASS OPEN-SUBJECT (0 open entries; every generated function is classified)
[m3-coverage] PASS ENV-BINDING (constants bound in the elaborated environment; citations are theorems mentioning their constants)
  info: ENV-A-PASS 515 classified constants bound to their modules; 108 elaborator matchers tolerated
  info: ENV-B-PASS 13 citations verified; ENV-C-PASS 16 citations verified
[m3-coverage] SUMMARY total=515 classified=515 extracted=491 proved=1 modeled=18 excepted=5 open=0
[m3-coverage] PASS (all checks) — subject: …/proofs/m3
```

`verification/m3-coverage.json` carries the reviewed 515-function
classification over the regenerated module (extracted 491, proved 1,
modeled 18, excepted 5, open 0). The `acceptance.check` entry's
citations are the twelve M2 fixture refinements plus the sixteen v1
theorems whose statements bind the extracted constant
(`outcomes_via_refinement_v1`, `accepted_via_refinement_v1`, the
B-CHECK-02/05 rejection agreements, and the two decision-path rows);
checker A binds all 515 classified constants to their generated
modules, and every citation is verified exactly once across the three
checkers. The M2 floor gate and the M2 coverage battery stay green on
`proofs/m2` with the same record counts.

Two gate defects were found and fixed while running this matrix; both
were *under-refusals*, i.e. the positive baseline was green while a
mutation passed:

1. The coverage join's citation routing partitioned *entries* rather
   than citations, so a site citing both an M2-floor theorem and an M3
   theorem was routed to one checker and its other citation was never
   verified (the `dangling-citation` mutation exposed it). The routing
   now partitions the citation sets, and the mutation is refused.
2. The gate's required-theorem list was a ten-name excerpt of the
   contract; it now enumerates the full inventory of
   [m3-proof-evidence.md](m3-proof-evidence.md) §6 (the
   `missing-per-word-theorem` mutation then exposed two further
   defects: `NobleM2.validateSchemas_ok_bounded` was named by the
   contract but never stated — now stated and proved — and four names
   were held to the wrong axiom class — now reclassified).

## Task 6.2 evidence: the v1 refusal matrices

Each matrix runs the positive baseline and then mutated scratch copies
(proof-root and crate copies share the lake package cache through hard
links; the repository tree is never touched). Every mutation must be
refused at its named check; the six proof-gate mutations and the six
coverage-gate mutations are all refused, and the baselines stay green.

```console
$ verification/m3-proof-gate-refusals.sh      # exit 0
== positive baseline ==
== mutation: proof-hole (expected refusal: NO-SORRY) ==
refused: gate exit 1 at NO-SORRY
== mutation: missing-per-word-theorem (expected refusal: REQUIRED-THEOREMS) ==
refused: gate exit 1 at REQUIRED-THEOREMS
== mutation: substituted-subject (expected refusal: GENERATED-KERNEL) ==
refused: gate exit 1 at GENERATED-KERNEL
== mutation: unexplained-external (expected refusal: EXTERNAL-MODELS) ==
refused: gate exit 1 at EXTERNAL-MODELS
== mutation: removed-eliminator-oracle-arm (expected refusal: HARNESS) ==
refused: harness exit 1
== mutation: schema-revision-regression (expected refusal: HARNESS) ==
refused: harness exit 1
== all refusals demonstrated ==

$ verification/m3-coverage-refusals.sh          # exit 0
== positive baseline ==
== mutation: missing-per-word-coverage-case (expected refusal: UNCLASSIFIED) ==
refused at UNCLASSIFIED (gate exit 1)
== mutation: unclassified-def (expected refusal: UNCLASSIFIED) ==
refused at UNCLASSIFIED (gate exit 1)
== mutation: dangling-citation (expected refusal: CITATION) ==
refused at CITATION (gate exit 1)
== mutation: misbound-citation (expected refusal: CITATION-BINDING) ==
refused at CITATION-BINDING (gate exit 1)
== mutation: stale-entry (expected refusal: STALE-ENTRY) ==
refused at STALE-ENTRY (gate exit 1)
== mutation: bad-status (expected refusal: STATUS) ==
refused at STATUS (gate exit 1)
== all refusals demonstrated ==
```

The M2 refusal matrices stay green as the floor (four proof-gate
mutations and five coverage-gate mutations, each refused at its named
check), so the v1 matrices extend rather than replace them.

## Task 7.1 evidence: the full control matrix on the final tree

All commands run from the implementation-worktree root on the final tree
with the pinned toolchain
(`/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21`,
`CARGO_HOME=/tmp/cargo-home`; octet
`/nix/store/vj0sg5rn291z78im7gd2cqzqzd0bq3zy-cargo-octet-0.1.0/bin`;
lean 4.31.0 on PATH for the proof gates). Sequence and outcomes:

| Step | Command | Outcome |
|---|---|---|
| Build | `cargo build --offline --workspace --all-targets` | exit 0 |
| Kernel tests | `cargo test --offline -p noble-kernel` | 9 suites all ok (0/11/2/2/9/6/3/11/0) |
| Workspace tests | `cargo test --offline --workspace` | all suites ok |
| Clippy | `cargo clippy --offline --workspace --all-targets --all-features -- -D warnings` | exit 0, 0 warnings |
| rustfmt | `cargo fmt --check` | clean (after the harness's array-formatting fix) |
| Octet architecture | `rm -rf .octet/compiler-architecture/shards && cargo-octet check --workspace -- --all-targets --all-features` | 0 errors, **Architecture findings: 0**, receipt `c16e2583efea75d8012fa35a769ab04acd7bbe73f840a40a44b39345c6b5e753` |
| Cairn validation | `bun tools/cairn.mjs validate --root .` | exit 0 |
| Cairn gates | `bun tools/cairn.mjs gate {proposal,design,tasks} m3-checker-coverage --root .` | exit 0 each |
| M2 floor | `verification/m2-proof-gate.sh` | PASS (all checks) |
| M2 floor | `verification/m2-coverage-gate.sh` | PASS (all checks), total=515 open=0 |
| M2 floor | `verification/m2-proof-gate-refusals.sh` | baseline passes, four mutations refused |
| M2 floor | `verification/m2-coverage-refusals.sh` | five mutations refused |
| v1 gate | `verification/m3-proof-gate.sh` | PASS (strict tier, all checks) |
| v1 gate | `verification/m3-coverage-gate.sh` | PASS (all checks), total=515 open=0 |
| v1 refusals | `verification/m3-proof-gate-refusals.sh` | six mutations refused |
| v1 refusals | `verification/m3-coverage-refusals.sh` | six mutations refused |
| Extraction | the regeneration loop from `proofs/m2/NobleKernel.lean`'s header | charon exit 0, aeneas exit 0, emitted `Types.lean`/`Funs.lean` byte-identical to the checked-in root ([m3-extraction-probe.md](m3-extraction-probe.md)) |

The M1 reference evidence (the fifteen-phase acceptance run retained
under `.pi/m1-acceptance/`, tree `4420049e`) is unchanged by this change
and is not re-executed here; the M2 gates above are the M2 reference
checks on this tree, and both stay green.

## Task 3.3 evidence: DX-01 controls for the v1 rejection families

`tests/dx01/v1.rs` (module `v1` of the `dx01` test target) extends the
DX-01 diagnostic controls to the new rejection families; the extended
case list:

- `dx01_recursive_dependency_names_the_definition_identity` — a
  recursive definition dependency rejects as `unsupported` before any
  body check, naming the definition identity
  (`UnsupportedKind::RecursiveDependency(Definition(23))`).
- `dx01_recursive_schema_names_the_declaration` — a user-declared
  recursive schema rejects as `unsupported`, naming the declaration
  (`UnsupportedKind::RecursiveSchema(SchemaId(7))`).
- `dx01_cyclic_witness_names_the_constraint_and_reports_unavailable_provenance`
  — a self-referential witness rejects as `invalid` with
  `constraint = CyclicWitness` at the failing node, provenance
  unavailable.
- `dx01_swap_witness_cycle_inside_a_body_names_the_node` — the cycle
  inside a compound body names the swap node (`NodeId(1)`), not the body.
- `dx01_case_join_names_the_word_and_both_branch_stacks` — the `case`
  join diagnostic names the word and carries both branch programs in
  `expected` (claimed equal join) and `actual`.
- `dx01_list_case_join_names_the_word_and_both_branch_stacks` — the
  same shape for `list.case`'s nil/cons branches.

Wherever a `Diagnostic` exists, `provenance_available == false` is
asserted (unavailable, never invented); the unsupported rejections
carry their identity in the kind and issue no host request (B-RESULT-02).

```console
$ cargo test --offline -p noble-kernel --test dx01
test result: ok. 9 passed; 0 failed; ... (3 M2 variants + 6 v1-family controls)
```

## Task 3.2 evidence: per-word doc examples and the coverage assertion

`verification/m3-docexamples.md` adds 34 executed `noble-check`
examples: one positive example per word (23) and one rejection per
constrained word (dup/drop/quote eligibility, dip/compose/run/case/if/
list.case latent effects and joins, test.emit effect inclusion, plus an
`=` shape rejection). The harness (`tests/docexamples.rs`) now scans the
v1 fragment and examples documents and asserts per-word coverage:

```console
$ cargo test --offline -p noble-kernel --test docexamples -- --nocapture
docexamples: 42 noble-check examples executed against the actual checker,
3 illustrative fenced blocks ignored, word coverage 23/23
```

The coverage assertion fails by mutation: deleting the `swap` examples
from a scratch copy of `m3-docexamples.md` and re-running the suite
gives

```console
test documented_noble_check_examples_run_through_the_actual_checker ... FAILED
Error: "words without an executed noble-check example: swap"
test result: FAILED. 1 passed; 1 failed; ... (exit 101)
```

and the restored file is green again (2 passed). The illustrative
control still proves untagged blocks stay unexecuted.
