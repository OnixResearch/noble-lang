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
external `offer` (taken by the guest action `receiveBranch`), and a named `nested` constructor with explicit parameters — resolved against a closed
definitions table under a **bounded** unfolding depth. The endpoint is an abstract move-only resource
carrying channel identity, side, protocol identity, current state and generation. The stack-aware
operand carries a bounded return stack beside the endpoint: `enter` saves a nested parameter, and
`finish` pops it (or orderly-ends when the stack is empty). The balanced table's unbounded trace
language `sendⁿ·recvⁿ` is not regular and needs that return stack; this model admits it only up to the
declared stack limit, so its accepted bounded subset is finite.

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
| `trace_send_then_recv`, `trace_recursive_entry_unfolds_to_the_send_node`, `trace_orderly_close`, `trace_bounded_buffering_refuses_the_third_payload` | positive traces over one closed table: sequencing, the legacy depth-only nested control, orderly close once, bounded buffering stopping at the limit |
| `send_events_never_observe` | the asynchronous event vocabulary keeps guest request, admission, host commit, dequeue, observation, close and cleanup distinct; **a send sequence carries no observation** |
| `trace_send_then_recv_advances`, `closed_endpoint_cannot_receive` | send then receive advances through the table, and receive-after-close is refused; the stack-aware recursive entry/return traces are listed below |
| `receive_after_dequeue_is_well_formed`, `forged_observation_without_dequeue_is_refused` | an observation needs a preceding dequeue; a forged peer response is refused |
| `orderly_close_with_cleanup_is_well_formed`, `close_without_cleanup_is_refused` | an omitted cleanup is refused rather than read as a successful orderly close |
| `both_choices_are_distinct_at_the_boundary` | internal choice branches stay distinct and a receive state does not step as a send |
| `typed_send_advances_on_the_declared_type`, `wrong_message_type_is_refused`, `wrong_order_is_refused` | the continuation is typed: the declared message type and order are required, and a mismatch is refused rather than coerced |
| `trace_internal_choice_branch_one`, `trace_wrong_branch_is_refused`, `external_choice_is_not_guest_choice`, `trace_internal_choice_branch_zero` | internal choice takes either selected declared branch, a wrong branch is refused, and the guest cannot select at an external choice (its dual is the internal one) |
| `trace_external_choice_branch_zero`, `trace_external_choice_branch_one`, `undeclared_branch_receipt_is_refused`, `external_branch_keeps_its_typed_continuation`, `branch_receipt_is_refused_at_an_internal_choice`, `receipt_of_another_branch_is_refused` | **external choice is taken**: the guest's `receiveBranch` action receives the peer's selection; both branches of `offerProtocol` reach their own typed continuations and complete, an undeclared or other branch is refused at every offer node, and receiving is refused at an internal choice |
| `dualAction_is_involutive`, `continuationOf_dual`, `resolve_dual`, `step_on_the_dual_table`, `dual_table_receives_the_selected_branch` | **duality of steps**: for every node and action the dual action at the dual node declares the same continuation, so every step has the matching step on the dual table (a guest selection is the peer's receipt) |
| `stack_external_choice_trace_is_accepted` | the stack-aware operand takes the same external choice and accepts the complete trace |
| `enqueue_raises_the_depth_by_one`, `dequeue_never_raises_the_depth`, `dequeue_of_empty_mailbox_is_unchanged`, `accounting_tracks_the_operations` | the mailbox depth is **derived from a ledger** of admissions and dequeues, not stored beside them |
| `full_queue_misaccounting_is_refused` | an observation cannot borrow depth the ledger never recorded |
| `bounded_admission_is_bounded`, `admission_at_the_limit_is_refused` | the bounded admission is the only operand that may raise the ledger, refuses at or past the declared limit, and never derives a depth past it while the ledger invariant holds |
| `dequeue_restores_admission_room` | the **dequeue/accounting pair**: an admission refused at the limit is accepted after one accounted dequeue, so the refusal depends on accounted depth rather than on a count of attempts |
| `unchecked_admission_breaks_the_bound` | a control: an admission that ignores the limit derives a depth past the bound, so the bounded operand is load-bearing rather than decorative |
| `boundary_preserves_the_committed_prefix`, `cancellation_keeps_its_own_event`, `quota_boundary_invents_nothing` | cancellation, trap, quota exhaustion and peer disappearance only **shorten** a trace; a cancelled request stays visible and no boundary completes a pending cleanup or invents an observation |
| `peer_disappearance_keeps_the_cleanup_pending`, `ordinary_close_does_not_keep_the_cleanup_pending` | an unobserved cleanup stays an unresolved fact, and only an observed cleanup clears it |
| `advance_preserves_the_endpoint_identity`, `steppers_preserve_the_endpoint_identity` | state preservation is proved **once, from the one step helper every stepper delegates to**, not once per stepper |
| `step_acts_only_inside_the_current_protocol` | **no action outside the current protocol**: a successful step is exactly a resolved current node declaring that action, and its sole successor is that node's declared continuation. The typed send, typed receive and internal choice steppers *are* this operand by definition, so a mismatched message type or an undeclared branch is one case of the same rule rather than a second convention |
| `stackAdvance_keeps_the_stack`, `stackAdvance_respects_the_limit`, `stackState_within_the_limit`, `stackEnter_respects_the_limit`, `stack_step_respects_the_limit` | communication continuations, nested entry and finished-node returns preserve the declared limit and keep every successful stack step within it when the input stack is within it |
| `stack_entry_and_return_at_depth_one`, `stack_entry_and_return_at_depth_two` | the bounded stack saves one and two nested return continuations and pops them in last-in-first-out order |
| `balanced_trace_send_two_recv_two_is_accepted` | `balancedProtocol` accepts two sends followed by two receives, with the stack empty at orderly end |
| `balanced_trace_send_two_recv_three_is_refused`, `balanced_trace_send_three_recv_two_is_refused` | the balanced fixture refuses an extra receive and refuses a trace with too few receives to close all sends |
| `stack_push_beyond_the_limit_is_refused` | a push at the declared return-stack limit is refused |
| `dropped_return_continuation_accepts_unbalanced_and_rejects_balanced` | mutant control: dropping the saved continuation accepts two sends/one receive, refuses the balanced two-send/two-receive trace, while the real stack refuses the same unbalanced trace |
| `guest_local_progress_on_a_declared_action` | the **scoped progress statement**: at an open owned endpoint whose resolved node declares the requested guest action, that action has a successor. Its premises are exactly open endpoint + resolved state + declared action; peer behaviour, scheduling, queue capacity and delivery are **outside** it |
| `offer_state_declares_only_its_branch_receipt`, `finished_state_declares_no_guest_action` | named states rather than unnamed refusals: at an external choice the guest's only action is receiving the declared branch (whether the peer selects is the peer's premise), and a finished protocol declares no guest action |
| `environmentStep_of_ready`, `progress_under_explicit_premises` | the scoped progress statement **under explicit queue, peer, scheduler and host premises**: `stepIn` takes the guest step and its environment change together, and it succeeds at an open owned endpoint whose resolved node declares the action when the scheduler grants the step, the host admits it, and the action's queue premise (outbox room for `send`/`choose`) or peer premise (an accounted inbox payload; for `receiveBranch`, the peer's selection of that branch) holds. The premises are assumed for the step, never derived |
| `progress_fixture_send_then_receive`, `unscheduled_guest_takes_no_step`, `host_refusal_takes_no_step`, `full_outbox_refuses_send`, `silent_peer_leaves_receive_pending`, `branch_receipt_needs_the_peer_selection` | the fixture send-then-receive progresses through one outbox admission and one dequeue, and each premise is load-bearing: without the scheduler grant, with a host refusal, with a full outbox, with an empty inbox or with another selected branch, the same step is refused |
| `protoStep`, `stackStep_eq_protoStep`, `runStack_eq_runProto`, `mem_enabled_iff` | the configuration step (definition index and return stack) **is** the model's `stackStep` on an open endpoint, and so is the trace run; each configuration lists the at most two actions it takes |
| `protocolEquality_sound`, `protocolEquality_witness`, `exhaustion_is_refused`, `equal_protocols_run_alike` | **protocol type equality under a declared work bound**: a memoised pair exploration spends one unit per pair; `equal` is proved to be trace equivalence (the same admitted traces and orderly ends), `differ` carries a trace on which the two states are proved to disagree, and a bound that runs out is the refusal `exhausted`, never `equal`. `enter`, `skipNested` and `finish` are guest actions here, so the equality is strong rather than equi-recursive; no bound is proved sufficient for every protocol |
| `protoStep_dual`, `orderlyEnd_dual`, `observe_dual`, `protocolDuality_sound` | **duality** is equality with the dual table: a `dual` verdict means each trace of one side is admitted exactly when its dual trace is admitted by the other, with the same orderly ends |
| `renumbered_balanced_is_equal`, `stack_limit_separates_types`, `unfolding_is_equality_within_the_bound`, `order_separates_types`, `client_and_server_are_dual`, `exhausted_duality_is_refused` | fixtures: renumbering is equality; one fewer saved return is a different type, with the separating trace; a one-node cycle equals its two-node unrolling at three units and is refused as exhausted at two; order is part of the type; a client and server are dual and a mistyped server is not; a duality check that exhausts its work is refused |
| `accepting_exhaustion_accepts_unequal_types`, `memo_is_load_bearing` | load-bearing mutants: reading exhaustion as acceptance accepts two protocols the check proves unequal, and without the memo of assumed pairs a cyclic protocol is never decided |
| `checkPairs_not_exhausted`, `configs_closed`, `pairsOf_closed`, `protocolEquality_not_exhausted`, `protocolEquality_decides`, `protocolDuality_decides`, `workBound_decides_the_fixtures` and ten supporting lemmas | **a proved sufficient work bound**. The pending count plus twice the pairs of a closed set not yet assumed falls by at least one per unit of work, so a bound at least that large is never exhausted. Every configuration reachable from an empty return stack lies in an explicit list: the start state or an index the table names, with a return stack of at most the limit over the table's saved returns. That list is closed under every step, so `workBound`, one plus twice the number of reachable pairs, is computed from the two tables alone. Under it the check returns `equal` **exactly** when the two states show the same traces and orderly ends, and likewise for duality with the dual table. On the fixtures the bound is 13 for the cycle and its unrolling, 19 for client and server, and 301 for the balanced table at stack limits 2 and 1, and each decides. The bound grows with the number of stacks, exponentially in the limit; it is a sufficient bound, not a tight one |
| `handshake_resolves_without_unfolding`, `handshake_trace_advances`, `nested_entry_refused_at_depth_zero` | the ordinary finite-handshake **control** beside the legacy depth-only nested operand: the handshake resolves at unfolding depth zero and its two actions advance, while the legacy nested entry is refused there and resolves only under unfolding; the balanced table's unbounded `sendⁿ·recvⁿ` language needs the stack, and this model admits it only up to the declared limit |
| `different_branches_are_different_traces`, `branch_one_reaches_its_own_continuation`, `branch_zero_refuses_a_bool_send`, `branch_one_accepts_the_bool_send` | an internal choice carries its branch, so two branches are two traces; a node declares one branch and an undeclared branch is refused |

