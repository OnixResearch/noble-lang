import Std

namespace NobleFoundation.GuardedRewrite

abbrev Bits := BitVec 64

inductive Relation where
  | equality
  | scopedEquality
  | directedRefinement
  deriving DecidableEq, Repr

inductive Rule where
  | scoped
  | preflight
  deriving DecidableEq, Repr

inductive BinaryOp where
  | add
  | sub
  | mul
  deriving DecidableEq, Repr

def BinaryOp.apply : BinaryOp → Bits → Bits → Bits
  | .add, first, second => first + second
  | .sub, first, second => first - second
  | .mul, first, second => first * second

inductive Operand where
  | input (slot : Nat)
  | constant (value : Bits)
  deriving DecidableEq, Repr

inductive Node where
  | binary (operation : BinaryOp) (first second : Operand)
  | constant (value : Bits)
  | unsupported
  deriving DecidableEq, Repr

inductive GuardValue where
  | operand (value : Operand)
  | result
  deriving DecidableEq, Repr

structure GuardBefore where
  node : Nat
  value : Operand
  exact : Bits
  deriving DecidableEq, Repr

structure GuardAfter where
  node : Nat
  value : GuardValue
  exact : Bits
  deriving DecidableEq, Repr

structure InputFact where
  slot : Nat
  exact : Bits
  deriving DecidableEq, Repr

inductive Output where
  | result
  | input (slot : Nat)
  | constant (value : Bits)
  deriving DecidableEq, Repr

structure Graph where
  inputCount : Nat
  inputFacts : List InputFact
  target : Nat
  node : Node
  before : List GuardBefore
  after : List GuardAfter
  outputs : List Output
  deriving DecidableEq, Repr

inductive Scope where
  | constant
  | inputFact (fact : InputFact)
  | guard (fact : GuardBefore)
  deriving DecidableEq, Repr

structure Step where
  rule : Rule
  node : Nat
  first : Operand
  second : Operand
  firstScope : Scope
  secondScope : Scope
  firstValue : Bits
  secondValue : Bits
  claimed : Bits
  deriving DecidableEq, Repr

structure Derivation where
  revision : Nat
  relation : Relation
  steps : List Step
  deriving DecidableEq, Repr

inductive Refusal where
  | staleRevision
  | malformed
  | scopeUnavailable
  | scopeValueMismatch
  | scopeConflict
  | scopeKindMismatch
  | claimMismatch
  | interfaceChanged
  | resultMismatch
  deriving DecidableEq, Repr

inductive Unsupported where
  | relation (kind : Relation)
  | operation
  deriving DecidableEq, Repr

inductive Exhausted where
  | steps
  deriving DecidableEq, Repr

inductive Outcome where
  | accepted
  | refused (reason : Refusal)
  | unsupported (reason : Unsupported)
  | exhausted (reason : Exhausted)
  | cancelled
  deriving DecidableEq, Repr

inductive Observation where
  | refused
  | guardFailed (node : Nat) (exact : Bits)
  | normal (outputs : List Bits)
  deriving DecidableEq, Repr

def revision : Nat := 0

def maxSteps : Nat := 1

def operandValid (inputCount : Nat) : Operand → Bool
  | .input slot => decide (slot < inputCount)
  | .constant _ => true

def guardValueValid (inputCount : Nat) : GuardValue → Bool
  | .operand value => operandValid inputCount value
  | .result => true

def strictlyIncreasing : List Nat → Bool
  | [] => true
  | [_] => true
  | first :: second :: rest => decide (first < second) && strictlyIncreasing (second :: rest)

def wellFormed (graph : Graph) : Bool :=
  let nodeOperandsValid := match graph.node with
    | .binary _ first second => operandValid graph.inputCount first && operandValid graph.inputCount second
    | _ => true
  let beforeValid :=
    graph.before.all (fun guard => decide (guard.node < graph.target) && operandValid graph.inputCount guard.value)
      && strictlyIncreasing (graph.before.map GuardBefore.node)
  let afterValid :=
    graph.after.all (fun guard => decide (graph.target < guard.node) && guardValueValid graph.inputCount guard.value)
      && strictlyIncreasing (graph.after.map GuardAfter.node)
  let factsValid := graph.inputFacts.all (fun fact => decide (fact.slot < graph.inputCount))
  let outputsValid := graph.outputs.all (fun output =>
    match output with
    | .input slot => decide (slot < graph.inputCount)
    | _ => true)
  nodeOperandsValid && beforeValid && afterValid && factsValid && outputsValid

