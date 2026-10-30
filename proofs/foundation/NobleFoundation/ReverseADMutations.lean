/-
  Mutated-update controls for the reverse-AD reference (issue #45).

  `adjointAfter` adds each processed consumer's contribution, `stepCoefficient`, into its
  operands' adjoints. This module reruns the same sweep with exactly one update rule changed and
  checks that a fixture detects every change: each mutant's slot gradient differs from the
  forward tangent, which `tangent_is_directional_derivative` proves is the Mathlib derivative.

  The parameterised sweep `adjointAfterWith` is proved equal to `adjointAfter` under the model's
  own rule, so the mutants differ from the checked sweep only in their one rule. The mutations
  are seeded faults, not an exhaustive fault model, and nothing here relates the sweep to
  `grad.rs`.
-/
import NobleFoundation.ReverseAD

namespace NobleFoundation.ReverseAD.Mutations

open NobleFoundation.ReverseAD

/-! ## The sweep with its update rule as a parameter -/

/-- The reverse sweep with the partial-derivative rule as a parameter. -/
def adjointAfterWith (coefficient : (Nat → ℝ) → Node → Nat → ℝ) (graph : Graph)
    (values seed : Nat → ℝ) : Nat → (Nat → ℝ)
  | 0 => seed
  | fuel + 1 =>
      let consumer := graph.arity - 1 - fuel
      let previous := adjointAfterWith coefficient graph values seed fuel
      fun operand =>
        previous operand + previous consumer * coefficient values (graph.node consumer) operand

/-- **Under the model's own rule the parameterised sweep is exactly `adjointAfter`.** -/
theorem adjointAfterWith_stepCoefficient (graph : Graph) (values seed : Nat → ℝ) :
    ∀ fuel, adjointAfterWith stepCoefficient graph values seed fuel =
      adjointAfter graph values seed fuel
  | 0 => rfl
  | fuel + 1 => by
      funext operand
      simp only [adjointAfterWith, adjointAfter, contribution,
        adjointAfterWith_stepCoefficient graph values seed fuel]

/-- One slot's gradient under a given update rule. -/
def slotGradientWith (coefficient : (Nat → ℝ) → Node → Nat → ℝ) (graph : Graph)
    (values seed : Nat → ℝ) (slot : Nat) : ℝ :=
  adjointAfterWith coefficient graph values seed graph.arity (slotNode graph slot)

theorem slotGradientWith_stepCoefficient (graph : Graph) (values seed : Nat → ℝ) (slot : Nat) :
    slotGradientWith stepCoefficient graph values seed slot = slotGradient graph values seed slot := by
  simp [slotGradientWith, slotGradient, adjointAfterWith_stepCoefficient]

/-! ## Seeded single-rule faults -/

/-- Multiplication pairs each operand with its own value instead of its partner's. -/
def mulOwnValue (values : Nat → ℝ) (node : Node) (operand : Nat) : ℝ :=
  match node with
  | .mul a b => (if a = operand then values a else 0) + (if b = operand then values b else 0)
  | other => stepCoefficient values other operand

/-- Addition passes the adjoint to its first operand only. -/
def addDropsSecond (values : Nat → ℝ) (node : Node) (operand : Nat) : ℝ :=
  match node with
  | .add a _ => if a = operand then 1 else 0
  | other => stepCoefficient values other operand

/-- A repeated multiplication operand is counted once: the first matching occurrence wins. -/
def repeatedCountedOnce (values : Nat → ℝ) (node : Node) (operand : Nat) : ℝ :=
  match node with
  | .mul a b => if a = operand then values b else if b = operand then values a else 0
  | other => stepCoefficient values other operand

/-- Subtraction passes the adjoint to its second operand with the wrong sign. -/
def subSignFlipped (values : Nat → ℝ) (node : Node) (operand : Nat) : ℝ :=
  match node with
  | .sub a b => (if a = operand then 1 else 0) + (if b = operand then 1 else 0)
  | other => stepCoefficient values other operand

/-- The sweep overwrites an operand's adjoint with the newest contribution instead of adding to
it. -/
def adjointAfterOverwrite (graph : Graph) (values seed : Nat → ℝ) : Nat → (Nat → ℝ)
  | 0 => seed
  | fuel + 1 =>
      let consumer := graph.arity - 1 - fuel
      let previous := adjointAfterOverwrite graph values seed fuel
      fun operand =>
        if operand ∈ (graph.node consumer).operands then
          previous consumer * stepCoefficient values (graph.node consumer) operand
        else previous operand

/-! ## Every fault is caught on a checked fixture

On `slotGraph` at `slot 0 = 3`, `slot 1 = 2` with seed `1`, the forward tangents are `17` and
`26` (`slotGraph_tangent_slot_zero`, `slotGraph_tangent_slot_one`). -/

/-- **Wrong multiplication partner**: both slot gradients move, to `24` and `22`. -/
theorem slotGraph_mulOwnValue_caught :
    slotGradientWith mulOwnValue slotGraph slotGraphValues (slotGraphSeed 1) 0 = 24 ∧
      slotGradientWith mulOwnValue slotGraph slotGraphValues (slotGraphSeed 1) 1 = 22 ∧
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 ≠ 24 ∧
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 ≠ 22 := by
  rw [slotGraph_tangent_slot_zero, slotGraph_tangent_slot_one]
  refine ⟨?_, ?_, by norm_num, by norm_num⟩ <;>
  · simp [slotGradientWith, adjointAfterWith, mulOwnValue, stepCoefficient, slotNode,
      slotNodeUpTo, slotGraph, slotGraphValues, slotGraphSeed, value, valuesUpTo, evalStep,
      slotGraphInputs]
    try norm_num

/-- **Dropped second addition operand**: the slot gradients collapse to `0` and `1`. -/
theorem slotGraph_addDropsSecond_caught :
    slotGradientWith addDropsSecond slotGraph slotGraphValues (slotGraphSeed 1) 0 = 0 ∧
      slotGradientWith addDropsSecond slotGraph slotGraphValues (slotGraphSeed 1) 1 = 1 ∧
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 ≠ 0 ∧
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 ≠ 1 := by
  rw [slotGraph_tangent_slot_zero, slotGraph_tangent_slot_one]
  refine ⟨?_, ?_, by norm_num, by norm_num⟩ <;>
  · simp [slotGradientWith, adjointAfterWith, addDropsSecond, stepCoefficient, slotNode,
      slotNodeUpTo, slotGraph, slotGraphValues, slotGraphSeed, value, valuesUpTo, evalStep,
      slotGraphInputs]
    try norm_num

/-- **Repeated operand counted once**: only the slot behind `mul 0 0` moves, to `24`. The other
slot keeps its correct `17`, so this fault is visible on slot 1 alone. -/
theorem slotGraph_repeatedCountedOnce_caught :
    slotGradientWith repeatedCountedOnce slotGraph slotGraphValues (slotGraphSeed 1) 0 = 17 ∧
      slotGradientWith repeatedCountedOnce slotGraph slotGraphValues (slotGraphSeed 1) 1 = 24 ∧
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 ≠ 24 := by
  rw [slotGraph_tangent_slot_one]
  refine ⟨?_, ?_, by norm_num⟩ <;>
  · simp [slotGradientWith, adjointAfterWith, repeatedCountedOnce, stepCoefficient, slotNode,
      slotNodeUpTo, slotGraph, slotGraphValues, slotGraphSeed, value, valuesUpTo, evalStep,
      slotGraphInputs]
    try norm_num

/-- **Overwriting instead of accumulating**: both slot gradients collapse to `1`. -/
theorem slotGraph_overwrite_caught :
    adjointAfterOverwrite slotGraph slotGraphValues (slotGraphSeed 1) slotGraph.arity
        (slotNode slotGraph 0) = 1 ∧
      adjointAfterOverwrite slotGraph slotGraphValues (slotGraphSeed 1) slotGraph.arity
        (slotNode slotGraph 1) = 1 ∧
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 ≠ 1 ∧
      tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 ≠ 1 := by
  rw [slotGraph_tangent_slot_zero, slotGraph_tangent_slot_one]
  refine ⟨?_, ?_, by norm_num, by norm_num⟩ <;>
  · simp [adjointAfterOverwrite, stepCoefficient, Node.operands, slotNode, slotNodeUpTo,
      slotGraph, slotGraphValues, slotGraphSeed, value, valuesUpTo, evalStep, slotGraphInputs]
    try norm_num

/-! ## A subtraction fixture for the sign control

`subGraph` computes `(x - y)^2` with slots that are not index-aligned: node `0` implements slot
`1` (`y`), node `1` implements slot `0` (`x`), node `2` is `sub 1 0` and node `3` is
`mul 2 2`. At `x = 3`, `y = 2` the output is `1` and the derivatives are `2` and `-2`. -/

def subGraph : Graph where
  arity := 4
  node := fun index =>
    if index = 0 then .input 1
    else if index = 1 then .input 0
    else if index = 2 then .sub 1 0
    else .mul 2 2

def subGraphValues : Nat → ℝ := fun index =>
  value subGraph slotGraphInputs (fun _ => 0) index

def subGraphSeed : Nat → ℝ := fun index => if index = 3 then 1 else 0

theorem subGraph_wellFormed : WellFormed subGraph := by
  intro index
  by_cases h0 : index = 0
  · subst h0
    simp [subGraph, Node.operands]
  by_cases h1 : index = 1
  · subst h1
    simp [subGraph, Node.operands]
  by_cases h2 : index = 2
  · subst h2
    simp [subGraph, Node.operands]
  · simp [subGraph, h0, h1, h2, Node.operands]
    omega

theorem subGraph_value : value subGraph slotGraphInputs (fun _ => 0) 3 = 1 := by
  simp [value, valuesUpTo, evalStep, subGraph, slotGraphInputs]
  try norm_num

/-- The forward tangents are `2` along `x` and `-2` along `y`. -/
theorem subGraph_tangents :
    tangent subGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 3 = 2 ∧
      tangent subGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 3 = -2 := by
  constructor <;>
  · simp [tangent, tangentUpTo, valuesUpTo, evalStep, tangentStep, subGraph, slotGraphInputs,
      slotGraphDirection]
    try norm_num

/-- The `y` tangent is the Mathlib derivative of `(x - y)^2` along `y`. -/
theorem subGraph_derivative_slot_one :
    HasDerivAt
      (fun t : ℝ => value subGraph
        (line slotGraphInputs (slotGraphDirection 1) t)
        (line (fun _ => 0) (fun _ => 0) t) 3) (-2) (0 : ℝ) := by
  have h := tangent_is_directional_derivative subGraph subGraph_wellFormed slotGraphInputs
    (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 3
  simpa [subGraph_tangents.2] using h

/-- **The model sweep gets the subtraction sign right**: slot gradients `2` and `-2`, the tangents. -/
theorem subGraph_slotGradients_match_tangents :
    slotGradient subGraph subGraphValues subGraphSeed 0 = 2 ∧
      slotGradient subGraph subGraphValues subGraphSeed 1 = -2 := by
  constructor <;>
  · simp [slotGradient, adjointAfter, contribution, stepCoefficient, slotNode, slotNodeUpTo,
      subGraph, subGraphValues, subGraphSeed, value, valuesUpTo, evalStep, slotGraphInputs]
    try norm_num

/-- **Sign-flipped subtraction is caught**: the `y` gradient becomes `2` where the derivative is
`-2`; the `x` gradient is unchanged. -/
theorem subGraph_subSignFlipped_caught :
    slotGradientWith subSignFlipped subGraph subGraphValues subGraphSeed 0 = 2 ∧
      slotGradientWith subSignFlipped subGraph subGraphValues subGraphSeed 1 = 2 ∧
      tangent subGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 3 ≠ 2 := by
  rw [subGraph_tangents.2]
  refine ⟨?_, ?_, by norm_num⟩ <;>
  · simp [slotGradientWith, adjointAfterWith, subSignFlipped, stepCoefficient, slotNode,
      slotNodeUpTo, subGraph, subGraphValues, subGraphSeed, value, valuesUpTo, evalStep,
      slotGraphInputs]
    try norm_num

end NobleFoundation.ReverseAD.Mutations
