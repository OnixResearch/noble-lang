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

/-- Finite generation headroom is a per-pair resource, not a persistent
global state invariant: an accepted rewrite may consume the final increment. -/
def PhysicalState.pairHeadroom (state : PhysicalState) (pair : PhysicalPair) : Bool :=
  match state.slots[pair.first]?, state.slots[pair.second]? with
  | some first, some second =>
      first.generation != UInt64.ofNat (UInt64.size - 1) &&
        second.generation != UInt64.ofNat (UInt64.size - 1)
  | _, _ => false

/-- Deliberately excluded checked-DAG boundary: a selected rule auxiliary
edge loops into either consumed physical slot. -/
def PhysicalState.consumedBoundaryAlias (state : PhysicalState)
    (pair : PhysicalPair) (redex : INet.XRedex NetSym) : Bool :=
  let (slot, symbol) := match redex.2.2.1 with
    | .num _ => (pair.second, redex.2.2.2.1)
    | symbol => (pair.first, symbol)
  let ports := (List.range (NetI64.arity symbol)).map (· + 1)
  ports.any fun port =>
    match (state.slots[slot]?).bind (fun owner => owner.ports[port]?) with
    | some (some (peer, _)) => peer == pair.first || peer == pair.second
    | _ => false

/-- Fill both reciprocal write inventories from exactly the initialized
result ports; this helper is the only place the model constructs write lists. -/
private def BytePlan.withRequiredWrites (plan : BytePlan) : BytePlan :=
  let writes := plan.requiredWrites
  { plan with writes, externalWrites := writes }

/-- Every image assembled through the result-port helper has exact and
identical outgoing and reciprocal write inventories. -/
theorem BytePlan.withRequiredWrites_exact (plan : BytePlan) :
    (plan.withRequiredWrites).writes =
      (plan.withRequiredWrites).requiredWrites ∧
    (plan.withRequiredWrites).externalWrites =
      (plan.withRequiredWrites).requiredWrites := by
  simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]

/-- The selected rule agent and consumed number slot in either principal
orientation; unsupported rule pairs have no physical result assignment. -/
private def BytePlan.ruleSlots (left right : NetSym)
    (pair : PhysicalPair) : Option (Nat × Nat) :=
  match left, right with
  | .op2 _, .num _ | .op1 _ _, .num _ | .dup, .num _ | .era, .num _ =>
      some (pair.first, pair.second)
  | .num _, .op2 _ | .num _, .op1 _ _ | .num _, .dup | .num _, .era =>
      some (pair.second, pair.first)
  | _, _ => none

/-- Once a selected rule and orientation have been decoded, construct the
ordered boundary and all physical fields without inspecting a ghost net. -/
private def PhysicalState.candidateBody (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) (rhs : XRhs NetSym)
    (opcode : UInt8) (ruleSlot valueSlot : Nat) : Option BytePlan := do
  let boundaryPorts := if opcode == 1 || opcode == 3 then [1, 2]
    else if opcode == 2 then [1] else []
  let boundaries ← boundaryPorts.mapM (state.captureAux ruleSlot)
  if state.consumedBoundaryAlias pair redex ||
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
      let publications ← results.flatMapM (state.publicationFor pair boundaries)
      let plan : BytePlan :=
        { reuse, opcode, pair := (pair.first, pair.firstGeneration.toNat,
            pair.second, pair.secondGeneration.toNat), action,
          boundaries, results, writes := [], externalWrites := [],
          arenaReservation := (results.filter fun result => result.1).length,
          scratchReservation := 256,
          worklistReservation := publications.length, publications }
      some plan

/-- A successful ordered option traversal retains a projection of every
input element when the constructor retains it pointwise. -/
private theorem mapM_preserves_projection {α β γ : Type}
    (inputs : List α) (f : α → Option β) (source : α → γ)
    (target : β → γ)
    (pointwise : ∀ input output, f input = some output →
      target output = source input)
    {outputs : List β} (h : inputs.mapM f = some outputs) :
    outputs.map target = inputs.map source := by
  induction inputs generalizing outputs with
  | nil =>
    simp at h
    cases h
    rfl
  | cons input rest ih =>
    simp only [List.mapM_cons] at h
    obtain ⟨output, head, h⟩ := Option.bind_eq_some_iff.mp h
    obtain ⟨tail, restResult, h⟩ := Option.bind_eq_some_iff.mp h
    simp at h
    cases h
    simp [pointwise input output head, ih restResult]

/-- A total RHS slot traversal has exactly the symbols retained by the
compiled result image; child identities do not affect symbol ordering. -/
private theorem mapM_filterMap_symbols {α β γ : Type}
    (xs : List α) (f : α → Option β)
    (g : α → β → γ) (project : γ → β)
    (hproject : ∀ x y, project (g x y) = y) {ys : List β}
    (h : xs.mapM f = some ys) :
    (xs.filterMap (fun x => (f x).map (g x))).map project = ys := by
  induction xs generalizing ys with
  | nil =>
    simp at h
    cases h
    rfl
  | cons x xs ih =>
    simp only [List.mapM_cons] at h
    obtain ⟨head, hhead, h⟩ := Option.bind_eq_some_iff.mp h
    obtain ⟨tail, htail, h⟩ := Option.bind_eq_some_iff.mp h
    simp at h
    cases h
    simp [hhead, hproject, ih htail]

/-- Every total selected RHS slot labeling produces the abstract compiler's
result symbols in exactly that order, for either orientation of every rule. -/
theorem PhysicalState.rule_result_symbols {H : HNet NetSym}
    {redex : XRedex NetSym} {symbols : List NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 =
      some redex.2.2.2.2)
    (h : redex.2.2.2.2.slots.mapM
      (fun slot => redex.2.2.2.2.lab slot.1 slot.2) = some symbols) :
    (Plan.compile H redex hRule).initializedAgents.map Prod.snd =
      symbols := by
  have hm := mapM_filterMap_symbols redex.2.2.2.2.slots
    (fun slot => redex.2.2.2.2.lab slot.1 slot.2)
    (fun slot symbol =>
      (child redex.1 redex.2.1 slot.1 slot.2, symbol)) Prod.snd
    (by intros; rfl) h
  simpa [Plan.compile, localResult, XNet.fire, HNet.nbhd] using hm

/-- An abstract selected rule with two zero-arity agents has no auxiliary
capture, regardless of principal-wire targets or map insertion order. -/
theorem Plan.zero_arity_boundary
    (H : HNet NetSym) (redex : XRedex NetSym)
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 =
      some redex.2.2.2.2)
    (hfirst : NetI64.arity redex.2.2.1 = 0)
    (hsecond : NetI64.arity redex.2.2.2.1 = 0) :
    ((Plan.compile H redex hRule).capturedBoundary.filter
      fun edge => edge.1.2 != 0) = [] := by
  simp [Plan.compile, HNet.nearWires, portsOf, hfirst, hsecond]
  intro a1 b1 a2 b2 h
  rcases h with ⟨_, _, _, ⟨_, hz⟩, _, _⟩ |
    ⟨_, _, _, ⟨_, hz⟩, _, _⟩ <;> omega

/-- Empty physical captures of a zero-arity pair equal the abstract
compiler's empty nonprincipal boundary without inspecting a ghost net. -/
theorem PhysicalState.lowered_zero_arity_boundary
    {state : PhysicalState} {redex : XRedex NetSym}
    {plan : BytePlan}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 =
      some redex.2.2.2.2)
    (hfirst : NetI64.arity redex.2.2.1 = 0)
    (hsecond : NetI64.arity redex.2.2.2.1 = 0)
    (hboundaries : plan.boundaries = []) :
    plan.logicalBoundary state = some
      ((Plan.compile state.project redex hRule).capturedBoundary.filter
        fun edge => edge.1.2 != 0) := by
  rw [Plan.zero_arity_boundary state.project redex hRule hfirst hsecond]
  simp [BytePlan.logicalBoundary, hboundaries]

