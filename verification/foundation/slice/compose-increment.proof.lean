import MC1Obligation

open NobleContracts

namespace MC1Proof

/-- Self-contained MC1 proof (no Mathlib) of the exported claim for
`[ quote [ 1 + ] compose run ]`; the same statement is proved in the Mathlib
foundation as `NobleFoundation.slice_exported_claim`/`slice_body_exec`. -/
theorem proof : MC1Obligation.claim := by
  change exportedClaim
    [.word 8, .block [.lit (.i64 (BitVec.ofInt 64 1)), .word 4], .word 9, .word 10]
    [.i64] [.i64] []
    (Holds (.bool true))
    (Holds (.eq (.output 0) (.add (.input 0) (.i64 1))))
  intro tail before params hi hp hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  have exactResult : final = tail ++ [.i64 (n + BitVec.ofInt 64 1)] := by
    simpa [Exec, List.reverse_append] using he
  refine ⟨[.i64 (n + BitVec.ofInt 64 1)], exactResult, .cons .i64 .nil, ?_⟩
  simp [Holds, evaluate]

end MC1Proof
