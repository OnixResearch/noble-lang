/-
  The transpose identity of the reverse-AD reference over every well-formed graph (issue #45).

  `ReverseAD.lean` proved the transpose pairing one step at a time (`adjointDot_step`) and for one
  fixture. This module proves it for every well-formed graph, every input assignment, every
  direction and every seed vector: after the full reverse sweep, the adjoints of the leaf nodes
  paired with their tangents equal the seed paired with every node's tangent. With a seed on one
  output node, that pairing is the output's forward tangent, which `tangent_is_directional_derivative`
  identifies with the Mathlib derivative, so the reverse sweep computes the derivative.

  The proof keeps a frontier: after `fuel` reverse steps, processed nodes with operands carry no
  weight and every other node carries its tangent. Each step moves one node's adjoint onto its
  operands through the partial-derivative coefficients, and those coefficients paired with the
  operand tangents are exactly the node's tangent (`coefficients_pair_to_the_tangent`), so the
  frontier pairing never changes.

  This is a real-valued model theorem. It says nothing about binary32 gradients, operation order or
  `grad.rs`.
-/
import NobleFoundation.ReverseAD

namespace NobleFoundation.ReverseAD.Transpose

open NobleFoundation.ReverseAD

/-! ## Prefix values and tangents are final once computed -/

theorem valuesUpTo_stable (graph : Graph) (is ip : Nat → ℝ) :
    ∀ count index, index < count → valuesUpTo graph is ip count index = value graph is ip index := by
  intro count
  induction count with
  | zero => intro index below; exact absurd below (Nat.not_lt_zero index)
  | succ count ih =>
      intro index below
      by_cases last : index = count
      · subst last
        rfl
      · simp only [valuesUpTo, if_neg last]
        exact ih index (by omega)

theorem tangentUpTo_stable (graph : Graph) (is ip ds dp : Nat → ℝ) :
    ∀ count index, index < count →
      tangentUpTo graph is ip ds dp count index = tangent graph is ip ds dp index := by
  intro count
  induction count with
  | zero => intro index below; exact absurd below (Nat.not_lt_zero index)
  | succ count ih =>
      intro index below
      by_cases last : index = count
      · subst last
        rfl
      · simp only [tangentUpTo, if_neg last]
        exact ih index (by omega)

/-! ## A node's coefficients pair its operand tangents to its own tangent -/

/-- A node assigns no coefficient to an index that is not one of its operands. -/
theorem stepCoefficient_of_not_operand (values : Nat → ℝ) (node : Node) (index : Nat)
    (absent : index ∉ node.operands) : stepCoefficient values node index = 0 := by
  cases node <;> simp_all [stepCoefficient, Node.operands, eq_comm]

/-- **Linearity of the forward tangent**: for a node with operands, its partial-derivative
coefficients paired with the operand tangents give the node's own tangent. -/
theorem coefficients_pair_to_the_tangent (graph : Graph) (hwf : WellFormed graph)
    (is ip ds dp : Nat → ℝ) (bound consumer : Nat) (inRange : consumer < bound)
    (inner : (graph.node consumer).operands ≠ []) :
    ∑ index ∈ Finset.range bound,
        stepCoefficient (value graph is ip) (graph.node consumer) index *
          tangent graph is ip ds dp index =
      tangent graph is ip ds dp consumer := by
  have earlier := hwf consumer
  have own : tangent graph is ip ds dp consumer =
      tangentStep (valuesUpTo graph is ip consumer) (tangentUpTo graph is ip ds dp consumer)
        (graph.node consumer) ds dp := by
    simp [tangent, tangentUpTo]
  rw [own]
  cases hnode : graph.node consumer with
  | input slot => simp [hnode, Node.operands] at inner
  | param slot => simp [hnode, Node.operands] at inner
  | constant value => simp [hnode, Node.operands] at inner
  | add a b =>
      rw [hnode] at earlier
      have ha : a < consumer := earlier a (by simp [Node.operands])
      have hb : b < consumer := earlier b (by simp [Node.operands])
      simp only [stepCoefficient, tangentStep, add_mul, ite_mul, one_mul, zero_mul,
        Finset.sum_add_distrib, Finset.sum_ite_eq, Finset.mem_range]
      rw [if_pos (by omega), if_pos (by omega),
        tangentUpTo_stable graph is ip ds dp consumer a ha,
        tangentUpTo_stable graph is ip ds dp consumer b hb]
  | sub a b =>
      rw [hnode] at earlier
      have ha : a < consumer := earlier a (by simp [Node.operands])
      have hb : b < consumer := earlier b (by simp [Node.operands])
      simp only [stepCoefficient, tangentStep, add_mul, ite_mul, one_mul, zero_mul,
        Finset.sum_add_distrib, Finset.sum_ite_eq, Finset.mem_range]
      rw [if_pos (by omega), if_pos (by omega),
        tangentUpTo_stable graph is ip ds dp consumer a ha,
        tangentUpTo_stable graph is ip ds dp consumer b hb]
      ring
  | mul a b =>
      rw [hnode] at earlier
      have ha : a < consumer := earlier a (by simp [Node.operands])
      have hb : b < consumer := earlier b (by simp [Node.operands])
      simp only [stepCoefficient, tangentStep, add_mul, ite_mul, zero_mul,
        Finset.sum_add_distrib, Finset.sum_ite_eq, Finset.mem_range]
      rw [if_pos (by omega), if_pos (by omega),
        tangentUpTo_stable graph is ip ds dp consumer a ha,
        tangentUpTo_stable graph is ip ds dp consumer b hb,
        valuesUpTo_stable graph is ip consumer a ha,
        valuesUpTo_stable graph is ip consumer b hb]
      ring

/-- A node without operands assigns every index the coefficient zero. -/
theorem leaf_coefficients_vanish (values : Nat → ℝ) (node : Node) (leaf : node.operands = [])
    (index : Nat) : stepCoefficient values node index = 0 :=
  stepCoefficient_of_not_operand values node index (by simp [leaf])

/-! ## The frontier invariant -/

/-- The frontier weight after `fuel` reverse steps: a processed node with operands carries no
weight, and every other node carries its tangent. -/
def frontier (graph : Graph) (tangents : Nat → ℝ) (fuel index : Nat) : ℝ :=
  if graph.arity - fuel ≤ index ∧ (graph.node index).operands ≠ [] then 0 else tangents index

/-- One reverse step moves only the processed consumer, and only when it has operands, out of the
frontier. -/
theorem frontier_succ (graph : Graph) (tangents : Nat → ℝ) (fuel index : Nat)
    (within : fuel + 1 ≤ graph.arity) :
    frontier graph tangents fuel index =
      if graph.arity - 1 - fuel = index ∧ (graph.node index).operands ≠ [] then tangents index
      else frontier graph tangents (fuel + 1) index := by
  unfold frontier
  by_cases inner : (graph.node index).operands ≠ []
  · simp only [and_iff_left inner]
    split_ifs <;> first | rfl | (exfalso; omega)
  · simp only [inner, and_false, if_false]

/-- **The frontier pairing never changes**: after any number of reverse steps up to the arity, the
adjoints paired with the frontier weights equal the seed paired with every tangent. -/
theorem frontier_invariant (graph : Graph) (hwf : WellFormed graph) (is ip ds dp seed : Nat → ℝ) :
    ∀ fuel, fuel ≤ graph.arity →
      adjointDot graph (adjointAfter graph (value graph is ip) seed fuel)
          (frontier graph (tangent graph is ip ds dp) fuel) =
        adjointDot graph seed (tangent graph is ip ds dp) := by
  intro fuel
  induction fuel with
  | zero =>
      intro _
      unfold adjointDot
      refine Finset.sum_congr rfl fun index member => ?_
      have below := Finset.mem_range.mp member
      simp only [adjointAfter, frontier]
      rw [if_neg (by omega)]
  | succ fuel ih =>
      intro within
      set consumer := graph.arity - 1 - fuel with hconsumer
      have inRange : consumer < graph.arity := by omega
      have previous := ih (by omega)
      rw [adjointDot_step]
      -- The processed consumer's contribution, paired with the new frontier, is its tangent share.
      have operandWeights : ∀ index ∈ Finset.range graph.arity,
          contribution (value graph is ip)
              (adjointAfter graph (value graph is ip) seed fuel (graph.arity - 1 - fuel))
              (graph.node (graph.arity - 1 - fuel)) index *
            frontier graph (tangent graph is ip ds dp) (fuel + 1) index =
          adjointAfter graph (value graph is ip) seed fuel consumer *
            (stepCoefficient (value graph is ip) (graph.node consumer) index *
              tangent graph is ip ds dp index) := by
        intro index _
        unfold contribution
        rw [← hconsumer]
        by_cases operand : index ∈ (graph.node consumer).operands
        · have before : index < consumer := hwf consumer index operand
          simp only [frontier]
          rw [if_neg (by omega)]
          ring
        · rw [stepCoefficient_of_not_operand _ _ _ operand]
          ring
      have contributionShare : contributionDot graph (value graph is ip)
          (adjointAfter graph (value graph is ip) seed fuel (graph.arity - 1 - fuel))
          (frontier graph (tangent graph is ip ds dp) (fuel + 1)) (graph.arity - 1 - fuel) =
          adjointAfter graph (value graph is ip) seed fuel consumer *
            ∑ index ∈ Finset.range graph.arity,
              stepCoefficient (value graph is ip) (graph.node consumer) index *
                tangent graph is ip ds dp index := by
        unfold contributionDot
        rw [Finset.sum_congr rfl operandWeights, ← Finset.mul_sum]
      rw [contributionShare]
      by_cases inner : (graph.node consumer).operands ≠ []
      · -- The consumer leaves the frontier and its tangent moves onto its operands.
        rw [coefficients_pair_to_the_tangent graph hwf is ip ds dp graph.arity consumer inRange inner]
        have split : adjointDot graph (adjointAfter graph (value graph is ip) seed fuel)
              (frontier graph (tangent graph is ip ds dp) fuel) =
            adjointDot graph (adjointAfter graph (value graph is ip) seed fuel)
                (frontier graph (tangent graph is ip ds dp) (fuel + 1)) +
              adjointAfter graph (value graph is ip) seed fuel consumer *
                tangent graph is ip ds dp consumer := by
          unfold adjointDot
          have pointwise : ∀ index ∈ Finset.range graph.arity,
              adjointAfter graph (value graph is ip) seed fuel index *
                  frontier graph (tangent graph is ip ds dp) fuel index =
                adjointAfter graph (value graph is ip) seed fuel index *
                    frontier graph (tangent graph is ip ds dp) (fuel + 1) index +
                  (if consumer = index then
                    adjointAfter graph (value graph is ip) seed fuel index *
                      tangent graph is ip ds dp index else 0) := by
            intro index _
            rw [frontier_succ graph (tangent graph is ip ds dp) fuel index within, ← hconsumer]
            by_cases same : consumer = index
            · subst same
              have gone : frontier graph (tangent graph is ip ds dp) (fuel + 1) consumer = 0 := by
                unfold frontier
                rw [if_pos ⟨by omega, inner⟩]
              rw [if_pos ⟨rfl, inner⟩, if_pos rfl, gone]
              ring
            · rw [if_neg (fun h => same h.1), if_neg same, add_zero]
          rw [Finset.sum_congr rfl pointwise, Finset.sum_add_distrib, Finset.sum_ite_eq,
            if_pos (Finset.mem_range.mpr inRange)]
        rw [split] at previous
        linarith
      · -- A leaf carries the same frontier weight before and after, and contributes nothing.
        have leaf : (graph.node consumer).operands = [] := by simpa using inner
        have zero : ∑ index ∈ Finset.range graph.arity,
            stepCoefficient (value graph is ip) (graph.node consumer) index *
              tangent graph is ip ds dp index = 0 := by
          apply Finset.sum_eq_zero
          intro index _
          rw [leaf_coefficients_vanish _ _ leaf, zero_mul]
        rw [zero, mul_zero, add_zero]
        have same : adjointDot graph (adjointAfter graph (value graph is ip) seed fuel)
              (frontier graph (tangent graph is ip ds dp) (fuel + 1)) =
            adjointDot graph (adjointAfter graph (value graph is ip) seed fuel)
              (frontier graph (tangent graph is ip ds dp) fuel) := by
          unfold adjointDot
          refine Finset.sum_congr rfl fun index _ => ?_
          rw [frontier_succ graph (tangent graph is ip ds dp) fuel index within, ← hconsumer,
            if_neg (fun h => h.2 (h.1 ▸ leaf))]
        rw [same]
        exact previous

/-! ## The transpose identity and the derivative -/

/-- **The transpose identity over every well-formed graph**: after the full reverse sweep, the
leaf nodes' adjoints paired with their tangents equal the seed paired with every node's tangent. -/
theorem transpose_identity (graph : Graph) (hwf : WellFormed graph) (is ip ds dp seed : Nat → ℝ) :
    ∑ index ∈ Finset.range graph.arity,
        (if (graph.node index).operands = [] then
          adjointAfter graph (value graph is ip) seed graph.arity index *
            tangent graph is ip ds dp index
        else 0) =
      ∑ index ∈ Finset.range graph.arity, seed index * tangent graph is ip ds dp index := by
  have final := frontier_invariant graph hwf is ip ds dp seed graph.arity (le_refl _)
  unfold adjointDot at final
  rw [← final]
  refine Finset.sum_congr rfl fun index _ => ?_
  simp only [frontier, Nat.sub_self, Nat.zero_le, true_and]
  by_cases leaf : (graph.node index).operands = []
  · rw [if_pos leaf, if_neg (fun h => h leaf)]
  · rw [if_neg leaf, if_pos leaf, mul_zero]

/-- A seed of `1` on one output node and `0` elsewhere. -/
def unitSeed (output : Nat) : Nat → ℝ := fun index => if index = output then 1 else 0

/-- **The reverse sweep computes the derivative**: with a unit seed on any output node of a
well-formed graph, the leaf adjoints paired with their tangents are the Mathlib derivative of that
output along the direction line. -/
theorem reverse_sweep_is_the_derivative (graph : Graph) (hwf : WellFormed graph)
    (is ip ds dp : Nat → ℝ) (output : Nat) (inRange : output < graph.arity) :
    HasDerivAt (fun t : ℝ => value graph (line is ds t) (line ip dp t) output)
      (∑ index ∈ Finset.range graph.arity,
        (if (graph.node index).operands = [] then
          adjointAfter graph (value graph is ip) (unitSeed output) graph.arity index *
            tangent graph is ip ds dp index
        else 0)) (0 : ℝ) := by
  rw [transpose_identity graph hwf is ip ds dp (unitSeed output)]
  have pick : ∑ index ∈ Finset.range graph.arity, unitSeed output index *
      tangent graph is ip ds dp index = tangent graph is ip ds dp output := by
    simp only [unitSeed, ite_mul, one_mul, zero_mul, Finset.sum_ite_eq', Finset.mem_range,
      if_pos inRange]
  rw [pick]
  exact tangent_is_directional_derivative graph hwf is ip ds dp output

/-- **The identity is not vacuous**: on `slotGraph` the full sweep with seed `1` pairs the leaves
to the output tangent `17` along slot `0`. -/
theorem slotGraph_transpose :
    ∑ index ∈ Finset.range slotGraph.arity,
        (if (slotGraph.node index).operands = [] then
          adjointAfter slotGraph (value slotGraph slotGraphInputs (fun _ => 0)) (slotGraphSeed 1)
              slotGraph.arity index *
            tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) index
        else 0) = 17 := by
  rw [transpose_identity slotGraph slotGraph_wellFormed]
  have pick : ∑ index ∈ Finset.range slotGraph.arity, slotGraphSeed 1 index *
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) index =
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 := by
    simp only [slotGraphSeed, ite_mul, one_mul, zero_mul, Finset.sum_ite_eq', Finset.mem_range]
    rw [if_pos (by decide)]
  rw [pick, slotGraph_tangent_slot_zero]

end NobleFoundation.ReverseAD.Transpose
