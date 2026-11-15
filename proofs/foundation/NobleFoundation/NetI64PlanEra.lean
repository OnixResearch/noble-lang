import NobleFoundation.NetI64PlanInstance

/-!
# Emitted ERA admission in both orientations (issue #110)

From a legal arena, a current selected pair, a matching projection and its
redex alone, an actually lowered ERA image that is not excluded by queue
capacity executes and passes the independent semantic checker at its exact
reservations. The erasure image installs no results, captures no boundary,
writes no endpoint, publishes nothing and retires both consumed slots in
either storage policy; the candidate shape and the abstract image are derived
here, never assumed. This is a statement about the Lean model only.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- **Emitted ERA admission**, for either orientation and storage policy: an
actually lowered erasure image (no results, boundaries, writes or
publications, both consumed slots retired) of a legal, current, matched arena
that is not excluded by queue capacity executes and passes the independent
semantic checker at its exact reservations. -/
theorem PhysicalState.era_accepted
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    {ruleSlot valueSlot : Nat} {ruleId valueId : Id} {value : BitVec 64}
    {ruleOld valueOld : PhysicalSlot}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (redex.1, redex.2.1))
    (horient : (pair.first = ruleSlot ∧ pair.second = valueSlot) ∨
      (pair.first = valueSlot ∧ pair.second = ruleSlot))
    (hids : (redex.1 = ruleId ∧ redex.2.1 = valueId) ∨
      (redex.1 = valueId ∧ redex.2.1 = ruleId))
    (hrule : state.slots[ruleSlot]? = some ruleOld)
    (hruleAgent : ruleOld.agent = some (ruleId, .era))
    (hrulePort : ruleOld.ports[0]? = some (some (valueSlot, 0)))
    (hruleRoom : ruleOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hvalue : state.slots[valueSlot]? = some valueOld)
    (hvalueAgent : valueOld.agent = some (valueId, .num value))
    (hvaluePort : valueOld.ports[0]? = some (some (ruleSlot, 0)))
    (hvalueRoom : valueOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hdistinct : ruleSlot ≠ valueSlot)
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hresults : plan.results = [])
    (hboundaries : plan.boundaries = [])
    (hwrites : plan.externalWrites = [])
    (hpublications : plan.publications = [])
    (haction : plan.action = (0, 0))
    (hpairPlan : plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat))
    (hselAgents : (Plan.compile state.project redex hRule).initializedAgents = [])
    (hselWrites : (Plan.compile state.project redex hRule).wireWrites = [])
    (hselCaptured : (Plan.compile state.project redex hRule).capturedBoundary =
      [((redex.1, 0), (redex.2.1, 0)), ((redex.2.1, 0), (redex.1, 0))])
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit redex hRule plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  have hdecode := PhysicalState.lowerPlan_decodes hLower
  have hruleLive : ruleOld.agent.isSome = true := by simp [hruleAgent]
  have hvalueLive : valueOld.agent.isSome = true := by simp [hvalueAgent]
  -- the decoded pair is the selected current pair
  have hpairEq : PhysicalPair.mk plan.pair.1 (UInt64.ofNat plan.pair.2.1)
      plan.pair.2.2.1 (UInt64.ofNat plan.pair.2.2.2) = pair := by
    rw [hpairPlan]
    cases pair
    simp [UInt64.ofNat_toNat]
  have hcurrentPlan := hpairEq ▸ hcurrent
  have hfirstPlan : plan.pair.1 = pair.first := by rw [hpairPlan]
  have hsecondPlan : plan.pair.2.2.1 = pair.second := by rw [hpairPlan]
  have hnoPubs : plan.publishedPairs = [] := by
    simp [BytePlan.publishedPairs, hpublications]
  -- installing no results leaves the arena unchanged
  have hinstall : PhysicalState.installResults plan redex.1 redex.2.1 redex.2.2.2.2 0
      state plan.results = some state := by
    rw [hresults]
    rfl
  have hpost : state.InstallPost state [] 0 :=
    { size := rfl
      outputs := rfl
      work := rfl
      free := rfl
      frame := fun _ _ _ => rfl
      result := by simp }
  obtain ⟨next, hexec, hobs⟩ := PhysicalState.execute_observations
    (redex := redex) (ruleSlot := ruleSlot) (valueSlot := valueSlot)
    (results := []) (edges := []) (fresh := 0)
    hdecode hcurrentPlan (by rw [hfirstPlan, hsecondPlan]; exact horient) hdistinct
    hinstall hpost
    (by simp)
    (by rw [hwrites]; simp)
    (by rw [hwrites]; simp)
    (by simp)
    (by rw [hwrites]; simp)
    (by rw [haction]; simp)
    (by rw [haction]; simp)
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
    (by simp)
    (by rw [hnoPubs]; simp)
    (by rw [hnoPubs]; simp)
    (.inl hnoPubs) hLegal.2.2.2.2.1
  -- the abstract boundary filtered to auxiliary ports is empty, like the physical one
  have hBoundary : plan.logicalBoundary state = some
      ((Plan.compile state.project redex hRule).capturedBoundary.filter
        fun edge => edge.1.2 != 0) := by
    rw [hselCaptured, BytePlan.logicalBoundary_eq_mapM, hboundaries]
    rfl
  -- slots outside the plan's touched set are outside the consumed pair
  have hTouched : ∀ index, index < state.slots.size →
      plan.touchedSlots.contains index = false →
      index ≠ ruleSlot ∧ index ≠ valueSlot ∧
      (∀ edge ∈ ([] : List EdgeView), ∀ port, edge.target ≠ (0, index, port)) ∧
      (∀ queued ∈ plan.publishedPairs, queued.first ≠ index ∧ queued.second ≠ index) := by
    intro index _ hnot
    have hmem : index ∉ plan.touchedSlots := by
      intro h
      have := List.contains_iff_mem.mpr h
      rw [hnot] at this
      exact Bool.false_ne_true this
    simp only [BytePlan.touchedSlots, hboundaries, hpublications, List.filterMap_nil,
      List.flatMap_nil, List.append_nil, List.mem_cons, List.not_mem_nil, or_false,
      not_or] at hmem
    obtain ⟨hfirst, hsecond⟩ := hmem
    rw [hfirstPlan] at hfirst
    rw [hsecondPlan] at hsecond
    refine ⟨?_, ?_, by simp, by rw [hnoPubs]; simp⟩
    · rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> omega
    · rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> omega
  -- the retired slots are exactly the consumed pair
  have hretired : plan.retiredSlots = [pair.second, pair.first] := by
    simp [BytePlan.retiredSlots, haction, hfirstPlan, hsecondPlan]
  have hpairDistinct : pair.second ≠ pair.first := by
    rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> rw [hf, hs]
    · exact Ne.symm hdistinct
    · exact hdistinct
  -- the local-rewrite bundle
  have hRewrite : state.LocalRewrite next (Plan.compile state.project redex hRule)
      ruleSlot valueSlot ruleId valueId .era (.num value)
      [] [] plan.retiredSlots 0 plan.publishedPairs := by
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
        edgeCapture := by simp
        resultSlots := by simp
        resultIds := by simp
        resultFresh := by simp
        resultPlace := by simp
        resultGeneration := by simp
        freshFilled := fun _ hlow hhigh => absurd hhigh (by omega)
        resultArity := by simp
        resultPorts := by simp
        edgeResult := by simp
        edgeResults := by simp
        retiredExact := ?_
        retiredNodup := ?_
        observed := hobs
        publishedExact := ?_
        publishedNodup := by rw [hnoPubs]; simp
        selectedPair := hids
        selectedAgents := by rw [hselAgents]; rfl
        selectedWrites := by rw [hselWrites]; rfl
        selectedErased := ?_
        selectedActive := rfl }
    · intro index
      rw [hretired]
      simp only [List.mem_cons, List.not_mem_nil, or_false, false_implies, implies_true,
        and_true]
      rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩
      · rw [hf, hs]
        exact Or.comm
      · rw [hf, hs]
    · rw [hretired]
      simp [hpairDistinct]
    · intro queued
      rw [hnoPubs]
      simp
    · intro port
      rw [hselCaptured]
      simp only [List.map_cons, List.map_nil, List.mem_cons, List.not_mem_nil, or_false,
        false_and, exists_false]
      constructor
      · rintro ((h | h) | (h | h)) <;> subst h <;>
          rcases hids with ⟨h1, h2⟩ | ⟨h1, h2⟩ <;> simp [h1, h2]
      · rintro (h | h) <;> subst h <;>
          rcases hids with ⟨h1, h2⟩ | ⟨h1, h2⟩ <;> simp [h1, h2]
  exact ⟨next, PhysicalState.semanticCheck_of_local_rewrite hRule hLower hBoundary hexec
    hRewrite hTouched hcapacity⟩