`verification/nested-session/receipt.json` is the retained receipt of one clean-source run: **144** model
theorems, every axiom inside the standard allow-list:
- 23 theorems use no axiom, including `stackState_within_the_limit`, `dualAction_is_involutive` and
  `StackAction.dual_dual`;
- 74 use `propext` alone, including the bounded return-stack and external-choice trace/control theorems,
  the step-duality theorems, the eight explicit-premise progress theorems and controls, and seven of
  the type-equality theorems;
- 45 use `propext` and `Quot.sound`, among them `full_mailbox_is_unchanged`,
  `step_acts_only_inside_the_current_protocol`, the type-equality soundness and witness theorems,
  `exhaustion_is_refused` and all seventeen work-bound theorems;
- 2 also use `Classical.choice`: `advance_preserves_the_endpoint_identity` and
  `steppers_preserve_the_endpoint_identity`.

Earlier versions of this paragraph said `propext` was the only reported axiom; the receipts never
supported that. Its `bindings` block names the protocol
definitions, the endpoint identity fields and the ledger-based queue accounting as the model's own
constants (bound through the source hash), records `helper`, `runtime` and `peer` as explicitly absent
with the reason, and its `claims` block labels the verdict.

## Non-claims

The definitions table is handwritten: there is no lowering, runtime, scheduler, queue or peer
correspondence here, progress is stated for **one step** only, guest-locally and under explicit queue, peer, scheduler
and host premises (termination, fairness, eventual delivery, network reliability and deadlock freedom
are separate and unclaimed), and endpoint
delegation (passing an endpoint as a
message) is excluded, so ownership obligations stay outside this model. Cancellation, trap, quota,
peer disappearance, mailbox fullness and cleanup facts **are** modelled at the trace level, the
bounded admission over the ledger is measured, and `step_acts_only_inside_the_current_protocol`
covers every guest action with the finite-handshake control beside the nested fixture, and the
inventory bound is stated over every operation including those that carry no transition, progress is
stated for the guest's own step under three named premises, and the balanced table's unbounded
`sendⁿ·recvⁿ` trace language needs a return stack while this model admits it only up to the declared
limit. The work bound is proved sufficient for every pair of tables (`protocolEquality_decides`), so a
declared bound below it can still be refused as exhaustion but a bound at or above it decides. What stays
open is the correspondence receipt's lowering/runtime/helper revisions, and one model item outside
this unit: a choice node carrying a label set. A node
declares one branch, so a two-branch choice is two declared nodes (the external-choice action and
both positive branch traces are now modelled).

The `claims` block names the progress, cancellation, nested-recursion and type-equality scope. The
fixtures are **model traces only**: nothing here executes emitted Wasm or a native transport.
