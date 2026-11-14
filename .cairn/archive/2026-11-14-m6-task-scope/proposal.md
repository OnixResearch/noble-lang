## Why

`async_tasks::Table` already retains each M6 task record, separates `prepare`
from infallible `commit`, and keeps result delivery, cancellation
acknowledgement, native stop, pin settlement and cleanup as distinct decisions.
`Task` is a move-only capability whose handle may be copied as identity but not
as the ability to receive a result. `ExecutionScope` in the worker owns exactly
one selected task and its issued cell.

None of that defines ownership of a *group*: which child belongs to which scope,
in which order results are reported, what happens when the second admission is
refused, which failure is the group's failure, or when a group may close. RA-CLEAN-04
already constrains any later scoped cleanup convenience: order, declared effects
and every ownership outcome must be specified, host retirement on abnormal exit
must be preserved, outstanding native pins must stay accounted, and shielding
cancellation must not imply eventual host completion.

This change supplies only that composition layer. It adds no kernel surface and
changes no per-task transition.

r[RA-GROUP-01] r[RA-GROUP-02] r[RA-GROUP-03]
r[RA-GROUP-04] r[RA-GROUP-05] r[RA-GROUP-06]

## What Changes

- **ADDED** `RA-GROUP-01`: the two-child bound, the host-issued scope identity,
  retained membership as ordered child references, attachment only through the
  scope's own move-only admitted value, and refusal of duplicate, foreign, stale
  and post-close attachment.
- **ADDED** `RA-GROUP-02`: incremental admission forced by the exclusive
  `Admission` borrow; a refused admission preserves every previously committed
  child and returns caller inputs unchanged; no whole-group preflight claim; no
  rollback of native work or consumed authorization; capacity charged once per
  child under the existing `Request`/`Limits` accounting.
- **ADDED** `RA-GROUP-03`: results in attachment order regardless of completion
  order, delivery only after the table's own delivery, the first abnormal outcome
  in the host's serialized order as the sticky primary failure, cancellation
  acknowledgement recorded as neither result nor settlement, and membership
  released exactly once per member.
- **ADDED** `RA-GROUP-04`: abnormal-exit retirement per undelivered child, native
  request/observation/pin settlement/cleanup kept separate, an unsettled child
  keeping the scope unsettled, and retirement inherited by the host scope
  registry when a wrapper is dropped.
- **ADDED** `RA-GROUP-05`: final close requiring every member to satisfy the
  existing M6 finalization contract and membership accounting to be settled,
  idempotent repeated close, and no manufactured completion.
- **ADDED** `RA-GROUP-06`: the finite two-child reference model over the completed
  #46 interleaving study, compared with an adapter driving the actual Rust M6
  table API, with scope policy, transition composition and adapter correspondence
  named separately and model proofs, runtime tests and host assumptions kept
  apart.

## Impact

- Rust source subjects: one test-only module group under
  `crates/noble-kernel/tests/m6_interleaving/`, added by `#[path]` to the
  existing `m6-interleaving` target. No production Rust subject changes, so no
  production source-inventory or Octet architecture change.
- Affected specs: `.cairn/specs/resource-adapters/spec.md` (delta only).
- Lean subjects: one new foundation module `NobleFoundation/TaskScope.lean`
  importing the existing `NobleFoundation/TaskInterleaving.lean` model.
- Coordination: #46 keeps the interleaving study, #67 keeps binary endpoints and
  runtime correspondence, #50 keeps the RegionProtocol trace relation, ADAPT-18
  keeps the resource-free pure fork/join study. None is reopened.
- Adoption: none. The scope is a host-side pilot, not a selected runtime surface.

## Non-goals

No guest spawn syntax, channels, endpoint protocols, new effect protocol
implementation, actor supervision, remote transport, arbitrary nesting, kernel
re-extraction, or claims of eventual native completion or cleanup after host
death. No production `ExecutionScope` change and no replacement of the per-task
state machine.
