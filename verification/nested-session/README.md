# Nested session protocol model (WIT-SESSION-01, WIT-SESSION-02)

Model-theorem check for `.cairn/changes/nested-session-protocol`. It builds
`proofs/m6/NestedSession.lean` in an isolated project with the pinned Lean toolchain, then checks
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
carrying channel identity, side, protocol identity, current state and generation. An admission-side
table check (`validateTable`) refuses a table past its definition-count, constructor-arity or
label-set-width bound, a continuation, name or parameter outside the table, or a repeated label, each
as `malformed` with its own fault class. The stack-aware
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
| `protoStep`, `stackStep_eq_protoStep`, `runStack_eq_runProto`, `mem_enabled_iff` | the configuration step (definition index and return stack) **is** the model's `stackStep` on an open endpoint, and so is the trace run; each configuration lists the actions it takes: one per declared label at a label-set node, and at most two elsewhere |
| `protocolEquality_sound`, `protocolEquality_witness`, `exhaustion_is_refused`, `equal_protocols_run_alike` | **protocol type equality under a declared work bound**: a memoised pair exploration spends one unit per pair; `equal` is proved to be trace equivalence (the same admitted traces and orderly ends), `differ` carries a trace on which the two states are proved to disagree, and a bound that runs out is the refusal `exhausted`, never `equal`. `enter`, `skipNested` and `finish` are guest actions here, so the equality is strong rather than equi-recursive; `protocolEquality_not_exhausted` proves the declared bound `workBound` sufficient for every pair of tables, so a smaller declared bound can be refused as `exhausted` but a bound at or above it decides |
| `protoStep_dual`, `orderlyEnd_dual`, `observe_dual`, `protocolDuality_sound` | **duality** is equality with the dual table: a `dual` verdict means each trace of one side is admitted exactly when its dual trace is admitted by the other, with the same orderly ends |
| `renumbered_balanced_is_equal`, `stack_limit_separates_types`, `unfolding_is_equality_within_the_bound`, `order_separates_types`, `client_and_server_are_dual`, `exhausted_duality_is_refused` | fixtures: renumbering is equality; one fewer saved return is a different type, with the separating trace; a one-node cycle equals its two-node unrolling at three units and is refused as exhausted at two; order is part of the type; a client and server are dual and a mistyped server is not; a duality check that exhausts its work is refused |
| `accepting_exhaustion_accepts_unequal_types`, `memo_is_load_bearing` | load-bearing mutants: reading exhaustion as acceptance accepts two protocols the check proves unequal, and without the memo of assumed pairs a cyclic protocol is never decided |
| `checkPairs_not_exhausted`, `configs_closed`, `pairsOf_closed`, `protocolEquality_not_exhausted`, `protocolEquality_decides`, `protocolDuality_decides`, `workBound_decides_the_fixtures` and twelve supporting lemmas | **a proved sufficient work bound**. The pending count plus the left table's `width` (its largest label set, and at least two) times the pairs of a closed set not yet assumed falls by at least one per unit of work, so a bound at least that large is never exhausted. Every configuration reachable from an empty return stack lies in an explicit list: the start state or an index the table names, with a return stack of at most the limit over the table's saved returns. That list is closed under every step, so `workBound`, one plus that width times the number of reachable pairs, is computed from the two tables alone. Under it the check returns `equal` **exactly** when the two states show the same traces and orderly ends, and likewise for duality with the dual table. On the fixtures the bound is 13 for the cycle and its unrolling, 19 for client and server, and 301 for the balanced table at stack limits 2 and 1, and each decides. The bound grows with the number of stacks, exponentially in the limit; it is a sufficient bound, not a tight one |
| `handshake_resolves_without_unfolding`, `handshake_trace_advances`, `nested_entry_refused_at_depth_zero` | the ordinary finite-handshake **control** beside the legacy depth-only nested operand: the handshake resolves at unfolding depth zero and its two actions advance, while the legacy nested entry is refused there and resolves only under unfolding; the balanced table's unbounded `sendⁿ·recvⁿ` language needs the stack, and this model admits it only up to the declared limit |
| `different_branches_are_different_traces`, `branch_one_reaches_its_own_continuation`, `branch_zero_refuses_a_bool_send`, `branch_one_accepts_the_bool_send` | an internal choice carries its branch, so two branches are two traces; a node declares one branch and an undeclared branch is refused |
| `lookup_isSome_iff`, `lookup_mem`, `two_le_width`, `nodeWidth_le_width`, `enabled_length_le_width`, `successorPairs_length_le_width` | **choice over a label set**: a `select` node (internal) or `branch` node (external, its dual) declares a list of `(label, continuation)` pairs at one state, and the first pair naming a label is taken. A lookup succeeds exactly for a declared label and returns a declared continuation. Such a state can enable more than two actions, so the two-successor bound is replaced by the table's `width` throughout the work bound; tables without label sets keep width two and their bounds |
| `label_set_enables_every_label`, `label_set_takes_each_branch`, `undeclared_label_is_refused`, `selection_and_receipt_meet_at_the_continuation`, `label_order_is_not_observable`, `a_missing_label_is_distinguished`, `label_set_duality`, `a_wide_choice_needs_its_width`, `first_declared_label_wins` | fixtures and controls: every label is enabled at one state and reaches its own typed continuation; an undeclared label and a branch receipt at an internal choice are refused; the guest's selection and the peer's receipt on the dual table meet at the same continuation; label order is not observable, a missing label is (witness `[choose 1]`), and duality is accepted; three labels enable three actions at a state of width three; `first_declared_label_wins` states the accessor's first-pair equation for the table the admission check rejects — no accepted table reaches it |
| `validateTable`, `validateTable_wellFormed_definitions_bounded`, `validateTable_wellFormed_entry`, `wellFormed_tables_have_unique_labels`, `wellFormed_tables_have_in_range_indices` and the supporting inversion lemmas (`validateContinuation_none`, `outOfRangeBranch_none`, `validateBranches_none_no_repeated_label`, `validateBranches_none_outOfRange`, `validateEntry_cons_some`, `validateEntry_cons_none`, `validateEntry_eq_none_iff`, `validateNode_none_namedIndices`, `repeated_label_branches_are_malformed`, `repeated_label_entry_is_malformed`, `repeated_label_table_is_refused`) | **admission-side table check**: a table past its declared definition count, with a continuation, nested name or nested parameter outside itself, with more branches than its label-set bound, or with a repeated label is `malformed` with its own fault class. Every accepted table is proved to be inside its definition bound, to declare no label twice and to name only its own definitions. A repeated label is **refused, not resolved**: the accessor's first-pair rule is reachable only on a rejected table |
| `validateTable_accepts_the_model_fixtures`, `validateTable_refuses_the_legacy_fixture`, `validateTable_refuses_a_repeated_label`, `validateTable_refuses_an_out_of_range_continuation`, `validateTable_refuses_an_out_of_range_name`, `validateTable_refuses_an_out_of_range_parameter`, `validateTable_refuses_a_constructor_past_the_arity`, `validateTable_refuses_too_many_definitions`, `validateTable_refuses_a_wide_label_set`, `validateTable_refuses_an_out_of_range_branch`, `repeated_label_is_refused_before_it_is_resolved` | decided controls: the model's fixtures are well formed; the legacy depth-only fixture is malformed because its nested parameter is outside its four-entry table; each fault class is refused by its own name; the specification's bounded-constructor-arity scenario refuses a constructor past the grammar's arity as `malformed` rather than retaining a partial unfolding |
| `pact_label_set_erases_to_the_fixture`, `pact_reannotation_is_not_observable`, `pact_label_set_duality_holds`, `pact_label_set_wrong_branch_is_refused`, `pact_label_set_preserves_the_endpoint_identity`, `pact_work_bound_is_the_skeleton_bound`, `pact_factor_does_not_select` | **Pact preference-factor adjunct, default off**: an annotated table's only meaning is its erasure to the plain skeleton, so duality, wrong-branch refusal, endpoint identity preservation and the work bound are the skeleton's, and reversing the factors neither enables, refuses nor selects a label. Factors only: no utility function, decision rule or solver |
| `vine_source_forms_denote_the_same_fixture`, `vine_source_continuation_mismatch_is_refused`, `vine_source_continuation_match_is_accepted` | **Vine source-form comparison, design examples only**: both proposed source forms denote the model's fixture, a source continuation that differs from the model's is refused with the witness trace `[send i64, recv bool]`, and the matched form is the same type. See [`vine-comparison.md`](vine-comparison.md) |

