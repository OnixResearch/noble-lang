import NobleFoundation.NetI64Plans

/-!
# Semantic admission for revision-3 physical plan images

This independent *model* checker decodes the complete byte stream, reserves its
resources, simulates the physical executor before publication, and checks the
result against the selected inets rewrite. Unlike the byte parser, admission
requires the complete ordered result-port and reciprocal-write inventories,
the captured external boundary, and the entire represented agent, wire,
output-target and directed-queue images. In particular, a syntactically valid
image with a missing output write is not admitted. This is not a proof that
Rust emits only admitted images or that its executor implements this checker.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- Every nonempty result port declares exactly one write; neither the writer
nor the independent checker may omit, duplicate, or append a reciprocal write.
The list order is the order of result slots, then ports. -/
def BytePlan.requiredWrites (plan : BytePlan) : List (Nat × Nat × Nat) :=
  plan.results.zipIdx.flatMap fun (result, resultIndex) =>
    result.2.2.2.2.zipIdx.filterMap fun (boundary, portIndex) =>
      boundary.map fun boundaryIndex => (resultIndex, portIndex, boundaryIndex)

/-- Decode the ordered physical boundary against the *pre-commit* arena.
The source endpoint must be a live selected-agent port, and the peer is
resolved either through its live slot or the stable output label. -/
def BytePlan.logicalBoundary (state : PhysicalState) (plan : BytePlan) :
    Option (List (Port × Port)) :=
  plan.boundaries.mapM fun (source, target) => do
    let left ← state.logicalPort source
    let right ← match target with
      | (0, slot, port) => state.logicalPort (slot, port)
      | (1, output, 0) => (state.outputs[output]?).map PhysicalOutput.label
      | _ => none
    some (left, right)

/-- The four selected operations, in either principal-pair orientation. -/
def BytePlan.ruleOpcode : NetSym → NetSym → Option UInt8
  | .op2 _, .num _ | .num _, .op2 _ => some 1
  | .op1 _ _, .num _ | .num _, .op1 _ _ => some 2
  | .dup, .num _ | .num _, .dup => some 3
  | .era, .num _ | .num _, .era => some 4
  | _, _ => none

/-- The physical slots allowed to change: the consumed pair, captured live
peers and both endpoints of each newly published canonical pair. Other old
slots (including generations and queue marks) are a byte-level frame. -/
def BytePlan.touchedSlots (plan : BytePlan) : List Nat :=
  [plan.pair.1, plan.pair.2.2.1] ++
    (plan.boundaries.filterMap fun boundary =>
      if boundary.2.1 == 0 then some boundary.2.2.1 else none) ++
    plan.publications.flatMap fun pair => [pair.1, pair.2.2.1]

/-- The inets worklist may contain non-redex candidates, notably a new
principal port wired to a free output (whose port number is also zero).
Only entries with two live agent endpoints are directed pending pairs in the
physical arena. Preserve the directed multiplicity, not invalid candidates. -/
def Plan.pendingPairs (net : HNet NetSym) : List (Id × Id) :=
  net.work.filter fun pair =>
    net.agents[pair.1]?.isSome && net.agents[pair.2]?.isSome

