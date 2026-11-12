import NobleM2.SizeChangeDecEq

/-!
# Bounded search for a decreasing power (issue #101)

A checker cannot visit an infinite closure, so it visits the first `bound + 1` powers of a graph and
asks whether one of them decreases. This module defines that search and proves it **sound**: a true
result names a power within the bound that really decreases.

Completeness needs no counting argument in this algebra: the first power of a graph is the graph
itself, because the identity graph is a unit and every relation is transitive. So `decreasesWithin n 0
g i` already decides whether `g` decreases measure `i` (`decreasesWithin_complete_zero`), and every
larger bound inherits the verdict by `decreasesWithin_mono`. No `n * n` bound is needed here; the
pigeonhole lemmas in `SizeChangeCounting` and `SizeChangeRepeat` concern the enumeration of graphs,
not the power search.
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

/-- The first power of a graph is the graph: the identity graph is a unit and every relation is
transitive. -/
theorem power_one_apply {n : Nat} (g : Graph n) (i : Fin n) : power 1 g i = g i := by
  cases h : g i <;> simp [power, composeGraph, idGraph, h, Rel.compose]

/-- At bound zero the search already reads the graph itself. -/
theorem decreasesWithin_zero {n : Nat} (g : Graph n) (i : Fin n) :
    decreasesWithin n 0 g i = (g i == .decrease) := by
  have hlist : List.finRange (0 + 1) = [(0 : Fin 1)] := rfl
  show (List.finRange (0 + 1)).any (fun j => power (j + 1) g i == .decrease) = _
  rw [hlist]
  simp only [List.any_cons, List.any_nil, Bool.or_false, Fin.val_zero, Nat.zero_add]
  rw [power_one_apply]

/-- **Completeness at the first power**: the search decides without any counting bound, so a
decreasing power is found exactly when the graph itself decreases that measure. -/
theorem decreasesWithin_complete_zero {n : Nat} (g : Graph n) (i : Fin n) :
    decreasesWithin n 0 g i = true ↔ g i = .decrease := by
  rw [decreasesWithin_zero, beq_iff_eq]

/-! ## Fixtures -/

/-- A decreasing graph is found at the first power within any bound. -/
def decreasingOne : Graph 1 := fun _ => .decrease

theorem decreasingOne_found :
    decreasesWithin 1 3 decreasingOne 0 = true := by
  decide

end NobleM2.SizeChange
