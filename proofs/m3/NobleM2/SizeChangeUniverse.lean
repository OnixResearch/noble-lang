import NobleM2.SizeChangeEnumeration

/-!
# What enumerating the universe decides (issue #101)

With `graphsAux` complete, a property can be certified by iterating the list instead of quantifying
over all graphs. Applied to the criterion's decrease test this gives a rule: if every graph in the
universe decreases at measure `i`, then every family is accepted at `i` (`everyoneDecreases_sound`).

The rule is empty, and this module proves it: the universe contains the all-`equal` graph, which
decreases nothing, so `everyoneDecreases i` is `false` for every measure and every domain size
(`everyoneDecreases_false`). The `n * n` counting bound does not rescue it, and is not what the
criterion's search needs either: every relation is transitive, so every graph equals each of its
positive powers (`SizeChangeIdempotent.every_graph_powers`) and one power decides. What remains open
is the criterion's soundness for a run model that honours non-strict steps, not its search space.
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

/-- **The universe rule is never usable**: the all-`equal` graph belongs to the universe, so no
measure decreases at every graph and the rule is `false` everywhere. -/
theorem everyoneDecreases_false {n : Nat} (i : Fin n) : everyoneDecreases i = false := by
  cases h : everyoneDecreases i with
  | false => rfl
  | true =>
      have equalGraph : (fun _ : Fin n => Rel.equal) i = Rel.decrease :=
        everyoneDecreases_sound h (fun _ : Fin n => Rel.equal)
      exact Rel.noConfusion equalGraph

/-! ## Fixtures -/

/-- The single-site family whose graph is decreasing is accepted, although the universe rule is not. -/
def decreasingSite : Sites 1 1 := fun _ _ => .decrease

theorem decreasingSite_certified : certifiedAll decreasingSite = true := by
  unfold certifiedAll
  exact List.any_eq_true.mpr ⟨0, List.mem_finRange 0,
    List.all_eq_true.mpr (fun _ _ => by simp [decreasingSite])⟩

end NobleM2.SizeChange
