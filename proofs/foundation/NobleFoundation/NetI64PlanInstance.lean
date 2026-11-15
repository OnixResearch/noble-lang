import NobleFoundation.NetI64PlanAdmit
import NobleFoundation.NetI64PlanShape
import NobleFoundation.NetI64PlanCompose

/-!
# Per-rule local rewrites of actually emitted images (issue #110)

For each selected rule shape this module builds the `PhysicalState.LocalRewrite`
bundle of an actually lowered image from the prestate alone: structural
legality, the current selected pair, a matching projection, the per-port
capture facts, and the absence of the alias and queue-capacity exclusions.
Execution is derived from the generic phase composition, never assumed.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The physical reference a captured boundary contributes to a result port. -/
def PhysicalState.boundaryReference
    (boundary : (Nat × Nat) × (Nat × Nat × Nat)) : Option (Nat × Nat) :=
  match boundary.2 with
  | (0, peer, port) => some (peer, port)
  | _ => none

/-- A result port naming a captured peer or output resolves to that
boundary's physical reference. -/
theorem BytePlan.resultTarget_boundary {plan : BytePlan} {index : Nat}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    (hboundary : plan.boundaries[index]? = some boundary)
    (hkind : (∃ peer port, boundary.2 = (0, peer, port)) ∨
      (∃ output, boundary.2 = (1, output, 0))) :
    plan.resultTarget (some index) =
      some (PhysicalState.boundaryReference boundary) := by
  rcases hkind with ⟨peer, port, hk⟩ | ⟨output, hk⟩
  · simp [BytePlan.resultTarget, hboundary, hk, PhysicalState.boundaryReference]
  · simp [BytePlan.resultTarget, hboundary, hk, PhysicalState.boundaryReference]

/-- A result whose principal port reads an output or a nonprincipal peer
port publishes no pair. -/
theorem PhysicalState.publicationFor_quiet
    {state : PhysicalState} {pair : PhysicalPair}
    {boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {index : Nat} {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    (hport : result.2.2.2.2[0]? = some (some index))
    (hboundary : boundaries[index]? = some boundary)
    (hquiet : boundary.2.1 ≠ 0 ∨ boundary.2.2.2 ≠ 0) :
    state.publicationFor pair boundaries result = some [] := by
  obtain ⟨source, kind, peer, peerPort⟩ := boundary
  simp only at hquiet
  simp [PhysicalState.publicationFor, hport, hboundary]
  intro hk hp
  rcases hquiet with h | h <;> contradiction

/-- A fresh result whose principal port reads a live peer's principal port
publishes exactly the canonical tuple at generation zero. -/
theorem PhysicalState.publicationFor_fresh_active
    {state : PhysicalState} {pair : PhysicalPair}
    {boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {index peer : Nat} {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {other : PhysicalSlot}
    (hport : result.2.2.2.2[0]? = some (some index))
    (hboundary : boundaries[index]? = some boundary)
    (hkind : boundary.2 = (0, peer, 0))
    (hfirst : peer ≠ pair.first) (hsecond : peer ≠ pair.second)
    (hself : peer ≠ result.2.1)
    (hpeer : state.slots[peer]? = some other) (hlive : other.agent.isSome = true)
    (hfresh : result.1 = true) :
    state.publicationFor pair boundaries result = some
      (if result.2.1 < peer then [(result.2.1, 0, peer, other.generation.toNat)]
        else [(peer, other.generation.toNat, result.2.1, 0)]) := by
  obtain ⟨source, kindPeer⟩ := boundary
  simp only at hkind
  subst hkind
  have hsome : other.agent ≠ none := by
    intro ha
    simp [ha] at hlive
  simp [PhysicalState.publicationFor, hport, hboundary, hfirst, hsecond, hself,
    hpeer, hsome, hfresh]
  split <;> rfl

/-- A reused result whose principal port reads a live peer's principal port
publishes exactly the canonical tuple at the reused slot's next generation. -/
theorem PhysicalState.publicationFor_reuse_active
    {state : PhysicalState} {pair : PhysicalPair}
    {boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {index peer : Nat} {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {other old : PhysicalSlot}
    (hport : result.2.2.2.2[0]? = some (some index))
    (hboundary : boundaries[index]? = some boundary)
    (hkind : boundary.2 = (0, peer, 0))
    (hfirst : peer ≠ pair.first) (hsecond : peer ≠ pair.second)
    (hself : peer ≠ result.2.1)
    (hpeer : state.slots[peer]? = some other) (hlive : other.agent.isSome = true)
    (hreuse : result.1 = false)
    (hold : state.slots[result.2.1]? = some old)
    (hroom : old.generation ≠ UInt64.ofNat (UInt64.size - 1)) :
    state.publicationFor pair boundaries result = some
      (if result.2.1 < peer
        then [(result.2.1, old.generation.toNat + 1, peer, other.generation.toNat)]
        else [(peer, other.generation.toNat, result.2.1, old.generation.toNat + 1)]) := by
  obtain ⟨source, kindPeer⟩ := boundary
  simp only at hkind
  subst hkind
  have hsome : other.agent ≠ none := by
    intro ha
    simp [ha] at hlive
  have hroom' : ¬ old.generation = 18446744073709551615 := by simpa using hroom
  simp [PhysicalState.publicationFor, hport, hboundary, hfirst, hsecond, hself,
    hpeer, hsome, hreuse, hold, hroom']
  split <;> rfl

/-- One non-aliased auxiliary port of a legal live rule slot yields a captured
edge: its physical endpoint is a reciprocal live peer outside the consumed pair
or its unique output, and the prestate projection wires the rule port to that
endpoint's logical port or stable label. -/
theorem PhysicalState.capture_edge
    {state : PhysicalState} (hLegal : state.Legal)
    {ruleSlot valueSlot : Nat} {ruleOld : PhysicalSlot} {ruleId : Id}
    {ruleSymbol : NetSym} {port : Nat}
    (hrule : state.slots[ruleSlot]? = some ruleOld)
    (hagent : ruleOld.agent = some (ruleId, ruleSymbol))
    (hport : port ≤ NetI64.arity ruleSymbol)
    (hnoAlias : ∀ peer peerPort, ruleOld.ports[port]? = some (some (peer, peerPort)) →
      peer ≠ ruleSlot ∧ peer ≠ valueSlot) :
    ∃ boundary endpoint, state.captureAux ruleSlot port = some boundary ∧
      boundary.1 = (ruleSlot, port) ∧
      ((∃ peer peerPort peerSlot peerId peerSymbol,
          boundary.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, port)) ∧
          endpoint = (peerId, peerPort)) ∨
       (∃ output out, boundary.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, port) ∧ endpoint = out.label)) ∧
      state.project.wires[(ruleId, port)]? = some endpoint ∧
      state.project.wires[endpoint]? = some (ruleId, port) ∧
      ruleOld.ports[port]? = some (PhysicalState.boundaryReference boundary) := by
  obtain ⟨boundary, hcap, hsource, hcase⟩ :=
    PhysicalState.captureAux_project hLegal hrule hagent hport
  rcases hcase with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, hcontent, hpeer,
      hpeerAgent, hpeerPort, _, _, _, hwire, hback⟩ |
    ⟨output, out, hk, hcontent, houtput, htarget, _, hwire, hback⟩
  · obtain ⟨h1, h2⟩ := hnoAlias peer peerPort hcontent
    refine ⟨boundary, (peerId, peerPort), hcap, hsource,
      .inl ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer, hpeerAgent,
        hpeerPort, rfl⟩, hwire, hback, ?_⟩
    simp [hcontent, PhysicalState.boundaryReference, hk]
  · refine ⟨boundary, out.label, hcap, hsource,
      .inr ⟨output, out, hk, houtput, htarget, rfl⟩, hwire, hback, ?_⟩
    simp [hcontent, PhysicalState.boundaryReference, hk]

/-- Captured edges leaving two different ports of the same rule slot have
different physical endpoints. -/
theorem PhysicalState.captured_targets_distinct
    {state : PhysicalState} {ruleSlot valueSlot first second : Nat}
    {firstTarget secondTarget : Nat × Nat × Nat} {firstEnd secondEnd : Port}
    (hports : first ≠ second)
    (hfirst : (∃ peer peerPort peerSlot peerId peerSymbol,
          firstTarget = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, first)) ∧
          firstEnd = (peerId, peerPort)) ∨
       (∃ output out, firstTarget = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, first) ∧ firstEnd = out.label))
    (hsecond : (∃ peer peerPort peerSlot peerId peerSymbol,
          secondTarget = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, second)) ∧
          secondEnd = (peerId, peerPort)) ∨
       (∃ output out, secondTarget = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, second) ∧ secondEnd = out.label)) :
    firstTarget ≠ secondTarget := by
  intro heq
  subst heq
  rcases hfirst with ⟨peer, peerPort, peerSlot, _, _, hk, _, _, hpeer, _, hp, _⟩ |
    ⟨output, out, hk, houtput, htarget, _⟩ <;>
  rcases hsecond with ⟨peer', peerPort', peerSlot', _, _, hk', _, _, hpeer', _, hp', _⟩ |
    ⟨output', out', hk', houtput', htarget', _⟩
  · rw [hk] at hk'
    simp only [Prod.mk.injEq, true_and] at hk'
    obtain ⟨rfl, rfl⟩ := hk'
    rw [hpeer] at hpeer'
    cases hpeer'
    rw [hp] at hp'
    simp at hp'
    exact hports hp'
  · rw [hk] at hk'
    simp at hk'
  · rw [hk] at hk'
    simp at hk'
  · rw [hk] at hk'
    simp only [Prod.mk.injEq, true_and, and_true] at hk'
    subst hk'
    rw [houtput] at houtput'
    cases houtput'
    rw [htarget] at htarget'
    simp at htarget'
    exact hports htarget'

