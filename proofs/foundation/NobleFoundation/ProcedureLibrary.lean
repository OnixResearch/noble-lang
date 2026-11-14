/-!
# Learned procedure-library model (issue #118)

The encoder below is deliberately an abstract, typed canonical representation:
its fields model the exact immutable subject tuple, not Rust's byte serializer or
its experimental digest. The Rust persistence tests are the separate execution
and encoder-refinement lane. This model proves only injectivity and state-change
insensitivity of the abstract subject identity, deterministic reload at that
abstraction boundary, exact-generation append/deduplication properties, and
rollback restoration for a retained generation.
-/
namespace NobleFoundation.ProcedureLibrary

structure Dependency where
  procedureId : List Nat
  chunkId : List Nat
  sourceRevision : List Nat
  deriving DecidableEq, Repr

structure Subject where
  format : Nat
  semanticRevision : Nat
  encoderId : List Nat
  encodingRevision : Nat
  experimental : Bool
  body : List Nat
  sourceRevision : List Nat
  dependencies : List Dependency
  deriving DecidableEq, Repr

structure CanonicalSubject where
  format : Nat
  semanticRevision : Nat
  encoderId : List Nat
  encodingRevision : Nat
  experimental : Bool
  body : List Nat
  sourceRevision : List Nat
  dependencies : List Dependency
  deriving DecidableEq, Repr

structure Evidence where
  obligation : Nat
  subjectBinding : CanonicalSubject
  evidenceClass : Nat
  outcome : Nat
  deriving DecidableEq, Repr

structure State where
  evidence : List Evidence
  status : Nat
  cost : Nat
  deriving DecidableEq, Repr

structure Record where
  subject : Subject
  state : State
  deriving DecidableEq, Repr

/-- Abstract canonical encoding of exactly the immutable record fields. -/
def encodeSubject (subject : Subject) : CanonicalSubject := {
  format := subject.format
  semanticRevision := subject.semanticRevision
  encoderId := subject.encoderId
  encodingRevision := subject.encodingRevision
  experimental := subject.experimental
  body := subject.body
  sourceRevision := subject.sourceRevision
  dependencies := subject.dependencies
}

def decodeSubject (encoded : CanonicalSubject) : Subject := {
  format := encoded.format
  semanticRevision := encoded.semanticRevision
  encoderId := encoded.encoderId
  encodingRevision := encoded.encodingRevision
  experimental := encoded.experimental
  body := encoded.body
  sourceRevision := encoded.sourceRevision
  dependencies := encoded.dependencies
}

def identity (record : Record) : CanonicalSubject := encodeSubject record.subject

theorem decode_encode_subject (subject : Subject) : decodeSubject (encodeSubject subject) = subject := by
  cases subject <;> rfl

theorem encode_subject_injective : Function.Injective encodeSubject := by
  intro left right equal
  have decoded := congrArg decodeSubject equal
  simpa [decode_encode_subject] using decoded

theorem identity_injective_on_subjects {left right : Record}
    (equal : identity left = identity right) : left.subject = right.subject := by
  apply encode_subject_injective
  exact equal

theorem identity_binds_body {left right : Record}
    (equal : identity left = identity right) : left.subject.body = right.subject.body := by
  exact congrArg Subject.body (identity_injective_on_subjects equal)

theorem identity_binds_dependency_closure {left right : Record}
    (equal : identity left = identity right) :
    left.subject.dependencies = right.subject.dependencies := by
  exact congrArg Subject.dependencies (identity_injective_on_subjects equal)

theorem identity_ignores_mutable_state (subject : Subject) (leftState rightState : State) :
    identity ⟨subject, leftState⟩ = identity ⟨subject, rightState⟩ := rfl

structure PersistedRecord where
  immutableSubject : CanonicalSubject
  mutableState : State
  deriving DecidableEq, Repr

def persistRecord (record : Record) : PersistedRecord := {
  immutableSubject := encodeSubject record.subject
  mutableState := record.state
}

def reloadRecord (persisted : PersistedRecord) : Record := {
  subject := decodeSubject persisted.immutableSubject
  state := persisted.mutableState
}

def identityList (records : List Record) : List CanonicalSubject := records.map identity

def reloadIdentities (persisted : List PersistedRecord) : List CanonicalSubject :=
  identityList (persisted.map reloadRecord)

theorem reload_recomputes_identity (records : List Record) :
    reloadIdentities (records.map persistRecord) = identityList records := by
  simp [reloadIdentities, identityList, reloadRecord, persistRecord, identity,
    decodeSubject, encodeSubject]

