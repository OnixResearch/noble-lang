/-
  Residualization boundary for issue #32.

  This model begins at the index-preserving graph produced by the existing
  static-input substitution/arithmetic-fold model. It adds exact I64 equality
  folding, then models pruning and dense node renumbering through a validated
  canonical survivor map. It covers I64 inputs and ordered outputs only; the
  survivor-map construction itself and the Rust/backend bridge remain open.
-/
import NobleFoundation.Specialize

namespace NobleFoundation.SpecializeResidual

open NobleFoundation.PackedSchedule
open NobleFoundation.Specialize

/-- The residualization model's node language, extending the packed graph with
an exact Boolean constant so an I64 equality can be folded. -/
inductive ResidualNode where
  | input (slot : Nat)
  | i64Constant (bits : BitVec 64)
  | boolConstant (value : Bool)
  | add (first second : Nat)
  | sub (first second : Nat)
  | mul (first second : Nat)
  | eq (first second : Nat)
deriving DecidableEq, Repr

/-- Ordered operands of a residualization node. -/
def ResidualNode.operands : ResidualNode → List Nat
  | .add first second => [first, second]
  | .sub first second => [first, second]
  | .mul first second => [first, second]
  | .eq first second => [first, second]
  | _ => []

/-- A finite graph with an ordered output vector. -/
structure ResidualGraph where
  arity : Nat
  outputs : List Nat
  node : Nat → ResidualNode

/-- Output indices and every operand index are in range and topologically
ordered. -/
def WellFormed (graph : ResidualGraph) : Prop :=
  (∀ output, output ∈ graph.outputs → output < graph.arity) ∧
    (∀ index, ∀ operand ∈ (graph.node index).operands, operand < index)

/-- Convert a node from the preceding packed-graph model. -/
def ResidualNode.ofPacked : Node → ResidualNode
  | .input slot => .input slot
  | .constant bits => .i64Constant bits
  | .add first second => .add first second
  | .sub first second => .sub first second
  | .mul first second => .mul first second
  | .eq first second => .eq first second

/-- Lift a packed graph into this model while selecting its ordered outputs. -/
def ofPacked (graph : Graph) (outputs : List Nat) : ResidualGraph :=
  { arity := graph.arity
    outputs := outputs
    node := fun index => ResidualNode.ofPacked (graph.node index) }

/-- Well-formedness of a lifted graph for an explicitly checked output vector. -/
theorem ofPacked_wellFormed (graph : Graph) (outputs : List Nat)
    (hwf : PackedSchedule.WellFormed graph)
    (hout : ∀ output, output ∈ outputs → output < graph.arity) :
    WellFormed (ofPacked graph outputs) := by
  refine ⟨hout, ?_⟩
  intro index operand hoperand
  have hsame : (ResidualNode.ofPacked (graph.node index)).operands =
      (graph.node index).operands := by
    cases graph.node index <;> rfl
  exact hwf.2 index operand (by simpa only [ofPacked, hsame] using hoperand)

/-- Evaluate one residual-model node. A wrong operand kind is undefined, as in
`PackedSchedule`. -/
def evalNode (node : ResidualNode) (values : Nat → Option Val)
    (inputs : Nat → BitVec 64) : Option Val :=
  match node with
  | .input slot => some (.i64 (inputs slot))
  | .i64Constant bits => some (.i64 bits)
  | .boolConstant value => some (.boolean value)
  | .add first second =>
      match values first, values second with
      | some (.i64 x), some (.i64 y) => some (.i64 (x + y))
      | _, _ => none
  | .sub first second =>
      match values first, values second with
      | some (.i64 x), some (.i64 y) => some (.i64 (x - y))
      | _, _ => none
  | .mul first second =>
      match values first, values second with
      | some (.i64 x), some (.i64 y) => some (.i64 (x * y))
      | _, _ => none
  | .eq first second =>
      match values first, values second with
      | some (.i64 x), some (.i64 y) => some (.boolean (x == y))
      | _, _ => none

/-- Evaluate one node in a residual graph. -/
def evalStep (graph : ResidualGraph) (values : Nat → Option Val)
    (inputs : Nat → BitVec 64) (index : Nat) : Option Val :=
  evalNode (graph.node index) values inputs

/-- Values of the first `count` residual nodes. -/
def valuesUpTo (graph : ResidualGraph) (inputs : Nat → BitVec 64) :
    Nat → (Nat → Option Val)
  | 0 => fun _ => none
  | count + 1 => fun index =>
      if index = count then evalStep graph (valuesUpTo graph inputs count) inputs count
      else valuesUpTo graph inputs count index

/-- The residual-model value of one node. -/
def value (graph : ResidualGraph) (inputs : Nat → BitVec 64) (index : Nat) : Option Val :=
  valuesUpTo graph inputs (index + 1) index

/-- Values of the ordered output vector. -/
def outputValues (graph : ResidualGraph) (inputs : Nat → BitVec 64) : List (Option Val) :=
  graph.outputs.map (value graph inputs)

