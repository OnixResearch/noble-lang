/-
Reference model: the fragment fixture matrix (M2 fragment v0).

One named definition per kernel acceptance test in
`crates/noble-kernel/tests/acceptance.rs`, plus an agreement harness that
runs the reference checker over each fixture and compares its outcome with
the outcome the kernel's test asserts. The kernel outcome is recorded as a
comment (and a value) on every fixture.
-/

import NobleM2.Check

namespace NobleM2.Fixtures

/-! ## Builders, mirroring `tests/support/mod.rs` -/

/-- Definition indices the kernel tests use (`contracts::Definition(_)`). -/
def dupDef : Nat := 0
def addDef : Nat := 4
def runDef : Nat := 9
def emitDef : Nat := 21
def makerDef : Nat := 22

def tyList : List Ty → TyList
  | [] => .nil
  | ty :: rest => .cons ty (tyList rest)

def seg (stack : List Ty) : Binding := .stack (tyList stack)

def val (ty : Ty) : Binding := .value ty

def eff (ids : List EffId) : Binding := .effect (EffSet.ofIds ids)

def litNode (lit : Lit) (stack : List Ty) : Node := .literal lit ⟨[seg stack]⟩

def quoteNode (body : List Nat) (r a c : List Ty) (e : List EffId) : Node :=
  .quotation body ⟨[seg r, seg a, seg c, eff e]⟩

def invoke (defn : Nat) (bindings : List Binding) : Node :=
  .invocation defn ⟨bindings⟩

def candidateOf (nodes : List Node) (body : List Nat) : Candidate :=
  ⟨0, 0, nodes, body⟩

/-- The kernel tests' default limits. -/
def baseLimits : Limits := ⟨65536, 256, 32, 64, 16, 10000, 64⟩

def requestOf (stackIn stackOut : List Ty) (allowed : List EffId) : Request :=
  ⟨64, ⟨tyList stackIn, tyList stackOut, EffSet.ofIds allowed⟩, baseLimits⟩

/-! ## Structural outcome equality

The outcome types derive no `DecidableEq`; the harness compares the fields
the kernel's tests assert on. -/

def constraintEq : Constraint → Constraint → Bool
  | .stackJoin, .stackJoin => true
  | .stackOrder, .stackOrder => true
  | .effectInclusion a, .effectInclusion b => a == b
  | .eligibility a, .eligibility b => a == b
  | .unknownEffect a, .unknownEffect b => a == b
  | .instantiationKind, .instantiationKind => true
  | .instantiationArity, .instantiationArity => true
  | .malformedReference a, .malformedReference b => a == b
  | .unknownDefinition a, .unknownDefinition b => a == b
  | _, _ => false

def diagnosticEq (a b : Diagnostic) : Bool :=
  a.node == b.node && a.definition == b.definition && a.expected == b.expected
    && a.actual == b.actual && constraintEq a.constraint b.constraint
    && a.provenanceAvailable == b.provenanceAvailable && a.truncated == b.truncated

def unsupportedEq : UnsupportedKind → UnsupportedKind → Bool
  | .formatRevision, .formatRevision => true
  | .nodeForm, .nodeForm => true
  | .schemeForm, .schemeForm => true
  | _, _ => false

def limitEq : LimitKind → LimitKind → Bool
  | .bytes, .bytes => true
  | .nodes, .nodes => true
  | .depth, .depth => true
  | .typeSize, .typeSize => true
  | .stackHeight, .stackHeight => true
  | .work, .work => true
  | .diagnostics, .diagnostics => true
  | _, _ => false

def interfaceEq (a b : Interface) : Bool :=
  a.stackIn == b.stackIn && a.stackOut == b.stackOut && a.effects == b.effects

def checkedEq (a b : Checked) : Bool :=
  interfaceEq a.interface b.interface && a.derivations.length == b.derivations.length
    && (a.derivations.zip b.derivations).all (fun pair =>
      pair.1.node == pair.2.node && interfaceEq pair.1.interface pair.2.interface)

def outcomeEq : Outcome → Outcome → Bool
  | .accepted a, .accepted b => checkedEq a b
  | .invalid a, .invalid b => diagnosticEq a b
  | .unsupported a, .unsupported b => unsupportedEq a b
  | .exhausted a, .exhausted b => limitEq a b
  | .internalFailure, .internalFailure => true
  | _, _ => false

/-! ## `sequence_and_literals_accept_arithmetic`

