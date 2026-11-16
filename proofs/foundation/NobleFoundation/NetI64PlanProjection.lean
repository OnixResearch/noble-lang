import NobleFoundation.NetI64PlanLegal

/-!
# Reading the physical-to-inets projection by lookup (issue #110)

`PhysicalState.project` builds its agent and wire hash maps by folding over
the physical slots and then over the ordered output table. These lemmas read
single lookups of those folds back as physical facts, under explicit key
separation: unique live logical identities, unique output labels and targets,
output labels whose identities are not live, and output targets that name an
empty port of a live slot. They also restate the Boolean structural checks
`reciprocalPorts` and `outputOwners` as propositions. They are lookup
characterizations of the model projection only; they do not relate any Rust
data structure to it.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- Logical identities carried by live physical slots, in slot order. -/
def PhysicalState.liveIds (state : PhysicalState) : List Id :=
  state.slots.toList.filterMap fun slot => slot.agent.map Prod.fst

/-- The projected key spaces cannot collide: live identities are unique, each
output has a unique label and target, no output label names a live agent, and
every output targets an empty port of an existing slot. -/
structure PhysicalState.KeysSeparated (state : PhysicalState) : Prop where
  unique : state.liveIds.Nodup
  labels : (state.outputs.map PhysicalOutput.label).Nodup
  targets : (state.outputs.map PhysicalOutput.target).Nodup
  labelDead : ∀ output ∈ state.outputs, output.label.1 ∉ state.liveIds
  targetEmpty : ∀ output ∈ state.outputs, ∃ slot,
    state.slots[output.target.1]? = some slot ∧
      slot.ports[output.target.2]? = some none

/-- One slot step of the projected agent fold. -/
private def agentStep (agents : Std.HashMap Id NetSym) (slot : PhysicalSlot) :
    Std.HashMap Id NetSym :=
  match slot.agent with
  | none => agents
  | some (logical, symbol) => agents.insert logical symbol

/-- One port step of `PhysicalState.portWires`. -/
private def portStep (state : PhysicalState) (logical : Id) (index : Nat)
    (target : Option (Nat × Nat)) (wires : Std.HashMap Port Port) :
    Std.HashMap Port Port :=
  match target with
  | none => wires
  | some target =>
    match state.logicalPort target with
    | none => wires
    | some endpoint => wires.insert (logical, index) endpoint

/-- One slot step of the projected internal-wire fold. -/
private def internalStep (state : PhysicalState) (wires : Std.HashMap Port Port)
    (slot : PhysicalSlot) : Std.HashMap Port Port :=
  match slot.agent with
  | none => wires
  | some (logical, _) => state.portWires logical 0 slot.ports wires

/-- One output step of the projected wire fold. -/
private def outputStep (state : PhysicalState) (wires : Std.HashMap Port Port)
    (output : PhysicalOutput) : Std.HashMap Port Port :=
  match state.logicalPort output.target with
  | none => wires
  | some endpoint => (wires.insert output.label endpoint).insert endpoint output.label

/-- The projected agents are the agent fold over the slots. -/
private theorem agents_eq_fold (state : PhysicalState) :
    state.agents = state.slots.toList.foldl agentStep {} := rfl

/-- `portWires` on a nonempty port list is one port step followed by the rest. -/
private theorem portWires_cons (state : PhysicalState) (logical : Id) (start : Nat)
    (target : Option (Nat × Nat)) (rest : List (Option (Nat × Nat)))
    (wires : Std.HashMap Port Port) :
    state.portWires logical start (target :: rest) wires =
      state.portWires logical (start + 1) rest (portStep state logical start target wires) :=
  rfl

/-- The projected wires are the output fold over the internal-wire fold. -/
private theorem wires_eq_fold (state : PhysicalState) :
    state.wires = state.outputs.foldl (outputStep state)
      (state.slots.toList.foldl (internalStep state) {}) := rfl

/-- Membership in the live identities of a slot list. -/
private theorem mem_filterMap_agent {l : List PhysicalSlot} {id : Id} :
    id ∈ l.filterMap (fun slot => slot.agent.map Prod.fst) ↔
      ∃ slot ∈ l, ∃ symbol, slot.agent = some (id, symbol) := by
  simp only [List.mem_filterMap, Option.map_eq_some_iff]
  constructor
  · rintro ⟨slot, hmem, ⟨logical, symbol⟩, hagent, rfl⟩
    exact ⟨slot, hmem, symbol, hagent⟩
  · rintro ⟨slot, hmem, symbol, hagent⟩
    exact ⟨slot, hmem, (id, symbol), hagent, rfl⟩

/-- A live head slot contributes its identity first. -/
private theorem filterMap_agent_cons_some {slot : PhysicalSlot}
    {rest : List PhysicalSlot} {id : Id} {symbol : NetSym}
    (h : slot.agent = some (id, symbol)) :
    (slot :: rest).filterMap (fun slot => slot.agent.map Prod.fst) =
      id :: rest.filterMap (fun slot => slot.agent.map Prod.fst) := by
  simp [h]

/-- A dead head slot contributes no identity. -/
private theorem filterMap_agent_cons_none {slot : PhysicalSlot}
    {rest : List PhysicalSlot} (h : slot.agent = none) :
    (slot :: rest).filterMap (fun slot => slot.agent.map Prod.fst) =
      rest.filterMap (fun slot => slot.agent.map Prod.fst) := by
  simp [h]

/-- Unique live identities of a slot list determine the list index. -/
private theorem index_unique_of_nodup {l : List PhysicalSlot} {first second : Nat}
    {firstSlot secondSlot : PhysicalSlot} {id : Id} {firstSymbol secondSymbol : NetSym}
    (hnodup : (l.filterMap fun slot => slot.agent.map Prod.fst).Nodup)
    (hfirst : l[first]? = some firstSlot)
    (hfirstAgent : firstSlot.agent = some (id, firstSymbol))
    (hsecond : l[second]? = some secondSlot)
    (hsecondAgent : secondSlot.agent = some (id, secondSymbol)) :
    first = second := by
  induction l generalizing first second with
  | nil => simp at hfirst
  | cons head rest ih =>
    cases first with
    | zero =>
      cases second with
      | zero => rfl
      | succ second =>
        have hhead : head = firstSlot := Option.some.inj hfirst
        rw [← hhead] at hfirstAgent
        rw [filterMap_agent_cons_some hfirstAgent, List.nodup_cons] at hnodup
        exact absurd (mem_filterMap_agent.2 ⟨secondSlot,
          List.mem_of_getElem? (show rest[second]? = some secondSlot from hsecond),
          secondSymbol, hsecondAgent⟩) hnodup.1
    | succ first =>
      cases second with
      | zero =>
        have hhead : head = secondSlot := Option.some.inj hsecond
        rw [← hhead] at hsecondAgent
        rw [filterMap_agent_cons_some hsecondAgent, List.nodup_cons] at hnodup
        exact absurd (mem_filterMap_agent.2 ⟨firstSlot,
          List.mem_of_getElem? (show rest[first]? = some firstSlot from hfirst),
          firstSymbol, hfirstAgent⟩) hnodup.1
      | succ second =>
        exact congrArg (· + 1) (ih
          (hnodup.sublist ((List.sublist_cons_self head rest).filterMap _)) hfirst hsecond)