/-- Lifting a packed graph changes node tags, not its reference evaluation. -/
theorem ofPacked_value (graph : Graph) (outputs : List Nat)
    (inputs : Nat → BitVec 64) (index : Nat) :
    value (ofPacked graph outputs) inputs index =
      PackedSchedule.value graph inputs index := by
  have hp : ∀ count (node : Nat),
      valuesUpTo (ofPacked graph outputs) inputs count node =
        PackedSchedule.valuesUpTo graph inputs count node := by
    intro count
    induction count with
    | zero => intro node; rfl
    | succ count ih =>
        intro node
        by_cases hlast : node = count
        · subst node
          simp only [valuesUpTo, PackedSchedule.valuesUpTo, ite_eq_left]
          have hfun : valuesUpTo (ofPacked graph outputs) inputs count =
              PackedSchedule.valuesUpTo graph inputs count := funext ih
          rw [hfun]
          cases hnode : graph.node count <;>
            simp only [evalStep, evalNode, PackedSchedule.evalStep, ofPacked,
              ResidualNode.ofPacked, hnode] <;> rfl
        · simp only [valuesUpTo, PackedSchedule.valuesUpTo, ite_eq_right hlast]
          exact ih node
  exact hp (index + 1) index

/-- A value in a strict prefix is the same as the value at its own prefix. -/
theorem valuesUpTo_eq_value (graph : ResidualGraph) (inputs : Nat → BitVec 64) :
    ∀ count index, index < count →
      valuesUpTo graph inputs count index = value graph inputs index := by
  intro count
  induction count with
  | zero => intro index h; exact absurd h (Nat.not_lt_zero index)
  | succ count ih =>
      intro index h
      by_cases hlast : index = count
      · rw [hlast]
        simp [value, valuesUpTo]
      · have hlt : index < count := by omega
        simp only [valuesUpTo, ite_eq_right hlast]
        exact ih index hlt

/-- The value at a node is its step over the strictly earlier prefix. -/
theorem value_eq_evalStep (graph : ResidualGraph) (inputs : Nat → BitVec 64)
    (index : Nat) :
    value graph inputs index = evalStep graph (valuesUpTo graph inputs index) inputs index := by
  simp [value, valuesUpTo]

/-- A literal node denotes its exact bits. -/
theorem i64Constant_value (graph : ResidualGraph) (inputs : Nat → BitVec 64)
    (index : Nat) (bits : BitVec 64) (hnode : graph.node index = .i64Constant bits) :
    value graph inputs index = some (.i64 bits) := by
  rw [value_eq_evalStep]
  simp [evalStep, evalNode, hnode]

/-- Equality folding considers the statically-known I64 operands of one node.
The comparison is BitVec equality, exactly the wrapping-I64 equality operation. -/
def foldEqNode (graph : ResidualGraph) (index : Nat) : ResidualNode :=
  match graph.node index with
  | .eq first second =>
      match graph.node first, graph.node second with
      | .i64Constant x, .i64Constant y => .boolConstant (x == y)
      | _, _ => .eq first second
  | node => node

/-- Fold every equality whose operands are exact I64 constants. -/
def foldEqualities (graph : ResidualGraph) : ResidualGraph :=
  { graph with node := foldEqNode graph }

/-- Equality folding can only remove operand edges, never add an edge. -/
theorem foldEqNode_operands_subset (graph : ResidualGraph) (index operand : Nat)
    (hoperand : operand ∈ (foldEqNode graph index).operands) :
    operand ∈ (graph.node index).operands := by
  cases hnode : graph.node index with
  | input slot => simp [foldEqNode, hnode, ResidualNode.operands] at hoperand
  | i64Constant bits => simp [foldEqNode, hnode, ResidualNode.operands] at hoperand
  | boolConstant value => simp [foldEqNode, hnode, ResidualNode.operands] at hoperand
  | add first second => simpa [foldEqNode, hnode, ResidualNode.operands] using hoperand
  | sub first second => simpa [foldEqNode, hnode, ResidualNode.operands] using hoperand
  | mul first second => simpa [foldEqNode, hnode, ResidualNode.operands] using hoperand
  | eq first second =>
      cases hfirst : graph.node first <;> cases hsecond : graph.node second <;>
        simp_all [foldEqNode, ResidualNode.operands]

theorem foldEqualities_wellFormed (graph : ResidualGraph) (hwf : WellFormed graph) :
    WellFormed (foldEqualities graph) := by
  refine ⟨?_, ?_⟩
  · simpa [foldEqualities] using hwf.1
  · intro index operand hoperand
    exact hwf.2 index operand
      (foldEqNode_operands_subset graph index operand (by simpa [foldEqualities] using hoperand))

