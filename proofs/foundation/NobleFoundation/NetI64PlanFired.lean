import NobleFoundation.NetI64Plans

/-!
# Symbolic abstract images of the eight selected rule orientations (issue #110)

For an arbitrary hash-map net whose selected principal ports are wired to
each other and whose rule-agent auxiliary ports are wired to endpoints outside
the consumed pair, the independently compiled `Plan` has an explicit captured
boundary, result-agent list and wire-write list. The endpoints stay symbolic;
no finite net is enumerated and no `decide` runs on symbolic hash maps. The
commit lookups then read the abstract post-net one key at a time.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- Forward OP2: one `op1` child wired to both captured auxiliary endpoints. -/
theorem Plan.op2_forward_fields (H : HNet NetSym) {a b : Id}
    {op : Op} {value : BitVec 64} {p1 p2 : Port}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0))
    (h1 : H.wires[(a, 1)]? = some p1) (h2 : H.wires[(a, 2)]? = some p2)
    (hp1 : p1.1 ≠ a ∧ p1.1 ≠ b) (hp2 : p2.1 ≠ a ∧ p2.1 ≠ b) :
    let selected := Plan.compile H (a, b, .op2 op, .num value,
      (op2NumData op value).toX) (by rfl)
    let c := child a b (some false) 0
    selected.capturedBoundary =
      [((a, 0), (b, 0)), ((a, 1), p1), ((a, 2), p2), ((b, 0), (a, 0))] ∧
    selected.initializedAgents = [(c, .op1 op value)] ∧
    selected.wireWrites = [((c, 0), p1), ((c, 1), p2), (p1, (c, 0)), (p2, (c, 1))] := by
  obtain ⟨i1, k1⟩ := p1
  obtain ⟨i2, k2⟩ := p2
  obtain ⟨hp1a, hp1b⟩ := hp1
  obtain ⟨hp2a, hp2b⟩ := hp2
  simp only at hp1a hp1b hp2a hp2b
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb, h1, h2, List.range_succ]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, op2NumData, RuleData.ofWires,
      RuleData.toX, assoc]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, h1, h2, fireTable, ifcPorts, XRhs.wiresFrom, op2NumData,
      RuleData.ofWires, RuleData.toX, retireTable, spliceAll, assoc, emb, LPort.isNew,
      XRhs.newPorts, List.range_succ, hp1a, hp1b, hp2a, hp2b, Ne.symm hp1a, Ne.symm hp2a]

/-- Reverse OP2: the `op2` agent is the second selected identity. -/
theorem Plan.op2_reverse_fields (H : HNet NetSym) {a b : Id}
    {op : Op} {value : BitVec 64} {p1 p2 : Port}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0))
    (h1 : H.wires[(b, 1)]? = some p1) (h2 : H.wires[(b, 2)]? = some p2)
    (hp1 : p1.1 ≠ a ∧ p1.1 ≠ b) (hp2 : p2.1 ≠ a ∧ p2.1 ≠ b) :
    let selected := Plan.compile H (a, b, .num value, .op2 op,
      (op2NumData op value).toX.swap) (by rfl)
    let c := child a b (some true) 0
    selected.capturedBoundary =
      [((a, 0), (b, 0)), ((b, 0), (a, 0)), ((b, 1), p1), ((b, 2), p2)] ∧
    selected.initializedAgents = [(c, .op1 op value)] ∧
    selected.wireWrites = [((c, 0), p1), ((c, 1), p2), (p1, (c, 0)), (p2, (c, 1))] := by
  obtain ⟨i1, k1⟩ := p1
  obtain ⟨i2, k2⟩ := p2
  obtain ⟨hp1a, hp1b⟩ := hp1
  obtain ⟨hp2a, hp2b⟩ := hp2
  simp only at hp1a hp1b hp2a hp2b
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb, h1, h2, List.range_succ]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, op2NumData, RuleData.ofWires,
      RuleData.toX, XRhs.swap, assoc]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, h1, h2, fireTable, ifcPorts, XRhs.wiresFrom, op2NumData,
      RuleData.ofWires, RuleData.toX, XRhs.swap, LPort.swap, retireTable, spliceAll, assoc, emb,
      LPort.isNew, XRhs.newPorts, List.range_succ, hp1a, hp1b, hp2a, hp2b, Ne.symm hp1b,
      Ne.symm hp2b]