/-- Membership in the live identities is exactly a live slot carrying it. -/
theorem PhysicalState.mem_liveIds {state : PhysicalState} {id : Id} :
    id ∈ state.liveIds ↔ ∃ (index : Nat) (slot : PhysicalSlot) (symbol : NetSym),
      state.slots[index]? = some slot ∧ slot.agent = some (id, symbol) := by
  unfold PhysicalState.liveIds
  rw [mem_filterMap_agent]
  constructor
  · rintro ⟨slot, hmem, symbol, hagent⟩
    obtain ⟨index, hslot⟩ := Array.mem_iff_getElem?.1 (Array.mem_toList_iff.1 hmem)
    exact ⟨index, slot, symbol, hslot, hagent⟩
  · rintro ⟨index, slot, symbol, hslot, hagent⟩
    exact ⟨slot, Array.mem_toList_iff.2 (Array.mem_of_getElem? hslot), symbol, hagent⟩

/-- Unique live identities determine their physical slot index. -/
theorem PhysicalState.slot_index_unique {state : PhysicalState}
    (hunique : state.liveIds.Nodup) {first second : Nat}
    {firstSlot secondSlot : PhysicalSlot} {id : Id}
    {firstSymbol secondSymbol : NetSym}
    (hfirst : state.slots[first]? = some firstSlot)
    (hfirstAgent : firstSlot.agent = some (id, firstSymbol))
    (hsecond : state.slots[second]? = some secondSlot)
    (hsecondAgent : secondSlot.agent = some (id, secondSymbol)) :
    first = second := by
  rw [← Array.getElem?_toList] at hfirst hsecond
  exact index_unique_of_nodup hunique hfirst hfirstAgent hsecond hsecondAgent

/-- The agent fold leaves every identity carried by no live slot untouched. -/
private theorem agentFold_of_not_mem {l : List PhysicalSlot}
    {agents : Std.HashMap Id NetSym} {id : Id}
    (h : id ∉ l.filterMap (fun slot => slot.agent.map Prod.fst)) :
    (l.foldl agentStep agents)[id]? = agents[id]? := by
  induction l generalizing agents with
  | nil => rfl
  | cons slot rest ih =>
    rw [List.foldl_cons]
    cases hs : slot.agent with
    | none =>
      rw [filterMap_agent_cons_none hs] at h
      rw [ih h]
      simp only [agentStep, hs]
    | some entry =>
      obtain ⟨logical, symbol⟩ := entry
      rw [filterMap_agent_cons_some hs, List.mem_cons, not_or] at h
      rw [ih h.2]
      simp only [agentStep, hs]
      rw [hget_insert, ite_eq_right (Ne.symm h.1)]

/-- With unique live identities, the agent fold reads a live slot's symbol. -/
private theorem agentFold_of_mem {l : List PhysicalSlot}
    {agents : Std.HashMap Id NetSym} {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hnodup : (l.filterMap fun slot => slot.agent.map Prod.fst).Nodup)
    (hmem : slot ∈ l) (hagent : slot.agent = some (id, symbol)) :
    (l.foldl agentStep agents)[id]? = some symbol := by
  induction l generalizing agents with
  | nil => exact absurd hmem List.not_mem_nil
  | cons head rest ih =>
    rw [List.foldl_cons]
    rcases List.mem_cons.1 hmem with heq | hrest
    · rw [heq] at hagent
      rw [filterMap_agent_cons_some hagent, List.nodup_cons] at hnodup
      rw [agentFold_of_not_mem hnodup.1]
      simp only [agentStep, hagent]
      exact Std.HashMap.getElem?_insert_self
    · exact ih (hnodup.sublist ((List.sublist_cons_self head rest).filterMap _)) hrest

/-- A live slot's symbol is the projected agent of its unique identity. -/
theorem PhysicalState.agents_getElem?_of_slot {state : PhysicalState}
    (hunique : state.liveIds.Nodup) {index : Nat} {slot : PhysicalSlot}
    {id : Id} {symbol : NetSym}
    (hslot : state.slots[index]? = some slot)
    (hagent : slot.agent = some (id, symbol)) :
    state.agents[id]? = some symbol := by
  rw [agents_eq_fold]
  exact agentFold_of_mem hunique (Array.mem_toList_iff.2 (Array.mem_of_getElem? hslot)) hagent

/-- An identity carried by no live slot has no projected agent. -/
theorem PhysicalState.agents_getElem?_of_not_live {state : PhysicalState}
    {id : Id} (h : id ∉ state.liveIds) :
    state.agents[id]? = none := by
  rw [agents_eq_fold, agentFold_of_not_mem h, Std.HashMap.getElem?_empty]

/-- A decoded logical port names a live slot's identity at the same port. -/
private theorem logicalPort_live {state : PhysicalState} {port : Nat × Nat}
    {endpoint : Port} (h : state.logicalPort port = some endpoint) :
    ∃ slot symbol, state.slots[port.1]? = some slot ∧
      slot.agent = some (endpoint.1, symbol) ∧ endpoint.2 = port.2 := by
  unfold PhysicalState.logicalPort at h
  cases hslot : state.slots[port.1]? with
  | none => simp [hslot] at h
  | some slot =>
    cases hagent : slot.agent with
    | none => simp [hslot, hagent] at h
    | some entry =>
      obtain ⟨logical, symbol⟩ := entry
      simp [hslot, hagent] at h
      subst h
      exact ⟨slot, symbol, rfl, hagent, rfl⟩