/-- Congruence for residual-model nodes: equal ordered operands and equal
operand values imply equal node values. -/
theorem evalNode_congr (node₁ node₂ : ResidualNode)
    (values₁ values₂ : Nat → Option Val) (inputs₁ inputs₂ : Nat → BitVec 64)
    (hnode : node₁ = node₂)
    (hinput : ∀ slot, node₁ = .input slot → inputs₁ slot = inputs₂ slot)
    (hoperand : ∀ operand ∈ node₁.operands, values₁ operand = values₂ operand) :
    evalNode node₁ values₁ inputs₁ = evalNode node₂ values₂ inputs₂ := by
  subst node₂
  cases hn : node₁ with
  | input slot => simp [evalNode, hinput slot hn]
  | i64Constant bits => simp [evalNode]
  | boolConstant value => simp [evalNode]
  | add first second =>
      have hfirst : first ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      have hsecond : second ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      simp [evalNode, hoperand first hfirst, hoperand second hsecond]
  | sub first second =>
      have hfirst : first ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      have hsecond : second ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      simp [evalNode, hoperand first hfirst, hoperand second hsecond]
  | mul first second =>
      have hfirst : first ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      have hsecond : second ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      simp [evalNode, hoperand first hfirst, hoperand second hsecond]
  | eq first second =>
      have hfirst : first ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      have hsecond : second ∈ node₁.operands := by simp [hn, ResidualNode.operands]
      simp [evalNode, hoperand first hfirst, hoperand second hsecond]

/-- Same-index congruence for two well-formed graphs. -/
theorem value_congr (graph₁ graph₂ : ResidualGraph) (inputs₁ inputs₂ : Nat → BitVec 64)
    (hwf₁ : WellFormed graph₁) (hwf₂ : WellFormed graph₂) (index : Nat)
    (hnode : graph₁.node index = graph₂.node index)
    (hinput : ∀ slot, graph₁.node index = .input slot → inputs₁ slot = inputs₂ slot)
    (hoperand : ∀ operand ∈ (graph₁.node index).operands,
      value graph₁ inputs₁ operand = value graph₂ inputs₂ operand) :
    value graph₁ inputs₁ index = value graph₂ inputs₂ index := by
  have hvalue₁ := value_eq_evalStep graph₁ inputs₁ index
  have hvalue₂ := value_eq_evalStep graph₂ inputs₂ index
  rw [hvalue₁, hvalue₂]
  apply evalNode_congr (graph₁.node index) (graph₂.node index)
  · exact hnode
  · exact hinput
  · intro operand hmem
    have hmem₂ : operand ∈ (graph₂.node index).operands := by rw [← hnode]; exact hmem
    have hprefix₁ : valuesUpTo graph₁ inputs₁ index operand = value graph₁ inputs₁ operand :=
      valuesUpTo_eq_value graph₁ inputs₁ index operand (hwf₁.2 index operand hmem)
    have hprefix₂ : valuesUpTo graph₂ inputs₂ index operand = value graph₂ inputs₂ operand := by
      apply valuesUpTo_eq_value graph₂ inputs₂ index operand
      exact hwf₂.2 index operand hmem₂
    rw [hprefix₁, hprefix₂]
    exact hoperand operand hmem

/-- Exact I64 equality folding preserves every graph value. -/
theorem foldEqualities_sound (graph : ResidualGraph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) :
    ∀ index, value (foldEqualities graph) inputs index = value graph inputs index := by
  have hfoldwf := foldEqualities_wellFormed graph hwf
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      by_cases hfold : ∃ first second x y,
          graph.node index = .eq first second ∧
          graph.node first = .i64Constant x ∧ graph.node second = .i64Constant y
      · obtain ⟨first, second, x, y, hnode, hfirstNode, hsecondNode⟩ := hfold
        have hfirst : first < index := hwf.2 index first
          (by simp [hnode, ResidualNode.operands])
        have hsecond : second < index := hwf.2 index second
          (by simp [hnode, ResidualNode.operands])
        have hfirstPrefix : valuesUpTo graph inputs index first = some (.i64 x) := by
          rw [valuesUpTo_eq_value graph inputs index first hfirst]
          exact i64Constant_value graph inputs first x hfirstNode
        have hsecondPrefix : valuesUpTo graph inputs index second = some (.i64 y) := by
          rw [valuesUpTo_eq_value graph inputs index second hsecond]
          exact i64Constant_value graph inputs second y hsecondNode
        have hfoldNode : (foldEqualities graph).node index =
            .boolConstant (x == y) := by
          simp [foldEqualities, foldEqNode, hnode, hfirstNode, hsecondNode]
        calc
          value (foldEqualities graph) inputs index =
              some (.boolean (x == y)) := by
                rw [value_eq_evalStep]
                simp [evalStep, evalNode, hfoldNode]
          _ = value graph inputs index := by
                rw [value_eq_evalStep]
                simp [evalStep, evalNode, hnode, hfirstPrefix, hsecondPrefix]
      · have hsame : (foldEqualities graph).node index = graph.node index := by
          cases hnode : graph.node index with
          | input slot => simp [foldEqualities, foldEqNode, hnode]
          | i64Constant bits => simp [foldEqualities, foldEqNode, hnode]
          | boolConstant value => simp [foldEqualities, foldEqNode, hnode]
          | add first second => simp [foldEqualities, foldEqNode, hnode]
          | sub first second => simp [foldEqualities, foldEqNode, hnode]
          | mul first second => simp [foldEqualities, foldEqNode, hnode]
          | eq first second =>
              cases hfirst : graph.node first <;> cases hsecond : graph.node second <;>
                simp_all [foldEqualities, foldEqNode]
        apply value_congr (foldEqualities graph) graph inputs inputs hfoldwf hwf index hsame
        · intro slot hinput
          rfl
        · intro operand hoperand
          have hsource : operand ∈ (graph.node index).operands := by
            simpa only [hsame] using hoperand
          exact ih operand (hwf.2 index operand hsource)

