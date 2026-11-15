import NobleFoundation.NetI64PlanInstance

/-!
# Emitted OP2 admission in both orientations (issue #110)

From a legal arena, a current selected pair, a matching projection and its
redex alone, an actually lowered OP2 image that is not excluded by queue
capacity executes and passes the independent semantic checker at its exact
reservations. Captures, publications, the candidate shape and the abstract
image are all derived here, never assumed.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- **Forward OP2 emitted admission.** -/
theorem PhysicalState.op2_forward_admitted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {value : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.op2 op) (.num value)
      (op2NumData op value).toX.toRhs)
    (hLower : state.lowerPlan pair (a, b, .op2 op, .num value,
      (op2NumData op value).toX) reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .op2 op, .num value, (op2NumData op value).toX) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  have halias := PhysicalState.lowerPlan_no_alias hLower
  simp [PhysicalState.consumedBoundaryAlias, hf, NetI64.arity, List.range_succ] at halias
  have hnoAlias1 : ∀ peer peerPort, first.ports[1]? = some (some (peer, peerPort)) →
      peer ≠ pair.first ∧ peer ≠ pair.second := by
    intro peer peerPort hcontent
    have h := halias.1
    simp [hcontent] at h
    exact h
  have hnoAlias2 : ∀ peer peerPort, first.ports[2]? = some (some (peer, peerPort)) →
      peer ≠ pair.first ∧ peer ≠ pair.second := by
    intro peer peerPort hcontent
    have h := halias.2
    simp [hcontent] at h
    exact h
  obtain ⟨boundaryOne, p1, hcap1, hsrc1, hcapture1, hw1, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 1) (by simp [NetI64.arity]) hnoAlias1
  obtain ⟨boundaryTwo, p2, hcap2, hsrc2, hcapture2, hw2, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 2) (by simp [NetI64.arity]) hnoAlias2
  have hfirstBound : pair.first < state.slots.size := (getElem?_eq_some_iff.mp hf).1
  obtain ⟨published, hpub⟩ := PhysicalState.publicationFor_defined (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.first else state.slots.size, .op1 op value,
      pair.first, [some 0, some 1, none]))
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
      | true => exact ⟨first, by simpa using hf, hroomF⟩
      | false => simp at hr)
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨hresults, hboundaries, hwrites, hpublications, haction, hpairPlan⟩ :=
    PhysicalState.op2_forward_candidate_shape hcurrent hf hs (by simp [hfa])
      (by simp [hsa]) hfp hsp hcap1 hcap2 hpub hc
  have haway1 := PhysicalState.captured_endpoint_away hLegal hf hfa hs hsa hcapture1
  have haway2 := PhysicalState.captured_endpoint_away hLegal hf hfa hs hsa hcapture2
  obtain ⟨hselBoundary, hselAgents, hselWrites⟩ :=
    Plan.op2_forward_fields state.project (op := op) (value := value) hw0 hwb hw1 hw2
      haway1 haway2
  refine PhysicalState.op2_admitted (by rfl) hLegal hm hcurrent (.inl ⟨rfl, rfl⟩)
    (.inl ⟨rfl, rfl⟩) hf hfa hfp hroomF hs hsa hsp hroomS hdistinct
    (rhsSlot := (some false, 0)) rfl
    (by simp [op2NumData, RuleData.toX, RuleData.ofWires, assoc]) rfl
    hsrc1 hcapture1 hsrc2 hcapture2 hLower hresults hboundaries hwrites
    (by rw [hpublications]; exact hpub)
    (by rw [haction]; simp [hdistinct])
    hpairPlan hselAgents hselWrites ?_ ?_ hcapacity
  · intro port
    rw [hselBoundary]
    simp only [List.map_cons, List.map_nil, List.mem_cons, List.not_mem_nil, or_false]
    constructor
    · rintro ((h | h | h | h) | (h | h | h | h))
      · exact .inl h
      · exact .inr (.inr (.inl h))
      · exact .inr (.inr (.inr (.inl h)))
      · exact .inr (.inl h)
      · exact .inr (.inl h)
      · exact .inr (.inr (.inr (.inr (.inl h))))
      · exact .inr (.inr (.inr (.inr (.inr h))))
      · exact .inl h
    · rintro (h | h | h | h | h | h)
      · exact .inl (.inl h)
      · exact .inl (.inr (.inr (.inr h)))
      · exact .inl (.inr (.inl h))
      · exact .inl (.inr (.inr (.inl h)))
      · exact .inr (.inr (.inl h))
      · exact .inr (.inr (.inr (.inl h)))
  · rw [hselBoundary]
    simp [List.filter_cons]

