import Protected.Semantics

/-!
The independently selected amended challenge retains the three theorem signatures,
but its imported protected threshold is 64 rather than 32. The approval manifest
binds this challenge's exact source bytes and exported semantic closure before
an amended solution is evaluated.
-/
namespace Protected

theorem zero_allowed : allowed 0 := by
  unfold allowed threshold
  omega

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) := by
  unfold allowed threshold
  omega

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) := by
  unfold allowed threshold
  omega

end Protected
