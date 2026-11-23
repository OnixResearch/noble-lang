import MC1Obligation
open NobleContracts
namespace MC1Proof
theorem proof : MC1Obligation.claim :=
  fun _tail _before _params _inputs _parameters hpre => absurd hpre (holds_false _ _ _)
end MC1Proof