/-- Resolve a single captured output boundary against the physical prestate.
This uses the output's stable logical label rather than its mutable target. -/
theorem BytePlan.single_output_logical_boundary
    {state : PhysicalState} {plan : BytePlan}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {slot output : Nat} {logical : Id} {old : PhysicalOutput}
    (hboundaries : plan.boundaries = [boundary])
    (hsource : boundary.1 = (slot, 1))
    (hkind : boundary.2 = (1, output, 0))
    (hleft : state.logicalPort (slot, 1) = some (logical, 1))
    (houtput : state.outputs[output]? = some old) :
    plan.logicalBoundary state = some [((logical, 1), old.label)] := by
  simp [BytePlan.logicalBoundary, hboundaries, hsource,
    hkind, hleft, houtput]

/-- Only OP1's one auxiliary port survives the independent abstract
near-wires capture, in either selected principal orientation. -/
theorem Plan.op1_forward_output_boundary
    (H : HNet NetSym) (a b : Id)
    (op : Op) (firstValue secondValue : BitVec 64)
    (label : Port)
    (hwire : H.wires[(a, 1)]? = some label) :
    ((Plan.compile H
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX)
      (by rfl)).capturedBoundary.filter fun edge => edge.1.2 != 0) =
        [((a, 1), label)] := by
  cases ha : H.wires[(a, 0)]? <;>
    cases hb : H.wires[(b, 0)]? <;>
    simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity,
      List.range_succ, ha, hb, hwire]
  all_goals rfl

theorem Plan.op1_reverse_output_boundary
    (H : HNet NetSym) (a b : Id)
    (op : Op) (firstValue secondValue : BitVec 64)
    (label : Port)
    (hwire : H.wires[(b, 1)]? = some label) :
    ((Plan.compile H
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap)
      (by rfl)).capturedBoundary.filter fun edge => edge.1.2 != 0) =
        [((b, 1), label)] := by
  cases ha : H.wires[(a, 0)]? <;>
    cases hb : H.wires[(b, 0)]? <;>
    simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity,
      List.range_succ, ha, hb, hwire]

/-- Derive the opcode, ordered physical capture and all reservation counters
from the builder's inputs, not from the later semantic-admission predicate. -/
theorem PhysicalState.candidateBody_fields
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool} {rhs : XRhs NetSym}
    {opcode : UInt8} {ruleSlot valueSlot : Nat} {plan : BytePlan}
    (h : state.candidateBody pair redex reuse rhs opcode ruleSlot valueSlot =
      some plan) :
    plan.opcode = opcode ∧
    (∃ boundaries,
      (if opcode == 1 || opcode == 3 then [1, 2]
        else if opcode == 2 then [1] else []).mapM
          (state.captureAux ruleSlot) = some boundaries ∧
      plan.boundaries = boundaries) ∧
    (∃ symbols, rhs.slots.mapM (fun slot => rhs.lab slot.1 slot.2) =
      some symbols ∧
      plan.results.map (fun result => result.2.2.1) = symbols) ∧
    plan.arenaReservation = (plan.results.filter fun result => result.1).length ∧
    plan.scratchReservation = 256 ∧
    plan.worklistReservation = plan.publications.length := by
  unfold PhysicalState.candidateBody at h
  obtain ⟨boundaries, hb, h⟩ := Option.bind_eq_some_iff.mp h
  by_cases ha : (state.consumedBoundaryAlias pair redex ||
      !(decide (boundaries.map Prod.snd).Nodup)) = true
  · simp [ha] at h
  · simp [ha] at h
    obtain ⟨symbols, hs, h⟩ := Option.bind_eq_some_iff.mp h
    simp at h
    obtain ⟨_, h⟩ := h
    obtain ⟨results, hr, h⟩ := Option.bind_eq_some_iff.mp h
    obtain ⟨publications, hp, h⟩ := Option.bind_eq_some_iff.mp h
    cases Option.some.inj h
    have hsymbols : results.map (fun result => result.2.2.1) =
        symbols := by
      let f : NetSym × Nat →
          Option (Bool × Nat × NetSym × Nat × List (Option Nat)) :=
        fun (symbol, index) => do
          let donor ← (if opcode == 3 then [ruleSlot, valueSlot]
            else if opcode == 4 then [] else [ruleSlot])[index]?
          let ports ← (if opcode == 1 then [[some 0, some 1, none]]
            else if opcode == 2 then [[some 0, none, none]]
            else if opcode == 3 then [[some 0, none, none], [some 1, none, none]]
            else [])[index]?
          some (not reuse, if reuse then donor else state.slots.size + index,
            symbol, donor, ports)
      have hmap : symbols.zipIdx.mapM f = some results := by
        simpa [f] using hr
      have hm := mapM_preserves_projection symbols.zipIdx f
        Prod.fst (fun result => result.2.2.1) (by
          intro ⟨symbol, index⟩ output ho
          dsimp [f] at ho
          obtain ⟨donor, _, ho⟩ := Option.bind_eq_some_iff.mp ho
          obtain ⟨ports, _, ho⟩ := Option.bind_eq_some_iff.mp ho
          cases Option.some.inj ho
          rfl) hmap
      simpa using hm
    exact ⟨rfl, ⟨boundaries, hb, rfl⟩, ⟨symbols, hs, hsymbols⟩,
      rfl, rfl, rfl⟩

/-- Construct all physical fields from a matched pair; the final wrapper
derives the two write inventories from the initialized result ports. -/
private def PhysicalState.planCandidateCore (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) : Option BytePlan := do
  let rhs ← NetI64.xRule redex.2.2.1 redex.2.2.2.1
  if state.logicalPair pair != some (redex.1, redex.2.1) then none else do
    let first ← state.slots[pair.first]?
    let second ← state.slots[pair.second]?
    if first.agent.map Prod.snd != some redex.2.2.1 ||
        second.agent.map Prod.snd != some redex.2.2.2.1 ||
        first.ports[0]? != some (some (pair.second, 0)) ||
        second.ports[0]? != some (some (pair.first, 0))
    then none else do
      let opcode ← BytePlan.ruleOpcode redex.2.2.1 redex.2.2.2.1
      let (ruleSlot, valueSlot) ←
        BytePlan.ruleSlots redex.2.2.1 redex.2.2.2.1 pair
      state.candidateBody pair redex reuse rhs opcode ruleSlot valueSlot

/-- Materialize both ordered wire inventories before checking the version-one
byte decoder. There is no second, independent implementation of the writes. -/
private def PhysicalState.planCandidate (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) : Option BytePlan :=
  (state.planCandidateCore pair redex reuse).map BytePlan.withRequiredWrites

/-- Erasure has no auxiliary capture, result installation, reciprocal write or
publication. A current principal pair with the stated physical symbols
therefore always produces an erasure candidate, in either storage policy. -/
theorem PhysicalState.era_forward_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64} {reuse : Bool}
    {first second : PhysicalSlot}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some .era)
    (hsa : second.agent.map Prod.snd = some (.num value))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0))) :
    ∃ plan, state.planCandidate pair
      (a, b, .era, .num value, eraNumData.toX) reuse = some plan ∧
      plan.results = [] ∧ plan.boundaries = [] ∧
      plan.externalWrites = [] ∧
      plan.publications = [] ∧ plan.action = (0, 0) ∧
      plan.pair = (pair.first, pair.firstGeneration.toNat,
        pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    PhysicalState.consumedBoundaryAlias, NetI64.arity,
    eraNumData, RuleData.toX, RuleData.ofWires]
  simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]

/-- The reverse erasure orientation has the same empty physical image and
requires only the corresponding current principal and symbol fields. -/
theorem PhysicalState.era_reverse_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64} {reuse : Bool}
    {first second : PhysicalSlot}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num value))
    (hsa : second.agent.map Prod.snd = some .era)
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0))) :
    ∃ plan, state.planCandidate pair
      (a, b, .num value, .era, eraNumData.toX.swap) reuse = some plan ∧
      plan.results = [] ∧ plan.boundaries = [] ∧
      plan.externalWrites = [] ∧
      plan.publications = [] ∧ plan.action = (0, 0) ∧
      plan.pair = (pair.first, pair.firstGeneration.toNat,
        pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    PhysicalState.consumedBoundaryAlias, NetI64.arity,
    eraNumData, RuleData.toX, RuleData.ofWires, XRhs.swap]
  simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]

