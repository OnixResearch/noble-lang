/-
Arithmetic models are not interchangeable (issue #91).

Program-contract evidence names the arithmetic model it was checked under (PROGRAM-CONTRACTS
VC-NUM-01). This file is the retained negative control for a mislabelled model: a claim that holds
over the reals fails in the selected binary32 model, so a `reals` proof never stands in for a
`binary32-rne-noble-foundation` claim. The binary64 example at `x = 1e17` that motivated #91 is
the same pattern; Noble selects no binary64 model, so the control is stated in binary32 at
`x = 2^24`, where `x + 1` rounds back to `x` under round to nearest, ties to even.
-/
import Mathlib.Basic.Real.Basic
import Mathlib.Tactic.NormNum
import NobleFoundation.Binary32

namespace NobleFoundation.ArithmeticModels

open NobleFoundation.Binary32

/-- **Over the reals** `(x + 1) - x > 1/2` holds for every `x`. -/
theorem reals_claim_holds (x : ℝ) : (x + 1) - x > 1 / 2 := by
  norm_num

/-- **In the binary32 model** `2^24 + 1` lies halfway between `2^24` and `2^24 + 2`, and the tie
rounds to the even significand, back to `2^24`. -/
theorem binary32_increment_rounds_back : add twoPow24Bits oneBits = twoPow24Bits := by
  decide

/-- So `(x + 1) - x` is positive zero at `x = 2^24` in binary32, and the claim that holds over the
reals fails there. -/
theorem binary32_claim_fails : sub (add twoPow24Bits oneBits) twoPow24Bits = zeroOf false := by
  decide

end NobleFoundation.ArithmeticModels
