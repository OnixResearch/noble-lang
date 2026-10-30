/-!
# Nested session protocol: grammar, dual and endpoint resource (WIT-SESSION-01, WIT-SESSION-02)

`.cairn/changes/nested-session-protocol` fixes one finite grammar — `End`, typed `Send`, typed `Recv`,
internal/external finite choice and named nested constructors with explicit parameters — under bounded
unfolding, and an abstract move-only endpoint resource carrying channel identity, side, protocol
identity, protocol state and generation.

This module is a **closed model**: a definitions table and bounded stepping functions, not a process
calculus and not a lowering claim. Exhaustion is a refusal rather than an acceptance, duality is
defined by construction and involutive, and every endpoint operation that would copy, discard,
capture, use a stale generation or close twice is refused. A stack-aware nested step saves a return
continuation and pops it at a finished node; the stack limit is the only bound on those returns.
Send is not receipt: nothing here models peer receipt. At an external choice the guest takes one
action, `receiveBranch`, which names the branch the peer selected, and a selection the node does not
declare is refused. A `choose` or `offer` node declares one branch; a `select` or `branch` node
declares a whole label set at one state.

Declared premises, kept premises:
* the grammar is versioned but this model carries no version field yet;
* a bounded mailbox is modelled per session, but no scheduler or peer is; the one progress statement
  (`guest_local_progress_on_a_declared_action`) is guest-local, assumes only an open endpoint whose
  state resolves and declares the action, and is not deadlock freedom;
* the depth-only `resolve`/`step` operand is retained as a legacy control, while `stackStep` is the
  nested operand: a recursive entry saves the nested parameter and a finished node pops it. The
  balanced table's unbounded trace language `sendⁿ·recvⁿ` is not regular and needs that return stack;
  this model admits it only up to the declared stack limit, and claims no external runtime stack;
* no protocol type-equality algorithm is defined: `dual` is definitional and endpoint identity is
  compared field by field;
* a `select`/`branch` node carries a label set, and the first pair naming a label is the one taken;
  the exploration's work bound scales with the table's `width`, the largest label set and at least two;
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
  /-- Internal choice over a label set: each `(label, continuation)` pair declares one branch, and
  the first pair naming a label is the one taken. -/
  | select (branches : List (Nat × Nat))
  /-- External choice over a label set, the dual of `select`. -/
  | branch (branches : List (Nat × Nat))
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
  | .select branches => .branch branches
  | .branch branches => .select branches

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

/-- An endpoint is abstract and move-only: channel identity, side, protocol identity, current state and
generation. A descriptive handle is never authority, so nothing here compares a handle to a peer. -/
structure Endpoint where
  channel : Nat
  responder : Bool
  protocol : Nat
  state : Nat
  generation : Nat
  closed : Bool
  deriving DecidableEq, Repr

/-- One step's successor: only the state changes, and a closed endpoint has no successor at all. -/
def advance (endpoint : Endpoint) (continuation : Nat) : Option Endpoint :=
  if endpoint.closed then
    none
  else
    some ⟨endpoint.channel, endpoint.responder, endpoint.protocol, continuation,
      endpoint.generation, endpoint.closed⟩

/-- The identity fields of two endpoints are equal: channel, side, protocol identity and generation. -/
def sameIdentity (left right : Endpoint) : Bool :=
  left.channel == right.channel && left.responder == right.responder &&
    left.protocol == right.protocol && left.generation == right.generation

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
  | some (.send _message continuation) => advance endpoint continuation
  | _ => none

/-- Receipt is not modelled: a send step never observes a peer. The step returns only the endpoint's
own successor state. -/
theorem send_step_does_not_observe_a_peer (definitions : List Node) (depth : Nat)
    (endpoint : Endpoint) (successor : Endpoint)
    (stepped : stepSend definitions depth endpoint = some successor) :
    successor.responder = endpoint.responder ∧ successor.channel = endpoint.channel := by
  simp only [stepSend] at stepped
  split at stepped
  · simp only [advance] at stepped
    split at stepped
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
  simp only [stepSend, atSend, advance, if_true]

/-- One step from a send node advances the endpoint to exactly that node's continuation, and the
peer, channel, protocol and generation are unchanged: no action outside the protocol. -/
theorem send_step_advances_to_the_continuation (definitions : List Node) (depth : Nat)
    (endpoint : Endpoint) (message : MessageType) (continuation : Nat)
    (atSend : resolve definitions depth endpoint.state = some (.send message continuation)) :
    stepSend definitions depth ⟨endpoint.channel, endpoint.responder, endpoint.protocol,
      endpoint.state, endpoint.generation, false⟩ = some ⟨endpoint.channel, endpoint.responder,
        endpoint.protocol, continuation, endpoint.generation, false⟩ := by
  simp only [stepSend, atSend, advance, Bool.false_eq_true, if_false]

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

/-- A small closed-table control: `0` sends an `i64`, `1` receives an `i64`, `2` names `0` through a
nested constructor, and `3` finishes. Its parameter `7` is outside this four-entry table; the
depth-only resolver below retains the old dropped-continuation behaviour. -/
def exampleProtocol : List Node :=
  [.send .i64 1, .recv .i64 2, .nested 0 7, .finished]

/-- Sequenced send then receive: the state advances through the table and no peer is observed. -/
theorem trace_send_then_recv :
    (stepSend exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩).map (fun endpoint => endpoint.state) =
      some 1 := by
  decide

/-- The legacy depth-only resolver unfolds the named nested constructor to the send node's definition
instead of retaining its return parameter; stack-aware entry and return are proved below. -/
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
  | some (.recv _message continuation) => advance endpoint continuation
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
  simp only [stepRecv, atRecv, advance, if_true]

/-! ## No action outside the current protocol (WIT-SESSION-05)

The three typed steppers above are instances of one closed operand. `step` resolves the owned state
against the closed table, takes the continuation that node declares *for that action* and advances;
there is no other successor, and an action the node does not declare (including a mismatched message
type or an undeclared branch) has no continuation to take. -/

/-- The four communication actions a guest endpoint may take on an owned state. `receiveBranch` is the
external-choice action: the guest receives the branch the peer selected, it does not select one. -/
inductive Action where
  | send (message : MessageType)
  | recv (message : MessageType)
  | choose (branch : Nat)
  | receiveBranch (branch : Nat)
  deriving DecidableEq, Repr

/-- The continuation one node declares for one action. `none` is a refusal, never a default, and a
`finished` or unresolved `nested` node declares no guest action. -/
def continuationOf (action : Action) : Node → Option Nat
  | .send declared continuation =>
      match action with
      | .send message => if declared = message then some continuation else none
      | _ => none
  | .recv declared continuation =>
      match action with
      | .recv message => if declared = message then some continuation else none
      | _ => none
  | .choose declared continuation =>
      match action with
      | .choose branch => if declared = branch then some continuation else none
      | _ => none
  | .offer declared continuation =>
      match action with
      | .receiveBranch branch => if declared = branch then some continuation else none
      | _ => none
  | .select branches =>
      match action with
      | .choose label => branches.lookup label
      | _ => none
  | .branch branches =>
      match action with
      | .receiveBranch label => branches.lookup label
      | _ => none
  | .finished => none
  | .nested _ _ => none

/-- One guest action under the closed table. -/
def step (definitions : List Node) (depth : Nat) (endpoint : Endpoint) (action : Action) :
    Option Endpoint :=
  match resolve definitions depth endpoint.state with
  | some node => (continuationOf action node).bind (advance endpoint)
  | none => none

/-! ## Bounded nested return stack (WIT-SESSION-03, WIT-SESSION-05)

The depth-only `resolve`/`step` pair above remains available as a finite-table control. The
stack-aware operand below is the nested transition: `enter` saves the nested node's parameter as a
return index and starts the named definition, while `finish` pops that index. A `skipNested` action
is the optional branch in the balanced fixture; it takes the nested node's return index without
entering it. The stack limit bounds pushes, so no separate unfolding-depth check is needed here.
At a finished node, `finish` with an empty stack is the orderly end and leaves the finished state
unchanged.
-/

/-- A balanced request/response table: node `0` sends, node `1` optionally enters node `0` again or
skips to node `2`, node `2` receives, and node `3` finishes. A recursive entry saves `2`, so each
entry contributes one send and one later receive. -/
def balancedProtocol : List Node :=
  [.send .i64 1, .nested 0 2, .recv .i64 3, .finished]

/-- The endpoint plus its bounded nested return continuations. Existing `Endpoint` literals remain
unchanged; this state carries the additional stack beside an endpoint only for nested traces. -/
structure StackState where
  endpoint : Endpoint
  returnStack : List Nat
  stackLimit : Nat
  deriving DecidableEq, Repr

/-- Guest-visible actions for the stack-aware operand. `skipNested` is the optional non-recursive
branch at a nested call site, and `finish` either returns to a saved index or orderly-ends. -/
inductive StackAction where
  | send (message : MessageType)
  | recv (message : MessageType)
  | choose (branch : Nat)
  | receiveBranch (branch : Nat)
  | enter
  | skipNested
  | finish
  deriving DecidableEq, Repr

/-- Advance the endpoint portion of a stack state while preserving its return stack and limit. -/
def stackAdvance (state : StackState) (continuation : Nat) : Option StackState :=
  (advance state.endpoint continuation).map (fun endpoint => { state with endpoint := endpoint })

/-- A communication continuation changes only the endpoint state, never the saved stack or limit. -/
theorem stackAdvance_keeps_the_stack (state next : StackState) (continuation : Nat)
    (stepped : stackAdvance state continuation = some next) :
    next.returnStack = state.returnStack ∧ next.stackLimit = state.stackLimit := by
  unfold stackAdvance at stepped
  cases h : advance state.endpoint continuation with
  | none => simp [h] at stepped
  | some endpoint =>
      simp [h] at stepped
      cases stepped
      exact ⟨rfl, rfl⟩

/-- A successful endpoint continuation respects the stack limit when its input stack does. -/
theorem stackAdvance_respects_the_limit (state next : StackState) (continuation : Nat)
    (within : state.returnStack.length ≤ state.stackLimit)
    (stepped : stackAdvance state continuation = some next) :
    next.stackLimit = state.stackLimit ∧ next.returnStack.length ≤ next.stackLimit := by
  have kept := stackAdvance_keeps_the_stack state next continuation stepped
  constructor
  · exact kept.2
  · rw [kept.1, kept.2]
    exact within

/-- A stack state constructor with a bounded stack has the corresponding successor bound. -/
theorem stackState_within_the_limit (state next : StackState) (endpoint : Endpoint)
    (stack : List Nat) (within : stack.length ≤ state.stackLimit)
    (same : (⟨endpoint, stack, state.stackLimit⟩ : StackState) = next) :
    next.stackLimit = state.stackLimit ∧ next.returnStack.length ≤ next.stackLimit := by
  cases same
  exact ⟨rfl, by simpa using within⟩

/-- Enter a nested node and report its refusal class alongside the optional successor. A full return
stack is the named exhaustion refusal rather than an indistinguishable successful no-op. -/
def stackEnter (definitions : List Node) (state : StackState) : Outcome × Option StackState :=
  if state.endpoint.closed then
    (.refusedUnsupported, none)
  else
    match definitions[state.endpoint.state]? with
    | some (.nested name parameter) =>
        if state.returnStack.length < state.stackLimit then
          (.accepted,
            some ⟨{ state.endpoint with state := name }, parameter :: state.returnStack,
              state.stackLimit⟩)
        else
          (.refusedExhausted, none)
    | _ => (.refusedUnsupported, none)

/-- A successful nested entry preserves the limit and adds at most one frame. -/
theorem stackEnter_respects_the_limit (definitions : List Node) (state next : StackState)
    (entered : (stackEnter definitions state).2 = some next) :
    next.stackLimit = state.stackLimit ∧ next.returnStack.length ≤ next.stackLimit := by
  by_cases closed : state.endpoint.closed
  · simp [stackEnter, closed] at entered
  · simp only [stackEnter, closed, Bool.false_eq_true, ↓reduceIte] at entered
    split at entered
    · rename_i name parameter
      split at entered
      · simp only [Option.some.injEq] at entered
        subst next
        constructor
        · rfl
        · simp only [List.length_cons]
          omega
      · simp at entered
    · simp at entered

/-- One stack-aware protocol step. A recursive entry pushes the nested parameter, a nested skip
continues to that parameter without pushing, and a finished node pops one saved continuation. -/
def stackStep (definitions : List Node) (state : StackState) (action : StackAction) :
    Option StackState :=
  match definitions[state.endpoint.state]? with
  | some (.send declared continuation) =>
      match action with
      | .send message =>
          if declared = message then stackAdvance state continuation else none
      | _ => none
  | some (.recv declared continuation) =>
      match action with
      | .recv message =>
          if declared = message then stackAdvance state continuation else none
      | _ => none
  | some (.choose declared continuation) =>
      match action with
      | .choose branch =>
          if declared = branch then stackAdvance state continuation else none
      | _ => none
  | some (.offer declared continuation) =>
      match action with
      | .receiveBranch branch =>
          if declared = branch then stackAdvance state continuation else none
      | _ => none
  | some (.select branches) =>
      match action with
      | .choose label =>
          match branches.lookup label with
          | some continuation => stackAdvance state continuation
          | none => none
      | _ => none
  | some (.branch branches) =>
      match action with
      | .receiveBranch label =>
          match branches.lookup label with
          | some continuation => stackAdvance state continuation
          | none => none
      | _ => none
  | some (.nested _ parameter) =>
      match action with
      | .enter => (stackEnter definitions state).2
      | .skipNested => stackAdvance state parameter
      | _ => none
  | some .finished =>
      if state.endpoint.closed then none else
        match action with
        | .finish =>
            match state.returnStack with
            | [] => some state
            | continuation :: rest =>
                some ⟨{ state.endpoint with state := continuation }, rest, state.stackLimit⟩
        | _ => none
  | none => none

