/-
The fragment-v1 refinement matrix (task 5.4): per-word and per-rule
representative-input agreements between the extracted checker
`noble_kernel.acceptance.check` and the reference `NobleM2.check`, the
rejection agreements for the new B-CHECK-02/05/06 negatives, the
decision-path rows for eligibility and inclusion, and the generalized
five-way outcome correspondence. Every subject binds the generated checker
constant; each equation is closed by evaluation.
-/

import NobleKernel
import NobleM2.Embed
import NobleM2.Fixtures
import NobleM3.Rules
import NobleM3.RefinementWords

namespace NobleM3.Refine

open NobleM2 NobleM2.Fixtures NobleM2.WordCoverage NobleM2.WordRefinement
  noble_kernel Aeneas Aeneas.Std

/-- Clear the diagnostic provenance fields (node, definition) of an invalid
outcome; every other outcome and field is untouched. The extracted checker
locates failing sites the reference model does not record. -/
def eraseProvenance : Outcome → Outcome
  | .invalid d => .invalid { d with node := none, definition := none }
  | o => o

/-! ## Per-word checker agreements (task 5.4)

One theorem per table position: the extracted checker and the reference
decide the same accepted outcome on the word's canonical fixture. -/

def wordCheck (n : Nat) : Bool :=
  decide (project_result (acceptance.check (embedEnv bootstrapEnv)
      (embedRequest (wordRequestOf n)) (embedCandidate (wordCandidate n)))
    = NobleM2.check bootstrapEnv (wordRequestOf n) (wordCandidate n))

/-- `dup` (position 0): the two checkers agree on the word's fixture. -/
theorem refinement_word_dup : wordCheck 0 = true := by native_decide

/-- `drop` (position 1): the two checkers agree on the word's fixture. -/
theorem refinement_word_drop : wordCheck 1 = true := by native_decide

/-- `swap` (position 2): the two checkers agree on the word's fixture. -/
theorem refinement_word_swap : wordCheck 2 = true := by native_decide

/-- `dip` (position 3): the two checkers agree on the word's fixture. -/
theorem refinement_word_dip : wordCheck 3 = true := by native_decide

/-- `add` (position 4): the two checkers agree on the word's fixture. -/
theorem refinement_word_add : wordCheck 4 = true := by native_decide

/-- `sub` (position 5): the two checkers agree on the word's fixture. -/
theorem refinement_word_sub : wordCheck 5 = true := by native_decide

/-- `mul` (position 6): the two checkers agree on the word's fixture. -/
theorem refinement_word_mul : wordCheck 6 = true := by native_decide

/-- `equals` (position 7): the two checkers agree on the word's fixture. -/
theorem refinement_word_equals : wordCheck 7 = true := by native_decide

/-- `quote` (position 8): the two checkers agree on the word's fixture. -/
theorem refinement_word_quote : wordCheck 8 = true := by native_decide

/-- `compose` (position 9): the two checkers agree on the word's fixture. -/
theorem refinement_word_compose : wordCheck 9 = true := by native_decide

/-- `run` (position 10): the two checkers agree on the word's fixture. -/
theorem refinement_word_run : wordCheck 10 = true := by native_decide

/-- `reflect` (position 11): the two checkers agree on the word's fixture. -/
theorem refinement_word_reflect : wordCheck 11 = true := by native_decide

/-- `unit` (position 12): the two checkers agree on the word's fixture. -/
theorem refinement_word_unit : wordCheck 12 = true := by native_decide

/-- `pair` (position 13): the two checkers agree on the word's fixture. -/
theorem refinement_word_pair : wordCheck 13 = true := by native_decide

/-- `unpair` (position 14): the two checkers agree on the word's fixture. -/
theorem refinement_word_unpair : wordCheck 14 = true := by native_decide

/-- `inl` (position 15): the two checkers agree on the word's fixture. -/
theorem refinement_word_inl : wordCheck 15 = true := by native_decide

/-- `inr` (position 16): the two checkers agree on the word's fixture. -/
theorem refinement_word_inr : wordCheck 16 = true := by native_decide

/-- `case` (position 17): the two checkers agree on the word's fixture. -/
theorem refinement_word_case : wordCheck 17 = true := by native_decide

/-- `if` (position 18): the two checkers agree on the word's fixture. -/
theorem refinement_word_if : wordCheck 18 = true := by native_decide

/-- `nil` (position 19): the two checkers agree on the word's fixture. -/
theorem refinement_word_nil : wordCheck 19 = true := by native_decide