/-- **Reverse OP2 emitted admission.** -/
theorem PhysicalState.op2_reverse_admitted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {value : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num value) (.op2 op)
      (op2NumData op value).toX.swap.toRhs)
    (hLower : state.lowerPlan pair (a, b, .num value, .op2 op,
      (op2NumData op value).toX.swap) reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .num value, .op2 op, (op2NumData op value).toX.swap) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  have halias := PhysicalState.lowerPlan_no_alias hLower
  simp [PhysicalState.consumedBoundaryAlias, hs, NetI64.arity, List.range_succ] at halias
  have hnoAlias1 : ∀ peer peerPort, second.ports[1]? = some (some (peer, peerPort)) →
      peer ≠ pair.second ∧ peer ≠ pair.first := by
    intro peer peerPort hcontent
    have h := halias.1
    simp [hcontent] at h
    exact ⟨h.2, h.1⟩
  have hnoAlias2 : ∀ peer peerPort, second.ports[2]? = some (some (peer, peerPort)) →
      peer ≠ pair.second ∧ peer ≠ pair.first := by
    intro peer peerPort hcontent
    have h := halias.2
    simp [hcontent] at h
    exact ⟨h.2, h.1⟩
  obtain ⟨boundaryOne, p1, hcap1, hsrc1, hcapture1, hw1, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 1) (by simp [NetI64.arity]) hnoAlias1
  obtain ⟨boundaryTwo, p2, hcap2, hsrc2, hcapture2, hw2, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 2) (by simp [NetI64.arity]) hnoAlias2
  have hsecondBound : pair.second < state.slots.size := (getElem?_eq_some_iff.mp hs).1
  obtain ⟨published, hpub⟩ := PhysicalState.publicationFor_defined (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.second else state.slots.size, .op1 op value,
      pair.second, [some 0, some 1, none]))
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
      | true => exact ⟨second, by simpa using hs, hroomS⟩
      | false => simp at hr)
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨hresults, hboundaries, hwrites, hpublications, haction, hpairPlan⟩ :=
    PhysicalState.op2_reverse_candidate_shape hcurrent hf hs (by simp [hfa])
      (by simp [hsa]) hfp hsp hcap1 hcap2 hpub hc
  have haway1 := PhysicalState.captured_endpoint_away hLegal hs hsa hf hfa hcapture1
  have haway2 := PhysicalState.captured_endpoint_away hLegal hs hsa hf hfa hcapture2
  obtain ⟨hselBoundary, hselAgents, hselWrites⟩ :=
    Plan.op2_reverse_fields state.project (op := op) (value := value) hw0 hwb hw1 hw2
      ⟨haway1.2, haway1.1⟩ ⟨haway2.2, haway2.1⟩
  refine PhysicalState.op2_admitted (by rfl) hLegal hm hcurrent (.inr ⟨rfl, rfl⟩)
    (.inr ⟨rfl, rfl⟩) hs hsa hsp hroomS hf hfa hfp hroomF (Ne.symm hdistinct)
    (rhsSlot := (some true, 0)) rfl
    (by simp [op2NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]) rfl
    hsrc1 hcapture1 hsrc2 hcapture2 hLower hresults hboundaries hwrites
    (by rw [hpublications]; exact hpub)
    (by rw [haction]; simp [Ne.symm hdistinct])
    hpairPlan hselAgents hselWrites ?_ ?_ hcapacity
  · intro port
    rw [hselBoundary]
    simp only [List.map_cons, List.map_nil, List.mem_cons, List.not_mem_nil, or_false]
    constructor
    · rintro ((h | h | h | h) | (h | h | h | h))
      · exact .inr (.inl h)
      · exact .inl h
      · exact .inr (.inr (.inl h))
      · exact .inr (.inr (.inr (.inl h)))
      · exact .inl h
      · exact .inr (.inl h)
      · exact .inr (.inr (.inr (.inr (.inl h))))
      · exact .inr (.inr (.inr (.inr (.inr h))))
    · rintro (h | h | h | h | h | h)
      · exact .inl (.inr (.inl h))
      · exact .inl (.inl h)
      · exact .inl (.inr (.inr (.inl h)))
      · exact .inl (.inr (.inr (.inr h)))
      · exact .inr (.inr (.inr (.inl h)))
      · exact .inr (.inr (.inr (.inr h)))
  · rw [hselBoundary]
    simp [List.filter_cons]

end NobleFoundation.NetI64Plans
