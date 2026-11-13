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

set_option exponentiation.threshold 4096
set_option maxRecDepth 100000

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
          rw [Nat.log2_def, ite_eq_right (by omega)]
        simp [log2.go, hn, hlog]
      · have hlog : Nat.log2 n = Nat.log2 (n / 2) + 1 := by
          rw [Nat.log2_def, ite_eq_left (by omega)]
        simp only [log2.go, ite_eq_right hn]
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
  rw [ite_eq_right (by omega)]
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
  rw [ite_eq_right hne, ite_eq_right hne']

/-! ## Exact normal-bin RNE and the floor-logarithm bracket -/

/-- Rounding a quotient with zero remainder returns the quotient exactly. -/
theorem roundNearest_exact_multiple (m d : Nat) (hd : 0 < d) :
    roundNearest (m * d) d = m := by
  have hdiv : m * d / d = m := by
    rw [Nat.mul_comm m d]
    exact Nat.mul_div_cancel_left m hd
  have hmod : m * d % d = 0 := by
    rw [Nat.mul_comm m d]
    exact Nat.mul_mod_right _ _
  unfold roundNearest
  rw [hdiv, hmod]
  by_cases hgt : 2 * 0 > d
  · omega
  · by_cases heq : 2 * 0 = d
    · omega
    · have hzero : (0 : Nat) ≠ d := by omega
      simp [hzero]

/-- The model floor exponent is uniquely determined by its strict rational bracket. -/
theorem floorLog2Ratio_eq_of_ratio_brackets (a b : Nat) (e : Int)
    (ha : 0 < a) (hb : 0 < b)
    (hlo : (2 : ℚ) ^ e ≤ (a : ℚ) / b)
    (hhi : (a : ℚ) / b < (2 : ℚ) ^ (e + 1))
    (hnot : ¬ powerBoundary a b) :
    floorLog2Ratio a b = e := by
  let q := floorLog2Ratio a b
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  have hqlo : (2 : ℚ) ^ q ≤ (a : ℚ) / b :=
    (le_div_iff₀ hbq).2 (floorLog2Ratio_lower a b ha hb)
  have hqhi : (a : ℚ) / b < (2 : ℚ) ^ (q + 1) :=
    (div_lt_iff₀ hbq).2 (floorLog2Ratio_upper a b ha hb hnot)
  apply le_antisymm
  · by_contra h
    have heq : e < q := by omega
    have hp : (2 : ℚ) ^ (e + 1) ≤ (2 : ℚ) ^ q :=
      zpow_le_zpow_right₀ (by norm_num) (by omega)
    linarith
  · by_contra h
    have heq : q < e := by omega
    have hp : (2 : ℚ) ^ (q + 1) ≤ (2 : ℚ) ^ e :=
      zpow_le_zpow_right₀ (by norm_num) (by omega)
    linarith

/-- A normal exact significand is rounded once and encoded as the corresponding normal bits.
This covers exact dyadic inputs, including power-boundary inputs where the logarithm model's
strict upper bracket is attained and rounding carries into the next exponent. -/
theorem roundPositive_eq_normal_encoding_of_dyadic (a b : Nat) (e : Int) (m : Nat)
    (hb : 0 < b) (heLo : -126 ≤ e) (heHi : e ≤ 127)
    (hmLo : 2 ^ 23 ≤ m) (hmHi : m < 2 ^ 24)
    (hratio : (a : ℚ) / b = (m : ℚ) * (2 : ℚ) ^ (e - 23)) :
    BitVec.ofNat 32 (roundPositive a b) = normalEncoding e m := by
  have hpow23 : (2 : ℚ) ^ e = (2 : ℚ) ^ 23 * (2 : ℚ) ^ (e - 23) := by
    calc
      (2 : ℚ) ^ e = (2 : ℚ) ^ (23 + (e - 23)) := by congr 1; ring
      _ = (2 : ℚ) ^ 23 * (2 : ℚ) ^ (e - 23) :=
        zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0) _ _
  have hpow24 : (2 : ℚ) ^ (e + 1) = (2 : ℚ) ^ 24 * (2 : ℚ) ^ (e - 23) := by
    calc
      (2 : ℚ) ^ (e + 1) = (2 : ℚ) ^ (24 + (e - 23)) := by congr 1; ring
      _ = (2 : ℚ) ^ 24 * (2 : ℚ) ^ (e - 23) :=
        zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0) _ _
  have hmLoQ : (2 : ℚ) ^ 23 ≤ (m : ℚ) := by exact_mod_cast hmLo
  have hmHiQ : (m : ℚ) < (2 : ℚ) ^ 24 := by exact_mod_cast hmHi
  have hscalePos : 0 < (2 : ℚ) ^ (e - 23) := zpow_pos (by norm_num) _
  have hlo : (2 : ℚ) ^ e ≤ (a : ℚ) / b := by
    rw [hratio, hpow23]
    exact mul_le_mul_of_nonneg_right hmLoQ (le_of_lt hscalePos)
  have hhi : (a : ℚ) / b < (2 : ℚ) ^ (e + 1) := by
    rw [hratio, hpow24]
    exact mul_lt_mul_of_pos_right hmHiQ hscalePos
  by_cases boundary : powerBoundary a b
  · have hbq : (0 : ℚ) < b := by exact_mod_cast hb
    have hboundary : (a : ℚ) / b = (2 : ℚ) ^ (floorLog2Ratio a b + 1) := by
      rw [div_eq_iff (ne_of_gt hbq)]
      exact floorLog2Ratio_boundary a b boundary
    have hfloor : floorLog2Ratio a b + 1 = e := by
      apply le_antisymm
      · by_contra h
        have hexp : e < floorLog2Ratio a b + 1 := by omega
        have hp : (2 : ℚ) ^ (e + 1) ≤ (2 : ℚ) ^ (floorLog2Ratio a b + 1) :=
          zpow_le_zpow_right₀ (by norm_num) (by omega)
        linarith [hboundary, hhi]
      · by_contra h
        have hexp : floorLog2Ratio a b + 1 < e := by omega
        have hp : (2 : ℚ) ^ (floorLog2Ratio a b + 1) < (2 : ℚ) ^ e :=
          zpow_lt_zpow_right₀ (by norm_num : (1 : ℚ) < 2) hexp
        linarith [hboundary, hlo]
    have hpowne : (2 : ℚ) ^ (e - 23) ≠ 0 := zpow_ne_zero _ (by norm_num)
    have hrpow : (2 : ℚ) ^ e = (m : ℚ) * (2 : ℚ) ^ (e - 23) := by
      calc
        (2 : ℚ) ^ e = (a : ℚ) / b :=
          (hboundary.trans (by rw [hfloor])).symm
        _ = (m : ℚ) * (2 : ℚ) ^ (e - 23) := hratio
    have hprod : (m : ℚ) * (2 : ℚ) ^ (e - 23) =
        (2 : ℚ) ^ 23 * (2 : ℚ) ^ (e - 23) := hrpow.symm.trans hpow23
    have hmult : ((m : ℚ) - (2 : ℚ) ^ 23) * (2 : ℚ) ^ (e - 23) = 0 := by
      calc
        ((m : ℚ) - (2 : ℚ) ^ 23) * (2 : ℚ) ^ (e - 23) =
            (m : ℚ) * (2 : ℚ) ^ (e - 23) -
              (2 : ℚ) ^ 23 * (2 : ℚ) ^ (e - 23) := by ring
        _ = 0 := sub_eq_zero.mpr hprod
    have hmEqQ : (m : ℚ) = (2 : ℚ) ^ 23 := by
      rcases mul_eq_zero.mp hmult with hzero | hpzero
      · exact sub_eq_zero.mp hzero
      · exact (hpowne hpzero).elim
    have hmEq : m = 2 ^ 23 := by exact_mod_cast hmEqQ
    have hrounded : roundedQuotient a b = pow2 24 := boundary_rounded a b hb boundary
    have hcarry : carriedPair a b = (pow2 23, e) := by
      unfold carriedPair
      rw [hrounded]
      simp [hfloor, pow2]
    have hnormal : roundedNormalBits a b = normalEncoding e m := by
      simp [roundedNormalBits, hcarry, normalEncoding, hmEq, pow2]
    have hloCarry : -126 ≤ (carriedPair a b).2 := by rw [hcarry]; exact heLo
    have hhiCarry : (carriedPair a b).2 ≤ 127 := by rw [hcarry]; exact heHi
    exact (roundPositive_eq_roundedNormalBits a b hloCarry hhiCarry).trans hnormal
  · have hfloor := floorLog2Ratio_eq_of_ratio_brackets a b e
      (by
        by_contra hnotPositive
        have haZero : a = 0 := by omega
        rw [haZero] at hratio
        norm_num at hratio
        rcases hratio with hmZero | hpZero
        · have hmPositive : 0 < m := lt_of_lt_of_le (by norm_num) hmLo
          omega
        · exact (zpow_ne_zero _ (by norm_num : (2 : ℚ) ≠ 0)) hpZero)
      hb hlo hhi boundary
    have hdenNat : 0 < (scalePair a b).2 := scalePair_den_pos a b hb
    have hden : (0 : ℚ) < (scalePair a b).2 := by exact_mod_cast hdenNat
    have hscaled := scaled_ratio a b hb
    rw [hfloor, hratio] at hscaled
    have hpowcancel : (2 : ℚ) ^ (e - 23) * (2 : ℚ) ^ (23 - e) = 1 := by
      rw [← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
      have : e - 23 + (23 - e) = 0 := by ring
      rw [this]
      norm_num
    have hscaledRatio : ((scalePair a b).1 : ℚ) / (scalePair a b).2 = m := by
      calc
        ((scalePair a b).1 : ℚ) / (scalePair a b).2 =
            (m : ℚ) * ((2 : ℚ) ^ (e - 23) * (2 : ℚ) ^ (23 - e)) := by
              simpa [mul_assoc] using hscaled
        _ = m := by rw [hpowcancel, mul_one]
    have hscaledMul : (scalePair a b).1 = m * (scalePair a b).2 := by
      have h := (div_eq_iff (ne_of_gt hden)).mp hscaledRatio
      exact_mod_cast h
    have hrounded : roundedQuotient a b = m := by
      unfold roundedQuotient
      dsimp only
      rw [hscaledMul]
      exact roundNearest_exact_multiple m (scalePair a b).2 hdenNat
    have hnotcarry : m ≠ pow2 24 := by
      intro h
      rw [h, pow2] at hmHi
      omega
    have hcarry : carriedPair a b = (m, e) := by
      unfold carriedPair
      rw [hfloor, hrounded]
      simp [hnotcarry]
    have hnormal : roundedNormalBits a b = normalEncoding e m := by
      simp [roundedNormalBits, hcarry, normalEncoding]
    have hloCarry : -126 ≤ (carriedPair a b).2 := by rw [hcarry]; exact heLo
    have hhiCarry : (carriedPair a b).2 ≤ 127 := by rw [hcarry]; exact heHi
    exact (roundPositive_eq_roundedNormalBits a b hloCarry hhiCarry).trans hnormal

/-- Exact-value consequence of the normal-bin RNE theorem. -/
theorem exactValue_roundPositive_of_dyadic (a b : Nat) (e : Int) (m : Nat)
    (hb : 0 < b) (heLo : -126 ≤ e) (heHi : e ≤ 127)
    (hmLo : 2 ^ 23 ≤ m) (hmHi : m < 2 ^ 24)
    (hratio : (a : ℚ) / b = (m : ℚ) * (2 : ℚ) ^ (e - 23)) :
    exactValue (BitVec.ofNat 32 (roundPositive a b)) =
      (m : ℚ) * (2 : ℚ) ^ (e - 23) := by
  rw [roundPositive_eq_normal_encoding_of_dyadic a b e m hb heLo heHi hmLo hmHi hratio]
  exact exactValue_normalEncoding heLo heHi hmLo hmHi

/-- **On the power boundary the result is exact.** The rounded significand `2^24` carries to
`2^23` in the next exponent, which encodes the exact ratio. -/
theorem boundary_rounds_exactly (a b : Nat) (hb : 0 < b) (h : powerBoundary a b)
    (hcarryLo : -126 ≤ (carriedPair a b).2) (hcarryHi : (carriedPair a b).2 ≤ 127) :
    exactValue (BitVec.ofNat 32 (roundPositive a b)) = (a : ℚ) / b := by
  have hq := boundary_rounded a b hb h
  have hcarry : carriedPair a b = (pow2 23, floorLog2Ratio a b + 1) := by
    unfold carriedPair
    simp only [hq, ite_true]
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

/-! ## The binary32 overflow boundary -/

/-- The midpoint between the largest finite binary32 value and the overflow result. -/
def overflowMidpoint : Nat := 2 ^ 128 - 2 ^ 103

theorem overflowMidpoint_eq_maxFinite_plus_halfUlp :
    overflowMidpoint = (2 ^ 24 - 1) * 2 ^ 104 + 2 ^ 103 := by
  norm_num [overflowMidpoint]

/-- Immediately below the overflow midpoint, the reference selects the largest finite value. -/
theorem roundPositive_belowOverflowMidpoint :
    roundPositive (overflowMidpoint - 1) 1 = 255 * 2 ^ 23 - 1 := by
  decide

/-- At the exact halfway point, ties-to-even selects the overflow result. -/
theorem roundPositive_overflowMidpoint :
    roundPositive overflowMidpoint 1 = 255 * 2 ^ 23 := by
  decide

private theorem roundNearest_overflowMidpoint_iff (n d : Nat) (hd : 0 < d)
    (upper : n < 2 ^ 24 * d) :
    roundNearest n d = 2 ^ 24 ↔ 2 * n ≥ (2 ^ 25 - 1) * d := by
  have quotientUpper : n / d ≤ 2 ^ 24 - 1 := by
    have quotientStrict : n / d < 2 ^ 24 := (Nat.div_lt_iff_lt_mul hd).2 upper
    omega
  have remainderBound : n % d < d := Nat.mod_lt n hd
  have division : n = (n / d) * d + n % d := by
    simpa [Nat.mul_comm] using (Nat.div_add_mod n d).symm
  unfold roundNearest
  by_cases aboveHalf : 2 * (n % d) > d
  · rw [ite_eq_left aboveHalf]
    constructor
    · intro rounded
      have quotient : n / d = 2 ^ 24 - 1 := by omega
      rw [quotient] at division
      omega
    · intro threshold
      have quotient : n / d = 2 ^ 24 - 1 := by
        by_contra other
        have lower : n / d < 2 ^ 24 - 1 := by omega
        have numerator : n < (2 ^ 24 - 1) * d :=
          (Nat.div_lt_iff_lt_mul hd).mp lower
        omega
      omega
  · by_cases tie : 2 * (n % d) = d
    · rw [ite_eq_right aboveHalf, ite_eq_left tie]
      constructor
      · intro rounded
        have quotient : n / d = 2 ^ 24 - 1 := by omega
        rw [quotient] at division
        omega
      · intro threshold
        have quotient : n / d = 2 ^ 24 - 1 := by
          by_contra other
          have lower : n / d < 2 ^ 24 - 1 := by omega
          have numerator : n < (2 ^ 24 - 1) * d :=
            (Nat.div_lt_iff_lt_mul hd).mp lower
          omega
        simp [quotient]
    · rw [ite_eq_right aboveHalf, ite_eq_right tie]
      constructor
      · intro rounded
        omega
      · intro threshold
        have quotient : n / d = 2 ^ 24 - 1 := by
          by_contra other
          have lower : n / d < 2 ^ 24 - 1 := by omega
          have numerator : n < (2 ^ 24 - 1) * d :=
            (Nat.div_lt_iff_lt_mul hd).mp lower
          omega
        rw [quotient] at division
        omega

private theorem carriedPair_lower_exponent (a b : Nat) :
    floorLog2Ratio a b ≤ (carriedPair a b).2 := by
  unfold carriedPair
  dsimp only
  split
  · omega
  · omega

private theorem carriedPair_upper_exponent (a b : Nat) :
    (carriedPair a b).2 ≤ floorLog2Ratio a b + 1 := by
  unfold carriedPair
  dsimp only
  split
  · omega
  · omega

private theorem overflowMidpoint_double :
    2 * overflowMidpoint = (2 ^ 25 - 1) * 2 ^ 104 := by
  norm_num [overflowMidpoint]

private theorem overflowMidpoint_above_twoPow127 :
    (2 : ℚ) ^ (127 : Int) < (overflowMidpoint : ℚ) := by
  norm_num [overflowMidpoint]

private theorem overflowMidpoint_at_most_twoPow128 :
    (overflowMidpoint : ℚ) ≤ (2 : ℚ) ^ (128 : Int) := by
  norm_num [overflowMidpoint]

/-- **Exact finite-range boundary.** For every positive natural ratio `a / b`, the rounding
    carried exponent is finite exactly below the ties-to-even overflow midpoint
    `2^128 - 2^103`. Equality is overflow because the tie rounds to the even infinity result. -/
theorem carriedPair_finite_iff_below_overflowMidpoint (a b : Nat)
    (ha : 0 < a) (hb : 0 < b) :
    (carriedPair a b).2 ≤ 127 ↔ a < b * overflowMidpoint := by
  let e := floorLog2Ratio a b
  have exponent : e = floorLog2Ratio a b := rfl
  by_cases below : e < 127
  · have ratioBound : (a : ℚ) ≤ (2 : ℚ) ^ 127 * b := by
      by_cases boundary : powerBoundary a b
      · have exact := floorLog2Ratio_boundary a b boundary
        have powerBound : (2 : ℚ) ^ (e + 1) ≤ (2 : ℚ) ^ (127 : Int) :=
          zpow_le_zpow_right₀ (by norm_num) (by omega)
        rw [← exponent] at exact
        rw [exact]
        exact mul_le_mul_of_nonneg_right powerBound (Nat.cast_nonneg b)
      · have strict := floorLog2Ratio_upper a b ha hb boundary
        have powerBound : (2 : ℚ) ^ (e + 1) ≤ (2 : ℚ) ^ (127 : Int) :=
          zpow_le_zpow_right₀ (by norm_num) (by omega)
        rw [← exponent] at strict
        exact le_trans (le_of_lt strict)
          (mul_le_mul_of_nonneg_right powerBound (Nat.cast_nonneg b))
    have midpointBound :
        (a : ℚ) < (overflowMidpoint : ℚ) * b :=
      lt_of_le_of_lt ratioBound
        (mul_lt_mul_of_pos_right overflowMidpoint_above_twoPow127
          (by exact_mod_cast hb))
    have naturalBound : a < b * overflowMidpoint := by
      have castBound : (a : ℚ) < ((b * overflowMidpoint : Nat) : ℚ) := by
        simpa [Nat.cast_mul, mul_comm] using midpointBound
      exact_mod_cast castBound
    have finite : (carriedPair a b).2 ≤ 127 := by
      have carry := carriedPair_upper_exponent a b
      rw [exponent] at below
      omega
    exact ⟨fun _ => naturalBound, fun _ => finite⟩
  · by_cases atBoundary : e = 127
    · have notPowerBoundary : ¬ powerBoundary a b := by
        intro boundary
        have exact := floorLog2Ratio_boundary a b boundary
        rw [← exponent, atBoundary] at exact
        have bigger : (b : ℚ) < (a : ℚ) := by
          rw [exact]
          have power : (1 : ℚ) < (2 : ℚ) ^ (128 : Int) := by norm_num
          have positive : (0 : ℚ) < b := by exact_mod_cast hb
          rw [show (127 : Int) + 1 = 128 by omega]
          nlinarith [mul_pos (sub_pos.mpr power) positive]
        have smaller : (a : ℚ) < b := by exact_mod_cast boundary.1
        linarith
      have highScale := scale_upper a b ha hb notPowerBoundary
      have scale : scalePair a b = (a, b * 2 ^ 104) := by
        unfold scalePair
        rw [← exponent, atBoundary]
        norm_num [pow2Int, pow2]
      rw [scale] at highScale
      have highNumerator : a < 2 ^ 24 * (b * 2 ^ 104) := by
        simpa using highScale
      have denominator : 0 < b * 2 ^ 104 := Nat.mul_pos hb (by norm_num)
      have rounded := roundNearest_overflowMidpoint_iff a (b * 2 ^ 104)
        denominator highNumerator
      have midpointScale : (2 ^ 25 - 1) * (b * 2 ^ 104) = 2 * (b * overflowMidpoint) := by
        calc
          (2 ^ 25 - 1) * (b * 2 ^ 104) = b * ((2 ^ 25 - 1) * 2 ^ 104) := by ring
          _ = b * (2 * overflowMidpoint) := by rw [overflowMidpoint_double]
          _ = 2 * (b * overflowMidpoint) := by ring
      have roundedThreshold :
          roundNearest a (b * 2 ^ 104) = 2 ^ 24 ↔ b * overflowMidpoint ≤ a := by
        rw [rounded, midpointScale]
        omega
      have scaled : roundedQuotient a b = roundNearest a (b * 2 ^ 104) := by
        unfold roundedQuotient
        rw [scale]
      have finiteCarry :
          (carriedPair a b).2 ≤ 127 ↔ roundedQuotient a b ≠ 2 ^ 24 := by
        unfold carriedPair
        dsimp only
        rw [← exponent, atBoundary]
        by_cases isCarry : roundedQuotient a b = pow2 24
        · have carry : roundedQuotient a b = 2 ^ 24 := by simpa [pow2] using isCarry
          rw [ite_eq_left isCarry]
          constructor
          · intro impossible
            omega
          · intro noCarry
            exact (noCarry carry).elim
        · have noCarry : roundedQuotient a b ≠ 2 ^ 24 := by simpa [pow2] using isCarry
          rw [ite_eq_right isCarry]
          constructor
          · intro _
            exact noCarry
          · intro _
            norm_num
      have final :
          (carriedPair a b).2 ≤ 127 ↔ a < b * overflowMidpoint := by
        rw [finiteCarry, scaled]
        constructor
        · intro noCarry
          by_contra notSmall
          have overflow : b * overflowMidpoint ≤ a := Nat.le_of_not_gt notSmall
          exact noCarry (roundedThreshold.mpr overflow)
        · intro small carry
          have overflow : b * overflowMidpoint ≤ a := roundedThreshold.mp carry
          omega
      exact final
    · have above : 127 < e := by omega
      have ratioLower : (2 : ℚ) ^ (128 : Int) * b ≤ a := by
        have lower := floorLog2Ratio_lower a b ha hb
        have exponentBound : (128 : Int) ≤ e := by omega
        have powerBound : (2 : ℚ) ^ (128 : Int) ≤ (2 : ℚ) ^ e :=
          zpow_le_zpow_right₀ (by norm_num) exponentBound
        exact le_trans
          (mul_le_mul_of_nonneg_right powerBound (Nat.cast_nonneg b)) lower
      have naturalLower : b * overflowMidpoint ≤ a := by
        have castLower : ((b * overflowMidpoint : Nat) : ℚ) ≤ a := by
          simpa [Nat.cast_mul, mul_comm] using
            le_trans (mul_le_mul_of_nonneg_right overflowMidpoint_at_most_twoPow128
              (Nat.cast_nonneg b)) ratioLower
        exact_mod_cast castLower
      have notFinite : ¬ (carriedPair a b).2 ≤ 127 := by
        have carried := carriedPair_lower_exponent a b
        omega
      constructor
      · intro finite
        exact (notFinite finite).elim
      · intro small
        have : ¬ b * overflowMidpoint ≤ a := Nat.not_le_of_gt small
        exact (this naturalLower).elim

end NobleFoundation.Binary32.Scale