/-- Installing the single result of an OP1/OP2 image, donated by a live rule
slot with headroom, either rewrites that slot at the next generation (reuse)
or appends it at generation zero (fresh). -/
theorem PhysicalState.install_one_result
    {state : PhysicalState} {plan : BytePlan} {left right : Id}
    {rhs : XRhs NetSym} {reuse : Bool} {slot : Nat} {old : PhysicalSlot}
    {symbol : NetSym} {ports : List (Option Nat)}
    {resolved : List (Option (Nat × Nat))} {rhsSlot : Option Bool × Nat}
    (hold : state.slots[slot]? = some old) (hlive : old.agent.isSome = true)
    (hroom : old.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hrhs : rhs.slots[0]? = some rhsSlot)
    (hlab : rhs.lab rhsSlot.1 rhsSlot.2 = some symbol)
    (hports : ports.mapM plan.resultTarget = some resolved) :
    ∃ installed, PhysicalState.installResults plan left right rhs 0 state
        [(!reuse, if reuse then slot else state.slots.size, symbol, slot, ports)] =
        some installed ∧
      state.InstallPost installed
        [{ slot := if reuse then slot else state.slots.size
           id := child left right rhsSlot.1 rhsSlot.2
           symbol
           generation := if reuse then old.generation + 1 else 0
           ports := resolved }]
        (if reuse then 0 else 1) := by
  have hbound : slot < state.slots.size := (getElem?_eq_some_iff.mp hold).1
  cases reuse with
  | true =>
    obtain ⟨installed, hinstall, hsize, houts, hwork, hfree, hframe, hslots⟩ :=
      PhysicalState.installResults_reuse (plan := plan) (left := left) (right := right)
        (rhs := rhs) [(false, slot, symbol, slot, ports)] 0 state (by simp)
        (by
          intro j result hj
          cases j with
          | zero =>
            simp at hj
            subst result
            exact ⟨rfl, rfl, ⟨old, hold, hlive, hroom⟩, rhsSlot, resolved, hrhs, hlab, hports⟩
          | succ j => simp at hj)
    refine ⟨installed, by simpa using hinstall, ⟨by simpa using hsize, houts, hwork, hfree,
      ?_, ?_⟩⟩
    · intro index hindex hnot
      exact hframe index (by simpa using hnot)
    · intro result hresult
      simp at hresult
      subst result
      simpa using hslots 0 (false, slot, symbol, slot, ports) rhsSlot resolved old
        (by simp) hrhs hports hold
  | false =>
    obtain ⟨installed, hinstall, hsize, houts, hwork, hfree, hframe, hslots⟩ :=
      PhysicalState.installResults_fresh (plan := plan) (left := left) (right := right)
        (rhs := rhs) [(true, state.slots.size, symbol, slot, ports)] 0 state
        (by
          intro j result hj
          cases j with
          | zero =>
            simp at hj
            subst result
            exact ⟨rfl, rfl, hbound, ⟨old, hold, hlive⟩, rhsSlot, resolved, hrhs, hlab,
              hports⟩
          | succ j => simp at hj)
    refine ⟨installed, by simpa using hinstall, ⟨by simpa using hsize, houts, hwork, hfree,
      ?_, ?_⟩⟩
    · intro index hindex _
      exact hframe index hindex
    · intro result hresult
      simp at hresult
      subst result
      simpa using hslots 0 (true, state.slots.size, symbol, slot, ports) rhsSlot resolved
        (by simp) hrhs hports

/-- One entry of `BytePlan.logicalBoundary`, as a named function. -/
def BytePlan.boundaryEntry (state : PhysicalState)
    (boundary : (Nat × Nat) × (Nat × Nat × Nat)) : Option (Port × Port) :=
  match boundary with
  | (source, target) => do
    let left ← state.logicalPort source
    let right ← match target with
      | (0, slot, port) => state.logicalPort (slot, port)
      | (1, output, 0) => (state.outputs[output]?).map PhysicalOutput.label
      | _ => none
    some (left, right)

