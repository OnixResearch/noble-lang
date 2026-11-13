namespace Protected

/-- The protected semantics are unchanged; only the theorem proof is invalid. -/
def threshold : Nat := 32

def allowed (n : Nat) : Prop := n < threshold

end Protected
