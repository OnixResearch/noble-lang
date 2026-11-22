import MC1Obligation
import NobleContracts
import NobleContracts.Expression

open NobleContracts

namespace MC1Proof

/-- Any precondition, `ensures true`: `[ [ 0 ] [ drop ] list.case ]` over one `List I64`
returns `0` on `nil` and the head on `cons`, both `I64`. -/
theorem proof : MC1Obligation.claim := by
  intro tail before params hi _hp _hpre final he
  obtain ⟨v, vs, rfl, hv, hvs⟩ := (stackTyped_cons_iff before (.list .i64) []).mp hi
  obtain rfl := (stackTyped_nil_iff vs).mp hvs
  cases hv with
  | list hmem =>
    rename_i xs
    cases xs with
    | nil =>
      have hfinal : final = tail ++ [.i64 (BitVec.ofInt 64 0)] := by
        simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1,
          MC1Obligation.node_2, MC1Obligation.node_3, MC1Obligation.node_4,
          Exec, Run, Step] using he
      exact ⟨[.i64 (BitVec.ofInt 64 0)], hfinal, .cons .i64 .nil, holds_true _ _ _⟩
    | cons a as =>
      have hfinal : final = tail ++ [a] := by
        simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1,
          MC1Obligation.node_2, MC1Obligation.node_3, MC1Obligation.node_4,
          Exec, Run, Step] using he
      exact ⟨[a], hfinal, .cons (hmem a (by simp)) .nil, holds_true _ _ _⟩

end MC1Proof
