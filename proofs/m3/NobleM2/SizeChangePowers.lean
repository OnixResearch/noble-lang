import NobleM2.SizeChangeChecker

/-!
# Powers of call graphs (issue #101)

The size-change criterion is stated over the closure of composed graphs. A path that stays at one
site composes that site's graph with itself, so the closure contains every positive power of every
site graph. This module states the identity graph, proves powers stay in the closure and proves a
decrease survives them, which is what lets `closure_decreases` see repeated paths without unfolding
the closure each time.

What is *not* proved here is the bound that makes the criterion decidable in practice - that powers
up to `n * n` suffice (the Lee-Jones-Ben-Amram bound). Without it, the criterion is stated over an
infinite closure and the decision procedure only sees what it enumerates. That bound, and the
general soundness theorem, remain open.
-/

namespace NobleM2.SizeChange

namespace Rel

/-- Composing with `equal` on the right passes the relation through. -/
theorem compose_equal_right (x : Rel) : x.compose .equal = x := by
  cases x <;> rfl

end Rel

/-- The identity graph: every measure passes through unchanged. -/
def idGraph {n : Nat} : Graph n := fun _ => .equal

/-- Composing with the identity graph changes nothing. -/
@[simp] theorem composeGraph_id {n : Nat} (g : Graph n) : composeGraph g idGraph = g := by
  funext i
  exact Rel.compose_equal_right (g i)

/-- The identity graph composes away on the left too. -/
@[simp] theorem composeGraph_id_left {n : Nat} (g : Graph n) : composeGraph idGraph g = g := by
  funext i
  cases h : g i <;> simp [composeGraph, idGraph, Rel.compose, h]

/-- `j + 1` copies of a graph: one step, then `j` more. -/
def power {n : Nat} : Nat → Graph n → Graph n
  | 0, _ => idGraph
  | j + 1, g => composeGraph g (power j g)

/-- Every positive power of a closure member is itself in the closure. -/
theorem closure_power_succ {k n : Nat} {sites : Sites k n} {g : Graph n}
    (member : Closure sites g) : ∀ j, Closure sites (power (j + 1) g) := by
  intro j
  induction j with
  | zero => simpa [power] using member
  | succ j ih => exact Closure.compose member ih

/-- A decrease at a measure survives every power. -/
theorem power_decreases {n : Nat} {g : Graph n} {i : Fin n} (hi : g i = .decrease) :
    ∀ j, power (j + 1) g i = .decrease := by
  intro j
  induction j with
  | zero => simpa [power] using hi
  | succ j _ => exact composeGraph_decrease_left hi

/-- A certified family's shared measure survives every power of every site. -/
theorem powers_of_certified_decrease {k n : Nat} {sites : Sites k n}
    (h : certifiedAll sites = true) (s : Fin k) :
    ∀ j, ∃ i, power (j + 1) (sites s) i = .decrease := by
  obtain ⟨i, hi⟩ := certifiedAll_sound h
  intro j
  exact ⟨i, power_decreases (hi s) j⟩

/-! ## Fixtures -/

/-- A decreasing graph stays decreasing at every power. -/
def decreasingGraph : Graph 2 := fun i => if i = 0 then .decrease else .unknown

theorem decreasingGraph_powers : ∀ j, power (j + 1) decreasingGraph 0 = .decrease :=
  power_decreases (by simp [decreasingGraph])

end NobleM2.SizeChange