/-- Remap only node indices; input slots and constants are unchanged. -/
def renumberNode (map : Nat → Nat) : ResidualNode → ResidualNode
  | .input slot => .input slot
  | .i64Constant bits => .i64Constant bits
  | .boolConstant value => .boolConstant value
  | .add first second => .add (map first) (map second)
  | .sub first second => .sub (map first) (map second)
  | .mul first second => .mul (map first) (map second)
  | .eq first second => .eq (map first) (map second)

theorem renumberNode_operands (map : Nat → Nat) (node : ResidualNode) :
    (renumberNode map node).operands = node.operands.map map := by
  cases node <;> rfl

/-- A valid survivor map for pruning and dense, order-preserving renumbering.
`sourceAt` and `newIndex` are inverse on the dense kept range. All output roots,
all live operands and every dynamic input node are retained. -/
structure PruneMap (graph : ResidualGraph) where
  kept : List Nat
  sourceAt : Nat → Nat
  newIndex : Nat → Nat
  sourceAt_kept : ∀ index, index < kept.length → sourceAt index ∈ kept
  kept_bound : ∀ index, index ∈ kept → index < graph.arity
  sourceAt_newIndex : ∀ index, index ∈ kept → sourceAt (newIndex index) = index
  newIndex_sourceAt : ∀ index, index < kept.length → newIndex (sourceAt index) = index
  newIndex_bound : ∀ index, index ∈ kept → newIndex index < kept.length
  output_kept : ∀ output, output ∈ graph.outputs → output ∈ kept
  operand_kept : ∀ index, index ∈ kept →
    ∀ operand, operand ∈ (graph.node index).operands → operand ∈ kept
  input_kept : ∀ index slot, index < graph.arity →
    graph.node index = .input slot → index ∈ kept
  canonical_order : ∀ first, first ∈ kept → ∀ second, second ∈ kept →
    first < second → newIndex first < newIndex second
  operand_order : ∀ index, index ∈ kept →
    ∀ operand, operand ∈ (graph.node index).operands → newIndex operand < newIndex index

/-- Delete dropped nodes, rewrite surviving operands through the dense map, and
preserve the source output order. -/
def pruneRenumber (graph : ResidualGraph) (mapping : PruneMap graph) : ResidualGraph :=
  { arity := mapping.kept.length
    outputs := graph.outputs.map mapping.newIndex
    node := fun index =>
      if index < mapping.kept.length then
        renumberNode mapping.newIndex (graph.node (mapping.sourceAt index))
      else
        .input 0 }

theorem pruneRenumber_wellFormed (graph : ResidualGraph) (_hwf : WellFormed graph)
    (mapping : PruneMap graph) : WellFormed (pruneRenumber graph mapping) := by
  refine ⟨?_, ?_⟩
  · intro output houtput
    simp only [pruneRenumber, List.mem_map] at houtput
    rcases houtput with ⟨old, hold, rfl⟩
    exact mapping.newIndex_bound old (mapping.output_kept old hold)
  · intro index operand hoperand
    by_cases hindex : index < mapping.kept.length
    · have hsourceKept := mapping.sourceAt_kept index hindex
      have hsourceIndex : mapping.newIndex (mapping.sourceAt index) = index :=
        mapping.newIndex_sourceAt index hindex
      have hoperandSource :
          ∃ oldOperand, oldOperand ∈ (graph.node (mapping.sourceAt index)).operands ∧
            mapping.newIndex oldOperand = operand := by
        simp only [pruneRenumber, ite_eq_left hindex, renumberNode_operands] at hoperand
        exact List.mem_map.mp hoperand
      obtain ⟨oldOperand, hmem, rfl⟩ := hoperandSource
      have horder := mapping.operand_order (mapping.sourceAt index) hsourceKept oldOperand hmem
      simpa only [hsourceIndex] using horder
    · simp [pruneRenumber, hindex, ResidualNode.operands] at hoperand

