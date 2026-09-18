/-
Reference model: soundness of the reference decision function (PO-09).

The acceptance theorem's shape (V-CHECK-03) is:
`check(candidate, expected_interface, environment) = Accepted(checked)` implies
`WellFormed(checked) and TypingDerivation(checked, expected_interface)`.

This module develops the lemmas that carry the fold's successful outcome back
to the declarative judgment.
-/

import NobleM2.Check

namespace NobleM2

/-- An accepted scheme application exposes the instantiated interface: the
interface's stacks and bound are exactly the substitution's results. -/
theorem applyScheme_ok {env : Env} {limits : Limits} {scheme : Scheme} {inst : Inst}
    {iface : Interface} (accepted : applyScheme env limits scheme inst = .ok iface) :
    scheme.instantiate inst = some (iface.stackIn, iface.stackOut, iface.effects) := by
  unfold applyScheme at accepted
  split at accepted
  · contradiction
  · split at accepted
    · contradiction
    · split at accepted
      · contradiction
      · split at accepted
        · contradiction
        · injection accepted with iface_eq
          subst iface_eq
          assumption

end NobleM2