/-- **Forward ERA emitted admission.** -/
theorem PhysicalState.era_forward_accepted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b .era (.num value)
      eraNumData.toX.toRhs)
    (hLower : state.lowerPlan pair (a, b, .era, .num value, eraNumData.toX) reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .era, .num value, eraNumData.toX) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  obtain ⟨candidate, hc, hresults, hboundaries, hwrites, hpublications, haction,
    hpairPlan⟩ := PhysicalState.era_forward_candidate_exists (reuse := reuse) (value := value)
      hcurrent hf hs (by simp [hfa]) (by simp [hsa]) hfp hsp
  have heq : candidate = plan :=
    Option.some.inj (hc.symm.trans (PhysicalState.lowerPlan_has_candidate hLower))
  subst heq
  obtain ⟨hselCaptured, hselAgents, hselWrites⟩ :=
    Plan.era_forward_fields state.project (value := value) hw0 hwb
  exact PhysicalState.era_accepted (by rfl) hLegal hm hcurrent (.inl ⟨rfl, rfl⟩)
    (.inl ⟨rfl, rfl⟩) hf hfa hfp hroomF hs hsa hsp hroomS hdistinct hLower hresults
    hboundaries hwrites hpublications haction hpairPlan hselAgents hselWrites hselCaptured
    hcapacity

