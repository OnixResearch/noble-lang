/-
Issue #62 protected-semantics control: the shared semantics a challenge and a solution must agree on.

This file is a lane fixture, not part of any proof build. `threshold` and `allowed` are the constants
the challenge statements mention, so the exported-closure fallback requires their types and bodies to
be *identical* in the challenge and solution. The challenge lists no definition holes: protected
semantics is the thing under test.
-/
namespace Protected

/-- The bound every allowed value stays under. -/
def threshold : Nat := 32

/-- A value is allowed exactly when it is below the threshold. -/
def allowed (n : Nat) : Prop := n < threshold

end Protected
