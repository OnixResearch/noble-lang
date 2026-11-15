import NobleFoundation.NetI64PlanRewrite

/-!
# Projected pending queue after a local rewrite (issue #110)

From the rule-independent `PhysicalState.LocalRewrite` bundle, the executed
arena's generation-checked directed queue is a permutation of the abstract
commit's live-agent pending pairs: old entries touching a consumed slot go
stale, untouched entries survive in order, and each published canonical pair
contributes exactly the two directed principal pairs of the abstract image.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- A generation with headroom increments by exactly one as a natural number. -/
private theorem generation_succ_toNat {generation : UInt64}
    (hroom : generation ≠ UInt64.ofNat (UInt64.size - 1)) :
    (generation + 1).toNat = generation.toNat + 1 := by
  have hmax : generation.toNat ≠ UInt64.size - 1 := by
    intro he
    apply hroom
    rw [← UInt64.ofNat_toNat (x := generation), he]
  have hlt : generation.toNat + 1 < UInt64.size := by
    have hb := UInt64.toNat_lt generation
    have hsize : UInt64.size = 2 ^ 64 := rfl
    omega
  rw [UInt64.toNat_add, UInt64.toNat_ofNat]
  simp only [Nat.one_mod]
  rw [Nat.mod_eq_of_lt (by simpa [UInt64.size] using hlt)]

