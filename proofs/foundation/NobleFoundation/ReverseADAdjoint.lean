/-
  The final adjoint of every node, node by node (issue #45).

  `ReverseAD.lean` defines the reverse sweep step by step, and `ReverseADTranspose.lean` proves the
  pairing it computes. This module states what the sweep leaves in each node: for a well-formed
  graph, the final adjoint of a node is its seed plus, for every consumer of the node, that
  consumer's **final** adjoint times its partial-derivative coefficient for the node. A repeated
  operand counts once per occurrence, through `stepCoefficient`.

  Two facts carry the proof. The sweep unrolls to the seed plus every processed step's contribution
  (`adjointAfter_unrolled`). Once a node has been processed, no later step changes its adjoint,
  because every later step processes an earlier node whose operands are earlier still
  (`adjoint_final_once_processed`). So the adjoint each consumer contributes with is already its
  final adjoint, and reindexing the steps by the node they process gives the statement.

  This is a real-valued model theorem. It says nothing about binary32 gradients, operation order or
  `grad.rs`.
-/
import NobleFoundation.ReverseADTranspose

namespace NobleFoundation.ReverseAD.Adjoint

open NobleFoundation.ReverseAD

/-- **The sweep unrolled**: after `fuel` steps, every adjoint is its seed plus the contributions of
the `fuel` processed steps, each made with the processed node's adjoint at the time it was
processed. No well-formedness is needed. -/
theorem adjointAfter_unrolled (graph : Graph) (values seed : Nat → ℝ) (fuel index : Nat) :
    adjointAfter graph values seed fuel index
      = seed index + ∑ step ∈ Finset.range fuel,
          contribution values (adjointAfter graph values seed step (graph.arity - 1 - step))
            (graph.node (graph.arity - 1 - step)) index := by
  induction fuel with
  | zero => simp [adjointAfter]
  | succ fuel ih =>
      rw [adjoint_step_adds_contribution, ih, Finset.sum_range_succ]
      ring

/-- One step leaves unchanged every adjoint at or above the node it processes: on a well-formed
graph, that node's operands are strictly below it. -/
theorem adjoint_step_frame (graph : Graph) (hwf : WellFormed graph) (values seed : Nat → ℝ)
    (fuel index : Nat) (above : graph.arity - 1 - fuel ≤ index) :
    adjointAfter graph values seed (fuel + 1) index = adjointAfter graph values seed fuel index := by
  apply adjoint_unchanged_when_operand_absent
  intro operand
  have := hwf (graph.arity - 1 - fuel) index operand
  omega

/-- **Final once processed**: on a well-formed graph, a node at or above the one processed by step
`fuel + 1` keeps the adjoint it has after `fuel` steps for the rest of the sweep. -/
theorem adjoint_final_once_processed (graph : Graph) (hwf : WellFormed graph) (values seed : Nat → ℝ)
    (fuel index : Nat) (above : graph.arity - 1 - fuel ≤ index) :
    ∀ extra, adjointAfter graph values seed (fuel + extra) index
      = adjointAfter graph values seed fuel index := by
  intro extra
  induction extra with
  | zero => rfl
  | succ extra ih =>
      rw [← Nat.add_assoc, adjoint_step_frame graph hwf values seed (fuel + extra) index (by omega), ih]

/-- The adjoint a node contributes with is its final adjoint: step `arity - 1 - node` processes the
node, and on a well-formed graph nothing changes the node's adjoint after that. -/
theorem processed_adjoint_is_final (graph : Graph) (hwf : WellFormed graph) (values seed : Nat → ℝ)
    (step : Nat) (inRange : step < graph.arity) :
    adjointAfter graph values seed step (graph.arity - 1 - step)
      = adjointAfter graph values seed graph.arity (graph.arity - 1 - step) := by
  have h := adjoint_final_once_processed graph hwf values seed step (graph.arity - 1 - step) le_rfl
    (graph.arity - step)
  rw [show step + (graph.arity - step) = graph.arity by omega] at h
  exact h.symm

/-- **The final adjoint of every node**: on a well-formed graph, after the full reverse sweep, each
node's adjoint is its seed plus, over every node, that node's final adjoint times its
partial-derivative coefficient for this node. -/
theorem final_adjoint_eq (graph : Graph) (hwf : WellFormed graph) (values seed : Nat → ℝ)
    (index : Nat) :
    adjointAfter graph values seed graph.arity index
      = seed index + ∑ consumer ∈ Finset.range graph.arity,
          contribution values (adjointAfter graph values seed graph.arity consumer)
            (graph.node consumer) index := by
  rw [adjointAfter_unrolled]
  congr 1
  rw [← Finset.sum_range_reflect]
  refine Finset.sum_congr rfl ?_
  intro step member
  have inRange := Finset.mem_range.mp member
  rw [processed_adjoint_is_final graph hwf values seed (graph.arity - 1 - step) (by omega),
    show graph.arity - 1 - (graph.arity - 1 - step) = step by omega]

/-- **Seed plus its consumers' contributions**: the same statement with the sum restricted to the
nodes that name this node as an operand. Every other node contributes nothing. -/
theorem final_adjoint_eq_seed_plus_consumers (graph : Graph) (hwf : WellFormed graph)
    (values seed : Nat → ℝ) (index : Nat) :
    adjointAfter graph values seed graph.arity index
      = seed index + ∑ consumer ∈ (Finset.range graph.arity).filter
            (fun consumer => index ∈ (graph.node consumer).operands),
          contribution values (adjointAfter graph values seed graph.arity consumer)
            (graph.node consumer) index := by
  rw [final_adjoint_eq graph hwf, Finset.sum_filter]
  congr 1
  refine Finset.sum_congr rfl ?_
  intro consumer _
  split
  · rfl
  · rename_i absent
    simp [contribution, Transpose.stepCoefficient_of_not_operand values _ index absent]

/-- A node no node consumes keeps its seed as its final adjoint. -/
theorem unconsumed_final_adjoint_is_seed (graph : Graph) (hwf : WellFormed graph)
    (values seed : Nat → ℝ) (index : Nat)
    (unconsumed : ∀ consumer < graph.arity, index ∉ (graph.node consumer).operands) :
    adjointAfter graph values seed graph.arity index = seed index := by
  rw [final_adjoint_eq_seed_plus_consumers graph hwf]
  have empty : (Finset.range graph.arity).filter
      (fun consumer => index ∈ (graph.node consumer).operands) = ∅ := by
    apply Finset.filter_false_of_mem
    intro consumer member
    exact unconsumed consumer (Finset.mem_range.mp member)
  rw [empty, Finset.sum_empty, add_zero]

/-! ## The checked fixture, node by node

On `exampleGraph` (`input 0`, `constant 1`, `add 0 1`, `mul 2 2`, values `3`, `1`, `4`, `16`), seed
the output node 3 with `1` and the interior node 1 with `5`:
- node 3 has no consumer and keeps its seed `1`;
- node 2 is consumed twice by `mul 2 2`, so its adjoint is `0 + 1·(4 + 4) = 8`;
- node 1 keeps its own seed and receives the addition's contribution: `5 + 8·1 = 13`;
- node 0 receives only the addition's contribution, `0 + 8·1 = 8`.

A sweep that dropped the seed of an interior node would read `8` at node 1, and one that counted
`mul 2 2` once would read `4` at nodes 2, 1 and 0. -/

/-- The fixture seed: `1` on the output node and `5` on the interior node 1. -/
def interiorSeed : Nat → ℝ := fun index => if index = 3 then 1 else if index = 1 then 5 else 0

/-- The fixture's final adjoints, node by node. -/
theorem exampleGraph_final_adjoints :
    adjointAfter exampleGraph exampleGraphValues interiorSeed exampleGraph.arity 3 = 1 ∧
    adjointAfter exampleGraph exampleGraphValues interiorSeed exampleGraph.arity 2 = 8 ∧
    adjointAfter exampleGraph exampleGraphValues interiorSeed exampleGraph.arity 1 = 13 ∧
    adjointAfter exampleGraph exampleGraphValues interiorSeed exampleGraph.arity 0 = 8 := by
  simp [adjointAfter, contribution, stepCoefficient, exampleGraph, exampleGraphValues, interiorSeed,
    value, valuesUpTo, evalStep]
  norm_num

/-- Node 1's final adjoint is its own seed plus its single consumer's contribution, as
`final_adjoint_eq_seed_plus_consumers` states for every node. -/
theorem exampleGraph_node_one_is_seed_plus_consumer :
    adjointAfter exampleGraph exampleGraphValues interiorSeed exampleGraph.arity 1
      = interiorSeed 1
        + contribution exampleGraphValues
            (adjointAfter exampleGraph exampleGraphValues interiorSeed exampleGraph.arity 2)
            (exampleGraph.node 2) 1 := by
  obtain ⟨-, two, one, -⟩ := exampleGraph_final_adjoints
  rw [one, two]
  simp [contribution, stepCoefficient, exampleGraph, interiorSeed]
  norm_num

end NobleFoundation.ReverseAD.Adjoint
