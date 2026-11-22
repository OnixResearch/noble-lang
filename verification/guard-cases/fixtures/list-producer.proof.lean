import MC1Obligation
import NobleContracts
import NobleContracts.Expression

open NobleContracts

namespace MC1Proof

/-- `[ 5 nil cons ]` from the empty interface returns the nonempty `[5]`. -/
theorem proof : MC1Obligation.claim := by
  intro tail before params hi _hp _hpre final he
  obtain rfl := (stackTyped_nil_iff before).mp hi
  have hfinal : final = tail ++ [.list [.i64 (BitVec.ofInt 64 5)]] := by
    simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1,
      MC1Obligation.node_2, Exec, Run, Step] using he
  refine ⟨[.list [.i64 (BitVec.ofInt 64 5)]], hfinal, .cons (.list (by simp)) .nil, ?_⟩
  unfold MC1Obligation.postcondition MC1Obligation.expression_3 MC1Obligation.expression_2
    MC1Obligation.expression_1
  simp [Holds, evaluate]

end MC1Proof