/-- Members of a list with distinct images under a map are determined by
their images. -/
private theorem eq_of_mem_of_map_nodup {α β : Type} {f : α → β} :
    ∀ {l : List α}, (l.map f).Nodup → ∀ {a b : α}, a ∈ l → b ∈ l → f a = f b → a = b
  | [], _, _, _, ha, _, _ => absurd ha List.not_mem_nil
  | x :: rest, hnd, a, b, ha, hb, hab => by
    rw [List.map_cons, List.nodup_cons] at hnd
    rcases List.mem_cons.mp ha with rfl | ha'
    · rcases List.mem_cons.mp hb with rfl | hb'
      · rfl
      · exact absurd (List.mem_map.mpr ⟨b, hb', hab.symm⟩) hnd.1
    · rcases List.mem_cons.mp hb with rfl | hb'
      · exact absurd (List.mem_map.mpr ⟨a, ha', hab⟩) hnd.1
      · exact eq_of_mem_of_map_nodup hnd.2 ha' hb' hab

/-- `flatMap` only reads its function on list members. -/
private theorem flatMap_congr_mem {α β : Type} {f g : α → List β} :
    ∀ {l : List α}, (∀ a ∈ l, f a = g a) → l.flatMap f = l.flatMap g
  | [], _ => rfl
  | a :: rest, h => by
    rw [List.flatMap_cons, List.flatMap_cons, h a List.mem_cons_self,
      flatMap_congr_mem (fun b hb => h b (List.mem_cons_of_mem a hb))]

/-- A filter-map that is injective on the members it keeps preserves `Nodup`. -/
private theorem nodup_filterMap {α β : Type} {f : α → Option β} {l : List α}
    (hl : l.Nodup)
    (hinj : ∀ a ∈ l, ∀ a' ∈ l, ∀ b, f a = some b → f a' = some b → a = a') :
    (l.filterMap f).Nodup := by
  apply List.pairwise_filterMap.mpr
  refine List.Pairwise.imp_of_mem ?_ hl
  intro a a' ha ha' hne b hb b' hb' hbb
  subst hbb
  exact hne (hinj a ha a' ha' b hb hb')

/-- Membership in the principal pairs of a wire list. -/
private theorem mem_principalPairs_iff {ws : List (Port × Port)} {x : Id × Id} :
    x ∈ principalPairs ws ↔ ∃ w ∈ ws, w.1.2 = 0 ∧ w.2.2 = 0 ∧ x = (w.1.1, w.2.1) := by
  unfold principalPairs
  rw [List.mem_filterMap]
  constructor
  · rintro ⟨w, hw, hx⟩
    by_cases hc : w.1.2 = 0 ∧ w.2.2 = 0
    · rw [ite_eq_left hc] at hx
      exact ⟨w, hw, hc.1, hc.2, (Option.some.inj hx).symm⟩
    · rw [ite_eq_right hc] at hx
      cases hx
  · rintro ⟨w, hw, h1, h2, rfl⟩
    exact ⟨w, hw, by rw [ite_eq_left ⟨h1, h2⟩]⟩

/-- The kept image of one wire under the principal-pair filter. -/
private theorem principal_some {w : Port × Port} {x : Id × Id}
    (h : (if w.1.2 = 0 ∧ w.2.2 = 0 then some (w.1.1, w.2.1) else none) = some x) :
    w.1.2 = 0 ∧ w.2.2 = 0 ∧ x = (w.1.1, w.2.1) := by
  by_cases hc : w.1.2 = 0 ∧ w.2.2 = 0
  · rw [ite_eq_left hc] at h
    exact ⟨hc.1, hc.2, (Option.some.inj h).symm⟩
  · rw [ite_eq_right hc] at h
    cases h

/-- A decoded queue entry names two current live endpoint slots. -/
private theorem logicalPair_some {st : PhysicalState} {pair : PhysicalPair}
    {left right : Id} (h : st.logicalPair pair = some (left, right)) :
    ∃ first second firstSymbol secondSymbol,
      st.slots[pair.first]? = some first ∧ st.slots[pair.second]? = some second ∧
      first.agent = some (left, firstSymbol) ∧ second.agent = some (right, secondSymbol) := by
  unfold PhysicalState.logicalPair at h
  cases hf : st.slots[pair.first]? with
  | none => simp [hf] at h
  | some first =>
    cases hs : st.slots[pair.second]? with
    | none => simp [hf, hs] at h
    | some second =>
      cases ha : first.agent with
      | none => simp [hf, hs, ha] at h
      | some fa =>
        cases hb : second.agent with
        | none => simp [hf, hs, hb] at h
        | some sb =>
          obtain ⟨l, ls⟩ := fa
          obtain ⟨r, rs⟩ := sb
          simp [hf, hs, ha, hb] at h
          obtain ⟨-, rfl, rfl⟩ := h
          exact ⟨first, second, ls, rs, rfl, rfl, ha, hb⟩

/-- An entry whose first endpoint is dead or of another generation is stale. -/
private theorem logicalPair_none_first {st : PhysicalState} {pair : PhysicalPair}
    {slot : PhysicalSlot} (hslot : st.slots[pair.first]? = some slot)
    (hbad : slot.agent = none ∨ slot.generation ≠ pair.firstGeneration) :
    st.logicalPair pair = none := by
  rcases hbad with hdead | hstale
  · exact PhysicalState.logicalPair_dead_endpoint (Or.inl (by simp [hslot, hdead]))
  · exact PhysicalState.stale_first_refused hslot hstale

/-- An entry whose second endpoint is dead or of another generation is stale. -/
private theorem logicalPair_none_second {st : PhysicalState} {pair : PhysicalPair}
    {slot : PhysicalSlot} (hslot : st.slots[pair.second]? = some slot)
    (hbad : slot.agent = none ∨ slot.generation ≠ pair.secondGeneration) :
    st.logicalPair pair = none := by
  rcases hbad with hdead | hstale
  · exact PhysicalState.logicalPair_dead_endpoint (Or.inr (by simp [hslot, hdead]))
  · cases hf : st.slots[pair.first]? with
    | none => simp [PhysicalState.logicalPair, hf]
    | some first => simp [PhysicalState.logicalPair, hf, hslot, hstale]

/-- Two arenas agreeing on both endpoint agents and generations decode an
entry identically. -/
private theorem logicalPair_congr {st st' : PhysicalState} {pair : PhysicalPair}
    {first second first' second' : PhysicalSlot}
    (hf : st.slots[pair.first]? = some first) (hs : st.slots[pair.second]? = some second)
    (hf' : st'.slots[pair.first]? = some first') (hs' : st'.slots[pair.second]? = some second')
    (haf : first'.agent = first.agent) (hgf : first'.generation = first.generation)
    (has : second'.agent = second.agent) (hgs : second'.generation = second.generation) :
    st'.logicalPair pair = st.logicalPair pair := by
  simp [PhysicalState.logicalPair, hf, hs, hf', hs', haf, hgf, has, hgs]

/-- Membership in the two orientations of a pair, listed in either order. -/
private theorem mem_orientations {α : Type} {c : Prop} [Decidable c] {a b : α}
    {x : α × α} :
    x ∈ (if c then [(a, b), (b, a)] else [(b, a), (a, b)]) ↔ x = (a, b) ∨ x = (b, a) := by
  split
  · simp
  · simp [or_comm]

/-- The two orientations of a pair of distinct identities are distinct. -/
private theorem nodup_orientations {α : Type} {c : Prop} [Decidable c] {a b : α}
    (hab : a ≠ b) :
    (if c then [(a, b), (b, a)] else [(b, a), (a, b)]).Nodup := by
  split
  · simp [List.nodup_cons, hab]
  · simp [List.nodup_cons, Ne.symm hab]

section

variable {state next : PhysicalState} {selected : Plan}
  {ruleSlot valueSlot : Nat} {ruleId valueId : Id}
  {ruleSymbol valueSymbol : NetSym}
  {results : List ResultView} {edges : List EdgeView}
  {retired : List Nat} {fresh : Nat} {published : List PhysicalPair}

/-- Both consumed identities are live in the prestate. -/
private theorem PhysicalState.LocalRewrite.queue_consumed_live
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) {id : Id}
    (hid : id = ruleId ∨ id = valueId) : id ∈ state.liveIds := by
  rcases hid with hid | hid
  · obtain ⟨old, hold, hagent, -⟩ := h.ruleOld
    exact PhysicalState.mem_liveIds.mpr ⟨ruleSlot, old, ruleSymbol, hold, hid ▸ hagent⟩
  · obtain ⟨old, hold, hagent, -⟩ := h.valueOld
    exact PhysicalState.mem_liveIds.mpr ⟨valueSlot, old, valueSymbol, hold, hid ▸ hagent⟩

/-- One abstract committed agent lookup, read through the bundle's consumed
identities and installed results. -/
private theorem PhysicalState.LocalRewrite.queue_agents
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) (id : Id) :
    (Plan.commit state.project selected state.project.work).agents[id]? =
      (assoc id (results.map fun result => (result.id, result.symbol))).or
        (if id = ruleId ∨ id = valueId then none else state.agents[id]?) := by
  have hdistinct : (selected.initializedAgents.map Prod.fst).Nodup := by
    rw [h.selectedAgents, List.map_map]
    exact h.resultIds
  rw [Plan.commit_agents_getElem? _ _ _ _ hdistinct, h.selectedAgents]
  rcases h.selectedPair with ⟨h1, h2⟩ | ⟨h1, h2⟩
  · rw [h1, h2]
    rfl
  · rw [h1, h2]
    by_cases hc : id = ruleId ∨ id = valueId
    · rw [ite_eq_left hc, ite_eq_left (Or.comm.mp hc)]
    · rw [ite_eq_right hc, ite_eq_right (fun hc' => hc (Or.comm.mp hc'))]
      rfl

/-- The consumed identities are dead in the abstract commit. -/
private theorem PhysicalState.LocalRewrite.queue_dead
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) {id : Id}
    (hid : id = ruleId ∨ id = valueId) :
    (Plan.commit state.project selected state.project.work).agents[id]? = none := by
  have hlive := h.queue_consumed_live hid
  rw [h.queue_agents, ite_eq_left hid, assoc_none_of_forall]
  · rfl
  · intro entry hentry heq
    obtain ⟨result, hr, rfl⟩ := List.mem_map.mp hentry
    apply (h.resultFresh result hr).1
    rw [show result.id = id from heq]
    exact hlive

/-- Every installed result identity is live in the abstract commit. -/
private theorem PhysicalState.LocalRewrite.queue_live_result
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {result : ResultView} (hr : result ∈ results) :
    ((Plan.commit state.project selected state.project.work).agents[result.id]?).isSome =
      true := by
  rw [h.queue_agents, Option.isSome_or]
  obtain ⟨symbol, hsymbol⟩ := assoc_of_mem
    (List.mem_map.mpr ⟨result, hr, rfl⟩ :
      (result.id, result.symbol) ∈ results.map fun result => (result.id, result.symbol))
  rw [hsymbol]
  rfl

/-- A prestate agent outside the consumed slots is live in the abstract
commit. -/
private theorem PhysicalState.LocalRewrite.queue_live_state
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol))
    (hrule : index ≠ ruleSlot) (hvalue : index ≠ valueSlot) :
    ((Plan.commit state.project selected state.project.work).agents[id]?).isSome =
      true := by
  have hunique : state.liveIds.Nodup := h.legal.2.2.2.2.2.1
  have hnot : ¬(id = ruleId ∨ id = valueId) := by
    rintro (hid | hid)
    · obtain ⟨old, hold, holdAgent, -⟩ := h.ruleOld
      exact hrule (PhysicalState.slot_index_unique hunique hslot hagent hold
        (hid ▸ holdAgent))
    · obtain ⟨old, hold, holdAgent, -⟩ := h.valueOld
      exact hvalue (PhysicalState.slot_index_unique hunique hslot hagent hold
        (hid ▸ holdAgent))
  rw [h.queue_agents, ite_eq_right hnot,
    PhysicalState.agents_getElem?_of_slot hunique hslot hagent, Option.isSome_or]
  simp

