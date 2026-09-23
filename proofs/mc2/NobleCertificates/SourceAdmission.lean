import NobleCertificates.SourceSupport

open Aeneas Aeneas.Std Result

namespace NobleCertificates.SourceAdmission

set_option maxHeartbeats 1000000
set_option maxRecDepth 4096
set_option Aeneas.customDoElab false

open NobleCertificates.SourceSupport

/-!
Closed source-evaluation equations for the actual extracted admission and
release transitions. These are not universal checker-soundness theorems.

The explicit external premise is that the isolated, sound MC1 host checker has
checked `MC1Proof.proof` for the two exact statement/declaration pairs below,
including its declaration type and dependency/axiom policy, and that the host
observation is authentic. `assumedAuthenticSoundHostObservationForNamedIncrement`
represents only those two concrete observations. The actual production
`begin_admission` and `complete_admission` compare the retained generated
statement, complete offered declaration bytes, evidence class and request
context before minting evidence. No effectful verifier dictionary is supplied.
Establishing semantic checker soundness or observation authenticity remains
outside this module.

The named sources and declaration bytes are the repository fixtures
`verification/mc2/contracts/{increment,guarded}.{contract,proof.lean}`. In
particular, the guarded declaration proves conditional monotonicity, not the
unguarded increment proposition. Native evaluation has the same separate
source-evidence role as `NobleContractImpl.Statement` in MC1.
-/

def incrementSource (guarded : Bool) : String :=
  if guarded then
    "(contract 1 conditional-monotonicity\n  (input (x I64))\n  (output (y I64))\n  (program [ 1 + ])\n  (requires (lt (in x) 9223372036854775807))\n  (ensures (lt (in x) (out y))))\n"
  else
    "(contract 1 increment\n  (input (x I64))\n  (output (y I64))\n  (program [ 1 + ])\n  (requires true)\n  (ensures (eq (out y) (add (in x) 1))))\n"

/-- Fixed expected exports, not an export compared with itself. -/
def incrementStatement (guarded : Bool) : String :=
  if guarded then
    "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .lit (.i64 (BitVec.ofInt 64 (1)))\ndef node_1 : Op := .word 4\ndef program : List Op := [node_0, node_1]\n\ndef inputTypes : List Ty := [.i64]\ndef outputTypes : List Ty := [.i64]\ndef paramTypes : List Ty := []\ndef expression_0 : Term := .input 0\ndef expression_1 : Term := .i64 (9223372036854775807)\ndef expression_2 : Term := .lt expression_0 expression_1\ndef expression_3 : Term := .input 0\ndef expression_4 : Term := .output 0\ndef expression_5 : Term := .lt expression_3 expression_4\ndef precondition : Term := expression_2\ndef postcondition : Term := expression_5\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"
  else
    "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .lit (.i64 (BitVec.ofInt 64 (1)))\ndef node_1 : Op := .word 4\ndef program : List Op := [node_0, node_1]\n\ndef inputTypes : List Ty := [.i64]\ndef outputTypes : List Ty := [.i64]\ndef paramTypes : List Ty := []\ndef expression_0 : Term := .bool true\ndef expression_1 : Term := .output 0\ndef expression_2 : Term := .input 0\ndef expression_3 : Term := .i64 (1)\ndef expression_4 : Term := .add expression_2 expression_3\ndef expression_5 : Term := .eq expression_1 expression_4\ndef precondition : Term := expression_0\ndef postcondition : Term := expression_5\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"

