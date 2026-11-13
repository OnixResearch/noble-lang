/-
Issue #62 protected-semantics control: semantics shared with the challenge.
-/
namespace Protected

def threshold : Nat := 32

def allowed (n : Nat) : Prop := n < threshold

end Protected