def sameInterface (left right : Graph) : Bool := decide (
  left.inputCount = right.inputCount ∧
  left.inputFacts = right.inputFacts ∧
  left.target = right.target ∧
  left.before = right.before ∧
  left.after = right.after ∧
  left.outputs = right.outputs)

def inputFactsHold (facts : List InputFact) (inputs : Nat → Bits) : Bool :=
  facts.all (fun fact => decide (inputs fact.slot = fact.exact))

def operandValue (inputs : Nat → Bits) : Operand → Bits
  | .input slot => inputs slot
  | .constant value => value

def inputEvidence : List InputFact → Nat → List Bits
  | [], _ => []
  | fact :: rest, slot =>
      (if fact.slot = slot then [fact.exact] else []) ++ inputEvidence rest slot

def nodeValue (graph : Graph) (inputs : Nat → Bits) : Option Bits :=
  match graph.node with
  | .binary operation first second =>
      some (operation.apply (operandValue inputs first) (operandValue inputs second))
  | .constant value => some value
  | .unsupported => none

def guardBeforeFails (inputs : Nat → Bits) (guard : GuardBefore) : Bool :=
  decide (operandValue inputs guard.value ≠ guard.exact)

def firstBeforeFailure (guards : List GuardBefore) (inputs : Nat → Bits) : Option (Nat × Bits) :=
  match guards with
  | [] => none
  | guard :: rest =>
      if guardBeforeFails inputs guard then some (guard.node, guard.exact)
      else firstBeforeFailure rest inputs

def guardAfterValue (graph : Graph) (inputs : Nat → Bits) : GuardValue → Option Bits
  | .operand value => some (operandValue inputs value)
  | .result => nodeValue graph inputs

def guardAfterFails (graph : Graph) (inputs : Nat → Bits) (guard : GuardAfter) : Bool :=
  match guardAfterValue graph inputs guard.value with
  | none => true
  | some value => decide (value ≠ guard.exact)

def firstAfterFailure (graph : Graph) (inputs : Nat → Bits) (guards : List GuardAfter) : Option (Nat × Bits) :=
  match guards with
  | [] => none
  | guard :: rest =>
      if guardAfterFails graph inputs guard then some (guard.node, guard.exact)
      else firstAfterFailure graph inputs rest

def outputValue (graph : Graph) (inputs : Nat → Bits) : Output → Bits
  | .result => (nodeValue graph inputs).getD 0
  | .input slot => inputs slot
  | .constant value => value

def observe (graph : Graph) (inputs : Nat → Bits) : Observation :=
  if !inputFactsHold graph.inputFacts inputs then
    .refused
  else
    match firstBeforeFailure graph.before inputs with
    | some (node, exact) => .guardFailed node exact
    | none =>
        match graph.node with
        | .unsupported => .refused
        | _ =>
            match firstAfterFailure graph inputs graph.after with
            | some (node, exact) => .guardFailed node exact
            | none => .normal (graph.outputs.map (outputValue graph inputs))

def scopeValue (graph : Graph) (operand : Operand) : Scope → Option Bits
  | .constant =>
      match operand with
      | .constant value => some value
      | .input _ => none
  | .inputFact fact =>
      match operand with
      | .constant _ => none
      | .input slot =>
          if decide (fact.slot = slot ∧ fact ∈ graph.inputFacts) then some fact.exact else none
  | .guard fact =>
      if decide (fact.node < graph.target ∧ fact.value = operand ∧ fact ∈ graph.before) then
        some fact.exact
      else none

def evidenceValues (graph : Graph) (operand : Operand) : List Bits :=
  let initial := match operand with
    | .constant value => [value]
    | .input slot => inputEvidence graph.inputFacts slot
  initial ++ (graph.before.filterMap (fun guard =>
    if decide (guard.value = operand) then some guard.exact else none))

def evidenceAgrees (graph : Graph) (operand : Operand) (value : Bits) : Bool :=
  (evidenceValues graph operand).all (fun evidence => decide (evidence = value))

def usesGuard (scope : Scope) : Bool :=
  match scope with
  | .guard _ => true
  | _ => false

