import Protected.Semantics

/-!
# Issue #62 protected-semantics solution: the real proof

A separately built solution of the challenge's three theorems. Its signatures match the challenge's;
its proofs are the straightforward tactic proofs. Each `show` states the goal
with `Protected.threshold` unfolded, which is definitional, so the tactic proofs work on numerals.
-/
namespace Protected

theorem zero_allowed : allowed 0 := by
  show 0 < 32
  decide

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) := by
  show n + 1 < 32
  omega

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) := by
  show a + b < 32
  omega

end Protected
