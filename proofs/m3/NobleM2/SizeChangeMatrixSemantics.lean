import NobleM2.SizeChangeMatrix

/-!
# Semantics of matrix edges (issue #101)

A matrix graph relates the old value of one measure to the new value of another. This module gives
that relation its meaning: a step satisfies a matrix when every entry that is not `unknown` holds of
the two states.

On that semantics, the atom of the classical cycle argument is proved: a strict diagonal entry drops
its measure on every step, so a run whose steps satisfy such a matrix cannot be infinite. The cycle
argument then composes this along a path through non-increasing edges - that composition is the next
step and is not attempted here.

The vector lane's counterexample does not arise: `nonIncrease` records what `unknown` could not, so a
step that must not raise a measure can say so.
-/

namespace NobleM2.SizeChange

/-- A step from state `s` to state `t` satisfies the matrix `g`: every known entry holds. -/
def MatrixSatisfies {n : Nat} (g : MGraph n) (s t : Fin n → Nat) : Prop :=
  ∀ i j, g i j ≠ .unknown →
    ((g i j = .strict → t j < s i) ∧
      (g i j = .nonIncrease → t j ≤ s i) ∧
      (g i j = .same → t j = s i))

/-- A strict diagonal entry drops its measure. -/
theorem strict_diagonal_drops {n : Nat} {g : MGraph n} {s t : Fin n → Nat}
    (hs : MatrixSatisfies g s t) {i : Fin n} (h : g i i = .strict) : t i < s i := by
  have known : g i i ≠ .unknown := by rw [h]; exact Rel4.noConfusion
  exact (hs i i known).1 h

/-- A step satisfying a matrix with a strict diagonal cannot repeat indefinitely. -/
theorem matrix_strict_diagonal_terminates {n : Nat} {g : MGraph n} {i : Fin n}
    (h : g i i = .strict) (state : Nat → Fin n → Nat)
    (sat : ∀ t, MatrixSatisfies g (state t) (state (t + 1))) : False := by
  apply no_decreasing_chain (fun t => state t i)
  intro t
  exact strict_diagonal_drops (sat t) h

/-- A non-increasing diagonal entry never raises its measure. -/
theorem nonIncrease_diagonal_bounded {n : Nat} {g : MGraph n} {s t : Fin n → Nat}
    (hs : MatrixSatisfies g s t) {i : Fin n} (h : g i i = .nonIncrease) : t i ≤ s i := by
  have known : g i i ≠ .unknown := by rw [h]; exact Rel4.noConfusion
  exact (hs i i known).2.1 h

/-- An equal diagonal entry preserves its measure. -/
theorem same_diagonal_preserved {n : Nat} {g : MGraph n} {s t : Fin n → Nat}
    (hs : MatrixSatisfies g s t) {i : Fin n} (h : g i i = .same) : t i = s i := by
  have known : g i i ≠ .unknown := by rw [h]; exact Rel4.noConfusion
  exact (hs i i known).2.2 h

end NobleM2.SizeChange
