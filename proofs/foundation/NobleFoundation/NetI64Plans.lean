import NobleFoundation.NetI64

/-!
# Checked revision-3 plan semantics (issue #110)

This is an independent kernel model over the vendored inets hash-map/worklist
machine. It models the plan's captured neighbourhood and result/write image,
then stutters observably through the pre-commit phases and commits one inets
firing. It does not claim correspondence with the Rust byte compiler or the
Rust generation-tagged vector arena; finite Rust differential tests remain a
separate evidence class.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The finite output image produced from one exact revision-3 interaction rule. -/
structure RulePlan where
  left : NetSym
  right : NetSym
  rhs : XRhs NetSym
  results : List (((Option Bool × Nat) × NetSym))
  commitPoint : Nat

/-- Compile the selected revision-3 rule table into its result-agent image. -/
def compileRule (left right : NetSym) : Option RulePlan :=
  (NetI64.xRule left right).map fun rhs =>
    { left := left
      right := right
      rhs := rhs
      results := rhs.slots.filterMap fun slot =>
        (rhs.lab slot.1 slot.2).map fun symbol => (slot, symbol)
      commitPoint := 6 }

/-- A rule-table entry is preserved exactly by the plan-rule compiler. -/
theorem compileRule_exact_rhs {left right : NetSym} {rhs : XRhs NetSym}
    (h : NetI64.xRule left right = some rhs) :
    (compileRule left right).map RulePlan.rhs = some rhs := by
  simp [compileRule, h]

