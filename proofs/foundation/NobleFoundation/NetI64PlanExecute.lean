import NobleFoundation.NetI64PlanLegal

/-!
# Exact physical effects of the checked executor microsteps (issue #110)

Each successful microstep of `PhysicalState.executeCheckedImage` is given an
exact closed-form result: result installation appends or rewrites one slot,
a reciprocal write rewrites one live peer port or retargets one output,
retirement clears one consumed slot, and publication marks two endpoints and
appends one canonical pair. The list folds over results and reciprocal writes
are then composed under the invariant that every installed result occupies a
reserved slot (the next fresh index or a consumed pair slot) and every later
write targets a captured peer port or output that no other write targets.
These are statements about the Lean model executor only.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The physical slot written for one installed result. -/
def PhysicalState.resultSlot (logical : Id) (symbol : NetSym)
    (generation : UInt64) (ports : List (Option (Nat × Nat))) : PhysicalSlot :=
  { agent := some (logical, symbol)
    generation
    queuedGeneration := none
    ports }

/-- One installed result in the physical post-state. -/
structure ResultView where
  slot : Nat
  id : Id
  symbol : NetSym
  generation : UInt64
  ports : List (Option (Nat × Nat))

/-- One captured auxiliary boundary of the consumed rule agent: the rule
port it leaves, its physical endpoint (`(0, peer, port)` or `(1, output, 0)`),
that endpoint's logical port or stable label, and the installed result port
now wired to it. -/
structure EdgeView where
  sourcePort : Nat
  target : Nat × Nat × Nat
  endpoint : Port
  result : Nat × Nat
  resultId : Id

/-- The canonical physical pair published for a result principal port wired
to a live peer's principal port. -/
def PhysicalState.canonicalPair (slot : Nat) (generation : UInt64)
    (peer : Nat) (peerGeneration : UInt64) : PhysicalPair :=
  if slot < peer then
    { first := slot, firstGeneration := generation,
      second := peer, secondGeneration := peerGeneration }
  else
    { first := peer, firstGeneration := peerGeneration,
      second := slot, secondGeneration := generation }

/-- The physical post-state of one executed local rewrite, observed slot by
slot: installed results, retired consumed slots, rewritten captured peer
ports, untouched slots, retargeted outputs, appended queue entries and the
extended free list. -/
structure PhysicalState.RewriteObservations (state next : PhysicalState)
    (ruleSlot valueSlot : Nat) (results : List ResultView) (edges : List EdgeView)
    (retired : List Nat) (fresh : Nat) (published : List PhysicalPair) : Prop where
  size : next.slots.size = state.slots.size + fresh
  resultAfter : ∀ result ∈ results, ∃ slot, next.slots[result.slot]? = some slot ∧
    slot.agent = some (result.id, result.symbol) ∧
    slot.generation = result.generation ∧ slot.ports = result.ports
  retiredAfter : ∀ index ∈ retired, ∃ old slot, state.slots[index]? = some old ∧
    next.slots[index]? = some slot ∧ slot.agent = none ∧
    slot.generation = old.generation + 1 ∧
    old.generation ≠ UInt64.ofNat (UInt64.size - 1)
  otherAfter : ∀ (index : Nat) (old : PhysicalSlot), state.slots[index]? = some old →
    index ≠ ruleSlot → index ≠ valueSlot → ∃ slot : PhysicalSlot,
      next.slots[index]? = some slot ∧ slot.agent = old.agent ∧
      slot.generation = old.generation ∧ slot.ports.length = old.ports.length ∧
      (∀ edge ∈ edges, ∀ port, edge.target = (0, index, port) →
        slot.ports[port]? = some (some edge.result)) ∧
      (∀ port, (∀ edge ∈ edges, edge.target ≠ (0, index, port)) →
        slot.ports[port]? = old.ports[port]?)
  untouchedAfter : ∀ index, index < state.slots.size →
    index ≠ ruleSlot → index ≠ valueSlot →
    (∀ edge ∈ edges, ∀ port, edge.target ≠ (0, index, port)) →
    (∀ pair ∈ published, pair.first ≠ index ∧ pair.second ≠ index) →
    next.slots[index]? = state.slots[index]?
  outputsAfter : next.outputs.length = state.outputs.length ∧
    ∀ (output : Nat) (old : PhysicalOutput), state.outputs[output]? = some old →
      ∃ out : PhysicalOutput,
      next.outputs[output]? = some out ∧ out.label = old.label ∧
      (∀ edge ∈ edges, edge.target = (1, output, 0) → out.target = edge.result) ∧
      ((∀ edge ∈ edges, edge.target ≠ (1, output, 0)) → out.target = old.target)
  workAfter : next.work = state.work ++ published
  freeAfter : next.freeSlots = retired ++ state.freeSlots
  queueAfter : next.queuedGenerationsBound = true

/-- A fresh result is appended exactly at the next physical index. -/
theorem PhysicalState.installResult_fresh_eq
    {state : PhysicalState} {plan : BytePlan} {logical : Id}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {donor : PhysicalSlot} {ports : List (Option (Nat × Nat))}
    (hdonor : state.slots[result.2.2.2.1]? = some donor)
    (hlive : donor.agent.isSome = true)
    (hports : result.2.2.2.2.mapM plan.resultTarget = some ports)
    (hfresh : result.1 = true)
    (hindex : result.2.1 = state.slots.size) :
    state.installResult plan logical result =
      some (state.appendSlot
        (PhysicalState.resultSlot logical result.2.2.1 0 ports)) := by
  cases ha : donor.agent with
  | none => simp [ha] at hlive
  | some owner =>
    simp [PhysicalState.installResult, hdonor, ha, hports, hfresh, hindex,
      PhysicalState.resultSlot]

/-- A reused result rewrites its live slot with the next generation. -/
theorem PhysicalState.installResult_reuse_eq
    {state : PhysicalState} {plan : BytePlan} {logical : Id}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {donor old : PhysicalSlot} {ports : List (Option (Nat × Nat))}
    (hdonor : state.slots[result.2.2.2.1]? = some donor)
    (hlive : donor.agent.isSome = true)
    (hports : result.2.2.2.2.mapM plan.resultTarget = some ports)
    (hreuse : result.1 = false)
    (hold : state.slots[result.2.1]? = some old)
    (holdLive : old.agent.isSome = true)
    (hroom : old.generation ≠ UInt64.ofNat (UInt64.size - 1)) :
    state.installResult plan logical result =
      some (state.writeSlot result.2.1
        (PhysicalState.resultSlot logical result.2.2.1 (old.generation + 1) ports)) := by
  cases ha : donor.agent with
  | none => simp [ha] at hlive
  | some owner =>
    cases hb : old.agent with
    | none => simp [hb] at holdLive
    | some live =>
      simp [PhysicalState.installResult, hdonor, ha, hports, hreuse, hold, hb,
        PhysicalState.resultSlot]
      simpa using hroom

