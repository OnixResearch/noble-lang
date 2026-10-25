/-
Test-local faulty binary32 models (issue #38, phase 2).

Noble's oracle discipline requires that the reference be accompanied by
*test-local* faulty variants which the retained expectations catch. Nothing here
is compiled into a production path or referenced by the reference itself: every
variant is a separate definition and every theorem states that a named retained
expectation detects it.

A variant that escaped would mean the retained family cannot distinguish it, so
the lane would need a new distinguishing case rather than a weakened variant.
-/
import NobleFoundation.Binary32
import NobleFoundation.Binary32Expectations

namespace NobleFoundation.Binary32.Mutations

set_option maxRecDepth 100000
set_option exponentiation.threshold 4096

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Expectations

/-- Variant 1: rounding that breaks ties away from zero instead of to even. -/
def roundPositiveTiesAway (a b : Nat) : Nat :=
  let e := floorLog2Ratio a b
  let scaled : Nat × Nat :=
    if (0 : Int) ≤ 23 - e then (a * pow2Int (23 - e), b)
    else (a, b * pow2Int (e - 23))
  let q := scaled.1 / scaled.2
  let r := scaled.1 % scaled.2
  let rounded := if 2 * r ≥ scaled.2 then q + 1 else q
  let carried := if rounded = pow2 24 then (pow2 23, e + 1) else (rounded, e)
  let m := carried.1
  let e' := carried.2
  if e' > 127 then
    255 * pow2 23
  else if e' < -126 then
    let n := (a * pow2 149) / b
    let r' := (a * pow2 149) % b
    let sub := if n % 2 == 1 && 2 * r' = b then n + 1 else if 2 * r' > b then n + 1 else n
    if sub ≥ pow2 23 then pow2 23 else sub
  else
    ((e' + 127).toNat) * pow2 23 + (m - pow2 23)

def roundScaledTiesAway (m : Int) (s : Int) (isNeg : Bool) : Bits :=
  let a := m.natAbs
  let numerator := if (0 : Int) ≤ s then a * pow2Int s else a
  let denominator := if (0 : Int) ≤ s then 1 else pow2Int (-s)
  let magnitude := roundPositiveTiesAway numerator denominator
  BitVec.ofNat 32 (if isNeg then magnitude ||| 0x80000000 else magnitude)

/-- Variant 1 as a complete addition: the same model with the tie rule broken. -/
def addTiesAway (x y : Bits) : Bits :=
  match classify x, classify y with
  | .notANumber, _ => canonicalNaN
  | _, .notANumber => canonicalNaN
  | .infinity, .infinity =>
      if isNegative x == isNegative y then infinityOf (isNegative x) else canonicalNaN
  | .infinity, _ => infinityOf (isNegative x)
  | _, .infinity => infinityOf (isNegative y)
  | _, _ =>
      let (mx, sx) := scaledValue x
      let (my, sy) := scaledValue y
      let s := if sx ≤ sy then sx else sy
      let sum := mx * (pow2Int (sx - s) : Int) + my * (pow2Int (sy - s) : Int)
      if sum = 0 then zeroOf (isNegative x && isNegative y)
      else roundScaledTiesAway sum s (sum < 0)

/-- Variant 2: every zero result is positive, so signed zeros collapse. -/
def addPositiveZeros (x y : Bits) : Bits :=
  match classify x, classify y with
  | .notANumber, _ => canonicalNaN
  | _, .notANumber => canonicalNaN
  | .infinity, .infinity =>
      if isNegative x == isNegative y then infinityOf (isNegative x) else canonicalNaN
  | .infinity, _ => infinityOf (isNegative x)
  | _, .infinity => infinityOf (isNegative y)
  | _, _ =>
      let (mx, sx) := scaledValue x
      let (my, sy) := scaledValue y
      let s := if sx ≤ sy then sx else sy
      let sum := mx * (pow2Int (sx - s) : Int) + my * (pow2Int (sy - s) : Int)
      if sum = 0 then zeroOf false else roundScaled sum s (sum < 0)

/-- Variant 3: gradual underflow is replaced by flush-to-zero. -/
def flushSubnormal (b : Bits) : Bits :=
  if classify b == .subnormal then zeroOf (isNegative b) else b

/-- Variant 4: the zero sign is ignored, so the two zero observations collapse
while the reference keeps them distinct. -/
def classIgnoringZeroSign (b : Bits) : Class :=
  if classify b == .negZero then .posZero else classify b

def obsEqIgnoringZeroSign (x y : Bits) : Bool :=
  classIgnoringZeroSign x == classIgnoringZeroSign y
    && (!(classIgnoringZeroSign x == .subnormal || classIgnoringZeroSign x == .normal) || x == y)

/-- Variant 5: subtraction is treated as commutative. -/
def subReversed (x y : Bits) : Bits := add y x

/-- Variant 6: implicit contraction, which rounds the product before the sum. -/
def contractedSum (x y z : Bits) : Bits := add (mul x y) z

/-- Variant 1 is caught: a halfway tie that the reference keeps at `1.0` moves a
retained answer under the away-from-zero rule. -/
theorem ties_away_is_caught :
    addTiesAway 0x3F800000 0x33800000 ≠ add 0x3F800000 0x33800000 := by
  decide

/-- Variant 2 is caught: `(-0) + (-0)` is a retained negative zero, and the
collapsing variant returns `+0`. -/
theorem positive_zero_collapse_is_caught :
    addPositiveZeros 0x80000000 0x80000000 ≠ add 0x80000000 0x80000000 := by
  decide

/-- Variant 3 is caught: the smallest subnormal is retained by the reference and
flushed to zero by the variant. -/
theorem flush_to_zero_is_caught :
    flushSubnormal 0x00000001 ≠ 0x00000001
    ∧ mul 0x00000001 0x3F800000 = 0x00000001
    ∧ flushSubnormal 0x007FFFFF ≠ 0x007FFFFF := by
  decide

/-- Variant 4 is caught: the reference separates the two zero signs and two
distinct finite encodings, and the zero-sign-insensitive variant does not. -/
theorem zero_sign_collapse_is_caught :
    obsEqIgnoringZeroSign 0x00000000 0x80000000 = true
    ∧ obsEq 0x00000000 0x80000000 = false
    ∧ obsEqIgnoringZeroSign 0x3F800000 0x3F800001 = false
    ∧ obsEq 0x3F800000 0x3F800001 = false := by
  decide

/-- Variant 5 is caught: the retained cancellation `(1.0 + 2^-23) - 1.0` is
`2^-23`, and the commutative variant returns `-2^-23`. -/
theorem commutative_subtraction_is_caught :
    subReversed 0x3F800001 0x3F800000 ≠ sub 0x3F800001 0x3F800000
    ∧ sub 0x3F800001 0x3F800000 = 0x34000000 := by
  decide

/-- Variant 6 is caught: contracting the product into the sum changes the
retained three-operand answer, so implicit FMA cannot be substituted silently. -/
theorem implicit_contraction_is_caught :
    (let (exact, s) := exactProductPlusThird 0x3F800001 0x3F800001 0x33800000
     contractedSum 0x3F800001 0x3F800001 0x33800000 ≠ roundScaled exact s (exact < 0)) := by
  decide

/-- The mutation lane is closed: every declared variant is caught by a named
retained expectation, and no variant escapes. -/
theorem all_declared_mutations_are_caught :
    (addTiesAway 0x3F800000 0x33800000 != add 0x3F800000 0x33800000)
    && (addPositiveZeros 0x80000000 0x80000000 != add 0x80000000 0x80000000)
    && (flushSubnormal 0x00000001 != 0x00000001)
    && (obsEqIgnoringZeroSign 0x00000000 0x80000000)
    && (!obsEq 0x00000000 0x80000000)
    && (subReversed 0x3F800001 0x3F800000 != sub 0x3F800001 0x3F800000) = true := by
  decide

end NobleFoundation.Binary32.Mutations
