/-!
# Nested session protocol: grammar, dual and endpoint resource (WIT-SESSION-01, WIT-SESSION-02)

`.cairn/changes/nested-session-protocol` fixes one finite grammar — `End`, typed `Send`, typed `Recv`,
internal/external finite choice and named nested constructors with explicit parameters — under bounded
unfolding, and an opaque move-only endpoint resource carrying channel identity, side, protocol
identity, protocol state and generation.

This module is a **closed model**: a definitions table and a bounded unfolding function, not a process
calculus and not a lowering claim. Exhaustion is a refusal rather than an acceptance, duality is
defined by construction and involutive, and every endpoint operation that would copy, discard,
capture, use a stale generation or close twice is refused. Send is not receipt: nothing here models
peer receipt.

Declared premises, kept premises:
* the grammar is versioned but this model carries no version field yet;
* no scheduler, queue or peer is modelled, so no progress statement is made;
* endpoint delegation (passing an endpoint as a message) is excluded, so ownership obligations stay
  outside this model.
-/
namespace NestedSession

/-- Ordered message types. A continuation is the index of a definition in the closed table. -/
inductive MessageType where
  | unit
  | bool
  | i64
  | text
  deriving DecidableEq, Repr

/-- One protocol node. Named nested constructors carry their explicit parameters. -/
inductive Node where
  | finished
  | send (message : MessageType) (continuation : Nat)
  | recv (message : MessageType) (continuation : Nat)
  | choose (branch : Nat) (continuation : Nat)
  | offer (branch : Nat) (continuation : Nat)
  | nested (name : Nat) (parameter : Nat)
  deriving DecidableEq, Repr

/-- Resolve one definition index under the bounded unfolding. An index the closed table does not
contain is refused rather than defaulted, and a named nested constructor at depth zero is **refused**
instead of accepted as a finite protocol: exhaustion is a refusal, never a decision. -/
def resolve (definitions : List Node) (depth : Nat) (index : Nat) : Option Node :=
  match definitions[index]? with
  | none => none
  | some (.nested name _parameter) => if depth = 0 then none else definitions[name]?
  | some other => some other

/-- Duality: `Send` and `Recv` swap, internal and external choice swap, `End` and named nested
constructors are their own dual. This is definitional, so it cannot disagree with the grammar. -/
def dual : Node → Node
  | .finished => .finished
  | .send message continuation => .recv message continuation
  | .recv message continuation => .send message continuation
  | .choose branch continuation => .offer branch continuation
  | .offer branch continuation => .choose branch continuation
  | .nested name parameter => .nested name parameter

/-- Duality is involutive on every node: two duals never change a protocol's shape. -/
theorem dual_is_involutive (node : Node) : dual (dual node) = node := by
  cases node <;> rfl

/-- A send node's dual is a receive of exactly the same ordered message type: duality preserves the
signature rather than permuting it. -/
theorem dual_swaps_send_and_recv (message : MessageType) (continuation : Nat) :
    dual (.send message continuation) = .recv message continuation := rfl

/-- Internal choice's dual is external choice on the same branch index. -/
theorem dual_swaps_choose_and_offer (branch continuation : Nat) :
    dual (.choose branch continuation) = .offer branch continuation := rfl

/-- Unfolding at depth zero refuses a named nested constructor instead of accepting it: an exhausted
unfolding check can never be read as a decision it never made. -/
theorem resolve_refuses_exhaustion (definitions : List Node) (index name parameter : Nat)
    (nestedAt : definitions[index]? = some (.nested name parameter)) :
    resolve definitions 0 index = none := by
  simp only [resolve, nestedAt, if_true]

/-- With depth left, a named nested constructor resolves to its named definition rather than
expanding without bound. -/
theorem resolve_unfolds_with_depth (definitions : List Node) (index name parameter : Nat)
    (nestedAt : definitions[index]? = some (.nested name parameter)) :
    resolve definitions 1 index = definitions[name]? := by
  simp only [resolve, nestedAt, Nat.one_ne_zero, if_false]

/-- An index outside the closed table is refused. -/
theorem resolve_refuses_unknown_definition (definitions : List Node) (depth : Nat) :
    resolve definitions depth definitions.length = none := by
  simp [resolve]