/-- Every successful stack step preserves the declared limit and cannot exceed it when the input
stack is within the limit. -/
theorem stack_step_respects_the_limit (definitions : List Node) (state : StackState)
    (action : StackAction) (next : StackState)
    (within : state.returnStack.length ≤ state.stackLimit)
    (stepped : stackStep definitions state action = some next) :
    next.stackLimit = state.stackLimit ∧ next.returnStack.length ≤ next.stackLimit := by
  unfold stackStep at stepped
  (repeat' split at stepped) <;> try simp_all
  all_goals
    try exact stackEnter_respects_the_limit definitions state next stepped
    try exact stackAdvance_respects_the_limit state next _ within stepped
    try
      (refine stackState_within_the_limit state next _ _ ?_ stepped
       omega)

/-- A mutant that retains the real `.enter` refusal and limit check but drops the saved continuation.
It is used only as a load-bearing negative control. -/
def stackStepDroppingReturn (definitions : List Node) (state : StackState) (action : StackAction) :
    Option StackState :=
  match action with
  | .enter =>
      (stackStep definitions state .enter).map (fun next =>
        { next with returnStack := state.returnStack })
  | _ => stackStep definitions state action

/-- Execute a finite trace with the supplied stack stepper. -/
def runStackWith
    (stepper : List Node → StackState → StackAction → Option StackState)
    (definitions : List Node) : StackState → List StackAction → Option StackState
  | state, [] => some state
  | state, action :: actions =>
      (stepper definitions state action).bind
        (fun next => runStackWith stepper definitions next actions)

/-- Execute a finite stack-aware trace. -/
def runStack (definitions : List Node) : StackState → List StackAction → Option StackState :=
  runStackWith stackStep definitions

/-- Keep only traces that reach a finished node with no saved continuation. A stack transition can
return a state before the trace's orderly end; such a trace is refused here. The finished node is
looked up in the supplied definitions rather than being a fixture-specific state index. -/
def acceptedStackTraceWith
    (stepper : List Node → StackState → StackAction → Option StackState)
    (definitions : List Node) (initial : StackState) (trace : List StackAction) : Option StackState :=
  match runStackWith stepper definitions initial trace with
  | some final =>
      match definitions[final.endpoint.state]? with
      | some .finished => if final.returnStack = [] then some final else none
      | _ => none
  | none => none

/-- Accepted trace using the real bounded return-stack stepper. -/
def acceptedStackTrace (definitions : List Node) (initial : StackState)
    (trace : List StackAction) : Option StackState :=
  acceptedStackTraceWith stackStep definitions initial trace

/-- Accepted trace using the dropped-continuation mutant. -/
def acceptedStackTraceDroppingReturn (definitions : List Node) (initial : StackState)
    (trace : List StackAction) : Option StackState :=
  acceptedStackTraceWith stackStepDroppingReturn definitions initial trace

/-- The stack-aware entry/return operand is separate from the depth-only legacy `resolve` operand. -/
theorem stack_entry_and_return_at_depth_one :
    runStack balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .skipNested, .recv .i64, .finish,
          .recv .i64, .finish] =
      some ⟨⟨5, false, 9, 3, 4, false⟩, [], 2⟩ := by
  decide

/-- Two nested entries save two return indices and pop them in last-in-first-out order. -/
theorem stack_entry_and_return_at_depth_two :
    runStack balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .enter, .send .i64, .skipNested,
          .recv .i64, .finish, .recv .i64, .finish, .recv .i64, .finish] =
      some ⟨⟨5, false, 9, 3, 4, false⟩, [], 2⟩ := by
  decide

/-- A return stack of length equal to the declared limit cannot be pushed further. -/
theorem stack_push_beyond_the_limit_is_refused :
    stackStep balancedProtocol
        ⟨⟨5, false, 9, 1, 4, false⟩, [2], 1⟩ .enter = none ∧
      (stackEnter balancedProtocol
        ⟨⟨5, false, 9, 1, 4, false⟩, [2], 1⟩).1 = .refusedExhausted := by
  decide

/-- The stack-aware operand accepts two sends followed by two returns. -/
theorem balanced_trace_send_two_recv_two_is_accepted :
    acceptedStackTrace balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .skipNested, .recv .i64, .finish,
          .recv .i64, .finish] =
      some ⟨⟨5, false, 9, 3, 4, false⟩, [], 2⟩ := by
  decide

/-- A third receive has no matching send in the two-entry trace and is refused. -/
theorem balanced_trace_send_two_recv_three_is_refused :
    acceptedStackTrace balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .skipNested, .recv .i64, .finish,
          .recv .i64, .finish, .recv .i64] = none := by
  decide

/-- Two receives cannot complete a three-entry trace, so the unbalanced trace is refused. -/
theorem balanced_trace_send_three_recv_two_is_refused :
    acceptedStackTrace balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .enter, .send .i64, .skipNested,
          .recv .i64, .finish, .recv .i64, .finish] = none := by
  decide

/-- The dropped-continuation mutant accepts two sends followed by one receive, refuses the balanced
two-send/two-receive trace, and the real stack refuses that same unbalanced trace. -/
theorem dropped_return_continuation_accepts_unbalanced_and_rejects_balanced :
    acceptedStackTraceDroppingReturn balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .skipNested, .recv .i64, .finish] =
      some ⟨⟨5, false, 9, 3, 4, false⟩, [], 2⟩ ∧
    acceptedStackTraceDroppingReturn balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .skipNested, .recv .i64, .finish,
          .recv .i64, .finish] = none ∧
    acceptedStackTrace balancedProtocol
        ⟨⟨5, false, 9, 0, 4, false⟩, [], 2⟩
        [.send .i64, .enter, .send .i64, .skipNested, .recv .i64, .finish] = none := by
  decide

/-- **No action outside the current protocol**: a successful step is exactly a resolved current node
declaring that action, and its sole successor is that node's declared continuation. -/
theorem step_acts_only_inside_the_current_protocol (definitions : List Node) (depth : Nat)
    (endpoint successor : Endpoint) (action : Action)
    (stepped : step definitions depth endpoint action = some successor) :
    ∃ node continuation, resolve definitions depth endpoint.state = some node ∧
      continuationOf action node = some continuation ∧ advance endpoint continuation = some successor := by
  unfold step at stepped
  split at stepped <;> try exact absurd stepped (by simp)
  rename_i node resolved
  rw [Option.bind_eq_some_iff] at stepped
  obtain ⟨continuation, declared, advanced⟩ := stepped
  exact ⟨node, continuation, resolved, declared, advanced⟩

/-- Both choices stay distinct at the boundary: two internal branches are different nodes, and a
receive node does not step as a send node. -/
theorem both_choices_are_distinct_at_the_boundary :
    Node.choose 0 1 ≠ Node.choose 1 1 ∧ (stepSend exampleProtocol 2 ⟨5, false, 9, 1, 4, false⟩).isNone := by
  decide

/-! ## Typed continuation and choice stepping (WIT-SESSION-04, WIT-SESSION-05)

A continuation is typed: a step carries the ordered message type it sends or receives, and a mismatch
is refused rather than coerced. Internal choice is chosen by the guest endpoint, external choice is
offered by the peer, and a wrong branch is refused. -/

/-- One typed send step, as the closed operand's send action. The supplied message type must be
exactly the declared one, otherwise the step is refused: a type check is not ownership authority and
is not silently coerced. -/
def stepSendTyped (definitions : List Node) (depth : Nat) (endpoint : Endpoint)
    (message : MessageType) : Option Endpoint :=
  step definitions depth endpoint (.send message)

/-- One typed receive step, refused for the same reasons. -/
def stepRecvTyped (definitions : List Node) (depth : Nat) (endpoint : Endpoint)
    (message : MessageType) : Option Endpoint :=
  step definitions depth endpoint (.recv message)

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
  step definitions depth endpoint (.choose branch)

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
and its dual remains an internal choice for the peer. The guest's action there is `receiveBranch`. -/
theorem external_choice_is_not_guest_choice :
    (stepChoose [.offer 0 5] 2 ⟨5, false, 9, 0, 4, false⟩ 0).isNone ∧
      dual (Node.offer 0 5) = Node.choose 0 5 := by
  decide

/-- The other declared internal branch of `choiceProtocol`: choosing branch `0` at node `1` advances to
that branch's continuation `5`. -/
theorem trace_internal_choice_branch_zero :
    Option.map Endpoint.state
        (stepChoose choiceProtocol 2 ⟨5, false, 9, 1, 4, false⟩ 0) = some 5 := by
  decide

/-! ## External choice: receiving the peer's selection (WIT-SESSION-03, WIT-SESSION-04)

At an `offer` node the peer selects and the guest receives that selection. The guest's action is
`receiveBranch`; it advances only to the continuation of the branch the node declares. -/

/-- External choice: the guest receives the branch the peer selected. -/
def stepReceiveBranch (definitions : List Node) (depth : Nat) (endpoint : Endpoint) (branch : Nat) :
    Option Endpoint :=
  step definitions depth endpoint (.receiveBranch branch)

/-- A closed protocol offering both branches. One node declares one branch, so the two branches are
two offer nodes: `0` offers branch `0` and continues with an `i64` receive at `2`, `1` offers branch `1`
and continues with a `bool` receive at `3`, and `4` finishes. -/
def offerProtocol : List Node :=
  [.offer 0 2, .offer 1 3, .recv .i64 4, .recv .bool 4, .finished]

/-- Receiving branch `0` reaches branch `0`'s typed continuation, which then completes. -/
theorem trace_external_choice_branch_zero :
    (stepReceiveBranch offerProtocol 2 ⟨5, true, 9, 0, 4, false⟩ 0).bind
        (fun mid => stepRecvTyped offerProtocol 2 mid MessageType.i64) =
      some ⟨5, true, 9, 4, 4, false⟩ := by
  decide

/-- Receiving branch `1` reaches branch `1`'s typed continuation, which then completes. -/
theorem trace_external_choice_branch_one :
    (stepReceiveBranch offerProtocol 2 ⟨5, true, 9, 1, 4, false⟩ 1).bind
        (fun mid => stepRecvTyped offerProtocol 2 mid MessageType.bool) =
      some ⟨5, true, 9, 4, 4, false⟩ := by
  decide

/-- A selection the offer node does not declare is refused rather than defaulted. -/
theorem undeclared_branch_receipt_is_refused :
    stepReceiveBranch offerProtocol 2 ⟨5, true, 9, 0, 4, false⟩ 1 = none := by
  decide

/-- Each received branch keeps its own typed continuation: after branch `0`, a `bool` receive is
refused. -/
theorem external_branch_keeps_its_typed_continuation :
    (stepReceiveBranch offerProtocol 2 ⟨5, true, 9, 0, 4, false⟩ 0).bind
        (fun mid => stepRecvTyped offerProtocol 2 mid MessageType.bool) = none := by
  decide

/-- Receiving a selection is an external-choice action only: at an internal choice it is refused. -/
theorem branch_receipt_is_refused_at_an_internal_choice :
    stepReceiveBranch choiceProtocol 2 ⟨5, false, 9, 1, 4, false⟩ 0 = none := by
  decide

/-- A receipt of any branch other than the one an offer node declares is refused, at every offer node:
when the peer's dual selection declared branch `declared`, receiving `received ≠ declared` has no
continuation. -/
theorem receipt_of_another_branch_is_refused (declared continuation received : Nat)
    (different : received ≠ declared) :
    continuationOf (.receiveBranch received) (.offer declared continuation) = none := by
  simp only [continuationOf]
  rw [if_neg (Ne.symm different)]

/-! ## Duality of steps (WIT-SESSION-01, WIT-SESSION-05)

The peer follows the dual table. A guest send is the peer's receive, and a guest selection is the
peer's receipt of that branch, so each step on one side has the matching step on the other. -/

/-- The dual of a guest action: send and receive swap, and selecting a branch swaps with receiving it. -/
def dualAction : Action → Action
  | .send message => .recv message
  | .recv message => .send message
  | .choose branch => .receiveBranch branch
  | .receiveBranch branch => .choose branch

/-- The action dual is involutive. -/
theorem dualAction_is_involutive (action : Action) : dualAction (dualAction action) = action := by
  cases action <;> rfl

/-- **Duality preserves declared continuations**: for every node and action, the dual action at the dual
node declares exactly the continuation the action declares at the node. -/
theorem continuationOf_dual (action : Action) (node : Node) :
    continuationOf (dualAction action) (dual node) = continuationOf action node := by
  cases node <;> cases action <;> rfl

/-- Looking a state up in the dual table gives the dual of the lookup, including through bounded
unfolding of a named nested constructor. -/
theorem resolve_dual (definitions : List Node) (depth index : Nat) :
    resolve (definitions.map dual) depth index = (resolve definitions depth index).map dual := by
  unfold resolve
  rw [List.getElem?_map]
  cases definitions[index]? with
  | none => rfl
  | some node =>
      cases node with
      | nested name parameter =>
          by_cases exhausted : depth = 0
          · simp [dual, exhausted]
          · simp [dual, exhausted, List.getElem?_map]
      | finished => rfl
      | send message continuation => rfl
      | recv message continuation => rfl
      | choose branch continuation => rfl
      | offer branch continuation => rfl
      | select branches => rfl
      | branch branches => rfl

/-- **A step on one side has the matching step on the dual**: the dual table under the dual action takes
exactly the transition the original table takes under the action, for every table, depth, endpoint
and action. -/
theorem step_on_the_dual_table (definitions : List Node) (depth : Nat) (endpoint : Endpoint)
    (action : Action) :
    step (definitions.map dual) depth endpoint (dualAction action) =
      step definitions depth endpoint action := by
  unfold step
  rw [resolve_dual]
  cases resolve definitions depth endpoint.state with
  | none => rfl
  | some node => simp only [Option.map_some, continuationOf_dual]

/-- The peer's side of `choiceProtocol`: the guest's selection of branch `0` at node `1` is the peer's
receipt of branch `0` on the dual table, reaching the same continuation. -/
theorem dual_table_receives_the_selected_branch :
    stepReceiveBranch (choiceProtocol.map dual) 2 ⟨5, true, 9, 1, 4, false⟩ 0 =
      some ⟨5, true, 9, 5, 4, false⟩ := by
  decide

/-- The stack-aware operand takes the same external choice: receiving branch `0` and its `i64` reach the
finished node with an empty stack, which `acceptedStackTrace` accepts as a complete trace. -/
theorem stack_external_choice_trace_is_accepted :
    acceptedStackTrace offerProtocol ⟨⟨5, true, 9, 0, 4, false⟩, [], 1⟩
        [.receiveBranch 0, .recv .i64] =
      some ⟨⟨5, true, 9, 4, 4, false⟩, [], 1⟩ := by
  decide

