import NobleFoundation.NetI64PlanRewrite

/-!
# Projected agent and wire maps of a physical local rewrite (issue #110)

From the rule-independent `PhysicalState.LocalRewrite` bundle, the executed
arena's live identities stay unique, its projected key spaces stay separated,
and every projected agent and wire lookup equals the independently compiled
abstract commit's lookup. Output targets therefore resolve to the abstract
wires of their stable labels.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- Distinct images identify list members. -/
private theorem inj_of_nodup_map {α β : Type} {f : α → β} :
    ∀ {l : List α}, (l.map f).Nodup → ∀ {x y : α}, x ∈ l → y ∈ l → f x = f y → x = y
  | [], _, _, _, hx, _, _ => nomatch hx
  | _ :: _, hd, _, _, hx, hy, hxy => by
    rw [List.map_cons, List.nodup_cons] at hd
    rcases List.mem_cons.1 hx with rfl | hx' <;> rcases List.mem_cons.1 hy with rfl | hy'
    · rfl
    · exact (hd.1 (hxy ▸ List.mem_map_of_mem hy')).elim
    · exact (hd.1 (hxy.symm ▸ List.mem_map_of_mem hx')).elim
    · exact inj_of_nodup_map hd.2 hx' hy' hxy

/-- In a duplicate-free list, equal lookups occur at one index. -/
private theorem index_eq_of_nodup {α : Type} :
    ∀ {l : List α}, l.Nodup → ∀ {i j : Nat} {a : α}, l[i]? = some a → l[j]? = some a → i = j
  | [], _, _, _, _, hi, _ => by simp at hi
  | _ :: _, hd, i, j, _, hi, hj => by
    rw [List.nodup_cons] at hd
    cases i with
    | zero =>
      cases j with
      | zero => rfl
      | succ j =>
        rw [List.getElem?_cons_zero] at hi
        rw [List.getElem?_cons_succ] at hj
        cases hi
        exact (hd.1 (List.mem_of_getElem? hj)).elim
    | succ i =>
      cases j with
      | zero =>
        rw [List.getElem?_cons_zero] at hj
        rw [List.getElem?_cons_succ] at hi
        cases hj
        exact (hd.1 (List.mem_of_getElem? hi)).elim
      | succ j =>
        rw [List.getElem?_cons_succ] at hi hj
        rw [index_eq_of_nodup hd.2 hi hj]

/-- A list whose equal lookups always share one index has no duplicates. -/
private theorem nodup_of_index_eq {α : Type} :
    ∀ {l : List α}, (∀ (i j : Nat) (a : α), l[i]? = some a → l[j]? = some a → i = j) →
      l.Nodup
  | [], _ => List.nodup_nil
  | b :: _, h => by
    refine List.nodup_cons.2 ⟨fun hb => ?_, nodup_of_index_eq fun i j a hi hj => ?_⟩
    · obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 hb
      have := h 0 (k + 1) b List.getElem?_cons_zero (by rw [List.getElem?_cons_succ]; exact hk)
      omega
    · have := h (i + 1) (j + 1) a (by rw [List.getElem?_cons_succ]; exact hi)
        (by rw [List.getElem?_cons_succ]; exact hj)
      omega

/-- Equal images under a duplicate-free map identify list indices. -/
private theorem index_eq_of_nodup_map {α β : Type} {f : α → β} {l : List α}
    (hd : (l.map f).Nodup) {i j : Nat} {a b : α}
    (hi : l[i]? = some a) (hj : l[j]? = some b) (hab : f a = f b) : i = j :=
  index_eq_of_nodup hd (a := f a) (by simp [hi]) (by simp [hj, hab])

/-- A filter-mapped list is duplicate-free when equal images come from one index. -/
private theorem filterMap_nodup {α β : Type} (f : α → Option β) :
    ∀ {l : List α}, (∀ (i j : Nat) (b : β), (l[i]?).bind f = some b →
      (l[j]?).bind f = some b → i = j) → (l.filterMap f).Nodup
  | [], _ => List.nodup_nil
  | a :: l, h => by
    have ih : (l.filterMap f).Nodup := filterMap_nodup f fun i j b hi hj => by
      have := h (i + 1) (j + 1) b (by rw [List.getElem?_cons_succ]; exact hi)
        (by rw [List.getElem?_cons_succ]; exact hj)
      omega
    cases hfa : f a with
    | none =>
      rw [List.filterMap_cons_none hfa]
      exact ih
    | some b =>
      rw [List.filterMap_cons_some hfa]
      refine List.nodup_cons.2 ⟨fun hb => ?_, ih⟩
      obtain ⟨a', ha', hfa'⟩ := List.mem_filterMap.1 hb
      obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 ha'
      have := h 0 (k + 1) b (by rw [List.getElem?_cons_zero]; exact hfa)
        (by rw [List.getElem?_cons_succ, hk]; exact hfa')
      omega

/-- Live identities are unique when equal identities always share one slot index. -/
private theorem liveIds_nodup_of {state : PhysicalState}
    (hindex : ∀ (i j : Nat) (si sj : PhysicalSlot) (id : Id) (s1 s2 : NetSym),
      state.slots[i]? = some si → si.agent = some (id, s1) →
      state.slots[j]? = some sj → sj.agent = some (id, s2) → i = j) :
    state.liveIds.Nodup := by
  unfold PhysicalState.liveIds
  apply filterMap_nodup
  intro i j b hi hj
  rw [Array.getElem?_toList] at hi hj
  cases hsi : state.slots[i]? with
  | none =>
    rw [hsi] at hi
    cases hi
  | some si =>
    cases hsj : state.slots[j]? with
    | none =>
      rw [hsj] at hj
      cases hj
    | some sj =>
      rw [hsi] at hi
      rw [hsj] at hj
      change si.agent.map Prod.fst = some b at hi
      change sj.agent.map Prod.fst = some b at hj
      obtain ⟨⟨idi, s1⟩, hai, hbi⟩ := Option.map_eq_some_iff.1 hi
      obtain ⟨⟨idj, s2⟩, haj, hbj⟩ := Option.map_eq_some_iff.1 hj
      have hij : idj = idi := hbj.trans hbi.symm
      subst hij
      exact hindex i j si sj idj s1 s2 hsi hai hsj haj

/-- Components of equal endpoint triples. -/
private theorem triple_eq {a b c a' b' c' : Nat} (h : (a, b, c) = (a', b', c')) :
    a = a' ∧ b = b' ∧ c = c' := by
  cases h
  exact ⟨rfl, rfl, rfl⟩

/-- With distinct keys, a list lookup returns the listed value. -/
private theorem assoc_some {α β : Type} [DecidableEq α] {l : List (α × β)}
    (hd : (l.map Prod.fst).Nodup) {p : α} {q : β} (hm : (p, q) ∈ l) : assoc p l = some q :=
  assoc_eq_of_unique hm fun _ hq' => (Prod.mk.inj (inj_of_nodup_map hd hq' hm rfl)).2

/-- A key absent from the list has no lookup. -/
private theorem assoc_none {α β : Type} [DecidableEq α] {l : List (α × β)} {p : α}
    (h : p ∉ l.map Prod.fst) : assoc p l = none := by
  cases hc : assoc p l with
  | none => rfl
  | some q => exact absurd (List.mem_map.2 ⟨(p, q), assoc_mem hc, rfl⟩) h

/-- A live port above its symbol's arity stores no reference. -/
private theorem port_above_arity {state : PhysicalState} (hrecip : state.reciprocalPorts = true)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym} {port : Nat}
    {target : Option (Nat × Nat)}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol))
    (hport : slot.ports[port]? = some target) (hgt : NetI64.arity symbol < port) :
    target = none :=
  ((PhysicalState.reciprocalPorts_iff.1 hrecip index slot id symbol hslot hagent).2
    port target hport).1 hgt

/-- A stored live-port reference lies within arity and is reciprocated by its target. -/
private theorem reciprocal_ref {state : PhysicalState} (hrecip : state.reciprocalPorts = true)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym} {port peer peerPort : Nat}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol))
    (hport : slot.ports[port]? = some (some (peer, peerPort))) :
    port ≤ NetI64.arity symbol ∧ ∃ remote, state.slots[peer]? = some remote ∧
      remote.ports[peerPort]? = some (some (index, port)) := by
  obtain ⟨hgt, hle⟩ := (PhysicalState.reciprocalPorts_iff.1 hrecip index slot id symbol
    hslot hagent).2 port _ hport
  have hin : port ≤ NetI64.arity symbol := by
    rcases Nat.lt_or_ge (NetI64.arity symbol) port with hlt | hge
    · exact absurd (hgt hlt) (by simp)
    · exact hge
  obtain ⟨remote, hremote, _, hback, _⟩ := hle hin
  exact ⟨hin, remote, hremote, hback⟩

