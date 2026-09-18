# M3 acceptance control run (run-so-far, 2026-09-18)

Scope: the `m3-checker-coverage` change — bootstrap fragment v1 (the
23-entry word table, the B-CHECK-02 recursion/schema rejections, the
B-CHECK-05 cyclic-substitution rejections, and the extended controls).
This file records the baseline and gate matrix run so far; task 7.1
extends it to the full end-to-end control matrix. Baseline tree at first
recording: `c668dac` (kernel v1 `53f2fcf`, re-extraction `be7d986`,
package `50807a9`, proof-root commits through `c668dac`).
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

| Gate | Command | Result (re-run 2026-09-18) |
|---|---|---|
| Tests | `cargo test --offline -p noble-kernel` | 9 suites all ok: acceptance 11, budget 2, docexamples 2, dx01 3, fragment 6, property 2, recursion-schema-cycle 11 (+ lib/doctests 0) |
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
  root (tasks 5.1–5.4). Until those theorems land and
  `verification/m3-proof-evidence.md` records them, the acceptance
  claims stand on the M2 floor above plus the v1 kernel controls; the
  v1 gates record the handoff explicitly.

## Gate-extension status (updated as tasks complete)

| Gate / matrix | Status |
|---|---|
| `verification/m3-proof-gate.sh` | open (task 6.1) |
| `verification/m3-coverage-gate.sh` | open (task 6.1) |
| `verification/m3-proof-gate-refusals.sh` | open (task 6.2) |
| `verification/m3-coverage-refusals.sh` | open (task 6.2) |
| per-word doc-example coverage assertion | done (task 3.2, evidence below) |
| DX-01 rejection-family controls | done (task 3.3, evidence below) |

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
