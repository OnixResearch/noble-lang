import NobleFoundation.Sequence

/-!
# Scalar branch model for the foundation differential comparator

This file extends the straight-line I64 fragment with one scalar `if` and one
I64 sum case. The reference dispatches only to the selected body. The MC1
proof preserves an arbitrary public stack prefix; it is not a backend theorem.
-/

namespace NobleFoundation

open NobleContracts

inductive BranchKind where
  | ite
  | sum
  deriving DecidableEq, Repr

/-- Select the executed branch; `true` is the left/first branch. -/
def selectedOps (left : Bool) (p q : List AOp) : List AOp :=
  if left then p else q

/-- Visit counts indexed by branch: the untaken branch has zero visits. -/
def branchVisits (left : Bool) : List Nat :=
  [if left then 1 else 0, if left then 0 else 1]

/-- The sum selector injects the top I64; the if selector pushes its Bool. -/
def branchCode (kind : BranchKind) (left : Bool) (p q : List AOp) : List Op :=
  match kind with
  | .ite =>
      [.lit (.bool left), .block (lowerAll p), .block (lowerAll q), .word 18]
  | .sum =>
      [.word (if left then 15 else 16), .block (lowerAll p),
        .block (lowerAll q), .word 17]

/-- Independent reference; a sum case requires a top-first I64 payload. -/
def branchRef (kind : BranchKind) (left : Bool) (p q : List AOp)
    (zs : List ℤ) : Option (List ℤ) :=
  match kind with
  | .ite => refRun (selectedOps left p q) zs
  | .sum =>
      match zs with
      | [] => none
      | _ => refRun (selectedOps left p q) zs

private theorem enc_cons (z : ℤ) (zs : List ℤ) :
    enc (z :: zs) = enc zs ++ [i64v z] := by
  simp [enc]

private theorem exec_if_true {p q : List Op} {s t : Stack} :
    Exec [.lit (.bool true), .block p, .block q, .word 18] s t ↔ Exec p s t := by
  simp [Exec]

private theorem exec_if_false {p q : List Op} {s t : Stack} :
    Exec [.lit (.bool false), .block p, .block q, .word 18] s t ↔ Exec q s t := by
  simp [Exec]

private theorem exec_sum_left {p q : List Op} {v : Value} {s t : Stack} :
    Exec [.word 15, .block p, .block q, .word 17] (s ++ [v]) t ↔
      Exec p (s ++ [v]) t := by
  simp [Exec]

private theorem exec_sum_right {p q : List Op} {v : Value} {s t : Stack} :
    Exec [.word 16, .block p, .block q, .word 17] (s ++ [v]) t ↔
      Exec q (s ++ [v]) t := by
  simp [Exec]

/-- The selected body executes; the other body is constructed but not run. -/
theorem exec_branchCode_selected (kind : BranchKind) (left : Bool)
    (p q : List AOp) (zs : List ℤ)
    (hinput : kind = .ite ∨ zs ≠ []) (pre t : Stack) :
    Exec (branchCode kind left p q) (pre ++ enc zs) t ↔
      Exec (lowerAll (selectedOps left p q)) (pre ++ enc zs) t := by
  cases kind with
  | ite =>
    cases left with
    | false =>
        simpa [branchCode, selectedOps] using
          (exec_if_false (p := lowerAll p) (q := lowerAll q) (s := pre ++ enc zs) (t := t))
    | true =>
        simpa [branchCode, selectedOps] using
          (exec_if_true (p := lowerAll p) (q := lowerAll q) (s := pre ++ enc zs) (t := t))
  | sum =>
    cases zs with
    | nil =>
        rcases hinput with hkind | hne
        · cases hkind
        · exact False.elim (hne rfl)
    | cons z rest =>
        rw [enc_cons]
        cases left with
        | false =>
            simpa [branchCode, selectedOps, List.append_assoc] using
              (exec_sum_right (p := lowerAll p) (q := lowerAll q) (v := i64v z)
                (s := pre ++ enc rest) (t := t))
        | true =>
            simpa [branchCode, selectedOps, List.append_assoc] using
              (exec_sum_left (p := lowerAll p) (q := lowerAll q) (v := i64v z)
                (s := pre ++ enc rest) (t := t))

/-- Successful branch reference results are exactly the MC1 normal results. -/
theorem exec_branchRef {kind : BranchKind} {left : Bool} {p q : List AOp}
    {zs zs' : List ℤ} (h : branchRef kind left p q zs = some zs')
    (pre t : Stack) :
    Exec (branchCode kind left p q) (pre ++ enc zs) t ↔ t = pre ++ enc zs' := by
  have hinput : kind = .ite ∨ zs ≠ [] := by
    cases kind with
    | ite => exact Or.inl rfl
    | sum =>
        cases zs with
        | nil => simp [branchRef] at h
        | cons z rest => exact Or.inr (by simp)
  have href : refRun (selectedOps left p q) zs = some zs' := by
    cases kind with
    | ite => simpa [branchRef] using h
    | sum =>
        cases zs with
        | nil => simp [branchRef] at h
        | cons z rest => simpa [branchRef] using h
  rw [exec_branchCode_selected kind left p q zs hinput pre t]
  exact exec_lowerAll href pre t

/-- Untaken changes preserve the selected arithmetic syntax exactly. -/
theorem selectedOps_untaken (left : Bool) (selected other₁ other₂ : List AOp) :
    selectedOps left (if left then selected else other₁) (if left then other₁ else selected) =
      selectedOps left (if left then selected else other₂) (if left then other₂ else selected) := by
  cases left <;> rfl

/-- Untaken changes preserve reference results, including empty-sum refusal. -/
theorem branchRef_untaken (kind : BranchKind) (left : Bool)
    (selected other₁ other₂ : List AOp) (zs : List ℤ) :
    branchRef kind left
        (if left then selected else other₁) (if left then other₁ else selected) zs =
      branchRef kind left
        (if left then selected else other₂) (if left then other₂ else selected) zs := by
  have hs := selectedOps_untaken left selected other₁ other₂
  cases kind with
  | ite => simp [branchRef, hs]
  | sum => cases zs <;> simp [branchRef, hs]

/-- On admitted inputs, untaken changes preserve every MC1 normal result. -/
theorem exec_branchCode_untaken (kind : BranchKind) (left : Bool)
    (selected other₁ other₂ : List AOp) (zs : List ℤ)
    (hinput : kind = .ite ∨ zs ≠ []) (pre t : Stack) :
    Exec (branchCode kind left
        (if left then selected else other₁) (if left then other₁ else selected))
        (pre ++ enc zs) t ↔
      Exec (branchCode kind left
        (if left then selected else other₂) (if left then other₂ else selected))
        (pre ++ enc zs) t := by
  rw [exec_branchCode_selected kind left
      (if left then selected else other₁) (if left then other₁ else selected)
      zs hinput pre t]
  rw [exec_branchCode_selected kind left
      (if left then selected else other₂) (if left then other₂ else selected)
      zs hinput pre t]
  rw [selectedOps_untaken]

/-- Exact visit counts: one selected visit and zero untaken visits. -/
theorem branchVisits_selected (left : Bool) :
    branchVisits left = if left then [1, 0] else [0, 1] := by
  cases left <;> rfl

end NobleFoundation
