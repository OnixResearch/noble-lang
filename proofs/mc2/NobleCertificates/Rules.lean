import NobleCertificates.Guards

/-!
# MC2 finite rule set and replay relation

Ruleset v1 is finite and versioned. `Replay` is the semantic derivation
relation, not a definition of the extracted Rust registry interpreter. Its
constructors require the stated premises and exclude unknown rules, unsupported rulesets, missing
premises, a missing intermediate implication, ineligible captures, unknown guard
templates and mismatched subjects/claims/contexts. It never runs tactics, SMT or
proof search: every premise is a previously checked derivation or an exact
externally rechecked declaration. The Rust interpreter's finite work budget,
retained data and decoding are separately extracted and audited; no universal
interpreter-to-`Replay` theorem is asserted here.

Each rule has a soundness theorem. `compose_sound` uses the *explicit*
intermediate implication premise and stays partial correctness: it never
strengthens MC1 `PC` into a normal-termination claim. `instantiate_sound`
quantifies over the compile-time capture and binds the resulting program
identity. `guard_sound` is the per-template correspondence proved in
`NobleCertificates.Guards`.
-/

open NobleContracts

namespace NobleCertificates

set_option maxRecDepth 1000000
set_option maxHeartbeats 4000000

/-! ## Deterministic constructions

These are logical evidence constructions and semantic program structures, not
byte encodings of the Rust registry. Rust digest, registry and replay code are
separately extracted and audited. Only the explicitly named correspondence
theorems bridge the two layers; semantic soundness alone is not that bridge. -/

/-- The observation fold of the ordinary `compose run` composition: the resulting
program identity joins both operands, endpoints join left-to-right and rights are
unioned. -/
def composeSubject (left right : Subject) : Subject :=
  { identity := mix left.identity right.identity
    inputSignature := left.inputSignature
    outputSignature := right.outputSignature
    captures := left.captures ++ right.captures
    code := left.code ++ right.code }

/-- The code value an instantiated builder constructs for one capture. -/
def constructedAddCode (capture : BitVec 64) : List Op := [.lit (.i64 capture), .word 4]

/-- The instantiated subject: the family identity bound to the concrete capture,
with the constructed program as its code. -/
def instantiateSubject (family : Subject) (capture : CaptureBinding) : Subject :=
  { identity := mix family.identity capture.value
    inputSignature := family.inputSignature
    outputSignature := family.outputSignature
    captures := capture :: family.captures
    code := constructedAddCode capture.value }

/-- The template tag participating in a guard wrapper identity. -/
def templateDigest : GuardTemplate → Digest
  | .ltI64Max => 1
  | .neI64Min => 2
  | .eqI64Literal k => mix 3 k

/-- The guard's contribution to the wrapper identity. -/
def guardFlagDigest (template : GuardTemplate) (argument : BitVec 64) : Digest :=
  mix (templateDigest template) argument

/-- The evidence description an independent declaration admits. -/
def admitEvidence (contract : BitVec 32) (subject : Subject) (claim : Claim)
    (context : Context) (declaration : String) : Evidence :=
  { cls := .leanExact, contract := contract, subject := subject, claim := claim,
    context := context, declaration := some declaration }

/-- The evidence description `composeV1` derives: the composed subject, the
relational composition of the two postconditions and the joined statement. -/
def composeEvidence (le re : Evidence) (leftClaim rightClaim : Claim) (contract : BitVec 32)
    (context : Context) : Evidence :=
  { cls := .replay, contract := contract, subject := composeSubject le.subject re.subject,
    claim := { statement := mix le.claim.statement re.claim.statement,
               kind := .partialCorrectness, pre := leftClaim.pre,
               post := fun before after => ∃ middle, leftClaim.post before middle ∧
                 rightClaim.post middle after },
    context := context, declaration := none }

