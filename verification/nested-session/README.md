# Nested session protocol model (WIT-SESSION-01, WIT-SESSION-02)

Model-theorem check for `.cairn/changes/nested-session-protocol`. It builds
`proofs/m6/NestedSession.lean` in an isolated project with the pinned Lean 4.31 toolchain, then checks
every inventoried theorem's compiled statement and transitive axioms.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/nested-session/check.mjs --output /tmp/nested-session-check
```

Same refusals as the region-trace harness: a caller environment carrying `LEAN*`, `LAKE*`, `BASH*`,
`GIT_*`, `NIX_*` or `ELAN*` is refused, an output directory inside the source tree is refused, and a
dirty worktree needs the explicit `--allow-dirty` flag (which the receipt then records).

## What is modelled and proved

The grammar is one finite set of nodes — `finished`, typed `send`, typed `recv`, internal `choose`,
external `offer`, and a named `nested` constructor with explicit parameters — resolved against a closed
definitions table under a **bounded** unfolding depth. The endpoint is an opaque move-only resource
carrying channel identity, side, protocol identity, current state and generation.

| Theorem | Claim |
|---|---|
| `dual_is_involutive`, `dual_swaps_send_and_recv`, `dual_swaps_choose_and_offer` | duality is definitional, involutive, and swaps Send/Recv and internal/external choice on the same message/branch |
| `resolve_refuses_exhaustion`, `resolve_unfolds_with_depth`, `resolve_refuses_unknown_definition` | exhaustion and an unknown index are **refusals**, never acceptances |
| `endpoint_copy_is_refused`, `endpoint_discard_is_refused`, `endpoint_capture_is_refused` | copy, discard and capture each have their own named refusal |
| `stale_generation_is_refused`, `matching_generation_is_accepted`, `double_close_is_refused` | identity alone is not authority; the matching generation is the non-vacuous control; a second close is refused |
| `closed_endpoint_cannot_send`, `send_step_advances_to_the_continuation` | a closed endpoint cannot send, and one step advances exactly to the send node's continuation with peer/channel/protocol/generation unchanged |
| `send_step_does_not_observe_a_peer` | a send step returns only the endpoint's own successor: **receipt is not modelled** |
| `admitOne_is_bounded`, `admission_is_bounded`, `full_mailbox_is_unchanged` | admission keeps the mailbox depth inside the declared limit (given a well-formed session) and a full mailbox refuses without a silent drop |
| `admission_preserves_the_endpoint_inventory`, `no_endpoint_duplication` | no operation in this model increases the endpoint inventory |
| `trace_send_then_recv`, `trace_recursive_entry_unfolds_to_the_send_node`, `trace_orderly_close`, `trace_bounded_buffering_refuses_the_third_payload` | positive traces over one closed table: sequencing, recursive entry unfolding to its definition, orderly close once, bounded buffering stopping at the limit |
| `send_events_never_observe` | the asynchronous event vocabulary keeps guest request, admission, host commit, dequeue, observation, close and cleanup distinct; **a send sequence carries no observation** |
| `trace_send_then_recv_advances`, `closed_endpoint_cannot_receive` | the return half: send then receive advances through the table, and receive-after-close is refused |
| `receive_after_dequeue_is_well_formed`, `forged_observation_without_dequeue_is_refused` | an observation needs a preceding dequeue; a forged peer response is refused |
| `orderly_close_with_cleanup_is_well_formed`, `close_without_cleanup_is_refused` | an omitted cleanup is refused rather than read as a successful orderly close |
| `both_choices_are_distinct_at_the_boundary` | internal choice branches stay distinct and a receive state does not step as a send |
| `typed_send_advances_on_the_declared_type`, `wrong_message_type_is_refused`, `wrong_order_is_refused` | the continuation is typed: the declared message type and order are required, and a mismatch is refused rather than coerced |
| `trace_internal_choice_branch_one`, `trace_wrong_branch_is_refused`, `external_choice_is_not_guest_choice` | internal choice takes the selected declared branch, a wrong branch is refused, and external choice belongs to the peer (its dual is the internal one) |
| `enqueue_raises_the_depth_by_one`, `dequeue_never_raises_the_depth`, `dequeue_of_empty_mailbox_is_unchanged`, `accounting_tracks_the_operations` | the mailbox depth is **derived from a ledger** of admissions and dequeues, not stored beside them |
| `full_queue_misaccounting_is_refused` | an observation cannot borrow depth the ledger never recorded |
| `boundary_preserves_the_committed_prefix`, `cancellation_keeps_its_own_event`, `quota_boundary_invents_nothing` | cancellation, trap, quota exhaustion and peer disappearance only **shorten** a trace; a cancelled request stays visible and no boundary completes a pending cleanup or invents an observation |
| `peer_disappearance_keeps_the_cleanup_pending`, `ordinary_close_does_not_keep_the_cleanup_pending` | an unobserved cleanup stays an unresolved fact, and only an observed cleanup clears it |
| `advance_preserves_the_endpoint_identity`, `steppers_preserve_the_endpoint_identity` | state preservation is proved **once, from the one step helper every stepper delegates to**, not once per stepper |
| `different_branches_are_different_traces`, `branch_one_reaches_its_own_continuation`, `branch_zero_refuses_a_bool_send`, `branch_one_accepts_the_bool_send` | an internal choice carries its branch, so two branches are two traces; a node declares one branch and an undeclared branch is refused |

`verification/nested-session/receipt.json` is the retained receipt of one clean-source run: **54** model
theorems, `propext` the only reported axiom and inside the standard allow-list. Its `bindings` block names the protocol
definitions, the endpoint identity fields and the ledger-based queue accounting as the model's own
constants (bound through the source hash), records `helper`, `runtime` and `peer` as explicitly absent
with the reason, and its `claims` block labels the verdict.

## Non-claims

The definitions table is handwritten: there is no lowering, runtime, scheduler, queue or peer
correspondence here, **no progress statement**, and endpoint delegation (passing an endpoint as a
message) is excluded, so ownership obligations stay outside this model. Cancellation, mailbox
fullness and cleanup facts are not modelled yet. WIT-SESSION-03 through WIT-SESSION-06 remain open.