/-- **Reverse ERA emitted admission.** -/
theorem PhysicalState.era_reverse_accepted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num value) .era
      eraNumData.toX.swap.toRhs)
    (hLower : state.lowerPlan pair (a, b, .num value, .era, eraNumData.toX.swap) reuse =
      some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .num value, .era, eraNumData.toX.swap) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  obtain ⟨candidate, hc, hresults, hboundaries, hwrites, hpublications, haction,
    hpairPlan⟩ := PhysicalState.era_reverse_candidate_exists (reuse := reuse) (value := value)
      hcurrent hf hs (by simp [hfa]) (by simp [hsa]) hfp hsp
  have heq : candidate = plan :=
    Option.some.inj (hc.symm.trans (PhysicalState.lowerPlan_has_candidate hLower))
  subst heq
  obtain ⟨hselCaptured, hselAgents, hselWrites⟩ :=
    Plan.era_reverse_fields state.project (value := value) hw0 hwb
  exact PhysicalState.era_accepted (by rfl) hLegal hm hcurrent (.inr ⟨rfl, rfl⟩)
    (.inr ⟨rfl, rfl⟩) hs hsa hsp hroomS hf hfa hfp hroomF (Ne.symm hdistinct) hLower
    hresults hboundaries hwrites hpublications haction hpairPlan hselAgents hselWrites
    hselCaptured hcapacity

end NobleFoundation.NetI64Plans
