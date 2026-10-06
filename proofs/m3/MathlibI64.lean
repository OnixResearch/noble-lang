import Mathlib.Data.ZMod.Basic

/-!
An independent mathematical oracle for the *implemented* signed wrapping I64
subset. This is not a proof of the Noble compiler, Wasm backend, or the exact
integer/rational calculator. The verifier executes the actual compiled backend
and compares its finite observations with these Mathlib residues.
-/

namespace MathlibI64

private def modulus : ℕ := 2 ^ 64
private def signBit : ℕ := 2 ^ 63

/-- The signed representative of a residue: `0 .. 2^63-1` stays positive;
`2^63 .. 2^64-1` denotes `-2^63 .. -1`. -/
def signed (r : ZMod modulus) : ℤ :=
  if r.val < signBit then (r.val : ℤ) else (r.val : ℤ) - (modulus : ℤ)

def wrap (x : ℤ) : ℤ := signed (x : ZMod modulus)

-- Universal algebraic facts about this independent Mathlib model. Neither
-- theorem refers to a Noble instruction or establishes backend refinement.
theorem add_residue (a b : ℤ) :
    wrap (a + b) = signed ((a : ZMod modulus) + (b : ZMod modulus)) := by
  simp only [wrap, Int.cast_add]

theorem sub_residue (a b : ℤ) :
    wrap (a - b) = signed ((a : ZMod modulus) - (b : ZMod modulus)) := by
  simp only [wrap, Int.cast_sub]

theorem mul_residue (a b : ℤ) :
    wrap (a * b) = signed ((a : ZMod modulus) * (b : ZMod modulus)) := by
  simp only [wrap, Int.cast_mul]

-- Boundary equations are checked by Lean, not asserted by the JS comparator.
example : wrap (9223372036854775807 + 1) = -9223372036854775808 := by decide
example : wrap (-9223372036854775808 - 1) = 9223372036854775807 := by decide
example : wrap (-9223372036854775808 * -1) = -9223372036854775808 := by decide

end MathlibI64