/-- `cons` (position 20): the two checkers agree on the word's fixture. -/
theorem refinement_word_cons : wordCheck 20 = true := by native_decide

/-- `list_case` (position 21): the two checkers agree on the word's fixture. -/
theorem refinement_word_list_case : wordCheck 21 = true := by native_decide

/-- `test_emit` (position 22): the two checkers agree on the word's fixture. -/
theorem refinement_word_test_emit : wordCheck 22 = true := by native_decide

/-- All 23 per-word checker agreements hold at once. -/
theorem refinement_words : (List.range 23).all wordCheck = true := by native_decide
/-! ## Per-rule representative agreements (task 5.4) -/

/-- The empty-body fixture: `[] -- []` with nothing allowed. -/
def emptyCandidate : Candidate := candidateOf [] []
def emptyRequest : Request := requestOf [] [] []
def emptyChecked : Checked :=
  ⟨⟨tyList [], tyList [], EffSet.empty⟩, []⟩

/-- EMPTY: both checkers accept the empty body with no derivations. -/
theorem refinement_rule_empty :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest emptyRequest) (embedCandidate emptyCandidate))
      = NobleM2.check bootstrapEnv emptyRequest emptyCandidate := by
  native_decide

/-- LITERAL: both checkers accept the unit-literal fixture. -/
def literalCandidate : Candidate := candidateOf [litNode .unit []] [0]

theorem refinement_rule_literal :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [.unit] [])) (embedCandidate literalCandidate))
      = NobleM2.check bootstrapEnv (requestOf [] [.unit] []) literalCandidate := by
  native_decide

/-- WORD: both checkers accept the `add` word's fixture (position 4). -/
theorem refinement_rule_word : wordCheck 4 = true := by native_decide

/-- SEQUENCE: both checkers accept the arithmetic-sequence fixture. -/
theorem refinement_rule_sequence :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [.i64] [])) (embedCandidate sequenceCandidate))
      = NobleM2.check bootstrapEnv (requestOf [] [.i64] []) sequenceCandidate := by
  native_decide

/-- QUOTATION: both checkers accept the quotation fixture. -/
theorem refinement_rule_quotation :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest fortyTwoRequest) (embedCandidate fortyTwoCandidate))
      = NobleM2.check bootstrapEnv fortyTwoRequest fortyTwoCandidate := by
  native_decide

/-- CASE: both checkers accept the `case` eliminator's fixture (position 17). -/
theorem refinement_rule_case : wordCheck 17 = true := by native_decide

/-- IF: both checkers accept the `if` eliminator's fixture (position 18). -/
theorem refinement_rule_if : wordCheck 18 = true := by native_decide

/-- LISTCASE: both checkers accept the `list.case` eliminator's fixture
(position 21). -/
theorem refinement_rule_listcase : wordCheck 21 = true := by native_decide

/-! ## The rejection agreements (B-CHECK-02/05/06) -/

/-- One extra named definition `S -- S Unit` per declared dependency list. -/
def withDeps (deps : List (List Nat)) : Env :=
  { defs := bootstrapTable.map (fun entry => entry.2) ++ List.replicate deps.length unitScheme
    kinds := bootstrapTable.map (fun entry => entry.1) ++ List.replicate deps.length .named
    effects := [testEmitEffect]
    deps := List.replicate bootstrapTable.length [] ++ deps
    schemas := [] }

/-- The environment with one user-declared schema. -/
def withSchema (id : Nat) (recursive : Bool) : Env :=
  { bootstrapEnv with schemas := [⟨id, unitScheme, recursive⟩] }

/-- A recursive dependency closing on definition 23: both checkers reject
unsupported, naming the identity (B-CHECK-02). -/
theorem refinement_recursive_dependency_self :
    project_result (acceptance.check (embedEnv (withDeps [[23]]))
        (embedRequest (requestOf [] [] [])) (embedCandidate emptyCandidate))
      = NobleM2.check (withDeps [[23]]) (requestOf [] [] []) emptyCandidate := by
  native_decide

/-- A mutual dependency cycle between definitions 23 and 24: both reject
unsupported, naming 23. -/
theorem refinement_recursive_dependency_mutual :
    project_result (acceptance.check (embedEnv (withDeps [[24], [23]]))
        (embedRequest (requestOf [] [] [])) (embedCandidate emptyCandidate))
      = NobleM2.check (withDeps [[24], [23]]) (requestOf [] [] []) emptyCandidate := by
  native_decide

