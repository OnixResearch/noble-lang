/-
Issue #62 protected-semantics control: changed imported semantics.

`allowed` is textually the same definition, but the protected bound it mentions is 64 instead of 32.
Every statement still elaborates to the same constant names, and every proof still goes through; only
the meaning of `Protected.threshold` changed. The exported-closure comparison must refuse this
solution on the `Protected.threshold` definition.
-/
namespace Protected

/-- The changed bound. -/
def threshold : Nat := 64

/-- Textually the same definition as the challenge's. -/
def allowed (n : Nat) : Prop := n < threshold

end Protected
