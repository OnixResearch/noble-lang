import NobleFoundation.NetI64PlanRewrite

/-!
# Physical ownership after a local rewrite (issue #110)

From the rule-independent `PhysicalState.LocalRewrite` bundle, the executed
arena keeps an exact, duplicate-free, in-bounds free-slot inventory, a
canonical physical queue, reciprocal physical ports for every live slot and
uniquely owned outputs targeting empty live ports.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- A present array lookup is in bounds. -/
private theorem array_lt_of_some {α : Type _} {xs : Array α} {i : Nat} {a : α}
    (h : xs[i]? = some a) : i < xs.size := by
  rcases Nat.lt_or_ge i xs.size with hlt | hge
  · exact hlt
  · rw [Array.getElem?_eq_none hge] at h
    cases h

/-- Members of a list whose images are duplicate-free are determined by their
image. -/
private theorem inj_of_nodup_map {α β : Type _} {f : α → β} {l : List α}
    (h : (l.map f).Nodup) {x y : α} (hx : x ∈ l) (hy : y ∈ l) (hxy : f x = f y) :
    x = y := by
  induction l with
  | nil => simp at hx
  | cons a t ih =>
    rw [List.map_cons, List.nodup_cons] at h
    obtain ⟨hnot, ht⟩ := h
    rcases List.mem_cons.mp hx with rfl | hx' <;> rcases List.mem_cons.mp hy with rfl | hy'
    · rfl
    · exact absurd (List.mem_map.mpr ⟨y, hy', hxy.symm⟩) hnot
    · exact absurd (List.mem_map.mpr ⟨x, hx', hxy⟩) hnot
    · exact ih ht hx' hy'

/-- In a duplicate-free list an element determines its index. -/
private theorem index_inj_of_nodup {α : Type _} {l : List α} (h : l.Nodup)
    {i j : Nat} {a : α} (hi : l[i]? = some a) (hj : l[j]? = some a) : i = j := by
  obtain ⟨hi', rfl⟩ := List.getElem?_eq_some_iff.mp hi
  obtain ⟨hj', hj''⟩ := List.getElem?_eq_some_iff.mp hj
  have hpair := List.pairwise_iff_getElem.mp h
  rcases Nat.lt_trichotomy i j with hlt | heq | hgt
  · exact absurd hj''.symm (hpair i j hi' hj' hlt)
  · exact heq
  · exact absurd hj'' (hpair j i hj' hi' hgt)

/-- A list in which every element determines its index is duplicate-free. -/
private theorem nodup_of_index_inj {α : Type _} {l : List α}
    (h : ∀ (i j : Nat) (a : α), l[i]? = some a → l[j]? = some a → i = j) : l.Nodup :=
  List.pairwise_iff_getElem.mpr fun i j hi hj hij heq => by
    have := h i j l[i] (List.getElem?_eq_getElem hi)
      (by rw [heq]; exact List.getElem?_eq_getElem hj)
    omega

/-- A list with exactly one index satisfying a Boolean predicate filters to a
single element. -/
private theorem filter_length_one {α : Type _} {p : α → Bool} {l : List α}
    {i : Nat} {a : α} (hi : l[i]? = some a) (hp : p a = true)
    (huniq : ∀ (j : Nat) (b : α), l[j]? = some b → p b = true → j = i) :
    (l.filter p).length = 1 := by
  induction l generalizing i with
  | nil => simp at hi
  | cons x xs ih =>
    cases i with
    | zero =>
      simp only [List.getElem?_cons_zero, Option.some.injEq] at hi
      subst hi
      have hnil : xs.filter p = [] := List.filter_eq_nil_iff.mpr fun b hb hpb => by
        obtain ⟨j, hj⟩ := List.mem_iff_getElem?.mp hb
        have := huniq (j + 1) b (by simpa using hj) hpb
        omega
      simp [List.filter_cons_of_pos hp, hnil]
    | succ i =>
      have hx : ¬p x = true := fun hpx => by
        have := huniq 0 x (by simp) hpx
        omega
      rw [List.filter_cons_of_neg hx]
      exact ih (by simpa using hi) fun j b hb hpb => by
        have := huniq (j + 1) b (by simpa using hb) hpb
        omega

/-- Lists of equal length agreeing pointwise on a Boolean predicate filter to
equal lengths. -/
private theorem filter_length_congr {α : Type _} {p : α → Bool} {l₁ l₂ : List α}
    (hlen : l₁.length = l₂.length)
    (hp : ∀ (i : Nat) (a b : α), l₁[i]? = some a → l₂[i]? = some b → p a = p b) :
    (l₁.filter p).length = (l₂.filter p).length := by
  induction l₁ generalizing l₂ with
  | nil =>
    cases l₂ with
    | nil => rfl
    | cons _ _ => simp at hlen
  | cons a as ih =>
    cases l₂ with
    | nil => simp at hlen
    | cons b bs =>
      have hab : p a = p b := hp 0 a b (by simp) (by simp)
      have hrest := ih (l₂ := bs) (by simpa using hlen) fun i x y hx hy =>
        hp (i + 1) x y (by simpa using hx) (by simpa using hy)
      cases hpb : p b <;> simp [hab, hpb, hrest]

section

variable {state next : PhysicalState} {selected : Plan}
  {ruleSlot valueSlot : Nat} {ruleId valueId : Id}
  {ruleSymbol valueSymbol : NetSym}
  {results : List ResultView} {edges : List EdgeView}
  {retired : List Nat} {fresh : Nat} {published : List PhysicalPair}

/-- Prestate legality classifies an in-bounds slot as free exactly when it is
dead. -/
private theorem prestate_free_iff (hl : state.Legal) {index : Nat}
    {old : PhysicalSlot} (hold : state.slots[index]? = some old) :
    index ∈ state.freeSlots ↔ old.agent = none := by
  have hinv := List.all_eq_true.mp hl.2.2.1 index
    (List.mem_range.mpr (array_lt_of_some hold))
  rw [hold] at hinv
  simp only [Option.map_some, Option.getD_some] at hinv
  cases hag : old.agent <;> simpa [hag] using hinv

/-- Prestate legality gives every live slot reciprocal physical ports. -/
private theorem prestate_reciprocal
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol)) :
    state.PortsReciprocalAt index slot symbol :=
  PhysicalState.reciprocalPorts_iff.mp h.legal.2.2.2.2.2.2.1 index slot id symbol
    hslot hagent