/-- A forward one-operand rule has exactly one captured auxiliary endpoint,
one result symbol, and one potential physical publication. The capture and
publication premises inspect only the prestate, not the semantic checker. -/
theorem PhysicalState.op1_forward_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.op1 op firstValue))
    (hsa : second.agent.map Prod.snd = some (.num secondValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.first 1 = some boundary)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX) = false)
    (hpub : state.publicationFor pair [boundary]
      (not reuse, if reuse then pair.first else state.slots.size,
        .num (op.apply firstValue secondValue), pair.first,
        [some 0, none, none]) = some published) :
    ∃ plan, state.planCandidate pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX) reuse = some plan := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap, hpub,
    op1NumData, RuleData.toX, RuleData.ofWires, assoc]
  refine ⟨_, _, ⟨?_, rfl⟩, rfl⟩
  simpa [op1NumData, RuleData.toX, RuleData.ofWires, assoc]
    using halias

/-- A selected forward OP1 candidate fixes the one-result physical image,
including its single reciprocal boundary write and storage action. -/
theorem PhysicalState.op1_forward_candidate_shape
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.op1 op firstValue))
    (hsa : second.agent.map Prod.snd = some (.num secondValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.first 1 = some boundary)
    (hpub : state.publicationFor pair [boundary]
      (not reuse, if reuse then pair.first else state.slots.size,
        .num (op.apply firstValue secondValue), pair.first,
        [some 0, none, none]) = some published)
    (hc : state.planCandidate pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX) reuse = some plan) :
    plan.results =
      [(not reuse, if reuse then pair.first else state.slots.size,
        .num (op.apply firstValue secondValue), pair.first,
        [some 0, none, none])] ∧
    plan.boundaries = [boundary] ∧
    plan.externalWrites = [(0, 0, 0)] ∧
    plan.publications = published ∧
    plan.action = (if reuse then 1 else 0, 0) ∧
    plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore at hc
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap, hpub,
    op1NumData, RuleData.toX, RuleData.ofWires, assoc] at hc
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

/-- The mirrored one-operand rule captures the second physical slot's
auxiliary port while keeping the single result and publication order. -/
theorem PhysicalState.op1_reverse_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num secondValue))
    (hsa : second.agent.map Prod.snd = some (.op1 op firstValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.second 1 = some boundary)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap) = false)
    (hpub : state.publicationFor pair [boundary]
      (not reuse, if reuse then pair.second else state.slots.size,
        .num (op.apply firstValue secondValue), pair.second,
        [some 0, none, none]) = some published) :
    ∃ plan, state.planCandidate pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap) reuse = some plan := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap, hpub,
    op1NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]
  refine ⟨_, _, ⟨?_, rfl⟩, rfl⟩
  simpa [op1NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]
    using halias

/-- The reverse OP1 candidate has the same one-result/write shape, with its
donor and reusable destination in the second consumed physical slot. -/
theorem PhysicalState.op1_reverse_candidate_shape
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num secondValue))
    (hsa : second.agent.map Prod.snd = some (.op1 op firstValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.second 1 = some boundary)
    (hpub : state.publicationFor pair [boundary]
      (not reuse, if reuse then pair.second else state.slots.size,
        .num (op.apply firstValue secondValue), pair.second,
        [some 0, none, none]) = some published)
    (hc : state.planCandidate pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap) reuse = some plan) :
    plan.results =
      [(not reuse, if reuse then pair.second else state.slots.size,
        .num (op.apply firstValue secondValue), pair.second,
        [some 0, none, none])] ∧
    plan.boundaries = [boundary] ∧
    plan.externalWrites = [(0, 0, 0)] ∧
    plan.publications = published ∧
    plan.action = (0, if reuse then 1 else 0) ∧
    plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat) := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore at hc
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap, hpub,
    op1NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc] at hc
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

/-- A forward OP1 image whose sole captured endpoint is an output executes
after candidate construction and byte roundtrip, for both storage modes.
The one-result and write shape is derived from the selected candidate. -/
theorem PhysicalState.op1_forward_output_candidate_completes
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {output : Nat} {oldOutput : PhysicalOutput}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.op1 op firstValue))
    (hsa : second.agent.map Prod.snd = some (.num secondValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.first 1 = some boundary)
    (hkind : boundary.2 = (1, output, 0))
    (houtput : state.outputs[output]? = some oldOutput)
    (htarget : oldOutput.target = boundary.1)
    (hc : state.planCandidate pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX)
      reuse = some plan)
    (hdecode : BytePlan.decode plan.emit = some plan)
    (hhead : state.pairHeadroom pair = true) :
    ∃ next, state.executeCheckedImage plan.emit
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX) = some next := by
  let result : Bool × Nat × NetSym × Nat × List (Option Nat) :=
    (not reuse, if reuse then pair.first else state.slots.size,
      .num (op.apply firstValue secondValue), pair.first,
      [some 0, none, none])
  have hpub : state.publicationFor pair [boundary] result = some [] := by
    simp [result, PhysicalState.publicationFor, hkind]
  obtain ⟨hresults, hboundaries, hwrites, hpubs, haction, hpairFields⟩ :=
    PhysicalState.op1_forward_candidate_shape hpair hf hs
      hfa hsa hfp hsp hcap hpub hc
  have hresult : plan.results = [result] := hresults
  have hbound : plan.boundaries[0]? = some boundary := by
    simp [hboundaries]
  have hportShape : result.2.2.2.2 = [some 0, none, none] := rfl
  have hcurrent : state.logicalPair
      { first := plan.pair.1,
        firstGeneration := UInt64.ofNat plan.pair.2.1,
        second := plan.pair.2.2.1,
        secondGeneration := UInt64.ofNat plan.pair.2.2.2 } =
        some (a, b) := by
    simpa [hpairFields] using hpair
  have hroom :
      (first.generation != UInt64.ofNat (UInt64.size - 1) &&
        second.generation != UInt64.ofNat (UInt64.size - 1)) = true := by
    simpa [PhysicalState.pairHeadroom, hf, hs] using hhead
  have hrooms : (first.generation != UInt64.ofNat (UInt64.size - 1)) =
      true ∧
      (second.generation != UInt64.ofNat (UInt64.size - 1)) = true := by
    simpa using hroom
  have hfirstRoom : first.generation ≠
      UInt64.ofNat (UInt64.size - 1) := by
    simpa using hrooms.1
  have hsecondRoom : second.generation ≠
      UInt64.ofNat (UInt64.size - 1) := by
    simpa using hrooms.2
  have hfirstLive : first.agent.isSome = true := by
    cases ha : first.agent with
    | none => simp [ha] at hfa
    | some _ => rfl
  have hsecondLive : second.agent.isSome = true := by
    cases ha : second.agent with
    | none => simp [ha] at hsa
    | some _ => rfl
  have hdistinct : pair.first ≠ pair.second := by
    intro heq
    rw [heq] at hf
    have heqSlots : first = second := Option.some.inj (hf.symm.trans hs)
    subst second
    simp [hfa] at hsa
  have hpolicy :
      (result.1 = true ∧ result.2.1 = state.slots.size ∧
        plan.action = (0, 0)) ∨
      (result.1 = false ∧ result.2.1 = plan.pair.1 ∧
        plan.action = (1, 0)) ∨
      (result.1 = false ∧ result.2.1 = plan.pair.2.2.1 ∧
        plan.action = (0, 1)) := by
    cases reuse with
    | false =>
      left
      simp [result, haction]
    | true =>
      right
      left
      simp [result, haction, hpairFields]
  have hreuse : result.1 = false →
      ∃ old, state.slots[result.2.1]? = some old ∧
        old.agent.isSome = true ∧
        old.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    intro hr
    cases reuse with
    | false => simp [result] at hr
    | true =>
      refine ⟨first, ?_, hfirstLive, hfirstRoom⟩
      simpa [result] using hf
  have hslot : (op1NumData op firstValue secondValue).toX.slots[0]? =
      some (some false, 0) := by rfl
  have hlab : (op1NumData op firstValue secondValue).toX.lab
      (some false) 0 = some result.2.2.1 := by rfl
  have hfPlan : state.slots[plan.pair.1]? = some first := by
    simpa [hpairFields] using hf
  have hsPlan : state.slots[plan.pair.2.2.1]? = some second := by
    simpa [hpairFields] using hs
  exact PhysicalState.execute_single_output_image_success
    hdecode hcurrent hresult
    hslot hlab hbound hportShape hkind houtput htarget
    hwrites hpubs hpolicy hf hfirstLive hreuse
    hfPlan hsPlan hfirstLive hsecondLive hfirstRoom hsecondRoom
    (by simpa [hpairFields] using hdistinct)