def verifyOperand (graph : Graph) (operand : Operand) (scope : Scope) (value : Bits) : Except Refusal Unit :=
  match scopeValue graph operand scope with
  | none => .error .scopeUnavailable
  | some available =>
      if available = value then
        if evidenceAgrees graph operand value then .ok () else .error .scopeConflict
      else .error .scopeValueMismatch

def ruleMatches (step : Step) : Bool :=
  let hasGuard := usesGuard step.firstScope || usesGuard step.secondScope
  match step.rule with
  | .scoped => hasGuard
  | .preflight => !hasGuard

def verifyStep (left right : Graph) (step : Step) : Except Refusal Unit :=
  match left.node with
  | .unsupported => .error .malformed
  | .constant _ => .error .malformed
  | .binary operation first second =>
      if step.node = left.target ∧ step.first = first ∧ step.second = second then
        match verifyOperand left first step.firstScope step.firstValue with
        | .error reason => .error reason
        | .ok _ =>
            match verifyOperand left second step.secondScope step.secondValue with
            | .error reason => .error reason
            | .ok _ =>
                if !ruleMatches step then .error .scopeKindMismatch
                else if step.claimed = operation.apply step.firstValue step.secondValue then
                  if right.node = .constant step.claimed then .ok () else .error .resultMismatch
                else .error .claimMismatch
      else .error .malformed

private theorem verifyOperand_sound (graph : Graph) (operand : Operand) (scope : Scope)
    (value : Bits) (verified : verifyOperand graph operand scope value = .ok ()) :
    scopeValue graph operand scope = some value := by
  unfold verifyOperand at verified
  cases availableProof : scopeValue graph operand scope with
  | none => simp [availableProof] at verified
  | some available =>
      by_cases hvalue : available = value
      · cases hagrees : evidenceAgrees graph operand value with
        | false => simp [availableProof, hvalue, hagrees] at verified
        | true =>
          simp [availableProof, hvalue, hagrees] at verified
          exact congrArg some hvalue
      · simp [availableProof, hvalue] at verified

private theorem verifyStep_sound (left right : Graph) (step : Step)
    (verified : verifyStep left right step = .ok ()) :
    ∃ operation first second,
      left.node = .binary operation first second ∧
      step.node = left.target ∧ step.first = first ∧ step.second = second ∧
      scopeValue left first step.firstScope = some step.firstValue ∧
      scopeValue left second step.secondScope = some step.secondValue ∧
      step.claimed = operation.apply step.firstValue step.secondValue ∧
      right.node = .constant step.claimed := by
  cases hnode : left.node with
  | unsupported => simp [verifyStep, hnode] at verified
  | constant bits => simp [verifyStep, hnode] at verified
  | binary operation first second =>
      by_cases hfields : step.node = left.target ∧ step.first = first ∧ step.second = second
      · cases hFirst : verifyOperand left first step.firstScope step.firstValue with
        | error reason => simp [verifyStep, hnode, hfields, hFirst] at verified
        | ok firstUnit =>
            cases firstUnit
            have firstScope := verifyOperand_sound left first step.firstScope step.firstValue hFirst
            cases hSecond : verifyOperand left second step.secondScope step.secondValue with
            | error reason => simp [verifyStep, hnode, hfields, hFirst, hSecond] at verified
            | ok secondUnit =>
                cases secondUnit
                have secondScope := verifyOperand_sound left second step.secondScope step.secondValue hSecond
                cases hRule : ruleMatches step with
                | false => simp [verifyStep, hnode, hfields, hFirst, hSecond, hRule] at verified
                | true =>
                    by_cases hClaim : step.claimed = operation.apply step.firstValue step.secondValue
                    · by_cases hResult : right.node = .constant step.claimed
                      · simp [verifyStep, hnode, hfields, hFirst, hSecond, hRule, hClaim, hResult] at verified
                        refine ⟨operation, first, second, rfl, hfields.1, hfields.2.1,
                          hfields.2.2, firstScope, secondScope, hClaim, ?_⟩
                        exact hResult
                      · have hResultNode : right.node = .constant step.claimed := by
                          simpa [verifyStep, hnode, hfields, hFirst, hSecond, hRule, hClaim] using verified
                        exact False.elim (hResult hResultNode)
                    · simp [verifyStep, hnode, hfields, hFirst, hSecond, hRule, hClaim] at verified
      · simp [verifyStep, hnode, hfields] at verified

