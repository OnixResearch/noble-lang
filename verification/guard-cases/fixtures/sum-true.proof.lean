import MC1Obligation
import NobleContracts
import NobleContracts.Expression

open NobleContracts

namespace MC1Proof

/-- Any precondition, `ensures true`: `[ [ 1 + ] [ 2 + ] case ]` over one `Sum I64 I64`
adds 1 to an `inl` payload and 2 to an `inr` payload; both results are `I64`. -/
theorem proof : MC1Obligation.claim := by
  intro tail before params hi _hp _hpre final he
  obtain ⟨v, vs, rfl, hv, hvs⟩ := (stackTyped_cons_iff before (.sum .i64 .i64) []).mp hi
  obtain rfl := (stackTyped_nil_iff vs).mp hvs
  cases hv with
  | inl hk =>
    cases hk
    rename_i k
    have hfinal : final = tail ++ [.i64 (k + BitVec.ofInt 64 1)] := by
      simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1, MC1Obligation.node_2, MC1Obligation.node_3,
          MC1Obligation.node_4, MC1Obligation.node_5, MC1Obligation.node_6, Exec, Run, Step] using he
    exact ⟨[.i64 (k + BitVec.ofInt 64 1)], hfinal, .cons .i64 .nil, holds_true _ _ _⟩
  | inr hk =>
    cases hk
    rename_i k
    have hfinal : final = tail ++ [.i64 (k + BitVec.ofInt 64 2)] := by
      simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1, MC1Obligation.node_2, MC1Obligation.node_3,
          MC1Obligation.node_4, MC1Obligation.node_5, MC1Obligation.node_6, Exec, Run, Step] using he
    exact ⟨[.i64 (k + BitVec.ofInt 64 2)], hfinal, .cons .i64 .nil, holds_true _ _ _⟩

end MC1Proof