/-- Boundary edges are determined by their installed result port. -/
private theorem edge_inj
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {e₁ e₂ : EdgeView} (h₁ : e₁ ∈ edges) (h₂ : e₂ ∈ edges)
    (heq : e₁.result = e₂.result) : e₁ = e₂ :=
  inj_of_nodup_map h.edgeResults h₁ h₂ heq

/-- A boundary edge whose physical endpoint is a live-peer port captures a
non-consumed live peer storing the rule's source port. -/
private theorem edge_peer
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {peer port : Nat}
    (hT : edge.target = (0, peer, port)) :
    peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧ ∃ peerSlot peerId peerSymbol,
      state.slots[peer]? = some peerSlot ∧ peerSlot.agent = some (peerId, peerSymbol) ∧
      peerSlot.ports[port]? = some (some (ruleSlot, edge.sourcePort)) := by
  rcases h.edgeCapture edge he with
    ⟨peer', port', peerSlot, peerId, peerSymbol, hT', hr, hv, hs, ha, hp, -⟩ |
    ⟨_, _, hT', -⟩
  · rw [hT] at hT'
    simp only [Prod.mk.injEq, true_and] at hT'
    obtain ⟨rfl, rfl⟩ := hT'
    exact ⟨hr, hv, peerSlot, peerId, peerSymbol, hs, ha, hp⟩
  · rw [hT] at hT'
    simp at hT'

/-- A boundary edge whose physical endpoint is an output captures a prestate
output targeting the rule's source port. -/
private theorem edge_output
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {o : Nat} (hT : edge.target = (1, o, 0)) :
    ∃ out, state.outputs[o]? = some out ∧ out.target = (ruleSlot, edge.sourcePort) := by
  rcases h.edgeCapture edge he with
    ⟨_, _, _, _, _, hT', -⟩ | ⟨o', out, hT', hout, htarget, -⟩
  · rw [hT] at hT'
    simp at hT'
  · rw [hT] at hT'
    simp only [Prod.mk.injEq, true_and, and_true] at hT'
    subst hT'
    exact ⟨out, hout, htarget⟩

/-- A slot index in the prestate other than the consumed pair is no installed
result slot. -/
private theorem not_result_of_other
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} (hlt : index < state.slots.size)
    (hrule : index ≠ ruleSlot) (hvalue : index ≠ valueSlot) :
    ∀ result ∈ results, result.slot ≠ index := by
  intro result hr heq
  rcases h.resultPlace result hr with (h1 | h1) | ⟨h1, -⟩
  · exact hrule (by rw [← heq]; exact h1)
  · exact hvalue (by rw [← heq]; exact h1)
  · omega

/-- Every post-state slot is an installed result, a retired consumed slot or a
prestate slot outside the consumed pair. -/
private theorem slot_cases
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} (hlt : index < next.slots.size) :
    (∃ result ∈ results, result.slot = index) ∨ index ∈ retired ∨
      (index < state.slots.size ∧ index ≠ ruleSlot ∧ index ≠ valueSlot) := by
  by_cases hres : ∃ result ∈ results, result.slot = index
  · exact Or.inl hres
  · right
    by_cases hc : index = ruleSlot ∨ index = valueSlot
    · exact Or.inl ((h.retiredExact index).mpr ⟨hc, fun r hr heq => hres ⟨r, hr, heq⟩⟩)
    · refine Or.inr ⟨?_, fun e => hc (Or.inl e), fun e => hc (Or.inr e)⟩
      rcases Nat.lt_or_ge index state.slots.size with hlt' | hge
      · exact hlt'
      · rw [h.observed.size] at hlt
        obtain ⟨r, hr, heq⟩ := h.freshFilled index hge hlt
        exact (hres ⟨r, hr, heq⟩).elim

