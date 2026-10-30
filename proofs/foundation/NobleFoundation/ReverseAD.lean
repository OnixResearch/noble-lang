/-
Reverse-mode AD reference for the bounded smooth graph slice (issue #45, phase 1).

The fragment is a finite, well-formed, acyclic graph over the reals with explicit
input and parameter slots, exact finite constants, addition, subtraction and
multiplication. It has one selected output node. Guards, indexing, Boolean paths,
activation corners, surrogate rules and dynamic Programs are outside this file, and
so are ordered affine terms. Subtraction was admitted after the add/multiply core was
proved, so the sign of its second operand has a checked mutation control
(`ReverseADMutations.lean`).

The two concerns stay separate, following the "differentiate, then
transpose" decomposition of *You Only Linearize Once* as a proof decomposition
rather than as three runtime passes:

* `valuesUpTo` is the intended-real denotation with the stored operation order
  preserved, and `tangentUpTo` is the forward tangent recursion;
  `tangent_is_directional_derivative` proves the tangent *is* Mathlib's
  derivative of the denotation along a direction line at `t = 0` (the JVP
  specification);
* `adjointAfter` is the reverse traversal. Each processed consumer *adds* its
  contribution to its operands' adjoints (`adjoint_step_adds_contribution`), a
  repeated operand contributes from both occurrences
  (`repeated_operand_contributes_twice`), and a node that is not an operand is left
  unchanged (`adjoint_unchanged_when_operand_absent`). A hand-written vector of
  overwritten adjoints pairs to `4` where the sweep pairs to `8`
  (`the_overwriting_update_pairs_to_four`,
  `the_sweep_pairs_to_eight_on_the_second_slot`); it is a literal control, not a
  mutation of the sweep.

The slot-facing extraction is modelled separately by `slotNode` and
`slotGradient`, with a non-index-aligned fan-out/repeated-slot fixture.

This is a *real* model. Nothing here says that binary32 gradients equal exact
derivatives, and nothing here authorises reassociation, FMA contraction or
parallel reduction. The binary32 operation sequence, the Rust `grad.rs` loop and
the emitted-artifact correspondence are separate, and still open, obligations.
The transpose identity is proved as a one-step pairing invariant
(`adjointDot_step`) and for the fixture (`exampleGraph_adjointDot`); the identity
over arbitrary well-formed graphs stays open.
-/
import Mathlib.Data.Real.Basic
import Mathlib.Analysis.Calculus.Deriv.Basic
import Mathlib.Analysis.Calculus.Deriv.Add
import Mathlib.Analysis.Calculus.Deriv.Mul

namespace NobleFoundation.ReverseAD

/-- One node of the fragment. Operand fields are node indices. -/
inductive Node where
  /-- Interface input slot `n`. -/
  | input (slot : Nat)
  /-- Interface parameter slot `n`. -/
  | param (slot : Nat)
  /-- A finite real constant, interpreted exactly. -/
  | constant (value : ℝ)
  /-- `a + b`. -/
  | add (a b : Nat)
  /-- `a - b`. -/
  | sub (a b : Nat)
  /-- `a * b`. -/
  | mul (a b : Nat)

/-- Operand indices of one node, with multiplicity: `mul a a` names the same
operand twice, which is exactly the case where accumulation must add rather
than overwrite. -/
def Node.operands : Node → List Nat
  | .add a b => [a, b]
  | .sub a b => [a, b]
  | .mul a b => [a, b]
  | _ => []

/-- A finite graph: an arity and a total node function. Indices at or beyond the
arity are junk and are never read by a well-formed graph's output. -/
structure Graph where
  arity : Nat
  node : Nat → Node

/-- Well-formedness: every operand is a strictly earlier node, so the graph is
acyclic and the stored order is a topological order. -/
def WellFormed (graph : Graph) : Prop :=
  ∀ index, ∀ operand ∈ (graph.node index).operands, operand < index

/-- Evaluate one node against the values of the strictly earlier nodes. -/
def evalStep (values : Nat → ℝ) (node : Node) (is ip : Nat → ℝ) : ℝ :=
  match node with
  | .input slot => is slot
  | .param slot => ip slot
  | .constant value => value
  | .add a b => values a + values b
  | .sub a b => values a - values b
  | .mul a b => values a * values b

/-- The denotation of the first `count` nodes. Node `count - 1` is the newest,
and every earlier value is available to it by well-formedness. -/
def valuesUpTo (graph : Graph) (is ip : Nat → ℝ) : Nat → (Nat → ℝ)
  | 0 => fun _ => 0
  | count + 1 => fun index =>
      if index = count then evalStep (valuesUpTo graph is ip count) (graph.node count) is ip
      else valuesUpTo graph is ip count index

/-- The intended-real denotation of one node. -/
def value (graph : Graph) (is ip : Nat → ℝ) (index : Nat) : ℝ :=
  valuesUpTo graph is ip (index + 1) index

/-- The forward tangent of one node, given the already-computed values and
tangents of the strictly earlier nodes. -/
def tangentStep (values tangents : Nat → ℝ) (node : Node) (ds dp : Nat → ℝ) : ℝ :=
  match node with
  | .input slot => ds slot
  | .param slot => dp slot
  | .constant _ => 0
  | .add a b => tangents a + tangents b
  | .sub a b => tangents a - tangents b
  | .mul a b => tangents a * values b + values a * tangents b

/-- The forward tangent of the first `count` nodes. -/
def tangentUpTo (graph : Graph) (is ip ds dp : Nat → ℝ) : Nat → (Nat → ℝ)
  | 0 => fun _ => 0
  | count + 1 => fun index =>
      if index = count then
        tangentStep (valuesUpTo graph is ip count) (tangentUpTo graph is ip ds dp count)
          (graph.node count) ds dp
      else tangentUpTo graph is ip ds dp count index

/-- The forward tangent of one node. -/
def tangent (graph : Graph) (is ip ds dp : Nat → ℝ) (index : Nat) : ℝ :=
  tangentUpTo graph is ip ds dp (index + 1) index

/-- The direction line through the declared assignment. -/
def line (base direction : Nat → ℝ) (t : ℝ) : Nat → ℝ :=
  fun slot => base slot + t * direction slot

@[simp] theorem line_zero (base direction : Nat → ℝ) : line base direction 0 = base := by
  funext slot
  simp [line]

/-! ### The JVP specification

Every node reads only strictly earlier values, so the denotation of each node is
differentiable along the direction line as soon as its operands are. The
induction carries the statement for all earlier indices at once. -/

theorem evalStep_hasDerivAt (values : ℝ → Nat → ℝ) (tangents : Nat → ℝ) (node : Node)
    (is ip ds dp : Nat → ℝ)
    (hvalues : ∀ slot ∈ node.operands,
      HasDerivAt (fun t : ℝ => values t slot) (tangents slot) (0 : ℝ)) :
    HasDerivAt (fun t : ℝ => evalStep (values t) node (line is ds t) (line ip dp t))
      (tangentStep (values 0) tangents node ds dp) (0 : ℝ) := by
  cases node with
  | input slot =>
      have hid : HasDerivAt (fun t : ℝ => t * ds slot) (ds slot) (0 : ℝ) := by
        simpa using (hasDerivAt_id (0 : ℝ)).mul_const (ds slot)
      have hadd : HasDerivAt (fun t : ℝ => is slot + t * ds slot) (ds slot) (0 : ℝ) :=
        hid.const_add (is slot)
      simpa [evalStep, tangentStep, line] using hadd
  | param slot =>
      have hid : HasDerivAt (fun t : ℝ => t * dp slot) (dp slot) (0 : ℝ) := by
        simpa using (hasDerivAt_id (0 : ℝ)).mul_const (dp slot)
      have hadd : HasDerivAt (fun t : ℝ => ip slot + t * dp slot) (dp slot) (0 : ℝ) :=
        hid.const_add (ip slot)
      simpa [evalStep, tangentStep, line] using hadd
  | constant value =>
      exact hasDerivAt_const (x := (0 : ℝ)) value
  | add a b =>
      exact HasDerivAt.add (hvalues a (by simp [Node.operands]))
        (hvalues b (by simp [Node.operands]))
  | sub a b =>
      exact HasDerivAt.sub (hvalues a (by simp [Node.operands]))
        (hvalues b (by simp [Node.operands]))
  | mul a b =>
      exact HasDerivAt.mul (hvalues a (by simp [Node.operands]))
        (hvalues b (by simp [Node.operands]))

theorem valuesUpTo_hasDerivAt (graph : Graph) (hwf : WellFormed graph) (is ip ds dp : Nat → ℝ) :
    ∀ count index, index < count →
      HasDerivAt (fun t : ℝ => valuesUpTo graph (line is ds t) (line ip dp t) count index)
        (tangentUpTo graph is ip ds dp count index) (0 : ℝ) := by
  intro count
  induction count with
  | zero =>
      intro index hindex
      exact absurd hindex (Nat.not_lt_zero index)
  | succ count ih =>
      intro index hindex
      by_cases hlast : index = count
      · have hstep :=
          evalStep_hasDerivAt (fun t : ℝ => valuesUpTo graph (line is ds t) (line ip dp t) count)
            (tangentUpTo graph is ip ds dp count) (graph.node count) is ip ds dp
            (fun slot hslot => ih slot (hwf count slot hslot))
        rw [hlast]
        simpa [valuesUpTo, tangentUpTo, line_zero] using hstep
      · have hlt : index < count := by omega
        simpa [valuesUpTo, tangentUpTo, hlast] using ih index hlt

/-- The JVP specification: the forward tangent is the derivative of the
intended-real denotation along the direction line, at the declared assignment. -/
theorem tangent_is_directional_derivative (graph : Graph) (hwf : WellFormed graph)
    (is ip ds dp : Nat → ℝ) (index : Nat) :
    HasDerivAt (fun t : ℝ => value graph (line is ds t) (line ip dp t) index)
      (tangent graph is ip ds dp index) (0 : ℝ) := by
  simpa [value, tangent] using
    valuesUpTo_hasDerivAt graph hwf is ip ds dp (index + 1) index (Nat.lt_succ_self index)

/-! ### The reverse traversal

The reverse traversal processes nodes in the opposite of the stored order and
accumulates into the adjoint of each *operand* of the processed node. Two things
make it load-bearing rather than decorative:

* a repeated operand (`mul a a`) contributes **twice**, and shared fan-out
  contributes once per edge, so contributions are added rather than overwritten;
* contributions only flow to strictly earlier nodes, so an adjoint is already
  final when its own node is processed.

The transpose identity is proved at two scopes below: a generic *one-step pairing
invariant* (`adjointDot_step`), which pairs the accumulated adjoints with a
direction over the graph's slots and adds exactly the processed consumer's
contributions, and the fixture-level identity (`exampleGraph_adjointDot`), where
the reverse sweep's accumulated adjoints pair with the declared direction to
exactly the tangent. What stays open is the identity over *arbitrary* graphs: no
theorem here says the sweep equals the derivative for every well-formed DAG.
-/

/-- The partial-derivative coefficient of one node with respect to one operand,
using the values of the strictly earlier nodes. A repeated operand contributes
from both occurrences. -/
def stepCoefficient (values : Nat → ℝ) (node : Node) (operand : Nat) : ℝ :=
  match node with
  | .add a b => (if a = operand then 1 else 0) + (if b = operand then 1 else 0)
  | .sub a b => (if a = operand then 1 else 0) + (if b = operand then -1 else 0)
  | .mul a b =>
      (if a = operand then values b else 0) + (if b = operand then values a else 0)
  | .input _ => 0
  | .param _ => 0
  | .constant _ => 0

/-- Contribution of one processed consumer to one operand's adjoint: the **consumer's** own adjoint
multiplies its partial-derivative coefficient. The first draft of this model multiplied the *operand's*
adjoint instead, which cannot carry a seed from the output node to its operands at all; the
fixture-level transpose identity below refuted it, and the model was corrected rather than the
identity being weakened. -/
def contribution (values : Nat → ℝ) (consumerAdjoint : ℝ) (node : Node) (operand : Nat) : ℝ :=
  consumerAdjoint * stepCoefficient values node operand

/-- The adjoints after processing `fuel` nodes from the top of the stored order
downwards. The seed is the initial adjoint vector, which carries the selected
output's seed and zeros elsewhere. -/
def adjointAfter (graph : Graph) (values seed : Nat → ℝ) : Nat → (Nat → ℝ)
  | 0 => seed
  | fuel + 1 =>
      let consumer := graph.arity - 1 - fuel
      let previous := adjointAfter graph values seed fuel
      fun operand =>
        previous operand + contribution values (previous consumer) (graph.node consumer) operand

/-- Find the node implementing an interface slot by scanning the stored node order.

The result is total for the same reason that `Graph.node` is total: an absent
slot falls back to node `0`.  The checked fixture below only asks for declared,
unique slots, so that fallback is never used there.  Scanning from the newest
node makes the convention deterministic if a malformed graph repeats a slot.
-/
def slotNodeUpTo (graph : Graph) (slot : Nat) : Nat → Nat
  | 0 => 0
  | count + 1 =>
      match graph.node count with
      | .input declared =>
          if declared = slot then count else slotNodeUpTo graph slot count
      | .param declared =>
          if declared = slot then count else slotNodeUpTo graph slot count
      | _ => slotNodeUpTo graph slot count

/-- Node index corresponding to an interface slot, in the same shape as the
`CheckedGraph::slot_node` lookup used by the Rust gradient extraction. -/
def slotNode (graph : Graph) (slot : Nat) : Nat := slotNodeUpTo graph slot graph.arity

/-- Read one interface-slot gradient from the completed reverse accumulation. -/
def slotGradient (graph : Graph) (values seed : Nat → ℝ) (slot : Nat) : ℝ :=
  adjointAfter graph values seed graph.arity (slotNode graph slot)

/-- A test-local wrong-slot extraction: it treats the slot number as a node
index instead of looking up the node that implements that slot. -/
def wrongSlotGradient (graph : Graph) (values seed : Nat → ℝ) (slot : Nat) : ℝ :=
  adjointAfter graph values seed graph.arity slot

/-- The contribution of a repeated operand is counted twice, which is accumulation
rather than overwriting. -/
theorem repeated_operand_contributes_twice (values : Nat → ℝ) (consumerAdjoint : ℝ) (a : Nat) :
    contribution values consumerAdjoint (.mul a a) a
      = consumerAdjoint * (values a + values a) := by
  simp [contribution, stepCoefficient]

/-- Accumulation adds the consumer's contribution to the operand's own adjoint;
it does not replace it. -/
theorem adjoint_step_adds_contribution (graph : Graph) (values seed : Nat → ℝ)
    (fuel operand : Nat) :
    adjointAfter graph values seed (fuel + 1) operand
      = adjointAfter graph values seed fuel operand +
          contribution values
            (adjointAfter graph values seed fuel (graph.arity - 1 - fuel))
            (graph.node (graph.arity - 1 - fuel)) operand := rfl

/-- A node that is not an operand of the processed node leaves every adjoint
unchanged, so descending order is what makes an adjoint final before its own step. -/
theorem adjoint_unchanged_when_operand_absent (graph : Graph) (values seed : Nat → ℝ)
    (fuel operand : Nat)
    (habsent : operand ∉ (graph.node (graph.arity - 1 - fuel)).operands) :
    adjointAfter graph values seed (fuel + 1) operand = adjointAfter graph values seed fuel operand := by
  have hcoefficient : stepCoefficient values (graph.node (graph.arity - 1 - fuel)) operand = 0 := by
    simp only [stepCoefficient]
    split <;> simp_all [eq_comm, Node.operands]
  simp [adjointAfter, contribution, hcoefficient]

/-- The transpose pairing: the accumulated adjoints paired with a direction over the graph's slots. -/
def adjointDot (graph : Graph) (adjoints direction : Nat → ℝ) : ℝ :=
  ∑ index ∈ Finset.range graph.arity, adjoints index * direction index

/-- One processed consumer's contributions paired with the direction. -/
def contributionDot (graph : Graph) (values : Nat → ℝ) (consumerAdjoint : ℝ)
    (direction : Nat → ℝ) (consumer : Nat) : ℝ :=
  ∑ operand ∈ Finset.range graph.arity,
    contribution values consumerAdjoint (graph.node consumer) operand * direction operand

/-- **The reverse-loop pairing invariant**: pairing the adjoints after one more processed consumer
with a direction adds exactly that consumer's contributions and nothing else, so accumulation is
what the sweep does step by step rather than only at the end. -/
theorem adjointDot_step (graph : Graph) (values seed direction : Nat → ℝ) (fuel : Nat) :
    adjointDot graph (adjointAfter graph values seed (fuel + 1)) direction
      = adjointDot graph (adjointAfter graph values seed fuel) direction
        + contributionDot graph values
            (adjointAfter graph values seed fuel (graph.arity - 1 - fuel)) direction
            (graph.arity - 1 - fuel) := by
  unfold adjointDot contributionDot
  rw [← Finset.sum_add_distrib]
  refine Finset.sum_congr rfl ?_
  intro operand _
  simp only [adjointAfter, contribution]
  ring

/-! ### Concrete checked fixtures

A single graph exercises an input slot, an exact constant, an addition and a
node that names the same operand twice (`mul 2 2`), at the declared assignment
`inputs = 3`. Values: `3`, `1`, `4`, `16`. With the direction `d(input 0) = 1`
and every parameter direction zero, the tangent is `2·4·1 = 8`. -/
def exampleGraph : Graph where
  arity := 4
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .constant 1
    else if index = 2 then .add 0 1
    else .mul 2 2

theorem exampleGraph_wellFormed : WellFormed exampleGraph := by
  intro index
  by_cases h0 : index = 0
  · subst h0
    simp [exampleGraph, Node.operands]
  by_cases h1 : index = 1
  · subst h1
    simp [exampleGraph, Node.operands]
  by_cases h2 : index = 2
  · subst h2
    simp [exampleGraph, Node.operands]
  · simp [exampleGraph, h0, h1, h2, Node.operands]
    omega

theorem exampleGraph_value : value exampleGraph (fun _ => 3) (fun _ => 0) 3 = 16 := by
  simp [value, valuesUpTo, evalStep, exampleGraph]
  try norm_num

theorem exampleGraph_tangent :
    tangent exampleGraph (fun _ => 3) (fun _ => 0) (fun slot => if slot = 0 then 1 else 0)
      (fun _ => 0) 3 = 8 := by
  simp [tangent, tangentUpTo, valuesUpTo, evalStep, tangentStep, exampleGraph]
  try norm_num

/-- The tangent statement of the fixture is Mathlib's derivative statement, so
the JVP specification is exercised on a concrete graph rather than only
formally. -/
theorem exampleGraph_derivative :
    HasDerivAt
      (fun t : ℝ => value exampleGraph (line (fun _ => 3) (fun slot => if slot = 0 then 1 else 0) t)
        (line (fun _ => 0) (fun _ => 0) t) 3) 8 (0 : ℝ) := by
  have h := tangent_is_directional_derivative exampleGraph exampleGraph_wellFormed
    (fun _ => 3) (fun _ => 0) (fun slot => if slot = 0 then 1 else 0) (fun _ => 0) 3
  simpa [exampleGraph_tangent] using h

/-- A direction on an unused input slot contributes nothing: the fragment's
tangent reads only the slots its nodes use, so this tangent is zero. -/
theorem unused_input_slot_tangent :
    tangent exampleGraph (fun _ => 3) (fun _ => 0) (fun slot => if slot = 1 then 5 else 0)
      (fun _ => 0) 3 = 0 := by
  simp [tangent, tangentUpTo, valuesUpTo, evalStep, tangentStep, exampleGraph]
  try norm_num

/-- The reverse traversal keeps a non-unit seed until a consumer contributes. -/
theorem non_unit_seed_unconsumed :
    adjointAfter exampleGraph (fun _ => 3) (fun operand => if operand = 3 then 5 else 0) 1 3 = 5 := by
  simp only [adjointAfter, contribution, stepCoefficient, exampleGraph]
  try norm_num

/-- The evaluated node values of the fixture at the declared assignment `inputs = 3`. The reverse
sweep needs these, not the input assignment: passing the assignment where node values belong is
what the first draft of this fixture did, and it read `6` instead of the tangent's `8`. -/
def exampleGraphValues : Nat → ℝ := fun index => value exampleGraph (fun _ => 3) (fun _ => 0) index

/-- On the checked fixture the reverse sweep's accumulated adjoints pair with the declared direction
to exactly the tangent `8`, so the accumulated adjoints reproduce the declared derivative for this
graph rather than only per step. -/
theorem exampleGraph_adjointDot :
    adjointDot exampleGraph
        (adjointAfter exampleGraph exampleGraphValues
          (fun operand => if operand = 3 then 1 else 0) 4)
        (fun slot => if slot = 0 then 1 else 0) = 8 := by
  simp [adjointDot, adjointAfter, contribution, stepCoefficient, exampleGraph, exampleGraphValues,
    value, valuesUpTo, evalStep]
  try norm_num

/-- Updating an adjoint by overwriting instead of accumulating the repeated operand's two
contributions pairs to `4` on the same fixture, where the sweep pairs to `8`: the fixture
discriminates the update rule rather than agreeing with anything. -/
def overwritingAdjoints : Nat → ℝ :=
  fun operand =>
    if operand = 0 then 8 else if operand = 1 then 8 else if operand = 2 then 4 else 1

/-- The overwriting update pairs to `4` on the fixture's second slot. -/
theorem the_overwriting_update_pairs_to_four :
    adjointDot exampleGraph overwritingAdjoints (fun slot => if slot = 2 then 1 else 0) = 4 := by
  simp [adjointDot, exampleGraph, overwritingAdjoints]
  try norm_num

/-- The sweep pairs to `8` on the same slot, so the two updates are told apart rather than both
being accepted. -/
theorem the_sweep_pairs_to_eight_on_the_second_slot :
    adjointDot exampleGraph
        (adjointAfter exampleGraph exampleGraphValues
          (fun operand => if operand = 3 then 1 else 0) 4)
        (fun slot => if slot = 2 then 1 else 0) = 8 := by
  simp [adjointDot, adjointAfter, contribution, stepCoefficient, exampleGraph, exampleGraphValues,
    value, valuesUpTo, evalStep]
  try norm_num

/-! ### Slot extraction and fan-out fixture

The second fixture deliberately separates slot and node indices: node `0`
implements slot `1`, while node `1` implements slot `0`.  Node `2` fans out to
the distinct consumers `5` and `6`; node `4` is `mul 0 0`, so the slot node at
node `0` is consumed twice; and node `3` is a multiplication of the distinct
operands `0` and `1`.  At `slot 0 = 3` and `slot 1 = 2`, the output is `39`
with derivatives `17` and `26` respectively. -/

def slotGraph : Graph where
  arity := 8
  node := fun index =>
    if index = 0 then .input 1
    else if index = 1 then .input 0
    else if index = 2 then .add 0 1
    else if index = 3 then .mul 0 1
    else if index = 4 then .mul 0 0
    else if index = 5 then .add 2 4
    else if index = 6 then .mul 2 3
    else .add 5 6

def slotGraphInputs : Nat → ℝ := fun slot =>
  if slot = 0 then 3 else if slot = 1 then 2 else 0

def slotGraphValues : Nat → ℝ := fun index =>
  value slotGraph slotGraphInputs (fun _ => 0) index

def slotGraphSeed (seed : ℝ) : Nat → ℝ := fun index =>
  if index = 7 then seed else 0

def slotGraphDirection (slot : Nat) : Nat → ℝ := fun candidate =>
  if candidate = slot then 1 else 0

theorem slotGraph_wellFormed : WellFormed slotGraph := by
  intro index
  by_cases h0 : index = 0
  · subst h0
    simp [slotGraph, Node.operands]
  by_cases h1 : index = 1
  · subst h1
    simp [slotGraph, Node.operands]
  by_cases h2 : index = 2
  · subst h2
    simp [slotGraph, Node.operands]
  by_cases h3 : index = 3
  · subst h3
    simp [slotGraph, Node.operands]
  by_cases h4 : index = 4
  · subst h4
    simp [slotGraph, Node.operands]
  by_cases h5 : index = 5
  · subst h5
    simp [slotGraph, Node.operands]
  by_cases h6 : index = 6
  · subst h6
    simp [slotGraph, Node.operands]
  · simp [slotGraph, h0, h1, h2, h3, h4, h5, h6, Node.operands]
    omega

theorem slotGraph_slotNode_map :
    slotNode slotGraph 0 = 1 ∧ slotNode slotGraph 1 = 0 := by
  simp [slotNode, slotNodeUpTo, slotGraph]

theorem slotGraph_value :
    value slotGraph slotGraphInputs (fun _ => 0) 7 = 39 := by
  simp [value, valuesUpTo, evalStep, slotGraph, slotGraphInputs]
  try norm_num

theorem slotGraph_tangent_slot_zero :
    tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 = 17 := by
  simp [tangent, tangentUpTo, valuesUpTo, evalStep, tangentStep, slotGraph, slotGraphInputs,
    slotGraphDirection]
  try norm_num

theorem slotGraph_tangent_slot_one :
    tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 = 26 := by
  simp [tangent, tangentUpTo, valuesUpTo, evalStep, tangentStep, slotGraph, slotGraphInputs,
    slotGraphDirection]
  try norm_num

theorem slotGraph_derivative_slot_zero :
    HasDerivAt
      (fun t : ℝ => value slotGraph
        (line slotGraphInputs (slotGraphDirection 0) t)
        (line (fun _ => 0) (fun _ => 0) t) 7) 17 (0 : ℝ) := by
  have h := tangent_is_directional_derivative slotGraph slotGraph_wellFormed slotGraphInputs
    (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7
  simpa [slotGraph_tangent_slot_zero] using h

theorem slotGraph_derivative_slot_one :
    HasDerivAt
      (fun t : ℝ => value slotGraph
        (line slotGraphInputs (slotGraphDirection 1) t)
        (line (fun _ => 0) (fun _ => 0) t) 7) 26 (0 : ℝ) := by
  have h := tangent_is_directional_derivative slotGraph slotGraph_wellFormed slotGraphInputs
    (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7
  simpa [slotGraph_tangent_slot_one] using h

theorem slotGraph_slotGradient_unit :
    slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 0 = 17 ∧
      slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 1 = 26 := by
  simp [slotGradient, slotNode, slotNodeUpTo, slotGraph, slotGraphValues, slotGraphSeed,
    value, valuesUpTo, evalStep, slotGraphInputs, adjointAfter, contribution, stepCoefficient]
  try norm_num

theorem slotGraph_slotGradients_match_unit_tangent :
    slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 0 =
        tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 ∧
      slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 1 =
        tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 := by
  constructor
  · calc
      slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 0 = 17 :=
        slotGraph_slotGradient_unit.1
      _ = tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 :=
        slotGraph_tangent_slot_zero.symm
  · calc
      slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 1 = 26 :=
        slotGraph_slotGradient_unit.2
      _ = tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 :=
        slotGraph_tangent_slot_one.symm

theorem slotGraph_slotGradient_five :
    slotGradient slotGraph slotGraphValues (slotGraphSeed 5) 0 = 85 ∧
      slotGradient slotGraph slotGraphValues (slotGraphSeed 5) 1 = 130 := by
  simp [slotGradient, slotNode, slotNodeUpTo, slotGraph, slotGraphValues, slotGraphSeed,
    value, valuesUpTo, evalStep, slotGraphInputs, adjointAfter, contribution, stepCoefficient]
  try norm_num

theorem slotGraph_slotGradients_match_five_tangent :
    slotGradient slotGraph slotGraphValues (slotGraphSeed 5) 0 =
        5 * tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 ∧
      slotGradient slotGraph slotGraphValues (slotGraphSeed 5) 1 =
        5 * tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 := by
  constructor
  · calc
      slotGradient slotGraph slotGraphValues (slotGraphSeed 5) 0 = 85 :=
        slotGraph_slotGradient_five.1
      _ = 5 * tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 0) (fun _ => 0) 7 := by
        rw [slotGraph_tangent_slot_zero]
        norm_num
  · calc
      slotGradient slotGraph slotGraphValues (slotGraphSeed 5) 1 = 130 :=
        slotGraph_slotGradient_five.2
      _ = 5 * tangent slotGraph slotGraphInputs (fun _ => 0) (slotGraphDirection 1) (fun _ => 0) 7 := by
        rw [slotGraph_tangent_slot_one]
        norm_num

theorem slotGraph_wrong_slot_control :
    wrongSlotGradient slotGraph slotGraphValues (slotGraphSeed 1) 0 ≠
        slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 0 ∧
      wrongSlotGradient slotGraph slotGraphValues (slotGraphSeed 1) 1 ≠
        slotGradient slotGraph slotGraphValues (slotGraphSeed 1) 1 := by
  simp [wrongSlotGradient, slotGradient, slotNode, slotNodeUpTo, slotGraph, slotGraphValues,
    slotGraphSeed, value, valuesUpTo, evalStep, slotGraphInputs, adjointAfter, contribution,
    stepCoefficient]
  try norm_num

end NobleFoundation.ReverseAD