Kernel: `Accepted`; three derivations; `stack_out = [i64]`. -/

def sequenceCandidate : Candidate := candidateOf
  [ litNode (.i64 41) [],
    litNode (.i64 1) [.i64],
    invoke addDef [seg []] ]
  [0, 1, 2]

def sequenceReference : Outcome :=
  check bootstrapEnv (requestOf [] [.i64] []) sequenceCandidate

def sequenceKernel : Outcome := .accepted
  ⟨⟨tyList [], tyList [.i64], EffSet.empty⟩,
    [ ⟨0, ⟨tyList [], tyList [.i64], EffSet.empty⟩⟩,
      ⟨1, ⟨tyList [.i64], tyList [.i64, .i64], EffSet.empty⟩⟩,
      ⟨2, ⟨tyList [.i64, .i64], tyList [.i64], EffSet.empty⟩⟩ ]⟩

/-! ## `quotation_construction_checks_body_and_runs`

The `forty_two` program, also exercised at the limit boundaries below.
Kernel: `Accepted` with five derivations. -/

def progI64 : Ty := .program (tyList [.i64]) (tyList [.i64]) EffSet.empty

def fortyTwoCandidate : Candidate := candidateOf
  [ litNode (.i64 41) [],
    litNode (.i64 1) [.i64],
    invoke addDef [seg []],
    quoteNode [1, 2] [.i64] [.i64] [.i64] [],
    invoke runDef [seg [.i64], seg [.i64], eff []] ]
  [0, 3, 4]

def fortyTwoRequest : Request := requestOf [] [.i64] []

def fortyTwoReference : Outcome := check bootstrapEnv fortyTwoRequest fortyTwoCandidate

def fortyTwoKernel : Outcome := .accepted
  ⟨⟨tyList [], tyList [.i64], EffSet.empty⟩,
    [ ⟨0, ⟨tyList [], tyList [.i64], EffSet.empty⟩⟩,
      ⟨1, ⟨tyList [.i64], tyList [.i64, .i64], EffSet.empty⟩⟩,
      ⟨2, ⟨tyList [.i64, .i64], tyList [.i64], EffSet.empty⟩⟩,
      ⟨3, ⟨tyList [.i64], tyList [.i64, progI64], EffSet.empty⟩⟩,
      ⟨4, ⟨tyList [.i64, progI64], tyList [.i64], EffSet.empty⟩⟩ ]⟩

/-! ## `limits_boundaries_and_exhaustion`

Kernel: accepted at `nodes = 5`, `depth = 1`, and `work = 8`; exhausted
(`nodes`, `depth`, `work`) one step below each boundary. -/

def nodesExactReference : Outcome :=
  check bootstrapEnv { fortyTwoRequest with limits := { baseLimits with nodes := 5 } }
    fortyTwoCandidate

def nodesOverReference : Outcome :=
  check bootstrapEnv { fortyTwoRequest with limits := { baseLimits with nodes := 4 } }
    fortyTwoCandidate

def depthDeepReference : Outcome :=
  check bootstrapEnv { fortyTwoRequest with limits := { baseLimits with depth := 1 } }
    fortyTwoCandidate

def depthShallowReference : Outcome :=
  check bootstrapEnv { fortyTwoRequest with limits := { baseLimits with depth := 0 } }
    fortyTwoCandidate

/-- The single-unit-literal program checked at the work boundary. -/
def unitCandidate : Candidate := candidateOf [litNode .unit []] [0]

def workEightReference : Outcome :=
  check bootstrapEnv { requestOf [] [.unit] [] with limits := { baseLimits with work := 8 } }
    unitCandidate

/-- Kernel: `Accepted`; one derivation; `stack_out = [unit]`. -/
def workEightKernel : Outcome := .accepted
  ⟨⟨tyList [], tyList [.unit], EffSet.empty⟩,
    [⟨0, ⟨tyList [], tyList [.unit], EffSet.empty⟩⟩]⟩

def workFourReference : Outcome :=
  check bootstrapEnv { requestOf [] [.unit] [] with limits := { baseLimits with work := 4 } }
    unitCandidate

/-! ## `hidden_emit_rejects_against_empty_bound`

Kernel: `Invalid`; constraint `EffectInclusion(0)`; empty stacks. -/

def hiddenEmitCandidate : Candidate := candidateOf
  [ invoke emitDef [seg []],
    quoteNode [0] [] [.text] [.unit] [] ]
  [1]

