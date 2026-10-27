/-
Model of the declared packed graph layout and its checker obligations (issue #43).

`crates/noble-ngraph/src/layout.rs` emits a `packed_layout()` declaration — a revision, a
declared register count, one declared register per node, the constant and interface-slot
tables, one instruction range per layer, the failure-suffix prefix minima and a layout build
key — and validates it against the graph the checker produced. This file models the *declared
tables and the obligations*, not the Rust bytes, and proves:

* an inflated register count is refused (`count_inflated_refused`): the count may sit one
  above the highest declared register but no further;
* a missing register, an out-of-range register and a shared register are each refused
  (`missing_register_refused`, `out_of_range_refused`, `alias_refused`);
* the constant table must have one entry per constant node (`constant_count_refused`), and an
  entry that names no node is refused (`constant_entry_refused`); the Boolean table check is
  tied to the readable obligation by `constant_predicate_iff` and `slot_predicate_iff`;
* the layer table must have one ordered range per layer (`layer_shape_refused`,
  `layers_unordered_refused`) and its ranges must cover exactly the instruction nodes
  (`coverage_refused`);
* acceptance implies a declared register per node, non-aliasing and coverage
  (`accepted_implies_registers`, `accepted_implies_distinct`, `accepted_implies_coverage`);
* a declared failure-suffix table that differs from the graph's own prefix minima is
  refused (`fail_suffix_mismatch_refused`).

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

/-- The declared count sits at most one above each declared register it names. -/
def countWithinOne (graph : Graph) (layout : Layout) : Bool :=
  (List.range graph.arity).all fun index =>
    match layout.reg index with
    | some register => decide (layout.regs ≤ register + 2)
    | none => true

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

/-- Every node that produces an instruction is covered exactly once: the last layer range
ends at the instruction count. -/
def coverageOk (graph : Graph) (layout : Layout) : Bool :=
  let instructions := ((List.range graph.arity).filter fun index =>
    match graph.node index with
    | .input _ => false
    | .constant _ => false
    | _ => true).length
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
    countWithinOne graph layout,
    layout.consts.all (constantEntryOk graph layout),
    constantCountOk graph layout,
    layout.slotRegs.all (slotEntryOk graph layout),
    layerShapeOk layerCount layout,
    layerOrderOk layout,
    failSuffixOk layerCount graph fails layerOf layout,
    coverageOk graph layout]

/-- Acceptance of a declared layout is the conjunction of those obligations. -/
def accepted (graph : Graph) (layerCount : Nat) (fails : Node → Bool) (layerOf : Nat → Nat)
    (layout : Layout) : Bool :=
  (obligations graph layerCount fails layerOf layout).all id

/-! ### Refusals -/

/-- An inflated declared count is refused: the count may sit one above the highest declared
register, but a declaration two above it is a wrong declaration rather than headroom. -/
theorem count_inflated_refused (graph : Graph) (layout : Layout) (index register : Nat)
    (hinflated : layout.regs = register + 3) (hdeclared : layout.reg index = some register)
    (hin : index < graph.arity) :
    countWithinOne graph layout = false := by
  unfold countWithinOne
  rw [List.all_eq_false]
  refine ⟨index, List.mem_range.mpr hin, ?_⟩
  simp [hdeclared, hinflated]

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
theorem coverage_refused (graph : Graph) (layout : Layout) (start end_ : Nat)
    (hlayers : layout.layers = [(start, end_)])
    (hinstructions : ((List.range graph.arity).filter fun index =>
      match graph.node index with
      | .input _ => false
      | .constant _ => false
      | _ => true).length ≠ end_) :
    coverageOk graph layout = false := by
  unfold coverageOk
  rw [hlayers]
  simp only [List.reverse_cons, List.reverse_nil, List.nil_append]
  rw [decide_eq_false_iff_not]
  exact fun equal => hinstructions equal.symm

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

/-- An accepted layout's last layer range ends exactly at the instruction count. -/
theorem accepted_implies_coverage (graph : Graph) (layerCount : Nat) (fails : Node → Bool)
    (layerOf : Nat → Nat) (layout : Layout)
    (haccepted : accepted graph layerCount fails layerOf layout = true) :
    coverageOk graph layout = true := by
  unfold accepted at haccepted
  rw [List.all_eq_true] at haccepted
  have hmem : coverageOk graph layout ∈ obligations graph layerCount fails layerOf layout := by
    unfold obligations
    exact List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_of_mem _ (List.mem_cons_self))))))))))
  have hcoverage := haccepted (coverageOk graph layout) hmem
  simpa only [id_eq] using hcoverage

end NobleFoundation.PackedLayout