/-- The acyclic boundary: one declared edge exactly within one unit of
declared work — both accept (B-CHECK-02/07). -/
theorem refinement_dependency_boundary :
    project_result (acceptance.check (embedEnv (withDeps [[24], []]))
        (embedRequest { (requestOf [] [] []) with limits := { baseLimits with work := 1 } })
        (embedCandidate emptyCandidate))
      = NobleM2.check (withDeps [[24], []])
          { (requestOf [] [] []) with limits := { baseLimits with work := 1 } } emptyCandidate := by
  native_decide

/-- The exhausted companion: no work for the walk's single charge — both
fail closed exhausted. -/
theorem refinement_dependency_exhausted :
    project_result (acceptance.check (embedEnv (withDeps [[24], []]))
        (embedRequest { (requestOf [] [] []) with limits := { baseLimits with work := 0 } })
        (embedCandidate emptyCandidate))
      = NobleM2.check (withDeps [[24], []])
          { (requestOf [] [] []) with limits := { baseLimits with work := 0 } } emptyCandidate := by
  native_decide

/-- External environment data is validated before any candidate body check:
a recursive dependency rejects even a candidate whose body would also
reject as invalid. -/
def danglingCandidate : Candidate := candidateOf [] [9]

theorem refinement_dependency_before_body :
    project_result (acceptance.check (embedEnv (withDeps [[23]]))
        (embedRequest (requestOf [] [] [])) (embedCandidate danglingCandidate))
      = NobleM2.check (withDeps [[23]]) (requestOf [] [] []) danglingCandidate := by
  native_decide

/-- A user-declared recursive schema: both reject unsupported, naming the
declaration (B-CHECK-02). -/
theorem refinement_recursive_schema :
    project_result (acceptance.check (embedEnv (withSchema 7 true))
        (embedRequest (requestOf [] [] [])) (embedCandidate emptyCandidate))
      = NobleM2.check (withSchema 7 true) (requestOf [] [] []) emptyCandidate := by
  native_decide

/-- Two non-recursive schema entries scan within two units of declared work:
both accept. -/
def withSchemas2 : Env :=
  { bootstrapEnv with schemas := [⟨1, unitScheme, false⟩, ⟨2, unitScheme, false⟩] }

theorem refinement_schema_boundary :
    project_result (acceptance.check (embedEnv withSchemas2)
        (embedRequest { (requestOf [] [] []) with limits := { baseLimits with work := 2 } })
        (embedCandidate emptyCandidate))
      = NobleM2.check withSchemas2
          { (requestOf [] [] []) with limits := { baseLimits with work := 2 } } emptyCandidate := by
  native_decide

/-- One below the scan's charge: both fail closed exhausted. -/
theorem refinement_schema_exhausted :
    project_result (acceptance.check (embedEnv withSchemas2)
        (embedRequest { (requestOf [] [] []) with limits := { baseLimits with work := 1 } })
        (embedCandidate emptyCandidate))
      = NobleM2.check withSchemas2
          { (requestOf [] [] []) with limits := { baseLimits with work := 1 } } emptyCandidate := by
  native_decide

/-- A self-referential witness: `dup` with its value variable bound to
itself. Both reject invalid with the cyclic-witness constraint; the
extracted diagnostic locates the site, so both sides compare after
`eraseProvenance` (B-CHECK-05). -/
def cyclicSelfCandidate : Candidate :=
  candidateOf [invoke dupDef [seg [], .ref 1]] [0]

theorem refinement_cyclic_witness_self :
    eraseProvenance (project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [] [])) (embedCandidate cyclicSelfCandidate)))
      = eraseProvenance
        (NobleM2.check bootstrapEnv (requestOf [] [] []) cyclicSelfCandidate) := by
  native_decide

/-- Mutually referential witnesses: `swap` with its two value variables
bound to each other. -/
def cyclicMutualCandidate : Candidate :=
  candidateOf [invoke 2 [seg [], .ref 2, .ref 1]] [0]

theorem refinement_cyclic_witness_mutual :
    eraseProvenance (project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [] [])) (embedCandidate cyclicMutualCandidate)))
      = eraseProvenance
        (NobleM2.check bootstrapEnv (requestOf [] [] []) cyclicMutualCandidate) := by
  native_decide

/-- A resolvable reference chain: `swap` with its first value bound by
reference to the second. Both accept with the resolved interface. -/
def resolvableCandidate : Candidate := candidateOf
  [ litNode (.bool true) [],
    litNode (.bool false) [.bool],
    invoke 2 [seg [], .ref 2, val .bool] ]
  [0, 1, 2]

theorem refinement_resolvable_chain :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [.bool, .bool] [])) (embedCandidate resolvableCandidate))
      = NobleM2.check bootstrapEnv (requestOf [] [.bool, .bool] []) resolvableCandidate := by
  native_decide

