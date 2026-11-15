import NobleFoundation.NetI64PlanChecked

/-!
# Model lowering from selected physical pair to version-one plan bytes

This is an executable model of the sequential compiler's four selected
rule shapes and two storage policies. It derives symbols from the selected
revision-3 right-hand side, but derives endpoints, slot generations, output
indices and publications from physical state. It refuses unsupported aliases,
stale pairs, malformed reciprocal peers, counter wrap and unencodable bytes.
It is not a Rust implementation-refinement theorem.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- Capture the physical target of a selected auxiliary port. A missing
physical slot edge represents a unique stable output; a present slot edge
must have a reciprocal peer and cannot simultaneously be an output. -/
def PhysicalState.captureAux (state : PhysicalState)
    (slot port : Nat) : Option ((Nat × Nat) × (Nat × Nat × Nat)) := do
  let owner ← state.slots[slot]?
  if owner.agent.isNone then none else do
    let target ← owner.ports[port]?
    match target with
    | some (peer, peerPort) => do
        if state.outputs.any (fun output => output.target == (slot, port)) then none else do
          let other ← state.slots[peer]?
          let (_, symbol) ← other.agent
          if peerPort > NetI64.arity symbol ||
              other.ports[peerPort]? != some (some (slot, port))
          then none else some ((slot, port), (0, peer, peerPort))
    | none =>
        match state.outputs.zipIdx.filter (fun entry => entry.1.target == (slot, port)) with
        | [(_, index)] => some ((slot, port), (1, index, 0))
        | _ => none

/-- One generated physical result can publish an agent-agent principal pair;
connections to an output or nonprincipal peer are not executable pairs. -/
def PhysicalState.publicationFor (state : PhysicalState) (pair : PhysicalPair)
    (boundaries : List ((Nat × Nat) × (Nat × Nat × Nat)))
    (result : Bool × Nat × NetSym × Nat × List (Option Nat)) :
    Option (List (Nat × Nat × Nat × Nat)) := do
  let some index ← result.2.2.2.2[0]? | none
  let boundary ← boundaries[index]?
  let (kind, peer, peerPort) := boundary.2
  if kind != 0 || peerPort != 0 then some [] else do
    if peer == pair.first || peer == pair.second || peer == result.2.1
    then none else do
      let other ← state.slots[peer]?
      if other.agent.isNone then none else do
        let generation ← if result.1 then some 0 else do
          let old ← state.slots[result.2.1]?
          if old.generation == UInt64.ofNat (UInt64.size - 1)
          then none else some (old.generation.toNat + 1)
        let physical := result.2.1
        let current := other.generation.toNat
        if physical < peer then some [(physical, generation, peer, current)]
        else some [(peer, current, physical, generation)]