def incrementDeclaration (guarded : Bool) : String :=
  if guarded then
    "import MC1Obligation\n\nopen NobleContracts\n\nnamespace MC1Proof\n\n/-- Conditional monotonicity for the guarded increment contract: the program is\nthe same `[ 1 + ]`, but admission is guarded by the `LtI64Max` template, so the\ncaller only reaches the body with an argument below `i64::MAX`. On that domain\nwrapping addition is strictly increasing, which is exactly what the stated\npostcondition `lt (in x) (out y)` claims. The unguarded version of this contract\nis false at `i64::MAX`, which is why this proof needs the precondition. -/\ntheorem proof : MC1Obligation.claim := by\n  change exportedClaim increment [.i64] [.i64] []\n    (Holds (.lt (.input 0) (.i64 9223372036854775807)))\n    (Holds (.lt (.input 0) (.output 0)))\n  intro tail before params hi hp hpre final he\n  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi\n  refine ⟨[.i64 (n + 1)], (increment_exec_iff n tail final).mp he, .cons .i64 .nil, ?_⟩\n  -- The guard's precondition, read as an integer comparison on the argument.\n  have hn : n.toInt < 9223372036854775807 := by\n    simpa [Holds, evaluate, i64] using hpre\n  -- Adding one without signed overflow is strictly increasing.\n  have hmono : n.toInt < (n + 1).toInt := by\n    have h1 : BitVec.toInt (1 : BitVec 64) = 1 := by\n      rw [BitVec.toInt_ofNat]; simp\n    have hb : (n + 1).toInt = (n.toInt + 1).bmod 18446744073709551616 := by\n      rw [BitVec.toInt_add, h1]\n    rw [hb]\n    have hlo : -(9223372036854775808 : Int) ≤ n.toInt + 1 := by\n      have hbnd := BitVec.le_toInt n\n      omega\n    have hhi : n.toInt + 1 < (9223372036854775808 : Int) := by omega\n    rw [Int.bmod_eq_of_le hlo hhi]\n    omega\n  simpa [Holds, evaluate, i64] using hmono\n\nend MC1Proof\n"
  else
    "import MC1Obligation\n\nopen NobleContracts\n\nnamespace MC1Proof\n\ntheorem proof : MC1Obligation.claim := by\n  change exportedClaim increment [.i64] [.i64] []\n    (Holds (.bool true))\n    (Holds (.eq (.output 0) (.add (.input 0) (.i64 1))))\n  intro tail before params hi hp hpre final he\n  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi\n  refine ⟨[.i64 (n + 1)], (increment_exec_iff n tail final).mp he,\n    .cons .i64 .nil, ?_⟩\n  simp [Holds, evaluate]\n\nend MC1Proof\n"

def incrementDeclarationBytes (guarded : Bool) : alloc.vec.Vec U8 :=
  ⟨if guarded then toStr (incrementDeclaration true)
    else toStr (incrementDeclaration false)⟩

def incrementOffer (guarded : Bool) : noble_contracts.companion.admit.EvidenceOffer :=
  { «class» := .LeanExact
    refutation := false
    payload := .Declaration (incrementDeclarationBytes guarded) }

/-- The explicit soundness/authenticity premise concerns ONLY these two exact
named certificates. This is complete host observation data, not a callback or
an acceptance flag on an offer. Production completion must compare its fixed
statement/source/class against the real pending request. -/
def assumedAuthenticSoundHostObservationForNamedIncrement (guarded : Bool) :
    Result noble_contracts.companion.admit.CheckObservation :=
  noble_contracts.companion.admit.observation.CheckObservation.new
    (incrementStatement guarded) (incrementDeclarationBytes guarded) (.Ok .LeanExact)

/-- A concrete refusing host observation grants nothing, even with both exact
input bindings present. It requires no successful-proof soundness premise. -/
def refusingHostObservation : Result noble_contracts.companion.admit.CheckObservation :=
  noble_contracts.companion.admit.observation.CheckObservation.new
    (incrementStatement false) (incrementDeclarationBytes false) (.Err .ForgedStatus)

/-- Preparation errors remain failures of the source computation. -/
def preparedIncrement (guarded : Bool) : Result noble_contracts.Prepared := do
  let result ← prepare (if guarded then toStr (incrementSource true)
    else toStr (incrementSource false))
  match result with
  | .Ok prepared => ok prepared
  | .Err _ => fail .panic

def beginIncrement
    (guarded : Bool) (offer : noble_contracts.companion.admit.EvidenceOffer) :
    Result ((core.result.Result noble_contracts.companion.admit.AdmissionRequest
      noble_contracts.companion.admit.Admission) × noble_contracts.companion.Core) := do
  let prepared ← preparedIncrement guarded
  let engine ← fresh
  noble_contracts.companion.core.Core.begin_admission engine prepared offer

