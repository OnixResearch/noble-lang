import NobleM2.SizeChangeTightness

/-!
# Composition is associative (issue #101)

The closure of composed call graphs is a semigroup, and the bound argument for the criterion needs
that structure: a path of three sites must be composable in either grouping, and `idGraph` must be
its identity. This module proves both and promotes associativity to the closure itself, so a member
built by any parenthesisation of the same path is the same member.

The bound itself - that powers and composites up to `n * n` suffice - is not proved here; that is
the remaining open step, and it needs a finiteness or pigeonhole argument over `3 ^ n` graphs as
well as the semigroup laws shown here.
-/

namespace NobleM2.SizeChange

/-- Composition of call graphs is associative. -/
theorem composeGraph_assoc {n : Nat} (a b c : Graph n) :
    composeGraph (composeGraph a b) c = composeGraph a (composeGraph b c) := by
  funext i
  cases ha : a i <;> cases hb : b i <;> cases hc : c i <;>
    simp [composeGraph, Rel.compose, ha, hb, hc]

/-- The left identity law follows from the identity graph's definition. -/
theorem composeGraph_left_identity {n : Nat} (g : Graph n) : composeGraph idGraph g = g :=
  composeGraph_id_left g

/-- One copy of a graph is the graph itself. -/
theorem power_one {n : Nat} (g : Graph n) : power 1 g = g := by
  simp [power]

/-- Two copies compose the graph with itself. -/
theorem power_two {n : Nat} (g : Graph n) : power 2 g = composeGraph g g := by
  simp [power]

/-- Associativity lifts to the closure: any parenthesisation of the same path is a member. -/
theorem closure_compose_assoc {k n : Nat} {sites : Sites k n} {g h j : Graph n}
    (hg : Closure sites g) (hh : Closure sites h) (hj : Closure sites j) :
    Closure sites (composeGraph g (composeGraph h j)) := by
  rw [← composeGraph_assoc]
  exact Closure.compose (Closure.compose hg hh) hj

/-- A member composed with itself twice is in the closure under either grouping. -/
theorem closure_assoc_self {k n : Nat} {sites : Sites k n} {g : Graph n}
    (member : Closure sites g) :
    Closure sites (composeGraph (composeGraph g g) g) ∧
      Closure sites (composeGraph g (composeGraph g g)) := by
  have left : Closure sites (composeGraph (composeGraph g g) g) :=
    Closure.compose (Closure.compose member member) member
  have right : Closure sites (composeGraph g (composeGraph g g)) := by
    rw [← composeGraph_assoc]
    exact left
  exact ⟨left, right⟩

end NobleM2.SizeChange
