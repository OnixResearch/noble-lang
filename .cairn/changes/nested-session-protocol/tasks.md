## Phase 1: Design

- [x] [serial] Specify the finite versioned grammar with named nested constructors, the bounded equality/unfolding/duality algorithm and its refusal classes. r[WIT-SESSION-01]
- [x] [serial] Specify the opaque move-only endpoint resource with identity, role, protocol identity and state, and reject copy, discard, capture, stale generation and double close. r[WIT-SESSION-02]
- [x] [serial] Specify the asynchronous operational model with distinct request, admission, commit, observation, dequeue, close, cancellation and cleanup events and no send-as-receipt. r[WIT-SESSION-03]
- [x] [serial] Specify ordered-stack and latent-effect signatures, single-endpoint continuation advancement, composition with quote, compose and run, and excluded endpoint delegation. r[WIT-SESSION-04]
- [x] [serial] Specify the Lean obligations for duality/state preservation, no action outside the protocol, no duplication, and the scoped progress statement. r[WIT-SESSION-05]
- [x] [serial] Specify the correspondence route reusing the trace discipline and interleaving infrastructure, keeping model theorems separate. r[WIT-SESSION-06]
- [x] [serial] Record the explicit comparison with the existing Task and callback ownership showing reused and new capabilities. r[WIT-SESSION-02]

## Phase 2: Model and fixtures

- [x] [serial] Implement the grammar, the terminating equality/unfolding/duality algorithm and the endpoint resource in Lean for the selected closed model. r[WIT-SESSION-01] r[WIT-SESSION-02]
  Work bound 2026-10-30: a bound computed from the two tables alone, `workBound` (one plus twice
  the number of reachable configuration pairs), is proved sufficient for every pair of tables
  (`protocolEquality_not_exhausted`). Under it the check returns `equal` exactly when the two states
  show the same traces and orderly ends (`protocolEquality_decides`), and likewise for duality
  (`protocolDuality_decides`). The bound is sufficient, not tight: it grows exponentially in the
  return-stack limit. A declared bound below it can still be refused as exhaustion.
  Ticked 2026-10-30: `protocolEquality` decides strong equality of guest-visible traces and orderly
  ends over the stack-aware semantics (`stackStep_eq_protoStep`) by a memoised pair exploration that
  spends one unit of a declared work bound per pair, so it terminates for every bound. An exhausted
  bound is the refusal `exhausted` (`exhaustion_is_refused`); `equal` is proved to be trace
  equivalence (`protocolEquality_sound`) and `differ` carries a proved witness
  (`protocolEquality_witness`). Duality is equality with the dual table (`protocolDuality_sound`). No
  bound is proved sufficient for every protocol, so a too-small declared bound is refused rather
  than decided.
  Un-ticked 2026-10-29: the grammar, bounded unfolding, duality and the endpoint resource exist
  (`proofs/m6/NestedSession.lean`), but no protocol type-equality algorithm is defined. The legacy
  depth-only `resolve` path drops nested returns; the stack-aware operand now saves and pops the
  nested parameter under a declared return-stack limit.
- [x] [serial] Prove duality/state preservation, no action outside the current protocol and no endpoint duplication. r[WIT-SESSION-05]
  Complete at model level: duality is definitional and involutive; state preservation is proved once from
  the one step helper every stepper delegates to (`advance_preserves_the_endpoint_identity`); copy,
  discard and capture are refused; no action outside the current protocol is proved once for every guest
  action (`step_acts_only_inside_the_current_protocol`, with the typed send/receive and internal choice
  steppers defined as that operand); and the inventory bound is stated over every modelled operation
  (`every_operation_preserves_the_endpoint_inventory`), including the two that carry no transition, with
  `duplicating_operation_breaks_the_bound` showing the bound load-bearing. The scope is the operations
  this model enumerates: a future transition outside `Operation` would need its own case.
- [x] [serial] Prove the scoped progress statement under the explicit queue, peer, scheduler and host premises. r[WIT-SESSION-05]
  Ticked 2026-10-30: `progress_under_explicit_premises` gives one guest step at an open owned endpoint
  whose resolved node declares the action, when the scheduler grants the step, the host admits it, and
  the queue premise (outbox room for `send`/`choose`) or peer premise (an accounted inbox payload, and the
  selected branch for `receiveBranch`) holds; `stepIn` takes the guest step and the environment change
  together. One control per premise shows it load-bearing. The premises are assumed per step, so
  termination, fairness, eventual delivery and deadlock freedom stay unclaimed.
  Un-ticked 2026-10-29: the theorem below carries none of the queue, peer, scheduler or host premises
  this task names; it is guest-local only.
  What exists is a deliberately narrow statement: `guest_local_progress_on_a_declared_action` gives a successor
  at an open owned endpoint whose resolved node declares the requested guest action, under exactly
  those three premises. Peer behaviour, scheduling, queue capacity and delivery are **outside** it, and
  `offer_state_declares_only_its_branch_receipt` / `finished_state_declares_no_guest_action` name the
  external-choice and terminal states. Termination, fairness, eventual delivery, network reliability and deadlock
  freedom stay separate and unclaimed.