/-- `BytePlan.logicalBoundary` decodes entry by entry. -/
theorem BytePlan.logicalBoundary_eq_mapM (state : PhysicalState) (plan : BytePlan) :
    plan.logicalBoundary state = plan.boundaries.mapM (BytePlan.boundaryEntry state) := rfl

/-- A captured boundary decodes against the prestate to the rule agent's
logical port and the captured endpoint's logical port or stable label. -/
theorem PhysicalState.logicalBoundary_entry
    {state : PhysicalState} {ruleSlot valueSlot port : Nat} {ruleOld : PhysicalSlot}
    {ruleId : Id} {ruleSymbol : NetSym} {endpoint : Port}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    (hrule : state.slots[ruleSlot]? = some ruleOld)
    (hagent : ruleOld.agent = some (ruleId, ruleSymbol))
    (hsource : boundary.1 = (ruleSlot, port))
    (hcapture : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundary.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, port)) ∧
          endpoint = (peerId, peerPort)) ∨
       (∃ output out, boundary.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, port) ∧ endpoint = out.label)) :
    BytePlan.boundaryEntry state boundary = some ((ruleId, port), endpoint) := by
  obtain ⟨source, target⟩ := boundary
  simp only at hsource
  subst hsource
  rcases hcapture with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, _, _, hpeer,
      hpeerAgent, _, hend⟩ | ⟨output, out, hk, houtput, _, hend⟩
  · simp only at hk
    subst hk
    subst hend
    simp [BytePlan.boundaryEntry, PhysicalState.logicalPort, hrule, hagent, hpeer, hpeerAgent]
  · simp only at hk
    subst hk
    subst hend
    simp [BytePlan.boundaryEntry, PhysicalState.logicalPort, hrule, hagent, houtput]

