import MC1Obligation
import NobleContracts
import NobleContracts.Expression

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  intro tail before params hi _hp _hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  have he' : Exec increment (tail ++ [.i64 n]) final := by
    simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1,
      increment] using he
  have hfinal : final = tail ++ [.i64 (n + 1)] :=
    (increment_exec_iff n tail final).mp he'
  exact ⟨[.i64 (n + 1)], hfinal, .cons .i64 .nil, holds_true _ _ _⟩

end MC1Proof