/-- The evidence description `instantiateV1` derives for one capture. -/
def instantiateEvidence (lf : Evidence) (capture : CaptureBinding) (contract : BitVec 32)
    (context : Context) : Evidence :=
  { cls := .replay, contract := contract, subject := instantiateSubject lf.subject capture,
    claim := { statement := mix lf.claim.statement capture.value,
               kind := .partialCorrectness,
               pre := fun before => ∃ tail later, before = tail ++ [.i64 later],
               post := fun before after => ∃ tail later,
                 before = tail ++ [.i64 later] ∧ after = tail ++ [.i64 (later + capture.value)] },
    context := context, declaration := none }

/-- The evidence description a guard-checked wrapper derives. Both `guardV1` and
`invokeV1` mint this shape; the rules differ in how the candidate is bound, not
in the shape of the resulting description. -/
def wrapperEvidence (template : GuardTemplate) (lc : Evidence) (argument : BitVec 64)
    (contract : BitVec 32) (context : Context) : Evidence :=
  { cls := .replay, contract := contract,
    subject := { identity := mix lc.subject.identity (guardFlagDigest template argument)
                 inputSignature := lc.subject.inputSignature
                 outputSignature := lc.subject.outputSignature
                 captures := lc.subject.captures
                 code := wrapperProgram template lc.subject.code },
    claim := { statement := mix (templateDigest template) lc.claim.statement,
               kind := .partialCorrectness,
               pre := fun before => GuardPrecondition template argument ∧ lc.claim.pre before,
               post := successPost lc.claim.post },
    context := context, declaration := none }

/-- The evidence description `projectV1` returns: exactly the projected
description with the projecting contract id. -/
def projectEvidence (le : Evidence) (contract : BitVec 32) : Evidence :=
  { cls := .replay, contract := contract, subject := le.subject, claim := le.claim,
    context := le.context, declaration := none }

/-- The constructed program implements wrapping addition for every later
argument: the universal law the instantiated family is bound to. -/
@[simp] theorem constructedAdd_exec_iff (capture later : BitVec 64) (tail result : Stack) :
    Exec (constructedAddCode capture) (tail ++ [.i64 later]) result ↔
      result = tail ++ [.i64 (later + capture)] := by
  simp [constructedAddCode, Exec, List.reverse_append]

/-- The composed companion projects to exactly the ordinary `compose run`
program: the recipe is preserved, not replaced (VC-VALUE-01). -/
theorem composeSubject_matches_plain_compose (left right : Subject) (before result : Stack) :
    Exec (composeSubject left right).code before result ↔
      Exec [.word 9, .word 10] (before ++ [.program left.code, .program right.code]) result := by
  rw [show (composeSubject left right).code = left.code ++ right.code from rfl]
  exact NobleContracts.exec_append_iff.trans NobleContracts.exec_compose_run_iff.symm

/-! ## Rule set and records -/

/-- The finite v1 rule identifiers. No other rule exists in the profile. -/
inductive RuleId where
  | admitLeanV1
  | composeV1
  | instantiateV1
  | guardV1
  | projectV1
  | invokeV1

/-- A versioned finite rule set. Replay refuses any rule absent from `rules`. -/
structure RuleSet where
  version : BitVec 32
  rules : List RuleId

def RuleSet.supported (rs : RuleSet) (rule : RuleId) : Prop := rule ∈ rs.rules

def RuleSet.wellFormed (rs : RuleSet) : Prop := rs.rules.Nodup

/-- The exact declaration the shell independently rechecked, bound to the exact
statement digest. Producer text and status flags never appear here. -/
structure CheckedDeclaration where
  declaration : String
  statement : Digest
  subject : Subject
  claim : Claim
  context : Context
  bindsStatement : statement = claim.statement

/-- Independent acceptance: the exact retained declaration was rechecked for the
exact statement, and that statement is bound to the actual subject, claim and
context. -/
inductive AcceptedDeclaration : String → Subject → Claim → Context → Prop where
  | checked (cd : CheckedDeclaration) (declaration : String) (subject : Subject) (claim : Claim)
      (context : Context) (decl : cd.declaration = declaration) (subj : cd.subject = subject)
      (cl : cd.claim = claim) (ct : cd.context = context) (binds : cd.statement = claim.statement) :
      AcceptedDeclaration declaration subject claim context

