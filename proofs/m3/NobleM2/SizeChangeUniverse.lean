import NobleM2.SizeChangeEnumeration

/-!
# What enumerating the universe decides (issue #101)

With `graphsAux` complete, a property can be certified by iterating the list instead of quantifying
over all graphs. Applied to the criterion's decrease test this gives a sound rule - if every graph
in the universe decreases at measure `i`, then every family is accepted at `i` - and the module
proves that rule is **incomplete**: on one measure the universe already contains a pass-through
graph, so the rule refuses a family the criterion can still accept (for example the single-site
family whose graph *is* the decreasing graph).

That gap is the reason the `n * n` bound matters. Enumerating the universe is the cheap rule; the
criterion lives on the closure, which is a strict subset, and deciding it there needs the bound.

The pigeonhole argument that powers up to `n * n` suffice is still not proved.
-/

namespace NobleM2.SizeChange

/-- Every graph in the universe decreases at measure `i`. -/
def everyoneDecreases {n : Nat} (i : Fin n) : Bool :=
  (graphsAux n).all fun g => g i == .decrease

/-- The universe rule is sound: a true result really holds of every graph. -/
theorem everyoneDecreases_sound {n : Nat} {i : Fin n} (h : everyoneDecreases i = true) :
    ∀ g : Graph n, g i = .decrease := by
  intro g
  have listed := List.all_eq_true.mp h g (graphsAux_complete n g)
  simpa using listed

/-- A universe certificate covers every site family at that measure. -/
theorem universe_certificate {k n : Nat} {i : Fin n} (h : everyoneDecreases i = true) :
    (∀ g : Graph n, g i = .decrease) ∧
      certifiedAll (fun (_ : Fin k) => (fun _ => .decrease : Graph n)) = true := by
  refine ⟨everyoneDecreases_sound h, ?_⟩
  unfold certifiedAll
  exact List.any_eq_true.mpr ⟨i, List.mem_finRange i,
    List.all_eq_true.mpr (fun _ _ => by simp)⟩

/-- **Incompleteness of the universe rule**: on one measure it is already refuted. -/
theorem everyoneDecreases_one_refuted : everyoneDecreases (0 : Fin 1) = false := by
  cases h : everyoneDecreases (0 : Fin 1) with
  | false => rfl
  | true =>
      have equalGraph : (fun _ : Fin 1 => Rel.equal) 0 = Rel.decrease :=
        everyoneDecreases_sound h (fun _ : Fin 1 => Rel.equal)
      exact Rel.noConfusion equalGraph

/-! ## Fixtures -/

/-- The single-site family whose graph is decreasing is accepted, although the universe rule is not. -/
def decreasingSite : Sites 1 1 := fun _ _ => .decrease

theorem decreasingSite_certified : certifiedAll decreasingSite = true := by
  unfold certifiedAll
  exact List.any_eq_true.mpr ⟨0, List.mem_finRange 0,
    List.all_eq_true.mpr (fun _ _ => by simp [decreasingSite])⟩

end NobleM2.SizeChange