/-- The reciprocal side of one captured live-peer write. -/
def PhysicalState.peerWrite (old : PhysicalSlot) (port : Nat)
    (target : Nat × Nat) : PhysicalSlot :=
  { old with
    ports := old.ports.set port (some target)
    queuedGeneration := if port == 0 then none else old.queuedGeneration }

/-- A peer write rewrites exactly its captured live peer slot. -/
theorem PhysicalState.writeExternal_peer_eq
    {state : PhysicalState} {plan : BytePlan}
    {write : Nat × Nat × Nat}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {peer port : Nat} {old : PhysicalSlot}
    (hresult : plan.results[write.1]? = some result)
    (hboundary : plan.boundaries[write.2.2]? = some boundary)
    (hport : result.2.2.2.2[write.2.1]? = some (some write.2.2))
    (hkind : boundary.2 = (0, peer, port))
    (hleft : peer ≠ plan.pair.1)
    (hright : peer ≠ plan.pair.2.2.1)
    (hpeer : state.slots[peer]? = some old)
    (hlive : old.agent.isSome = true)
    (htarget : old.ports[port]? = some (some boundary.1)) :
    state.writeExternal plan write =
      some (state.writeSlot peer
        (PhysicalState.peerWrite old port (result.2.1, write.2.1))) := by
  have hsome : old.agent ≠ none := by
    intro ha
    simp [ha] at hlive
  simp [PhysicalState.writeExternal, hresult, hboundary, hport,
    hkind, hleft, hright, hpeer, hsome, htarget, PhysicalState.peerWrite]

/-- An output write retargets exactly its captured output entry. -/
theorem PhysicalState.writeExternal_output_eq
    {state : PhysicalState} {plan : BytePlan}
    {write : Nat × Nat × Nat}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {output : Nat} {old : PhysicalOutput}
    (hresult : plan.results[write.1]? = some result)
    (hboundary : plan.boundaries[write.2.2]? = some boundary)
    (hport : result.2.2.2.2[write.2.1]? = some (some write.2.2))
    (hkind : boundary.2 = (1, output, 0))
    (houtput : state.outputs[output]? = some old)
    (htarget : old.target = boundary.1) :
    state.writeExternal plan write =
      state.retargetOutput output (result.2.1, write.2.1) := by
  simp [PhysicalState.writeExternal, hresult, hboundary, hport,
    hkind, houtput, htarget]

/-- The slot left by retiring one consumed live generation. -/
def PhysicalState.retiredSlot (old : PhysicalSlot) : PhysicalSlot :=
  { old with
    agent := none
    generation := old.generation + 1
    queuedGeneration := none }

/-- Retiring a live slot with headroom clears it and frees its index. -/
theorem PhysicalState.retire_eq
    {state : PhysicalState} {index : Nat} {old : PhysicalSlot}
    (hslot : state.slots[index]? = some old)
    (hlive : old.agent.isSome = true)
    (hroom : old.generation ≠ UInt64.ofNat (UInt64.size - 1)) :
    state.retire index false =
      some { state.writeSlot index (PhysicalState.retiredSlot old) with
        freeSlots := index :: state.freeSlots } := by
  cases ha : old.agent with
  | none => simp [ha] at hlive
  | some owner =>
    simp [PhysicalState.retire, hslot, ha, PhysicalState.retiredSlot,
      PhysicalState.writeSlot]
    simpa using hroom

/-- A reused consumed slot is not retired. -/
theorem PhysicalState.retire_reused (state : PhysicalState) (index : Nat) :
    state.retire index true = some state := by
  simp [PhysicalState.retire]

/-- Publication marks both current endpoints and appends the canonical pair. -/
theorem PhysicalState.publish_eq
    {state : PhysicalState} {pair : PhysicalPair}
    {first second : PhysicalSlot} {a b : Id}
    (horder : pair.first < pair.second)
    (hfirst : state.slots[pair.first]? = some first)
    (hsecond : state.slots[pair.second]? = some second)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hmarkFirst : first.queuedGeneration ≠ some pair.firstGeneration)
    (hmarkSecond : second.queuedGeneration ≠ some pair.secondGeneration)
    (hcapacity : state.work.length < state.slots.size) :
    state.publish pair =
      some (((state.writeSlot pair.first
        { first with queuedGeneration := some pair.firstGeneration }).writeSlot
        pair.second { second with queuedGeneration := some pair.secondGeneration }).appendPublications
          [pair]) := by
  simp [PhysicalState.publish, horder, hfirst, hsecond, hcurrent,
    hfp, hsp, hmarkFirst, hmarkSecond, hcapacity]

/-- Retargeting an existing output replaces exactly that table entry. -/
theorem PhysicalState.retargetOutput_eq
    {state : PhysicalState} {index : Nat} {old : PhysicalOutput}
    (target : Nat × Nat)
    (houtput : state.outputs[index]? = some old) :
    state.retargetOutput index target =
      some { state with outputs := state.outputs.set index { old with target } } := by
  have hlist (outputs : List PhysicalOutput) (index : Nat)
      (h : outputs[index]? = some old) :
      retargetOutputList index target outputs =
        some (outputs.set index { old with target }) := by
    induction outputs generalizing index with
    | nil => simp at h
    | cons output rest ih =>
      cases index with
      | zero =>
        simp at h
        subst h
        rfl
      | succ n =>
        have hrest : rest[n]? = some old := by simpa using h
        simp [retargetOutputList, ih n hrest]
  simp [PhysicalState.retargetOutput, hlist state.outputs index houtput]

/-- The physical endpoint a reciprocal write targets. -/
def BytePlan.writeTarget (plan : BytePlan) (write : Nat × Nat × Nat) :
    Option (Nat × Nat × Nat) :=
  (plan.boundaries[write.2.2]?).map Prod.snd

/-- The installed result port a reciprocal write stores at its target. -/
def BytePlan.writeSource (plan : BytePlan) (write : Nat × Nat × Nat) :
    Option (Nat × Nat) :=
  (plan.results[write.1]?).map fun result => (result.2.1, write.2.1)

