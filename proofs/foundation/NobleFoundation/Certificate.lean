/-
Model of the replayable rewrite-certificate format (issue #42, format half).

`crates/noble-ngraph/src/cert.rs` emits `ngraph-cert/0` derivations: a revision, an
ordered step list where each step names a rule, the rewritten node, the ordered
operands, the exact constant bits the rule consumed and the claimed value. This
file models that *format* and proves what its acceptance does and does not buy:

* the format revision and the step bound are refusals, not warnings;
* a rule identity outside the modelled set cannot decode to a claim at all;
* a step is accepted only when the graph's own node shapes and constant bits match
  the declared ones *and* the claim passes the graph checker, so acceptance of a
  derived certificate implies the rule theorem for its fold step
  (`accepted_fold_step_denotes`);
* replaying every accepted fold in order, while leaving identity steps untouched,
  preserves every node denotation (`accepted_endpoint_equivalence`); the step
  premises in this model are checked against the original graph, as in `cert.rs`;
* non-increasing node order is refused.

What this is not: no decoding of actual bytes is modelled, so nothing here binds
`serde`/binary output of the Rust encoder to these structures, and no statement
covers envelope digests, budget accounting, or the identity rules.
-/
import NobleFoundation.GraphRewrite

namespace NobleFoundation.Certificate

open NobleFoundation.PackedSchedule
open NobleFoundation.GraphRewrite

/-! ### Wire format -/

/-- The format revision this model accepts. -/
def revision : String := "ngraph-cert/0"

/-- The declared step bound: larger derivations are refused before any node is
inspected. -/
def maxSteps : Nat := 64

/-- One declared derivation step, in wire order. `rule` is the serialized rule
identity (0 fold, 1 zero-left, 2 one-right). -/
structure Step where
  rule : Nat
  index : Nat
  first : Nat
  second : Nat
  firstBits : BitVec 64
  secondBits : BitVec 64
  claimed : BitVec 64

/-- A declared derivation. -/
structure Derivation where
  revision : String
  steps : List Step

/-- Decode a step to the graph checker's claim. An unknown rule identity decodes to
nothing, so a certificate cannot smuggle a rule the checker does not know. -/
def decodeStep (step : Step) : Option Claim :=
  match step.rule with
  | 0 => some (.constAdd step.index step.first step.second step.claimed)
  | 1 => some (.zeroLeft step.index step.first step.second)
  | 2 => some (.oneRight step.index step.first step.second)
  | _ => none

/-- Node identities in a step list are strictly increasing. -/
def strictlyIncreasing : List Step → Bool
  | [] => true
  | [_] => true
  | only :: next :: rest => (only.index < next.index) && strictlyIncreasing (next :: rest)

/-- The declared constant bits of a step match the graph's own literal, for the
operand the rule consumed. Structural: it never recomputes the claim. -/
def bitsMatch (graph : Graph) (step : Step) : Bool :=
  match step.rule, graph.node step.first, graph.node step.second with
  | 0, .constant x, .constant y => decide (x = step.firstBits ∧ y = step.secondBits)
  | 1, .constant x, _ => decide (x = 0 ∧ step.firstBits = step.secondBits)
  | 2, _, .constant y => decide (y = 1 ∧ step.firstBits = step.secondBits)
  | _, _, _ => false

/-- Acceptance of one step: known rule identity, declared bits equal to the graph's
own bits, and the graph checker accepting the decoded claim. -/
def stepAccepted (graph : Graph) (step : Step) : Bool :=
  bitsMatch graph step &&
    match decodeStep step with
    | some claim => Checked graph claim
    | none => false

/-- A derivation is well formed when it names this revision within the step bound
and its steps are ordered. -/
def wellFormed (derivation : Derivation) : Bool :=
  (derivation.revision = revision) && derivation.steps.length ≤ maxSteps &&
    strictlyIncreasing derivation.steps

/-- Acceptance of a whole derivation. -/
def accepted (graph : Graph) (derivation : Derivation) : Bool :=
  wellFormed derivation && derivation.steps.all (stepAccepted graph)

/-! ### Replay -/

/-- Apply the graph-changing part of one certificate step. Fold steps replace
their target node with the claimed constant; identity steps are recorded but do
not change the graph. The nonzero branch also leaves unknown rules unchanged,
but such a step cannot occur in an accepted derivation. -/
def applyStep (graph : Graph) (step : Step) : Graph :=
  match step.rule with
  | 0 =>
      { graph with
        node := fun index =>
          if index = step.index then .constant step.claimed else graph.node index }
  | _ => graph

/-- Replay a list of steps in its declared order. -/
def applySteps (graph : Graph) : List Step → Graph
  | [] => graph
  | step :: rest => applySteps (applyStep graph step) rest

/-- The result graph denoted by a derivation. This mirrors `cert.rs::replay`:
only constant-folding steps are applied in place, while identity steps remain
recorded without changing node identities. -/
def applyDerivation (graph : Graph) (derivation : Derivation) : Graph :=
  applySteps graph derivation.steps

/-! ### Refusals -/

/-- A certificate that names another revision is refused. -/
theorem stale_revision_refused (steps : List Step) :
    wellFormed ⟨"ngraph-cert/1", steps⟩ = false := by
  simp [wellFormed, revision]

/-- A certificate over the declared step bound is refused. -/
theorem over_bound_refused (steps : List Step) (hbound : maxSteps < steps.length) :
    wellFormed ⟨revision, steps⟩ = false := by
  have hfalse : steps.length ≤ maxSteps ↔ False :=
    ⟨Nat.not_le.mpr hbound, False.elim⟩
  unfold wellFormed
  simp [revision, hfalse]

/-- Non-increasing node order is refused. -/
theorem unordered_refused (first second : Nat) (rule₁ rule₂ : Nat) (claimed : BitVec 64)
    (horder : ¬ first < second) :
    strictlyIncreasing
        [⟨rule₁, first, 0, 0, 0, 0, claimed⟩, ⟨rule₂, second, 0, 0, 0, 0, claimed⟩] =
      false := by
  simp [strictlyIncreasing, horder]

/-- An unknown rule identity cannot decode, so a derivation that declares one cannot
be accepted. -/
theorem unknown_rule_refused (graph : Graph) (step : Step) (hrule : decodeStep step = none) :
    stepAccepted graph step = false := by
  unfold stepAccepted
  rw [hrule]
  simp

/-- A stale revision kills acceptance even when every step would be accepted. -/
theorem stale_revision_kills_acceptance (graph : Graph) (steps : List Step) :
    accepted graph ⟨"ngraph-cert/1", steps⟩ = false := by
  unfold accepted
  simp [wellFormed, revision]

/-! ### Acceptance implies the rule theorem -/

/-- Every step of an accepted derivation has a decoded claim the graph checker
accepts. The claim is only ever the one the step decodes to. -/
theorem accepted_implies_checked (graph : Graph) (derivation : Derivation)
    (haccepted : accepted graph derivation = true) (step : Step)
    (hmem : step ∈ derivation.steps) (claim : Claim)
    (hdecode : decodeStep step = some claim) :
    Checked graph claim = true := by
  unfold accepted at haccepted
  rw [Bool.and_eq_true] at haccepted
  obtain ⟨_, hall⟩ := haccepted
  rw [List.all_eq_true] at hall
  have hstep := hall step hmem
  unfold stepAccepted at hstep
  rw [Bool.and_eq_true] at hstep
  obtain ⟨_, hclaim⟩ := hstep
  rw [hdecode] at hclaim
  exact hclaim

/-- Consequently an accepted certificate's fold step denotes the claimed constant:
the encoder's claim is not taken on faith, it is the checker's own denotation. -/
theorem accepted_fold_step_denotes (graph : Graph) (hwf : WellFormed graph)
    (derivation : Derivation) (haccepted : accepted graph derivation = true)
    (step : Step) (hmem : step ∈ derivation.steps) (inputs : Nat → BitVec 64)
    (hfold : step.rule = 0) :
    value graph inputs step.index = some (.i64 step.claimed) := by
  have hclaimEq : decodeStep step =
      some (.constAdd step.index step.first step.second step.claimed) := by
    unfold decodeStep
    rw [hfold]
    rfl
  exact checked_sound_const_add graph hwf inputs step.index step.first step.second
    step.claimed
    (accepted_implies_checked graph derivation haccepted step hmem _ hclaimEq)

/-- An accepted derivation preserves every graph denotation when its replayed
graph is compared with the original. Acceptance checks each step against the
original graph, exactly as the Rust checker does; the induction therefore uses
the already-established agreement invariant to show that later fold targets
still have their original denotations. -/
theorem accepted_endpoint_equivalence (graph : Graph) (hwf : WellFormed graph)
    (derivation : Derivation) (haccepted : accepted graph derivation = true)
    (inputs : Nat → BitVec 64) :
    ∀ index, value graph inputs index =
      value (applyDerivation graph derivation) inputs index := by
  have go : ∀ (steps : List Step) (current : Graph),
      (∀ step ∈ steps, step ∈ derivation.steps) →
      WellFormed current →
      (∀ index, value graph inputs index = value current inputs index) →
      ∀ index, value graph inputs index = value (applySteps current steps) inputs index := by
    intro steps
    induction steps with
    | nil =>
        intro current _ _ hagree index
        simpa [applySteps] using hagree index
    | cons step rest ih =>
        intro current hmem hwfCurrent hagree index
        have hstepmem : step ∈ derivation.steps := hmem step (by simp)
        have hrestmem : ∀ tailStep ∈ rest, tailStep ∈ derivation.steps := by
          intro tailStep htail
          exact hmem tailStep (by simp [htail])
        by_cases hrule : step.rule = 0
        · have hdecode : decodeStep step =
              some (.constAdd step.index step.first step.second step.claimed) := by
            simp [decodeStep, hrule]
          have hchecked :
              Checked graph (.constAdd step.index step.first step.second step.claimed) = true :=
            accepted_implies_checked graph derivation haccepted step hstepmem _ hdecode
          have hdenote : value graph inputs step.index = some (.i64 step.claimed) :=
            checked_sound_const_add graph hwf inputs step.index step.first step.second
              step.claimed hchecked
          have hcurrent : value current inputs step.index = some (.i64 step.claimed) := by
            rw [← hagree step.index]
            exact hdenote
          have hnextWf : WellFormed (applyStep current step) := by
            refine ⟨?_, ?_⟩
            · simpa [applyStep, hrule] using hwfCurrent.1
            intro index operand hoperand
            by_cases hindex : index = step.index
            · subst index
              simp [applyStep, hrule, Node.operands] at hoperand
            · apply hwfCurrent.2 index operand
              simpa [applyStep, hrule, hindex] using hoperand
          have hnode : (applyStep current step).node step.index =
              .constant step.claimed := by
            simp [applyStep, hrule]
          have hnext : value (applyStep current step) inputs step.index =
              some (.i64 step.claimed) :=
            constant_value (applyStep current step) inputs step.index step.claimed hnode
          have hlocal : ∀ index, value current inputs index =
              value (applyStep current step) inputs index := by
            apply replacement_lift current (applyStep current step) inputs
              hwfCurrent hnextWf step.index
            · intro index hlt
              simp [applyStep, hrule, Nat.ne_of_lt hlt]
            · intro index hgt
              have hne : index ≠ step.index := by omega
              simp [applyStep, hrule, hne]
            · exact hcurrent.trans hnext.symm
          have htailAgree : ∀ index, value graph inputs index =
              value (applyStep current step) inputs index := by
            intro index
            exact (hagree index).trans (hlocal index)
          have htail := ih (applyStep current step) hrestmem hnextWf htailAgree
          simpa [applySteps] using htail index
        · have htail := ih current hrestmem hwfCurrent hagree
          simpa [applySteps, applyStep, hrule] using htail index
  intro index
  simpa [applyDerivation] using
    go derivation.steps graph (by intro step hmem; exact hmem) hwf
      (by intro index; rfl) index

/-! ### Replay fixture -/

/-- Two independent constant additions in strictly increasing node order. -/
def multiFoldGraph : Graph where
  arity := 6
  output := 5
  node := fun index =>
    if index = 0 then .constant 3
    else if index = 1 then .constant 5
    else if index = 2 then .add 0 1
    else if index = 3 then .constant 7
    else if index = 4 then .constant 9
    else .add 3 4

/-- The two folds are checked against the original graph, as in `cert.rs`. -/
def multiFoldDerivation : Derivation where
  revision := revision
  steps := [
    ⟨0, 2, 0, 1, 3, 5, 8⟩,
     ⟨0, 5, 3, 4, 7, 9, 16⟩]

theorem multiFoldGraph_wellFormed : WellFormed multiFoldGraph := by
  refine ⟨by decide, ?_⟩
  intro index operand hmem
  by_cases h0 : index = 0
  · subst h0
    simp [multiFoldGraph, Node.operands] at hmem
  by_cases h1 : index = 1
  · subst h1
    simp [multiFoldGraph, Node.operands] at hmem
  by_cases h2 : index = 2
  · subst h2
    simp [multiFoldGraph, Node.operands] at hmem
    omega
  by_cases h3 : index = 3
  · subst h3
    simp [multiFoldGraph, Node.operands] at hmem
  by_cases h4 : index = 4
  · subst h4
    simp [multiFoldGraph, Node.operands] at hmem
  · simp [multiFoldGraph, h0, h1, h2, h3, h4, Node.operands] at hmem
    omega

theorem multiFold_derivation_accepted :
    accepted multiFoldGraph multiFoldDerivation = true := by
  decide

theorem multiFold_reference :
    value multiFoldGraph (fun _ => 0) 2 = some (.i64 8) ∧
      value multiFoldGraph (fun _ => 0) 5 = some (.i64 16) := by
  decide

theorem multiFold_endpoint :
    ∀ index, value multiFoldGraph (fun _ => 0) index =
      value (applyDerivation multiFoldGraph multiFoldDerivation) (fun _ => 0) index := by
  exact accepted_endpoint_equivalence multiFoldGraph multiFoldGraph_wellFormed
    multiFoldDerivation multiFold_derivation_accepted (fun _ => 0)

/-- A result that substitutes a different constant at the first fold target is
not the graph produced by replay. -/
def substitutedMultiFoldGraph : Graph where
  arity := multiFoldGraph.arity
  output := multiFoldGraph.output
  node := fun index =>
    if index = 2 then .constant 9 else multiFoldGraph.node index

theorem substituted_result_differs :
    substitutedMultiFoldGraph ≠ applyDerivation multiFoldGraph multiFoldDerivation := by
  intro heq
  have hnode := congrArg (fun candidate : Graph => candidate.node 2) heq
  simp [substitutedMultiFoldGraph, applyDerivation, applySteps, applyStep,
    multiFoldDerivation, multiFoldGraph] at hnode

end NobleFoundation.Certificate
