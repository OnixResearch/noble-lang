import NobleFoundation.DecimalImport

/-!
# Decimal spelling algebra for the import specification (issue #40)

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

Not claimed here: the round-trip lemma itself (`rne (spellingOf b) = b` for a finite binary32 `b`)
needs a decimal spelling of `m · 5^149`, and the range lemmas at `2^128 − 2^103` need the scaled
branch of `roundPositive`. Both stay open; this module is the shared algebra they will consume.
-/

namespace NobleFoundation.Binary32.DecimalImport

open NobleFoundation.Binary32

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

end NobleFoundation.Binary32.DecimalImport