/-- Mirrored OP1 with a captured output likewise executes its actual
one-result candidate in either physical storage mode. -/
theorem PhysicalState.op1_reverse_output_candidate_completes
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {output : Nat} {oldOutput : PhysicalOutput}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num secondValue))
    (hsa : second.agent.map Prod.snd = some (.op1 op firstValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.second 1 = some boundary)
    (hkind : boundary.2 = (1, output, 0))
    (houtput : state.outputs[output]? = some oldOutput)
    (htarget : oldOutput.target = boundary.1)
    (hc : state.planCandidate pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap)
      reuse = some plan)
    (hdecode : BytePlan.decode plan.emit = some plan)
    (hhead : state.pairHeadroom pair = true) :
    ∃ next, state.executeCheckedImage plan.emit
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap) = some next := by
  let result : Bool × Nat × NetSym × Nat × List (Option Nat) :=
    (not reuse, if reuse then pair.second else state.slots.size,
      .num (op.apply firstValue secondValue), pair.second,
      [some 0, none, none])
  have hpub : state.publicationFor pair [boundary] result = some [] := by
    simp [result, PhysicalState.publicationFor, hkind]
  obtain ⟨hresults, hboundaries, hwrites, hpubs, haction, hpairFields⟩ :=
    PhysicalState.op1_reverse_candidate_shape hpair hf hs
      hfa hsa hfp hsp hcap hpub hc
  have hresult : plan.results = [result] := hresults
  have hbound : plan.boundaries[0]? = some boundary := by
    simp [hboundaries]
  have hportShape : result.2.2.2.2 = [some 0, none, none] := rfl
  have hcurrent : state.logicalPair
      { first := plan.pair.1,
        firstGeneration := UInt64.ofNat plan.pair.2.1,
        second := plan.pair.2.2.1,
        secondGeneration := UInt64.ofNat plan.pair.2.2.2 } =
        some (a, b) := by
    simpa [hpairFields] using hpair
  have hroom :
      (first.generation != UInt64.ofNat (UInt64.size - 1) &&
        second.generation != UInt64.ofNat (UInt64.size - 1)) = true := by
    simpa [PhysicalState.pairHeadroom, hf, hs] using hhead
  have hrooms : (first.generation != UInt64.ofNat (UInt64.size - 1)) =
      true ∧
      (second.generation != UInt64.ofNat (UInt64.size - 1)) = true := by
    simpa using hroom
  have hfirstRoom : first.generation ≠
      UInt64.ofNat (UInt64.size - 1) := by
    simpa using hrooms.1
  have hsecondRoom : second.generation ≠
      UInt64.ofNat (UInt64.size - 1) := by
    simpa using hrooms.2
  have hfirstLive : first.agent.isSome = true := by
    cases ha : first.agent with
    | none => simp [ha] at hfa
    | some _ => rfl
  have hsecondLive : second.agent.isSome = true := by
    cases ha : second.agent with
    | none => simp [ha] at hsa
    | some _ => rfl
  have hdistinct : pair.first ≠ pair.second := by
    intro heq
    rw [heq] at hf
    have heqSlots : first = second := Option.some.inj (hf.symm.trans hs)
    subst second
    simp [hfa] at hsa
  have hpolicy :
      (result.1 = true ∧ result.2.1 = state.slots.size ∧
        plan.action = (0, 0)) ∨
      (result.1 = false ∧ result.2.1 = plan.pair.1 ∧
        plan.action = (1, 0)) ∨
      (result.1 = false ∧ result.2.1 = plan.pair.2.2.1 ∧
        plan.action = (0, 1)) := by
    cases reuse with
    | false =>
      left
      simp [result, haction]
    | true =>
      right
      right
      simp [result, haction, hpairFields]
  have hreuse : result.1 = false →
      ∃ old, state.slots[result.2.1]? = some old ∧
        old.agent.isSome = true ∧
        old.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    intro hr
    cases reuse with
    | false => simp [result] at hr
    | true =>
      refine ⟨second, ?_, hsecondLive, hsecondRoom⟩
      simpa [result] using hs
  have hslot : (op1NumData op firstValue secondValue).toX.swap.slots[0]? =
      some (some true, 0) := by rfl
  have hlab : (op1NumData op firstValue secondValue).toX.swap.lab
      (some true) 0 = some result.2.2.1 := by rfl
  have hfPlan : state.slots[plan.pair.1]? = some first := by
    simpa [hpairFields] using hf
  have hsPlan : state.slots[plan.pair.2.2.1]? = some second := by
    simpa [hpairFields] using hs
  exact PhysicalState.execute_single_output_image_success
    hdecode hcurrent hresult
    hslot hlab hbound hportShape hkind houtput htarget
    hwrites hpubs hpolicy hs hsecondLive hreuse
    hfPlan hsPlan hfirstLive hsecondLive hfirstRoom hsecondRoom
    (by simpa [hpairFields] using hdistinct)

/-- Forward OP2 candidate existence requires two distinct physical captures
and only the independently computed publication of its one result. -/
theorem PhysicalState.op2_forward_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.op2 op))
    (hsa : second.agent.map Prod.snd = some (.num value))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.first 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.first 2 = some boundaryTwo)
    (hdistinct : [boundaryOne.2, boundaryTwo.2].Nodup)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .op2 op, .num value, (op2NumData op value).toX) = false)
    (hpub : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.first else state.slots.size,
        .op1 op value, pair.first, [some 0, some 1, none]) =
      some published) :
    ∃ plan, state.planCandidate pair
      (a, b, .op2 op, .num value, (op2NumData op value).toX)
      reuse = some plan := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hdistinct, hpub,
    op2NumData, RuleData.toX, RuleData.ofWires, assoc]
  refine ⟨_, _, ⟨?_, rfl⟩, rfl⟩
  simpa [op2NumData, RuleData.toX, RuleData.ofWires, assoc]
    using halias

/-- Reverse OP2 candidate existence captures the second physical slot's
ports in order, without changing the selected RHS result symbol. -/
theorem PhysicalState.op2_reverse_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {published : List (Nat × Nat × Nat × Nat)}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num value))
    (hsa : second.agent.map Prod.snd = some (.op2 op))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.second 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.second 2 = some boundaryTwo)
    (hdistinct : [boundaryOne.2, boundaryTwo.2].Nodup)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .num value, .op2 op, (op2NumData op value).toX.swap) = false)
    (hpub : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.second else state.slots.size,
        .op1 op value, pair.second, [some 0, some 1, none]) =
      some published) :
    ∃ plan, state.planCandidate pair
      (a, b, .num value, .op2 op, (op2NumData op value).toX.swap)
      reuse = some plan := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hdistinct, hpub,
    op2NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]
  refine ⟨_, _, ⟨?_, rfl⟩, rfl⟩
  simpa [op2NumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]
    using halias

/-- DUP produces two ordered number results, potentially at both consumed
slots, and each result's publication is checked against the prestate. -/
theorem PhysicalState.dup_forward_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {firstPublished secondPublished : List (Nat × Nat × Nat × Nat)}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some .dup)
    (hsa : second.agent.map Prod.snd = some (.num value))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.first 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.first 2 = some boundaryTwo)
    (hdistinct : [boundaryOne.2, boundaryTwo.2].Nodup)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .dup, .num value, (dupNumData value).toX) = false)
    (hpub1 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.first else state.slots.size,
        .num value, pair.first, [some 0, none, none]) =
      some firstPublished)
    (hpub2 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.second else state.slots.size + 1,
        .num value, pair.second, [some 1, none, none]) =
      some secondPublished) :
    ∃ plan, state.planCandidate pair
      (a, b, .dup, .num value, (dupNumData value).toX)
      reuse = some plan := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hdistinct, hpub1, hpub2,
    dupNumData, RuleData.toX, RuleData.ofWires, assoc]
  refine ⟨_, _, ⟨?_, rfl⟩, rfl⟩
  simpa [dupNumData, RuleData.toX, RuleData.ofWires, assoc]
    using halias

