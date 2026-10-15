import NobleFoundation.Repr

/-!
# Source literal acceptance is not wrapping (issue #15, Phase 1)

The MC1 helper `NobleContracts.i64 n = .i64 (BitVec.ofInt 64 n)` wraps every
integer.  The Noble frontend does **not**: `crates/noble-contracts/src/syntax/
integer.rs` (`Accumulator::digit`) accumulates decimal digits with checked
`i64` multiplication and addition (subtraction for a leading `-`) and rejects
with `integer literal is outside I64` on the first overflow.

* `acceptLiteral` is the *specification*: accept exactly the literals whose
  mathematical value lies in `I64Range`.
* `accumulate` is a line-by-line *transcription* of the Rust checked
  accumulator into Lean (not an extraction).  `accumulate_eq_acceptLiteral`
  proves that the transcribed algorithm meets the specification for every digit
  string.  That the Rust bytes behave like the transcription is **not** proved
  here (the Aeneas extraction in `proofs/mc1/NobleContractImpl/Funs.lean` has no
  refinement theorem for this function); it is covered by tests only.
* Token classification (leading zeros, a bare `-`, non-digits) happens before
  the accumulator in `source/lexer/tokens.rs` and is outside this model.
-/

namespace NobleFoundation

/-- Value of a decimal digit string, most significant digit first. -/
def decimal (ds : List (Fin 10)) : ℕ := ds.foldl (fun a d => a * 10 + d.val) 0

/-- The signed mathematical value of a literal `[-]digits`. -/
def literalValue (negative : Bool) (ds : List (Fin 10)) : ℤ :=
  if negative then -(decimal ds : ℤ) else (decimal ds : ℤ)

/-- Frontend specification: accept exactly the in-range literals, unchanged. -/
def acceptLiteral (negative : Bool) (ds : List (Fin 10)) : Option ℤ :=
  if I64Range (literalValue negative ds) then some (literalValue negative ds) else none

/-- A checked I64 result (`i64::checked_*` returning `Some`/`None`). -/
def checked (v : ℤ) : Option ℤ := if I64Range v then some v else none

/-- One `Accumulator::digit` call: `checked_mul(10)` then `checked_sub(d)` for a
negative literal or `checked_add(d)` otherwise. -/
def accStep (negative : Bool) (acc : Option ℤ) (d : Fin 10) : Option ℤ :=
  acc.bind fun v => (checked (v * 10)).bind fun w =>
    checked (if negative then w - d.val else w + d.val)

/-- The transcribed accumulator, starting from `value: 0`. -/
def accumulate (negative : Bool) (ds : List (Fin 10)) : Option ℤ :=
  ds.foldl (accStep negative) (some 0)

/-- Unchecked accumulation in `ℤ`. -/
def extend (negative : Bool) (v : ℤ) (ds : List (Fin 10)) : ℤ :=
  ds.foldl (fun a d => if negative then a * 10 - d.val else a * 10 + d.val) v

private theorem foldl_none (negative : Bool) (ds : List (Fin 10)) :
    ds.foldl (accStep negative) none = none := by
  induction ds with
  | nil => rfl
  | cons d ds ih => simpa [List.foldl, accStep] using ih

private theorem extend_mono_pos (v : ℤ) (hv : 0 ≤ v) (ds : List (Fin 10)) :
    v ≤ extend false v ds := by
  induction ds generalizing v with
  | nil => simp [extend]
  | cons d ds ih =>
      simp only [extend, List.foldl_cons, Bool.false_eq_true, if_false] at *
      have := ih (v * 10 + d.val) (by omega)
      omega

private theorem extend_mono_neg (v : ℤ) (hv : v ≤ 0) (ds : List (Fin 10)) :
    extend true v ds ≤ v := by
  induction ds generalizing v with
  | nil => simp [extend]
  | cons d ds ih =>
      simp only [extend, List.foldl_cons, if_true] at *
      have := ih (v * 10 - d.val) (by omega)
      omega

/-- The accumulator is exact: overflow at any intermediate step implies the
final value is outside the range, because magnitudes only grow. -/
private theorem foldl_checked (negative : Bool) (ds : List (Fin 10)) (v : ℤ)
    (hr : I64Range v) (hs : if negative then v ≤ 0 else 0 ≤ v) :
    ds.foldl (accStep negative) (some v) = checked (extend negative v ds) := by
  induction ds generalizing v with
  | nil => simp [extend, checked, hr]
  | cons d ds ih =>
      have hd := d.isLt
      simp only [List.foldl_cons, accStep, Option.bind_some]
      unfold checked
      cases negative with
      | false =>
          simp only [Bool.false_eq_true, if_false] at hs ⊢
          have hmono := extend_mono_pos (v * 10 + d.val) (by omega) ds
          by_cases h1 : I64Range (v * 10)
          · simp only [h1, if_true, Option.bind_some]
            by_cases h2 : I64Range (v * 10 + d.val)
            · simp only [h2, if_true]
              rw [ih _ h2 (by simp only [Bool.false_eq_true, if_false]; omega)]; rfl
            · simp only [h2, if_false, foldl_none]
              have : ¬ I64Range (extend false v (d :: ds)) := by
                unfold I64Range at *; simp only [extend, List.foldl_cons,
                  Bool.false_eq_true, if_false] at *; omega
              simp [this]
          · simp only [h1, if_false, Option.bind_none, foldl_none]
            have : ¬ I64Range (extend false v (d :: ds)) := by
              unfold I64Range at *; simp only [extend, List.foldl_cons,
                Bool.false_eq_true, if_false] at *; omega
            simp [this]
      | true =>
          simp only [if_true] at hs ⊢
          have hmono := extend_mono_neg (v * 10 - d.val) (by omega) ds
          by_cases h1 : I64Range (v * 10)
          · simp only [h1, if_true, Option.bind_some]
            by_cases h2 : I64Range (v * 10 - d.val)
            · simp only [h2, if_true]
              rw [ih _ h2 (by simp only [if_true]; omega)]; rfl
            · simp only [h2, if_false, foldl_none]
              have : ¬ I64Range (extend true v (d :: ds)) := by
                unfold I64Range at *; simp only [extend, List.foldl_cons,
                  if_true] at *; omega
              simp [this]
          · simp only [h1, if_false, Option.bind_none, foldl_none]
            have : ¬ I64Range (extend true v (d :: ds)) := by
              unfold I64Range at *; simp only [extend, List.foldl_cons,
                if_true] at *; omega
            simp [this]

