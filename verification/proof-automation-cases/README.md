# Proof-automation cases PAUTO-CASE-02..06

`gate.mjs` executes the counterexample and vacuity rows of the
`proof-automation-diagnostics` design table (task 4) on one `noble` binary built
with `cargo build --locked` from a clean checkout:

| Case | Route | Expected |
|---|---|---|
| PAUTO-CASE-02 | `goal-view` and `verify --proof` on `verification/mc1/wrap.noble-contract` | the goal view states two's-complement wrapping and the exact `BitVec 64` statement; `proved`, 0 |
| PAUTO-CASE-03 | `verify --counterexample boundary` on `verification/mc1/monotonic.noble-contract` | candidate `9223372036854775807` (wrapped); the reviewed refutation template accepted; the compiled run returns `-9223372036854775808`; `disproved`, 1, advertised as an executable counterexample |
| PAUTO-CASE-04 | `verify --counterexample input:5` on the same contract | the refutation template is refused; `unknown/candidate-model-only`, 3 |
| PAUTO-CASE-05 | `obligations`, `verify --proof`, `release` and `verify-module` with a literal `false` precondition | vacuity `suspected`; `proved/checked-vacuous`, 0; the owner-law release refuses with `owner-law-requires-mismatch`; rev-1 refuses with `error`, 2 |
| PAUTO-CASE-06 | a text literal in `ensures`; a two-input rev-1 contract | `unsupported`, 4, with the source span of the refused construct; no producer runs |

The boundary producer, its host evaluator and the supplied `input:N` candidate are
untrusted. A candidate is `disproved` only when the pinned Lean consumer accepts
`NobleContracts.exportedClaim_unaryI64_refutation` instantiated at that input
against the independently regenerated claim. The compiled run goes through the
ordinary isolated `noble run` path; it is `observed-execution` evidence and never
decides the outcome. A semantic/compiled disagreement is reported in the
`discrepancy` field for artifact correspondence (#10) and resolves nothing.

```sh
node verification/proof-automation-cases/gate.mjs --self-test
node verification/proof-automation-cases/gate.mjs --out NEW_DIRECTORY
```

The receipt (`acceptance.json`) binds the tested commit, the source snapshot of
the gate's scope, the design rows, the binary digest before and after the run,
every fixture, command line and raw report. It promotes nothing by itself.
Registration in `specs/conformance` and the requirement sync stay with the
change's review task.

## Retained runs

- [`runs/20261122-388fa4c6/acceptance.json`](runs/20261122-388fa4c6/acceptance.json)
  (`sha256:50c8fd78…`): clean commit `388fa4c6`, verdict `all-passed`,
  PAUTO-CASE-02..06 promotable, with the raw reports, fixtures and the honest
  increment build the release step used.
- [`runs/20261123-0fffc7d8/acceptance.json`](runs/20261123-0fffc7d8/acceptance.json)
  (`sha256:58878425…`): clean commit `0fffc7d8` (Cairn push B, after the merge
  onto the batches 4-6 head and the Aeneas-translatable unary-rule comparison of
  `6fe17303`), source revision `sha256:9f4f7dd7…`, verdict `all-passed`,
  PAUTO-CASE-02..06 promotable, with the same kinds of retained raw reports,
  fixtures and build.

## Proof holes and retained evidence (PAUTO-CASE-11, PAUTO-CASE-15)

[`holes-gate.mjs`](holes-gate.mjs) executes the retained-evidence and
editor-only proof-hole rows of the `proof-automation-diagnostics` design table
(task 3) on one `noble` binary built with `cargo build --locked` from a clean
checkout, keeping the PAUTO-CASE-02..06 lane and its retained `runs/` receipts
separate:

| Case | Route | Expected |
|---|---|---|
| PAUTO-CASE-11 | `verify-module --emit`, `goal-view --evidence`, `verify-module --evidence` on `increment` (`[ 1 + ]` -> `[ 2 + ]`), `model_revision` mismatch, and forged evidence | `verify-module --emit` records `noble-intrinsic-evidence/v1`; matching `goal-view --evidence` stays `incomplete`; changed subject or `model_revision` marks `goal-view` `stale` and refuses `verify-module --evidence` with `intrinsic-stale-proof`, 4 without modifying `evidence.json`; forged evidence refuses with `evidence-invalid`, 2 |
| PAUTO-CASE-15 | `verify-module` with undeclared `(use h)`, `goal-view --proof-holes` (`noble-proof-editor/v1`), misplaced transport holes, and raw `(editor-hole ...)` across admission and ordinary commands | undeclared `(use h)` refuses at `resolve` with `intrinsic-resolve-invalid`/`rejected-source`, 2 (or at `acceptance` with `intrinsic-proof-refused`/`rejected-term`, 2 when shadowed by a value binder); valid proof-hole transport reports `unknown/incomplete-hole`, 3 (`incomplete` even with matching `--evidence`); misplaced transport holes refuse with `proof-hole-transport`, 2; raw `(editor-hole ...)` refuses on every admission and ordinary route with `editor-only` and zero guest requests |

```sh
node verification/proof-automation-cases/holes-gate.mjs --self-test
node verification/proof-automation-cases/holes-gate.mjs --out NEW_DIRECTORY
```

Pass `--noble PATH` to reuse an externally built clean-source binary, or
`--allow-dirty` for local development checks (marked non-retainable in the
receipt). The receipt (`acceptance.json`, schema
`noble-proof-automation-holes-acceptance/v1`) binds the tested commit, scope
snapshot, design rows, binary digest before and after execution, all relative
runs/raw outputs, and checks for public-repo path hygiene.

### Retained holes runs

- [`runs-holes/20261124-b4791ff3/acceptance.json`](runs-holes/20261124-b4791ff3/acceptance.json)
  (`sha256:4139179f…`): clean commit `b4791ff3` (`tested_commit`
  `b4791ff32536cd2d9693c84b2b82ce6eb502e1a1`), `source_revision`
  `sha256:ef816430528dabb1c9ffe3fe5ecc1e247837edf22584dd2a12cebd9497cea395`,
  verdict `all-passed`, `PAUTO-CASE-11` and `PAUTO-CASE-15` promotable, with
  raw reports, fixtures, and emitted `noble-intrinsic-evidence/v1` artifacts.