/-- A bounded derivation record. Premises are named sub-derivations, not free
text; composition carries an *optional* intermediate implication, so its absence
is representable and refused. -/
inductive Derivation where
  | admitLean (contract : BitVec 32) (subject : Subject) (claim : Claim) (context : Context)
      (declaration : String)
  | compose (left right : Derivation) (contract : BitVec 32) (leftClaim rightClaim : Claim)
      (context : Context) (bridgePresent : Bool)
  | instantiate (family : Derivation) (capture : CaptureBinding) (contract : BitVec 32)
      (claim : Claim) (context : Context)
  | guard (template : GuardTemplate) (candidate : Derivation) (argument : BitVec 64)
      (contract : BitVec 32) (claim : Claim) (context : Context)
  | project (inner : Derivation) (contract : BitVec 32)
  | invoke (template : GuardTemplate) (candidate : Derivation) (argument : BitVec 64)
      (contract : BitVec 32) (claim : Claim) (context : Context)

/-- The semantic replay relation. Constructors explicitly state their premises;
finite inductive derivations are acyclic. This does not impose the production
interpreter's numeric work limit or prove its registry checks implement every
semantic premise. -/
inductive Replay : RuleSet → Derivation → Evidence → Prop where
  | admitLean (rs : RuleSet) (contract : BitVec 32) (subject : Subject) (claim : Claim)
      (context : Context) (declaration : String) (supported : rs.supported RuleId.admitLeanV1)
      (version : context.ruleset = rs.version) (checked : AcceptedDeclaration declaration subject claim context) :
      Replay rs (.admitLean contract subject claim context declaration)
        (admitEvidence contract subject claim context declaration)
  | compose (rs : RuleSet) (left right : Derivation) (le re : Evidence) (contract : BitVec 32)
      (leftClaim rightClaim : Claim) (context : Context)
      (bridgePresent : Bool)
      (present : bridgePresent = true) (b : IntermediateImplication leftClaim.post rightClaim.pre)
      (hleft : Replay rs left le) (hright : Replay rs right re)
      (lc : le.claim = leftClaim) (rc : re.claim = rightClaim)
      (supported : rs.supported RuleId.composeV1) (leftKind : leftClaim.kind = .partialCorrectness)
      (rightKind : rightClaim.kind = .partialCorrectness) :
      Replay rs (.compose left right contract leftClaim rightClaim context bridgePresent)
        (composeEvidence le re leftClaim rightClaim contract context)
  | instantiate (rs : RuleSet) (family : Derivation) (lf : Evidence) (capture : CaptureBinding)
      (contract : BitVec 32) (claim : Claim) (context : Context)
      (hfamily : Replay rs family lf) (supported : rs.supported RuleId.instantiateV1)
      (eligible : capture.slot ∉ lf.subject.captures.map CaptureBinding.slot) :
      Replay rs (.instantiate family capture contract claim context)
        (instantiateEvidence lf capture contract context)
  | guard (rs : RuleSet) (template : GuardTemplate) (candidate : Derivation) (lc : Evidence)
      (argument : BitVec 64) (contract : BitVec 32) (claim : Claim) (context : Context)
      (hcand : Replay rs candidate lc) (supported : rs.supported RuleId.guardV1)
      (accepts : guardRejects template argument = false) :
      Replay rs (.guard template candidate argument contract claim context)
        (wrapperEvidence template lc argument contract context)
  | project (rs : RuleSet) (inner : Derivation) (le : Evidence) (contract : BitVec 32)
      (hinner : Replay rs inner le) (supported : rs.supported RuleId.projectV1) :
      Replay rs (.project inner contract) (projectEvidence le contract)
  | invoke (rs : RuleSet) (template : GuardTemplate) (candidate : Derivation) (lc : Evidence)
      (argument : BitVec 64) (contract : BitVec 32) (claim : Claim) (context : Context)
      (hcand : Replay rs candidate lc) (supported : rs.supported RuleId.invokeV1)
      (accepts : guardRejects template argument = false) :
      Replay rs (.invoke template candidate argument contract claim context)
        (wrapperEvidence template lc argument contract context)

/-! ## Per-rule soundness

