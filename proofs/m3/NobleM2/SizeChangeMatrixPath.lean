import NobleM2.SizeChangeMatrixStrict

/-!
# Composing two steps preserves semantics (issue #101)

Given a step satisfying `g` and a following step satisfying `h`, a strict entry of the product at
`(i, k)` drops the measure `k` below `i` across both steps. With `unknown` absorbing in the
composition table, the extraction lemma names a measure `j` whose composed factor is strict, which
means one factor is strict and the other known - and in either case the inequalities chain.

Closing the path into a cycle with the identity matrix, and the SCC criterion over it, are the next
steps and are not attempted here.
-/

namespace NobleM2.SizeChange

/-- A strict entry relates the two states strictly. -/
theorem MatrixSatisfies.strict {n : Nat} {g : MGraph n} {s t : Fin n → Nat}
    (hs : MatrixSatisfies g s t) {i j : Fin n} (h : g i j = .strict) : t j < s i := by
  have known : g i j ≠ .unknown := by rw [h]; exact Rel4.noConfusion
  exact (hs i j known).1 h

/-- A non-increasing entry bounds the two states. -/
theorem MatrixSatisfies.nonIncrease {n : Nat} {g : MGraph n} {s t : Fin n → Nat}
    (hs : MatrixSatisfies g s t) {i j : Fin n} (h : g i j = .nonIncrease) : t j ≤ s i := by
  have known : g i j ≠ .unknown := by rw [h]; exact Rel4.noConfusion
  exact (hs i j known).2.1 h

/-- An equal entry preserves the measure. -/
theorem MatrixSatisfies.sameEq {n : Nat} {g : MGraph n} {s t : Fin n → Nat}
    (hs : MatrixSatisfies g s t) {i j : Fin n} (h : g i j = .same) : t j = s i := by
  have known : g i j ≠ .unknown := by rw [h]; exact Rel4.noConfusion
  exact (hs i j known).2.2 h

/-- **Two steps compose**: a strict product entry drops the measure across them. -/
theorem compose_strict_steps {n : Nat} {g h : MGraph n} {i k : Fin n}
    (hprod : composeMatrix g h i k = .strict) {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hh : MatrixSatisfies h t u) : u k < s i := by
  obtain ⟨j, _, hj⟩ := composeMatrix_strict_exists hprod
  have hsplit := (Rel4.compose_eq_strict (g i j) (h j k)).mp hj
  clear hj hprod
  rcases hsplit with ⟨hgstrict, hknown⟩ | ⟨hhstrict, hknown⟩
  · have hlt : t j < s i := MatrixSatisfies.strict hg hgstrict
    cases hcase : h j k
    · exact Nat.lt_trans (MatrixSatisfies.strict hh hcase) hlt
    · exact Nat.lt_of_le_of_lt (MatrixSatisfies.nonIncrease hh hcase) hlt
    · rw [MatrixSatisfies.sameEq hh hcase]; exact hlt
    · exact absurd hcase hknown
  · have hlt : u k < t j := MatrixSatisfies.strict hh hhstrict
    cases hcase : g i j
    · exact Nat.lt_trans hlt (MatrixSatisfies.strict hg hcase)
    · exact Nat.lt_of_lt_of_le hlt (MatrixSatisfies.nonIncrease hg hcase)
    · rw [MatrixSatisfies.sameEq hg hcase] at hlt; exact hlt
    · exact absurd hcase hknown

end NobleM2.SizeChange