def checkWithCancellation (cancelled : Bool) (left right : Graph) (certificate : Derivation) : Outcome :=
  if certificate.revision ≠ revision then
    .refused .staleRevision
  else if certificate.relation = .scopedEquality then
    .unsupported (.relation .scopedEquality)
  else if certificate.relation = .directedRefinement then
    .unsupported (.relation .directedRefinement)
  else if certificate.steps.length > maxSteps then
    .exhausted .steps
  else if cancelled then
    .cancelled
  else if !sameInterface left right then
    .refused .interfaceChanged
  else if !wellFormed left then
    .refused .malformed
  else
    match certificate.steps with
    | [step] =>
        match left.node with
        | .unsupported => .unsupported .operation
        | .constant _ => .unsupported .operation
        | .binary _ _ _ =>
            match verifyStep left right step with
            | .ok _ => .accepted
            | .error reason => .refused reason
    | _ => .refused .malformed

def check (left right : Graph) (certificate : Derivation) : Outcome :=
  checkWithCancellation false left right certificate

def guardedFact : GuardBefore := ⟨1, .input 0, 0⟩

def guardedLeft : Graph :=
  { inputCount := 1
    inputFacts := []
    target := 2
    node := .binary .add (.input 0) (.constant 0)
    before := [guardedFact]
    after := []
    outputs := [.result] }

def guardedRight : Graph :=
  { guardedLeft with node := .constant 0 }

def guardedStep : Step :=
  { rule := .scoped
    node := 2
    first := .input 0
    second := .constant 0
    firstScope := .guard guardedFact
    secondScope := .constant
    firstValue := 0
    secondValue := 0
    claimed := 0 }

def guardedCertificate : Derivation :=
  { revision := revision
    relation := .equality
    steps := [guardedStep] }

def preflightFact : InputFact := ⟨0, 3⟩

def preflightLeft : Graph :=
  { inputCount := 1
    inputFacts := [preflightFact]
    target := 2
    node := .binary .sub (.constant 9) (.input 0)
    before := []
    after := []
    outputs := [.result] }

def preflightRight : Graph :=
  { preflightLeft with node := .constant 6 }

def preflightCertificate : Derivation :=
  { revision := revision
    relation := .equality
    steps :=
      [{ rule := .preflight
         node := 2
         first := .constant 9
         second := .input 0
         firstScope := .constant
         secondScope := .inputFact preflightFact
         firstValue := 9
         secondValue := 3
         claimed := 6 }] }

def futureGuard : GuardBefore := ⟨3, .input 0, 0⟩

def futureGuardLeft : Graph :=
  { inputCount := 1
    inputFacts := []
    target := 2
    node := .binary .add (.input 0) (.constant 1)
    before := []
    after := [⟨3, .operand (.input 0), 0⟩]
    outputs := [.result] }

def futureGuardRight : Graph :=
  { futureGuardLeft with node := .constant 1 }

def futureGuardCertificate : Derivation :=
  { revision := revision
    relation := .equality
    steps :=
      [{ rule := .scoped
         node := 2
         first := .input 0
         second := .constant 1
         firstScope := .guard futureGuard
         secondScope := .constant
         firstValue := 0
         secondValue := 1
         claimed := 1 }] }

def staleFact : InputFact := ⟨0, 0⟩

def staleFactLeft : Graph :=
  { inputCount := 1
    inputFacts := []
    target := 2
    node := .binary .add (.input 0) (.constant 0)
    before := []
    after := []
    outputs := [.result] }

def staleFactRight : Graph :=
  { staleFactLeft with node := .constant 0 }

def staleFactCertificate : Derivation :=
  { revision := revision
    relation := .equality
    steps :=
      [{ rule := .preflight
         node := 2
         first := .input 0
         second := .constant 0
         firstScope := .inputFact staleFact
         secondScope := .constant
         firstValue := 0
         secondValue := 0
         claimed := 0 }] }

def wrongGuardValueCertificate : Derivation :=
  { guardedCertificate with steps := [{ guardedStep with firstValue := 1 }] }

def removedGuardRight : Graph :=
  { guardedRight with before := [] }

def changedInputDomainRight : Graph :=
  { preflightRight with inputFacts := [⟨0, 4⟩] }

def substitutedResultRight : Graph :=
  { guardedRight with node := .constant 1 }

