namespace Protected

/-- The amended challenge's independently approved allowance bound. -/
def threshold : Nat := 64

/-- The amended challenge's predicate under that bound. -/
def allowed (n : Nat) : Prop := n < threshold

end Protected