/-- One reciprocal write is admissible in `state`: its result port names its
boundary, and that boundary's live peer port or output still points at the
captured consumed source. -/
def PhysicalState.WriteReady (state : PhysicalState) (plan : BytePlan)
    (write : Nat × Nat × Nat) : Prop :=
  ∃ result boundary, plan.results[write.1]? = some result ∧
    plan.boundaries[write.2.2]? = some boundary ∧
    result.2.2.2.2[write.2.1]? = some (some write.2.2) ∧
    ((∃ peer port old, boundary.2 = (0, peer, port) ∧
        peer ≠ plan.pair.1 ∧ peer ≠ plan.pair.2.2.1 ∧
        state.slots[peer]? = some old ∧ old.agent.isSome = true ∧
        old.ports[port]? = some (some boundary.1)) ∨
     (∃ output old, boundary.2 = (1, output, 0) ∧
        state.outputs[output]? = some old ∧ old.target = boundary.1))

/-- The exact effect of one admissible reciprocal write. -/
theorem PhysicalState.writeExternal_ready
    {state : PhysicalState} {plan : BytePlan} {write : Nat × Nat × Nat}
    (h : state.WriteReady plan write) :
    ∃ result boundary, plan.results[write.1]? = some result ∧
      plan.boundaries[write.2.2]? = some boundary ∧
      ((∃ peer port old, boundary.2 = (0, peer, port) ∧
          state.slots[peer]? = some old ∧
          state.writeExternal plan write =
            some (state.writeSlot peer
              (PhysicalState.peerWrite old port (result.2.1, write.2.1)))) ∨
       (∃ output old, boundary.2 = (1, output, 0) ∧
          state.outputs[output]? = some old ∧
          state.writeExternal plan write =
            some { state with outputs :=
              state.outputs.set output { old with target := (result.2.1, write.2.1) } })) := by
  obtain ⟨result, boundary, hresult, hboundary, hport, hkind⟩ := h
  refine ⟨result, boundary, hresult, hboundary, ?_⟩
  rcases hkind with ⟨peer, port, old, hk, hleft, hright, hpeer, hlive, htarget⟩ |
    ⟨output, old, hk, houtput, htarget⟩
  · exact .inl ⟨peer, port, old, hk, hpeer,
      PhysicalState.writeExternal_peer_eq hresult hboundary hport hk
        hleft hright hpeer hlive htarget⟩
  · refine .inr ⟨output, old, hk, houtput, ?_⟩
    rw [PhysicalState.writeExternal_output_eq hresult hboundary hport hk
      houtput htarget]
    exact PhysicalState.retargetOutput_eq _ houtput

