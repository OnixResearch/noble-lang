## Context

Grounded at branch base `fed77835`; every cited line was re-read there.

- `crates/noble-kernel/src/async_tasks/mod.rs:1-11` declares the module contract:
  serialized ownership decisions independent of native storage and authority; the
  shell owns payloads and native provenance; `prepare` reserves and starts nothing
  and its exclusive borrow makes a later `commit` infallible; committed inputs
  belong to the task although the storage stays in the shell; cancellation
  acknowledges revoked guest access only, and native terminal observation, pin
  settlement, cleanup and finalization stay separate. `mod.rs:37` states that
  dropping a `Task` does not clean up.
- `crates/noble-kernel/src/async_tasks/table.rs:19-22` shows why group preflight
  is not available: `Admission<'a>` retains `table: &'a mut Table`, so a second
  preparation cannot exist while the first is alive.
- `crates/noble-kernel/src/async_tasks/table/admission.rs:9-59` is `prepare`:
  it validates the requested footprint, finds a free slot, checks the shared
  reservation, refuses generation exhaustion, and retains a `Pending` snapshot.
  `admit` (`:62-77`) returns the caller's inputs unchanged on rejection, and
  `Admission::commit` (`:104-129`) is the infallible local step that records the
  snapshot, advances the table generation and returns the move-only `Task`.
- `crates/noble-kernel/src/async_tasks/table/ownership.rs` keeps the decisions
  separate: `deliver` and `cancel` consume the move-only task; `retire` takes a
  raw handle and a failure; `cleanup` acknowledges a completed obligation mask;
  `finish` releases reservations only after native stop, all pins and every
  remaining obligation have settled.
- `crates/noble-kernel/src/async_tasks/table/inspection.rs:22,30` provides the
  read-only `snapshot` and `observation` reports the adapter compares against.
- `crates/noble-kernel/tests/m6-interleaving.rs` already drives the real table:
  `:26` shows `finish` refusing while native work runs and `cleanup` refusing with
  pins outstanding; `:78` shows a stale generation refused; `:112` shows a
  duplicate callback charging nothing; `:156` shows the sticky first abnormal
  failure surviving a later success. Its fixtures are `m6_interleaving/support.rs`
  (limits, request, completion) and `m6_interleaving/capacity.rs` (admission
  enabledness depends on release).
- `crates/noble-cli/src/core/worker_program_guest.rs:686-716` is the existing
  single-invocation `ExecutionScope`: it owns one task and one issued cell, and its
  `Drop` retires the task. That is the retirement precedent a scope wrapper must
  not lose.
- `proofs/foundation/NobleFoundation/TaskInterleaving.lean` (issue #46) is the
  completed interleaving study: `Table` (`:63`), `step` (`:147`), `reaches`
  (`:527`), `observe` (`:597`), the declared alphabet and enumeration
  (`:810`, `:828`, `:834`), `independent` (`:1100`) and the refutation of
  "different ids are independent" (`differentIds` `:1620`,
  `different_ids_rule_is_unsound` `:1624`).
- `specs/RESOURCE-ADAPTERS.md:142` (canonical `.cairn/specs/resource-adapters/spec.md`)
  is RA-CLEAN-04: any later scoped cleanup convenience must specify cleanup order,
  declared effects and all ownership outcomes, must preserve host retirement on
  abnormal exit and accounting for outstanding native pins, and must not let
  shielding cancellation imply eventual host completion.

The gap is a composition layer, not a new task machine. A table of independent
tasks defines no group ownership, group result order, partial-admission policy or
final-close semantics, and the kernel must not grow one: `async_tasks` is part of
the extracted Lean translation (`proofs/m3/NobleKernel/Funs.lean`) that
`proofs/m6/M6Async.lean` builds on, so changing it forces a re-extraction.

## Decisions

### Decision: membership is a host-issued scope identity plus (slot, generation), granted only by the scope's own move-only task

**Choice:** a `ScopeId` is host-issued and retained in the host registry. A member
is the pair (slot, generation) of a child that was admitted *through that scope*;
attachment appends to an ordered membership list bounded at two. Attachment
consumes the scope's own child value (in the pilot: the `Admitted<T>` returned by
`Table::admit`). A copied `Handle` or `Callback` can name a child but cannot
attach, deliver, cancel or release it. Duplicate, foreign-table, stale-generation,
post-close and fabricated membership are refused, and the refusal returns the
offered task unchanged.

