import MC1Obligation
import NobleContracts.Obligation
import NobleContracts.Expression

open NobleContracts

namespace IntrinsicFeasibility

private def literal (tail : Stack) (x : BitVec 64) :
    PC [.lit (.i64 1)]
      (fun before => before = tail ++ [.i64 x])
      (fun _ middle => middle = (tail ++ [.i64 x]) ++ [.i64 1]) :=
  pc_exact (code := [.lit (.i64 1)]) (s := tail ++ [.i64 x])
    (expected := (tail ++ [.i64 x]) ++ [.i64 1])
    (fun final => exec_lit_iff (v := .i64 1)
      (s := tail ++ [.i64 x]) (t := final))

private def addition (tail : Stack) (x : BitVec 64) :
    PC [.word 4]
      (fun middle => middle = tail ++ [.i64 x, .i64 1])
      (fun _ final => final = tail ++ [.i64 (x + 1)]) :=
  pc_exact (code := [.word 4]) (s := tail ++ [.i64 x, .i64 1])
    (expected := tail ++ [.i64 (x + 1)])
    (fun final => exec_add_iff (a := x) (b := 1)
      (s := tail) (t := final))

/-- Both fixed-stack exact rules are instantiated inside the tail/input binders.
The bridge changes the first rule's actual intermediate stack to the second
rule's complete input; the join retains the initial stack, not just the final
value. Neither implication can be supplied merely by matching types. -/
private def composed (tail : Stack) (x : BitVec 64) :
    PC ([.lit (.i64 1)] ++ [.word 4])
      (fun before => before = tail ++ [.i64 x])
      (fun before final => before = tail ++ [.i64 x] ∧
        final = tail ++ [.i64 (x + 1)]) :=
  @pc_sequence
    [.lit (.i64 1)] [.word 4]
    (fun (before : Stack) => before = tail ++ [.i64 x])
    (fun (_ middle : Stack) => middle = (tail ++ [.i64 x]) ++ [.i64 1])
    (fun (middle : Stack) => middle = tail ++ [.i64 x, .i64 1])
    (fun (_ final : Stack) => final = tail ++ [.i64 (x + 1)])
    (fun (before final : Stack) => before = tail ++ [.i64 x] ∧
      final = tail ++ [.i64 (x + 1)])
    (literal tail x)
    (addition tail x)
    (fun _ middle _ exactMiddle =>
      exactMiddle.trans (List.append_assoc tail [.i64 x] [.i64 1]))
    (fun _ _ _ initial _ exactFinal => ⟨initial, exactFinal⟩)

/-- The inner PC is insufficient on its own: first extract universal equality,
then export exact I64 stacks and prove MC1 Holds consequences. -/
theorem increment : MC1Obligation.claim :=
  exportedClaim_consequence
    (exportedClaim_unaryI64
      (code := [.lit (.i64 1)] ++ [.word 4]) (f := fun x => x + 1)
      (fun tail x final execution =>
        ((composed tail x) (tail ++ [.i64 x]) final rfl execution).2))
    (fun _ _ _ => True.intro)
    (fun before after params _ exactPost =>
      match exactPost with
      | ⟨x, hbefore, hafter⟩ =>
        hbefore.symm ▸ hafter.symm ▸
        (holds_eq (.output 0) (.add (.input 0) (.i64 1))
          ⟨[.i64 x], [.i64 (x + 1)], params⟩
          (.i64 (x + 1)) (.i64 (x + 1)) rfl rfl).mpr rfl)

end IntrinsicFeasibility