/-- Every output of a key-separated arena targets an empty port. -/
private theorem state_target_empty {state : PhysicalState} (hsep : state.KeysSeparated)
    {out : PhysicalOutput} (hout : out ∈ state.outputs) {slot : PhysicalSlot}
    (hslot : state.slots[out.target.1]? = some slot) : slot.ports[out.target.2]? = some none := by
  obtain ⟨slot', hslot', hempty⟩ := hsep.targetEmpty out hout
  rw [hslot] at hslot'
  cases hslot'
  exact hempty

/-- A live slot reads its logical ports. -/
private theorem logicalPort_live {state : PhysicalState} {index : Nat} {slot : PhysicalSlot}
    {id : Id} {symbol : NetSym} (hslot : state.slots[index]? = some slot)
    (hagent : slot.agent = some (id, symbol)) (port : Nat) :
    state.logicalPort (index, port) = some (id, port) := by
  simp [PhysicalState.logicalPort, hslot, hagent]

/-- Arenas agreeing on a slot's agent read the same logical ports there. -/
private theorem logicalPort_congr {first second : PhysicalState} {index : Nat}
    {firstSlot secondSlot : PhysicalSlot}
    (hfirst : first.slots[index]? = some firstSlot)
    (hsecond : second.slots[index]? = some secondSlot)
    (hagent : firstSlot.agent = secondSlot.agent) (port : Nat) :
    first.logicalPort (index, port) = second.logicalPort (index, port) := by
  simp [PhysicalState.logicalPort, hfirst, hsecond, hagent]

section

variable {state next : PhysicalState} {selected : Plan}
  {ruleSlot valueSlot : Nat} {ruleId valueId : Id}
  {ruleSymbol valueSymbol : NetSym}
  {results : List ResultView} {edges : List EdgeView}
  {retired : List Nat} {fresh : Nat} {published : List PhysicalPair}

/-- The prestate's live ports are reciprocal. -/
private theorem PhysicalState.LocalRewrite.stateRecip
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    state.reciprocalPorts = true := by
  obtain ⟨_, _, _, _, _, _, hrecip, _⟩ := h.legal
  exact hrecip

/-- The consumed rule identity is live in the prestate. -/
private theorem PhysicalState.LocalRewrite.rule_live
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    ruleId ∈ state.liveIds := by
  obtain ⟨old, hslot, hagent, _⟩ := h.ruleOld
  exact PhysicalState.mem_liveIds.2 ⟨ruleSlot, old, ruleSymbol, hslot, hagent⟩

/-- The consumed value identity is live in the prestate. -/
private theorem PhysicalState.LocalRewrite.value_live
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    valueId ∈ state.liveIds := by
  obtain ⟨old, hslot, hagent, _⟩ := h.valueOld
  exact PhysicalState.mem_liveIds.2 ⟨valueSlot, old, valueSymbol, hslot, hagent⟩

/-- A live prestate slot is consumed exactly when it carries a consumed identity. -/
private theorem PhysicalState.LocalRewrite.consumed_iff
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol)) :
    (index = ruleSlot ↔ id = ruleId) ∧ (index = valueSlot ↔ id = valueId) := by
  have hu := (PhysicalState.Legal.keysSeparated h.legal).unique
  obtain ⟨ro, hro, hra, _⟩ := h.ruleOld
  obtain ⟨vo, hvo, hva, _⟩ := h.valueOld
  refine ⟨⟨fun he => ?_, fun he => ?_⟩, ⟨fun he => ?_, fun he => ?_⟩⟩
  · rw [he, hro] at hslot
    cases hslot
    rw [hra] at hagent
    exact (Prod.mk.inj (Option.some.inj hagent)).1.symm
  · rw [he] at hagent
    exact PhysicalState.slot_index_unique hu hslot hagent hro hra
  · rw [he, hvo] at hslot
    cases hslot
    rw [hva] at hagent
    exact (Prod.mk.inj (Option.some.inj hagent)).1.symm
  · rw [he] at hagent
    exact PhysicalState.slot_index_unique hu hslot hagent hvo hva

/-- An edge's result identity names the result installed at its result slot. -/
private theorem PhysicalState.LocalRewrite.edge_resultId
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {result : ResultView} (hr : result ∈ results) :
    edge.resultId = result.id ↔ edge.result.1 = result.slot := by
  obtain ⟨r, hr', hrs, hri, _⟩ := h.edgeResult edge he
  constructor
  · intro hid
    have hrr : r = result := inj_of_nodup_map h.resultIds hr' hr (hri.symm.trans hid)
    rw [← hrs, hrr]
  · intro hslot
    have hrr : r = result := inj_of_nodup_map h.resultSlots hr' hr (hrs.trans hslot)
    rw [hri, hrr]

/-- Installed result slots are consumed or fresh, hence differ from every live
prestate slot outside the consumed pair. -/
private theorem PhysicalState.LocalRewrite.result_slot_ne
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {result : ResultView} (hr : result ∈ results) {index : Nat} {old : PhysicalSlot}
    (hold : state.slots[index]? = some old) (h1 : index ≠ ruleSlot) (h2 : index ≠ valueSlot) :
    result.slot ≠ index := by
  intro heq
  obtain ⟨hlt, _⟩ := Array.getElem?_eq_some_iff.1 hold
  rcases h.resultPlace result hr with (hrule | hvalue) | ⟨hge, _⟩
  · exact h1 (heq.symm.trans hrule)
  · exact h2 (heq.symm.trans hvalue)
  · omega

/-- Every auxiliary port of the consumed rule agent is the source of a boundary edge. -/
private theorem PhysicalState.LocalRewrite.edge_of_port
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {port : Nat} (hpos : 0 < port) (hle : port ≤ NetI64.arity ruleSymbol) :
    ∃ edge ∈ edges, edge.sourcePort = port := by
  have hmem : port ∈ edges.map EdgeView.sourcePort := by
    rw [h.edgePorts]
    exact List.mem_map.2 ⟨port - 1, List.mem_range.2 (by omega), Nat.sub_add_cancel hpos⟩
  exact List.mem_map.1 hmem