/-- A retained node's residual denotation equals its source denotation. -/
theorem pruneRenumber_value (graph : ResidualGraph) (hwf : WellFormed graph)
    (mapping : PruneMap graph) (inputs : Nat → BitVec 64) :
    ∀ index, index ∈ mapping.kept →
      value (pruneRenumber graph mapping) inputs (mapping.newIndex index) =
        value graph inputs index := by
  have hreswf := pruneRenumber_wellFormed graph hwf mapping
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      intro hkept
      have hmapBound := mapping.newIndex_bound index hkept
      have hsource : mapping.sourceAt (mapping.newIndex index) = index :=
        mapping.sourceAt_newIndex index hkept
      have hnode : (pruneRenumber graph mapping).node (mapping.newIndex index) =
          renumberNode mapping.newIndex (graph.node index) := by
        simp [pruneRenumber, hmapBound, hsource]
      have hresValue := value_eq_evalStep (pruneRenumber graph mapping) inputs
        (mapping.newIndex index)
      have hsourceValue := value_eq_evalStep graph inputs index
      cases hgraphNode : graph.node index with
      | input slot =>
          rw [hresValue, hsourceValue]
          simp [evalStep, evalNode, hnode, hgraphNode, renumberNode]
      | i64Constant bits =>
          rw [hresValue, hsourceValue]
          simp [evalStep, evalNode, hnode, hgraphNode, renumberNode]
      | boolConstant value =>
          rw [hresValue, hsourceValue]
          simp [evalStep, evalNode, hnode, hgraphNode, renumberNode]
      | add first second =>
          have hfirstlt := hwf.2 index first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondlt := hwf.2 index second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstKept := mapping.operand_kept index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondKept := mapping.operand_kept index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstResidual := ih first hfirstlt hfirstKept
          have hsecondResidual := ih second hsecondlt hsecondKept
          have hfirstOrder := mapping.operand_order index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondOrder := mapping.operand_order index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstSource : valuesUpTo graph inputs index first = value graph inputs first :=
            valuesUpTo_eq_value graph inputs index first hfirstlt
          have hsecondSource : valuesUpTo graph inputs index second = value graph inputs second :=
            valuesUpTo_eq_value graph inputs index second hsecondlt
          have hfirstTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex first) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) hfirstOrder
          have hsecondTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex second) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) hsecondOrder
          rw [hresValue, hsourceValue]
          simp only [evalStep, hnode, hgraphNode, renumberNode, evalNode]
          rw [hfirstTarget, hsecondTarget, hfirstSource, hsecondSource,
            hfirstResidual, hsecondResidual]
      | sub first second =>
          have hfirstlt := hwf.2 index first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondlt := hwf.2 index second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstKept := mapping.operand_kept index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondKept := mapping.operand_kept index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstResidual := ih first hfirstlt hfirstKept
          have hsecondResidual := ih second hsecondlt hsecondKept
          have hfirstOrder := mapping.operand_order index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondOrder := mapping.operand_order index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstSource : valuesUpTo graph inputs index first = value graph inputs first :=
            valuesUpTo_eq_value graph inputs index first hfirstlt
          have hsecondSource : valuesUpTo graph inputs index second = value graph inputs second :=
            valuesUpTo_eq_value graph inputs index second hsecondlt
          have hfirstTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex first) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) hfirstOrder
          have hsecondTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex second) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) hsecondOrder
          rw [hresValue, hsourceValue]
          simp only [evalStep, hnode, hgraphNode, renumberNode, evalNode]
          rw [hfirstTarget, hsecondTarget, hfirstSource, hsecondSource,
            hfirstResidual, hsecondResidual]
      | mul first second =>
          have hfirstlt := hwf.2 index first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondlt := hwf.2 index second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstKept := mapping.operand_kept index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondKept := mapping.operand_kept index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstResidual := ih first hfirstlt hfirstKept
          have hsecondResidual := ih second hsecondlt hsecondKept
          have hfirstOrder := mapping.operand_order index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondOrder := mapping.operand_order index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstSource : valuesUpTo graph inputs index first = value graph inputs first :=
            valuesUpTo_eq_value graph inputs index first hfirstlt
          have hsecondSource : valuesUpTo graph inputs index second = value graph inputs second :=
            valuesUpTo_eq_value graph inputs index second hsecondlt
          have hfirstTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex first) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) hfirstOrder
          have hsecondTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex second) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) hsecondOrder
          rw [hresValue, hsourceValue]
          simp only [evalStep, hnode, hgraphNode, renumberNode, evalNode]
          rw [hfirstTarget, hsecondTarget, hfirstSource, hsecondSource,
            hfirstResidual, hsecondResidual]
      | eq first second =>
          have hfirstlt := hwf.2 index first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondlt := hwf.2 index second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstKept := mapping.operand_kept index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondKept := mapping.operand_kept index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstResidual := ih first hfirstlt hfirstKept
          have hsecondResidual := ih second hsecondlt hsecondKept
          have hfirstOrder := mapping.operand_order index hkept first (by simp [hgraphNode, ResidualNode.operands])
          have hsecondOrder := mapping.operand_order index hkept second (by simp [hgraphNode, ResidualNode.operands])
          have hfirstSource : valuesUpTo graph inputs index first = value graph inputs first :=
            valuesUpTo_eq_value graph inputs index first hfirstlt
          have hsecondSource : valuesUpTo graph inputs index second = value graph inputs second :=
            valuesUpTo_eq_value graph inputs index second hsecondlt
          have hfirstTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex first) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex first) hfirstOrder
          have hsecondTarget : valuesUpTo (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) =
                value (pruneRenumber graph mapping) inputs (mapping.newIndex second) :=
            valuesUpTo_eq_value (pruneRenumber graph mapping) inputs
              (mapping.newIndex index) (mapping.newIndex second) hsecondOrder
          rw [hresValue, hsourceValue]
          simp only [evalStep, hnode, hgraphNode, renumberNode, evalNode]
          rw [hfirstTarget, hsecondTarget, hfirstSource, hsecondSource,
            hfirstResidual, hsecondResidual]

