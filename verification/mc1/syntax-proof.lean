import MC1Obligation
open NobleContracts
namespace MC1Proof
theorem proof : MC1Obligation.claim := by
  change exportedClaim [.word 11] [.program [.i64] [.i64]] [.«syntax»] []
    (Holds (.bool true)) (Holds (.bool true))
  intro tail before params hi hp hpre final he
  obtain ⟨p, rfl⟩ : ∃ p, before = [.program p] := by
    cases hi with
    | cons htype rest =>
      cases rest
      cases htype
      exact ⟨_, rfl⟩
  exact ⟨[.«syntax» p], exec_reflect_iff.mp he, .cons .«syntax» .nil, rfl⟩
end MC1Proof