`verification/nested-session/receipt-2026-11-13.json` is the retained receipt of the current
clean-source run: **193** model theorems, every axiom inside the standard allow-list:
- 43 theorems use no axiom, including the table check's decided controls and its definitional
  inversion lemmas, `stackState_within_the_limit`, `dualAction_is_involutive`, `StackAction.dual_dual`,
  `lookup_mem`, and the erasure equations `pact_label_set_erases_to_the_fixture` and
  `vine_source_forms_denote_the_same_fixture`;
- 87 use `propext` alone. They include:
  - the bounded return-stack and external-choice trace/control theorems;
  - the step-duality theorems;
  - the eight explicit-premise progress theorems and controls;
  - three of the type-equality theorems and `lookup_isSome_iff`;
  - `nodeWidth_le_width`;
  - the label-set traces;
  - the repeated-label refusal theorems and the table-check controls that compare a fault against a
    `TableCheck` constructor;
- 61 use `propext` and `Quot.sound`. Among them:
  - `full_mailbox_is_unchanged` and `step_acts_only_inside_the_current_protocol`;
  - the configuration-step bridges, and the type-equality soundness and witness theorems;
  - `exhaustion_is_refused`;
  - 18 of the 19 work-bound theorems;
  - the label-set equality and duality fixtures;
  - the table check's acceptance soundness lemmas (`validateEntry_eq_none_iff`,
    `wellFormed_tables_have_unique_labels`, `validateNode_none_namedIndices`,
    `wellFormed_tables_have_in_range_indices`) and the Pact duality and Vine fixture theorems.

  The label-set cases moved four theorems here from `propext` alone: `stackStep_eq_protoStep`,
  `mem_enabled_iff`, `runStack_eq_runProto` and `assumed_pairs_observe_alike`.
