import NobleCertificates.SourceSupport
import NobleCertificates.SourceAdmission

open Aeneas Aeneas.Std Result
open noble_contracts
open NobleCertificates.SourceSupport NobleCertificates.SourceAdmission

set_option maxHeartbeats 1000000
set_option maxRecDepth 4096
set_option Aeneas.customDoElab false

namespace NobleCertificates.SourceReplay

/-!
These closed equations execute extracted production transitions, starting with
`acceptedIncrement` and its exact-statement, externally checked certificate
boundary. They do not assert soundness of arbitrary host observations or
reconstruct an accepted registry. The only constructed evidence descriptions below are
untrusted replay requests; all retained authority comes from the concrete
`begin_admission`/`complete_admission` protocol, `derive_compose`, and `replay`.

The named fragment is the increment fixture, its two- and three-step composition,
and the conditional-monotonicity fixture with the `LtI64Max` guard. Native source
evaluation is separate from the strict universal rule/wrapper proofs. In
particular these equations do not establish universal interpreter refinement,
host-checker soundness, or correctness of every finite replay graph.
-/

/-- Unexpected preparation/admission/transition failure cannot supply a fixture. -/
private def requireOk {α : Type}
    (result : core.result.Result α companion.Refusal) : Result α :=
  match result with
  | .Ok value => ok value
  | .Err _ => fail .panic

/-- Read-only observation of a table populated by the actual production API. -/
private def retained (engine : companion.Core) (evidence : companion.registry.EvidenceId) :
    Result companion.registry.EvidenceEntry :=
  match engine.registry.evidence.val[evidence.val]? with
  | some entry => ok entry
  | none => fail .panic

private def admitted (guarded : Bool) :
    Result (companion.registry.EvidenceId × companion.registry.EvidenceEntry × companion.Core) := do
  let (admission, engine) ← acceptedIncrement guarded
  match admission.outcome, admission.contract, admission.evidence with
  | .Proved, some contract, some evidence =>
    let entry ← retained engine evidence
    if entry.contract == contract && entry.statement == admission.statement_digest then
      ok (evidence, entry, engine)
    else fail .panic
  | _, _, _ => fail .panic

private def refusedAs {α : Type}
    (result : core.result.Result α companion.Refusal) (expected : companion.Refusal) :
    Result Bool :=
  match result with
  | .Err actual => companion.Refusal.Insts.CoreCmpPartialEqRefusal.eq actual expected
  | .Ok _ => ok false

