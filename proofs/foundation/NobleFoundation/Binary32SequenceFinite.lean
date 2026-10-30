/-
  Rounding-error bounds for operation sequences with subnormal intermediates (issue #44).

  `Binary32Sequence.lean` bounds the ordered affine fold when every rounding's carried exponent is
  in the normal range. This module drops the lower half of that premise. Below the normal range a
  rounding is within `η = 2^-150`, half the smallest subnormal spacing, and that error cannot be
  relative (`Subnormal.subnormal_results_are_not_relatively_bounded`). So every finite rounding
  satisfies the mixed rule `|rn(x) - x| ≤ u · |x| + η`, with `u = 2^-24`.

  - `roundPositive_mixed_error`: the mixed rule for one positive rounding whose result does not
    overflow;
  - `add_mixed_error`, `mul_mixed_error`: the mixed rule for one model addition or multiplication;
  - `fold_finite_error`: the ordered affine fold of `n` terms is within
    `((1 + u)^(2n) - 1) · (|bias| + Σ|cᵢ · xᵢ|) + 3n(1 + u)^n · η` of its exact value.

  Overflow stays outside the premises. This is a statement about the exact reference model only: no
  Rust, Wasm or host floating-point correspondence is claimed.
-/
import NobleFoundation.Binary32Sequence
import NobleFoundation.Binary32Subnormal

namespace NobleFoundation.Binary32.SequenceFinite

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error
open NobleFoundation.Binary32.Scale
open NobleFoundation.Binary32.Subnormal
open NobleFoundation.Binary32.Sequence

set_option exponentiation.threshold 4096

/-! ## One finite rounding -/

/-- Half the smallest subnormal spacing: the absolute error allowance below the normal range. -/
def eta : ℚ := (2 : ℚ) ^ (-150 : ℤ)

theorem eta_pos : 0 < eta := by
  unfold eta
  exact zpow_pos (by norm_num) _

/-- The premise of the finite rules: the carried exponent does not overflow. -/
def NoOverflow (args : Nat × Nat) : Prop := (carriedPair args.1 args.2).2 ≤ 127

/-- A rounding that does not overflow leaves the sign bit clear: in the normal range by
`roundPositive_lt`, and below it the significand is at most `2^23`. -/
theorem roundPositive_lt_finite (a b : Nat) (ha : 0 < a) (hb : 0 < b) (finite : NoOverflow (a, b)) :
    roundPositive a b < 2 ^ 31 := by
  unfold NoOverflow at finite
  dsimp only at finite
  by_cases normal : -126 ≤ (carriedPair a b).2
  · exact roundPositive_lt a b ha hb ⟨normal, finite⟩
  · rw [roundPositive_eq_significand a b ha hb (by omega)]
    have bound := subnormal_significand_bound a b ha hb (by omega)
    omega

/-- **The mixed rule for one rounding**: a positive ratio whose rounded result does not overflow is
within `u · (a / b) + η` of it. -/
theorem roundPositive_mixed_error (a b : Nat) (ha : 0 < a) (hb : 0 < b) (finite : NoOverflow (a, b)) :
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤ unit * ((a : ℚ) / b) + eta := by
  unfold NoOverflow at finite
  dsimp only at finite
  have absolute := roundPositive_finite_absolute_error a b ha hb finite
  have hbq : (0 : ℚ) < b := by exact_mod_cast hb
  have ratioNonneg : 0 ≤ (a : ℚ) / b := div_nonneg (Nat.cast_nonneg a) (le_of_lt hbq)
  have scaledNonneg : 0 ≤ unit * ((a : ℚ) / b) := mul_nonneg unit_nonneg ratioNonneg
  refine le_trans absolute ?_
  by_cases normal : -126 ≤ floorLog2Ratio a b
  · rw [max_eq_left normal]
    have power : (2 : ℚ) ^ floorLog2Ratio a b ≤ (a : ℚ) / b :=
      (le_div_iff₀ hbq).2 (floorLog2Ratio_lower a b ha hb)
    have split : (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23) =
        unit * (2 : ℚ) ^ floorLog2Ratio a b := by
      rw [zpow_sub₀ (by norm_num : (2 : ℚ) ≠ 0)]
      unfold unit
      norm_num
      ring
    rw [split]
    have weakened := mul_le_mul_of_nonneg_left power unit_nonneg
    linarith [eta_pos]
  · rw [max_eq_right (by omega)]
    have value : (1 : ℚ) / 2 * (2 : ℚ) ^ ((-126 : ℤ) - 23) = eta := by
      unfold eta
      norm_num
    rw [value]
    linarith

/-- **The mixed rule for one signed rounding**: rounding `m · 2^s` with the sign of `m` is within
`u · |m · 2^s| + η` of it whenever the result does not overflow. -/
theorem roundScaled_mixed_error (m s : Int) (nonzero : m ≠ 0) (finite : NoOverflow (scaledArgs m s)) :
    |exactValue (roundScaled m s (decide (m < 0))) - (m : ℚ) * (2 : ℚ) ^ s| ≤
      unit * |(m : ℚ) * (2 : ℚ) ^ s| + eta := by
  obtain ⟨numeratorPos, denominatorPos⟩ := scaledArgs_pos m s nonzero
  have mixed := roundPositive_mixed_error (scaledArgs m s).1 (scaledArgs m s).2 numeratorPos
    denominatorPos finite
  rw [scaledArgs_ratio] at mixed
  have small := roundPositive_lt_finite (scaledArgs m s).1 (scaledArgs m s).2 numeratorPos
    denominatorPos finite
  have power : (0 : ℚ) < (2 : ℚ) ^ s := zpow_pos (by norm_num) s
  have magnitude : |(m : ℚ) * (2 : ℚ) ^ s| = (m.natAbs : ℚ) * (2 : ℚ) ^ s := by
    rw [abs_mul, abs_of_pos power, Nat.cast_natAbs, Int.cast_abs]
  have roundedIs : roundScaled m s (decide (m < 0)) =
      BitVec.ofNat 32 (if decide (m < 0) then roundPositive (scaledArgs m s).1 (scaledArgs m s).2 |||
        0x80000000 else roundPositive (scaledArgs m s).1 (scaledArgs m s).2) := rfl
  rw [roundedIs, magnitude]
  by_cases negative : m < 0
  · simp only [negative, decide_true, if_true]
    rw [exactValue_signed _ small]
    have signedValue : (m : ℚ) * (2 : ℚ) ^ s = -((m.natAbs : ℚ) * (2 : ℚ) ^ s) := by
      have : (m : ℚ) = -(m.natAbs : ℚ) := by
        rw [Nat.cast_natAbs, Int.cast_abs, abs_of_neg (by exact_mod_cast negative)]
        ring
      rw [this]
      ring
    rw [signedValue, show ∀ p q : ℚ, -p - -q = -(p - q) from fun p q => by ring, abs_neg]
    exact mixed
  · simp only [negative, decide_false, Bool.false_eq_true, if_false]
    have signedValue : (m : ℚ) * (2 : ℚ) ^ s = (m.natAbs : ℚ) * (2 : ℚ) ^ s := by
      rw [Nat.cast_natAbs, Int.cast_abs, abs_of_nonneg (by exact_mod_cast not_lt.mp negative)]
    rw [signedValue]
    exact mixed

/-! ## One operation -/

/-- The premises of one finite addition: finite operands, and no overflow whenever the exact sum is
nonzero. -/
def AddFinite (x y : Bits) : Prop :=
  Finite x ∧ Finite y ∧
    ((addScaled x y).1 ≠ 0 → NoOverflow (scaledArgs (addScaled x y).1 (addScaled x y).2))

/-- **One finite addition satisfies the mixed rule.** -/
theorem add_mixed_error (x y : Bits) (ok : AddFinite x y) :
    |exactValue (add x y) - (exactValue x + exactValue y)| ≤
      unit * |exactValue x + exactValue y| + eta := by
  obtain ⟨finiteX, finiteY, range⟩ := ok
  rw [add_eq x y finiteX finiteY, exact_sum]
  split
  · rename_i zero
    rw [zero, exactValue_zeroOf]
    simp only [Int.cast_zero, zero_mul, sub_zero, abs_zero, mul_zero, zero_add]
    exact le_of_lt eta_pos
  · rename_i nonzero
    exact roundScaled_mixed_error _ _ nonzero (range nonzero)

/-- The premises of one finite multiplication. -/
def MulFinite (x y : Bits) : Prop :=
  Finite x ∧ Finite y ∧
    ((mulScaled x y).1 ≠ 0 → NoOverflow (scaledArgs (mulScaled x y).1 (mulScaled x y).2))

/-- **One finite multiplication satisfies the mixed rule.** -/
theorem mul_mixed_error (x y : Bits) (ok : MulFinite x y) :
    |exactValue (mul x y) - exactValue x * exactValue y| ≤
      unit * |exactValue x * exactValue y| + eta := by
  obtain ⟨⟨xNaN, xInfinite⟩, ⟨yNaN, yInfinite⟩, range⟩ := ok
  by_cases zeroX : classify x = .posZero ∨ classify x = .negZero
  · have result : mul x y = zeroOf (isNegative x != isNegative y) := by
      unfold mul
      rcases zeroX with classX | classX <;>
      · rw [classX]
        cases classY : classify y <;> simp_all
    rw [result, exactValue_zeroOf, zero_class_value x zeroX]
    simp only [zero_mul, sub_zero, abs_zero, mul_zero, zero_add]
    exact le_of_lt eta_pos
  by_cases zeroY : classify y = .posZero ∨ classify y = .negZero
  · have result : mul x y = zeroOf (isNegative x != isNegative y) := by
      unfold mul
      rcases zeroY with classY | classY <;>
      · rw [classY]
        cases classX : classify x <;> simp_all
    rw [result, exactValue_zeroOf, zero_class_value y zeroY]
    simp only [mul_zero, sub_zero, abs_zero, zero_add]
    exact le_of_lt eta_pos
  have nonzeroX : classify x = .subnormal ∨ classify x = .normal := by
    cases classX : classify x <;> simp_all
  have nonzeroY : classify y = .subnormal ∨ classify y = .normal := by
    cases classY : classify y <;> simp_all
  obtain ⟨nonzero, _⟩ := product_sign x y (nonzero_class_magnitude x nonzeroX)
    (nonzero_class_magnitude y nonzeroY)
  rw [mul_scaled_eq x y nonzeroX nonzeroY, exact_product]
  exact roundScaled_mixed_error _ _ nonzero (range nonzero)

/-! ## The ordered affine fold -/

/-- The premises of a finite fold: each product and accumulation meets its finite premises on the
actual intermediate accumulator. -/
def FoldFinite : Bits → List (Bits × Bits) → Prop
  | _, [] => True
  | accumulator, (coefficient, input) :: rest =>
      MulFinite coefficient input ∧ AddFinite accumulator (mul coefficient input) ∧
        FoldFinite (add accumulator (mul coefficient input)) rest

/-- **One step of the mixed error recursion**: the relative part grows as in `step_bound`, and the
absolute part `H` becomes `(1 + u) · H + (2 + u) · η`. -/
theorem step_bound_mixed (u r H η A E a t p v : ℚ) (hu : 0 ≤ u) (hr : 0 ≤ r) (hA : 0 ≤ A)
    (hE : |E| ≤ A) (ha : |a - E| ≤ r * A + H)
    (hp : |p - t| ≤ u * |t| + η) (hv : |v - (a + p)| ≤ u * |a + p| + η) :
    |v - (E + t)| ≤ ((1 + u) ^ 2 * (1 + r) - 1) * (A + |t|) + ((1 + u) * H + (2 + u) * η) := by
  have tNonneg : 0 ≤ |t| := abs_nonneg t
  have total : |v - (E + t)| ≤ |v - (a + p)| + |a - E| + |p - t| := by
    have first := abs_add_le (v - (a + p) + (a - E)) (p - t)
    have second := abs_add_le (v - (a + p)) (a - E)
    have same : v - (E + t) = v - (a + p) + (a - E) + (p - t) := by ring
    rw [same]
    linarith
  have sumBound : |a + p| ≤ A + r * A + H + |t| + u * |t| + η := by
    have first := abs_add_le (E + (a - E) + t) (p - t)
    have second := abs_add_le (E + (a - E)) t
    have third := abs_add_le E (a - E)
    have same : a + p = E + (a - E) + t + (p - t) := by ring
    rw [same]
    linarith
  have vBound : |v - (a + p)| ≤ u * (A + r * A + H + |t| + u * |t| + η) + η := by
    have scaled := mul_le_mul_of_nonneg_left sumBound hu
    linarith
  nlinarith [mul_nonneg hu hA, mul_nonneg (mul_nonneg hu hu) hA, mul_nonneg (mul_nonneg hu hr) hA,
    mul_nonneg (mul_nonneg (mul_nonneg hu hu) hr) hA, mul_nonneg hr tNonneg,
    mul_nonneg (mul_nonneg hu hr) tNonneg, mul_nonneg (mul_nonneg (mul_nonneg hu hu) hr) tNonneg]

/-- The absolute part after `k` steps is at most `3k(1 + u)^k · η`. -/
theorem absolute_growth (k : Nat) :
    (1 + unit) * (3 * (k : ℚ) * (1 + unit) ^ k * eta) + (2 + unit) * eta ≤
      3 * ((k + 1 : Nat) : ℚ) * (1 + unit) ^ (k + 1) * eta := by
  have unitNonneg := unit_nonneg
  have unitSmall : unit ≤ 1 := by
    unfold unit
    norm_num
  have etaPos := eta_pos
  have powerOne : 1 ≤ (1 + unit) ^ k := one_le_pow₀ (by linarith)
  have grown : 1 ≤ (1 + unit) ^ k * (1 + unit) :=
    one_le_mul_of_one_le_of_one_le powerOne (by linarith)
  have scaled := mul_le_mul_of_nonneg_right grown (le_of_lt etaPos)
  rw [pow_succ]
  push_cast
  nlinarith [mul_nonneg (sub_nonneg.mpr unitSmall) (le_of_lt etaPos)]

/-- The fold from an accumulator already within the mixed bound after `k` steps. -/
theorem fold_finite_error_from (terms : List (Bits × Bits)) :
    ∀ (accumulator : Bits) (E A : ℚ) (k : Nat), FoldFinite accumulator terms →
      |exactValue accumulator - E| ≤
        ((1 + unit) ^ (2 * k) - 1) * A + 3 * (k : ℚ) * (1 + unit) ^ k * eta →
      |E| ≤ A → 0 ≤ A →
      |exactValue (fold accumulator terms) -
          (E + (terms.map fun term => exactValue term.1 * exactValue term.2).sum)| ≤
        ((1 + unit) ^ (2 * (k + terms.length)) - 1) *
            (A + (terms.map fun term => |exactValue term.1 * exactValue term.2|).sum) +
          3 * ((k + terms.length : Nat) : ℚ) * (1 + unit) ^ (k + terms.length) * eta := by
  induction terms with
  | nil =>
      intro accumulator E A k _ close _ _
      simpa [fold] using close
  | cons term rest ih =>
      obtain ⟨coefficient, input⟩ := term
      intro accumulator E A k ok close bounded nonneg
      obtain ⟨mulOk, addOk, restOk⟩ := ok
      have product := mul_mixed_error coefficient input mulOk
      have sum := add_mixed_error accumulator (mul coefficient input) addOk
      have step := step_bound_mixed unit ((1 + unit) ^ (2 * k) - 1) (3 * (k : ℚ) * (1 + unit) ^ k * eta)
        eta A E (exactValue accumulator) (exactValue coefficient * exactValue input)
        (exactValue (mul coefficient input)) (exactValue (add accumulator (mul coefficient input)))
        unit_nonneg (growth_nonneg k) nonneg bounded close product sum
      have power : (1 + unit) ^ 2 * (1 + ((1 + unit) ^ (2 * k) - 1)) - 1 =
          (1 + unit) ^ (2 * (k + 1)) - 1 := by ring
      rw [power] at step
      have closer : |exactValue (add accumulator (mul coefficient input)) -
          (E + exactValue coefficient * exactValue input)| ≤
          ((1 + unit) ^ (2 * (k + 1)) - 1) * (A + |exactValue coefficient * exactValue input|) +
            3 * ((k + 1 : Nat) : ℚ) * (1 + unit) ^ (k + 1) * eta := by
        have growth := absolute_growth k
        linarith
      have next := ih (add accumulator (mul coefficient input))
        (E + exactValue coefficient * exactValue input)
        (A + |exactValue coefficient * exactValue input|) (k + 1) restOk closer
        (le_trans (abs_add_le _ _) (by linarith)) (by positivity)
      simp only [fold, List.map_cons, List.sum_cons, List.length_cons]
      rw [show k + (rest.length + 1) = k + 1 + rest.length by omega, ← add_assoc, ← add_assoc]
      exact next

/-- **The ordered affine fold with finite intermediates**: it is within
`((1 + u)^(2n) - 1) · (|bias| + Σ|cᵢ · xᵢ|) + 3n(1 + u)^n · η` of its exact value whenever no rounding
overflows. Subnormal intermediate results are allowed. -/
theorem fold_finite_error (bias : Bits) (terms : List (Bits × Bits)) (ok : FoldFinite bias terms) :
    |exactValue (fold bias terms) - exactAffine bias terms| ≤
      ((1 + unit) ^ (2 * terms.length) - 1) * affineScale bias terms +
        3 * (terms.length : ℚ) * (1 + unit) ^ terms.length * eta := by
  have result := fold_finite_error_from terms bias (exactValue bias) |exactValue bias| 0 ok
    (by simp) le_rfl (abs_nonneg _)
  simpa [exactAffine, affineScale] using result

/-! ## The subnormal fixture

`+0 + 1.5 · 2^-149` rounds the exact product `3 · 2^-150` to the even subnormal `2^-148`. The
error is `2^-150`, a third of the result. The normal-range premises of `Sequence.fold_error` fail,
and a bound relative to the term scale alone would be false. The finite premises hold, and the
absolute term covers the error. -/

/-- `1.5` and the smallest subnormal `2^-149` as encodings. -/
def threeHalvesBits : Bits := 0x3FC00000
def smallestSubnormalBits : Bits := 0x00000001

/-- One term whose product is subnormal. -/
def subnormalTerms : List (Bits × Bits) := [(threeHalvesBits, smallestSubnormalBits)]

set_option maxRecDepth 100000 in
theorem subnormal_premises : FoldFinite (zeroOf false) subnormalTerms := by
  refine ⟨⟨⟨by decide, by decide⟩, ⟨by decide, by decide⟩, fun _ => ?_⟩,
    ⟨⟨by decide, by decide⟩, ⟨by decide, by decide⟩, fun _ => ?_⟩, trivial⟩
  all_goals (unfold NoOverflow; decide)

set_option maxRecDepth 100000 in
/-- The normal-range premises of `Sequence.fold_error` fail here: the product is subnormal. -/
theorem subnormal_fold_is_outside_the_normal_premises : ¬ FoldOk (zeroOf false) subnormalTerms := by
  intro ok
  have range := ok.1.2.2 (by decide)
  exact absurd range.1 (by decide)

set_option maxRecDepth 100000 in
theorem subnormal_fold_values :
    exactValue (fold (zeroOf false) subnormalTerms) = 2 / 2 ^ 149 ∧
      exactAffine (zeroOf false) subnormalTerms = 3 / 2 ^ 150 ∧
      affineScale (zeroOf false) subnormalTerms = 3 / 2 ^ 150 := by
  have folded : fold (zeroOf false) subnormalTerms = 0x00000002 := by decide
  have two : scaledMagnitude (0x00000002 : Bits) = (2, -149) := by decide
  have threeHalves : scaledMagnitude threeHalvesBits = (12582912, -23) := by decide
  have smallest : scaledMagnitude smallestSubnormalBits = (1, -149) := by decide
  have zero : scaledMagnitude (zeroOf false) = (0, -149) := by decide
  have signTwo : isNegative (0x00000002 : Bits) = false := by decide
  have signThreeHalves : isNegative threeHalvesBits = false := by decide
  have signSmallest : isNegative smallestSubnormalBits = false := by decide
  have signZero : isNegative (zeroOf false) = false := by decide
  refine ⟨?_, ?_, ?_⟩
  · rw [folded]
    norm_num [exactValue, two, signTwo]
  · norm_num [exactAffine, subnormalTerms, exactValue, threeHalves, smallest, zero, signThreeHalves,
      signSmallest, signZero]
  · norm_num [affineScale, subnormalTerms, exactValue, threeHalves, smallest, zero, signThreeHalves,
      signSmallest, signZero]

/-- **The absolute term is needed**: the error is `η`, within the finite fold bound, while the
relative part alone is far below it. -/
theorem subnormal_needs_the_absolute_term :
    |exactValue (fold (zeroOf false) subnormalTerms) - exactAffine (zeroOf false) subnormalTerms| =
        eta ∧
      |exactValue (fold (zeroOf false) subnormalTerms) - exactAffine (zeroOf false) subnormalTerms| ≤
        ((1 + unit) ^ (2 * subnormalTerms.length) - 1) * affineScale (zeroOf false) subnormalTerms +
          3 * (subnormalTerms.length : ℚ) * (1 + unit) ^ subnormalTerms.length * eta ∧
      ¬ |exactValue (fold (zeroOf false) subnormalTerms) - exactAffine (zeroOf false) subnormalTerms| ≤
        ((1 + unit) ^ (2 * subnormalTerms.length) - 1) * affineScale (zeroOf false) subnormalTerms := by
  obtain ⟨folded, exact, scale⟩ := subnormal_fold_values
  have bound := fold_finite_error (zeroOf false) subnormalTerms subnormal_premises
  rw [folded, exact] at bound ⊢
  have etaValue : eta = 1 / 2 ^ 150 := by
    unfold eta
    norm_num
  refine ⟨by rw [etaValue]; norm_num, bound, ?_⟩
  rw [scale]
  unfold unit
  norm_num [subnormalTerms]

end NobleFoundation.Binary32.SequenceFinite
