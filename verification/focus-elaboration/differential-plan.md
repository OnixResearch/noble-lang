# Differential test plan for values, stack order, resource obligations and effect traces (issue #79)

This is the *plan* the issue asks for. Nothing in it is executed against an elaborator, because the
issue excludes implementation and no focus elaborator exists. What has changed since the first
revision is that the worked reference pairs now exist and are measured
(`receipts/2026-11-13/measure.json`), including the type-changing **nested** pair and the `dip`
forms, and the observation surface of the CLI is now recorded per form instead of being assumed.
The plan exists so that a later implementation has a decided oracle and decided vectors rather than
invented ones.

## Oracles, per axis

| Axis | Reference observation | Why not the elaborator itself |
|---|---|---|
| values | the existing checker plus `noble live repl` stack reporting on a hand-authored explicit program, exactly as `receipts/<date>/measure.json` does for every compiled form | the elaborator's output cannot referee its own meaning |
| stack order | the ordered typed stack the CLI reports (`readStack`, `crates/noble-cli/src/core/runtime/values.mjs:44-64`) with `metrics.stack_length` cross-checked against the reported cell count; the reported array runs from the bottom of the stack upward, which `measure.mjs` establishes with its own `1 5` probe rather than asserting; plus the ordered `stack_in`/`stack_out` shapes of the bootstrap contracts the elaboration claims to target (`dip`, `compose`, `run`, `pair`, `unpair`), which the checker refuses to match by accident | signature IDs are complete and ordered; argument counts and suffix matches are not accepted, and a differential that compares only heights cannot see a swap of two same-typed cells |
| resource obligations | **#59's selected fragment**, reused rather than duplicated: the ownership-accounting oracle over ordered stack rules (`proofs/foundation/NobleFoundation/Ownership.lean`), together with its conservation theorems (`OwnershipConservation.lean`) | a second ownership oracle would be a second authority on the same question |
| effect traces | `Program.effects` passthrough recorded by the elaborator, compared against the hand-authored program's traces, which the CLI reports as `guest_requests`/`request_trace` (recorded per form in the receipt; empty for every pure form here) | a pure elaborator must not make an effectful program pure, and the trace is where that shows |

## Vectors

| Vector | Explicit reference program | What the differential must show |
|---|---|---|
| `nested-product` | `forms/nested-product.explicit.noble` | equal value `Pair(2, 5)`; untouched component `5` observed exactly once |
| `nested-product` (`dip`) | `forms/nested-product.dip.noble` | the same value through `dip`, i.e. without a swap-based framing and without capturing a residual |
| `type-changing-step` | `forms/type-changing-step.explicit.noble` | equal value `Bool false`; the focus boundary's input `I64` and output `Bool` both checked |
| `type-changing-nested` | `forms/type-changing-nested.explicit.noble` | a nested update whose inner operation changes type: equal `Pair(Bool false, 5)`, and the reconstruction type differing from the input type is visible in the reported cell types |
| `preserved-prefix` | partially covered by `nested-product` | where a resource prefix is supported; today it is not, so this vector is `not-run` with that reason rather than asserted |
| `move-threaded` | none exists in source | admitted only after a must-use resource word exists in the profile |

## Rules the differential must obey

1. **No identity promises.** Differently elaborated programs may produce different recipes or
   hashes. The comparison is on values, stack order, resource obligations and effect traces — never
   on artifact digests being equal.
2. **No evidence transfer.** Any evidence binds the exact source commit, elaborator revision,
   profile and generated Program digest. Matching signatures or outputs do not transfer proof
   evidence between Programs.
3. **Refusals are observations.** Ambiguous or unsupported focus, exhaustion and invalid ownership
   must be *distinct diagnostics*, not one generic failure, and each is recorded as a refusal the
   differential expects rather than a crash it tolerates.
4. **Recorded, not assumed.** A vector that does not run is recorded as `not-run` with its reason.
   A vector that runs on one form but not the other is a defect of the plan, not a pass.

## Item 4's refusal/admission controls against the selected fragment

The issue forbids creating a second ownership oracle, so every control is either mapped to a theorem
that already exists in #59's selected fragment (`proofs/foundation/NobleFoundation/Ownership*.lean`,
whose word set is `dup`/`drop`/`swap`/`quote` and whose program set is `word`/`compose`/`branch`/
`fail` — `Ownership.lean:38-93`) or recorded as not expressible in that fragment. A control that is
neither mapped nor excluded would be a missing obligation, not a closed one.

