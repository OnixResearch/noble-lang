/-
  The scale premises of the binary32 normal-range error rule, derived (issue #44).

  `roundPositive_normal_absolute_error` in `Binary32Error.lean` takes two scale premises: the
  scaled quotient lies in one normal significand bin, `2^23 · d ≤ n < 2^24 · d`. This module
  derives the lower premise for every positive input.

  The upper premise is not universal. For ratios below one that are exact powers of two (`1/2`,
  `1/4`, …) the model's `floorLog2Ratio` is one below the true floor logarithm, the scaled quotient
  is exactly `2^24`, and the upper premise fails (`upper_premise_fails_for_one_half`). There the
  rounded significand carries into the next exponent and the encoded result is exact
  (`boundary_rounds_exactly`). So the absolute and relative error rules hold for every positive
  input whose carried exponent is in the normal range, with the scale premises discharged rather
  than assumed.

  This is a statement about the exact reference model only. The carried-exponent range remains a
  premise, subnormal results stay open, and no Rust, Wasm or host floating-point correspondence is
  claimed.
-/
import NobleFoundation.Binary32Error

namespace NobleFoundation.Binary32.Scale

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error

/-! ## The model logarithm -/

/-- The fuel-bounded logarithm agrees with the core floor logarithm whenever the fuel covers the
input. -/
theorem log2_go_eq : ∀ (fuel n acc : Nat), n ≤ fuel → log2.go n acc fuel = acc + Nat.log2 n
  | 0, n, acc, h => by
      have hn : n = 0 := by omega
      subst hn
      simp [log2.go]
  | fuel + 1, n, acc, h => by
      by_cases hn : n < 2
      · have hlog : Nat.log2 n = 0 := by
          rw [Nat.log2_def, if_neg (by omega)]
        simp [log2.go, hn, hlog]
      · have hlog : Nat.log2 n = Nat.log2 (n / 2) + 1 := by
          rw [Nat.log2_def, if_pos (by omega)]
        simp only [log2.go, if_neg hn]
        rw [log2_go_eq fuel (n / 2) (acc + 1) (by omega), hlog]
        omega

/-- The model's `log2` is the core floor logarithm. -/
theorem log2_eq (n : Nat) : log2 n = Nat.log2 n := by
  have h := log2_go_eq (n + 1) n 0 (by omega)
  simpa [log2] using h

/-- The floor-logarithm bracket of the model logarithm. -/
theorem log2_bracket (n : Nat) (hn : 0 < n) : 2 ^ log2 n ≤ n ∧ n < 2 ^ (log2 n + 1) := by
  rw [log2_eq]
  exact (Nat.log2_eq_iff (Nat.pos_iff_ne_zero.mp hn)).1 rfl

/-! ## The bracket of `floorLog2Ratio` -/

/-- Ratios below one that are exact powers of two: `a < b` and `b = a · 2^m` with
`m = log2 (b / a)`. -/
def powerBoundary (a b : Nat) : Prop := a < b ∧ a * 2 ^ log2 (b / a) = b

instance (a b : Nat) : Decidable (powerBoundary a b) := by
  unfold powerBoundary
  infer_instance

/-- **The lower bracket holds for every positive input**: `2^e · b ≤ a`. -/
theorem floorLog2Ratio_lower (a b : Nat) (ha : 0 < a) (hb : 0 < b) :
    (2 : ℚ) ^ floorLog2Ratio a b * b ≤ a := by
  unfold floorLog2Ratio
  split
  · rename_i hba
    obtain ⟨hlo, -⟩ := log2_bracket (a / b) (Nat.div_pos hba hb)
    have hnat : 2 ^ log2 (a / b) * b ≤ a :=
      Nat.le_trans (Nat.mul_le_mul_right b hlo) (Nat.div_mul_le_self a b)
    rw [zpow_natCast]
    exact_mod_cast hnat
  · rename_i hba
    have hab : a ≤ b := by omega
    obtain ⟨-, hhi⟩ := log2_bracket (b / a) (Nat.div_pos hab ha)
    have hnat : b ≤ 2 ^ (log2 (b / a) + 1) * a := by
      calc b ≤ a * (b / a + 1) := Nat.le_of_lt (Nat.lt_mul_div_succ b ha)
        _ ≤ a * 2 ^ (log2 (b / a) + 1) := Nat.mul_le_mul_left a hhi
        _ = 2 ^ (log2 (b / a) + 1) * a := Nat.mul_comm _ _
    have hpos : (0 : ℚ) < (2 : ℚ) ^ (log2 (b / a) + 1) := by positivity
    have hexp : (-(((log2 (b / a) : Nat) : Int) + 1) : Int) =
        -(((log2 (b / a) + 1 : Nat)) : Int) := by
      push_cast
      ring
    rw [hexp, zpow_neg, zpow_natCast, inv_mul_le_iff₀ hpos]
    exact_mod_cast hnat

/-- **The strict upper bracket holds off the power boundary**: `a < 2^(e+1) · b`. -/
theorem floorLog2Ratio_upper (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hnot : ¬ powerBoundary a b) : (a : ℚ) < (2 : ℚ) ^ (floorLog2Ratio a b + 1) * b := by
  unfold floorLog2Ratio
  split
  · rename_i hba
    obtain ⟨-, hhi⟩ := log2_bracket (a / b) (Nat.div_pos hba hb)
    have hnat : a < 2 ^ (log2 (a / b) + 1) * b := by
      calc a < b * (a / b + 1) := Nat.lt_mul_div_succ a hb
        _ ≤ b * 2 ^ (log2 (a / b) + 1) := Nat.mul_le_mul_left b hhi
        _ = 2 ^ (log2 (a / b) + 1) * b := Nat.mul_comm _ _
    have hexp : (((log2 (a / b) : Nat) : Int) + 1 : Int) = (((log2 (a / b) + 1 : Nat)) : Int) := by
      push_cast
      ring
    rw [hexp, zpow_natCast]
    exact_mod_cast hnat
  · rename_i hba
    have hab : a < b := by omega
    obtain ⟨hlo, -⟩ := log2_bracket (b / a) (Nat.div_pos (Nat.le_of_lt hab) ha)
    have hle : 2 ^ log2 (b / a) * a ≤ b :=
      Nat.le_trans (Nat.mul_le_mul_right a hlo) (Nat.div_mul_le_self b a)
    have hne : 2 ^ log2 (b / a) * a ≠ b := fun heq =>
      hnot ⟨hab, by rw [Nat.mul_comm]; exact heq⟩
    have hnat : 2 ^ log2 (b / a) * a < b := Nat.lt_of_le_of_ne hle hne
    have hpos : (0 : ℚ) < (2 : ℚ) ^ log2 (b / a) := by positivity
    have hexp : (-(((log2 (b / a) : Nat) : Int) + 1) + 1 : Int) =
        -(((log2 (b / a) : Nat)) : Int) := by
      ring
    rw [hexp, zpow_neg, zpow_natCast, lt_inv_mul_iff₀ hpos]
    exact_mod_cast hnat

/-- **On the power boundary the ratio is exactly `2^(e+1)`**: the upper bracket is attained. -/
theorem floorLog2Ratio_boundary (a b : Nat) (h : powerBoundary a b) :
    (a : ℚ) = (2 : ℚ) ^ (floorLog2Ratio a b + 1) * b := by
  obtain ⟨hab, heq⟩ := h
  unfold floorLog2Ratio
  rw [if_neg (by omega)]
  have hexp : (-(((log2 (b / a) : Nat) : Int) + 1) + 1 : Int) =
      -(((log2 (b / a) : Nat)) : Int) := by
    ring
  have hb : (b : ℚ) = a * 2 ^ log2 (b / a) := by exact_mod_cast heq.symm
  rw [hexp, zpow_neg, zpow_natCast, hb]
  field_simp

/-! ## The scale premises -/

/-- The scaled quotient is the exact ratio times `2^(23 - e)`. -/
theorem scaled_ratio (a b : Nat) (hb : 0 < b) :
    ((scalePair a b).1 : ℚ) / (scalePair a b).2 =
      (a : ℚ) / b * (2 : ℚ) ^ (23 - floorLog2Ratio a b) := by
  have h := scaled_exact_ratio a b hb
  simp only at h
  have hp : (2 : ℚ) ^ (floorLog2Ratio a b - 23) * (2 : ℚ) ^ (23 - floorLog2Ratio a b) = 1 := by
    rw [← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
    norm_num
  calc ((scalePair a b).1 : ℚ) / (scalePair a b).2
      = ((scalePair a b).1 : ℚ) / (scalePair a b).2 *
          ((2 : ℚ) ^ (floorLog2Ratio a b - 23) * (2 : ℚ) ^ (23 - floorLog2Ratio a b)) := by
        rw [hp, mul_one]
    _ = (((scalePair a b).1 : ℚ) / (scalePair a b).2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23)) *
          (2 : ℚ) ^ (23 - floorLog2Ratio a b) := by ring
    _ = (a : ℚ) / b * (2 : ℚ) ^ (23 - floorLog2Ratio a b) := by rw [h]

/-- `2^e · 2^(23 - e)` is `2^23`, and `2^(e+1) · 2^(23 - e)` is `2^24`. -/
theorem scale_powers (e : Int) :
    (2 : ℚ) ^ e * (2 : ℚ) ^ (23 - e) = 2 ^ 23 ∧
      (2 : ℚ) ^ (e + 1) * (2 : ℚ) ^ (23 - e) = 2 ^ 24 := by
  constructor
  · rw [← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0), show e + (23 - e) = (23 : ℕ) by ring,
      zpow_natCast]
  · rw [← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0), show e + 1 + (23 - e) = (24 : ℕ) by ring,
      zpow_natCast]

