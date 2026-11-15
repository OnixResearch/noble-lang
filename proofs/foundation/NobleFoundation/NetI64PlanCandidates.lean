import NobleFoundation.NetI64PlanInstance

/-!
# Candidate existence from a legal matched arena (issue #110)

For all eight selected rule orientations and both storage policies, a legal
arena with a current selected pair that is a redex of a matching projection
always yields a model-emitter candidate unless the consumed-boundary alias
exclusion holds or the pair has no generation headroom. Captures and
publications are derived from the prestate; nothing about the candidate is
assumed.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- Forward OP2 has a candidate in every legal, current, matched, non-aliased
arena with generation headroom. -/
theorem PhysicalState.op2_forward_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {value : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.op2 op) (.num value)
      (op2NumData op value).toX.toRhs)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .op2 op, .num value, (op2NumData op value).toX) = false)
    (hroom : state.pairHeadroom pair = true) :
    ∃ plan, state.planCandidate pair
      (a, b, .op2 op, .num value, (op2NumData op value).toX) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, _, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  have hroomF : first.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    simp only [PhysicalState.pairHeadroom, hf, hs, Bool.and_eq_true, bne_iff_ne, ne_eq] at hroom
    exact hroom.1
  have haliasFacts := halias
  simp [PhysicalState.consumedBoundaryAlias, hf, NetI64.arity, List.range_succ] at haliasFacts
  obtain ⟨boundaryOne, p1, hcap1, _, hcapture1, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 1) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.1
        simp [hcontent] at h
        exact h)
  obtain ⟨boundaryTwo, p2, hcap2, _, hcapture2, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 2) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.2
        simp [hcontent] at h
        exact h)
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
  have hdistinctTargets := PhysicalState.captured_targets_distinct (by decide) hcapture1 hcapture2
  exact PhysicalState.op2_forward_candidate_exists hcurrent hf hs (by simp [hfa])
    (by simp [hsa]) hfp hsp hcap1 hcap2 (by simp [hdistinctTargets]) halias hpub

/-- Reverse OP2 has a candidate in every legal, current, matched, non-aliased
arena with generation headroom. -/
theorem PhysicalState.op2_reverse_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {value : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num value) (.op2 op)
      (op2NumData op value).toX.swap.toRhs)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .num value, .op2 op, (op2NumData op value).toX.swap) = false)
    (hroom : state.pairHeadroom pair = true) :
    ∃ plan, state.planCandidate pair
      (a, b, .num value, .op2 op, (op2NumData op value).toX.swap) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, _, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  have hroomS : second.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    simp only [PhysicalState.pairHeadroom, hf, hs, Bool.and_eq_true, bne_iff_ne, ne_eq] at hroom
    exact hroom.2
  have haliasFacts := halias
  simp [PhysicalState.consumedBoundaryAlias, hs, NetI64.arity, List.range_succ] at haliasFacts
  obtain ⟨boundaryOne, p1, hcap1, _, hcapture1, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 1) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.1
        simp [hcontent] at h
        exact ⟨h.2, h.1⟩)
  obtain ⟨boundaryTwo, p2, hcap2, _, hcapture2, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 2) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.2
        simp [hcontent] at h
        exact ⟨h.2, h.1⟩)
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
  have hdistinctTargets := PhysicalState.captured_targets_distinct (by decide) hcapture1 hcapture2
  exact PhysicalState.op2_reverse_candidate_exists hcurrent hf hs (by simp [hfa])
    (by simp [hsa]) hfp hsp hcap1 hcap2 (by simp [hdistinctTargets]) halias hpub

