import NobleFoundation.DecimalImport
import NobleFoundation.Binary32Error
import NobleFoundation.Binary32Subnormal
import NobleFoundation.Binary32SequenceFinite
import Mathlib.Data.Nat.Bitwise

/-!
# Decimal spelling algebra and rounding specification (issue #40)

`DecimalImport` gives the exact rational reading of a decimal spelling, one-round RNE, and the
exact-import predicate. What it does not yet give is the algebra those definitions need for the
universal range and round-trip lemmas the issue asks for: how a digit sequence's value grows when a
digit is appended, what each exponent branch of `decimalRatio` is, and what `rne` returns for a
zero magnitude.

This module proves exactly that and adds a canonical decimal digit expansion for finite encodings:
- `digitsValue_snoc`: appending a digit multiplies by ten and adds it;
- `decimalRatio_nonneg` / `decimalRatio_neg`: the two branches of the ratio, stated once with the
  branch condition as an explicit premise (the identity cannot hold across `e = -1`, where the
  denominator is `10` and the next exponent's is `1`);
- `rne_zero_magnitude`, `rne_empty_digits`: a zero magnitude rounds to the spelling's signed zero;
- `rne_zero_exactValue`: the signed-zero result has exactly the spelling's rational value;
- `spellingOf`: the exact binary32 dyadic magnitude rendered as finite decimal digits;
- the two `zeroOf` values, and that `signedMagnitude` without a sign is the plain magnitude.

The rounding theorem below relates the model's single RNE operation directly to the exact signed
rational, with the finite-range premise made explicit. It is a model theorem, not a Rust parser or
implementation correspondence. The canonical spelling round-trip below is also a model theorem;
it does not establish syntax/resource-policy behavior or a Rust parser correspondence. The sharp
overflow cut `2^128 − 2^103` is characterized in `Binary32.Scale.carriedPair_finite_iff_below_overflowMidpoint`.
-/

namespace NobleFoundation.Binary32.DecimalImport

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error
open NobleFoundation.Binary32.Scale
open NobleFoundation.Binary32.Sequence
open NobleFoundation.Binary32.SequenceFinite
open NobleFoundation.Binary32.Subnormal

set_option maxRecDepth 100000

/-- Appending one decimal digit multiplies the value by ten and adds that digit. -/
theorem digitsValue_snoc (digits : List (Fin 10)) (digit : Fin 10) :
    digitsValue (digits ++ [digit]) = digitsValue digits * 10 + digit.val := by
  simp only [digitsValue, List.foldl_append, List.foldl_cons, List.foldl_nil]

/-- The most-significant-digit-first base-ten digits of a natural number. -/
def decimalDigits (value : Nat) : List (Fin 10) :=
  if h : value < 10 then
    [⟨value, by omega⟩]
  else
    decimalDigits (value / 10) ++ [⟨value % 10, Nat.mod_lt value (by decide)⟩]
termination_by value
decreasing_by omega

/-- `decimalDigits` preserves the natural value of its input. -/
theorem digitsValue_decimalDigits (value : Nat) :
    digitsValue (decimalDigits value) = value := by
  induction value using Nat.strong_induction_on with
  | h value ih =>
    by_cases small : value < 10
    · simp [decimalDigits, small, digitsValue]
    · have divided : value / 10 < value := by omega
      have recurse := ih (value / 10) divided
      have digits : decimalDigits value =
          decimalDigits (value / 10) ++ [⟨value % 10, Nat.mod_lt value (by decide)⟩] := by
        rw [decimalDigits]
        exact dite_eq_right small
      rw [digits, digitsValue_snoc, recurse]
      simpa [Nat.mul_comm] using Nat.div_add_mod value 10

/-- The empty digit sequence has value zero. -/
theorem digitsValue_nil : digitsValue [] = 0 := rfl

/-- A nonnegative exponent scales the digit value by a power of ten over one. -/
theorem decimalRatio_nonneg {d : Decimal} (h : (0 : Int) ≤ d.exponent) :
    decimalRatio d = (digitsValue d.digits * 10 ^ d.exponent.toNat, 1) := by
  unfold decimalRatio
  simp only [h, ite_true]

/-- A negative exponent puts a power of ten under the digit value. -/
theorem decimalRatio_neg {d : Decimal} (h : d.exponent < 0) :
    decimalRatio d = (digitsValue d.digits, 10 ^ (-d.exponent).toNat) := by
  unfold decimalRatio
  have hnot : ¬ (0 : Int) ≤ d.exponent := by omega
  simp only [hnot, ite_false]

/-- The exact dyadic magnitude of a binary32 encoding written as a decimal spelling. -/
def spellingOf (bits : Bits) : Decimal :=
  let (magnitude, exponent) := scaledMagnitude bits
  if exponent < 0 then
    ⟨isNegative bits, decimalDigits (magnitude * 5 ^ (-exponent).toNat), exponent⟩
  else
    ⟨isNegative bits, decimalDigits (magnitude * pow2Int exponent), 0⟩

/-- Every finite binary32 value has an exact finite decimal spelling. For a negative binary
exponent, multiplying the dyadic significand by `5^k` and retaining exponent `-k` gives the same
rational; for a nonnegative exponent the integer is expanded directly. -/
theorem exactDecimalValue_spellingOf (bits : Bits) :
    exactDecimalValue (spellingOf bits) = exactValue bits := by
  rcases hscaled : scaledMagnitude bits with ⟨m, s⟩
  have spellingSign : (spellingOf bits).negative = isNegative bits := by
    by_cases h : s < 0 <;> simp [spellingOf, hscaled, h]
  have exactScaled : exactValue bits =
      (if isNegative bits then (-(m : Int) : ℚ) else (m : ℚ)) * (2 : ℚ) ^ s := by
    unfold exactValue
    rw [hscaled]
  by_cases hs : 0 ≤ s
  · have hnot : ¬ s < 0 := by omega
    have hratio : decimalRatio (spellingOf bits) = (m * pow2Int s, 1) := by
      simp [spellingOf, hscaled, hnot, decimalRatio, digitsValue_decimalDigits]
    have hpow := NobleFoundation.Binary32.Error.pow2Int_rat s hs
    have hvalue : exactDecimalValue (spellingOf bits) =
        (if isNegative bits then -(m : ℚ) else (m : ℚ)) * (2 : ℚ) ^ s := by
      simp [exactDecimalValue, spellingSign, hratio, hpow]
    rw [hvalue, exactScaled]
    simp only [Int.cast_natCast]
  · have hneg : s < 0 := by omega
    have hratio : decimalRatio (spellingOf bits) =
        (m * 5 ^ (-s).toNat, 10 ^ (-s).toNat) := by
      simp [spellingOf, hscaled, hneg, decimalRatio, digitsValue_decimalDigits]
    have hpow := NobleFoundation.Binary32.Error.pow2Int_rat (-s) (by omega)
    have hpowneg : (2 : ℚ) ^ s = ((pow2Int (-s) : ℚ))⁻¹ := by
      calc
        (2 : ℚ) ^ s = (2 : ℚ) ^ (-(-s)) := by congr 1; ring
        _ = ((2 : ℚ) ^ (-s))⁻¹ := zpow_neg (2 : ℚ) (-s)
        _ = ((pow2Int (-s) : ℚ))⁻¹ := by rw [hpow]
    have hdenNat : 10 ^ (-s).toNat = pow2Int (-s) * 5 ^ (-s).toNat := by
      change 10 ^ (-s).toNat = 2 ^ (-s).toNat * 5 ^ (-s).toNat
      rw [show (10 : Nat) = 2 * 5 by norm_num, Nat.mul_pow]
    have hdenRat : (10 : ℚ) ^ (-s).toNat =
        (pow2Int (-s) : ℚ) * (5 : ℚ) ^ (-s).toNat := by exact_mod_cast hdenNat
    have hfive : (5 : ℚ) ^ (-s).toNat ≠ 0 := by positivity
    have hpow2 : (pow2Int (-s) : ℚ) ≠ 0 := by
      have hpos : 0 < pow2Int (-s) := by
        unfold pow2Int pow2
        positivity
      exact_mod_cast (ne_of_gt hpos)
    have hmag : ((m * 5 ^ (-s).toNat : Nat) : ℚ) /
        ((10 ^ (-s).toNat : Nat) : ℚ) = (m : ℚ) * (2 : ℚ) ^ s := by
      calc
        ((m * 5 ^ (-s).toNat : Nat) : ℚ) / ((10 ^ (-s).toNat : Nat) : ℚ) =
            (m : ℚ) * (5 : ℚ) ^ (-s).toNat /
              ((pow2Int (-s) : ℚ) * (5 : ℚ) ^ (-s).toNat) := by
                push_cast
                rw [hdenRat]
        _ = (m : ℚ) * (pow2Int (-s) : ℚ)⁻¹ := by
          field_simp [hfive, hpow2]
        _ = (m : ℚ) * (2 : ℚ) ^ s := by rw [hpowneg]
    have hvalue : exactDecimalValue (spellingOf bits) =
        (if isNegative bits then -(m : ℚ) else (m : ℚ)) * (2 : ℚ) ^ s := by
      unfold exactDecimalValue
      dsimp only
      rw [spellingSign, hratio]
      dsimp only
      cases hsign : isNegative bits
      · simpa [hsign] using hmag
      · simpa [hsign, neg_div] using congrArg (fun value : ℚ => -value) hmag
    rw [hvalue, exactScaled]
    simp only [Int.cast_natCast]

/-- The raw binary32 words are the sign, exponent and fraction fields in their arithmetic layout. -/
theorem bits_toNat_fields (bits : Bits) :
    bits.toNat = (if isNegative bits then 2 ^ 31 else 0) + exponent bits * 2 ^ 23 + fraction bits := by
  have hn : bits.toNat < 2 ^ 32 := bits.isLt
  have hf := fraction_of_toNat bits
  have he := exponent_of_toNat bits
  have hs := isNegative_of_toNat bits
  have hq : bits.toNat / 2 ^ 23 < 2 ^ 9 := by omega
  have hqsplit : bits.toNat / 2 ^ 23 =
      bits.toNat / 2 ^ 31 * 2 ^ 8 + bits.toNat / 2 ^ 23 % 2 ^ 8 := by omega
  have hsign : bits.toNat / 2 ^ 31 = if isNegative bits then 1 else 0 := by
    have hlt : bits.toNat / 2 ^ 31 < 2 := by omega
    cases hnegative : isNegative bits <;> simp [hnegative] at hs ⊢ <;> omega
  have hdivmod := Nat.div_add_mod bits.toNat (2 ^ 23)
  have hdivmod' : bits.toNat / 2 ^ 23 * 2 ^ 23 + bits.toNat % 2 ^ 23 = bits.toNat := by
    calc
      bits.toNat / 2 ^ 23 * 2 ^ 23 + bits.toNat % 2 ^ 23 =
          2 ^ 23 * (bits.toNat / 2 ^ 23) + bits.toNat % 2 ^ 23 := by rw [Nat.mul_comm]
      _ = bits.toNat := hdivmod
  calc
    bits.toNat = bits.toNat / 2 ^ 23 * 2 ^ 23 + bits.toNat % 2 ^ 23 := hdivmod'.symm
    _ = (bits.toNat / 2 ^ 31 * 2 ^ 8 + bits.toNat / 2 ^ 23 % 2 ^ 8) * 2 ^ 23 + bits.toNat % 2 ^ 23 := by
      nth_rw 1 [hqsplit]
    _ = (if isNegative bits then 1 else 0) * 2 ^ 31 +
        (bits.toNat / 2 ^ 23 % 2 ^ 8) * 2 ^ 23 + bits.toNat % 2 ^ 23 := by
      cases sign : isNegative bits
      · have hquot : bits.toNat / 2 ^ 31 = 0 := by
          have hlt : bits.toNat / 2 ^ 31 < 2 := by omega
          simp [sign] at hs
          omega
        simp
        omega
      · have hquot : bits.toNat / 2 ^ 31 = 1 := by
          have hlt : bits.toNat / 2 ^ 31 < 2 := by omega
          simp [sign] at hs
          omega
        rw [hquot]
        norm_num [pow_succ]
        ring
    _ = (if isNegative bits then 2 ^ 31 else 0) + exponent bits * 2 ^ 23 + fraction bits := by
      rw [he, hf]
      cases isNegative bits <;> norm_num

/-- Reconstructing an encoding from its observed three fields is exact. -/
theorem bits_eq_fields (bits : Bits) :
    bits = BitVec.ofNat 32
      ((if isNegative bits then 2 ^ 31 else 0) + exponent bits * 2 ^ 23 + fraction bits) := by
  have hfields := bits_toNat_fields bits
  have hraw : (if isNegative bits then 2 ^ 31 else 0) + exponent bits * 2 ^ 23 + fraction bits < 2 ^ 32 := by
    rw [← hfields]
    exact bits.isLt
  apply BitVec.eq_of_toNat_eq
  rw [BitVec.toNat_ofNat, Nat.mod_eq_of_lt hraw]
  exact hfields

/-- A sign bit does not overlap a magnitude below `2^31`, so natural OR is addition. -/
theorem nat_or_two_pow_eq_add (m k : Nat) (hm : m < 2 ^ k) :
    m ||| 2 ^ k = 2 ^ k + m := by
  induction k generalizing m with
  | zero =>
      have hzero : m = 0 := by simpa using hm
      subst m
      norm_num
  | succ k ih =>
    have hdiv : m / 2 < 2 ^ k := by omega
    have hpow : 2 ^ (k + 1) = Nat.bit false (2 ^ k) := by
      rw [Nat.pow_succ, Nat.bit_val]
      simp
      ring
    calc
      m ||| 2 ^ (k + 1) = Nat.bit (Nat.bodd m) (m / 2) ||| Nat.bit false (2 ^ k) := by
        rw [hpow]
        exact congrArg (fun value => value ||| Nat.bit false (2 ^ k))
          (Nat.bit_bodd_div2 m).symm
      _ = Nat.bit (Nat.bodd m || false) ((m / 2) ||| 2 ^ k) := by rw [Nat.lor_bit]
      _ = Nat.bit (Nat.bodd m) (2 ^ k + m / 2) := by simp [ih _ hdiv]
      _ = 2 ^ (k + 1) + m := by
        rw [Nat.bit_val, Nat.pow_succ]
        have hdecomp := Nat.bodd_add_div2 m
        rw [Nat.div2_val] at hdecomp
        omega

/-- A set sign and the decoded magnitude fields reconstruct the original bits. -/
theorem signedMagnitude_eq_fields (bits : Bits) :
    signedMagnitude (isNegative bits) (exponent bits * 2 ^ 23 + fraction bits) = bits := by
  let magnitude := exponent bits * 2 ^ 23 + fraction bits
  have hfrac : fraction bits < 2 ^ 23 := by
    rw [fraction_of_toNat]
    exact Nat.mod_lt _ (by norm_num)
  have hexp : exponent bits < 2 ^ 8 := by
    rw [exponent_of_toNat]
    exact Nat.mod_lt _ (by norm_num)
  have hsmall : magnitude < 2 ^ 31 := by
    dsimp [magnitude]
    omega
  by_cases hnegative : isNegative bits
  · have hfields : bits = BitVec.ofNat 32 (2 ^ 31 + magnitude) := by
      simpa [hnegative, magnitude, Nat.add_assoc] using bits_eq_fields bits
    have hor : magnitude ||| 0x80000000 = 2 ^ 31 + magnitude := by
      simpa only [show (0x80000000 : Nat) = 2 ^ 31 by norm_num] using
        nat_or_two_pow_eq_add magnitude 31 hsmall
    calc
      signedMagnitude (isNegative bits) (exponent bits * 2 ^ 23 + fraction bits) =
          BitVec.ofNat 32 (magnitude ||| 0x80000000) := by
        simp [signedMagnitude, hnegative, magnitude]
      _ = BitVec.ofNat 32 (2 ^ 31 + magnitude) := by rw [hor]
      _ = bits := hfields.symm
  · have hfields : bits = BitVec.ofNat 32 magnitude := by
      simpa [hnegative, magnitude] using bits_eq_fields bits
    calc
      signedMagnitude (isNegative bits) (exponent bits * 2 ^ 23 + fraction bits) =
          BitVec.ofNat 32 magnitude := by
        simp [signedMagnitude, hnegative, magnitude]
      _ = bits := hfields.symm

/-- The sign-preserving decimal spelling denotes the exact nonnegative magnitude. -/
theorem decimalRatio_spellingOf_dyadic (bits : Bits) :
    (decimalRatio (spellingOf bits)).1 / (decimalRatio (spellingOf bits)).2 =
      ((scaledMagnitude bits).1 : ℚ) * (2 : ℚ) ^ (scaledMagnitude bits).2 := by
  rcases hscaled : scaledMagnitude bits with ⟨m, s⟩
  have hsign : (spellingOf bits).negative = isNegative bits := by
    by_cases h : s < 0 <;> simp [spellingOf, hscaled, h]
  have hvalue := exactDecimalValue_spellingOf bits
  unfold exactDecimalValue exactValue at hvalue
  rw [hsign, hscaled] at hvalue
  dsimp only at hvalue
  change (decimalRatio (spellingOf bits)).1 / (decimalRatio (spellingOf bits)).2 =
      (m : ℚ) * (2 : ℚ) ^ s
  cases hnegative : isNegative bits
  · simpa [hnegative, exactValue, hscaled] using hvalue
  · have hneg : -((decimalRatio (spellingOf bits)).1 : ℚ) /
        (decimalRatio (spellingOf bits)).2 =
      -((m : ℚ) * (2 : ℚ) ^ s) := by
        simpa [hnegative, exactValue, hscaled, neg_div, neg_mul] using hvalue
    have hneg' : -(((decimalRatio (spellingOf bits)).1 : ℚ) /
        (decimalRatio (spellingOf bits)).2) =
      -((m : ℚ) * (2 : ℚ) ^ s) := by simpa [neg_div] using hneg
    exact neg_inj.mp hneg'