/-- A boundary edge targeting a live peer port captured that port of a live
prestate slot outside the consumed pair. -/
private theorem PhysicalState.LocalRewrite.edge_peer
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {peer port : Nat}
    (ht : edge.target = (0, peer, port)) :
    peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧ ∃ peerSlot peerId peerSymbol,
      state.slots[peer]? = some peerSlot ∧ peerSlot.agent = some (peerId, peerSymbol) ∧
      peerSlot.ports[port]? = some (some (ruleSlot, edge.sourcePort)) ∧
      edge.endpoint = (peerId, port) := by
  rcases h.edgeCapture edge he with
    ⟨peer', port', peerSlot, peerId, peerSymbol, ht', h1, h2, hps, hpa, hpp, hend⟩ |
    ⟨_, _, ht', _⟩
  · rw [ht] at ht'
    obtain ⟨_, rfl, rfl⟩ := triple_eq ht'
    exact ⟨h1, h2, peerSlot, peerId, peerSymbol, hps, hpa, hpp, hend⟩
  · rw [ht] at ht'
    exact absurd (triple_eq ht').1 (by decide)

/-- A boundary edge targeting an output captured that prestate output. -/
private theorem PhysicalState.LocalRewrite.edge_output
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {k x : Nat} (ht : edge.target = (1, k, x)) :
    ∃ out, state.outputs[k]? = some out ∧ out.target = (ruleSlot, edge.sourcePort) ∧
      edge.endpoint = out.label := by
  rcases h.edgeCapture edge he with
    ⟨_, _, _, _, _, ht', _⟩ | ⟨output, out, ht', hout, htarget, hend⟩
  · rw [ht] at ht'
    exact absurd (triple_eq ht').1 (by decide)
  · rw [ht] at ht'
    obtain ⟨_, rfl, _⟩ := triple_eq ht'
    exact ⟨out, hout, htarget, hend⟩

/-- The installed result port of a boundary edge stores the edge's live peer
reference, or nothing for an output edge. -/
private theorem PhysicalState.LocalRewrite.edge_port
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {result : ResultView} (hr : result ∈ results)
    (hslot : edge.result.1 = result.slot) :
    (∀ peer port, edge.target = (0, peer, port) →
      result.ports[edge.result.2]? = some (some (peer, port))) ∧
    (∀ output x, edge.target = (1, output, x) → result.ports[edge.result.2]? = some none) := by
  obtain ⟨r, hr', hrs, _, hlt⟩ := h.edgeResult edge he
  have hrr : r = result := inj_of_nodup_map h.resultSlots hr' hr (hrs.trans hslot)
  subst hrr
  obtain ⟨t, ht⟩ : ∃ t, r.ports[edge.result.2]? = some t := ⟨_, List.getElem?_eq_getElem hlt⟩
  have hres : edge.result = (r.slot, edge.result.2) := Prod.ext hrs.symm rfl
  rcases h.resultPorts r hr edge.result.2 t ht with ⟨e', he', her', _, _, hcase⟩ | ⟨hun, _, _⟩
  · have hee : e' = edge := inj_of_nodup_map h.edgeResults he' he (her'.trans hres.symm)
    subst hee
    refine ⟨fun peer port ht'' => ?_, fun output x ht'' => ?_⟩
    · rw [ht]
      rcases hcase with ⟨peer', pp, ht', rfl⟩ | ⟨_, ht', _⟩
      · rw [ht'] at ht''
        obtain ⟨_, rfl, rfl⟩ := triple_eq ht''
        rfl
      · rw [ht'] at ht''
        exact absurd (triple_eq ht'').1 (by decide)
    · rw [ht]
      rcases hcase with ⟨_, _, ht', _⟩ | ⟨_, _, rfl⟩
      · rw [ht'] at ht''
        exact absurd (triple_eq ht'').1 (by decide)
      · rfl
  · exact absurd hres (hun edge he)

/-- Boundary endpoints never carry an installed result identity. -/
private theorem PhysicalState.LocalRewrite.endpoint_ne_result
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {result : ResultView} (hr : result ∈ results) :
    edge.endpoint.1 ≠ result.id := by
  intro heq
  rcases h.edgeCapture edge he with
    ⟨peer, _, ps, _, psym, _, _, _, hps, hpa, _, hend⟩ | ⟨_, out, _, hout, _, hend⟩
  · rw [hend] at heq
    exact (h.resultFresh result hr).1 (heq ▸ PhysicalState.mem_liveIds.2 ⟨peer, ps, psym, hps, hpa⟩)
  · rw [hend] at heq
    exact (h.resultFresh result hr).2 out (List.mem_of_getElem? hout) heq

/-- Boundary edges with equal endpoints have equal targets. -/
private theorem PhysicalState.LocalRewrite.endpoint_target
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {a b : EdgeView} (ha : a ∈ edges) (hb : b ∈ edges) (heq : a.endpoint = b.endpoint) :
    a.target = b.target := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  rcases h.edgeCapture a ha with
    ⟨pa, qa, sa, ida, syma, hta, _, _, hsa, haa, _, hea⟩ | ⟨ka, outa, hta, houta, _, hea⟩ <;>
  rcases h.edgeCapture b hb with
    ⟨pb, qb, sb, idb, symb, htb, _, _, hsb, hab, _, heb⟩ | ⟨kb, outb, htb, houtb, _, heb⟩
  · have hid : (ida, qa) = (idb, qb) := hea.symm.trans (heq.trans heb)
    obtain ⟨rfl, rfl⟩ := Prod.mk.inj hid
    rw [hta, htb, PhysicalState.slot_index_unique hS.unique hsa haa hsb hab]
  · exfalso
    have hid : (ida, qa) = outb.label := hea.symm.trans (heq.trans heb)
    exact hS.labelDead outb (List.mem_of_getElem? houtb)
      (hid ▸ PhysicalState.mem_liveIds.2 ⟨pa, sa, syma, hsa, haa⟩)
  · exfalso
    have hid : outa.label = (idb, qb) := hea.symm.trans (heq.trans heb)
    exact hS.labelDead outa (List.mem_of_getElem? houta)
      (hid.symm ▸ PhysicalState.mem_liveIds.2 ⟨pb, sb, symb, hsb, hab⟩)
  · rw [hta, htb, index_eq_of_nodup_map hS.labels houta houtb (hea.symm.trans (heq.trans heb))]

/-- A prestate output captured by no boundary edge targets an empty port of a
live prestate slot outside the consumed pair. -/
private theorem PhysicalState.LocalRewrite.untouched_output
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {k : Nat} {out : PhysicalOutput} (hk : state.outputs[k]? = some out)
    (hfree : ∀ edge ∈ edges, edge.target ≠ (1, k, 0)) :
    out.target.1 ≠ ruleSlot ∧ out.target.1 ≠ valueSlot ∧
      ∃ old id symbol, state.slots[out.target.1]? = some old ∧
        old.agent = some (id, symbol) ∧ old.ports[out.target.2]? = some none := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  have hmem : out ∈ state.outputs := List.mem_of_getElem? hk
  obtain ⟨old, id, symbol, hold, hagent, harity⟩ := h.outputsInArity out hmem
  have hempty := state_target_empty hS hmem hold
  refine ⟨fun hrule => ?_, fun hvalue => ?_, old, id, symbol, hold, hagent, hempty⟩
  · obtain ⟨ro, hro, hra, hr0⟩ := h.ruleOld
    rw [hrule, hro] at hold
    cases hold
    rw [hra] at hagent
    rw [← (Prod.mk.inj (Option.some.inj hagent)).2] at harity
    rcases Nat.eq_zero_or_pos out.target.2 with h0 | hpos
    · rw [h0, hr0] at hempty
      cases hempty
    · obtain ⟨edge, he, hsrc⟩ := h.edge_of_port hpos harity
      rcases h.edgeCapture edge he with
        ⟨_, _, _, _, _, _, _, _, hps, hpa, hpp, _⟩ | ⟨output, out', ht, hout', htarget, _⟩
      · obtain ⟨_, remote, hremote, hrp⟩ := reciprocal_ref h.stateRecip hps hpa hpp
        rw [hro] at hremote
        cases hremote
        rw [hsrc, hempty] at hrp
        cases hrp
      · have hkk : output = k := index_eq_of_nodup_map hS.targets hout' hk
          (by rw [htarget, hsrc, ← hrule])
        exact hfree edge he (hkk ▸ ht)
  · obtain ⟨vo, hvo, hva, hv0⟩ := h.valueOld
    rw [hvalue, hvo] at hold
    cases hold
    rw [hva] at hagent
    rw [← (Prod.mk.inj (Option.some.inj hagent)).2, h.valueArity] at harity
    rw [Nat.le_zero.1 harity, hv0] at hempty
    cases hempty

/-- A live port outside the consumed pair that references the pair is the
captured target of a boundary edge. -/
private theorem PhysicalState.LocalRewrite.ref_into_pair
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym} {port peer peerPort : Nat}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol))
    (h1 : index ≠ ruleSlot) (h2 : index ≠ valueSlot)
    (hport : slot.ports[port]? = some (some (peer, peerPort)))
    (hpeer : peer = ruleSlot ∨ peer = valueSlot) :
    ∃ edge ∈ edges, edge.target = (0, index, port) := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  obtain ⟨_, remote, hremote, hrp⟩ := reciprocal_ref h.stateRecip hslot hagent hport
  rcases hpeer with hpeer | hpeer
  · obtain ⟨ro, hro, hra, hr0⟩ := h.ruleOld
    rw [hpeer, hro] at hremote
    rw [← Option.some.inj hremote] at hrp
    obtain ⟨hle, _⟩ := reciprocal_ref h.stateRecip hro hra hrp
    rcases Nat.eq_zero_or_pos peerPort with h0 | hpos
    · rw [h0, hr0] at hrp
      exact absurd (Prod.mk.inj (Option.some.inj (Option.some.inj hrp))).1.symm h2
    · obtain ⟨edge, he, hsrc⟩ := h.edge_of_port hpos hle
      rcases h.edgeCapture edge he with
        ⟨p', q', ps, _, _, ht, _, _, hps, hpa, hpp, _⟩ | ⟨_, out, _, hout, htarget, _⟩
      · obtain ⟨_, remote', hremote', hrp'⟩ := reciprocal_ref h.stateRecip hps hpa hpp
        rw [hro] at hremote'
        rw [← Option.some.inj hremote'] at hrp'
        rw [hsrc, hrp] at hrp'
        have hpq := Prod.mk.inj (Option.some.inj (Option.some.inj hrp'))
        refine ⟨edge, he, ?_⟩
        rw [ht, hpq.1, hpq.2]
      · exfalso
        have hE : ro.ports[peerPort]? = some none := by
          have hempty := state_target_empty hS (List.mem_of_getElem? hout) (slot := ro)
            (by rw [htarget]; exact hro)
          rw [htarget, hsrc] at hempty
          exact hempty
        rw [hrp] at hE
        cases hE
  · obtain ⟨vo, hvo, hva, hv0⟩ := h.valueOld
    rw [hpeer, hvo] at hremote
    rw [← Option.some.inj hremote] at hrp
    obtain ⟨hle, _⟩ := reciprocal_ref h.stateRecip hvo hva hrp
    rw [h.valueArity] at hle
    rw [Nat.le_zero.1 hle, hv0] at hrp
    exact absurd (Prod.mk.inj (Option.some.inj (Option.some.inj hrp))).1.symm h1

/-- Every live post-state slot holds an installed result, or keeps the live
prestate agent of a slot outside the consumed pair. -/
private theorem PhysicalState.LocalRewrite.next_live
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : next.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol)) :
    (∃ result ∈ results, result.slot = index ∧ result.id = id) ∨
    (index ≠ ruleSlot ∧ index ≠ valueSlot ∧
      ∃ old, state.slots[index]? = some old ∧ old.agent = some (id, symbol)) := by
  by_cases hres : ∃ result ∈ results, result.slot = index
  · obtain ⟨result, hr, hrs⟩ := hres
    obtain ⟨slot', hslot', hagent', _, _⟩ := h.observed.resultAfter result hr
    rw [hrs, hslot] at hslot'
    cases hslot'
    rw [hagent] at hagent'
    exact Or.inl ⟨result, hr, hrs, (Prod.mk.inj (Option.some.inj hagent')).1.symm⟩
  · obtain ⟨hidx, _⟩ := Array.getElem?_eq_some_iff.1 hslot
    rw [h.observed.size] at hidx
    have hlt : index < state.slots.size := by
      rcases Nat.lt_or_ge index state.slots.size with hlt | hge
      · exact hlt
      · exact (hres (h.freshFilled index hge hidx)).elim
    obtain ⟨old, hold⟩ : ∃ old, state.slots[index]? = some old :=
      ⟨_, Array.getElem?_eq_getElem hlt⟩
    by_cases hcons : index = ruleSlot ∨ index = valueSlot
    · obtain ⟨_, slot', _, hslot', hnone, _⟩ := h.observed.retiredAfter index
        ((h.retiredExact index).2 ⟨hcons, fun result hr heq => hres ⟨result, hr, heq⟩⟩)
      rw [hslot] at hslot'
      cases hslot'
      rw [hagent] at hnone
      cases hnone
    · have h1 : index ≠ ruleSlot := fun e => hcons (Or.inl e)
      have h2 : index ≠ valueSlot := fun e => hcons (Or.inr e)
      obtain ⟨slot', hslot', hag, _⟩ := h.observed.otherAfter index old hold h1 h2
      rw [hslot] at hslot'
      cases hslot'
      exact Or.inr ⟨h1, h2, old, hold, hag.symm.trans hagent⟩

/-- The post-state live identities are the installed result identities and
the prestate identities outside the consumed pair. -/
private theorem PhysicalState.LocalRewrite.mem_next_liveIds
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {id : Id} :
    id ∈ next.liveIds ↔ (∃ result ∈ results, result.id = id) ∨
      (id ∈ state.liveIds ∧ id ≠ ruleId ∧ id ≠ valueId) := by
  constructor
  · intro hmem
    obtain ⟨index, slot, symbol, hslot, hagent⟩ := PhysicalState.mem_liveIds.1 hmem
    rcases h.next_live hslot hagent with ⟨result, hr, _, hid⟩ | ⟨h1, h2, old, hold, hag⟩
    · exact Or.inl ⟨result, hr, hid⟩
    · have hc := h.consumed_iff hold hag
      exact Or.inr ⟨PhysicalState.mem_liveIds.2 ⟨index, old, symbol, hold, hag⟩,
        fun e => h1 (hc.1.2 e), fun e => h2 (hc.2.2 e)⟩
  · rintro (⟨result, hr, rfl⟩ | ⟨hlive, h1, h2⟩)
    · obtain ⟨slot, hslot, hagent, _, _⟩ := h.observed.resultAfter result hr
      exact PhysicalState.mem_liveIds.2 ⟨result.slot, slot, result.symbol, hslot, hagent⟩
    · obtain ⟨index, old, symbol, hold, hag⟩ := PhysicalState.mem_liveIds.1 hlive
      have hc := h.consumed_iff hold hag
      obtain ⟨slot, hslot, hagent, _⟩ := h.observed.otherAfter index old hold
        (fun e => h1 (hc.1.1 e)) (fun e => h2 (hc.2.1 e))
      exact PhysicalState.mem_liveIds.2 ⟨index, slot, symbol, hslot, hagent.trans hag⟩

/-- The executed arena keeps one live slot per logical identity. -/
theorem PhysicalState.LocalRewrite.liveIds_nodup
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.liveIds.Nodup := by
  have hu := (PhysicalState.Legal.keysSeparated h.legal).unique
  apply liveIds_nodup_of
  intro i j si sj id s1 s2 hi hai hj haj
  rcases h.next_live hi hai with ⟨ri, hri, hris, hrid⟩ | ⟨_, _, oi, hoi, hoai⟩ <;>
    rcases h.next_live hj haj with ⟨rj, hrj, hrjs, hrjd⟩ | ⟨_, _, oj, hoj, hoaj⟩
  · have hrr : ri = rj := inj_of_nodup_map h.resultIds hri hrj (hrid.trans hrjd.symm)
    rw [← hris, ← hrjs, hrr]
  · exact absurd (PhysicalState.mem_liveIds.2 ⟨j, oj, s2, hoj, hoaj⟩)
      (hrid ▸ (h.resultFresh ri hri).1)
  · exact absurd (PhysicalState.mem_liveIds.2 ⟨i, oi, s1, hoi, hoai⟩)
      (hrjd ▸ (h.resultFresh rj hrj).1)
  · exact PhysicalState.slot_index_unique hu hoi hoai hoj hoaj

/-- The post-state output table keeps the prestate labels in order. -/
private theorem PhysicalState.LocalRewrite.next_labels
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.outputs.map PhysicalOutput.label = state.outputs.map PhysicalOutput.label := by
  apply List.ext_getElem?
  intro k
  rw [List.getElem?_map, List.getElem?_map]
  cases hk : state.outputs[k]? with
  | none =>
    have hlen : state.outputs.length ≤ k := List.getElem?_eq_none_iff.1 hk
    rw [List.getElem?_eq_none (by rw [h.observed.outputsAfter.1]; exact hlen)]
  | some old =>
    obtain ⟨out, hout, hlabel, _, _⟩ := h.observed.outputsAfter.2 k old hk
    rw [hout]
    simp [hlabel]

/-- Every post-state output label is a prestate output label. -/
private theorem PhysicalState.LocalRewrite.next_label_mem
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {o : PhysicalOutput} (ho : o ∈ next.outputs) :
    ∃ out ∈ state.outputs, out.label = o.label := by
  have hlab : o.label ∈ state.outputs.map PhysicalOutput.label := by
    rw [← h.next_labels]
    exact List.mem_map_of_mem ho
  exact List.mem_map.1 hlab

/-- Every post-state output keeps its prestate label, and is either retargeted
by the boundary edge capturing it or keeps an untouched prestate target. -/
private theorem PhysicalState.LocalRewrite.next_output
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {k : Nat} {o : PhysicalOutput} (hk : next.outputs[k]? = some o) :
    ∃ out, state.outputs[k]? = some out ∧ o.label = out.label ∧
      ((∃ edge ∈ edges, edge.target = (1, k, 0) ∧ o.target = edge.result ∧
          edge.endpoint = out.label) ∨
       ((∀ edge ∈ edges, edge.target ≠ (1, k, 0)) ∧ o.target = out.target ∧
          out.target.1 ≠ ruleSlot ∧ out.target.1 ≠ valueSlot ∧
          ∃ old id symbol, state.slots[out.target.1]? = some old ∧
            old.agent = some (id, symbol) ∧ old.ports[out.target.2]? = some none)) := by
  have hlt : k < state.outputs.length := by
    rw [← h.observed.outputsAfter.1]
    obtain ⟨hlt, _⟩ := List.getElem?_eq_some_iff.1 hk
    exact hlt
  obtain ⟨out, hout⟩ : ∃ out, state.outputs[k]? = some out :=
    ⟨_, List.getElem?_eq_getElem hlt⟩
  obtain ⟨o', ho', hlabel, hedge, hfree⟩ := h.observed.outputsAfter.2 k out hout
  rw [hk] at ho'
  cases ho'
  refine ⟨out, hout, hlabel, ?_⟩
  by_cases hcap : ∃ edge ∈ edges, edge.target = (1, k, 0)
  · obtain ⟨edge, he, ht⟩ := hcap
    obtain ⟨out', hout', _, hend⟩ := h.edge_output he ht
    rw [hout] at hout'
    cases hout'
    exact Or.inl ⟨edge, he, ht, hedge edge he ht, hend⟩
  · have hnot : ∀ edge ∈ edges, edge.target ≠ (1, k, 0) :=
      fun edge he ht => hcap ⟨edge, he, ht⟩
    exact Or.inr ⟨hnot, hfree hnot, h.untouched_output hout hnot⟩

/-- A post-state output targeting an installed result slot was retargeted by
the boundary edge whose result is that target port. -/
private theorem PhysicalState.LocalRewrite.next_output_result
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {o : PhysicalOutput} (ho : o ∈ next.outputs) {result : ResultView}
    (hr : result ∈ results) {port : Nat} (ht : o.target = (result.slot, port)) :
    ∃ edge ∈ edges, edge.result = (result.slot, port) ∧ ∃ k, edge.target = (1, k, 0) := by
  obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 ho
  obtain ⟨out, _, _, hc⟩ := h.next_output hk
  rcases hc with ⟨edge, he, het, hot, _⟩ | ⟨_, hot, h1, h2, old, _, _, hold, _, _⟩
  · exact ⟨edge, he, hot.symm.trans ht, k, het⟩
  · exfalso
    apply h.result_slot_ne hr hold h1 h2
    rw [← hot, ht]

/-- A post-state output targeting a live prestate slot outside the consumed
pair kept the target of its prestate output. -/
private theorem PhysicalState.LocalRewrite.next_output_old
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {o : PhysicalOutput} (ho : o ∈ next.outputs) {index : Nat} {old : PhysicalSlot}
    (hold : state.slots[index]? = some old) (h1 : index ≠ ruleSlot) (h2 : index ≠ valueSlot)
    {port : Nat} (ht : o.target = (index, port)) :
    ∃ out ∈ state.outputs, out.target = (index, port) ∧ out.label = o.label := by
  obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 ho
  obtain ⟨out, hout, hol, hc⟩ := h.next_output hk
  rcases hc with ⟨edge, he, _, hot, _⟩ | ⟨_, hot, _⟩
  · exfalso
    obtain ⟨r, hr, hrs, _, _⟩ := h.edgeResult edge he
    apply h.result_slot_ne hr hold h1 h2
    rw [hrs, ← hot, ht]
  · exact ⟨out, List.mem_of_getElem? hout, hot.symm.trans ht, hol.symm⟩

/-- A post-state result port reads as its installed logical port. -/
private theorem PhysicalState.LocalRewrite.next_logicalPort_result
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {result : ResultView} (hr : result ∈ results) (port : Nat) :
    next.logicalPort (result.slot, port) = some (result.id, port) := by
  obtain ⟨slot, hslot, hagent, _, _⟩ := h.observed.resultAfter result hr
  exact logicalPort_live hslot hagent port

/-- A post-state slot outside the consumed pair reads its prestate logical ports. -/
private theorem PhysicalState.LocalRewrite.next_logicalPort_old
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {old : PhysicalSlot}
    (hold : state.slots[index]? = some old) (h1 : index ≠ ruleSlot) (h2 : index ≠ valueSlot)
    (port : Nat) : next.logicalPort (index, port) = state.logicalPort (index, port) := by
  obtain ⟨slot, hslot, hagent, _⟩ := h.observed.otherAfter index old hold h1 h2
  exact logicalPort_congr hslot hold hagent port

/-- An edge's installed result port reads as the edge's abstract forward key. -/
private theorem PhysicalState.LocalRewrite.next_logicalPort_edge
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) :
    next.logicalPort edge.result = some (edge.resultId, edge.result.2) := by
  obtain ⟨result, hr, hrs, hri, _⟩ := h.edgeResult edge he
  obtain ⟨slot, hslot, hagent, _, _⟩ := h.observed.resultAfter result hr
  rw [hrs] at hslot
  rw [hri]
  exact logicalPort_live hslot hagent edge.result.2

/-- The executed arena's projected key spaces remain separated. -/
theorem PhysicalState.LocalRewrite.keysSeparated
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.KeysSeparated := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  refine ⟨h.liveIds_nodup, ?_, ?_, ?_, ?_⟩
  · rw [h.next_labels]
    exact hS.labels
  · apply nodup_of_index_eq
    intro i j t hi hj
    rw [List.getElem?_map] at hi hj
    obtain ⟨oi, hoi, rfl⟩ := Option.map_eq_some_iff.1 hi
    obtain ⟨oj, hoj, hji⟩ := Option.map_eq_some_iff.1 hj
    obtain ⟨outi, hsi, _, hci⟩ := h.next_output hoi
    obtain ⟨outj, hsj, _, hcj⟩ := h.next_output hoj
    rcases hci with ⟨ei, hei, hti, hri, _⟩ | ⟨_, hri, hi1, hi2, oldi, _, _, holdi, _, _⟩ <;>
      rcases hcj with ⟨ej, hej, htj, hrj, _⟩ | ⟨_, hrj, hj1, hj2, oldj, _, _, holdj, _, _⟩
    · have hee : ei = ej :=
        inj_of_nodup_map h.edgeResults hei hej (hri.symm.trans (hji.symm.trans hrj))
      subst hee
      exact (triple_eq (hti.symm.trans htj)).2.1
    · exfalso
      obtain ⟨r, hr, hrs, _, _⟩ := h.edgeResult ei hei
      apply h.result_slot_ne hr holdj hj1 hj2
      rw [hrs, ← hri, ← hji, hrj]
    · exfalso
      obtain ⟨r, hr, hrs, _, _⟩ := h.edgeResult ej hej
      apply h.result_slot_ne hr holdi hi1 hi2
      rw [hrs, ← hrj, hji, hri]
    · exact index_eq_of_nodup_map hS.targets hsi hsj (hri.symm.trans (hji.symm.trans hrj))
  · intro o ho hlive
    obtain ⟨out, hout, hol⟩ := h.next_label_mem ho
    rcases h.mem_next_liveIds.1 hlive with ⟨r, hr, hrid⟩ | ⟨hl, _, _⟩
    · exact (h.resultFresh r hr).2 out hout ((congrArg Prod.fst hol).trans hrid.symm)
    · exact hS.labelDead out hout (hol ▸ hl)
  · intro o ho
    obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 ho
    obtain ⟨out, _, _, hc⟩ := h.next_output hk
    rcases hc with ⟨edge, he, het, hot, _⟩ | ⟨_, hot, h1, h2, old, _, _, hold, _, hempty⟩
    · obtain ⟨r, hr, hrs, _, _⟩ := h.edgeResult edge he
      obtain ⟨slot, hslot, _, _, hports⟩ := h.observed.resultAfter r hr
      refine ⟨slot, by rw [hot, ← hrs]; exact hslot, ?_⟩
      rw [hot, hports]
      exact (h.edge_port he hr hrs.symm).2 k 0 het
    · obtain ⟨slot, hslot, _, _, _, _, hkeep⟩ := h.observed.otherAfter _ old hold h1 h2
      have hnoedge : ∀ edge ∈ edges, edge.target ≠ (0, out.target.1, out.target.2) := by
        intro edge he ht
        obtain ⟨_, _, ps, _, _, hps, _, hpp, _⟩ := h.edge_peer he ht
        rw [hold] at hps
        cases hps
        rw [hempty] at hpp
        cases hpp
      refine ⟨slot, by rw [hot]; exact hslot, ?_⟩
      rw [hot, hkeep _ hnoedge]
      exact hempty

/-- Every projected agent lookup equals the abstract commit's lookup. -/
theorem PhysicalState.LocalRewrite.agents_eq
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) (id : Id) :
    next.agents[id]? = (Plan.commit state.project selected rest).agents[id]? := by
  have hu := (PhysicalState.Legal.keysSeparated h.legal).unique
  have hN := h.liveIds_nodup
  have hkeys : selected.initializedAgents.map Prod.fst = results.map ResultView.id := by
    rw [h.selectedAgents]
    exact List.map_map
  have hdist : (selected.initializedAgents.map Prod.fst).Nodup := by
    rw [hkeys]
    exact h.resultIds
  rw [Plan.commit_agents_getElem? _ _ _ _ hdist]
  by_cases hres : ∃ result ∈ results, result.id = id
  · obtain ⟨result, hr, rfl⟩ := hres
    obtain ⟨slot, hslot, hagent, _, _⟩ := h.observed.resultAfter result hr
    have hm : (result.id, result.symbol) ∈ selected.initializedAgents := by
      rw [h.selectedAgents]
      exact List.mem_map_of_mem hr
    rw [PhysicalState.agents_getElem?_of_slot hN hslot hagent, assoc_some hdist hm,
      Option.some_or]
  · have hnot : id ∉ selected.initializedAgents.map Prod.fst := by
      rw [hkeys]
      intro hm
      obtain ⟨r, hr, hrid⟩ := List.mem_map.1 hm
      exact hres ⟨r, hr, hrid⟩
    rw [assoc_none hnot, Option.none_or]
    have hpair : (id = selected.redex.1 ∨ id = selected.redex.2.1) ↔
        (id = ruleId ∨ id = valueId) := by
      rcases h.selectedPair with ⟨h1, h2⟩ | ⟨h1, h2⟩
      · rw [h1, h2]
      · rw [h1, h2]
        exact Or.comm
    by_cases hc : id = ruleId ∨ id = valueId
    · rw [ite_eq_left (hpair.2 hc)]
      apply PhysicalState.agents_getElem?_of_not_live
      intro hmem
      rcases h.mem_next_liveIds.1 hmem with ⟨r, hr, hrid⟩ | ⟨_, h1, h2⟩
      · exact hres ⟨r, hr, hrid⟩
      · rcases hc with hc | hc
        · exact h1 hc
        · exact h2 hc
    · rw [ite_eq_right (fun hc' => hc (hpair.1 hc'))]
      show next.agents[id]? = state.agents[id]?
      by_cases hlive : id ∈ state.liveIds
      · obtain ⟨index, old, symbol, hold, hagent⟩ := PhysicalState.mem_liveIds.1 hlive
        have hcons := h.consumed_iff hold hagent
        obtain ⟨slot, hslot, hag, _⟩ := h.observed.otherAfter index old hold
          (fun e => hc (Or.inl (hcons.1.1 e))) (fun e => hc (Or.inr (hcons.2.1 e)))
        rw [PhysicalState.agents_getElem?_of_slot hN hslot (hag.trans hagent),
          PhysicalState.agents_getElem?_of_slot hu hold hagent]
      · rw [PhysicalState.agents_getElem?_of_not_live hlive]
        apply PhysicalState.agents_getElem?_of_not_live
        intro hmem
        rcases h.mem_next_liveIds.1 hmem with ⟨r, hr, hrid⟩ | ⟨hl, _, _⟩
        · exact hres ⟨r, hr, hrid⟩
        · exact hlive hl

/-- The abstract wire writes have pairwise distinct keys. -/
private theorem PhysicalState.LocalRewrite.writes_nodup
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    (selected.wireWrites.map Prod.fst).Nodup := by
  rw [h.selectedWrites, List.map_append, List.map_map, List.map_map, List.nodup_append]
  refine ⟨?_, ?_, ?_⟩
  · apply List.pairwise_map.2
    refine List.Pairwise.imp_of_mem ?_ (List.pairwise_map.1 h.edgeResults)
    intro a b ha hb hne heq
    apply hne
    obtain ⟨r, hr, hrs, hri, _⟩ := h.edgeResult a ha
    have hk : (a.resultId, a.result.2) = (b.resultId, b.result.2) := heq
    have hb1 : b.result.1 = r.slot :=
      (h.edge_resultId hb hr).1 ((Prod.mk.inj hk).1.symm.trans hri)
    exact Prod.ext (hrs.symm.trans hb1.symm) (Prod.mk.inj hk).2
  · apply List.pairwise_map.2
    refine List.Pairwise.imp_of_mem ?_ (List.pairwise_map.1 h.edgeTargets)
    intro a b ha hb hne heq
    exact hne (h.endpoint_target ha hb heq)
  · intro x hx y hy hxy
    obtain ⟨a, ha, rfl⟩ := List.mem_map.1 hx
    obtain ⟨b, hb, rfl⟩ := List.mem_map.1 hy
    obtain ⟨r, hr, _, hri, _⟩ := h.edgeResult a ha
    have hk : (a.resultId, a.result.2) = b.endpoint := hxy
    exact h.endpoint_ne_result hb hr ((congrArg Prod.fst hk).symm.trans hri)

/-- A key whose identity is no installed result identity is no forward key. -/
private theorem PhysicalState.LocalRewrite.not_forward
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {port : Port} (hres : ∀ result ∈ results, result.id ≠ port.1) :
    ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ port := by
  intro edge he heq
  obtain ⟨r, hr, _, hri, _⟩ := h.edgeResult edge he
  exact hres r hr (hri.symm.trans (congrArg Prod.fst heq))

/-- A key that is no forward key and no endpoint is outside every abstract wire write. -/
private theorem PhysicalState.LocalRewrite.unwritten
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {port : Port} (hfwd : ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ port)
    (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ port) :
    port ∉ selected.wireWrites.map Prod.fst := by
  rw [h.selectedWrites, List.map_append, List.mem_append]
  rintro (hm | hm)
  · obtain ⟨w, hw, hwp⟩ := List.mem_map.1 hm
    obtain ⟨edge, he, rfl⟩ := List.mem_map.1 hw
    exact hfwd edge he hwp
  · obtain ⟨w, hw, hwp⟩ := List.mem_map.1 hm
    obtain ⟨edge, he, rfl⟩ := List.mem_map.1 hw
    exact hbwd edge he hwp

/-- The abstract commit wires each forward key to its boundary endpoint. -/
private theorem PhysicalState.LocalRewrite.commit_forward
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {edge : EdgeView} (he : edge ∈ edges) :
    (Plan.commit state.project selected rest).wires[(edge.resultId, edge.result.2)]? =
      some edge.endpoint := by
  have hm : ((edge.resultId, edge.result.2), edge.endpoint) ∈ selected.wireWrites := by
    rw [h.selectedWrites]
    exact List.mem_append_left _ (List.mem_map_of_mem he)
  rw [Plan.commit_wires_getElem? _ _ _ _ h.writes_nodup, assoc_some h.writes_nodup hm,
    Option.some_or]

/-- The abstract commit wires each boundary endpoint back to its result port. -/
private theorem PhysicalState.LocalRewrite.commit_backward
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {edge : EdgeView} (he : edge ∈ edges) :
    (Plan.commit state.project selected rest).wires[edge.endpoint]? =
      some (edge.resultId, edge.result.2) := by
  have hm : (edge.endpoint, (edge.resultId, edge.result.2)) ∈ selected.wireWrites := by
    rw [h.selectedWrites]
    exact List.mem_append_right _ (List.mem_map_of_mem he)
  rw [Plan.commit_wires_getElem? _ _ _ _ h.writes_nodup, assoc_some h.writes_nodup hm,
    Option.some_or]

/-- An unwritten key at a captured wire end has no committed wire. -/
private theorem PhysicalState.LocalRewrite.commit_unwritten_captured
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {port : Port}
    (hfwd : ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ port)
    (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ port)
    (hc : port = (ruleId, 0) ∨ port = (valueId, 0) ∨
      ∃ edge ∈ edges, port = (ruleId, edge.sourcePort) ∨ port = edge.endpoint) :
    (Plan.commit state.project selected rest).wires[port]? = none := by
  rw [Plan.commit_wires_getElem? _ _ _ _ h.writes_nodup, assoc_none (h.unwritten hfwd hbwd),
    Option.none_or, ite_eq_left ((h.selectedErased port).2 hc)]

/-- An unwritten key without a prestate wire has no committed wire. -/
private theorem PhysicalState.LocalRewrite.commit_unwritten_none
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {port : Port}
    (hfwd : ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ port)
    (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ port) (hnone : state.wires[port]? = none) :
    (Plan.commit state.project selected rest).wires[port]? = none := by
  rw [Plan.commit_wires_getElem? _ _ _ _ h.writes_nodup, assoc_none (h.unwritten hfwd hbwd),
    Option.none_or]
  split
  · rfl
  · exact hnone

/-- An unwritten key of no consumed identity keeps its prestate wire. -/
private theorem PhysicalState.LocalRewrite.commit_unwritten_plain
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {port : Port}
    (hfwd : ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ port)
    (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ port)
    (hrule : port.1 ≠ ruleId) (hvalue : port.1 ≠ valueId) :
    (Plan.commit state.project selected rest).wires[port]? = state.wires[port]? := by
  rw [Plan.commit_wires_getElem? _ _ _ _ h.writes_nodup, assoc_none (h.unwritten hfwd hbwd),
    Option.none_or, ite_eq_right]
  · rfl
  · intro hc
    rcases (h.selectedErased port).1 hc with hc | hc | ⟨edge, he, hc | hc⟩
    · exact hrule (congrArg Prod.fst hc)
    · exact hvalue (congrArg Prod.fst hc)
    · exact hrule (congrArg Prod.fst hc)
    · exact hbwd edge he hc.symm

/-- A live prestate port above its agent's arity has no prestate wire. -/
private theorem PhysicalState.LocalRewrite.state_wires_above
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym} {port : Nat}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol))
    (hgt : NetI64.arity symbol < port) : state.wires[(id, port)]? = none := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  have hfree : ∀ output ∈ state.outputs, output.target ≠ (index, port) := by
    intro output hout ht
    obtain ⟨slot', id', symbol', hslot', hagent', hle⟩ := h.outputsInArity output hout
    have e1 : output.target.1 = index := by rw [ht]
    have e2 : output.target.2 = port := by rw [ht]
    rw [e1, hslot] at hslot'
    cases hslot'
    rw [hagent] at hagent'
    rw [e2, ← (Prod.mk.inj (Option.some.inj hagent')).2] at hle
    omega
  rw [PhysicalState.wires_internal hS hslot hagent hfree]
  cases hp : slot.ports[port]? with
  | none => rfl
  | some t =>
    rw [port_above_arity h.stateRecip hslot hagent hp hgt]
    rfl

/-- Wire agreement at a port of an installed result. -/
private theorem PhysicalState.LocalRewrite.wires_result
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {result : ResultView} (hr : result ∈ results) (q : Nat) :
    next.wires[(result.id, q)]? =
      (Plan.commit state.project selected rest).wires[(result.id, q)]? := by
  have hN := h.keysSeparated
  obtain ⟨ns, hns, hna, _, hnp⟩ := h.observed.resultAfter result hr
  by_cases hedge : ∃ edge ∈ edges, edge.result = (result.slot, q)
  · obtain ⟨edge, he, her⟩ := hedge
    have hslot : edge.result.1 = result.slot := by rw [her]
    have hq : edge.result.2 = q := by rw [her]
    have hcommit := h.commit_forward rest he
    rw [(h.edge_resultId he hr).2 hslot, hq] at hcommit
    rw [hcommit]
    rcases h.edgeCapture edge he with
      ⟨peer, pp, ps, _, _, ht, h1, h2, hps, hpa, _, hend⟩ | ⟨k, out, ht, hout, _, hend⟩
    · have hfree : ∀ o ∈ next.outputs, o.target ≠ (result.slot, q) := by
        intro o ho hot
        obtain ⟨edge', he', her', k, ht'⟩ := h.next_output_result ho hr hot
        have hee : edge' = edge := inj_of_nodup_map h.edgeResults he' he (her'.trans her.symm)
        rw [hee, ht] at ht'
        exact absurd (triple_eq ht').1 (by decide)
      have hport := (h.edge_port he hr hslot).1 peer pp ht
      rw [hq] at hport
      rw [PhysicalState.wires_internal hN hns hna hfree, hnp, hport]
      show next.logicalPort (peer, pp) = some edge.endpoint
      rw [h.next_logicalPort_old hps h1 h2 pp, logicalPort_live hps hpa pp, hend]
    · obtain ⟨o, hok, hol, hot, _⟩ := h.observed.outputsAfter.2 k out hout
      have hoT : o.target = (result.slot, q) := (hot edge he ht).trans her
      rw [PhysicalState.wires_endpoint hN (List.mem_of_getElem? hok)
        (by rw [hoT]; exact h.next_logicalPort_result hr q), hol, hend]
  · have hfwd : ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ (result.id, q) := by
      intro edge he heq
      have hslot : edge.result.1 = result.slot :=
        (h.edge_resultId he hr).1 (Prod.mk.inj heq).1
      exact hedge ⟨edge, he, Prod.ext hslot (Prod.mk.inj heq).2⟩
    have hbwd : ∀ edge ∈ edges, edge.endpoint ≠ (result.id, q) := fun edge he heq =>
      h.endpoint_ne_result he hr (congrArg Prod.fst heq)
    have hfree : ∀ o ∈ next.outputs, o.target ≠ (result.slot, q) := by
      intro o ho hot
      obtain ⟨edge, he, her, _⟩ := h.next_output_result ho hr hot
      exact hedge ⟨edge, he, her⟩
    have hS0 : state.wires[(result.id, q)]? = none :=
      PhysicalState.wires_absent (h.resultFresh result hr).1
        fun out hout heq => (h.resultFresh result hr).2 out hout (congrArg Prod.fst heq)
    rw [h.commit_unwritten_none rest hfwd hbwd hS0,
      PhysicalState.wires_internal hN hns hna hfree, hnp]
    cases hp : result.ports[q]? with
    | none => rfl
    | some t =>
      rcases h.resultPorts result hr q t hp with ⟨edge, he, her, _⟩ | ⟨_, _, rfl⟩
      · exact (hedge ⟨edge, he, her⟩).elim
      · rfl

/-- Wire agreement at a captured boundary endpoint. -/
private theorem PhysicalState.LocalRewrite.wires_edge_endpoint
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {edge : EdgeView} (he : edge ∈ edges) :
    next.wires[edge.endpoint]? =
      (Plan.commit state.project selected rest).wires[edge.endpoint]? := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  have hN := h.keysSeparated
  rw [h.commit_backward rest he]
  rcases h.edgeCapture edge he with
    ⟨peer, pp, ps, _, _, ht, h1, h2, hps, hpa, hpp, hend⟩ | ⟨k, out, ht, hout, _, hend⟩
  · obtain ⟨ns, hns, hna, _, _, hcap, _⟩ := h.observed.otherAfter peer ps hps h1 h2
    have hfree : ∀ o ∈ next.outputs, o.target ≠ (peer, pp) := by
      intro o ho hot
      obtain ⟨out, hout, hot', _⟩ := h.next_output_old ho hps h1 h2 hot
      have hE : ps.ports[pp]? = some none := by
        have hempty := state_target_empty hS hout (slot := ps) (by rw [hot']; exact hps)
        rw [hot'] at hempty
        exact hempty
      rw [hpp] at hE
      cases hE
    rw [hend, PhysicalState.wires_internal hN hns (hna.trans hpa) hfree, hcap edge he pp ht]
    exact h.next_logicalPort_edge he
  · obtain ⟨o, hok, hol, hot, _⟩ := h.observed.outputsAfter.2 k out hout
    rw [hend, ← hol, PhysicalState.wires_label hN (List.mem_of_getElem? hok), hot edge he ht]
    exact h.next_logicalPort_edge he

/-- Wire agreement at a port of a consumed agent that no edge captured as an endpoint. -/
private theorem PhysicalState.LocalRewrite.wires_consumed
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {id : Id} {q : Nat} (hcons : id = ruleId ∨ id = valueId)
    (hfwd : ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ (id, q))
    (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ (id, q)) :
    next.wires[(id, q)]? = (Plan.commit state.project selected rest).wires[(id, q)]? := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  have hlive : id ∈ state.liveIds := by
    rcases hcons with hc | hc
    · rw [hc]
      exact h.rule_live
    · rw [hc]
      exact h.value_live
  have hL : next.wires[(id, q)]? = none := by
    apply PhysicalState.wires_absent
    · intro hmem
      rcases h.mem_next_liveIds.1 hmem with ⟨r, hr, hrid⟩ | ⟨_, h1, h2⟩
      · exact (h.resultFresh r hr).1 (hrid ▸ hlive)
      · rcases hcons with hc | hc
        · exact h1 hc
        · exact h2 hc
    · intro o ho heq
      obtain ⟨out, hout, hol⟩ := h.next_label_mem ho
      exact hS.labelDead out hout (by rw [hol, heq]; exact hlive)
  rw [hL]
  by_cases hc : (id, q) = (ruleId, 0) ∨ (id, q) = (valueId, 0) ∨
      ∃ edge ∈ edges, (id, q) = (ruleId, edge.sourcePort) ∨ (id, q) = edge.endpoint
  · exact (h.commit_unwritten_captured rest hfwd hbwd hc).symm
  · refine (h.commit_unwritten_none rest hfwd hbwd ?_).symm
    rcases hcons with hcr | hcv
    · obtain ⟨ro, hro, hra, _⟩ := h.ruleOld
      rw [hcr]
      apply h.state_wires_above hro hra
      rcases Nat.lt_or_ge (NetI64.arity ruleSymbol) q with hlt | hge
      · exact hlt
      · exfalso
        rcases Nat.eq_zero_or_pos q with h0 | hpos
        · exact hc (Or.inl (by rw [hcr, h0]))
        · obtain ⟨edge, he, hsrc⟩ := h.edge_of_port hpos hge
          exact hc (Or.inr (Or.inr ⟨edge, he, Or.inl (by rw [hcr, hsrc])⟩))
    · obtain ⟨vo, hvo, hva, _⟩ := h.valueOld
      rw [hcv]
      apply h.state_wires_above hvo hva
      rw [h.valueArity]
      rcases Nat.eq_zero_or_pos q with h0 | hpos
      · exact (hc (Or.inr (Or.inl (by rw [hcv, h0])))).elim
      · exact hpos

/-- Wire agreement at an output label that no edge captured. -/
private theorem PhysicalState.LocalRewrite.wires_label_free
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {k : Nat} {out : PhysicalOutput}
    (hout : state.outputs[k]? = some out) (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ out.label) :
    next.wires[out.label]? = (Plan.commit state.project selected rest).wires[out.label]? := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  have hN := h.keysSeparated
  have hmem : out ∈ state.outputs := List.mem_of_getElem? hout
  have hfree : ∀ edge ∈ edges, edge.target ≠ (1, k, 0) := by
    intro edge he ht
    obtain ⟨out', hout', _, hend⟩ := h.edge_output he ht
    rw [hout] at hout'
    cases hout'
    exact hbwd edge he hend
  obtain ⟨h1, h2, old, _, _, hold, _, _⟩ := h.untouched_output hout hfree
  obtain ⟨o, hok, hol, _, hot⟩ := h.observed.outputsAfter.2 k out hout
  have hdead := hS.labelDead out hmem
  have hfwd : ∀ edge ∈ edges, (edge.resultId, edge.result.2) ≠ out.label :=
    h.not_forward fun r hr e => (h.resultFresh r hr).2 out hmem e.symm
  rw [h.commit_unwritten_plain rest hfwd hbwd (fun e => hdead (e ▸ h.rule_live))
      (fun e => hdead (e ▸ h.value_live)),
    ← hol, PhysicalState.wires_label hN (List.mem_of_getElem? hok), hol,
    PhysicalState.wires_label hS hmem, hot hfree]
  exact h.next_logicalPort_old hold h1 h2 out.target.2

/-- Wire agreement at a port of a live prestate agent outside the consumed
pair that no edge captured as an endpoint. -/
private theorem PhysicalState.LocalRewrite.wires_old
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {index : Nat} {old : PhysicalSlot} {id : Id} {symbol : NetSym}
    {q : Nat} (hold : state.slots[index]? = some old) (hagent : old.agent = some (id, symbol))
    (h1 : index ≠ ruleSlot) (h2 : index ≠ valueSlot)
    (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ (id, q)) :
    next.wires[(id, q)]? = (Plan.commit state.project selected rest).wires[(id, q)]? := by
  have hS := PhysicalState.Legal.keysSeparated h.legal
  have hN := h.keysSeparated
  have hc := h.consumed_iff hold hagent
  have hlive : id ∈ state.liveIds := PhysicalState.mem_liveIds.2 ⟨index, old, symbol, hold, hagent⟩
  rw [h.commit_unwritten_plain rest
    (h.not_forward fun r hr e => (h.resultFresh r hr).1 (e ▸ hlive)) hbwd
    (fun e => h1 (hc.1.2 e)) (fun e => h2 (hc.2.2 e))]
  obtain ⟨ns, hns, hna, _, _, _, hkeep⟩ := h.observed.otherAfter index old hold h1 h2
  have hnagent : ns.agent = some (id, symbol) := hna.trans hagent
  by_cases hout : ∃ out ∈ state.outputs, out.target = (index, q)
  · obtain ⟨out, hmem, hot⟩ := hout
    obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 hmem
    have hfree : ∀ edge ∈ edges, edge.target ≠ (1, k, 0) := by
      intro edge he ht
      obtain ⟨out', hout', htarget, _⟩ := h.edge_output he ht
      rw [hk] at hout'
      cases hout'
      exact h1 (Prod.mk.inj (hot.symm.trans htarget)).1
    obtain ⟨o, hok, hol, _, hot'⟩ := h.observed.outputsAfter.2 k out hk
    have hoT : o.target = (index, q) := (hot' hfree).trans hot
    rw [PhysicalState.wires_endpoint hN (List.mem_of_getElem? hok)
        (by rw [hoT]; exact logicalPort_live hns hnagent q),
      PhysicalState.wires_endpoint hS hmem (by rw [hot]; exact logicalPort_live hold hagent q),
      hol]
  · have hfreeS : ∀ out ∈ state.outputs, out.target ≠ (index, q) :=
      fun out hm ht => hout ⟨out, hm, ht⟩
    have hfreeN : ∀ o ∈ next.outputs, o.target ≠ (index, q) := by
      intro o ho hot
      obtain ⟨out, hm, hot', _⟩ := h.next_output_old ho hold h1 h2 hot
      exact hfreeS out hm hot'
    have hnoedge : ∀ edge ∈ edges, edge.target ≠ (0, index, q) := by
      intro edge he ht
      obtain ⟨_, _, ps, pid, _, hps, hpa, _, hend⟩ := h.edge_peer he ht
      rw [hold] at hps
      cases hps
      rw [hagent] at hpa
      exact hbwd edge he (by rw [hend, (Prod.mk.inj (Option.some.inj hpa)).1])
    rw [PhysicalState.wires_internal hN hns hnagent hfreeN,
      PhysicalState.wires_internal hS hold hagent hfreeS, hkeep q hnoedge]
    cases hp : old.ports[q]? with
    | none => rfl
    | some t =>
      cases t with
      | none => rfl
      | some ref =>
        obtain ⟨peer, pp⟩ := ref
        show next.logicalPort (peer, pp) = state.logicalPort (peer, pp)
        obtain ⟨_, remote, hremote, _⟩ := reciprocal_ref h.stateRecip hold hagent hp
        have hp1 : peer ≠ ruleSlot := fun e => by
          obtain ⟨edge, he, ht⟩ := h.ref_into_pair hold hagent h1 h2 hp (Or.inl e)
          exact hnoedge edge he ht
        have hp2 : peer ≠ valueSlot := fun e => by
          obtain ⟨edge, he, ht⟩ := h.ref_into_pair hold hagent h1 h2 hp (Or.inr e)
          exact hnoedge edge he ht
        exact h.next_logicalPort_old hremote hp1 hp2 pp

/-- Wire agreement at a key that is neither a live port nor an output label. -/
private theorem PhysicalState.LocalRewrite.wires_dead
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) {port : Port}
    (hres : ∀ result ∈ results, result.id ≠ port.1) (hlive : port.1 ∉ state.liveIds)
    (hlab : ∀ out ∈ state.outputs, out.label ≠ port)
    (hbwd : ∀ edge ∈ edges, edge.endpoint ≠ port) :
    next.wires[port]? = (Plan.commit state.project selected rest).wires[port]? := by
  rw [h.commit_unwritten_none rest (h.not_forward hres) hbwd
    (PhysicalState.wires_absent hlive hlab)]
  apply PhysicalState.wires_absent
  · intro hmem
    rcases h.mem_next_liveIds.1 hmem with ⟨r, hr, hrid⟩ | ⟨hl, _, _⟩
    · exact hres r hr hrid
    · exact hlive hl
  · intro o ho heq
    obtain ⟨out, hout, hol⟩ := h.next_label_mem ho
    exact hlab out hout (hol.trans heq)

/-- Every projected wire lookup equals the abstract commit's lookup. -/
theorem PhysicalState.LocalRewrite.wires_eq
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) (port : Port) :
    next.wires[port]? = (Plan.commit state.project selected rest).wires[port]? := by
  obtain ⟨id, q⟩ := port
  by_cases hres : ∃ result ∈ results, result.id = id
  · obtain ⟨result, hr, rfl⟩ := hres
    exact h.wires_result rest hr q
  have hres' : ∀ result ∈ results, result.id ≠ (id, q).1 := fun r hr e => hres ⟨r, hr, e⟩
  by_cases hend : ∃ edge ∈ edges, edge.endpoint = (id, q)
  · obtain ⟨edge, he, heq⟩ := hend
    rw [← heq]
    exact h.wires_edge_endpoint rest he
  have hbwd : ∀ edge ∈ edges, edge.endpoint ≠ (id, q) := fun e he heq => hend ⟨e, he, heq⟩
  by_cases hcons : id = ruleId ∨ id = valueId
  · exact h.wires_consumed rest hcons (h.not_forward hres') hbwd
  by_cases hlab : ∃ out ∈ state.outputs, out.label = (id, q)
  · obtain ⟨out, hmem, heq⟩ := hlab
    obtain ⟨k, hk⟩ := List.mem_iff_getElem?.1 hmem
    rw [← heq]
    exact h.wires_label_free rest hk fun e he h' => hbwd e he (h'.trans heq)
  by_cases hlive : id ∈ state.liveIds
  · obtain ⟨index, old, symbol, hold, hagent⟩ := PhysicalState.mem_liveIds.1 hlive
    have hc := h.consumed_iff hold hagent
    exact h.wires_old rest hold hagent (fun e => hcons (Or.inl (hc.1.1 e)))
      (fun e => hcons (Or.inr (hc.2.1 e))) hbwd
  · exact h.wires_dead rest hres' hlive (fun out hm heq => hlab ⟨out, hm, heq⟩) hbwd

/-- Every output's current target resolves to the abstract wire of its label. -/
theorem PhysicalState.LocalRewrite.outputs_field
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    (rest : List (Id × Id)) :
    next.outputs.all (fun output =>
      decide (next.logicalPort output.target =
        (Plan.commit state.project selected rest).wires[output.label]?)) = true := by
  have hN := h.keysSeparated
  rw [List.all_eq_true]
  intro output hmem
  rw [decide_eq_true_eq, ← h.wires_eq rest output.label, PhysicalState.wires_label hN hmem]

end

end NobleFoundation.NetI64Plans
