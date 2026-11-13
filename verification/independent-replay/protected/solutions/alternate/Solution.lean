import Protected.Semantics

/-!
# Issue #62 protected-semantics positive control: an alternate proof

The same statements under the same semantics, proved with different terms than both the challenge and
the real solution. The exported-closure comparison must accept it: proof bodies are excluded from the
semantic digest, so an alternate proof of the same statement is a positive control for the lane.
-/
namespace Protected

theorem zero_allowed : allowed 0 :=
  Nat.zero_lt_succ 31

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) :=
  Nat.lt_of_lt_of_le (Nat.succ_lt_succ h) (Nat.le_refl 32)

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) :=
  Nat.add_lt_add ha hb

end Protected