/-- Output label identities are dead in the abstract commit. -/
private theorem PhysicalState.LocalRewrite.queue_dead_label
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {out : PhysicalOutput} (hout : out ∈ state.outputs) :
    (Plan.commit state.project selected state.project.work).agents[out.label.1]? = none := by
  have hdead := PhysicalState.agents_getElem?_of_not_live
    (h.legal.keysSeparated.labelDead out hout)
  rw [h.queue_agents, assoc_none_of_forall]
  · by_cases hc : out.label.1 = ruleId ∨ out.label.1 = valueId
    · rw [ite_eq_left hc]
      rfl
    · rw [ite_eq_right hc, hdead]
      rfl
  · intro entry hentry heq
    obtain ⟨result, hr, rfl⟩ := List.mem_map.mp hentry
    exact (h.resultFresh result hr).2 out hout heq.symm

/-- A consumed slot holding a bounded queued generation is dead or of a newer
generation in the post-state. -/
private theorem PhysicalState.LocalRewrite.queue_consumed_stale
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} (hindex : index = ruleSlot ∨ index = valueSlot)
    {old : PhysicalSlot} (hold : state.slots[index]? = some old)
    {generation : UInt64} (hgen : generation.toNat ≤ old.generation.toNat) :
    ∃ slot, next.slots[index]? = some slot ∧
      (slot.agent = none ∨ slot.generation ≠ generation) := by
  by_cases hres : ∃ result ∈ results, result.slot = index
  · obtain ⟨result, hr, hslot⟩ := hres
    obtain ⟨slot, hnext, -, hslotGen, -⟩ := h.observed.resultAfter result hr
    obtain ⟨hroom, hresultGen⟩ := h.resultGeneration result hr old (by rw [hslot]; exact hold)
    refine ⟨slot, by rw [← hslot]; exact hnext, Or.inr ?_⟩
    intro heq
    have hsucc := generation_succ_toNat hroom
    have hnat : generation.toNat = old.generation.toNat + 1 := by
      rw [← heq, hslotGen, hresultGen, hsucc]
    omega
  · have hret : index ∈ retired := (h.retiredExact index).mpr
      ⟨hindex, fun result hr he => hres ⟨result, hr, he⟩⟩
    obtain ⟨-, slot, -, hnext, hdead, -⟩ := h.observed.retiredAfter index hret
    exact ⟨slot, hnext, Or.inl hdead⟩