/-- Forward OP1 has a candidate in every legal, current, matched, non-aliased
arena with generation headroom. -/
theorem PhysicalState.op1_forward_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {firstValue secondValue : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.op1 op firstValue)
      (.num secondValue) (op1NumData op firstValue secondValue).toX.toRhs)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX) = false)
    (hroom : state.pairHeadroom pair = true) :
    ∃ plan, state.planCandidate pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, _, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  have hroomF : first.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    simp only [PhysicalState.pairHeadroom, hf, hs, Bool.and_eq_true, bne_iff_ne, ne_eq] at hroom
    exact hroom.1
  have haliasFacts := halias
  simp [PhysicalState.consumedBoundaryAlias, hf, NetI64.arity] at haliasFacts
  obtain ⟨boundary, p1, hcap, _, hcapture, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 1) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts
        simp [hcontent] at h
        exact h)
  have hfirstBound : pair.first < state.slots.size := (getElem?_eq_some_iff.mp hf).1
  obtain ⟨published, hpub⟩ := PhysicalState.publicationFor_defined (pair := pair)
    (boundaries := [boundary])
    (result := (!reuse, if reuse then pair.first else state.slots.size,
      .num (op.apply firstValue secondValue), pair.first, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.first) (valueSlot := pair.second)
    rfl rfl hcapture (.inl ⟨rfl, rfl⟩)
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
  exact PhysicalState.op1_forward_candidate_exists hcurrent hf hs (by simp [hfa])
    (by simp [hsa]) hfp hsp hcap halias hpub

/-- Reverse OP1 has a candidate in every legal, current, matched, non-aliased
arena with generation headroom. -/
theorem PhysicalState.op1_reverse_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {op : Op} {firstValue secondValue : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num secondValue)
      (.op1 op firstValue) (op1NumData op firstValue secondValue).toX.swap.toRhs)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap) = false)
    (hroom : state.pairHeadroom pair = true) :
    ∃ plan, state.planCandidate pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, _, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  have hroomS : second.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    simp only [PhysicalState.pairHeadroom, hf, hs, Bool.and_eq_true, bne_iff_ne, ne_eq] at hroom
    exact hroom.2
  have haliasFacts := halias
  simp [PhysicalState.consumedBoundaryAlias, hs, NetI64.arity] at haliasFacts
  obtain ⟨boundary, p1, hcap, _, hcapture, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 1) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts
        simp [hcontent] at h
        exact ⟨h.2, h.1⟩)
  have hsecondBound : pair.second < state.slots.size := (getElem?_eq_some_iff.mp hs).1
  obtain ⟨published, hpub⟩ := PhysicalState.publicationFor_defined (pair := pair)
    (boundaries := [boundary])
    (result := (!reuse, if reuse then pair.second else state.slots.size,
      .num (op.apply firstValue secondValue), pair.second, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.second) (valueSlot := pair.first)
    rfl rfl hcapture (.inr ⟨rfl, rfl⟩)
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
  exact PhysicalState.op1_reverse_candidate_exists hcurrent hf hs (by simp [hfa])
    (by simp [hsa]) hfp hsp hcap halias hpub

/-- Forward ERA always has its empty candidate in a legal, current, matched
arena. -/
theorem PhysicalState.era_forward_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b .era (.num value)
      eraNumData.toX.toRhs) :
    ∃ plan, state.planCandidate pair
      (a, b, .era, .num value, eraNumData.toX) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, _, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨plan, hplan, _⟩ := PhysicalState.era_forward_candidate_exists (reuse := reuse)
    (value := value) hcurrent hf hs (by simp [hfa]) (by simp [hsa]) hfp hsp
  exact ⟨plan, hplan⟩

/-- Reverse ERA always has its empty candidate in a legal, current, matched
arena. -/
theorem PhysicalState.era_reverse_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num value) .era
      eraNumData.toX.swap.toRhs) :
    ∃ plan, state.planCandidate pair
      (a, b, .num value, .era, eraNumData.toX.swap) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, _, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨plan, hplan, _⟩ := PhysicalState.era_reverse_candidate_exists (reuse := reuse)
    (value := value) hcurrent hf hs (by simp [hfa]) (by simp [hsa]) hfp hsp
  exact ⟨plan, hplan⟩

