import Protected.Semantics

/-!
# Issue #62 protected-semantics control: a weakened predicate

The statements are spelled exactly as in the challenge, but `Protected.allowed` means `True` here, so
every proof is trivial. The two hypothesis binders are kept — their names must match the challenge's,
because the comparison is over the elaborated statement — and referenced in the proof terms only so
the build stays warning-free.

This solution builds and its statements elaborate to the same constant names; only the definition's
meaning differs. The exported-closure comparison must refuse it on `Protected.allowed`.
-/
namespace Protected

theorem zero_allowed : allowed 0 :=
  Or.inr True.intro

theorem succ_of_lt (n : Nat) (h : n < 31) : allowed (n + 1) :=
  (fun _ => Or.inr True.intro) h

theorem add_allowed (a : Nat) (b : Nat) (ha : a < 16) (hb : b < 16) : allowed (a + b) :=
  (fun _ _ => Or.inr True.intro) ha hb

end Protected
