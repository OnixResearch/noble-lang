import Protected.Semantics

namespace Protected

/-- The statement matches the challenge, but Lean must reject this ill-typed proof term. -/
theorem zero_allowed : allowed 0 := True.intro

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) := by
  show n + 1 < 32
  omega

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) := by
  show a + b < 32
  omega

end Protected