def droppedAssumptionLeft : Graph :=
  { guardedLeft with before := [] }

def droppedAssumptionRight : Graph :=
  { guardedRight with before := [] }

def directedRefinementCertificate : Derivation :=
  { guardedCertificate with relation := .directedRefinement }

def scopedEqualityCertificate : Derivation :=
  { guardedCertificate with relation := .scopedEquality }

def exhaustedCertificate : Derivation :=
  { guardedCertificate with steps := [guardedStep, guardedStep] }

private theorem inputFactsHold_member (facts : List InputFact) (inputs : Nat → Bits)
    (fact : InputFact) (holds : inputFactsHold facts inputs = true)
    (member : fact ∈ facts) : inputs fact.slot = fact.exact := by
  have h := (List.all_eq_true.mp holds) fact member
  exact of_decide_eq_true h

private theorem firstBeforeFailure_member_passes (guards : List GuardBefore)
    (inputs : Nat → Bits) (guard : GuardBefore)
    (member : guard ∈ guards) (noFailure : firstBeforeFailure guards inputs = none) :
    operandValue inputs guard.value = guard.exact := by
  induction guards with
  | nil => simp at member
  | cons first rest ih =>
      simp only [List.mem_cons] at member
      cases member with
      | inl same =>
          subst guard
          have passes : guardBeforeFails inputs first = false := by
            cases fails : guardBeforeFails inputs first with
            | false => rfl
            | true => simp [firstBeforeFailure, fails] at noFailure
          have notUnequal : ¬ operandValue inputs first.value ≠ first.exact :=
            of_decide_eq_false (by simpa [guardBeforeFails] using passes)
          by_cases equal : operandValue inputs first.value = first.exact
          · exact equal
          · exact False.elim (notUnequal equal)
      | inr inRest =>
          have restNoFailure : firstBeforeFailure rest inputs = none := by
            cases fails : guardBeforeFails inputs first with
            | false => simpa [firstBeforeFailure, fails] using noFailure
            | true => simp [firstBeforeFailure, fails] at noFailure
          exact ih inRest restNoFailure

private theorem scopeValue_observes (graph : Graph) (inputs : Nat → Bits)
    (operand : Operand) (scope : Scope) (value : Bits)
    (factsHold : inputFactsHold graph.inputFacts inputs = true)
    (beforePass : firstBeforeFailure graph.before inputs = none)
    (available : scopeValue graph operand scope = some value) :
    operandValue inputs operand = value := by
  cases scope with
  | constant =>
      cases operand with
      | input slot => simp [scopeValue] at available
      | constant bits =>
          simp [scopeValue] at available
          simpa [operandValue] using available
  | inputFact fact =>
      cases operand with
      | constant bits => simp [scopeValue] at available
      | input slot =>
          by_cases evidence : fact.slot = slot ∧ fact ∈ graph.inputFacts
          · simp [scopeValue, evidence] at available
            have factValue := inputFactsHold_member graph.inputFacts inputs fact factsHold evidence.2
            have inputValue : inputs slot = fact.exact := by simpa [evidence.1] using factValue
            simpa [operandValue, inputValue] using available
          · simp [scopeValue, evidence] at available
  | guard fact =>
      by_cases evidence : fact.node < graph.target ∧ fact.value = operand ∧ fact ∈ graph.before
      · simp [scopeValue, evidence] at available
        have guardValue := firstBeforeFailure_member_passes graph.before inputs fact evidence.2.2 beforePass
        calc
          operandValue inputs operand = operandValue inputs fact.value := by rw [evidence.2.1]
          _ = fact.exact := guardValue
          _ = value := available
      · simp [scopeValue, evidence] at available

private theorem guardAfterFails_congr (left right : Graph) (inputs : Nat → Bits)
    (guard : GuardAfter) (nodeValueSame : nodeValue left inputs = nodeValue right inputs) :
    guardAfterFails left inputs guard = guardAfterFails right inputs guard := by
  cases guard.value <;> simp [guardAfterFails, guardAfterValue, nodeValueSame]

private theorem firstAfterFailure_congr (left right : Graph) (inputs : Nat → Bits)
    (guards : List GuardAfter) (nodeValueSame : nodeValue left inputs = nodeValue right inputs) :
    firstAfterFailure left inputs guards = firstAfterFailure right inputs guards := by
  induction guards with
  | nil => rfl
  | cons guard rest ih =>
      simp only [firstAfterFailure]
      rw [guardAfterFails_congr left right inputs guard nodeValueSame]
      cases guardAfterFails right inputs guard <;> simp [ih]

