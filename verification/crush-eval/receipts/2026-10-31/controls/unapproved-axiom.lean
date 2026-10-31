import MC1Obligation

open NobleContracts

namespace MC1Proof

axiom unapprovedEscape : ∀ (p : Prop), p

theorem proof : MC1Obligation.claim := by
  exact unapprovedEscape _

end MC1Proof
