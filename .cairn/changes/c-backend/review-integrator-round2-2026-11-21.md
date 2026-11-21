# Integrator review, round 2: c-backend source-only slice (`cairn/cbackend-source`), 2026-11-21

**Reviewer:** the campaign integrator, under owner decision 28 (2026-11-21): reviewer agents are
unavailable, so the integrator reviews branches it did not implement. This is an integrator review,
not a fresh-context one. The owner keeps a veto before any push.

**Reviewed:** `2a0711ac` (round-1 fixes) and `950fcb80`/`e1263e81` (later slice commits) against the
fresh-context round-1 record `.cairn/changes/c-backend/review-2026-11-20-fd.md` (REVISE at
`8a0ec89e`). The slice agent wrote these commits; the integrator did not.

## Round-1 findings

| ID | Status at `2a0711ac` | Evidence |
|---|---|---|
| F-1 (major) Unit encoding in aggregates | **Resolved** | `c_backend.rs:496` renders `{"type":"Unit","value":null}`; `tests/c_backend.rs:693-698` asserts the exact Unit/Pair/Sum/List encodings. |
| F-2 (major) misdated result-form sentence | **Resolved** | Delta `spec.md:62` dates it to the source-only WIP text the `8c6ab3d0` mid-change sync applied. The `e76d8469` sentence at `:127` is the separate, already-accepted refusal-clause dating (c-backend packet review R-06). |
| F-3 (minor) CB-ADMIT-01 misdescribes the inputs | **Resolved, with new finding R2-1** | A dated CB-ADMIT-01 insertion now describes the descriptor walk, sealed snapshot and output binding. |
| F-4 (minor) status line exceeds item 6 | **Resolved** | Canonical `spec.md:17` reads "I64/Bool/Unit/Text-result source emission with internal Pair/Sum/List aggregates", the owner's item-6 scope; every word matches `c_backend.rs`. |
| F-5 (minor) stale view | **Resolved for this slice** | `cairn-specs` check at the branch reports only `safety` and `program-contracts`, which the round-1 reviewer attributed to the then-current wave1; on the merged tree the views must pass after `--write-views`. |
| F-6 (minor) reconciliation transcriptions | **Resolved** | `a36ffbe0…48ec2d`, `c_backend.rs:535`, `:67`. |
| F-7 (minor) output-directory window | **Open at `2a0711ac`** | The comment (`inputs.rs:254-257`), the CB-ADMIT-01 insertion and the task note still said the check refuses a directory replaced during creation. |
| F-8 (minor) stale status documents | **Partly open** | The proposal now dates its earlier sentences; `specs/ROADMAP.md:1552` still said canonical CB-BUILD-01 does not describe the source-only route. |
| N-1..N-5 (notes) | Addressed or recorded | N-1: the recheck now also covers link count, size and timestamps; N-3: refusal tests added for non-regular, oversized and over-deep inputs; N-5: re-pin and inventory renewal happen at integration. |

## New finding

**R2-1 (minor).** The CB-ADMIT-01 insertion, proposal and design said the recheck "compares identity
only, so an in-place rewrite of the same inode is not detected". The code (`inputs.rs:199, 212-219`)
requires the bytes read to equal `st_size` and rechecks device, inode, link count, size, and the
modification and change timestamps with nanoseconds. The documents understated the check and
overstated the residual.

## Corrections applied by the integrator

Commit `5caddfcc` on `cairn/cbackend-source-r2` (integrator-authored; the owner veto applies to it):

- R2-1: the insertion, proposal and design describe the actual recheck; the residual is an in-place
  rewrite that leaves the inode, link count, size and both timestamps unchanged.
- F-7: the `create_output` doc comment, the insertion, the design and the task note now state the
  window: the leaf name is re-opened once after `mkdirat`, so a writer of the parent can substitute
  the directory before that re-open. No code behaviour changes.
- F-8: the ROADMAP sentence now says canonical CB-BUILD-01 carries the source-only paragraph that
  the `8c6ab3d0` sync applied, and that the later disclosures await their reviewed sync.

## Verdict

**ACCEPT (integrator review, round 2), with the `5caddfcc` corrections.** Owner packet items 2 (FD
alignment) and 6 (status line) are reviewed; item 2's own text then authorizes the sync. Input
integrity, pins, the emitter, the runtime, the sandbox and the differential stay open, and no
native, admission or CB-CASE claim follows.