/-- Forward DUP has a candidate in every legal, current, matched,
non-aliased arena with generation headroom. -/
theorem PhysicalState.dup_forward_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b .dup (.num value)
      (dupNumData value).toX.toRhs)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .dup, .num value, (dupNumData value).toX) = false)
    (hroom : state.pairHeadroom pair = true) :
    ∃ plan, state.planCandidate pair
      (a, b, .dup, .num value, (dupNumData value).toX) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  have hroomBoth : first.generation ≠ UInt64.ofNat (UInt64.size - 1) ∧
      second.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    simp only [PhysicalState.pairHeadroom, hf, hs, Bool.and_eq_true, bne_iff_ne, ne_eq] at hroom
    exact hroom
  have haliasFacts := halias
  simp [PhysicalState.consumedBoundaryAlias, hf, NetI64.arity, List.range_succ] at haliasFacts
  obtain ⟨boundaryOne, p1, hcap1, _, hcapture1, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 1) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.1
        simp [hcontent] at h
        exact h)
  obtain ⟨boundaryTwo, p2, hcap2, _, hcapture2, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 2) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.2
        simp [hcontent] at h
        exact h)
  have hfirstBound : pair.first < state.slots.size := (getElem?_eq_some_iff.mp hf).1
  have hsecondBound : pair.second < state.slots.size := (getElem?_eq_some_iff.mp hs).1
  obtain ⟨firstPublished, hpub1⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.first else state.slots.size, .num value,
      pair.first, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.first) (valueSlot := pair.second)
    rfl rfl hcapture1 (.inl ⟨rfl, rfl⟩)
    (by
      intro peer hpeer hne _
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨first, by simpa using hf, hroomBoth.1⟩
      | false => simp at hr)
  obtain ⟨secondPublished, hpub2⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.second else state.slots.size + 1, .num value,
      pair.second, [some 1, none, none]))
    (index := 1) (ruleSlot := pair.first) (valueSlot := pair.second)
    rfl rfl hcapture2 (.inl ⟨rfl, rfl⟩)
    (by
      intro peer hpeer _ hne
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨second, by simpa using hs, hroomBoth.2⟩
      | false => simp at hr)
  have hdistinctTargets := PhysicalState.captured_targets_distinct (by decide) hcapture1 hcapture2
  exact PhysicalState.dup_forward_candidate_exists hcurrent hf hs (by simp [hfa])
    (by simp [hsa]) hfp hsp hcap1 hcap2 (by simp [hdistinctTargets]) halias hpub1 hpub2

/-- Reverse DUP has a candidate in every legal, current, matched,
non-aliased arena with generation headroom. -/
theorem PhysicalState.dup_reverse_candidate_of_legal
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num value) .dup
      (dupNumData value).toX.swap.toRhs)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .num value, .dup, (dupNumData value).toX.swap) = false)
    (hroom : state.pairHeadroom pair = true) :
    ∃ plan, state.planCandidate pair
      (a, b, .num value, .dup, (dupNumData value).toX.swap) reuse = some plan := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, _, _, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  have hroomBoth : first.generation ≠ UInt64.ofNat (UInt64.size - 1) ∧
      second.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    simp only [PhysicalState.pairHeadroom, hf, hs, Bool.and_eq_true, bne_iff_ne, ne_eq] at hroom
    exact hroom
  have haliasFacts := halias
  simp [PhysicalState.consumedBoundaryAlias, hs, NetI64.arity, List.range_succ] at haliasFacts
  obtain ⟨boundaryOne, p1, hcap1, _, hcapture1, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 1) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.1
        simp [hcontent] at h
        exact ⟨h.2, h.1⟩)
  obtain ⟨boundaryTwo, p2, hcap2, _, hcapture2, _, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 2) (by simp [NetI64.arity]) (by
        intro peer peerPort hcontent
        have h := haliasFacts.2
        simp [hcontent] at h
        exact ⟨h.2, h.1⟩)
  have hfirstBound : pair.first < state.slots.size := (getElem?_eq_some_iff.mp hf).1
  have hsecondBound : pair.second < state.slots.size := (getElem?_eq_some_iff.mp hs).1
  obtain ⟨firstPublished, hpub1⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.second else state.slots.size, .num value,
      pair.second, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.second) (valueSlot := pair.first)
    rfl rfl hcapture1 (.inr ⟨rfl, rfl⟩)
    (by
      intro peer hpeer hne _
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨second, by simpa using hs, hroomBoth.2⟩
      | false => simp at hr)
  obtain ⟨secondPublished, hpub2⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.first else state.slots.size + 1, .num value,
      pair.first, [some 1, none, none]))
    (index := 1) (ruleSlot := pair.second) (valueSlot := pair.first)
    rfl rfl hcapture2 (.inr ⟨rfl, rfl⟩)
    (by
      intro peer hpeer _ hne
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨first, by simpa using hf, hroomBoth.1⟩
      | false => simp at hr)
  have hdistinctTargets := PhysicalState.captured_targets_distinct (by decide) hcapture1 hcapture2
  exact PhysicalState.dup_reverse_candidate_exists hcurrent hf hs (by simp [hfa])
    (by simp [hsa]) hfp hsp hcap1 hcap2 (by simp [hdistinctTargets]) halias hpub1 hpub2

end NobleFoundation.NetI64Plans