/-- A kind-crossed reference: a stack variable bound by reference to a value
variable. Both reject with the instantiation-kind constraint (after
provenance erasure). -/
def kindCrossedCandidate : Candidate :=
  candidateOf [invoke dupDef [.ref 1, val .bool]] [0]

theorem refinement_kind_crossed_reference :
    eraseProvenance (project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [] [])) (embedCandidate kindCrossedCandidate)))
      = eraseProvenance
        (NobleM2.check bootstrapEnv (requestOf [] [] []) kindCrossedCandidate) := by
  native_decide

/-- The duplication negative (B-CHECK-03/K-CHECK-01): a quotation's program
value is duplicated by `dup`, and running both copies cannot claim an
independent instantiation — both checkers reject with the stack-join
constraint. -/
def dupProgram : Ty := .program (WordCoverage.tl [.i64]) (WordCoverage.tl [.i64]) EffSet.empty

def duplicationCandidate : Candidate := candidateOf
  [ quoteNode [] [] [.i64] [.i64] [],
    invoke dupDef [seg [], val dupProgram],
    invoke runDef [seg [], seg [.i64], eff []],
    invoke runDef [seg [], seg [.i64], eff []] ]
  [0, 1, 2, 3]

theorem refinement_duplication_negative :
    eraseProvenance (project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [.i64, dupProgram] []))
        (embedCandidate duplicationCandidate)))
      = eraseProvenance
        (NobleM2.check bootstrapEnv (requestOf [] [.i64, dupProgram] [])
          duplicationCandidate) := by
  native_decide

/-- The implicit-union negative (B-CHECK-06): an `if` whose left branch
carries the latent `test.emit` bound and whose right branch is quiet. The
derived union rejects against the empty allowance — a checker that
implicitly joined to the quiet branch's bound would accept. -/
def emittingProgram : Ty :=
  .program (tyList []) (tyList [.unit]) (EffSet.ofIds [0])

def implicitUnionCandidate : Candidate := candidateOf
  [ litNode (.bool true) [],
    quoteNode [2, 3] [.bool] [] [.unit] [0],
    litNode .text [],
    invoke emitDef [seg []],
    quoteNode [5] [.bool, emittingProgram] [] [.unit] [],
    litNode .unit [],
    invoke 18 [seg [], seg [.unit], eff [0], eff []] ]
  [0, 1, 4, 6]

theorem refinement_implicit_union_negative :
    eraseProvenance (project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [.unit] []))
        (embedCandidate implicitUnionCandidate)))
      = eraseProvenance
        (NobleM2.check bootstrapEnv (requestOf [] [.unit] []) implicitUnionCandidate) := by
  native_decide
/-! ## The decision-path rows (B-CHECK-04) -/

/-- The reference-side eligibility correspondence: the guard decides the
declarative side condition (per constrained word below). -/
theorem eligibility_iff {env : Env} {index : Nat} {inst : Inst} :
    dataOkAt env index inst = true ↔ DataOk env index inst :=
  dataOkAt_iff env index inst

/-- The constraint a rejected extracted instantiation reports, projected. -/
def extractedConstraint
    (r : Result (core.result.Result (untrusted.Interface × words.Inst) acceptance.Fail)) :
    Option Constraint :=
  match r.match with
  | .ok (core.result.Result.Err (acceptance.Fail.Invalid d)) =>
    some (projectConstraint d.constraint)
  | _ => none

/-- The resource witness for the constrained words' value slot. -/
def resourceWit : Inst := ⟨[.stack .nil, .value (.resource fixtureResource)]⟩

/-- `dup`: the extracted path rejects a resource witness with the eligibility
constraint exactly where the reference guard declines `DataOk`. -/
theorem eligibility_dup :
    dataOkAt bootstrapEnv 0 resourceWit = false
      ∧ extractedConstraint (acceptance.parts.instantiate.apply (embedScheme dupScheme)
          (embedInst resourceWit) (some (embedU32 1)) ⟨none, none⟩
          (WordRefinement.wordCtx 0))
        = some (.eligibility (.resource fixtureResource)) := by
  constructor <;> native_decide

/-- `drop`: as `dup`. -/
theorem eligibility_drop :
    dataOkAt bootstrapEnv 1 resourceWit = false
      ∧ extractedConstraint (acceptance.parts.instantiate.apply (embedScheme dropScheme)
          (embedInst resourceWit) (some (embedU32 1)) ⟨none, none⟩
          (WordRefinement.wordCtx 1))
        = some (.eligibility (.resource fixtureResource)) := by
  constructor <;> native_decide

