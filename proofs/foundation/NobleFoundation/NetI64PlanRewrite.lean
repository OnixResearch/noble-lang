import NobleFoundation.NetI64PlanProjection
import NobleFoundation.NetI64PlanFired
import NobleFoundation.NetI64PlanExecute

/-!
# Generic physical local rewrite and its semantic fields (issue #110)

`PhysicalState.LocalRewrite` records, for one selected consumed pair, exactly
how an executed physical image changed the arena: which slots now hold the
installed results (fresh or reused), which consumed slots were retired, which
captured live-peer ports and outputs now point at installed result ports, and
which canonical pairs were appended to the queue. It also records the
prestate facts of a legal matched arena and the independently compiled
abstract plan's children, wire writes and captured wire ends. From this
bundle alone the theorems below derive the post-state semantic-checker fields:
agent and wire map agreement with `Plan.commit`, output targets, unique live
identities, the free-slot inventory, canonical queue order, the directed-pair
queue permutation, reciprocal physical ports and owned outputs. The bundle is
rule-independent; per-rule theorems construct it from actual emitted images.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The abstract forward wire write of one boundary edge. -/
def EdgeView.forward (edge : EdgeView) : Port × Port :=
  ((edge.resultId, edge.result.2), edge.endpoint)

/-- The abstract reciprocal wire write of one boundary edge. -/
def EdgeView.backward (edge : EdgeView) : Port × Port :=
  (edge.endpoint, (edge.resultId, edge.result.2))

/-- A complete description of one executed local rewrite of a legal arena:
the physical post-state observations, the prestate capture facts, and the
independently compiled abstract plan fields. -/
structure PhysicalState.LocalRewrite (state next : PhysicalState)
    (selected : Plan)
    (ruleSlot valueSlot : Nat) (ruleId valueId : Id)
    (ruleSymbol valueSymbol : NetSym)
    (results : List ResultView) (edges : List EdgeView)
    (retired : List Nat) (fresh : Nat) (published : List PhysicalPair) : Prop where
  -- prestate
  legal : state.Legal
  ruleOld : ∃ old, state.slots[ruleSlot]? = some old ∧
    old.agent = some (ruleId, ruleSymbol) ∧
    old.ports[0]? = some (some (valueSlot, 0))
  valueOld : ∃ old, state.slots[valueSlot]? = some old ∧
    old.agent = some (valueId, valueSymbol) ∧
    old.ports[0]? = some (some (ruleSlot, 0))
  valueArity : NetI64.arity valueSymbol = 0
  distinctSlots : ruleSlot ≠ valueSlot
  outputsInArity : ∀ output ∈ state.outputs, ∃ slot id symbol,
    state.slots[output.target.1]? = some slot ∧ slot.agent = some (id, symbol) ∧
      output.target.2 ≤ NetI64.arity symbol
  -- captured boundary edges
  edgePorts : edges.map EdgeView.sourcePort =
    (List.range (NetI64.arity ruleSymbol)).map (· + 1)
  edgeTargets : (edges.map EdgeView.target).Nodup
  edgeCapture : ∀ edge ∈ edges,
    (∃ peer port peerSlot peerId peerSymbol,
      edge.target = (0, peer, port) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
      state.slots[peer]? = some peerSlot ∧ peerSlot.agent = some (peerId, peerSymbol) ∧
      peerSlot.ports[port]? = some (some (ruleSlot, edge.sourcePort)) ∧
      edge.endpoint = (peerId, port)) ∨
    (∃ output out, edge.target = (1, output, 0) ∧
      state.outputs[output]? = some out ∧
      out.target = (ruleSlot, edge.sourcePort) ∧
      edge.endpoint = out.label)
  -- installed results
  resultSlots : (results.map ResultView.slot).Nodup
  resultIds : (results.map ResultView.id).Nodup
  resultFresh : ∀ result ∈ results, result.id ∉ state.liveIds ∧
    ∀ output ∈ state.outputs, output.label.1 ≠ result.id
  resultPlace : ∀ result ∈ results,
    (result.slot = ruleSlot ∨ result.slot = valueSlot) ∨
      (state.slots.size ≤ result.slot ∧ result.slot < state.slots.size + fresh)
  resultGeneration : ∀ result ∈ results, ∀ old,
    state.slots[result.slot]? = some old →
      old.generation ≠ UInt64.ofNat (UInt64.size - 1) ∧
        result.generation = old.generation + 1
  freshFilled : ∀ index, state.slots.size ≤ index →
    index < state.slots.size + fresh → ∃ result ∈ results, result.slot = index
  resultArity : ∀ result ∈ results,
    NetI64.arity result.symbol + 1 ≤ result.ports.length
  resultPorts : ∀ result ∈ results, ∀ port target, result.ports[port]? = some target →
    (∃ edge ∈ edges, edge.result = (result.slot, port) ∧
      edge.resultId = result.id ∧
      port ≤ NetI64.arity result.symbol ∧
      ((∃ peer peerPort, edge.target = (0, peer, peerPort) ∧ target = some (peer, peerPort)) ∨
       (∃ output, edge.target = (1, output, 0) ∧ target = none))) ∨
    ((∀ edge ∈ edges, edge.result ≠ (result.slot, port)) ∧
      NetI64.arity result.symbol < port ∧ target = none)
  edgeResult : ∀ edge ∈ edges, ∃ result ∈ results,
    result.slot = edge.result.1 ∧ edge.resultId = result.id ∧
      edge.result.2 < result.ports.length
  edgeResults : (edges.map EdgeView.result).Nodup
  -- retired consumed slots
  retiredExact : ∀ index, index ∈ retired ↔
    ((index = ruleSlot ∨ index = valueSlot) ∧ ∀ result ∈ results, result.slot ≠ index)
  retiredNodup : retired.Nodup
  -- physical post-state observations
  observed : state.RewriteObservations next ruleSlot valueSlot results edges
    retired fresh published
  -- publications
  publishedExact : ∀ pair, pair ∈ published ↔
    ∃ result ∈ results, ∃ edge ∈ edges, ∃ peer peerSlot,
      edge.result = (result.slot, 0) ∧ edge.target = (0, peer, 0) ∧
      state.slots[peer]? = some peerSlot ∧
      pair = PhysicalState.canonicalPair result.slot result.generation
        peer peerSlot.generation
  publishedNodup : published.Nodup
  -- independently compiled abstract plan
  selectedPair : (selected.redex.1 = ruleId ∧ selected.redex.2.1 = valueId) ∨
    (selected.redex.1 = valueId ∧ selected.redex.2.1 = ruleId)
  selectedAgents : selected.initializedAgents =
    results.map fun result => (result.id, result.symbol)
  selectedWrites : selected.wireWrites =
    edges.map EdgeView.forward ++ edges.map EdgeView.backward
  selectedErased : ∀ port,
    (port ∈ selected.capturedBoundary.map Prod.fst ∨
      port ∈ selected.capturedBoundary.map Prod.snd) ↔
    (port = (ruleId, 0) ∨ port = (valueId, 0) ∨
      ∃ edge ∈ edges, port = (ruleId, edge.sourcePort) ∨ port = edge.endpoint)
  selectedActive : selected.newlyActive = principalPairs selected.wireWrites

end NobleFoundation.NetI64Plans
