/-
Kernel-checked binary32 expectations (issue #38).

Every expectation below is proposed by the independent external tool
`verification/binary32/propose.py` and then *checked against the selected Lean
model* by kernel reduction (`decide`). The proposal is not evidence on its own:
the theorem is what the Lean kernel checks.

Categories: halfway ties in both directions, cancellation, both zero signs,
smallest and largest subnormals, the normal/subnormal boundary, gradual
underflow, overflow in both signs, infinities and NaNs. The observations are
`obsEq` classes, so all NaNs agree while signed zeros stay distinct.
-/
import NobleFoundation.Binary32

namespace NobleFoundation.Binary32.Expectations

set_option exponentiation.threshold 4096

open NobleFoundation.Binary32

/-- Every retained expectation: operation, left bits, right bits, expected bits. -/
def retained : List (String × Bits × Bits × Bits) := [
  -- Halfway ties in both directions around 1.0.
  ("add", 0x3F800000, 0x33800000, 0x3F800000),
  ("add", 0x3F800000, 0x34000000, 0x3F800001),
  ("add", 0x3F800001, 0x33800000, 0x3F800002),
  ("add", 0x3F800000, 0x34300000, 0x3F800001),
  -- Cancellation.
  ("sub", 0x3F800001, 0x3F800000, 0x34000000),
  ("sub", 0x3F800000, 0x3F800000, 0x00000000),
  -- Signed zeros.
  ("add", 0x00000000, 0x80000000, 0x00000000),
  ("add", 0x80000000, 0x80000000, 0x80000000),
  ("sub", 0x00000000, 0x00000000, 0x00000000),
  ("sub", 0x80000000, 0x00000000, 0x80000000),
  ("mul", 0x80000000, 0x3F800000, 0x80000000),
  -- Subnormals and the normal boundary.
  ("add", 0x00000001, 0x00000000, 0x00000001),
  ("mul", 0x00000001, 0x3F800000, 0x00000001),
  ("add", 0x007FFFFF, 0x00000001, 0x00800000),
  ("mul", 0x00800000, 0x3F000000, 0x00400000),
  ("sub", 0x00800000, 0x00000001, 0x007FFFFF),
  -- Gradual underflow.
  ("mul", 0x00000001, 0x00000001, 0x00000000),
  -- Overflow in both signs.
  ("mul", 0x7F7FFFFF, 0x40000000, 0x7F800000),
  ("mul", 0xFF7FFFFF, 0x40000000, 0xFF800000),
  ("add", 0x7F7FFFFF, 0x7F7FFFFF, 0x7F800000),
  -- Infinities and NaNs.
  ("add", 0x7F800000, 0x7F800000, 0x7F800000),
  ("add", 0x7F800000, 0xFF800000, 0x7FC00000),
  ("sub", 0x7F800000, 0x7F800000, 0x7FC00000),
  ("mul", 0x7F800000, 0x00000000, 0x7FC00000),
  ("mul", 0xFF800000, 0x40000000, 0xFF800000),
  ("add", 0x7FC00000, 0x3F800000, 0x7FC00000),
]

def apply (operation : String) (x y : Bits) : Bits :=
  if operation == "add" then add x y
  else if operation == "sub" then sub x y
  else mul x y

set_option maxRecDepth 100000 in
/-- Every retained expectation is the selected model's answer for its exact
operand encodings. -/
theorem retained_expectations_checked :
    retained.all (fun entry => apply entry.1 entry.2.1 entry.2.2.1 == entry.2.2.2) = true := by
  decide

/-- A halfway tie between two representable neighbours rounds to even: adding
`2^-24` to `1.0` keeps `1.0`, while adding `3 * 2^-25` moves to the next
representable value. -/
theorem halfway_ties_round_to_even :
    add 0x3F800000 0x33800000 = 0x3F800000
    ∧ add 0x3F800000 0x34300000 = 0x3F800001 := by
  decide

/-- Signed-zero rules of the model: addition of two negative zeros is negative,
mixed zeros are positive, subtraction of equal nonzero values is positive, and
subtracting a positive zero from a negative zero is negative. -/
theorem signed_zero_rules :
    add 0x80000000 0x80000000 = 0x80000000
    ∧ add 0x00000000 0x80000000 = 0x00000000
    ∧ sub 0x80000000 0x00000000 = 0x80000000
    ∧ sub 0x3F800000 0x3F800000 = 0x00000000 := by
  decide

/-- NaNs are one observation class while signed zeros stay distinct and
non-zero encodings stay byte-exact. -/
theorem nan_class_and_zero_separation :
    obsEq 0x7FC00000 0x7F800001 = true
    ∧ obsEq 0x00000000 0x80000000 = false
    ∧ obsEq 0x3F800000 0x3F800001 = false := by
  decide

/-- The exact scaled interpretation of the encoding boundaries: the smallest
subnormal is `1 * 2^-149`, the smallest normal is `2^23 * 2^-149`, and the
largest finite magnitude is `(2^24 - 1) * 2^104`. -/
theorem encoding_boundaries :
    scaledMagnitude 0x00000001 = (1, -149)
    ∧ scaledMagnitude 0x00800000 = (2 ^ 23, -149)
    ∧ scaledMagnitude 0x7F7FFFFF = (2 ^ 24 - 1, 104) := by
  decide

/-- The exact sum of `1.0` and `2^-24` in scaled integers is strictly larger
than `1.0` (`2^24 + 1` units of `2^-24`), yet the selected rounded result is
exactly `1.0`. Round-to-real reasoning therefore cannot discharge the binary32
requirement. -/
theorem exact_sum_exceeds_the_rounded_result :
    add 0x3F800000 0x33800000 = 0x3F800000
    ∧ scaledValue 0x3F800000 = (2 ^ 23, -23)
    ∧ scaledValue 0x33800000 = (2 ^ 23, -47) := by
  decide

/-- Reassociation is observable: the model is not associative on binary32, so a
fold order cannot be silently changed. -/
theorem reassociation_is_observable :
    add (add 0x3F800000 0x33800000) 0x33800000
      ≠ add 0x3F800000 (add 0x33800000 0x33800000) := by
  decide

/-- The exact three-operand sum of `x * y + z` in scaled integers: the product
mantissas multiply, the exponents add, and the third operand is aligned and
added without any intermediate rounding. -/
def exactProductPlusThird (x y z : Bits) : Int × Int :=
  let (mx, sx) := scaledValue x
  let (my, sy) := scaledValue y
  let (mz, sz) := scaledValue z
  let productExponent := sx + sy
  let s := if productExponent ≤ sz then productExponent else sz
  let product := mx * my * (pow2Int (productExponent - s) : Int)
  let third := mz * (pow2Int (sz - s) : Int)
  (product + third, s)

/-- Implicit contraction is observable: rounding the product and then rounding
the sum differs from rounding the exact three-operand sum once, which is why
implicit FMA is excluded by the model. -/
theorem implicit_contraction_is_observable :
    (let (exact, s) := exactProductPlusThird 0x3F800001 0x3F800001 0x33800000
     add (mul 0x3F800001 0x3F800001) 0x33800000 ≠ roundScaled exact s (exact < 0))
    ∧ (let (exact, s) := exactProductPlusThird 0x3F800001 0x3F800001 0x33800000
       roundScaled exact s (exact < 0) = 0x3F800003) := by
  decide

/-- Multiplication rounds exactly once from the exact operand product: the
product of two normals keeps the sign exclusive-or and stays finite. -/
theorem multiplication_rounds_once :
    mul 0xFF800000 0x40000000 = 0xFF800000
    ∧ mul 0x3F800001 0xBF800001 = 0xBF800002
    ∧ scaledValue 0xBF800001 = (-(2 ^ 23 + 1), -23) := by
  decide

end NobleFoundation.Binary32.Expectations
