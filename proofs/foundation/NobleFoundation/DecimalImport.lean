import NobleFoundation.Binary32
import NobleFoundation.Binary32Error

/-!
A kernel-reducible decimal-to-binary32 import model (issue #40).

A `Decimal` keeps the sign spelling, a base-ten digit sequence, and an
integer power of ten.  `decimalRatio` interprets that spelling as a natural
ratio.  `rne` applies the existing positive binary32 rounding rule once and
then restores the spelling's sign; it does not route through another binary
format.  `exactImport` accepts only a finite rounded result whose scaled
integer value is exactly the decimal ratio.

This file is a reference model.  It does not select an overflow response,
syntax, resource limit, or implementation correspondence.
-/

namespace NobleFoundation.Binary32.DecimalImport

open NobleFoundation.Binary32

set_option exponentiation.threshold 4096
set_option maxRecDepth 100000

/-- A decimal spelling before syntax and resource-policy decisions. -/
structure Decimal where
  negative : Bool
  digits : List (Fin 10)
  exponent : Int
deriving DecidableEq, Repr

/-- The natural value of a most-significant-digit-first decimal sequence. -/
def digitsValue (digits : List (Fin 10)) : Nat :=
  digits.foldl (fun value digit => value * 10 + digit.val) 0

/-- The exact natural numerator and denominator of a decimal spelling. -/
def decimalRatio (d : Decimal) : Nat × Nat :=
  let magnitude := digitsValue d.digits
  if (0 : Int) ≤ d.exponent then
    (magnitude * 10 ^ d.exponent.toNat, 1)
  else
    (magnitude, 10 ^ (-d.exponent).toNat)

/-- The signed exact rational value of a decimal spelling. -/
def exactDecimalValue (d : Decimal) : ℚ :=
  let ratio := decimalRatio d
  if d.negative then -(ratio.1 : ℚ) / ratio.2 else (ratio.1 : ℚ) / ratio.2

/-- Set the binary32 sign bit on a nonnegative rounded magnitude. -/
def signedMagnitude (negative : Bool) (magnitude : Nat) : Bits :=
  BitVec.ofNat 32 (if negative then magnitude ||| 0x80000000 else magnitude)

/-- Round one nonzero decimal magnitude once with round-to-nearest, ties-to-even.
    A zero magnitude retains its explicit sign spelling. -/
def rne (d : Decimal) : Bits :=
  let magnitude := digitsValue d.digits
  if magnitude = 0 then
    zeroOf d.negative
  else
    let ratio := decimalRatio d
    signedMagnitude d.negative (roundPositive ratio.1 ratio.2)

/-- Result classes used only to expose rounded overflow to a caller. -/
inductive OutcomeClass where
  | finite
  | overflow
deriving DecidableEq, Repr

/-- Classify an RNE result; this does not choose an infinity-versus-error policy. -/
def outcomeClass (d : Decimal) : OutcomeClass :=
  if classify (rne d) == .infinity then .overflow else .finite

/-- Whether an encoding is finite in the binary32 observation model. -/
def finiteBits (bits : Bits) : Bool :=
  match classify bits with
  | .infinity => false
  | .notANumber => false
  | _ => true

/-- Exactness of a finite encoding, checked by natural cross multiplication.
    The two branches move a negative binary exponent to the opposite side. -/
def scaledMatches (d : Decimal) (bits : Bits) : Bool :=
  let ratio := decimalRatio d
  let scaled := scaledMagnitude bits
  if (0 : Int) ≤ scaled.2 then
    scaled.1 * pow2Int scaled.2 * ratio.2 == ratio.1
  else
    scaled.1 * ratio.2 == ratio.1 * pow2Int (-scaled.2)

/-- Exact import retains a result only when the rounded finite encoding is exact. -/
def exactImport (d : Decimal) : Option Bits :=
  let bits := rne d
  if finiteBits bits then
    if scaledMatches d bits then some bits else none
  else
    none

/-- The decimal denominator is always positive. -/
theorem decimalRatio_den_pos (d : Decimal) : 0 < (decimalRatio d).2 := by
  unfold decimalRatio
  dsimp
  split
  · simp
  · exact Nat.pow_pos (by decide)

/-- The existing binary32 sign predicate is the bitvector most-significant bit. -/
theorem isNegative_eq_msb (bits : Bits) : isNegative bits = bits.msb := by
  rw [BitVec.msb_eq_decide]
  unfold isNegative
  simp only [Nat.shiftRight_eq_div_pow]
  have hbits : bits.toNat < 2 ^ 32 := bits.isLt
  by_cases h : 2147483648 ≤ bits.toNat
  · have hdiv : bits.toNat / 2147483648 = 1 := by omega
    simp only [hdiv]
    norm_num [h]
  · have hlt : bits.toNat < 2147483648 := by omega
    have hdiv : bits.toNat / 2147483648 = 0 := Nat.div_eq_of_lt hlt
    simp only [hdiv]
    norm_num [h]

/-- A negative spelling always retains its sign bit, including signed zero and
    underflowed magnitudes. -/
theorem rne_negative_sign (digits : List (Fin 10)) (exponent : Int) :
    isNegative (rne ⟨true, digits, exponent⟩) = true := by
  rw [isNegative_eq_msb]
  simp only [rne]
  split
  · decide
  · unfold signedMagnitude
    simp only [if_true]
    rw [BitVec.ofNat_or, BitVec.msb_or]
    change (BitVec.ofNat 32 (roundPositive (decimalRatio ⟨true, digits, exponent⟩).1
      (decimalRatio ⟨true, digits, exponent⟩).2)).msb ||
      (BitVec.ofNat 32 0x80000000).msb = true
    have hsign : (BitVec.ofNat 32 0x80000000).msb = true := by decide
    simp [hsign]

/-- A zero-valued spelling keeps its explicit sign, independently of exponent. -/
theorem zero_spelling (negative : Bool) (digits : List (Fin 10)) (exponent : Int)
    (hzero : digitsValue digits = 0) :
    rne ⟨negative, digits, exponent⟩ = zeroOf negative := by
  simp [rne, hzero]

/-- The classifier reports overflow exactly for an infinity-magnitude result. -/
theorem outcomeClass_overflow_iff (d : Decimal) :
    outcomeClass d = .overflow ↔ classify (rne d) = .infinity := by
  unfold outcomeClass
  split <;> simp_all

/-- Exact import's acceptance test is finite output plus natural cross equality. -/
theorem exactImport_some_iff (d : Decimal) :
    exactImport d = some (rne d) ↔
      finiteBits (rne d) = true ∧ scaledMatches d (rne d) = true := by
  unfold exactImport
  dsimp
  by_cases hfinite : finiteBits (rne d)
  · simp [hfinite]
  · simp [hfinite]

/-- Any accepted exact import is the single RNE result for that spelling. -/
theorem exactImport_agrees_with_rne {d : Decimal} {bits : Bits}
    (h : exactImport d = some bits) : bits = rne d := by
  unfold exactImport at h
  dsimp at h
  split at h <;> simp_all

/-- A finite exact import agrees with the rational exact-value model. -/
theorem exactImport_exactValue {d : Decimal} {bits : Bits}
    (h : exactImport d = some bits)
    (hsign : isNegative bits = d.negative) :
    NobleFoundation.Binary32.Error.exactValue bits = exactDecimalValue d := by
  have hrne : bits = rne d := exactImport_agrees_with_rne h
  have hsame : exactImport d = some (rne d) := by
    rw [← hrne]
    exact h
  have hmatch : scaledMatches d (rne d) = true :=
    (exactImport_some_iff d).mp hsame |>.2
  rcases hratio : decimalRatio d with ⟨a, den⟩
  rcases hscaled : scaledMagnitude bits with ⟨m, s⟩
  have hden : 0 < den := by
    have hden' := decimalRatio_den_pos d
    rw [hratio] at hden'
    exact hden'
  rw [← hrne] at hmatch
  unfold scaledMatches at hmatch
  dsimp at hmatch
  rw [hscaled, hratio] at hmatch
  by_cases hs : (0 : Int) ≤ s
  · simp only [if_pos hs] at hmatch
    have heq : m * pow2Int s * den = a := by
      simpa using hmatch
    have hpow := NobleFoundation.Binary32.Error.pow2Int_rat s hs
    have hrat : (m : ℚ) * (2 : ℚ) ^ s = (a : ℚ) / den := by
      rw [← hpow]
      have hcast : ((m * pow2Int s * den : Nat) : ℚ) = a := by
        exact_mod_cast heq
      field_simp
      simpa [Nat.cast_mul, Nat.cast_ofNat] using hcast
    unfold NobleFoundation.Binary32.Error.exactValue exactDecimalValue
    rw [hscaled, hsign, hratio]
    cases hd : d.negative
    all_goals simp [hrat]
    all_goals ring
  · have hns : 0 ≤ -s := by omega
    simp only [if_neg hs] at hmatch
    have heq : m * den = a * pow2Int (-s) := by
      simpa using hmatch
    have hpow := NobleFoundation.Binary32.Error.pow2Int_rat (-s) hns
    have hrat : (m : ℚ) * (2 : ℚ) ^ s = (a : ℚ) / den := by
      have hpowneg : (2 : ℚ) ^ s = ((2 : ℚ) ^ (-s))⁻¹ := by
        calc
          (2 : ℚ) ^ s = (2 : ℚ) ^ (-(-s)) := by
            congr 1
            ring
          _ = ((2 : ℚ) ^ (-s))⁻¹ := zpow_neg (2 : ℚ) (-s)
      rw [hpowneg, ← hpow]
      have hcast : ((m * den : Nat) : ℚ) = a * pow2Int (-s) := by
        exact_mod_cast heq
      have hpowpos : 0 < pow2Int (-s) := by
        unfold pow2Int pow2
        exact Nat.pow_pos (by decide)
      have hpowne : (pow2Int (-s) : ℚ) ≠ 0 := by
        exact_mod_cast (Nat.ne_of_gt hpowpos)
      field_simp [hpowne]
      simpa [Nat.cast_mul, Nat.cast_ofNat, mul_comm] using hcast
    unfold NobleFoundation.Binary32.Error.exactValue exactDecimalValue
    rw [hscaled, hsign, hratio]
    cases hd : d.negative
    all_goals simp [hrat]
    all_goals ring

/-- The decimal spelling `-0` is retained as negative zero. -/
theorem negative_zero_fixture :
    rne ⟨true, [0], 0⟩ = 0x80000000 := by
  decide

/-- The exact decimal `0.1` is not exactly representable, while RNE has one
    deterministic binary32 result. -/
theorem decimal_point_one_fixture :
    rne ⟨false, [1], -1⟩ = 0x3DCCCCCD ∧
    exactImport ⟨false, [1], -1⟩ = none := by
  constructor <;> decide

/-- The exact decimal `0.5` is accepted by exact import. -/
theorem decimal_point_five_fixture :
    exactImport ⟨false, [5], -1⟩ = some 0x3F000000 := by
  decide

/-- Decimal `10^39` rounds to an infinity magnitude and is classified as
    overflow; the response policy is intentionally outside this classifier. -/
theorem decimal_one_e39_overflow_fixture :
    outcomeClass ⟨false, [1], 39⟩ = .overflow := by
  decide

/-- Decimal `10^-45` rounds to the least positive binary32 subnormal. -/
theorem decimal_least_subnormal_fixture :
    rne ⟨false, [1], -45⟩ = 0x00000001 := by
  decide

/-- The exact halfway decimal `1 + 2^-24` selects the even lower encoding. -/
theorem decimal_tie_fixture :
    rne ⟨false, [1,0,0,0,0,0,0,0,5,9,6,0,4,6,4,4,7,7,5,3,9,0,6,2,5], -24⟩ =
      0x3F800000 := by
  decide

/-- A decimal just above the preceding tie selects the next encoding by one
    RNE operation; no intermediate binary format is part of this statement. -/
theorem decimal_just_above_tie_fixture :
    rne ⟨false, [1,0,0,0,0,0,0,0,5,9,6,0,4,6,4,4,7,7,5,3,9,0,6,2,6], -24⟩ =
      0x3F800001 := by
  decide

end NobleFoundation.Binary32.DecimalImport
