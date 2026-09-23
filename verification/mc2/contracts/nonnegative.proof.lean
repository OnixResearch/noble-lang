import MC1Obligation

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  change exportedClaim increment [.i64] [.i64] []
    (Holds (.le (.i64 0) (.input 0)))
    (Holds (.eq (.output 0) (.add (.input 0) (.i64 1))))
  intro tail before params hi hp hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  refine ⟨[.i64 (n + 1)], (increment_exec_iff n tail final).mp he,
    .cons .i64 .nil, ?_⟩
  simp [Holds, evaluate]

end MC1Proof