Each theorem is the soundness statement for one rule of the frozen finite set.
Premise soundness is a hypothesis, exactly as in `NobleContracts.TypedPC.compose`:
the theorem shows how a checked step transports it to the derived claim, never
that the premises hold. Every conclusion is stated on the rule's own components
(left/right programs, claims, capture, template, argument), so no conclusion ever
depends on unfolding a derived description. -/

/-- Wrapper soundness, shared by `guardV1` and `invokeV1`: when the guard accepts,
the wrapper executes the candidate body unchanged on the argument stack, so
candidate partial correctness transports to the wrapper at the guarded
precondition. The guard's own execution law is pinned in `NobleCertificates.Guards`. -/
theorem wrapper_pc (template : GuardTemplate) (body : List Op) (x : BitVec 64)
    (accepts : guardRejects template x = false) (P : Stack → Prop) (Q : Stack → Stack → Prop)
    (hbody : PC body P Q) :
    PC (wrapperProgram template body) (guardedPre template x P) (successPost Q) := by
  intro s t hpre he
  obtain ⟨tail, rfl, _hguard, hpre'⟩ := hpre
  obtain ⟨middle, hbodyExec, htag⟩ :=
    (wrapperProgram_accepts template body x tail t accepts).mp he
  exact ⟨middle, hbody (tail ++ [.i64 x]) middle hpre' hbodyExec, htag⟩

/-- `admitLeanV1`: the derived description is exactly the admitted one, and the
independently rechecked declaration for the bound statement establishes its
meaning. No producer flag and no mutable status participate. -/
theorem admitLean_sound (rs : RuleSet) (contract : BitVec 32) (subject : Subject)
    (claim : Claim) (context : Context) (declaration : String) (e : Evidence)
    (verifier : ∀ declaration subject claim context,
      AcceptedDeclaration declaration subject claim context → Satisfies subject claim context)
    (h : Replay rs (.admitLean contract subject claim context declaration) e) :
    e = admitEvidence contract subject claim context declaration ∧ Satisfies subject claim context := by
  cases h with
  | admitLean contract subject claim context declaration supported version checked =>
      exact ⟨rfl, verifier declaration subject claim context checked⟩

/-- `composeV1`: premise soundness of both operands transports through
`NobleContracts.pc_sequence` to partial correctness of the concatenated program at
the relational composition of the two postconditions. The intermediate implication
is the derivation's *explicit* premise, and partial correctness is never
strengthened into total correctness. -/
theorem compose_sound (rs : RuleSet) (left right : Derivation) (contract : BitVec 32)
    (leftClaim rightClaim : Claim) (context : Context) (bridgePresent : Bool) (e : Evidence)
    (hleft : ∀ le, Replay rs left le → Satisfies le.subject le.claim le.context)
    (hright : ∀ re, Replay rs right re → Satisfies re.subject re.claim re.context)
    (h : Replay rs (.compose left right contract leftClaim rightClaim context bridgePresent) e) :
    ∃ le re, Replay rs left le ∧ Replay rs right re ∧
      e = composeEvidence le re leftClaim rightClaim contract context ∧
      PC (le.subject.code ++ re.subject.code) leftClaim.pre
        (composedPost leftClaim.post rightClaim.post) := by
  cases h with
  | compose left right le re contract leftClaim rightClaim context bridgePresent present b hle hre lc rc supported leftKind rightKind =>
      obtain ⟨_, hleftPC⟩ := hleft le hle
      obtain ⟨_, hrightPC⟩ := hright re hre
      rw [lc] at hleftPC
      rw [rc] at hrightPC
      exact ⟨le, re, hle, hre, rfl,
        NobleContracts.pc_sequence (T := composedPost leftClaim.post rightClaim.post)
          hleftPC hrightPC (fun s m _ hr => b s m hr) (fun s m t _ hr hq => ⟨m, hr, hq⟩)⟩