/-! ## Mailbox accounting and the full-queue misaccounting control (WIT-SESSION-03)

The mailbox carries a ledger: admissions and dequeues, with the depth derived from them instead of
asserted beside them. So an observation cannot claim depth the ledger never admitted, and a dequeue of
an empty mailbox leaves the ledger unchanged. -/

/-- The mailbox ledger. The depth is derived, not stored: a stored depth could drift from the
operations that actually happened. -/
structure Mailbox where
  enqueued : Nat
  dequeued : Nat
  deriving DecidableEq, Repr

def mailboxDepth (box : Mailbox) : Nat :=
  box.enqueued - box.dequeued

/-- One enqueue into the mailbox ledger. -/
def enqueue (box : Mailbox) : Mailbox :=
  { box with enqueued := box.enqueued + 1 }

/-- One dequeue. An empty mailbox keeps the ledger unchanged: underflow is not modelled. -/
def dequeue (box : Mailbox) : Mailbox :=
  if 0 < mailboxDepth box then { box with dequeued := box.dequeued + 1 } else box

/-- An enqueue raises the derived depth by exactly one, **given the ledger invariant** that no more was
dequeued than admitted. Without it the truncated subtraction would not describe a real mailbox. -/
theorem enqueue_raises_the_depth_by_one (box : Mailbox) (sound : box.dequeued ≤ box.enqueued) :
    mailboxDepth (enqueue box) = mailboxDepth box + 1 := by
  simp only [mailboxDepth, enqueue]
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
    mailboxDepth (dequeue (enqueue (enqueue ⟨0, 0⟩))) = 1 := by
  decide

/-! ## Bounded admission over the ledger (WIT-SESSION-03, the full-queue accounting pair)

The ledger alone says nothing about a queue limit: `enqueue` raises the depth however deep it already
is. The bounded admission below is the only operand that may raise the ledger from a session, and it
refuses at the declared limit. Pairing it with a real dequeue is what makes the refusal depend on
accounted depth rather than on a count of attempts. -/

/-- One bounded admission: at or past the limit it is refused and the ledger stays unchanged, so queue
accounting cannot be inflated past the declared limit. -/
def boundedAdmit (box : Mailbox) (limit : Nat) : Option Mailbox :=
  if mailboxDepth box < limit then some (enqueue box) else none

/-- A bounded admission never derives a depth past the declared limit, **given the ledger invariant**
that no more was dequeued than admitted. -/
theorem bounded_admission_is_bounded (box : Mailbox) (limit : Nat) (reached : Mailbox)
    (sound : box.dequeued ≤ box.enqueued) (accepted : boundedAdmit box limit = some reached) :
    mailboxDepth reached ≤ limit := by
  unfold boundedAdmit at accepted
  by_cases room : mailboxDepth box < limit
  · rw [if_pos room] at accepted
    simp only [Option.some.injEq] at accepted
    subst accepted
    have unfolded : mailboxDepth box = box.enqueued - box.dequeued := rfl
    simp only [mailboxDepth, enqueue]
    omega
  · rw [if_neg room] at accepted
    exact absurd accepted (by simp)

/-- A mailbox at its limit refuses the next admission instead of recording it. -/
theorem admission_at_the_limit_is_refused (box : Mailbox) (limit : Nat)
    (full : limit ≤ mailboxDepth box) : boundedAdmit box limit = none := by
  unfold boundedAdmit
  rw [if_neg (by omega)]

/-- The dequeue/accounting pair: an admission refused at the limit is accepted after one accounted
dequeue. A model that counted attempts instead of depth would accept it in both places. -/
theorem dequeue_restores_admission_room (box : Mailbox) (limit : Nat) (room : 0 < limit)
    (atLimit : mailboxDepth box = limit) :
    (boundedAdmit box limit).isNone ∧ (boundedAdmit (dequeue box) limit).isSome := by
  have full : limit ≤ mailboxDepth box := Nat.le_of_eq atLimit.symm
  have refused : boundedAdmit box limit = none := admission_at_the_limit_is_refused box limit full
  have nonempty : 0 < mailboxDepth box := by rw [atLimit]; exact room
  have step : mailboxDepth (dequeue box) = mailboxDepth box - 1 := by
    rw [dequeue, if_pos nonempty]
    simp only [mailboxDepth]
    omega
  have dropped : mailboxDepth (dequeue box) < limit := by
    rw [step, atLimit]
    omega
  refine ⟨by rw [refused]; rfl, ?_⟩
  unfold boundedAdmit
  rw [if_pos dropped]
  rfl

/-- The limit is load-bearing: an admission that ignores it derives a depth past the declared bound,
so the bounded operand above is not decoration. -/
def uncheckedAdmit (box : Mailbox) : Mailbox := enqueue box

theorem unchecked_admission_breaks_the_bound :
    mailboxDepth (uncheckedAdmit ⟨0, 0⟩) = 1 ∧ ¬ mailboxDepth (uncheckedAdmit ⟨0, 0⟩) ≤ 0 := by
  decide

/-! ## Committed prefix across cancellation, trap, quota and peer disappearance (WIT-SESSION-06)

Every asynchronous boundary may cut a trace, but it may not rewrite the committed prefix, and a
cleanup that was not observed stays an unresolved fact rather than being filled in. Cancellation is an
event class here but still carries no transition. -/

/-- The asynchronous boundaries this model names. `peerGone` is the peer disappearing, which is not the
guest releasing anything. -/
inductive Boundary where
  | none
  | cancellation
  | trap
  | quotaExhaustion
  | peerGone
  deriving DecidableEq, Repr

/-- The prefix committed before one boundary: everything up to and including the boundary's own event
is retained, and nothing after it is invented. -/
def retainUntilBoundary (boundary : Boundary) (events : List Event) : List Event :=
  match boundary with
  | .none => events
  | .cancellation => events.takeWhile (fun event => event != Event.cancellation)
  | .trap => events
  | .quotaExhaustion => events
  | .peerGone => events

/-- A boundary only shortens the trace: the retained events are a prefix of the full event list. -/
theorem boundary_preserves_the_committed_prefix (boundary : Boundary) (events : List Event) :
    retainUntilBoundary boundary events <+: events := by
  cases boundary
  · exact List.prefix_refl _
  · exact List.takeWhile_prefix _
  · exact List.prefix_refl _
  · exact List.prefix_refl _
  · exact List.prefix_refl _

/-- Cancellation cuts exactly at its own event rather than dropping it, so a cancelled request stays
visible in the retained prefix. -/
theorem cancellation_keeps_its_own_event :
    retainUntilBoundary .cancellation
      [Event.guestRequest 0, Event.admission 1, Event.cancellation, Event.cleanup 1] =
      [Event.guestRequest 0, Event.admission 1] := by
  decide

/-- An unresolved cleanup stays a fact: a close whose cleanup was not observed keeps the pending flag,
whichever boundary cut the trace. -/
def cleanupPending (boundary : Boundary) (events : List Event) : Bool :=
  hasClose (retainUntilBoundary boundary events) && !hasCleanup (retainUntilBoundary boundary events)

theorem peer_disappearance_keeps_the_cleanup_pending :
    cleanupPending .peerGone [Event.guestRequest 0, Event.close] = true := by
  decide

theorem ordinary_close_does_not_keep_the_cleanup_pending :
    cleanupPending .peerGone [Event.guestRequest 0, Event.close, Event.cleanup 1] = false := by
  decide

/-! ## Every modelled operation is covered by the duplication bound (WIT-SESSION-05)

`no_endpoint_duplication` was proved for admission alone, and the duplication task stayed open because
cancellation and cleanup carry no endpoint transition at all. Rather than leave that as an omission in
the prose, the inventory bound is now stated over **every** operation this model has: a session step,
a boundary, or an observed cleanup. The two operations that carry no transition are the interesting
cases, because they are exactly where an unchecked implementation could manufacture an endpoint. -/

/-- Every operation this model has. `boundary` and `cleanupObserved` carry no endpoint transition. -/
inductive Operation where
  | deliver
  | boundary (kind : Boundary)
  | cleanupObserved
  deriving DecidableEq, Repr

/-- Apply one operation. A boundary and an observed cleanup leave the session exactly as it was: they
are events on the trace, not transitions on the endpoints or the queue. -/
def applyOperation (session : Session) : Operation → Session
  | .deliver => deliver session
  | .boundary _ => session
  | .cleanupObserved => session

/-- **Every modelled operation preserves the endpoint inventory**, including the two that carry no
transition, so no operation can manufacture a second endpoint. -/
theorem every_operation_preserves_the_endpoint_inventory (session : Session) (operation : Operation) :
    (applyOperation session operation).endpoints = session.endpoints := by
  cases operation <;> simp only [applyOperation, admission_preserves_the_endpoint_inventory]

/-- Hence no operation increases the endpoint inventory, whether or not it carries a transition. -/
theorem every_operation_keeps_the_endpoint_inventory_bound (session : Session)
    (operation : Operation) :
    (applyOperation session operation).endpoints.length ≤ session.endpoints.length := by
  rw [every_operation_preserves_the_endpoint_inventory]
  exact Nat.le_refl _

/-- The bound is load-bearing: an operation that appends an endpoint duplicates one and breaks it, so
the preserved inventory above is not a statement about an inert model. -/
def duplicatingOperation (session : Session) : Session :=
  { session with endpoints := session.endpoints ++ session.endpoints.take 1 }

theorem duplicating_operation_breaks_the_bound :
    ¬ (duplicatingOperation ⟨[⟨5, false, 9, 0, 4, false⟩], 0, 3⟩).endpoints.length ≤
      (⟨[⟨5, false, 9, 0, 4, false⟩], 0, 3⟩ : Session).endpoints.length := by
  decide

/-- A quota boundary never converts a pending cleanup into a completed one, and it never invents an
observation: the retained prefix carries exactly the events that happened before it. -/
theorem quota_boundary_invents_nothing :
    retainUntilBoundary .quotaExhaustion
        [Event.guestRequest 0, Event.admission 1, Event.cancellation]
      <+: [Event.guestRequest 0, Event.admission 1, Event.cancellation] ∧
    cleanupPending .quotaExhaustion [Event.guestRequest 0, Event.admission 1] = false := by
  decide

/-! ## Choice traces over the event vocabulary, and general state preservation (WIT-SESSION-03/05)

An internal choice is an event with the selected branch, so two different branches are two different
traces rather than one trace with a comment. State preservation is proved once for every step this
model implements instead of once per stepper. -/

/-- The one step helper preserves the endpoint's identity: only the state changes. -/
theorem advance_preserves_the_endpoint_identity (endpoint successor : Endpoint) (continuation : Nat)
    (stepped : advance endpoint continuation = some successor) :
    sameIdentity successor endpoint = true := by
  simp only [advance] at stepped
  split at stepped
  · exact absurd stepped (by simp)
  · simp only [Option.some.injEq] at stepped
    rw [← stepped]
    simp only [sameIdentity, beq_self_eq_true, Bool.and_self]

/-- Every stepper this model implements preserves the endpoint's identity, because each one delegates its
successor to `advance`. -/
theorem steppers_preserve_the_endpoint_identity (endpoint successor : Endpoint)
    (continuation : Nat) (stepped : advance endpoint continuation = some successor) :
    sameIdentity successor endpoint = true :=
  advance_preserves_the_endpoint_identity endpoint successor continuation stepped

/-- An internal choice carries the selected branch as its own event, so two branches are two traces. -/
def choiceEvents (branch : Nat) (continuation : Nat) : List (Nat × Nat) :=
  [(branch, continuation)]

theorem different_branches_are_different_traces :
    choiceEvents 0 5 ≠ choiceEvents 1 5 := by
  decide

/-- A protocol whose node `0` declares internal branch `1`, whose continuation is node `2` (a `bool`
send). One node declares one branch, so a choice trace is a choice node plus its declared branch. -/
def branchingProtocol : List Node :=
  [.choose 1 2, .send .i64 3, .send .bool 3, .finished]

/-- The declared branch reaches its own continuation. -/
theorem branch_one_reaches_its_own_continuation :
    Option.map Endpoint.state
        (stepChoose branchingProtocol 2 ⟨5, false, 9, 0, 4, false⟩ 1) = some 2 := by
  decide

/-- A branch the node does not declare is refused rather than defaulted to the declared one. -/
theorem branch_zero_refuses_a_bool_send :
    stepChoose branchingProtocol 2 ⟨5, false, 9, 0, 4, false⟩ 0 = none := by
  decide

/-- The declared branch's typed continuation is the `bool` send: the branch keeps its own typed
continuation instead of collapsing into a shared state. -/
theorem branch_one_accepts_the_bool_send :
    Option.map Endpoint.state
        (stepSendTyped branchingProtocol 2 ⟨5, false, 9, 2, 4, false⟩ MessageType.bool) = some 3 := by
  decide

/-! ## Finite-handshake control beside the nested fixture (WIT-SESSION-05)

Node `2` of the legacy `exampleProtocol` is a named nested constructor, so it has no resolution at
unfolding depth zero. The ordinary handshake below declares no nested constructor and does resolve
there; that is what this control measures. The unbounded `balancedProtocol` trace language
`sendⁿ·recvⁿ` is not regular and needs the bounded return stack defined above; the theorem below
remains specifically about the legacy depth-only operand.
-/

/-- An ordinary finite handshake: send an `i64`, receive the acknowledgement, finish. -/
def handshakeProtocol : List Node :=
  [.send .i64 1, .recv .i64 2, .finished]

/-- The handshake resolves at unfolding depth zero, node by node. -/
theorem handshake_resolves_without_unfolding :
    resolve handshakeProtocol 0 0 = some (.send .i64 1) ∧
      resolve handshakeProtocol 0 1 = some (.recv .i64 2) ∧
      resolve handshakeProtocol 0 2 = some .finished := by
  decide

/-- The handshake's two guest actions advance through it and finish. -/
theorem handshake_trace_advances :
    (step handshakeProtocol 0 ⟨5, false, 9, 0, 4, false⟩ (.send .i64)).bind
        (fun mid => step handshakeProtocol 0 mid (.recv .i64)) =
      some ⟨5, false, 9, 2, 4, false⟩ := by
  decide

