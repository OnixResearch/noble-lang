import NobleFoundation.CertificateCodec

/-!
Model of the decimal-token transport for `ngraph-cert/2`.

The model consumes the chained profile's transition tags, exact 32-byte endpoint
digests, seven-field rewrite premises, and final token boundary. It is a Lean model
only: no correspondence theorem binds this parser or its transition semantics to
Rust `cert.rs`, graph admission, or pruning implementation.
-/

namespace NobleFoundation.CertificateChained

open NobleFoundation.Certificate
open NobleFoundation.CertificateCodec

/-- The modelled chained certificate revision. -/
def chainedRevision : String := "ngraph-cert/2"

/-- An endpoint identity is exactly 32 unsigned bytes. -/
abbrev Digest := List Nat

/-- One transported state transition. -/
inductive Action where
  | rewrite (step : Step)
  | prune
  deriving DecidableEq, Repr

/-- Every transition binds the identities of its actual source and destination state. -/
structure Transition where
  action : Action
  before : Digest
  after : Digest
  deriving DecidableEq, Repr

/-- Chained derivation envelope after its fixed header has been parsed. -/
structure ChainedDerivation where
  revision : String
  model : String
  left : Digest
  right : Digest
  steps : List Transition
  deriving DecidableEq, Repr

/-- Consume exactly `count` leading tokens without changing their order. -/
def takePrefix (count : Nat) (tokens : List String) :
    Option (List String × List String) :=
  if count ≤ tokens.length then some (tokens.take count, tokens.drop count) else none

/-- Decode exactly 32 decimal tokens as unsigned bytes, leaving the suffix untouched. -/
def decodeDigest (tokens : List String) : Option (Digest × List String) := do
  let (byteTokens, rest) ← takePrefix 32 tokens
  let bytes ← byteTokens.foldrM (fun token decoded => do
    let value ← decimal token
    if value < 256 then some (value :: decoded) else none) []
  some (bytes, rest)

/-- Decode one transition, rejecting unknown tags, malformed rewrite steps, bad bytes,
and truncated endpoint digests. -/
def decodeTransition (tokens : List String) : Option (Transition × List String) :=
  match tokens with
  | "0" :: rest => do
      let (fields, rest) ← takePrefix 7 rest
      let step ← CertificateCodec.decodeStep fields
      let (before, rest) ← decodeDigest rest
      let (after, rest) ← decodeDigest rest
      some (⟨.rewrite step, before, after⟩, rest)
  | "1" :: rest => do
      let (before, rest) ← decodeDigest rest
      let (after, rest) ← decodeDigest rest
      some (⟨.prune, before, after⟩, rest)
  | _ => none

/-- Decode the declared number of transitions and return all unconsumed tokens. -/
def decodeTransitions : Nat → List String → Option (List Transition × List String)
  | 0, tokens => some ([], tokens)
  | count + 1, tokens => do
      let (step, rest) ← decodeTransition tokens
      let (steps, rest) ← decodeTransitions count rest
      some (step :: steps, rest)

/-- Decode a chained envelope body. Count bounds and the exact trailing-token boundary
are checked; the caller supplies the three fixed header tokens already parsed. -/
def decodeChained (revisionToken modelToken : String) (declaredSteps : Nat)
    (tokens : List String) : Option ChainedDerivation :=
  if revisionToken = chainedRevision then
    if modelToken = modelRevision then
      if declaredSteps ≤ maxSteps then do
        let (left, tokens) ← decodeDigest tokens
        let (right, tokens) ← decodeDigest tokens
        let (steps, tokens) ← decodeTransitions declaredSteps tokens
        if tokens.isEmpty then
          some ⟨revisionToken, modelToken, left, right, steps⟩
        else none
      else none
    else none
  else none

end NobleFoundation.CertificateChained
