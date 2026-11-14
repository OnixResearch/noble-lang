import NobleFoundation.Binary32SequenceFinite
import NobleFoundation.Binary32Checker
import NobleFoundation.CheckedI64
import NobleFoundation.DecimalImport

/-!
# Explicit I64/binary32 boundary conversions (issue #40)

This file models named conversions over the existing binary32 encoding and exact-value reference.
An I64 is a `BitVec 64`; its signed mathematical value is `toInt`. The RNE direction calls the
existing one-round `roundScaled` operation. The reverse direction uses a signed quotient with
truncation toward zero, then checks finiteness and the signed I64 range before constructing a
64-bit result. Exact adapters additionally compare against the reference rational value. These
are model definitions and theorems, not a proof that the Rust adapters implement them.
-/

namespace NobleFoundation.Binary32.Convert

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error
open NobleFoundation.Binary32.DecimalImport
open NobleFoundation.Binary32.Sequence
open NobleFoundation.Binary32.SequenceFinite
open NobleFoundation.CheckedI64

set_option exponentiation.threshold 4096

/-- The three explicit refusal classes of the scalar boundary adapters. -/
inductive ConversionError where
  | inexact
  | notFinite
  | overflow
deriving DecidableEq, Repr

/-- One integer-scaled division, with signed quotient truncation toward zero. -/
def truncateScaled (magnitude : Int) (exponent : Int) : Int :=
  if 0 ≤ exponent then magnitude * (pow2Int exponent : Int)
  else CheckedI64.tquot magnitude (pow2Int (-exponent) : Int)

/-- The integer obtained by truncating a finite binary32 value toward zero. -/
def truncateBinary32 (bits : Bits) : Int :=
  let (magnitude, exponent) := scaledValue bits
  truncateScaled magnitude exponent

/-- I64 to binary32 round-to-nearest, ties-to-even, through the shared model. -/
def i64ToF32Rne (value : BitVec 64) : Bits :=
  if value.toInt = 0 then zeroOf false
  else roundScaled value.toInt 0 (decide (value.toInt < 0))

/-- I64 to binary32 exact conversion. -/
def i64ToF32Exact (value : BitVec 64) : Except ConversionError Bits :=
  if finiteBits (i64ToF32Rne value) then
    if exactValue (i64ToF32Rne value) = (value.toInt : ℚ) then
      .ok (i64ToF32Rne value)
    else
      .error .inexact
  else
    .error .overflow

/-- Binary32 to I64 truncation toward zero, followed by a signed-range check. -/
def f32ToI64Trunc (bits : Bits) : Except ConversionError (BitVec 64) :=
  if finiteBits bits then
    if I64Range (truncateBinary32 bits) then
      .ok (BitVec.ofInt 64 (truncateBinary32 bits))
    else
      .error .overflow
  else
    .error .notFinite

/-- Binary32 to I64 exact conversion. -/
def f32ToI64Exact (bits : Bits) : Except ConversionError (BitVec 64) :=
  if finiteBits bits then
    if I64Range (truncateBinary32 bits) then
      if exactValue bits = (truncateBinary32 bits : ℚ) then
        .ok (BitVec.ofInt 64 (truncateBinary32 bits))
      else
        .error .inexact
    else
      .error .overflow
  else
    .error .notFinite

/-- The RNE adapter preserves the signed exact integer within the shared model's mixed rounding
bound whenever the one scaled rounding is finite. The finite-range premise is explicit. -/
theorem i64ToF32Rne_mixed_error (value : BitVec 64) (nonzero : value.toInt ≠ 0)
    (finite : NoOverflow (scaledArgs value.toInt 0)) :
    |exactValue (i64ToF32Rne value) - (value.toInt : ℚ)| ≤
      unit * |(value.toInt : ℚ)| + eta := by
  have bound := roundScaled_mixed_error value.toInt 0 nonzero finite
  simpa [i64ToF32Rne, nonzero] using bound

/-- A finite model encoding is recognized as finite by the import model. -/
theorem finiteBits_of_Finite {bits : Bits}
    (finite : NobleFoundation.Binary32.Sequence.Finite bits) :
    finiteBits bits = true := by
  have notNaN := finite.1
  have notInfinity := finite.2
  unfold finiteBits
  cases hclass : classify bits with
  | negZero => rfl
  | posZero => rfl
  | subnormal => rfl
  | normal => rfl
  | infinity => exact False.elim (notInfinity hclass)
  | notANumber => exact False.elim (notNaN hclass)