/-- A zero digit value rounds to the spelling's own signed zero, before any ratio is formed. -/
theorem rne_zero_magnitude (d : Decimal) (h : digitsValue d.digits = 0) :
    rne d = zeroOf d.negative := by
  unfold rne
  simp only [h, ite_true]

/-- The model's RNE result is the single positive `roundPositive` result with the spelling's sign.
The nonzero premise prevents the signed-zero branch from hiding the ratio. -/
theorem rne_nonzero_encoding (d : Decimal) (h : 0 < digitsValue d.digits) :
    rne d = signedMagnitude d.negative (roundPositive (decimalRatio d).1 (decimalRatio d).2) := by
  unfold rne
  have hne : digitsValue d.digits ≠ 0 := Nat.ne_of_gt h
  simp [hne]

/-- A finite encoding's exponent field is strictly below the all-ones nonfinite field. -/
private theorem finite_exp_lt_255 (bits : Bits) (h : finiteBits bits = true) :
    exponent bits < 255 := by
  have hexp_lt : exponent bits < 256 := by
    rw [exponent_of_toNat]
    exact Nat.mod_lt _ (by norm_num)
  by_contra hbad
  have he : exponent bits = 255 := by omega
  cases hf : fraction bits with
  | zero => simp [finiteBits, classify, he, hf] at h
  | succ f => simp [finiteBits, classify, he, hf] at h

