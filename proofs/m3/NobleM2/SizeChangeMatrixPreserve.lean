import NobleM2.SizeChangeMatrixExtract

/-!
# Composition preserves step semantics (issue #101)

If a step satisfies `g` and the next satisfies `h`, then the step pair satisfies the product
`composeMatrix g h`: each entry of the product is the join over the factors that compose to it, and
each possible relation of an entry is realised by a factor whose two halves chain.

- `compose_eq_nonIncrease` and `compose_eq_same` classify the factor pairs for the two non-strict
  relations;
- `compose_nonIncrease_steps` and `compose_same_steps` chain the inequalities for them, as
  `compose_strict_steps` does for strict;
- `matrixSatisfies_compose` assembles the three into preservation.

Iterating preservation gives period-`k` cycles; the SCC criterion is the next step.
-/

namespace NobleM2.SizeChange

/-- A composition is non-increasing exactly for these factor pairs. -/
theorem compose_eq_nonIncrease (r s : Rel4) :
    r.compose s = .nonIncrease ↔
      (r = .nonIncrease ∧ s = .nonIncrease) ∨
        (r = .nonIncrease ∧ s = .same) ∨ (r = .same ∧ s = .nonIncrease) := by
  cases r <;> cases s <;> decide

/-- A composition is the identity relation exactly for `same ∘ same`. -/
theorem compose_eq_same (r s : Rel4) : r.compose s = .same ↔ r = .same ∧ s = .same := by
  cases r <;> cases s <;> decide

/-- A non-increasing product entry bounds the measure across the two steps. -/
theorem compose_nonIncrease_steps {n : Nat} {g h : MGraph n} {i k : Fin n}
    (hprod : composeMatrix g h i k = .nonIncrease) {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hh : MatrixSatisfies h t u) : u k ≤ s i := by
  have hfold : (List.finRange n).foldr (fun j acc => ((g i j).compose (h j k)).join acc) .unknown
      = .nonIncrease := by simpa [composeMatrix] using hprod
  rcases foldr_join_eq_of_ne_unknown (fun j => (g i j).compose (h j k)) (List.finRange n)
      .unknown .nonIncrease hfold (by simp) with ⟨j', _, hj'⟩ | hacc
  · rcases (compose_eq_nonIncrease (g i j') (h j' k)).mp hj' with ⟨hgN, hhN⟩ | hrest
    · exact Nat.le_trans (MatrixSatisfies.nonIncrease hh hhN) (MatrixSatisfies.nonIncrease hg hgN)
    · rcases hrest with ⟨hgN, hhS⟩ | ⟨hgS, hhN⟩
      · have hu : u k = t j' := MatrixSatisfies.sameEq hh hhS
        rw [hu]
        exact MatrixSatisfies.nonIncrease hg hgN
      · have hs : s i = t j' := (MatrixSatisfies.sameEq hg hgS).symm
        rw [hs]
        exact MatrixSatisfies.nonIncrease hh hhN
  · exact absurd hacc (by simp)

/-- An identity product entry preserves the measure across the two steps. -/
theorem compose_same_steps {n : Nat} {g h : MGraph n} {i k : Fin n}
    (hprod : composeMatrix g h i k = .same) {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hh : MatrixSatisfies h t u) : u k = s i := by
  have hfold : (List.finRange n).foldr (fun j acc => ((g i j).compose (h j k)).join acc) .unknown
      = .same := by simpa [composeMatrix] using hprod
  rcases foldr_join_eq_of_ne_unknown (fun j => (g i j).compose (h j k)) (List.finRange n)
      .unknown .same hfold (by simp) with ⟨j, _, hj⟩ | hacc
  · rcases (compose_eq_same (g i j) (h j k)).mp hj with ⟨hgS, hhS⟩
    rw [MatrixSatisfies.sameEq hh hhS, MatrixSatisfies.sameEq hg hgS]
  · exact absurd hacc (by simp)

/-- **Composition preserves semantics.** -/
theorem matrixSatisfies_compose {n : Nat} {g h : MGraph n} {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hh : MatrixSatisfies h t u) :
    MatrixSatisfies (composeMatrix g h) s u := by
  intro i k _
  exact ⟨fun hst => compose_strict_steps hst hg hh,
    fun hni => compose_nonIncrease_steps hni hg hh,
    fun hsm => compose_same_steps hsm hg hh⟩

end NobleM2.SizeChange