/-- Acceptance in the one-node fragment preserves complete observations for every input.
The `before` list is checked in node order, and `after` guards may observe the rewritten
node, so equality includes the identity and fact of the first failing guard. -/
theorem accepted_preserves_observations (left right : Graph) (certificate : Derivation)
    (inputs : Nat → Bits) (accepted : check left right certificate = .accepted) :
    observe left inputs = observe right inputs := by
  cases certificate with
  | mk certRevision certRelation certSteps =>
      cases certRevision with
      | zero =>
          cases certRelation with
          | scopedEquality => simp [check, checkWithCancellation, revision] at accepted
          | directedRefinement => simp [check, checkWithCancellation, revision] at accepted
          | equality =>
              cases certSteps with
              | nil =>
                  cases hInterface : sameInterface left right with
                  | false => simp [check, checkWithCancellation, revision, maxSteps, hInterface] at accepted
                  | true => simp [check, checkWithCancellation, revision, maxSteps, hInterface] at accepted
              | cons step rest =>
                  cases rest with
                  | cons next tail => simp [check, checkWithCancellation, revision, maxSteps] at accepted
                  | nil =>
                      cases hInterface : sameInterface left right with
                      | false => simp [check, checkWithCancellation, revision, maxSteps, hInterface] at accepted
                      | true =>
                          cases hWellFormed : wellFormed left with
                          | false => simp [check, checkWithCancellation, revision, maxSteps, hInterface, hWellFormed] at accepted
                          | true =>
                              cases hNode : left.node with
                              | unsupported => simp [check, checkWithCancellation, revision, maxSteps, hInterface, hWellFormed, hNode] at accepted
                              | constant bits => simp [check, checkWithCancellation, revision, maxSteps, hInterface, hWellFormed, hNode] at accepted
                              | binary operation first second =>
                                  cases hVerify : verifyStep left right step with
                                  | error reason => simp [check, checkWithCancellation, revision, maxSteps, hInterface, hWellFormed, hNode, hVerify] at accepted
                                  | ok verifiedUnit =>
                                      cases verifiedUnit
                                      rcases verifyStep_sound left right step hVerify with
                                        ⟨actualOperation, actualFirst, actualSecond, actualNode,
                                          _, _, _, firstScopeBound, secondScopeBound, claimBound, resultBound⟩
                                      have interfaceFacts :
                                          left.inputCount = right.inputCount ∧
                                          left.inputFacts = right.inputFacts ∧
                                          left.target = right.target ∧
                                          left.before = right.before ∧
                                          left.after = right.after ∧
                                          left.outputs = right.outputs := by
                                        exact of_decide_eq_true (by simpa [sameInterface] using hInterface)
                                      rcases interfaceFacts with ⟨_, hInputFacts, _, hBefore, hAfter, hOutputs⟩
                                      cases hInputDomain : inputFactsHold left.inputFacts inputs with
                                      | false =>
                                          have hRightInputDomain : inputFactsHold right.inputFacts inputs = false := by
                                            rw [← hInputFacts]
                                            exact hInputDomain
                                          simp [observe, hInputDomain, hRightInputDomain]
                                      | true =>
                                          have hRightInputDomain : inputFactsHold right.inputFacts inputs = true := by
                                            rw [← hInputFacts]
                                            exact hInputDomain
                                          cases hBeforeFail : firstBeforeFailure left.before inputs with
                                          | some failure =>
                                              have hRightBeforeFail : firstBeforeFailure right.before inputs = some failure := by
                                                rw [← hBefore]
                                                exact hBeforeFail
                                              simp [observe, hInputDomain, hRightInputDomain, hBeforeFail, hRightBeforeFail]
                                          | none =>
                                              have hRightBeforePass : firstBeforeFailure right.before inputs = none := by
                                                rw [← hBefore]
                                                exact hBeforeFail
                                              have firstValue := scopeValue_observes left inputs actualFirst
                                                step.firstScope step.firstValue hInputDomain hBeforeFail firstScopeBound
                                              have secondValue := scopeValue_observes left inputs actualSecond
                                                step.secondScope step.secondValue hInputDomain hBeforeFail secondScopeBound
                                              have hLeftNodeValue : nodeValue left inputs = some step.claimed := by
                                                simp [nodeValue, actualNode, firstValue, secondValue, claimBound]
                                              have hRightNodeValue : nodeValue right inputs = some step.claimed := by
                                                simp [nodeValue, resultBound]
                                              have hNodeValueSame : nodeValue left inputs = nodeValue right inputs := by
                                                rw [hLeftNodeValue, hRightNodeValue]
                                              have hAfterSame :
                                                  firstAfterFailure left inputs left.after =
                                                    firstAfterFailure right inputs right.after := by
                                                calc
                                                  firstAfterFailure left inputs left.after =
                                                      firstAfterFailure right inputs left.after :=
                                                    firstAfterFailure_congr left right inputs left.after hNodeValueSame
                                                  _ = firstAfterFailure right inputs right.after := by rw [hAfter]
                                              cases hAfterFail : firstAfterFailure left inputs left.after with
                                              | some failure =>
                                                  have hRightAfterFail :
                                                      firstAfterFailure right inputs right.after = some failure := by
                                                    rw [← hAfterSame]
                                                    exact hAfterFail
                                                  simp [observe, hInputDomain, hRightInputDomain, hBeforeFail,
                                                    hRightBeforePass, hAfterFail, hRightAfterFail, actualNode,
                                                    resultBound]
                                              | none =>
                                                  have hRightAfterPass :
                                                      firstAfterFailure right inputs right.after = none := by
                                                    rw [← hAfterSame]
                                                    exact hAfterFail
                                                  have hOutputsSame :
                                                      left.outputs.map (outputValue left inputs) =
                                                        right.outputs.map (outputValue right inputs) := by
                                                    rw [← hOutputs]
                                                    induction left.outputs with
                                                    | nil => rfl
                                                    | cons output rest ih =>
                                                        cases output <;> simp [outputValue, hNodeValueSame, ih]
                                                  simp [observe, hInputDomain, hRightInputDomain, hBeforeFail,
                                                    hRightBeforePass, hAfterFail, hRightAfterPass, actualNode,
                                                    resultBound, hOutputsSame]
      | succ nextRevision => simp [check, checkWithCancellation, revision] at accepted

