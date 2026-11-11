import NobleM2.SizeChangeMatrixSemantics

/-!
# Strictness through the matrix product (issue #101)

A composed matrix entry is `strict` exactly when one of the factors composing to it is strict, with
the other factor known (not `unknown`). This module proves that algebra, which is what turns "the
entry after a path is strict" into "some step on the path is strict" - the premise the cycle
argument needs before it can use the semantics of `strict_diagonal_drops`.

- `Rel4.join_eq_strict` and `Rel4.compose_eq_strict` characterise strictness;
- `foldr_join_strict` shows that a strict fold over joins comes from some entry or the accumulator;
- `composeMatrix_strict_exists` extracts the intermediate measure whose composed factor is strict.

The semantic composition - that a path with one strict step drops the measure between its endpoints -
is the next step and is not attempted here.
-/

namespace NobleM2.SizeChange

namespace Rel4

/-- A join is strict exactly when one side is. -/
theorem join_eq_strict (r s : Rel4) : r.join s = .strict ↔ r = .strict ∨ s = .strict := by
  cases r <;> cases s <;> simp [Rel4.join]

/-- A composition is strict exactly when one side is strict and the other is known. -/
theorem compose_eq_strict (r s : Rel4) :
    r.compose s = .strict ↔
      (r = .strict ∧ s ≠ .unknown) ∨ (s = .strict ∧ r ≠ .unknown) := by
  cases r <;> cases s <;> simp [Rel4.compose]

end Rel4

/-- A strict fold over joins comes from a strict entry or from the accumulator. -/
theorem foldr_join_strict {α : Type} (f : α → Rel4) :
    ∀ (l : List α) (acc : Rel4),
      l.foldr (fun x a => (f x).join a) acc = .strict → (∃ x ∈ l, f x = .strict) ∨ acc = .strict
  | [], acc, h => Or.inr h
  | x :: rest, acc, h => by
      rw [List.foldr_cons] at h
      rcases (Rel4.join_eq_strict (f x) (rest.foldr (fun y a => (f y).join a) acc)).mp h with hx | hrest
      · exact Or.inl ⟨x, by simp, hx⟩
      · rcases foldr_join_strict f rest acc hrest with ⟨y, hy, hys⟩ | hacc
        · exact Or.inl ⟨y, by simp [hy], hys⟩
        · exact Or.inr hacc

/-- A strict entry of the product comes from a measure whose composed factor is strict. -/
theorem composeMatrix_strict_exists {n : Nat} {g h : MGraph n} {i k : Fin n}
    (hs : composeMatrix g h i k = .strict) :
    ∃ j ∈ List.finRange n, (g i j).compose (h j k) = .strict := by
  have hfold : (List.finRange n).foldr (fun j acc => ((g i j).compose (h j k)).join acc) .unknown
      = .strict := by
    simpa [composeMatrix] using hs
  rcases foldr_join_strict (fun j => (g i j).compose (h j k)) (List.finRange n) .unknown hfold with
    ⟨j, hj, hjs⟩ | hacc
  · exact ⟨j, hj, hjs⟩
  · exact absurd hacc (by simp)

end NobleM2.SizeChange
