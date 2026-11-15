import Protected.Semantics

/-!
This is a separate solution to the approved amended challenge. Its proof terms
explicitly use the revised bound; the original approval must not admit it.
-/
namespace Protected

theorem zero_allowed : allowed 0 := by
  show 0 < 64
  decide

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) := by
  show n + 1 < 64
  omega

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) := by
  show a + b < 64
  omega

end Protected
