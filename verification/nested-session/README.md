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
| `admission_preserves_the_endpoint_inventory`, `no_endpoint_duplication` | admission neither copies nor removes an endpoint |
| `every_operation_preserves_the_endpoint_inventory`, `every_operation_keeps_the_endpoint_inventory_bound` | the bound is stated over **every** operation this model has — a session step, a boundary, or an observed cleanup — so the two operations that carry no transition are covered by case analysis rather than by omission in prose |
| `duplicating_operation_breaks_the_bound` | the bound is load-bearing: an operation that appends an endpoint duplicates one and breaks it |
| `trace_send_then_recv`, `trace_recursive_entry_unfolds_to_the_send_node`, `trace_orderly_close`, `trace_bounded_buffering_refuses_the_third_payload` | positive traces over one closed table: sequencing, recursive entry unfolding to its definition, orderly close once, bounded buffering stopping at the limit |
| `send_events_never_observe` | the asynchronous event vocabulary keeps guest request, admission, host commit, dequeue, observation, close and cleanup distinct; **a send sequence carries no observation** |
| `trace_send_then_recv_advances`, `closed_endpoint_cannot_receive` | send then receive advances through the table, and receive-after-close is refused. This is sequencing, not a recursive return: no return stack is modelled |
| `receive_after_dequeue_is_well_formed`, `forged_observation_without_dequeue_is_refused` | an observation needs a preceding dequeue; a forged peer response is refused |
| `orderly_close_with_cleanup_is_well_formed`, `close_without_cleanup_is_refused` | an omitted cleanup is refused rather than read as a successful orderly close |
| `both_choices_are_distinct_at_the_boundary` | internal choice branches stay distinct and a receive state does not step as a send |
| `typed_send_advances_on_the_declared_type`, `wrong_message_type_is_refused`, `wrong_order_is_refused` | the continuation is typed: the declared message type and order are required, and a mismatch is refused rather than coerced |
| `trace_internal_choice_branch_one`, `trace_wrong_branch_is_refused`, `external_choice_is_not_guest_choice` | internal choice takes the selected declared branch, a wrong branch is refused, and external choice belongs to the peer (its dual is the internal one) |
| `enqueue_raises_the_depth_by_one`, `dequeue_never_raises_the_depth`, `dequeue_of_empty_mailbox_is_unchanged`, `accounting_tracks_the_operations` | the mailbox depth is **derived from a ledger** of admissions and dequeues, not stored beside them |
| `full_queue_misaccounting_is_refused` | an observation cannot borrow depth the ledger never recorded |
| `bounded_admission_is_bounded`, `admission_at_the_limit_is_refused` | the bounded admission is the only operand that may raise the ledger, refuses at or past the declared limit, and never derives a depth past it while the ledger invariant holds |
| `dequeue_restores_admission_room` | the **dequeue/accounting pair**: an admission refused at the limit is accepted after one accounted dequeue, so the refusal depends on accounted depth rather than on a count of attempts |
| `unchecked_admission_breaks_the_bound` | a control: an admission that ignores the limit derives a depth past the bound, so the bounded operand is load-bearing rather than decorative |
| `boundary_preserves_the_committed_prefix`, `cancellation_keeps_its_own_event`, `quota_boundary_invents_nothing` | cancellation, trap, quota exhaustion and peer disappearance only **shorten** a trace; a cancelled request stays visible and no boundary completes a pending cleanup or invents an observation |
| `peer_disappearance_keeps_the_cleanup_pending`, `ordinary_close_does_not_keep_the_cleanup_pending` | an unobserved cleanup stays an unresolved fact, and only an observed cleanup clears it |
| `advance_preserves_the_endpoint_identity`, `steppers_preserve_the_endpoint_identity` | state preservation is proved **once, from the one step helper every stepper delegates to**, not once per stepper |
| `step_acts_only_inside_the_current_protocol` | **no action outside the current protocol**: a successful step is exactly a resolved current node declaring that action, and its sole successor is that node's declared continuation. The typed send, typed receive and internal choice steppers *are* this operand by definition, so a mismatched message type or an undeclared branch is one case of the same rule rather than a second convention |
| `guest_local_progress_on_a_declared_action` | the **scoped progress statement**: at an open owned endpoint whose resolved node declares the requested guest action, that action has a successor. Its premises are exactly open endpoint + resolved state + declared action; peer behaviour, scheduling, queue capacity and delivery are **outside** it |
| `offer_state_offers_no_guest_action`, `finished_state_declares_no_guest_action` | the two states where guest progress ends are named states rather than unnamed refusals: a peer-side offer declares no guest choice, and a finished protocol declares no guest action |
| `handshake_resolves_without_unfolding`, `handshake_trace_advances`, `nested_entry_refused_at_depth_zero` | the ordinary finite-handshake **control** beside the nested fixture: the handshake resolves at unfolding depth zero and its two actions advance, while the nested fixture's recursive entry is refused there and resolves only under unfolding. This does **not** show non-regularity: `resolve` ignores the nested parameter, so `exampleProtocol` is the regular loop `(send i64 · recv i64)*` |
| `different_branches_are_different_traces`, `branch_one_reaches_its_own_continuation`, `branch_zero_refuses_a_bool_send`, `branch_one_accepts_the_bool_send` | an internal choice carries its branch, so two branches are two traces; a node declares one branch and an undeclared branch is refused |

`verification/nested-session/receipt.json` is the retained receipt of one clean-source run: **68** model
theorems, with `propext` and `Quot.sound` the only reported axioms (`Quot.sound` for
`full_mailbox_is_unchanged`), both inside the standard allow-list. Its `bindings` block names the protocol
definitions, the endpoint identity fields and the ledger-based queue accounting as the model's own
constants (bound through the source hash), records `helper`, `runtime` and `peer` as explicitly absent
with the reason, and its `claims` block labels the verdict.

## Non-claims

The definitions table is handwritten: there is no lowering, runtime, scheduler, queue or peer
correspondence here, the progress statement is **guest-local only** (termination, fairness, eventual
delivery, network reliability and deadlock freedom are separate and unclaimed), and endpoint
delegation (passing an endpoint as a
message) is excluded, so ownership obligations stay outside this model. Cancellation, trap, quota,
peer disappearance, mailbox fullness and cleanup facts **are** modelled at the trace level, the
bounded admission over the ledger is measured, and `step_acts_only_inside_the_current_protocol`
covers every guest action with the finite-handshake control beside the nested fixture, and the
inventory bound is stated over every operation including those that carry no transition, and progress
is stated for the guest's own step under three named premises. What stays open is the correspondence
receipt's lowering/runtime/helper revisions, and four model items the 2026-10-29 audit found
over-ticked:
- a protocol type-equality algorithm;
- progress under queue, peer, scheduler and host premises;
- external choice and recursive return in the positive traces;
- a non-regular nested fixture, which needs a return stack.

The `claims` block names the progress, cancellation, nested-recursion and type-equality scope. The
fixtures are **model traces only**: nothing here executes emitted Wasm or a native transport.