/-- An admissible write remains admissible after another write with a
different target: peers keep their liveness and other ports, and other
outputs keep their targets. -/
theorem PhysicalState.writeReady_after
    {state next : PhysicalState} {plan : BytePlan}
    {first second : Nat × Nat × Nat}
    (hfirst : state.WriteReady plan first)
    (hstep : state.writeExternal plan first = some next)
    (hsecond : state.WriteReady plan second)
    (hdistinct : plan.writeTarget first ≠ plan.writeTarget second) :
    next.WriteReady plan second := by
  obtain ⟨result, boundary, hresult, hboundary, heffect⟩ :=
    PhysicalState.writeExternal_ready hfirst
  obtain ⟨result', boundary', hresult', hboundary', hport', hkind'⟩ := hsecond
  have htargets : boundary.2 ≠ boundary'.2 := by
    intro he
    apply hdistinct
    simp [BytePlan.writeTarget, hboundary, hboundary', he]
  refine ⟨result', boundary', hresult', hboundary', hport', ?_⟩
  rcases heffect with ⟨peer, port, old, hk, hpeer, heq⟩ |
    ⟨output, old, hk, houtput, heq⟩
  · rw [heq] at hstep
    cases hstep
    rcases hkind' with ⟨peer', port', old', hk', hleft', hright', hpeer', hlive', htarget'⟩ |
      ⟨output', old', hk', houtput', htarget'⟩
    · left
      by_cases hsame : peer = peer'
      · subst peer'
        have hold : old' = old := Option.some.inj (hpeer'.symm.trans hpeer)
        subst old'
        have hports : port ≠ port' := by
          intro hp
          subst hp
          exact htargets (hk.trans hk'.symm)
        refine ⟨peer, port', PhysicalState.peerWrite old port (result.2.1, first.2.1),
          hk', hleft', hright', PhysicalState.writeSlot_self hpeer, ?_, ?_⟩
        · simpa [PhysicalState.peerWrite] using hlive'
        · simpa [PhysicalState.peerWrite, List.getElem?_set_ne hports] using htarget'
      · exact ⟨peer', port', old', hk', hleft', hright',
          (PhysicalState.writeSlot_frame hsame).trans hpeer', hlive', htarget'⟩
    · exact .inr ⟨output', old', hk', by simpa [PhysicalState.writeSlot] using houtput',
        htarget'⟩
  · rw [heq] at hstep
    cases hstep
    rcases hkind' with ⟨peer', port', old', hk', hleft', hright', hpeer', hlive', htarget'⟩ |
      ⟨output', old', hk', houtput', htarget'⟩
    · exact .inl ⟨peer', port', old', hk', hleft', hright', hpeer', hlive', htarget'⟩
    · right
      have houtputs : output ≠ output' := by
        intro ho
        subst ho
        exact htargets (hk.trans hk'.symm)
      refine ⟨output', old', hk', ?_, htarget'⟩
      simpa [List.getElem?_set_ne houtputs] using houtput'

/-- Observations of the arena after a list of reciprocal writes: only the
targeted live-peer ports and output targets change, and every targeted
location holds the installed result port. -/
structure PhysicalState.WritesPost (state next : PhysicalState) (plan : BytePlan)
    (writes : List (Nat × Nat × Nat)) : Prop where
  size : next.slots.size = state.slots.size
  work : next.work = state.work
  free : next.freeSlots = state.freeSlots
  outputLength : next.outputs.length = state.outputs.length
  slotFrame : ∀ index, (∀ write ∈ writes, ∀ port,
      plan.writeTarget write ≠ some (0, index, port)) →
    next.slots[index]? = state.slots[index]?
  peerSlot : ∀ index old, state.slots[index]? = some old → ∃ slot,
    next.slots[index]? = some slot ∧ slot.agent = old.agent ∧
    slot.generation = old.generation ∧ slot.ports.length = old.ports.length ∧
    (slot.queuedGeneration = old.queuedGeneration ∨ slot.queuedGeneration = none) ∧
    ∀ port, (∀ write ∈ writes, plan.writeTarget write ≠ some (0, index, port)) →
      slot.ports[port]? = old.ports[port]?
  written : ∀ write ∈ writes, ∀ index port source,
    plan.writeTarget write = some (0, index, port) →
    plan.writeSource write = some source →
    ∃ slot, next.slots[index]? = some slot ∧
      slot.ports[port]? = some (some source) ∧
      (port = 0 → slot.queuedGeneration = none)
  outputFrame : ∀ output old, state.outputs[output]? = some old → ∃ out,
    next.outputs[output]? = some out ∧ out.label = old.label ∧
    ((∀ write ∈ writes, plan.writeTarget write ≠ some (1, output, 0)) → out = old)
  outputWritten : ∀ write ∈ writes, ∀ output source,
    plan.writeTarget write = some (1, output, 0) →
    plan.writeSource write = some source →
    ∃ out, next.outputs[output]? = some out ∧ out.target = source

/-- One admissible reciprocal write has exactly the single-write observations. -/
theorem PhysicalState.writeExternal_post
    {state next : PhysicalState} {plan : BytePlan} {write : Nat × Nat × Nat}
    (hready : state.WriteReady plan write)
    (hstep : state.writeExternal plan write = some next) :
    state.WritesPost next plan [write] := by
  obtain ⟨result, boundary, hresult, hboundary, heffect⟩ :=
    PhysicalState.writeExternal_ready hready
  have htarget : plan.writeTarget write = some boundary.2 := by
    simp [BytePlan.writeTarget, hboundary]
  have hsource : plan.writeSource write = some (result.2.1, write.2.1) := by
    simp [BytePlan.writeSource, hresult]
  rcases heffect with ⟨peer, port, old, hk, hpeer, heq⟩ |
    ⟨output, old, hk, houtput, heq⟩
  · rw [heq] at hstep
    cases hstep
    have hlen : port < old.ports.length := by
      rcases hready with ⟨result', boundary', _, hboundary', _, hkind'⟩
      rw [hboundary] at hboundary'
      cases hboundary'
      rcases hkind' with ⟨peer', port', old', hk', _, _, hpeer', _, htarget'⟩ |
        ⟨output', old', hk', _, _⟩
      · rw [hk] at hk'
        simp only [Prod.mk.injEq, true_and] at hk'
        obtain ⟨rfl, rfl⟩ := hk'
        rw [hpeer] at hpeer'
        cases hpeer'
        exact (getElem?_eq_some_iff.mp htarget').1
      · rw [hk] at hk'
        simp at hk'
    refine ⟨by simp [PhysicalState.writeSlot], rfl, rfl, rfl, ?_, ?_, ?_, ?_, ?_⟩
    · intro index hnot
      have hne : peer ≠ index := by
        intro he
        subst he
        exact hnot write (by simp) port (by rw [htarget, hk])
      exact PhysicalState.writeSlot_frame hne
    · intro index slot hslot
      by_cases he : peer = index
      · subst he
        have hsame : slot = old := Option.some.inj (hslot.symm.trans hpeer)
        subst slot
        refine ⟨PhysicalState.peerWrite old port (result.2.1, write.2.1),
          PhysicalState.writeSlot_self hpeer, rfl, rfl,
          by simp [PhysicalState.peerWrite], ?_, ?_⟩
        · by_cases hz : port = 0
          · right
            simp [PhysicalState.peerWrite, hz]
          · left
            simp [PhysicalState.peerWrite, hz]
        · intro other hnot
          have hne : port ≠ other := by
            intro hp
            subst hp
            exact hnot write (by simp) (by rw [htarget, hk])
          simp [PhysicalState.peerWrite, List.getElem?_set_ne hne]
      · exact ⟨slot, (PhysicalState.writeSlot_frame he).trans hslot,
          rfl, rfl, rfl, .inl rfl, fun _ _ => rfl⟩
    · intro w hw index other source hw' hs'
      simp at hw
      subst w
      rw [htarget, hk] at hw'
      simp only [Option.some.injEq, Prod.mk.injEq] at hw'
      obtain ⟨_, rfl, rfl⟩ := hw'
      rw [hsource] at hs'
      cases hs'
      refine ⟨PhysicalState.peerWrite old port (result.2.1, write.2.1),
        PhysicalState.writeSlot_self hpeer, ?_, ?_⟩
      · simp [PhysicalState.peerWrite, List.getElem?_set_self hlen]
      · intro hz
        simp [PhysicalState.peerWrite, hz]
    · intro index out hout
      exact ⟨out, by simpa [PhysicalState.writeSlot] using hout, rfl, fun _ => rfl⟩
    · intro w hw output source hw' _
      simp at hw
      subst w
      rw [htarget, hk] at hw'
      simp at hw'
  · rw [heq] at hstep
    cases hstep
    have hbound : output < state.outputs.length := (getElem?_eq_some_iff.mp houtput).1
    refine ⟨rfl, rfl, rfl, by simp, fun _ _ => rfl, ?_, ?_, ?_, ?_⟩
    · intro index slot hslot
      exact ⟨slot, hslot, rfl, rfl, rfl, .inl rfl, fun _ _ => rfl⟩
    · intro w hw index port source hw' _
      simp at hw
      subst w
      rw [htarget, hk] at hw'
      simp at hw'
    · intro index out hout
      by_cases he : output = index
      · subst he
        have hsame : out = old := Option.some.inj (hout.symm.trans houtput)
        subst out
        refine ⟨{ old with target := (result.2.1, write.2.1) },
          by simp [List.getElem?_set_self hbound], rfl, ?_⟩
        intro hnot
        exact absurd (by rw [htarget, hk]) (hnot write (by simp))
      · exact ⟨out, by simpa [List.getElem?_set_ne he] using hout, rfl, fun _ => rfl⟩
    · intro w hw index source hw' hs'
      simp at hw
      subst w
      rw [htarget, hk] at hw'
      simp only [Option.some.injEq, Prod.mk.injEq, true_and] at hw'
      obtain ⟨rfl, -⟩ := hw'
      rw [hsource] at hs'
      cases hs'
      exact ⟨{ old with target := (result.2.1, write.2.1) },
        by simp [List.getElem?_set_self hbound], rfl⟩

/-- Write observations compose when the head write's target is not written
again by the remaining writes. -/
theorem PhysicalState.writesPost_cons
    {state middle next : PhysicalState} {plan : BytePlan}
    {write : Nat × Nat × Nat} {rest : List (Nat × Nat × Nat)}
    (hhead : state.WritesPost middle plan [write])
    (htail : middle.WritesPost next plan rest)
    (hfresh : ∀ other ∈ rest, plan.writeTarget write ≠ plan.writeTarget other) :
    state.WritesPost next plan (write :: rest) := by
  refine ⟨htail.size.trans hhead.size, htail.work.trans hhead.work,
    htail.free.trans hhead.free, htail.outputLength.trans hhead.outputLength,
    ?_, ?_, ?_, ?_, ?_⟩
  · intro index hnot
    rw [htail.slotFrame index (fun w hw => hnot w (List.mem_cons_of_mem _ hw)),
      hhead.slotFrame index (fun w hw => hnot w (by simp at hw; simp [hw]))]
  · intro index old hold
    obtain ⟨slot₁, hslot₁, ha₁, hg₁, hl₁, hq₁, hp₁⟩ := hhead.peerSlot index old hold
    obtain ⟨slot₂, hslot₂, ha₂, hg₂, hl₂, hq₂, hp₂⟩ := htail.peerSlot index slot₁ hslot₁
    refine ⟨slot₂, hslot₂, ha₂.trans ha₁, hg₂.trans hg₁, hl₂.trans hl₁, ?_, ?_⟩
    · rcases hq₂ with hq₂ | hq₂
      · rcases hq₁ with hq₁ | hq₁
        · exact .inl (hq₂.trans hq₁)
        · exact .inr (hq₂.trans hq₁)
      · exact .inr hq₂
    · intro port hnot
      rw [hp₂ port (fun w hw => hnot w (List.mem_cons_of_mem _ hw)),
        hp₁ port (fun w hw => hnot w (by simp at hw; simp [hw]))]
  · intro w hw index port source htarget hsource
    rcases List.mem_cons.mp hw with rfl | hrest
    · obtain ⟨slot₁, hslot₁, hport₁, hmark₁⟩ :=
        hhead.written w (by simp) index port source htarget hsource
      obtain ⟨slot₂, hslot₂, _, _, _, hq₂, hp₂⟩ := htail.peerSlot index slot₁ hslot₁
      refine ⟨slot₂, hslot₂, ?_, ?_⟩
      · rw [hp₂ port (fun other ho => by
          rw [← htarget]
          exact (hfresh other ho).symm)]
        exact hport₁
      · intro hz
        rcases hq₂ with hq₂ | hq₂
        · exact hq₂.trans (hmark₁ hz)
        · exact hq₂
    · exact htail.written w hrest index port source htarget hsource
  · intro output old hold
    obtain ⟨out₁, hout₁, hl₁, hf₁⟩ := hhead.outputFrame output old hold
    obtain ⟨out₂, hout₂, hl₂, hf₂⟩ := htail.outputFrame output out₁ hout₁
    refine ⟨out₂, hout₂, hl₂.trans hl₁, fun hnot => ?_⟩
    rw [hf₂ (fun w hw => hnot w (List.mem_cons_of_mem _ hw)),
      hf₁ (fun w hw => hnot w (by simp at hw; simp [hw]))]
  · intro w hw output source htarget hsource
    rcases List.mem_cons.mp hw with rfl | hrest
    · obtain ⟨out₁, hout₁, htarget₁⟩ :=
        hhead.outputWritten w (by simp) output source htarget hsource
      obtain ⟨out₂, hout₂, _, hf₂⟩ := htail.outputFrame output out₁ hout₁
      refine ⟨out₂, hout₂, ?_⟩
      rw [hf₂ (fun other ho => by
        rw [← htarget]
        exact (hfresh other ho).symm)]
      exact htarget₁
    · exact htail.outputWritten w hrest output source htarget hsource

/-- **List-fold reciprocal writes.** Every write of a list whose targets are
pairwise distinct and admissible in the starting arena succeeds in order, and
the final arena has exactly the cumulative write observations. -/
theorem PhysicalState.writeExternals_success {plan : BytePlan} :
    ∀ (writes : List (Nat × Nat × Nat)) (state : PhysicalState),
      (∀ write ∈ writes, state.WriteReady plan write) →
      (writes.map plan.writeTarget).Nodup →
      ∃ next, state.writeExternals plan writes = some next ∧
        state.WritesPost next plan writes := by
  intro writes
  induction writes with
  | nil =>
    intro state _ _
    refine ⟨state, rfl, rfl, rfl, rfl, rfl, fun _ _ => rfl, ?_, ?_, ?_, ?_⟩
    · intro index old hold
      exact ⟨old, hold, rfl, rfl, rfl, .inl rfl, fun _ _ => rfl⟩
    · intro w hw
      simp at hw
    · intro output old hold
      exact ⟨old, hold, rfl, fun _ => rfl⟩
    · intro w hw
      simp at hw
  | cons write rest ih =>
    intro state hready hnodup
    have hhead := hready write (by simp)
    obtain ⟨result, boundary, _, _, heffect⟩ := PhysicalState.writeExternal_ready hhead
    have hexists : ∃ middle, state.writeExternal plan write = some middle := by
      rcases heffect with ⟨_, _, _, _, _, heq⟩ | ⟨_, _, _, _, heq⟩
      · exact ⟨_, heq⟩
      · exact ⟨_, heq⟩
    obtain ⟨middle, hstep⟩ := hexists
    rw [List.map_cons] at hnodup
    have hcons := List.nodup_cons.mp hnodup
    have hdistinct : ∀ other ∈ rest, plan.writeTarget write ≠ plan.writeTarget other := by
      intro other ho he
      exact hcons.1 (List.mem_map.mpr ⟨other, ho, he.symm⟩)
    have hrestReady : ∀ other ∈ rest, middle.WriteReady plan other := fun other ho =>
      PhysicalState.writeReady_after hhead hstep (hready other (List.mem_cons_of_mem _ ho))
        (hdistinct other ho)
    obtain ⟨next, hrest, hpost⟩ := ih middle hrestReady hcons.2
    refine ⟨next, ?_, PhysicalState.writesPost_cons
      (PhysicalState.writeExternal_post hhead hstep) hpost hdistinct⟩
    simp [PhysicalState.writeExternals, hstep, hrest]

/-- **List-fold fresh installation.** Results that each occupy the next
reserved fresh index, with a live consumed donor and resolvable ports, all
install in order; every old slot is untouched and each appended slot holds
exactly its result. -/
theorem PhysicalState.installResults_fresh {plan : BytePlan}
    {left right : Id} {rhs : XRhs NetSym} :
    ∀ (results : List (Bool × Nat × NetSym × Nat × List (Option Nat)))
      (index : Nat) (state : PhysicalState),
      (∀ j result, results[j]? = some result →
        result.1 = true ∧ result.2.1 = state.slots.size + j ∧
        result.2.2.2.1 < state.slots.size ∧
        (∃ donor, state.slots[result.2.2.2.1]? = some donor ∧
          donor.agent.isSome = true) ∧
        ∃ ruleSlot ports, rhs.slots[index + j]? = some ruleSlot ∧
          rhs.lab ruleSlot.1 ruleSlot.2 = some result.2.2.1 ∧
          result.2.2.2.2.mapM plan.resultTarget = some ports) →
      ∃ next, PhysicalState.installResults plan left right rhs index state results =
          some next ∧
        next.slots.size = state.slots.size + results.length ∧
        next.outputs = state.outputs ∧ next.work = state.work ∧
        next.freeSlots = state.freeSlots ∧
        (∀ i, i < state.slots.size → next.slots[i]? = state.slots[i]?) ∧
        ∀ j result ruleSlot ports, results[j]? = some result →
          rhs.slots[index + j]? = some ruleSlot →
          result.2.2.2.2.mapM plan.resultTarget = some ports →
          next.slots[state.slots.size + j]? = some (PhysicalState.resultSlot
            (child left right ruleSlot.1 ruleSlot.2) result.2.2.1 0 ports) := by
  intro results
  induction results with
  | nil =>
    intro index state _
    refine ⟨state, rfl, by simp, rfl, rfl, rfl, fun _ _ => rfl, ?_⟩
    intro j result _ _ hj
    simp at hj
  | cons result rest ih =>
    intro index state hready
    obtain ⟨hfresh, hindex, hdonorBound, ⟨donor, hdonor, hlive⟩,
      ruleSlot, ports, hrule, hlab, hports⟩ := hready 0 result (by simp)
    simp only [Nat.add_zero] at hindex hrule
    have hinstall := PhysicalState.installResult_fresh_eq
      (logical := child left right ruleSlot.1 ruleSlot.2)
      hdonor hlive hports hfresh hindex
    let middle := state.appendSlot
      (PhysicalState.resultSlot (child left right ruleSlot.1 ruleSlot.2)
        result.2.2.1 0 ports)
    have hsize : middle.slots.size = state.slots.size + 1 := by
      simp [middle, PhysicalState.appendSlot]
    have hrestReady : ∀ j other, rest[j]? = some other →
        other.1 = true ∧ other.2.1 = middle.slots.size + j ∧
        other.2.2.2.1 < middle.slots.size ∧
        (∃ donor, middle.slots[other.2.2.2.1]? = some donor ∧
          donor.agent.isSome = true) ∧
        ∃ ruleSlot ports, rhs.slots[index + 1 + j]? = some ruleSlot ∧
          rhs.lab ruleSlot.1 ruleSlot.2 = some other.2.2.1 ∧
          other.2.2.2.2.mapM plan.resultTarget = some ports := by
      intro j other hj
      obtain ⟨hf, hi, hb, ⟨d, hd, hl⟩, rs, ps, hr, hla, hp⟩ :=
        hready (j + 1) other (by simpa using hj)
      refine ⟨hf, by omega, by omega, ⟨d, ?_, hl⟩, rs, ps,
        by simpa [Nat.add_assoc, Nat.add_comm 1 j] using hr, hla, hp⟩
      simpa [middle] using (PhysicalState.appendSlot_frame hb).trans hd
    obtain ⟨next, hnext, hnsize, houts, hwork, hfree, hframe, hslots⟩ :=
      ih (index + 1) middle hrestReady
    refine ⟨next, ?_, ?_, houts, hwork, hfree, ?_, ?_⟩
    · simp [PhysicalState.installResults, hrule, hlab, hinstall, middle] at hnext ⊢
      exact hnext
    · simp [hnsize, hsize]
      omega
    · intro i hi
      rw [hframe i (by omega)]
      exact PhysicalState.appendSlot_frame hi
    · intro j other rs ps hj hrs hps
      cases j with
      | zero =>
        simp at hj
        subst other
        simp only [Nat.add_zero] at hrs
        rw [hrule] at hrs
        cases hrs
        rw [hports] at hps
        cases hps
        simp only [Nat.add_zero]
        rw [hframe state.slots.size (by omega)]
        simp [middle, PhysicalState.appendSlot]
      | succ j =>
        have := hslots j other rs ps (by simpa using hj)
          (by simpa [Nat.add_assoc, Nat.add_comm 1 j] using hrs) hps
        rw [hsize] at this
        simpa [Nat.add_assoc, Nat.add_comm 1 j] using this

/-- **List-fold reuse installation.** Results that each rewrite a distinct
live consumed donor slot with generation headroom all install in order;
other slots are untouched and each reused slot holds exactly its result at
the next generation. -/
theorem PhysicalState.installResults_reuse {plan : BytePlan}
    {left right : Id} {rhs : XRhs NetSym} :
    ∀ (results : List (Bool × Nat × NetSym × Nat × List (Option Nat)))
      (index : Nat) (state : PhysicalState),
      (results.map fun result => result.2.1).Nodup →
      (∀ j result, results[j]? = some result →
        result.1 = false ∧ result.2.1 = result.2.2.2.1 ∧
        (∃ old, state.slots[result.2.1]? = some old ∧
          old.agent.isSome = true ∧
          old.generation ≠ UInt64.ofNat (UInt64.size - 1)) ∧
        ∃ ruleSlot ports, rhs.slots[index + j]? = some ruleSlot ∧
          rhs.lab ruleSlot.1 ruleSlot.2 = some result.2.2.1 ∧
          result.2.2.2.2.mapM plan.resultTarget = some ports) →
      ∃ next, PhysicalState.installResults plan left right rhs index state results =
          some next ∧
        next.slots.size = state.slots.size ∧
        next.outputs = state.outputs ∧ next.work = state.work ∧
        next.freeSlots = state.freeSlots ∧
        (∀ i, (∀ result ∈ results, result.2.1 ≠ i) →
          next.slots[i]? = state.slots[i]?) ∧
        ∀ j result ruleSlot ports old, results[j]? = some result →
          rhs.slots[index + j]? = some ruleSlot →
          result.2.2.2.2.mapM plan.resultTarget = some ports →
          state.slots[result.2.1]? = some old →
          next.slots[result.2.1]? = some (PhysicalState.resultSlot
            (child left right ruleSlot.1 ruleSlot.2) result.2.2.1
            (old.generation + 1) ports) := by
  intro results
  induction results with
  | nil =>
    intro index state _ _
    refine ⟨state, rfl, rfl, rfl, rfl, rfl, fun _ _ => rfl, ?_⟩
    intro j result _ _ _ hj
    simp at hj
  | cons result rest ih =>
    intro index state hnodup hready
    obtain ⟨hreuse, hdonorSlot, ⟨old, hold, holdLive, hroom⟩,
      ruleSlot, ports, hrule, hlab, hports⟩ := hready 0 result (by simp)
    simp only [Nat.add_zero] at hrule
    have hdonor : state.slots[result.2.2.2.1]? = some old := by
      rw [← hdonorSlot]
      exact hold
    have hinstall := PhysicalState.installResult_reuse_eq
      (logical := child left right ruleSlot.1 ruleSlot.2)
      hdonor holdLive hports hreuse hold holdLive hroom
    let middle := state.writeSlot result.2.1
      (PhysicalState.resultSlot (child left right ruleSlot.1 ruleSlot.2)
        result.2.2.1 (old.generation + 1) ports)
    rw [List.map_cons] at hnodup
    have hcons := List.nodup_cons.mp hnodup
    have hdistinct : ∀ other ∈ rest, result.2.1 ≠ other.2.1 := by
      intro other ho he
      exact hcons.1 (List.mem_map.mpr ⟨other, ho, he.symm⟩)
    have hrestReady : ∀ j other, rest[j]? = some other →
        other.1 = false ∧ other.2.1 = other.2.2.2.1 ∧
        (∃ old, middle.slots[other.2.1]? = some old ∧
          old.agent.isSome = true ∧
          old.generation ≠ UInt64.ofNat (UInt64.size - 1)) ∧
        ∃ ruleSlot ports, rhs.slots[index + 1 + j]? = some ruleSlot ∧
          rhs.lab ruleSlot.1 ruleSlot.2 = some other.2.2.1 ∧
          other.2.2.2.2.mapM plan.resultTarget = some ports := by
      intro j other hj
      obtain ⟨hf, hd, ⟨o, ho, hl, hr⟩, rs, ps, hrs, hla, hp⟩ :=
        hready (j + 1) other (by simpa using hj)
      have hmem : other ∈ rest := List.mem_of_getElem? hj
      refine ⟨hf, hd, ⟨o, ?_, hl, hr⟩, rs, ps,
        by simpa [Nat.add_assoc, Nat.add_comm 1 j] using hrs, hla, hp⟩
      exact (PhysicalState.writeSlot_frame (hdistinct other hmem)).trans ho
    obtain ⟨next, hnext, hnsize, houts, hwork, hfree, hframe, hslots⟩ :=
      ih (index + 1) middle hcons.2 hrestReady
    refine ⟨next, ?_, ?_, houts, hwork, hfree, ?_, ?_⟩
    · simp [PhysicalState.installResults, hrule, hlab, hinstall, middle] at hnext ⊢
      exact hnext
    · simp [hnsize, middle, PhysicalState.writeSlot]
    · intro i hi
      rw [hframe i (fun other ho => hi other (List.mem_cons_of_mem _ ho))]
      exact PhysicalState.writeSlot_frame (hi result (by simp))
    · intro j other rs ps o hj hrs hps ho
      cases j with
      | zero =>
        simp at hj
        subst other
        simp only [Nat.add_zero] at hrs
        rw [hrule] at hrs
        cases hrs
        rw [hports] at hps
        cases hps
        rw [hold] at ho
        cases ho
        rw [hframe result.2.1 (fun other hmem => (hdistinct other hmem).symm)]
        exact PhysicalState.writeSlot_self hold
      | succ j =>
        have hmem : other ∈ rest := List.mem_of_getElem? (by simpa using hj)
        have hmid : middle.slots[other.2.1]? = some o :=
          (PhysicalState.writeSlot_frame (hdistinct other hmem)).trans ho
        exact hslots j other rs ps o (by simpa using hj)
          (by simpa [Nat.add_assoc, Nat.add_comm 1 j] using hrs) hps hmid

/-- One canonical pair is publishable in `state`: ordered, current,
reciprocally principal, not already marked at its current generations. -/
def PhysicalState.PublishReady (state : PhysicalState) (pair : PhysicalPair) : Prop :=
  pair.first < pair.second ∧
  ∃ first second owners, state.slots[pair.first]? = some first ∧
    state.slots[pair.second]? = some second ∧
    state.logicalPair pair = some owners ∧
    first.ports[0]? = some (some (pair.second, 0)) ∧
    second.ports[0]? = some (some (pair.first, 0)) ∧
    first.queuedGeneration ≠ some pair.firstGeneration ∧
    second.queuedGeneration ≠ some pair.secondGeneration

/-- **List-fold publication.** Pairs with pairwise disjoint endpoints, each
publishable in the starting arena, all publish when the physical queue has
room for every one of them; only queue marks and the queue change. -/
theorem PhysicalState.publishAll_success :
    ∀ (pairs : List PhysicalPair) (state : PhysicalState),
      (∀ pair ∈ pairs, state.PublishReady pair) →
      (pairs.flatMap fun pair => [pair.first, pair.second]).Nodup →
      state.work.length + pairs.length ≤ state.slots.size →
      ∃ next, state.publishAll pairs = some next ∧
        next.work = state.work ++ pairs ∧ next.outputs = state.outputs ∧
        next.freeSlots = state.freeSlots ∧ next.slots.size = state.slots.size ∧
        (∀ (index : Nat) (old : PhysicalSlot), state.slots[index]? = some old →
          ∃ slot : PhysicalSlot,
          next.slots[index]? = some slot ∧ slot.agent = old.agent ∧
          slot.generation = old.generation ∧ slot.ports = old.ports) ∧
        ∀ index : Nat, (∀ pair ∈ pairs, pair.first ≠ index ∧ pair.second ≠ index) →
          next.slots[index]? = state.slots[index]? := by
  intro pairs
  induction pairs with
  | nil =>
    intro state _ _ _
    refine ⟨state, rfl, by simp, rfl, rfl, rfl, ?_, fun _ _ => rfl⟩
    intro index old hold
    exact ⟨old, hold, rfl, rfl, rfl⟩
  | cons pair rest ih =>
    intro state hready hnodup hcapacity
    obtain ⟨horder, first, second, ⟨ownerA, ownerB⟩, hfirst, hsecond, hcurrent, hfp, hsp,
      hmarkFirst, hmarkSecond⟩ := hready pair (by simp)
    have hcap : state.work.length < state.slots.size := by
      simp at hcapacity
      omega
    have hpublish := PhysicalState.publish_eq horder hfirst hsecond hcurrent
      hfp hsp hmarkFirst hmarkSecond hcap
    let markedFirst := state.writeSlot pair.first
      { first with queuedGeneration := some pair.firstGeneration }
    let marked := markedFirst.writeSlot pair.second
      { second with queuedGeneration := some pair.secondGeneration }
    let middle := marked.appendPublications [pair]
    rw [List.flatMap_cons] at hnodup
    have hsplit := List.nodup_append.mp hnodup
    have hdisjoint : ∀ other ∈ rest, other.first ≠ pair.first ∧ other.first ≠ pair.second ∧
        other.second ≠ pair.first ∧ other.second ≠ pair.second := by
      intro other ho
      have hfirstMem : other.first ∈ rest.flatMap fun pair => [pair.first, pair.second] :=
        List.mem_flatMap.mpr ⟨other, ho, by simp⟩
      have hsecondMem : other.second ∈ rest.flatMap fun pair => [pair.first, pair.second] :=
        List.mem_flatMap.mpr ⟨other, ho, by simp⟩
      exact ⟨fun he => hsplit.2.2 pair.first (by simp) other.first hfirstMem he.symm,
        fun he => hsplit.2.2 pair.second (by simp) other.first hfirstMem he.symm,
        fun he => hsplit.2.2 pair.first (by simp) other.second hsecondMem he.symm,
        fun he => hsplit.2.2 pair.second (by simp) other.second hsecondMem he.symm⟩
    have hframe (index : Nat) (h1 : index ≠ pair.first) (h2 : index ≠ pair.second) :
        middle.slots[index]? = state.slots[index]? := by
      change (markedFirst.writeSlot pair.second _).slots[index]? = _
      rw [PhysicalState.writeSlot_frame (Ne.symm h2), PhysicalState.writeSlot_frame (Ne.symm h1)]
    have hrestReady : ∀ other ∈ rest, middle.PublishReady other := by
      intro other ho
      obtain ⟨o1, o2, o3, o4⟩ := hdisjoint other ho
      obtain ⟨horder', first', second', owners', hfirst', hsecond', hcurrent', hfp', hsp',
        hm1, hm2⟩ := hready other (List.mem_cons_of_mem _ ho)
      refine ⟨horder', first', second', owners', (hframe _ o1 o2).trans hfirst',
        (hframe _ o3 o4).trans hsecond', ?_, hfp', hsp', hm1, hm2⟩
      have hsame : middle.logicalPair other = state.logicalPair other := by
        change ((markedFirst.writeSlot pair.second _).appendPublications [pair]).logicalPair other = _
        simp only [PhysicalState.logicalPair, PhysicalState.appendPublications]
        rw [show (markedFirst.writeSlot pair.second
            { second with queuedGeneration := some pair.secondGeneration }).slots[other.first]? =
            state.slots[other.first]? by
          rw [PhysicalState.writeSlot_frame (Ne.symm o2), PhysicalState.writeSlot_frame (Ne.symm o1)],
          show (markedFirst.writeSlot pair.second
            { second with queuedGeneration := some pair.secondGeneration }).slots[other.second]? =
            state.slots[other.second]? by
          rw [PhysicalState.writeSlot_frame (Ne.symm o4), PhysicalState.writeSlot_frame (Ne.symm o3)]]
      rw [hsame]
      exact hcurrent'
    have hrestNodup : (rest.flatMap fun pair => [pair.first, pair.second]).Nodup :=
      hsplit.2.1
    have hmiddleSize : middle.slots.size = state.slots.size := by
      simp [middle, marked, markedFirst, PhysicalState.appendPublications, PhysicalState.writeSlot]
    have hmiddleWork : middle.work = state.work ++ [pair] := by
      simp [middle, marked, markedFirst, PhysicalState.appendPublications, PhysicalState.writeSlot]
    have hrestCapacity : middle.work.length + rest.length ≤ middle.slots.size := by
      rw [hmiddleSize, hmiddleWork]
      simp at hcapacity ⊢
      omega
    obtain ⟨next, hnext, hwork, houts, hfree, hsize, hkeep, hframeRest⟩ :=
      ih middle hrestReady hrestNodup hrestCapacity
    refine ⟨next, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
    · simp [PhysicalState.publishAll, hpublish]
      exact hnext
    · rw [hwork, hmiddleWork]
      simp
    · rw [houts]
      simp [middle, marked, markedFirst, PhysicalState.appendPublications, PhysicalState.writeSlot]
    · rw [hfree]
      simp [middle, marked, markedFirst, PhysicalState.appendPublications, PhysicalState.writeSlot]
    · rw [hsize, hmiddleSize]
    · intro index old hold
      have hmid : ∃ slot : PhysicalSlot, middle.slots[index]? = some slot ∧
          slot.agent = old.agent ∧
          slot.generation = old.generation ∧ slot.ports = old.ports := by
        by_cases h2 : index = pair.second
        · subst h2
          have hs : second = old := Option.some.inj (hsecond.symm.trans hold)
          subst hs
          refine ⟨{ second with queuedGeneration := some pair.secondGeneration }, ?_, rfl, rfl, rfl⟩
          change (markedFirst.writeSlot pair.second _).slots[pair.second]? = _
          exact PhysicalState.writeSlot_self
            ((PhysicalState.writeSlot_frame (Nat.ne_of_lt horder)).trans hsecond)
        · by_cases h1 : index = pair.first
          · subst h1
            have hs : first = old := Option.some.inj (hfirst.symm.trans hold)
            subst hs
            refine ⟨{ first with queuedGeneration := some pair.firstGeneration }, ?_, rfl, rfl, rfl⟩
            change (markedFirst.writeSlot pair.second _).slots[pair.first]? = _
            rw [PhysicalState.writeSlot_frame (Nat.ne_of_gt horder)]
            exact PhysicalState.writeSlot_self hfirst
          · exact ⟨old, (hframe index h1 h2).trans hold, rfl, rfl, rfl⟩
      obtain ⟨slot, hslot, ha, hg, hp⟩ := hmid
      obtain ⟨slot', hslot', ha', hg', hp'⟩ := hkeep index slot hslot
      exact ⟨slot', hslot', ha'.trans ha, hg'.trans hg, hp'.trans hp⟩
    · intro index hnot
      obtain ⟨h1, h2⟩ := hnot pair (by simp)
      rw [hframeRest index (fun other ho => hnot other (List.mem_cons_of_mem _ ho))]
      exact hframe index (Ne.symm h1) (Ne.symm h2)

/-- The arena after result installation, described by result views: every
view's slot holds exactly its installed result, every other old slot is
untouched, and only the arena size changes. -/
structure PhysicalState.InstallPost (state installed : PhysicalState)
    (results : List ResultView) (fresh : Nat) : Prop where
  size : installed.slots.size = state.slots.size + fresh
  outputs : installed.outputs = state.outputs
  work : installed.work = state.work
  free : installed.freeSlots = state.freeSlots
  frame : ∀ index, index < state.slots.size →
    (∀ result ∈ results, result.slot ≠ index) →
    installed.slots[index]? = state.slots[index]?
  result : ∀ result ∈ results, installed.slots[result.slot]? =
    some (PhysicalState.resultSlot result.id result.symbol result.generation result.ports)

end NobleFoundation.NetI64Plans