/-- A live slot decodes each of its physical ports to its logical port. -/
private theorem logicalPort_of_slot {state : PhysicalState} {port : Nat × Nat}
    {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : state.slots[port.1]? = some slot)
    (hagent : slot.agent = some (id, symbol)) :
    state.logicalPort port = some (id, port.2) := by
  simp [PhysicalState.logicalPort, hslot, hagent]

/-- A decoded logical port carries a live identity. -/
private theorem logicalPort_mem_liveIds {state : PhysicalState} {port : Nat × Nat}
    {endpoint : Port} (h : state.logicalPort port = some endpoint) :
    endpoint.1 ∈ state.liveIds := by
  obtain ⟨slot, symbol, hslot, hagent, _⟩ := logicalPort_live h
  exact PhysicalState.mem_liveIds.2 ⟨port.1, slot, symbol, hslot, hagent⟩

/-- Under unique live identities, a decoded logical port determines its
physical port. -/
private theorem logicalPort_injective {state : PhysicalState}
    (hunique : state.liveIds.Nodup) {first second : Nat × Nat} {endpoint : Port}
    (hfirst : state.logicalPort first = some endpoint)
    (hsecond : state.logicalPort second = some endpoint) : first = second := by
  obtain ⟨firstSlot, firstSymbol, hfirstSlot, hfirstAgent, hfirstPort⟩ :=
    logicalPort_live hfirst
  obtain ⟨secondSlot, secondSymbol, hsecondSlot, hsecondAgent, hsecondPort⟩ :=
    logicalPort_live hsecond
  exact Prod.ext
    (PhysicalState.slot_index_unique hunique hfirstSlot hfirstAgent hsecondSlot hsecondAgent)
    (hfirstPort.symm.trans hsecondPort)

/-- A port step writes at most the key of its own port. -/
private theorem portStep_getElem? (state : PhysicalState) (logical : Id) (index : Nat)
    (target : Option (Nat × Nat)) (wires : Std.HashMap Port Port) (key : Port) :
    (portStep state logical index target wires)[key]? =
      if (logical, index) = key then (target.bind state.logicalPort).or wires[key]?
      else wires[key]? := by
  cases target with
  | none => simp [portStep]
  | some target =>
    cases hp : state.logicalPort target with
    | none => simp [portStep, hp]
    | some endpoint => simp [portStep, hp, hget_insert]

/-- Writing one slot's ports leaves keys of other identities, and keys below
the first written port, untouched. -/
private theorem portWires_away (state : PhysicalState) (logical : Id) {key : Port}
    {start : Nat} {ports : List (Option (Nat × Nat))} {wires : Std.HashMap Port Port}
    (h : key.1 ≠ logical ∨ key.2 < start) :
    (state.portWires logical start ports wires)[key]? = wires[key]? := by
  induction ports generalizing start wires with
  | nil => rfl
  | cons target rest ih =>
    rw [portWires_cons, ih (h.imp id (fun hlt => Nat.lt_succ_of_lt hlt)),
      portStep_getElem?, ite_eq_right]
    intro heq
    rcases h with hne | hlt
    · exact hne (by rw [← heq])
    · rw [← heq] at hlt
      exact Nat.lt_irrefl _ hlt

/-- Writing one slot's ports reads the stored target of the requested port,
falling back to the initial map. -/
private theorem portWires_self (state : PhysicalState) (logical : Id)
    {ports : List (Option (Nat × Nat))} :
    ∀ {start : Nat} {wires : Std.HashMap Port Port} (k : Nat),
      (state.portWires logical start ports wires)[(logical, start + k)]? =
        ((ports[k]?).bind fun target => target.bind state.logicalPort).or
          wires[(logical, start + k)]? := by
  induction ports with
  | nil => intro start wires k; simp [PhysicalState.portWires]
  | cons target rest ih =>
    intro start wires k
    cases k with
    | zero =>
      rw [portWires_cons, portWires_away state logical (key := (logical, start + 0))
          (start := start + 1) (Or.inr (Nat.lt_succ_self start)),
        portStep_getElem?,
        ite_eq_left (show (logical, start) = (logical, start + 0) from rfl)]
      rfl
    | succ k =>
      have hk : start + (k + 1) = start + 1 + k := by omega
      rw [hk, portWires_cons, ih k, portStep_getElem?,
        ite_eq_right (fun heq => by injection heq with _ heq; omega)]
      rfl

/-- The internal-wire fold leaves every key of a non-live identity untouched. -/
private theorem internalFold_of_not_mem (state : PhysicalState) {l : List PhysicalSlot}
    {wires : Std.HashMap Port Port} {key : Port}
    (h : key.1 ∉ l.filterMap (fun slot => slot.agent.map Prod.fst)) :
    (l.foldl (internalStep state) wires)[key]? = wires[key]? := by
  induction l generalizing wires with
  | nil => rfl
  | cons slot rest ih =>
    rw [List.foldl_cons]
    cases hs : slot.agent with
    | none =>
      rw [filterMap_agent_cons_none hs] at h
      rw [ih h]
      simp only [internalStep, hs]
    | some entry =>
      obtain ⟨logical, symbol⟩ := entry
      rw [filterMap_agent_cons_some hs, List.mem_cons, not_or] at h
      rw [ih h.2]
      simp only [internalStep, hs]
      exact portWires_away state logical (Or.inl h.1)