- 2 also use `Classical.choice`: `advance_preserves_the_endpoint_identity` and
  `steppers_preserve_the_endpoint_identity`.

`verification/nested-session/receipt.json` is the earlier 157-theorem receipt of the label-set commit
`fea76dad2`, at Lean 4.31.0; it is retained unchanged. Earlier versions of this paragraph said
`propext` was the only reported axiom and that no work bound was proved sufficient; the receipts never
supported the first, and `protocolEquality_not_exhausted`/`protocolEquality_decides` refute the second.
The current receipt's `bindings` block names the protocol definitions, the endpoint identity fields and
the ledger-based queue accounting as the model's own constants (bound through the source hash), records
`helper`, `runtime` and `peer` as explicitly absent with the reason, and its `claims` block labels each
verdict by kind.

## Claims, by kind

The receipt's `claims` block keeps the results apart rather than reporting one as another:

| Kind | Established here | Not established |
|---|---|---|
| communication safety | every successful step is a resolved current node declaring that action, and its sole successor is that node's declared continuation (`step_acts_only_inside_the_current_protocol`); no send observes a peer; no endpoint is duplicated; duality is equality with the dual table | peer authenticity, payload ownership transfer, endpoint delegation |
| deadlock freedom | **only under explicit premises**, and only for one guest step: `progress_under_explicit_premises` assumes the scheduler grant, host admission, outbox room, or an accounted inbox payload (and the peer's selected branch for `receiveBranch`) | deadlock freedom for a whole session, fairness, eventual delivery, network reliability |
| termination | the table check and the equality/duality exploration terminate for every declared bound, and an exhausted bound is refused (`exhaustion_is_refused`) | termination of a session or of any runtime; no scheduler or peer is modelled |
| resource bounds | definition count, constructor arity, label-set width and every named index are checked at admission; mailbox depth and return-stack length are bounded by declared limits, and every successful step stays within them | memory, time or host quotas; the equality bound is sufficient but grows exponentially in the return-stack limit |

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
declared bound below it can still be refused as exhaustion but a bound at or above it decides. Choice
over a label set is modelled (`select`/`branch`); a repeated label is refused by the admission check
(`wellFormed_tables_have_unique_labels`), and the accessor's first-pair equation is only a fact about
the rejected table. The **Pact adjunct is default off**: it attaches factors to an annotated table
whose only meaning is its erasure to the plain skeleton, and it adds no utility function, decision
rule, solver, agent model, game-theoretic claim or equilibrium result. The **Vine comparison is design
only** ([`vine-comparison.md`](vine-comparison.md)): neither source form is implemented, and
`vine_source_continuation_mismatch_is_refused` is a model control, not a source-to-model
correspondence. What stays open is the correspondence receipt's lowering/runtime/helper revisions:
this change ships none of them, and WIT-SESSION-06 requires the binding once a lowering, runtime or
helper exists rather than binding a revision that does not.

### Pact adjunct: obligations a factor-carrying profile would add

The adjunct establishes only that factors erase to the skeleton. A profile that gave them meaning would
add, at least:

- a factor well-formedness rule (every factor names a declared label at a declared state, and the
  world value is typed) with its own refusal class;
- a utility semantics over factors and a decision rule with its own refusal class, so a selected label
  is distinguishable from a merely declared one;
- an agent model for the deciding side, including what a self-interested peer may choose;
- a proof that factors never grant an endpoint action, never change a trace, verdict or work bound,
  and never act as authority — the property this section proves only for the erased skeleton;
- any game-theoretic or equilibrium claim kept separate from communication safety and never reported
  as one (Pact's own semantics are unpublished);
- a check that the annotation stays default off in the profile and in every gate that consumes it.

The `claims` block names each verdict by kind: communication safety, deadlock freedom under explicit
premises, termination of the checks under their declared bounds, resource bounds, progress,
cancellation, nested recursion, type equality, and the default-off adjuncts. The fixtures are **model
traces only**: nothing here executes emitted Wasm or a native transport.