**Rationale:** this mirrors the kernel's rule that handle identity is not
authority (`mod.rs:64-70`, `Task::handle`). It also matches RA-CLEAN-04: an
ownership outcome must be explicit, so no path may obtain membership from a copy
or a serialized descriptor.

**Reopen conditions:** a use case that must attach a child the scope did not
admit; a kernel API that mints membership from a raw handle; or an owner decision
that a scope may be nested.

### Decision: admission is incremental, and a refused second admission keeps the first child

**Choice:** one child at a time, at the host's serialized boundary, with at most
one scope-issued value awaiting attachment. A later `admit` is refused until
that move-only value is attached; dropping the scope-issued value transfers its
retirement to the host registry and leaves the scope fail-closed. When the
second table admission is refused, the first child stays a member with its
footprint, pins and native obligations unchanged, and the caller's second input
comes back unchanged (the kernel already does this: `admit` returns
`Rejected { error, input }`).

**Rationale:** `Admission<'a>` holds `table: &'a mut Table`, so whole-group
preflight is not expressible without a new kernel API — and a new kernel API
forces the re-extraction this change excludes. Refusal also cannot be described
as rollback: nothing in the kernel can reconstruct caller ownership after commit,
and no rollback of native work or of a consumed authorization is claimed.

**Reopen conditions:** a kernel admission API that can preflight several requests
under one borrow.

### Decision: results in attachment order, primary failure is the first abnormal outcome in the host's serialized order, and it sticks

**Choice:** the scope reports a child's result only after the table delivered it,
and it reports results in attachment order regardless of completion order. The
scope's primary failure is the first abnormal outcome in the host's serialized
order (the same order in which it issued the table decisions); later successes,
later failures, late completions and duplicate callbacks do not replace it.
Cancellation acknowledgement is recorded as neither a result nor a settlement.

**Rationale:** the kernel already has the per-task sticky failure
(`m6-interleaving.rs:156` and `abnormal_failure_survives_a_later_success`), and a
group-level failure that was replaced by a later success would report a completed
group after an abnormal exit. Attachment order is the only order the host can
justify without inspecting native completion.

**Reopen conditions:** a consumer that requires completion order; a kernel rule
that lets a delivered result be re-delivered.

### Decision: close requires every member to have finalized, repeated close is idempotent, membership is released exactly once

**Choice:** close succeeds only when every member's `finish` succeeded — native
stopped, pins settled, every remaining obligation acknowledged — and every member
has been released exactly once. A repeated close after success is idempotent and
releases nothing again; an early close is refused and leaves the scope able to
settle. Close never manufactures completion, delivery or native stop.

**Rationale:** `finish` is the kernel's finalization contract and refuses while
`finalized` is unset (`ownership.rs`, `transition/cleanup.rs`). A close that
ignored it would report settlement for outstanding native debt, which RA-CLEAN-04
forbids. Exactly-once release keeps the scope's own accounting from double
crediting capacity.

**Reopen conditions:** a kernel close/batch API; a scope that must close with
members still running (which would need its own cancellation-and-wait policy).

### Decision: an unsettled child keeps the scope unsettled, and a dropped wrapper's retirement moves to the host registry

**Choice:** cancellation, deadline, trap and host failure retire each undelivered
child; the native cancellation request, the native-stop observation, pin
settlement and cleanup stay separate steps, and the scope reports "cancellation
acknowledged" distinctly from "settled". Dropping the wrapper does not clean up:
the host scope registry inherits the outstanding retirement (the same shape as
`ExecutionScope::drop`, which retires the task rather than assuming it finished).
The host-retained input payload remains owned while cleanup is outstanding and is
dropped only after the table accepts cleanup; if the wrapper is dropped earlier,
the registry inherits that payload with the pending obligations.

