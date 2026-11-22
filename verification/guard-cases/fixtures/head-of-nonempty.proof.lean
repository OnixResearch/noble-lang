import MC1Obligation
import NobleContracts
import NobleContracts.Expression

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  intro tail before params hi _hp hpre final he
  obtain ⟨v, vs, rfl, hv, hvs⟩ := (stackTyped_cons_iff before (.list .i64) []).mp hi
  obtain rfl := (stackTyped_nil_iff vs).mp hvs
  cases hv with
  | list hmem =>
    rename_i xs
    cases xs with
    | nil =>
      unfold MC1Obligation.precondition MC1Obligation.expression_2 MC1Obligation.expression_1
        MC1Obligation.expression_0 at hpre
      simp [Holds, evaluate] at hpre
    | cons a as =>
      have hfinal : final = tail ++ [a] := by
        simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1,
          MC1Obligation.node_2, MC1Obligation.node_3, MC1Obligation.node_4,
          Exec, Run, Step] using he
      refine ⟨[a], hfinal, .cons (hmem a (by simp)) .nil, ?_⟩
      unfold MC1Obligation.postcondition MC1Obligation.expression_6 MC1Obligation.expression_5
        MC1Obligation.expression_4 MC1Obligation.expression_3
      simp [Holds, evaluate]

end MC1Proof