/-- A changed concrete program, independently folded through the public observer. -/
private def changedIncrement (subject : companion.Subject) :
    Result companion.Subject := do
  let events ← alloc.vec.Vec.push (alloc.vec.Vec.new (Std.U32 × Std.U64)) (1#u32, 2#u64)
  let events ← alloc.vec.Vec.push events (2#u32, 4#u64)
  companion.core.Core.observe (alloc.vec.Vec.deref events)
    { input := subject.input_signature, output := subject.output_signature }

/-- Typed composition may reuse an existing proposition twice. No new admission
or hand-built accepted entry is needed to obtain the second replay premise. -/
private def doubledIncrement :
    Result (companion.registry.EvidenceId × companion.registry.EvidenceEntry ×
      companion.rules.Derived × companion.Subject × companion.Core) := do
  let (evidence, entry, engine) ← admitted false
  let composed ← companion.core.Core.compose_subject entry.subject entry.subject
  let subject ← requireOk composed
  let (result, engine) ← companion.core.Core.derive_compose engine evidence evidence subject
  let derived ← requireOk result
  ok (evidence, entry, derived, subject, engine)

/-- Observe the minted status, recomputed claim and rule, exact applicability,
and complete release descriptor of one production composition transition. -/
private def certifiedComposition
    (engine : companion.Core) (evidence : companion.registry.EvidenceId)
    (subject : companion.Subject) (amount : Std.I64)
    (premises : List companion.registry.EvidenceId) : Result Bool := do
  let entry ← retained engine evidence
  let claimResult ← companion.core.Core.claim engine evidence
  let claim ← requireOk claimResult
  let ruleResult ← companion.core.Core.evidence_rule engine evidence
  let rule ← requireOk ruleResult
  let bindResult ← companion.core.Core.bind engine evidence subject
  let bound ← requireOk bindResult
  let sameSubject ← companion.Subject.Insts.CoreCmpPartialEqSubject.eq bound subject
  let releaseResult ← companion.core.Core.release engine evidence
  let released ← requireOk releaseResult
  let statement ← companion.admit.ClaimTemplate.digest (.IncrementBy amount)
  match entry.outcome, entry.class, claim, rule with
  | .Proved, .Replay, .IncrementBy actual, some .ComposeV1 =>
    ok (actual == amount && sameSubject && entry.premises.val == premises &&
      entry.statement == statement && released.statement == statement &&
      released.contract == entry.contract && released.evidence == evidence &&
      released.subject == subject.identity && released.policy == engine.policy &&
      released.ruleset == 1#u32)
  | _, _, _, _ => ok false

/-- The serialized replay has distinct roots, one of which itself depends twice
on the first root. This exercises the real finite ancestor traversal, not only
rule decoding or a manually populated evidence table. -/
def compositionReplay : Result Bool := do
  let (original, entry, twice, twiceSubject, engine) ← doubledIncrement
  let twiceCertified ← certifiedComposition engine twice.evidence twiceSubject 2#i64
    [original, original]
  let composed ← companion.core.Core.compose_subject entry.subject twiceSubject
  let thriceSubject ← requireOk composed
  let statement ← companion.admit.ClaimTemplate.digest (.IncrementBy 3#i64)
  let premises ← alloc.vec.Vec.push (alloc.vec.Vec.new companion.registry.EvidenceId) original
  let premises ← alloc.vec.Vec.push premises twice.evidence
  let (result, engine) ← companion.core.Core.replay engine {
    rule := .ComposeV1
    contract := entry.contract
    statement
    subject := thriceSubject
    premises
  }
  let thrice ← requireOk result
  let thriceCertified ← certifiedComposition engine thrice thriceSubject 3#i64
    [original, twice.evidence]
  let guards ← companion.core.Core.evidence_guard_templates engine thrice
  let noInventedGuard ← refusedAs guards .UnsupportedGuardTemplate
  ok (twiceCertified && thriceCertified && noInventedGuard)

theorem composition_replay_retains_certification : observed compositionReplay = true := by
  native_decide

/-- A cohesive rejected-replay sequence against the same real registry. Every
returned Core is passed to the next call. The final release checks both retained
authority and absence of authority at the would-be next evidence identifier. -/
def rejectedReplay : Result Bool := do
  let (original, entry, twice, twiceSubject, engine) ← doubledIncrement
  let empty := alloc.vec.Vec.new companion.registry.EvidenceId
  let single ← alloc.vec.Vec.push empty original
  let base : companion.rules.Derivation := {
    rule := .AdmitLeanV1
    contract := entry.contract
    statement := entry.statement
    subject := entry.subject
    premises := single
  }
  let next ← twice.evidence + 1#u32
  let future ← next + 1#u32
  let cyclic ← alloc.vec.Vec.push empty next
  let missing ← alloc.vec.Vec.push empty future
  let repeated ← alloc.vec.Vec.push single original
  let replayPremise ← alloc.vec.Vec.push empty twice.evidence
  let changed ← changedIncrement entry.subject
  let wrongStatement := if entry.statement == 0#u64 then 1#u64 else 0#u64
  let (zeroResult, engine) ← companion.core.Core.replay engine { base with premises := empty }
  let zeroRefused ← refusedAs zeroResult .MissingPremise
  let (cycleResult, engine) ← companion.core.Core.replay engine { base with premises := cyclic }
  let cycleRefused ← refusedAs cycleResult .CyclicDerivation
  let (missingResult, engine) ← companion.core.Core.replay engine { base with premises := missing }
  let missingRefused ← refusedAs missingResult .MissingPremise
  let (duplicateResult, engine) ← companion.core.Core.replay engine
    { base with rule := .ComposeV1, premises := repeated }
  let duplicateRefused ← refusedAs duplicateResult .DuplicatePremise
  let (subjectResult, engine) ← companion.core.Core.replay engine { base with subject := changed }
  let subjectRefused ← refusedAs subjectResult .MismatchedSubject
  let (claimResult, engine) ← companion.core.Core.replay engine { base with statement := wrongStatement }
  let claimRefused ← refusedAs claimResult .MismatchedClaim
  let (contextResult, engine) ← companion.core.Core.replay engine { base with contract := twice.contract }
  let contextRefused ← refusedAs contextResult .MismatchedContext
  let (classResult, engine) ← companion.core.Core.replay engine {
    base with
    contract := twice.contract
    statement := twice.statement_digest
    subject := twiceSubject
    premises := replayPremise
  }
  let classRefused ← refusedAs classResult .WrongPremiseClass
  let absentResult ← companion.core.Core.release engine next
  let absent ← refusedAs absentResult .UnknownEvidence
  let retainedCertified ← certifiedComposition engine twice.evidence twiceSubject 2#i64
    [original, original]
  ok (zeroRefused && cycleRefused && missingRefused && duplicateRefused &&
    subjectRefused && claimRefused && contextRefused && classRefused &&
    absent && retainedCertified)

theorem malformed_replay_refuses_without_minting : observed rejectedReplay = true := by
  native_decide

private def staleApplicationAndRelease (engine : companion.Core)
    (evidence : companion.registry.EvidenceId) (subject : companion.Subject) :
    Result Bool := do
  let bindResult ← companion.core.Core.bind engine evidence subject
  let bindRefused ← refusedAs bindResult .StaleContext
  let releaseResult ← companion.core.Core.release engine evidence
  let releaseRefused ← refusedAs releaseResult .StaleContext
  ok (bindRefused && releaseRefused)

/-- Exact retained and independently observed subjects bind to the canonical
subject; the same interface with a different literal does not. All four actual
context setters invalidate both applicability and release. Restoring the old
policy value cannot restore an old revision's authority. -/
def applicationTransitions : Result Bool := do
  let (evidence, entry, engine) ← admitted false
  let programResult ← companion.core.Core.contract_program engine entry.contract
  let program ← requireOk programResult
  let observation ← companion.core.Core.observe program
    { input := entry.subject.input_signature, output := entry.subject.output_signature }
  let exactResult ← companion.core.Core.bind engine evidence entry.subject
  let exact ← requireOk exactResult
  let observedResult ← companion.core.Core.bind engine evidence observation
  let boundObservation ← requireOk observedResult
  let exactCanonical ← companion.Subject.Insts.CoreCmpPartialEqSubject.eq exact entry.subject
  let observedCanonical ← companion.Subject.Insts.CoreCmpPartialEqSubject.eq
    boundObservation entry.subject
  let changed ← changedIncrement entry.subject
  let changedResult ← companion.core.Core.bind engine evidence changed
  let changedRefused ← refusedAs changedResult .MismatchedSubject
  let forgedIdentityResult ← companion.core.Core.bind engine evidence
    { changed with identity := entry.subject.identity }
  let forgedIdentityRefused ← refusedAs forgedIdentityResult .MismatchedSubject
  let unchanged ← companion.core.context.Core.set_policy engine engine.policy
  let unchangedBind ← companion.core.Core.bind unchanged evidence observation
  let unchangedBound ← requireOk unchangedBind
  let unchangedCanonical ← companion.Subject.Insts.CoreCmpPartialEqSubject.eq
    unchangedBound entry.subject
  let unchangedRelease ← companion.core.Core.release unchanged evidence
  let released ← requireOk unchangedRelease
  let policy ← companion.core.context.Core.set_policy engine 1#u32
  let policyStale ← staleApplicationAndRelease policy evidence observation
  let restored ← companion.core.context.Core.set_policy policy engine.policy
  let restoredStale ← staleApplicationAndRelease restored evidence observation
  let semantic ← companion.core.context.Core.set_semantic_revision engine 1#u32
  let semanticStale ← staleApplicationAndRelease semantic evidence observation
  let host ← companion.core.context.Core.set_host_contract engine 1#u64
  let hostStale ← staleApplicationAndRelease host evidence observation
  let environment ← companion.core.context.Core.set_environment_fact engine 1#u64
  let environmentStale ← staleApplicationAndRelease environment evidence observation
  ok (exactCanonical && observedCanonical && changedRefused && forgedIdentityRefused &&
    unchangedCanonical && released.evidence == evidence && released.contract == entry.contract &&
    released.statement == entry.statement && released.subject == entry.subject.identity &&
    policyStale && restoredStale && semanticStale && hostStale && environmentStale)

theorem exact_application_and_context_changes : observed applicationTransitions = true := by
  native_decide

/-- The complete guarded precondition selects exactly one known template. Its
actual source and live-invocation renderers have the literal rejection/acceptance
branch layouts used by the separate strict wrapper execution lemmas. This is a
renderer equation, not an assertion that rendering itself executes the guard. -/
def guardedSelection : Result Bool := do
  let (evidence, entry, engine) ← admitted true
  let contractResult ← companion.core.Core.guard_templates engine entry.contract
  let contractGuards ← requireOk contractResult
  let evidenceResult ← companion.core.Core.evidence_guard_templates engine evidence
  let evidenceGuards ← requireOk evidenceResult
  match contractGuards.val, evidenceGuards.val with
  | [.LtI64Max], [template@.LtI64Max] =>
    let source ← companion.core.Core.wrapper_source engine template (toStr "1 +")
    let live ← companion.core.Core.invocation_wrapper engine template
    let releaseResult ← companion.core.Core.release engine evidence
    let released ← requireOk releaseResult
    let stale ← companion.core.context.Core.set_policy engine 1#u32
    let staleContractResult ← companion.core.Core.guard_templates stale entry.contract
    let staleContract ← refusedAs staleContractResult .StaleContext
    let staleEvidenceResult ← companion.core.Core.evidence_guard_templates stale evidence
    let staleEvidence ← refusedAs staleEvidenceResult .StaleContext
    let staleReleaseResult ← companion.core.Core.release stale evidence
    let staleRelease ← refusedAs staleReleaseResult .StaleContext
    ok (source == "[ dup 9223372036854775807 = [ drop 1 inl ] [ [ 1 + ] run inr ] if ]" &&
      live == "[ swap dup 9223372036854775807 = [ drop drop 1 inl ] [ swap run inr ] if ]" &&
      released.evidence == evidence && released.contract == entry.contract &&
      released.statement == entry.statement && released.subject == entry.subject.identity &&
      staleContract && staleEvidence && staleRelease)
  | _, _ => ok false

theorem guarded_selection_and_wrapper_layout : observed guardedSelection = true := by
  native_decide

end NobleCertificates.SourceReplay