/-- Every operation and payload admits the forward OP2 plan image. -/
theorem compileRule_op2_forward (op : Op) (first : BitVec 64) :
    (compileRule (.op2 op) (.num first)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- Every operation and payload admits the mirrored OP2 plan image. -/
theorem compileRule_op2_reverse (op : Op) (first : BitVec 64) :
    (compileRule (.num first) (.op2 op)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- Every operation and pair of values admits the forward OP1 plan image. -/
theorem compileRule_op1_forward (op : Op) (first second : BitVec 64) :
    (compileRule (.op1 op first) (.num second)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- Every operation and pair of values admits the mirrored OP1 plan image. -/
theorem compileRule_op1_reverse (op : Op) (first second : BitVec 64) :
    (compileRule (.num second) (.op1 op first)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- A DUP/number pair admits the forward duplication plan image. -/
theorem compileRule_dup_forward (value : BitVec 64) :
    (compileRule .dup (.num value)).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- A number/DUP pair admits the mirrored duplication plan image. -/
theorem compileRule_dup_reverse (value : BitVec 64) :
    (compileRule (.num value) .dup).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- An ERA/number pair admits the forward erasure plan image. -/
theorem compileRule_era_forward (value : BitVec 64) :
    (compileRule .era (.num value)).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- A number/ERA pair admits the mirrored erasure plan image. -/
theorem compileRule_era_reverse (value : BitVec 64) :
    (compileRule (.num value) .era).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- The finite executable result for a selected pair in the inets worklist model. -/
def localResult (H : HNet NetSym) (r : XRedex NetSym) : XNet NetSym :=
  let a := r.1
  let b := r.2.1
  let α := r.2.2.1
  let β := r.2.2.2.1
  let R := r.2.2.2.2
  (H.nbhd NetI64.arity a b α β).fire NetI64.arity a b α β R

/-- A checked plan image captured from the inets hash-map/worklist state. -/
structure Plan where
  redex : XRedex NetSym
  ruleValid : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2
  capturedBoundary : List (Port × Port)
  initializedAgents : List (Id × NetSym)
  wireWrites : List (Port × Port)
  newlyActive : List (Id × Id)
  arenaReservation : Nat
  scratchReservation : Nat
  worklistReservation : Nat
  commitPoint : Nat

/-- Capture the local neighbourhood, result agents, wire image, publications and bounds.
The plan phases refine the committed net; physical Rust slot reuse is intentionally
outside this model. -/
def Plan.compile (H : HNet NetSym) (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2) : Plan :=
  let a := r.1
  let b := r.2.1
  let α := r.2.2.1
  let β := r.2.2.2.1
  let fired := localResult H r
  let boundary := H.nearWires NetI64.arity a b α β
  let published := principalPairs fired.wires
  { redex := r
    ruleValid := hRule
    capturedBoundary := boundary
    initializedAgents := fired.agents
    wireWrites := fired.wires
    newlyActive := published
    arenaReservation := r.2.2.2.2.slots.length
    scratchReservation := boundary.length + fired.wires.length
    worklistReservation := published.length
    commitPoint := 6 }

/-- Commit the image produced by a plan: erase both directions of its captured old
boundary, retire the consumed pair, install initialized agents and declared wires,
and publish its checked active-pair list. -/
def Plan.commit (H : HNet NetSym) (p : Plan) (rest : List (Id × Id)) : HNet NetSym :=
  let oldWires := p.capturedBoundary ++ p.capturedBoundary.map fun edge => (edge.2, edge.1)
  let retiredWires := oldWires.foldl (fun wires edge => wires.erase edge.1) H.wires
  let agents := p.initializedAgents.foldl (fun agents edge => agents.insert edge.1 edge.2)
    ((H.agents.erase p.redex.1).erase p.redex.2.1)
  let wires := p.wireWrites.foldl (fun wires edge => wires.insert edge.1 edge.2) retiredWires
  { agents := agents
    wires := wires
    free := H.free
    work := p.newlyActive ++ rest }

/-- Executing a compiled plan image is exactly the corresponding inets worklist firing. -/
theorem Plan.compile_commit (H : HNet NetSym) (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id)) :
    Plan.commit H (Plan.compile H r hRule) rest =
      H.fire NetI64.xsys.sys.ar r.1 r.2.1 r.2.2.1 r.2.2.2.1 r.2.2.2.2 rest := by
  rfl

/-- The worklist published by the commit is exactly the captured local active-pair image. -/
theorem Plan.commit_worklist (H : HNet NetSym) (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id)) :
    (Plan.commit H (Plan.compile H r hRule) rest).work =
      (Plan.compile H r hRule).newlyActive ++ rest := by
  rfl

/-- Before the explicit commit point, every plan phase leaves the abstract net unchanged. -/
inductive Phase where
  | captured
  | reserved
  | initialized
  | rewired
  | retired
  | published
  | committed
  deriving DecidableEq

def observe (H : HNet NetSym) (p : Plan) (rest : List (Id × Id)) : Phase → HNet NetSym
  | .captured => H
  | .reserved => H
  | .initialized => H
  | .rewired => H
  | .retired => H
  | .published => H
  | .committed => Plan.commit H p rest

/-- The six pre-commit microsteps are observational stutters. -/
theorem Plan.precommit_stutters (H : HNet NetSym) (p : Plan)
    (rest : List (Id × Id)) (phase : Phase) (h : phase ≠ .committed) :
    (observe H p rest phase).toNet = H.toNet := by
  cases phase with
  | captured => rfl
  | reserved => rfl
  | initialized => rfl
  | rewired => rfl
  | retired => rfl
  | published => rfl
  | committed => exact (h rfl).elim

/-- A checked plan commit is exactly one abstract revision-3 net reduction step. -/
theorem Plan.commit_refines {H : HNet NetSym} (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id))
    (hm : H.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system H.toNet
      r.1 r.2.1 r.2.2.1 r.2.2.2.1 r.2.2.2.2.toRhs) :
    CStep NetI64.system H.toNet
      (Plan.commit H (Plan.compile H r hRule) rest).toNet := by
  rw [Plan.compile_commit]
  refine ⟨H.toNet.fire r.1 r.2.1 r.2.2.2.2.toRhs, ?_, ?_⟩
  · exact ⟨r.1, r.2.1, r.2.2.1, r.2.2.2.1,
      r.2.2.2.2.toRhs, hredex, rfl⟩
  · simpa [NetI64.xsys] using
      (HNet.fire_toNet (xs := NetI64.xsys) hm hredex hRule rest).1

end NobleFoundation.NetI64Plans
