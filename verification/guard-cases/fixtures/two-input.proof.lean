import MC1Obligation
import NobleContracts
import NobleContracts.Expression

open NobleContracts

namespace MC1Proof

/-- `[ 0 + drop [ 0 ] [ drop ] list.case ]` over `[List I64, I64]`, `ensures true`. -/
theorem proof : MC1Obligation.claim := by
  intro tail before params hi _hp _hpre final he
  obtain ⟨v, vs, rfl, hv, hvs⟩ := (stackTyped_cons_iff before (.list .i64) [.i64]).mp hi
  obtain ⟨w, ws, rfl, hw, hws⟩ := (stackTyped_cons_iff vs .i64 []).mp hvs
  obtain rfl := (stackTyped_nil_iff ws).mp hws
  cases hw
  cases hv with
  | list hmem =>
    rename_i xs
    cases xs with
    | nil =>
      have hfinal : final = tail ++ [.i64 (BitVec.ofInt 64 0)] := by
        simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1, MC1Obligation.node_2, MC1Obligation.node_3,
          MC1Obligation.node_4, MC1Obligation.node_5, MC1Obligation.node_6, MC1Obligation.node_7, Exec, Run, Step] using he
      exact ⟨[.i64 (BitVec.ofInt 64 0)], hfinal, .cons .i64 .nil, holds_true _ _ _⟩
    | cons a as =>
      have hfinal : final = tail ++ [a] := by
        simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1, MC1Obligation.node_2, MC1Obligation.node_3,
          MC1Obligation.node_4, MC1Obligation.node_5, MC1Obligation.node_6, MC1Obligation.node_7, Exec, Run, Step] using he
      exact ⟨[a], hfinal, .cons (hmem a (by simp)) .nil, holds_true _ _ _⟩

end MC1Proof