/-- **The legacy nested control is refused at depth zero**: its named entry has no depth-only
resolution there, while the handshake above does. The stack-aware balanced fixture uses its own
bounded return stack rather than this dropped-continuation operand. -/
theorem nested_entry_refused_at_depth_zero :
    resolve exampleProtocol 0 2 = none ∧ resolve exampleProtocol 2 2 = some (.send .i64 1) := by
  decide

/-! ## The scoped progress statement (WIT-SESSION-05)

Progress is stated **only** for the guest endpoint's own step, and only under the premises named in
the theorem: the endpoint is open, its owned state resolves, and the resolved node declares the
requested action. Nothing is claimed about the peer, the scheduler, the queue or delivery - a
receive step here says the guest endpoint may take the step, never that a payload will arrive. A
refused step is a *named* refusal (closed endpoint, unresolved node, a branch the offer does not
declare, wrong type or wrong branch), never an unnamed deadlock.

This is deliberately not deadlock freedom: a frozen peer, participant, scheduler or host leaves the
guest exactly where this statement puts it, and none of those premises is modelled. -/

/-- **The scoped progress statement**: at an open owned endpoint whose resolved node declares the
requested guest action, that action has a successor. The premises are exactly the three the theorem
names; peer behaviour, scheduling and delivery are outside it. -/
theorem guest_local_progress_on_a_declared_action (definitions : List Node) (depth : Nat)
    (endpoint : Endpoint) (action : Action) (node : Node) (continuation : Nat)
    (openState : endpoint.closed = false)
    (resolved : resolve definitions depth endpoint.state = some node)
    (declared : continuationOf action node = some continuation) :
    exists successor, step definitions depth endpoint action = some successor := by
  refine ⟨⟨endpoint.channel, endpoint.responder, endpoint.protocol, continuation,
    endpoint.generation, endpoint.closed⟩, ?_⟩
  unfold step
  simp only [resolved, Option.bind, declared]
  unfold advance
  rw [openState]
  simp

/-- At an external choice the guest's only action is receiving the declared branch: any other action,
and any other branch, has no continuation. Whether the peer ever selects is the peer's premise, not
the guest's progress. -/
theorem offer_state_declares_only_its_branch_receipt (branch continuation : Nat) (action : Action)
    (next : Nat) (declared : continuationOf action (.offer branch continuation) = some next) :
    action = .receiveBranch branch ∧ next = continuation := by
  cases action with
  | send message => simp [continuationOf] at declared
  | recv message => simp [continuationOf] at declared
  | choose chosen => simp [continuationOf] at declared
  | receiveBranch received =>
      simp only [continuationOf] at declared
      split at declared
      · rename_i same
        cases declared
        exact ⟨by rw [same], rfl⟩
      · exact absurd declared (by simp)

/-- A finished protocol declares no guest action either, so progress ends in a named terminal state
rather than in an unnamed refusal. -/
theorem finished_state_declares_no_guest_action (action : Action) :
    continuationOf action .finished = none := by
  cases action <;> rfl

/-! ## Progress under explicit environment premises (WIT-SESSION-05)

`guest_local_progress_on_a_declared_action` is guest-local. The statement below adds the four
environment premises the task names, each as an explicit field of `Environment`:

* **queue**: an outgoing action (`send`, `choose`) needs room in the bounded outbox;
* **peer**: an incoming action (`recv`, `receiveBranch`) needs an accounted payload in the inbox, and
  a branch receipt needs the peer's selection to be that branch;
* **scheduler**: the guest moves only in a step the scheduler grants;
* **host**: the host has not refused the operation (quota, cancellation or teardown).

Under exactly those premises the step succeeds (`progress_under_explicit_premises`), and one control
per premise shows it is load-bearing. The premises are assumed for this one step, never derived, so
this is still not deadlock freedom, fairness, termination or eventual delivery.
-/

/-- The environment one guest step depends on. -/
structure Environment where
  outbox : Mailbox
  outboxLimit : Nat
  inbox : Mailbox
  peerBranch : Option Nat
  scheduled : Bool
  hostAdmits : Bool
  deriving DecidableEq, Repr

/-- The environment change of one action, or a refusal: an outgoing action takes one bounded
admission into the outbox, and an incoming action dequeues one accounted payload, a branch receipt
only when the peer selected that branch. -/
def environmentStep (env : Environment) : Action → Option Environment
  | .send _ => (boundedAdmit env.outbox env.outboxLimit).map fun outbox => { env with outbox }
  | .choose _ => (boundedAdmit env.outbox env.outboxLimit).map fun outbox => { env with outbox }
  | .recv _ =>
      if 0 < mailboxDepth env.inbox then some { env with inbox := dequeue env.inbox } else none
  | .receiveBranch branch =>
      if env.peerBranch = some branch ∧ 0 < mailboxDepth env.inbox then
        some { env with inbox := dequeue env.inbox, peerBranch := none }
      else none

/-- One environment-aware guest step. The scheduler and the host gate it, and both the guest's own
step and the environment change must succeed; neither is taken alone. -/
def stepIn (definitions : List Node) (depth : Nat) (endpoint : Endpoint) (env : Environment)
    (action : Action) : Option (Endpoint × Environment) :=
  if env.scheduled && env.hostAdmits then
    match step definitions depth endpoint action, environmentStep env action with
    | some successor, some next => some (successor, next)
    | _, _ => none
  else none

/-- The queue and peer premises one action needs. -/
def EnvironmentReady (env : Environment) : Action → Prop
  | .send _ => mailboxDepth env.outbox < env.outboxLimit
  | .choose _ => mailboxDepth env.outbox < env.outboxLimit
  | .recv _ => 0 < mailboxDepth env.inbox
  | .receiveBranch branch => env.peerBranch = some branch ∧ 0 < mailboxDepth env.inbox

/-- A ready environment always admits the action's environment change. -/
theorem environmentStep_of_ready (env : Environment) (action : Action)
    (ready : EnvironmentReady env action) : ∃ next, environmentStep env action = some next := by
  cases action with
  | send message =>
      simp only [EnvironmentReady] at ready
      exact ⟨{ env with outbox := enqueue env.outbox }, by simp [environmentStep, boundedAdmit, ready]⟩
  | choose branch =>
      simp only [EnvironmentReady] at ready
      exact ⟨{ env with outbox := enqueue env.outbox }, by simp [environmentStep, boundedAdmit, ready]⟩
  | recv message =>
      simp only [EnvironmentReady] at ready
      exact ⟨{ env with inbox := dequeue env.inbox }, by simp [environmentStep, ready]⟩
  | receiveBranch branch =>
      simp only [EnvironmentReady] at ready
      exact ⟨{ env with inbox := dequeue env.inbox, peerBranch := none },
        by simp [environmentStep, ready]⟩

/-- **The scoped progress statement under explicit premises**: at an open owned endpoint whose
resolved node declares the requested action, the step succeeds when the scheduler grants it, the host
admits it, and the queue or peer premise for that action holds. -/
theorem progress_under_explicit_premises (definitions : List Node) (depth : Nat)
    (endpoint : Endpoint) (env : Environment) (action : Action) (node : Node) (continuation : Nat)
    (openState : endpoint.closed = false)
    (resolved : resolve definitions depth endpoint.state = some node)
    (declared : continuationOf action node = some continuation)
    (scheduled : env.scheduled = true) (host : env.hostAdmits = true)
    (ready : EnvironmentReady env action) :
    ∃ successor next, stepIn definitions depth endpoint env action = some (successor, next) := by
  obtain ⟨successor, guest⟩ := guest_local_progress_on_a_declared_action definitions depth endpoint
    action node continuation openState resolved declared
  obtain ⟨next, environment⟩ := environmentStep_of_ready env action ready
  exact ⟨successor, next, by simp [stepIn, scheduled, host, guest, environment]⟩

/-- A ready environment for the `exampleProtocol` send-then-receive fixture: room for one outgoing
payload and one accounted incoming payload. -/
def readyEnvironment : Environment :=
  ⟨⟨0, 0⟩, 1, ⟨1, 0⟩, none, true, true⟩

/-- **The fixture progresses**: the send takes the outbox's one admission and the receive dequeues
the peer's one payload, so the endpoint reaches the state after the receive. -/
theorem progress_fixture_send_then_receive :
    ((stepIn exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩ readyEnvironment (.send .i64)).bind
        fun after => stepIn exampleProtocol 2 after.1 after.2 (.recv .i64)) =
      some (⟨5, false, 9, 2, 4, false⟩, ⟨⟨1, 0⟩, 1, ⟨1, 1⟩, none, true, true⟩) := by
  decide

/-- **Scheduler premise**: an unscheduled guest takes no step, though everything else is ready. -/
theorem unscheduled_guest_takes_no_step :
    stepIn exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩ { readyEnvironment with scheduled := false }
      (.send .i64) = none := by
  decide

/-- **Host premise**: a host refusal stops the step, though everything else is ready. -/
theorem host_refusal_takes_no_step :
    stepIn exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩ { readyEnvironment with hostAdmits := false }
      (.send .i64) = none := by
  decide

/-- **Queue premise**: a full outbox refuses the send, and the guest stays where it was. -/
theorem full_outbox_refuses_send :
    stepIn exampleProtocol 2 ⟨5, false, 9, 0, 4, false⟩ { readyEnvironment with outbox := ⟨1, 0⟩ }
      (.send .i64) = none := by
  decide

/-- **Peer premise**: with nothing accounted in the inbox the receive stays pending. -/
theorem silent_peer_leaves_receive_pending :
    stepIn exampleProtocol 2 ⟨5, false, 9, 1, 4, false⟩ { readyEnvironment with inbox := ⟨0, 0⟩ }
      (.recv .i64) = none := by
  decide

/-- **Peer premise for external choice**: a payload is queued, but the peer selected branch `1`,
so receiving branch `0` is refused; receiving branch `1` at its own offer node is taken. -/
theorem branch_receipt_needs_the_peer_selection :
    stepIn offerProtocol 2 ⟨5, true, 9, 0, 4, false⟩ { readyEnvironment with peerBranch := some 1 }
        (.receiveBranch 0) = none ∧
      (stepIn offerProtocol 2 ⟨5, true, 9, 1, 4, false⟩ { readyEnvironment with peerBranch := some 1 }
        (.receiveBranch 1)).isSome = true := by
  decide

/-! ## Protocol type equality and duality under a declared work bound (WIT-SESSION-01)

Two protocol states are *equal types* when they admit exactly the same guest-visible traces, ending
orderly at the same points. In this model `enter`, `skipNested` and `finish` are guest actions, so
they belong to the observable protocol: the equality is strong, not an equi-recursive equality that
silently unfolds nested constructors. Cyclic definitions are still identified up to unfolding, so a
one-node `μX. send. X` equals the same protocol written with two nodes.

`protocolEquality` decides it by exploring pairs of configurations (definition index, return stack)
with a memo of the pairs already assumed equal. Every step of the exploration spends one unit of the
declared work bound, so the check terminates for every bound, and a bound that runs out is the
refusal `exhausted`, never `equal`. Both decided verdicts are proved: `equal` gives trace
equivalence (`protocolEquality_sound`), and `differ` carries a trace on which the two protocols
observably disagree (`protocolEquality_witness`). Duality is equality with the dual table
(`protocolDuality_sound`). No bound is proved sufficient for every protocol: a declared bound that is
too small is reported as exhaustion, not decided.
-/

/-- A protocol configuration: the current definition index and the saved return indices. -/
abbrev Config := Nat × List Nat

/-- One stack-aware protocol step on a configuration. It is `stackStep` without the endpoint's
identity fields (`stackStep_eq_protoStep`). -/
def protoStep (definitions : List Node) (limit : Nat) (config : Config) (action : StackAction) :
    Option Config :=
  match definitions[config.1]? with
  | some (.send declared continuation) =>
      match action with
      | .send message => if declared = message then some (continuation, config.2) else none
      | _ => none
  | some (.recv declared continuation) =>
      match action with
      | .recv message => if declared = message then some (continuation, config.2) else none
      | _ => none
  | some (.choose declared continuation) =>
      match action with
      | .choose branch => if declared = branch then some (continuation, config.2) else none
      | _ => none
  | some (.offer declared continuation) =>
      match action with
      | .receiveBranch branch => if declared = branch then some (continuation, config.2) else none
      | _ => none
  | some (.select branches) =>
      match action with
      | .choose label =>
          match branches.lookup label with
          | some continuation => some (continuation, config.2)
          | none => none
      | _ => none
  | some (.branch branches) =>
      match action with
      | .receiveBranch label =>
          match branches.lookup label with
          | some continuation => some (continuation, config.2)
          | none => none
      | _ => none
  | some (.nested name parameter) =>
      match action with
      | .enter => if config.2.length < limit then some (name, parameter :: config.2) else none
      | .skipNested => some (parameter, config.2)
      | _ => none
  | some .finished =>
      match action with
      | .finish =>
          match config.2 with
          | [] => some config
          | continuation :: rest => some (continuation, rest)
      | _ => none
  | none => none

/-- **The configuration step is the model's stack step**: on an open endpoint, `stackStep` moves
exactly as `protoStep` does and changes nothing else about the endpoint. -/
theorem stackStep_eq_protoStep (definitions : List Node) (endpoint : Endpoint) (stack : List Nat)
    (limit : Nat) (action : StackAction) (openState : endpoint.closed = false) :
    stackStep definitions ⟨endpoint, stack, limit⟩ action =
      (protoStep definitions limit (endpoint.state, stack) action).map
        (fun config => ⟨{ endpoint with state := config.1 }, config.2, limit⟩) := by
  obtain ⟨channel, responder, protocol, state, generation, closed⟩ := endpoint
  simp only at openState
  subst openState
  unfold stackStep protoStep
  cases lookup : definitions[state]? with
  | none => simp
  | some node =>
      cases node <;> cases action <;>
        simp [stackAdvance, advance, stackEnter, lookup] <;>
        (try split) <;> (try cases stack) <;> simp_all

/-- The actions a configuration admits: one per declared label at a label-set node, and at most two
anywhere else, so each exploration step is finite. -/
def enabled (definitions : List Node) (limit : Nat) (config : Config) : List StackAction :=
  match definitions[config.1]? with
  | some (.send declared _) => [.send declared]
  | some (.recv declared _) => [.recv declared]
  | some (.choose declared _) => [.choose declared]
  | some (.offer declared _) => [.receiveBranch declared]
  | some (.select branches) => branches.map fun entry => .choose entry.1
  | some (.branch branches) => branches.map fun entry => .receiveBranch entry.1
  | some (.nested _ _) => if config.2.length < limit then [.enter, .skipNested] else [.skipNested]
  | some .finished => [.finish]
  | none => []