/-- **The lower scale premise holds for every positive input.** -/
theorem scale_lower (a b : Nat) (ha : 0 < a) (hb : 0 < b) :
    2 ^ 23 * (scalePair a b).2 ≤ (scalePair a b).1 := by
  have hd : (0 : ℚ) < (scalePair a b).2 := by exact_mod_cast scalePair_den_pos a b hb
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  have hlow : (2 : ℚ) ^ floorLog2Ratio a b ≤ (a : ℚ) / b :=
    (le_div_iff₀ hbq).2 (floorLog2Ratio_lower a b ha hb)
  have key : (2 : ℚ) ^ 23 ≤ ((scalePair a b).1 : ℚ) / (scalePair a b).2 := by
    rw [scaled_ratio a b hb, ← (scale_powers (floorLog2Ratio a b)).1]
    exact mul_le_mul_of_nonneg_right hlow (le_of_lt (zpow_pos (by norm_num : (0 : ℚ) < 2) _))
  rw [le_div_iff₀ hd] at key
  have : ((2 ^ 23 * (scalePair a b).2 : Nat) : ℚ) ≤ ((scalePair a b).1 : ℚ) := by
    push_cast
    linarith
  exact_mod_cast this

/-- **The upper scale premise holds off the power boundary.** -/
theorem scale_upper (a b : Nat) (ha : 0 < a) (hb : 0 < b) (hnot : ¬ powerBoundary a b) :
    (scalePair a b).1 < 2 ^ 24 * (scalePair a b).2 := by
  have hd : (0 : ℚ) < (scalePair a b).2 := by exact_mod_cast scalePair_den_pos a b hb
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  have hhigh : (a : ℚ) / b < (2 : ℚ) ^ (floorLog2Ratio a b + 1) :=
    (div_lt_iff₀ hbq).2 (floorLog2Ratio_upper a b ha hb hnot)
  have key : ((scalePair a b).1 : ℚ) / (scalePair a b).2 < (2 : ℚ) ^ 24 := by
    rw [scaled_ratio a b hb, ← (scale_powers (floorLog2Ratio a b)).2]
    exact mul_lt_mul_of_pos_right hhigh (zpow_pos (by norm_num : (0 : ℚ) < 2) _)
  rw [div_lt_iff₀ hd] at key
  have : ((scalePair a b).1 : ℚ) < ((2 ^ 24 * (scalePair a b).2 : Nat) : ℚ) := by
    push_cast
    linarith
  exact_mod_cast this

