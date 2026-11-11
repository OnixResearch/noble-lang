import NobleM2.SizeChangeMatrixShortening

/-!
# A path is the product of its steps (issue #101)

`pathMatrix` composes a path directly; `matrixProduct` composes a list. This module proves they agree,
which is what lets the shortening lemma - stated about lists - speak about actual paths.

- `stepList`: the first `k` steps from position `t`, defined recursively so its successor equation
  is definitional;
- `matrixProduct_stepList`: `matrixProduct (stepList step k t) = pathMatrix k step t`.

With it, `matrixProduct_idempotent_shorten` applies to any list of steps, and the question the bound
asks - which step lists a checker must enumerate so that every path has a shorter representative -
becomes a question about lists of matrices rather than about a bespoke product.

That counting question is not answered here.
-/

namespace NobleM2.SizeChange

/-- The first `k` steps of a matrix sequence starting at `t`. -/
def stepList {n : Nat} : (Nat → MGraph n) → Nat → Nat → List (MGraph n)
  | _, 0, _ => []
  | step, k + 1, t => step t :: stepList step k (t + 1)

/-- A path is the product of its steps. -/
theorem matrixProduct_stepList {n : Nat} (step : Nat → MGraph n) :
    ∀ (k t : Nat), matrixProduct (stepList step k t) = pathMatrix k step t
  | 0, _ => rfl
  | k + 1, t => by
      show composeMatrix (step t) (matrixProduct (stepList step k (t + 1)))
        = composeMatrix (step t) (pathMatrix k step (t + 1))
      rw [matrixProduct_stepList step k (t + 1)]


/-- A one-matrix product is that matrix. -/
theorem matrixProduct_singleton {n : Nat} (g : MGraph n) : matrixProduct [g] = g := by
  show composeMatrix g idMatrix = g
  exact composeMatrix_id_right g

/-- **The three product lines agree**: a constant step list's product is the repeated power. -/
theorem matrixProduct_stepList_const {n : Nat} (g : MGraph n) (k : Nat) :
    matrixProduct (stepList (fun _ => g) k 0) = powerMatrix k g := by
  rw [matrixProduct_stepList]
  exact pathMatrix_const g k 0

end NobleM2.SizeChange