/-- Construct all physical fields from a matched pair; a separate admission
step rejects any image that the version-one decoder cannot read back. -/
private def PhysicalState.planCandidate (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) : Option BytePlan := do
  let rhs ← NetI64.xRule redex.2.2.1 redex.2.2.2.1
  if state.logicalPair pair != some (redex.1, redex.2.1) then none else do
    let first ← state.slots[pair.first]?
    let second ← state.slots[pair.second]?
    if first.agent.map Prod.snd != some redex.2.2.1 ||
        second.agent.map Prod.snd != some redex.2.2.2.1 ||
        first.ports[0]? != some (some (pair.second, 0)) ||
        second.ports[0]? != some (some (pair.first, 0)) ||
        first.generation == UInt64.ofNat (UInt64.size - 1) ||
        second.generation == UInt64.ofNat (UInt64.size - 1)
    then none else do
      let opcode ← BytePlan.ruleOpcode redex.2.2.1 redex.2.2.2.1
      let (ruleSlot, valueSlot) ← match redex.2.2.1, redex.2.2.2.1 with
        | .op2 _, .num _ | .op1 _ _, .num _ | .dup, .num _ | .era, .num _ =>
            some (pair.first, pair.second)
        | .num _, .op2 _ | .num _, .op1 _ _ | .num _, .dup | .num _, .era =>
            some (pair.second, pair.first)
        | _, _ => none
      let boundaryPorts := if opcode == 1 || opcode == 3 then [1, 2]
        else if opcode == 2 then [1] else []
      let boundaries ← boundaryPorts.mapM (state.captureAux ruleSlot)
      if boundaries.any (fun boundary => boundary.2.1 == 0 &&
          (boundary.2.2.1 == pair.first || boundary.2.2.1 == pair.second)) ||
          !(decide (boundaries.map Prod.snd).Nodup)
      then none else do
        let symbols ← rhs.slots.mapM (fun slot => rhs.lab slot.1 slot.2)
        if symbols.length != (if opcode == 3 then 2 else if opcode == 4 then 0 else 1)
        then none else do
          let donors := if opcode == 3 then [ruleSlot, valueSlot]
            else if opcode == 4 then [] else [ruleSlot]
          let requested := if opcode == 1 then [[some 0, some 1, none]]
            else if opcode == 2 then [[some 0, none, none]]
            else if opcode == 3 then [[some 0, none, none], [some 1, none, none]]
            else []
          let results ← (symbols.zipIdx).mapM fun (symbol, index) => do
            let donor ← donors[index]?
            let ports ← requested[index]?
            let slot := if reuse then donor else state.slots.size + index
            some (not reuse, slot, symbol, donor, ports)
          let reused := if reuse then donors else []
          let action : UInt8 × UInt8 :=
            (if reused.contains pair.first then 1 else 0,
             if reused.contains pair.second then 1 else 0)
          let writes := BytePlan.requiredWrites
            { reuse, opcode, pair := (pair.first, pair.firstGeneration.toNat,
                pair.second, pair.secondGeneration.toNat), action,
              boundaries, results, writes := [], externalWrites := [],
              arenaReservation := 0, scratchReservation := 256,
              worklistReservation := 0, publications := [] }
          let publications ← results.flatMapM (state.publicationFor pair boundaries)
          let plan : BytePlan :=
            { reuse, opcode, pair := (pair.first, pair.firstGeneration.toNat,
                pair.second, pair.secondGeneration.toNat), action,
              boundaries, results, writes, externalWrites := writes,
              arenaReservation := (results.filter fun result => result.1).length,
              scratchReservation := 256,
              worklistReservation := publications.length, publications }
          some plan

/-- Lower one physically matched revision-3 rule to the actual version-one
byte representation. The rule supplies result symbols; the arena supplies
endpoints, generations, output indices and publications. `reuse` selects
consumed-slot reuse or fresh append in the same order as Rust. A plan that
cannot roundtrip through the independent byte decoder is refused. -/
def PhysicalState.lowerPlan (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) : Option BytePlan := do
  let plan ← state.planCandidate pair redex reuse
  if BytePlan.decode plan.emit == some plan then some plan else none

/-- Every emitted model image is parsed back byte-for-byte before return;
this does not imply the stronger physical semantic checker accepts it. -/
theorem PhysicalState.lowerPlan_decodes {state : PhysicalState}
    {pair : PhysicalPair} {redex : XRedex NetSym} {reuse : Bool} {plan : BytePlan}
    (h : state.lowerPlan pair redex reuse = some plan) :
    BytePlan.decode plan.emit = some plan := by
  unfold PhysicalState.lowerPlan at h
  cases hp : state.planCandidate pair redex reuse with
  | none => simp [hp] at h
  | some candidate =>
    by_cases hd : BytePlan.decode candidate.emit = some candidate
    · simp [hp, hd] at h
      cases h
      exact hd
    · simp [hp, hd] at h

/-- Exact bytes from the model lowering, with decoder roundtrip checked
before return. These are comparable byte-for-byte to a Rust-emitted stream. -/
def PhysicalState.lowerBytes (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) : Option (List UInt8) :=
  (state.lowerPlan pair redex reuse).map BytePlan.emit

/-- The physical emitter needs a *separate* completeness proof: decoding a
model-emitted image and observing that admission succeeded imply one whole-net
rewrite, but the decoder alone does not establish semantic admission. -/
theorem PhysicalState.lowerPlan_refines_if_checked {state next : PhysicalState}
    {pair : PhysicalPair} {redex : XRedex NetSym} {reuse : Bool}
    {plan : BytePlan} {arena scratch queue : Nat}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hChecked : BytePlan.semanticCheck state plan.emit redex hRule
      arena scratch queue = some next)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system state.project.toNet
      redex.1 redex.2.1 redex.2.2.1 redex.2.2.2.1 redex.2.2.2.2.toRhs) :
    BytePlan.decode plan.emit = some plan ∧
    CStep NetI64.system state.project.toNet next.project.toNet ∧
    next.project.work.Perm
      (Plan.pendingPairs (Plan.commit state.project
        (Plan.compile state.project redex hRule) state.project.work)) ∧
    next.project.free = state.project.free := by
  exact ⟨PhysicalState.lowerPlan_decodes hLower,
    BytePlan.semanticCheck_refines hRule hChecked hm hredex⟩