/-- Every signed I64 is far below the binary32 overflow range, so RNE conversion always returns a
finite encoding. -/
theorem i64ToF32Rne_finite (value : BitVec 64) :
    finiteBits (i64ToF32Rne value) = true := by
  by_cases zero : value.toInt = 0
  · have finite := NobleFoundation.Binary32.Checker.zeroOf_finite false
    simpa [i64ToF32Rne, zero] using finiteBits_of_Finite finite
  · have range := toInt_range value
    have lower : -(2 : ℚ) ^ 63 ≤ (value.toInt : ℚ) := by exact_mod_cast range.1
    have upper : (value.toInt : ℚ) ≤ (2 : ℚ) ^ 63 := by
      exact_mod_cast (le_of_lt range.2)
    have magnitude : |(value.toInt : ℚ)| ≤ (2 : ℚ) ^ 63 := abs_le.mpr ⟨lower, upper⟩
    have small : |(value.toInt : ℚ) * (2 : ℚ) ^ (0 : Int)| <
        NobleFoundation.Binary32.Checker.overflowLimit := by
      have powers : (2 : ℚ) ^ (63 : Int) < (2 : ℚ) ^ (127 : Int) := by norm_num
      exact lt_of_le_of_lt (by simpa using magnitude) (by simpa [NobleFoundation.Binary32.Checker.overflowLimit] using powers)
    have noOverflow := NobleFoundation.Binary32.Checker.noOverflow_scaled
      value.toInt 0 zero small
    have finite := NobleFoundation.Binary32.Checker.roundScaled_finite
      value.toInt 0 zero noOverflow (decide (value.toInt < 0))
    simpa [i64ToF32Rne, zero] using finiteBits_of_Finite finite

/-- Exact conversion from I64 cannot fail for overflow: its RNE intermediate is always finite. -/
theorem i64ToF32Exact_no_overflow (value : BitVec 64) :
    i64ToF32Exact value ≠ .error .overflow := by
  by_cases exact : exactValue (i64ToF32Rne value) = (value.toInt : ℚ)
  · simp [i64ToF32Exact, i64ToF32Rne_finite, exact]
  · simp [i64ToF32Exact, i64ToF32Rne_finite, exact]

/-- Every successful exact I64-to-F32 conversion preserves the signed integer's exact rational
value. -/
theorem i64ToF32Exact_success_value (value : BitVec 64) (result : Bits)
    (success : i64ToF32Exact value = .ok result) :
    exactValue result = (value.toInt : ℚ) := by
  unfold i64ToF32Exact at success
  by_cases finite : finiteBits (i64ToF32Rne value) = true
  · simp only [finite, ite_true] at success
    by_cases exact : exactValue (i64ToF32Rne value) = (value.toInt : ℚ)
    · simp only [exact, ite_true] at success
      cases success
      exact exact
    · simp only [exact, ite_false] at success
      cases success
  · have finiteFalse : finiteBits (i64ToF32Rne value) = false := by
      cases resultFinite : finiteBits (i64ToF32Rne value) with
      | false => rfl
      | true => exact False.elim (finite resultFinite)
    simp only [finiteFalse] at success
    cases success

/-- A negative scaled quotient reconstructs its source as divisor times the truncating quotient
plus the signed remainder. This pins the toward-zero division rule used by the F32-to-I64 adapter.
-/
theorem truncateScaled_division_identity (magnitude exponent : Int)
    (negative : exponent < 0) :
    magnitude =
      (pow2Int (-exponent) : Int) *
        truncateScaled magnitude exponent +
      CheckedI64.trem magnitude (pow2Int (-exponent) : Int) := by
  have denominator_nonzero : (pow2Int (-exponent) : Int) ≠ 0 := by
    have denominator_positive : 0 < (pow2Int (-exponent) : Int) := by
      exact_mod_cast pow2Int_pos (-exponent)
    exact ne_of_gt denominator_positive
  have exponent_not_nonnegative : ¬ 0 ≤ exponent := by omega
  simpa [truncateScaled, exponent_not_nonnegative] using
    CheckedI64.div_rem_identity magnitude (pow2Int (-exponent) : Int) denominator_nonzero

/-- Every successful truncating adapter result is in the declared signed I64 range. -/
theorem f32ToI64Trunc_success_range (bits : Bits) (result : BitVec 64)
    (success : f32ToI64Trunc bits = .ok result) : I64Range result.toInt := by
  by_cases finite : finiteBits bits = true
  · by_cases range : I64Range (truncateBinary32 bits)
    · simp [f32ToI64Trunc, finite, range] at success
      subst result
      exact toInt_range _
    · simp [f32ToI64Trunc, finite, range] at success
  · simp [f32ToI64Trunc, finite] at success

/-- Every successful truncating conversion returns the quotient specified by `truncateBinary32`.
-/
theorem f32ToI64Trunc_success_value (bits : Bits) (result : BitVec 64)
    (success : f32ToI64Trunc bits = .ok result) :
    result.toInt = truncateBinary32 bits := by
  have range := f32ToI64Trunc_success_range bits result success
  by_cases finite : finiteBits bits = true
  · have value_range : I64Range (truncateBinary32 bits) := by
      by_contra refused
      simp [f32ToI64Trunc, finite, refused] at success
    simp [f32ToI64Trunc, finite, value_range] at success
    subst result
    rw [toInt_ofInt_of_range value_range]
  · simp [f32ToI64Trunc, finite] at success

