# Noble Source-of-Truth Map

## A. Baseline project sources

These are preserved project-backed documents and should be treated as historical/normative baselines for the revision they name, not silently rewritten to reflect later conversation decisions:

- `reference/baseline/rfc-0001-minimal-programs-as-data.md`
- `reference/baseline/SPEC-0001-baseline.md`
- `reference/baseline/SPEC-0001-decisions.md`
- `reference/baseline/VERIFICATION.md`
- `reference/baseline/VERIFICATION-TOOLCHAIN.md`
- `reference/baseline/PROGRAM-CONTRACTS.md`
- `reference/baseline/VERIFICATION-AMENDMENT.md`
- `reference/baseline/REFERENCES-BEND-AMBIENT.md`

Important baseline facts:

- older surface spelling uses `call` / `reify`;
- the older lexical draft uses `//` comments;
- WIT/component lowering is described as open work;
- standard concurrency is not yet defined there as Syndicate.

Do not mistake those historical spellings/open items for the latest selected direction.

## B. Newer safety workstream

Located under `reference/working-safety/`.

Status:

- architecture selected;
- working normative draft;
- 37 safety requirements;
- 15 conformance scenarios;
- 9 proof/implementation obligations;
- structural validation passed;
- runtime tests not executed;
- formal proofs not completed.

The safety preview is **not** a canonical merged replacement for every other preview.

## C. Newer WIT/WASI workstream

Located under `reference/working-wit-wasi/`.

Status:

- architecture selected;
- working normative draft;
- 15 conformance scenarios;
- structural validation passed;
- all new scenarios not-run.

This work resolves the *architectural direction* of baseline O-07 toward Component Model + WIT + WASI, but implementation/type mapping/resource adapters/toolchain validation remain open.

The WIT/WASI preview is **not** a canonical merged replacement for the safety preview.

## D. Concurrency direction

The newest selected direction is Syndicate/Synit + Preserves, recorded in the handoff documents. There is not yet a standalone merged normative concurrency package in this handoff set reflecting that decision.

The older `Beam Learning References.txt` is retained under `reference/historical/` because it summarizes useful prior concurrency work, but its BEAM-first framing is superseded where it conflicts with the newer Syndicate decision.

## E. Choreography

The older choreography summary is retained under `reference/historical/`. Its core protocol ideas remain useful, but the runtime/projection layer must now be reconciled with Syndicate/Synit rather than assumed to be independent.

## F. Implementation status

No file in this handoff set is an authoritative current Rust/Lean repository test report. Prior conversation indicates implementation work occurred, but the exact state must be established from the actual repository and fresh test runs.

## G. Canonical-spec policy

Until a new integrated SPEC-0001 is produced, treat the following hierarchy as authoritative for *intent*:

1. latest explicit user decisions in the project conversation;
2. newer selected-decision registers (`DEC-S001`, `DEC-W001`) for their workstreams;
3. verification/core baseline documents for semantics not superseded by later decisions;
4. generated historical reference/concurrency/choreography packages for context only.

For normative publication, do not rely on this hierarchy indefinitely: perform the merge described in `MERGE-PLAN.md` and publish one revisioned integrated spec family.
