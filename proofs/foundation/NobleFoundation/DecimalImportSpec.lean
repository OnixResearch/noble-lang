import NobleFoundation.DecimalImport
import NobleFoundation.Binary32Error
import NobleFoundation.Binary32Subnormal
import NobleFoundation.Binary32SequenceFinite

/-!
# Decimal spelling algebra and rounding specification (issue #40)

`DecimalImport` gives the exact rational reading of a decimal spelling, one-round RNE, and the
exact-import predicate. What it does not yet give is the algebra those definitions need for the
universal range and round-trip lemmas the issue asks for: how a digit sequence's value grows when a
digit is appended, what each exponent branch of `decimalRatio` is, and what `rne` returns for a
zero magnitude.

This module proves exactly that, with no new definitions:
- `digitsValue_snoc`: appending a digit multiplies by ten and adds it;
- `decimalRatio_nonneg` / `decimalRatio_neg`: the two branches of the ratio, stated once with the
  branch condition as an explicit premise (the identity cannot hold across `e = -1`, where the
  denominator is `10` and the next exponent's is `1`);
- `rne_zero_magnitude`, `rne_empty_digits`: a zero magnitude rounds to the spelling's signed zero;
- the two `zeroOf` values, and that `signedMagnitude` without a sign is the plain magnitude.

The rounding theorem below relates the model's single RNE operation directly to the exact signed
rational, with the finite-range premise made explicit. It is a model theorem, not a Rust parser or
implementation correspondence. Decimal expansion for `spellingOf` and the sharp overflow cut
`2^128 − 2^103` are separate proof obligations and are not claimed by this module.
-/

namespace NobleFoundation.Binary32.DecimalImport

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Error
open NobleFoundation.Binary32.Sequence
open NobleFoundation.Binary32.SequenceFinite
open NobleFoundation.Binary32.Subnormal

/-- Appending one decimal digit multiplies the value by ten and adds that digit. -/
theorem digitsValue_snoc (digits : List (Fin 10)) (digit : Fin 10) :
    digitsValue (digits ++ [digit]) = digitsValue digits * 10 + digit.val := by
  simp only [digitsValue, List.foldl_append, List.foldl_cons, List.foldl_nil]

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

/-- The signed-zero encodings of `zeroOf`. -/
theorem zeroOf_false : zeroOf false = 0 := rfl

theorem zeroOf_true : zeroOf true = 0x80000000 := rfl

/-- Without a sign, `signedMagnitude` is the plain magnitude. -/
theorem signedMagnitude_false (magnitude : Nat) :
    signedMagnitude false magnitude = BitVec.ofNat 32 magnitude := rfl

/-- A zero digit value rounds to the spelling's own signed zero, before any ratio is formed. -/
theorem rne_zero_magnitude (d : Decimal) (h : digitsValue d.digits = 0) :
    rne d = zeroOf d.negative := by
  unfold rne
  simp only [h, ite_true]

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

/-- The model's RNE result is the single positive `roundPositive` result with the spelling's sign.
The nonzero premise prevents the signed-zero branch from hiding the ratio. -/
theorem rne_nonzero_encoding (d : Decimal) (h : 0 < digitsValue d.digits) :
    rne d = signedMagnitude d.negative (roundPositive (decimalRatio d).1 (decimalRatio d).2) := by
  unfold rne
  have hne : digitsValue d.digits ≠ 0 := Nat.ne_of_gt h
  simp [hne]

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

end NobleFoundation.Binary32.DecimalImport
