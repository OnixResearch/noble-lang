import MC1Obligation
import Crush

open NobleContracts

namespace MC1Proof

set_option crush.timeout 120000 in
set_option crush.trust "reconstruct" in
theorem proof : MC1Obligation.claim := by
  unfold MC1Obligation.claim
  crush

#print axioms proof

end MC1Proof
