import NobleFoundation.NetI64PlanOp2
import NobleFoundation.NetI64PlanOp1
import NobleFoundation.NetI64PlanDup
import NobleFoundation.NetI64PlanEra
import NobleFoundation.NetI64PlanCandidates
import NobleFoundation.NetI64PlanEncode

/-!
# Universal emitter classification (issue #110)

`PhysicalState.Representable` is the independent precondition: a structurally
legal arena, a generation-current selected physical pair, and a matching
projection in which the pair is a redex of the selected revision-3 rule. For
every such instance, every selected rule and orientation (OP2, OP1, DUP, ERA)
and either storage policy, the Lean model emitter is classified exhaustively:
either one named exclusion refuses it (consumed-boundary alias, unencodable
32-bit index table, exhausted generation, physical queue capacity), or
`lowerPlan` returns an image that `BytePlan.semanticCheck` admits at its exact
reservations, and that admitted commit refines one abstract `CStep` with a
directed-pair queue permutation and stable free ports. The emitter's byte
roundtrip check is not an exclusion: inside the index envelope every
candidate decodes exactly (`PhysicalState.planCandidate_decodes`). This
concerns the Lean model emitter, executor and checker only; it is not a
statement about the Rust compiler, checker or executor.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The independent representability relation for one selected instance. -/
def PhysicalState.Representable (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) : Prop :=
  state.Legal ∧ state.logicalPair pair = some (redex.1, redex.2.1) ∧
    state.project.toNet.Matching NetI64.arity ∧
    Redex NetI64.system state.project.toNet redex.1 redex.2.1 redex.2.2.1 redex.2.2.2.1
      redex.2.2.2.2.toRhs

/-- Every representable instance of a selected rule has a model-emitter
candidate unless the alias exclusion holds or generation headroom is
exhausted, in every orientation and storage policy. -/
theorem PhysicalState.candidate_of_representable
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    (reuse : Bool)
    (hRep : state.Representable pair redex)
    (halias : state.consumedBoundaryAlias pair redex = false)
    (hroom : state.pairHeadroom pair = true) :
    ∃ plan, state.planCandidate pair redex reuse = some plan := by
  obtain ⟨a, b, alpha, beta, rhs⟩ := redex
  obtain ⟨hLegal, hcurrent, hm, hredex⟩ := hRep
  simp only at hRule hcurrent hredex halias ⊢
  cases alpha <;> cases beta <;> simp [NetI64.xRule] at hRule <;> subst hRule
  · exact PhysicalState.op2_reverse_candidate_of_legal hLegal hm hcurrent hredex halias hroom
  · exact PhysicalState.op1_reverse_candidate_of_legal hLegal hm hcurrent hredex halias hroom
  · exact PhysicalState.dup_reverse_candidate_of_legal hLegal hm hcurrent hredex halias hroom
  · exact PhysicalState.era_reverse_candidate_of_legal hLegal hm hcurrent hredex
  · exact PhysicalState.op2_forward_candidate_of_legal hLegal hm hcurrent hredex halias hroom
  · exact PhysicalState.op1_forward_candidate_of_legal hLegal hm hcurrent hredex halias hroom
  · exact PhysicalState.dup_forward_candidate_of_legal hLegal hm hcurrent hredex halias hroom
  · exact PhysicalState.era_forward_candidate_of_legal hLegal hm hcurrent hredex

/-- Every actually emitted image of a representable instance that is not
excluded by queue capacity is admitted by the independent semantic checker
at its exact reservations, in every orientation and storage policy. -/
theorem PhysicalState.accepted_of_representable
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    (hRep : state.Representable pair redex)
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit redex hRule
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨a, b, alpha, beta, rhs⟩ := redex
  obtain ⟨hLegal, hcurrent, hm, hredex⟩ := hRep
  simp only at hRule hcurrent hredex hLower ⊢
  cases alpha <;> cases beta <;> simp [NetI64.xRule] at hRule <;> subst hRule
  · exact PhysicalState.op2_reverse_accepted hLegal hm hcurrent hredex hLower hcapacity
  · exact PhysicalState.op1_reverse_accepted hLegal hm hcurrent hredex hLower hcapacity
  · exact PhysicalState.dup_reverse_accepted hLegal hm hcurrent hredex hLower hcapacity
  · exact PhysicalState.era_reverse_accepted hLegal hm hcurrent hredex hLower hcapacity
  · exact PhysicalState.op2_forward_accepted hLegal hm hcurrent hredex hLower hcapacity
  · exact PhysicalState.op1_forward_accepted hLegal hm hcurrent hredex hLower hcapacity
  · exact PhysicalState.dup_forward_accepted hLegal hm hcurrent hredex hLower hcapacity
  · exact PhysicalState.era_forward_accepted hLegal hm hcurrent hredex hLower hcapacity