/-- The mirrored DUP emits the same two ordered symbols but takes donor
slots in rule-slot, number-slot order. -/
theorem PhysicalState.dup_reverse_candidate_exists
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)}
    {firstPublished secondPublished : List (Nat × Nat × Nat × Nat)}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num value))
    (hsa : second.agent.map Prod.snd = some .dup)
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap1 : state.captureAux pair.second 1 = some boundaryOne)
    (hcap2 : state.captureAux pair.second 2 = some boundaryTwo)
    (hdistinct : [boundaryOne.2, boundaryTwo.2].Nodup)
    (halias : state.consumedBoundaryAlias pair
      (a, b, .num value, .dup, (dupNumData value).toX.swap) = false)
    (hpub1 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.second else state.slots.size,
        .num value, pair.second, [some 0, none, none]) =
      some firstPublished)
    (hpub2 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (not reuse, if reuse then pair.first else state.slots.size + 1,
        .num value, pair.first, [some 1, none, none]) =
      some secondPublished) :
    ∃ plan, state.planCandidate pair
      (a, b, .num value, .dup, (dupNumData value).toX.swap)
      reuse = some plan := by
  unfold PhysicalState.planCandidate PhysicalState.planCandidateCore
  simp [NetI64.xRule, hpair, hf, hs, hfa, hsa, hfp, hsp,
    BytePlan.ruleSlots, BytePlan.ruleOpcode, PhysicalState.candidateBody,
    hcap1, hcap2, hdistinct, hpub1, hpub2,
    dupNumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]
  refine ⟨_, _, ⟨?_, rfl⟩, rfl⟩
  simpa [dupNumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]
    using halias

/-- Independently detected aliases are refused for any rule orientation,
payload, physical layout, or storage mode before a result can be emitted. -/
theorem PhysicalState.planCandidate_refuses_alias
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool}
    (h : state.consumedBoundaryAlias pair redex = true) :
    state.planCandidate pair redex reuse = none := by
  simp [PhysicalState.planCandidate, PhysicalState.planCandidateCore,
    PhysicalState.candidateBody, h]

/-- Even a reuse-only candidate refuses if the ambient arena/output
representation cannot be indexed by version-one 32-bit slot fields. -/
def PhysicalState.planIndicesEncodable (state : PhysicalState) : Bool :=
  state.slots.size + 2 ≤ 2 ^ 32 && state.outputs.length < 2 ^ 32

/-- Lower one physically matched revision-3 rule to the actual version-one
byte representation. The rule supplies result symbols; the arena supplies
endpoints, generations, output indices and publications. `reuse` selects
consumed-slot reuse or fresh append in the same order as Rust. A plan that
cannot roundtrip through the independent byte decoder is refused. -/
def PhysicalState.lowerPlan (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) : Option BytePlan := do
  if state.planIndicesEncodable then
    if state.pairHeadroom pair then
      let plan ← state.planCandidate pair redex reuse
      if BytePlan.decode plan.emit == some plan then some plan else none
    else none
  else none

/-- An arena/output table outside the version-one 32-bit field envelope
refuses before a candidate is assembled, including the reuse policy. This
conservative representability check is separate from the exact byte decoder. -/
theorem PhysicalState.lowerPlan_refuses_unrepresentable
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool}
    (h : state.planIndicesEncodable = false) :
    state.lowerPlan pair redex reuse = none := by
  simp [PhysicalState.lowerPlan, h]

/-- Exhausted generation headroom refuses before an image can be emitted,
regardless of rule orientation or storage policy. -/
theorem PhysicalState.lowerPlan_refuses_exhausted
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool}
    (h : state.pairHeadroom pair = false) :
    state.lowerPlan pair redex reuse = none := by
  simp [PhysicalState.lowerPlan, h]

/-- The explicit consumed-pair auxiliary alias has priority over encoding
and physical execution: even an otherwise roundtrippable byte plan refuses. -/
theorem PhysicalState.lowerPlan_refuses_alias
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool}
    (h : state.consumedBoundaryAlias pair redex = true) :
    state.lowerPlan pair redex reuse = none := by
  simp [PhysicalState.lowerPlan, PhysicalState.planCandidate_refuses_alias h]

/-- Independently computed byte images longer than the version-one maximum
cannot be decoded, irrespective of rule or physical-state assertions. -/
theorem BytePlan.decode_refuses_oversized_emit (plan : BytePlan)
    (h : plan.emit.length > 256) :
    BytePlan.decode plan.emit = none := by
  simp [BytePlan.decode, h]

/-- An assembled candidate whose byte image cannot roundtrip is explicitly
refused, not silently truncated into a different plan. -/
theorem PhysicalState.lowerPlan_refuses_unencodable_candidate
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool} {plan : BytePlan}
    (hc : state.planCandidate pair redex reuse = some plan)
    (hd : BytePlan.decode plan.emit ≠ some plan) :
    state.lowerPlan pair redex reuse = none := by
  cases hh : state.pairHeadroom pair <;>
    simp [PhysicalState.lowerPlan, hh, hc, hd]

/-- The concrete 256-byte length exclusion is a sufficient instance of
the general encoder roundtrip refusal. -/
theorem PhysicalState.lowerPlan_refuses_oversized_candidate
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool} {plan : BytePlan}
    (hc : state.planCandidate pair redex reuse = some plan)
    (h : plan.emit.length > 256) :
    state.lowerPlan pair redex reuse = none := by
  exact PhysicalState.lowerPlan_refuses_unencodable_candidate hc
    (by rw [BytePlan.decode_refuses_oversized_emit plan h]; simp)

/-- Every actually emitted plan has checked counter headroom for both
consumed slots; successful byte parsing alone does not prove this fact. -/
theorem PhysicalState.lowerPlan_requires_headroom
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool} {plan : BytePlan}
    (h : state.lowerPlan pair redex reuse = some plan) :
    state.pairHeadroom pair = true := by
  cases hh : state.pairHeadroom pair with
  | false => simp [PhysicalState.lowerPlan, hh] at h
  | true => rfl

/-- Every emitted model image is parsed back byte-for-byte before return;
this does not imply the stronger physical semantic checker accepts it. -/
theorem PhysicalState.lowerPlan_decodes {state : PhysicalState}
    {pair : PhysicalPair} {redex : XRedex NetSym} {reuse : Bool} {plan : BytePlan}
    (h : state.lowerPlan pair redex reuse = some plan) :
    BytePlan.decode plan.emit = some plan := by
  unfold PhysicalState.lowerPlan at h
  cases he : state.planIndicesEncodable with
  | false => simp [he] at h
  | true =>
    cases hh : state.pairHeadroom pair with
    | false => simp [he, hh] at h
    | true =>
      cases hp : state.planCandidate pair redex reuse with
      | none => simp [he, hh, hp] at h
      | some candidate =>
        by_cases hd : BytePlan.decode candidate.emit = some candidate
        · simp [he, hh, hp, hd] at h
          cases h
          exact hd
        · simp [he, hh, hp, hd] at h

/-- A successful candidate is assembled with both exact ordered write lists. -/
theorem PhysicalState.planCandidate_writes_exact {state : PhysicalState}
    {pair : PhysicalPair} {redex : XRedex NetSym} {reuse : Bool}
    {plan : BytePlan}
    (h : state.planCandidate pair redex reuse = some plan) :
    plan.writes = plan.requiredWrites ∧
      plan.externalWrites = plan.requiredWrites := by
  cases hc : state.planCandidateCore pair redex reuse with
  | none => simp [PhysicalState.planCandidate, hc] at h
  | some candidate =>
    simp [PhysicalState.planCandidate, hc] at h
    cases h
    exact BytePlan.withRequiredWrites_exact candidate

