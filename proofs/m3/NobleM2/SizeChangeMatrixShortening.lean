import NobleM2.SizeChangeMatrixPeriod

/-!
# Shortening composite paths (issue #101)

The tight bound's mechanism is that long composites are dominated by shorter ones. The first concrete
instance is proved here: a **repeated idempotent block can be dropped from a path without changing
its product**.

- `matrixProduct`: the product of a list of matrices;
- `matrixProduct_append`: a product over a concatenation splits into the two halves;
- `matrixProduct_idempotent_shorten`: `pre ++ [x, x] ++ post` and `pre ++ [x] ++ post` have the same
  product whenever `x ∘ x = x`;
- `matrixProduct_all_same`: a non-empty path of a single idempotent matrix has that matrix as product.

This is what "dominated by a shorter composite" means at the level of one path. The remaining step
to the $n^2$ bound is which composites a checker must enumerate so that every path has such a
representative — a statement about the closure, not about a single path, and not attempted here.
-/

namespace NobleM2.SizeChange

/-- The product of a list of matrices, in path order. -/
def matrixProduct {n : Nat} : List (MGraph n) → MGraph n
  | [] => idMatrix
  | g :: gs => composeMatrix g (matrixProduct gs)

/-- A product over a concatenation splits into the two halves. -/
theorem matrixProduct_append {n : Nat} (l₁ l₂ : List (MGraph n)) :
    matrixProduct (l₁ ++ l₂) = composeMatrix (matrixProduct l₁) (matrixProduct l₂) := by
  induction l₁ with
  | nil =>
      show matrixProduct l₂ = composeMatrix idMatrix (matrixProduct l₂)
      rw [composeMatrix_id_left]
  | cons g gs ih =>
      show composeMatrix g (matrixProduct (gs ++ l₂))
        = composeMatrix (composeMatrix g (matrixProduct gs)) (matrixProduct l₂)
      rw [ih, composeMatrix_assoc]

/-- **Shortening a path at an idempotent block.** -/
theorem matrixProduct_idempotent_shorten {n : Nat} {x : MGraph n}
    (h : composeMatrix x x = x) (pre post : List (MGraph n)) :
    matrixProduct (pre ++ x :: x :: post) = matrixProduct (pre ++ x :: post) := by
  rw [matrixProduct_append, matrixProduct_append]
  congr 1
  show composeMatrix x (composeMatrix x (matrixProduct post))
    = composeMatrix x (matrixProduct post)
  rw [← composeMatrix_assoc, h]

/-- A non-empty path of a single idempotent matrix has that matrix as its product. -/
theorem matrixProduct_all_same {n : Nat} {x : MGraph n} (h : composeMatrix x x = x) :
    ∀ (l : List (MGraph n)), (∀ g ∈ l, g = x) → l ≠ [] → matrixProduct l = x
  | [], _, hne => absurd rfl hne
  | g :: gs, hall, _ => by
      have hg : g = x := hall g (by simp)
      rw [hg]
      cases gs with
      | nil =>
          show composeMatrix x idMatrix = x
          exact composeMatrix_id_right x
      | cons g' gs' =>
          show composeMatrix x (matrixProduct (g' :: gs')) = x
          rw [matrixProduct_all_same h (g' :: gs') (fun y hy => hall y (by simp [hy])) (by simp)]
          exact h

end NobleM2.SizeChange