/-- Include ordinary ingress refusals when exercising begin's checks. -/
def checkedIncrementWith
    (observation : Result noble_contracts.companion.admit.CheckObservation)
    (guarded : Bool) (offer : noble_contracts.companion.admit.EvidenceOffer) :
    Result (noble_contracts.companion.admit.Admission × noble_contracts.companion.Core) := do
  let (request, engine) ← beginIncrement guarded offer
  match request with
  | .Err admission => ok (admission, engine)
  | .Ok request =>
    let observation ← observation
    noble_contracts.companion.core.Core.complete_admission engine request observation

/-- Completion-specific equations require a genuine pending request. An early
ingress refusal cannot accidentally stand in for a completion refusal. -/
def completeIncrementWith
    (observation : Result noble_contracts.companion.admit.CheckObservation)
    (guarded : Bool) (offer : noble_contracts.companion.admit.EvidenceOffer) :
    Result (noble_contracts.companion.admit.Admission × noble_contracts.companion.Core) := do
  let (request, engine) ← beginIncrement guarded offer
  match request with
  | .Err _ => fail .panic
  | .Ok request =>
    let observation ← observation
    noble_contracts.companion.core.Core.complete_admission engine request observation

/-- Actual preparation and two-stage admission, retaining the actual returned
Admission and Core unchanged. No failed admission is replaced with a fixture. -/
def acceptedIncrement (guarded : Bool) :
    Result (noble_contracts.companion.admit.Admission × noble_contracts.companion.Core) := do
  let (admission, engine) ← completeIncrementWith
    (assumedAuthenticSoundHostObservationForNamedIncrement guarded)
    guarded (incrementOffer guarded)
  match admission.outcome, admission.contract, admission.evidence, admission.refusals.val with
  | .Proved, some _, some _, [] => ok (admission, engine)
  | _, _, _, _ => fail .panic

