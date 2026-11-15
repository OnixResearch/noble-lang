import NobleFoundation.NetI64PlanLowering

/-!
# Encodable model-emitter candidates decode exactly (issue #110)

Inside the version-one 32-bit index envelope checked by
`PhysicalState.planIndicesEncodable`, every candidate the model emitter
assembles satisfies `BytePlan.Admissible`. The independent decoder therefore
returns it exactly (`BytePlan.decode_emit`), so the emitter's byte-roundtrip
check never refuses an encodable candidate. The argument inspects only the
candidate builder; it needs no legality or matching premise.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The encodability gate's two numeric bounds. -/
theorem PhysicalState.planIndicesEncodable_bounds {state : PhysicalState}
    (h : state.planIndicesEncodable = true) :
    state.slots.size + 2 ≤ 2 ^ 32 ∧ state.outputs.length < 2 ^ 32 := by
  simpa [PhysicalState.planIndicesEncodable] using h

/-- Every revision-3 symbol has at most two auxiliary ports. -/
private theorem arity_le_two (symbol : NetSym) : NetI64.arity symbol ≤ 2 := by
  cases symbol <;> simp [NetI64.arity]

/-- A captured boundary of an encodable arena fits the version-one boundary
fields: an in-range source slot and port, and either a live peer slot with a
port byte or an in-range output index. -/
theorem PhysicalState.captureAux_encodable {state : PhysicalState} {slot port : Nat}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)}
    (hencode : state.planIndicesEncodable = true) (hport : port < UInt8.size)
    (h : state.captureAux slot port = some boundary) :
    boundary.1.1 < 256 ^ 4 ∧ boundary.1.2 < UInt8.size ∧
      ((boundary.2.1 = 0 ∧ boundary.2.2.1 < 256 ^ 4 ∧ boundary.2.2.2 < UInt8.size) ∨
        (boundary.2.1 = 1 ∧ boundary.2.2.1 < 256 ^ 4 ∧ boundary.2.2.2 = 0)) := by
  obtain ⟨hsize, houts⟩ := PhysicalState.planIndicesEncodable_bounds hencode
  have hwidth : (256 : Nat) ^ 4 = 2 ^ 32 := by decide
  have hbyte : UInt8.size = 256 := by decide
  unfold PhysicalState.captureAux at h
  obtain ⟨owner, howner, h⟩ := Option.bind_eq_some_iff.mp h
  have hslot : slot < state.slots.size := (Array.getElem?_eq_some_iff.mp howner).1
  by_cases hagent : owner.agent.isNone = true
  · simp [hagent] at h
  · simp only [hagent, Bool.false_eq_true, ↓reduceIte] at h
    obtain ⟨target, _, h⟩ := Option.bind_eq_some_iff.mp h
    cases target with
    | some reference =>
      obtain ⟨peer, peerPort⟩ := reference
      by_cases houtput : (state.outputs.any fun output => output.target == (slot, port)) = true
      · simp [houtput] at h
      · simp only [houtput, Bool.false_eq_true, ↓reduceIte] at h
        obtain ⟨other, hother, h⟩ := Option.bind_eq_some_iff.mp h
        have hpeer : peer < state.slots.size := (Array.getElem?_eq_some_iff.mp hother).1
        obtain ⟨entry, _, h⟩ := Option.bind_eq_some_iff.mp h
        obtain ⟨id, symbol⟩ := entry
        by_cases hbad : (decide (peerPort > NetI64.arity symbol) ||
            other.ports[peerPort]? != some (some (slot, port))) = true
        · simp [hbad] at h
        · simp only [hbad, Bool.false_eq_true, ↓reduceIte, Option.some.injEq] at h
          subst h
          have hle : peerPort ≤ NetI64.arity symbol := by
            simp only [Bool.or_eq_true, decide_eq_true_eq, not_or, Nat.not_lt] at hbad
            exact hbad.1
          have := arity_le_two symbol
          dsimp only
          refine ⟨by omega, by omega, .inl ⟨rfl, by omega, by omega⟩⟩
    | none =>
      simp only at h
      split at h
      · rename_i out index hfilter
        simp only [Option.some.injEq] at h
        subst h
        have hmem : (out, index) ∈ state.outputs.zipIdx.filter
            (fun entry => entry.1.target == (slot, port)) := by
          rw [hfilter]
          exact List.mem_singleton_self _
        have hbound := (List.mem_zipIdx (List.mem_filter.mp hmem).1).2.1
        dsimp only
        refine ⟨by omega, by omega, .inr ⟨rfl, by omega, rfl⟩⟩
      · simp at h