def hiddenEmitRequest : Request :=
  requestOf [] [.program (tyList [.text]) (tyList [.unit]) EffSet.empty] [0]

def hiddenEmitReference : Outcome :=
  check bootstrapEnv hiddenEmitRequest hiddenEmitCandidate

def hiddenEmitKernel : Outcome :=
  .invalid ⟨none, none, [], [], .effectInclusion 0, false, false⟩

/-! ## `diagnostics_report_order_shape_and_truncation`

Kernel: `Invalid`; constraint `StackOrder`; `expected = [bool, i64]` and
`actual = [i64, bool]` (wrong order, not wrong shape); with a diagnostic
budget of one entry, truncated to one stack entry in total. -/

def orderCandidate : Candidate := candidateOf
  [ litNode (.i64 7) [],
    litNode (.bool true) [.i64] ]
  [0, 1]

def orderRequest : Request := requestOf [] [.bool, .i64] []

def orderReference : Outcome := check bootstrapEnv orderRequest orderCandidate

def orderKernel : Outcome :=
  .invalid ⟨none, none, [.bool, .i64], [.i64, .bool], .stackOrder, false, false⟩

def tinyReference : Outcome :=
  check bootstrapEnv { orderRequest with limits := { baseLimits with diagnostics := 1 } }
    orderCandidate

def tinyKernel : Outcome :=
  .invalid ⟨none, none, [], [.i64], .stackOrder, false, true⟩

/-! ## `resource_eligibility_rejects_duplication`

Kernel: `Invalid`; constraint `Eligibility(Resource(0))`; empty stacks. -/

def makerScheme : Scheme :=
  schemeOf [Kind.stack]
    (PartList.ofList [stackVar 0])
    (PartList.ofList [stackVar 0, patternPart (.resource fixtureResource)])
    SlotList.nil

/-- The bootstrap environment plus the fixture's resource-maker definition. -/
def eligibilityEnv : Env :=
  { defs := bootstrapTable.map (fun entry => entry.2) ++ [makerScheme],
    kinds := bootstrapTable.map (fun entry => entry.1) ++ [Behavior.named],
    effects := [testEmitEffect] }

def eligibilityCandidate : Candidate := candidateOf
  [ invoke makerDef [seg []],
    invoke dupDef [seg [], val (.resource fixtureResource)] ]
  [0, 1]

def eligibilityReference : Outcome :=
  check eligibilityEnv (requestOf [] [.resource fixtureResource] []) eligibilityCandidate

def eligibilityKernel : Outcome :=
  .invalid ⟨none, none, [], [], .eligibility (.resource fixtureResource), false, false⟩

/-! ## Agreement harness -/

/-- One agreement row: a fixture, the reference outcome the checker computes,
and the kernel outcome the Rust test asserts. -/
structure Row where
  name : String
  reference : Outcome
  kernel : Outcome

def rows : List Row :=
  [ ⟨"sequence_and_literals_accept_arithmetic", sequenceReference, sequenceKernel⟩,
    ⟨"quotation_construction_checks_body_and_runs", fortyTwoReference, fortyTwoKernel⟩,
    ⟨"limits_nodes_exact", nodesExactReference, fortyTwoKernel⟩,
    ⟨"limits_nodes_over", nodesOverReference, .exhausted .nodes⟩,
    ⟨"limits_depth_deep", depthDeepReference, fortyTwoKernel⟩,
    ⟨"limits_depth_shallow", depthShallowReference, .exhausted .depth⟩,
    ⟨"limits_work_eight", workEightReference, workEightKernel⟩,
    ⟨"limits_work_four", workFourReference, .exhausted .work⟩,
    ⟨"hidden_emit_rejects_against_empty_bound", hiddenEmitReference, hiddenEmitKernel⟩,
    ⟨"diagnostics_report_order", orderReference, orderKernel⟩,
    ⟨"diagnostics_truncation", tinyReference, tinyKernel⟩,
    ⟨"resource_eligibility_rejects_duplication", eligibilityReference, eligibilityKernel⟩ ]

/- Every fixture's reference outcome agrees with the kernel's. -/
#eval rows.all (fun row => outcomeEq row.reference row.kernel)

/- The per-fixture agreement table. -/
#eval rows.map (fun row => (row.name, outcomeEq row.reference row.kernel))

end NobleM2.Fixtures