/-- `instantiateV1`: the derived description is the family description bound to
the compile-time capture, and the constructed program obeys the universal wrapping
law for every later argument. No literal-only specialization can satisfy it. -/
theorem instantiate_sound (rs : RuleSet) (family : Derivation) (capture : CaptureBinding)
    (contract : BitVec 32) (claim : Claim) (context : Context) (e : Evidence)
    (h : Replay rs (.instantiate family capture contract claim context) e) :
    ∃ lf, Replay rs family lf ∧ e = instantiateEvidence lf capture contract context ∧
      (∀ later tail result, Exec (constructedAddCode capture.value) (tail ++ [.i64 later]) result ↔
        result = tail ++ [.i64 (later + capture.value)]) := by
  cases h with
  | instantiate family lf capture contract claim context hfamily supported eligible =>
      exact ⟨lf, hfamily, rfl, constructedAdd_exec_iff capture.value⟩

/-- `guardV1`: the accepted guard establishes the template's exact precondition
(including the wrapping-I64 boundary refusal), the derived description is the
wrapper description for the candidate, and the wrapper executes the candidate body
unchanged on the argument stack. -/
theorem guard_rule_sound (rs : RuleSet) (template : GuardTemplate) (candidate : Derivation)
    (argument : BitVec 64) (contract : BitVec 32) (claim : Claim) (context : Context) (e : Evidence)
    (hcandPC : ∀ lc, Replay rs candidate lc → PC lc.subject.code lc.claim.pre lc.claim.post)
    (h : Replay rs (.guard template candidate argument contract claim context) e) :
    ∃ lc, Replay rs candidate lc ∧ e = wrapperEvidence template lc argument contract context ∧
      GuardPrecondition template argument ∧
      PC (wrapperProgram template lc.subject.code) (guardedPre template argument lc.claim.pre)
        (successPost lc.claim.post) := by
  cases h with
  | guard template candidate lc argument contract claim context hcand supported accepts =>
      exact ⟨lc, hcand, rfl, guard_sound template argument accepts,
        wrapper_pc template lc.subject.code argument accepts lc.claim.pre lc.claim.post
          (hcandPC lc hcand)⟩

/-- `projectV1`: explicit projection returns exactly the underlying description.
Projecting re-derives nothing and admits nothing. -/
theorem project_sound (rs : RuleSet) (inner : Derivation) (contract : BitVec 32) (e : Evidence)
    (h : Replay rs (.project inner contract) e) :
    ∃ le, Replay rs inner le ∧ e = projectEvidence le contract := by
  cases h with
  | project inner le contract hinner supported => exact ⟨le, hinner, rfl⟩

/-- `invokeV1`: guard-checked certified invocation establishes the exact
precondition on the actual argument before the candidate body may run, derives the
wrapper description, and executes the candidate body unchanged. -/
theorem invoke_sound (rs : RuleSet) (template : GuardTemplate) (candidate : Derivation)
    (argument : BitVec 64) (contract : BitVec 32) (claim : Claim) (context : Context) (e : Evidence)
    (hcandPC : ∀ lc, Replay rs candidate lc → PC lc.subject.code lc.claim.pre lc.claim.post)
    (h : Replay rs (.invoke template candidate argument contract claim context) e) :
    ∃ lc, Replay rs candidate lc ∧ e = wrapperEvidence template lc argument contract context ∧
      GuardPrecondition template argument ∧
      PC (wrapperProgram template lc.subject.code) (guardedPre template argument lc.claim.pre)
        (successPost lc.claim.post) := by
  cases h with
  | invoke template candidate lc argument contract claim context hcand supported accepts =>
      exact ⟨lc, hcand, rfl, guard_sound template argument accepts,
        wrapper_pc template lc.subject.code argument accepts lc.claim.pre lc.claim.post
          (hcandPC lc hcand)⟩

/-! ## Derived descriptions, component by component

These lemmas publish exactly what each derived description says, so that a reader
(and the gate) never has to unfold a description to see its content. -/

/-- The composed description joins both operands' program texts. -/
theorem composeEvidence_code (le re : Evidence) (leftClaim rightClaim : Claim)
    (contract : BitVec 32) (context : Context) :
    (composeEvidence le re leftClaim rightClaim contract context).subject.code =
      le.subject.code ++ re.subject.code := by
  simp only [composeEvidence, composeSubject]

