namespace Protected

/-- The separately approved amended allowance bound. -/
def threshold : Nat := 64

/-- The same predicate form, now interpreted against the amended bound. -/
def allowed (n : Nat) : Prop := n < threshold

end Protected