/-- A label-set lookup succeeds exactly for a declared label. -/
theorem lookup_isSome_iff (branches : List (Nat × Nat)) (label : Nat) :
    (branches.lookup label).isSome = true ↔ ∃ entry ∈ branches, entry.1 = label := by
  induction branches with
  | nil =>
      constructor
      · intro found
        exact absurd found Bool.false_ne_true
      · intro ⟨_, member, _⟩
        cases member
  | cons head rest ih =>
      obtain ⟨key, continuation⟩ := head
      cases same : label == key with
      | true =>
          have equal : label = key := of_decide_eq_true same
          have unfolded : List.lookup label ((key, continuation) :: rest) = some continuation := by
            rw [List.lookup, same]
          rw [unfolded]
          exact ⟨fun _ => ⟨(key, continuation), List.Mem.head _, equal.symm⟩, fun _ => rfl⟩
      | false =>
          have unfolded : List.lookup label ((key, continuation) :: rest) = List.lookup label rest := by
            rw [List.lookup, same]
          rw [unfolded, ih]
          constructor
          · intro ⟨entry, member, named⟩
            exact ⟨entry, List.Mem.tail _ member, named⟩
          · intro ⟨entry, member, named⟩
            cases member with
            | head =>
                have equalBeq : (label == key) = true := by
                  rw [← named]
                  exact decide_eq_true rfl
                rw [same] at equalBeq
                exact absurd equalBeq Bool.false_ne_true
            | tail _ member => exact ⟨entry, member, named⟩

/-- A label-set lookup returns one of the declared continuations. -/
theorem lookup_mem (branches : List (Nat × Nat)) (label continuation : Nat)
    (found : branches.lookup label = some continuation) : continuation ∈ branches.map Prod.snd := by
  induction branches with
  | nil => cases found
  | cons head rest ih =>
      obtain ⟨key, value⟩ := head
      cases same : label == key with
      | true =>
          have unfolded : List.lookup label ((key, value) :: rest) = some value := by
            rw [List.lookup, same]
          rw [unfolded] at found
          cases found
          exact List.Mem.head _
      | false =>
          have unfolded : List.lookup label ((key, value) :: rest) = List.lookup label rest := by
            rw [List.lookup, same]
          rw [unfolded] at found
          exact List.Mem.tail _ (ih found)

/-- An action is listed as enabled exactly when the configuration step takes it. -/
theorem mem_enabled_iff (definitions : List Node) (limit : Nat) (config : Config)
    (action : StackAction) :
    action ∈ enabled definitions limit config ↔ (protoStep definitions limit config action).isSome := by
  unfold enabled protoStep
  cases definitions[config.1]? with
  | none => simp
  | some node =>
      cases node
      case select branches =>
        cases action with
        | choose label =>
            dsimp only
            split
            · rename_i continuation found
              have present : ∃ entry ∈ branches, entry.1 = label :=
                (lookup_isSome_iff branches label).mp (by rw [found]; rfl)
              simp only [Option.isSome_some, iff_true, List.mem_map, StackAction.choose.injEq]
              exact present
            · rename_i found
              have absent : ¬ ∃ entry ∈ branches, entry.1 = label := fun present => by
                have declared := (lookup_isSome_iff branches label).mpr present
                rw [found] at declared
                exact Bool.false_ne_true declared
              simp only [Option.isSome_none, Bool.false_eq_true, iff_false, List.mem_map,
                StackAction.choose.injEq]
              exact absent
        | _ => simp
      case branch branches =>
        cases action with
        | receiveBranch label =>
            dsimp only
            split
            · rename_i continuation found
              have present : ∃ entry ∈ branches, entry.1 = label :=
                (lookup_isSome_iff branches label).mp (by rw [found]; rfl)
              simp only [Option.isSome_some, iff_true, List.mem_map, StackAction.receiveBranch.injEq]
              exact present
            · rename_i found
              have absent : ¬ ∃ entry ∈ branches, entry.1 = label := fun present => by
                have declared := (lookup_isSome_iff branches label).mpr present
                rw [found] at declared
                exact Bool.false_ne_true declared
              simp only [Option.isSome_none, Bool.false_eq_true, iff_false, List.mem_map,
                StackAction.receiveBranch.injEq]
              exact absent
        | _ => simp
      all_goals (cases action <;> simp <;> (try split) <;> simp_all [eq_comm])

/-- The orderly end: a finished node with no saved return index, the acceptance condition of
`acceptedStackTraceWith`. -/
def orderlyEnd (definitions : List Node) (config : Config) : Bool :=
  decide (definitions[config.1]? = some .finished) && config.2.isEmpty

/-- Run a trace of configuration steps. -/
def runProto (definitions : List Node) (limit : Nat) : Config → List StackAction → Option Config
  | config, [] => some config
  | config, action :: rest =>
      (protoStep definitions limit config action).bind fun next => runProto definitions limit next rest

/-- What a trace shows: whether it can be taken, and if so whether it ends orderly. -/
def observe (definitions : List Node) (limit : Nat) (config : Config) (trace : List StackAction) :
    Option Bool :=
  (runProto definitions limit config trace).map (orderlyEnd definitions)

theorem runProto_snoc (definitions : List Node) (limit : Nat) (trace : List StackAction)
    (action : StackAction) : ∀ config, runProto definitions limit config (trace ++ [action]) =
      (runProto definitions limit config trace).bind
        fun last => protoStep definitions limit last action := by
  induction trace with
  | nil => intro config; simp [runProto]
  | cons head rest ih =>
      intro config
      simp only [List.cons_append, runProto]
      cases protoStep definitions limit config head with
      | none => rfl
      | some next => simpa using ih next

/-- **The configuration run is the model's stack run** on an open endpoint. -/
theorem runStack_eq_runProto (definitions : List Node) (limit : Nat) :
    ∀ (trace : List StackAction) (endpoint : Endpoint) (stack : List Nat),
      endpoint.closed = false →
      runStack definitions ⟨endpoint, stack, limit⟩ trace =
        (runProto definitions limit (endpoint.state, stack) trace).map
          (fun config => ⟨{ endpoint with state := config.1 }, config.2, limit⟩) := by
  intro trace
  induction trace with
  | nil =>
      intro endpoint stack _
      cases endpoint
      rfl
  | cons action rest ih =>
      intro endpoint stack openState
      unfold runStack at ih ⊢
      simp only [runStackWith, runProto]
      rw [stackStep_eq_protoStep definitions endpoint stack limit action openState]
      cases protoStep definitions limit (endpoint.state, stack) action with
      | none => rfl
      | some next =>
          simp only [Option.map_some, Option.bind_some]
          exact ih { endpoint with state := next.1 } next.2 openState

/-- The verdict of the equality check. `exhausted` is a refusal: it never reports equality. -/
inductive Verdict where
  | equal
  | differ (witness : List StackAction)
  | exhausted
  deriving DecidableEq, Repr

/-- An action listed on exactly one side, if there is one. -/
def distinguish (left right : List StackAction) : Option StackAction :=
  match left.find? (fun action => !decide (action ∈ right)) with
  | some action => some action
  | none => right.find? (fun action => !decide (action ∈ left))

theorem distinguish_none (left right : List StackAction) (found : distinguish left right = none) :
    ∀ action, action ∈ left ↔ action ∈ right := by
  unfold distinguish at found
  split at found
  · cases found
  · rename_i leftNone
    have onLeft := List.find?_eq_none.mp leftNone
    have onRight := List.find?_eq_none.mp found
    intro action
    constructor
    · intro member
      simpa using onLeft action member
    · intro member
      simpa using onRight action member

theorem distinguish_some (left right : List StackAction) (action : StackAction)
    (found : distinguish left right = some action) :
    (action ∈ left ∧ action ∉ right) ∨ (action ∈ right ∧ action ∉ left) := by
  unfold distinguish at found
  split at found
  · rename_i first leftFound
    cases found
    exact Or.inl ⟨List.mem_of_find?_eq_some leftFound, by simpa using List.find?_some leftFound⟩
  · exact Or.inr ⟨List.mem_of_find?_eq_some found, by simpa using List.find?_some found⟩

/-- A pair still to compare, with the trace that reached it from the start. -/
abbrev Pending := (Config × Config) × List StackAction

/-- The successor pairs of a pair: one per action enabled on the left that both sides take. -/
def successorPairs (left : List Node) (leftLimit : Nat) (right : List Node) (rightLimit : Nat)
    (pair : Config × Config) (trace : List StackAction) : List Pending :=
  (enabled left leftLimit pair.1).filterMap fun action =>
    match protoStep left leftLimit pair.1 action, protoStep right rightLimit pair.2 action with
    | some next, some other => some ((next, other), trace ++ [action])
    | _, _ => none

/-- The exploration. Each pending pair spends one unit of `work`; a pair already assumed equal is
skipped, which is what lets a cyclic protocol be decided. -/
def checkPairs (left : List Node) (leftLimit : Nat) (right : List Node) (rightLimit : Nat) :
    Nat → List (Config × Config) → List Pending → Verdict
  | _, _, [] => .equal
  | 0, _, _ :: _ => .exhausted
  | work + 1, assumed, (pair, trace) :: rest =>
      if pair ∈ assumed then checkPairs left leftLimit right rightLimit work assumed rest
      else if orderlyEnd left pair.1 ≠ orderlyEnd right pair.2 then .differ trace
      else
        match distinguish (enabled left leftLimit pair.1) (enabled right rightLimit pair.2) with
        | some action => .differ (trace ++ [action])
        | none =>
            checkPairs left leftLimit right rightLimit work (pair :: assumed)
              (successorPairs left leftLimit right rightLimit pair trace ++ rest)

/-- **Protocol type equality** of `start` in `left` and `other` in `right` under the declared work
bound `work`. -/
def protocolEquality (left : List Node) (leftLimit : Nat) (right : List Node) (rightLimit : Nat)
    (work : Nat) (start other : Config) : Verdict :=
  checkPairs left leftLimit right rightLimit work [] [((start, other), [])]