theorem guarded_pilot_accepted :
    check guardedLeft guardedRight guardedCertificate = .accepted := by
  decide

theorem preflight_pilot_accepted :
    check preflightLeft preflightRight preflightCertificate = .accepted := by
  decide

theorem future_guard_refused :
    check futureGuardLeft futureGuardRight futureGuardCertificate = .refused .scopeUnavailable := by
  decide

theorem wrong_guarded_value_refused :
    check guardedLeft guardedRight wrongGuardValueCertificate = .refused .scopeValueMismatch := by
  decide

theorem stale_input_fact_refused :
    check staleFactLeft staleFactRight staleFactCertificate = .refused .scopeUnavailable := by
  decide

theorem removed_guard_refused :
    check guardedLeft removedGuardRight guardedCertificate = .refused .interfaceChanged := by
  decide

theorem changed_input_domain_refused :
    check preflightLeft changedInputDomainRight preflightCertificate = .refused .interfaceChanged := by
  decide

theorem substituted_result_refused :
    check guardedLeft substitutedResultRight guardedCertificate = .refused .resultMismatch := by
  decide

theorem scoped_equality_unsupported :
    check guardedLeft guardedRight scopedEqualityCertificate =
      .unsupported (.relation .scopedEquality) := by
  decide

theorem directed_refinement_unsupported :
    check guardedLeft guardedRight directedRefinementCertificate =
      .unsupported (.relation .directedRefinement) := by
  decide

theorem reversed_directed_refinement_unsupported :
    check guardedRight guardedLeft directedRefinementCertificate =
      .unsupported (.relation .directedRefinement) := by
  decide

theorem dropping_guard_assumption_refused :
    check droppedAssumptionLeft droppedAssumptionRight guardedCertificate =
      .refused .scopeUnavailable := by
  decide

theorem over_bound_derivation_exhausted :
    check guardedLeft guardedRight exhaustedCertificate = .exhausted .steps := by
  decide

theorem cancellation_is_not_acceptance :
    checkWithCancellation true guardedLeft guardedRight guardedCertificate = .cancelled := by
  decide

end NobleFoundation.GuardedRewrite