| Control | Status in the fragment | Witness or reason |
|---|---|---|
| duplicate owner | covered generically | `resource_operand_blocks_non_swap_word` (`Ownership.lean:141`): `dup` on a resource operand is refused |
| lost owner | covered generically | the same theorem for `drop`, plus conservation `step_conserves_obligations` (`OwnershipConservation.lean:25`) and `run_conserves_obligations` (`:91`) |
| closing a resource promised back | **not expressible** | the fragment has no close or return word: its only error path is `fail`. The *abandon* half is covered by `fail_rejects_outstanding_operand` (`Ownership.lean:229`); a promised-back close has no construct to state |
| stale pre-update binding | **not expressible** | no bindings, heap or snapshots exist in the fragment; the operand list *is* the whole state |
| overlapping focus | **not expressible** | "focus" is a surface notion with no counterpart in a one-cursor operand stack; the fragment's only joining construct, `branch`, is refused when the arms disagree (`branch_accounts_both_arms`, `Ownership.lean:216`) |
| wrong reconstruction type | observable on the value axis, **not expressible** as ownership | the receipt reports each cell's `type`, so a differential sees a wrong reconstruction type directly; the fragment is untyped payloads, so it cannot state the typing claim (that is the MC1 typing lane, and `obligations.md` row 1 keeps it open) |
| branch mismatch | covered | `branch_accounts_both_arms` (`:216`) with the join controls `distinct_owners_are_not_merged` (`:312`), `one_armed_join_is_refuted` (`:319`), `same_count_different_identity_is_conflicting` (`:326`), `unequal_arms_cannot_be_joined` (`:344`) |
| forgotten ordinary-error obligation | covered | `fail_requires_empty_operands` (`:223`) and `fail_rejects_outstanding_operand` (`:229`) |
| escape through returned aggregates | partially covered | a resource inside a compound payload is not duplicable at any depth — `wrapped_resource_not_duplicable` (`:119`), `wrapDepth_resource_not_duplicable` (`:136`) — so an aggregate cannot launder it past `dup`/`drop`/`quote`; the *return* edge itself has no word in the fragment and stays an obligation of the later lane |
| escape through saved Programs | partially covered | `quote_retains_one` (`:162`): the fragment's quotation analogue keeps the inspected operand on the stack, and a resource operand is refused (`:141`). A first-class saved Program that is captured and released later is not a fragment construct, so capture-and-release is not expressible here |

## Item 5's mutated elaborations

| Mutated elaboration | Status in the fragment | Witness or reason |
|---|---|---|
| insert a forbidden `dup`/`drop`/capture | covered generically | `resource_operand_blocks_non_swap_word` (`Ownership.lean:141`) for the inserted word, and `step_conserves_obligations` (`OwnershipConservation.lean:25`) for any admitted step |
| omit reconstruction | covered at the join | a branch that rebuilds a different obligation set cannot be joined: `branch_accounts_both_arms` (`:216`), `unequal_arms_cannot_be_joined` (`:344`), `same_count_different_identity_is_conflicting` (`:326`) |
| reorder effects | **not expressible** | the fragment has no effects and no ordered effect trace; ordering is the backend/host axis (obligations row 5, #82), and the plan above compares it through `Program.effects` passthrough, not through ownership |

## Items 4–5 are not applicable to this issue, and why

The recorded decision (`decision.md`) selects no new focus syntax, so no elaborator is built here
and there is no elaboration to mutate or to admit wrongly. Under that decision items 4 and 5 are
**not applicable to this issue**; the mapping above is what a future elaborator lane inherits, with
the covered controls signalling that a mutation which violates ownership is rejected by an existing
theorem, and the "not expressible" rows naming exactly what such a lane must add rather than
discover a second oracle for. Nothing in this table is claimed as an executed test.

## Status

| Vector | Status |
|---|---|
| nested-product | explicit, `dip` and library forms all report `Pair(2, 5)` today (measured); no elaborator output to compare |
| nested-product (`dip`) | same value, measured; the composed `dip` variant reports it too |
| type-changing-step | explicit and library forms report `Bool false` today (measured) |
| type-changing-nested | explicit, `dip` and library forms report `Pair(Bool false, 5)` today (measured); this is the nested pair the first revision of this plan listed as "not yet authored" |
| preserved-prefix | not-run: the selected profile has no resource prefix to preserve |
| move-threaded | not-run: no must-use resource word in source |