/-- Every auxiliary port of the consumed rule agent is captured by the boundary
edge with that source port: it stores a live peer, or it is empty and owned by
the captured output. -/
private theorem rule_aux
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {rOld : PhysicalSlot} (hrOld : state.slots[ruleSlot]? = some rOld)
    {p : Nat} (hp1 : 1 ≤ p) (hp2 : p ≤ NetI64.arity ruleSymbol) :
    ∃ edge ∈ edges, edge.sourcePort = p ∧
      ((∃ peer port, edge.target = (0, peer, port) ∧
          rOld.ports[p]? = some (some (peer, port))) ∨
        (∃ o out, edge.target = (1, o, 0) ∧ state.outputs[o]? = some out ∧
          out.target = (ruleSlot, p) ∧ rOld.ports[p]? = some none)) := by
  have hmem : p ∈ edges.map EdgeView.sourcePort := by
    rw [h.edgePorts]
    exact List.mem_map.mpr ⟨p - 1, List.mem_range.mpr (by omega), by omega⟩
  obtain ⟨edge, he, hsrc⟩ := List.mem_map.mp hmem
  refine ⟨edge, he, hsrc, ?_⟩
  rcases h.edgeCapture edge he with
    ⟨peer, port, peerSlot, _, peerSymbol, hT, -, -, hs, ha, hp, -⟩ |
    ⟨o, out, hT, hout, htarget, -⟩
  · left
    refine ⟨peer, port, hT, ?_⟩
    have hpre := prestate_reciprocal h hs ha
    unfold PhysicalState.PortsReciprocalAt at hpre
    obtain ⟨hhi, hlo⟩ := hpre.2 port _ hp
    have hle : port ≤ NetI64.arity peerSymbol := Nat.not_lt.mp fun hgt => by
      have := hhi hgt
      cases this
    obtain ⟨remote, hremote, -, hremPort, -⟩ := hlo hle
    rw [hrOld] at hremote
    cases hremote
    rw [← hsrc]
    exact hremPort
  · right
    refine ⟨o, out, hT, hout, by rw [htarget, hsrc], ?_⟩
    obtain ⟨-, -, howned⟩ := PhysicalState.outputOwners_iff.mp h.legal.2.2.2.2.2.2.2
    obtain ⟨slot, hs, -, hsp, -⟩ := howned out (List.mem_of_getElem? hout)
    simp only [htarget] at hs hsp
    rw [hrOld] at hs
    cases hs
    rw [← hsrc]
    exact hsp

