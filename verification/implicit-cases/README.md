# IMPL-CASE acceptance gate

`gate.mjs` builds the production `noble` binary from a clean checkout and runs IMPL-CASE-01..20
(`specs/conformance/implicit-cases.json`) through `noble verify-module`; IMPL-CASE-20 runs
through framed declared-module `noble session`s. Every positive case is accepted only by the
unchanged explicit checker and the pinned Lean consumer (`independent_recheck`), with the
repository's `proofs/mc1` library; every refusal is observed with its outcome, diagnostic code,
category and message. The receipt binds the source snapshot, the binary (hashed before and after
the run), every fixture's SHA-256, every command line and every raw output. Promotion verdicts
(`promotable`, `unmet`, `failed`) are derived from the observations: a case is promotable only
when every registered expectation was observed. It promotes nothing by itself.

Seven cases run under recorded amendments (`amendments` in the case file; integrator decision 37,
accepted by the owner as decision 38): IMPL-CASE-12, 13, 14, 16, 18, 19 and 20 each state what
their design input could not observe and what the gate observes instead.

IMPL-CASE-17 verifies P1, P2 and P3 100 times each in separate processes, alternating two
dependency-module orders, and requires one byte-identical `elaboration` record per pair.
IMPL-CASE-18 reads each budget's needs from the accepted run (the elaboration's own charge and
the whole request's) and observes one below the elaboration's need, one below the request's and
the exact need.

## Retained runs

| Run | Command | Result |
|---|---|---|
| [`runs/20261122-b5`](runs/20261122-b5/acceptance.json) | `node verification/implicit-cases/gate.mjs --out <new dir> --determinism-runs 100 --jobs 6` at commit `b4c63a1e` (Cairn batch 5) | `completed`, verdict `all-passed`: IMPL-CASE-01..20 promotable. Receipt sha256 `1641c70c…`, source revision `sha256:ed0941eb…`, binary `38d1e33d…` before and after, projection `6df4a693…` |

The binary is not retained; its digest is in the receipt. The promoted case records cite the
receipt. Proof stays open for every case: unification uniqueness and Rust elaborator refinement
(task 4) and source-to-Lean translation (V-INTRINSIC-01) are separate obligations.