/-- Inspect production entry status, exact retained statement, canonical program,
entry IDs, context and release identity. No registry or classifier is modeled. -/
def provedAndReleased (guarded : Bool) : Result Bool := do
  let (admission, engine) ← acceptedIncrement guarded
  let some contract := admission.contract | fail .panic
  let some evidence := admission.evidence | fail .panic
  let release ← noble_contracts.companion.core.Core.release engine evidence
  let correspondence ← noble_contracts.companion.core.Core.correspond engine evidence contract
  let program ← noble_contracts.companion.core.Core.contract_program engine contract
  let claim ← noble_contracts.companion.core.Core.claim engine evidence
  let rule ← noble_contracts.companion.core.Core.evidence_rule engine evidence
  match release, correspondence, program, rule,
      engine.registry.contracts.val, engine.registry.evidence.val with
  | .Ok released, .Ok (), .Ok events, .Ok none, [retained], [entry] =>
    let expectedClaim := match guarded, claim with
      | false, .Ok (.IncrementBy amount) => amount == 1#i64
      | true, .Ok (.Admitted statement) => statement == admission.statement_digest
      | _, _ => false
    match entry.outcome, entry.class, entry.subject.captures.val, entry.premises.val with
    | .Proved, .LeanExact, [], [] =>
      ok (expectedClaim && decide (
        contract = 0#u32 ∧ evidence = 0#u32 ∧
        retained.exact_statement = incrementStatement guarded ∧
        retained.statement = admission.statement_digest ∧
        entry.contract = contract ∧ entry.statement = admission.statement_digest ∧
        entry.subject.identity = admission.statement_digest ∧
        events.val = [(1#u32, 1#u64), (2#u32, 4#u64)] ∧
        retained.program.val = events.val ∧ entry.subject.events.val = events.val ∧
        entry.subject.input_signature = retained.input_signature ∧
        entry.subject.output_signature = retained.output_signature ∧
        engine.policy = 0#u32 ∧ engine.revision = 0#u32 ∧
        retained.policy = engine.policy ∧ retained.revision = engine.revision ∧
        entry.policy = engine.policy ∧ entry.revision = engine.revision ∧
        released.contract = contract ∧ released.evidence = evidence ∧
        released.statement = admission.statement_digest ∧
        released.subject = admission.statement_digest ∧
        released.policy = engine.policy ∧ released.ruleset = 1#u32))
    | _, _, _, _ => ok false
  | _, _, _, _, _, _ => ok false

/-- A retained contract is not accepted evidence: both release and claim lookup
must report UnknownEvidence after the exact refusal, with the registry empty of
proof entries and the original contract/digest still retained. -/
def refusedRetained
    (computation : Result (noble_contracts.companion.admit.Admission ×
      noble_contracts.companion.Core))
    (outcome : noble_contracts.companion.admit.Outcome)
    (refusal : noble_contracts.companion.Refusal) : Result Bool := do
  let (admission, engine) ← computation
  let release ← noble_contracts.companion.core.Core.release engine 0#u32
  let claim ← noble_contracts.companion.core.Core.claim engine 0#u32
  let outcomeMatches ←
    noble_contracts.companion.admit.Outcome.Insts.CoreCmpPartialEqOutcome.eq
      admission.outcome outcome
  match admission.contract, admission.evidence, admission.refusals.val,
      release, claim, engine.registry.contracts.val, engine.registry.evidence.val with
  | some contract, none, [actual], .Err .UnknownEvidence, .Err .UnknownEvidence,
      [retained], [] =>
    let refusalMatches ←
      noble_contracts.companion.Refusal.Insts.CoreCmpPartialEqRefusal.eq actual refusal
    ok (outcomeMatches && refusalMatches && decide (
      contract = 0#u32 ∧ retained.statement = admission.statement_digest))
  | _, _, _, _, _, _, _ => ok false

def rawClaimedProof :
    Result (noble_contracts.companion.admit.Admission × noble_contracts.companion.Core) := do
  let prepared ← preparedIncrement false
  let engine ← fresh
  noble_contracts.companion.core.Core.admit engine prepared (incrementOffer false)

/-- This altered source retains the same declaration name but changes both the
program and its proposition. The original authentic positive observation is
reused unchanged; production completion, not a local verifier, must reject the
different generated statement before minting. -/
def changedStatementAdmission :
    Result (noble_contracts.companion.admit.Admission × noble_contracts.companion.Core) := do
  let result ← prepare (toStr
    "(contract 1 increment (input (x I64)) (output (y I64)) (program [ 2 + ]) (requires true) (ensures (eq (out y) (add (in x) 2))))")
  match result with
  | .Err _ => fail .panic
  | .Ok prepared =>
    let engine ← fresh
    let (request, engine) ← noble_contracts.companion.core.Core.begin_admission
      engine prepared (incrementOffer false)
    match request with
    | .Err _ => fail .panic
    | .Ok request =>
      let observation ← assumedAuthenticSoundHostObservationForNamedIncrement false
      noble_contracts.companion.core.Core.complete_admission engine request observation

/-- Mutate the real consumer policy after beginning a request. Restoration
changes the policy back to zero but cannot roll its revision back to zero.
The same authentic observation still refers to the original exact certificate;
the refusal must come from production request-context validation. -/
def contextChangedAdmission (restorePolicy : Bool) :
    Result (noble_contracts.companion.admit.Admission × noble_contracts.companion.Core) := do
  let (request, engine) ← beginIncrement false (incrementOffer false)
  match request with
  | .Err _ => fail .panic
  | .Ok request =>
    let engine ← noble_contracts.companion.core.context.Core.set_policy engine 1#u32
    let engine ← if restorePolicy then
      noble_contracts.companion.core.context.Core.set_policy engine 0#u32
      else ok engine
    if engine.policy = (if restorePolicy then 0#u32 else 1#u32) ∧
        engine.revision = (if restorePolicy then 2#u32 else 1#u32) then
      let observation ← assumedAuthenticSoundHostObservationForNamedIncrement false
      noble_contracts.companion.core.Core.complete_admission engine request observation
    else
      fail .panic

/-- Capabilities must be refused before contract retention as well as before
proof minting. The begin entrypoint uses the real purity check on its
zero-declaration fallback; no local capability classifier is supplied. -/
def capabilityRefused
    (payload : noble_contracts.companion.admit.EvidencePayload) : Result Bool := do
  let (admission, engine) ← checkedIncrementWith
    (assumedAuthenticSoundHostObservationForNamedIncrement false) false
    { incrementOffer false with payload }
  let release ← noble_contracts.companion.core.Core.release engine 0#u32
  let program ← noble_contracts.companion.core.Core.contract_program engine 0#u32
  match admission.outcome, admission.contract, admission.evidence, admission.refusals.val,
      release, program, engine.registry.contracts.val, engine.registry.evidence.val with
  | .Error, none, none, [.LiveCapabilityInEvidence], .Err .UnknownEvidence,
      .Err .UnknownContract, [], [] => ok (admission.statement_digest == 0#u64)
  | _, _, _, _, _, _, _, _ => ok false

/-- Exact positive admission, including source recognition as IncrementBy(1),
and release of that evidence under its retained statement identity. -/
theorem checked_increment_is_proved_and_releasable :
    observed (provedAndReleased false) = true := by
  native_decide

/-- The conditional-monotonicity declaration is independently bound to its own
export; its retained claim is Admitted(statement), never unguarded IncrementBy. -/
theorem checked_guarded_increment_is_proved_and_releasable :
    observed (provedAndReleased true) = true := by
  native_decide

theorem raw_claimed_proof_cannot_grant_status :
    observed (refusedRetained rawClaimedProof .Error .ForgedStatus) = true := by
  native_decide

theorem checked_verifier_refusal_cannot_release :
    observed (refusedRetained
      (completeIncrementWith refusingHostObservation false (incrementOffer false))
      .Error .ForgedStatus) = true := by
  native_decide

theorem checked_wrong_evidence_class_cannot_release :
    observed (refusedRetained
      (checkedIncrementWith (assumedAuthenticSoundHostObservationForNamedIncrement false) false
        { incrementOffer false with «class» := .Assumption })
      .Unsupported .UnsupportedEvidenceClass) = true := by
  native_decide

theorem checked_refutation_class_mismatch_cannot_release :
    observed (refusedRetained
      (checkedIncrementWith (assumedAuthenticSoundHostObservationForNamedIncrement false) false
        { incrementOffer false with «class» := .LeanRefutation })
      .Error .WrongPremiseClass) = true := by
  native_decide

theorem checked_changed_declaration_cannot_release :
    observed (refusedRetained
      (completeIncrementWith (assumedAuthenticSoundHostObservationForNamedIncrement false)
        false (incrementOffer true))
      .Error .MismatchedClaim) = true := by
  native_decide

theorem checked_changed_statement_cannot_release :
    observed (refusedRetained changedStatementAdmission .Error .MismatchedClaim) = true := by
  native_decide

/-- The request's class/refutation pair is internally coherent and passes
begin, but an authentic LeanExact observation cannot mint a LeanRefutation. -/
theorem authentic_observation_wrong_class_cannot_mint :
    observed (refusedRetained
      (completeIncrementWith (assumedAuthenticSoundHostObservationForNamedIncrement false)
        false { incrementOffer false with «class» := .LeanRefutation, refutation := true })
      .Error .WrongPremiseClass) = true := by
  native_decide

theorem changed_policy_cannot_complete_pending_request :
    observed (refusedRetained (contextChangedAdmission false) .Error .StaleContext) = true := by
  native_decide

theorem restored_policy_cannot_complete_stale_request :
    observed (refusedRetained (contextChangedAdmission true) .Error .StaleContext) = true := by
  native_decide

theorem resource_evidence_is_refused_before_retention :
    observed (capabilityRefused (.Resource 7#u64)) = true := by
  native_decide

theorem service_capability_evidence_is_refused_before_retention :
    observed (capabilityRefused (.ServiceCapability 9#u64)) = true := by
  native_decide

end NobleCertificates.SourceAdmission
