import Protected.Semantics

/-!
# Issue #62 protected-semantics challenge

The caller-owned challenge package: a trusted reference that states — and kernel-checks — the three
theorems a solution must prove about `Protected.threshold` and `Protected.allowed`.

The challenge and solutions are separately built packages. The exported-closure fallback compares
the canonical statement for every theorem target and the declarations reached from those statements.
These targets are theorems with kernel-checked proofs, not axioms.

This is a lane fixture. No repository proof gate consumes this project, and none of its statements
is a Noble claim.
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
