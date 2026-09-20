import MC1Obligation
open NobleContracts
namespace MC1Proof
theorem proof : MC1Obligation.claim := by
  change exportedClaim [.lit (.i64 (BitVec.ofInt 64 9223372036854775807)), .lit (.i64 1), .word 4]
    [] [.i64] [] (Holds (.bool true))
    (Holds (.eq (.output 0) (.i64 (-9223372036854775808))))
  intro tail before params hi hp hpre final he
  cases hi
  have result : final = tail ++ [.i64 (BitVec.ofInt 64 (-9223372036854775808))] := by
    simpa [Exec] using he
  refine ⟨[.i64 (BitVec.ofInt 64 (-9223372036854775808))], result, .cons .i64 .nil, ?_⟩
  simp [Holds, evaluate]
end MC1Proof
