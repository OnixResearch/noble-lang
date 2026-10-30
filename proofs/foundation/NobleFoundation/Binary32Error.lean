/-
A checked rounding-error lane for the binary32 reference (issue #44).

The theorem in this file is deliberately about the exact reference model in
`Binary32.lean`.  It covers positive, finite, normal-range inputs for which
the scaled quotient is in one binary32 normal bin and the carried exponent is
still finite.  The hypotheses are arithmetic premises a certificate checker
can establish; no correspondence to a Rust, Wasm or host floating-point
implementation is claimed here.

Subnormal rounding remains an explicit open boundary.  The final theorem is a
load-bearing control: flushing the subnormal product of the smallest normal
and one half exceeds the smallest-subnormal absolute error allowance.
-/
import NobleFoundation.Binary32
import NobleFoundation.Binary32Mutations
import Mathlib.Algebra.Order.Ring.Abs
import Mathlib.Tactic.FieldSimp
import Mathlib.Tactic.Linarith
import Mathlib.Tactic.NormNum
import Mathlib.Tactic.Ring

namespace NobleFoundation.Binary32.Error

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Mutations

set_option exponentiation.threshold 4096
set_option maxRecDepth 100000

/-- The exact dyadic value represented by a finite encoding.  `scaledValue b`
    returns `m,s` for `m * 2^s`; the rational power is total, so this definition
    is also convenient for the finite-output lemmas below. -/
def exactValue (b : Bits) : ℚ :=
  let (m, s) := scaledMagnitude b
  (if isNegative b then (-(m : Int) : ℚ) else (m : ℚ)) * (2 : ℚ) ^ s

/-- The normal encoding with unbiased exponent `e` and significand integer `m`.
    Its intended domain is `-126 ≤ e ≤ 127` and `2^23 ≤ m < 2^24`. -/
def normalEncoding (e : Int) (m : Nat) : Bits :=
  BitVec.ofNat 32 (((e + 127).toNat) * pow2 23 + (m - pow2 23))

/-- The exact quotient scaling used by `roundPositive`. -/
def scalePair (a b : Nat) : Nat × Nat :=
  let e := floorLog2Ratio a b
  if (0 : Int) ≤ 23 - e then (a * pow2Int (23 - e), b)
  else (a, b * pow2Int (e - 23))

/-- A nonnegative model power is the corresponding exact rational power. -/
theorem pow2Int_rat (k : Int) (hk : 0 ≤ k) :
    (pow2Int k : ℚ) = (2 : ℚ) ^ k := by
  unfold pow2Int pow2
  norm_num only [Nat.cast_pow]
  have hcast : (k.toNat : Int) = k := Int.toNat_of_nonneg hk
  calc
    (2 : ℚ) ^ k.toNat = (2 : ℚ) ^ (k.toNat : Int) := by rw [zpow_natCast]
    _ = (2 : ℚ) ^ k := by rw [hcast]

/-- The nearest/ties-to-even integer quotient used by `roundPositive`. -/
def roundNearest (n d : Nat) : Nat :=
  let q := n / d
  let r := n % d
  if 2 * r > d then q + 1
  else if 2 * r = d then q + q % 2 else q

/-- The rounded significand before the carry into the next exponent bin. -/
def roundedQuotient (a b : Nat) : Nat :=
  let scaled := scalePair a b
  roundNearest scaled.1 scaled.2

/-- The significand/exponent pair after the `2^24` carry in `roundPositive`. -/
def carriedPair (a b : Nat) : Nat × Int :=
  let e := floorLog2Ratio a b
  let rounded := roundedQuotient a b
  if rounded = pow2 24 then (pow2 23, e + 1) else (rounded, e)

/-- The normal encoding corresponding to the carried significand pair. -/
def roundedNormalBits (a b : Nat) : Bits :=
  let carried := carriedPair a b
  normalEncoding carried.2 carried.1

/-- Absolute difference on naturals. -/
def natAbsDiff (x y : Nat) : Nat := if x ≥ y then x - y else y - x

/-- The integer quotient rounding rule is within half a denominator. -/
theorem quotient_round_error (q r d n : Nat) (hr : r < d)
    (hdiv : q * d + r = n) :
    let rounded := if 2 * r > d then q + 1
      else if 2 * r = d then q + q % 2 else q
    2 * natAbsDiff (rounded * d) n ≤ d := by
  dsimp
  have hplus : (q + 1) * d = q * d + d := by simp [Nat.add_mul]
  by_cases hgt : 2 * r > d
  · rw [if_pos hgt, hplus]
    by_cases hge : n ≤ q * d + d
    · simp [natAbsDiff, hge]
      omega
    · have hlt : q * d + d < n := by omega
      simp [natAbsDiff, hge]
      omega
  · by_cases heq : 2 * r = d
    · by_cases hodd : q % 2 = 1
      · have hround : q + q % 2 = q + 1 := by omega
        rw [if_neg hgt, if_pos heq, hround, hplus]
        by_cases hge : n ≤ q * d + d
        · simp [natAbsDiff, hge]
          omega
        · have hlt : q * d + d < n := by omega
          simp [natAbsDiff, hge]
          omega
      · have heven : q % 2 = 0 := by omega
        have hround : q + q % 2 = q := by omega
        rw [if_neg hgt, if_pos heq, hround]
        by_cases hge : n ≤ q * d
        · simp [natAbsDiff, hge]
          omega
        · have hlt : q * d < n := by omega
          simp [natAbsDiff, hge]
          omega
    · rw [if_neg hgt, if_neg heq]
      by_cases hge : n ≤ q * d
      · simp [natAbsDiff, hge]
        omega
      · have hlt : q * d < n := by omega
        simp [natAbsDiff, hge]
        omega

/-- Convert the preceding integer error statement to an exact rational one. -/
theorem nat_error_to_rat (u d n : Nat) (hd : 0 < d)
    (h : 2 * natAbsDiff (u * d) n ≤ d) :
    |(u : ℚ) - (n : ℚ) / d| ≤ (1 : ℚ) / 2 := by
  have hqpos : (0 : ℚ) < d := by exact_mod_cast hd
  have hrewrite : (u : ℚ) - (n : ℚ) / d = ((u : ℚ) * d - n) / d := by
    field_simp
  rw [hrewrite]
  by_cases hge : n ≤ u * d
  · have hdiff : 2 * (u * d - n) ≤ d := by simpa [natAbsDiff, hge] using h
    have hdiffq0 : (2 : ℚ) * ((u * d - n : Nat) : ℚ) ≤ d := by exact_mod_cast hdiff
    have hcast : ((u * d - n : Nat) : ℚ) = (u : ℚ) * d - n := by
      rw [Nat.cast_sub hge]
      norm_num
    have hdiffq : (2 : ℚ) * ((u : ℚ) * d - n) ≤ d := by simpa [hcast] using hdiffq0
    have hnonneg0 : (0 : ℚ) ≤ (u * d - n : Nat) := by positivity
    have hnonneg : (0 : ℚ) ≤ (u : ℚ) * d - n := by simpa [hcast] using hnonneg0
    rw [abs_of_nonneg (div_nonneg hnonneg (le_of_lt hqpos))]
    apply (div_le_iff₀ hqpos).2
    linarith
  · have hlt : u * d < n := by omega
    have hdiff : 2 * (n - u * d) ≤ d := by simpa [natAbsDiff, hge] using h
    have hdiffq0 : (2 : ℚ) * ((n - u * d : Nat) : ℚ) ≤ d := by exact_mod_cast hdiff
    have hcast : ((n - u * d : Nat) : ℚ) = (n : ℚ) - (u : ℚ) * d := by
      rw [Nat.cast_sub (Nat.le_of_lt hlt)]
      norm_num
    have hdiffq : (2 : ℚ) * ((n : ℚ) - (u : ℚ) * d) ≤ d := by simpa [hcast] using hdiffq0
    have hnonpos0 : (0 : ℚ) ≤ (n - u * d : Nat) := by positivity
    have hnonpos : ((u : ℚ) * d - n) ≤ 0 := by
      have : (0 : ℚ) ≤ (n : ℚ) - (u : ℚ) * d := by simpa [hcast] using hnonpos0
      linarith
    rw [abs_of_nonpos (div_nonpos_of_nonpos_of_nonneg hnonpos (le_of_lt hqpos))]
    rw [neg_div']
    apply (div_le_iff₀ hqpos).2
    linarith

/-- The exact rational half-denominator error of `roundNearest`. -/
theorem roundNearest_error (n d : Nat) (hd : 0 < d) :
    |(roundNearest n d : ℚ) - (n : ℚ) / d| ≤ (1 : ℚ) / 2 := by
  unfold roundNearest
  apply nat_error_to_rat _ _ _ hd
  exact quotient_round_error (n / d) (n % d) d n (Nat.mod_lt n hd) (by
    simpa [Nat.mul_comm] using (Nat.div_add_mod n d))

/-- A positive denominator stays positive under the model's scale choice. -/
theorem scalePair_den_pos (a b : Nat) (hb : 0 < b) :
    0 < (scalePair a b).2 := by
  unfold scalePair
  dsimp
  split
  · exact hb
  · exact Nat.mul_pos hb (by
      unfold pow2Int pow2
      exact Nat.pow_pos (by decide : 0 < (2 : Nat)))

/-- The scaled quotient has exactly the original rational value after undoing
    the binary exponent scale. -/
theorem scaled_exact_ratio (a b : Nat) (hb : 0 < b) :
    let e := floorLog2Ratio a b
    let scaled := scalePair a b
    ((scaled.1 : ℚ) / scaled.2) * (2 : ℚ) ^ (e - 23) = (a : ℚ) / b := by
  dsimp [scalePair]
  split
  · rename_i h
    have hk : 0 ≤ 23 - floorLog2Ratio a b := h
    have hP := pow2Int_rat (23 - floorLog2Ratio a b) hk
    rw [Nat.cast_mul, hP]
    have hpow : (2 : ℚ) ^ (23 - floorLog2Ratio a b) *
        (2 : ℚ) ^ (floorLog2Ratio a b - 23) = 1 := by
      rw [← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
      norm_num
    field_simp
    nlinarith
  · rename_i h
    have hk : 0 ≤ floorLog2Ratio a b - 23 := by omega
    have hP := pow2Int_rat (floorLog2Ratio a b - 23) hk
    rw [Nat.cast_mul, hP]
    field_simp

/-- The nearest quotient stays in the normal significand bin when its exact
    scaled input is in that bin. -/
theorem roundNearest_bounds (n d : Nat) (hd : 0 < d)
    (hlo : 2 ^ 23 * d ≤ n) (hhi : n < 2 ^ 24 * d) :
    2 ^ 23 ≤ roundNearest n d ∧ roundNearest n d ≤ 2 ^ 24 := by
  have hqlo : 2 ^ 23 ≤ n / d := (Nat.le_div_iff_mul_le hd).2 hlo
  have hqhi : n / d < 2 ^ 24 := (Nat.div_lt_iff_lt_mul hd).2 hhi
  have hqhi' : n / d ≤ 2 ^ 24 - 1 := by omega
  have hmod : n % d < d := Nat.mod_lt n hd
  unfold roundNearest
  by_cases h1 : 2 * (n % d) > d
  · simp [h1]
    omega
  · by_cases h2 : 2 * (n % d) = d
    · simp [h2]
      omega
    · simp [h1, h2]
      omega

/-- The scaled quotient's rounded significand obeys the same normal-bin bounds. -/
theorem roundedQuotient_bounds (a b : Nat) (hb : 0 < b)
    (hlo : 2 ^ 23 * (scalePair a b).2 ≤ (scalePair a b).1)
    (hhi : (scalePair a b).1 < 2 ^ 24 * (scalePair a b).2) :
    2 ^ 23 ≤ roundedQuotient a b ∧ roundedQuotient a b ≤ 2 ^ 24 := by
  simpa [roundedQuotient] using
    (roundNearest_bounds (scalePair a b).1 (scalePair a b).2
      (scalePair_den_pos a b hb) hlo hhi)

/-- Carrying a `2^24` significand preserves the represented exact dyadic. -/
theorem carry_value (e : Int) (rounded : Nat) :
    (if rounded = pow2 24 then
      ((pow2 23 : Nat) : ℚ) * (2 : ℚ) ^ (e + 1 - 23)
    else (rounded : ℚ) * (2 : ℚ) ^ (e - 23)) =
      (rounded : ℚ) * (2 : ℚ) ^ (e - 23) := by
  by_cases h : rounded = pow2 24
  · simp [h]
    rw [show e + 1 - 23 = (e - 23) + 1 by ring]
    rw [zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
    have hp : ((pow2 23 : Nat) : ℚ) * 2 = (pow2 24 : Nat) := by norm_num [pow2]
    calc
      ((pow2 23 : Nat) : ℚ) * ((2 : ℚ) ^ (e - 23) * (2 : ℚ) ^ 1) =
          (((pow2 23 : Nat) : ℚ) * 2) * (2 : ℚ) ^ (e - 23) := by norm_num; ring
      _ = ((pow2 24 : Nat) : ℚ) * (2 : ℚ) ^ (e - 23) := by rw [hp]
      _ = ((pow2 24 : Nat) : ℚ) * (2 : ℚ) ^ (e - 23) := by ring
  · simp [h]

/-- The carried significand remains in the normal encoding range. -/
theorem carriedPair_m_bounds (a b : Nat) (hb : 0 < b)
    (hlo : 2 ^ 23 * (scalePair a b).2 ≤ (scalePair a b).1)
    (hhi : (scalePair a b).1 < 2 ^ 24 * (scalePair a b).2) :
    2 ^ 23 ≤ (carriedPair a b).1 ∧ (carriedPair a b).1 < 2 ^ 24 := by
  have hrange := roundedQuotient_bounds a b hb hlo hhi
  unfold carriedPair
  by_cases h : roundedQuotient a b = pow2 24
  · simp only [if_pos h]
    norm_num [pow2]
  · have hupper : roundedQuotient a b < pow2 24 := by
      exact Nat.lt_of_le_of_ne hrange.2 h
    simp only [if_neg h]
    exact ⟨hrange.1, hupper⟩

/-- The binary32 reference's `roundPositive` agrees with the normal encoding
    whenever the carried exponent is finite and not subnormal. -/
theorem roundPositive_eq_roundedNormalBits (a b : Nat)
    (hlo : -126 ≤ (carriedPair a b).2)
    (hhi : (carriedPair a b).2 ≤ 127) :
    BitVec.ofNat 32 (roundPositive a b) = roundedNormalBits a b := by
  let c := carriedPair a b
  have hc : c = carriedPair a b := rfl
  have hbody : roundPositive a b =
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
  rw [hbody]
  unfold roundedNormalBits
  dsimp
  rw [← hc]
  have hclo : -126 ≤ c.2 := by simpa [c] using hlo
  have hchi : c.2 ≤ 127 := by simpa [c] using hhi
  by_cases h1 : c.2 > 127
  · omega
  · simp only [if_neg h1]
    by_cases h2 : c.2 < -126
    · omega
    · simp only [if_neg h2]
      rfl

/-- The exact rational interpretation of a normal encoding. -/
theorem normalEncoding_scaledMagnitude {e : Int} {m : Nat}
    (he₁ : -126 ≤ e) (he₂ : e ≤ 127)
    (hm₁ : 2 ^ 23 ≤ m) (hm₂ : m < 2 ^ 24) :
    scaledMagnitude (normalEncoding e m) = (m, e - 23) := by
  unfold scaledMagnitude normalEncoding exponent fraction
  simp only [pow2, BitVec.toNat_ushiftRight, BitVec.toNat_ofNat]
  have henc : ((e + 127).toNat) * 2 ^ 23 + (m - 2 ^ 23) < 2 ^ 32 := by
    have heNat : (e + 127).toNat ≤ 254 := by omega
    have hmSub : m - 2 ^ 23 < 2 ^ 23 := by omega
    have hmSubLe : m - 2 ^ 23 ≤ 2 ^ 23 - 1 := by omega
    have hprod : (e + 127).toNat * 2 ^ 23 ≤ 254 * 2 ^ 23 :=
      Nat.mul_le_mul_right _ heNat
    have hsum := Nat.add_le_add hprod hmSubLe
    norm_num at hsum ⊢
    exact lt_of_le_of_lt hsum (by norm_num)
  rw [Nat.mod_eq_of_lt henc]
  rw [show (255 : Nat) = 2 ^ 8 - 1 by norm_num,
      show (8388607 : Nat) = 2 ^ 23 - 1 by norm_num]
  simp only [Nat.shiftRight_eq_div_pow, Nat.and_two_pow_sub_one_eq_mod]
  have hepos : 0 ≤ e + 127 := by omega
  have hmSub : m - 2 ^ 23 < 2 ^ 23 := by omega
  have hquot : ((e + 127).toNat * 2 ^ 23 + (m - 2 ^ 23)) / 2 ^ 23 = (e + 127).toNat := by
    calc
      ((e + 127).toNat * 2 ^ 23 + (m - 2 ^ 23)) / 2 ^ 23 =
          (2 ^ 23 * (e + 127).toNat + (m - 2 ^ 23)) / 2 ^ 23 := by rw [Nat.mul_comm]
      _ = (e + 127).toNat + (m - 2 ^ 23) / 2 ^ 23 :=
        Nat.mul_add_div (by norm_num : 0 < 2 ^ 23) _ _
      _ = (e + 127).toNat := by rw [Nat.div_eq_of_lt hmSub, Nat.add_zero]
  have hrem : ((e + 127).toNat * 2 ^ 23 + (m - 2 ^ 23)) % 2 ^ 23 = m - 2 ^ 23 := by
    calc
      ((e + 127).toNat * 2 ^ 23 + (m - 2 ^ 23)) % 2 ^ 23 =
          (((e + 127).toNat * 2 ^ 23) % 2 ^ 23 + (m - 2 ^ 23) % 2 ^ 23) % 2 ^ 23 :=
            Nat.add_mod _ _ _
      _ = (m - 2 ^ 23) % 2 ^ 23 := by
        have hmul : ((e + 127).toNat * 2 ^ 23) % 2 ^ 23 = 0 := by
          rw [Nat.mul_comm]
          exact Nat.mul_mod_right _ _
        rw [hmul, Nat.zero_add]
        simp only [Nat.mod_eq_of_lt hmSub]
      _ = m - 2 ^ 23 := Nat.mod_eq_of_lt hmSub
  rw [hquot, hrem]
  have hE8 : (e + 127).toNat < 2 ^ 8 := by omega
  rw [Nat.mod_eq_of_lt hE8]
  rw [Int.toNat_of_nonneg hepos]
  have heNatPos : 0 < (e + 127).toNat := by
    have hcast : ((e + 127).toNat : Int) = e + 127 := Int.toNat_of_nonneg hepos
    omega
  have hcond : ((e + 127).toNat == 0) = false := beq_false_of_ne (Nat.ne_of_gt heNatPos)
  simp [hcond]
  omega

/-- Normal encodings in this domain have a positive sign bit. -/
theorem normalEncoding_not_negative {e : Int} {m : Nat}
    (he₁ : -126 ≤ e) (he₂ : e ≤ 127)
    (hm₁ : 2 ^ 23 ≤ m) (hm₂ : m < 2 ^ 24) :
    isNegative (normalEncoding e m) = false := by
  unfold isNegative normalEncoding
  simp only [pow2, BitVec.toNat_ofNat]
  have heNat : (e + 127).toNat ≤ 254 := by omega
  have hmSub : m - 2 ^ 23 < 2 ^ 23 := by omega
  have hmSubLe : m - 2 ^ 23 ≤ 2 ^ 23 - 1 := by omega
  have hprod : (e + 127).toNat * 2 ^ 23 ≤ 254 * 2 ^ 23 :=
    Nat.mul_le_mul_right _ heNat
  have hsum := Nat.add_le_add hprod hmSubLe
  have henc : ((e + 127).toNat) * 2 ^ 23 + (m - 2 ^ 23) < 2 ^ 31 := by
    norm_num at hsum ⊢
    exact lt_of_le_of_lt hsum (by norm_num)
  rw [Nat.mod_eq_of_lt (lt_trans henc (by norm_num : 2 ^ 31 < 2 ^ 32))]
  rw [Nat.shiftRight_eq_div_pow]
  have hz : ((e + 127).toNat * 2 ^ 23 + (m - 2 ^ 23)) / 2 ^ 31 = 0 :=
    Nat.div_eq_of_lt henc
  rw [hz]
  decide

/-- A finite encoding's exact value follows directly from its scaled magnitude
    and sign facts. Keeping this helper polymorphic in the encoding avoids any
    reduction of a particular bit pattern while applying the normal decoder. -/
theorem exactValue_of_scaledMagnitude (b : Bits) (m : Nat) (s : Int)
    (h : scaledMagnitude b = (m, s)) (hs : isNegative b = false) :
    exactValue b = (m : ℚ) * (2 : ℚ) ^ s := by
  unfold exactValue
  rw [h, hs]
  norm_num

/-- The exact rational value of every bounded normal encoding. -/
theorem exactValue_normalEncoding {e : Int} {m : Nat}
    (he₁ : -126 ≤ e) (he₂ : e ≤ 127)
    (hm₁ : 2 ^ 23 ≤ m) (hm₂ : m < 2 ^ 24) :
    exactValue (normalEncoding e m) = (m : ℚ) * (2 : ℚ) ^ (e - 23) := by
  apply exactValue_of_scaledMagnitude (normalEncoding e m) m (e - 23)
  · exact normalEncoding_scaledMagnitude he₁ he₂ hm₁ hm₂
  · exact normalEncoding_not_negative he₁ he₂ hm₁ hm₂

/-- **Universal normal-range absolute error rule.**  For every positive rational
    `a/b`, when the scaled exact quotient is in one normal significand bin and
    the carry stays in the finite normal exponent range, the model's exact
    rounded value is within one half of that bin's ulp.  The scale and bin
    hypotheses are explicit so a future checker can establish them rather than
    treating a real interval as evidence about an intermediate. -/
theorem roundPositive_normal_absolute_error (a b : Nat)
    (_ha : 0 < a) (hb : 0 < b)
    (hscaleLo : 2 ^ 23 * (scalePair a b).2 ≤ (scalePair a b).1)
    (hscaleHi : (scalePair a b).1 < 2 ^ 24 * (scalePair a b).2)
    (hcarryLo : -126 ≤ (carriedPair a b).2)
    (hcarryHi : (carriedPair a b).2 ≤ 127) :
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤
      (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23) := by
  have hm := carriedPair_m_bounds a b hb hscaleLo hscaleHi
  have hout := roundPositive_eq_roundedNormalBits a b hcarryLo hcarryHi
  have hval := exactValue_normalEncoding hcarryLo hcarryHi hm.1 hm.2
  have hval' : exactValue (roundedNormalBits a b) =
      ((carriedPair a b).1 : ℚ) * (2 : ℚ) ^ ((carriedPair a b).2 - 23) := by
    unfold roundedNormalBits
    exact hval
  have hratio := scaled_exact_ratio a b hb
  have hqerr :
      |(roundedQuotient a b : ℚ) -
        ((scalePair a b).1 : ℚ) / (scalePair a b).2| ≤ (1 : ℚ) / 2 := by
    exact roundNearest_error (scalePair a b).1 (scalePair a b).2
      (scalePair_den_pos a b hb)
  have hcarryVal :
      ((carriedPair a b).1 : ℚ) * (2 : ℚ) ^ ((carriedPair a b).2 - 23) =
        (roundedQuotient a b : ℚ) * (2 : ℚ) ^ (floorLog2Ratio a b - 23) := by
    unfold carriedPair
    dsimp
    by_cases h : roundedQuotient a b = pow2 24
    · simpa [h] using
        (carry_value (floorLog2Ratio a b) (roundedQuotient a b))
    · simp [h]
  calc
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| =
        |exactValue (roundedNormalBits a b) - (a : ℚ) / b| := by rw [hout]
    _ = |((carriedPair a b).1 : ℚ) *
          (2 : ℚ) ^ ((carriedPair a b).2 - 23) - (a : ℚ) / b| := by
      rw [hval']
    _ = |(roundedQuotient a b : ℚ) * (2 : ℚ) ^
          (floorLog2Ratio a b - 23) -
          ((scalePair a b).1 : ℚ) / (scalePair a b).2 *
            (2 : ℚ) ^ (floorLog2Ratio a b - 23)| := by
      rw [hcarryVal, ← hratio]
    _ = |((roundedQuotient a b : ℚ) -
          ((scalePair a b).1 : ℚ) / (scalePair a b).2) *
            (2 : ℚ) ^ (floorLog2Ratio a b - 23)| := by
      congr 1
      ring
    _ = |(roundedQuotient a b : ℚ) -
          ((scalePair a b).1 : ℚ) / (scalePair a b).2| *
            (2 : ℚ) ^ (floorLog2Ratio a b - 23) := by
      have hpow : (0 : ℚ) < (2 : ℚ) ^ (floorLog2Ratio a b - 23) :=
        zpow_pos (a := (2 : ℚ)) (by norm_num) _
      rw [abs_mul, abs_of_pos hpow]
    _ ≤ (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23) := by
      have hpow : (0 : ℚ) < (2 : ℚ) ^ (floorLog2Ratio a b - 23) :=
        zpow_pos (a := (2 : ℚ)) (by norm_num) _
      exact mul_le_mul_of_nonneg_right hqerr
        (le_of_lt hpow)

/-- **Non-vacuity witness**: the absolute rule applies to the exact input
    `1/3`.  Its custom logarithm reduces to `log2 3 = 1`, the scaled quotient
    is in one normal significand bin, and the carried exponent is finite. -/
theorem normal_error_applies_to_one_third :
    |exactValue (BitVec.ofNat 32 (roundPositive 1 3)) - (1 : ℚ) / 3| ≤
      (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio 1 3 - 23) := by
  have hlog : log2 3 = 1 := by decide
  have h25 : Int.toNat (25 : Int) = 25 := by decide
  have hlo : 2 ^ 23 * (scalePair 1 3).2 ≤ (scalePair 1 3).1 := by
    simp [scalePair, floorLog2Ratio, hlog, pow2, pow2Int, h25]
  have hhi : (scalePair 1 3).1 < 2 ^ 24 * (scalePair 1 3).2 := by
    simp [scalePair, floorLog2Ratio, hlog, pow2, pow2Int, h25]
  have hclo : -126 ≤ (carriedPair 1 3).2 := by
    simp [carriedPair, roundedQuotient, roundNearest, scalePair, floorLog2Ratio,
      hlog, pow2, pow2Int, h25]
  have hchi : (carriedPair 1 3).2 ≤ 127 := by
    simp [carriedPair, roundedQuotient, roundNearest, scalePair, floorLog2Ratio,
      hlog, pow2, pow2Int, h25]
  exact roundPositive_normal_absolute_error 1 3 (by decide) (by decide)
    hlo hhi hclo hchi

/-- The half-ulp rule is relative in the normal range once the exact input is
    known to be at least its bin's lower endpoint. -/
theorem roundPositive_normal_relative_error (a b : Nat)
    (_ha : 0 < a) (hb : 0 < b)
    (hscaleLo : 2 ^ 23 * (scalePair a b).2 ≤ (scalePair a b).1)
    (hscaleHi : (scalePair a b).1 < 2 ^ 24 * (scalePair a b).2)
    (hcarryLo : -126 ≤ (carriedPair a b).2)
    (hcarryHi : (carriedPair a b).2 ≤ 127)
    (hrange : (2 : ℚ) ^ floorLog2Ratio a b ≤ (a : ℚ) / b) :
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤
      (1 / (2 : ℚ) ^ 24) * ((a : ℚ) / b) := by
  have habs := roundPositive_normal_absolute_error a b _ha hb hscaleLo hscaleHi hcarryLo hcarryHi
  have hhalf : (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23) =
      (1 / (2 : ℚ) ^ 24) * (2 : ℚ) ^ floorLog2Ratio a b := by
    rw [show floorLog2Ratio a b - 23 = floorLog2Ratio a b + (-23) by ring,
      zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0), zpow_neg]
    norm_num [pow2]
    field_simp
    norm_num
  calc
    |exactValue (BitVec.ofNat 32 (roundPositive a b)) - (a : ℚ) / b| ≤
        (1 : ℚ) / 2 * (2 : ℚ) ^ (floorLog2Ratio a b - 23) := habs
    _ = (1 / (2 : ℚ) ^ 24) * (2 : ℚ) ^ floorLog2Ratio a b := hhalf
    _ ≤ (1 / (2 : ℚ) ^ 24) * ((a : ℚ) / b) := by
      exact mul_le_mul_of_nonneg_left hrange (by positivity)

/-- The flush-to-zero mutant violates the absolute half-smallest-subnormal
    allowance at `smallestNormal * 0.5`: the exact product is `2^-127`, while
    the mutant returns zero and the error is larger than `2^-150`. -/
theorem flush_to_zero_violates_absolute_bound :
    exactValue (flushSubnormal (mul 0x00800000 0x3F000000)) = 0
      ∧ exactValue (mul 0x00800000 0x3F000000) = (1 : ℚ) / (2 : ℚ) ^ 127
      ∧ |exactValue (flushSubnormal (mul 0x00800000 0x3F000000)) -
          exactValue (mul 0x00800000 0x3F000000)| > (1 : ℚ) / (2 : ℚ) ^ 150 := by
  have hmul : mul 0x00800000 0x3F000000 = 0x00400000 := by decide
  have hflush : flushSubnormal 0x00400000 = 0x00000000 := by decide
  have hzero : scaledMagnitude (0x00000000 : Bits) = (0, -149) := by decide
  have hsub : scaledMagnitude (0x00400000 : Bits) = (4194304, -149) := by decide
  have hzerosign : isNegative (0x00000000 : Bits) = false := by decide
  have hsubsign : isNegative (0x00400000 : Bits) = false := by decide
  rw [hmul, hflush]
  constructor
  · unfold exactValue
    rw [hzero, hzerosign]
    norm_num
  constructor
  · unfold exactValue
    rw [hsub, hsubsign]
    norm_num [pow_succ]
  · unfold exactValue
    rw [hzero, hzerosign, hsub, hsubsign]
    norm_num [pow_succ]

end NobleFoundation.Binary32.Error