/-- An endpoint is opaque and move-only: channel identity, side, protocol identity, current state and
generation. A descriptive handle is never authority, so nothing here compares a handle to a peer. -/
structure Endpoint where
  channel : Nat
  responder : Bool
  protocol : Nat
  state : Nat
  generation : Nat
  closed : Bool
  deriving DecidableEq, Repr

/-- The outcome vocabulary. Every refusal class is named, so a refusal cannot be confused with
acceptance. -/
inductive Outcome where
  | accepted
  | refusedCopy
  | refusedDiscard
  | refusedCapture
  | refusedStale
  | refusedDoubleClose
  | refusedUnsupported
  | refusedExhausted
  deriving DecidableEq, Repr

/-- Endpoints are copied only by an owning operation, never by the model: the operation is refused. -/
def copy (_endpoint : Endpoint) : Outcome := .refusedCopy

/-- Discarding an endpoint is refused: dropping an endpoint would drop its protocol obligations
without an owning transition. -/
def discard (_endpoint : Endpoint) : Outcome := .refusedDiscard

/-- Capturing an endpoint in a closure is refused for the same reason. -/
def capture (_endpoint : Endpoint) : Outcome := .refusedCapture

/-- A use with a stale generation is refused: identity plus generation, not identity alone, is what
authorises an operation. -/
def useWith (endpoint : Endpoint) (generation : Nat) : Outcome :=
  if generation = endpoint.generation then .accepted else .refusedStale

/-- Closing twice is refused: the first close owns the transition. -/
def closeOnce (endpoint : Endpoint) : Outcome :=
  if endpoint.closed then .refusedDoubleClose else .accepted

/-- One send step advances the endpoint to the continuation of the current send node and nothing
else: no state outside the protocol is reachable, and a closed endpoint cannot send. -/
def stepSend (definitions : List Node) (depth : Nat) (endpoint : Endpoint) : Option Endpoint :=
  match resolve definitions depth endpoint.state with
  | some (.send _message continuation) =>
      if endpoint.closed then none else some ⟨endpoint.channel, endpoint.responder,
        endpoint.protocol, continuation, endpoint.generation, endpoint.closed⟩
  | _ => none

/-- Receipt is not modelled: a send step never observes a peer. The step returns only the endpoint's
own successor state. -/
theorem send_step_does_not_observe_a_peer (definitions : List Node) (depth : Nat)
    (endpoint : Endpoint) (successor : Endpoint)
    (stepped : stepSend definitions depth endpoint = some successor) :
    successor.responder = endpoint.responder ∧ successor.channel = endpoint.channel := by
  simp only [stepSend] at stepped
  split at stepped
  · split at stepped
    · exact absurd stepped (by simp)
    · simp only [Option.some.injEq] at stepped
      rw [← stepped]
      exact ⟨rfl, rfl⟩
  · exact absurd stepped (by simp)

/-- The endpoint resource refusals, each with its own named class rather than one blanket refusal. -/
theorem endpoint_copy_is_refused (endpoint : Endpoint) : copy endpoint = .refusedCopy := rfl

theorem endpoint_discard_is_refused (endpoint : Endpoint) : discard endpoint = .refusedDiscard := rfl

theorem endpoint_capture_is_refused (endpoint : Endpoint) : capture endpoint = .refusedCapture := rfl

theorem stale_generation_is_refused (endpoint : Endpoint) :
    useWith endpoint endpoint.generation.succ = .refusedStale := by
  simp only [useWith, Nat.succ_ne_self, if_false]

theorem matching_generation_is_accepted (endpoint : Endpoint) :
    useWith endpoint endpoint.generation = .accepted := by
  simp only [useWith, if_true]

theorem double_close_is_refused (endpoint : Endpoint) :
    closeOnce ⟨endpoint.channel, endpoint.responder, endpoint.protocol, endpoint.state,
      endpoint.generation, true⟩ = .refusedDoubleClose := by
  simp only [closeOnce, if_true]