/-- **Emitted OP2 admission**, for either orientation and storage policy: an
actually lowered OP2 image of a legal, current, matched arena that is not
excluded by queue capacity executes and passes the independent semantic
checker at its exact reservations. -/
theorem PhysicalState.op2_accepted
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    {ruleSlot valueSlot : Nat} {ruleId valueId : Id} {op : Op} {value : BitVec 64}
    {ruleOld valueOld : PhysicalSlot} {rhsSlot : Option Bool × Nat}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)} {p1 p2 : Port}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (redex.1, redex.2.1))
    (horient : (pair.first = ruleSlot ∧ pair.second = valueSlot) ∨
      (pair.first = valueSlot ∧ pair.second = ruleSlot))
    (hids : (redex.1 = ruleId ∧ redex.2.1 = valueId) ∨
      (redex.1 = valueId ∧ redex.2.1 = ruleId))
    (hrule : state.slots[ruleSlot]? = some ruleOld)
    (hruleAgent : ruleOld.agent = some (ruleId, .op2 op))
    (hrulePort : ruleOld.ports[0]? = some (some (valueSlot, 0)))
    (hruleRoom : ruleOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hvalue : state.slots[valueSlot]? = some valueOld)
    (hvalueAgent : valueOld.agent = some (valueId, .num value))
    (hvaluePort : valueOld.ports[0]? = some (some (ruleSlot, 0)))
    (hvalueRoom : valueOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hdistinct : ruleSlot ≠ valueSlot)
    (hrhsSlot : redex.2.2.2.2.slots[0]? = some rhsSlot)
    (hrhsLab : redex.2.2.2.2.lab rhsSlot.1 rhsSlot.2 = some (.op1 op value))
    (hchild : child redex.1 redex.2.1 rhsSlot.1 rhsSlot.2 = 0 :: ruleId)
    (hsource1 : boundaryOne.1 = (ruleSlot, 1))
    (hcapture1 : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundaryOne.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, 1)) ∧
          p1 = (peerId, peerPort)) ∨
       (∃ output out, boundaryOne.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, 1) ∧ p1 = out.label))
    (hsource2 : boundaryTwo.1 = (ruleSlot, 2))
    (hcapture2 : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundaryTwo.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, 2)) ∧
          p2 = (peerId, peerPort)) ∨
       (∃ output out, boundaryTwo.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, 2) ∧ p2 = out.label))
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hresults : plan.results = [(!reuse, if reuse then ruleSlot else state.slots.size,
      .op1 op value, ruleSlot, [some 0, some 1, none])])
    (hboundaries : plan.boundaries = [boundaryOne, boundaryTwo])
    (hwrites : plan.externalWrites = [(0, 0, 0), (0, 1, 1)])
    (hpublications : state.publicationFor pair [boundaryOne, boundaryTwo]
      (!reuse, if reuse then ruleSlot else state.slots.size, .op1 op value, ruleSlot,
        [some 0, some 1, none]) = some plan.publications)
    (haction : plan.action = (if reuse ∧ ruleSlot = pair.first then 1 else 0,
      if reuse ∧ ruleSlot = pair.second then 1 else 0))
    (hpairPlan : plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat))
    (hselAgents : (Plan.compile state.project redex hRule).initializedAgents =
      [(0 :: ruleId, .op1 op value)])
    (hselWrites : (Plan.compile state.project redex hRule).wireWrites =
      [((0 :: ruleId, 0), p1), ((0 :: ruleId, 1), p2),
        (p1, (0 :: ruleId, 0)), (p2, (0 :: ruleId, 1))])
    (hselErased : ∀ port,
      (port ∈ (Plan.compile state.project redex hRule).capturedBoundary.map Prod.fst ∨
        port ∈ (Plan.compile state.project redex hRule).capturedBoundary.map Prod.snd) ↔
      (port = (ruleId, 0) ∨ port = (valueId, 0) ∨ port = (ruleId, 1) ∨
        port = (ruleId, 2) ∨ port = p1 ∨ port = p2))
    (hselBoundary : (Plan.compile state.project redex hRule).capturedBoundary.filter
      (fun edge => edge.1.2 != 0) = [((ruleId, 1), p1), ((ruleId, 2), p2)])
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit redex hRule plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  have hdecode := PhysicalState.lowerPlan_decodes hLower
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨_, _, _, _, _, harena, _, hqueue⟩ := PhysicalState.planCandidate_fields hc
  have hruleBound : ruleSlot < state.slots.size := (getElem?_eq_some_iff.mp hrule).1
  have hvalueBound : valueSlot < state.slots.size := (getElem?_eq_some_iff.mp hvalue).1
  have hruleLive : ruleOld.agent.isSome = true := by simp [hruleAgent]
  have hvalueLive : valueOld.agent.isSome = true := by simp [hvalueAgent]
  -- the resolved result ports
  have hkind1 : (∃ peer port, boundaryOne.2 = (0, peer, port)) ∨
      (∃ output, boundaryOne.2 = (1, output, 0)) := by
    rcases hcapture1 with ⟨peer, port, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
    · exact .inl ⟨peer, port, hk⟩
    · exact .inr ⟨output, hk⟩
  have hkind2 : (∃ peer port, boundaryTwo.2 = (0, peer, port)) ∨
      (∃ output, boundaryTwo.2 = (1, output, 0)) := by
    rcases hcapture2 with ⟨peer, port, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
    · exact .inl ⟨peer, port, hk⟩
    · exact .inr ⟨output, hk⟩
  have hports : ([some 0, some 1, none] : List (Option Nat)).mapM plan.resultTarget =
      some [PhysicalState.boundaryReference boundaryOne,
        PhysicalState.boundaryReference boundaryTwo, none] := by
    have h0 := BytePlan.resultTarget_boundary (index := 0)
      (by rw [hboundaries]; rfl) hkind1
    have h1 := BytePlan.resultTarget_boundary (index := 1)
      (by rw [hboundaries]; rfl) hkind2
    have hn : plan.resultTarget none = some none := rfl
    simp [List.mapM_cons, h0, h1, hn]
  -- installation
  obtain ⟨installed, hinstall, hpost⟩ := PhysicalState.install_one_result
    (plan := plan) (left := redex.1) (right := redex.2.1) (rhs := redex.2.2.2.2)
    (reuse := reuse) hrule hruleLive hruleRoom hrhsSlot hrhsLab hports
  rw [← hresults] at hinstall
  rw [hchild] at hpost
  -- the decoded pair is the selected current pair
  have hpairEq : PhysicalPair.mk plan.pair.1 (UInt64.ofNat plan.pair.2.1)
      plan.pair.2.2.1 (UInt64.ofNat plan.pair.2.2.2) = pair := by
    rw [hpairPlan]
    cases pair
    simp [UInt64.ofNat_toNat]
  have hcurrentPlan := hpairEq ▸ hcurrent
  have hfirstPlan : plan.pair.1 = pair.first := by rw [hpairPlan]
  have hsecondPlan : plan.pair.2.2.1 = pair.second := by rw [hpairPlan]
  have hpeerAway : ∀ peer, peer ≠ ruleSlot → peer ≠ valueSlot →
      peer ≠ pair.first ∧ peer ≠ pair.second := by
    intro peer h1 h2
    rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> rw [hf, hs] <;> exact ⟨by assumption, by assumption⟩
  -- views of the installed result and both captured edges
  have hslotR : (if reuse then ruleSlot else state.slots.size) = ruleSlot ∨
      (if reuse then ruleSlot else state.slots.size) = state.slots.size := by
    cases reuse <;> simp
  have hpeerNotSlot : ∀ peer, peer < state.slots.size → peer ≠ ruleSlot →
      peer ≠ (if reuse then ruleSlot else state.slots.size) := by
    intro peer hlt hne
    cases reuse <;> simp <;> omega
  -- admissible reciprocal writes
  have hresult0 : plan.results[0]? = some (!reuse, if reuse then ruleSlot else state.slots.size,
      .op1 op value, ruleSlot, [some 0, some 1, none]) := by
    rw [hresults]; rfl
  have hready1 : state.WriteReady plan (0, 0, 0) := by
    refine ⟨_, boundaryOne, hresult0, by rw [hboundaries]; rfl, rfl, ?_⟩
    rcases hcapture1 with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
        hpeerAgent, hpeerPort, _⟩ | ⟨output, out, hk, houtput, htarget, _⟩
    · obtain ⟨hf, hs⟩ := hpeerAway peer h1 h2
      exact .inl ⟨peer, peerPort, peerSlot, hk, hfirstPlan ▸ hf, hsecondPlan ▸ hs, hpeer,
        by simp [hpeerAgent], by rw [hsource1]; exact hpeerPort⟩
    · exact .inr ⟨output, out, hk, houtput, by rw [hsource1]; exact htarget⟩
  have hready2 : state.WriteReady plan (0, 1, 1) := by
    refine ⟨_, boundaryTwo, hresult0, by rw [hboundaries]; rfl, rfl, ?_⟩
    rcases hcapture2 with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
        hpeerAgent, hpeerPort, _⟩ | ⟨output, out, hk, houtput, htarget, _⟩
    · obtain ⟨hf, hs⟩ := hpeerAway peer h1 h2
      exact .inl ⟨peer, peerPort, peerSlot, hk, hfirstPlan ▸ hf, hsecondPlan ▸ hs, hpeer,
        by simp [hpeerAgent], by rw [hsource2]; exact hpeerPort⟩
    · exact .inr ⟨output, out, hk, houtput, by rw [hsource2]; exact htarget⟩
  have htarget1 : plan.writeTarget (0, 0, 0) = some boundaryOne.2 := by
    simp [BytePlan.writeTarget, hboundaries]
  have htarget2 : plan.writeTarget (0, 1, 1) = some boundaryTwo.2 := by
    simp [BytePlan.writeTarget, hboundaries]
  have hsource1' : plan.writeSource (0, 0, 0) =
      some (if reuse then ruleSlot else state.slots.size, 0) := by
    simp [BytePlan.writeSource, hresult0]
  have hsource2' : plan.writeSource (0, 1, 1) =
      some (if reuse then ruleSlot else state.slots.size, 1) := by
    simp [BytePlan.writeSource, hresult0]
  have hdistinctTargets : boundaryOne.2 ≠ boundaryTwo.2 :=
    PhysicalState.captured_targets_distinct (by decide) hcapture1 hcapture2
  -- publications: only a live peer's principal port on the result principal
  have hpubShape : (plan.publishedPairs = [] ∧ ∀ peer, boundaryOne.2 ≠ (0, peer, 0)) ∨
      ∃ peer peerSlot, boundaryOne.2 = (0, peer, 0) ∧ state.slots[peer]? = some peerSlot ∧
        peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
        plan.publishedPairs = [PhysicalState.canonicalPair
          (if reuse then ruleSlot else state.slots.size)
          (if reuse then ruleOld.generation + 1 else 0) peer peerSlot.generation] := by
    rcases hcapture1 with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
        hpeerAgent, _, _⟩ | ⟨output, out, hk, _, _, _⟩
    · by_cases hz : peerPort = 0
      · subst hz
        right
        refine ⟨peer, peerSlot, hk, hpeer, h1, h2, ?_⟩
        obtain ⟨hf, hs⟩ := hpeerAway peer h1 h2
        have hpeerBound : peer < state.slots.size := (getElem?_eq_some_iff.mp hpeer).1
        have hself := hpeerNotSlot peer hpeerBound h1
        have hpeerLive : peerSlot.agent.isSome = true := by simp [hpeerAgent]
        cases reuse with
        | true =>
          have hp := PhysicalState.publicationFor_reuse_active
            (boundaries := [boundaryOne, boundaryTwo]) (index := 0) (pair := pair)
            (result := (!true, if true then ruleSlot else state.slots.size, .op1 op value,
              ruleSlot, [some 0, some 1, none]))
            rfl rfl hk hf hs hself hpeer hpeerLive rfl hrule hruleRoom
          rw [hp] at hpublications
          have hpubs := (Option.some.inj hpublications).symm
          simp only [BytePlan.publishedPairs, hpubs]
          by_cases hlt : ruleSlot < peer
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_add, UInt64.ofNat_toNat]
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_add, UInt64.ofNat_toNat]
        | false =>
          have hp := PhysicalState.publicationFor_fresh_active
            (boundaries := [boundaryOne, boundaryTwo]) (index := 0) (pair := pair)
            (result := (!false, if false then ruleSlot else state.slots.size, .op1 op value,
              ruleSlot, [some 0, some 1, none]))
            rfl rfl hk hf hs hself hpeer hpeerLive rfl
          rw [hp] at hpublications
          have hpubs := (Option.some.inj hpublications).symm
          simp only [BytePlan.publishedPairs, hpubs]
          by_cases hlt : state.slots.size < peer
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_toNat]
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_toNat]
      · left
        have hp := PhysicalState.publicationFor_quiet (state := state)
          (boundaries := [boundaryOne, boundaryTwo]) (index := 0) (pair := pair)
          (result := (!reuse, if reuse then ruleSlot else state.slots.size, .op1 op value,
            ruleSlot, [some 0, some 1, none])) rfl rfl (.inr (by rw [hk]; exact hz))
        rw [hp] at hpublications
        refine ⟨by simp [BytePlan.publishedPairs, ← (Option.some.inj hpublications)], ?_⟩
        intro peer' h
        rw [hk] at h
        simp at h
        exact hz h.2
    · left
      have hp := PhysicalState.publicationFor_quiet (state := state)
        (boundaries := [boundaryOne, boundaryTwo]) (index := 0) (pair := pair)
        (result := (!reuse, if reuse then ruleSlot else state.slots.size, .op1 op value,
          ruleSlot, [some 0, some 1, none])) rfl rfl (.inl (by simp [hk]))
      rw [hp] at hpublications
      refine ⟨by simp [BytePlan.publishedPairs, ← (Option.some.inj hpublications)], ?_⟩
      intro peer' h
      rw [hk] at h
      simp at h
  -- the edge views
  let slotR : Nat := if reuse then ruleSlot else state.slots.size
  let view : ResultView :=
    { slot := slotR, id := 0 :: ruleId, symbol := .op1 op value,
      generation := if reuse then ruleOld.generation + 1 else 0,
      ports := [PhysicalState.boundaryReference boundaryOne,
        PhysicalState.boundaryReference boundaryTwo, none] }
  let edgeOne : EdgeView :=
    { sourcePort := 1, target := boundaryOne.2, endpoint := p1,
      result := (slotR, 0), resultId := 0 :: ruleId }
  let edgeTwo : EdgeView :=
    { sourcePort := 2, target := boundaryTwo.2, endpoint := p2,
      result := (slotR, 1), resultId := 0 :: ruleId }
  have hrefPeer : ∀ (boundary : (Nat × Nat) × (Nat × Nat × Nat)) (peer peerPort : Nat),
      boundary.2 = (0, peer, peerPort) →
      PhysicalState.boundaryReference boundary = some (peer, peerPort) := by
    intro boundary peer peerPort hk
    simp [PhysicalState.boundaryReference, hk]
  have hcapacityPub : plan.publishedPairs = [] ∨
      state.work.length + plan.publishedPairs.length ≤
        state.slots.size + (if reuse then 0 else 1) := by
    rcases hpubShape with ⟨hnone, _⟩ | ⟨peer, peerSlot, _, _, _, _, hone⟩
    · exact .inl hnone
    · right
      have hworklist : plan.worklistReservation = 1 := by
        rw [hqueue]
        have := congrArg List.length hone
        simpa [BytePlan.publishedPairs] using this
      have harena' : plan.arenaReservation = if reuse then 0 else 1 := by
        rw [harena, hresults]
        cases reuse <;> simp
      simp only [PhysicalState.queueCapacityExcluded, decide_eq_false_iff_not] at hcapacity
      rw [hone]
      simp only [List.length_singleton]
      rw [hworklist, harena'] at hcapacity
      omega
  obtain ⟨next, hexec, hobs⟩ := PhysicalState.execute_observations
    (redex := redex) (ruleSlot := ruleSlot) (valueSlot := valueSlot)
    (results := [view]) (edges := [edgeOne, edgeTwo])
    (fresh := if reuse then 0 else 1)
    hdecode hcurrentPlan (by rw [hfirstPlan, hsecondPlan]; exact horient) hdistinct
    hinstall hpost
    (by
      intro result hr
      simp at hr
      subst hr
      cases reuse <;> simp [view, slotR])
    (by
      rw [hwrites]
      intro write hw
      simp at hw
      rcases hw with rfl | rfl
      · exact hready1
      · exact hready2)
    (by
      rw [hwrites]
      simp [htarget1, htarget2, hdistinctTargets])
    (by
      intro edge he
      simp at he
      rcases he with rfl | rfl
      · exact ⟨(0, 0, 0), by rw [hwrites]; simp, htarget1, hsource1'⟩
      · exact ⟨(0, 1, 1), by rw [hwrites]; simp, htarget2, hsource2'⟩)
    (by
      rw [hwrites]
      intro write hw
      simp at hw
      rcases hw with rfl | rfl
      · exact ⟨edgeOne, by simp, htarget1, hsource1'⟩
      · exact ⟨edgeTwo, by simp, htarget2, hsource2'⟩)
    (by
      rw [haction, hfirstPlan]
      cases reuse <;> simp [view, slotR] <;>
        rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> simp [hf] <;> omega)
    (by
      rw [haction, hsecondPlan]
      cases reuse <;> simp [view, slotR] <;>
        rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> simp [hs] <;> omega)
    (by
      rw [hfirstPlan]
      rcases horient with ⟨hf, _⟩ | ⟨hf, _⟩ <;> rw [hf]
      · exact ⟨ruleOld, hrule, hruleLive, hruleRoom⟩
      · exact ⟨valueOld, hvalue, hvalueLive, hvalueRoom⟩)
    (by
      rw [hsecondPlan]
      rcases horient with ⟨_, hs⟩ | ⟨_, hs⟩ <;> rw [hs]
      · exact ⟨valueOld, hvalue, hvalueLive, hvalueRoom⟩
      · exact ⟨ruleOld, hrule, hruleLive, hruleRoom⟩)
    (by
      intro result hr edge he port peer peerPort hresult htarget
      simp at hr
      subst hr
      simp at he
      rcases he with rfl | rfl
      · simp [edgeOne] at hresult htarget
        obtain ⟨_, rfl⟩ := hresult
        simp [view, hrefPeer _ _ _ htarget]
      · simp [edgeTwo] at hresult htarget
        obtain ⟨_, rfl⟩ := hresult
        simp [view, hrefPeer _ _ _ htarget])
    (by
      intro queued hq
      rcases hpubShape with ⟨hnone, _⟩ | ⟨peer, peerSlot, hk, hpeer, _, _, hone⟩
      · rw [hnone] at hq
        simp at hq
      · rw [hone] at hq
        simp at hq
        subst hq
        exact ⟨view, by simp, edgeOne, by simp, peer, peerSlot, rfl, hk, hpeer, rfl⟩)
    (by
      rcases hpubShape with ⟨hnone, _⟩ | ⟨peer, peerSlot, _, hpeer, h1, _, hone⟩
      · rw [hnone]
        simp
      · rw [hone]
        have hpeerBound : peer < state.slots.size := (getElem?_eq_some_iff.mp hpeer).1
        have hne := hpeerNotSlot peer hpeerBound h1
        generalize (if reuse then ruleSlot else state.slots.size) = slot at hne ⊢
        unfold PhysicalState.canonicalPair
        by_cases hlt : slot < peer <;> simp [hlt] <;> omega)
    hcapacityPub hLegal.2.2.2.2.1
  -- the abstract boundary agrees with the physical capture
  have hBoundary : plan.logicalBoundary state = some
      ((Plan.compile state.project redex hRule).capturedBoundary.filter
        fun edge => edge.1.2 != 0) := by
    rw [hselBoundary]
    have h1 := PhysicalState.logicalBoundary_entry hrule hruleAgent hsource1 hcapture1
    have h2 := PhysicalState.logicalBoundary_entry hrule hruleAgent hsource2 hcapture2
    rw [BytePlan.logicalBoundary_eq_mapM, hboundaries]
    simp [List.mapM_cons, h1, h2]
  -- slots outside the plan's touched set are outside every rewritten location
  have hTouched : ∀ index, index < state.slots.size →
      plan.touchedSlots.contains index = false →
      index ≠ ruleSlot ∧ index ≠ valueSlot ∧
      (∀ edge ∈ [edgeOne, edgeTwo], ∀ port, edge.target ≠ (0, index, port)) ∧
      (∀ queued ∈ plan.publishedPairs, queued.first ≠ index ∧ queued.second ≠ index) := by
    intro index _ hnot
    have hmem : index ∉ plan.touchedSlots := by
      intro h
      have := List.contains_iff_mem.mpr h
      rw [hnot] at this
      exact Bool.false_ne_true this
    simp only [BytePlan.touchedSlots, hboundaries, List.mem_append, List.mem_cons,
      List.not_mem_nil, or_false, not_or, List.mem_filterMap, List.mem_flatMap,
      not_exists, not_and] at hmem
    obtain ⟨⟨⟨hfirst, hsecond⟩, hpeers⟩, hpubs⟩ := hmem
    rw [hfirstPlan] at hfirst
    rw [hsecondPlan] at hsecond
    refine ⟨?_, ?_, ?_, ?_⟩
    · rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> omega
    · rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> omega
    · intro edge he port htarget
      simp at he
      rcases he with rfl | rfl
      · exact hpeers boundaryOne (.inl rfl) (by
          have htarget' : boundaryOne.2 = (0, index, port) := htarget
          rw [htarget']; simp)
      · exact hpeers boundaryTwo (.inr rfl) (by
          have htarget' : boundaryTwo.2 = (0, index, port) := htarget
          rw [htarget']; simp)
    · intro queued hq
      simp only [BytePlan.publishedPairs, List.mem_map] at hq
      obtain ⟨⟨f, fg, s, sg⟩, htuple, rfl⟩ := hq
      have hnotIn := hpubs (f, fg, s, sg) htuple
      exact ⟨fun h => hnotIn.1 h.symm, fun h => hnotIn.2 h.symm⟩
  -- the local-rewrite bundle
  have hRewrite : state.LocalRewrite next (Plan.compile state.project redex hRule)
      ruleSlot valueSlot ruleId valueId (.op2 op) (.num value)
      [view] [edgeOne, edgeTwo] plan.retiredSlots (if reuse then 0 else 1)
      plan.publishedPairs := by
    obtain ⟨hfreshId, hfreshLabel⟩ :=
      PhysicalState.extension_unused hLegal hm hrule hruleAgent 0
    have hretired : plan.retiredSlots =
        (if reuse then [valueSlot] else [valueSlot, ruleSlot]) ∨
        (plan.retiredSlots = if reuse then [valueSlot] else [ruleSlot, valueSlot]) := by
      simp only [BytePlan.retiredSlots, haction, hfirstPlan, hsecondPlan]
      rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩
      · left
        cases reuse <;> simp [hf, hs, hdistinct]
      · right
        cases reuse <;> simp [hf, hs, hdistinct]
    refine
      { legal := hLegal
        ruleOld := ⟨ruleOld, hrule, hruleAgent, hrulePort⟩
        valueOld := ⟨valueOld, hvalue, hvalueAgent, hvaluePort⟩
        valueArity := rfl
        distinctSlots := hdistinct
        outputsInArity := fun output hmem =>
          PhysicalState.output_target_in_arity hLegal hm hmem
        edgePorts := rfl
        edgeTargets := by simp [edgeOne, edgeTwo, hdistinctTargets]
        edgeCapture := ?_
        resultSlots := by simp
        resultIds := by simp
        resultFresh := ?_
        resultPlace := ?_
        resultGeneration := ?_
        freshFilled := ?_
        resultArity := ?_
        resultPorts := ?_
        edgeResult := ?_
        edgeResults := by simp [edgeOne, edgeTwo]
        retiredExact := ?_
        retiredNodup := ?_
        observed := hobs
        publishedExact := ?_
        publishedNodup := ?_
        selectedPair := hids
        selectedAgents := by rw [hselAgents]; rfl
        selectedWrites := by rw [hselWrites]; rfl
        selectedErased := ?_
        selectedActive := rfl }
    · intro edge he
      simp at he
      rcases he with rfl | rfl
      · exact hcapture1
      · exact hcapture2
    · intro result hr
      simp at hr
      subst hr
      exact ⟨hfreshId, hfreshLabel⟩
    · intro result hr
      simp at hr
      subst hr
      cases reuse <;> simp [view, slotR]
    · intro result hr old hold
      simp at hr
      subst hr
      cases reuse with
      | true =>
        simp only [view, slotR, ↓reduceIte] at hold ⊢
        rw [hrule] at hold
        cases hold
        exact ⟨hruleRoom, rfl⟩
      | false =>
        simp [view, slotR] at hold
    · intro index hlow hhigh
      cases reuse with
      | true => simp at hhigh; omega
      | false =>
        refine ⟨view, by simp, ?_⟩
        simp [view, slotR]
        simp at hhigh
        omega
    · intro result hr
      simp at hr
      subst hr
      simp [view, NetI64.arity]
    · intro result hr port target hport
      simp at hr
      subst hr
      match port, hport with
      | 0, hport =>
        left
        refine ⟨edgeOne, by simp, rfl, rfl, by simp [view, NetI64.arity], ?_⟩
        simp [view] at hport
        subst hport
        rcases hcapture1 with ⟨peer, peerPort, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
        · exact .inl ⟨peer, peerPort, hk, by simp [PhysicalState.boundaryReference, hk]⟩
        · exact .inr ⟨output, hk, by simp [PhysicalState.boundaryReference, hk]⟩
      | 1, hport =>
        left
        refine ⟨edgeTwo, by simp, rfl, rfl, by simp [view, NetI64.arity], ?_⟩
        simp [view] at hport
        subst hport
        rcases hcapture2 with ⟨peer, peerPort, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
        · exact .inl ⟨peer, peerPort, hk, by simp [PhysicalState.boundaryReference, hk]⟩
        · exact .inr ⟨output, hk, by simp [PhysicalState.boundaryReference, hk]⟩
      | 2, hport =>
        right
        simp [view] at hport
        refine ⟨by simp [edgeOne, edgeTwo], by simp [view, NetI64.arity], hport.symm⟩
      | n + 3, hport =>
        simp [view] at hport
    · intro edge he
      simp at he
      rcases he with rfl | rfl
      · exact ⟨view, by simp, rfl, rfl, by simp [view, edgeOne]⟩
      · exact ⟨view, by simp, rfl, rfl, by simp [view, edgeTwo]⟩
    · intro index
      have hmemR : index ∈ plan.retiredSlots ↔
          (if reuse then index = valueSlot else (index = valueSlot ∨ index = ruleSlot)) := by
        rcases hretired with hr | hr <;> rw [hr] <;> cases reuse <;> simp [or_comm]
      rw [hmemR]
      cases reuse with
      | true =>
        simp only [view, slotR, ↓reduceIte, List.mem_singleton, forall_eq]
        constructor
        · intro h
          exact ⟨.inr h, fun he => hdistinct ((show ruleSlot = index from he).trans h)⟩
        · rintro ⟨h | h, hne⟩
          · simp at hne
            omega
          · exact h
      | false =>
        simp only [view, slotR, Bool.false_eq_true, ↓reduceIte, List.mem_singleton,
          forall_eq]
        constructor
        · rintro (h | h)
          · exact ⟨.inr h, by omega⟩
          · exact ⟨.inl h, by omega⟩
        · rintro ⟨h | h, _⟩
          · exact .inr h
          · exact .inl h
    · rcases hretired with hr | hr <;> rw [hr] <;> cases reuse <;>
        simp [hdistinct, Ne.symm hdistinct]
    · intro queued
      rcases hpubShape with ⟨hnone, hquiet⟩ | ⟨peer, peerSlot, hk, hpeer, _, _, hone⟩
      · rw [hnone]
        simp only [List.not_mem_nil, false_iff]
        rintro ⟨result, hr, edge, he, peer, peerSlot, hresult, htarget, _, _⟩
        simp at hr
        subst hr
        simp at he
        rcases he with rfl | rfl
        · exact hquiet peer htarget
        · simp [edgeTwo, view] at hresult
      · rw [hone]
        simp only [List.mem_singleton]
        constructor
        · intro hq
          exact ⟨view, by simp, edgeOne, by simp, peer, peerSlot, rfl, hk, hpeer, hq⟩
        · rintro ⟨result, hr, edge, he, peer', peerSlot', hresult, htarget, hpeer', hq⟩
          subst hr
          simp at he
          rcases he with rfl | rfl
          · have hpeerEq : peer' = peer := by
              have h := htarget.symm.trans hk
              simp at h
              exact h
            subst hpeerEq
            rw [hpeer] at hpeer'
            cases hpeer'
            exact hq
          · simp [edgeTwo, view] at hresult
    · rcases hpubShape with ⟨hnone, _⟩ | ⟨_, _, _, _, _, _, hone⟩
      · rw [hnone]; simp
      · rw [hone]; simp
    · intro port
      rw [hselErased]
      simp only [edgeOne, edgeTwo, List.mem_cons, List.not_mem_nil, or_false,
        exists_eq_or_imp, exists_eq_left]
      constructor
      · rintro (h | h | h | h | h | h)
        · exact .inl h
        · exact .inr (.inl h)
        · exact .inr (.inr (.inl (.inl h)))
        · exact .inr (.inr (.inr (.inl h)))
        · exact .inr (.inr (.inl (.inr h)))
        · exact .inr (.inr (.inr (.inr h)))
      · rintro (h | h | (h | h) | (h | h))
        · exact .inl h
        · exact .inr (.inl h)
        · exact .inr (.inr (.inl h))
        · exact .inr (.inr (.inr (.inr (.inl h))))
        · exact .inr (.inr (.inr (.inl h)))
        · exact .inr (.inr (.inr (.inr (.inr h))))
  exact ⟨next, PhysicalState.semanticCheck_of_local_rewrite hRule hLower hBoundary hexec
    hRewrite hTouched hcapacity⟩

/-- An actually emitted image is not refused by the consumed-boundary alias. -/
theorem PhysicalState.lowerPlan_no_alias
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    {reuse : Bool} {plan : BytePlan}
    (hLower : state.lowerPlan pair redex reuse = some plan) :
    state.consumedBoundaryAlias pair redex = false := by
  cases h : state.consumedBoundaryAlias pair redex with
  | false => rfl
  | true =>
    rw [PhysicalState.lowerPlan_refuses_alias h] at hLower
    cases hLower

/-- An actually emitted image has generation headroom at both consumed slots. -/
theorem PhysicalState.lowerPlan_slot_headroom
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    {reuse : Bool} {plan : BytePlan} {first second : PhysicalSlot}
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second) :
    first.generation ≠ UInt64.ofNat (UInt64.size - 1) ∧
      second.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
  have hroom := PhysicalState.lowerPlan_requires_headroom hLower
  simp only [PhysicalState.pairHeadroom, hf, hs, Bool.and_eq_true, bne_iff_ne, ne_eq] at hroom
  exact hroom

/-- A captured endpoint never names either consumed agent: a peer endpoint
is a live identity outside the consumed slots and an output label is not live. -/
theorem PhysicalState.captured_endpoint_away
    {state : PhysicalState} (hLegal : state.Legal)
    {ruleSlot valueSlot : Nat} {sourcePort : Nat}
    {target : Nat × Nat × Nat} {endpoint : Port}
    {first second : PhysicalSlot} {a b : Id} {alpha beta : NetSym}
    (hfirst : state.slots[ruleSlot]? = some first)
    (hfirstAgent : first.agent = some (a, alpha))
    (hsecond : state.slots[valueSlot]? = some second)
    (hsecondAgent : second.agent = some (b, beta))
    (hcapture : (∃ peer peerPort peerSlot peerId peerSymbol,
          target = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, sourcePort)) ∧
          endpoint = (peerId, peerPort)) ∨
       (∃ output out, target = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, sourcePort) ∧ endpoint = out.label)) :
    endpoint.1 ≠ a ∧ endpoint.1 ≠ b := by
  have hunique : state.liveIds.Nodup := hLegal.2.2.2.2.2.1
  rcases hcapture with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, _, h1, h2, hpeer,
      hpeerAgent, _, hend⟩ | ⟨output, out, _, houtput, _, hend⟩
  · subst hend
    refine ⟨fun he => h1 ?_, fun he => h2 ?_⟩
    · have he' : peerId = a := he
      subst he'
      exact PhysicalState.slot_index_unique hunique hpeer hpeerAgent hfirst hfirstAgent
    · have he' : peerId = b := he
      subst he'
      exact PhysicalState.slot_index_unique hunique hpeer hpeerAgent hsecond hsecondAgent
  · subst hend
    have hmem : out ∈ state.outputs := List.mem_of_getElem? houtput
    have hdead := hLegal.keysSeparated.labelDead out hmem
    refine ⟨fun he => hdead ?_, fun he => hdead ?_⟩
    · rw [he]
      exact PhysicalState.mem_liveIds.mpr ⟨ruleSlot, first, alpha, hfirst, hfirstAgent⟩
    · rw [he]
      exact PhysicalState.mem_liveIds.mpr ⟨valueSlot, second, beta, hsecond, hsecondAgent⟩

/-- The single result's publication of a one-result image is always defined
when its principal port reads a captured endpoint: quiet for outputs and
nonprincipal peers, one canonical tuple for a live peer's principal port. -/
theorem PhysicalState.publicationFor_defined
    {state : PhysicalState} {pair : PhysicalPair}
    {boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {index ruleSlot valueSlot sourcePort : Nat} {endpoint : Port}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    (hport : result.2.2.2.2[0]? = some (some index))
    (hboundary : boundaries[index]? = some boundary)
    (hcapture : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundary.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, sourcePort)) ∧
          endpoint = (peerId, peerPort)) ∨
       (∃ output out, boundary.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, sourcePort) ∧ endpoint = out.label))
    (hpair : (pair.first = ruleSlot ∧ pair.second = valueSlot) ∨
      (pair.first = valueSlot ∧ pair.second = ruleSlot))
    (hself : ∀ peer, state.slots[peer]? ≠ none → peer ≠ ruleSlot → peer ≠ result.2.1)
    (hgeneration : result.1 = false → ∃ old, state.slots[result.2.1]? = some old ∧
      old.generation ≠ UInt64.ofNat (UInt64.size - 1)) :
    ∃ published, state.publicationFor pair boundaries result = some published := by
  rcases hcapture with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
      hpeerAgent, _, _⟩ | ⟨output, out, hk, _, _, _⟩
  · by_cases hz : peerPort = 0
    · subst hz
      have hf : peer ≠ pair.first := by
        rcases hpair with ⟨hf, _⟩ | ⟨hf, _⟩ <;> rw [hf] <;> assumption
      have hs : peer ≠ pair.second := by
        rcases hpair with ⟨_, hs⟩ | ⟨_, hs⟩ <;> rw [hs] <;> assumption
      have hne := hself peer (by simp [hpeer]) h1
      have hlive : peerSlot.agent.isSome = true := by simp [hpeerAgent]
      cases hr : result.1 with
      | true =>
        exact ⟨_, PhysicalState.publicationFor_fresh_active hport hboundary hk hf hs hne
          hpeer hlive hr⟩
      | false =>
        obtain ⟨old, hold, hroom⟩ := hgeneration hr
        exact ⟨_, PhysicalState.publicationFor_reuse_active hport hboundary hk hf hs hne
          hpeer hlive hr hold hroom⟩
    · exact ⟨[], PhysicalState.publicationFor_quiet hport hboundary (.inr (by simp [hk, hz]))⟩
  · exact ⟨[], PhysicalState.publicationFor_quiet hport hboundary (.inl (by simp [hk]))⟩

