import NobleM2.SizeChangeMatrixCycle

/-!
# Extracting the factor that produced a product entry (issue #101)

A product entry is a join over the factors that compose to it. To prove that composition preserves
step semantics, the entry's relation has to be traced back to a factor. These two lemmas do that:

- `join_eq_of_ne_unknown`: a join that is not `unknown` comes from one of its sides;
- `foldr_join_eq_of_ne_unknown`: the same for the fold over the `n` factors in `composeMatrix`.

They are the prerequisites for composition preservation: with them, an entry that is strict names the
intermediate measure whose composed factor is strict, and the inequalities chain as in
`compose_strict_steps`.
-/

namespace NobleM2.SizeChange

/-- A non-`unknown` join comes from one side. -/
theorem join_eq_of_ne_unknown {r a b : Rel4} (h : a.join b = r) (_hr : r ≠ .unknown) :
    a = r ∨ b = r := by
  cases a <;> cases b <;> simp_all [Rel4.join]

/-- A non-`unknown` fold over joins comes from an entry or from the accumulator. -/
theorem foldr_join_eq_of_ne_unknown {α : Type} (f : α → Rel4) :
    ∀ (l : List α) (acc r : Rel4),
      l.foldr (fun x a => (f x).join a) acc = r → r ≠ .unknown →
      (∃ x ∈ l, f x = r) ∨ acc = r
  | [], _, _, h, _ => Or.inr h
  | x :: rest, acc, r, h, hr => by
      rw [List.foldr_cons] at h
      rcases join_eq_of_ne_unknown h hr with hx | hrest
      · exact Or.inl ⟨x, by simp, hx⟩
      · rcases foldr_join_eq_of_ne_unknown f rest acc r hrest hr with ⟨y, hy, hyr⟩ | hacc
        · exact Or.inl ⟨y, by simp [hy], hyr⟩
        · exact Or.inr hacc

end NobleM2.SizeChange