/-- Admission conditions are independently re-evaluated against the selected
rule, pre-commit boundary, and the simulated physical state. The two map-list
permutations are *finite executable extensional comparisons*, not equality of
hash-table insertion order. The exact byte-write lists are checked before any
net equality; the post-state checks include the actual mutable output targets.
No producer-origin metadata is trusted. -/
def BytePlan.SemanticFields (state next : PhysicalState) (plan : BytePlan)
    (redex : XRedex NetSym)
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2) : Prop :=
  let selected := Plan.compile state.project redex hRule
  let expected := Plan.commit state.project selected state.project.work
  some plan.opcode = BytePlan.ruleOpcode redex.2.2.1 redex.2.2.2.1 ∧
  plan.logicalBoundary state = some (selected.capturedBoundary.filter
    fun edge => edge.1.2 != 0) ∧
  plan.results.map (fun result => result.2.2.1) = selected.initializedAgents.map Prod.snd ∧
  plan.writes = plan.requiredWrites ∧
  plan.externalWrites = plan.requiredWrites ∧
  plan.arenaReservation = (plan.results.filter fun result => result.1).length ∧
  256 ≤ plan.scratchReservation ∧
  plan.publications.length ≤ plan.worklistReservation ∧
  (List.range state.slots.size).all (fun index =>
    plan.touchedSlots.contains index ||
      decide (next.slots[index]? = state.slots[index]?)) = true ∧
  next.outputs.all (fun output =>
    decide (next.logicalPort output.target = expected.wires[output.label]?)) = true ∧
  (next.slots.toList.filterMap fun slot => slot.agent.map Prod.fst).Nodup ∧
  next.freeSlots.Nodup ∧
  next.freeSlots.all (fun index => decide (index < next.slots.size)) = true ∧
  (List.range next.slots.size).all (fun index =>
    decide (index ∈ next.freeSlots) ==
      ((next.slots[index]?).map (fun slot => slot.agent.isNone)).getD false) = true ∧
  next.work.all (fun pair => decide (pair.first < pair.second)) = true ∧
  next.project.agents.toList.Perm expected.agents.toList ∧
  next.project.wires.toList.Perm expected.wires.toList ∧
  next.project.work.Perm (Plan.pendingPairs expected)

private theorem lookup_eq_of_some_iff {K V : Type} (first second : K → Option V)
    (h : ∀ key value, first key = some value ↔ second key = some value) :
    ∀ key, first key = second key := by
  intro key
  cases hf : first key with
  | none =>
    cases hs : second key with
    | none => rfl
    | some value =>
      have contradiction := (h key value).mpr hs
      simp [hf] at contradiction
  | some value =>
    have hs := (h key value).mp hf
    simp [hs]

/-- A proven whole-net equality implies the two finite map inventories have
the same extensional contents, even when their hash-table insertion orders
differ. This lets existing independently proved physical fixtures witness
semantic admission without runtime-only evaluation. -/
theorem PhysicalState.projected_maps_perm_of_toNet_eq
    (after : PhysicalState) (expected : HNet NetSym)
    (h : after.project.toNet = expected.toNet) :
    after.project.agents.toList.Perm expected.agents.toList ∧
      after.project.wires.toList.Perm expected.wires.toList := by
  have hAgents (id : Id) (symbol : NetSym) :
      after.project.agents[id]? = some symbol ↔ expected.agents[id]? = some symbol := by
    have congruent :=
      congrArg (fun net : Net NetSym => net.agent id symbol) h
    exact iff_of_eq congruent
  have hWires (first second : Port) :
      after.project.wires[first]? = some second ↔ expected.wires[first]? = some second := by
    have congruent :=
      congrArg (fun net : Net NetSym => net.link first second) h
    exact iff_of_eq congruent
  have hagents := lookup_eq_of_some_iff
    (fun id => after.project.agents[id]?) (fun id => expected.agents[id]?) hAgents
  have hwires := lookup_eq_of_some_iff
    (fun port => after.project.wires[port]?) (fun port => expected.wires[port]?) hWires
  exact ⟨(Std.HashMap.Equiv.of_forall_getElem?_eq hagents).toList_perm,
    (Std.HashMap.Equiv.of_forall_getElem?_eq hwires).toList_perm⟩

set_option synthInstance.maxSize 512 in
/-- The admission conditions are decidable from bounded plan and state lists. -/
private def BytePlan.semanticFieldsCheck (state next : PhysicalState) (plan : BytePlan)
    (redex : XRedex NetSym)
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2) : Bool :=
  @decide (plan.SemanticFields state next redex hRule) (by
    unfold BytePlan.SemanticFields
    infer_instance)

private theorem BytePlan.semanticFieldsCheck_iff (state next : PhysicalState)
    (plan : BytePlan) (redex : XRedex NetSym)
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2) :
    plan.semanticFieldsCheck state next redex hRule = true ↔
      plan.SemanticFields state next redex hRule := by
  unfold BytePlan.semanticFieldsCheck
  simp

/-- The checker reserves before simulating any destructive physical step.
`some next` is issued only after byte decoding, resource admission, successful
physical execution and all independent semantic field comparisons. -/
def BytePlan.semanticCheck (state : PhysicalState) (bytes : List UInt8)
    (redex : XRedex NetSym)
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    (arenaAvailable scratchAvailable worklistAvailable : Nat) : Option PhysicalState := do
  let plan ← BytePlan.decode bytes
  let _ ← state.reserve plan arenaAvailable scratchAvailable worklistAvailable
  let next ← state.executeCheckedImage bytes redex
  if plan.semanticFieldsCheck state next redex hRule then some next else none