/-- **On the power boundary the scaled quotient is exactly `2^24`**, so the upper premise fails
there. -/
theorem scale_at_boundary (a b : Nat) (hb : 0 < b) (h : powerBoundary a b) :
    (scalePair a b).1 = 2 ^ 24 * (scalePair a b).2 := by
  have hd : (0 : ℚ) < (scalePair a b).2 := by exact_mod_cast scalePair_den_pos a b hb
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  have hexact : (a : ℚ) / b = (2 : ℚ) ^ (floorLog2Ratio a b + 1) := by
    rw [div_eq_iff (ne_of_gt hbq)]
    exact floorLog2Ratio_boundary a b h
  have key : ((scalePair a b).1 : ℚ) / (scalePair a b).2 = (2 : ℚ) ^ 24 := by
    rw [scaled_ratio a b hb, hexact, (scale_powers (floorLog2Ratio a b)).2]
  rw [div_eq_iff (ne_of_gt hd)] at key
  have : ((scalePair a b).1 : ℚ) = ((2 ^ 24 * (scalePair a b).2 : Nat) : ℚ) := by
    push_cast
    linarith
  exact_mod_cast this

/-- **The upper premise is not universal**: `1/2` is on the power boundary and its scaled
quotient is not below `2^24`. -/
theorem upper_premise_fails_for_one_half :
    powerBoundary 1 2 ∧ ¬ (scalePair 1 2).1 < 2 ^ 24 * (scalePair 1 2).2 := by
  decide

/-! ## The boundary is exact, and the error rules hold without scale premises -/