/-- **The physical-field bridge.** Every actually emitted image of a
representable instance that is not excluded by queue capacity supplies all
three witnesses of `lowerPlan_refines_of_physical_fields` at its exact
reservations: the resource reservation succeeds, the generic physical
executor runs the emitted bytes, and every independently specified semantic
field holds. -/
theorem PhysicalState.emitted_image_physical_fields_on_representable_layout
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    (hRep : state.Representable pair redex)
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, (∃ transaction, state.reserve plan plan.arenaReservation 256
        (state.slots.size + plan.arenaReservation - state.work.length) = some transaction) ∧
      state.executeCheckedImage plan.emit redex = some next ∧
      plan.SemanticFields state next redex hRule := by
  obtain ⟨next, hnext⟩ := PhysicalState.accepted_of_representable hRule hRep hLower hcapacity
  obtain ⟨decoded, transaction, hdecode, hreserve, hexecute, hfields⟩ :=
    BytePlan.semanticCheck_fields hRule hnext
  have hplan : decoded = plan := by
    rw [PhysicalState.lowerPlan_decodes hLower] at hdecode
    exact (Option.some.inj hdecode).symm
  subst hplan
  exact ⟨next, ⟨transaction, hreserve⟩, hexecute, hfields⟩

/-- **Universal emitter classification.** For every representable instance of
a selected revision-3 rule, in either orientation and storage policy, one of
the following holds: a named exclusion refuses it (consumed-boundary alias;
unencodable index table; exhausted generation; physical queue capacity, which
the checker refuses before any mutation at every queue limit up to the
slot-derived headroom), or the lowered image is admitted by
`BytePlan.semanticCheck` at its exact reservations. No byte-roundtrip case
remains: an encodable candidate always decodes to itself. -/
theorem PhysicalState.emitter_classification
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    (reuse : Bool)
    (hRep : state.Representable pair redex) :
    (state.consumedBoundaryAlias pair redex = true ∧
      state.lowerPlan pair redex reuse = none) ∨
    (state.planIndicesEncodable = false ∧ state.lowerPlan pair redex reuse = none) ∨
    (state.pairHeadroom pair = false ∧ state.lowerPlan pair redex reuse = none) ∨
    (∃ plan, state.lowerPlan pair redex reuse = some plan ∧
      state.queueCapacityExcluded plan = true ∧
      ∀ arena scratch queue,
        queue ≤ state.slots.size + plan.arenaReservation - state.work.length →
        BytePlan.semanticCheck state plan.emit redex hRule arena scratch queue = none) ∨
    (∃ plan next, state.lowerPlan pair redex reuse = some plan ∧
      state.queueCapacityExcluded plan = false ∧
      BytePlan.semanticCheck state plan.emit redex hRule plan.arenaReservation 256
        (state.slots.size + plan.arenaReservation - state.work.length) = some next) := by
  cases hLower : state.lowerPlan pair redex reuse with
  | some plan =>
    cases hcap : state.queueCapacityExcluded plan with
    | true =>
      refine .inr (.inr (.inr (.inl ⟨plan, rfl, hcap, ?_⟩)))
      intro arena scratch queue hqueue
      exact BytePlan.semanticCheck_refuses_queue_capacity hRule
        (PhysicalState.lowerPlan_decodes hLower) hcap hqueue
    | false =>
      obtain ⟨next, hnext⟩ := PhysicalState.accepted_of_representable hRule hRep hLower hcap
      exact .inr (.inr (.inr (.inr ⟨plan, next, rfl, hcap, hnext⟩)))
  | none =>
    cases hencode : state.planIndicesEncodable with
    | false => exact .inr (.inl ⟨rfl, rfl⟩)
    | true =>
      cases hroom : state.pairHeadroom pair with
      | false => exact .inr (.inr (.inl ⟨rfl, rfl⟩))
      | true =>
        cases halias : state.consumedBoundaryAlias pair redex with
        | true => exact .inl ⟨rfl, rfl⟩
        | false =>
          obtain ⟨candidate, hcandidate⟩ :=
            PhysicalState.candidate_of_representable hRule reuse hRep halias hroom
          rw [PhysicalState.lowerPlan_eq_candidate hencode hroom, hcandidate] at hLower
          cases hLower

/-- **Unconditional refinement corollary.** Every representable instance whose
lowering is not refused by a named exclusion is admitted, and its admitted
commit is one abstract `CStep` of the matched net, with the projected
generation-checked queue a permutation of the abstract live pending pairs and
the stable free-port list unchanged. -/
theorem PhysicalState.emitter_refines
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    (hRep : state.Representable pair redex)
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit redex hRule plan.arenaReservation 256
        (state.slots.size + plan.arenaReservation - state.work.length) = some next ∧
      CStep NetI64.system state.project.toNet next.project.toNet ∧
      next.project.work.Perm
        (Plan.pendingPairs (Plan.commit state.project
          (Plan.compile state.project redex hRule) state.project.work)) ∧
      next.project.free = state.project.free := by
  obtain ⟨next, hnext⟩ := PhysicalState.accepted_of_representable hRule hRep hLower hcapacity
  exact ⟨next, hnext, BytePlan.semanticCheck_refines hRule hnext hRep.2.2.1 hRep.2.2.2⟩

end NobleFoundation.NetI64Plans