/-- Forward OP1: one number child wired to the single captured endpoint. -/
theorem Plan.op1_forward_fields (H : HNet NetSym) {a b : Id}
    {op : Op} {first second : BitVec 64} {p1 : Port}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0))
    (h1 : H.wires[(a, 1)]? = some p1)
    (hp1 : p1.1 ≠ a ∧ p1.1 ≠ b) :
    let selected := Plan.compile H (a, b, .op1 op first, .num second,
      (op1NumData op first second).toX) (by rfl)
    let c := child a b (some false) 0
    selected.capturedBoundary =
      [((a, 0), (b, 0)), ((a, 1), p1), ((b, 0), (a, 0))] ∧
    selected.initializedAgents = [(c, .num (op.apply first second))] ∧
    selected.wireWrites = [((c, 0), p1), (p1, (c, 0))] := by
  obtain ⟨i1, k1⟩ := p1
  obtain ⟨hp1a, hp1b⟩ := hp1
  simp only at hp1a hp1b
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb, h1, List.range_succ]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, op1NumData, RuleData.ofWires,
      RuleData.toX, assoc]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, h1, fireTable, ifcPorts, XRhs.wiresFrom, op1NumData,
      RuleData.ofWires, RuleData.toX, retireTable, spliceAll, assoc, emb, LPort.isNew,
      XRhs.newPorts, List.range_succ, hp1a, hp1b, Ne.symm hp1a]

/-- Reverse OP1: the `op1` agent is the second selected identity. -/
theorem Plan.op1_reverse_fields (H : HNet NetSym) {a b : Id}
    {op : Op} {first second : BitVec 64} {p1 : Port}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0))
    (h1 : H.wires[(b, 1)]? = some p1)
    (hp1 : p1.1 ≠ a ∧ p1.1 ≠ b) :
    let selected := Plan.compile H (a, b, .num second, .op1 op first,
      (op1NumData op first second).toX.swap) (by rfl)
    let c := child a b (some true) 0
    selected.capturedBoundary =
      [((a, 0), (b, 0)), ((b, 0), (a, 0)), ((b, 1), p1)] ∧
    selected.initializedAgents = [(c, .num (op.apply first second))] ∧
    selected.wireWrites = [((c, 0), p1), (p1, (c, 0))] := by
  obtain ⟨i1, k1⟩ := p1
  obtain ⟨hp1a, hp1b⟩ := hp1
  simp only at hp1a hp1b
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb, h1, List.range_succ]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, op1NumData, RuleData.ofWires,
      RuleData.toX, XRhs.swap, assoc]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, h1, fireTable, ifcPorts, XRhs.wiresFrom, op1NumData,
      RuleData.ofWires, RuleData.toX, XRhs.swap, LPort.swap, retireTable, spliceAll, assoc, emb,
      LPort.isNew, XRhs.newPorts, List.range_succ, hp1a, hp1b, Ne.symm hp1b]

/-- Forward DUP: two number children, one per captured endpoint. -/
theorem Plan.dup_forward_fields (H : HNet NetSym) {a b : Id}
    {value : BitVec 64} {p1 p2 : Port}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0))
    (h1 : H.wires[(a, 1)]? = some p1) (h2 : H.wires[(a, 2)]? = some p2)
    (hp1 : p1.1 ≠ a ∧ p1.1 ≠ b) (hp2 : p2.1 ≠ a ∧ p2.1 ≠ b) :
    let selected := Plan.compile H (a, b, .dup, .num value,
      (dupNumData value).toX) (by rfl)
    let c0 := child a b (some false) 0
    let c1 := child a b (some false) 1
    selected.capturedBoundary =
      [((a, 0), (b, 0)), ((a, 1), p1), ((a, 2), p2), ((b, 0), (a, 0))] ∧
    selected.initializedAgents = [(c0, .num value), (c1, .num value)] ∧
    selected.wireWrites = [((c0, 0), p1), ((c1, 0), p2), (p1, (c0, 0)), (p2, (c1, 0))] := by
  obtain ⟨i1, k1⟩ := p1
  obtain ⟨i2, k2⟩ := p2
  obtain ⟨hp1a, hp1b⟩ := hp1
  obtain ⟨hp2a, hp2b⟩ := hp2
  simp only at hp1a hp1b hp2a hp2b
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb, h1, h2, List.range_succ]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, dupNumData, RuleData.ofWires,
      RuleData.toX, assoc]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, h1, h2, fireTable, ifcPorts, XRhs.wiresFrom, dupNumData,
      RuleData.ofWires, RuleData.toX, retireTable, spliceAll, assoc, emb, LPort.isNew,
      XRhs.newPorts, List.range_succ, hp1a, hp1b, hp2a, hp2b, Ne.symm hp1a, Ne.symm hp2a]

