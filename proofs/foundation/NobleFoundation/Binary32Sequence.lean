/-
  Rounding-error bounds for whole binary32 operation sequences (issue #44).

  `Binary32Scale.lean` bounds one rounding of a positive ratio. This module carries that bound to
  the model's arithmetic operations and then through the ordered affine fold of `Binary32.lean`,
  `vᵢ = rn(vᵢ₋₁ + rn(cᵢ × xᵢ))`, which is the documented order of `affine` in `sem.rs`.

  - `exactValue_signed`: setting the sign bit of a magnitude below `2^31` negates its exact value.
  - `add_relative_error`, `mul_relative_error`: one model addition or multiplication of finite
    encodings is within one unit `u = 2^-24` of its exact result, relatively, whenever the exact
    result is zero or its carried exponent is in the normal range.
  - `fold_error`: an ordered affine fold of `n` terms is within `((1 + u)^(2n) - 1)` times the sum
    of the absolute bias and term magnitudes of the exact affine value, given those per-operation
    premises at every step.

  The premises are the carried exponent of each rounded operation, which a certificate checker can
  establish; overflow and subnormal results are outside them. This is a statement about the exact
  reference model only: no Rust, Wasm or host floating-point correspondence is claimed.
-/
import NobleFoundation.Binary32Scale
import Mathlib.Tactic.Linarith
import Mathlib.Tactic.NormNum
import Mathlib.Tactic.Ring

namespace NobleFoundation.Binary32.Sequence

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error
open NobleFoundation.Binary32.Scale

set_option exponentiation.threshold 4096

/-! ## Exact values and the sign bit -/

/-- The exact value is the signed scaled integer times its power of two. -/
theorem exactValue_eq_scaledValue (b : Bits) :
    exactValue b = ((scaledValue b).1 : ℚ) * (2 : ℚ) ^ (scaledValue b).2 := by
  unfold exactValue scaledValue
  cases isNegative b <;> simp

theorem toNat_ofNat_of_lt (n : Nat) (h : n < 2 ^ 32) : (BitVec.ofNat 32 n).toNat = n := by
  rw [BitVec.toNat_ofNat, Nat.mod_eq_of_lt h]

theorem exponent_of_toNat (b : Bits) : exponent b = b.toNat / 2 ^ 23 % 2 ^ 8 := by
  unfold exponent
  rw [BitVec.toNat_ushiftRight, Nat.shiftRight_eq_div_pow,
    show (0xFF : Nat) = 2 ^ 8 - 1 from rfl, Nat.and_two_pow_sub_one_eq_mod]

theorem fraction_of_toNat (b : Bits) : fraction b = b.toNat % 2 ^ 23 := by
  unfold fraction
  rw [show (0x7FFFFF : Nat) = 2 ^ 23 - 1 from rfl, Nat.and_two_pow_sub_one_eq_mod]

theorem isNegative_of_toNat (b : Bits) : isNegative b = (b.toNat / 2 ^ 31 == 1) := by
  unfold isNegative
  rw [Nat.shiftRight_eq_div_pow]

/-- **Setting the sign bit negates the exact value** of every magnitude below `2^31`. -/
theorem exactValue_signed (magnitude : Nat) (small : magnitude < 2 ^ 31) :
    exactValue (BitVec.ofNat 32 (magnitude ||| 0x80000000)) =
      -exactValue (BitVec.ofNat 32 magnitude) := by
  have orAdd : magnitude ||| 0x80000000 = magnitude + 2 ^ 31 := by
    rw [Nat.or_comm, show (0x80000000 : Nat) = 2 ^ 31 * 1 from rfl,
      ← Nat.two_pow_add_eq_or_of_lt small 1]
    omega
  have plainNat : (BitVec.ofNat 32 magnitude).toNat = magnitude :=
    toNat_ofNat_of_lt magnitude (by omega)
  have signedNat : (BitVec.ofNat 32 (magnitude ||| 0x80000000)).toNat = magnitude + 2 ^ 31 := by
    rw [orAdd]
    exact toNat_ofNat_of_lt _ (by omega)
  have sameExponent : exponent (BitVec.ofNat 32 (magnitude ||| 0x80000000)) =
      exponent (BitVec.ofNat 32 magnitude) := by
    rw [exponent_of_toNat, exponent_of_toNat, signedNat, plainNat,
      show magnitude + 2 ^ 31 = magnitude + 2 ^ 8 * 2 ^ 23 by norm_num,
      Nat.add_mul_div_right _ _ (by norm_num), Nat.add_mod_right]
  have sameFraction : fraction (BitVec.ofNat 32 (magnitude ||| 0x80000000)) =
      fraction (BitVec.ofNat 32 magnitude) := by
    rw [fraction_of_toNat, fraction_of_toNat, signedNat, plainNat,
      show magnitude + 2 ^ 31 = magnitude + 2 ^ 8 * 2 ^ 23 by norm_num, Nat.add_mul_mod_self_right]
  have signed : isNegative (BitVec.ofNat 32 (magnitude ||| 0x80000000)) = true := by
    rw [isNegative_of_toNat, signedNat,
      show magnitude + 2 ^ 31 = magnitude + 1 * 2 ^ 31 by ring, Nat.add_mul_div_right _ _ (by norm_num),
      Nat.div_eq_of_lt small]
    rfl
  have plain : isNegative (BitVec.ofNat 32 magnitude) = false := by
    rw [isNegative_of_toNat, plainNat, Nat.div_eq_of_lt small]
    rfl
  unfold exactValue scaledMagnitude
  simp only [sameExponent, sameFraction, signed, plain]
  push_cast
  ring

/-! ## One signed rounding -/

/-- The binary32 unit roundoff of round-to-nearest. -/
def unit : ℚ := 1 / 2 ^ 24

/-- The positive ratio `roundScaled m s` hands to `roundPositive`. -/
def scaledArgs (m s : Int) : Nat × Nat :=
  (if (0 : Int) ≤ s then m.natAbs * pow2Int s else m.natAbs,
    if (0 : Int) ≤ s then 1 else 1 * pow2Int (-s))

/-- The premise of every bound here: the carried exponent of a rounding is in the normal range,
so the result is neither an overflow nor subnormal. -/
def InRange (args : Nat × Nat) : Prop :=
  -126 ≤ (carriedPair args.1 args.2).2 ∧ (carriedPair args.1 args.2).2 ≤ 127

theorem pow2Int_pos (k : Int) : 0 < pow2Int k := by
  unfold pow2Int pow2
  exact Nat.two_pow_pos _

theorem scaledArgs_pos (m s : Int) (nonzero : m ≠ 0) :
    0 < (scaledArgs m s).1 ∧ 0 < (scaledArgs m s).2 := by
  have magnitude : 0 < m.natAbs := Int.natAbs_pos.mpr nonzero
  unfold scaledArgs
  by_cases nonneg : (0 : Int) ≤ s
  · simp only [if_pos nonneg]
    exact ⟨Nat.mul_pos magnitude (pow2Int_pos s), by norm_num⟩
  · simp only [if_neg nonneg]
    exact ⟨magnitude, by simpa using pow2Int_pos (-s)⟩

/-- The ratio is the magnitude of `m · 2^s`. -/
theorem scaledArgs_ratio (m s : Int) :
    ((scaledArgs m s).1 : ℚ) / (scaledArgs m s).2 = (m.natAbs : ℚ) * (2 : ℚ) ^ s := by
  unfold scaledArgs
  by_cases nonneg : (0 : Int) ≤ s
  · simp only [if_pos nonneg]
    rw [Nat.cast_mul, pow2Int_rat s nonneg]
    simp
  · simp only [if_neg nonneg]
    have negated : (0 : Int) ≤ -s := by omega
    rw [Nat.cast_mul, pow2Int_rat (-s) negated, zpow_neg]
    field_simp
    simp

/-- The carried significand stays below `2^24`: on the power boundary it carries to `2^23`, and
elsewhere both scale premises hold. -/
theorem carried_significand_lt (a b : Nat) (ha : 0 < a) (hb : 0 < b) :
    (carriedPair a b).1 < 2 ^ 24 := by
  by_cases boundary : powerBoundary a b
  · have rounded := boundary_rounded a b hb boundary
    unfold carriedPair
    simp only [rounded, if_true, pow2]
    norm_num
  · exact (carriedPair_m_bounds a b hb (scale_lower a b ha hb)
      (scale_upper a b ha hb boundary)).2

/-- In the normal range `roundPositive` is the normal encoding of the carried pair. -/
theorem roundPositive_normal_nat (a b : Nat) (range : InRange (a, b)) :
    roundPositive a b =
      ((carriedPair a b).2 + 127).toNat * pow2 23 + ((carriedPair a b).1 - pow2 23) := by
  obtain ⟨low, high⟩ := range
  dsimp only at low high
  let c := carriedPair a b
  have named : c = carriedPair a b := rfl
  have body : roundPositive a b =
      let m := c.1
      let e' := c.2
      if e' > 127 then 255 * pow2 23
      else if e' < -126 then
        let n := a * pow2 149 / b
        let r' := a * pow2 149 % b
        let sub := if 2 * r' > b then n + 1
          else if 2 * r' = b then n + n % 2 else n
        if sub ≥ pow2 23 then pow2 23 else sub
      else (e' + 127).toNat * pow2 23 + (m - pow2 23) := by
    rfl
  rw [body]
  dsimp only
  rw [← named]
  have notHigh : ¬ c.2 > 127 := by simp only [c]; omega
  have notLow : ¬ c.2 < -126 := by simp only [c]; omega
  simp only [if_neg notHigh, if_neg notLow]

/-- A normal-range rounding leaves the sign bit clear. -/
theorem roundPositive_lt (a b : Nat) (ha : 0 < a) (hb : 0 < b) (range : InRange (a, b)) :
    roundPositive a b < 2 ^ 31 := by
  rw [roundPositive_normal_nat a b range]
  have significand := carried_significand_lt a b ha hb
  obtain ⟨_, high⟩ := range
  dsimp only at high
  have exponentBound : ((carriedPair a b).2 + 127).toNat ≤ 254 := by omega
  simp only [pow2]
  have product : ((carriedPair a b).2 + 127).toNat * 2 ^ 23 ≤ 254 * 2 ^ 23 :=
    Nat.mul_le_mul_right _ exponentBound
  omega

/-- **One signed rounding is within one unit, relatively**: rounding `m · 2^s` with the sign of
`m` stays within `u · |m · 2^s|` of it whenever its carried exponent is in the normal range. -/
theorem roundScaled_relative_error (m s : Int) (nonzero : m ≠ 0) (range : InRange (scaledArgs m s)) :
    |exactValue (roundScaled m s (decide (m < 0))) - (m : ℚ) * (2 : ℚ) ^ s| ≤
      unit * |(m : ℚ) * (2 : ℚ) ^ s| := by
  obtain ⟨numeratorPos, denominatorPos⟩ := scaledArgs_pos m s nonzero
  have relative := roundPositive_relative_error (scaledArgs m s).1 (scaledArgs m s).2 numeratorPos
    denominatorPos range.1 range.2
  rw [scaledArgs_ratio] at relative
  have small := roundPositive_lt (scaledArgs m s).1 (scaledArgs m s).2 numeratorPos
    denominatorPos range
  have power : (0 : ℚ) < (2 : ℚ) ^ s := zpow_pos (by norm_num) s
  have magnitude : |(m : ℚ) * (2 : ℚ) ^ s| = (m.natAbs : ℚ) * (2 : ℚ) ^ s := by
    rw [abs_mul, abs_of_pos power, Nat.cast_natAbs, Int.cast_abs]
  have roundedIs : roundScaled m s (decide (m < 0)) =
      BitVec.ofNat 32 (if decide (m < 0) then roundPositive (scaledArgs m s).1 (scaledArgs m s).2 |||
        0x80000000 else roundPositive (scaledArgs m s).1 (scaledArgs m s).2) := rfl
  rw [roundedIs, magnitude]
  unfold unit
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
    exact relative
  · simp only [negative, decide_false, Bool.false_eq_true, if_false]
    have signedValue : (m : ℚ) * (2 : ℚ) ^ s = (m.natAbs : ℚ) * (2 : ℚ) ^ s := by
      rw [Nat.cast_natAbs, Int.cast_abs, abs_of_nonneg (by exact_mod_cast not_lt.mp negative)]
    rw [signedValue]
    exact relative

/-! ## One addition -/

/-- A finite encoding: neither a NaN nor an infinity. -/
def Finite (b : Bits) : Prop := classify b ≠ .notANumber ∧ classify b ≠ .infinity

/-- The aligned exact sum `add` forms before rounding, with its common scale. -/
def addScaled (x y : Bits) : Int × Int :=
  let s := if (scaledValue x).2 ≤ (scaledValue y).2 then (scaledValue x).2 else (scaledValue y).2
  ((scaledValue x).1 * (pow2Int ((scaledValue x).2 - s) : Int) +
    (scaledValue y).1 * (pow2Int ((scaledValue y).2 - s) : Int), s)

/-- Every signed zero has exact value zero. -/
theorem exactValue_zeroOf (negative : Bool) : exactValue (zeroOf negative) = 0 := by
  have magnitude : scaledMagnitude (zeroOf negative) = (0, -149) := by
    cases negative <;> decide
  unfold exactValue
  rw [magnitude]
  simp

/-- On finite operands `add` rounds the aligned exact sum once, or returns a signed zero. -/
theorem add_eq (x y : Bits) (finiteX : Finite x) (finiteY : Finite y) :
    add x y = if (addScaled x y).1 = 0 then zeroOf (isNegative x && isNegative y)
      else roundScaled (addScaled x y).1 (addScaled x y).2 (decide ((addScaled x y).1 < 0)) := by
  obtain ⟨xNaN, xInfinite⟩ := finiteX
  obtain ⟨yNaN, yInfinite⟩ := finiteY
  unfold add
  split
  · contradiction
  · contradiction
  · contradiction
  · contradiction
  · contradiction
  · rcases valueX : scaledValue x with ⟨mx, sx⟩
    rcases valueY : scaledValue y with ⟨my, sy⟩
    simp only [addScaled, valueX, valueY]

/-- Moving `m · 2^k` to a lower scale `s` multiplies the integer by `2^(k - s)`. -/
theorem rescale (m k s : Int) (lower : s ≤ k) :
    (m : ℚ) * (2 : ℚ) ^ k = ((m * (pow2Int (k - s) : Int) : Int) : ℚ) * (2 : ℚ) ^ s := by
  push_cast
  rw [pow2Int_rat (k - s) (by omega), mul_assoc, ← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
  congr 2
  ring

/-- The aligned sum is the exact sum. -/
theorem exact_sum (x y : Bits) :
    exactValue x + exactValue y = ((addScaled x y).1 : ℚ) * (2 : ℚ) ^ (addScaled x y).2 := by
  rw [exactValue_eq_scaledValue x, exactValue_eq_scaledValue y]
  unfold addScaled
  rcases scaledValue x with ⟨mx, sx⟩
  rcases scaledValue y with ⟨my, sy⟩
  dsimp only
  by_cases order : sx ≤ sy
  · simp only [if_pos order]
    rw [rescale mx sx sx le_rfl, rescale my sy sx order]
    push_cast
    ring
  · simp only [if_neg order]
    rw [rescale mx sx sy (by omega), rescale my sy sy le_rfl]
    push_cast
    ring

/-- The premises of one addition: finite operands, and a normal-range carried exponent whenever
the exact sum is nonzero. -/
def AddOk (x y : Bits) : Prop :=
  Finite x ∧ Finite y ∧ ((addScaled x y).1 ≠ 0 → InRange (scaledArgs (addScaled x y).1 (addScaled x y).2))

/-- **One addition is within one unit of the exact sum, relatively.** -/
theorem add_relative_error (x y : Bits) (ok : AddOk x y) :
    |exactValue (add x y) - (exactValue x + exactValue y)| ≤ unit * |exactValue x + exactValue y| := by
  obtain ⟨finiteX, finiteY, range⟩ := ok
  rw [add_eq x y finiteX finiteY, exact_sum]
  split
  · rename_i zero
    rw [zero, exactValue_zeroOf]
    simp
  · rename_i nonzero
    exact roundScaled_relative_error _ _ nonzero (range nonzero)

/-! ## One multiplication -/

/-- The exact product `mul` forms before rounding, with its scale. -/
def mulScaled (x y : Bits) : Int × Int :=
  ((scaledValue x).1 * (scaledValue y).1, (scaledValue x).2 + (scaledValue y).2)

/-- The exact product is the scaled product. -/
theorem exact_product (x y : Bits) :
    exactValue x * exactValue y = ((mulScaled x y).1 : ℚ) * (2 : ℚ) ^ (mulScaled x y).2 := by
  rw [exactValue_eq_scaledValue x, exactValue_eq_scaledValue y]
  unfold mulScaled
  push_cast
  rw [zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
  ring

/-- A signed zero has magnitude zero. -/
theorem zero_class_magnitude (b : Bits) (zero : classify b = .posZero ∨ classify b = .negZero) :
    scaledMagnitude b = (0, -149) := by
  unfold scaledMagnitude
  rcases zero with zero | zero <;>
  · unfold classify at zero
    split at zero <;> simp_all

theorem zero_class_value (b : Bits) (zero : classify b = .posZero ∨ classify b = .negZero) :
    exactValue b = 0 := by
  unfold exactValue
  rw [zero_class_magnitude b zero]
  simp

/-- A subnormal or normal encoding has a positive magnitude. -/
theorem nonzero_class_magnitude (b : Bits) (nonzero : classify b = .subnormal ∨ classify b = .normal) :
    0 < (scaledMagnitude b).1 := by
  unfold scaledMagnitude
  rcases nonzero with nonzero | nonzero <;>
  · unfold classify at nonzero
    split at nonzero <;> simp_all
    all_goals omega

/-- The sign rule of a product of two signed positive magnitudes. -/
theorem sign_rule (signX signY : Bool) (mx my : Nat) (px : 0 < mx) (py : 0 < my) :
    (if signX then -(mx : Int) else (mx : Int)) * (if signY then -(my : Int) else (my : Int)) ≠ 0 ∧
      decide ((if signX then -(mx : Int) else (mx : Int)) *
        (if signY then -(my : Int) else (my : Int)) < 0) = (signX != signY) := by
  have both : (0 : Int) < mx * my := Int.mul_pos (by exact_mod_cast px) (by exact_mod_cast py)
  cases signX <;> cases signY <;>
    simp only [Bool.false_eq_true, ite_true, ite_false, neg_mul, mul_neg, neg_neg, bne_self_eq_false,
      Bool.bne_false, Bool.bne_true, Bool.not_false]
  · exact ⟨ne_of_gt both, decide_eq_false (not_lt.mpr (le_of_lt both))⟩
  · exact ⟨neg_ne_zero.mpr (ne_of_gt both), decide_eq_true (neg_neg_of_pos both)⟩
  · exact ⟨neg_ne_zero.mpr (ne_of_gt both), decide_eq_true (neg_neg_of_pos both)⟩
  · exact ⟨ne_of_gt both, decide_eq_false (not_lt.mpr (le_of_lt both))⟩

/-- With positive magnitudes the product is nonzero and negative exactly when one sign is set. -/
theorem product_sign (x y : Bits) (positiveX : 0 < (scaledMagnitude x).1)
    (positiveY : 0 < (scaledMagnitude y).1) :
    (mulScaled x y).1 ≠ 0 ∧ decide ((mulScaled x y).1 < 0) = (isNegative x != isNegative y) := by
  unfold mulScaled scaledValue
  rcases magnitudeX : scaledMagnitude x with ⟨mx, sx⟩
  rcases magnitudeY : scaledMagnitude y with ⟨my, sy⟩
  rw [magnitudeX] at positiveX
  rw [magnitudeY] at positiveY
  exact sign_rule (isNegative x) (isNegative y) mx my positiveX positiveY

/-- On two subnormal or normal operands `mul` rounds the exact product once. -/
theorem mul_scaled_eq (x y : Bits) (nonzeroX : classify x = .subnormal ∨ classify x = .normal)
    (nonzeroY : classify y = .subnormal ∨ classify y = .normal) :
    mul x y = roundScaled (mulScaled x y).1 (mulScaled x y).2 (decide ((mulScaled x y).1 < 0)) := by
  obtain ⟨nonzero, sign⟩ := product_sign x y (nonzero_class_magnitude x nonzeroX)
    (nonzero_class_magnitude y nonzeroY)
  have branch : mul x y = if (mulScaled x y).1 = 0 then zeroOf (isNegative x != isNegative y)
      else roundScaled (mulScaled x y).1 (mulScaled x y).2 (isNegative x != isNegative y) := by
    unfold mul
    rcases nonzeroX with classX | classX <;> rcases nonzeroY with classY | classY <;>
    · rw [classX, classY]
      dsimp only
      rcases valueX : scaledValue x with ⟨mx, sx⟩
      rcases valueY : scaledValue y with ⟨my, sy⟩
      simp only [mulScaled, valueX, valueY]
  rw [branch, if_neg nonzero, sign]

/-- The premises of one multiplication: finite operands, and a normal-range carried exponent
whenever the exact product is nonzero. -/
def MulOk (x y : Bits) : Prop :=
  Finite x ∧ Finite y ∧ ((mulScaled x y).1 ≠ 0 → InRange (scaledArgs (mulScaled x y).1 (mulScaled x y).2))

/-- **One multiplication is within one unit of the exact product, relatively.** -/
theorem mul_relative_error (x y : Bits) (ok : MulOk x y) :
    |exactValue (mul x y) - exactValue x * exactValue y| ≤ unit * |exactValue x * exactValue y| := by
  obtain ⟨⟨xNaN, xInfinite⟩, ⟨yNaN, yInfinite⟩, range⟩ := ok
  by_cases zeroX : classify x = .posZero ∨ classify x = .negZero
  · have result : mul x y = zeroOf (isNegative x != isNegative y) := by
      unfold mul
      rcases zeroX with classX | classX <;>
      · rw [classX]
        cases classY : classify y <;> simp_all
    rw [result, exactValue_zeroOf, zero_class_value x zeroX]
    simp
  by_cases zeroY : classify y = .posZero ∨ classify y = .negZero
  · have result : mul x y = zeroOf (isNegative x != isNegative y) := by
      unfold mul
      rcases zeroY with classY | classY <;>
      · rw [classY]
        cases classX : classify x <;> simp_all
    rw [result, exactValue_zeroOf, zero_class_value y zeroY]
    simp
  have nonzeroX : classify x = .subnormal ∨ classify x = .normal := by
    cases classX : classify x <;> simp_all
  have nonzeroY : classify y = .subnormal ∨ classify y = .normal := by
    cases classY : classify y <;> simp_all
  obtain ⟨nonzero, _⟩ := product_sign x y (nonzero_class_magnitude x nonzeroX)
    (nonzero_class_magnitude y nonzeroY)
  rw [mul_scaled_eq x y nonzeroX nonzeroY, exact_product]
  exact roundScaled_relative_error _ _ nonzero (range nonzero)

/-! ## The ordered affine fold -/

/-- The exact affine value: the bias plus every exact term product. -/
def exactAffine (bias : Bits) (terms : List (Bits × Bits)) : ℚ :=
  exactValue bias + (terms.map fun term => exactValue term.1 * exactValue term.2).sum

/-- The absolute scale of the affine value: the bias and term magnitudes. -/
def affineScale (bias : Bits) (terms : List (Bits × Bits)) : ℚ :=
  |exactValue bias| + (terms.map fun term => |exactValue term.1 * exactValue term.2|).sum

/-- The premises of a fold: each product and each accumulation meets its one-operation premises,
evaluated on the actual intermediate accumulator. -/
def FoldOk : Bits → List (Bits × Bits) → Prop
  | _, [] => True
  | accumulator, (coefficient, input) :: rest =>
      MulOk coefficient input ∧ AddOk accumulator (mul coefficient input) ∧
        FoldOk (add accumulator (mul coefficient input)) rest

theorem unit_nonneg : 0 ≤ unit := by
  unfold unit
  positivity

theorem growth_nonneg (k : Nat) : 0 ≤ (1 + unit) ^ (2 * k) - 1 := by
  have base : 1 ≤ 1 + unit := by linarith [unit_nonneg]
  linarith [one_le_pow₀ (n := 2 * k) base]

/-- **One step of the error recursion.** If the accumulator is within `r · A` of its exact value,
the product within `u · |t|` and the sum within `u · |a + p|`, the next accumulator is within
`((1 + u)^2 (1 + r) - 1) (A + |t|)`. -/
theorem step_bound (u r A E a t p v : ℚ) (hu : 0 ≤ u) (hr : 0 ≤ r) (hA : 0 ≤ A) (hE : |E| ≤ A)
    (ha : |a - E| ≤ r * A) (hp : |p - t| ≤ u * |t|) (hv : |v - (a + p)| ≤ u * |a + p|) :
    |v - (E + t)| ≤ ((1 + u) ^ 2 * (1 + r) - 1) * (A + |t|) := by
  have tNonneg : 0 ≤ |t| := abs_nonneg t
  have total : |v - (E + t)| ≤ |v - (a + p)| + |a - E| + |p - t| := by
    have first := abs_add_le (v - (a + p) + (a - E)) (p - t)
    have second := abs_add_le (v - (a + p)) (a - E)
    have same : v - (E + t) = v - (a + p) + (a - E) + (p - t) := by ring
    rw [same]
    linarith
  have sumBound : |a + p| ≤ A + r * A + |t| + u * |t| := by
    have first := abs_add_le (E + (a - E) + t) (p - t)
    have second := abs_add_le (E + (a - E)) t
    have third := abs_add_le E (a - E)
    have same : a + p = E + (a - E) + t + (p - t) := by ring
    rw [same]
    linarith
  have vBound : |v - (a + p)| ≤ u * (A + r * A + |t| + u * |t|) :=
    le_trans hv (mul_le_mul_of_nonneg_left sumBound hu)
  nlinarith [mul_nonneg hu hA, mul_nonneg (mul_nonneg hu hu) hA, mul_nonneg (mul_nonneg hu hr) hA,
    mul_nonneg (mul_nonneg (mul_nonneg hu hu) hr) hA, mul_nonneg hr tNonneg,
    mul_nonneg (mul_nonneg hu hr) tNonneg, mul_nonneg (mul_nonneg (mul_nonneg hu hu) hr) tNonneg,
    mul_nonneg hu tNonneg, mul_nonneg (mul_nonneg hu hu) tNonneg]

/-- The fold from an accumulator already within `((1 + u)^(2k) - 1) · A` of an exact value `E`. -/
theorem fold_error_from (terms : List (Bits × Bits)) :
    ∀ (accumulator : Bits) (E A : ℚ) (k : Nat), FoldOk accumulator terms →
      |exactValue accumulator - E| ≤ ((1 + unit) ^ (2 * k) - 1) * A → |E| ≤ A → 0 ≤ A →
      |exactValue (fold accumulator terms) -
          (E + (terms.map fun term => exactValue term.1 * exactValue term.2).sum)| ≤
        ((1 + unit) ^ (2 * (k + terms.length)) - 1) *
          (A + (terms.map fun term => |exactValue term.1 * exactValue term.2|).sum) := by
  induction terms with
  | nil =>
      intro accumulator E A k _ close _ _
      simpa [fold] using close
  | cons term rest ih =>
      obtain ⟨coefficient, input⟩ := term
      intro accumulator E A k ok close bounded nonneg
      obtain ⟨mulOk, addOk, restOk⟩ := ok
      have product := mul_relative_error coefficient input mulOk
      have sum := add_relative_error accumulator (mul coefficient input) addOk
      have step := step_bound unit ((1 + unit) ^ (2 * k) - 1) A E (exactValue accumulator)
        (exactValue coefficient * exactValue input) (exactValue (mul coefficient input))
        (exactValue (add accumulator (mul coefficient input))) unit_nonneg (growth_nonneg k) nonneg
        bounded close product sum
      have power : (1 + unit) ^ 2 * (1 + ((1 + unit) ^ (2 * k) - 1)) - 1 =
          (1 + unit) ^ (2 * (k + 1)) - 1 := by ring
      rw [power] at step
      have next := ih (add accumulator (mul coefficient input))
        (E + exactValue coefficient * exactValue input)
        (A + |exactValue coefficient * exactValue input|) (k + 1) restOk step
        (le_trans (abs_add_le _ _) (by linarith)) (by positivity)
      simp only [fold, List.map_cons, List.sum_cons, List.length_cons]
      rw [show k + (rest.length + 1) = k + 1 + rest.length by omega, ← add_assoc, ← add_assoc]
      exact next

/-- **The ordered affine fold is within `((1 + u)^(2n) - 1)` of its exact value**, relative to the
absolute scale of the bias and the term products, whenever every product and accumulation meets
its one-operation premises. -/
theorem fold_error (bias : Bits) (terms : List (Bits × Bits)) (ok : FoldOk bias terms) :
    |exactValue (fold bias terms) - exactAffine bias terms| ≤
      ((1 + unit) ^ (2 * terms.length) - 1) * affineScale bias terms := by
  have result := fold_error_from terms bias (exactValue bias) |exactValue bias| 0 ok
    (by simp) le_rfl (abs_nonneg _)
  simpa [exactAffine, affineScale] using result

/-! ## The cancellation fixture

`Binary32.fold_order_is_load_bearing` folds `1 + 1·2^24 + (-1)·2^24` to `+0` because the `1` is
absorbed before the cancellation. Every premise of `fold_error` holds there, the error is `1`, and
a bound relative to the exact result alone (`u · 1`) would be false. The bound must scale with the
term magnitudes. -/

/-- The documented cancellation fold. -/
def cancellationTerms : List (Bits × Bits) := [(oneBits, twoPow24Bits), (negOneBits, twoPow24Bits)]

set_option maxRecDepth 100000 in
theorem cancellation_premises : FoldOk oneBits cancellationTerms := by
  refine ⟨⟨⟨by decide, by decide⟩, ⟨by decide, by decide⟩, fun _ => ?_⟩,
    ⟨⟨by decide, by decide⟩, ⟨by decide, by decide⟩, fun _ => ?_⟩,
    ⟨⟨by decide, by decide⟩, ⟨by decide, by decide⟩, fun _ => ?_⟩,
    ⟨⟨by decide, by decide⟩, ⟨by decide, by decide⟩, fun _ => ?_⟩, trivial⟩
  all_goals (unfold InRange; decide)

set_option maxRecDepth 100000 in
theorem cancellation_values :
    exactValue (fold oneBits cancellationTerms) = 0 ∧ exactAffine oneBits cancellationTerms = 1 ∧
      affineScale oneBits cancellationTerms = 1 + 2 * 2 ^ 24 := by
  have one : scaledMagnitude oneBits = (2 ^ 23, -23) := by decide
  have big : scaledMagnitude twoPow24Bits = (2 ^ 23, 1) := by decide
  have minus : scaledMagnitude negOneBits = (2 ^ 23, -23) := by decide
  have signOne : isNegative oneBits = false := by decide
  have signBig : isNegative twoPow24Bits = false := by decide
  have signMinus : isNegative negOneBits = true := by decide
  have folded : fold oneBits cancellationTerms = zeroOf false := fold_order_is_load_bearing.1
  refine ⟨by rw [folded, exactValue_zeroOf], ?_, ?_⟩
  · norm_num [exactAffine, cancellationTerms, exactValue, one, big, minus, signOne, signBig, signMinus]
  · norm_num [affineScale, cancellationTerms, exactValue, one, big, minus, signOne, signBig, signMinus]

/-- **The bound covers the cancellation, and the exact result alone cannot scale it**: the error is
`1`, within the fold bound, while `u` times the exact result is far below `1`. -/
theorem cancellation_needs_the_term_scale :
    |exactValue (fold oneBits cancellationTerms) - exactAffine oneBits cancellationTerms| = 1 ∧
      |exactValue (fold oneBits cancellationTerms) - exactAffine oneBits cancellationTerms| ≤
        ((1 + unit) ^ (2 * cancellationTerms.length) - 1) * affineScale oneBits cancellationTerms ∧
      ¬ |exactValue (fold oneBits cancellationTerms) - exactAffine oneBits cancellationTerms| ≤
        unit * |exactAffine oneBits cancellationTerms| := by
  obtain ⟨folded, exact, _⟩ := cancellation_values
  have bound := fold_error oneBits cancellationTerms cancellation_premises
  rw [folded, exact] at bound ⊢
  refine ⟨by norm_num, bound, ?_⟩
  unfold unit
  norm_num

end NobleFoundation.Binary32.Sequence