/-- With unique live identities, the internal-wire fold reads a live slot's
stored port reference, falling back to the initial map. -/
private theorem internalFold_of_mem (state : PhysicalState) {l : List PhysicalSlot}
    {wires : Std.HashMap Port Port} {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (port : Nat)
    (hnodup : (l.filterMap fun slot => slot.agent.map Prod.fst).Nodup)
    (hmem : slot ∈ l) (hagent : slot.agent = some (id, symbol)) :
    (l.foldl (internalStep state) wires)[(id, port)]? =
      ((slot.ports[port]?).bind fun target => target.bind state.logicalPort).or
        wires[(id, port)]? := by
  induction l generalizing wires with
  | nil => exact absurd hmem List.not_mem_nil
  | cons head rest ih =>
    rw [List.foldl_cons]
    rcases List.mem_cons.1 hmem with heq | hrest
    · subst heq
      rw [filterMap_agent_cons_some hagent, List.nodup_cons] at hnodup
      rw [internalFold_of_not_mem state hnodup.1]
      simp only [internalStep, hagent]
      have := portWires_self state id (ports := slot.ports) (start := 0) (wires := wires) port
      rw [Nat.zero_add] at this
      exact this
    · rw [ih (hnodup.sublist ((List.sublist_cons_self head rest).filterMap _)) hrest]
      cases hs : head.agent with
      | none => simp only [internalStep, hs]
      | some entry =>
        obtain ⟨logical, other⟩ := entry
        have hne : id ≠ logical := by
          rw [filterMap_agent_cons_some hs, List.nodup_cons] at hnodup
          intro heq
          rw [heq] at hagent
          exact hnodup.1 (mem_filterMap_agent.2 ⟨slot, hrest, symbol, hagent⟩)
        simp only [internalStep, hs]
        rw [portWires_away state logical (Or.inl hne)]

/-- A key whose identity is not live is absent from the internal wires. -/
private theorem internal_of_not_live {state : PhysicalState} {key : Port}
    (h : key.1 ∉ state.liveIds) :
    (state.slots.toList.foldl (internalStep state) {})[key]? = none := by
  rw [internalFold_of_not_mem state h, Std.HashMap.getElem?_empty]

/-- The output fold leaves a key untouched when no output names it as its
label or as its decoded endpoint. -/
private theorem outputFold_away (state : PhysicalState) {outputs : List PhysicalOutput}
    {wires : Std.HashMap Port Port} {key : Port}
    (h : ∀ output ∈ outputs,
      output.label ≠ key ∧ state.logicalPort output.target ≠ some key) :
    (outputs.foldl (outputStep state) wires)[key]? = wires[key]? := by
  induction outputs generalizing wires with
  | nil => rfl
  | cons output rest ih =>
    rw [List.foldl_cons, ih (fun other hother => h other (List.mem_cons_of_mem _ hother))]
    obtain ⟨hlabel, hend⟩ := h output List.mem_cons_self
    cases hp : state.logicalPort output.target with
    | none => simp only [outputStep, hp]
    | some endpoint =>
      simp only [outputStep, hp]
      rw [hget_insert, ite_eq_right (fun heq => hend (by rw [hp, heq])), hget_insert,
        ite_eq_right hlabel]

/-- No decoded endpoint is an output label under key separation. -/
private theorem PhysicalState.KeysSeparated.label_ne_endpoint {state : PhysicalState}
    (hsep : state.KeysSeparated) {output : PhysicalOutput} (hmem : output ∈ state.outputs)
    {port : Nat × Nat} {endpoint : Port} (h : state.logicalPort port = some endpoint) :
    output.label ≠ endpoint := by
  intro heq
  exact hsep.labelDead output hmem (by rw [heq]; exact logicalPort_mem_liveIds h)

/-- A stable output label resolves to its current physical target. -/
theorem PhysicalState.wires_label {state : PhysicalState}
    (hsep : state.KeysSeparated) {output : PhysicalOutput}
    (hmem : output ∈ state.outputs) :
    state.wires[output.label]? = state.logicalPort output.target := by
  obtain ⟨before, after, hsplit⟩ := List.append_of_mem hmem
  have hlabels := hsep.labels
  rw [hsplit, List.map_append, List.map_cons, List.nodup_append, List.nodup_cons] at hlabels
  have hendpoint : ∀ other : PhysicalOutput,
      state.logicalPort other.target ≠ some output.label :=
    fun other hend => hsep.label_ne_endpoint hmem hend rfl
  rw [wires_eq_fold, hsplit, List.foldl_append, List.foldl_cons,
    outputFold_away state (outputs := after)]
  · cases hp : state.logicalPort output.target with
    | none =>
      simp only [outputStep, hp]
      rw [outputFold_away state (outputs := before)]
      · exact internal_of_not_live (hsep.labelDead output hmem)
      · intro other hother
        exact ⟨hlabels.2.2 other.label (List.mem_map_of_mem hother) output.label
          List.mem_cons_self, hendpoint other⟩
    | some endpoint =>
      simp only [outputStep, hp]
      rw [hget_insert, ite_eq_right (fun heq => hsep.label_ne_endpoint hmem hp heq.symm),
        hget_insert, ite_eq_left rfl]
  · intro other hother
    exact ⟨fun heq => hlabels.2.1.1 (heq ▸ List.mem_map_of_mem hother), hendpoint other⟩

/-- The logical port targeted by an output resolves back to its label. -/
theorem PhysicalState.wires_endpoint {state : PhysicalState}
    (hsep : state.KeysSeparated) {output : PhysicalOutput}
    (hmem : output ∈ state.outputs) {endpoint : Port}
    (hend : state.logicalPort output.target = some endpoint) :
    state.wires[endpoint]? = some output.label := by
  obtain ⟨before, after, hsplit⟩ := List.append_of_mem hmem
  have htargets := hsep.targets
  rw [hsplit, List.map_append, List.map_cons, List.nodup_append, List.nodup_cons] at htargets
  rw [wires_eq_fold, hsplit, List.foldl_append, List.foldl_cons,
    outputFold_away state (outputs := after)]
  · simp only [outputStep, hend]
    exact Std.HashMap.getElem?_insert_self
  · intro other hother
    have hmem' : other ∈ state.outputs := by
      rw [hsplit]
      exact List.mem_append_right _ (List.mem_cons_of_mem _ hother)
    refine ⟨hsep.label_ne_endpoint hmem' hend, fun hother' => ?_⟩
    have htarget := logicalPort_injective hsep.unique hother' hend
    exact htargets.2.1.1 (htarget ▸ List.mem_map_of_mem hother)

/-- A live port not targeted by any output resolves through its own stored
physical reference. -/
theorem PhysicalState.wires_internal {state : PhysicalState}
    (hsep : state.KeysSeparated) {index : Nat} {slot : PhysicalSlot}
    {id : Id} {symbol : NetSym} {port : Nat}
    (hslot : state.slots[index]? = some slot)
    (hagent : slot.agent = some (id, symbol))
    (hfree : ∀ output ∈ state.outputs, output.target ≠ (index, port)) :
    state.wires[(id, port)]? =
      (slot.ports[port]?).bind fun target => target.bind state.logicalPort := by
  rw [wires_eq_fold, outputFold_away state (outputs := state.outputs),
    internalFold_of_mem state port hsep.unique
      (Array.mem_toList_iff.2 (Array.mem_of_getElem? hslot)) hagent,
    Std.HashMap.getElem?_empty, Option.or_none]
  intro output hmem
  refine ⟨fun heq => hsep.labelDead output hmem (by
    rw [heq]
    exact PhysicalState.mem_liveIds.2 ⟨index, slot, symbol, hslot, hagent⟩), fun hend => ?_⟩
  exact hfree output hmem (logicalPort_injective hsep.unique hend
    (logicalPort_of_slot (port := (index, port)) hslot hagent))

/-- A key that is neither a live port nor an output label has no wire. -/
theorem PhysicalState.wires_absent {state : PhysicalState}
    {key : Port}
    (hlive : key.1 ∉ state.liveIds)
    (hlabel : ∀ output ∈ state.outputs, output.label ≠ key) :
    state.wires[key]? = none := by
  rw [wires_eq_fold, outputFold_away state (outputs := state.outputs)
    (fun output hmem => ⟨hlabel output hmem, fun hend => hlive (logicalPort_mem_liveIds hend)⟩)]
  exact internal_of_not_live hlive

/-- The per-slot content of `reciprocalPorts` as a proposition. -/
def PhysicalState.PortsReciprocalAt (state : PhysicalState) (index : Nat)
    (slot : PhysicalSlot) (symbol : NetSym) : Prop :=
  NetI64.arity symbol + 1 ≤ slot.ports.length ∧
  ∀ port target, slot.ports[port]? = some target →
    (NetI64.arity symbol < port → target = none) ∧
    (port ≤ NetI64.arity symbol →
      match target with
      | some (peer, peerPort) =>
          ∃ remote, state.slots[peer]? = some remote ∧
            remote.agent.isSome = true ∧
            remote.ports[peerPort]? = some (some (index, port)) ∧
            ∀ output ∈ state.outputs, output.target ≠ (index, port)
      | none =>
          (state.outputs.filter fun output => output.target == (index, port)).length = 1)

/-- `reciprocalPorts` holds exactly when every live slot satisfies the
per-slot proposition. -/
theorem PhysicalState.reciprocalPorts_iff {state : PhysicalState} :
    state.reciprocalPorts = true ↔
      ∀ index slot id symbol, state.slots[index]? = some slot →
        slot.agent = some (id, symbol) →
        state.PortsReciprocalAt index slot symbol := by
  unfold PhysicalState.reciprocalPorts
  rw [List.all_eq_true]
  constructor
  · intro h index slot id symbol hslot hagent
    have hentry := h index (List.mem_range.2 (Array.getElem?_eq_some_iff.1 hslot).1)
    rw [hslot] at hentry
    dsimp only at hentry
    rw [hagent] at hentry
    dsimp only at hentry
    rw [Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true] at hentry
    refine ⟨hentry.1, fun port target hget => ?_⟩
    have hport := hentry.2 (target, port) (List.mem_zipIdx_iff_getElem?.2 hget)
    dsimp only at hport
    by_cases hgt : NetI64.arity symbol < port
    · rw [ite_eq_left hgt] at hport
      exact ⟨fun _ => Option.isNone_iff_eq_none.1 hport,
        fun hle => absurd hgt (Nat.not_lt.2 hle)⟩
    · rw [ite_eq_right hgt] at hport
      refine ⟨fun hlt => absurd hlt hgt, fun _ => ?_⟩
      cases target with
      | none =>
        dsimp only at hport ⊢
        exact beq_iff_eq.1 hport
      | some pair =>
        obtain ⟨peer, peerPort⟩ := pair
        dsimp only at hport ⊢
        cases hremote : state.slots[peer]? with
        | none => simp [hremote] at hport
        | some remote =>
          rw [hremote] at hport
          dsimp only at hport
          simp only [Bool.and_eq_true, beq_iff_eq, Bool.not_eq_true', List.any_eq_false] at hport
          exact ⟨remote, rfl, hport.1.1, hport.1.2, hport.2⟩
  · intro h index hindex
    obtain ⟨slot, hslot⟩ : ∃ slot, state.slots[index]? = some slot :=
      ⟨_, Array.getElem?_eq_getElem (List.mem_range.1 hindex)⟩
    rw [hslot]
    dsimp only
    cases hagent : slot.agent with
    | none => rfl
    | some entry =>
      obtain ⟨id, symbol⟩ := entry
      obtain ⟨hlen, hports⟩ := h index slot id symbol hslot hagent
      dsimp only
      rw [Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true]
      refine ⟨hlen, fun entry hmem => ?_⟩
      obtain ⟨target, port⟩ := entry
      obtain ⟨hhigh, hlow⟩ := hports port target (List.mem_zipIdx_iff_getElem?.1 hmem)
      dsimp only
      by_cases hgt : NetI64.arity symbol < port
      · rw [ite_eq_left hgt]
        exact Option.isNone_iff_eq_none.2 (hhigh hgt)
      · rw [ite_eq_right hgt]
        have hwire := hlow (Nat.not_lt.1 hgt)
        cases target with
        | none =>
          dsimp only at hwire ⊢
          exact beq_iff_eq.2 hwire
        | some pair =>
          obtain ⟨peer, peerPort⟩ := pair
          dsimp only at hwire ⊢
          obtain ⟨remote, hremote, hlive, hback, hfree⟩ := hwire
          rw [hremote]
          dsimp only
          simp only [Bool.and_eq_true, beq_iff_eq, Bool.not_eq_true', List.any_eq_false]
          exact ⟨⟨hlive, hback⟩, hfree⟩

/-- A slot list mentions an identity in a live slot exactly when it is live. -/
private theorem any_live_eq_false {state : PhysicalState} {id : Id} :
    (state.slots.toList.any fun live => live.agent.map Prod.fst == some id) = false ↔
      id ∉ state.liveIds := by
  unfold PhysicalState.liveIds
  simp only [List.any_eq_false, beq_iff_eq, List.mem_filterMap, not_exists, not_and]

/-- `outputOwners` as a proposition over the ordered output table. -/
theorem PhysicalState.outputOwners_iff {state : PhysicalState} :
    state.outputOwners = true ↔
      (state.outputs.map PhysicalOutput.label).Nodup ∧
      (state.outputs.map PhysicalOutput.target).Nodup ∧
      ∀ output ∈ state.outputs, ∃ slot,
        state.slots[output.target.1]? = some slot ∧
        slot.agent.isSome = true ∧
        slot.ports[output.target.2]? = some none ∧
        output.label.1 ∉ state.liveIds := by
  unfold PhysicalState.outputOwners
  rw [Bool.and_eq_true, Bool.and_eq_true, decide_eq_true_eq, decide_eq_true_eq,
    List.all_eq_true, and_assoc]
  refine and_congr_right fun _ => and_congr_right fun _ => forall_congr' fun output =>
    imp_congr_right fun _ => ?_
  cases hslot : state.slots[output.target.1]? with
  | none => simp
  | some slot =>
    dsimp only
    rw [Bool.and_eq_true, Bool.and_eq_true, beq_iff_eq, Bool.not_eq_true', any_live_eq_false]
    constructor
    · rintro ⟨⟨hlive, hport⟩, hdead⟩
      exact ⟨slot, rfl, hlive, hport, hdead⟩
    · rintro ⟨other, hother, hlive, hport, hdead⟩
      cases hother
      exact ⟨⟨hlive, hport⟩, hdead⟩

/-- Structural legality separates every projected key space. -/
theorem PhysicalState.Legal.keysSeparated {state : PhysicalState}
    (h : state.Legal) : state.KeysSeparated := by
  obtain ⟨hlabels, htargets, howners⟩ := PhysicalState.outputOwners_iff.1 h.2.2.2.2.2.2.2
  refine ⟨h.2.2.2.2.2.1, hlabels, htargets, fun output hmem => ?_, fun output hmem => ?_⟩
  · obtain ⟨_, _, _, _, hdead⟩ := howners output hmem
    exact hdead
  · obtain ⟨slot, hslot, _, hport, _⟩ := howners output hmem
    exact ⟨slot, hslot, hport⟩

/-- Filtering the indexed output table by target and forgetting the indices
filters the output table itself. -/
private theorem filter_zipIdx_map_fst (outputs : List PhysicalOutput) (target : Nat × Nat) :
    (outputs.zipIdx.filter (fun entry => entry.1.target == target)).map Prod.fst =
      outputs.filter (fun output => output.target == target) := by
  have h := List.filter_map (f := Prod.fst)
    (p := fun output : PhysicalOutput => output.target == target) (l := outputs.zipIdx)
  rw [List.zipIdx_map_fst] at h
  exact h.symm

/-- Every live port at or below its symbol's arity is captured by the
physical auxiliary-capture reader, either as a reciprocal live peer whose
projected wire is the peer's logical port, or as the unique output whose
projected wire is its stable label. -/
theorem PhysicalState.captureAux_project {state : PhysicalState}
    (hLegal : state.Legal) {index : Nat} {slot : PhysicalSlot}
    {id : Id} {symbol : NetSym} {port : Nat}
    (hslot : state.slots[index]? = some slot)
    (hagent : slot.agent = some (id, symbol))
    (hport : port ≤ NetI64.arity symbol) :
    ∃ boundary, state.captureAux index port = some boundary ∧
      boundary.1 = (index, port) ∧
      ((∃ peer peerPort peerSlot peerId peerSymbol,
          boundary.2 = (0, peer, peerPort) ∧
          slot.ports[port]? = some (some (peer, peerPort)) ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (index, port)) ∧
          peerPort ≤ NetI64.arity peerSymbol ∧
          (∀ output ∈ state.outputs, output.target ≠ (index, port)) ∧
          (∀ output ∈ state.outputs, output.target ≠ (peer, peerPort)) ∧
          state.project.wires[(id, port)]? = some (peerId, peerPort) ∧
          state.project.wires[(peerId, peerPort)]? = some (id, port)) ∨
       (∃ outputIndex output,
          boundary.2 = (1, outputIndex, 0) ∧
          slot.ports[port]? = some none ∧
          state.outputs[outputIndex]? = some output ∧
          output.target = (index, port) ∧
          (∀ other ∈ state.outputs, other.target = (index, port) → other = output) ∧
          state.project.wires[(id, port)]? = some output.label ∧
          state.project.wires[output.label]? = some (id, port))) := by
  have hsep := hLegal.keysSeparated
  have hrec := PhysicalState.reciprocalPorts_iff.1 hLegal.2.2.2.2.2.2.1
  obtain ⟨hlen, hports⟩ := hrec index slot id symbol hslot hagent
  obtain ⟨target, htarget⟩ : ∃ target, slot.ports[port]? = some target :=
    ⟨_, List.getElem?_eq_getElem (by omega)⟩
  have hwire := (hports port target htarget).2 hport
  cases target with
  | none =>
    dsimp only at hwire
    have hlength : (state.outputs.zipIdx.filter
        (fun entry => entry.1.target == (index, port))).length = 1 := by
      rw [← List.length_map (f := Prod.fst), filter_zipIdx_map_fst]
      exact hwire
    obtain ⟨⟨output, outputIndex⟩, hentries⟩ := List.length_eq_one_iff.1 hlength
    have hentry : (output, outputIndex) ∈ state.outputs.zipIdx.filter
        (fun entry => entry.1.target == (index, port)) := by
      rw [hentries]
      exact List.mem_singleton_self _
    obtain ⟨hzip, htargetBeq⟩ := List.mem_filter.1 hentry
    have hout : state.outputs[outputIndex]? = some output :=
      List.mem_zipIdx_iff_getElem?.1 hzip
    have hmem : output ∈ state.outputs := List.mem_of_getElem? hout
    have htargetEq : output.target = (index, port) := beq_iff_eq.1 htargetBeq
    have hunique : ∀ other ∈ state.outputs, other.target = (index, port) → other = output := by
      intro other hother hotherTarget
      have hfilter : other ∈ state.outputs.filter
          (fun output => output.target == (index, port)) :=
        List.mem_filter.2 ⟨hother, beq_iff_eq.2 hotherTarget⟩
      rw [← filter_zipIdx_map_fst, hentries] at hfilter
      exact List.mem_singleton.1 hfilter
    have hend : state.logicalPort output.target = some (id, port) := by
      rw [htargetEq]
      exact logicalPort_of_slot (port := (index, port)) hslot hagent
    refine ⟨((index, port), (1, outputIndex, 0)), ?_, rfl,
      Or.inr ⟨outputIndex, output, rfl, htarget, hout, htargetEq, hunique,
        PhysicalState.wires_endpoint hsep hmem hend,
        (PhysicalState.wires_label hsep hmem).trans hend⟩⟩
    unfold PhysicalState.captureAux
    rw [hentries]
    simp [hslot, hagent, htarget]
  | some pair =>
    obtain ⟨peer, peerPort⟩ := pair
    dsimp only at hwire
    obtain ⟨remote, hremote, hlive, hback, hfree⟩ := hwire
    obtain ⟨⟨peerId, peerSymbol⟩, hpeerAgent⟩ := Option.isSome_iff_exists.1 hlive
    obtain ⟨_, hpeerPorts⟩ := hrec peer remote peerId peerSymbol hremote hpeerAgent
    obtain ⟨hhigh, hlow⟩ := hpeerPorts peerPort (some (index, port)) hback
    have hpeerPort : peerPort ≤ NetI64.arity peerSymbol :=
      Nat.not_lt.1 fun hgt => by cases hhigh hgt
    have hpeerWire := hlow hpeerPort
    dsimp only at hpeerWire
    obtain ⟨_, _, _, _, hpeerFree⟩ := hpeerWire
    have hany : (state.outputs.any fun output => output.target == (index, port)) = false :=
      List.any_eq_false.2 fun other hother hbeq => hfree other hother (beq_iff_eq.1 hbeq)
    refine ⟨((index, port), (0, peer, peerPort)), ?_, rfl,
      Or.inl ⟨peer, peerPort, remote, peerId, peerSymbol, rfl, htarget, hremote, hpeerAgent,
        hback, hpeerPort, hfree, hpeerFree, ?_, ?_⟩⟩
    · unfold PhysicalState.captureAux
      simp [hslot, hagent, htarget, hany, hremote, hpeerAgent, hback, Nat.not_lt.2 hpeerPort]
    · show state.wires[(id, port)]? = some (peerId, peerPort)
      rw [PhysicalState.wires_internal hsep hslot hagent hfree, htarget]
      exact logicalPort_of_slot (port := (peer, peerPort)) hremote hpeerAgent
    · show state.wires[(peerId, peerPort)]? = some (id, port)
      rw [PhysicalState.wires_internal hsep hremote hpeerAgent hpeerFree, hback]
      exact logicalPort_of_slot (port := (index, port)) hslot hagent

/-- A decoded current pair names two live slots with the queued generations. -/
private theorem logicalPair_eq_some {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    (h : state.logicalPair pair = some (a, b)) :
    ∃ first second α β, state.slots[pair.first]? = some first ∧
      state.slots[pair.second]? = some second ∧
      first.generation = pair.firstGeneration ∧ second.generation = pair.secondGeneration ∧
      first.agent = some (a, α) ∧ second.agent = some (b, β) := by
  unfold PhysicalState.logicalPair at h
  cases hfirst : state.slots[pair.first]? with
  | none => simp [hfirst] at h
  | some first =>
    cases hsecond : state.slots[pair.second]? with
    | none => simp [hfirst, hsecond] at h
    | some second =>
      by_cases hg1 : first.generation = pair.firstGeneration
      · by_cases hg2 : second.generation = pair.secondGeneration
        · cases ha : first.agent with
          | none => simp [hfirst, hsecond, hg1, hg2, ha] at h
          | some left =>
            cases hb : second.agent with
            | none => simp [hfirst, hsecond, hg2, hb] at h
            | some right =>
              obtain ⟨l, α⟩ := left
              obtain ⟨r, β⟩ := right
              simp [hfirst, hsecond, hg1, hg2, ha, hb] at h
              obtain ⟨rfl, rfl⟩ := h
              exact ⟨first, second, α, β, rfl, rfl, hg1, hg2, ha, hb⟩
        · simp [hfirst, hsecond, hg2] at h
      · simp [hfirst, hsecond, hg1] at h

/-- A port whose projected wire reaches a live identity is not output-owned. -/
private theorem not_output_target_of_live_wire {state : PhysicalState}
    (hsep : state.KeysSeparated) {index : Nat} {slot : PhysicalSlot} {id : Id}
    {symbol : NetSym} {port : Nat} {endpoint : Port}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol))
    (hlive : endpoint.1 ∈ state.liveIds)
    (hwire : state.wires[(id, port)]? = some endpoint) :
    ∀ output ∈ state.outputs, output.target ≠ (index, port) := by
  intro output hmem htarget
  have hend : state.logicalPort output.target = some (id, port) := by
    rw [htarget]
    exact logicalPort_of_slot (port := (index, port)) hslot hagent
  rw [PhysicalState.wires_endpoint hsep hmem hend] at hwire
  have hlabel : output.label = endpoint := Option.some.inj hwire
  exact hsep.labelDead output hmem (by rw [hlabel]; exact hlive)

/-- A port with no output owner whose projected wire reaches a live logical
port stores the physical reference of that port. -/
private theorem port_of_live_wire {state : PhysicalState}
    (hsep : state.KeysSeparated) {index other : Nat} {slot otherSlot : PhysicalSlot}
    {id otherId : Id} {symbol otherSymbol : NetSym} {port otherPort : Nat}
    (hslot : state.slots[index]? = some slot) (hagent : slot.agent = some (id, symbol))
    (hother : state.slots[other]? = some otherSlot)
    (hotherAgent : otherSlot.agent = some (otherId, otherSymbol))
    (hfree : ∀ output ∈ state.outputs, output.target ≠ (index, port))
    (hwire : state.wires[(id, port)]? = some (otherId, otherPort)) :
    slot.ports[port]? = some (some (other, otherPort)) := by
  rw [PhysicalState.wires_internal hsep hslot hagent hfree] at hwire
  cases hp : slot.ports[port]? with
  | none =>
    rw [hp] at hwire
    cases hwire
  | some target =>
    rw [hp] at hwire
    cases target with
    | none => cases hwire
    | some target =>
      rw [logicalPort_injective hsep.unique
        (show state.logicalPort target = some (otherId, otherPort) from hwire)
        (logicalPort_of_slot (port := (other, otherPort)) hother hotherAgent)]

/-- A matching projection wires the two principal ports of a redex directly,
and legality turns that into the physical reciprocal principal pair. -/
theorem PhysicalState.redex_physical_pair {state : PhysicalState}
    {pair : PhysicalPair} {a b : Id} {α β : NetSym} {R : Rhs NetSym}
    (hLegal : state.Legal)
    (hpair : state.logicalPair pair = some (a, b))
    (hm : state.project.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system state.project.toNet a b α β R) :
    ∃ first second, state.slots[pair.first]? = some first ∧
      state.slots[pair.second]? = some second ∧
      first.agent = some (a, α) ∧ second.agent = some (b, β) ∧
      first.generation = pair.firstGeneration ∧
      second.generation = pair.secondGeneration ∧
      first.ports[0]? = some (some (pair.second, 0)) ∧
      second.ports[0]? = some (some (pair.first, 0)) ∧
      pair.first ≠ pair.second ∧
      state.project.wires[(a, 0)]? = some (b, 0) ∧
      state.project.wires[(b, 0)]? = some (a, 0) ∧
      (∀ output ∈ state.outputs, output.target ≠ (pair.first, 0)) ∧
      (∀ output ∈ state.outputs, output.target ≠ (pair.second, 0)) := by
  have hsep := hLegal.keysSeparated
  obtain ⟨first, second, α', β', hfirst, hsecond, hfirstGen, hsecondGen, hfirstAgent,
    hsecondAgent⟩ := logicalPair_eq_some hpair
  have hα : α' = α := by
    have hlookup : state.agents[a]? = some α := hredex.ha
    rw [PhysicalState.agents_getElem?_of_slot hsep.unique hfirst hfirstAgent] at hlookup
    exact Option.some.inj hlookup
  have hβ : β' = β := by
    have hlookup : state.agents[b]? = some β := hredex.hb
    rw [PhysicalState.agents_getElem?_of_slot hsep.unique hsecond hsecondAgent] at hlookup
    exact Option.some.inj hlookup
  subst hα hβ
  have hpaired : state.project.toNet.End NetI64.arity (a, 0) ∧
      state.project.toNet.End NetI64.arity (b, 0) ∧
      state.project.toNet.Conn NetI64.arity (a, 0) (b, 0) :=
    ⟨Or.inr ⟨α', hredex.ha, Nat.zero_le _⟩, Or.inr ⟨β', hredex.hb, Nat.zero_le _⟩,
      hredex.conn⟩
  have hlink : state.wires[(a, 0)]? = some (b, 0) := hm.paired_iff.1 hpaired
  have hback : state.wires[(b, 0)]? = some (a, 0) := hm.symm (p := (a, 0)) (q := (b, 0)) hlink
  have haLive : a ∈ state.liveIds :=
    PhysicalState.mem_liveIds.2 ⟨pair.first, first, α', hfirst, hfirstAgent⟩
  have hbLive : b ∈ state.liveIds :=
    PhysicalState.mem_liveIds.2 ⟨pair.second, second, β', hsecond, hsecondAgent⟩
  have hfreeFirst := not_output_target_of_live_wire hsep hfirst hfirstAgent hbLive hlink
  have hfreeSecond := not_output_target_of_live_wire hsep hsecond hsecondAgent haLive hback
  have hne : pair.first ≠ pair.second := by
    intro heq
    rw [heq, hsecond] at hfirst
    rw [← Option.some.inj hfirst, hsecondAgent] at hfirstAgent
    exact hredex.ne (Prod.mk.inj (Option.some.inj hfirstAgent)).1.symm
  exact ⟨first, second, hfirst, hsecond, hfirstAgent, hsecondAgent, hfirstGen, hsecondGen,
    port_of_live_wire hsep hfirst hfirstAgent hsecond hsecondAgent hfreeFirst hlink,
    port_of_live_wire hsep hsecond hsecondAgent hfirst hfirstAgent hfreeSecond hback,
    hne, hlink, hback, hfreeFirst, hfreeSecond⟩

/-- In a matching projection of a legal arena, every output targets a port
at or below the arity of a live slot. -/
theorem PhysicalState.output_target_in_arity {state : PhysicalState}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    {output : PhysicalOutput} (hmem : output ∈ state.outputs) :
    ∃ slot id symbol, state.slots[output.target.1]? = some slot ∧
      slot.agent = some (id, symbol) ∧
      output.target.2 ≤ NetI64.arity symbol := by
  have hsep := hLegal.keysSeparated
  obtain ⟨_, _, howners⟩ := PhysicalState.outputOwners_iff.1 hLegal.2.2.2.2.2.2.2
  obtain ⟨slot, hslot, hlive, _, _⟩ := howners output hmem
  obtain ⟨⟨id, symbol⟩, hagent⟩ := Option.isSome_iff_exists.1 hlive
  refine ⟨slot, id, symbol, hslot, hagent, ?_⟩
  have hwire := PhysicalState.wires_endpoint hsep hmem (logicalPort_of_slot hslot hagent)
  rcases hm.ends (p := (id, output.target.2)) (q := output.label) hwire with
    hfree | ⟨γ, hγ, hle⟩
  · obtain ⟨other, hother, hlabel⟩ := List.mem_map.1 hfree
    exact (hsep.labelDead other hother (by
      rw [hlabel]
      exact PhysicalState.mem_liveIds.2 ⟨_, _, _, hslot, hagent⟩)).elim
  · have hlookup : state.agents[id]? = some γ := hγ
    rw [PhysicalState.agents_getElem?_of_slot hsep.unique hslot hagent] at hlookup
    cases hlookup
    exact hle

/-- Names extending a live agent's identity are unused: no live slot and no
output label carries them. This is the matching projection's freshness. -/
theorem PhysicalState.extension_unused {state : PhysicalState}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    {index : Nat} {slot : PhysicalSlot} {id : Id} {symbol : NetSym}
    (hslot : state.slots[index]? = some slot)
    (hagent : slot.agent = some (id, symbol)) (k : Nat) :
    (k :: id) ∉ state.liveIds ∧
      ∀ output ∈ state.outputs, output.label.1 ≠ k :: id := by
  have hunique : state.liveIds.Nodup := hLegal.2.2.2.2.2.1
  have hfresh : ¬ state.project.toNet.Used (k :: id) := fun hused =>
    List.cons_ne_nil k [] (hm.fresh [k]
      (PhysicalState.agents_getElem?_of_slot hunique hslot hagent) hused)
  refine ⟨fun hlive => ?_, fun output hmem heq => ?_⟩
  · obtain ⟨_, _, other, hother, hotherAgent⟩ := PhysicalState.mem_liveIds.1 hlive
    exact hfresh (Or.inl ⟨other, PhysicalState.agents_getElem?_of_slot hunique hother hotherAgent⟩)
  · exact hfresh (Or.inr (Or.inr ⟨output.label.2, by
      rw [← heq]
      exact List.mem_map_of_mem hmem⟩))

end NobleFoundation.NetI64Plans