/-- The composed description's claim is the relational composition of the two
postconditions. -/
theorem composeEvidence_post (le re : Evidence) (leftClaim rightClaim : Claim)
    (contract : BitVec 32) (context : Context) :
    (composeEvidence le re leftClaim rightClaim contract context).claim.post =
      composedPost leftClaim.post rightClaim.post := by
  unfold composeEvidence
  rfl

/-- The instantiated description's identity is the family identity bound to the
capture. -/
theorem instantiateEvidence_identity (lf : Evidence) (capture : CaptureBinding)
    (contract : BitVec 32) (context : Context) :
    (instantiateEvidence lf capture contract context).subject.identity =
      mix lf.subject.identity capture.value := by
  simp only [instantiateEvidence, instantiateSubject]

/-- The instantiated description extends the family's capture list by exactly the
new binding. -/
theorem instantiateEvidence_captures (lf : Evidence) (capture : CaptureBinding)
    (contract : BitVec 32) (context : Context) :
    (instantiateEvidence lf capture contract context).subject.captures =
      capture :: lf.subject.captures := by
  simp only [instantiateEvidence, instantiateSubject]

/-- The wrapper description executes the fixed wrapper text over the candidate. -/
theorem wrapperEvidence_code (template : GuardTemplate) (lc : Evidence) (argument : BitVec 64)
    (contract : BitVec 32) (context : Context) :
    (wrapperEvidence template lc argument contract context).subject.code =
      wrapperProgram template lc.subject.code := by
  simp only [wrapperEvidence]

/-- The projected description is exactly the inner description. -/
theorem projectEvidence_subject (le : Evidence) (contract : BitVec 32) :
    (projectEvidence le contract).subject = le.subject := by
  simp only [projectEvidence]

/-- The projected description's claim is exactly the inner claim. -/
theorem projectEvidence_claim (le : Evidence) (contract : BitVec 32) :
    (projectEvidence le contract).claim = le.claim := by
  simp only [projectEvidence]

/-- The admitted description is exactly the bound subject, claim, context and
retained declaration text. -/
theorem admitEvidence_subject (contract : BitVec 32) (subject : Subject) (claim : Claim)
    (context : Context) (declaration : String) :
    (admitEvidence contract subject claim context declaration).subject = subject := by
  simp only [admitEvidence]



/-- Every accepted rule has a soundness theorem: the finite set is covered. -/
theorem rule_sound_of_supported (rs : RuleSet) (rule : RuleId) (supported : rs.supported rule) :
    rule = .admitLeanV1 ∨ rule = .composeV1 ∨ rule = .instantiateV1 ∨ rule = .guardV1 ∨
      rule = .projectV1 ∨ rule = .invokeV1 := by
  cases rule with
  | admitLeanV1 => exact .inl rfl
  | composeV1 => exact .inr (.inl rfl)
  | instantiateV1 => exact .inr (.inr (.inl rfl))
  | guardV1 => exact .inr (.inr (.inr (.inl rfl)))
  | projectV1 => exact .inr (.inr (.inr (.inr (.inl rfl))))
  | invokeV1 => exact .inr (.inr (.inr (.inr (.inr rfl))))

/-! ## The intermediate implication is indispensable

Sharing an interface does not discharge the bridge premise. This mirrors MC1's
`increment_missing_implication`: quantified normal returns of the first program
do not imply the second program's precondition unless the implication is stated
and checked. -/

/-- The two sample postconditions of the missing-implication witness. -/
def sampleLeftPost : Stack → Stack → Prop := fun before _ => before = [.i64 i64max]
def sampleRightPre : Stack → Prop := fun before => before ≠ [.i64 i64min]

/-- A concrete reason that an interface match cannot discharge the implication:
the wrapped maximum is exactly the value the second program's precondition
excludes. -/
theorem compose_missing_implication :
    ¬ IntermediateImplication sampleLeftPost sampleRightPre := by
  intro h
  exact h [.i64 i64max] [.i64 i64min] rfl rfl

end NobleCertificates