/-- An output not captured by any boundary edge targets an empty port of a
live prestate slot outside the consumed pair. -/
private theorem untouched_output
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {o : Nat} {old : PhysicalOutput} (hold : state.outputs[o]? = some old)
    (hfree : ∀ edge ∈ edges, edge.target ≠ (1, o, 0)) :
    old.target.1 ≠ ruleSlot ∧ old.target.1 ≠ valueSlot ∧ ∃ slot,
      state.slots[old.target.1]? = some slot ∧ slot.agent.isSome = true ∧
        slot.ports[old.target.2]? = some none := by
  have hmem := List.mem_of_getElem? hold
  obtain ⟨-, htargets, howned⟩ := PhysicalState.outputOwners_iff.mp h.legal.2.2.2.2.2.2.2
  obtain ⟨slot, hs, hsAgent, hsPort, -⟩ := howned old hmem
  obtain ⟨slot', _, sym', hs', ha', hle⟩ := h.outputsInArity old hmem
  rw [hs] at hs'
  cases hs'
  refine ⟨?_, ?_, slot, hs, hsAgent, hsPort⟩
  · intro heq
    obtain ⟨rOld, hrOld, hrAgent, hrPort0⟩ := h.ruleOld
    rw [heq, hrOld] at hs
    cases hs
    rw [hrAgent] at ha'
    simp only [Option.some.injEq, Prod.mk.injEq] at ha'
    obtain ⟨-, rfl⟩ := ha'
    rcases Nat.eq_zero_or_pos old.target.2 with h0 | hpos
    · rw [h0, hrPort0] at hsPort
      simp at hsPort
    · obtain ⟨e, he, -, hcase⟩ := rule_aux h hrOld hpos hle
      rcases hcase with ⟨_, _, -, hp'⟩ | ⟨o', out, hT, hout, htarget, -⟩
      · rw [hsPort] at hp'
        simp at hp'
      · have hto : old.target = out.target := by
          rw [htarget, ← heq]
        have hidx := index_inj_of_nodup htargets
          (by simp [List.getElem?_map, hold] :
            (state.outputs.map PhysicalOutput.target)[o]? = some old.target)
          (by simp [List.getElem?_map, hout, hto] :
            (state.outputs.map PhysicalOutput.target)[o']? = some old.target)
        subst hidx
        exact hfree e he hT
  · intro heq
    obtain ⟨vOld, hvOld, hvAgent, hvPort0⟩ := h.valueOld
    rw [heq, hvOld] at hs
    cases hs
    rw [hvAgent] at ha'
    simp only [Option.some.injEq, Prod.mk.injEq] at ha'
    obtain ⟨-, rfl⟩ := ha'
    rw [h.valueArity] at hle
    have h0 : old.target.2 = 0 := by omega
    rw [h0, hvPort0] at hsPort
    simp at hsPort

/-- An output not captured by any boundary edge never targets an installed
result slot. -/
private theorem untouched_not_result
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {o : Nat} {old : PhysicalOutput} (hold : state.outputs[o]? = some old)
    (hfree : ∀ edge ∈ edges, edge.target ≠ (1, o, 0)) :
    ∀ result ∈ results, result.slot ≠ old.target.1 := by
  obtain ⟨hr, hv, slot, hs, -, -⟩ := untouched_output h hold hfree
  exact not_result_of_other h (array_lt_of_some hs) hr hv

/-- Every post-state output keeps the label of the prestate output at its
index and is either retargeted by its capturing edge or keeps its target. -/
private theorem next_output
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {o : Nat} {out : PhysicalOutput} (hout : next.outputs[o]? = some out) :
    ∃ old, state.outputs[o]? = some old ∧ out.label = old.label ∧
      ((∃ edge ∈ edges, edge.target = (1, o, 0) ∧ out.target = edge.result) ∨
        ((∀ edge ∈ edges, edge.target ≠ (1, o, 0)) ∧ out.target = old.target)) := by
  obtain ⟨hlen, hafter⟩ := h.observed.outputsAfter
  have hlt : o < state.outputs.length := by
    rw [← hlen]
    exact (List.getElem?_eq_some_iff.mp hout).1
  obtain ⟨out', hout', hlabel, hre, hkeep⟩ :=
    hafter o state.outputs[o] (List.getElem?_eq_getElem hlt)
  rw [hout] at hout'
  cases hout'
  refine ⟨state.outputs[o], List.getElem?_eq_getElem hlt, hlabel, ?_⟩
  by_cases hex : ∃ edge ∈ edges, edge.target = (1, o, 0)
  · obtain ⟨edge, he, hT⟩ := hex
    exact Or.inl ⟨edge, he, hT, hre edge he hT⟩
  · have hfree : ∀ edge ∈ edges, edge.target ≠ (1, o, 0) :=
      fun edge he hT => hex ⟨edge, he, hT⟩
    exact Or.inr ⟨hfree, hkeep hfree⟩

/-- The only post-state output targeting the result port of a boundary edge
is the output that edge captured. -/
private theorem result_port_outputs
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {j : Nat} {b : PhysicalOutput}
    (hb : next.outputs[j]? = some b) (ht : b.target = edge.result) :
    edge.target = (1, j, 0) := by
  obtain ⟨old, hold, -, hcase⟩ := next_output h hb
  rcases hcase with ⟨e', he', hT', ht'⟩ | ⟨hfree, ht'⟩
  · have : e' = edge := edge_inj h he' he (by rw [← ht', ht])
    subst this
    exact hT'
  · obtain ⟨r, hr, hrSlot, -⟩ := h.edgeResult edge he
    exact (untouched_not_result h hold hfree r hr (by rw [hrSlot, ← ht, ht'])).elim

/-- A port of a prestate slot outside the consumed pair that no prestate output
targeted is targeted by no post-state output. -/
private theorem other_no_output
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index port : Nat} (hlt : index < state.slots.size)
    (hrule : index ≠ ruleSlot) (hvalue : index ≠ valueSlot)
    (hno : ∀ output ∈ state.outputs, output.target ≠ (index, port)) :
    ∀ output ∈ next.outputs, output.target ≠ (index, port) := by
  intro output hmem heq
  obtain ⟨o, ho⟩ := List.mem_iff_getElem?.mp hmem
  obtain ⟨old, hold, -, hcase⟩ := next_output h ho
  rcases hcase with ⟨edge, he, -, ht⟩ | ⟨-, ht⟩
  · obtain ⟨r, hr, hrSlot, -⟩ := h.edgeResult edge he
    exact not_result_of_other h hlt hrule hvalue r hr (by rw [hrSlot, ← ht, heq])
  · exact hno old (List.mem_of_getElem? hold) (by rw [← ht]; exact heq)

/-- The installed result port of a boundary edge is live and stores exactly the
edge's captured endpoint: the live peer port, or nothing for an output. -/
private theorem edge_result_port
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {edge : EdgeView} (he : edge ∈ edges) {rs rp : Nat}
    (hres : edge.result = (rs, rp)) :
    ∃ result ∈ results, result.slot = rs ∧ ∃ slot, next.slots[rs]? = some slot ∧
      slot.agent = some (result.id, result.symbol) ∧
      ((∃ peer peerPort, edge.target = (0, peer, peerPort) ∧
          slot.ports[rp]? = some (some (peer, peerPort))) ∨
        (∃ o, edge.target = (1, o, 0) ∧ slot.ports[rp]? = some none)) := by
  obtain ⟨r, hr, hslot, -, hlen⟩ := h.edgeResult edge he
  simp only [hres] at hslot hlen
  obtain ⟨s, hs, hsAgent, -, hsPorts⟩ := h.observed.resultAfter r hr
  have hp : r.ports[rp]? = some r.ports[rp] := List.getElem?_eq_getElem hlen
  rcases h.resultPorts r hr rp _ hp with ⟨e', he', heRes', -, -, hcase⟩ | ⟨hnone, -, -⟩
  · have : e' = edge := edge_inj h he' he (by rw [heRes', hres, hslot])
    subst this
    refine ⟨r, hr, hslot, s, hslot ▸ hs, hsAgent, ?_⟩
    rw [hsPorts, hp]
    rcases hcase with ⟨peer, peerPort, hT, ht⟩ | ⟨o, hT, ht⟩
    · exact Or.inl ⟨peer, peerPort, hT, by rw [ht]⟩
    · exact Or.inr ⟨o, hT, by rw [ht]⟩
  · exact (hnone edge he (by rw [hres, hslot])).elim

/-- An output label never names a live post-state identity. -/
private theorem label_not_live
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {old : PhysicalOutput} (hmem : old ∈ state.outputs) :
    old.label.1 ∉ next.liveIds := by
  intro hlive
  obtain ⟨index, s, symbol, hs, hag⟩ := PhysicalState.mem_liveIds.mp hlive
  rcases slot_cases h (array_lt_of_some hs) with ⟨r, hr, rfl⟩ | hret | ⟨hlt, hrule, hvalue⟩
  · obtain ⟨s', hs', hs'Agent, -, -⟩ := h.observed.resultAfter r hr
    rw [hs] at hs'
    cases hs'
    rw [hs'Agent] at hag
    simp only [Option.some.injEq, Prod.mk.injEq] at hag
    exact (h.resultFresh r hr).2 old hmem hag.1.symm
  · obtain ⟨-, s', -, hs', hs'Agent, -⟩ := h.observed.retiredAfter index hret
    rw [hs] at hs'
    cases hs'
    rw [hs'Agent] at hag
    cases hag
  · obtain ⟨s', hs', hs'Agent, -⟩ :=
      h.observed.otherAfter index state.slots[index] (Array.getElem?_eq_getElem hlt)
        hrule hvalue
    rw [hs] at hs'
    cases hs'
    have hold : old.label.1 ∈ state.liveIds :=
      PhysicalState.mem_liveIds.mpr ⟨index, state.slots[index], symbol,
        Array.getElem?_eq_getElem hlt, by rw [← hs'Agent]; exact hag⟩
    obtain ⟨-, -, howned⟩ := PhysicalState.outputOwners_iff.mp h.legal.2.2.2.2.2.2.2
    obtain ⟨-, -, -, -, hdead⟩ := howned old hmem
    exact hdead hold

/-- An installed result slot has reciprocal physical ports after the rewrite. -/
private theorem result_reciprocal
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {result : ResultView} (hr : result ∈ results)
    {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : next.slots[result.slot]? = some slot) (hagent : slot.agent = some (id, symbol)) :
    next.PortsReciprocalAt result.slot slot symbol := by
  obtain ⟨s, hs, hsAgent, -, hsPorts⟩ := h.observed.resultAfter result hr
  rw [hs] at hslot
  cases hslot
  rw [hsAgent] at hagent
  cases hagent
  unfold PhysicalState.PortsReciprocalAt
  rw [hsPorts]
  refine ⟨h.resultArity result hr, fun port target hport => ?_⟩
  rcases h.resultPorts result hr port target hport with
    ⟨edge, he, heRes, -, hle, hcase⟩ | ⟨-, hlt, rfl⟩
  · refine ⟨fun hgt => absurd hle (Nat.not_le.mpr hgt), fun _ => ?_⟩
    rcases hcase with ⟨peer, peerPort, hT, rfl⟩ | ⟨o, hT, rfl⟩
    · obtain ⟨hpr, hpv, peerSlot, _, _, hpeer, hpeerAgent, -⟩ := edge_peer h he hT
      obtain ⟨remote, hremote, hremAgent, -, -, hrew, -⟩ :=
        h.observed.otherAfter peer peerSlot hpeer hpr hpv
      refine ⟨remote, hremote, by rw [hremAgent, hpeerAgent]; rfl,
        by rw [hrew edge he peerPort hT, heRes], ?_⟩
      intro output hmem heq
      obtain ⟨j, hj⟩ := List.mem_iff_getElem?.mp hmem
      have hT' := result_port_outputs h he hj (by rw [heq, heRes])
      rw [hT] at hT'
      simp at hT'
    · obtain ⟨out, hout, -⟩ := edge_output h he hT
      obtain ⟨out', hout', -, hre, -⟩ := h.observed.outputsAfter.2 o out hout
      refine filter_length_one hout' (by simp [hre edge he hT, heRes]) ?_
      intro j b hb hpb
      have hbt : b.target = edge.result := by
        rw [heRes]
        simpa using hpb
      have hT' := result_port_outputs h he hb hbt
      rw [hT] at hT'
      simp only [Prod.mk.injEq, true_and, and_true] at hT'
      exact hT'.symm
  · exact ⟨fun _ => rfl, fun hle => absurd hlt (Nat.not_lt.mpr hle)⟩

/-- A live prestate slot outside the consumed pair keeps reciprocal physical
ports after the rewrite. -/
private theorem other_reciprocal
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published)
    {index : Nat} {old : PhysicalSlot} (hold : state.slots[index]? = some old)
    (hrule : index ≠ ruleSlot) (hvalue : index ≠ valueSlot)
    {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : next.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol)) :
    next.PortsReciprocalAt index slot symbol := by
  obtain ⟨s, hs, hsAgent, -, hsLen, hrew, hkeep⟩ :=
    h.observed.otherAfter index old hold hrule hvalue
  rw [hs] at hslot
  cases hslot
  have hpre := prestate_reciprocal h hold (hsAgent ▸ hagent)
  unfold PhysicalState.PortsReciprocalAt at hpre ⊢
  obtain ⟨hlen, hports⟩ := hpre
  have hlt := array_lt_of_some hold
  refine ⟨hsLen ▸ hlen, fun port target hport => ?_⟩
  by_cases hedge : ∃ edge ∈ edges, edge.target = (0, index, port)
  · obtain ⟨edge, he, hT⟩ := hedge
    rcases hres : edge.result with ⟨rs, rp⟩
    have htarget : target = some (rs, rp) := by
      rw [hrew edge he port hT, hres] at hport
      exact (Option.some.inj hport).symm
    subst htarget
    obtain ⟨-, -, peerSlot, _, _, hpeer, -, hpeerPort⟩ := edge_peer h he hT
    rw [hold] at hpeer
    cases hpeer
    obtain ⟨hhigh, hlow⟩ := hports port _ hpeerPort
    have hle : port ≤ NetI64.arity symbol := Nat.not_lt.mp fun hgt => by
      have := hhigh hgt
      cases this
    obtain ⟨_, -, -, -, hnoOut⟩ := hlow hle
    refine ⟨fun hgt => absurd hle (Nat.not_le.mpr hgt), fun _ => ?_⟩
    obtain ⟨_, _, _, s', hs', hs'Agent, hcase⟩ := edge_result_port h he hres
    rcases hcase with ⟨peer, peerPort, hT', hp'⟩ | ⟨_, hT', -⟩
    · rw [hT] at hT'
      simp only [Prod.mk.injEq, true_and] at hT'
      obtain ⟨rfl, rfl⟩ := hT'
      exact ⟨s', hs', by rw [hs'Agent]; rfl, hp', other_no_output h hlt hrule hvalue hnoOut⟩
    · rw [hT] at hT'
      simp at hT'
  · have hfreeP : ∀ edge ∈ edges, edge.target ≠ (0, index, port) :=
      fun edge he hT => hedge ⟨edge, he, hT⟩
    rw [hkeep port hfreeP] at hport
    obtain ⟨hhigh, hlow⟩ := hports port target hport
    refine ⟨hhigh, fun hle => ?_⟩
    rcases target with _ | ⟨peer, peerPort⟩
    · have hl : (state.outputs.filter fun output => output.target == (index, port)).length
          = 1 := hlow hle
      show (next.outputs.filter fun output => output.target == (index, port)).length = 1
      rw [← hl]
      refine filter_length_congr h.observed.outputsAfter.1 fun i a b ha hb => ?_
      obtain ⟨old', hold', -, hcase⟩ := next_output h ha
      rw [hb] at hold'
      cases hold'
      rcases hcase with ⟨e, he, hT, ht⟩ | ⟨-, ht⟩
      · obtain ⟨out, hout, hout'⟩ := edge_output h he hT
        rw [hb] at hout
        cases hout
        obtain ⟨r, hr, hrSlot, -⟩ := h.edgeResult e he
        have hna : a.target ≠ (index, port) := fun heq =>
          not_result_of_other h hlt hrule hvalue r hr (by rw [hrSlot, ← ht, heq])
        have hnb : b.target ≠ (index, port) := fun heq => by
          rw [hout'] at heq
          simp only [Prod.mk.injEq] at heq
          exact hrule heq.1.symm
        simp only [beq_false_of_ne hna, beq_false_of_ne hnb]
      · simp only [ht]
    · have hl : ∃ remote, state.slots[peer]? = some remote ∧ remote.agent.isSome = true ∧
          remote.ports[peerPort]? = some (some (index, port)) ∧
          ∀ output ∈ state.outputs, output.target ≠ (index, port) := hlow hle
      obtain ⟨remote, hremote, hremAgent, hremPort, hnoOut⟩ := hl
      have hpr : peer ≠ ruleSlot := by
        intro heq
        rw [heq] at hremote
        obtain ⟨rOld, hrOld, hrAgent, hrPort0⟩ := h.ruleOld
        rw [hrOld] at hremote
        cases hremote
        rcases Nat.eq_zero_or_pos peerPort with rfl | hpos
        · rw [hrPort0] at hremPort
          simp only [Option.some.injEq, Prod.mk.injEq] at hremPort
          exact hvalue hremPort.1.symm
        · have hpre' := prestate_reciprocal h hrOld hrAgent
          unfold PhysicalState.PortsReciprocalAt at hpre'
          obtain ⟨hhi', -⟩ := hpre'.2 peerPort _ hremPort
          have hle' : peerPort ≤ NetI64.arity ruleSymbol := Nat.not_lt.mp fun hgt => by
            have := hhi' hgt
            cases this
          obtain ⟨e, he, -, hcase⟩ := rule_aux h hrOld hpos hle'
          rcases hcase with ⟨peer', port', hT, hp'⟩ | ⟨_, _, -, -, -, hp'⟩
          · rw [hremPort] at hp'
            simp only [Option.some.injEq, Prod.mk.injEq] at hp'
            obtain ⟨rfl, rfl⟩ := hp'
            exact hfreeP e he hT
          · rw [hremPort] at hp'
            simp at hp'
      have hpv : peer ≠ valueSlot := by
        intro heq
        rw [heq] at hremote
        obtain ⟨vOld, hvOld, hvAgent, hvPort0⟩ := h.valueOld
        rw [hvOld] at hremote
        cases hremote
        have hpre' := prestate_reciprocal h hvOld hvAgent
        unfold PhysicalState.PortsReciprocalAt at hpre'
        obtain ⟨hhi', -⟩ := hpre'.2 peerPort _ hremPort
        have hle0 : peerPort ≤ NetI64.arity valueSymbol := Nat.not_lt.mp fun hgt => by
          have := hhi' hgt
          cases this
        rw [h.valueArity] at hle0
        have hz : peerPort = 0 := by omega
        rw [hz, hvPort0] at hremPort
        simp only [Option.some.injEq, Prod.mk.injEq] at hremPort
        exact hrule hremPort.1.symm
      obtain ⟨s', hs', hs'Agent, -, -, -, hkeep'⟩ :=
        h.observed.otherAfter peer remote hremote hpr hpv
      refine ⟨s', hs', by rw [hs'Agent]; exact hremAgent, ?_,
        other_no_output h hlt hrule hvalue hnoOut⟩
      rw [hkeep' peerPort ?_]
      · exact hremPort
      · intro e he hT
        obtain ⟨-, -, ps, _, _, hps, -, hpp⟩ := edge_peer h he hT
        rw [hremote] at hps
        cases hps
        rw [hremPort] at hpp
        simp only [Option.some.injEq, Prod.mk.injEq] at hpp
        exact hrule hpp.1

/-- The free-slot inventory stays exact, duplicate-free and in bounds. -/
theorem PhysicalState.LocalRewrite.free_fields
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.freeSlots.Nodup ∧
    next.freeSlots.all (fun index => decide (index < next.slots.size)) = true ∧
    (List.range next.slots.size).all (fun index =>
      decide (index ∈ next.freeSlots) ==
        ((next.slots[index]?).map (fun slot => slot.agent.isNone)).getD false) = true := by
  obtain ⟨rOld, hrOld, hrAgent, -⟩ := h.ruleOld
  obtain ⟨vOld, hvOld, hvAgent, -⟩ := h.valueOld
  have hl := h.legal
  have hrFree : ruleSlot ∉ state.freeSlots := fun hm => by
    have := (prestate_free_iff hl hrOld).mp hm
    rw [hrAgent] at this
    cases this
  have hvFree : valueSlot ∉ state.freeSlots := fun hm => by
    have := (prestate_free_iff hl hvOld).mp hm
    rw [hvAgent] at this
    cases this
  have hretMem : ∀ index ∈ retired, index = ruleSlot ∨ index = valueSlot :=
    fun index hm => ((h.retiredExact index).mp hm).1
  have hFreeLt : ∀ index ∈ state.freeSlots, index < state.slots.size := fun index hm => by
    have := List.all_eq_true.mp hl.2.1 index hm
    simpa using this
  refine ⟨?_, ?_, ?_⟩
  · rw [h.observed.freeAfter, List.nodup_append]
    refine ⟨h.retiredNodup, hl.1, fun a ha b hb hab => ?_⟩
    subst hab
    rcases hretMem a ha with rfl | rfl
    · exact hrFree hb
    · exact hvFree hb
  · rw [List.all_eq_true]
    intro index hm
    rw [h.observed.freeAfter, List.mem_append] at hm
    rw [h.observed.size]
    simp only [decide_eq_true_eq]
    rcases hm with hm | hm
    · rcases hretMem index hm with rfl | rfl
      · have := array_lt_of_some hrOld
        omega
      · have := array_lt_of_some hvOld
        omega
    · have := hFreeLt index hm
      omega
  · rw [List.all_eq_true]
    intro index hm
    have hlt := List.mem_range.mp hm
    rw [Array.getElem?_eq_getElem hlt]
    suffices hiff : index ∈ next.freeSlots ↔ next.slots[index].agent = none by
      simp only [Option.map_some, Option.getD_some]
      cases hag : next.slots[index].agent <;> simp [hiff, hag]
    rw [h.observed.freeAfter, List.mem_append]
    rcases slot_cases h hlt with ⟨r, hr, rfl⟩ | hret | ⟨hlt', hrule, hvalue⟩
    · obtain ⟨s, hs, hsAgent, -, -⟩ := h.observed.resultAfter r hr
      rw [Array.getElem?_eq_getElem hlt] at hs
      cases hs
      rw [hsAgent]
      simp only [reduceCtorEq, iff_false, not_or]
      refine ⟨fun hm => ((h.retiredExact _).mp hm).2 r hr rfl, fun hm => ?_⟩
      rcases h.resultPlace r hr with (heq | heq) | ⟨hge, -⟩
      · exact hrFree (heq ▸ hm)
      · exact hvFree (heq ▸ hm)
      · have := hFreeLt _ hm
        omega
    · obtain ⟨-, s, -, hs, hsAgent, -⟩ := h.observed.retiredAfter index hret
      rw [Array.getElem?_eq_getElem hlt] at hs
      cases hs
      exact iff_of_true (Or.inl hret) hsAgent
    · obtain ⟨s, hs, hsAgent, -⟩ :=
        h.observed.otherAfter index state.slots[index] (Array.getElem?_eq_getElem hlt')
          hrule hvalue
      rw [Array.getElem?_eq_getElem hlt] at hs
      cases hs
      rw [hsAgent, ← prestate_free_iff hl (Array.getElem?_eq_getElem hlt')]
      constructor
      · rintro (hm | hm)
        · rcases hretMem index hm with heq | heq
          · exact absurd heq hrule
          · exact absurd heq hvalue
        · exact hm
      · exact Or.inr

/-- Every physical queue entry stays in canonical slot order. -/
theorem PhysicalState.LocalRewrite.work_canonical
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.work.all (fun pair => decide (pair.first < pair.second)) = true := by
  rw [h.observed.workAfter, List.all_append, Bool.and_eq_true]
  refine ⟨h.legal.2.2.2.1, List.all_eq_true.mpr fun pair hp => ?_⟩
  obtain ⟨result, hr, edge, he, peer, peerSlot, -, hT, hpeer, rfl⟩ :=
    (h.publishedExact pair).mp hp
  obtain ⟨hpr, hpv, -⟩ := edge_peer h he hT
  have hne : result.slot ≠ peer := by
    intro heq
    rcases h.resultPlace result hr with (h1 | h1) | ⟨h1, -⟩
    · exact hpr (by rw [← heq]; exact h1)
    · exact hpv (by rw [← heq]; exact h1)
    · have := array_lt_of_some hpeer
      omega
  unfold PhysicalState.canonicalPair
  split <;> simp only [decide_eq_true_eq] <;> omega

/-- Every live port of the executed arena has a reciprocal owner. -/
theorem PhysicalState.LocalRewrite.reciprocalPorts
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.reciprocalPorts = true := by
  rw [PhysicalState.reciprocalPorts_iff]
  intro index slot id symbol hslot hagent
  rcases slot_cases h (array_lt_of_some hslot) with ⟨result, hr, rfl⟩ | hret |
    ⟨hlt, hrule, hvalue⟩
  · exact result_reciprocal h hr hslot hagent
  · obtain ⟨-, s, -, hs, hsAgent, -⟩ := h.observed.retiredAfter index hret
    rw [hs] at hslot
    cases hslot
    rw [hsAgent] at hagent
    cases hagent
  · exact other_reciprocal h (Array.getElem?_eq_getElem hlt) hrule hvalue hslot hagent

/-- Every output keeps a unique label and a unique empty live target. -/
theorem PhysicalState.LocalRewrite.outputOwners
    (h : state.LocalRewrite next selected ruleSlot valueSlot ruleId valueId
      ruleSymbol valueSymbol results edges retired fresh published) :
    next.outputOwners = true := by
  obtain ⟨hlabels, htargets, -⟩ :=
    PhysicalState.outputOwners_iff.mp h.legal.2.2.2.2.2.2.2
  rw [PhysicalState.outputOwners_iff]
  refine ⟨?_, ?_, ?_⟩
  · have hmap : next.outputs.map PhysicalOutput.label =
        state.outputs.map PhysicalOutput.label := by
      apply List.ext_getElem?
      intro o
      rw [List.getElem?_map, List.getElem?_map]
      cases hout : next.outputs[o]? with
      | none =>
        have hge : state.outputs.length ≤ o := by
          rw [← h.observed.outputsAfter.1]
          exact List.getElem?_eq_none_iff.mp hout
        rw [List.getElem?_eq_none hge]
      | some out =>
        obtain ⟨old, hold, hlabel, -⟩ := next_output h hout
        simp [hold, hlabel]
    rw [hmap]
    exact hlabels
  · apply nodup_of_index_inj
    intro i j t hi hj
    rw [List.getElem?_map] at hi hj
    obtain ⟨a, ha, rfl⟩ := Option.map_eq_some_iff.mp hi
    obtain ⟨b, hb, hab⟩ := Option.map_eq_some_iff.mp hj
    obtain ⟨oa, hoa, -, hca⟩ := next_output h ha
    obtain ⟨ob, hob, -, hcb⟩ := next_output h hb
    rcases hca with ⟨ea, hea, hTa, hta⟩ | ⟨hfa, hta⟩ <;>
      rcases hcb with ⟨eb, heb, hTb, htb⟩ | ⟨hfb, htb⟩
    · have : ea = eb := edge_inj h hea heb (by rw [← hta, ← htb, hab])
      subst this
      rw [hTa] at hTb
      simp only [Prod.mk.injEq, true_and, and_true] at hTb
      exact hTb
    · obtain ⟨r, hr, hrSlot, -⟩ := h.edgeResult ea hea
      exact (untouched_not_result h hob hfb r hr (by rw [hrSlot, ← hta, ← hab, htb])).elim
    · obtain ⟨r, hr, hrSlot, -⟩ := h.edgeResult eb heb
      exact (untouched_not_result h hoa hfa r hr (by rw [hrSlot, ← htb, hab, hta])).elim
    · exact index_inj_of_nodup htargets
        (by simp [List.getElem?_map, hoa, ← hta] :
          (state.outputs.map PhysicalOutput.target)[i]? = some a.target)
        (by simp [List.getElem?_map, hob, ← htb, hab] :
          (state.outputs.map PhysicalOutput.target)[j]? = some a.target)
  · intro output hmem
    obtain ⟨o, ho⟩ := List.mem_iff_getElem?.mp hmem
    obtain ⟨old, hold, hlabel, hcase⟩ := next_output h ho
    have hdead : output.label.1 ∉ next.liveIds := by
      rw [hlabel]
      exact label_not_live h (List.mem_of_getElem? hold)
    rcases hcase with ⟨edge, he, hT, ht⟩ | ⟨hfree, ht⟩
    · rcases hres : edge.result with ⟨rs, rp⟩
      obtain ⟨_, _, _, s, hs, hsAgent, hcase⟩ := edge_result_port h he hres
      rcases hcase with ⟨_, _, hT', -⟩ | ⟨_, -, hnone⟩
      · rw [hT] at hT'
        simp at hT'
      · rw [ht, hres]
        exact ⟨s, hs, by rw [hsAgent]; rfl, hnone, hdead⟩
    · obtain ⟨hr, hv, slot, hs, hsAgent, hsPort⟩ := untouched_output h hold hfree
      obtain ⟨s', hs', hs'Agent, -, -, -, hkeep⟩ :=
        h.observed.otherAfter old.target.1 slot hs hr hv
      refine ⟨s', by rw [ht]; exact hs', by rw [hs'Agent]; exact hsAgent, ?_, hdead⟩
      rw [ht, hkeep old.target.2 ?_]
      · exact hsPort
      · intro e he hT
        obtain ⟨-, -, ps, _, _, hps, -, hpp⟩ := edge_peer h he hT
        rw [hs] at hps
        cases hps
        rw [hsPort] at hpp
        simp at hpp

end

end NobleFoundation.NetI64Plans
