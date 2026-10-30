/-
  Identity forwarding by operand rewriting (issue #34).

  The graph IR has no alias node, so an identity node (`1·v`, `v·1`, `v + 0`, `0 + v`, `v − 0`) cannot
  be replaced by `v` in place. Forwarding does not need one: it rewrites every consumer's operand
  from an identity node to the node that identity forwards to, following chains of identities, and
  forwards the output the same way. Node indices and the arity are unchanged; the identity nodes stay
  in place and become dead.

  - `forward_sound`: on well-formed graphs without equality nodes every node keeps its value, and the
    forwarded output has the original output's value (`forward_output_sound`).
  - Applied twice to #34's first-projection residuals, the forwarded residual still computes the
    program (`forwarded_residual_sound`), no live multiplication reads a former opcode slot
    (`forwarding_removes_the_dispatch_overhead`), and its live part is exactly the program: the two
    dynamic inputs and the two selected operations (`forwarded_residual_is_the_program`). One pass
    is not enough when `mul` is selected (`one_pass_leaves_a_zero_sum`).
  - Controls: chains of identities are followed (`chain_forwards_to_the_input`), and `0 − v`, a
    negation rather than an identity, is not forwarded (`subtraction_from_zero_is_not_forwarded`).

  Nothing here is a Rust, Wasm or runtime correspondence claim.
-/
import NobleFoundation.FirstProjection

namespace NobleFoundation.OperandForwarding

open NobleFoundation.PackedSchedule
open NobleFoundation.GraphRewrite
open NobleFoundation.Specialize
open NobleFoundation.FirstProjection

/-! ## Forwarding targets -/

/-- The operand an identity node forwards to, if the node is an identity. -/
def identityOperand (graph : Graph) (index : Nat) : Option Nat :=
  match graph.node index with
  | .mul first second =>
      if graph.node first = .constant 1 then some second
      else if graph.node second = .constant 1 then some first
      else none
  | .add first second =>
      if graph.node second = .constant 0 then some first
      else if graph.node first = .constant 0 then some second
      else none
  | .sub first second => if graph.node second = .constant 0 then some first else none
  | _ => none

/-- Follow identity nodes towards earlier nodes, for at most `fuel` steps. -/
def targetFuel (graph : Graph) : Nat → Nat → Nat
  | 0, index => index
  | fuel + 1, index =>
      match identityOperand graph index with
      | some next => if next < index then targetFuel graph fuel next else index
      | none => index

/-- The forwarding target of a node. `index + 1` steps suffice, since each step reaches an earlier
node. -/
def target (graph : Graph) (index : Nat) : Nat := targetFuel graph (index + 1) index

/-- Forward the operands of one node. -/
def forwardNode (graph : Graph) (index : Nat) : Node :=
  match graph.node index with
  | .add first second => .add (target graph first) (target graph second)
  | .sub first second => .sub (target graph first) (target graph second)
  | .mul first second => .mul (target graph first) (target graph second)
  | .eq first second => .eq (target graph first) (target graph second)
  | other => other

/-- Forward every operand and the output. Node indices and the arity are unchanged. -/
def forward (graph : Graph) : Graph :=
  { arity := graph.arity, output := target graph graph.output, node := forwardNode graph }

/-! ## Soundness -/

theorem targetFuel_le (graph : Graph) : ∀ fuel index, targetFuel graph fuel index ≤ index := by
  intro fuel
  induction fuel with
  | zero =>
      intro index
      simp [targetFuel]
  | succ fuel ih =>
      intro index
      simp only [targetFuel]
      split
      · rename_i next _
        split
        · rename_i earlier
          exact Nat.le_trans (ih next) (Nat.le_of_lt earlier)
        · exact Nat.le_refl _
      · exact Nat.le_refl _

/-- A forwarding target is never later than the node. -/
theorem target_le (graph : Graph) (index : Nat) : target graph index ≤ index :=
  targetFuel_le graph (index + 1) index

/-- **An identity node has its forwarding operand's value.** -/
theorem identity_value (graph : Graph) (hwf : WellFormed graph) (noEq : NoEq graph)
    (inputs : Nat → BitVec 64) (index next : Nat) (identity : identityOperand graph index = some next) :
    value graph inputs index = value graph inputs next := by
  unfold identityOperand at identity
  cases hnode : graph.node index with
  | mul first second =>
      rw [hnode] at identity
      dsimp only at identity
      obtain ⟨x, hx⟩ := value_i64_of_noEq graph hwf noEq inputs first
      obtain ⟨y, hy⟩ := value_i64_of_noEq graph hwf noEq inputs second
      rw [value_mul graph hwf inputs index first second hnode x y hx hy]
      by_cases one : graph.node first = .constant 1
      · rw [if_pos one] at identity
        cases identity
        rw [constant_value graph inputs first 1 one] at hx
        cases hx
        rw [hy]
        simp
      · rw [if_neg one] at identity
        by_cases other : graph.node second = .constant 1
        · rw [if_pos other] at identity
          cases identity
          rw [constant_value graph inputs second 1 other] at hy
          cases hy
          rw [hx]
          simp
        · rw [if_neg other] at identity
          cases identity
  | add first second =>
      rw [hnode] at identity
      dsimp only at identity
      obtain ⟨x, hx⟩ := value_i64_of_noEq graph hwf noEq inputs first
      obtain ⟨y, hy⟩ := value_i64_of_noEq graph hwf noEq inputs second
      rw [value_add graph hwf inputs index first second hnode x y hx hy]
      by_cases zero : graph.node second = .constant 0
      · rw [if_pos zero] at identity
        cases identity
        rw [constant_value graph inputs second 0 zero] at hy
        cases hy
        rw [hx]
        simp
      · rw [if_neg zero] at identity
        by_cases other : graph.node first = .constant 0
        · rw [if_pos other] at identity
          cases identity
          rw [constant_value graph inputs first 0 other] at hx
          cases hx
          rw [hy]
          simp
        · rw [if_neg other] at identity
          cases identity
  | sub first second =>
      rw [hnode] at identity
      dsimp only at identity
      obtain ⟨x, hx⟩ := value_i64_of_noEq graph hwf noEq inputs first
      obtain ⟨y, hy⟩ := value_i64_of_noEq graph hwf noEq inputs second
      rw [value_sub graph hwf inputs index first second hnode x y hx hy]
      by_cases zero : graph.node second = .constant 0
      · rw [if_pos zero] at identity
        cases identity
        rw [constant_value graph inputs second 0 zero] at hy
        cases hy
        rw [hx]
        simp
      · rw [if_neg zero] at identity
        cases identity
  | input slot =>
      rw [hnode] at identity
      cases identity
  | constant bits =>
      rw [hnode] at identity
      cases identity
  | eq first second => exact absurd hnode (noEq index first second)

theorem targetFuel_value (graph : Graph) (hwf : WellFormed graph) (noEq : NoEq graph)
    (inputs : Nat → BitVec 64) :
    ∀ fuel index, value graph inputs (targetFuel graph fuel index) = value graph inputs index := by
  intro fuel
  induction fuel with
  | zero =>
      intro index
      rfl
  | succ fuel ih =>
      intro index
      simp only [targetFuel]
      split
      · rename_i next identity
        split
        · rw [ih next, identity_value graph hwf noEq inputs index next identity]
        · rfl
      · rfl

/-- **A forwarding target has the node's value.** -/
theorem target_value (graph : Graph) (hwf : WellFormed graph) (noEq : NoEq graph)
    (inputs : Nat → BitVec 64) (index : Nat) :
    value graph inputs (target graph index) = value graph inputs index :=
  targetFuel_value graph hwf noEq inputs (index + 1) index

/-- Forwarding keeps every operand strictly earlier and the output inside the arity. -/
theorem forward_wellFormed (graph : Graph) (hwf : WellFormed graph) : WellFormed (forward graph) := by
  refine ⟨Nat.lt_of_le_of_lt (target_le graph graph.output) hwf.1, ?_⟩
  intro index operand member
  change operand ∈ (forwardNode graph index).operands at member
  unfold forwardNode at member
  have earlier : ∀ original, original ∈ (graph.node index).operands → target graph original < index :=
    fun original isOperand => Nat.lt_of_le_of_lt (target_le graph original) (hwf.2 index original isOperand)
  cases hnode : graph.node index with
  | add first second =>
      rw [hnode] at member
      simp only [Node.operands, List.mem_cons, List.not_mem_nil, or_false] at member
      rcases member with same | same <;> rw [same] <;> apply earlier <;> simp [hnode, Node.operands]
  | sub first second =>
      rw [hnode] at member
      simp only [Node.operands, List.mem_cons, List.not_mem_nil, or_false] at member
      rcases member with same | same <;> rw [same] <;> apply earlier <;> simp [hnode, Node.operands]
  | mul first second =>
      rw [hnode] at member
      simp only [Node.operands, List.mem_cons, List.not_mem_nil, or_false] at member
      rcases member with same | same <;> rw [same] <;> apply earlier <;> simp [hnode, Node.operands]
  | eq first second =>
      rw [hnode] at member
      simp only [Node.operands, List.mem_cons, List.not_mem_nil, or_false] at member
      rcases member with same | same <;> rw [same] <;> apply earlier <;> simp [hnode, Node.operands]
  | input slot =>
      rw [hnode] at member
      simp [Node.operands] at member
  | constant bits =>
      rw [hnode] at member
      simp [Node.operands] at member

/-- Forwarding introduces no equality node. -/
theorem forward_noEq (graph : Graph) (noEq : NoEq graph) : NoEq (forward graph) := by
  intro index first second hnode
  change forwardNode graph index = .eq first second at hnode
  unfold forwardNode at hnode
  cases horig : graph.node index with
  | eq a b => exact noEq index a b horig
  | add a b =>
      rw [horig] at hnode
      simp at hnode
  | sub a b =>
      rw [horig] at hnode
      simp at hnode
  | mul a b =>
      rw [horig] at hnode
      simp at hnode
  | input slot =>
      rw [horig] at hnode
      simp at hnode
  | constant bits =>
      rw [horig] at hnode
      simp at hnode

/-- **Forwarding is sound** on well-formed graphs without equality nodes: every node keeps its
value. -/
theorem forward_sound (graph : Graph) (hwf : WellFormed graph) (noEq : NoEq graph)
    (inputs : Nat → BitVec 64) : ∀ index, value (forward graph) inputs index = value graph inputs index := by
  have hwfF := forward_wellFormed graph hwf
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      have operandValue : ∀ operand, operand < index →
          value (forward graph) inputs (target graph operand) = value graph inputs operand := by
        intro operand earlier
        rw [ih (target graph operand) (Nat.lt_of_le_of_lt (target_le graph operand) earlier)]
        exact target_value graph hwf noEq inputs operand
      cases hnode : graph.node index with
      | input slot =>
          have same : (forward graph).node index = .input slot := by
            change forwardNode graph index = _
            unfold forwardNode
            rw [hnode]
          rw [value_input (forward graph) inputs index slot same, value_input graph inputs index slot hnode]
      | constant bits =>
          have same : (forward graph).node index = .constant bits := by
            change forwardNode graph index = _
            unfold forwardNode
            rw [hnode]
          rw [constant_value (forward graph) inputs index bits same,
            constant_value graph inputs index bits hnode]
      | add first second =>
          have rewritten : (forward graph).node index = .add (target graph first) (target graph second) := by
            change forwardNode graph index = _
            unfold forwardNode
            rw [hnode]
          obtain ⟨x, hx⟩ := value_i64_of_noEq graph hwf noEq inputs first
          obtain ⟨y, hy⟩ := value_i64_of_noEq graph hwf noEq inputs second
          have fx := (operandValue first (hwf.2 index first (by simp [hnode, Node.operands]))).trans hx
          have fy := (operandValue second (hwf.2 index second (by simp [hnode, Node.operands]))).trans hy
          rw [value_add (forward graph) hwfF inputs index _ _ rewritten x y fx fy,
            value_add graph hwf inputs index first second hnode x y hx hy]
      | sub first second =>
          have rewritten : (forward graph).node index = .sub (target graph first) (target graph second) := by
            change forwardNode graph index = _
            unfold forwardNode
            rw [hnode]
          obtain ⟨x, hx⟩ := value_i64_of_noEq graph hwf noEq inputs first
          obtain ⟨y, hy⟩ := value_i64_of_noEq graph hwf noEq inputs second
          have fx := (operandValue first (hwf.2 index first (by simp [hnode, Node.operands]))).trans hx
          have fy := (operandValue second (hwf.2 index second (by simp [hnode, Node.operands]))).trans hy
          rw [value_sub (forward graph) hwfF inputs index _ _ rewritten x y fx fy,
            value_sub graph hwf inputs index first second hnode x y hx hy]
      | mul first second =>
          have rewritten : (forward graph).node index = .mul (target graph first) (target graph second) := by
            change forwardNode graph index = _
            unfold forwardNode
            rw [hnode]
          obtain ⟨x, hx⟩ := value_i64_of_noEq graph hwf noEq inputs first
          obtain ⟨y, hy⟩ := value_i64_of_noEq graph hwf noEq inputs second
          have fx := (operandValue first (hwf.2 index first (by simp [hnode, Node.operands]))).trans hx
          have fy := (operandValue second (hwf.2 index second (by simp [hnode, Node.operands]))).trans hy
          rw [value_mul (forward graph) hwfF inputs index _ _ rewritten x y fx fy,
            value_mul graph hwf inputs index first second hnode x y hx hy]
      | eq first second => exact absurd hnode (noEq index first second)

/-- **The forwarded output has the original output's value.** -/
theorem forward_output_sound (graph : Graph) (hwf : WellFormed graph) (noEq : NoEq graph)
    (inputs : Nat → BitVec 64) :
    value (forward graph) inputs (forward graph).output = value graph inputs graph.output := by
  change value (forward graph) inputs (target graph graph.output) = _
  rw [forward_sound graph hwf noEq inputs, target_value graph hwf noEq inputs]

/-! ## The first-projection residual, forwarded -/

/-- Zero annihilation introduces no equality node. -/
theorem annihilate_noEq (graph : Graph) (noEq : NoEq graph) : NoEq (annihilate graph) := by
  intro index first second hnode
  change annihilateNode graph index = .eq first second at hnode
  unfold annihilateNode at hnode
  cases horig : graph.node index with
  | eq a b => exact noEq index a b horig
  | mul a b =>
      rw [horig] at hnode
      dsimp only at hnode
      split at hnode <;> simp at hnode
  | add a b =>
      rw [horig] at hnode
      simp at hnode
  | sub a b =>
      rw [horig] at hnode
      simp at hnode
  | input slot =>
      rw [horig] at hnode
      simp at hnode
  | constant bits =>
      rw [horig] at hnode
      simp at hnode

/-- The forwarded residual of one program: specialization, zero annihilation, then two forwarding
passes. One pass forwards the identities visible in the residual; a sum whose operand becomes the
constant `0` only once its own operands are forwarded needs the second. -/
def forwardedResidual (program : Program) : Graph := forward (forward (residual program))

/-- **The forwarded residual still computes the program**, for every program and every dynamic
input. -/
theorem forwarded_residual_sound (program : Program) (d : Nat → BitVec 64) :
    value (forwardedResidual program) d (forwardedResidual program).output =
      some (.i64 (refEval program (d 0) (d 1))) := by
  have specialized :=
    specialize_wellFormed interpreterGraph (staticOf program) interpreterGraph_wellFormed
  have hwf : WellFormed (residual program) := annihilate_wellFormed _ specialized
  have noEq : NoEq (residual program) :=
    annihilate_noEq _ (specialize_noEq interpreterGraph (staticOf program) interpreterGraph_noEq)
  rw [forwardedResidual, forward_output_sound (forward (residual program)) (forward_wellFormed _ hwf)
    (forward_noEq _ noEq) d, forward_output_sound (residual program) hwf noEq d]
  exact residual_sound program d

/-- The operation node an opcode selects, over two operand indices. -/
def opNode : Op → Nat → Nat → Node
  | .add => .add
  | .sub => .sub
  | .mul => .mul

/-- The interpreter's first-instruction operation node for an opcode. -/
def firstOpIndex : Op → Nat
  | .add => 8
  | .sub => 9
  | .mul => 10

/-- **Forwarding removes the dispatch overhead**: for every program no live multiplication reads a
former opcode slot, where the residual keeps two. -/
theorem forwarding_removes_the_dispatch_overhead (first second : Op) :
    dispatchProducts (forwardedResidual (first, second)) = [] ∧
      (dispatchProducts (residual (first, second))).length = 2 := by
  cases first <;> cases second <;> decide

/-- **The forwarded residual is the program**: for every program its live part is exactly the two
dynamic inputs `x` (node 6) and `y` (node 7), the first selected operation over `x, y` and the second
over that result and `y`, which is the output. A third pass changes no live node. -/
theorem forwarded_residual_is_the_program (first second : Op) :
    live (forwardedResidual (first, second)) =
        [(forwardedResidual (first, second)).output, firstOpIndex first, 7, 6] ∧
      (forwardedResidual (first, second)).node 6 = .input 0 ∧
      (forwardedResidual (first, second)).node 7 = .input 1 ∧
      (forwardedResidual (first, second)).node (firstOpIndex first) = opNode first 6 7 ∧
      (forwardedResidual (first, second)).node (forwardedResidual (first, second)).output =
        opNode second (firstOpIndex first) 7 ∧
      live (forward (forwardedResidual (first, second))) = live (forwardedResidual (first, second)) := by
  cases first <;> cases second <;> decide

/-- **One pass is not enough when `mul` is selected**: after one pass the residual of `(add, mul)`
still has six live nodes, including the sum of a constant `0` and the selected product. -/
theorem one_pass_leaves_a_zero_sum :
    (live (forward (residual (.add, .mul)))).length = 6 ∧
      (forward (residual (.add, .mul))).node 23 = .add 19 18 ∧
      (forward (residual (.add, .mul))).node 19 = .constant 0 ∧
      (forward (residual (.add, .mul))).node 18 = .mul 8 7 := by
  decide

/-! ## Controls -/

/-- `((1·x) + 0)·1`: a chain of three identities over the input `x`. -/
def chainGraph : Graph :=
  ofList [.input 0, .constant 1, .mul 1 0, .constant 0, .add 2 3, .mul 4 1] 5

/-- **Chains are followed**: the output forwards through all three identities to the input. -/
theorem chain_forwards_to_the_input : target chainGraph 5 = 0 ∧ (forward chainGraph).output = 0 := by
  decide

/-- `0 − x`: a negation, which is not an identity. -/
def negationGraph : Graph := ofList [.constant 0, .input 0, .sub 0 1] 2

/-- **Subtraction from zero is not forwarded**: `0 − x` is a negation and keeps its node, and
forwarding it to `x` would change the value at `x = 1`. -/
theorem subtraction_from_zero_is_not_forwarded :
    identityOperand negationGraph 2 = none ∧ (forward negationGraph).output = 2 ∧
      value negationGraph (fun _ => 1) 2 ≠ value negationGraph (fun _ => 1) 1 := by
  decide

end NobleFoundation.OperandForwarding
