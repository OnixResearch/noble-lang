import NobleContracts

/-!
# MC2 companion model

First-class companions are *guest values*: an inert description of a contract,
an evidence envelope and a certified pairing. The companion itself never carries
proof authority. Authority is a derivation (`Accepted`) over the same finite,
versioned rule set the deterministic Rust core replays (`NobleCertificates.Rules`).

The semantic layer is the reviewed MC1 normal-return model
(`NobleContracts.Exec`/`PC`). Composition therefore inherits MC1's partial
correctness meaning: a companion claim constrains normal returns only. Nothing
in this file strengthens partial correctness into total correctness, and nothing
constructs executable code.

This module defines only the model. Rule replay and its soundness theorems live
in `NobleCertificates.Rules`; the certified-status invariant lives in
`CertifiedInvariant`; the extracted-Rust correspondence lives in
`MC2ExtractionGate`. Model identity folds below are logical indexing devices,
not a claim that the 64-bit Rust digest is injective or uses this formula.
-/

open NobleContracts

namespace NobleCertificates

/-- A 64-bit identity/digest fold. Digests name subjects, claims and statements. -/
abbrev Digest := BitVec 64

/-- A wrapping scalar index for the logical evidence descriptions. This is not
the production FNV/avalanche encoding; no byte correspondence is asserted. -/
def mix (a b : BitVec 64) : BitVec 64 :=
  a * 1099511628211 + b + 14695981039346656037

/-- A compile-time capture slot bound to a runtime scalar. -/
structure CaptureBinding where
  slot : BitVec 32
  value : BitVec 64

/-- The observed subject. `identity` is the deterministic observation fold; the
signatures and captures are the endpoints and bindings the core must match.
`code` is the ordinary program the companion projects to. -/
structure Subject where
  identity : Digest
  inputSignature : BitVec 64
  outputSignature : BitVec 64
  captures : List CaptureBinding
  code : List Op

/-- Reference implementations of the two MC2 contract meanings. -/
inductive ClaimKind where
  /-- Normal-return partial correctness, exactly `NobleContracts.PC`. -/
  | partialCorrectness
  /-- The stated proposition is refuted (an accepted counterexample). -/
  | refutation

/-- A typed proposition and its exact statement digest. `pre`/`post` are the
semantic content; `statement` is the digest the evidence must reproduce. -/
structure Claim where
  statement : Digest
  kind : ClaimKind
  pre : Stack → Prop
  post : Stack → Stack → Prop

/-- The accompanied semantic context. Applicability is indexed by this context:
policy, ruleset version, semantic revision and the assumption list. -/
structure Context where
  policy : BitVec 64
  ruleset : BitVec 32
  semantic : BitVec 64
  assumptions : List Digest

/-- Evidence classes admitted by the profile. -/
inductive EvidenceClass where
  | leanExact
  | leanRefutation
  | replay
  | assumption

/-- An evidence *description*: completely inert. It has no accepted flag, no
mutable status field and no live capability. It names a contract, a subject, a
claim, a context and (for exact evidence) the retained declaration text. -/
structure Evidence where
  cls : EvidenceClass
  contract : BitVec 32
  subject : Subject
  claim : Claim
  context : Context
  declaration : Option String

/-- The semantic meaning of a partial-correctness claim about a subject: the
claim must carry the partial-correctness meaning, and the underlying program must
meet it as `NobleContracts.PC`. It never asserts that a normal result exists.
Refutation claims are recorded by `kind` and discharged separately. -/
def Satisfies (s : Subject) (c : Claim) (_ctx : Context) : Prop :=
  c.kind = .partialCorrectness ∧ PC s.code c.pre c.post

/-! ## Identity specification

The following conditional identity lemmas require an injective observation
index. They do not prove that a finite Rust hash is collision-free; production
binding must compare complete retained observations independently of the hash. -/

/-- An explicit injectivity hypothesis for logical subject indexing. This
property is not asserted of the production 64-bit digest. -/
def IdentityDetermines (admitted : Subject → Prop) (s : Subject) : Prop :=
  ∀ t : Subject, admitted t → t.identity = s.identity →
    t.code = s.code ∧ t.inputSignature = s.inputSignature ∧
    t.outputSignature = s.outputSignature ∧ t.captures = s.captures

/-- A changed observable (code, endpoint or capture) cannot share an identity.
VC-VALUE-02/VC-VALUE-04: two companions that differ in observation must differ. -/
theorem changed_capture_changes_identity {admitted : Subject → Prop} {s t : Subject}
    (hs : IdentityDetermines admitted s) (ht : admitted t)
    (hcapture : t.captures ≠ s.captures) : t.identity ≠ s.identity := by
  intro hid
  exact hcapture (hs t ht hid).2.2.2

theorem changed_code_changes_identity {admitted : Subject → Prop} {s t : Subject}
    (hs : IdentityDetermines admitted s) (ht : admitted t)
    (hcode : t.code ≠ s.code) : t.identity ≠ s.identity := by
  intro hid
  exact hcode (hs t ht hid).1

/-- Evidence replacement is not an observation. Changing or erasing the evidence
of a certified pairing leaves the observed subject identity untouched
(VC-VALUE-05). -/
theorem evidence_replacement_preserves_identity (s : Subject) (e e' : Evidence) :
    ({ e with subject := s }).subject.identity = ({ e' with subject := s }).subject.identity :=
  rfl

/-- The intermediate implication for composition, stated once. Composition is
only sound when the left postcondition establishes the right precondition. -/
def IntermediateImplication (leftPost : Stack → Stack → Prop) (rightPre : Stack → Prop) : Prop :=
  ∀ initial middle, leftPost initial middle → rightPre middle

/-- The relational composition of two postconditions: a normal return of the
first program reaching a normal return of the second. This is the *only* meaning
`composeV1` may attach to the derived claim, and it is the exact proposition
`NobleContracts.pc_sequence` transports premise soundness to. -/
def composedPost (leftPost rightPost : Stack → Stack → Prop) : Stack → Stack → Prop :=
  fun initial final => ∃ middle, leftPost initial middle ∧ rightPost middle final

end NobleCertificates