/-- The same definedness for a result that may occupy the value slot (DUP's
second result): the captured peer differs from both consumed slots, so it
suffices that no live slot outside them is the result slot. -/
theorem PhysicalState.publicationFor_defined_outside
    {state : PhysicalState} {pair : PhysicalPair}
    {boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {index ruleSlot valueSlot sourcePort : Nat} {endpoint : Port}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    (hport : result.2.2.2.2[0]? = some (some index))
    (hboundary : boundaries[index]? = some boundary)
    (hcapture : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundary.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, sourcePort)) ∧
          endpoint = (peerId, peerPort)) ∨
       (∃ output out, boundary.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, sourcePort) ∧ endpoint = out.label))
    (hpair : (pair.first = ruleSlot ∧ pair.second = valueSlot) ∨
      (pair.first = valueSlot ∧ pair.second = ruleSlot))
    (hself : ∀ peer, state.slots[peer]? ≠ none → peer ≠ ruleSlot → peer ≠ valueSlot →
      peer ≠ result.2.1)
    (hgeneration : result.1 = false → ∃ old, state.slots[result.2.1]? = some old ∧
      old.generation ≠ UInt64.ofNat (UInt64.size - 1)) :
    ∃ published, state.publicationFor pair boundaries result = some published := by
  rcases hcapture with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
      hpeerAgent, _, _⟩ | ⟨output, out, hk, _, _, _⟩
  · by_cases hz : peerPort = 0
    · subst hz
      have hf : peer ≠ pair.first := by
        rcases hpair with ⟨hf, _⟩ | ⟨hf, _⟩ <;> rw [hf] <;> assumption
      have hs : peer ≠ pair.second := by
        rcases hpair with ⟨_, hs⟩ | ⟨_, hs⟩ <;> rw [hs] <;> assumption
      have hne := hself peer (by simp [hpeer]) h1 h2
      have hlive : peerSlot.agent.isSome = true := by simp [hpeerAgent]
      cases hr : result.1 with
      | true =>
        exact ⟨_, PhysicalState.publicationFor_fresh_active hport hboundary hk hf hs hne
          hpeer hlive hr⟩
      | false =>
        obtain ⟨old, hold, hroom⟩ := hgeneration hr
        exact ⟨_, PhysicalState.publicationFor_reuse_active hport hboundary hk hf hs hne
          hpeer hlive hr hold hroom⟩
    · exact ⟨[], PhysicalState.publicationFor_quiet hport hboundary (.inr (by simp [hk, hz]))⟩
  · exact ⟨[], PhysicalState.publicationFor_quiet hport hboundary (.inl (by simp [hk]))⟩

end NobleFoundation.NetI64Plans
