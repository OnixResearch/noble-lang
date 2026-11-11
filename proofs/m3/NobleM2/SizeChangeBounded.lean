import NobleM2.SizeChangeDecEq

/-!
# Bounded search for a decreasing power (issue #101)

A checker cannot visit an infinite closure, so it visits the first `bound + 1` powers of a graph and
asks whether one of them decreases. This module defines that search and proves it **sound**: a true
result names a power within the bound that really decreases.

What is deliberately absent is **completeness** - that some bound (in particular one expressible as
`n * n` over `3 ^ n` graphs) always finds a decreasing power whenever one exists. That is the
pigeonhole step, and it needs counting lemmas the project's Mathlib-free closure does not provide:
core has `List.Nodup`, `List.Sublist` and `List.Sublist.length_le`, but no `Nodup.length_le` and no
`List.nodup_map_iff`, and `Fin m` has no `Fintype` to decide an existential over indices. Until that
is built, the search is sound but not guaranteed to terminate with the right answer.
-/

namespace NobleM2.SizeChange

/-- Search the first `bound + 1` powers of `g` for one that decreases measure `i`. -/
def decreasesWithin (n : Nat) (bound : Nat) (g : Graph n) (i : Fin n) : Bool :=
  (List.finRange (bound + 1)).any fun j => power (j + 1) g i == .decrease

/-- A true result names a decreasing power inside the bound. -/
theorem decreasesWithin_sound {n bound : Nat} {g : Graph n} {i : Fin n}
    (h : decreasesWithin n bound g i = true) :
    ∃ j, j ≤ bound ∧ power (j + 1) g i = .decrease := by
  obtain ⟨j, hdec⟩ := by simpa [decreasesWithin] using h
  exact ⟨j, by have := j.isLt; omega, hdec⟩

/-- The search is monotone in its bound. -/
theorem decreasesWithin_mono {n bound bound' : Nat} {g : Graph n} {i : Fin n}
    (hle : bound ≤ bound') (h : decreasesWithin n bound g i = true) :
    decreasesWithin n bound' g i = true := by
  obtain ⟨j, hj, hdec⟩ := decreasesWithin_sound h
  refine List.any_eq_true.mpr ⟨⟨j, by omega⟩, List.mem_finRange _, ?_⟩
  simpa using hdec

/-! ## Fixtures -/

/-- A decreasing graph is found at the first power within any bound. -/
def decreasingOne : Graph 1 := fun _ => .decrease

theorem decreasingOne_found :
    decreasesWithin 1 3 decreasingOne 0 = true := by
  decide

end NobleM2.SizeChange