/-- The ordered output vector is preserved by a valid pruning/renumbering map. -/
theorem pruneRenumber_outputs (graph : ResidualGraph) (hwf : WellFormed graph)
    (mapping : PruneMap graph) (inputs : Nat → BitVec 64) :
    outputValues (pruneRenumber graph mapping) inputs = outputValues graph inputs := by
  simp only [outputValues, pruneRenumber, List.map_map]
  apply List.map_congr_left
  intro output houtput
  exact pruneRenumber_value graph hwf mapping inputs output
    (mapping.output_kept output houtput)

/-- Ordered-output preservation for the composed model: the previous model's
input substitution and wrapping arithmetic folds, exact I64 equality folding,
and a valid dead-node pruning/dense-renumber map. -/
theorem residualize_outputs_sound (graph : Graph) (static : Nat → Option (BitVec 64))
    (outputs : List Nat) (hwf : PackedSchedule.WellFormed graph)
    (hout : ∀ output, output ∈ outputs → output < graph.arity)
    (mapping : PruneMap (foldEqualities (ofPacked (specialize graph static) outputs)))
    (inputs : Nat → BitVec 64) :
    outputValues
        (pruneRenumber (foldEqualities (ofPacked (specialize graph static) outputs)) mapping)
        inputs = outputs.map (fun output => PackedSchedule.value graph (merge static inputs) output) := by
  have hspecwf := specialize_wellFormed graph static hwf
  have hfoldwf := foldEqualities_wellFormed (ofPacked (specialize graph static) outputs)
    (ofPacked_wellFormed (specialize graph static) outputs hspecwf hout)
  rw [pruneRenumber_outputs _ hfoldwf mapping inputs]
  simp only [outputValues]
  apply List.map_congr_left
  intro output houtput
  have hfold := foldEqualities_sound (ofPacked (specialize graph static) outputs)
    (ofPacked_wellFormed (specialize graph static) outputs hspecwf hout) inputs output
  calc
    value (foldEqualities (ofPacked (specialize graph static) outputs)) inputs output =
        value (ofPacked (specialize graph static) outputs) inputs output := hfold
    _ = PackedSchedule.value (specialize graph static) inputs output :=
      ofPacked_value (specialize graph static) outputs inputs output
    _ = PackedSchedule.value graph (merge static inputs) output :=
      specialize_sound graph static hwf inputs output

/-! ### Shared Rust/Lean residualization fixtures -/

def eqPruneGraph : Graph where
  arity := 5
  output := 3
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else if index = 2 then .constant 3
    else if index = 3 then .eq 0 2
    else .add 1 1

theorem eqPruneGraph_wellFormed : PackedSchedule.WellFormed eqPruneGraph := by
  refine ⟨by decide, ?_⟩
  intro index operand hmem
  by_cases h0 : index = 0
  · subst h0; simp [eqPruneGraph, Node.operands] at hmem
  by_cases h1 : index = 1
  · subst h1; simp [eqPruneGraph, Node.operands] at hmem
  by_cases h2 : index = 2
  · subst h2; simp [eqPruneGraph, Node.operands] at hmem
  by_cases h3 : index = 3
  · subst h3; simp [eqPruneGraph, Node.operands] at hmem; omega
  · simp [eqPruneGraph, h0, h1, h2, h3, Node.operands] at hmem; omega

def equalStatic : Nat → Option (BitVec 64)
  | 0 => some 3
  | _ => none

def unequalStatic : Nat → Option (BitVec 64)
  | 0 => some 4
  | _ => none

def eqPruneOutputs : List Nat := [3, 4]

def equalFoldedGraph : ResidualGraph :=
  foldEqualities (ofPacked (specialize eqPruneGraph equalStatic) eqPruneOutputs)