/-- Noncircular emitter fields: the selected rule and physical source ports
determine the opcode and ordered boundary; result ports alone determine both
write inventories and all three reservation counters. -/
theorem PhysicalState.planCandidate_fields {state : PhysicalState}
    {pair : PhysicalPair} {redex : XRedex NetSym} {reuse : Bool}
    {plan : BytePlan}
    (h : state.planCandidate pair redex reuse = some plan) :
    BytePlan.ruleOpcode redex.2.2.1 redex.2.2.2.1 = some plan.opcode ∧
    (∃ ruleSlot valueSlot,
      BytePlan.ruleSlots redex.2.2.1 redex.2.2.2.1 pair =
        some (ruleSlot, valueSlot) ∧
      (if plan.opcode == 1 || plan.opcode == 3 then [1, 2]
        else if plan.opcode == 2 then [1] else []).mapM
          (state.captureAux ruleSlot) = some plan.boundaries) ∧
    (∃ rhs, NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some rhs ∧
      rhs.slots.mapM (fun slot => rhs.lab slot.1 slot.2) =
        some (plan.results.map fun result => result.2.2.1)) ∧
    plan.writes = plan.requiredWrites ∧
    plan.externalWrites = plan.requiredWrites ∧
    plan.arenaReservation = (plan.results.filter fun result => result.1).length ∧
    plan.scratchReservation = 256 ∧
    plan.worklistReservation = plan.publications.length := by
  cases hc : state.planCandidateCore pair redex reuse with
  | none => simp [PhysicalState.planCandidate, hc] at h
  | some candidate =>
    simp [PhysicalState.planCandidate, hc] at h
    cases h
    unfold PhysicalState.planCandidateCore at hc
    obtain ⟨rhs, hRule, hc⟩ := Option.bind_eq_some_iff.mp hc
    simp at hc
    obtain ⟨_, hc⟩ := hc
    obtain ⟨first, hf, hc⟩ := Option.bind_eq_some_iff.mp hc
    obtain ⟨second, hs, hc⟩ := Option.bind_eq_some_iff.mp hc
    simp at hc
    obtain ⟨_, hc⟩ := hc
    obtain ⟨opcode, ho, hc⟩ := Option.bind_eq_some_iff.mp hc
    obtain ⟨slots, hp, hc⟩ := Option.bind_eq_some_iff.mp hc
    obtain ⟨ruleSlot, valueSlot⟩ := slots
    obtain ⟨hop, ⟨boundaries, hb, hboundary⟩,
      ⟨symbols, hsymbols, hresultSymbols⟩, harena, hscratch,
      hqueue⟩ := PhysicalState.candidateBody_fields hc
    have hwrites := BytePlan.withRequiredWrites_exact candidate
    refine ⟨?_, ⟨ruleSlot, valueSlot, hp, ?_⟩,
      ⟨rhs, hRule, ?_⟩, hwrites.1, hwrites.2, ?_, ?_, ?_⟩
    · simpa [BytePlan.withRequiredWrites, hop] using ho
    · simpa [BytePlan.withRequiredWrites, hop, hboundary] using hb
    · simpa [BytePlan.withRequiredWrites, ← hresultSymbols] using hsymbols
    · simpa [BytePlan.withRequiredWrites] using harena
    · simpa [BytePlan.withRequiredWrites] using hscratch
    · simpa [BytePlan.withRequiredWrites] using hqueue

/-- Successful lowering retains the exact candidate it constructed before
the independent image decoder checked its roundtrip. -/
private theorem PhysicalState.lowerPlan_has_candidate
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool} {plan : BytePlan}
    (hLower : state.lowerPlan pair redex reuse = some plan) :
    state.planCandidate pair redex reuse = some plan := by
  unfold PhysicalState.lowerPlan at hLower
  cases he : state.planIndicesEncodable with
  | false => simp [he] at hLower
  | true =>
    cases hh : state.pairHeadroom pair with
    | false => simp [he, hh] at hLower
    | true =>
      cases hp : state.planCandidate pair redex reuse with
      | none => simp [he, hh, hp] at hLower
      | some candidate =>
        by_cases hd : BytePlan.decode candidate.emit = some candidate
        · simp [he, hh, hp, hd] at hLower
          cases hLower
          rfl
        · simp [he, hh, hp, hd] at hLower

/-- An actually emitted forward OP1 image with an output boundary completes
the physical executor in both storage modes without an execution premise. -/
theorem PhysicalState.op1_forward_output_lowered_completes
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {output : Nat} {oldOutput : PhysicalOutput}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.op1 op firstValue))
    (hsa : second.agent.map Prod.snd = some (.num secondValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.first 1 = some boundary)
    (hkind : boundary.2 = (1, output, 0))
    (houtput : state.outputs[output]? = some oldOutput)
    (htarget : oldOutput.target = boundary.1)
    (hLower : state.lowerPlan pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX)
      reuse = some plan) :
    ∃ next, state.executeCheckedImage plan.emit
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX) = some next :=
  PhysicalState.op1_forward_output_candidate_completes
    hpair hf hs hfa hsa hfp hsp hcap hkind houtput htarget
    (PhysicalState.lowerPlan_has_candidate hLower)
    (PhysicalState.lowerPlan_decodes hLower)
    (PhysicalState.lowerPlan_requires_headroom hLower)

/-- The mirrored actually emitted OP1 output-boundary image also completes,
with the result allocated fresh or installed in the second selected slot. -/
theorem PhysicalState.op1_reverse_output_lowered_completes
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {output : Nat} {oldOutput : PhysicalOutput}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num secondValue))
    (hsa : second.agent.map Prod.snd = some (.op1 op firstValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.second 1 = some boundary)
    (hkind : boundary.2 = (1, output, 0))
    (houtput : state.outputs[output]? = some oldOutput)
    (htarget : oldOutput.target = boundary.1)
    (hLower : state.lowerPlan pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap)
      reuse = some plan) :
    ∃ next, state.executeCheckedImage plan.emit
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap) = some next :=
  PhysicalState.op1_reverse_output_candidate_completes
    hpair hf hs hfa hsa hfp hsp hcap hkind houtput htarget
    (PhysicalState.lowerPlan_has_candidate hLower)
    (PhysicalState.lowerPlan_decodes hLower)
    (PhysicalState.lowerPlan_requires_headroom hLower)

