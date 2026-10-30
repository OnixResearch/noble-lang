/-
  A first Futamura projection for one restricted source family (issue #34, phase 1).

  The source family is a fixed-length bytecode of two instructions over an accumulator. Each
  instruction applies `add`, `sub` or `mul` to the accumulator and a second dynamic value `y`; the
  accumulator starts at a dynamic value `x`. An instruction's opcode is encoded one-hot as three static
  I64 coefficients, and the interpreter is one `PackedSchedule.Graph` that computes
  `c_add·(a + y) + c_sub·(a − y) + c_mul·(a·y)` per instruction, with no branch. This is a deliberately
  restricted family, not a general interpreter.

  Specializing the interpreter to the static opcode slots (#32's `specialize`) yields a residual graph
  over the dynamic inputs `x, y` that computes the source program's meaning: that is the first
  projection, proved for every program in the family and every dynamic input.

  Dispatch removal is bounded by this graph IR. It has no alias node, so an identity rewrite
  (`1·v → v`, `v + 0 → v`) cannot be applied in place; the certificate format records identity steps
  without applying them for the same reason. What can be applied in place is zero annihilation
  (`0·v → 0`), which is proved sound here and makes every unselected branch dead. The selected branch
  keeps a multiplication by the constant `1` and additions of the constant `0`; that overhead is
  stated, not hidden. Nothing here is a Rust, Wasm or runtime correspondence claim.
-/
import NobleFoundation.PackedSchedule
import NobleFoundation.GraphRewrite
import NobleFoundation.Specialize

namespace NobleFoundation.FirstProjection

open NobleFoundation.PackedSchedule
open NobleFoundation.GraphRewrite
open NobleFoundation.Specialize

/-! ## Graphs from node lists -/

/-- A graph whose nodes are a finite list; indices past the list are the constant `0`. -/
def ofList (nodes : List Node) (out : Nat) : Graph where
  arity := nodes.length
  output := out
  node := fun index => nodes.getD index (.constant 0)

/-- Every listed node's operands are strictly earlier. -/
def operandsEarlier (nodes : List Node) : Bool :=
  (List.range nodes.length).all fun index =>
    ((nodes.getD index (.constant 0)).operands).all (· < index)

/-- A checked node list with an in-range output is a well-formed graph. -/
theorem ofList_wellFormed (nodes : List Node) (out : Nat) (outInRange : out < nodes.length)
    (earlier : operandsEarlier nodes = true) : WellFormed (ofList nodes out) := by
  refine ⟨outInRange, ?_⟩
  intro index operand member
  by_cases inRange : index < nodes.length
  · have check := List.all_eq_true.mp earlier index (List.mem_range.mpr inRange)
    have below := List.all_eq_true.mp check operand member
    simpa using below
  · have past : nodes[index]? = none := List.getElem?_eq_none (by omega)
    simp [ofList, List.getD_eq_getElem?_getD, past, Node.operands] at member

/-- A graph has no equality node; then every value is an `I64`. -/
def NoEq (graph : Graph) : Prop := ∀ index first second, graph.node index ≠ .eq first second

/-- A node is not an equality node. -/
def notEq : Node → Bool
  | .eq _ _ => false
  | _ => true

/-- A checked node list without equality nodes gives a graph without equality nodes. -/
theorem ofList_noEq (nodes : List Node) (out : Nat) (checked : nodes.all notEq = true) :
    NoEq (ofList nodes out) := by
  intro index first second hnode
  simp only [ofList, List.getD_eq_getElem?_getD] at hnode
  cases lookup : nodes[index]? with
  | none => simp [lookup] at hnode
  | some node =>
      rw [lookup, Option.getD_some] at hnode
      have member : node ∈ nodes := List.mem_of_getElem? lookup
      have isNotEq := List.all_eq_true.mp checked node member
      rw [hnode] at isNotEq
      simp [notEq] at isNotEq

/-! ## Value of one node from its operands' values -/

theorem value_input (graph : Graph) (inputs : Nat → BitVec 64) (index slot : Nat)
    (hnode : graph.node index = .input slot) :
    value graph inputs index = some (.i64 (inputs slot)) := by
  simp [value, valuesUpTo, evalStep, hnode]

/-- The value of an operand read inside the prefix of an earlier-operand node. -/
theorem operand_value (graph : Graph) (hwf : WellFormed graph) (inputs : Nat → BitVec 64)
    (index operand : Nat) (member : operand ∈ (graph.node index).operands) :
    valuesUpTo graph inputs index operand = value graph inputs operand :=
  valuesUpTo_eq_value graph inputs index operand (hwf.2 index operand member)

theorem value_add (graph : Graph) (hwf : WellFormed graph) (inputs : Nat → BitVec 64)
    (index first second : Nat) (hnode : graph.node index = .add first second) (x y : BitVec 64)
    (hx : value graph inputs first = some (.i64 x)) (hy : value graph inputs second = some (.i64 y)) :
    value graph inputs index = some (.i64 (x + y)) := by
  have fx : valuesUpTo graph inputs index first = some (.i64 x) := by
    rw [operand_value graph hwf inputs index first (by simp [hnode, Node.operands])]
    exact hx
  have fy : valuesUpTo graph inputs index second = some (.i64 y) := by
    rw [operand_value graph hwf inputs index second (by simp [hnode, Node.operands])]
    exact hy
  simp [value, valuesUpTo, evalStep, hnode, fx, fy]

theorem value_sub (graph : Graph) (hwf : WellFormed graph) (inputs : Nat → BitVec 64)
    (index first second : Nat) (hnode : graph.node index = .sub first second) (x y : BitVec 64)
    (hx : value graph inputs first = some (.i64 x)) (hy : value graph inputs second = some (.i64 y)) :
    value graph inputs index = some (.i64 (x - y)) := by
  have fx : valuesUpTo graph inputs index first = some (.i64 x) := by
    rw [operand_value graph hwf inputs index first (by simp [hnode, Node.operands])]
    exact hx
  have fy : valuesUpTo graph inputs index second = some (.i64 y) := by
    rw [operand_value graph hwf inputs index second (by simp [hnode, Node.operands])]
    exact hy
  simp [value, valuesUpTo, evalStep, hnode, fx, fy]

theorem value_mul (graph : Graph) (hwf : WellFormed graph) (inputs : Nat → BitVec 64)
    (index first second : Nat) (hnode : graph.node index = .mul first second) (x y : BitVec 64)
    (hx : value graph inputs first = some (.i64 x)) (hy : value graph inputs second = some (.i64 y)) :
    value graph inputs index = some (.i64 (x * y)) := by
  have fx : valuesUpTo graph inputs index first = some (.i64 x) := by
    rw [operand_value graph hwf inputs index first (by simp [hnode, Node.operands])]
    exact hx
  have fy : valuesUpTo graph inputs index second = some (.i64 y) := by
    rw [operand_value graph hwf inputs index second (by simp [hnode, Node.operands])]
    exact hy
  simp [value, valuesUpTo, evalStep, hnode, fx, fy]

/-- In a well-formed graph without equality nodes every node has an `I64` value. -/
theorem value_i64_of_noEq (graph : Graph) (hwf : WellFormed graph) (noEq : NoEq graph)
    (inputs : Nat → BitVec 64) : ∀ index, ∃ bits, value graph inputs index = some (.i64 bits) := by
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      cases hnode : graph.node index with
      | input slot => exact ⟨_, value_input graph inputs index slot hnode⟩
      | constant bits => exact ⟨_, constant_value graph inputs index bits hnode⟩
      | add first second =>
          obtain ⟨x, hx⟩ := ih first (hwf.2 index first (by simp [hnode, Node.operands]))
          obtain ⟨y, hy⟩ := ih second (hwf.2 index second (by simp [hnode, Node.operands]))
          exact ⟨_, value_add graph hwf inputs index first second hnode x y hx hy⟩
      | sub first second =>
          obtain ⟨x, hx⟩ := ih first (hwf.2 index first (by simp [hnode, Node.operands]))
          obtain ⟨y, hy⟩ := ih second (hwf.2 index second (by simp [hnode, Node.operands]))
          exact ⟨_, value_sub graph hwf inputs index first second hnode x y hx hy⟩
      | mul first second =>
          obtain ⟨x, hx⟩ := ih first (hwf.2 index first (by simp [hnode, Node.operands]))
          obtain ⟨y, hy⟩ := ih second (hwf.2 index second (by simp [hnode, Node.operands]))
          exact ⟨_, value_mul graph hwf inputs index first second hnode x y hx hy⟩
      | eq first second => exact absurd hnode (noEq index first second)

/-! ## The source family and its reference -/

/-- The three opcodes of the source family. -/
inductive Op where
  | add
  | sub
  | mul
  deriving DecidableEq, Repr

/-- One opcode's meaning on the accumulator and the second value. -/
def apply : Op → BitVec 64 → BitVec 64 → BitVec 64
  | .add, a, y => a + y
  | .sub, a, y => a - y
  | .mul, a, y => a * y

/-- A two-instruction program. -/
abbrev Program := Op × Op

/-- The independent reference: run both instructions over the accumulator, starting at `x`. -/
def refEval (program : Program) (x y : BitVec 64) : BitVec 64 :=
  apply program.2 (apply program.1 x y) y

/-- The one-hot encoding: `(c_add, c_sub, c_mul)`. -/
def coeff : Op → BitVec 64 × BitVec 64 × BitVec 64
  | .add => (1, 0, 0)
  | .sub => (0, 1, 0)
  | .mul => (0, 0, 1)

/-- One interpreted instruction as an expression over its coefficients. -/
def instrValue (cAdd cSub cMul a y : BitVec 64) : BitVec 64 :=
  cAdd * (a + y) + cSub * (a - y) + cMul * (a * y)

/-! ## The interpreter graph

Slots `0..2` and `3..5` hold the two instructions' one-hot coefficients (static); slot `6` is `x` and
slot `7` is `y` (dynamic). Nodes `8..15` interpret the first instruction and nodes `16..23` the second;
node `23` is the output. -/

def interpreterNodes : List Node :=
  [.input 0, .input 1, .input 2, .input 3, .input 4, .input 5, .input 6, .input 7,
   .add 6 7, .sub 6 7, .mul 6 7, .mul 0 8, .mul 1 9, .mul 2 10, .add 11 12, .add 14 13,
   .add 15 7, .sub 15 7, .mul 15 7, .mul 3 16, .mul 4 17, .mul 5 18, .add 19 20, .add 22 21]

def interpreterGraph : Graph := ofList interpreterNodes 23

theorem interpreterGraph_wellFormed : WellFormed interpreterGraph :=
  ofList_wellFormed interpreterNodes 23 (by decide) (by decide)

theorem interpreterGraph_noEq : NoEq interpreterGraph :=
  ofList_noEq interpreterNodes 23 (by decide)

/-- **The interpreter's value**, for every input assignment: two interpreted instructions, the second
over the first's result. -/
theorem interpreter_value (inputs : Nat → BitVec 64) :
    value interpreterGraph inputs interpreterGraph.output =
      some (.i64 (instrValue (inputs 3) (inputs 4) (inputs 5)
        (instrValue (inputs 0) (inputs 1) (inputs 2) (inputs 6) (inputs 7)) (inputs 7))) := by
  have hwf := interpreterGraph_wellFormed
  have v0 := value_input interpreterGraph inputs 0 0 rfl
  have v1 := value_input interpreterGraph inputs 1 1 rfl
  have v2 := value_input interpreterGraph inputs 2 2 rfl
  have v3 := value_input interpreterGraph inputs 3 3 rfl
  have v4 := value_input interpreterGraph inputs 4 4 rfl
  have v5 := value_input interpreterGraph inputs 5 5 rfl
  have v6 := value_input interpreterGraph inputs 6 6 rfl
  have v7 := value_input interpreterGraph inputs 7 7 rfl
  have v8 := value_add interpreterGraph hwf inputs 8 6 7 rfl _ _ v6 v7
  have v9 := value_sub interpreterGraph hwf inputs 9 6 7 rfl _ _ v6 v7
  have v10 := value_mul interpreterGraph hwf inputs 10 6 7 rfl _ _ v6 v7
  have v11 := value_mul interpreterGraph hwf inputs 11 0 8 rfl _ _ v0 v8
  have v12 := value_mul interpreterGraph hwf inputs 12 1 9 rfl _ _ v1 v9
  have v13 := value_mul interpreterGraph hwf inputs 13 2 10 rfl _ _ v2 v10
  have v14 := value_add interpreterGraph hwf inputs 14 11 12 rfl _ _ v11 v12
  have v15 := value_add interpreterGraph hwf inputs 15 14 13 rfl _ _ v14 v13
  have v16 := value_add interpreterGraph hwf inputs 16 15 7 rfl _ _ v15 v7
  have v17 := value_sub interpreterGraph hwf inputs 17 15 7 rfl _ _ v15 v7
  have v18 := value_mul interpreterGraph hwf inputs 18 15 7 rfl _ _ v15 v7
  have v19 := value_mul interpreterGraph hwf inputs 19 3 16 rfl _ _ v3 v16
  have v20 := value_mul interpreterGraph hwf inputs 20 4 17 rfl _ _ v4 v17
  have v21 := value_mul interpreterGraph hwf inputs 21 5 18 rfl _ _ v5 v18
  have v22 := value_add interpreterGraph hwf inputs 22 19 20 rfl _ _ v19 v20
  exact value_add interpreterGraph hwf inputs 23 22 21 rfl _ _ v22 v21

/-- A one-hot instruction is exactly its opcode's meaning. -/
theorem instrValue_coeff (op : Op) (a y : BitVec 64) :
    instrValue (coeff op).1 (coeff op).2.1 (coeff op).2.2 a y = apply op a y := by
  cases op <;> simp [instrValue, coeff, apply, BitVec.zero_mul]

/-- The full input assignment for a program and its dynamic values. -/
def inputsOf (program : Program) (x y : BitVec 64) : Nat → BitVec 64
  | 0 => (coeff program.1).1
  | 1 => (coeff program.1).2.1
  | 2 => (coeff program.1).2.2
  | 3 => (coeff program.2).1
  | 4 => (coeff program.2).2.1
  | 5 => (coeff program.2).2.2
  | 6 => x
  | 7 => y
  | _ => 0

/-- **The interpreter agrees with the reference** for every program in the family and every dynamic
input. -/
theorem interpreter_agrees (program : Program) (x y : BitVec 64) :
    value interpreterGraph (inputsOf program x y) interpreterGraph.output =
      some (.i64 (refEval program x y)) := by
  rw [interpreter_value]
  simp only [inputsOf, refEval, instrValue_coeff]

/-! ## First projection -/

/-- The static half of a program: its six opcode coefficients. `x` and `y` stay dynamic. -/
def staticOf (program : Program) : Nat → Option (BitVec 64)
  | 0 => some (coeff program.1).1
  | 1 => some (coeff program.1).2.1
  | 2 => some (coeff program.1).2.2
  | 3 => some (coeff program.2).1
  | 4 => some (coeff program.2).2.1
  | 5 => some (coeff program.2).2.2
  | _ => none

/-- Under the static program, the merged input assignment is the program's full assignment with the
dynamic inputs renumbered: `x` is dynamic slot `0` and `y` is dynamic slot `1`. -/
theorem merge_staticOf (program : Program) (d : Nat → BitVec 64) :
    merge (staticOf program) d 0 = (coeff program.1).1 ∧
      merge (staticOf program) d 1 = (coeff program.1).2.1 ∧
      merge (staticOf program) d 2 = (coeff program.1).2.2 ∧
      merge (staticOf program) d 3 = (coeff program.2).1 ∧
      merge (staticOf program) d 4 = (coeff program.2).2.1 ∧
      merge (staticOf program) d 5 = (coeff program.2).2.2 ∧
      merge (staticOf program) d 6 = d 0 ∧ merge (staticOf program) d 7 = d 1 := by
  obtain ⟨first, second⟩ := program
  cases first <;> cases second <;> exact ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

/-- **The first projection**: specializing the interpreter to a program's static opcodes gives a
residual graph that computes that program on the dynamic inputs, for every program in the family and
every dynamic input. -/
theorem first_projection (program : Program) (d : Nat → BitVec 64) :
    value (specialize interpreterGraph (staticOf program)) d interpreterGraph.output =
      some (.i64 (refEval program (d 0) (d 1))) := by
  rw [specialize_sound interpreterGraph (staticOf program) interpreterGraph_wellFormed d,
    interpreter_value]
  obtain ⟨m0, m1, m2, m3, m4, m5, m6, m7⟩ := merge_staticOf program d
  rw [m0, m1, m2, m3, m4, m5, m6, m7]
  simp only [refEval, instrValue_coeff]

/-! ## Zero annihilation: the dispatch-removal step this IR can express -/

/-- Rewrite a multiplication by the constant `0` to the constant `0`, in place. -/
def annihilateNode (graph : Graph) (index : Nat) : Node :=
  match graph.node index with
  | .mul first second =>
      if graph.node first = .constant 0 ∨ graph.node second = .constant 0 then .constant 0
      else .mul first second
  | other => other

/-- Zero annihilation with node indices, arity and output unchanged. -/
def annihilate (graph : Graph) : Graph := { graph with node := annihilateNode graph }

theorem annihilate_wellFormed (graph : Graph) (hwf : WellFormed graph) :
    WellFormed (annihilate graph) := by
  refine ⟨hwf.1, ?_⟩
  intro index operand member
  apply hwf.2 index operand
  change operand ∈ (annihilateNode graph index).operands at member
  unfold annihilateNode at member
  cases hnode : graph.node index with
  | mul first second =>
      rw [hnode] at member
      dsimp only at member
      split at member
      · simp [Node.operands] at member
      · exact member
  | input slot => rw [hnode] at member; exact member
  | constant bits => rw [hnode] at member; exact member
  | add first second => rw [hnode] at member; exact member
  | sub first second => rw [hnode] at member; exact member
  | eq first second => rw [hnode] at member; exact member

/-- **Zero annihilation is sound** on well-formed graphs without equality nodes: every node keeps its
value. -/
theorem annihilate_sound (graph : Graph) (hwf : WellFormed graph) (noEq : NoEq graph)
    (inputs : Nat → BitVec 64) : ∀ index, value (annihilate graph) inputs index = value graph inputs index := by
  have hwfA := annihilate_wellFormed graph hwf
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      by_cases zero : ∃ first second, graph.node index = .mul first second ∧
          (graph.node first = .constant 0 ∨ graph.node second = .constant 0)
      · obtain ⟨first, second, hnode, hzero⟩ := zero
        have rewritten : (annihilate graph).node index = .constant 0 := by
          change annihilateNode graph index = .constant 0
          unfold annihilateNode
          rw [hnode]
          dsimp only
          rw [if_pos hzero]
        rw [constant_value (annihilate graph) inputs index 0 rewritten]
        obtain ⟨x, hx⟩ := value_i64_of_noEq graph hwf noEq inputs first
        obtain ⟨y, hy⟩ := value_i64_of_noEq graph hwf noEq inputs second
        rw [value_mul graph hwf inputs index first second hnode x y hx hy]
        rcases hzero with zeroFirst | zeroSecond
        · rw [constant_value graph inputs first 0 zeroFirst] at hx
          cases hx
          simp [BitVec.zero_mul]
        · rw [constant_value graph inputs second 0 zeroSecond] at hy
          cases hy
          simp [BitVec.mul_zero]
      · have same : (annihilate graph).node index = graph.node index := by
          change annihilateNode graph index = graph.node index
          unfold annihilateNode
          cases hnode : graph.node index with
          | mul first second =>
              dsimp only
              rw [if_neg (fun hzero => zero ⟨first, second, hnode, hzero⟩)]
          | input slot => rfl
          | constant bits => rfl
          | add first second => rfl
          | sub first second => rfl
          | eq first second => rfl
        apply congruence (annihilate graph) graph inputs hwfA hwf index same
        intro operand member
        rw [same] at member
        exact ih operand (hwf.2 index operand member)

/-- `specialize` introduces no equality node. -/
theorem specialize_noEq (graph : Graph) (σ : Nat → Option (BitVec 64)) (noEq : NoEq graph) :
    NoEq (specialize graph σ) := by
  intro index first second hnode
  change specializeNode graph σ index = .eq first second at hnode
  unfold specializeNode at hnode
  cases horig : graph.node index with
  | input slot =>
      rw [horig] at hnode
      dsimp only at hnode
      split at hnode <;> simp at hnode
  | constant bits => rw [horig] at hnode; simp at hnode
  | add a b =>
      rw [horig] at hnode
      dsimp only at hnode
      split at hnode <;> simp at hnode
  | sub a b =>
      rw [horig] at hnode
      dsimp only at hnode
      split at hnode <;> simp at hnode
  | mul a b =>
      rw [horig] at hnode
      dsimp only at hnode
      split at hnode <;> simp at hnode
  | eq a b => exact noEq index a b horig

/-- The residual of one program: specialization followed by zero annihilation. -/
def residual (program : Program) : Graph :=
  annihilate (specialize interpreterGraph (staticOf program))

/-- **The residual still computes the program**, for every program and every dynamic input. -/
theorem residual_sound (program : Program) (d : Nat → BitVec 64) :
    value (residual program) d interpreterGraph.output =
      some (.i64 (refEval program (d 0) (d 1))) := by
  have hwf := specialize_wellFormed interpreterGraph (staticOf program) interpreterGraph_wellFormed
  have noEq := specialize_noEq interpreterGraph (staticOf program) interpreterGraph_noEq
  rw [residual, annihilate_sound _ hwf noEq d]
  exact first_projection program d

/-! ## Dispatch removal, measured on the residual -/

/-- Live nodes, computed backwards from the output in one pass (operands are strictly earlier). -/
def liveFrom (graph : Graph) : Nat → List Nat → List Nat
  | 0, live => live
  | k + 1, live =>
      if k ∈ live then liveFrom graph k (live ++ ((graph.node k).operands.filter (· ∉ live)))
      else liveFrom graph k live

def live (graph : Graph) : List Nat := liveFrom graph graph.arity [graph.output]

/-- **Unselected branches are dead**: for every program, of each instruction's three operation nodes
(`a + y`, `a − y`, `a·y`) exactly the selected one is live in the residual. -/
theorem unselected_branches_are_dead (first second : Op) :
    (8 ∈ live (residual (first, second)) ↔ first = .add) ∧
      (9 ∈ live (residual (first, second)) ↔ first = .sub) ∧
      (10 ∈ live (residual (first, second)) ↔ first = .mul) ∧
      (16 ∈ live (residual (first, second)) ↔ second = .add) ∧
      (17 ∈ live (residual (first, second)) ↔ second = .sub) ∧
      (18 ∈ live (residual (first, second)) ↔ second = .mul) := by
  cases first <;> cases second <;> decide

/-- A live multiplication whose first operand is a former opcode slot (`0..5`). -/
def dispatchProducts (graph : Graph) : List Nat :=
  (live graph).filter fun index =>
    match graph.node index with
    | .mul first _ => decide (first < 6)
    | _ => false

/-- **The remaining overhead, stated**: for every program exactly one live multiplication per
instruction reads a former opcode slot, and that slot holds the constant `1` (the selected
coefficient). Removing it needs an identity rewrite this IR cannot apply in place. -/
theorem remaining_dispatch_overhead (first second : Op) :
    (dispatchProducts (residual (first, second))).length = 2 ∧
      (dispatchProducts (residual (first, second))).all (fun index =>
        match (residual (first, second)).node index with
        | .mul slot _ => (residual (first, second)).node slot == .constant 1
        | _ => false) = true := by
  cases first <;> cases second <;> decide

/-! ## Seeded controls -/

/-- A seeded wrong encoding that maps `sub` to the multiplication coefficient. -/
def wrongCoeff : Op → BitVec 64 × BitVec 64 × BitVec 64
  | .sub => (0, 0, 1)
  | other => coeff other

/-- **Wrong opcode encoding is detected**: under the seeded encoding the interpreter disagrees with the
reference on `(sub, add)` at `x = 7`, `y = 2`. -/
theorem wrong_opcode_encoding_disagrees :
    instrValue (wrongCoeff .add).1 (wrongCoeff .add).2.1 (wrongCoeff .add).2.2
        (instrValue (wrongCoeff .sub).1 (wrongCoeff .sub).2.1 (wrongCoeff .sub).2.2 7 2) 2 ≠
      refEval (.sub, .add) 7 2 := by
  decide

/-- A seeded interpreter with the subtraction operands swapped (`y − a` instead of `a − y`). -/
def swappedNodes : List Node :=
  interpreterNodes.set 9 (.sub 7 6)

def swappedGraph : Graph := ofList swappedNodes 23

/-- **Swapped operands are detected**: the seeded interpreter disagrees with the reference on
`(sub, add)` at `x = 7`, `y = 2`. -/
theorem swapped_operands_disagree :
    value swappedGraph (inputsOf (.sub, .add) 7 2) swappedGraph.output ≠
      some (.i64 (refEval (.sub, .add) 7 2)) := by
  decide

end NobleFoundation.FirstProjection