/-- Reverse DUP: the duplicator is the second selected identity. -/
theorem Plan.dup_reverse_fields (H : HNet NetSym) {a b : Id}
    {value : BitVec 64} {p1 p2 : Port}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0))
    (h1 : H.wires[(b, 1)]? = some p1) (h2 : H.wires[(b, 2)]? = some p2)
    (hp1 : p1.1 ≠ a ∧ p1.1 ≠ b) (hp2 : p2.1 ≠ a ∧ p2.1 ≠ b) :
    let selected := Plan.compile H (a, b, .num value, .dup,
      (dupNumData value).toX.swap) (by rfl)
    let c0 := child a b (some true) 0
    let c1 := child a b (some true) 1
    selected.capturedBoundary =
      [((a, 0), (b, 0)), ((b, 0), (a, 0)), ((b, 1), p1), ((b, 2), p2)] ∧
    selected.initializedAgents = [(c0, .num value), (c1, .num value)] ∧
    selected.wireWrites = [((c0, 0), p1), ((c1, 0), p2), (p1, (c0, 0)), (p2, (c1, 0))] := by
  obtain ⟨i1, k1⟩ := p1
  obtain ⟨i2, k2⟩ := p2
  obtain ⟨hp1a, hp1b⟩ := hp1
  obtain ⟨hp2a, hp2b⟩ := hp2
  simp only at hp1a hp1b hp2a hp2b
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb, h1, h2, List.range_succ]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, dupNumData, RuleData.ofWires,
      RuleData.toX, XRhs.swap, assoc]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, h1, h2, fireTable, ifcPorts, XRhs.wiresFrom, dupNumData,
      RuleData.ofWires, RuleData.toX, XRhs.swap, LPort.swap, retireTable, spliceAll, assoc, emb,
      LPort.isNew, XRhs.newPorts, List.range_succ, hp1a, hp1b, hp2a, hp2b, Ne.symm hp1b,
      Ne.symm hp2b]

/-- Forward ERA: no children, no writes, only the principal wire captured. -/
theorem Plan.era_forward_fields (H : HNet NetSym) {a b : Id}
    {value : BitVec 64}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0)) :
    let selected := Plan.compile H (a, b, .era, .num value, eraNumData.toX) (by rfl)
    selected.capturedBoundary = [((a, 0), (b, 0)), ((b, 0), (a, 0))] ∧
    selected.initializedAgents = [] ∧
    selected.wireWrites = [] := by
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, eraNumData, RuleData.ofWires,
      RuleData.toX]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, fireTable, ifcPorts, XRhs.wiresFrom, eraNumData,
      RuleData.ofWires, RuleData.toX, retireTable, spliceAll, XRhs.newPorts]

/-- Reverse ERA: the eraser is the second selected identity. -/
theorem Plan.era_reverse_fields (H : HNet NetSym) {a b : Id}
    {value : BitVec 64}
    (h0 : H.wires[(a, 0)]? = some (b, 0)) (hb : H.wires[(b, 0)]? = some (a, 0)) :
    let selected := Plan.compile H (a, b, .num value, .era, eraNumData.toX.swap) (by rfl)
    selected.capturedBoundary = [((a, 0), (b, 0)), ((b, 0), (a, 0))] ∧
    selected.initializedAgents = [] ∧
    selected.wireWrites = [] := by
  refine ⟨?_, ?_, ?_⟩
  · simp [Plan.compile, HNet.nearWires, portsOf, NetI64.arity, h0, hb]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, eraNumData, RuleData.ofWires,
      RuleData.toX, XRhs.swap]
  · simp [Plan.compile, localResult, XNet.fire, HNet.nbhd, HNet.nearWires, portsOf,
      NetI64.arity, h0, hb, fireTable, ifcPorts, XRhs.wiresFrom, eraNumData,
      RuleData.ofWires, RuleData.toX, XRhs.swap, retireTable, spliceAll, XRhs.newPorts]

