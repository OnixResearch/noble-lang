import MC1Obligation

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  change exportedClaim captureBuilder [.i64] [.program [.i64] [.i64]] [.i64]
    (Holds (.bool true))
    (Holds (.maps (.output 0) (.param 0) (.add (.param 0) (.input 0))))
  intro tail before params hi hp hpre final he
  obtain ⟨capture, rfl⟩ := (stackTyped_i64_iff before).mp hi
  obtain ⟨later, rfl⟩ := (stackTyped_i64_iff params).mp hp
  refine ⟨[.program (capturedAdd capture)],
    (captureBuilder_exec_iff capture tail final).mp he,
    .cons (.program (capturedAdd_typed capture)) .nil, ?_⟩
  simp [Holds, evaluate, capturedAdd, Exec]

end MC1Proof