- [x] [serial] Add the positive traces for send/receive sequencing, both choices, recursive entry/return, orderly close and bounded asynchronous buffering. r[WIT-SESSION-03]
  Complete at model level (2026-10-29): external choice is now taken by the guest action
  `receiveBranch`, and both branches of `offerProtocol` reach their own typed continuations
  (`trace_external_choice_branch_zero`, `trace_external_choice_branch_one`); internal choice has
  positive traces for both declared branches (`trace_internal_choice_branch_zero`,
  `trace_internal_choice_branch_one`); `step_on_the_dual_table` proves every step has the matching
  dual step. A choice node declares one branch, so a two-branch choice is two declared nodes; a
  label-set node is not modelled. Recursive entry/return is now proved by the bounded
  `stackStep` operand, including depth-one/depth-two traces and balanced/unbalanced controls. The
  "return half" below remains the separate send-then-receive control. The balanced table's unbounded
  trace language `sendⁿ·recvⁿ` is not regular and needs the return stack; this model admits it only up
  to the declared stack limit.
  What exists at model level: sequencing, bounded recursive entry/return, orderly close once, bounded
  buffering stopping at the limit, the send-then-receive return half, and internal choice traces where
  two branches are two different traces over the event vocabulary (`choiceEvents`, the choice-state
  stepper `stepChoose` and its declared/undeclared-branch controls), and external-choice receipt with
  its undeclared-branch and typed-continuation controls.
  Update (2026-10-30): choice over a label set is now modelled. A `select` (internal) or `branch`
  (external, its dual) node declares a list of `(label, continuation)` pairs at one state, and the first
  pair naming a label is taken.
  - Fixtures: every label is enabled at one state and reaches its own typed continuation
    (`label_set_enables_every_label`, `label_set_takes_each_branch`); an undeclared label is refused
    (`undeclared_label_is_refused`).
  - Equality and duality decide label sets: `label_order_is_not_observable`,
    `a_missing_label_is_distinguished`, `label_set_duality`.
  - The work bound now scales with the table width, the largest label set and at least two
    (`enabled_length_le_width`). The two-successor bound was false for a three-label state
    (`a_wide_choice_needs_its_width`).
- [x] [serial] Add one ordinary finite-handshake control beside the non-regular nested fixture and record whether the nested fixture executes or is only a model trace. r[WIT-SESSION-05]
  Complete at model level: `handshake_resolves_without_unfolding`, `handshake_trace_advances` and
  `nested_entry_refused_at_depth_zero` measure the legacy depth-only control beside the nested fixture
  `balancedProtocol`. Its unbounded `sendⁿ·recvⁿ` trace language is not regular and needs a return stack; this
  model admits it only up to the declared stack limit. Recorded explicitly: **every** fixture in this
  model is a model trace — nothing executes emitted Wasm or a native transport.
- [x] [serial] Add the negative controls for wrong type or order, wrong branch, endpoint copy or discard, stale identity, double close, receive-after-close, full-queue misaccounting, forged peer response and omitted cleanup. r[WIT-SESSION-02] r[WIT-SESSION-03]
  Complete at model level: copy, discard, capture, stale generation, double close, receive-after-close,
  a forged peer response without a dequeue, an omitted cleanup, wrong ordered message type, wrong order in
  both directions, a wrong internal branch, and full-queue misaccounting with its dequeue/accounting pair
  (`bounded_admission_is_bounded`, `admission_at_the_limit_is_refused`, `dequeue_restores_admission_room`)
  plus the `unchecked_admission_breaks_the_bound` control.
- [x] [serial] Preserve the committed trace prefix across cancellation, trap, quota exhaustion and peer disappearance, retaining unresolved cleanup facts. r[WIT-SESSION-06]
- [ ] [serial] Bind protocol definitions, endpoint identities, queue limits and lowering/runtime/helper revisions in a retained correspondence receipt. r[WIT-SESSION-06]
  Partial: the retained receipt binds the model revision, names the closed definitions fixtures, the six
  endpoint identity fields and the ledger-based queue accounting, and records helper/runtime/peer as null
  with the reason. Lowering/runtime/helper revisions cannot be bound because no correspondence route
  exists, so the task stays open.
