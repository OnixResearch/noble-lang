import MC1Obligation

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  change exportedClaim [.word 13] [.list .i64, .i64] [.pair (.list .i64) .i64] []
    (Holds (.definition 0 (.bool true)))
    (Holds (.and (.eq (.first (.output 0)) (.input 0))
      (.eq (.second (.output 0)) (.input 1))))
  intro tail before params hi hp hpre final he
  obtain ⟨a, b, rfl, ha, hb⟩ :
      ∃ a b, before = [a, b] ∧ HasType a (.list .i64) ∧ HasType b .i64 := by
    cases hi with
    | cons ha rest =>
      cases rest with
      | cons hb last =>
        cases last
        exact ⟨_, _, rfl, ha, hb⟩
  refine ⟨[.pair a b], exec_pair_iff.mp he, .cons (.pair ha hb) .nil, ?_⟩
  simp [Holds, evaluate]

end MC1Proof