/-- With pairwise distinct keys, a sequence of map inserts reads back the listed
value of a key, and otherwise the base map. -/
private theorem foldl_insert_getElem? {K V : Type} [BEq K] [LawfulBEq K] [Hashable K]
    [DecidableEq K] (entries : List (K × V)) (base : Std.HashMap K V) (key : K)
    (hdistinct : (entries.map Prod.fst).Nodup) :
    (entries.foldl (fun m entry => m.insert entry.1 entry.2) base)[key]? =
      (assoc key entries).or base[key]? := by
  induction entries generalizing base with
  | nil => rfl
  | cons entry rest ih =>
    obtain ⟨k, v⟩ := entry
    simp only [List.map_cons, List.nodup_cons] at hdistinct
    simp only [List.foldl_cons]
    rw [ih (base := base.insert k v) hdistinct.2, hget_insert]
    by_cases hk : k = key
    · subst hk
      have hnone : assoc k rest = none := by
        cases h : assoc k rest with
        | none => rfl
        | some q => exact absurd (List.mem_map.2 ⟨(k, q), assoc_mem h, rfl⟩) hdistinct.1
      simp [assoc, hnone]
    · simp [assoc, hk]

/-- A sequence of map erases removes exactly the listed keys. -/
private theorem foldl_erase_getElem? {K V : Type} [BEq K] [LawfulBEq K] [Hashable K]
    [DecidableEq K] (entries : List (K × V)) (base : Std.HashMap K V) (key : K) :
    (entries.foldl (fun m entry => m.erase entry.1) base)[key]? =
      if key ∈ entries.map Prod.fst then none else base[key]? := by
  induction entries generalizing base with
  | nil => simp
  | cons entry rest ih =>
    simp only [List.foldl_cons]
    rw [ih (base := base.erase entry.1), hget_erase]
    by_cases hk : entry.1 = key
    · simp [hk]
    · simp only [hk, Ne.symm hk, List.map_cons, List.mem_cons, false_or, ↓reduceIte]

/-- One abstract committed agent lookup: a fresh child if the plan initializes
it, otherwise the prestate agent unless it is one of the consumed pair. -/
theorem Plan.commit_agents_getElem? (H : HNet NetSym) (p : Plan)
    (rest : List (Id × Id)) (id : Id)
    (hdistinct : (p.initializedAgents.map Prod.fst).Nodup) :
    (Plan.commit H p rest).agents[id]? =
      (assoc id p.initializedAgents).or
        (if id = p.redex.1 ∨ id = p.redex.2.1 then none else H.agents[id]?) := by
  change (p.initializedAgents.foldl (fun m entry => m.insert entry.1 entry.2)
    ((H.agents.erase p.redex.1).erase p.redex.2.1))[id]? = _
  rw [foldl_insert_getElem? _ _ _ hdistinct, hget_erase, hget_erase]
  by_cases hleft : id = p.redex.1
  · simp [hleft]
  · by_cases hright : id = p.redex.2.1
    · simp [hright]
    · simp [hleft, hright, Ne.symm hleft, Ne.symm hright]

/-- One abstract committed wire lookup: a declared write if present, otherwise
nothing at either end of a captured wire, otherwise the prestate wire. -/
theorem Plan.commit_wires_getElem? (H : HNet NetSym) (p : Plan)
    (rest : List (Id × Id)) (port : Port)
    (hdistinct : (p.wireWrites.map Prod.fst).Nodup) :
    (Plan.commit H p rest).wires[port]? =
      (assoc port p.wireWrites).or
        (if port ∈ p.capturedBoundary.map Prod.fst ∨
            port ∈ p.capturedBoundary.map Prod.snd
          then none else H.wires[port]?) := by
  change (p.wireWrites.foldl (fun m entry => m.insert entry.1 entry.2)
    ((p.capturedBoundary ++ p.capturedBoundary.map (fun edge => (edge.2, edge.1))).foldl
      (fun m entry => m.erase entry.1) H.wires))[port]? = _
  rw [foldl_insert_getElem? _ _ _ hdistinct, foldl_erase_getElem?]
  have hmem : port ∈ (p.capturedBoundary ++
        p.capturedBoundary.map (fun edge => (edge.2, edge.1))).map Prod.fst ↔
      port ∈ p.capturedBoundary.map Prod.fst ∨ port ∈ p.capturedBoundary.map Prod.snd := by
    simp only [List.map_append, List.map_map, List.mem_append]
    exact Iff.rfl
  by_cases hcaptured : port ∈ p.capturedBoundary.map Prod.fst ∨
      port ∈ p.capturedBoundary.map Prod.snd
  · rw [ite_eq_left (hmem.2 hcaptured), ite_eq_left hcaptured]
  · rw [ite_eq_right (fun h => hcaptured (hmem.1 h)), ite_eq_right hcaptured]

end NobleFoundation.NetI64Plans