/-- The independently modeled five physical layouts lower to precisely
their original byte images, including the live-peer publication case. -/
theorem PhysicalState.five_fixture_images_from_lowering :
    op2PhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
      op2PhysicalRedex true = some op2OutputFixture.emit ∧
    op2PublishedStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
      op2PhysicalRedex true = some op2PublishedFixture.emit ∧
    op1PhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
      op1PhysicalRedex true = some op1OutputFixture.emit ∧
    dupPhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
      dupPhysicalRedex true = some dupOutputFixture.emit ∧
    eraPhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
      eraPhysicalRedex true = some eraPlanBytes := by
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  all_goals set_option maxRecDepth 3000 in decide

/-- Five actual lowered model byte streams pass full semantic admission;
this does not establish completeness for all physically representable nets. -/
theorem PhysicalState.five_lowered_images_accepted :
    (∃ bytes, op2PhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
        op2PhysicalRedex true = some bytes ∧
      BytePlan.semanticCheck op2PhysicalStart bytes op2PhysicalRedex
        (by rfl) 0 256 0 = some op2PhysicalAfter) ∧
    (∃ bytes, op2PublishedStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
        op2PhysicalRedex true = some bytes ∧
      BytePlan.semanticCheck op2PublishedStart bytes op2PhysicalRedex
        (by rfl) 0 256 1 = some op2PublishedAfter) ∧
    (∃ bytes, op1PhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
        op1PhysicalRedex true = some bytes ∧
      BytePlan.semanticCheck op1PhysicalStart bytes op1PhysicalRedex
        (by rfl) 0 256 0 = some op1PhysicalAfter) ∧
    (∃ bytes, dupPhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
        dupPhysicalRedex true = some bytes ∧
      BytePlan.semanticCheck dupPhysicalStart bytes dupPhysicalRedex
        (by rfl) 0 256 0 = some dupPhysicalAfter) ∧
    (∃ bytes, eraPhysicalStart.lowerBytes
      { first := 0, firstGeneration := 0, second := 1, secondGeneration := 0 }
        eraPhysicalRedex true = some bytes ∧
      BytePlan.semanticCheck eraPhysicalStart bytes eraPhysicalRedex
        (by rfl) 0 256 0 = some eraPhysicalAfter) := by
  rcases PhysicalState.five_fixture_images_from_lowering with
    ⟨op2, published, op1, dup, era⟩
  exact ⟨⟨op2OutputFixture.emit, op2, BytePlan.op2_output_fixture_accepted⟩,
    ⟨op2PublishedFixture.emit, published, BytePlan.op2_published_fixture_accepted⟩,
    ⟨op1OutputFixture.emit, op1, BytePlan.op1_output_fixture_accepted⟩,
    ⟨dupOutputFixture.emit, dup, BytePlan.dup_output_fixture_accepted⟩,
    ⟨eraPlanBytes, era, BytePlan.era_fixture_accepted⟩⟩

/-- Same abstract rule and active pair, but the first physical generation
has reached its finite maximum; no retirement or reuse can increment it. -/
def overflowOp2Start : PhysicalState :=
  { op2PhysicalStart with
    slots := op2PhysicalStart.slots.setIfInBounds 0
      { agent := some ([1], .op2 .add)
        generation := UInt64.ofNat (UInt64.size - 1)
        queuedGeneration := none
        ports := [some (1, 0), none, none] } }

def overflowOp2Pair : PhysicalPair :=
  { first := 0, firstGeneration := UInt64.ofNat (UInt64.size - 1),
    second := 1, secondGeneration := 0 }

/-- Abstract matching plus a live physical pair does not imply a legal
bounded plan: finite-generation headroom is an independent prerequisite. -/
theorem PhysicalState.overflow_pair_refuses_both_modes :
    overflowOp2Start.logicalPair overflowOp2Pair = some ([1], [2]) ∧
    overflowOp2Start.lowerPlan overflowOp2Pair op2PhysicalRedex true = none ∧
    overflowOp2Start.lowerPlan overflowOp2Pair op2PhysicalRedex false = none := by
  refine ⟨?_, ?_, ?_⟩
  all_goals set_option maxRecDepth 3000 in decide

end NobleFoundation.NetI64Plans
