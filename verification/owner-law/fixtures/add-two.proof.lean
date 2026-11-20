import MC1Obligation

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  change exportedClaim MC1Obligation.program [.i64] [.i64] []
    (Holds (.bool true))
    (Holds (.eq (.output 0) (.add (.input 0) (.i64 2))))
  intro tail before params hi hp hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  have exec_iff : ∀ (tail : Stack) (result : Stack),
      Exec MC1Obligation.program (tail ++ [.i64 n]) result ↔
        result = tail ++ [.i64 (n + 2)] := by
    intro tail result
    simp [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1, Exec]
  refine ⟨[.i64 (n + 2)], (exec_iff tail final).mp he, .cons .i64 .nil, ?_⟩
  simp [Holds, evaluate]

end MC1Proof
