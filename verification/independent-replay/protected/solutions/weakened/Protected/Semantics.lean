/-
Issue #62 protected-semantics control: a *weakened* predicate under the same name and type.

`allowed` is weakened to `n < threshold ∨ True`, so every statement that mentions `allowed` becomes
trivial while its elaborated statement still names the same constant with the same type. Retaining
the `threshold` reference keeps the semantic dependency closure complete, allowing the comparison
to identify the changed `Protected.allowed` body rather than misclassifying an absent dependency.
-/
namespace Protected

/-- Still declared, and still equal to the challenge's. -/
def threshold : Nat := 32

/-- The weakened predicate: same name and dependencies, but a strictly weaker proposition. -/
def allowed (n : Nat) : Prop := n < threshold ∨ True

end Protected
