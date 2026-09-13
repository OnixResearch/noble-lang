# Noble Project Handoff Set

Date: 2026-09-12
Status: project handoff snapshot; not itself a normative language specification

This package is intended to let another engineer or agent continue Noble without replaying the full conversation history.

## Read first

1. `PROJECT-HANDOFF.md` — current architecture and project state.
2. `CURRENT-DECISIONS.md` — decisions that should be treated as selected direction.
3. `SOURCE-OF-TRUTH.md` — which documents are baseline, working amendments, historical, or not yet formally merged.
4. `MERGE-PLAN.md` — how to create one canonical specification without overwriting newer work.
5. `NEXT-WORK.md` — continuation order and milestone criteria.
6. `STATUS.json` — machine-readable snapshot.

## Critical warning

There is **no single canonical SPEC-0001 file yet that contains every newest decision simultaneously**. The project baseline predates several later decisions; the safety and WIT/WASI workstreams each produced separate integration previews; the Syndicate/Synit/Preserves concurrency direction is newer still and has not yet been formalized into its own merged specification package.

Do not replace the project spec wholesale with any one preview. Reconcile the workstreams deliberately using `MERGE-PLAN.md`.

## Current surface terminology

The selected surface direction uses:

- `run` instead of older `call`;
- `reflect` instead of older `reify`;
- `#` for comments rather than the older baseline `//` direction.

Older source documents retain their historical spelling and should not be silently rewritten when cited as baselines.