/-- Field witnesses are sufficient for admission; this direction is useful
when proving that the semantic checker actually admits concrete byte images. -/
theorem BytePlan.semanticCheck_of_fields
    {state next : PhysicalState} {bytes : List UInt8} {plan : BytePlan}
    {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (hd : BytePlan.decode bytes = some plan)
    (hr : ∃ transaction, state.reserve plan arena scratch queue = some transaction)
    (he : state.executeCheckedImage bytes redex = some next)
    (fields : plan.SemanticFields state next redex hRule) :
    BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next := by
  obtain ⟨transaction, hr⟩ := hr
  have checked :=
    (BytePlan.semanticFieldsCheck_iff state next plan redex hRule).mpr fields
  simp [BytePlan.semanticCheck, hd, hr, he, checked]

/-- Admission exposes the complete decoded byte image, successful reservation,
exact physical execution, and every checked field separately. -/
theorem BytePlan.semanticCheck_fields {state next : PhysicalState}
    {bytes : List UInt8} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (h : BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next) :
    ∃ plan transaction, BytePlan.decode bytes = some plan ∧
      state.reserve plan arena scratch queue = some transaction ∧
      state.executeCheckedImage bytes redex = some next ∧
      plan.SemanticFields state next redex hRule := by
  unfold BytePlan.semanticCheck at h
  cases hd : BytePlan.decode bytes with
  | none => simp [hd] at h
  | some plan =>
    cases hr : state.reserve plan arena scratch queue with
    | none => simp [hd, hr] at h
    | some transaction =>
      cases he : state.executeCheckedImage bytes redex with
      | none => simp [hd, he] at h
      | some actual =>
        by_cases hc : plan.semanticFieldsCheck state actual redex hRule = true
        · simp [hd, hr, he, hc] at h
          cases h
          exact ⟨plan, transaction, rfl, hr, rfl,
            (BytePlan.semanticFieldsCheck_iff state next plan redex hRule).mp hc⟩
        · have hfalse : plan.semanticFieldsCheck state actual redex hRule = false :=
            Bool.eq_false_iff.mpr hc
          simp [hd, hr, he, hfalse] at h

/-- Successful semantic admission proves pre-write reservation, an exact
reciprocal-write inventory and a byte-level physical frame. For all old slots
outside the consumed pair, boundary peers and publication marks, the complete
physical slot (including generation, owner, ports and queue mark) is unchanged. -/
theorem BytePlan.semanticCheck_frame {state next : PhysicalState}
    {bytes : List UInt8} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (h : BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next) :
    ∃ plan, BytePlan.decode bytes = some plan ∧
      plan.arenaReservation ≤ arena ∧
      plan.scratchReservation ≤ scratch ∧
      plan.worklistReservation ≤ queue ∧
      plan.writes = plan.requiredWrites ∧
      plan.externalWrites = plan.requiredWrites ∧
      (∀ index, index < state.slots.size → index ∉ plan.touchedSlots →
        next.slots[index]? = state.slots[index]?) := by
  obtain ⟨plan, transaction, hd, hr, _, fields⟩ :=
    BytePlan.semanticCheck_fields hRule h
  have limits := PhysicalState.reserve_within_limits hr
  dsimp [BytePlan.SemanticFields] at fields
  obtain ⟨_, _, _, hwrites, hexternal, _, _, _, hframe, _⟩ := fields
  refine ⟨plan, hd, limits.1, limits.2.1, limits.2.2,
    hwrites, hexternal, ?_⟩
  intro index bound outside
  have hnot : plan.touchedSlots.contains index = false :=
    Bool.eq_false_iff.mpr (fun hc => outside (List.contains_iff_mem.mp hc))
  have checked := (List.all_eq_true.mp hframe) index (List.mem_range.mpr bound)
  simp only [hnot, Bool.false_or] at checked
  exact of_decide_eq_true checked

/-- The parser accepts this deliberately incomplete OP2 image, but the
semantic checker refuses it for every reservation bound: the second required
reciprocal output write is absent. -/
theorem BytePlan.op2_omitted_external_semantically_refused
    (arena scratch queue : Nat) :
    BytePlan.semanticCheck op2PhysicalStart op2OmittedExternalFixture.emit
      op2PhysicalRedex (by rfl) arena scratch queue = none := by
  cases h : BytePlan.semanticCheck op2PhysicalStart
      op2OmittedExternalFixture.emit op2PhysicalRedex (by rfl)
      arena scratch queue with
  | none => rfl
  | some next =>
    obtain ⟨plan, hd, _, _, _, _, hexternal, _⟩ :=
      BytePlan.semanticCheck_frame (by rfl) h
    rw [BytePlan.op2_omitted_external_decodes] at hd
    cases hd
    have impossible :
        op2OmittedExternalFixture.externalWrites ≠
          op2OmittedExternalFixture.requiredWrites := by
      decide
    exact (impossible hexternal).elim

/-- The admitted post-state has pairwise distinct live logical owners, a
nonduplicated/in-bounds free inventory that agrees with every slot's live
status, and canonical physical queue entries. Stale old generations remain
in the physical queue until popped; projection never identifies them with a
new occupant. -/
theorem BytePlan.semanticCheck_ownership {state next : PhysicalState}
    {bytes : List UInt8} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (h : BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next) :
    (next.slots.toList.filterMap fun slot => slot.agent.map Prod.fst).Nodup ∧
    next.freeSlots.Nodup ∧
    next.freeSlots.all (fun index => decide (index < next.slots.size)) = true ∧
    (List.range next.slots.size).all (fun index =>
      decide (index ∈ next.freeSlots) ==
        ((next.slots[index]?).map (fun slot => slot.agent.isNone)).getD false) = true ∧
    next.work.all (fun pair => decide (pair.first < pair.second)) = true := by
  obtain ⟨_, _, _, _, _, fields⟩ := BytePlan.semanticCheck_fields hRule h
  dsimp [BytePlan.SemanticFields] at fields
  obtain ⟨_, _, _, _, _, _, _, _, _, _, hunique, hnodup,
    hinbounds, hinventory, hcanonical, _⟩ := fields
  exact ⟨hunique, hnodup, hinbounds, hinventory, hcanonical⟩

/-- Reservation produces an unchanged snapshot. Any staged internal physical
change (including the candidate next state) is invisible to observers until
commit, and rollback restores exactly the original arena. -/
theorem BytePlan.semanticCheck_journal {state next : PhysicalState}
    {bytes : List UInt8} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (h : BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next)
    (wellFormed : state.WellFormed) :
    ∃ transaction staged : PhysicalTransaction, transaction.current = state ∧
      transaction.journal = some state ∧
      transaction.stage (fun _ => next) = some staged ∧
      staged.Related state.project ∧
      (staged.rollback).current = state := by
  obtain ⟨_, transaction, _, hr, _, _⟩ := BytePlan.semanticCheck_fields hRule h
  obtain ⟨current, journal⟩ := PhysicalState.reserve_snapshot hr
  let staged : PhysicalTransaction := { current := next, journal := some state }
  have step : transaction.stage (fun _ => next) = some staged := by
    simp [PhysicalTransaction.stage, staged, journal]
  have related : transaction.Related state.project := by
    simp [PhysicalTransaction.Related, PhysicalTransaction.visible, journal, wellFormed]
  have stutters := PhysicalTransaction.stage_stutters related step
  have rollback := (PhysicalTransaction.rollback_restores stutters).2
  exact ⟨transaction, staged, current, journal, step, stutters, by
    simpa [PhysicalTransaction.visible, staged] using rollback⟩

/-- For every semantically admitted byte image and live matched pair, the
entire physical agent/wire/free-port net is the independently compiled abstract
rewrite. No finite fixture or producer origin is assumed. -/
theorem BytePlan.semanticCheck_refines {state next : PhysicalState}
    {bytes : List UInt8} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (h : BytePlan.semanticCheck state bytes redex hRule arena scratch queue = some next)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system state.project.toNet
      redex.1 redex.2.1 redex.2.2.1 redex.2.2.2.1 redex.2.2.2.2.toRhs) :
    CStep NetI64.system state.project.toNet next.project.toNet ∧
    next.project.work.Perm
      (Plan.pendingPairs (Plan.commit state.project
        (Plan.compile state.project redex hRule) state.project.work)) ∧
    next.project.free = state.project.free := by
  obtain ⟨plan, transaction, hd, hr, he, fields⟩ :=
    BytePlan.semanticCheck_fields hRule h
  dsimp [BytePlan.SemanticFields] at fields
  obtain ⟨_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, hagents, hwires, hqueue⟩ := fields
  let expected := Plan.commit state.project
    (Plan.compile state.project redex hRule) state.project.work
  have hagentsAt (id : Id) : next.project.agents[id]? = expected.agents[id]? :=
    (Std.HashMap.Equiv.of_toList_perm hagents).getElem?_eq
  have hwiresAt (port : Port) : next.project.wires[port]? = expected.wires[port]? :=
    (Std.HashMap.Equiv.of_toList_perm hwires).getElem?_eq
  have hfree : next.project.free = state.project.free :=
    PhysicalState.executeCheckedImage_preserves_free he
  have himage : next.project.toNet = expected.toNet := by
    apply Net.ext'
    · intro id symbol
      change next.project.agents[id]? = some symbol ↔ expected.agents[id]? = some symbol
      rw [hagentsAt]
    · intro first second
      change next.project.wires[first]? = some second ↔ expected.wires[first]? = some second
      rw [hwiresAt]
    · intro port
      change port ∈ next.project.free ↔ port ∈ expected.free
      simp [hfree, expected, Plan.commit]
  exact ⟨himage ▸ Plan.commit_refines redex hRule state.project.work hm hredex,
    hqueue, hfree⟩

