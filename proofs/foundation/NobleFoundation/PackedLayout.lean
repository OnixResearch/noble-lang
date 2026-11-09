/-
Model of the declared packed graph layout and its checker obligations (issue #43).

`crates/noble-ngraph/src/layout.rs` emits a `packed_layout()` declaration — a revision, a
declared register count, one declared register per node, the constant and interface-slot
tables, one instruction range per layer, the failure-suffix prefix minima and a layout build
key — and validates it against the graph the checker produced. This file models the *declared
tables and the obligations*, not the Rust bytes, and proves:

* the declared register count is one register per node plus the constant-one register, the
  last, which no node declares (issue #104): an inflated count, a count without that register
  and a node holding it are each refused (`count_inflated_refused`,
  `count_without_one_register_refused`, `one_register_held_refused`), the compiler's count is
  accepted (`count_reserving_one_is_accepted`), and an accepted layout keeps every node's
  register below the reserved one (`accepted_reserves_one_register`);
* a missing register, an out-of-range register and a shared register are each refused
  (`missing_register_refused`, `out_of_range_refused`, `alias_refused`);
* the constant table must have one entry per constant node (`constant_count_refused`), no entry
  twice (`constant_duplicate_refused`, issue #94), and an entry that names no node is refused
  (`constant_entry_refused`); the Boolean table check is tied to the readable obligation by
  `constant_predicate_iff` and `slot_predicate_iff`;
* the layer table must have one ordered range per layer (`layer_shape_refused`,
  `layers_unordered_refused`), and its ranges must tile the instruction stream: each starts where
  its predecessor ends and holds exactly its layer's instruction nodes (`layer_range_refused`,
  issue #95), and the last ends at the instruction count (`coverage_refused`);
* acceptance implies a declared register per node, non-aliasing, distinct constant entries and
  coverage (`accepted_implies_registers`, `accepted_implies_distinct`,
  `accepted_implies_constants_distinct`, `accepted_implies_coverage`);
* a declared failure-suffix table that differs from the graph's own prefix minima is
  refused (`fail_suffix_mismatch_refused`).
* the layout `Compiled::new` emits for `PackedSchedule.exampleGraph`'s Rust counterpart,
  copied from the Rust declaration that `layout::check` accepts, is accepted here too
  (`exampleLayout_accepted`), so the obligations are satisfiable by a real layout rather than
  only refusing.

What this is not: no decoding of the Rust declaration's bytes is modelled, so nothing here
binds `serde`/binary output to these tables; the failure-suffix table is modelled as a
declared list compared against the graph's own prefix minima, and no statement covers the
layout build key, the compiler revision or the identity scheme.

The node vocabulary here is the pure-`I64` fragment (`input`, `constant` and the four
integer instructions), so the modelled slot table is the one for **input** slots. The
`ngraph-f32/0` profile has a second slot kind — `Op::Param` — and the Rust validator covers
both kinds in one table (issue #68); this file models neither the parameter nodes nor the
F32 instructions, so it says nothing about parameter layouts.
-/
import Init.Data.List.Lemmas
import NobleFoundation.PackedSchedule

namespace NobleFoundation.PackedLayout

open NobleFoundation.PackedSchedule

/-! ### The declared tables -/

/-- The layout declaration revision this model accepts. -/
def revision : String := "ngraph-layout/0"

/-- The largest register index the declaration may name. -/
def maxRegister : Nat := 2 ^ 32 - 1

/-- A declared packed layout. Registers are `Option`-valued: a node declared without a
register is refused by the fragment checker, which has no guard nodes. -/
structure Layout where
  revision : String
  regs : Nat
  reg : Nat → Option Nat
  consts : List (Nat × BitVec 64)
  slotRegs : List (Nat × Nat)
  layers : List (Nat × Nat)
  failSuffixMin : List Nat

/-- The register a node declares, if it declares one inside the count. -/
def declares (layout : Layout) (index : Nat) : Bool :=
  match layout.reg index with
  | some register => register < layout.regs
  | none => false

/-- Two distinct nodes never share a register. -/
def distinctRegisters (graph : Graph) (layout : Layout) : Bool :=
  (List.range graph.arity).all fun first =>
    (List.range graph.arity).all fun second =>
      first = second ||
        match layout.reg first, layout.reg second with
        | some left, some right => left != right
        | _, _ => true

/-- The declared count is one register per node plus the constant-one register, which is the
last and which no node declares. This is the Rust `layout::check` rule for the
`regs=dense;one-reg=last` layout options (issue #104); the fragment has no guard nodes, so every
node holds a register. With distinct registers below the count, it leaves the nodes exactly the
registers below the last. -/
def countReservesOne (graph : Graph) (layout : Layout) : Bool :=
  decide (layout.regs = graph.arity + 1) &&
    (List.range graph.arity).all fun index => layout.reg index != some graph.arity

/-- How many nodes declare the given register. -/
def registerCount (graph : Graph) (layout : Layout) (register : Nat) : Nat :=
  ((List.range graph.arity).filter fun index =>
    layout.reg index = some register).length

/-- A constant table entry names exactly one node whose constant bits are exactly the
declared bits and whose register is the declared register. -/
def constantEntryOk (graph : Graph) (layout : Layout) (entry : Nat × BitVec 64) : Bool :=
  decide (((List.range graph.arity).filter fun index =>
    match graph.node index, layout.reg index with
    | .constant bits, some register => register = entry.1 ∧ bits = entry.2
    | _, _ => false).length = 1)

/-- The constant table has one entry per constant node. -/
def constantCountOk (graph : Graph) (layout : Layout) : Bool :=
  let constants := ((List.range graph.arity).filter fun index =>
    match graph.node index with
    | .constant _ => true
    | _ => false).length
  decide (layout.consts.length = constants)

/-- The constant table names each entry at most once. An entry repeated in place of another
constant's entry keeps the count and every entry individually valid while leaving that other
constant undeclared (issue #94). -/
def constantsDistinctOk (layout : Layout) : Bool :=
  decide layout.consts.Nodup

/-- A slot table entry names exactly one interface input node of that slot. -/
def slotEntryOk (graph : Graph) (layout : Layout) (entry : Nat × Nat) : Bool :=
  decide (((List.range graph.arity).filter fun index =>
    match graph.node index, layout.reg index with
    | .input slot, some register => register = entry.1 ∧ slot = entry.2
    | _, _ => false).length = 1)

/-- The layer table declares one range per layer. -/
def layerShapeOk (layerCount : Nat) (layout : Layout) : Bool :=
  decide (layout.layers.length = layerCount)

/-- Each layer range is ordered inside itself and against its predecessor. -/
def layerOrderList : List (Nat × Nat) → Bool
  | [] => true
  | [range] => decide (range.1 ≤ range.2)
  | first :: second :: rest =>
      decide (first.1 ≤ first.2 ∧ first.2 ≤ second.1) && layerOrderList (second :: rest)

/-- Each layer range is ordered inside itself and against its predecessor. -/
def layerOrderOk (layout : Layout) : Bool :=
  layerOrderList layout.layers

/-- Whether a node produces an instruction: inputs and constants are table entries. -/
def instructionNode : Node → Bool
  | .input _ => false
  | .constant _ => false
  | _ => true

/-- The instruction nodes the checker assigns to one layer. -/
def layerInstructions (graph : Graph) (layerOf : Nat → Nat) (layer : Nat) : Nat :=
  ((List.range graph.arity).filter fun index =>
    instructionNode (graph.node index) && decide (layerOf index = layer)).length

/-- From a running offset, the declared ranges tile the instruction stream layer by layer: each
range starts where its predecessor ends and holds exactly its layer's instruction nodes. -/
def tiles (graph : Graph) (layerOf : Nat → Nat) : Nat → Nat → List (Nat × Nat) → Bool
  | _, _, [] => true
  | offset, layer, range :: rest =>
      decide (range.1 = offset ∧ range.2 = offset + layerInstructions graph layerOf layer) &&
        tiles graph layerOf range.2 (layer + 1) rest

/-- Every node that produces an instruction is covered exactly once: the ranges tile the stream
layer by layer (issue #95), and the last range ends at the instruction count, so no instruction
node lies outside the declared layers. -/
def coverageOk (graph : Graph) (layerOf : Nat → Nat) (layout : Layout) : Bool :=
  let instructions := ((List.range graph.arity).filter fun index =>
    instructionNode (graph.node index)).length
  tiles graph layerOf 0 0 layout.layers &&
    match layout.layers.reverse with
    | [] => decide (instructions = 0)
    | last :: _ => decide (last.2 = instructions)

/-- The graph's own failure-suffix prefix minima: over every layer at or after each layer,
the least node index that needs a failure suffix. The declared table is compared against
this, never regenerated from it. -/
def atLayer (graph : Graph) (fails : Node → Bool) (layerOf : Nat → Nat) (layer : Nat) : Nat :=
  (List.range graph.arity).foldl (fun acc index =>
    if fails (graph.node index) && layerOf index = layer then acc.min index else acc)
    maxRegister

/-- The prefix minima table, one entry per layer plus one trailing entry. -/
def expectedMinima (layerCount : Nat) (graph : Graph) (fails : Node → Bool)
    (layerOf : Nat → Nat) : List Nat :=
  (List.range layerCount).map fun layer =>
    (List.range (layerCount - layer)).foldl (fun acc offset =>
      acc.min (atLayer graph fails layerOf (layer + offset))) maxRegister

/-- The declared failure-suffix table is the graph's own prefix minima table. -/
def failSuffixOk (layerCount : Nat) (graph : Graph) (fails : Node → Bool)
    (layerOf : Nat → Nat) (layout : Layout) : Bool :=
  decide (layout.failSuffixMin = expectedMinima layerCount graph fails layerOf ++ [maxRegister])

/-- The obligations a declared layout must satisfy, in the fragment checker's order. -/
def obligations (graph : Graph) (layerCount : Nat) (fails : Node → Bool)
    (layerOf : Nat → Nat) (layout : Layout) : List Bool :=
  [decide (layout.revision = revision),
    (List.range graph.arity).all (declares layout),
    distinctRegisters graph layout,
    countReservesOne graph layout,
    layout.consts.all (constantEntryOk graph layout),
    constantCountOk graph layout,
    constantsDistinctOk layout,
    layout.slotRegs.all (slotEntryOk graph layout),
    layerShapeOk layerCount layout,
    layerOrderOk layout,
    failSuffixOk layerCount graph fails layerOf layout,
    coverageOk graph layerOf layout]

/-- Acceptance of a declared layout is the conjunction of those obligations. -/
def accepted (graph : Graph) (layerCount : Nat) (fails : Node → Bool) (layerOf : Nat → Nat)
    (layout : Layout) : Bool :=
  (obligations graph layerCount fails layerOf layout).all id

/-! ### Refusals -/

/-- An inflated declared count is refused: headroom beyond the constant-one register is a wrong
declaration rather than capacity. -/
theorem count_inflated_refused (graph : Graph) (layout : Layout)
    (hinflated : graph.arity + 1 < layout.regs) :
    countReservesOne graph layout = false := by
  simp only [countReservesOne, Bool.and_eq_false_iff, decide_eq_false_iff_not]
  left
  omega

/-- A count without the constant-one register is refused, although every node's register stays
inside it (issue #104). -/
theorem count_without_one_register_refused (graph : Graph) (layout : Layout)
    (hmissing : layout.regs = graph.arity) :
    countReservesOne graph layout = false := by
  simp only [countReservesOne, Bool.and_eq_false_iff, decide_eq_false_iff_not]
  left
  omega

/-- A node that declares the constant-one register is refused (issue #104). -/
theorem one_register_held_refused (graph : Graph) (layout : Layout) (index : Nat)
    (hin : index < graph.arity) (hheld : layout.reg index = some graph.arity) :
    countReservesOne graph layout = false := by
  simp only [countReservesOne, Bool.and_eq_false_iff]
  right
  rw [List.all_eq_false]
  exact ⟨index, List.mem_range.mpr hin, by simp [hheld]⟩

/-- The count the compiler declares, one register per node plus the constant-one register that
no node holds, is accepted. -/
theorem count_reserving_one_is_accepted (graph : Graph) (layout : Layout)
    (hcount : layout.regs = graph.arity + 1)
    (hfree : ∀ index < graph.arity, layout.reg index ≠ some graph.arity) :
    countReservesOne graph layout = true := by
  simp only [countReservesOne, Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true,
    List.mem_range, bne_iff_ne, ne_eq]
  exact ⟨hcount, fun index hin => hfree index hin⟩

/-- A node declared without a register is refused. -/
theorem missing_register_refused (layout : Layout) (index : Nat)
    (hmissing : layout.reg index = none) :
    declares layout index = false := by
  simp [declares, hmissing]

/-- A register outside the declared count is refused. -/
theorem out_of_range_refused (layout : Layout) (index register : Nat)
    (hdeclared : layout.reg index = some register) (hcount : layout.regs ≤ register) :
    declares layout index = false := by
  simp [declares, hdeclared]
  omega

/-- Two distinct nodes that declare one register alias it, and an aliased layout is refused. -/
theorem alias_refused (graph : Graph) (layout : Layout) (first second : Nat)
    (hdistinct : first ≠ second) (hfirst : layout.reg first = some 7)
    (hsecond : layout.reg second = some 7) (hfirstIn : first < graph.arity)
    (hsecondIn : second < graph.arity) :
    distinctRegisters graph layout = false := by
  unfold distinctRegisters
  rw [List.all_eq_false]
  refine ⟨first, List.mem_range.mpr hfirstIn, ?_⟩
  simp only [Bool.not_eq_true]
  rw [List.all_eq_false]
  exact ⟨second, List.mem_range.mpr hsecondIn, by simp [hdistinct, hfirst, hsecond]⟩

/-- The per-node constant match predicate is true exactly when the node is that constant and
declares that register. The table obligation counts matches, so this correspondence is what
ties the Boolean table check to the readable obligation. -/
theorem constant_predicate_iff (graph : Graph) (layout : Layout) (entry : Nat × BitVec 64)
    (index : Nat) :
    ((match graph.node index, layout.reg index with
        | .constant bits, some register => decide (register = entry.1 ∧ bits = entry.2)
        | _, _ => false) = true) ↔
      (layout.reg index = some entry.1 ∧ graph.node index = .constant entry.2) := by
  cases hnode : graph.node index <;> cases hregister : layout.reg index <;>
    simp [decide_eq_true_eq]

/-- The per-node slot match predicate is true exactly when the node is that interface input
and declares that register. -/
theorem slot_predicate_iff (graph : Graph) (layout : Layout) (entry : Nat × Nat)
    (index : Nat) :
    ((match graph.node index, layout.reg index with
        | .input slot, some register => decide (register = entry.1 ∧ slot = entry.2)
        | _, _ => false) = true) ↔
      (layout.reg index = some entry.1 ∧ graph.node index = .input entry.2) := by
  cases hnode : graph.node index <;> cases hregister : layout.reg index <;>
    simp [decide_eq_true_eq]

/-- A constant table entry that names no node is refused: the table obligation counts matches,
so zero matches fail it. -/
theorem constant_entry_refused (graph : Graph) (layout : Layout) (entry : Nat × BitVec 64)
    (hmatching : ∀ index, index < graph.arity →
      ¬ (layout.reg index = some entry.1 ∧ graph.node index = .constant entry.2)) :
    constantEntryOk graph layout entry = false := by
  unfold constantEntryOk
  have hempty : ((List.range graph.arity).filter fun index =>
      match graph.node index, layout.reg index with
      | .constant bits, some register => register = entry.1 ∧ bits = entry.2
      | _, _ => false) = [] := by
    rw [List.filter_eq_nil_iff]
    intro index hmem hpredicate
    exact hmatching index (List.mem_range.mp hmem)
      ((constant_predicate_iff graph layout entry index).mp hpredicate)
  rw [hempty]
  simp

/-- A slot table entry that names no interface input of that slot is refused. -/
theorem slot_entry_refused (graph : Graph) (layout : Layout) (entry : Nat × Nat)
    (hmatching : ∀ index, index < graph.arity →
      ¬ (layout.reg index = some entry.1 ∧ graph.node index = .input entry.2)) :
    slotEntryOk graph layout entry = false := by
  unfold slotEntryOk
  have hempty : ((List.range graph.arity).filter fun index =>
      match graph.node index, layout.reg index with
      | .input slot, some register => register = entry.1 ∧ slot = entry.2
      | _, _ => false) = [] := by
    rw [List.filter_eq_nil_iff]
    intro index hmem hpredicate
    exact hmatching index (List.mem_range.mp hmem)
      ((slot_predicate_iff graph layout entry index).mp hpredicate)
  rw [hempty]
  simp

/-- A constant table with fewer entries than constant nodes is refused. -/
theorem constant_count_refused (graph : Graph) (layout : Layout) (expected : Nat)
    (hconstants : ((List.range graph.arity).filter fun index =>
      match graph.node index with
      | .constant _ => true
      | _ => false).length = expected)
    (hentries : layout.consts.length ≠ expected) :
    constantCountOk graph layout = false := by
  unfold constantCountOk
  simp [hconstants, hentries]

/-- A constant table that names one entry twice is refused, even when its length equals the
constant count and every entry names a real constant (issue #94). -/
theorem constant_duplicate_refused (layout : Layout) (hduplicate : ¬ layout.consts.Nodup) :
    constantsDistinctOk layout = false := by
  simp [constantsDistinctOk, hduplicate]

/-- A layer table without one range per layer is refused. -/
theorem layer_shape_refused (layerCount : Nat) (layout : Layout)
    (hshape : layout.layers.length ≠ layerCount) :
    layerShapeOk layerCount layout = false := by
  simp [layerShapeOk, hshape]

/-- A layer range that starts before its predecessor ends is refused. -/
theorem layers_unordered_refused (layout : Layout) (start end_ previousStart previousEnd : Nat)
    (hlayers : layout.layers = [(previousStart, previousEnd), (start, end_)])
    (hfirst : decide (previousStart ≤ previousEnd) = true)
    (hsecond : decide (previousEnd ≤ start) = true)
    (horder : decide (start ≤ end_) = false) :
    layerOrderOk layout = false := by
  unfold layerOrderOk
  rw [hlayers]
  simp [layerOrderList, hfirst, hsecond, horder]

/-- A layer table whose last range does not end at the instruction count is refused. -/
theorem coverage_refused (graph : Graph) (layerOf : Nat → Nat) (layout : Layout)
    (start end_ : Nat)
    (hlayers : layout.layers = [(start, end_)])
    (hinstructions : ((List.range graph.arity).filter fun index =>
      instructionNode (graph.node index)).length ≠ end_) :
    coverageOk graph layerOf layout = false := by
  have hend : end_ ≠ ((List.range graph.arity).filter fun index =>
      instructionNode (graph.node index)).length := fun equal => hinstructions equal.symm
  unfold coverageOk
  rw [hlayers]
  simp [hend]

/-- A first layer range that does not start at the beginning of the stream, or does not hold
exactly layer 0's instruction nodes, is refused. Emptying a layer or moving one of its
instructions into a neighbouring range is therefore caught even when every range stays ordered
and the last ends at the right total (issue #95). -/
theorem layer_range_refused (graph : Graph) (layerOf : Nat → Nat) (layout : Layout)
    (range : Nat × Nat) (rest : List (Nat × Nat))
    (hlayers : layout.layers = range :: rest)
    (hrange : ¬ (range.1 = 0 ∧ range.2 = layerInstructions graph layerOf 0)) :
    coverageOk graph layerOf layout = false := by
  unfold coverageOk
  rw [hlayers]
  simp [tiles, hrange]

/-- A declared failure-suffix table that differs from the graph's own prefix minima is
refused. -/
theorem fail_suffix_mismatch_refused (layerCount : Nat) (graph : Graph)
    (fails : Node → Bool) (layerOf : Nat → Nat) (layout : Layout)
    (hmismatch : layout.failSuffixMin ≠ expectedMinima layerCount graph fails layerOf ++
      [maxRegister]) :
    failSuffixOk layerCount graph fails layerOf layout = false := by
  simp [failSuffixOk, hmismatch]

/-! ### Acceptance implies the obligations -/

/-- An accepted layout declares a register for every node. -/
theorem accepted_implies_registers (graph : Graph) (layerCount : Nat) (fails : Node → Bool)
    (layerOf : Nat → Nat) (layout : Layout)
    (haccepted : accepted graph layerCount fails layerOf layout = true) (index : Nat)
    (hin : index < graph.arity) : declares layout index = true := by
  unfold accepted at haccepted
  rw [List.all_eq_true] at haccepted
  have hmem : (List.range graph.arity).all (declares layout) ∈
      obligations graph layerCount fails layerOf layout := by
    unfold obligations
    exact List.mem_cons_of_mem _ List.mem_cons_self
  have hdeclares := haccepted ((List.range graph.arity).all (declares layout)) hmem
  simp only [id_eq] at hdeclares
  rw [List.all_eq_true] at hdeclares
  exact hdeclares index (List.mem_range.mpr hin)

/-- An accepted layout never aliases a register. -/
theorem accepted_implies_distinct (graph : Graph) (layerCount : Nat) (fails : Node → Bool)
    (layerOf : Nat → Nat) (layout : Layout)
    (haccepted : accepted graph layerCount fails layerOf layout = true) :
    distinctRegisters graph layout = true := by
  unfold accepted at haccepted
  rw [List.all_eq_true] at haccepted
  have hmem : distinctRegisters graph layout ∈ obligations graph layerCount fails layerOf layout := by
    unfold obligations
    exact List.mem_cons_of_mem _ (List.mem_cons_of_mem _ List.mem_cons_self)
  have hdistinct := haccepted (distinctRegisters graph layout) hmem
  simpa only [id_eq] using hdistinct

/-- An accepted layout names each constant at most once. -/
theorem accepted_implies_constants_distinct (graph : Graph) (layerCount : Nat)
    (fails : Node → Bool) (layerOf : Nat → Nat) (layout : Layout)
    (haccepted : accepted graph layerCount fails layerOf layout = true) :
    layout.consts.Nodup := by
  unfold accepted at haccepted
  rw [List.all_eq_true] at haccepted
  have hmem : constantsDistinctOk layout ∈ obligations graph layerCount fails layerOf layout := by
    unfold obligations
    simp
  have hdistinct := haccepted (constantsDistinctOk layout) hmem
  simpa [constantsDistinctOk] using hdistinct

/-- An accepted layout's layer ranges tile the instruction stream: each holds exactly its
layer's instruction nodes, and the last ends at the instruction count. -/
theorem accepted_implies_coverage (graph : Graph) (layerCount : Nat) (fails : Node → Bool)
    (layerOf : Nat → Nat) (layout : Layout)
    (haccepted : accepted graph layerCount fails layerOf layout = true) :
    coverageOk graph layerOf layout = true := by
  unfold accepted at haccepted
  rw [List.all_eq_true] at haccepted
  have hmem : coverageOk graph layerOf layout ∈ obligations graph layerCount fails layerOf layout := by
    unfold obligations
    simp
  have hcoverage := haccepted (coverageOk graph layerOf layout) hmem
  simpa only [id_eq] using hcoverage

/-- An accepted layout reserves the last register for the constant one: the count is one per
node plus one, and every node's register lies below it (issue #104). -/
theorem accepted_reserves_one_register (graph : Graph) (layerCount : Nat)
    (fails : Node → Bool) (layerOf : Nat → Nat) (layout : Layout)
    (haccepted : accepted graph layerCount fails layerOf layout = true) :
    layout.regs = graph.arity + 1 ∧
      ∀ index < graph.arity, ∀ register, layout.reg index = some register →
        register < graph.arity := by
  unfold accepted at haccepted
  rw [List.all_eq_true] at haccepted
  have hcountMem : countReservesOne graph layout ∈
      obligations graph layerCount fails layerOf layout := by
    unfold obligations
    simp
  have hdeclaresMem : (List.range graph.arity).all (declares layout) ∈
      obligations graph layerCount fails layerOf layout := by
    unfold obligations
    exact List.mem_cons_of_mem _ List.mem_cons_self
  have hcount := haccepted _ hcountMem
  have hdeclares := haccepted _ hdeclaresMem
  simp only [id_eq, countReservesOne, Bool.and_eq_true, decide_eq_true_eq, List.all_eq_true,
    List.mem_range, bne_iff_ne, ne_eq] at hcount hdeclares
  obtain ⟨hregs, hfree⟩ := hcount
  refine ⟨hregs, fun index hin register hreg => ?_⟩
  have hdeclared := hdeclares index hin
  simp only [declares, hreg, decide_eq_true_eq] at hdeclared
  have hne : register ≠ graph.arity := fun hsame => hfree index hin (by rw [hreg, hsame])
  omega

/-! ### A real layout is accepted -/

/-- The layer of each `exampleGraph` node in the Rust layered schedule: the input and the
constant at layer 0, the addition at layer 1, the equality at layer 2. -/
def exampleLayerOf (index : Nat) : Nat :=
  if index ≤ 1 then 0 else index - 1

/-- The declaration `Compiled::new(..).packed_layout()` emits for the Rust graph with the same
nodes as `exampleGraph` (one `I64` input, the constant 5, their sum, the sum compared with
itself): `regs = 5`, registers `[0, 1, 2, 3]`, constants `[(1, 5)]`, slots `[(0, 0)]`, layers
`[(0, 0), (0, 1), (1, 2)]` and four failure-suffix entries of `u32::MAX`, which the Rust
`layout::check` accepts. -/
def exampleLayout : Layout where
  revision := revision
  regs := 5
  reg := fun index => if index < 4 then some index else none
  consts := [(1, 5)]
  slotRegs := [(0, 0)]
  layers := [(0, 0), (0, 1), (1, 2)]
  failSuffixMin := [maxRegister, maxRegister, maxRegister, maxRegister]

/-- **The real layout is accepted**: every obligation holds for the declaration the Rust
scheduler emits, so acceptance is not vacuous. -/
theorem exampleLayout_accepted :
    accepted exampleGraph 3 (fun _ => false) exampleLayerOf exampleLayout = true := by
  decide

end NobleFoundation.PackedLayout
