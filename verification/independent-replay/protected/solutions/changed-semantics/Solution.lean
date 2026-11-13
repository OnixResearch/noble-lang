import Protected.Semantics

/-!
# Issue #62 protected-semantics control: changed imported semantics

The statements are identical to the challenge's, and all three proofs are still valid because the
bound rose from 32 to 64. Only `Protected.threshold` means something different. Each `show` states
the goal with the protected bound unfolded, so the proofs work on numerals.
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
