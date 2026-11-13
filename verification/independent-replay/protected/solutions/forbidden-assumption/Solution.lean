import Protected.Axiom

/-!
# Issue #62 protected-semantics control: a forbidden external assumption

The theorem statements and imported semantics match the challenge, but `zero_allowed` is proved by
the custom axiom `Protected.external_zero`. The checker must classify the assumption violation
separately from statement, definition and missing-constant diagnostics.
-/
namespace Protected

theorem zero_allowed : allowed 0 := external_zero

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) := by
  show n + 1 < 32
  omega

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) := by
  show a + b < 32
  omega

end Protected
