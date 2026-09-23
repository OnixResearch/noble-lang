import MC1Obligation

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  change exportedClaim twoIncrements [.i64] [.i64] []
    (Holds (.bool true))
    (Holds (.eq (.output 0) (.add (.add (.input 0) (.i64 1)) (.i64 1))))
  intro tail before params hi hp hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  have exactResult : final = tail ++ [.i64 ((n + 1) + 1)] := by
    simpa [twoIncrements, increment, Exec] using he
  refine ⟨[.i64 ((n + 1) + 1)], exactResult, .cons .i64 .nil, ?_⟩
  simp [Holds, evaluate]

end MC1Proof