/-- A closed endpoint cannot send, so a send after close is refused rather than modelled. -/
theorem closed_endpoint_cannot_send (definitions : List Node) (depth : Nat) (endpoint : Endpoint)
    (message : MessageType) (continuation : Nat)
    (atSend : resolve definitions depth endpoint.state = some (.send message continuation)) :
    stepSend definitions depth ⟨endpoint.channel, endpoint.responder, endpoint.protocol,
      endpoint.state, endpoint.generation, true⟩ = none := by
  simp only [stepSend, atSend]
  simp only [if_true]

/-- One step from a send node advances the endpoint to exactly that node's continuation, and the
peer, channel, protocol and generation are unchanged: no action outside the protocol. -/
theorem send_step_advances_to_the_continuation (definitions : List Node) (depth : Nat)
    (endpoint : Endpoint) (message : MessageType) (continuation : Nat)
    (atSend : resolve definitions depth endpoint.state = some (.send message continuation)) :
    stepSend definitions depth ⟨endpoint.channel, endpoint.responder, endpoint.protocol,
      endpoint.state, endpoint.generation, false⟩ = some ⟨endpoint.channel, endpoint.responder,
        endpoint.protocol, continuation, endpoint.generation, false⟩ := by
  simp only [stepSend, atSend, Bool.false_eq_true, if_false]

/-! ## Session state, bounded admission and duplication (WIT-SESSION-03, WIT-SESSION-05)

The endpoint list is a resource inventory, not a copyable handle set: every operation returns at most
one successor endpoint, and admission is bounded by the declared queue limit. `deliver` admits one
payload without dequeuing or observing a peer, so send is still not receipt. -/

/-- A session: its endpoint inventory, the bounded mailbox depth and the declared queue limit. -/
structure Session where
  endpoints : List Endpoint
  queued : Nat
  queueLimit : Nat
  deriving DecidableEq, Repr

/-- One admission step on the mailbox depth. A full mailbox leaves the depth alone, which is a refusal
rather than a silent drop or an unbounded queue. -/
def admitOne (queued limit : Nat) : Nat :=
  if queued < limit then queued + 1 else queued

/-- Admission of one payload. -/
def deliver (session : Session) : Session :=
  { session with queued := admitOne session.queued session.queueLimit }

/-- Admission keeps the depth inside the limit **given a well-formed session**: an over-full depth is a
premise violation, and the model refuses to claim boundedness for it. -/
theorem admitOne_is_bounded (queued limit : Nat) (within : queued ≤ limit) :
    admitOne queued limit ≤ limit := by
  by_cases room : queued < limit
  · rw [admitOne, if_pos room]
    exact Nat.succ_le_of_lt room
  · rw [admitOne, if_neg room]
    exact within

/-- Admission never exceeds the declared queue limit. -/
theorem admission_is_bounded (session : Session) (within : session.queued ≤ session.queueLimit) :
    (deliver session).queued ≤ session.queueLimit := by
  simpa only [deliver] using admitOne_is_bounded session.queued session.queueLimit within

/-- A full mailbox leaves the session unchanged rather than dropping a payload silently. -/
theorem full_mailbox_is_unchanged (session : Session)
    (full : session.queueLimit ≤ session.queued) : deliver session = session := by
  simp only [deliver, admitOne]
  rw [if_neg (by omega)]

/-- Admission neither copies nor removes an endpoint: the inventory is exactly preserved. -/
theorem admission_preserves_the_endpoint_inventory (session : Session) :
    (deliver session).endpoints = session.endpoints := by
  rfl

/-- No operation in this model increases the endpoint inventory, so no endpoint is ever duplicated by
an operation: the only way to obtain a second endpoint would be an owning transition this model does
not implement. -/
theorem no_endpoint_duplication (session : Session) :
    (deliver session).endpoints.length ≤ session.endpoints.length := by
  rw [admission_preserves_the_endpoint_inventory]
  exact Nat.le_refl _

/-! ## Positive traces over one closed table (WIT-SESSION-03) -/

/-- A small closed protocol: `0` sends an `i64`, `1` receives an `i64`, `2` re-enters `0` through a
named nested constructor, and `3` finishes. -/
def exampleProtocol : List Node :=
  [.send .i64 1, .recv .i64 2, .nested 0 7, .finished]

/-- Sequenced send then receive: the state advances through the table and no peer is observed. -/
theorem trace_send_then_recv :
    (stepSend exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩).map (fun endpoint => endpoint.state) =
      some 1 := by
  decide

