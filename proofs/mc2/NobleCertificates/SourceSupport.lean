import NobleContractImpl.Funs

open Aeneas Aeneas.Std Result

namespace NobleCertificates.SourceSupport

set_option Aeneas.customDoElab false

/-- Observe only actual successful computations. Failure, divergence and a
visible host effect do not count as a passing source equation. -/
def observed (computation : Result Bool) : Bool :=
  match computation.match with
  | .ok result => result
  | _ => false

/-- Actual production defaults and constructor, not a hand-assembled registry. -/
def fresh : Result noble_contracts.companion.Core := do
  let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
  noble_contracts.companion.core.context.Core.new limits

def prepare (source : Str) :
    Result (core.result.Result noble_contracts.Prepared noble_contracts.Diagnostic) := do
  let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
  noble_contracts.prepare source limits

/-- A concrete empty public observation for refusal checks with no retained
contract. It cannot provide evidence authority or mask a failed computation. -/
def emptyObservation : Result noble_contracts.companion.Subject :=
  noble_contracts.companion.core.Core.observe
    (alloc.vec.Vec.deref (alloc.vec.Vec.new (U32 × U64)))
    { input := 0#u64, output := 0#u64 }

end NobleCertificates.SourceSupport
