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

end NestedSession