When the retained table snapshot has no pins after native stop, the host may
acknowledge the empty pin set without issuing the table's invalid zero-mask
settlement; cleanup remains a separate required decision.

**Rationale:** the kernel's `Task` drop performs no cleanup (`mod.rs:37`), so a
wrapper drop that reported cleanup would be false. In the pilot the registry is an
explicit retained owner, and the adapter test asserts the retirement state after
the wrapper is gone.

**Reopen conditions:** a kernel-owned scope record; a host that can block on
native stop before dropping.

### Decision: placement is a host-side / test-only pilot; the kernel and the extracted translation are untouched

**Choice:** the reference implementation of the policy is an adapter in
`crates/noble-kernel/tests/m6_interleaving/`, added by `#[path]` to the existing
`m6-interleaving` target, plus a foundation Lean model. No production Rust, no
new crate, no new test target, no kernel change.

**Rationale:** the kernel module is extracted (`proofs/m3/NobleKernel/Funs.lean`)
and `proofs/m6/M6Async.lean` depends on it; a kernel change forces re-extraction
(#97). The issue scopes this to a design plus a pilot, and the acceptance items
are about policy, not about shipping a runtime surface.

**Reopen conditions:** an owner decision to select the scope policy in the
runtime, with the extraction and inventory work that implies.

### Decision: the reference boundary states three separate claims

**Choice:** the Lean model proves *scope policy* over the #46 interleaving model;
the adapter test proves *runtime behaviour of the adapter* over the real table;
neither is a *correspondence* theorem between the two, and the receipt keeps them
in separate sections. A model obligation is never reported as an adapter
guarantee.

**Rationale:** #46's own non-claim says no correspondence is claimed between its
model and the Rust table API, and a bounded model of two children cannot establish
the general composition layer. Keeping the labels apart is part of the design
(RA-GROUP-06), not a footnote.

## Risks / Trade-offs

- Incremental admission makes partial admission a normal outcome rather than an
  edge case; the policy must be understood by callers, and a caller that assumed
  atomic admission would misread a refusal as a failed group.
- Attachment-order results need one retained record per member, which is only
  bounded because the scope is bounded at two children.
- A host registry that inherits retirement is only honest if it is actually
  retained; a pilot that drops both wrapper and registry would lose the debt, so
  the adapter test asserts the retained state rather than assuming it.
- The model's two children and declared alphabet cannot establish nesting or
  arbitrary composition; the acceptance items are stated at that bound.

## Proof obligations (named; discharged in `NobleFoundation/TaskScope.lean`)

- membership: attachment requires a live generation the scope was granted;
  duplicate, foreign, stale and post-close attachment are refused;
- result order: both completion orders produce the same attachment-ordered
  results, and completion never precedes delivery;
- refusal: a refused second admission leaves the first member's state and the
  caller's inputs unchanged;
- cancellation: cancelling before delivery keeps the pin and refuses the late
  delivery, and a late completion cannot replace the primary failure;
- settlement: close is refused while any member is unsettled, and a repeated
  close releases nothing again;
- capacity: two scope children share one admission capacity, so distinct child
  identities are not independent.

## Fixtures and controls

Positive: two children admitted and completed in both orders; cancellation before
and after one child completes; a close after full finalization; retirement
inherited by the registry after a wrapper drop.

Negative: duplicate attachment; foreign child; stale generation; fabricated
membership; attachment after close; close with an unsettled member; repeated
release; late completion after cancellation.

## Non-claims

- No implementation, proof, test or benchmark result is claimed by this design
  package as such; the model, adapter and receipt record their own results.
- No guest spawn syntax, channels, endpoint protocols, actor supervision, remote
  transport, nesting or new runtime surface.
- No correspondence theorem between the model and the Rust table API, and no
  claim about eventual native completion or cleanup after host death.
- No change to `async_tasks`, its extracted translation, `M6Async.lean`, or the
  `ExecutionScope` behaviour; those regressions are rerun, not modified.