/-- On the power boundary the rounded significand is exactly `2^24`. -/
theorem boundary_rounded (a b : Nat) (hb : 0 < b) (h : powerBoundary a b) :
    roundedQuotient a b = pow2 24 := by
  have hs := scale_at_boundary a b hb h
  have hd := scalePair_den_pos a b hb
  have hdiv : (scalePair a b).1 / (scalePair a b).2 = 2 ^ 24 := by
    rw [hs, Nat.mul_div_cancel _ hd]
  have hmod : (scalePair a b).1 % (scalePair a b).2 = 0 := by
    rw [hs, Nat.mul_mod_left]
  unfold roundedQuotient roundNearest
  simp only [hdiv, hmod, pow2]
  have hne : ¬ 2 * 0 > (scalePair a b).2 := by omega
  have hne' : ¬ 2 * 0 = (scalePair a b).2 := by omega
  rw [if_neg hne, if_neg hne']

/-- **On the power boundary the result is exact.** The rounded significand `2^24` carries to
`2^23` in the next exponent, which encodes the exact ratio. -/
theorem boundary_rounds_exactly (a b : Nat) (hb : 0 < b) (h : powerBoundary a b)
    (hcarryLo : -126 ≤ (carriedPair a b).2) (hcarryHi : (carriedPair a b).2 ≤ 127) :
    exactValue (BitVec.ofNat 32 (roundPositive a b)) = (a : ℚ) / b := by
  have hq := boundary_rounded a b hb h
  have hcarry : carriedPair a b = (pow2 23, floorLog2Ratio a b + 1) := by
    unfold carriedPair
    simp only [hq, if_true]
  have he₁ : -126 ≤ floorLog2Ratio a b + 1 := by simpa [hcarry] using hcarryLo
  have he₂ : floorLog2Ratio a b + 1 ≤ 127 := by simpa [hcarry] using hcarryHi
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  rw [roundPositive_eq_roundedNormalBits a b hcarryLo hcarryHi]
  unfold roundedNormalBits
  rw [hcarry]
  dsimp only
  rw [exactValue_normalEncoding he₁ he₂ (by norm_num [pow2]) (by norm_num [pow2]),
    eq_div_iff (ne_of_gt hbq), floorLog2Ratio_boundary a b h]
  have hsplit : (2 : ℚ) ^ (floorLog2Ratio a b + 1) =
      (2 : ℚ) ^ (23 : ℕ) * (2 : ℚ) ^ (floorLog2Ratio a b + 1 - 23) := by
    rw [← zpow_natCast, ← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
    congr 1
    push_cast
    ring
  rw [hsplit]
  norm_num [pow2]

/-- **Normal-range absolute error with both scale premises discharged.** For every positive
`a / b` whose carried exponent is in the normal range, the encoded result is within half an ulp
of the exact ratio. -/
theorem roundPositive_absolute_error (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hcarryLo : -126 ≤ (carriedPair a b).2) (hcarryHi : (carriedPair a b).2 ≤ 127) :
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤
      (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23) := by
  by_cases h : powerBoundary a b
  · rw [boundary_rounds_exactly a b hb h hcarryLo hcarryHi, sub_self, abs_zero]
    exact mul_nonneg (by norm_num) (le_of_lt (zpow_pos (by norm_num : (0 : ℚ) < 2) _))
  · exact roundPositive_normal_absolute_error a b ha hb (scale_lower a b ha hb)
      (scale_upper a b ha hb h) hcarryLo hcarryHi

/-- **Normal-range relative error with every premise except the exponent range discharged.** -/
theorem roundPositive_relative_error (a b : Nat) (ha : 0 < a) (hb : 0 < b)
    (hcarryLo : -126 ≤ (carriedPair a b).2) (hcarryHi : (carriedPair a b).2 ≤ 127) :
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤
      (1 / (2 : ℚ) ^ 24) * ((a : ℚ) / b) := by
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  by_cases h : powerBoundary a b
  · rw [boundary_rounds_exactly a b hb h hcarryLo hcarryHi, sub_self, abs_zero]
    exact mul_nonneg (by norm_num) (div_nonneg (Nat.cast_nonneg a) (Nat.cast_nonneg b))
  · exact roundPositive_normal_relative_error a b ha hb (scale_lower a b ha hb)
      (scale_upper a b ha hb h) hcarryLo hcarryHi
      ((le_div_iff₀ hbq).2 (floorLog2Ratio_lower a b ha hb))

/-- **The universal rule covers the boundary**: at `1/2`, where the upper premise fails, the
absolute rule still holds, with error zero. -/
theorem absolute_error_covers_one_half :
    exactValue (BitVec.ofNat 32 (roundPositive 1 2)) = (1 : ℚ) / 2 ∧
      |exactValue (BitVec.ofNat 32 (roundPositive 1 2)) - (1 : ℚ) / 2| ≤
        (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio 1 2 - 23) := by
  have hb : powerBoundary 1 2 := by decide
  have hlo : -126 ≤ (carriedPair 1 2).2 := by decide
  have hhi : (carriedPair 1 2).2 ≤ 127 := by decide
  exact ⟨by simpa using boundary_rounds_exactly 1 2 (by decide) hb hlo hhi,
    roundPositive_absolute_error 1 2 (by decide) (by decide) hlo hhi⟩

end NobleFoundation.Binary32.Scale