/-- Below the normal binade, an exact subnormal decimal spelling rounds to its original
significand. The only carry case is isolated at logarithm `-127`; there the scaled quotient is
exactly twice the subnormal significand and remains below `2^24`. -/
private theorem subnormal_spelling_rounds (bits : Bits)
    (hm : scaledMagnitude bits = (fraction bits, -149))
    (hmpos : 0 < fraction bits) (hmhi : fraction bits < 2^23) :
    roundPositive (decimalRatio (spellingOf bits)).1 (decimalRatio (spellingOf bits)).2 =
      fraction bits := by
  rcases hpair : decimalRatio (spellingOf bits) with ⟨a, b⟩
  have hb : 0 < b := by
    have := decimalRatio_den_pos (spellingOf bits)
    rw [hpair] at this
    exact this
  have hratio : (a : ℚ) / b = (fraction bits : ℚ) * (2 : ℚ)^(-149 : Int) := by
    have h := decimalRatio_spellingOf_dyadic bits
    rw [hpair, hm] at h
    exact h
  have ha : 0 < a := by
    have hbQ : (0 : ℚ) < (b : ℚ) := by exact_mod_cast hb
    have hrpos : (0 : ℚ) < (fraction bits : ℚ) * (2 : ℚ)^(-149 : Int) :=
      mul_pos (by exact_mod_cast hmpos) (zpow_pos (by norm_num) _)
    by_contra hnonpos
    have hzero : a = 0 := by omega
    have hleft : (a : ℚ) / b = 0 := by simp [hzero]
    rw [hleft] at hratio
    linarith
  have hpowScale : (2 : ℚ)^(23 : Int) * (2 : ℚ)^(-149 : Int) = (2 : ℚ)^(-126 : Int) := by
    rw [← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
    norm_num
  have hratioSmall : (a : ℚ) / b < (2 : ℚ)^(-126 : Int) := by
    have hmQ : (fraction bits : ℚ) < (2 : ℚ)^23 := by exact_mod_cast hmhi
    rw [hratio]
    calc
      (fraction bits : ℚ) * (2 : ℚ)^(-149 : Int) < (2 : ℚ)^23 * (2 : ℚ)^(-149 : Int) :=
        mul_lt_mul_of_pos_right hmQ (zpow_pos (by norm_num) _)
      _ = (2 : ℚ)^(-126 : Int) := hpowScale
  have hqle : floorLog2Ratio a b ≤ -127 := by
    by_contra hbad
    have hq : (-126 : Int) ≤ floorLog2Ratio a b := by omega
    have hpow : (2 : ℚ)^(-126 : Int) ≤ (2 : ℚ)^floorLog2Ratio a b :=
      zpow_le_zpow_right₀ (by norm_num) hq
    have hbQ : (0 : ℚ) < (b : ℚ) := by exact_mod_cast hb
    have hloCross : (2 : ℚ)^floorLog2Ratio a b * (b : ℚ) ≤ (a : ℚ) :=
      floorLog2Ratio_lower a b ha hb
    have hlo : (2 : ℚ)^floorLog2Ratio a b ≤ (a : ℚ) / b :=
      (le_div_iff₀ hbQ).2 hloCross
    exact (not_le_of_gt hratioSmall) (le_trans hpow hlo)
  have hcarryUpper : (carriedPair a b).2 ≤ floorLog2Ratio a b + 1 := by
    unfold carriedPair
    dsimp only
    split <;> dsimp only <;> omega
  have hlow : (carriedPair a b).2 < -126 := by
    by_cases hq : floorLog2Ratio a b ≤ -128
    · omega
    · have hqeq : floorLog2Ratio a b = -127 := by omega
      have hpow : (2 : ℚ)^(-149 : Int) * (2 : ℚ)^(150 : Int) = 2 := by
        rw [← zpow_add₀ (by norm_num : (2 : ℚ) ≠ 0)]
        norm_num
      have hscaleRatio : ((scalePair a b).1 : ℚ) / (scalePair a b).2 =
          (2 * fraction bits : ℚ) := by
        rw [scaled_ratio a b hb, hratio, hqeq]
        calc
          (fraction bits : ℚ) * (2 : ℚ)^(-149 : Int) *
              (2 : ℚ)^((23 : Int) - (-127 : Int)) =
              (fraction bits : ℚ) * ((2 : ℚ)^(-149 : Int) * (2 : ℚ)^(150 : Int)) := by ring
          _ = (fraction bits : ℚ) * 2 := by rw [hpow]
          _ = (2 * fraction bits : ℚ) := by ring
      have hdQ : (0 : ℚ) < (scalePair a b).2 := by
        exact_mod_cast scalePair_den_pos a b hb
      have hscaleMul : (scalePair a b).1 = 2 * fraction bits * (scalePair a b).2 := by
        have h := (div_eq_iff (ne_of_gt hdQ)).mp hscaleRatio
        exact_mod_cast h
      have hrounded : roundedQuotient a b = 2 * fraction bits := by
        unfold roundedQuotient
        dsimp only
        rw [hscaleMul]
        exact NobleFoundation.Binary32.Scale.roundNearest_exact_multiple
          (2 * fraction bits) (scalePair a b).2 (scalePair_den_pos a b hb)
      have hnotcarry : 2 * fraction bits ≠ pow2 24 := by
        intro heq
        rw [pow2] at heq
        omega
      have hcarry : (carriedPair a b).2 = -127 := by
        unfold carriedPair
        rw [hqeq, hrounded]
        simp [hnotcarry]
      omega
  have hcrossQ : (a : ℚ) * (2 : ℚ)^149 = (fraction bits : ℚ) * b := by
    have hbQ : (0 : ℚ) < (b : ℚ) := by exact_mod_cast hb
    have hmul := (div_eq_iff (ne_of_gt hbQ)).mp hratio
    have hpowneg : (2 : ℚ)^(-149 : Int) = ((2 : ℚ)^149)⁻¹ := by
      rw [zpow_neg]
      norm_num
    rw [hpowneg] at hmul
    field_simp at hmul
    nlinarith
  have hcross : a * pow2 149 = fraction bits * b := by exact_mod_cast hcrossQ
  have hround : roundNearest (a * pow2 149) b = fraction bits := by
    rw [hcross]
    exact NobleFoundation.Binary32.Scale.roundNearest_exact_multiple (fraction bits) b hb
  rw [roundPositive_eq_significand a b ha hb hlow]
  simpa [hpair] using hround

/-- A finite normal exact spelling rounds to the same exponent and significand. -/
private theorem normal_spelling_rounds (bits : Bits) (finite : finiteBits bits = true)
    (hexp : exponent bits ≠ 0) :
    BitVec.ofNat 32 (roundPositive (decimalRatio (spellingOf bits)).1
      (decimalRatio (spellingOf bits)).2) =
        BitVec.ofNat 32 (exponent bits * 2^23 + fraction bits) := by
  rcases hpair : decimalRatio (spellingOf bits) with ⟨a, b⟩
  let e : Int := (exponent bits : Int) - 127
  let m : Nat := 2^23 + fraction bits
  have hexpLt : exponent bits < 255 := finite_exp_lt_255 bits finite
  have hfraction : fraction bits < 2^23 := by
    rw [fraction_of_toNat]
    exact Nat.mod_lt _ (by norm_num)
  have heLo : -126 ≤ e := by dsimp [e]; omega
  have heHi : e ≤ 127 := by dsimp [e]; omega
  have hmLo : 2^23 ≤ m := by dsimp [m]; omega
  have hmHi : m < 2^24 := by dsimp [m]; omega
  have hscaled : scaledMagnitude bits = (m, e - 23) := by
    unfold scaledMagnitude
    simp [hexp, m, e]
  have hratio : (a : ℚ) / b = (m : ℚ) * (2 : ℚ)^(e - 23) := by
    have h := decimalRatio_spellingOf_dyadic bits
    rw [hpair, hscaled] at h
    exact h
  have hb : 0 < b := by
    have := decimalRatio_den_pos (spellingOf bits)
    rw [hpair] at this
    exact this
  have hnormal := roundPositive_eq_normal_encoding_of_dyadic a b e m hb
    heLo heHi hmLo hmHi hratio
  have hePlus : e + 127 = (exponent bits : Int) := by
    dsimp [e]
    ring
  have heNat : (e + 127).toNat = exponent bits := by
    rw [hePlus]
    simp
  have hmSub : m - pow2 23 = fraction bits := by
    dsimp [m, pow2]
    omega
  have hnormalRaw : normalEncoding e m = BitVec.ofNat 32 (exponent bits * 2^23 + fraction bits) := by
    unfold normalEncoding
    rw [heNat, hmSub]
    simp [pow2]
  rw [hnormalRaw] at hnormal
  exact hnormal

/-- Restore the sign bit when the unsigned rounded word matches the source magnitude word. -/
private theorem signedMagnitude_eq_of_unsigned_encoding (negative : Bool) (bits : Bits)
    (raw magnitude : Nat) (hencoded : BitVec.ofNat 32 raw = BitVec.ofNat 32 magnitude)
    (hsmall : magnitude < 2^31)
    (hfields : bits = BitVec.ofNat 32 (if negative then 2^31 + magnitude else magnitude)) :
    signedMagnitude negative raw = bits := by
  cases negative
  · have hbits : bits = BitVec.ofNat 32 magnitude := by simpa using hfields
    calc
      signedMagnitude false raw = BitVec.ofNat 32 raw := rfl
      _ = BitVec.ofNat 32 magnitude := hencoded
      _ = bits := hbits.symm
  · have hbits : bits = BitVec.ofNat 32 (2^31 + magnitude) := by simpa using hfields
    have hor : magnitude ||| 0x80000000 = 2^31 + magnitude := by
      simpa only [show (0x80000000 : Nat) = 2^31 by norm_num] using
        nat_or_two_pow_eq_add magnitude 31 hsmall
    calc
      signedMagnitude true raw = BitVec.ofNat 32 (raw ||| 0x80000000) := rfl
      _ = BitVec.ofNat 32 (magnitude ||| 0x80000000) := by
        rw [BitVec.ofNat_or, BitVec.ofNat_or, hencoded]
      _ = BitVec.ofNat 32 (2^31 + magnitude) := by rw [hor]
      _ = bits := hbits.symm

/-- **Finite binary32 spelling round-trip.** Rendering any finite bit pattern as its exact decimal
spelling and rounding that spelling once returns the identical 32-bit pattern, including both
zero signs and every subnormal. -/
theorem rne_spellingOf_finite (bits : Bits) (finite : finiteBits bits = true) :
    rne (spellingOf bits) = bits := by
  have hexpLt : exponent bits < 255 := finite_exp_lt_255 bits finite
  have hfracLt : fraction bits < 2^23 := by
    rw [fraction_of_toNat]
    exact Nat.mod_lt _ (by norm_num)
  let rawMagnitude := exponent bits * 2^23 + fraction bits
  have hrawSmall : rawMagnitude < 2^31 := by
    dsimp [rawMagnitude]
    omega
  have hfields : bits = BitVec.ofNat 32
      (if isNegative bits then 2^31 + rawMagnitude else rawMagnitude) := by
    cases hsign : isNegative bits
    · simpa [rawMagnitude, hsign] using bits_eq_fields bits
    · simpa [rawMagnitude, hsign, Nat.add_assoc] using bits_eq_fields bits
  have spellingSign : (spellingOf bits).negative = isNegative bits := by
    rcases hscaled : scaledMagnitude bits with ⟨m, s⟩
    by_cases hs : s < 0 <;> simp [spellingOf, hscaled, hs]
  by_cases hzero : (scaledMagnitude bits).1 = 0
  · have hexp : exponent bits = 0 := by
      by_contra h
      have hmag : (scaledMagnitude bits).1 = 2^23 + fraction bits := by
        simp [scaledMagnitude, h]
      rw [hmag] at hzero
      omega
    have hfrac : fraction bits = 0 := by
      have hmag : (scaledMagnitude bits).1 = fraction bits := by simp [scaledMagnitude, hexp]
      rw [hmag] at hzero
      exact hzero
    have hscaledZero : scaledMagnitude bits = (0, -149) := by
      simp [scaledMagnitude, hexp, hfrac]
    have hdigits : digitsValue (spellingOf bits).digits = 0 := by
      simp [spellingOf, hscaledZero, digitsValue_decimalDigits]
    have hzeroBits : bits = zeroOf (isNegative bits) := by
      cases hsign : isNegative bits
      · simpa [zeroOf, hsign, hexp, hfrac] using bits_eq_fields bits
      · simpa [zeroOf, hsign, hexp, hfrac] using bits_eq_fields bits
    have hzeroResult : rne (spellingOf bits) = zeroOf (isNegative bits) := by
      rw [rne_zero_magnitude _ hdigits, spellingSign]
    exact hzeroResult.trans hzeroBits.symm
  · have hmagPos : 0 < (scaledMagnitude bits).1 := Nat.pos_of_ne_zero hzero
    have hdigits : 0 < digitsValue (spellingOf bits).digits := by
      rcases hscaled : scaledMagnitude bits with ⟨m, s⟩
      have hmpos : 0 < m := by simpa [hscaled] using hmagPos
      by_cases hs : s < 0
      · have hfive : 0 < 5 ^ (-s).toNat := by positivity
        have hvalue : 0 < m * 5^(-s).toNat := Nat.mul_pos hmpos hfive
        simpa [spellingOf, hscaled, hs, digitsValue_decimalDigits] using hvalue
      · have hpow : 0 < pow2Int s := by
          unfold pow2Int pow2
          exact Nat.pow_pos (by decide)
        have hvalue : 0 < m * pow2Int s := Nat.mul_pos hmpos hpow
        simpa [spellingOf, hscaled, hs, digitsValue_decimalDigits] using hvalue
    by_cases hexp : exponent bits = 0
    · have hsub : scaledMagnitude bits = (fraction bits, -149) := by
        simp [scaledMagnitude, hexp]
      have hsubpos : 0 < fraction bits := by
        have hm : (scaledMagnitude bits).1 = fraction bits := congrArg Prod.fst hsub
        rw [hm] at hzero
        omega
      have hround := subnormal_spelling_rounds bits hsub hsubpos hfracLt
      have hencoded : BitVec.ofNat 32 (roundPositive (decimalRatio (spellingOf bits)).1
          (decimalRatio (spellingOf bits)).2) = BitVec.ofNat 32 (fraction bits) := by rw [hround]
      have hsigned := signedMagnitude_eq_of_unsigned_encoding (isNegative bits) bits
        (roundPositive (decimalRatio (spellingOf bits)).1 (decimalRatio (spellingOf bits)).2)
        (fraction bits) hencoded (Nat.lt_of_lt_of_le hfracLt (by norm_num)) (by
          simpa [rawMagnitude, hexp] using hfields)
      calc
        rne (spellingOf bits) = signedMagnitude (spellingOf bits).negative
            (roundPositive (decimalRatio (spellingOf bits)).1 (decimalRatio (spellingOf bits)).2) :=
          rne_nonzero_encoding (spellingOf bits) hdigits
        _ = signedMagnitude (isNegative bits)
            (roundPositive (decimalRatio (spellingOf bits)).1 (decimalRatio (spellingOf bits)).2) := by
              rw [spellingSign]
        _ = bits := hsigned
    · have hround := normal_spelling_rounds bits finite hexp
      have hsigned := signedMagnitude_eq_of_unsigned_encoding (isNegative bits) bits
        (roundPositive (decimalRatio (spellingOf bits)).1 (decimalRatio (spellingOf bits)).2)
        rawMagnitude hround hrawSmall hfields
      calc
        rne (spellingOf bits) = signedMagnitude (spellingOf bits).negative
            (roundPositive (decimalRatio (spellingOf bits)).1 (decimalRatio (spellingOf bits)).2) :=
          rne_nonzero_encoding (spellingOf bits) hdigits
        _ = signedMagnitude (isNegative bits)
            (roundPositive (decimalRatio (spellingOf bits)).1 (decimalRatio (spellingOf bits)).2) := by
              rw [spellingSign]
        _ = bits := hsigned

/-- The signed-zero encodings of `zeroOf`. -/
theorem zeroOf_false : zeroOf false = 0 := rfl

theorem zeroOf_true : zeroOf true = 0x80000000 := rfl

/-- Without a sign, `signedMagnitude` is the plain magnitude. -/
theorem signedMagnitude_false (magnitude : Nat) :
    signedMagnitude false magnitude = BitVec.ofNat 32 magnitude := rfl

/-- A zero decimal spelling's signed-zero RNE result has exactly its signed rational value. -/
theorem rne_zero_exactValue (d : Decimal) (h : digitsValue d.digits = 0) :
    exactValue (rne d) = exactDecimalValue d := by
  rw [rne_zero_magnitude d h, exactValue_zeroOf]
  unfold exactDecimalValue
  have numerator_zero : (decimalRatio d).1 = 0 := by
    unfold decimalRatio
    dsimp
    split <;> simp [h]
  have rational_zero : exactDecimalValue d = 0 := by
    unfold exactDecimalValue
    rcases hratio : decimalRatio d with ⟨numerator, denominator⟩
    have numerator_eq_zero : numerator = 0 := by
      simpa [hratio] using numerator_zero
    simp [numerator_eq_zero]
  exact rational_zero.symm

/-- The empty digit sequence is a zero spelling in either sign. -/
theorem rne_empty_digits (negative : Bool) (exponent : Int) :
    rne ⟨negative, [], exponent⟩ = zeroOf negative :=
  rne_zero_magnitude ⟨negative, [], exponent⟩ rfl

/-- The decimal rational has a positive numerator for every nonzero digit magnitude. -/
theorem decimalRatio_num_pos (d : Decimal) (h : 0 < digitsValue d.digits) :
    0 < (decimalRatio d).1 := by
  unfold decimalRatio
  dsimp
  split
  · exact Nat.mul_pos h (Nat.pow_pos (by decide))
  · exact h

/-- The binary32 exact value follows the sign selected by `signedMagnitude`. -/
theorem exactValue_signedMagnitude (negative : Bool) (magnitude : Nat)
    (small : magnitude < 2 ^ 31) :
    exactValue (signedMagnitude negative magnitude) =
      if negative then -exactValue (BitVec.ofNat 32 magnitude)
      else exactValue (BitVec.ofNat 32 magnitude) := by
  cases negative
  · rfl
  · simpa [signedMagnitude] using exactValue_signed magnitude small

/-- **Single-round decimal specification.** Every nonzero decimal spelling whose exact rational
rounds to a finite binary32 result is within half the spacing of the resulting binade, with the
subnormal spacing used below the normal range. `NoOverflow` is an explicit premise on the carried
positive magnitude; no host parser or intermediate binary format appears in the statement. -/
theorem rne_exactValue_absolute_error (d : Decimal)
    (hnonzero : 0 < digitsValue d.digits)
    (hfinite : NoOverflow ((decimalRatio d).1, (decimalRatio d).2)) :
    |exactValue (rne d) - exactDecimalValue d| ≤
      (1 : ℚ) / 2 *
        (2 : ℚ) ^ (max (floorLog2Ratio (decimalRatio d).1 (decimalRatio d).2) (-126) - 23) := by
  have ha : 0 < (decimalRatio d).1 := decimalRatio_num_pos d hnonzero
  have hb : 0 < (decimalRatio d).2 := decimalRatio_den_pos d
  have hsmall := roundPositive_lt_finite _ _ ha hb hfinite
  have hround := roundPositive_finite_absolute_error _ _ ha hb hfinite
  have hrne := rne_nonzero_encoding d hnonzero
  rw [hrne, exactValue_signedMagnitude _ _ hsmall]
  unfold exactDecimalValue
  by_cases hnegative : d.negative
  · simp only [hnegative, ite_true]
    rw [neg_div]
    have hsign : -(exactValue (BitVec.ofNat 32
        (roundPositive (decimalRatio d).1 (decimalRatio d).2))) -
        (-(((decimalRatio d).1 : ℚ) / (decimalRatio d).2)) =
        -((exactValue (BitVec.ofNat 32
          (roundPositive (decimalRatio d).1 (decimalRatio d).2))) -
          ((decimalRatio d).1 : ℚ) / (decimalRatio d).2) := by ring
    rw [hsign, abs_neg]
    exact hround
  · simp only [hnegative]
    exact hround

/-- A finite rounded result exactly matching its decimal rational is accepted by the exact-import
predicate. This universal theorem makes the model's exactness test explicit: `scaledMatches` is the
natural cross-multiplication witness for equality, and the finite-result premise excludes NaN or
infinity. -/
theorem exactImport_accepts_representable_rne (d : Decimal)
    (finite : finiteBits (rne d) = true)
    (representable : scaledMatches d (rne d) = true) :
    exactImport d = some (rne d) :=
  (exactImport_some_iff d).2 ⟨finite, representable⟩

/-- Exact rational agreement of a finite signed binary32 result is equivalent to the natural
cross-multiplication witness consumed by `exactImport`. -/
theorem scaledMatches_of_exactValue (d : Decimal) (bits : Bits)
    (hsign : isNegative bits = d.negative)
    (hexact : exactValue bits = exactDecimalValue d) :
    scaledMatches d bits = true := by
  rcases hratio : decimalRatio d with ⟨a, den⟩
  rcases hscaled : scaledMagnitude bits with ⟨m, s⟩
  have hden : 0 < den := by
    have h := decimalRatio_den_pos d
    rw [hratio] at h
    exact h
  have hrat_signed :
      (if d.negative then -(m : ℚ) * (2 : ℚ) ^ s else (m : ℚ) * (2 : ℚ) ^ s) =
      (if d.negative then -(a : ℚ) / den else (a : ℚ) / den) := by
    simpa [exactValue, exactDecimalValue, hscaled, hratio, hsign] using hexact
  have hrat : (m : ℚ) * (2 : ℚ) ^ s = (a : ℚ) / den := by
    cases hd : d.negative with
    | false => simpa [hd] using hrat_signed
    | true =>
        have h : -((m : ℚ) * (2 : ℚ) ^ s) = -((a : ℚ) / den) := by
          simpa [hd, neg_div, neg_mul] using hrat_signed
        exact neg_inj.mp h
  by_cases hs : (0 : Int) ≤ s
  · have hpow := NobleFoundation.Binary32.Error.pow2Int_rat s hs
    rw [← hpow] at hrat
    have hcross : (m : ℚ) * (pow2Int s : ℚ) * den = (a : ℚ) := by
      rw [hrat]
      field_simp
    have hnat : m * pow2Int s * den = a := by exact_mod_cast hcross
    unfold scaledMatches
    rw [hscaled, hratio]
    simp [hs, hnat]
  · have hns : 0 ≤ -s := by omega
    have hpow := NobleFoundation.Binary32.Error.pow2Int_rat (-s) hns
    have hpowneg : (2 : ℚ) ^ s = ((pow2Int (-s) : ℚ))⁻¹ := by
      calc
        (2 : ℚ) ^ s = (2 : ℚ) ^ (-(-s)) := by congr 1; ring
        _ = ((2 : ℚ) ^ (-s))⁻¹ := zpow_neg (2 : ℚ) (-s)
        _ = ((pow2Int (-s) : ℚ))⁻¹ := by rw [hpow]
    rw [hpowneg] at hrat
    have hdenq : (den : ℚ) ≠ 0 := by exact_mod_cast (Nat.ne_of_gt hden)
    have hpowne : (pow2Int (-s) : ℚ) ≠ 0 :=
      ne_of_gt (by rw [hpow]; exact zpow_pos (by norm_num) _)
    field_simp [hdenq, hpowne] at hrat
    have hnat' : m * den = pow2Int (-s) * a := by exact_mod_cast hrat
    have hnat : m * den = a * pow2Int (-s) := by simpa [Nat.mul_comm] using hnat'
    unfold scaledMatches
    rw [hscaled, hratio]
    simp [hs, hnat]

/-- Every finite RNE result exactly equal to the signed decimal rational is accepted by the exact
import path; the theorem derives its natural cross-product check from rational equality. -/
theorem exactImport_accepts_representable (d : Decimal)
    (finite : finiteBits (rne d) = true)
    (hsign : isNegative (rne d) = d.negative)
    (representable : exactValue (rne d) = exactDecimalValue d) :
    exactImport d = some (rne d) :=
  exactImport_accepts_representable_rne d finite
    (scaledMatches_of_exactValue d (rne d) hsign representable)

end NobleFoundation.Binary32.DecimalImport
