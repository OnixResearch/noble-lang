/-
  An ordered binary32 reverse-AD reference (issue #45, item 4 of the deliverable).

  `ReverseAD.lean` and its companions are real-valued: they prove that the reverse sweep computes
  the exact derivative. They say nothing about the binary32 gradients `grad.rs` computes, where
  every multiplication and every accumulation rounds, so the result depends on the order of
  operations. This module fixes that order as a model, over the kernel-reducible binary32
  operations of `Binary32.lean` (issue #38):

  - forward values are computed in stored node order, one rounding per operation, as
    `forward_observed` does;
  - every node's adjoint starts at `+0`, and each `(output, seed)` pair is accumulated in turn as
    `rn(gr[o] + s)`;
  - nodes are processed in descending stored order, and each node's adjoint `gn` is read once,
    before its contributions;
  - `add a b` accumulates `gn` into `a`, then into `b`; `sub a b` accumulates `gn` into `a`, then
    `rn(+0 − gn)` into `b`; `mul a b` accumulates `rn(gn · v_b)` into `a`, then `rn(gn · v_a)`
    into `b`. Every accumulation is one rounded addition, so a repeated operand is accumulated
    once per occurrence.

  That is the order of `backward` in `crates/noble-ngraph/src/grad.rs` for `R = f32`, restricted
  to inputs, parameters, constants, addition, subtraction and multiplication. The model is a
  reference: no theorem here relates it to `grad.rs`, and the fixtures are checked by kernel
  evaluation (`decide`), not by running Rust.
-/
import NobleFoundation.Binary32
import NobleFoundation.ReverseAD

namespace NobleFoundation.ReverseAD.Ordered

open NobleFoundation.Binary32 (Bits zeroOf oneBits negOneBits twoPow24Bits)

set_option exponentiation.threshold 4096

/-- One node of the ordered fragment. Operands are node indices; constants are encodings. -/
inductive Node where
  | input (slot : Nat)
  | param (slot : Nat)
  | constant (bits : Bits)
  | add (a b : Nat)
  | sub (a b : Nat)
  | mul (a b : Nat)

/-- `+0`, the initial adjoint and the value read for an absent index. -/
def zero : Bits := zeroOf false

/-- One node evaluated against the values of the earlier nodes, with one rounding. -/
def evalStep (values : List Bits) (node : Node) (inputs params : Nat → Bits) : Bits :=
  match node with
  | .input slot => inputs slot
  | .param slot => params slot
  | .constant bits => bits
  | .add a b => Binary32.add (values.getD a zero) (values.getD b zero)
  | .sub a b => Binary32.sub (values.getD a zero) (values.getD b zero)
  | .mul a b => Binary32.mul (values.getD a zero) (values.getD b zero)

/-- Forward values in stored node order. -/
def forward (nodes : List Node) (inputs params : Nat → Bits) : List Bits :=
  nodes.foldl (fun values node => values ++ [evalStep values node inputs params]) []

/-- `gr[target] = rn(gr[target] + d)`: one rounded accumulation. -/
def accumulate (adjoints : List Bits) (target : Nat) (d : Bits) : List Bits :=
  adjoints.set target (Binary32.add (adjoints.getD target zero) d)

/-- Every adjoint starts at `+0`, then each `(output, seed)` pair is accumulated in order. -/
def seedAdjoints (count : Nat) (seeds : List (Nat × Bits)) : List Bits :=
  seeds.foldl (fun adjoints seed => accumulate adjoints seed.1 seed.2) (List.replicate count zero)

/-- The contributions of one node: its adjoint is read once, then the first operand is
accumulated before the second. -/
def stepNode (values adjoints : List Bits) (index : Nat) (node : Node) : List Bits :=
  let gn := adjoints.getD index zero
  match node with
  | .add a b => accumulate (accumulate adjoints a gn) b gn
  | .sub a b => accumulate (accumulate adjoints a gn) b (Binary32.sub zero gn)
  | .mul a b =>
      accumulate (accumulate adjoints a (Binary32.mul gn (values.getD b zero))) b
        (Binary32.mul gn (values.getD a zero))
  | _ => adjoints

/-- Process nodes `remaining - 1` down to `0`. -/
def sweep (nodes : List Node) (values : List Bits) : Nat → List Bits → List Bits
  | 0, adjoints => adjoints
  | remaining + 1, adjoints =>
      sweep nodes values remaining (stepNode values adjoints remaining (nodes.getD remaining (.constant zero)))

/-- The ordered reverse pass: seeds, then descending stored order. -/
def backward (nodes : List Node) (values : List Bits) (seeds : List (Nat × Bits)) : List Bits :=
  sweep nodes values nodes.length (seedAdjoints nodes.length seeds)

/-- Gradient of the node at `index`, for one input assignment and one seed list. -/
def gradient (nodes : List Node) (inputs params : Nat → Bits) (seeds : List (Nat × Bits))
    (index : Nat) : Bits :=
  (backward nodes (forward nodes inputs params) seeds).getD index zero

/-! ## Order-dependent controls

Each control changes one ordering or initialization rule of the reference, so a fixture that
separates it from the reference detects that change. -/

/-- Ascending traversal: nodes processed in stored order, so a node can contribute before its own
consumers have reached it. -/
def sweepAscending (nodes : List Node) (values : List Bits) (count : Nat) :
    Nat → List Bits → List Bits
  | 0, adjoints => adjoints
  | remaining + 1, adjoints =>
      let index := count - 1 - remaining
      sweepAscending nodes values count remaining
        (stepNode values adjoints index (nodes.getD index (.constant zero)))

def gradientAscending (nodes : List Node) (inputs params : Nat → Bits) (seeds : List (Nat × Bits))
    (index : Nat) : Bits :=
  let values := forward nodes inputs params
  (sweepAscending nodes values nodes.length nodes.length (seedAdjoints nodes.length seeds)).getD index zero

/-- A repeated multiplication operand accumulated once, with its two partials summed first:
`rn(gr + rn(d₁ + d₂))` instead of `rn(rn(gr + d₁) + d₂)`. -/
def stepNodePresummed (values adjoints : List Bits) (index : Nat) (node : Node) : List Bits :=
  match node with
  | .mul a b =>
      if a = b then
        let gn := adjoints.getD index zero
        accumulate adjoints a
          (Binary32.add (Binary32.mul gn (values.getD b zero)) (Binary32.mul gn (values.getD a zero)))
      else stepNode values adjoints index node
  | _ => stepNode values adjoints index node

def sweepPresummed (nodes : List Node) (values : List Bits) : Nat → List Bits → List Bits
  | 0, adjoints => adjoints
  | remaining + 1, adjoints =>
      sweepPresummed nodes values remaining
        (stepNodePresummed values adjoints remaining (nodes.getD remaining (.constant zero)))

def gradientPresummed (nodes : List Node) (inputs params : Nat → Bits) (seeds : List (Nat × Bits))
    (index : Nat) : Bits :=
  let values := forward nodes inputs params
  (sweepPresummed nodes values nodes.length (seedAdjoints nodes.length seeds)).getD index zero

/-- Seeds assigned rather than accumulated: `gr[o] = s` instead of `gr[o] = rn(gr[o] + s)`. -/
def seedAssigned (count : Nat) (seeds : List (Nat × Bits)) : List Bits :=
  seeds.foldl (fun adjoints seed => adjoints.set seed.1 seed.2) (List.replicate count zero)

def gradientSeedAssigned (nodes : List Node) (inputs params : Nat → Bits)
    (seeds : List (Nat × Bits)) (index : Nat) : Bits :=
  let values := forward nodes inputs params
  (sweep nodes values nodes.length (seedAssigned nodes.length seeds)).getD index zero

/-! ## Fixtures

`2^24` has a unit in the last place of `2`, so `rn(2^24 + 1) = 2^24` by ties-to-even and
`rn(-2^24 + 2^24) = +0` exactly. -/

/-- `-2^24` as an encoding. -/
def negTwoPow24Bits : Bits := 0xCB800000

/-- `2^24 + 2`, the next encoding above `2^24`. -/
def twoPow24PlusTwoBits : Bits := 0x4B800001

/-- `2.0` and `3.0` as encodings. -/
def twoBits : Bits := 0x40000000
def threeBits : Bits := 0x40400000

/-- `x·1 + x·2^24 + x·(-2^24)` with the consumers stored as `1`, `2^24`, `-2^24`. Its exact
derivative in `x` is `1`. -/
def cancelFirst : List Node :=
  [.input 0, .constant oneBits, .constant twoPow24Bits, .constant negTwoPow24Bits,
    .mul 0 1, .mul 0 2, .mul 0 3, .add 4 5, .add 7 6]

/-- The same function with the first and third consumers swapped in stored order:
`x·(-2^24) + x·2^24 + x·1`. Its exact derivative in `x` is also `1`. -/
def cancelSecond : List Node :=
  [.input 0, .constant oneBits, .constant twoPow24Bits, .constant negTwoPow24Bits,
    .mul 0 3, .mul 0 2, .mul 0 1, .add 4 5, .add 7 6]

/-- `x = 1`. -/
def atOne : Nat → Bits := fun _ => oneBits

/-- The unit seed on node 8. -/
def outputSeed : List (Nat × Bits) := [(8, oneBits)]

/-- **The stored order changes the binary32 gradient.** In descending stored order the first
graph's contributions reach `x` as `-2^24`, `2^24`, `1` and sum to `1`; the second graph's reach
it as `1`, `2^24`, `-2^24`, where `rn(1 + 2^24) = 2^24` absorbs the `1` and the result is `+0`. -/
theorem stored_order_changes_the_gradient :
    gradient cancelFirst atOne atOne outputSeed 0 = oneBits ∧
    gradient cancelSecond atOne atOne outputSeed 0 = zero := by
  constructor <;> decide

/-- Ascending traversal reads each consumer's adjoint before it is accumulated, so `x` receives
nothing from the first graph's consumers. -/
theorem ascending_traversal_is_caught :
    gradientAscending cancelFirst atOne atOne outputSeed 0 = zero := by
  decide

/-- `x·x + x·2^24`. Its exact derivative at `x = 1` is `2 + 2^24`. -/
def repeated : List Node :=
  [.input 0, .constant twoPow24Bits, .mul 0 0, .mul 0 1, .add 2 3]

/-- **Per-occurrence accumulation is observable.** The `x·2^24` consumer is processed first, so `x`
holds `2^24` when the two occurrences of `mul 0 0` each add `1`, and each addition rounds back to
`2^24`. Summing the two partials first gives `rn(2^24 + 2) = 2^24 + 2`, the exact derivative. -/
theorem per_occurrence_accumulation_is_observable :
    gradient repeated atOne atOne [(4, oneBits)] 0 = twoPow24Bits ∧
    gradientPresummed repeated atOne atOne [(4, oneBits)] 0 = twoPow24PlusTwoBits := by
  constructor <;> decide

/-- The identity graph: one input node that is also the output. -/
def identity : List Node := [.input 0]

/-- **Seeds accumulate.** Two seeds on the same output add, `1 + 2 = 3`, where assigning them
keeps only the last, `2`. -/
theorem repeated_output_seeds_accumulate :
    gradient identity atOne atOne [(0, oneBits), (0, twoBits)] 0 = threeBits ∧
    gradientSeedAssigned identity atOne atOne [(0, oneBits), (0, twoBits)] 0 = twoBits := by
  constructor <;> decide

/-- **A negative-zero seed starts from `+0`.** `rn(+0 + -0) = +0`, so the reference reports a
positive zero where assigning the seed would keep the negative zero. -/
theorem negative_zero_seed_becomes_positive :
    gradient identity atOne atOne [(0, zeroOf true)] 0 = zeroOf false ∧
    gradientSeedAssigned identity atOne atOne [(0, zeroOf true)] 0 = zeroOf true := by
  constructor <;> decide

/-- On a fixture whose values and partials are all exact, the ordered sweep returns the exact
derivative: `(x + 1)·(x + 1)` at `x = 3` has derivative `2·(3 + 1) = 8`. -/
def square : List Node := [.input 0, .constant oneBits, .add 0 1, .mul 2 2]

/-- `3.0` and `8.0` as encodings. -/
def eightBits : Bits := 0x41000000

theorem exact_fixture_gives_the_derivative :
    gradient square (fun _ => threeBits) (fun _ => threeBits) [(3, oneBits)] 0 = eightBits := by
  decide

/-! ## The exact derivatives of the cancellation fixtures

The real-valued reference of `ReverseAD.lean`, on the same two stored orders, gives `1` for both:
the order sensitivity above is rounding, not a different function. -/

/-- The real-valued counterpart of `cancelFirst`. -/
def realFirst : ReverseAD.Graph where
  arity := 9
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .constant 1
    else if index = 2 then .constant (2 ^ 24)
    else if index = 3 then .constant (-(2 ^ 24))
    else if index = 4 then .mul 0 1
    else if index = 5 then .mul 0 2
    else if index = 6 then .mul 0 3
    else if index = 7 then .add 4 5
    else .add 7 6

/-- The real-valued counterpart of `cancelSecond`. -/
def realSecond : ReverseAD.Graph where
  arity := 9
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .constant 1
    else if index = 2 then .constant (2 ^ 24)
    else if index = 3 then .constant (-(2 ^ 24))
    else if index = 4 then .mul 0 3
    else if index = 5 then .mul 0 2
    else if index = 6 then .mul 0 1
    else if index = 7 then .add 4 5
    else .add 7 6

/-- Both stored orders have exact reverse-sweep gradient `1` in `x` at `x = 1`. -/
theorem exact_gradient_is_one_in_both_orders :
    adjointAfter realFirst (value realFirst (fun _ => 1) (fun _ => 1))
        (fun index => if index = 8 then 1 else 0) 9 0 = 1 ∧
    adjointAfter realSecond (value realSecond (fun _ => 1) (fun _ => 1))
        (fun index => if index = 8 then 1 else 0) 9 0 = 1 := by
  constructor <;>
    simp [adjointAfter, contribution, stepCoefficient, realFirst, realSecond, value, valuesUpTo,
      ReverseAD.evalStep]

end NobleFoundation.ReverseAD.Ordered
