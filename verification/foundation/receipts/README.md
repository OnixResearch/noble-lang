# Retained foundation receipts (issue #15)

Each subdirectory holds the receipts of one clean-source run, named by
source commit. The raw run directories (emitted `.noble`/`.wat`/`.wasm`,
Lean audit files, sandbox outputs) stay outside the repository. `index.json`
records their SHA-256.

## 898cfe1 — original foundation run

`898cfe1/` contains these files. Its source tree is `d11cd3c70621`, and it is the
clean run of `verification/foundation/ci.sh`, which reported `overall=0`. This run
supersedes the earlier `ce2e5ce` copy.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim `check.mjs` receipt: 142 theorems, printed statements and axioms |
| `slice-receipt.json` | verbatim `slice.mjs` receipt for the Phase 3 chain and its refusal controls |
| `differential-summary.json` | `differential.mjs` receipt with the 163 per-case rows removed. `compacted_from.sha256` names the raw file |
| `index.json` | raw-file hashes and a result summary |

A receipt in this directory is evidence only for the commit and tree it
names. It is stale for every later commit, and that includes the commit
that adds it. `differential.mjs --verify` therefore refuses it on a
different tree, which is the intended behavior. Re-run the commands to
obtain current evidence.

## 3d5ccc2 — fixed-input branch EMI (issue #53)

[`3d5ccc2/`](3d5ccc2/) records clean source commit
`3d5ccc2770d3928a0071c143e86fdc6abbad22d4`, tree
`51f9f9b053e25c75f7547f1ec948da3ced14fe78`. The actual invocation was
`verification/foundation/ci.sh NEW_OUTPUT_DIR --skip-slice`; its
[`ci-summary.txt`](3d5ccc2/ci-summary.txt) reports `overall=0`.
The host-service Phase 3 slice was explicitly **skipped**. No fresh slice
result is claimed, and the historical `898cfe1` packet is unchanged.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim Lean inventory/axiom audit: 148 theorems, including the six new branch-model theorems |
| `differential-summary.json` | existing arithmetic comparator: 163 matching rows, all seven controls detected, and the 1,000-oracle/200-malformed kernel checker lanes passed; raw per-case rows remain external |
| `emi.json` | verbatim exact-subject/source/tool/model/artifact receipt for all 19 generated pairs, including the three eligibility refusals |
| `emi-replay.json` | verbatim independent admission, fresh Lean checks and actual wrong-lowering discrepancy replay |
| `minimal-control.json` | replayable fixed-input original/variant pair with the intentional lowering fault |
| `*-verify.json` | actual clean-tree verifier stdout; all fresh receipt checks passed |
| `stale-receipt-refused.json` | actual refusal of the older arithmetic receipt on the new tree |
| `nix-*` | verbatim outputs from the five selected Nix acceptance gates |
| `index.json` | raw and retained-file hashes, result summary, and selected Nix gate evidence |

The EMI run admitted 16 pairs: 12 scalar `if` and four I64 sum `case`.
Each of the four transformation kinds has four eligible pairs. Original
and variant executed with opt off and on: 64 nominal executions, all with
the independently checked ordered result, preserved prefix, sufficient
headroom, and empty host trace/request count. All seven EMI controls were
detected, including the real wrong-selected-branch Wasm lowering in both
optimization modes.

The lowering-control shrink used 13 attempts, retained 12 reductions, and
reached a local minimum without exhausting the 64-attempt bound. It
preserved the complete ordered interface and reduced the mutation site
from 2 to 0. The retained minimal discrepancy replayed successfully.

The exact source commit also passed `octet`, `source-coverage`,
`native-assurance`, `policy`, and `source-inventory-controls`. Octet
reported zero findings, warnings and errors; source coverage matched all
12,053 reviewed subjects. These selected gates are not the entire flake
check set, and inventory/native-assurance success is not refinement or
soundness evidence.

These are finite fixed-input model and execution results, not universal
program equivalence or a compiler theorem. Rust renderer/lowering
refinement remains open; downloaded Mathlib `.olean` files remain a trust
assumption. Like every packet above, this packet is stale for the commit
that adds it: the successful verifier outputs name the tested source tree,
not the later evidence-only commit.