/-- The already decoded erasure fixture has a kernel-checked positive
semantic-admission witness; this is stronger than merely executing its
physical image or evaluating its checker result. -/
theorem BytePlan.era_fixture_admitted :
    BytePlan.semanticCheck eraPhysicalStart eraPlanBytes
      eraPhysicalRedex (by rfl) 0 256 0 = some eraPhysicalAfter := by
  let image : BytePlan :=
    { reuse := true, opcode := 4, pair := (0, 0, 1, 0), action := (0, 0),
      boundaries := [], results := [], writes := [], externalWrites := [],
      arenaReservation := 0, scratchReservation := 256,
      worklistReservation := 0, publications := [] }
  have fields :
      image.SemanticFields eraPhysicalStart eraPhysicalAfter
        eraPhysicalRedex (by rfl) := by
    have maps := PhysicalState.projected_maps_perm_of_toNet_eq
      _ _ PhysicalState.era_refines
    have observables := PhysicalState.era_compiled_observables
    dsimp only [BytePlan.SemanticFields]
    refine ⟨by decide, ?_, ?_, by decide, by decide, by decide,
      by decide, by decide, by decide, by decide, by decide, by decide,
      by decide, by decide, by decide, maps.1, maps.2, ?_⟩
    · simpa [image, BytePlan.logicalBoundary] using observables.1.symm
    · simpa [image] using observables.2.1.symm
    · change ([] : List (Id × Id)).Perm
        (Plan.pendingPairs (Plan.commit eraPhysicalStart.project
          (Plan.compile eraPhysicalStart.project eraPhysicalRedex (by rfl))
          eraPhysicalStart.project.work))
      simp only [Plan.pendingPairs, observables.2.2, List.filter_nil,
        List.perm_nil]
  exact BytePlan.semanticCheck_of_fields (by rfl)
    (by simpa only [image] using BytePlan.era_image)
    ⟨{ current := eraPhysicalStart, journal := some eraPhysicalStart }, by rfl⟩
    PhysicalState.era_execution_exact fields

