import MC1Obligation

open NobleContracts

namespace MC1Proof

theorem refutation : Not MC1Obligation.claim := by
  change ¬ exportedClaim increment [.i64] [.i64] []
    (Holds (.bool true)) (Holds (.lt (.input 0) (.output 0)))
  intro claim
  let maximum : BitVec 64 := BitVec.ofInt 64 9223372036854775807
  have run := increment_normal maximum []
  obtain ⟨after, hafter, _, hpost⟩ := claim [] [.i64 maximum] []
    (.cons .i64 .nil) .nil rfl _ run
  have : after = [.i64 (maximum + 1)] := by simpa using hafter.symm
  subst after
  have impossible : ¬ Holds (.lt (.input 0) (.output 0))
      [.i64 maximum] [.i64 (maximum + 1)] [] := by
    simp [Holds, evaluate, maximum]
  exact impossible hpost

end MC1Proof
