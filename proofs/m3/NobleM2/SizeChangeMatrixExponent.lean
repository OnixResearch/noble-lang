import NobleM2.SizeChangeMatrixJoinFold
import NobleM2.SizeChangeMatrixPreserve

/-!
# Exponent arithmetic for matrix powers (issue #101)

With associativity and both units, `composeMatrix` is a monoid, so powers add: the product over
`a + b` steps splits into the two halves. The induction is on the left exponent, which avoids
needing powers to commute with the graph itself — both sides prepend.

This is the last algebraic step before the idempotent power.
-/

namespace NobleM2.SizeChange


/-- **Exponent law**: a product over `a + b` steps splits into the halves. -/
theorem powerMatrix_add {n : Nat} (g : MGraph n) :
    ∀ (a b : Nat), powerMatrix (a + b) g = composeMatrix (powerMatrix a g) (powerMatrix b g)
  | 0, b => by
      rw [Nat.zero_add]
      show powerMatrix b g = composeMatrix idMatrix (powerMatrix b g)
      rw [composeMatrix_id_left]
  | a + 1, b => by
      have hsucc : (a + 1) + b = (a + b) + 1 := by omega
      rw [hsucc]
      show composeMatrix g (powerMatrix (a + b) g)
        = composeMatrix (composeMatrix g (powerMatrix a g)) (powerMatrix b g)
      rw [powerMatrix_add g a b]
      exact (composeMatrix_assoc g (powerMatrix a g) (powerMatrix b g)).symm

end NobleM2.SizeChange