/-- The complete output-connected OP2 image is admitted. In contrast, the
parser-valid image omitting one external write is rejected for every bound. -/
theorem BytePlan.op2_output_fixture_admitted :
    BytePlan.semanticCheck op2PhysicalStart op2OutputFixture.emit
      op2PhysicalRedex (by rfl) 0 256 0 = some op2PhysicalAfter := by
  have fields :
      op2OutputFixture.SemanticFields op2PhysicalStart op2PhysicalAfter
        op2PhysicalRedex (by rfl) := by
    have maps := PhysicalState.projected_maps_perm_of_toNet_eq
      _ _ PhysicalState.op2_output_refines
    have observables := PhysicalState.op2_output_compiled_observables
    have wireLookup (port : Port) :
        op2PhysicalAfter.project.wires[port]? =
          (Plan.commit op2PhysicalStart.project
            (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl))
            op2PhysicalStart.project.work).wires[port]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.2).getElem?_eq
    have agentLookup (id : Id) :
        op2PhysicalAfter.project.agents[id]? =
          (Plan.commit op2PhysicalStart.project
            (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl))
            op2PhysicalStart.project.work).agents[id]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.1).getElem?_eq
    have noOutput :
        (Plan.commit op2PhysicalStart.project
          (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl))
          op2PhysicalStart.project.work).agents[[100]]? = none := by
      rw [←agentLookup]
      simp [PhysicalState.project, PhysicalState.agents, op2PhysicalAfter,
        show child [1] [2] (some false) 0 = [0, 1] from rfl]
    have noMem :
        ¬ [100] ∈ (Plan.commit op2PhysicalStart.project
          (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl))
          op2PhysicalStart.project.work).agents := by
      simpa using noOutput
    have outputs : op2PhysicalAfter.outputs.all (fun output =>
        decide (op2PhysicalAfter.logicalPort output.target =
          (Plan.commit op2PhysicalStart.project
            (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl))
            op2PhysicalStart.project.work).wires[output.label]?)) = true := by
      apply List.all_eq_true.mpr
      intro output hmem
      have before := (List.all_eq_true.mp observables.2.2) output hmem
      simpa only [←wireLookup output.label] using before
    dsimp only [BytePlan.SemanticFields]
    refine ⟨by decide, ?_, by decide, by decide, by decide, by decide,
      by decide, by decide, by decide, outputs, by decide, by decide,
      by decide, by decide, by decide, maps.1, maps.2, ?_⟩
    · rw [observables.1]
      decide
    · change ([] : List (Id × Id)).Perm
        (Plan.pendingPairs (Plan.commit op2PhysicalStart.project
          (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl))
          op2PhysicalStart.project.work))
      simp only [Plan.pendingPairs, observables.2.1]
      simp [noMem]
  exact BytePlan.semanticCheck_of_fields (by rfl) BytePlan.op2_output_image
    ⟨{ current := op2PhysicalStart, journal := some op2PhysicalStart }, by rfl⟩
    PhysicalState.op2_output_execution_exact fields