/-- Successful exact conversion preserves the finite binary32 rational exactly. -/
theorem f32ToI64Exact_success_value (bits : Bits) (result : BitVec 64)
    (success : f32ToI64Exact bits = .ok result) :
    exactValue bits = (result.toInt : ℚ) := by
  by_cases finite : finiteBits bits = true
  · by_cases range : I64Range (truncateBinary32 bits)
    · by_cases exact : exactValue bits = (truncateBinary32 bits : ℚ)
      · simp [f32ToI64Exact, finite, range, exact] at success
        subst result
        rw [toInt_ofInt_of_range range]
        exact exact
      · simp [f32ToI64Exact, finite, range, exact] at success
    · simp [f32ToI64Exact, finite, range] at success
  · simp [f32ToI64Exact, finite] at success

/-- A finite, in-range fractional binary32 value is refused by the exact adapter as inexact. -/
theorem f32ToI64Exact_refuses_fractional (bits : Bits)
    (finite : finiteBits bits = true)
    (range : I64Range (truncateBinary32 bits))
    (fractional : exactValue bits ≠ (truncateBinary32 bits : ℚ)) :
    f32ToI64Exact bits = .error .inexact := by
  simp [f32ToI64Exact, finite, range, fractional]

/-- Either signed zero converts to integer zero under truncating conversion. -/
theorem f32ToI64Trunc_signed_zero (negative : Bool) :
    f32ToI64Trunc (zeroOf negative) = .ok (BitVec.ofInt 64 0) := by
  cases negative <;> decide

/-- Either signed zero converts to integer zero under exact conversion. -/
theorem f32ToI64Exact_signed_zero (negative : Bool) :
    f32ToI64Exact (zeroOf negative) = .ok (BitVec.ofInt 64 0) := by
  cases negative with
  | false =>
    have finite : finiteBits (zeroOf false) = true := by decide
    have zeroClass : classify (zeroOf false) = .posZero := by decide
    have zeroMagnitude := zero_class_magnitude (zeroOf false) (Or.inl zeroClass)
    have zeroValue : exactValue (zeroOf false) = 0 := exactValue_zeroOf false
    have zeroScaled : scaledValue (zeroOf false) = (0, -149) := by
      unfold scaledValue
      rw [zeroMagnitude]
      decide
    have truncated : truncateBinary32 (zeroOf false) = 0 := by
      rw [truncateBinary32, zeroScaled]
      norm_num [truncateScaled, CheckedI64.tquot, CheckedI64.sgn]
    simp [f32ToI64Exact, finite, zeroValue, truncated, I64Range]
  | true =>
    have finite : finiteBits (zeroOf true) = true := by decide
    have zeroClass : classify (zeroOf true) = .negZero := by decide
    have zeroMagnitude := zero_class_magnitude (zeroOf true) (Or.inr zeroClass)
    have zeroValue : exactValue (zeroOf true) = 0 := exactValue_zeroOf true
    have zeroScaled : scaledValue (zeroOf true) = (0, -149) := by
      unfold scaledValue
      rw [zeroMagnitude]
      decide
    have truncated : truncateBinary32 (zeroOf true) = 0 := by
      rw [truncateBinary32, zeroScaled]
      norm_num [truncateScaled, CheckedI64.tquot, CheckedI64.sgn]
    simp [f32ToI64Exact, finite, zeroValue, truncated, I64Range]

/-- Zero has the unique positive encoding in the I64-to-F32 direction. -/
theorem i64ToF32Rne_zero : i64ToF32Rne (BitVec.ofInt 64 0) = zeroOf false := by
  decide

/-- `2^24` and its two neighboring odd integers exercise exactness and both ties-to-even sides.
-/
theorem i64ToF32Rne_twoPow24_plus_one :
    i64ToF32Rne (BitVec.ofInt 64 16777217) = 0x4b800000 := by decide

theorem i64ToF32Rne_twoPow24_plus_three :
    i64ToF32Rne (BitVec.ofInt 64 16777219) = 0x4b800002 := by decide

theorem i64ToF32Rne_negative_twoPow24_plus_one :
    i64ToF32Rne (BitVec.ofInt 64 (-16777217)) = 0xcb800000 := by decide

theorem i64ToF32Rne_negative_twoPow24_plus_three :
    i64ToF32Rne (BitVec.ofInt 64 (-16777219)) = 0xcb800002 := by decide

