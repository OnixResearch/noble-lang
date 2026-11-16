import NobleFoundation.NetI64PlanLegal

/-!
# Exact candidate images for OP2 and DUP (issue #110)

The OP1 candidate-shape theorems fix every field of a constructed candidate
from its capture and publication inputs. These are the corresponding exact
images for both orientations of OP2 (one `op1` result reading two captured
endpoints) and DUP (two number results, one per captured endpoint), in either
storage policy. They inspect only the prestate inputs of the model emitter.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- A selected forward OP2 candidate has one result reading both captured
endpoints in auxiliary-port order. -/
theorem PhysicalState.op2_forward_candidate_shape
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)} {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.op2 op))
    (hsa : second.agent.map Prod.snd = some (.num value))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.first 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.first 2 = some boundaryTwo)
    (hpub : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.first else state.slots.size,
        .op1 op value, pair.first, [some 0, some 1, none]) = some published)
    (hc : state.planCandidate pair
      (a, b, .op2 op, .num value, (op2NumData op value).toX) reuse = some plan) :
    plan.results =
      [(not reuse, if reuse then pair.first else state.slots.size,
        .op1 op value, pair.first, [some 0, some 1, none])] ∧
    plan.boundaries = [boundaryOne, boundaryTwo] ∧
    plan.externalWrites = [(0, 0, 0), (0, 1, 1)] ∧
    plan.publications = published ∧
    plan.action = (if reuse then 1 else 0, 0) ∧
    plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore at hc
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hpub,
    op2NumData, RuleData.toX, RuleData.ofWires, assoc] at hc
  obtain ⟨body, ⟨_, hbody⟩, hwrap⟩ := hc
  subst body
  rw [← hwrap]
  have hdistinct : pair.first ≠ pair.second := by
    intro heq
    rw [heq] at hf
    have heqSlots : first = second := Option.some.inj (hf.symm.trans hs)
    subst second
    simp [hfa] at hsa
  simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites, hdistinct.symm]

/-- A selected reverse OP2 candidate reads the second physical slot's
captured endpoints into its one result. -/
theorem PhysicalState.op2_reverse_candidate_shape
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)} {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num value))
    (hsa : second.agent.map Prod.snd = some (.op2 op))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.second 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.second 2 = some boundaryTwo)
    (hpub : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.second else state.slots.size,
        .op1 op value, pair.second, [some 0, some 1, none]) = some published)
    (hc : state.planCandidate pair
      (a, b, .num value, .op2 op, (op2NumData op value).toX.swap) reuse = some plan) :
    plan.results =
      [(not reuse, if reuse then pair.second else state.slots.size,
        .op1 op value, pair.second, [some 0, some 1, none])] ∧
    plan.boundaries = [boundaryOne, boundaryTwo] ∧
    plan.externalWrites = [(0, 0, 0), (0, 1, 1)] ∧
    plan.publications = published ∧
    plan.action = (0, if reuse then 1 else 0) ∧
    plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore at hc
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hpub,
    op2NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc] at hc
  obtain ⟨body, ⟨_, hbody⟩, hwrap⟩ := hc
  subst body
  rw [← hwrap]
  have hdistinct : pair.first ≠ pair.second := by
    intro heq
    rw [heq] at hf
    have heqSlots : first = second := Option.some.inj (hf.symm.trans hs)
    subst second
    simp [hfa] at hsa
  simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites, hdistinct]

/-- A selected forward DUP candidate has two number results, the first
donated by the duplicator slot and the second by the number slot. -/
theorem PhysicalState.dup_forward_candidate_shape
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {firstPublished secondPublished : List (Nat × Nat × Nat × Nat)}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some .dup)
    (hsa : second.agent.map Prod.snd = some (.num value))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.first 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.first 2 = some boundaryTwo)
    (hpub1 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.first else state.slots.size,
        .num value, pair.first, [some 0, none, none]) = some firstPublished)
    (hpub2 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.second else state.slots.size + 1,
        .num value, pair.second, [some 1, none, none]) = some secondPublished)
    (hc : state.planCandidate pair
      (a, b, .dup, .num value, (dupNumData value).toX) reuse = some plan) :
    plan.results =
      [(not reuse, if reuse then pair.first else state.slots.size,
          .num value, pair.first, [some 0, none, none]),
        (not reuse, if reuse then pair.second else state.slots.size + 1,
          .num value, pair.second, [some 1, none, none])] ∧
    plan.boundaries = [boundaryOne, boundaryTwo] ∧
    plan.externalWrites = [(0, 0, 0), (1, 0, 1)] ∧
    plan.publications = firstPublished ++ secondPublished ∧
    plan.action = (if reuse then 1 else 0, if reuse then 1 else 0) ∧
    plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore at hc
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hpub1, hpub2,
    dupNumData, RuleData.toX, RuleData.ofWires, assoc] at hc
  obtain ⟨body, ⟨_, hbody⟩, hwrap⟩ := hc
  subst body
  rw [← hwrap]
  cases reuse <;> simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]

/-- A selected reverse DUP candidate takes its donors in rule-slot,
number-slot order, i.e. second then first physical slot. -/
theorem PhysicalState.dup_reverse_candidate_shape
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {firstPublished secondPublished : List (Nat × Nat × Nat × Nat)}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num value))
    (hsa : second.agent.map Prod.snd = some .dup)
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.second 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.second 2 = some boundaryTwo)
    (hpub1 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.second else state.slots.size,
        .num value, pair.second, [some 0, none, none]) = some firstPublished)
    (hpub2 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.first else state.slots.size + 1,
        .num value, pair.first, [some 1, none, none]) = some secondPublished)
    (hc : state.planCandidate pair
      (a, b, .num value, .dup, (dupNumData value).toX.swap) reuse = some plan) :
    plan.results =
      [(not reuse, if reuse then pair.second else state.slots.size,
          .num value, pair.second, [some 0, none, none]),
        (not reuse, if reuse then pair.first else state.slots.size + 1,
          .num value, pair.first, [some 1, none, none])] ∧
    plan.boundaries = [boundaryOne, boundaryTwo] ∧
    plan.externalWrites = [(0, 0, 0), (1, 0, 1)] ∧
    plan.publications = firstPublished ++ secondPublished ∧
    plan.action = (if reuse then 1 else 0, if reuse then 1 else 0) ∧
    plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore at hc
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hpub1, hpub2,
    dupNumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc] at hc
  obtain ⟨body, ⟨_, hbody⟩, hwrap⟩ := hc
  subst body
  rw [← hwrap]
  cases reuse <;> simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]

end NobleFoundation.NetI64Plans
