import Protected.Semantics

/-!
# Issue #62 protected-semantics control: a valid proof of the wrong statement

The solution package builds successfully, but `zero_allowed` changes the theorem statement from
`allowed 0` to `allowed 1`. The other two statements are unchanged. The comparison must classify
this as a statement mismatch, not as a semantic-definition mismatch or a proof failure.
-/
namespace Protected

theorem zero_allowed : allowed 1 := by
  show 1 < 32
  decide

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) := by
  show n + 1 < 32
  omega

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) := by
  show a + b < 32
  omega

end Protected
