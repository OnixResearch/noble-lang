import NobleFoundation.NetI64PlanInstance

/-!
# Emitted OP1 admission in both orientations (issue #110)

This is a statement about the Lean model emitter, executor and semantic
checker only; it makes no claim about the Rust implementation.

`PhysicalState.op1_accepted` builds the `PhysicalState.LocalRewrite` bundle of
an actually lowered OP1 image (one number result reading the single captured
auxiliary endpoint of the consumed `op1` agent, one reciprocal write) for
either orientation and storage policy, derives its execution from the generic
phase composition, and concludes that the image passes the independent
semantic checker at its exact reservations.

The two wrappers instantiate it for the forward (`op1 ⋈ num`) and reverse
(`num ⋈ op1`) orientations from a legal arena, a current selected pair, a
matching projection and its redex alone: the capture, the publication, the
candidate shape and the abstract image are all derived, never assumed.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- **Emitted OP1 admission**, for either orientation and storage policy: an
actually lowered OP1 image of a legal, current, matched arena that is not
excluded by queue capacity executes and passes the independent semantic
checker at its exact reservations. -/
theorem PhysicalState.op1_accepted
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    {ruleSlot valueSlot : Nat} {ruleId valueId : Id} {op : Op} {first second : BitVec 64}
    {ruleOld valueOld : PhysicalSlot} {rhsSlot : Option Bool × Nat}
    {boundaryOne : (Nat × Nat) × (Nat × Nat × Nat)} {p1 : Port}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (redex.1, redex.2.1))
    (horient : (pair.first = ruleSlot ∧ pair.second = valueSlot) ∨
      (pair.first = valueSlot ∧ pair.second = ruleSlot))
    (hids : (redex.1 = ruleId ∧ redex.2.1 = valueId) ∨
      (redex.1 = valueId ∧ redex.2.1 = ruleId))
    (hrule : state.slots[ruleSlot]? = some ruleOld)
    (hruleAgent : ruleOld.agent = some (ruleId, .op1 op first))
    (hrulePort : ruleOld.ports[0]? = some (some (valueSlot, 0)))
    (hruleRoom : ruleOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hvalue : state.slots[valueSlot]? = some valueOld)
    (hvalueAgent : valueOld.agent = some (valueId, .num second))
    (hvaluePort : valueOld.ports[0]? = some (some (ruleSlot, 0)))
    (hvalueRoom : valueOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hdistinct : ruleSlot ≠ valueSlot)
    (hrhsSlot : redex.2.2.2.2.slots[0]? = some rhsSlot)
    (hrhsLab : redex.2.2.2.2.lab rhsSlot.1 rhsSlot.2 = some (.num (op.apply first second)))
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
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hresults : plan.results = [(!reuse, if reuse then ruleSlot else state.slots.size,
      .num (op.apply first second), ruleSlot, [some 0, none, none])])
    (hboundaries : plan.boundaries = [boundaryOne])
    (hwrites : plan.externalWrites = [(0, 0, 0)])
    (hpublications : state.publicationFor pair [boundaryOne]
      (!reuse, if reuse then ruleSlot else state.slots.size, .num (op.apply first second),
        ruleSlot, [some 0, none, none]) = some plan.publications)
    (haction : plan.action = (if reuse ∧ ruleSlot = pair.first then 1 else 0,
      if reuse ∧ ruleSlot = pair.second then 1 else 0))
    (hpairPlan : plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat))
    (hselAgents : (Plan.compile state.project redex hRule).initializedAgents =
      [(0 :: ruleId, .num (op.apply first second))])
    (hselWrites : (Plan.compile state.project redex hRule).wireWrites =
      [((0 :: ruleId, 0), p1), (p1, (0 :: ruleId, 0))])
    (hselErased : ∀ port,
      (port ∈ (Plan.compile state.project redex hRule).capturedBoundary.map Prod.fst ∨
        port ∈ (Plan.compile state.project redex hRule).capturedBoundary.map Prod.snd) ↔
      (port = (ruleId, 0) ∨ port = (valueId, 0) ∨ port = (ruleId, 1) ∨ port = p1))
    (hselBoundary : (Plan.compile state.project redex hRule).capturedBoundary.filter
      (fun edge => edge.1.2 != 0) = [((ruleId, 1), p1)])
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
  have hports : ([some 0, none, none] : List (Option Nat)).mapM plan.resultTarget =
      some [PhysicalState.boundaryReference boundaryOne, none, none] := by
    have h0 := BytePlan.resultTarget_boundary (index := 0)
      (by rw [hboundaries]; rfl) hkind1
    have hn : plan.resultTarget none = some none := rfl
    simp [List.mapM_cons, h0, hn]
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
  have hpeerNotSlot : ∀ peer, peer < state.slots.size → peer ≠ ruleSlot →
      peer ≠ (if reuse then ruleSlot else state.slots.size) := by
    intro peer hlt hne
    cases reuse <;> simp <;> omega
  -- the admissible reciprocal write
  have hresult0 : plan.results[0]? = some (!reuse, if reuse then ruleSlot else state.slots.size,
      .num (op.apply first second), ruleSlot, [some 0, none, none]) := by
    rw [hresults]; rfl
  have hready1 : state.WriteReady plan (0, 0, 0) := by
    refine ⟨_, boundaryOne, hresult0, by rw [hboundaries]; rfl, rfl, ?_⟩
    rcases hcapture1 with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
        hpeerAgent, hpeerPort, _⟩ | ⟨output, out, hk, houtput, htarget, _⟩
    · obtain ⟨hf, hs⟩ := hpeerAway peer h1 h2
      exact .inl ⟨peer, peerPort, peerSlot, hk, hfirstPlan ▸ hf, hsecondPlan ▸ hs, hpeer,
        by simp [hpeerAgent], by rw [hsource1]; exact hpeerPort⟩
    · exact .inr ⟨output, out, hk, houtput, by rw [hsource1]; exact htarget⟩
  have htarget1 : plan.writeTarget (0, 0, 0) = some boundaryOne.2 := by
    simp [BytePlan.writeTarget, hboundaries]
  have hsource1' : plan.writeSource (0, 0, 0) =
      some (if reuse then ruleSlot else state.slots.size, 0) := by
    simp [BytePlan.writeSource, hresult0]
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
            (boundaries := [boundaryOne]) (index := 0) (pair := pair)
            (result := (!true, if true then ruleSlot else state.slots.size,
              .num (op.apply first second), ruleSlot, [some 0, none, none]))
            rfl rfl hk hf hs hself hpeer hpeerLive rfl hrule hruleRoom
          rw [hp] at hpublications
          have hpubs := (Option.some.inj hpublications).symm
          simp only [BytePlan.publishedPairs, hpubs]
          by_cases hlt : ruleSlot < peer
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_add, UInt64.ofNat_toNat]
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_add, UInt64.ofNat_toNat]
        | false =>
          have hp := PhysicalState.publicationFor_fresh_active
            (boundaries := [boundaryOne]) (index := 0) (pair := pair)
            (result := (!false, if false then ruleSlot else state.slots.size,
              .num (op.apply first second), ruleSlot, [some 0, none, none]))
            rfl rfl hk hf hs hself hpeer hpeerLive rfl
          rw [hp] at hpublications
          have hpubs := (Option.some.inj hpublications).symm
          simp only [BytePlan.publishedPairs, hpubs]
          by_cases hlt : state.slots.size < peer
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_toNat]
          · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_toNat]
      · left
        have hp := PhysicalState.publicationFor_quiet (state := state)
          (boundaries := [boundaryOne]) (index := 0) (pair := pair)
          (result := (!reuse, if reuse then ruleSlot else state.slots.size,
            .num (op.apply first second), ruleSlot, [some 0, none, none])) rfl rfl
          (.inr (by rw [hk]; exact hz))
        rw [hp] at hpublications
        refine ⟨by simp [BytePlan.publishedPairs, ← (Option.some.inj hpublications)], ?_⟩
        intro peer' h
        rw [hk] at h
        simp at h
        exact hz h.2
    · left
      have hp := PhysicalState.publicationFor_quiet (state := state)
        (boundaries := [boundaryOne]) (index := 0) (pair := pair)
        (result := (!reuse, if reuse then ruleSlot else state.slots.size,
          .num (op.apply first second), ruleSlot, [some 0, none, none])) rfl rfl
        (.inl (by simp [hk]))
      rw [hp] at hpublications
      refine ⟨by simp [BytePlan.publishedPairs, ← (Option.some.inj hpublications)], ?_⟩
      intro peer' h
      rw [hk] at h
      simp at h
  -- the result and edge views
  let slotR : Nat := if reuse then ruleSlot else state.slots.size
  let view : ResultView :=
    { slot := slotR, id := 0 :: ruleId, symbol := .num (op.apply first second),
      generation := if reuse then ruleOld.generation + 1 else 0,
      ports := [PhysicalState.boundaryReference boundaryOne, none, none] }
  let edgeOne : EdgeView :=
    { sourcePort := 1, target := boundaryOne.2, endpoint := p1,
      result := (slotR, 0), resultId := 0 :: ruleId }
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
    (results := [view]) (edges := [edgeOne])
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
      subst hw
      exact hready1)
    (by
      rw [hwrites]
      simp)
    (by
      intro edge he
      simp at he
      subst he
      exact ⟨(0, 0, 0), by rw [hwrites]; simp, htarget1, hsource1'⟩)
    (by
      rw [hwrites]
      intro write hw
      simp at hw
      subst hw
      exact ⟨edgeOne, by simp, htarget1, hsource1'⟩)
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
      subst he
      simp [edgeOne] at hresult htarget
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
    rw [BytePlan.logicalBoundary_eq_mapM, hboundaries]
    simp [List.mapM_cons, h1]
  -- slots outside the plan's touched set are outside every rewritten location
  have hTouched : ∀ index, index < state.slots.size →
      plan.touchedSlots.contains index = false →
      index ≠ ruleSlot ∧ index ≠ valueSlot ∧
      (∀ edge ∈ [edgeOne], ∀ port, edge.target ≠ (0, index, port)) ∧
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
      subst he
      exact hpeers boundaryOne rfl (by
        have htarget' : boundaryOne.2 = (0, index, port) := htarget
        rw [htarget']; simp)
    · intro queued hq
      simp only [BytePlan.publishedPairs, List.mem_map] at hq
      obtain ⟨⟨f, fg, s, sg⟩, htuple, rfl⟩ := hq
      have hnotIn := hpubs (f, fg, s, sg) htuple
      exact ⟨fun h => hnotIn.1 h.symm, fun h => hnotIn.2 h.symm⟩
  -- the local-rewrite bundle
  have hRewrite : state.LocalRewrite next (Plan.compile state.project redex hRule)
      ruleSlot valueSlot ruleId valueId (.op1 op first) (.num second)
      [view] [edgeOne] plan.retiredSlots (if reuse then 0 else 1)
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
        edgeTargets := by simp
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
        edgeResults := by simp
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
      subst he
      exact hcapture1
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
        right
        simp [view] at hport
        refine ⟨by simp [edgeOne], by simp [view, NetI64.arity], hport.symm⟩
      | 2, hport =>
        right
        simp [view] at hport
        refine ⟨by simp [edgeOne], by simp [view, NetI64.arity], hport.symm⟩
      | n + 3, hport =>
        simp [view] at hport
    · intro edge he
      simp at he
      subst he
      exact ⟨view, by simp, rfl, rfl, by simp [view, edgeOne]⟩
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
        subst he
        exact hquiet peer htarget
      · rw [hone]
        simp only [List.mem_singleton]
        constructor
        · intro hq
          exact ⟨view, by simp, edgeOne, by simp, peer, peerSlot, rfl, hk, hpeer, hq⟩
        · rintro ⟨result, hr, edge, he, peer', peerSlot', hresult, htarget, hpeer', hq⟩
          subst hr
          subst he
          have hpeerEq : peer' = peer := by
            have h := htarget.symm.trans hk
            simp at h
            exact h
          subst hpeerEq
          rw [hpeer] at hpeer'
          cases hpeer'
          exact hq
    · rcases hpubShape with ⟨hnone, _⟩ | ⟨_, _, _, _, _, _, hone⟩
      · rw [hnone]; simp
      · rw [hone]; simp
    · intro port
      rw [hselErased]
      simp only [edgeOne, List.mem_cons, List.not_mem_nil, or_false,
        exists_eq_left]
  exact ⟨next, PhysicalState.semanticCheck_of_local_rewrite hRule hLower hBoundary hexec
    hRewrite hTouched hcapacity⟩

/-- **Forward OP1 emitted admission.** From a legal arena, a current selected
`op1 ⋈ num` pair, a matching projection and its redex alone, an actually
lowered image that is not excluded by queue capacity passes the semantic
checker at its exact reservations. -/
theorem PhysicalState.op1_forward_accepted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {first second : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.op1 op first) (.num second)
      (op1NumData op first second).toX.toRhs)
    (hLower : state.lowerPlan pair (a, b, .op1 op first, .num second,
      (op1NumData op first second).toX) reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .op1 op first, .num second, (op1NumData op first second).toX) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨firstOld, secondOld, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  have halias := PhysicalState.lowerPlan_no_alias hLower
  simp [PhysicalState.consumedBoundaryAlias, hf, NetI64.arity, List.range_succ] at halias
  have hnoAlias1 : ∀ peer peerPort, firstOld.ports[1]? = some (some (peer, peerPort)) →
      peer ≠ pair.first ∧ peer ≠ pair.second := by
    intro peer peerPort hcontent
    have h := halias
    simp [hcontent] at h
    exact h
  obtain ⟨boundaryOne, p1, hcap1, hsrc1, hcapture1, hw1, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 1) (by simp [NetI64.arity]) hnoAlias1
  obtain ⟨published, hpub⟩ := PhysicalState.publicationFor_defined (pair := pair)
    (boundaries := [boundaryOne])
    (result := (!reuse, if reuse then pair.first else state.slots.size,
      .num (op.apply first second), pair.first, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.first) (valueSlot := pair.second)
    rfl rfl hcapture1 (.inl ⟨rfl, rfl⟩)
    (by
      intro peer hpeer hne
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨firstOld, by simpa using hf, hroomF⟩
      | false => simp at hr)
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨hresults, hboundaries, hwrites, hpublications, haction, hpairPlan⟩ :=
    PhysicalState.op1_forward_candidate_shape hcurrent hf hs (by simp [hfa])
      (by simp [hsa]) hfp hsp hcap1 hpub hc
  have haway1 := PhysicalState.captured_endpoint_away hLegal hf hfa hs hsa hcapture1
  obtain ⟨hselBoundary, hselAgents, hselWrites⟩ :=
    Plan.op1_forward_fields state.project (op := op) (first := first) (second := second)
      hw0 hwb hw1 haway1
  refine PhysicalState.op1_accepted (by rfl) hLegal hm hcurrent (.inl ⟨rfl, rfl⟩)
    (.inl ⟨rfl, rfl⟩) hf hfa hfp hroomF hs hsa hsp hroomS hdistinct
    (rhsSlot := (some false, 0)) rfl
    (by simp [op1NumData, RuleData.toX, RuleData.ofWires, assoc]) rfl
    hsrc1 hcapture1 hLower hresults hboundaries hwrites
    (by rw [hpublications]; exact hpub)
    (by rw [haction]; simp [hdistinct])
    hpairPlan hselAgents hselWrites ?_ ?_ hcapacity
  · intro port
    rw [hselBoundary]
    simp only [List.map_cons, List.map_nil, List.mem_cons, List.not_mem_nil, or_false]
    constructor
    · rintro ((h | h | h) | (h | h | h))
      · exact .inl h
      · exact .inr (.inr (.inl h))
      · exact .inr (.inl h)
      · exact .inr (.inl h)
      · exact .inr (.inr (.inr h))
      · exact .inl h
    · rintro (h | h | h | h)
      · exact .inl (.inl h)
      · exact .inl (.inr (.inr h))
      · exact .inl (.inr (.inl h))
      · exact .inr (.inr (.inl h))
  · rw [hselBoundary]
    simp [List.filter_cons]

/-- **Reverse OP1 emitted admission.** The mirrored `num ⋈ op1` orientation:
the `op1` agent is the second selected identity and its physical slot donates
the single result. -/
theorem PhysicalState.op1_reverse_accepted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {first second : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num second) (.op1 op first)
      (op1NumData op first second).toX.swap.toRhs)
    (hLower : state.lowerPlan pair (a, b, .num second, .op1 op first,
      (op1NumData op first second).toX.swap) reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .num second, .op1 op first, (op1NumData op first second).toX.swap) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨firstOld, secondOld, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  have halias := PhysicalState.lowerPlan_no_alias hLower
  simp [PhysicalState.consumedBoundaryAlias, hs, NetI64.arity, List.range_succ] at halias
  have hnoAlias1 : ∀ peer peerPort, secondOld.ports[1]? = some (some (peer, peerPort)) →
      peer ≠ pair.second ∧ peer ≠ pair.first := by
    intro peer peerPort hcontent
    have h := halias
    simp [hcontent] at h
    exact ⟨h.2, h.1⟩
  obtain ⟨boundaryOne, p1, hcap1, hsrc1, hcapture1, hw1, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 1) (by simp [NetI64.arity]) hnoAlias1
  obtain ⟨published, hpub⟩ := PhysicalState.publicationFor_defined (pair := pair)
    (boundaries := [boundaryOne])
    (result := (!reuse, if reuse then pair.second else state.slots.size,
      .num (op.apply first second), pair.second, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.second) (valueSlot := pair.first)
    rfl rfl hcapture1 (.inr ⟨rfl, rfl⟩)
    (by
      intro peer hpeer hne
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨secondOld, by simpa using hs, hroomS⟩
      | false => simp at hr)
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨hresults, hboundaries, hwrites, hpublications, haction, hpairPlan⟩ :=
    PhysicalState.op1_reverse_candidate_shape hcurrent hf hs (by simp [hfa])
      (by simp [hsa]) hfp hsp hcap1 hpub hc
  have haway1 := PhysicalState.captured_endpoint_away hLegal hs hsa hf hfa hcapture1
  obtain ⟨hselBoundary, hselAgents, hselWrites⟩ :=
    Plan.op1_reverse_fields state.project (op := op) (first := first) (second := second)
      hw0 hwb hw1 ⟨haway1.2, haway1.1⟩
  refine PhysicalState.op1_accepted (by rfl) hLegal hm hcurrent (.inr ⟨rfl, rfl⟩)
    (.inr ⟨rfl, rfl⟩) hs hsa hsp hroomS hf hfa hfp hroomF (Ne.symm hdistinct)
    (rhsSlot := (some true, 0)) rfl
    (by simp [op1NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]) rfl
    hsrc1 hcapture1 hLower hresults hboundaries hwrites
    (by rw [hpublications]; exact hpub)
    (by rw [haction]; simp [Ne.symm hdistinct])
    hpairPlan hselAgents hselWrites ?_ ?_ hcapacity
  · intro port
    rw [hselBoundary]
    simp only [List.map_cons, List.map_nil, List.mem_cons, List.not_mem_nil, or_false]
    constructor
    · rintro ((h | h | h) | (h | h | h))
      · exact .inr (.inl h)
      · exact .inl h
      · exact .inr (.inr (.inl h))
      · exact .inl h
      · exact .inr (.inl h)
      · exact .inr (.inr (.inr h))
    · rintro (h | h | h | h)
      · exact .inl (.inr (.inl h))
      · exact .inl (.inl h)
      · exact .inl (.inr (.inr h))
      · exact .inr (.inr (.inr h))
  · rw [hselBoundary]
    simp [List.filter_cons]

end NobleFoundation.NetI64Plans