/-- Independent prestate endpoint/projection witnesses identify the one
physical output capture with OP1's sole abstract auxiliary edge. They do
not assume post-state equality or semantic-checker acceptance. -/
theorem PhysicalState.op1_forward_output_lowered_boundary
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {output : Nat} {oldOutput : PhysicalOutput}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.op1 op firstValue))
    (hsa : second.agent.map Prod.snd = some (.num secondValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.first 1 = some boundary)
    (hsource : boundary.1 = (pair.first, 1))
    (hkind : boundary.2 = (1, output, 0))
    (hleft : state.logicalPort (pair.first, 1) = some (a, 1))
    (houtput : state.outputs[output]? = some oldOutput)
    (hwire : state.project.wires[(a, 1)]? = some oldOutput.label)
    (hLower : state.lowerPlan pair
      (a, b, .op1 op firstValue, .num secondValue,
        (op1NumData op firstValue secondValue).toX)
      reuse = some plan) :
    plan.logicalBoundary state = some
      ((Plan.compile state.project
        (a, b, .op1 op firstValue, .num secondValue,
          (op1NumData op firstValue secondValue).toX)
        (by rfl)).capturedBoundary.filter fun edge => edge.1.2 != 0) := by
  have hpub : state.publicationFor pair [boundary]
      (not reuse, if reuse then pair.first else state.slots.size,
        .num (op.apply firstValue secondValue), pair.first,
        [some 0, none, none]) = some [] := by
    simp [PhysicalState.publicationFor, hkind]
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  have hshape := PhysicalState.op1_forward_candidate_shape
    hpair hf hs hfa hsa hfp hsp hcap hpub hc
  rw [Plan.op1_forward_output_boundary state.project a b op
    firstValue secondValue oldOutput.label hwire]
  exact BytePlan.single_output_logical_boundary hshape.2.1
    hsource hkind hleft houtput

theorem PhysicalState.op1_reverse_output_lowered_boundary
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {op : Op} {firstValue secondValue : BitVec 64}
    {reuse : Bool} {first second : PhysicalSlot}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    {output : Nat} {oldOutput : PhysicalOutput}
    {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num secondValue))
    (hsa : second.agent.map Prod.snd = some (.op1 op firstValue))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hcap : state.captureAux pair.second 1 = some boundary)
    (hsource : boundary.1 = (pair.second, 1))
    (hkind : boundary.2 = (1, output, 0))
    (hleft : state.logicalPort (pair.second, 1) = some (b, 1))
    (houtput : state.outputs[output]? = some oldOutput)
    (hwire : state.project.wires[(b, 1)]? = some oldOutput.label)
    (hLower : state.lowerPlan pair
      (a, b, .num secondValue, .op1 op firstValue,
        (op1NumData op firstValue secondValue).toX.swap)
      reuse = some plan) :
    plan.logicalBoundary state = some
      ((Plan.compile state.project
        (a, b, .num secondValue, .op1 op firstValue,
          (op1NumData op firstValue secondValue).toX.swap)
        (by rfl)).capturedBoundary.filter fun edge => edge.1.2 != 0) := by
  have hpub : state.publicationFor pair [boundary]
      (not reuse, if reuse then pair.second else state.slots.size,
        .num (op.apply firstValue secondValue), pair.second,
        [some 0, none, none]) = some [] := by
    simp [PhysicalState.publicationFor, hkind]
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  have hshape := PhysicalState.op1_reverse_candidate_shape
    hpair hf hs hfa hsa hfp hsp hcap hpub hc
  rw [Plan.op1_reverse_output_boundary state.project a b op
    firstValue secondValue oldOutput.label hwire]
  exact BytePlan.single_output_logical_boundary hshape.2.1
    hsource hkind hleft houtput

/-- The actual lowered byte image already satisfies the first eight
independently checked semantic fields when its physical capture denotes the
abstract selected boundary. The remaining fields concern the physical
post-state, not result-symbol or reciprocal-write construction. -/
theorem PhysicalState.lowerPlan_semantic_prefix {state : PhysicalState}
    {pair : PhysicalPair} {redex : XRedex NetSym} {reuse : Bool}
    {plan : BytePlan}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 =
      some redex.2.2.2.2)
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hBoundary : plan.logicalBoundary state = some
      ((Plan.compile state.project redex hRule).capturedBoundary.filter
        fun edge => edge.1.2 != 0)) :
    let selected := Plan.compile state.project redex hRule
    some plan.opcode =
      BytePlan.ruleOpcode redex.2.2.1 redex.2.2.2.1 ∧
    plan.logicalBoundary state = some (selected.capturedBoundary.filter
      fun edge => edge.1.2 != 0) ∧
    plan.results.map (fun result => result.2.2.1) =
      selected.initializedAgents.map Prod.snd ∧
    plan.writes = plan.requiredWrites ∧
    plan.externalWrites = plan.requiredWrites ∧
    plan.arenaReservation =
      (plan.results.filter fun result => result.1).length ∧
    256 ≤ plan.scratchReservation ∧
    plan.publications.length ≤ plan.worklistReservation := by
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨hop, _, ⟨rhs, hrule, hsymbols⟩, hw, hew,
    harena, hscratch, hqueue⟩ := PhysicalState.planCandidate_fields hc
  have hrhs : rhs = redex.2.2.2.2 := by
    rw [hRule] at hrule
    exact (Option.some.inj hrule).symm
  subst rhs
  exact ⟨hop.symm, hBoundary,
    (PhysicalState.rule_result_symbols hRule hsymbols).symm,
    hw, hew, harena, by omega, by omega⟩

/-- Any lowered zero-result image whose two consumed slots are distinct and
live completes its physical step, preserves every output target, and keeps
the old physical pending queue exactly. Both erasure orientations yield this
image shape under the per-rule candidate-existence witnesses above. -/
theorem PhysicalState.lowered_empty_image_completes
    {state : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool}
    {plan : BytePlan} {first second : PhysicalSlot}
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hShape : plan.results = [] ∧ plan.externalWrites = [] ∧
      plan.publications = [] ∧ plan.action = (0, 0) ∧
      plan.pair = (pair.first, pair.firstGeneration.toNat,
        pair.second, pair.secondGeneration.toNat))
    (hpair : state.logicalPair pair = some (redex.1, redex.2.1))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hliveFirst : first.agent.isSome = true)
    (hliveSecond : second.agent.isSome = true)
    (hdistinct : pair.first ≠ pair.second) :
    ∃ next, state.executeCheckedImage plan.emit redex = some next ∧
      next.outputs = state.outputs ∧ next.work = state.work := by
  obtain ⟨hresults, hwrites, hpubs, haction, hpairPlan⟩ := hShape
  have hroom := PhysicalState.lowerPlan_requires_headroom hLower
  simp [PhysicalState.pairHeadroom, hf, hs] at hroom
  have hcurrent : state.logicalPair
      { first := plan.pair.1,
        firstGeneration := UInt64.ofNat plan.pair.2.1,
        second := plan.pair.2.2.1,
        secondGeneration := UInt64.ofNat plan.pair.2.2.2 } =
      some (redex.1, redex.2.1) := by
    simpa [hpairPlan] using hpair
  have hf' : state.slots[plan.pair.1]? = some first := by
    simpa [hpairPlan] using hf
  have hs' : state.slots[plan.pair.2.2.1]? = some second := by
    simpa [hpairPlan] using hs
  have hdistinct' : plan.pair.1 ≠ plan.pair.2.2.1 := by
    simpa [hpairPlan] using hdistinct
  have hd := PhysicalState.lowerPlan_decodes hLower
  obtain ⟨next, he⟩ := PhysicalState.execute_empty_image_success
    hd hcurrent hresults hwrites hpubs haction hf' hs'
    hliveFirst hliveSecond hroom.1 hroom.2 hdistinct'
  exact ⟨next, he,
    PhysicalState.execute_empty_image_preserves_outputs
      hd hcurrent hresults hwrites hpubs haction he,
    by simpa [hpubs] using
      PhysicalState.executeCheckedImage_work_append hd he⟩

/-- Every actually lowered forward ERA image executes for distinct consumed
physical slots, retaining all output targets and the old physical queue. -/
theorem PhysicalState.era_forward_lowered_completes
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64} {reuse : Bool}
    {first second : PhysicalSlot} {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some .era)
    (hsa : second.agent.map Prod.snd = some (.num value))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hdistinct : pair.first ≠ pair.second)
    (hLower : state.lowerPlan pair
      (a, b, .era, .num value, eraNumData.toX) reuse = some plan) :
    ∃ next, state.executeCheckedImage plan.emit
        (a, b, .era, .num value, eraNumData.toX) = some next ∧
      next.outputs = state.outputs ∧ next.work = state.work := by
  obtain ⟨candidate, hc, hresults, _, hwrites, hpubs, haction,
    hpairPlan⟩ := PhysicalState.era_forward_candidate_exists
      hpair hf hs hfa hsa hfp hsp
  have hcandidate := PhysicalState.lowerPlan_has_candidate hLower
  have heq : candidate = plan := Option.some.inj (hc.symm.trans hcandidate)
  subst candidate
  have hliveFirst : first.agent.isSome = true := by
    cases ha : first.agent with
    | none => simp [ha] at hfa
    | some _ => rfl
  have hliveSecond : second.agent.isSome = true := by
    cases ha : second.agent with
    | none => simp [ha] at hsa
    | some _ => rfl
  exact PhysicalState.lowered_empty_image_completes hLower
    ⟨hresults, hwrites, hpubs, haction, hpairPlan⟩
    hpair hf hs hliveFirst hliveSecond hdistinct

