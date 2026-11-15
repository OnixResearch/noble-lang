import NobleFoundation.NetI64PlanLowering

/-!
# Structural physical-plan legality

These finite arena invariants are independent of the semantic checker's
post-state comparison. Headroom is a per-selected-pair resource rather than
an inductive property of every committed arena: an accepted increment can
legitimately leave a generation at its maximum, after which further reuse
or retirement must refuse before any byte image is emitted.
-/

namespace NobleFoundation.NetI64Plans

open NobleFoundation.NetI64

/-- Exact free-slot inventory, canonical old queue, unique logical owners,
reciprocal physical ports and unique, owned output targets. No claim of
emitter completeness is built into this predicate. -/
def PhysicalState.Legal (state : PhysicalState) : Prop :=
  state.freeSlots.Nodup ∧
    state.freeSlots.all (fun index => decide (index < state.slots.size)) = true ∧
    (List.range state.slots.size).all (fun index =>
      decide (index ∈ state.freeSlots) ==
        ((state.slots[index]?).map (fun slot => slot.agent.isNone)).getD false) = true ∧
    state.work.all (fun pair => decide (pair.first < pair.second)) = true ∧
    (state.slots.toList.filterMap fun slot => slot.agent.map Prod.fst).Nodup ∧
    state.reciprocalPorts = true ∧ state.outputOwners = true


/-- A pair's append mode may need two new indices in the 32-bit byte format.
This is independent from a legal physical graph and from generation headroom.
Rust slot/output references use `u32`; the Lean arena uses arbitrary `Nat`. -/
def PhysicalState.planIndicesEncodable (state : PhysicalState) : Bool :=
  state.slots.size + 2 ≤ 2 ^ 32 && state.outputs.length < 2 ^ 32

set_option synthInstance.maxSize 1024 in
/-- The five independently modeled initial layouts satisfy all finite
structural legality checks, including the live-peer publication layout. -/
theorem PhysicalState.five_initial_layouts_legal :
    op2PhysicalStart.Legal ∧ op2PublishedStart.Legal ∧
    op1PhysicalStart.Legal ∧ dupPhysicalStart.Legal ∧
    eraPhysicalStart.Legal := by
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  all_goals
    unfold PhysicalState.Legal
    set_option maxRecDepth 3000 in decide

/-- The same five admitted finite commits still satisfy structural legality;
this is not a preservation theorem for arbitrary selected layouts. -/
theorem PhysicalState.five_committed_layouts_legal :
    op2PhysicalAfter.Legal ∧ op2PublishedAfter.Legal ∧
    op1PhysicalAfter.Legal ∧ dupPhysicalAfter.Legal ∧
    eraPhysicalAfter.Legal := by
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  all_goals
    unfold PhysicalState.Legal
    set_option maxRecDepth 3000 in decide

/-- Generation exhaustion does not make the arena structurally illegal.
Its current selected pair instead refuses emission before committing. -/
theorem PhysicalState.overflow_layout_legal :
    overflowOp2Start.Legal ∧
    overflowOp2Start.pairHeadroom overflowOp2Pair = false := by
  constructor
  · unfold PhysicalState.Legal
    set_option maxRecDepth 3000 in decide
  · decide

/-- A reciprocal auxiliary loop is structurally legal but outside the
selected checked DAG-origin Rust admission profile. -/
def op2AuxLoopStart : PhysicalState :=
  { op2PhysicalStart with
    slots := op2PhysicalStart.slots.setIfInBounds 0
      { agent := some ([1], .op2 .add)
        generation := 0
        queuedGeneration := none
        ports := [some (1, 0), some (0, 2), some (0, 1)] }
    outputs := [] }

/-- Mere structural legality, a current selected pair and available
generation headroom cannot imply emission: the explicit alias refusal is
distinct from the generation-exhaustion refusal. -/
theorem PhysicalState.aux_loop_is_explicit_exclusion :
    op2AuxLoopStart.Legal ∧
    op2AuxLoopStart.logicalPair
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 } =
        some ([1], [2]) ∧
    op2AuxLoopStart.pairHeadroom
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 } = true ∧
    op2AuxLoopStart.consumedBoundaryAlias
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
      op2PhysicalRedex = true ∧
    op2AuxLoopStart.lowerPlan
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
      op2PhysicalRedex true = none := by
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · unfold PhysicalState.Legal
    set_option maxRecDepth 3000 in decide
  all_goals set_option maxRecDepth 3000 in decide

/-- A semantically checked step restores the ownership, free-slot and
canonical-queue subset of structural legality. Full legality additionally
uses the explicit physical-edge checks in `SemanticFields`. -/
theorem PhysicalState.checked_ownership_inventory
    {state next : PhysicalState} {bytes : List UInt8}
    {redex : INet.XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (h : BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next) :
    next.freeSlots.Nodup ∧
    next.freeSlots.all (fun index => decide (index < next.slots.size)) = true ∧
    (List.range next.slots.size).all (fun index =>
      decide (index ∈ next.freeSlots) ==
        ((next.slots[index]?).map (fun slot => slot.agent.isNone)).getD false) = true ∧
    next.work.all (fun pair => decide (pair.first < pair.second)) = true ∧
    (next.slots.toList.filterMap fun slot => slot.agent.map Prod.fst).Nodup := by
  obtain ⟨owners, free, bounds, inventory, queueOrder⟩ :=
    BytePlan.semanticCheck_ownership hRule h
  exact ⟨free, bounds, inventory, queueOrder, owners⟩

/-- Any fully checker-admitted commit has a structurally legal physical
post-state. This is a checker guarantee, not emitter completeness. -/
theorem PhysicalState.semanticCheck_result_legal
    {state next : PhysicalState} {bytes : List UInt8}
    {redex : INet.XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (h : BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next) :
    next.Legal := by
  obtain ⟨free, bounds, inventory, queueOrder, owners⟩ :=
    PhysicalState.checked_ownership_inventory hRule h
  obtain ⟨_, _, _, _, _, fields⟩ := BytePlan.semanticCheck_fields hRule h
  dsimp [BytePlan.SemanticFields] at fields
  obtain ⟨_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _,
    reciprocal, outputs⟩ := fields
  exact ⟨free, bounds, inventory, queueOrder, owners, reciprocal, outputs⟩

/-- Staging any physical microstep retains the saved legal committed view;
the potentially incomplete current image is not externally visible. -/
theorem PhysicalTransaction.stage_preserves_visible_legal
    {transaction next : PhysicalTransaction}
    {change : PhysicalState → PhysicalState}
    (legal : transaction.visible.Legal)
    (step : transaction.stage change = some next) :
    next.visible.Legal := by
  unfold PhysicalTransaction.stage at step
  cases journal : transaction.journal with
  | none => simp [journal] at step
  | some saved =>
    simp [journal] at step
    cases step
    simpa [PhysicalTransaction.visible, journal] using legal

end NobleFoundation.NetI64Plans