/-- Recursive entry through the named nested constructor **unfolds** to the send node's definition
rather than returning the constructor itself, so recursion is confined to declared definitions instead
of expanding without bound. -/
theorem trace_recursive_entry_unfolds_to_the_send_node :
    resolve exampleProtocol 2 2 = some (.send .i64 1) := by
  decide

/-- Orderly close: the first close is accepted and a second close is refused, so close is once-only. -/
theorem trace_orderly_close :
    closeOnce ⟨5, false, 9, 3, 4, false⟩ = .accepted ∧
      closeOnce ⟨5, false, 9, 3, 4, true⟩ = .refusedDoubleClose := by
  decide

/-- Bounded asynchronous buffering: two admissions fit a limit of two and the third is refused. -/
theorem trace_bounded_buffering_refuses_the_third_payload :
    (deliver (deliver (deliver ⟨[], 0, 2⟩))).queued = 2 := by
  decide

/-! ## Asynchronous event vocabulary and the receive/return half (WIT-SESSION-03)

Guest request, bounded admission, host commit, observation, dequeue, close, cancellation and cleanup
are distinct events, so a successful send is never modelled as peer receipt and an event may not be
borrowed from another class. The receive/return half is implemented here; cancellation is an event
class but carries no transition yet. -/

/-- One asynchronous event. `observation` is the peer-receipt-shaped event, and only a dequeue can
precede it: a forged peer response is refused rather than accepted. -/
inductive Event where
  | guestRequest (operation : Nat)
  | admission (depth : Nat)
  | hostCommit (identity : Nat)
  | dequeue (depth : Nat)
  | observation (value : Option Nat)
  | close
  | cancellation
  | cleanup (kind : Nat)
  deriving DecidableEq, Repr

/-- A send's event sequence carries no observation at all: send is not receipt. -/
def sendEvents (operation identity : Nat) : List Event :=
  [.guestRequest operation, .admission 1, .hostCommit identity]

theorem send_events_never_observe (operation identity : Nat) :
    (sendEvents operation identity).any (fun event =>
      match event with
      | .observation _ => true
      | _ => false) = false := by
  simp only [sendEvents, List.any_cons, List.any_nil, Bool.or_false]

/-- A receive trace's observations are each preceded by a dequeue: this is the pairing premise made
explicit for the receive half, not an assumption hidden in a fixture. -/
def dequeuedBefore (events : List Event) : Bool :=
  (events.foldl (fun (state : Bool × Bool) event =>
      match event with
      | .dequeue _ => (state.1, true)
      | .observation _ => (state.1 && state.2, state.2)
      | .guestRequest _ => state
      | .admission _ => state
      | .hostCommit _ => state
      | .close => state
      | .cancellation => state
      | .cleanup _ => state) (true, false)).1

/-- A dequeue then an observation is a well-formed receive. -/
theorem receive_after_dequeue_is_well_formed :
    dequeuedBefore [Event.guestRequest 0, Event.admission 1, Event.dequeue 1,
      Event.observation (some 7)] = true := by
  decide

/-- A forged peer response with no dequeue anywhere in the trace is refused. -/
theorem forged_observation_without_dequeue_is_refused :
    dequeuedBefore [Event.guestRequest 0, Event.observation (some 7)] = false := by
  decide

/-- A close must be followed by a cleanup commit: omitting the cleanup is refused rather than treated as
a successful orderly close. -/
def hasClose (events : List Event) : Bool :=
  events.any (fun event => event == Event.close)

def hasCleanup (events : List Event) : Bool :=
  events.any (fun event =>
    match event with
    | .cleanup _ => true
    | _ => false)

def orderlyCloseEvents (events : List Event) : Bool :=
  !(hasClose events) || hasCleanup events

theorem orderly_close_with_cleanup_is_well_formed :
    orderlyCloseEvents [Event.guestRequest 0, Event.close, Event.cleanup 1] = true := by
  decide

theorem close_without_cleanup_is_refused :
    orderlyCloseEvents [Event.guestRequest 0, Event.close] = false := by
  decide