/-- Reverse ERA has the same symbolic retirement and output/queue frame as
forward ERA; only the physical orientation of the rule symbols changes. -/
theorem PhysicalState.era_reverse_lowered_completes
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64} {reuse : Bool}
    {first second : PhysicalSlot} {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num value))
    (hsa : second.agent.map Prod.snd = some .era)
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hdistinct : pair.first ≠ pair.second)
    (hLower : state.lowerPlan pair
      (a, b, .num value, .era, eraNumData.toX.swap) reuse = some plan) :
    ∃ next, state.executeCheckedImage plan.emit
        (a, b, .num value, .era, eraNumData.toX.swap) = some next ∧
      next.outputs = state.outputs ∧ next.work = state.work := by
  obtain ⟨candidate, hc, hresults, _, hwrites, hpubs, haction,
    hpairPlan⟩ := PhysicalState.era_reverse_candidate_exists
      hpair hf hs hfa hsa hfp hsp
  have hcandidate := PhysicalState.lowerPlan_has_candidate hLower
  have heq : candidate = plan := Option.some.inj (hc.symm.trans hcandidate)
  subst candidate
  have hliveFirst : first.agent.isSome = true := by
    cases ha : first.agent with
    | none => simp [ha] at hfa
    | some _ => rfl
  have hliveSecond : second.agent.isSome = true := by
    cases ha : second.agent with
    | none => simp [ha] at hsa
    | some _ => rfl
  exact PhysicalState.lowered_empty_image_completes hLower
    ⟨hresults, hwrites, hpubs, haction, hpairPlan⟩
    hpair hf hs hliveFirst hliveSecond hdistinct

/-- Forward ERA's actually emitted empty capture equals the independent
abstract selected boundary for arbitrary physical/abstract map contents. -/
theorem PhysicalState.era_forward_lowered_boundary
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64} {reuse : Bool}
    {first second : PhysicalSlot} {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some .era)
    (hsa : second.agent.map Prod.snd = some (.num value))
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hLower : state.lowerPlan pair
      (a, b, .era, .num value, eraNumData.toX) reuse = some plan) :
    plan.logicalBoundary state = some
      ((Plan.compile state.project
        (a, b, .era, .num value, eraNumData.toX) (by rfl)).capturedBoundary.filter
        fun edge => edge.1.2 != 0) := by
  obtain ⟨candidate, hc, _, hboundaries, _, _, _, _⟩ :=
    PhysicalState.era_forward_candidate_exists hpair hf hs hfa hsa hfp hsp
  have hcandidate := PhysicalState.lowerPlan_has_candidate hLower
  have heq : candidate = plan := Option.some.inj (hc.symm.trans hcandidate)
  subst candidate
  exact PhysicalState.lowered_zero_arity_boundary
    (by rfl) rfl rfl hboundaries

/-- Mirrored ERA has the same boundary correspondence: both selected agents
have arity zero, and the physical image captures no auxiliary endpoint. -/
theorem PhysicalState.era_reverse_lowered_boundary
    {state : PhysicalState} {pair : PhysicalPair}
    {a b : Id} {value : BitVec 64} {reuse : Bool}
    {first second : PhysicalSlot} {plan : BytePlan}
    (hpair : state.logicalPair pair = some (a, b))
    (hf : state.slots[pair.first]? = some first)
    (hs : state.slots[pair.second]? = some second)
    (hfa : first.agent.map Prod.snd = some (.num value))
    (hsa : second.agent.map Prod.snd = some .era)
    (hfp : first.ports[0]? = some (some (pair.second, 0)))
    (hsp : second.ports[0]? = some (some (pair.first, 0)))
    (hLower : state.lowerPlan pair
      (a, b, .num value, .era, eraNumData.toX.swap) reuse = some plan) :
    plan.logicalBoundary state = some
      ((Plan.compile state.project
        (a, b, .num value, .era, eraNumData.toX.swap) (by rfl)).capturedBoundary.filter
        fun edge => edge.1.2 != 0) := by
  obtain ⟨candidate, hc, _, hboundaries, _, _, _, _⟩ :=
    PhysicalState.era_reverse_candidate_exists hpair hf hs hfa hsa hfp hsp
  have hcandidate := PhysicalState.lowerPlan_has_candidate hLower
  have heq : candidate = plan := Option.some.inj (hc.symm.trans hcandidate)
  subst candidate
  exact PhysicalState.lowered_zero_arity_boundary
    (by rfl) rfl rfl hboundaries

/-- Exact bytes from the model lowering, with decoder roundtrip checked
before return. These are comparable byte-for-byte to a Rust-emitted stream. -/
def PhysicalState.lowerBytes (state : PhysicalState) (pair : PhysicalPair)
    (redex : XRedex NetSym) (reuse : Bool) : Option (List UInt8) :=
  (state.lowerPlan pair redex reuse).map BytePlan.emit

/-- Independent physical queue-capacity exclusion. Stale entries still occupy
space; additional plan publications cannot exceed the post-allocation slot
bound. This is a resource refusal, not an alias or byte-encoding failure. -/
def PhysicalState.queueCapacityExcluded (state : PhysicalState)
    (plan : BytePlan) : Bool :=
  decide (state.slots.size + plan.arenaReservation - state.work.length <
    plan.worklistReservation)

/-- The negated queue exclusion reserves exactly the post-allocation physical
headroom, including stale old entries. This only admits a journal; execution
and the semantic post-state are independent obligations. -/
theorem PhysicalState.reserve_if_queue_capacity
    {state : PhysicalState} {plan : BytePlan}
    (hscratch : plan.scratchReservation = 256)
    (hcap : state.queueCapacityExcluded plan = false) :
    ∃ transaction, state.reserve plan plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) =
      some transaction := by
  have hqueue : plan.worklistReservation ≤
      state.slots.size + plan.arenaReservation - state.work.length := by
    simp only [PhysicalState.queueCapacityExcluded, decide_eq_false_iff_not] at hcap
    omega
  refine ⟨{current := state, journal := some state}, ?_⟩
  simp [PhysicalState.reserve, hscratch, hqueue]

/-- With no more available worklist entries than the actual slot-derived
headroom, a queue-capacity exclusion refuses before any physical mutation. -/
theorem BytePlan.semanticCheck_refuses_queue_capacity
    {state : PhysicalState} {bytes : List UInt8} {plan : BytePlan}
    {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {arena scratch queue : Nat}
    (hd : BytePlan.decode bytes = some plan)
    (hcap : state.queueCapacityExcluded plan = true)
    (hqueue : queue ≤ state.slots.size + plan.arenaReservation -
      state.work.length) :
    BytePlan.semanticCheck state bytes redex hRule arena scratch queue = none := by
  have hshort : ¬ plan.worklistReservation ≤ queue := by
    simp only [PhysicalState.queueCapacityExcluded, decide_eq_true_eq] at hcap
    omega
  simp [BytePlan.semanticCheck, hd, PhysicalState.reserve, hshort]

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

/-- The final implication needs no checker-admission premise: generic physical
execution plus the independently specified field facts and reserved resources
establish admission and then the complete abstract refinement. The emitter
still needs a separate proof supplying these witnesses from legal layouts. -/
theorem PhysicalState.lowerPlan_refines_of_physical_fields
    {state next : PhysicalState} {pair : PhysicalPair}
    {redex : XRedex NetSym} {reuse : Bool}
    {plan : BytePlan} {arena scratch queue : Nat}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hReserve : ∃ transaction,
      state.reserve plan arena scratch queue = some transaction)
    (hExecute : state.executeCheckedImage plan.emit redex = some next)
    (hFields : plan.SemanticFields state next redex hRule)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system state.project.toNet
      redex.1 redex.2.1 redex.2.2.1 redex.2.2.2.1 redex.2.2.2.2.toRhs) :
    BytePlan.semanticCheck state plan.emit redex hRule
      arena scratch queue = some next ∧
    CStep NetI64.system state.project.toNet next.project.toNet ∧
    next.project.work.Perm
      (Plan.pendingPairs (Plan.commit state.project
        (Plan.compile state.project redex hRule) state.project.work)) ∧
    next.project.free = state.project.free := by
  have hChecked := BytePlan.semanticCheck_of_fields hRule
    (PhysicalState.lowerPlan_decodes hLower) hReserve hExecute hFields
  exact ⟨hChecked, BytePlan.semanticCheck_refines hRule hChecked hm hredex⟩

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
