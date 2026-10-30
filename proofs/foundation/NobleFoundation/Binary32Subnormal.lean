/-
  The subnormal branch of the binary32 rounding-error rule (issue #44).

  When the carried exponent falls below the normal range, `roundPositive` rounds `a · 2^149 / b` to
  nearest, ties to even, and encodes it as a subnormal, or as the smallest normal when the rounding
  reaches `2^23`. This module proves that branch is within `2^-150` of the exact quotient, half the
  smallest subnormal spacing, and combines it with the normal-range rule of `Binary32Scale.lean`
  into one absolute bound for every positive input whose result is finite: half the spacing of the
  result's binade, where the subnormal binade has spacing `2^-149`.

  The subnormal rule is absolute only: `subnormal_results_are_not_relatively_bounded` shows a
  subnormal result whose relative error is a third. Overflow (a carried exponent above `127`) stays
  outside, and nothing here is a Rust, Wasm or host floating-point correspondence claim.
-/
import NobleFoundation.Binary32Scale

namespace NobleFoundation.Binary32.Subnormal

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error
open NobleFoundation.Binary32.Scale

set_option exponentiation.threshold 4096
set_option maxRecDepth 100000

/-! ## Below the normal range the significand is at most `2^23` -/

/-- The carried exponent is the scale exponent or one more. -/
theorem floorLog2Ratio_le_carried (a b : Nat) : floorLog2Ratio a b ≤ (carriedPair a b).2 := by
  unfold carriedPair
  dsimp only
  split <;> simp

/-- Off or on the power boundary, the ratio is at most `2^(e+1)`. -/
theorem floorLog2Ratio_upper_le (a b : Nat) (ha : 0 < a) (hb : 0 < b) :
    (a : ℚ) ≤ (2 : ℚ) ^ (floorLog2Ratio a b + 1) * b := by
  by_cases boundary : powerBoundary a b
  · exact le_of_eq (floorLog2Ratio_boundary a b boundary)
  · exact le_of_lt (floorLog2Ratio_upper a b ha hb boundary)

/-- Below the normal range the subnormal numerator is at most `2^23` denominators. -/
theorem subnormal_numerator_bound (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hlow : (carriedPair a b).2 < -126) : a * pow2 149 ≤ 2 ^ 23 * b := by
  have he : floorLog2Ratio a b + 1 ≤ -126 := by
    have := floorLog2Ratio_le_carried a b
    omega
  have hq : (a : ℚ) ≤ (2 : ℚ) ^ (-126 : ℤ) * b :=
    le_trans (floorLog2Ratio_upper_le a b ha hb)
      (mul_le_mul_of_nonneg_right (zpow_le_zpow_right₀ (by norm_num) he) (Nat.cast_nonneg b))
  have hpow : (2 : ℚ) ^ (-126 : ℤ) * (2 : ℚ) ^ (149 : ℕ) = (2 : ℚ) ^ (23 : ℕ) := by
    rw [← zpow_natCast, ← zpow_natCast, ← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
    norm_num
  have key : (a : ℚ) * (2 : ℚ) ^ (149 : ℕ) ≤ (2 : ℚ) ^ (23 : ℕ) * b := by
    calc (a : ℚ) * (2 : ℚ) ^ (149 : ℕ) ≤ (2 : ℚ) ^ (-126 : ℤ) * b * (2 : ℚ) ^ (149 : ℕ) :=
          mul_le_mul_of_nonneg_right hq (by positivity)
      _ = (2 : ℚ) ^ (23 : ℕ) * b := by rw [← hpow]; ring
  unfold pow2
  exact_mod_cast key

/-- Below the normal range the rounded significand is at most `2^23`. -/
theorem subnormal_significand_bound (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hlow : (carriedPair a b).2 < -126) : roundNearest (a * pow2 149) b ≤ 2 ^ 23 := by
  have herr := roundNearest_error (a * pow2 149) b hb
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  have hnum : ((a * pow2 149 : Nat) : ℚ) ≤ (2 : ℚ) ^ 23 * b := by
    exact_mod_cast subnormal_numerator_bound a b ha hb hlow
  have hratio : ((a * pow2 149 : Nat) : ℚ) / b ≤ (2 : ℚ) ^ 23 := by
    rw [div_le_iff₀ hbq]
    linarith
  have hupper := (abs_le.mp herr).2
  have hlt : (roundNearest (a * pow2 149) b : ℚ) < (2 : ℚ) ^ 23 + 1 := by linarith
  have : roundNearest (a * pow2 149) b < 2 ^ 23 + 1 := by exact_mod_cast hlt
  omega

/-! ## The subnormal branch of `roundPositive` -/

/-- Below the normal range `roundPositive` is the rounded subnormal significand, capped at the
smallest normal. -/
theorem roundPositive_subnormal (a b : Nat) (hlow : (carriedPair a b).2 < -126) :
    roundPositive a b =
      if roundNearest (a * pow2 149) b ≥ pow2 23 then pow2 23
      else roundNearest (a * pow2 149) b := by
  have hbody : roundPositive a b =
      if (carriedPair a b).2 > 127 then 255 * pow2 23
      else if (carriedPair a b).2 < -126 then
        (if roundNearest (a * pow2 149) b ≥ pow2 23 then pow2 23
          else roundNearest (a * pow2 149) b)
      else ((carriedPair a b).2 + 127).toNat * pow2 23 + ((carriedPair a b).1 - pow2 23) := by
    rfl
  rw [hbody, if_neg (by omega), if_pos hlow]

/-- Below the normal range the cap never changes the value: the significand is at most `2^23`. -/
theorem roundPositive_eq_significand (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hlow : (carriedPair a b).2 < -126) :
    roundPositive a b = roundNearest (a * pow2 149) b := by
  have hbound := subnormal_significand_bound a b ha hb hlow
  rw [roundPositive_subnormal a b hlow]
  split
  · rename_i reached
    unfold pow2 at reached hbound ⊢
    omega
  · rfl

/-! ## Decoding a significand of at most `2^23` -/

/-- A significand below `2^23` encodes the subnormal `n · 2^-149`. -/
theorem subnormal_scaledMagnitude (n : Nat) (hn : n < 2 ^ 23) :
    scaledMagnitude (BitVec.ofNat 32 n) = (n, -149) := by
  unfold scaledMagnitude exponent fraction
  simp only [BitVec.toNat_ushiftRight, BitVec.toNat_ofNat]
  rw [Nat.mod_eq_of_lt (lt_trans hn (by norm_num : (2 : Nat) ^ 23 < 2 ^ 32))]
  rw [show (255 : Nat) = 2 ^ 8 - 1 by norm_num,
      show (8388607 : Nat) = 2 ^ 23 - 1 by norm_num]
  simp only [Nat.shiftRight_eq_div_pow, Nat.and_two_pow_sub_one_eq_mod]
  rw [Nat.div_eq_of_lt hn, Nat.mod_eq_of_lt hn]
  simp

/-- The significand `2^23` encodes the smallest normal, whose value is also `2^23 · 2^-149`. -/
theorem smallest_normal_scaledMagnitude :
    scaledMagnitude (BitVec.ofNat 32 (2 ^ 23)) = (2 ^ 23, -149) := by
  decide

theorem small_encoding_not_negative (n : Nat) (hn : n ≤ 2 ^ 23) :
    isNegative (BitVec.ofNat 32 n) = false := by
  unfold isNegative
  simp only [BitVec.toNat_ofNat]
  rw [Nat.mod_eq_of_lt (by omega : n < 2 ^ 32), Nat.shiftRight_eq_div_pow,
    Nat.div_eq_of_lt (by omega : n < 2 ^ 31)]
  decide

/-- **Every significand of at most `2^23` encodes exactly `n · 2^-149`.** -/
theorem exactValue_small (n : Nat) (hn : n ≤ 2 ^ 23) :
    exactValue (BitVec.ofNat 32 n) = (n : ℚ) * (2 : ℚ) ^ (-149 : ℤ) := by
  apply exactValue_of_scaledMagnitude _ n (-149) _ (small_encoding_not_negative n hn)
  rcases Nat.lt_or_ge n (2 ^ 23) with below | atLeast
  · exact subnormal_scaledMagnitude n below
  · have reached : n = 2 ^ 23 := by omega
    subst reached
    exact smallest_normal_scaledMagnitude

/-! ## The error rules -/

/-- **The subnormal absolute error rule**: below the normal range the encoded result is within
`2^-150`, half the smallest subnormal spacing, of the exact quotient. -/
theorem roundPositive_subnormal_absolute_error (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hlow : (carriedPair a b).2 < -126) :
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤
      (1 : ℚ) / 2 * (2 : ℚ) ^ (-149 : ℤ) := by
  rw [roundPositive_eq_significand a b ha hb hlow,
    exactValue_small _ (subnormal_significand_bound a b ha hb hlow)]
  have herr := roundNearest_error (a * pow2 149) b hb
  have hbq : (0 : ℚ) ≠ b := by exact_mod_cast (Nat.ne_of_lt hb)
  have hpos : (0 : ℚ) < (2 : ℚ) ^ (-149 : ℤ) := zpow_pos (by norm_num) _
  have hpow : (2 : ℚ) ^ (149 : ℕ) * (2 : ℚ) ^ (-149 : ℤ) = 1 := by
    rw [← zpow_natCast, ← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
    norm_num
  have hscale : (a : ℚ) / b = ((a * pow2 149 : Nat) : ℚ) / b * (2 : ℚ) ^ (-149 : ℤ) := by
    simp only [pow2, Nat.cast_mul, Nat.cast_pow, Nat.cast_ofNat]
    rw [mul_div_right_comm, mul_assoc, hpow, mul_one]
  rw [hscale, ← sub_mul, abs_mul, abs_of_pos hpos]
  exact mul_le_mul_of_nonneg_right herr (le_of_lt hpos)

/-- **The finite absolute error rule**: for every positive input whose result is finite, the
encoded result is within half the spacing of its binade, `2^(max e (-126) - 23)`, where `e` is
`floorLog2Ratio a b`. The normal range uses `roundPositive_absolute_error`; below it the subnormal
rule applies. -/
theorem roundPositive_finite_absolute_error (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hfinite : (carriedPair a b).2 ≤ 127) :
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤
      (1 : ℚ) / 2 * (2 : ℚ) ^ (max (floorLog2Ratio a b) (-126) - 23) := by
  by_cases normal : -126 ≤ (carriedPair a b).2
  · calc |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b|
        ≤ (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23) :=
          roundPositive_absolute_error a b ha hb normal hfinite
      _ ≤ (1 : ℚ) / 2 * (2 : ℚ) ^ (max (floorLog2Ratio a b) (-126) - 23) :=
          mul_le_mul_of_nonneg_left (zpow_le_zpow_right₀ (by norm_num) (by omega)) (by norm_num)
  · calc |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b|
        ≤ (1 : ℚ) / 2 * (2 : ℚ) ^ (-149 : ℤ) :=
          roundPositive_subnormal_absolute_error a b ha hb (by omega)
      _ ≤ (1 : ℚ) / 2 * (2 : ℚ) ^ (max (floorLog2Ratio a b) (-126) - 23) :=
          mul_le_mul_of_nonneg_left (zpow_le_zpow_right₀ (by norm_num) (by omega)) (by norm_num)

/-! ## Fixtures and controls -/

/-- **Non-vacuity**: `1 / (3 · 2^130)` is below the normal range, rounds to the subnormal
significand `174763`, and the subnormal rule applies to it. -/
theorem subnormal_rule_applies_below_the_normal_range :
    (carriedPair 1 (3 * 2 ^ 130)).2 < -126 ∧ roundPositive 1 (3 * 2 ^ 130) = 174763 ∧
      |exactValue (BitVec.ofNat 32 (roundPositive 1 (3 * 2 ^ 130))) - (1 : ℚ) / (3 * 2 ^ 130 : Nat)|
        ≤ (1 : ℚ) / 2 * (2 : ℚ) ^ (-149 : ℤ) := by
  have hlow : (carriedPair 1 (3 * 2 ^ 130)).2 < -126 := by decide
  exact ⟨hlow, by decide,
    roundPositive_subnormal_absolute_error 1 (3 * 2 ^ 130) (by decide) (by decide) hlow⟩

/-- **The bound is attained where rounding reaches the smallest normal**: `(2^24 - 1) / 2^150` is a
tie halfway below `2^-126`, ties to the even significand `2^23`, and its error is exactly
`2^-150`. -/
theorem rounding_to_the_smallest_normal_attains_the_bound :
    (carriedPair (2 ^ 24 - 1) (2 ^ 150)).2 < -126 ∧ roundPositive (2 ^ 24 - 1) (2 ^ 150) = 2 ^ 23 ∧
      |exactValue (BitVec.ofNat 32 (roundPositive (2 ^ 24 - 1) (2 ^ 150))) -
          ((2 ^ 24 - 1 : Nat) : ℚ) / (2 ^ 150 : Nat)| = (1 : ℚ) / 2 * (2 : ℚ) ^ (-149 : ℤ) := by
  have hround : roundPositive (2 ^ 24 - 1) (2 ^ 150) = 2 ^ 23 := by decide
  refine ⟨by decide, hround, ?_⟩
  rw [hround, exactValue_small (2 ^ 23) (le_refl _)]
  norm_num

/-- **The subnormal rule cannot be relative**: `3 / 2^151` rounds to the smallest subnormal, an
error of a third of the exact value, far above the normal-range relative bound `2^-24`. -/
theorem subnormal_results_are_not_relatively_bounded :
    (carriedPair 3 (2 ^ 151)).2 < -126 ∧
      (1 / (2 : ℚ) ^ 24) * ((3 : ℚ) / (2 ^ 151 : Nat)) <
        |exactValue (BitVec.ofNat 32 (roundPositive 3 (2 ^ 151))) - (3 : ℚ) / (2 ^ 151 : Nat)| := by
  have hround : roundPositive 3 (2 ^ 151) = 1 := by decide
  refine ⟨by decide, ?_⟩
  rw [hround, exactValue_small 1 (by norm_num)]
  norm_num

end NobleFoundation.Binary32.Subnormal