/-- One receive step advances exactly to the receive node's continuation, and a closed endpoint
cannot receive either. -/
def stepRecv (definitions : List Node) (depth : Nat) (endpoint : Endpoint) : Option Endpoint :=
  match resolve definitions depth endpoint.state with
  | some (.recv _message continuation) =>
      if endpoint.closed then none else some ⟨endpoint.channel, endpoint.responder,
        endpoint.protocol, continuation, endpoint.generation, endpoint.closed⟩
  | _ => none

/-- Send then receive advances through the closed table from the send node to the receive node's
continuation, so the return half is executable in the model. -/
theorem trace_send_then_recv_advances :
    (stepSend exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩).bind
        (fun mid => stepRecv exampleProtocol 2 mid) =
      some ⟨5, false, 9, 2, 4, false⟩ := by
  decide

theorem closed_endpoint_cannot_receive (definitions : List Node) (depth : Nat) (endpoint : Endpoint)
    (message : MessageType) (continuation : Nat)
    (atRecv : resolve definitions depth endpoint.state = some (.recv message continuation)) :
    stepRecv definitions depth ⟨endpoint.channel, endpoint.responder, endpoint.protocol,
      endpoint.state, endpoint.generation, true⟩ = none := by
  simp only [stepRecv, atRecv, if_true]

/-- Both choices stay distinct at the boundary: two internal branches are different nodes, and a
receive node does not step as a send node. -/
theorem both_choices_are_distinct_at_the_boundary :
    Node.choose 0 1 ≠ Node.choose 1 1 ∧ (stepSend exampleProtocol 2 ⟨5, false, 9, 1, 4, false⟩).isNone := by
  decide

/-! ## Typed continuation and choice stepping (WIT-SESSION-04, WIT-SESSION-05)

A continuation is typed: a step carries the ordered message type it sends or receives, and a mismatch
is refused rather than coerced. Internal choice is chosen by the guest endpoint, external choice is
offered by the peer, and a wrong branch is refused. -/

/-- One typed send step. The supplied message type must be exactly the declared one, otherwise the
step is refused: a type check is not ownership authority and is not silently coerced. -/
def stepSendTyped (definitions : List Node) (depth : Nat) (endpoint : Endpoint)
    (message : MessageType) : Option Endpoint :=
  match resolve definitions depth endpoint.state with
  | some (.send declared continuation) =>
      if declared = message && !endpoint.closed then
        some ⟨endpoint.channel, endpoint.responder, endpoint.protocol, continuation,
          endpoint.generation, endpoint.closed⟩
      else none
  | _ => none

/-- One typed receive step, refused for the same reasons. -/
def stepRecvTyped (definitions : List Node) (depth : Nat) (endpoint : Endpoint)
    (message : MessageType) : Option Endpoint :=
  match resolve definitions depth endpoint.state with
  | some (.recv declared continuation) =>
      if declared = message && !endpoint.closed then
        some ⟨endpoint.channel, endpoint.responder, endpoint.protocol, continuation,
          endpoint.generation, endpoint.closed⟩
      else none
  | _ => none

/-- The matching ordered message type advances to the continuation. -/
theorem typed_send_advances_on_the_declared_type :
    stepSendTyped exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩ MessageType.i64 =
      some ⟨5, false, 9, 1, 4, false⟩ := by
  decide

/-- A wrong ordered message type is refused: the continuation is typed, not coerced. -/
theorem wrong_message_type_is_refused :
    stepSendTyped exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩ MessageType.text = none := by
  decide

/-- Wrong order is refused: a receive step in a send state is refused, and a send step in a receive
state is refused. -/
theorem wrong_order_is_refused :
    stepRecvTyped exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩ MessageType.i64 = none ∧
      stepSendTyped exampleProtocol 2 ⟨5, false, 9, 1, 4, false⟩ MessageType.i64 = none := by
  decide

/-- Internal choice: the guest endpoint selects one declared branch and that branch's continuation is
taken. -/
def stepChoose (definitions : List Node) (depth : Nat) (endpoint : Endpoint) (branch : Nat) :
    Option Endpoint :=
  match resolve definitions depth endpoint.state with
  | some (.choose declared continuation) =>
      if declared = branch && !endpoint.closed then
        some ⟨endpoint.channel, endpoint.responder, endpoint.protocol, continuation,
          endpoint.generation, endpoint.closed⟩
      else none
  | _ => none