/-- `quote`: as `dup`, with the third (stack) binding its scheme
requires. -/
def quoteResourceWit : Inst :=
  ⟨[.stack .nil, .value (.resource fixtureResource), .stack .nil]⟩

theorem eligibility_quote :
    dataOkAt bootstrapEnv 8 quoteResourceWit = false
      ∧ extractedConstraint (acceptance.parts.instantiate.apply (embedScheme quoteScheme)
          (embedInst quoteResourceWit) (some (embedU32 1)) ⟨none, none⟩
          (WordRefinement.wordCtx 8))
        = some (.eligibility (.resource fixtureResource)) := by
  constructor <;> native_decide

/-- The reference-side inclusion correspondence: the decided inclusion is the
declarative subset. -/
theorem inclusion_iff {a b : EffSet} : a.subsetOf b = true ↔ a.Subset b :=
  NobleM2.subsetOf_iff

/-- The extracted inclusion pairs the matrix runs. -/
def inclusionPairs : List (List Nat × List Nat) :=
  [([], []), ([0], []), ([0], [0, 1]), ([0, 1], [0]), ([1], [0, 2]), ([2, 1, 0], [0, 1, 2])]

/-- The extracted `is_subset_of`, run over the embedded sets. -/
def extractedSubset (a b : EffSet) : Bool :=
  match (noble_kernel.types.EffSet.is_subset_of (embedEffSet a) (embedEffSet b)).match with
  | .ok v => v
  | _ => false

/-- The extracted-side inclusion agreement: over the matrix, the extracted
inclusion decides exactly the reference `subsetOf`. -/
theorem inclusion_extracted :
    inclusionPairs.all (fun p =>
      extractedSubset (EffSet.ofIds p.1) (EffSet.ofIds p.2)
        = (EffSet.ofIds p.1).subsetOf (EffSet.ofIds p.2)) = true := by
  native_decide

/-! ## The generalized outcome correspondence -/

/-- The v1 generalization of `accepted_via_refinement`: one agreement
equation between the extracted run and the reference decision yields the
full five-way outcome correspondence — acceptance projects the checked
result, and each rejection projects its diagnostic, identity, and limit. -/
theorem outcomes_via_refinement_v1 {env : Env} {req : Request} {cand : Candidate}
    (h : project_result (acceptance.check (embedEnv env) (embedRequest req)
        (embedCandidate cand)) = NobleM2.check env req cand) :
    (∀ gchecked, acceptance.check (embedEnv env) (embedRequest req) (embedCandidate cand)
        = Result.ok (untrusted.Outcome.Accepted gchecked) →
      NobleM2.check env req cand = Outcome.accepted (project_checked gchecked))
    ∧ (∀ gdiag, acceptance.check (embedEnv env) (embedRequest req) (embedCandidate cand)
        = Result.ok (untrusted.Outcome.Invalid gdiag) →
      NobleM2.check env req cand = Outcome.invalid (projectDiagnostic gdiag))
    ∧ (∀ gkind, acceptance.check (embedEnv env) (embedRequest req) (embedCandidate cand)
        = Result.ok (untrusted.Outcome.Unsupported gkind) →
      NobleM2.check env req cand = Outcome.unsupported (projectUnsupportedKind gkind))
    ∧ (∀ gkind, acceptance.check (embedEnv env) (embedRequest req) (embedCandidate cand)
        = Result.ok (untrusted.Outcome.Exhausted gkind) →
      NobleM2.check env req cand = Outcome.exhausted (projectLimitKind gkind)) := by
  refine ⟨?_, ?_, ?_, ?_⟩ <;> intro gX hX <;> rw [hX] at h <;>
    simp only [project_result, project_outcome] at h <;> exact h.symm

/-- The v1 acceptance composition: extracted/reference agreement plus the
extracted run's acceptance hands the extracted checked result to the
reference soundness story. -/
theorem accepted_via_refinement_v1 {env : Env} {req : Request} {cand : Candidate}
    {gchecked : noble_kernel.untrusted.Checked}
    (h : project_result (acceptance.check (embedEnv env) (embedRequest req)
        (embedCandidate cand)) = NobleM2.check env req cand)
    (gacc : acceptance.check (embedEnv env) (embedRequest req) (embedCandidate cand)
      = Result.ok (untrusted.Outcome.Accepted gchecked)) :
    NobleM2.check env req cand = Outcome.accepted (project_checked gchecked) :=
  (outcomes_via_refinement_v1 h).1 gchecked gacc

end NobleM3.Refine