/-- A live agent in a consumed slot carries one of the consumed identities. -/
private theorem PhysicalState.LocalRewrite.queue_consumed_id
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} (hindex : index = ruleSlot ∨ index = valueSlot)
    {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol)) :
    id = ruleId ∨ id = valueId := by
  rcases hindex with rfl | rfl
  · obtain ⟨old, hold, holdAgent, -⟩ := h.ruleOld
    rw [hold] at hslot
    cases hslot
    rw [holdAgent] at hagent
    cases hagent
    exact Or.inl rfl
  · obtain ⟨old, hold, holdAgent, -⟩ := h.valueOld
    rw [hold] at hslot
    cases hslot
    rw [holdAgent] at hagent
    cases hagent
    exact Or.inr rfl

/-- An old queue entry decodes in the post-state exactly to its prestate
orientations that survive in the abstract committed net. -/
private theorem PhysicalState.LocalRewrite.queue_old
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {pair : PhysicalPair} (hp : pair ∈ state.work) :
    next.pairOrientations pair = (state.pairOrientations pair).filter fun directed =>
      ((Plan.commit state.project selected state.project.work).agents[directed.1]?).isSome &&
      ((Plan.commit state.project selected state.project.work).agents[directed.2]?).isSome := by
  obtain ⟨⟨first, hf, hfg⟩, ⟨second, hs, hsg⟩⟩ :=
    PhysicalState.queuedGenerationsBound_iff.mp h.legal.2.2.2.2.1 pair hp
  by_cases hc : (pair.first = ruleSlot ∨ pair.first = valueSlot) ∨
      (pair.second = ruleSlot ∨ pair.second = valueSlot)
  · have hnone : next.logicalPair pair = none := by
      rcases hc with hc | hc
      · obtain ⟨slot, hslot, hbad⟩ := h.queue_consumed_stale hc hf hfg
        exact logicalPair_none_first hslot hbad
      · obtain ⟨slot, hslot, hbad⟩ := h.queue_consumed_stale hc hs hsg
        exact logicalPair_none_second hslot hbad
    unfold PhysicalState.pairOrientations
    rw [hnone]
    cases hst : state.logicalPair pair with
    | none => rfl
    | some ab =>
      obtain ⟨left, right⟩ := ab
      obtain ⟨first', second', fs, ss, hf', hs', hfa, hsa⟩ := logicalPair_some hst
      rcases hc with hc | hc
      · have hdead := h.queue_dead (h.queue_consumed_id hc hf' hfa)
        simp only [List.filter_cons, List.filter_nil, hdead, Option.isSome_none,
          Bool.false_and, Bool.and_false, Bool.false_eq_true, ↓reduceIte]
      · have hdead := h.queue_dead (h.queue_consumed_id hc hs' hsa)
        simp only [List.filter_cons, List.filter_nil, hdead, Option.isSome_none,
          Bool.false_and, Bool.and_false, Bool.false_eq_true, ↓reduceIte]
  · simp only [not_or] at hc
    obtain ⟨⟨hf1, hf2⟩, hs1, hs2⟩ := hc
    obtain ⟨nf, hnf, hnfa, hnfg, -⟩ := h.observed.otherAfter pair.first first hf hf1 hf2
    obtain ⟨ns, hns, hnsa, hnsg, -⟩ := h.observed.otherAfter pair.second second hs hs1 hs2
    have heq : next.logicalPair pair = state.logicalPair pair :=
      logicalPair_congr hf hs hnf hns hnfa hnfg hnsa hnsg
    unfold PhysicalState.pairOrientations
    rw [heq]
    cases hst : state.logicalPair pair with
    | none => rfl
    | some ab =>
      obtain ⟨left, right⟩ := ab
      obtain ⟨first', second', fs, ss, hf', hs', hfa, hsa⟩ := logicalPair_some hst
      have hleft := h.queue_live_state hf' hfa hf1 hf2
      have hright := h.queue_live_state hs' hsa hs1 hs2
      simp only [List.filter_cons, List.filter_nil, hleft, hright, Bool.and_self,
        ↓reduceIte]

/-- A captured live-peer edge names a live prestate slot outside the consumed
pair, and its endpoint is that slot's logical port. -/
private theorem PhysicalState.LocalRewrite.queue_peer_edge
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {peer port : Nat}
    (htarget : edge.target = (0, peer, port)) :
    ∃ peerSlot peerId peerSymbol, peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
      state.slots[peer]? = some peerSlot ∧ peerSlot.agent = some (peerId, peerSymbol) ∧
      edge.endpoint = (peerId, port) := by
  rcases h.edgeCapture edge he with
    ⟨peer', port', peerSlot, peerId, peerSymbol, htarget', hpr, hpv, hpeer, hagent, -, hend⟩ |
    ⟨output, out, htarget', -, -, -⟩
  · rw [htarget] at htarget'
    cases htarget'
    exact ⟨peerSlot, peerId, peerSymbol, hpr, hpv, hpeer, hagent, hend⟩
  · rw [htarget] at htarget'
    simp at htarget'

/-- An edge's recorded result identity is that of the result installed at its
result slot. -/
private theorem PhysicalState.LocalRewrite.queue_edge_resultId
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {result : ResultView} (hr : result ∈ results)
    (hslot : result.slot = edge.result.1) : edge.resultId = result.id := by
  obtain ⟨other, hother, hotherSlot, hid, -⟩ := h.edgeResult edge he
  rw [hid, eq_of_mem_of_map_nodup h.resultSlots hother hr (hotherSlot.trans hslot.symm)]

/-- No captured endpoint carries an installed result identity. -/
private theorem PhysicalState.LocalRewrite.queue_endpoint_ne
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {result : ResultView} (hr : result ∈ results) :
    edge.endpoint.1 ≠ result.id := by
  rcases h.edgeCapture edge he with
    ⟨peer, port, peerSlot, peerId, peerSymbol, -, -, -, hpeer, hagent, -, hend⟩ |
    ⟨output, out, -, hout, -, hend⟩
  · rw [hend]
    intro heq
    apply (h.resultFresh result hr).1
    rw [← show peerId = result.id from heq]
    exact PhysicalState.mem_liveIds.mpr ⟨peer, peerSlot, peerSymbol, hpeer, hagent⟩
  · rw [hend]
    exact (h.resultFresh result hr).2 out (List.mem_of_getElem? hout)

/-- Two edges at the same result port index with the same result identity
coincide. -/
private theorem PhysicalState.LocalRewrite.queue_edge_eq
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {a b : EdgeView} (ha : a ∈ edges) (hb : b ∈ edges)
    (hport : a.result.2 = b.result.2) (hid : a.resultId = b.resultId) : a = b := by
  obtain ⟨ra, hra, hraSlot, hraId, -⟩ := h.edgeResult a ha
  obtain ⟨rb, hrb, hrbSlot, hrbId, -⟩ := h.edgeResult b hb
  have hsame : ra = rb :=
    eq_of_mem_of_map_nodup h.resultIds hra hrb (hraId.symm.trans (hid.trans hrbId))
  subst hsame
  exact eq_of_mem_of_map_nodup h.edgeResults ha hb
    (Prod.ext (hraSlot.symm.trans hrbSlot) hport)

/-- The post-state decodes the canonical pair of an installed result and a
captured live peer to both of their identities, in canonical slot order. -/
private theorem PhysicalState.LocalRewrite.queue_orientations
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {result : ResultView} (hr : result ∈ results)
    {peer : Nat} {peerSlot : PhysicalSlot} {peerId : Id} {peerSymbol : NetSym}
    (hpr : peer ≠ ruleSlot) (hpv : peer ≠ valueSlot)
    (hpeer : state.slots[peer]? = some peerSlot)
    (hagent : peerSlot.agent = some (peerId, peerSymbol)) :
    next.pairOrientations
        (PhysicalState.canonicalPair result.slot result.generation peer peerSlot.generation) =
      if result.slot < peer then [(result.id, peerId), (peerId, result.id)]
      else [(peerId, result.id), (result.id, peerId)] := by
  obtain ⟨slot, hslot, hslotAgent, hslotGen, -⟩ := h.observed.resultAfter result hr
  obtain ⟨other, hother, hotherAgent, hotherGen, -⟩ :=
    h.observed.otherAfter peer peerSlot hpeer hpr hpv
  rw [hagent] at hotherAgent
  unfold PhysicalState.canonicalPair
  by_cases hlt : result.slot < peer
  · simp [hlt, PhysicalState.pairOrientations, PhysicalState.logicalPair, hslot, hother,
      hslotAgent, hotherAgent, hslotGen, hotherGen]
  · simp [hlt, PhysicalState.pairOrientations, PhysicalState.logicalPair, hslot, hother,
      hslotAgent, hotherAgent, hslotGen, hotherGen]

/-- Each published pair is the canonical pair of an installed result whose
principal port is captured by a live peer's principal port. -/
private theorem PhysicalState.LocalRewrite.queue_published
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {pair : PhysicalPair} (hp : pair ∈ published) :
    ∃ result ∈ results, ∃ edge ∈ edges, ∃ peer peerSlot peerId peerSymbol,
      edge.result = (result.slot, 0) ∧ edge.endpoint = (peerId, 0) ∧
      edge.resultId = result.id ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
      state.slots[peer]? = some peerSlot ∧ peerSlot.agent = some (peerId, peerSymbol) ∧
      pair = PhysicalState.canonicalPair result.slot result.generation peer
        peerSlot.generation := by
  obtain ⟨result, hr, edge, he, peer, peerSlot, hres, htarget, hpeer, rfl⟩ :=
    (h.publishedExact pair).mp hp
  obtain ⟨peerSlot', peerId, peerSymbol, hpr, hpv, hpeer', hagent, hend⟩ :=
    h.queue_peer_edge he htarget
  rw [hpeer] at hpeer'
  cases hpeer'
  exact ⟨result, hr, edge, he, peer, peerSlot, peerId, peerSymbol, hres, hend,
    h.queue_edge_resultId he hr (by rw [hres]), hpr, hpv, hpeer, hagent, rfl⟩

/-- A captured principal-to-principal edge whose endpoint stays live in the
abstract commit was published, and that pair decodes to both orientations. -/
private theorem PhysicalState.LocalRewrite.queue_edge_published
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) (hresult : edge.result.2 = 0)
    (hend : edge.endpoint.2 = 0)
    (hlive : ((Plan.commit state.project selected state.project.work).agents[
      edge.endpoint.1]?).isSome = true) :
    ∃ pair ∈ published, ∀ x, x ∈ next.pairOrientations pair ↔
      x = (edge.resultId, edge.endpoint.1) ∨ x = (edge.endpoint.1, edge.resultId) := by
  rcases h.edgeCapture edge he with
    ⟨peer, port, peerSlot, peerId, peerSymbol, htarget, hpr, hpv, hpeer, hagent, -,
      hendpoint⟩ |
    ⟨output, out, -, hout, -, hendpoint⟩
  · rw [hendpoint] at hend ⊢
    change port = 0 at hend
    subst hend
    obtain ⟨result, hr, hslot, hid, -⟩ := h.edgeResult edge he
    refine ⟨PhysicalState.canonicalPair result.slot result.generation peer
        peerSlot.generation,
      (h.publishedExact _).mpr ⟨result, hr, edge, he, peer, peerSlot,
        Prod.ext hslot.symm hresult, htarget, hpeer, rfl⟩, fun x => ?_⟩
    rw [h.queue_orientations hr hpr hpv hpeer hagent, mem_orientations, hid]
  · rw [hendpoint, h.queue_dead_label (List.mem_of_getElem? hout)] at hlive
    simp at hlive

/-- The published canonical pairs decode to exactly the live directed
principal pairs of the abstract wire writes, each once. -/
private theorem PhysicalState.LocalRewrite.queue_new
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    (published.flatMap next.pairOrientations).Perm
      ((principalPairs selected.wireWrites).filter fun directed =>
        ((Plan.commit state.project selected state.project.work).agents[directed.1]?).isSome &&
        ((Plan.commit state.project selected state.project.work).agents[directed.2]?).isSome) := by
  have hunique : state.liveIds.Nodup := h.legal.2.2.2.2.2.1
  have hedges : edges.Nodup :=
    List.Pairwise.of_map EdgeView.result (fun _ _ hne heq => hne (congrArg _ heq))
      h.edgeResults
  rw [h.selectedWrites]
  -- the published side has no repeated directed pair
  have hleft : (published.flatMap next.pairOrientations).Nodup := by
    apply List.pairwise_flatMap.mpr
    constructor
    · intro pair hp
      obtain ⟨result, hr, edge, he, peer, peerSlot, peerId, peerSymbol, -, -, -, hpr, hpv,
        hpeer, hagent, rfl⟩ := h.queue_published hp
      rw [h.queue_orientations hr hpr hpv hpeer hagent]
      apply nodup_orientations
      intro heq
      apply (h.resultFresh result hr).1
      rw [heq]
      exact PhysicalState.mem_liveIds.mpr ⟨peer, peerSlot, peerSymbol, hpeer, hagent⟩
    · refine List.Pairwise.imp_of_mem ?_ h.publishedNodup
      intro p1 p2 hp1 hp2 hne x hx y hy hxy
      subst hxy
      obtain ⟨r1, hr1, e1, he1, peer1, peerSlot1, id1, sym1, -, -, -, hpr1, hpv1,
        hpeer1, hagent1, rfl⟩ := h.queue_published hp1
      obtain ⟨r2, hr2, e2, he2, peer2, peerSlot2, id2, sym2, -, -, -, hpr2, hpv2,
        hpeer2, hagent2, rfl⟩ := h.queue_published hp2
      rw [h.queue_orientations hr1 hpr1 hpv1 hpeer1 hagent1, mem_orientations] at hx
      rw [h.queue_orientations hr2 hpr2 hpv2 hpeer2 hagent2, mem_orientations] at hy
      have hlive1 : id1 ∈ state.liveIds :=
        PhysicalState.mem_liveIds.mpr ⟨peer1, peerSlot1, sym1, hpeer1, hagent1⟩
      have hlive2 : id2 ∈ state.liveIds :=
        PhysicalState.mem_liveIds.mpr ⟨peer2, peerSlot2, sym2, hpeer2, hagent2⟩
      have hfresh1 := (h.resultFresh r1 hr1).1
      have hfresh2 := (h.resultFresh r2 hr2).1
      have key : r1.id = r2.id ∧ id1 = id2 := by
        rcases hx with rfl | rfl <;> rcases hy with hy | hy <;>
          obtain ⟨ha, hb⟩ := Prod.mk.inj hy
        · exact ⟨ha, hb⟩
        · exact absurd (by rw [ha]; exact hlive2) hfresh1
        · exact absurd (by rw [← ha]; exact hlive1) hfresh2
        · exact ⟨hb, ha⟩
      have hr : r1 = r2 := eq_of_mem_of_map_nodup h.resultIds hr1 hr2 key.1
      have hpeer : peer1 = peer2 := PhysicalState.slot_index_unique hunique hpeer1 hagent1
        hpeer2 (by rw [key.2]; exact hagent2)
      subst hr hpeer
      rw [hpeer1] at hpeer2
      cases hpeer2
      exact hne rfl
  -- the abstract side has no repeated directed pair
  have hright : (principalPairs (edges.map EdgeView.forward ++
      edges.map EdgeView.backward)).Nodup := by
    unfold principalPairs
    rw [List.filterMap_append, List.filterMap_map, List.filterMap_map]
    refine List.nodup_append.mpr ⟨?_, ?_, ?_⟩
    · refine nodup_filterMap hedges ?_
      intro a ha a' ha' x hx hx'
      obtain ⟨h1, -, rfl⟩ := principal_some (w := EdgeView.forward a) hx
      obtain ⟨h1', -, hx'⟩ := principal_some (w := EdgeView.forward a') hx'
      exact h.queue_edge_eq ha ha' (h1.trans h1'.symm) (Prod.mk.inj hx').1
    · refine nodup_filterMap hedges ?_
      intro a ha a' ha' x hx hx'
      obtain ⟨-, h2, rfl⟩ := principal_some (w := EdgeView.backward a) hx
      obtain ⟨-, h2', hx'⟩ := principal_some (w := EdgeView.backward a') hx'
      exact h.queue_edge_eq ha ha' (h2.trans h2'.symm) (Prod.mk.inj hx').2
    · intro x hx y hy hxy
      obtain ⟨a, ha, hax⟩ := List.mem_filterMap.mp hx
      obtain ⟨b, hb, hby⟩ := List.mem_filterMap.mp hy
      obtain ⟨-, -, rfl⟩ := principal_some (w := EdgeView.forward a) hax
      obtain ⟨-, -, rfl⟩ := principal_some (w := EdgeView.backward b) hby
      obtain ⟨result, hr, -, hid, -⟩ := h.edgeResult a ha
      exact h.queue_endpoint_ne hb hr ((Prod.mk.inj hxy).1.symm.trans hid)
  apply (List.perm_ext_iff_of_nodup hleft
    ((List.filter_sublist).nodup hright)).mpr
  intro x
  simp only [List.mem_filter, List.mem_flatMap]
  constructor
  · rintro ⟨pair, hp, hx⟩
    obtain ⟨result, hr, edge, he, peer, peerSlot, peerId, peerSymbol, hres, hend, hrid,
      hpr, hpv, hpeer, hagent, rfl⟩ := h.queue_published hp
    rw [h.queue_orientations hr hpr hpv hpeer hagent, mem_orientations] at hx
    have hliveResult := h.queue_live_result hr
    have hlivePeer := h.queue_live_state hpeer hagent hpr hpv
    rcases hx with rfl | rfl
    · refine ⟨mem_principalPairs_iff.mpr ⟨EdgeView.forward edge,
        List.mem_append_left _ (List.mem_map.mpr ⟨edge, he, rfl⟩), ?_, ?_, ?_⟩, ?_⟩
      · simp [EdgeView.forward, hres]
      · simp [EdgeView.forward, hend]
      · simp [EdgeView.forward, hend, hrid]
      · simp only [hliveResult, hlivePeer, Bool.and_self]
    · refine ⟨mem_principalPairs_iff.mpr ⟨EdgeView.backward edge,
        List.mem_append_right _ (List.mem_map.mpr ⟨edge, he, rfl⟩), ?_, ?_, ?_⟩, ?_⟩
      · simp [EdgeView.backward, hend]
      · simp [EdgeView.backward, hres]
      · simp [EdgeView.backward, hend, hrid]
      · simp only [hliveResult, hlivePeer, Bool.and_self]
  · rintro ⟨hx, hlive⟩
    obtain ⟨w, hw, h1, h2, rfl⟩ := mem_principalPairs_iff.mp hx
    rw [Bool.and_eq_true] at hlive
    rcases List.mem_append.mp hw with hw | hw
    · obtain ⟨edge, he, rfl⟩ := List.mem_map.mp hw
      obtain ⟨pair, hp, hmem⟩ := h.queue_edge_published he h1 h2 hlive.2
      exact ⟨pair, hp, (hmem _).mpr (Or.inl rfl)⟩
    · obtain ⟨edge, he, rfl⟩ := List.mem_map.mp hw
      obtain ⟨pair, hp, hmem⟩ := h.queue_edge_published he h2 h1 hlive.1
      exact ⟨pair, hp, (hmem _).mpr (Or.inr rfl)⟩

/-- The projected physical queue permutes the abstract live pending pairs. -/
theorem PhysicalState.LocalRewrite.work_perm
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.project.work.Perm
      (Plan.pendingPairs (Plan.commit state.project selected state.project.work)) := by
  have hold : state.work.flatMap next.pairOrientations =
      (state.work.flatMap state.pairOrientations).filter fun directed =>
        ((Plan.commit state.project selected state.project.work).agents[directed.1]?).isSome &&
        ((Plan.commit state.project selected state.project.work).agents[directed.2]?).isSome := by
    rw [List.filter_flatMap]
    exact flatMap_congr_mem fun pair hp => h.queue_old hp
  have hnew := h.queue_new
  rw [← h.selectedActive] at hnew
  show (next.work.flatMap next.pairOrientations).Perm
    ((selected.newlyActive ++ state.work.flatMap state.pairOrientations).filter _)
  rw [h.observed.workAfter, List.flatMap_append, List.filter_append, hold]
  exact List.perm_append_comm.trans (List.Perm.append hnew (List.Perm.refl _))

end

end NobleFoundation.NetI64Plans
