## Why

Issue #10 asks for the next bounded strengthening of the chain from an independently selected contract and checked proof to the exact artifact that runs. It also coordinates the gaps shared by #6–#9.

At `44329dc`, two correspondence routes exist but are never joined:
- **`noble admit-artifact`** compares final Wasm bytes with a host rebuild, but it takes no proof, claim or policy, and it does not compare tool hashes against pins.
- **`noble build --require-proof`** binds checked evidence to the source recipe, but its report records only artifact sizes. It records no tool or helper identity or optimization mode, and it re-reads the assembled bytes without a hash check.

PO-17 and PO-18 have no evidence. r[ACORR-ENV-01] r[ACORR-TB-01]

## What Changes

- Inventory the nine-edge assurance chain (owner law → source/deps → claim → proof → lowering → WAT/Wasm + optimization → loaded bytes → helpers/adapters → invocation). For each edge, list existing checks, identity bindings, trusted components, unsupported profiles and open obligations, and distinguish artifact identity from semantic preservation. r[ACORR-TB-01]
- Select one extension: the proof-bound artifact admission envelope PAA-1 for the MC2 pure quotation profile (optionally with a #8 v1 guard wrapper). It joins both routes; the consumer authenticates every field; refusal precedes instantiation; only the compared buffers are instantiated in a fresh engine. r[ACORR-ENV-01]
- Decide on a strengthened trusted build (policy-pinned rebuild plus final-byte equality) rather than bounded translation validation for this slice, with an explicit comparison and rationale. r[ACORR-TB-01]
- Specify cache invalidation, optimization-mode handling, erasure and guard non-erasure. r[ACORR-CACHE-01]
- Plan ACORR-CASE-01…12: reproducible positives and independently prepared negatives, while retaining the existing S-CASE-07, CONTRACT-13 and CONTRACT-22/26 controls. r[ACORR-CTRL-01]
- State proof obligations O1–O7 separately from hashes, tests and extraction, plus the residual assumptions, unsupported cases and cross-issue ownership (#1, #3, #4, #5, #6, #7, #8, #9, #15). r[ACORR-TB-01]

## Impact

- **Files**: only `.cairn/changes/artifact-correspondence-extension/`, with ADDED deltas in program-contracts (ACORR-ENV-01, ACORR-CACHE-01, ACORR-CTRL-01) and verification (ACORR-TB-01). There are no canonical spec, conformance, obligation, status or generated-view edits, and no changes to the S-CASE-07 or intrinsic-proof records.
- **Code**: none. Route A, Route B, the worker, report schemas, policy pins and the stale `runtime_assets` list are unchanged; the stale list is noted as a prerequisite.
- **Testing**: document validation only. No admission, control or proof evidence is produced or claimed.
