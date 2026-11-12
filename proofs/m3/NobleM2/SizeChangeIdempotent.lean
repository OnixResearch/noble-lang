import NobleM2.SizeChangeRepeat

/-!
# Idempotent call graphs absorb their powers (issue #101)

The tight size-change criterion is stated over idempotent members of the closure: a path that
composes to itself is the only kind that can appear on a cycle without changing the relation, so the
criterion only has to inspect those. This module formalises idempotence and proves the fact the
argument uses - an idempotent graph is its own every power - together with the cases that produce
idempotence.

The decisive fact for this algebra is that idempotence is free: every relation is transitive, so
`Rel.compose_self` holds for each of them and every graph is idempotent (`every_graph_idempotent`).
Consequently the idempotence clause of the tight criterion imposes nothing here, every positive power
of every graph is the graph itself (`every_graph_powers`), and the power search needs no bound - one
power decides (`SizeChangeBounded.decreasesWithin_complete_zero`).

What is **not** proved here, or anywhere in this project, is the other half of the tight criterion:
a condition tying the strict edge to a cycle for a run model that also honours `equal` and
non-increasing steps. `SizeChangeCriterion` shows why per-member strict edges do not suffice for the
run model this project defines, so that half cannot be obtained by counting alone.

-/

namespace NobleM2.SizeChange

/-- A graph that composes with itself to itself. -/
def Idempotent {n : Nat} (g : Graph n) : Prop := composeGraph g g = g

/-- The identity graph is idempotent. -/
theorem idGraph_idempotent {n : Nat} : Idempotent (idGraph : Graph n) := by
  simp [Idempotent]

/-- An idempotent graph equals each of its positive powers. -/
theorem idempotent_powers {n : Nat} {g : Graph n} (h : Idempotent g) :
    ∀ j, power (j + 1) g = g := by
  intro j
  induction j with
  | zero => simp [power]
  | succ j ih =>
      rw [power, ih]
      exact h

/-- Consequently its powers stay nodup-free of novelty: every power is the same graph. -/
theorem idempotent_power_two_eq {n : Nat} {g : Graph n} (h : Idempotent g) :
    power 2 g = g := by
  simpa using idempotent_powers h 1

/-- An idempotent member's powers are all itself, and all remain in the closure. -/
theorem idempotent_closure_powers {k n : Nat} {sites : Sites k n} {g : Graph n}
    (member : Closure sites g) (h : Idempotent g) (j : Nat) :
    Closure sites (power (j + 1) g) ∧ power (j + 1) g = g :=
  ⟨(idempotent_powers h j).symm ▸ closure_power_succ member j, idempotent_powers h j⟩

/-- Composing an idempotent graph with any power of itself is itself. -/
theorem idempotent_compose_power {n : Nat} {g : Graph n} (h : Idempotent g) (j : Nat) :
    composeGraph g (power (j + 1) g) = g := by
  rw [idempotent_powers h j]
  exact h

/-! ## The idempotence clause is vacuous in this algebra -/

/-- Each relation is transitive, hence idempotent under composition. -/
theorem Rel.compose_self (r : Rel) : r.compose r = r := by
  cases r <;> rfl

/-- Every graph is idempotent, so the tight criterion's idempotence clause imposes nothing here:
what remains of it is "every member of the closure carries a strict edge on a cycle", which is
not proved. -/
theorem every_graph_idempotent {n : Nat} (g : Graph n) : Idempotent g := by
  funext i
  exact Rel.compose_self (g i)

/-- Consequently every graph equals each of its positive powers. -/
theorem every_graph_powers {n : Nat} (g : Graph n) (j : Nat) : power (j + 1) g = g :=
  idempotent_powers (every_graph_idempotent g) j

end NobleM2.SizeChange