/-- **Exhaustion is refused**: a work bound that runs out with a pair still pending is `exhausted`,
never `equal`. -/
theorem exhaustion_is_refused (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (assumed : List (Config × Config)) (entry : Pending) (rest : List Pending) :
    checkPairs left leftLimit right rightLimit 0 assumed (entry :: rest) = .exhausted := rfl

/-- Every assumed pair agrees on its orderly end and its enabled actions, and both sides' successors
lie in `closure`. -/
def Assumed (left : List Node) (leftLimit : Nat) (right : List Node) (rightLimit : Nat)
    (assumed : List (Config × Config)) (closure : Config × Config → Prop) : Prop :=
  ∀ pair ∈ assumed,
    (orderlyEnd left pair.1 = orderlyEnd right pair.2 ∧
      ∀ action, action ∈ enabled left leftLimit pair.1 ↔ action ∈ enabled right rightLimit pair.2) ∧
    ∀ action ∈ enabled left leftLimit pair.1, ∃ next other,
      protoStep left leftLimit pair.1 action = some next ∧
      protoStep right rightLimit pair.2 action = some other ∧ closure (next, other)

/-- Pairs closed under assumption observe every trace alike. -/
theorem assumed_pairs_observe_alike (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (assumed : List (Config × Config))
    (closed : Assumed left leftLimit right rightLimit assumed (· ∈ assumed)) :
    ∀ trace pair, pair ∈ assumed →
      observe left leftLimit pair.1 trace = observe right rightLimit pair.2 trace := by
  intro trace
  induction trace with
  | nil =>
      intro pair member
      obtain ⟨⟨same, -⟩, -⟩ := closed pair member
      simp [observe, runProto, same]
  | cons action rest ih =>
      intro pair member
      obtain ⟨⟨-, sameEnabled⟩, successors⟩ := closed pair member
      by_cases onLeft : action ∈ enabled left leftLimit pair.1
      · obtain ⟨next, other, stepLeft, stepRight, kept⟩ := successors action onLeft
        simpa [observe, runProto, stepLeft, stepRight] using ih (next, other) kept
      · have offRight : action ∉ enabled right rightLimit pair.2 :=
          fun onRight => onLeft ((sameEnabled action).2 onRight)
        rw [mem_enabled_iff] at onLeft offRight
        have noneLeft : protoStep left leftLimit pair.1 action = none := by
          simpa using onLeft
        have noneRight : protoStep right rightLimit pair.2 action = none := by
          simpa using offRight
        simp [observe, runProto, noneLeft, noneRight]

theorem checkPairs_equal (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (work : Nat) :
    ∀ (assumed : List (Config × Config)) (pending : List Pending),
      Assumed left leftLimit right rightLimit assumed
        (fun pair => pair ∈ assumed ∨ ∃ trace, (pair, trace) ∈ pending) →
      checkPairs left leftLimit right rightLimit work assumed pending = .equal →
      ∃ closure : List (Config × Config),
        Assumed left leftLimit right rightLimit closure (· ∈ closure) ∧
          (∀ pair ∈ assumed, pair ∈ closure) ∧ ∀ entry ∈ pending, entry.1 ∈ closure := by
  induction work with
  | zero =>
      intro assumed pending invariant result
      cases pending with
      | nil =>
          refine ⟨assumed, ?_, fun _ member => member, by simp⟩
          intro pair member
          obtain ⟨consistent, successors⟩ := invariant pair member
          refine ⟨consistent, fun action enabledLeft => ?_⟩
          obtain ⟨next, other, stepLeft, stepRight, kept⟩ := successors action enabledLeft
          exact ⟨next, other, stepLeft, stepRight, by simpa using kept⟩
      | cons entry rest => simp [checkPairs] at result
  | succ work ih =>
      intro assumed pending invariant result
      cases pending with
      | nil =>
          refine ⟨assumed, ?_, fun _ member => member, by simp⟩
          intro pair member
          obtain ⟨consistent, successors⟩ := invariant pair member
          refine ⟨consistent, fun action enabledLeft => ?_⟩
          obtain ⟨next, other, stepLeft, stepRight, kept⟩ := successors action enabledLeft
          exact ⟨next, other, stepLeft, stepRight, by simpa using kept⟩
      | cons entry rest =>
          obtain ⟨pair, trace⟩ := entry
          simp only [checkPairs] at result
          split at result
          · rename_i known
            have next : Assumed left leftLimit right rightLimit assumed
                (fun candidate => candidate ∈ assumed ∨ ∃ trace, (candidate, trace) ∈ rest) := by
              intro candidate member
              obtain ⟨consistent, successors⟩ := invariant candidate member
              refine ⟨consistent, fun action enabledLeft => ?_⟩
              obtain ⟨next, other, stepLeft, stepRight, kept⟩ := successors action enabledLeft
              refine ⟨next, other, stepLeft, stepRight, ?_⟩
              rcases kept with inAssumed | ⟨found, foundMember⟩
              · exact Or.inl inAssumed
              · rcases List.mem_cons.mp foundMember with same | inRest
                · simp only [Prod.mk.injEq] at same
                  exact Or.inl (same.1 ▸ known)
                · exact Or.inr ⟨found, inRest⟩
            obtain ⟨closure, closed, keep, cover⟩ := ih assumed rest next result
            refine ⟨closure, closed, keep, ?_⟩
            intro candidate member
            rcases List.mem_cons.mp member with same | inRest
            · rw [same]
              exact keep pair known
            · exact cover candidate inRest
          · split at result
            · cases result
            · rename_i known sameEnd
              split at result
              · cases result
              · rename_i none
                have sameEnabled := distinguish_none _ _ none
                have next : Assumed left leftLimit right rightLimit (pair :: assumed)
                    (fun candidate => candidate ∈ pair :: assumed ∨ ∃ reached,
                      (candidate, reached) ∈
                        successorPairs left leftLimit right rightLimit pair trace ++ rest) := by
                  intro candidate member
                  rcases List.mem_cons.mp member with same | inAssumed
                  · subst same
                    refine ⟨⟨by simpa using sameEnd, sameEnabled⟩, fun action enabledLeft => ?_⟩
                    have enabledRight := (sameEnabled action).1 enabledLeft
                    rw [mem_enabled_iff, Option.isSome_iff_exists] at enabledLeft enabledRight
                    obtain ⟨next, stepLeft⟩ := enabledLeft
                    obtain ⟨other, stepRight⟩ := enabledRight
                    refine ⟨next, other, stepLeft, stepRight, Or.inr ⟨trace ++ [action], ?_⟩⟩
                    apply List.mem_append_left
                    unfold successorPairs
                    rw [List.mem_filterMap]
                    refine ⟨action, ?_, by simp [stepLeft, stepRight]⟩
                    rw [mem_enabled_iff, stepLeft]
                    rfl
                  · obtain ⟨consistent, successors⟩ := invariant candidate inAssumed
                    refine ⟨consistent, fun action enabledLeft => ?_⟩
                    obtain ⟨next, other, stepLeft, stepRight, kept⟩ := successors action enabledLeft
                    refine ⟨next, other, stepLeft, stepRight, ?_⟩
                    rcases kept with inOld | ⟨found, foundMember⟩
                    · exact Or.inl (List.mem_cons_of_mem _ inOld)
                    · rcases List.mem_cons.mp foundMember with same | inRest
                      · simp only [Prod.mk.injEq] at same
                        exact Or.inl (same.1 ▸ List.mem_cons_self)
                      · exact Or.inr ⟨found, List.mem_append_right _ inRest⟩
                obtain ⟨closure, closed, keep, cover⟩ := ih (pair :: assumed) _ next result
                refine ⟨closure, closed, fun candidate member => keep candidate
                  (List.mem_cons_of_mem _ member), ?_⟩
                intro candidate member
                rcases List.mem_cons.mp member with same | inRest
                · rw [same]
                  exact keep pair List.mem_cons_self
                · exact cover candidate (List.mem_append_right _ inRest)

/-- **An `equal` verdict is trace equivalence**: the two protocol states admit the same traces and
end orderly on the same ones. -/
theorem protocolEquality_sound (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (work : Nat) (start other : Config)
    (result : protocolEquality left leftLimit right rightLimit work start other = .equal) :
    ∀ trace, observe left leftLimit start trace = observe right rightLimit other trace := by
  obtain ⟨closure, closed, -, cover⟩ :=
    checkPairs_equal left leftLimit right rightLimit work [] [((start, other), [])]
      (by intro pair member; simp at member) result
  intro trace
  exact assumed_pairs_observe_alike left leftLimit right rightLimit closure closed trace
    (start, other) (cover ((start, other), []) (by simp))

theorem checkPairs_differ (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (start other : Config) (work : Nat) :
    ∀ (assumed : List (Config × Config)) (pending : List Pending) (witness : List StackAction),
      (∀ entry ∈ pending, runProto left leftLimit start entry.2 = some entry.1.1 ∧
        runProto right rightLimit other entry.2 = some entry.1.2) →
      checkPairs left leftLimit right rightLimit work assumed pending = .differ witness →
      observe left leftLimit start witness ≠ observe right rightLimit other witness := by
  induction work with
  | zero =>
      intro assumed pending witness _ result
      cases pending <;> simp [checkPairs] at result
  | succ work ih =>
      intro assumed pending witness reach result
      cases pending with
      | nil => simp [checkPairs] at result
      | cons entry rest =>
          obtain ⟨pair, trace⟩ := entry
          have here := reach (pair, trace) List.mem_cons_self
          simp only at here
          simp only [checkPairs] at result
          split at result
          · exact ih assumed rest witness
              (fun entry member => reach entry (List.mem_cons_of_mem _ member)) result
          · split at result
            · rename_i differs
              cases result
              simp [observe, here.1, here.2, differs]
            · split at result
              · rename_i action found
                cases result
                rcases distinguish_some _ _ _ found with ⟨onLeft, offRight⟩ | ⟨onRight, offLeft⟩
                · rw [mem_enabled_iff, Option.isSome_iff_exists] at onLeft
                  obtain ⟨next, stepLeft⟩ := onLeft
                  rw [mem_enabled_iff] at offRight
                  have stepRight : protoStep right rightLimit pair.2 action = none := by
                    simpa using offRight
                  simp [observe, runProto_snoc, here.1, here.2, stepLeft, stepRight]
                · rw [mem_enabled_iff, Option.isSome_iff_exists] at onRight
                  obtain ⟨next, stepRight⟩ := onRight
                  rw [mem_enabled_iff] at offLeft
                  have stepLeft : protoStep left leftLimit pair.1 action = none := by
                    simpa using offLeft
                  simp [observe, runProto_snoc, here.1, here.2, stepLeft, stepRight]
              · refine ih (pair :: assumed) _ witness ?_ result
                intro entry member
                rcases List.mem_append.mp member with inSuccessors | inRest
                · unfold successorPairs at inSuccessors
                  obtain ⟨action, -, built⟩ := List.mem_filterMap.mp inSuccessors
                  split at built
                  · rename_i next other' stepLeft stepRight
                    cases built
                    exact ⟨by simp [runProto_snoc, here.1, stepLeft],
                      by simp [runProto_snoc, here.2, stepRight]⟩
                  · cases built
                · exact reach entry (List.mem_cons_of_mem _ inRest)

/-- **A `differ` verdict carries a genuine witness**: on that trace the two protocol states
observably disagree, either one admits it and the other does not, or only one ends orderly. -/
theorem protocolEquality_witness (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (work : Nat) (start other : Config) (witness : List StackAction)
    (result : protocolEquality left leftLimit right rightLimit work start other = .differ witness) :
    observe left leftLimit start witness ≠ observe right rightLimit other witness :=
  checkPairs_differ left leftLimit right rightLimit start other work [] _ witness
    (by
      intro entry member
      simp only [List.mem_singleton] at member
      subst member
      exact ⟨rfl, rfl⟩)
    result

/-- **Equal protocols run alike in the model**: on open endpoints, `runStack` admits the same
traces from two configurations the check found equal. -/
theorem equal_protocols_run_alike (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (work : Nat) (leftEndpoint rightEndpoint : Endpoint)
    (leftStack rightStack : List Nat) (leftOpen : leftEndpoint.closed = false)
    (rightOpen : rightEndpoint.closed = false)
    (result : protocolEquality left leftLimit right rightLimit work
      (leftEndpoint.state, leftStack) (rightEndpoint.state, rightStack) = .equal)
    (trace : List StackAction) :
    (runStack left ⟨leftEndpoint, leftStack, leftLimit⟩ trace).isSome =
      (runStack right ⟨rightEndpoint, rightStack, rightLimit⟩ trace).isSome := by
  have same := congrArg Option.isSome
    (protocolEquality_sound left leftLimit right rightLimit work _ _ result trace)
  rw [runStack_eq_runProto left leftLimit trace leftEndpoint leftStack leftOpen,
    runStack_eq_runProto right rightLimit trace rightEndpoint rightStack rightOpen]
  simpa [observe] using same

/-! ### Duality as equality with the dual table -/

/-- The dual guest action: send and receive swap, choosing and receiving a branch swap, and the
nested-call actions are their own duals. -/
def StackAction.dual : StackAction → StackAction
  | .send message => .recv message
  | .recv message => .send message
  | .choose branch => .receiveBranch branch
  | .receiveBranch branch => .choose branch
  | .enter => .enter
  | .skipNested => .skipNested
  | .finish => .finish

theorem StackAction.dual_dual (action : StackAction) : action.dual.dual = action := by
  cases action <;> rfl

/-- The dual table takes the dual action exactly where the table takes the action. -/
theorem protoStep_dual (definitions : List Node) (limit : Nat) (config : Config)
    (action : StackAction) :
    protoStep (definitions.map dual) limit config action.dual =
      protoStep definitions limit config action := by
  unfold protoStep
  rw [List.getElem?_map]
  cases definitions[config.1]? with
  | none => rfl
  | some node => cases node <;> cases action <;> rfl

theorem orderlyEnd_dual (definitions : List Node) :
    orderlyEnd (definitions.map dual) = orderlyEnd definitions := by
  funext config
  unfold orderlyEnd
  rw [List.getElem?_map]
  cases definitions[config.1]? with
  | none => rfl
  | some node => cases node <;> rfl

theorem observe_dual (definitions : List Node) (limit : Nat) :
    ∀ (config : Config) (trace : List StackAction),
      observe (definitions.map dual) limit config (trace.map StackAction.dual) =
        observe definitions limit config trace := by
  intro config trace
  unfold observe
  rw [orderlyEnd_dual]
  congr 1
  induction trace generalizing config with
  | nil => rfl
  | cons action rest ih =>
      simp only [List.map_cons, runProto, protoStep_dual]
      cases protoStep definitions limit config action with
      | none => rfl
      | some next => exact ih next

/-- **Protocol duality**: `start` in `left` is dual to `other` in `right` when it equals `other` in
the dual table. -/
def protocolDuality (left : List Node) (leftLimit : Nat) (right : List Node) (rightLimit : Nat)
    (work : Nat) (start other : Config) : Verdict :=
  protocolEquality left leftLimit (right.map dual) rightLimit work start other

/-- **A dual verdict means dual behaviour**: every trace of one side is admitted exactly when its
dual trace is admitted by the other, with the same orderly ends. -/
theorem protocolDuality_sound (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (work : Nat) (start other : Config)
    (result : protocolDuality left leftLimit right rightLimit work start other = .equal) :
    ∀ trace, observe left leftLimit start trace =
      observe right rightLimit other (trace.map StackAction.dual) := by
  intro trace
  have same := protocolEquality_sound left leftLimit (right.map dual) rightLimit work start other
    result trace
  have back := observe_dual right rightLimit other (trace.map StackAction.dual)
  rw [List.map_map, show StackAction.dual ∘ StackAction.dual = id from
    funext StackAction.dual_dual, List.map_id] at back
  exact same.trans back

/-! ### A sufficient work bound

`checkPairs` spends one unit per pending pair. A pair not yet assumed adds at most the left table's
`width` of successors (two without label sets) and is then assumed for good; an assumed pair is dropped.
So the pending count plus `width` times the pairs of a successor-closed set not yet assumed falls by at
least one per unit, and a bound at least that large
never runs out (`checkPairs_not_exhausted`). Every configuration reachable from an empty return stack
lies in an explicit finite set (`configs`), which gives a work bound computed from the two tables
alone (`workBound`). Under it the check decides equality and duality exactly. -/

/-- The widest choice a node declares: its label count at a label-set node, and two elsewhere. -/
def nodeWidth : Node → Nat
  | .select branches => branches.length
  | .branch branches => branches.length
  | _ => 2

/-- The widest choice a table declares, and at least two. -/
def width (definitions : List Node) : Nat :=
  definitions.foldr (fun node widest => max (nodeWidth node) widest) 2

theorem two_le_width (definitions : List Node) : 2 ≤ width definitions := by
  induction definitions with
  | nil => simp [width]
  | cons node rest ih =>
      simp only [width, List.foldr_cons] at ih ⊢
      exact Nat.le_trans ih (Nat.le_max_right _ _)

theorem nodeWidth_le_width (definitions : List Node) (node : Node) (present : node ∈ definitions) :
    nodeWidth node ≤ width definitions := by
  induction definitions with
  | nil => simp at present
  | cons head rest ih =>
      simp only [width, List.foldr_cons] at ih ⊢
      rcases List.mem_cons.mp present with same | later
      · subst same
        exact Nat.le_max_left _ _
      · exact Nat.le_trans (ih later) (Nat.le_max_right _ _)

/-- A configuration lists at most the table's width of enabled actions. -/
theorem enabled_length_le_width (definitions : List Node) (limit : Nat) (config : Config) :
    (enabled definitions limit config).length ≤ width definitions := by
  have two := two_le_width definitions
  unfold enabled
  cases found : definitions[config.1]? with
  | none => simp
  | some node =>
      have bounded := nodeWidth_le_width definitions node (List.mem_of_getElem? found)
      cases node with
      | select branches => simpa [nodeWidth] using bounded
      | branch branches => simpa [nodeWidth] using bounded
      | nested name parameter =>
          simp only
          split <;> simp <;> omega
      | _ =>
          simp only [List.length_singleton]
          omega

/-- A pair adds at most the left table's width of successor pairs. -/
theorem successorPairs_length_le_width (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (pair : Config × Config) (trace : List StackAction) :
    (successorPairs left leftLimit right rightLimit pair trace).length ≤ width left :=
  Nat.le_trans (List.length_filterMap_le _ _) (enabled_length_le_width left leftLimit pair.1)

/-- A set of configuration pairs is closed when both sides stepping on the same action from a member
reach a member. -/
def ClosedPairs (left : List Node) (leftLimit : Nat) (right : List Node) (rightLimit : Nat)
    (pairs : List (Config × Config)) : Prop :=
  ∀ pair ∈ pairs, ∀ action next other,
    protoStep left leftLimit pair.1 action = some next →
    protoStep right rightLimit pair.2 action = some other → (next, other) ∈ pairs

theorem successorPairs_closed (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (pairs : List (Config × Config))
    (closed : ClosedPairs left leftLimit right rightLimit pairs) (pair : Config × Config)
    (inPairs : pair ∈ pairs) (trace : List StackAction) :
    ∀ entry ∈ successorPairs left leftLimit right rightLimit pair trace, entry.1 ∈ pairs := by
  intro entry member
  unfold successorPairs at member
  obtain ⟨action, _, produced⟩ := List.mem_filterMap.mp member
  cases stepLeft : protoStep left leftLimit pair.1 action with
  | none => simp [stepLeft] at produced
  | some next =>
      cases stepRight : protoStep right rightLimit pair.2 action with
      | none => simp [stepLeft, stepRight] at produced
      | some other =>
          simp only [stepLeft, stepRight, Option.some.injEq] at produced
          subst produced
          exact closed pair inPairs action next other stepLeft stepRight

/-- The members of `pairs` not yet assumed, counted with multiplicity. -/
def unassumed (pairs assumed : List (Config × Config)) : Nat :=
  (pairs.filter fun pair => !decide (pair ∈ assumed)).length

/-- Assuming a member of `pairs` that was not yet assumed lowers the count by at least one. -/
theorem unassumed_assume (pairs assumed : List (Config × Config)) (pair : Config × Config)
    (inPairs : pair ∈ pairs) (fresh : pair ∉ assumed) :
    unassumed pairs (pair :: assumed) + 1 ≤ unassumed pairs assumed := by
  unfold unassumed
  have split : (pairs.filter fun candidate => !decide (candidate ∈ pair :: assumed)) =
      (pairs.filter fun candidate => !decide (candidate ∈ assumed)).filter
        fun candidate => !decide (candidate = pair) := by
    rw [List.filter_filter]
    apply List.filter_congr
    intro candidate _
    by_cases same : candidate = pair
    · subst same
      simp
    · by_cases old : candidate ∈ assumed <;> simp [same, old]
  rw [split]
  have smaller : ((pairs.filter fun candidate => !decide (candidate ∈ assumed)).filter
      fun candidate => !decide (candidate = pair)).length <
      (pairs.filter fun candidate => !decide (candidate ∈ assumed)).length := by
    rw [List.length_filter_lt_length_iff_exists]
    exact ⟨pair, List.mem_filter.mpr ⟨inPairs, by simpa using fresh⟩, by simp⟩
  omega

/-- **Enough work never runs out**: when every pending pair lies in a closed set and the work is at
least the pending count plus the left table's width times the set's pairs not yet assumed, the
exploration ends in `equal` or `differ`. -/
theorem checkPairs_not_exhausted (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) (pairs : List (Config × Config))
    (closed : ClosedPairs left leftLimit right rightLimit pairs) :
    ∀ (work : Nat) (assumed : List (Config × Config)) (pending : List Pending),
      (∀ entry ∈ pending, entry.1 ∈ pairs) →
      pending.length + width left * unassumed pairs assumed ≤ work →
      checkPairs left leftLimit right rightLimit work assumed pending ≠ .exhausted := by
  intro work
  induction work with
  | zero =>
      intro assumed pending _ enough
      cases pending with
      | nil => simp [checkPairs]
      | cons entry rest => simp at enough
  | succ work ih =>
      intro assumed pending inPairs enough
      cases pending with
      | nil => simp [checkPairs]
      | cons entry rest =>
          obtain ⟨pair, trace⟩ := entry
          have pairIn : pair ∈ pairs := inPairs _ List.mem_cons_self
          have restIn : ∀ entry ∈ rest, entry.1 ∈ pairs :=
            fun entry member => inPairs entry (List.mem_cons_of_mem _ member)
          simp only [List.length_cons] at enough
          simp only [checkPairs]
          split
          · exact ih assumed rest restIn (by omega)
          · rename_i fresh
            split
            · simp
            · split
              · simp
              · apply ih (pair :: assumed)
                · intro entry member
                  rcases List.mem_append.mp member with fromPair | fromRest
                  · exact successorPairs_closed left leftLimit right rightLimit pairs closed pair
                      pairIn trace entry fromPair
                  · exact restIn entry fromRest
                · have successors :=
                    successorPairs_length_le_width left leftLimit right rightLimit pair trace
                  have assumedOne := unassumed_assume pairs assumed pair pairIn fresh
                  have scaled : width left * unassumed pairs (pair :: assumed) + width left ≤
                      width left * unassumed pairs assumed := by
                    have grown := Nat.mul_le_mul_left (width left) assumedOne
                    rw [Nat.mul_add, Nat.mul_one] at grown
                    exact grown
                  simp only [List.length_append]
                  omega

/-- Every definition index a table names: continuations, nested names and saved returns. -/
def indices (definitions : List Node) : List Nat :=
  definitions.flatMap fun node =>
    match node with
    | .finished => []
    | .send _ continuation => [continuation]
    | .recv _ continuation => [continuation]
    | .choose _ continuation => [continuation]
    | .offer _ continuation => [continuation]
    | .nested name parameter => [name, parameter]
    | .select branches => branches.map Prod.snd
    | .branch branches => branches.map Prod.snd

/-- Every return continuation a nested call can save. -/
def parameters (definitions : List Node) : List Nat :=
  definitions.flatMap fun node =>
    match node with
    | .nested _ parameter => [parameter]
    | _ => []

/-- Every return stack of at most `bound` entries drawn from `alphabet`. -/
def stacksUpTo (alphabet : List Nat) : Nat → List (List Nat)
  | 0 => [[]]
  | bound + 1 => [] :: alphabet.flatMap fun entry => (stacksUpTo alphabet bound).map (entry :: ·)

theorem mem_stacksUpTo (alphabet : List Nat) :
    ∀ (bound : Nat) (stack : List Nat), stack ∈ stacksUpTo alphabet bound ↔
      stack.length ≤ bound ∧ ∀ entry ∈ stack, entry ∈ alphabet := by
  intro bound
  induction bound with
  | zero => intro stack; cases stack <;> simp [stacksUpTo]
  | succ bound ih =>
      intro stack
      cases stack with
      | nil => simp [stacksUpTo]
      | cons head tail =>
          simp only [stacksUpTo, List.mem_cons, reduceCtorEq, false_or, List.mem_flatMap,
            List.mem_map, List.cons.injEq, List.length_cons, forall_eq_or_imp]
          constructor
          · rintro ⟨entry, inAlphabet, rest, inStacks, same, rfl⟩
            subst same
            have := (ih rest).mp inStacks
            exact ⟨by omega, inAlphabet, this.2⟩
          · rintro ⟨short, inAlphabet, rest⟩
            exact ⟨head, inAlphabet, tail, (ih tail).mpr ⟨by omega, rest⟩, rfl, rfl⟩

/-- Every configuration reachable from `start` with an empty return stack lies in this list. -/
def configs (definitions : List Node) (limit start : Nat) : List Config :=
  (start :: indices definitions).flatMap fun state =>
    (stacksUpTo (parameters definitions) limit).map fun stack => (state, stack)

theorem mem_configs (definitions : List Node) (limit start : Nat) (config : Config) :
    config ∈ configs definitions limit start ↔
      config.1 ∈ start :: indices definitions ∧ config.2.length ≤ limit ∧
        ∀ entry ∈ config.2, entry ∈ parameters definitions := by
  obtain ⟨state, stack⟩ := config
  simp only [configs, List.mem_flatMap, List.mem_map, Prod.mk.injEq]
  constructor
  · rintro ⟨chosen, inStates, saved, inStacks, rfl, rfl⟩
    exact ⟨inStates, (mem_stacksUpTo _ limit saved).mp inStacks⟩
  · rintro ⟨inStates, bounded⟩
    exact ⟨state, inStates, stack, (mem_stacksUpTo _ limit stack).mpr bounded, rfl, rfl⟩

theorem mem_indices (definitions : List Node) (node : Node) (present : node ∈ definitions)
    (index : Nat) (named : index ∈ (match node with
      | .finished => []
      | .send _ continuation => [continuation]
      | .recv _ continuation => [continuation]
      | .choose _ continuation => [continuation]
      | .offer _ continuation => [continuation]
      | .nested name parameter => [name, parameter]
      | .select branches => branches.map Prod.snd
      | .branch branches => branches.map Prod.snd)) :
    index ∈ indices definitions :=
  List.mem_flatMap.mpr ⟨node, present, named⟩

theorem mem_parameters (definitions : List Node) (name parameter : Nat)
    (present : Node.nested name parameter ∈ definitions) : parameter ∈ parameters definitions :=
  List.mem_flatMap.mpr ⟨.nested name parameter, present, by simp⟩

theorem parameters_subset_indices (definitions : List Node) (index : Nat)
    (saved : index ∈ parameters definitions) : index ∈ indices definitions := by
  obtain ⟨node, present, named⟩ := List.mem_flatMap.mp saved
  cases node <;> simp at named
  subst named
  exact mem_indices definitions _ present _ (by simp)

/-- **The configuration list is closed under every step.** -/
theorem configs_closed (definitions : List Node) (limit start : Nat) (config : Config)
    (member : config ∈ configs definitions limit start) (action : StackAction) (next : Config)
    (step : protoStep definitions limit config action = some next) :
    next ∈ configs definitions limit start := by
  obtain ⟨state, stack⟩ := config
  rw [mem_configs] at member ⊢
  obtain ⟨_, bounded, savedOk⟩ := member
  unfold protoStep at step
  cases lookup : definitions[state]? with
  | none => simp [lookup] at step
  | some node =>
      have present : node ∈ definitions := List.mem_of_getElem? lookup
      simp only [lookup] at step
      cases node with
      | finished =>
          cases action <;> simp at step
          cases stack with
          | nil =>
              simp only [Option.some.injEq] at step
              subst step
              exact ⟨by assumption, bounded, savedOk⟩
          | cons top rest =>
              simp only [Option.some.injEq] at step
              subst step
              refine ⟨List.mem_cons_of_mem _ (parameters_subset_indices definitions top
                (savedOk top List.mem_cons_self)), by simp at bounded ⊢; omega,
                fun entry inRest => savedOk entry (List.mem_cons_of_mem _ inRest)⟩
      | send declared continuation =>
          cases action <;> simp at step
          obtain ⟨_, rfl⟩ := step
          exact ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _ (by simp)), bounded,
            savedOk⟩
      | recv declared continuation =>
          cases action <;> simp at step
          obtain ⟨_, rfl⟩ := step
          exact ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _ (by simp)), bounded,
            savedOk⟩
      | select branches =>
          cases action with
          | choose label =>
              cases found : branches.lookup label with
              | none => simp [found] at step
              | some continuation =>
                  simp only [found, Option.some.injEq] at step
                  subst step
                  exact ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _
                    (by simpa using lookup_mem branches label continuation found)), bounded, savedOk⟩
          | _ => simp at step
      | branch branches =>
          cases action with
          | receiveBranch label =>
              cases found : branches.lookup label with
              | none => simp [found] at step
              | some continuation =>
                  simp only [found, Option.some.injEq] at step
                  subst step
                  exact ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _
                    (by simpa using lookup_mem branches label continuation found)), bounded, savedOk⟩
          | _ => simp at step
      | choose declared continuation =>
          cases action <;> simp at step
          obtain ⟨_, rfl⟩ := step
          exact ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _ (by simp)), bounded,
            savedOk⟩
      | offer declared continuation =>
          cases action <;> simp at step
          obtain ⟨_, rfl⟩ := step
          exact ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _ (by simp)), bounded,
            savedOk⟩
      | nested name parameter =>
          cases action <;> simp at step
          · obtain ⟨room, rfl⟩ := step
            refine ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _ (by simp)),
              by simp; omega, ?_⟩
            intro entry member
            rcases List.mem_cons.mp member with same | old
            · subst same
              exact mem_parameters definitions name entry present
            · exact savedOk entry old
          · subst step
            exact ⟨List.mem_cons_of_mem _ (mem_indices definitions _ present _ (by simp)), bounded,
              savedOk⟩

/-- Every pair of reachable configurations of the two tables. -/
def pairsOf (left : List Node) (leftLimit start : Nat) (right : List Node)
    (rightLimit other : Nat) : List (Config × Config) :=
  (configs left leftLimit start).flatMap fun first =>
    (configs right rightLimit other).map fun second => (first, second)

theorem mem_pairsOf (left : List Node) (leftLimit start : Nat) (right : List Node)
    (rightLimit other : Nat) (pair : Config × Config) :
    pair ∈ pairsOf left leftLimit start right rightLimit other ↔
      pair.1 ∈ configs left leftLimit start ∧ pair.2 ∈ configs right rightLimit other := by
  obtain ⟨first, second⟩ := pair
  simp only [pairsOf, List.mem_flatMap, List.mem_map, Prod.mk.injEq]
  constructor
  · rintro ⟨chosen, inLeft, paired, inRight, rfl, rfl⟩
    exact ⟨inLeft, inRight⟩
  · rintro ⟨inLeft, inRight⟩
    exact ⟨first, inLeft, second, inRight, rfl, rfl⟩

theorem pairsOf_closed (left : List Node) (leftLimit start : Nat) (right : List Node)
    (rightLimit other : Nat) :
    ClosedPairs left leftLimit right rightLimit (pairsOf left leftLimit start right rightLimit other) := by
  intro pair member action next following stepLeft stepRight
  rw [mem_pairsOf] at member ⊢
  exact ⟨configs_closed left leftLimit start pair.1 member.1 action next stepLeft,
    configs_closed right rightLimit other pair.2 member.2 action following stepRight⟩

/-- **A work bound computed from the two tables alone**: one plus the left table's width times the
number of pairs of reachable configurations. The width is two for a table without label sets. -/
def workBound (left : List Node) (leftLimit start : Nat) (right : List Node)
    (rightLimit other : Nat) : Nat :=
  1 + width left * (pairsOf left leftLimit start right rightLimit other).length

/-- **The bound is sufficient**: with at least `workBound` units the check never refuses as
exhausted, for every pair of tables, limits and start states. -/
theorem protocolEquality_not_exhausted (left : List Node) (leftLimit start : Nat)
    (right : List Node) (rightLimit other work : Nat)
    (enough : workBound left leftLimit start right rightLimit other ≤ work) :
    protocolEquality left leftLimit right rightLimit work (start, []) (other, []) ≠ .exhausted := by
  apply checkPairs_not_exhausted left leftLimit right rightLimit
    (pairsOf left leftLimit start right rightLimit other)
    (pairsOf_closed left leftLimit start right rightLimit other)
  · intro entry member
    simp only [List.mem_singleton] at member
    subst member
    rw [mem_pairsOf, mem_configs, mem_configs]
    simp
  · have counted : unassumed (pairsOf left leftLimit start right rightLimit other) [] ≤
        (pairsOf left leftLimit start right rightLimit other).length :=
      List.length_filter_le _ _
    have scaled := Nat.mul_le_mul_left (width left) counted
    unfold workBound at enough
    simp only [List.length_singleton]
    omega

/-- **Protocol type equality is decided**: under `workBound` the check returns `equal` exactly when
the two protocol states admit the same traces with the same orderly ends. -/
theorem protocolEquality_decides (left : List Node) (leftLimit start : Nat) (right : List Node)
    (rightLimit other work : Nat)
    (enough : workBound left leftLimit start right rightLimit other ≤ work) :
    protocolEquality left leftLimit right rightLimit work (start, []) (other, []) = .equal ↔
      ∀ trace, observe left leftLimit (start, []) trace = observe right rightLimit (other, []) trace := by
  constructor
  · exact protocolEquality_sound left leftLimit right rightLimit work _ _
  · intro alike
    cases verdict : protocolEquality left leftLimit right rightLimit work (start, []) (other, []) with
    | equal => rfl
    | differ witness =>
        exact absurd (alike witness) (protocolEquality_witness left leftLimit right rightLimit work _
          _ witness verdict)
    | exhausted =>
        exact absurd verdict (protocolEquality_not_exhausted left leftLimit start right rightLimit
          other work enough)

/-- **Protocol duality is decided** under the bound of the dual table: the check returns `equal`
exactly when every trace of one side is admitted with the same orderly end as its dual trace on the
other. -/
theorem protocolDuality_decides (left : List Node) (leftLimit start : Nat) (right : List Node)
    (rightLimit other work : Nat)
    (enough : workBound left leftLimit start (right.map dual) rightLimit other ≤ work) :
    protocolDuality left leftLimit right rightLimit work (start, []) (other, []) = .equal ↔
      ∀ trace, observe left leftLimit (start, []) trace =
        observe right rightLimit (other, []) (trace.map StackAction.dual) := by
  unfold protocolDuality
  rw [protocolEquality_decides left leftLimit start (right.map dual) rightLimit other work enough]
  constructor
  · intro alike trace
    have back := observe_dual right rightLimit (other, []) (trace.map StackAction.dual)
    rw [List.map_map, show StackAction.dual ∘ StackAction.dual = id from
      funext StackAction.dual_dual, List.map_id] at back
    exact (alike trace).trans back
  · intro dualAlike trace
    have back := observe_dual right rightLimit (other, []) (trace.map StackAction.dual)
    rw [List.map_map, show StackAction.dual ∘ StackAction.dual = id from
      funext StackAction.dual_dual, List.map_id] at back
    rw [dualAlike trace, back]

/-! ### Fixtures and load-bearing controls -/

/-- `balancedProtocol` with every definition index renumbered: node `i` moves to `3 - i`. -/
def renumberedBalanced : List Node :=
  [.finished, .recv .i64 0, .nested 3 1, .send .i64 2]

/-- **Renumbering is equality**: the balanced table and its renumbered copy are equal types. -/
theorem renumbered_balanced_is_equal :
    protocolEquality balancedProtocol 2 renumberedBalanced 2 64 (0, []) (3, []) = .equal := by
  decide

/-- **The return-stack limit is part of the type**: with one fewer saved return the balanced
protocol is a different type, and the check names the trace that tells them apart. -/
theorem stack_limit_separates_types :
    protocolEquality balancedProtocol 2 balancedProtocol 1 64 (0, []) (0, []) =
      .differ [.send .i64, .enter, .send .i64, .enter] := by
  decide

/-- A one-node protocol that sends forever. -/
def sendForever : List Node := [.send .i64 0]

/-- The same protocol written with two nodes. -/
def sendForeverUnrolled : List Node := [.send .i64 1, .send .i64 0]

/-- A one-node protocol that receives forever, the dual of `sendForever`. -/
def receiveForever : List Node := [.recv .i64 0]

/-- **Cycles are identified up to unfolding**, and the work bound is load-bearing: two units leave
a pair pending and are refused as exhausted, three decide equality. -/
theorem unfolding_is_equality_within_the_bound :
    protocolEquality sendForever 0 sendForeverUnrolled 0 2 (0, []) (0, []) = .exhausted ∧
      protocolEquality sendForever 0 sendForeverUnrolled 0 3 (0, []) (0, []) = .equal := by
  decide

/-- **Order is part of the type**: sending then receiving differs from receiving then sending. -/
theorem order_separates_types :
    protocolEquality [.send .i64 1, .recv .i64 2, .finished] 0
        [.recv .i64 1, .send .i64 2, .finished] 0 8 (0, []) (0, []) = .differ [.send .i64] := by
  decide

/-- A client that sends a number and receives a flag. -/
def clientProtocol : List Node := [.send .i64 1, .recv .bool 2, .finished]

/-- The matching server. -/
def serverProtocol : List Node := [.recv .i64 1, .send .bool 2, .finished]

/-- **Client and server are dual**, and a server answering with the wrong type is not. -/
theorem client_and_server_are_dual :
    protocolDuality clientProtocol 0 serverProtocol 0 8 (0, []) (0, []) = .equal ∧
      protocolDuality clientProtocol 0 [.recv .i64 1, .send .i64 2, .finished] 0 8 (0, []) (0, []) =
        .differ [.send .i64, .recv .bool] := by
  decide

/-- **A duality check that exhausts its work is refused**, not accepted (the WIT-SESSION-01
scenario): two units leave the cycle pending, three decide the duality. -/
theorem exhausted_duality_is_refused :
    protocolDuality sendForeverUnrolled 0 receiveForever 0 2 (0, []) (0, []) = .exhausted ∧
      protocolDuality sendForeverUnrolled 0 receiveForever 0 3 (0, []) (0, []) = .equal := by
  decide

/-- A mutant reading of the verdict that treats exhaustion as acceptance. -/
def acceptingExhaustion : Verdict → Verdict
  | .exhausted => .equal
  | verdict => verdict

/-- **Treating exhaustion as acceptance accepts unequal types**: two sends and three sends differ,
which the check proves with three units of work (after two sends only one side ends orderly), yet
the mutant reading accepts them at two. -/
theorem accepting_exhaustion_accepts_unequal_types :
    acceptingExhaustion (protocolEquality [.send .i64 1, .send .i64 2, .finished] 0
        [.send .i64 1, .send .i64 2, .send .i64 3, .finished] 0 2 (0, []) (0, [])) = .equal ∧
      protocolEquality [.send .i64 1, .send .i64 2, .finished] 0
        [.send .i64 1, .send .i64 2, .send .i64 3, .finished] 0 3 (0, []) (0, []) =
        .differ [.send .i64, .send .i64] := by
  decide

/-- A mutant exploration without the memo of assumed pairs. -/
def checkPairsWithoutMemo (left : List Node) (leftLimit : Nat) (right : List Node)
    (rightLimit : Nat) : Nat → List Pending → Verdict
  | _, [] => .equal
  | 0, _ :: _ => .exhausted
  | work + 1, (pair, trace) :: rest =>
      if orderlyEnd left pair.1 ≠ orderlyEnd right pair.2 then .differ trace
      else
        match distinguish (enabled left leftLimit pair.1) (enabled right rightLimit pair.2) with
        | some action => .differ (trace ++ [action])
        | none =>
            checkPairsWithoutMemo left leftLimit right rightLimit work
              (successorPairs left leftLimit right rightLimit pair trace ++ rest)

/-- **The memo is load-bearing**: without it a cyclic protocol is never decided, so even sixty-four
units end in exhaustion where the real check decides equality with three. -/
theorem memo_is_load_bearing :
    checkPairsWithoutMemo sendForever 0 sendForeverUnrolled 0 64 [(((0, []), (0, [])), [])] =
        .exhausted ∧
      protocolEquality sendForever 0 sendForeverUnrolled 0 3 (0, []) (0, []) = .equal := by
  decide

set_option maxRecDepth 100000 in
/-- On the fixtures the bound is small, and the check under it decides: the cycle and its unrolling
are equal, client and server are dual, and the two stack limits of the balanced table differ. -/
theorem workBound_decides_the_fixtures :
    workBound sendForever 0 0 sendForeverUnrolled 0 0 = 13 ∧
      protocolEquality sendForever 0 sendForeverUnrolled 0 13 (0, []) (0, []) = .equal ∧
      workBound clientProtocol 0 0 (serverProtocol.map dual) 0 0 = 19 ∧
      protocolDuality clientProtocol 0 serverProtocol 0 19 (0, []) (0, []) = .equal ∧
      workBound balancedProtocol 2 0 balancedProtocol 1 0 = 301 ∧
      protocolEquality balancedProtocol 2 balancedProtocol 1 301 (0, []) (0, []) =
        .differ [.send .i64, .enter, .send .i64, .enter] := by
  decide


/-! ## Choice over a label set

A `select` node declares every branch of an internal choice at one state, and `branch` is its dual
external choice. Each label offers its own continuation, and the first pair naming a label is the one
taken. One state can therefore admit more than two actions, which is why the exploration's work bound
scales with the table's `width`. -/

/-- Internal choice between sending an `i64` (label `0`) and receiving a `bool` (label `1`), both
ending the session. -/
def labelSetProtocol : List Node :=
  [.select [(0, 1), (1, 2)], .send .i64 3, .recv .bool 3, .finished]

/-- The same choice with its labels declared in the other order. -/
def labelSetReordered : List Node :=
  [.select [(1, 2), (0, 1)], .send .i64 3, .recv .bool 3, .finished]

/-- A one-branch choice where the label set declares two. -/
def singleBranchProtocol : List Node := [.choose 0 1, .send .i64 2, .finished]

/-- Three labels at one state. -/
def threeLabels : List Node := [.select [(0, 1), (1, 1), (2, 1)], .finished]

/-- **A label-set node enables every declared label at one state.** -/
theorem label_set_enables_every_label :
    enabled labelSetProtocol 0 (0, []) = [.choose 0, .choose 1] := by
  decide

/-- **Each label reaches its own typed continuation**: label `0` sends an `i64`, label `1` receives a
`bool`, and both end orderly. -/
theorem label_set_takes_each_branch :
    runProto labelSetProtocol 0 (0, []) [.choose 0, .send .i64, .finish] = some (3, []) ∧
      runProto labelSetProtocol 0 (0, []) [.choose 1, .recv .bool, .finish] = some (3, []) := by
  decide

/-- **An undeclared label is refused**, and so is receiving a branch at an internal choice. -/
theorem undeclared_label_is_refused :
    protoStep labelSetProtocol 0 (0, []) (.choose 2) = none ∧
      protoStep labelSetProtocol 0 (0, []) (.receiveBranch 0) = none := by
  decide

/-- **The guest selects a label, and the peer receives it on the dual table**, reaching the same
continuation. -/
theorem selection_and_receipt_meet_at_the_continuation :
    step labelSetProtocol 1 ⟨5, false, 9, 0, 4, false⟩ (.choose 1) = some ⟨5, false, 9, 2, 4, false⟩ ∧
      step (labelSetProtocol.map dual) 1 ⟨5, true, 9, 0, 4, false⟩ (.receiveBranch 1) =
        some ⟨5, true, 9, 2, 4, false⟩ := by
  decide

/-- **Label order is not observable**: the reordered declaration is equal under the work bound. -/
theorem label_order_is_not_observable :
    protocolEquality labelSetProtocol 0 labelSetReordered 0
      (workBound labelSetProtocol 0 0 labelSetReordered 0 0) (0, []) (0, []) = .equal := by
  decide

/-- **A missing label is observable**: a one-branch choice differs from the label set, and the witness
is the label it lacks. -/
theorem a_missing_label_is_distinguished :
    protocolEquality labelSetProtocol 0 singleBranchProtocol 0
      (workBound labelSetProtocol 0 0 singleBranchProtocol 0 0) (0, []) (0, []) = .differ [.choose 1] := by
  decide

/-- **The dual of a label set is the external choice over the same labels**, with dual continuations,
and the duality check accepts the pair. -/
theorem label_set_duality :
    labelSetProtocol.map dual = [.branch [(0, 1), (1, 2)], .recv .i64 3, .send .bool 3, .finished] ∧
      protocolDuality labelSetProtocol 0 (labelSetProtocol.map dual) 0
        (workBound labelSetProtocol 0 0 ((labelSetProtocol.map dual).map dual) 0 0) (0, []) (0, []) =
        .equal := by
  decide

/-- **A wide choice needs its width**: three labels enable three actions at one state, more than the
two every other node admits, while tables without label sets keep width two. -/
theorem a_wide_choice_needs_its_width :
    (enabled threeLabels 0 (0, [])).length = 3 ∧ width threeLabels = 3 ∧
      width labelSetProtocol = 2 ∧ width balancedProtocol = 2 ∧
      protocolEquality threeLabels 0 threeLabels 0 (workBound threeLabels 0 0 threeLabels 0 0)
        (0, []) (0, []) = .equal := by
  decide

/-- **With a repeated label, the first declared pair is the one taken.** -/
theorem first_declared_label_wins :
    protoStep [.select [(0, 1), (0, 2)], .finished, .finished] 0 (0, []) (.choose 0) = some (1, []) := by
  decide

end NestedSession
