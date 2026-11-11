import NobleM2.SizeChangeUniverse

/-!
# Decidable equality for call graphs (issue #101)

Core has no `DecidableEq` for a function type, and this project imports no `Fintype`, so graphs
could not be compared at all - a checker that either enumerates them or looks one up had no way to
decide `g = h`. This module supplies the decision procedure: compare the two graphs at every
argument, recursing on the domain size, and prove the procedure exact with function extensionality.

With `DecidableEq (Graph n)` the enumeration becomes searchable (`List.idxOf`), which is what a
checker needs before the bound can be stated.
-/

namespace NobleM2.SizeChange

/-- Compare two graphs argument by argument, recursing on the domain size. -/
def pointwiseEq : (n : Nat) → Graph n → Graph n → Bool
  | 0, _, _ => true
  | n + 1, a, b => (a 0 == b 0) && pointwiseEq n (fun i => a i.succ) (fun i => b i.succ)

/-- The procedure is exact. -/
theorem pointwiseEq_true_iff {n : Nat} (a b : Graph n) :
    pointwiseEq n a b = true ↔ a = b := by
  induction n with
  | zero =>
      constructor
      · intro _
        funext i
        exact Fin.elim0 i
      · intro _
        rfl
  | succ n ih =>
      constructor
      · intro h
        simp only [pointwiseEq, Bool.and_eq_true, beq_iff_eq] at h
        obtain ⟨h0, hrest⟩ := h
        have htail := (ih (fun i => a i.succ) (fun i => b i.succ)).mp hrest
        funext i
        cases i using Fin.cases with
        | zero => exact h0
        | succ j => exact congrFun htail j
      · intro h
        subst h
        simp [pointwiseEq, ih]

/-- Graphs now have decidable equality. -/
instance {n : Nat} : DecidableEq (Graph n) := fun a b =>
  decidable_of_iff (pointwiseEq n a b = true) (pointwiseEq_true_iff a b)

/-! ## Fixtures -/

theorem pointwiseEq_same : pointwiseEq 2 (fun _ => .decrease) (fun _ => .decrease) = true := rfl

theorem pointwiseEq_differs : pointwiseEq 2 (fun _ => .decrease) (fun i => if i = 0 then .decrease else .equal) = false := by
  decide

/-- The decidable equality agrees with the proposition on a difference. -/
theorem decEq_differs :
    ¬ ((fun _ : Fin 2 => Rel.decrease) = (fun i => if i = 0 then Rel.decrease else Rel.equal)) := by
  intro h
  have := congrFun h 1
  simp at this

end NobleM2.SizeChange