def unequalFoldedGraph : ResidualGraph :=
  foldEqualities (ofPacked (specialize eqPruneGraph unequalStatic) eqPruneOutputs)

def eqPruneKept : List Nat := [1, 3, 4]

def eqPruneSource (index : Nat) : Nat :=
  if index = 0 then 1 else if index = 1 then 3 else 4

def eqPruneRenumber (index : Nat) : Nat :=
  if index = 1 then 0 else if index = 3 then 1 else if index = 4 then 2 else 0

def equalPruneMap : PruneMap equalFoldedGraph := by
  refine ⟨eqPruneKept, eqPruneSource, eqPruneRenumber, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro index hindex
    have hsmall : index = 0 ∨ index = 1 ∨ index = 2 := by
      simp [eqPruneKept] at hindex
      omega
    rcases hsmall with rfl | rfl | rfl <;> decide
  · intro index hindex
    simp [eqPruneKept] at hindex
    change index < 5
    rcases hindex with rfl | rfl | rfl <;> omega
  · intro index hindex
    simp [eqPruneKept] at hindex
    rcases hindex with rfl | rfl | rfl <;>
      decide
  · intro index hindex
    have hsmall : index = 0 ∨ index = 1 ∨ index = 2 := by
      simp [eqPruneKept] at hindex
      omega
    rcases hsmall with rfl | rfl | rfl <;> decide
  · intro index hindex
    simp [eqPruneKept] at hindex
    rcases hindex with rfl | rfl | rfl <;> decide
  · intro output houtput
    change output ∈ eqPruneOutputs at houtput
    simp [eqPruneOutputs] at houtput
    rcases houtput with rfl | rfl <;> decide
  · intro index hindex operand hoperand
    simp [eqPruneKept] at hindex
    rcases hindex with hindex | hindex | hindex <;> subst hindex
    · have hshape : equalFoldedGraph.node 1 = .input 0 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : equalFoldedGraph.node 3 = .boolConstant true := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : equalFoldedGraph.node 4 = .add 1 1 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
      rcases hoperand with rfl | rfl <;> decide
  · intro index slot hindex hnode
    have hsmall : index = 0 ∨ index = 1 ∨ index = 2 ∨ index = 3 ∨ index = 4 := by
      change index < 5 at hindex
      omega
    rcases hsmall with rfl | rfl | rfl | rfl | rfl
    · have hshape : equalFoldedGraph.node 0 = .i64Constant 3 := by decide
      simp [hshape] at hnode
    · simp [eqPruneKept]
    · have hshape : equalFoldedGraph.node 2 = .i64Constant 3 := by decide
      simp [hshape] at hnode
    · have hshape : equalFoldedGraph.node 3 = .boolConstant true := by decide
      simp [hshape] at hnode
    · have hshape : equalFoldedGraph.node 4 = .add 1 1 := by decide
      simp [hshape] at hnode
  · intro first hfirst second hsecond hlt
    simp [eqPruneKept] at hfirst hsecond
    rcases hfirst with rfl | rfl | rfl <;>
      rcases hsecond with rfl | rfl | rfl <;>
      simp [eqPruneRenumber] at hlt ⊢ <;> omega
  · intro index hindex operand hoperand
    simp [eqPruneKept] at hindex
    rcases hindex with hindex | hindex | hindex <;> subst hindex
    · have hshape : equalFoldedGraph.node 1 = .input 0 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : equalFoldedGraph.node 3 = .boolConstant true := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : equalFoldedGraph.node 4 = .add 1 1 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
      rcases hoperand with rfl | rfl <;> decide