/-- One result's publications in an encodable arena: at most one canonical
pair, whose slot indices and generations fit the version-one pair fields,
provided the result's own slot index fits. -/
theorem PhysicalState.publicationFor_encodable {state : PhysicalState}
    {pair : PhysicalPair} {boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    {published : List (Nat × Nat × Nat × Nat)}
    (hencode : state.planIndicesEncodable = true)
    (hslot : result.2.1 < 256 ^ 4)
    (h : state.publicationFor pair boundaries result = some published) :
    published.length ≤ 1 ∧ ∀ entry ∈ published,
      entry.1 < 256 ^ 4 ∧ entry.2.1 < 256 ^ 8 ∧
        entry.2.2.1 < 256 ^ 4 ∧ entry.2.2.2 < 256 ^ 8 := by
  obtain ⟨hsize, _⟩ := PhysicalState.planIndicesEncodable_bounds hencode
  have hwidth4 : (256 : Nat) ^ 4 = 2 ^ 32 := by decide
  have hwidth8 : (256 : Nat) ^ 8 = 2 ^ 64 := by decide
  have hmaxNat : (UInt64.ofNat (UInt64.size - 1)).toNat = 2 ^ 64 - 1 := by decide
  unfold PhysicalState.publicationFor at h
  obtain ⟨first, _, h⟩ := Option.bind_eq_some_iff.mp h
  cases first with
    | none => simp at h
    | some index =>
      simp only at h
      obtain ⟨boundary, _, h⟩ := Option.bind_eq_some_iff.mp h
      obtain ⟨source, kind, peer, peerPort⟩ := boundary
      by_cases hquiet : (kind != 0 || peerPort != 0) = true
      · simp only [hquiet, ↓reduceIte, Option.some.injEq] at h
        subst h
        simp
      · simp only [hquiet, Bool.false_eq_true, ↓reduceIte] at h
        by_cases hconsumed :
            (peer == pair.first || peer == pair.second || peer == result.2.1) = true
        · simp [hconsumed] at h
        · simp only [hconsumed, Bool.false_eq_true, ↓reduceIte] at h
          obtain ⟨other, hother, h⟩ := Option.bind_eq_some_iff.mp h
          have hpeer : peer < state.slots.size := (Array.getElem?_eq_some_iff.mp hother).1
          by_cases hdead : other.agent.isNone = true
          · simp [hdead] at h
          · simp only [hdead, Bool.false_eq_true, ↓reduceIte] at h
            have hcurrent := UInt64.toNat_lt other.generation
            have key : ∃ generation, generation < 2 ^ 64 ∧
                ((if result.2.1 < peer then
                    some [(result.2.1, generation, peer, other.generation.toNat)]
                  else some [(peer, other.generation.toNat, result.2.1, generation)]) =
                  some published) := by
              cases hfresh : result.1
              · simp only [hfresh, Bool.false_eq_true, ↓reduceIte] at h
                obtain ⟨old, _, h⟩ := Option.bind_eq_some_iff.mp h
                by_cases hmax : old.generation = UInt64.ofNat (UInt64.size - 1)
                · simp [hmax] at h
                · have hne : old.generation.toNat ≠ 2 ^ 64 - 1 := by
                    intro heq
                    exact hmax (UInt64.toNat_inj.mp (heq.trans hmaxNat.symm))
                  have hlt := UInt64.toNat_lt old.generation
                  refine ⟨old.generation.toNat + 1, by omega, ?_⟩
                  simp at h
                  exact h.2
              · simp only [hfresh, ↓reduceIte] at h
                exact ⟨0, by decide, by simpa using h⟩
            obtain ⟨generation, hgenBound, h⟩ := key
            split at h <;>
              · simp only [Option.some.injEq] at h
                subst h
                simp only [List.length_singleton, Nat.le_refl, List.mem_singleton,
                  forall_eq, true_and]
                omega

/-- A successful option traversal keeps the input length, and every output
comes from some input. -/
private theorem mapM_some_mem {α β : Type} {f : α → Option β} :
    ∀ {inputs : List α} {outputs : List β}, inputs.mapM f = some outputs →
      outputs.length = inputs.length ∧ ∀ y ∈ outputs, ∃ x ∈ inputs, f x = some y
  | [], outputs, h => by
    simp at h
    subst h
    simp
  | x :: xs, outputs, h => by
    simp only [List.mapM_cons] at h
    obtain ⟨y, hy, h⟩ := Option.bind_eq_some_iff.mp h
    obtain ⟨ys, hys, h⟩ := Option.bind_eq_some_iff.mp h
    simp at h
    subst h
    obtain ⟨hlen, hmem⟩ := mapM_some_mem hys
    refine ⟨by simp [hlen], ?_⟩
    intro z hz
    rcases List.mem_cons.mp hz with rfl | hz
    · exact ⟨x, by simp, hy⟩
    · obtain ⟨w, hw, hfw⟩ := hmem z hz
      exact ⟨w, by simp [hw], hfw⟩

/-- The four selected rule opcodes. -/
private theorem ruleOpcode_cases {left right : NetSym} {opcode : UInt8}
    (h : BytePlan.ruleOpcode left right = some opcode) :
    opcode = 1 ∨ opcode = 2 ∨ opcode = 3 ∨ opcode = 4 := by
  cases left <;> cases right <;> simp [BytePlan.ruleOpcode] at h <;> subst h <;> decide

/-- The rule and value slots are the selected pair's two slots, in some order. -/
private theorem ruleSlots_cases {left right : NetSym} {pair : PhysicalPair}
    {ruleSlot valueSlot : Nat}
    (h : BytePlan.ruleSlots left right pair = some (ruleSlot, valueSlot)) :
    (ruleSlot = pair.first ∧ valueSlot = pair.second) ∨
      (ruleSlot = pair.second ∧ valueSlot = pair.first) := by
  cases left <;> cases right <;> simp [BytePlan.ruleSlots] at h
  all_goals
    obtain ⟨h1, h2⟩ := h
    subst h1 h2
    simp

/-- **Encodable candidates are admissible.** Every candidate the model emitter
assembles in an arena inside the version-one 32-bit index envelope satisfies
`BytePlan.Admissible`, for every selected rule, orientation and storage
policy, without any legality premise. -/
theorem PhysicalState.planCandidate_admissible
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    {reuse : Bool} {plan : BytePlan}
    (hencode : state.planIndicesEncodable = true)
    (h : state.planCandidate pair redex reuse = some plan) :
    plan.Admissible := by
  obtain ⟨hsize, houts⟩ := PhysicalState.planIndicesEncodable_bounds hencode
  have hwidth4 : (256 : Nat) ^ 4 = 2 ^ 32 := by decide
  have hwidth8 : (256 : Nat) ^ 8 = 2 ^ 64 := by decide
  have hbyte : UInt8.size = 256 := by decide
  cases hc : state.planCandidateCore pair redex reuse with
  | none => simp [PhysicalState.planCandidate, hc] at h
  | some candidate =>
    simp [PhysicalState.planCandidate, hc] at h
    subst h
    unfold PhysicalState.planCandidateCore at hc
    obtain ⟨rhs, _, hc⟩ := Option.bind_eq_some_iff.mp hc
    simp at hc
    obtain ⟨_, hc⟩ := hc
    obtain ⟨first, hf, hc⟩ := Option.bind_eq_some_iff.mp hc
    obtain ⟨second, hs, hc⟩ := Option.bind_eq_some_iff.mp hc
    simp at hc
    obtain ⟨_, hc⟩ := hc
    obtain ⟨opcode, ho, hc⟩ := Option.bind_eq_some_iff.mp hc
    obtain ⟨slots, hp, hc⟩ := Option.bind_eq_some_iff.mp hc
    obtain ⟨ruleSlot, valueSlot⟩ := slots
    have hfirst : pair.first < state.slots.size := (Array.getElem?_eq_some_iff.mp hf).1
    have hsecond : pair.second < state.slots.size := (Array.getElem?_eq_some_iff.mp hs).1
    have hruleSlot : ruleSlot < state.slots.size := by
      rcases ruleSlots_cases hp with ⟨rfl, _⟩ | ⟨rfl, _⟩ <;> assumption
    have hvalueSlot : valueSlot < state.slots.size := by
      rcases ruleSlots_cases hp with ⟨_, rfl⟩ | ⟨_, rfl⟩ <;> assumption
    unfold PhysicalState.candidateBody at hc
    obtain ⟨boundaries, hb, hc⟩ := Option.bind_eq_some_iff.mp hc
    by_cases ha : (state.consumedBoundaryAlias pair redex ||
        !(decide (boundaries.map Prod.snd).Nodup)) = true
    · simp [ha] at hc
    simp [ha] at hc
    obtain ⟨symbols, _, hc⟩ := Option.bind_eq_some_iff.mp hc
    simp at hc
    obtain ⟨hlen, hc⟩ := hc
    obtain ⟨results, hr, hc⟩ := Option.bind_eq_some_iff.mp hc
    obtain ⟨publications, hpub, hc⟩ := Option.bind_eq_some_iff.mp hc
    cases Option.some.inj hc
    clear hc
    obtain ⟨hbLen, hbMem⟩ := mapM_some_mem hb
    have hboundaryLen : boundaries.length ≤ 2 := by
      rw [hbLen]
      split <;> (try split) <;> simp
    have hboundaryValid : ∀ boundary ∈ boundaries,
        boundary.1.1 < 256 ^ 4 ∧ boundary.1.2 < UInt8.size ∧
          ((boundary.2.1 = 0 ∧ boundary.2.2.1 < 256 ^ 4 ∧ boundary.2.2.2 < UInt8.size) ∨
            (boundary.2.1 = 1 ∧ boundary.2.2.1 < 256 ^ 4 ∧ boundary.2.2.2 = 0)) := by
      intro boundary hmem
      obtain ⟨port, hport, hcap⟩ := hbMem boundary hmem
      have hportByte : port < UInt8.size := by
        rw [hbyte]
        split at hport <;> (try split at hport) <;> simp at hport <;> omega
      exact PhysicalState.captureAux_encodable hencode hportByte hcap
    have hpairValid : pair.first < 256 ^ 4 ∧ pair.firstGeneration.toNat < 256 ^ 8 ∧
        pair.second < 256 ^ 4 ∧ pair.secondGeneration.toNat < 256 ^ 8 := by
      have := UInt64.toNat_lt pair.firstGeneration
      have := UInt64.toNat_lt pair.secondGeneration
      omega
    rcases ruleOpcode_cases ho with rfl | rfl | rfl | rfl
    · -- OP2: one `op1` result reading both captured endpoints.
      simp at hlen hr
      obtain ⟨symbol, rfl⟩ := List.length_eq_one_iff.mp hlen
      simp at hr
      subst hr
      simp [List.flatMapM, List.flatMapM.loop] at hpub
      have hresultSlot : (if reuse = true then ruleSlot else state.slots.size) < 256 ^ 4 := by
        split <;> omega
      obtain ⟨hpubLen, hpubValid⟩ :=
        PhysicalState.publicationFor_encodable hencode hresultSlot hpub
      apply BytePlan.admissible_of_bounds
      all_goals simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]
      all_goals first
        | (split <;> decide)
        | decide
        | omega
        | (simpa using hpairValid)
        | (intro a b c d e hmem; simpa using hboundaryValid _ hmem)
        | (intro a b c d hmem; simpa using hpubValid _ hmem)
        | (refine ⟨?_, by omega⟩; split <;> omega)
        | (cases reuse <;> simp)
    · -- OP1: one number result reading the captured endpoint.
      simp at hlen hr
      obtain ⟨symbol, rfl⟩ := List.length_eq_one_iff.mp hlen
      simp at hr
      subst hr
      simp [List.flatMapM, List.flatMapM.loop] at hpub
      have hresultSlot : (if reuse = true then ruleSlot else state.slots.size) < 256 ^ 4 := by
        split <;> omega
      obtain ⟨hpubLen, hpubValid⟩ :=
        PhysicalState.publicationFor_encodable hencode hresultSlot hpub
      apply BytePlan.admissible_of_bounds
      all_goals simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]
      all_goals first
        | (split <;> decide)
        | decide
        | omega
        | (simpa using hpairValid)
        | (intro a b c d e hmem; simpa using hboundaryValid _ hmem)
        | (intro a b c d hmem; simpa using hpubValid _ hmem)
        | (refine ⟨?_, by omega⟩; split <;> omega)
        | (cases reuse <;> simp)
    · -- DUP: two number results, one per captured endpoint.
      simp at hlen hr
      rcases symbols with _ | ⟨symbolOne, _ | ⟨symbolTwo, _ | ⟨symbolThree, rest⟩⟩⟩ <;>
        simp at hlen
      simp at hr
      subst hr
      simp only [List.flatMapM, List.flatMapM.loop] at hpub
      obtain ⟨publishedOne, hpubOne, hpub⟩ := Option.bind_eq_some_iff.mp hpub
      obtain ⟨publishedTwo, hpubTwo, hpub⟩ := Option.bind_eq_some_iff.mp hpub
      simp at hpub
      subst hpub
      have hslotOne : (if reuse = true then ruleSlot else state.slots.size) < 256 ^ 4 := by
        split <;> omega
      have hslotTwo : (if reuse = true then valueSlot else state.slots.size + 1) < 256 ^ 4 := by
        split <;> omega
      obtain ⟨hlenOne, hvalidOne⟩ :=
        PhysicalState.publicationFor_encodable hencode hslotOne hpubOne
      obtain ⟨hlenTwo, hvalidTwo⟩ :=
        PhysicalState.publicationFor_encodable hencode hslotTwo hpubTwo
      apply BytePlan.admissible_of_bounds
      all_goals simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]
      all_goals first
        | (split <;> decide)
        | decide
        | omega
        | (simpa using hpairValid)
        | (intro a b c d e hmem; simpa using hboundaryValid _ hmem)
        | (refine ⟨⟨?_, by omega⟩, ?_, by omega⟩ <;> split <;> omega)
        | (cases reuse <;> simp)
        | (intro a b c d hmem
           rcases hmem with hmem | hmem
           · simpa using hvalidOne _ hmem
           · simpa using hvalidTwo _ hmem)
    · -- ERA: no results, boundaries or publications.
      simp at hlen hr
      subst hlen
      simp at hr
      subst hr
      simp [List.flatMapM, List.flatMapM.loop] at hpub
      subst hpub
      apply BytePlan.admissible_of_bounds
      all_goals simp [BytePlan.withRequiredWrites, BytePlan.requiredWrites]
      all_goals first
        | (split <;> decide)
        | decide
        | omega
        | (simpa using hpairValid)
        | (intro a b c d e hmem; simpa using hboundaryValid _ hmem)

/-- **The byte-roundtrip exclusion is uninhabited.** In an arena inside the
version-one index envelope, every model-emitter candidate decodes back to
exactly itself. -/
theorem PhysicalState.planCandidate_decodes
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    {reuse : Bool} {plan : BytePlan}
    (hencode : state.planIndicesEncodable = true)
    (h : state.planCandidate pair redex reuse = some plan) :
    BytePlan.decode plan.emit = some plan :=
  BytePlan.decode_emit plan (PhysicalState.planCandidate_admissible hencode h)

/-- Lowering an encodable pair with generation headroom returns exactly the
model-emitter candidate: its byte-roundtrip check never refuses. -/
theorem PhysicalState.lowerPlan_eq_candidate
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    {reuse : Bool}
    (hencode : state.planIndicesEncodable = true)
    (hroom : state.pairHeadroom pair = true) :
    state.lowerPlan pair redex reuse = state.planCandidate pair redex reuse := by
  cases hc : state.planCandidate pair redex reuse with
  | none => simp [PhysicalState.lowerPlan, hencode, hroom, hc]
  | some plan =>
    simp [PhysicalState.lowerPlan, hencode, hroom, hc,
      PhysicalState.planCandidate_decodes hencode hc]

end NobleFoundation.NetI64Plans
