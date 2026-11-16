import NobleFoundation.NetI64PlanRewriteMaps
import NobleFoundation.NetI64PlanRewriteOwners
import NobleFoundation.NetI64PlanRewriteQueue

/-!
# Semantic admission of an emitted image from its local rewrite (issue #110)

An actually lowered image whose captured boundary denotes the abstract
selected boundary, whose physical execution is described by a
`PhysicalState.LocalRewrite` bundle, whose untouched slots are outside the
plan's touched set, and which is not excluded by queue capacity, passes the
independent semantic checker at exactly its arena, scratch and slot-derived
queue reservations. No checker-acceptance premise is used: every
`SemanticFields` conjunct is derived.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- **Admission from a local rewrite.** -/
theorem PhysicalState.semanticCheck_of_local_rewrite
    {state next : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    {ruleSlot valueSlot : Nat} {ruleId valueId : Id}
    {ruleSymbol valueSymbol : NetSym}
    {results : List ResultView} {edges : List EdgeView}
    {retired : List Nat} {fresh : Nat} {published : List PhysicalPair}
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hBoundary : plan.logicalBoundary state = some
      ((Plan.compile state.project redex hRule).capturedBoundary.filter
        fun edge => edge.1.2 != 0))
    (hExecute : state.executeCheckedImage plan.emit redex = some next)
    (hRewrite : state.LocalRewrite next (Plan.compile state.project redex hRule)
      ruleSlot valueSlot ruleId valueId ruleSymbol valueSymbol
      results edges retired fresh published)
    (hTouched : ∀ index, index < state.slots.size →
      plan.touchedSlots.contains index = false →
      index ≠ ruleSlot ∧ index ≠ valueSlot ∧
      (∀ edge ∈ edges, ∀ port, edge.target ≠ (0, index, port)) ∧
      (∀ queued ∈ published, queued.first ≠ index ∧ queued.second ≠ index))
    (hcap : state.queueCapacityExcluded plan = false) :
    BytePlan.semanticCheck state plan.emit redex hRule plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨_, _, _, _, _, _, hscratch, _⟩ := PhysicalState.planCandidate_fields hc
  have hreserve := PhysicalState.reserve_if_queue_capacity hscratch hcap
  have hprefix := PhysicalState.lowerPlan_semantic_prefix hRule hLower hBoundary
  apply BytePlan.semanticCheck_of_fields hRule (PhysicalState.lowerPlan_decodes hLower)
    hreserve hExecute
  obtain ⟨hop, hbound, hsymbols, hwrites, hexternal, harena, hscratch', hqueue⟩ := hprefix
  obtain ⟨hfreeNodup, hfreeBounds, hfreeInventory⟩ := hRewrite.free_fields
  refine ⟨hop, hbound, hsymbols, hwrites, hexternal, harena, hscratch', hqueue,
    ?_, hRewrite.outputs_field _, hRewrite.liveIds_nodup, hfreeNodup, hfreeBounds,
    hfreeInventory, hRewrite.work_canonical, ?_, ?_, hRewrite.work_perm,
    hRewrite.reciprocalPorts, hRewrite.outputOwners, hRewrite.observed.queueAfter⟩
  · apply List.all_eq_true.mpr
    intro index hindex
    have hlt : index < state.slots.size := List.mem_range.mp hindex
    cases htouched : plan.touchedSlots.contains index with
    | true => simp
    | false =>
      obtain ⟨h1, h2, hedge, hpub⟩ := hTouched index hlt htouched
      simp [hRewrite.observed.untouchedAfter index hlt h1 h2 hedge hpub]
  · exact (Std.HashMap.Equiv.of_forall_getElem?_eq fun id =>
      hRewrite.agents_eq _ id).toList_perm
  · exact (Std.HashMap.Equiv.of_forall_getElem?_eq fun port =>
      hRewrite.wires_eq _ port).toList_perm

end NobleFoundation.NetI64Plans
