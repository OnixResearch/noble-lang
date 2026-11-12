import NobleM2.SizeChangeMatrixPigeonhole

/-!
# Decidable equality for matrices (issue #101)

A checker that scans composites, or states a nodup list of them, needs to decide equality of
matrices. The vector lane's `pointwiseEq` decides equality of `Fin n → Rel`, but it is not reused
here: `MGraph n` is a function into rows, and a row is a function into `Rel4`. So the generic
`funEq` procedure - parameterised by a decision procedure for the codomain - is instantiated twice,
with `rel4Eq` for rows and with `rowEq` for matrices, and each instantiation is proved exact.

With `DecidableEq (MGraph n)` the enumeration is searchable and `Nodup` becomes usable over lists of
composites, which is what `matrix_composites_bounded` consumes.
-/

namespace NobleM2.SizeChange

/-- Compare two functions on `Fin n` into `α`, given a decision procedure for `α`. -/
def funEq {α : Type} (eqα : α → α → Bool) : (n : Nat) → (Fin n → α) → (Fin n → α) → Bool
  | 0, _, _ => true
  | n + 1, a, b => eqα (a 0) (b 0) && funEq eqα n (fun i => a i.succ) (fun i => b i.succ)

/-- The generic procedure is exact. -/
theorem funEq_true_iff {α : Type} (eqα : α → α → Bool)
    (heq : ∀ x y, eqα x y = true ↔ x = y) :
    ∀ (n : Nat) (a b : Fin n → α), funEq eqα n a b = true ↔ a = b := by
  intro n
  induction n with
  | zero =>
      intro a b
      constructor
      · intro _
        funext i
        exact Fin.elim0 i
      · intro _
        rfl
  | succ n ih =>
      intro a b
      constructor
      · intro h
        simp only [funEq, Bool.and_eq_true] at h
        obtain ⟨h0, hrest⟩ := h
        have hhead : a 0 = b 0 := (heq _ _).mp h0
        have htail : (fun i : Fin n => a i.succ) = (fun i : Fin n => b i.succ) :=
          (ih _ _).mp hrest
        funext i
        cases i using Fin.cases with
        | zero => exact hhead
        | succ j => exact congrFun htail j
      · intro h
        subst h
        simp [funEq, heq, ih]

/-- Decide equality of two relations. -/
def rel4Eq (x y : Rel4) : Bool := decide (x = y)

theorem rel4Eq_true_iff (x y : Rel4) : rel4Eq x y = true ↔ x = y := by
  simp [rel4Eq]

/-- Compare two rows, i.e. two functions `Fin n → Rel4`. -/
def rowEq (n : Nat) : (Fin n → Rel4) → (Fin n → Rel4) → Bool := funEq rel4Eq n

/-- Compare two matrices row by row, by instantiating `funEq` with the row procedure. -/
def matrixEq (n : Nat) (a b : MGraph n) : Bool := funEq (rowEq n) n a b

/-- The matrix procedure is exact. -/
theorem rowEq_true_iff (n : Nat) (a b : Fin n → Rel4) : rowEq n a b = true ↔ a = b :=
  funEq_true_iff rel4Eq rel4Eq_true_iff n a b

theorem matrixEq_true_iff (n : Nat) (a b : MGraph n) : matrixEq n a b = true ↔ a = b :=
  funEq_true_iff (rowEq n) (rowEq_true_iff n) n a b

/-- Matrices now have decidable equality. -/
instance {n : Nat} : DecidableEq (MGraph n) := fun a b =>
  decidable_of_iff (matrixEq n a b = true) (matrixEq_true_iff n a b)

end NobleM2.SizeChange