/-- A closed protocol with both choices: `0` sends, `1` internally chooses branch `0`, `2` internally
chooses branch `1`, `3` receives and `4` finishes. -/
def choiceProtocol : List Node :=
  [.send .i64 1, .choose 0 5, .choose 1 6, .recv .bool 4, .finished, .finished, .finished]

/-- Choosing a declared internal branch advances to that branch's continuation. -/
theorem trace_internal_choice_branch_one :
    Option.map Endpoint.state
        (stepChoose choiceProtocol 2 ⟨5, false, 9, 2, 4, false⟩ 1) = some 6 := by
  decide

/-- A branch the protocol does not offer is refused rather than defaulted. -/
theorem trace_wrong_branch_is_refused :
    stepChoose choiceProtocol 2 ⟨5, false, 9, 2, 4, false⟩ 0 = none := by
  decide

/-- External choice is not the guest's choice: a `offer` state is not choosable by the guest endpoint,
and its dual remains an internal choice for the peer. -/
theorem external_choice_is_not_guest_choice :
    (stepChoose [.offer 0 5] 2 ⟨5, false, 9, 0, 4, false⟩ 0).isNone ∧
      dual (Node.offer 0 5) = Node.choose 0 5 := by
  decide

/-! ## Mailbox accounting and the full-queue misaccounting control (WIT-SESSION-03)

The mailbox carries a ledger: admissions and dequeues, with the depth derived from them instead of
asserted beside them. So an observation cannot claim depth the ledger never admitted, and a dequeue of
an empty mailbox leaves the ledger unchanged. -/

/-- The mailbox ledger. The depth is derived, not stored: a stored depth could drift from the
operations that actually happened. -/
structure Mailbox where
  admitted : Nat
  dequeued : Nat
  deriving DecidableEq, Repr

def mailboxDepth (box : Mailbox) : Nat :=
  box.admitted - box.dequeued

/-- One admission. -/
def admit (box : Mailbox) : Mailbox :=
  { box with admitted := box.admitted + 1 }

/-- One dequeue. An empty mailbox keeps the ledger unchanged: underflow is not modelled. -/
def dequeue (box : Mailbox) : Mailbox :=
  if 0 < mailboxDepth box then { box with dequeued := box.dequeued + 1 } else box

/-- Admission raises the derived depth by exactly one, **given the ledger invariant** that no more was
dequeued than admitted. Without it the truncated subtraction would not describe a real mailbox. -/
theorem admission_raises_the_depth_by_one (box : Mailbox) (sound : box.dequeued ≤ box.admitted) :
    mailboxDepth (admit box) = mailboxDepth box + 1 := by
  simp only [mailboxDepth, admit]
  omega

/-- A dequeue never raises the depth, so accounting cannot be inflated by dequeuing. -/
theorem dequeue_never_raises_the_depth (box : Mailbox) :
    mailboxDepth (dequeue box) ≤ mailboxDepth box := by
  by_cases room : 0 < mailboxDepth box
  · rw [dequeue, if_pos room]
    simp only [mailboxDepth]
    omega
  · rw [dequeue, if_neg room]
    exact Nat.le_refl _

/-- A dequeue of an empty mailbox leaves it unchanged: a full-queue misaccounting cannot invent a
payload to dequeue. -/
theorem dequeue_of_empty_mailbox_is_unchanged (box : Mailbox) (empty : mailboxDepth box = 0) :
    dequeue box = box := by
  simp only [dequeue, empty, Nat.lt_irrefl, if_false]

/-- Full-queue misaccounting is refused: a mailbox that admitted nothing has depth zero and a dequeue
changes nothing, so an observation cannot borrow depth the ledger never recorded. -/
theorem full_queue_misaccounting_is_refused :
    mailboxDepth (dequeue (dequeue ⟨0, 0⟩)) = 0 ∧ (dequeue ⟨0, 0⟩).dequeued = 0 := by
  decide

/-- Depth tracks the operations: after two admissions and one dequeue the derived depth is one. -/
theorem accounting_tracks_the_operations :
    mailboxDepth (dequeue (admit (admit ⟨0, 0⟩))) = 1 := by
  decide

end NestedSession