def unequalPruneMap : PruneMap unequalFoldedGraph := by
  refine ⟨eqPruneKept, eqPruneSource, eqPruneRenumber, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro index hindex
    have hsmall : index = 0 ∨ index = 1 ∨ index = 2 := by
      simp [eqPruneKept] at hindex
      omega
    rcases hsmall with rfl | rfl | rfl <;> decide
  · intro index hindex
    simp [eqPruneKept] at hindex
    change index < 5
    rcases hindex with rfl | rfl | rfl <;> omega
  · intro index hindex
    simp [eqPruneKept] at hindex
    rcases hindex with rfl | rfl | rfl <;> decide
  · intro index hindex
    have hsmall : index = 0 ∨ index = 1 ∨ index = 2 := by
      simp [eqPruneKept] at hindex
      omega
    rcases hsmall with rfl | rfl | rfl <;> decide
  · intro index hindex
    simp [eqPruneKept] at hindex
    rcases hindex with rfl | rfl | rfl <;> decide
  · intro output houtput
    change output ∈ eqPruneOutputs at houtput
    simp [eqPruneOutputs] at houtput
    rcases houtput with rfl | rfl <;> decide
  · intro index hindex operand hoperand
    simp [eqPruneKept] at hindex
    rcases hindex with hindex | hindex | hindex <;> subst hindex
    · have hshape : unequalFoldedGraph.node 1 = .input 0 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : unequalFoldedGraph.node 3 = .boolConstant false := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : unequalFoldedGraph.node 4 = .add 1 1 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
      rcases hoperand with rfl | rfl <;> decide
  · intro index slot hindex hnode
    have hsmall : index = 0 ∨ index = 1 ∨ index = 2 ∨ index = 3 ∨ index = 4 := by
      change index < 5 at hindex
      omega
    rcases hsmall with rfl | rfl | rfl | rfl | rfl
    · have hshape : unequalFoldedGraph.node 0 = .i64Constant 4 := by decide
      simp [hshape] at hnode
    · simp [eqPruneKept]
    · have hshape : unequalFoldedGraph.node 2 = .i64Constant 3 := by decide
      simp [hshape] at hnode
    · have hshape : unequalFoldedGraph.node 3 = .boolConstant false := by decide
      simp [hshape] at hnode
    · have hshape : unequalFoldedGraph.node 4 = .add 1 1 := by decide
      simp [hshape] at hnode
  · intro first hfirst second hsecond hlt
    simp [eqPruneKept] at hfirst hsecond
    rcases hfirst with rfl | rfl | rfl <;>
      rcases hsecond with rfl | rfl | rfl <;>
      simp [eqPruneRenumber] at hlt ⊢ <;> omega
  · intro index hindex operand hoperand
    simp [eqPruneKept] at hindex
    rcases hindex with hindex | hindex | hindex <;> subst hindex
    · have hshape : unequalFoldedGraph.node 1 = .input 0 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : unequalFoldedGraph.node 3 = .boolConstant false := by decide
      simp [hshape, ResidualNode.operands] at hoperand
    · have hshape : unequalFoldedGraph.node 4 = .add 1 1 := by decide
      simp [hshape, ResidualNode.operands] at hoperand
      rcases hoperand with rfl | rfl <;> decide

def equalResidual : ResidualGraph := pruneRenumber equalFoldedGraph equalPruneMap

def unequalResidual : ResidualGraph := pruneRenumber unequalFoldedGraph unequalPruneMap

theorem rust_fixture_equal_shape :
    equalResidual.outputs = [1, 2] ∧
      equalResidual.node 0 = .input 0 ∧
      equalResidual.node 1 = .boolConstant true ∧
      equalResidual.node 2 = .add 0 0 := by
  decide

theorem rust_fixture_unequal_shape :
    unequalResidual.outputs = [1, 2] ∧
      unequalResidual.node 0 = .input 0 ∧
      unequalResidual.node 1 = .boolConstant false ∧
      unequalResidual.node 2 = .add 0 0 := by
  decide

theorem rust_fixture_equal_values (inputs : Nat → BitVec 64) :
    outputValues equalResidual inputs =
      [some (.boolean true), some (.i64 (inputs 0 + inputs 0))] := by
  have hout : ∀ output, output ∈ eqPruneOutputs → output < eqPruneGraph.arity := by
    intro output houtput
    change output ∈ [3, 4] at houtput
    change output < 5
    simp at houtput
    omega
  have hsound := residualize_outputs_sound eqPruneGraph equalStatic eqPruneOutputs
    eqPruneGraph_wellFormed hout equalPruneMap inputs
  calc
    outputValues equalResidual inputs =
        eqPruneOutputs.map (fun output =>
          PackedSchedule.value eqPruneGraph (merge equalStatic inputs) output) := by
          simpa only [equalResidual, equalFoldedGraph] using hsound
    _ = [some (.boolean true), some (.i64 (inputs 0 + inputs 0))] := by
      simp [eqPruneOutputs, eqPruneGraph, PackedSchedule.value,
        PackedSchedule.valuesUpTo, PackedSchedule.evalStep, merge, equalStatic, rank]

theorem rust_fixture_unequal_values (inputs : Nat → BitVec 64) :
    outputValues unequalResidual inputs =
      [some (.boolean false), some (.i64 (inputs 0 + inputs 0))] := by
  have hout : ∀ output, output ∈ eqPruneOutputs → output < eqPruneGraph.arity := by
    intro output houtput
    change output ∈ [3, 4] at houtput
    change output < 5
    simp at houtput
    omega
  have hsound := residualize_outputs_sound eqPruneGraph unequalStatic eqPruneOutputs
    eqPruneGraph_wellFormed hout unequalPruneMap inputs
  calc
    outputValues unequalResidual inputs =
        eqPruneOutputs.map (fun output =>
          PackedSchedule.value eqPruneGraph (merge unequalStatic inputs) output) := by
          simpa only [unequalResidual, unequalFoldedGraph] using hsound
    _ = [some (.boolean false), some (.i64 (inputs 0 + inputs 0))] := by
      simp [eqPruneOutputs, eqPruneGraph, PackedSchedule.value,
        PackedSchedule.valuesUpTo, PackedSchedule.evalStep, merge, unequalStatic, rank]

end NobleFoundation.SpecializeResidual
