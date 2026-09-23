import MC1Obligation

open NobleContracts

namespace MC1Proof

/-- Conditional monotonicity for the guarded increment contract: the program is
the same `[ 1 + ]`, but admission is guarded by the `LtI64Max` template, so the
caller only reaches the body with an argument below `i64::MAX`. On that domain
wrapping addition is strictly increasing, which is exactly what the stated
postcondition `lt (in x) (out y)` claims. The unguarded version of this contract
is false at `i64::MAX`, which is why this proof needs the precondition. -/
theorem proof : MC1Obligation.claim := by
  change exportedClaim increment [.i64] [.i64] []
    (Holds (.lt (.input 0) (.i64 9223372036854775807)))
    (Holds (.lt (.input 0) (.output 0)))
  intro tail before params hi hp hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  refine ⟨[.i64 (n + 1)], (increment_exec_iff n tail final).mp he, .cons .i64 .nil, ?_⟩
  -- The guard's precondition, read as an integer comparison on the argument.
  have hn : n.toInt < 9223372036854775807 := by
    simpa [Holds, evaluate, i64] using hpre
  -- Adding one without signed overflow is strictly increasing.
  have hmono : n.toInt < (n + 1).toInt := by
    have h1 : BitVec.toInt (1 : BitVec 64) = 1 := by
      rw [BitVec.toInt_ofNat]; simp
    have hb : (n + 1).toInt = (n.toInt + 1).bmod 18446744073709551616 := by
      rw [BitVec.toInt_add, h1]
    rw [hb]
    have hlo : -(9223372036854775808 : Int) ≤ n.toInt + 1 := by
      have hbnd := BitVec.le_toInt n
      omega
    have hhi : n.toInt + 1 < (9223372036854775808 : Int) := by omega
    rw [Int.bmod_eq_of_le hlo hhi]
    omega
  simpa [Holds, evaluate, i64] using hmono

end MC1Proof
