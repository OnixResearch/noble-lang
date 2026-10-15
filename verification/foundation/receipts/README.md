# Retained foundation receipts (issue #15)

Each subdirectory holds the receipts of one clean-source run, named by
source commit. The raw run directories (emitted `.noble`/`.wat`/`.wasm`,
Lean audit files, sandbox outputs) stay outside the repository. `index.json`
records their SHA-256.

`ce2e5ce/` contains these files. Its source tree is `b20649ef1ed1`, and the
runs used the commands in `proofs/foundation/README.md`.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim `check.mjs` receipt: 111 theorems, printed statements and axioms |
| `slice-receipt.json` | verbatim `slice.mjs` receipt for the Phase 3 chain and its refusal controls |
| `differential-summary.json` | `differential.mjs` receipt with the 163 per-case rows removed. `compacted_from.sha256` names the raw file |
| `index.json` | raw-file hashes and a result summary |

A receipt in this directory is evidence only for the commit and tree it
names. It is stale for every later commit, and that includes the commit
that adds it. `differential.mjs --verify` therefore refuses it on a
different tree, which is the intended behavior. Re-run the commands to
obtain current evidence.