theorem reload_repeated_identity (record : Record) :
    identity (reloadRecord (persistRecord (reloadRecord (persistRecord record)))) =
      identity record := by
  simp [reloadRecord, persistRecord, identity, decodeSubject, encodeSubject]

def containsIdentity (records : List Record) (key : CanonicalSubject) : Bool :=
  records.any (fun record => identity record == key)

def deduplicate (records : List Record) (incoming : Record) : List Record :=
  if containsIdentity records (identity incoming) then records else records ++ [incoming]

theorem deduplicate_reuses_exact_subject (records : List Record) (incoming : Record)
    (present : containsIdentity records (identity incoming) = true) :
    deduplicate records incoming = records := by
  simp [deduplicate, present]

theorem deduplicate_reuse_preserves_dependency_closures (records : List Record)
    (incoming : Record)
    (present : containsIdentity records (identity incoming) = true) :
    (deduplicate records incoming).map (fun record => record.subject.dependencies) =
      records.map (fun record => record.subject.dependencies) := by
  rw [deduplicate_reuses_exact_subject records incoming present]

theorem deduplicate_reuse_preserves_all_evidence (records : List Record)
    (incoming : Record)
    (present : containsIdentity records (identity incoming) = true) :
    (deduplicate records incoming).map (fun record => record.state.evidence) =
      records.map (fun record => record.state.evidence) := by
  rw [deduplicate_reuses_exact_subject records incoming present]

def appendEvidence (record : Record) (evidence : Evidence) : Record := {
  record with
  state := { record.state with evidence := record.state.evidence ++ [evidence] }
}

def appendBoundEvidence (record : Record) (evidence : Evidence) : Option Record :=
  if evidence.subjectBinding == identity record then some (appendEvidence record evidence) else none

theorem bound_evidence_appends_to_existing_evidence (record : Record) (evidence : Evidence)
    (bound : evidence.subjectBinding = identity record) :
    appendBoundEvidence record evidence = some (appendEvidence record evidence) := by
  simp [appendBoundEvidence, bound]

theorem appended_evidence_preserves_identity (record : Record) (evidence : Evidence) :
    identity (appendEvidence record evidence) = identity record := rfl

theorem appended_evidence_preserves_prior_obligations (record : Record) (evidence : Evidence) :
    (appendEvidence record evidence).state.evidence = record.state.evidence ++ [evidence] := rfl

structure Generation where
  number : Nat
  records : List Record
  deriving DecidableEq, Repr

structure History where
  prior : List Generation
  current : Generation
  deriving DecidableEq, Repr

def publish (history : History) (records : List Record) : History := {
  prior := history.prior ++ [history.current]
  current := { number := history.current.number + 1, records := records }
}

def publishBoundEvidence (history : History) (record : Record) (evidence : Evidence) : History :=
  if evidence.subjectBinding == identity record then
    publish history (history.current.records.map fun retained =>
      if identity retained == identity record then appendEvidence retained evidence else retained)
  else history

theorem evidence_publication_appends_a_generation (history : History) (record : Record)
    (evidence : Evidence) (bound : evidence.subjectBinding = identity record) :
    (publishBoundEvidence history record evidence).current.number = history.current.number + 1 := by
  simp [publishBoundEvidence, publish, bound]

theorem evidence_publication_retains_prior_generation (history : History) (record : Record)
    (evidence : Evidence) (bound : evidence.subjectBinding = identity record) :
    (publishBoundEvidence history record evidence).prior = history.prior ++ [history.current] := by
  simp [publishBoundEvidence, publish, bound]

theorem wrong_subject_evidence_does_not_publish (history : History) (record : Record)
    (evidence : Evidence) (wrong : evidence.subjectBinding ≠ identity record) :
    publishBoundEvidence history record evidence = history := by
  simp [publishBoundEvidence, wrong]

def rollback (history : History) (retained : Generation)
    (_available : retained ∈ history.prior ++ [history.current]) : History := {
  prior := history.prior ++ [history.current]
  current := { number := history.current.number + 1, records := retained.records }
}

theorem rollback_restores_exact_retained_closure (history : History) (retained : Generation)
    (available : retained ∈ history.prior ++ [history.current]) :
    (rollback history retained available).current.records = retained.records := rfl

theorem rollback_never_rewrites_retained_history (history : History) (retained : Generation)
    (available : retained ∈ history.prior ++ [history.current]) :
    (rollback history retained available).prior = history.prior ++ [history.current] := rfl

end NobleFoundation.ProcedureLibrary