/-- The signed I64 endpoint and adjacent inputs all round without wrapping; exact conversion
accepts the exactly representable minimum and refuses the inexact maximum. -/
theorem i64ToF32Rne_minimum :
    i64ToF32Rne (BitVec.ofInt 64 (-(2 ^ 63 : Int))) = 0xdf000000 := by decide

theorem i64ToF32Rne_minimum_succ :
    i64ToF32Rne (BitVec.ofInt 64 (-(2 ^ 63 : Int) + 1)) = 0xdf000000 := by decide

theorem i64ToF32Rne_maximum :
    i64ToF32Rne (BitVec.ofInt 64 (2 ^ 63 - 1)) = 0x5f000000 := by decide

theorem i64ToF32Rne_maximum_pred :
    i64ToF32Rne (BitVec.ofInt 64 (2 ^ 63 - 2)) = 0x5f000000 := by decide

theorem i64ToF32Exact_minimum :
    i64ToF32Exact (BitVec.ofInt 64 (-(2 ^ 63 : Int))) = .ok 0xdf000000 := by
  have exact_value : exactValue (3741319168#32) = (-(2 ^ 63 : Int) : ℚ) := by
    norm_num [exactValue, scaledMagnitude, exponent, fraction, isNegative, pow2]
  have rounded : i64ToF32Rne (9223372036854775808#64) = (0xdf000000 : Bits) := by
    simpa using i64ToF32Rne_minimum
  simp [i64ToF32Exact, rounded, finiteBits, classify, exponent, fraction, isNegative, exact_value]

theorem i64ToF32Exact_maximum :
    i64ToF32Exact (BitVec.ofInt 64 (2 ^ 63 - 1)) = .error .inexact := by
  have exact_value : exactValue (1593835520#32) = (2 ^ 63 : Int) := by
    norm_num [exactValue, scaledMagnitude, exponent, fraction, isNegative, pow2]
  have rounded : i64ToF32Rne (9223372036854775807#64) = (0x5f000000 : Bits) := by
    simpa using i64ToF32Rne_maximum
  simp [i64ToF32Exact, rounded, finiteBits, classify, exponent, fraction, isNegative, exact_value]

/-- F32 fractional truncation, subnormal underflow, signed range endpoints, infinities and NaNs.
-/
theorem f32ToI64Trunc_positive_fraction :
    f32ToI64Trunc 0x3fe00000 = .ok (BitVec.ofInt 64 1) := by decide

theorem f32ToI64Trunc_negative_fraction :
    f32ToI64Trunc 0xbfe00000 = .ok (BitVec.ofInt 64 (-1)) := by decide

theorem f32ToI64Trunc_subnormal_underflow :
    f32ToI64Trunc 0x00000001 = .ok (BitVec.ofInt 64 0) := by decide

theorem f32ToI64Trunc_negative_subnormal_underflow :
    f32ToI64Trunc 0x80000001 = .ok (BitVec.ofInt 64 0) := by decide

theorem f32ToI64Trunc_minimum :
    f32ToI64Trunc 0xdf000000 = .ok (BitVec.ofInt 64 (-(2 ^ 63 : Int))) := by decide

theorem f32ToI64Exact_minimum :
    f32ToI64Exact 0xdf000000 = .ok (BitVec.ofInt 64 (-(2 ^ 63 : Int))) := by
  have exact_value : exactValue (3741319168#32) = (-(2 ^ 63 : Int) : ℚ) := by
    norm_num [exactValue, scaledMagnitude, exponent, fraction, isNegative, pow2]
  have truncated_value : truncateBinary32 (3741319168#32) = -(2 ^ 63 : Int) := by
    decide
  simp [f32ToI64Exact, finiteBits, classify, exponent, fraction, isNegative, exact_value,
    truncated_value, I64Range]

theorem f32ToI64Trunc_positive_overflow :
    f32ToI64Trunc 0x5f000000 = .error .overflow := by decide

theorem f32ToI64Exact_positive_overflow :
    f32ToI64Exact 0x5f000000 = .error .overflow := by decide

theorem f32ToI64Trunc_negative_overflow :
    f32ToI64Trunc 0xdf000001 = .error .overflow := by decide

theorem f32ToI64Trunc_infinity_and_nan :
    f32ToI64Trunc 0x7f800000 = .error .notFinite ∧
      f32ToI64Trunc 0xff800000 = .error .notFinite ∧
      f32ToI64Trunc 0x7fc00001 = .error .notFinite ∧
      f32ToI64Trunc 0xffc00001 = .error .notFinite := by decide

theorem f32ToI64Exact_infinity_and_nan :
    f32ToI64Exact 0x7f800000 = .error .notFinite ∧
      f32ToI64Exact 0xff800000 = .error .notFinite ∧
      f32ToI64Exact 0x7fc00001 = .error .notFinite ∧
      f32ToI64Exact 0xffc00001 = .error .notFinite := by decide

end NobleFoundation.Binary32.Convert