/-- The staged wrapping subtraction image also passes independent semantic
admission at its exact arena, scratch and queue reservations. -/
theorem BytePlan.op1_output_fixture_admitted :
    BytePlan.semanticCheck op1PhysicalStart op1OutputFixture.emit
      op1PhysicalRedex (by rfl) 0 256 0 = some op1PhysicalAfter := by
  have fields :
      op1OutputFixture.SemanticFields op1PhysicalStart op1PhysicalAfter
        op1PhysicalRedex (by rfl) := by
    have maps := PhysicalState.projected_maps_perm_of_toNet_eq
      _ _ PhysicalState.op1_wrapping_refines
    have observables := PhysicalState.op1_output_compiled_observables
    have wireLookup (port : Port) :
        op1PhysicalAfter.project.wires[port]? =
          (Plan.commit op1PhysicalStart.project
            (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl))
            op1PhysicalStart.project.work).wires[port]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.2).getElem?_eq
    have agentLookup (id : Id) :
        op1PhysicalAfter.project.agents[id]? =
          (Plan.commit op1PhysicalStart.project
            (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl))
            op1PhysicalStart.project.work).agents[id]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.1).getElem?_eq
    have noOutput :
        (Plan.commit op1PhysicalStart.project
          (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl))
          op1PhysicalStart.project.work).agents[[100]]? = none := by
      rw [←agentLookup]
      simp [PhysicalState.project, PhysicalState.agents, op1PhysicalAfter,
        show child [1] [2] (some false) 0 = [0, 1] from rfl]
    have noMem :
        ¬ [100] ∈ (Plan.commit op1PhysicalStart.project
          (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl))
          op1PhysicalStart.project.work).agents := by
      simpa using noOutput
    have outputs : op1PhysicalAfter.outputs.all (fun output =>
        decide (op1PhysicalAfter.logicalPort output.target =
          (Plan.commit op1PhysicalStart.project
            (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl))
            op1PhysicalStart.project.work).wires[output.label]?)) = true := by
      apply List.all_eq_true.mpr
      intro output hmem
      have before := (List.all_eq_true.mp observables.2.2) output hmem
      simpa only [←wireLookup output.label] using before
    dsimp only [BytePlan.SemanticFields]
    refine ⟨by decide, ?_, by decide, by decide, by decide, by decide,
      by decide, by decide, by decide, outputs, by decide, by decide,
      by decide, by decide, by decide, maps.1, maps.2, ?_⟩
    · rw [observables.1]
      decide
    · change ([] : List (Id × Id)).Perm
        (Plan.pendingPairs (Plan.commit op1PhysicalStart.project
          (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl))
          op1PhysicalStart.project.work))
      simp only [Plan.pendingPairs, observables.2.1]
      simp [noMem]
  exact BytePlan.semanticCheck_of_fields (by rfl)
    BytePlan.op1_wrapping_output_image
    ⟨{ current := op1PhysicalStart, journal := some op1PhysicalStart }, by rfl⟩
    PhysicalState.op1_wrapping_execution_exact fields

