import NobleCertificates.Rules

/-!
# MC2 certified-status invariant (PO-20)

`Certified` pairs an inert evidence description with a *proof* that the
evidence is accepted. There is no status field, no mutable flag and no
deserializer path that establishes certified status: the only construction rule
is `Accepted`, whose two cases are independent acceptance of an exact declaration
and a checked derivation from the finite rule set.

Applicability is indexed by the exact context. A changed policy, semantic
revision or premise set does not transfer: the old acceptance still means what it
meant, but it is no longer applicable until revalidated. Serialized records carry
a producer flag which is demonstrably insufficient.
-/

open NobleContracts

namespace NobleCertificates

/-- Independent acceptance of an offered evidence description: the retained
declaration was rechecked for the exact statement bound to the subject, claim and
context. -/
def IndependentlyAccepted (e : Evidence) : Prop :=
  ∃ declaration, e.declaration = some declaration ∧
    AcceptedDeclaration declaration e.subject e.claim e.context

/-- The only way to obtain certified status. -/
inductive Accepted : Evidence → Prop where
  /-- An independent recheck of the exact declaration. -/
  | independent {e : Evidence} (h : IndependentlyAccepted e) : Accepted e
  /-- A checked derivation from the finite, versioned rule set. -/
  | derived {rs : RuleSet} {d : Derivation} {e : Evidence} (h : Replay rs d e) : Accepted e

/-- A certified pairing. The only field establishing status is the acceptance
proof; there is no boolean and no constructor that supplies one. -/
structure Certified where
  evidence : Evidence
  accepted : Accepted evidence

/-- Every construction path preserves the invariant: possession of a `Certified`
yields acceptance of its evidence. A record constructor cannot fabricate this. -/
theorem certified_evidence_accepted (c : Certified) : Accepted c.evidence := c.accepted

/-- Accepted status has exactly one of two provenances: independent acceptance or
a checked derivation. Nothing else (not a status field, deserializer or producer
policy) can produce it. -/
theorem accepted_cases {e : Evidence} (h : Accepted e) :
    IndependentlyAccepted e ∨ ∃ rs d, Replay rs d e := by
  cases h with
  | independent h => exact .inl h
  | derived h => exact .inr ⟨_, _, h⟩

/-- Every rule replay yields exactly one of two evidence classes: an admitted
description carrying its independently checked declaration, or a derived
description of one of the five deriving rules. No other class can come from
replay, and none of them is a host-supplied status. -/
theorem replay_evidence_class {rs : RuleSet} {d : Derivation} {e : Evidence}
    (h : Replay rs d e) : e.cls = .leanExact ∨ e.cls = .replay := by
  cases h <;>
    simp only [admitEvidence, composeEvidence, instantiateEvidence, wrapperEvidence, projectEvidence, true_or, or_true]

/-- A serialized record with a producer-supplied flag. Import must not trust it. -/
structure Record where
  evidence : Evidence
  suppliedFlag : Bool

/-- The producer flag is not evidence: a record can carry `true` on an evidence
description that is not accepted at all. This is the concrete refusal of
"serialized certified flags are ignored on import". -/
theorem flag_does_not_establish_certified :
    ∃ r : Record, r.suppliedFlag = true ∧ ¬ Accepted r.evidence := by
  let witness : Evidence :=
    { cls := .assumption, contract := 0, context := ⟨0, 0, 0, []⟩, declaration := none,
      subject := { identity := 0, inputSignature := 0, outputSignature := 0, captures := [],
                   code := [] },
      claim := { statement := 0, kind := .refutation, pre := fun _ => True,
                 post := fun _ _ => True } }
  refine ⟨⟨witness, true⟩, rfl, ?_⟩
  intro h
  rcases accepted_cases h with hindependent | ⟨rs, d, hreplay⟩
  · obtain ⟨declaration, hdecl, _⟩ := hindependent
    cases hdecl
  · rcases replay_evidence_class hreplay with hcls | hcls <;> cases hcls

/-- Applicability of an evidence description under an exact context. -/
def Applicable (e : Evidence) (ctx : Context) : Prop := e.context = ctx ∧ Accepted e

/-- A changed policy context invalidates applicability until revalidation. The
prior acceptance is untouched; it simply does not apply to the new context. -/
theorem changed_context_invalidates {e : Evidence} {ctx ctx' : Context}
    (bound : e.context = ctx) (changed : ctx' ≠ ctx) : ¬ Applicable e ctx' := by
  intro h
  exact changed (h.1.symm.trans bound)

/-- A changed semantic revision invalidates applicability in the same way; the
policy and semantic revisions are separate fields, not one opaque stamp. -/
theorem changed_semantic_invalidates {e : Evidence} {ctx ctx' : Context}
    (bound : e.context = ctx) (changed : ctx'.semantic ≠ ctx.semantic) :
    ¬ Applicable e ctx' :=
  changed_context_invalidates bound (fun h => changed (congrArg Context.semantic h))

/-- Explicit projection returns the underlying subject and no admission. -/
def Certified.project (c : Certified) : Subject := c.evidence.subject

/-- Projection preserves the underlying program identity and recipe
(VC-VALUE-01/VC-VALUE-05): the projection is exactly the certified subject. -/
theorem project_preserves_subject (c : Certified) :
    c.project.identity = c.evidence.subject.identity ∧
      c.project.code = c.evidence.subject.code ∧
      c.project.captures = c.evidence.subject.captures := ⟨rfl, rfl, rfl⟩

/-- Evidence erasure keeps the ordinary observation and discards no executable
check: the wrapper code (which contains the guard) is part of the subject code,
so erasing the proof text leaves the guarded program intact. -/
def eraseEvidence (c : Certified) : Subject := c.evidence.subject

theorem erasure_preserves_observation (c : Certified) :
    (eraseEvidence c).identity = c.evidence.subject.identity ∧
      (eraseEvidence c).code = c.evidence.subject.code := ⟨rfl, rfl⟩

/-- Attaching a *different accepted* evidence to the same subject never changes
the subject identity: proof metadata is separate from program identity. -/
theorem evidence_attachment_preserves_identity {e e' : Evidence}
    (sameSubject : e'.subject = e.subject) (h : Accepted e') :
    ({ evidence := e', accepted := h } : Certified).project.identity = e.subject.identity := by
  rw [Certified.project, sameSubject]

end NobleCertificates