private theorem extend_zero (negative : Bool) (ds : List (Fin 10)) :
    extend negative 0 ds = literalValue negative ds := by
  have key : ∀ (a : ℕ), extend negative (if negative then -(a : ℤ) else a) ds =
      if negative then -((ds.foldl (fun a d => a * 10 + d.val) a : ℕ) : ℤ)
      else ((ds.foldl (fun a d => a * 10 + d.val) a : ℕ) : ℤ) := by
    induction ds with
    | nil => intro a; simp [extend]
    | cons d ds ih =>
        intro a
        have := ih (a * 10 + d.val)
        cases negative <;> simp only [extend, List.foldl_cons, Bool.false_eq_true,
          if_true, if_false] at this ⊢ <;> push_cast at this ⊢
        · exact this
        · rw [← this]; congr 1; ring
  have := key 0
  cases negative <;> simpa [literalValue, decimal] using this

/-- **The transcribed checked accumulator meets the acceptance specification
for every digit string.** -/
theorem accumulate_eq_acceptLiteral (negative : Bool) (ds : List (Fin 10)) :
    accumulate negative ds = acceptLiteral negative ds := by
  unfold accumulate
  rw [foldl_checked negative ds 0 (by unfold I64Range; norm_num)
    (by cases negative <;> simp), extend_zero]
  rfl

/-- Accepted literals are never wrapped: the MC1 value built from an accepted
literal decodes to exactly the source value. -/
theorem accepted_literal_exact {negative ds n} (h : acceptLiteral negative ds = some n) :
    I64Range n ∧ n = literalValue negative ds ∧ (BitVec.ofInt 64 n).toInt = n := by
  unfold acceptLiteral at h
  split_ifs at h with hr
  cases h
  exact ⟨hr, rfl, toInt_ofInt_of_range hr⟩

theorem acceptLiteral_eq_none_iff (negative : Bool) (ds : List (Fin 10)) :
    acceptLiteral negative ds = none ↔ ¬ I64Range (literalValue negative ds) := by
  unfold acceptLiteral; split_ifs with h <;> simp [h]

/-- `BitVec.ofInt` alone would *silently broaden* the accepted literals: two
distinct source integers, one of them rejected by the frontend, collapse to the
same I64 value. -/
theorem ofInt_collapses_out_of_range :
    BitVec.ofInt 64 (2 ^ 63) = BitVec.ofInt 64 (-2 ^ 63) ∧
    (2 ^ 63 : ℤ) ≠ -2 ^ 63 ∧ ¬ I64Range (2 ^ 63) ∧ I64Range (-2 ^ 63) := by
  refine ⟨by decide, by omega, by unfold I64Range; norm_num,
    by unfold I64Range; norm_num⟩

/-- Accepted source literals are exactly the integers that `BitVec.ofInt`
represents without wrapping. -/
theorem literal_range_iff_no_wrap (n : ℤ) :
    I64Range n ↔ (BitVec.ofInt 64 n).toInt = n := (toInt_ofInt_eq_iff n).symm

/-- Digits of `9223372036854775808 = 2^63`. -/
def digitsTwoPow63 : List (Fin 10) := [9,2,2,3,3,7,2,0,3,6,8,5,4,7,7,5,8,0,8]
/-- Digits of `9223372036854775807 = 2^63 - 1`. -/
def digitsMax : List (Fin 10) := [9,2,2,3,3,7,2,0,3,6,8,5,4,7,7,5,8,0,7]

/-- Regression: `9223372036854775808` is rejected, `-9223372036854775808` and
`9223372036854775807` are accepted unchanged, `-0` is zero. -/
theorem literal_boundary_regressions :
    accumulate false digitsTwoPow63 = none ∧
    accumulate true digitsTwoPow63 = some (-9223372036854775808) ∧
    accumulate false digitsMax = some 9223372036854775807 ∧
    accumulate true digitsMax = some (-9223372036854775807) ∧
    accumulate true [0] = some 0 ∧
    accumulate true [9,2,2,3,3,7,2,0,3,6,8,5,4,7,7,5,8,0,9] = none := by
  refine ⟨by decide, by decide, by decide, by decide, by decide, by decide⟩

end NobleFoundation
