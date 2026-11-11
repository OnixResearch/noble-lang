import NobleM2.SizeChangeMatrixPath

/-!
# Closing the path into a cycle (issue #101)

`compose_strict_steps` drops a measure across two steps when the product's entry is strict. Composing
a graph with itself closes that path into a cycle: if `g ∘ g` is strict on the diagonal at `i`, then
every pair of consecutive steps drops measure `i`, so the states at even times form a strictly
decreasing chain and no infinite run exists.

This is the first cycle form the lane can state - period two, one graph, diagonal entry. The general
criterion (any idempotent closure member, any cycle in an SCC) uses the same composition law with
more bookkeeping and is not attempted here.
-/

namespace NobleM2.SizeChange

/-- A two-step cycle with a strict diagonal drops the measure over each pair of steps. -/
theorem cycle_pair_drops {n : Nat} {g : MGraph n} {i : Fin n}
    (hprod : composeMatrix g g i i = .strict) {s t u : Fin n → Nat}
    (hg : MatrixSatisfies g s t) (hg' : MatrixSatisfies g t u) : u i < s i :=
  compose_strict_steps hprod hg hg'

/-- **A strict two-step cycle terminates**: no infinite run repeats a graph with a strict diagonal in
its square. -/
theorem cycle_terminates {n : Nat} {g : MGraph n} {i : Fin n}
    (hprod : composeMatrix g g i i = .strict)
    (state : Nat → Fin n → Nat) (sat : ∀ t, MatrixSatisfies g (state t) (state (t + 1))) : False := by
  apply no_decreasing_chain (fun t => state (2 * t) i)
  intro t
  have hstep := cycle_pair_drops hprod (sat (2 * t)) (sat (2 * t + 1))
  have harith : 2 * t + 1 + 1 = 2 * (t + 1) := by omega
  rw [harith] at hstep
  exact hstep

end NobleM2.SizeChange
