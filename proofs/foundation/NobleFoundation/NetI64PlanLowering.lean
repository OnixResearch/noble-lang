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