/-- Both independently captured DUP outputs point to their corresponding
physical replacement number after the decoded two-result image executes. -/
theorem BytePlan.dup_output_fixture_admitted :
    BytePlan.semanticCheck dupPhysicalStart dupOutputFixture.emit
      dupPhysicalRedex (by rfl) 0 256 0 = some dupPhysicalAfter := by
  have fields :
      dupOutputFixture.SemanticFields dupPhysicalStart dupPhysicalAfter
        dupPhysicalRedex (by rfl) := by
    have maps := PhysicalState.projected_maps_perm_of_toNet_eq
      _ _ PhysicalState.dup_refines
    have observables := PhysicalState.dup_output_compiled_observables
    have wireLookup (port : Port) :
        dupPhysicalAfter.project.wires[port]? =
          (Plan.commit dupPhysicalStart.project
            (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
            dupPhysicalStart.project.work).wires[port]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.2).getElem?_eq
    have agentLookup (id : Id) :
        dupPhysicalAfter.project.agents[id]? =
          (Plan.commit dupPhysicalStart.project
            (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
            dupPhysicalStart.project.work).agents[id]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.1).getElem?_eq
    have noOutput (id : Id) (hid : id = [100] ∨ id = [101]) :
        (Plan.commit dupPhysicalStart.project
          (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
          dupPhysicalStart.project.work).agents[id]? = none := by
      rw [←agentLookup]
      rcases hid with h | h <;> subst id <;>
        simp [PhysicalState.project, PhysicalState.agents, dupPhysicalAfter,
          show child [1] [2] (some false) 0 = [0, 1] from rfl,
          show child [1] [2] (some false) 1 = [2, 1] from rfl]
    have noMem100 :
        ¬ [100] ∈ (Plan.commit dupPhysicalStart.project
          (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
          dupPhysicalStart.project.work).agents := by
      simpa using noOutput [100] (Or.inl rfl)
    have noMem101 :
        ¬ [101] ∈ (Plan.commit dupPhysicalStart.project
          (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
          dupPhysicalStart.project.work).agents := by
      simpa using noOutput [101] (Or.inr rfl)
    have outputs : dupPhysicalAfter.outputs.all (fun output =>
        decide (dupPhysicalAfter.logicalPort output.target =
          (Plan.commit dupPhysicalStart.project
            (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
            dupPhysicalStart.project.work).wires[output.label]?)) = true := by
      apply List.all_eq_true.mpr
      intro output hmem
      have before := (List.all_eq_true.mp observables.2.2) output hmem
      simpa only [←wireLookup output.label] using before
    dsimp only [BytePlan.SemanticFields]
    refine ⟨by decide, ?_, by decide, by decide, by decide, by decide,
      by decide, by decide, by decide, outputs, by decide, by decide,
      by decide, by decide, by decide, maps.1, maps.2, ?_⟩
    · rw [observables.1]
      decide
    · change ([] : List (Id × Id)).Perm
        (Plan.pendingPairs (Plan.commit dupPhysicalStart.project
          (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
          dupPhysicalStart.project.work))
      simp only [Plan.pendingPairs, observables.2.1]
      simp [noMem100, noMem101]
  exact BytePlan.semanticCheck_of_fields (by rfl) BytePlan.dup_output_image
    ⟨{ current := dupPhysicalStart, journal := some dupPhysicalStart }, by rfl⟩
    PhysicalState.dup_execution_exact fields

/-- An actually newly active external peer keeps both directed queue entries
under admission; the output-only cases above filter non-agent candidates. -/
theorem BytePlan.op2_published_fixture_admitted :
    BytePlan.semanticCheck op2PublishedStart op2PublishedFixture.emit
      op2PhysicalRedex (by rfl) 0 256 1 = some op2PublishedAfter := by
  have fields :
      op2PublishedFixture.SemanticFields op2PublishedStart op2PublishedAfter
        op2PhysicalRedex (by rfl) := by
    have maps := PhysicalState.projected_maps_perm_of_toNet_eq
      _ _ PhysicalState.op2_published_refines.1
    have observables := PhysicalState.op2_published_compiled_observables
    have wireLookup (port : Port) :
        op2PublishedAfter.project.wires[port]? =
          (Plan.commit op2PublishedStart.project
            (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
            op2PublishedStart.project.work).wires[port]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.2).getElem?_eq
    have agentLookup (id : Id) :
        op2PublishedAfter.project.agents[id]? =
          (Plan.commit op2PublishedStart.project
            (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
            op2PublishedStart.project.work).agents[id]? :=
      (Std.HashMap.Equiv.of_toList_perm maps.1).getElem?_eq
    have liveChild :
        ((Plan.commit op2PublishedStart.project
          (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
          op2PublishedStart.project.work).agents[
            child [1] [2] (some false) 0]?).isSome = true := by
      rw [←agentLookup]
      simp [PhysicalState.project, PhysicalState.agents, op2PublishedAfter,
        show child [1] [2] (some false) 0 = [0, 1] from rfl]
    have livePeer :
        ((Plan.commit op2PublishedStart.project
          (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
          op2PublishedStart.project.work).agents[[3]]?).isSome = true := by
      rw [←agentLookup]
      simp [PhysicalState.project, PhysicalState.agents, op2PublishedAfter,
        show child [1] [2] (some false) 0 = [0, 1] from rfl]
    have liveChildMem :
        child [1] [2] (some false) 0 ∈
          (Plan.commit op2PublishedStart.project
            (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
            op2PublishedStart.project.work).agents := by
      simpa using liveChild
    have livePeerMem :
        [3] ∈ (Plan.commit op2PublishedStart.project
          (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
          op2PublishedStart.project.work).agents := by
      simpa using livePeer
    have outputs : op2PublishedAfter.outputs.all (fun output =>
        decide (op2PublishedAfter.logicalPort output.target =
          (Plan.commit op2PublishedStart.project
            (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
            op2PublishedStart.project.work).wires[output.label]?)) = true := by
      apply List.all_eq_true.mpr
      intro output hmem
      have before := (List.all_eq_true.mp observables.2.2) output hmem
      simpa only [←wireLookup output.label] using before
    dsimp only [BytePlan.SemanticFields]
    refine ⟨by decide, ?_, by decide, by decide, by decide, by decide,
      by decide, by decide, by decide, outputs, by decide, by decide,
      by decide, by decide, by decide, maps.1, maps.2, ?_⟩
    · rw [observables.1]
      decide
    · have filtered :
          Plan.pendingPairs (Plan.commit op2PublishedStart.project
            (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
            op2PublishedStart.project.work) =
          (Plan.commit op2PublishedStart.project
            (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
            op2PublishedStart.project.work).work := by
        simp [Plan.pendingPairs, observables.2.1, liveChildMem, livePeerMem]
      rw [filtered]
      exact PhysicalState.op2_published_refines.2
  exact BytePlan.semanticCheck_of_fields (by rfl) BytePlan.op2_published_image
    ⟨{ current := op2PublishedStart, journal := some op2PublishedStart }, by rfl⟩
    PhysicalState.op2_published_execution_exact fields

end NobleFoundation.NetI64Plans
