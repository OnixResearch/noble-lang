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

end NobleFoundation.Certificate
