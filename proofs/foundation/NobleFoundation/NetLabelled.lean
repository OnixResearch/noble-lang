import InteractionNets

/-!
# Labelled scalar interaction system (issue #105)

This module extends the revision-three I64 symbol set with label-indexed
superposition and duplication agents. Its reduction relation is an actual
`INet.System`; the inets kernel results therefore apply to every accepted
well-formed net over this vocabulary.
-/

namespace NobleFoundation.NetLabelled

/-- The scalar operations carried by the two-stage I64 agents. -/
inductive Op where
  | add
  | sub
  | mul
  | eq
  deriving DecidableEq, Repr

/-- Wrapping I64 semantics for a completed arithmetic pair. -/
def applyOp : Op → BitVec 64 → BitVec 64 → BitVec 64
  | .add, a, b => a + b
  | .sub, a, b => a - b
  | .mul, a, b => a * b
  | .eq, a, b => if a = b then BitVec.ofNat 64 1 else BitVec.ofNat 64 0

/-- Revision-three arithmetic, plus separately labelled DUP/SUP symbols. -/
inductive Symbol where
  | num (value : BitVec 64)
  | op2 (op : Op)
  | op1 (op : Op) (first : BitVec 64)
  | dup
  | dupL (label : Nat)
  | sup (label : Nat)
  | era
  deriving DecidableEq, Repr

/-- Number of auxiliary ports; inets adds principal port zero. -/
def arity : Symbol → Nat
  | .num _ => 0
  | .op2 _ => 2
  | .op1 _ _ => 1
  | .dup => 2
  | .dupL _ => 2
  | .sup _ => 2
  | .era => 0

private def duplicateNumber (value : BitVec 64) : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some false, 0), .num value), ((some false, 1), .num value)]
    [(.ifc false 0, .new (some false) 0 0), (.ifc false 1, .new (some false) 1 0)]).toX

private def duplicateErase : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some false, 0), .era), ((some false, 1), .era)]
    [(.ifc false 0, .new (some false) 0 0), (.ifc false 1, .new (some false) 1 0)]).toX

private def eraseNumber : INet.XRhs Symbol :=
  (INet.RuleData.ofWires [] []).toX

private def eraseSup (_label : Nat) : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some true, 0), .era), ((some true, 1), .era)]
    [(.ifc true 0, .new (some true) 0 0), (.ifc true 1, .new (some true) 1 0)]).toX

private def project : INet.XRhs Symbol :=
  (INet.RuleData.ofWires []
    [(.ifc false 0, .ifc true 0), (.ifc false 1, .ifc true 1)]).toX

private def differentLabels (duplicateLabel superLabel : Nat) : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some false, 0), .sup superLabel), ((some false, 1), .sup superLabel),
      ((some true, 0), .dupL duplicateLabel), ((some true, 1), .dupL duplicateLabel)]
    [(.ifc false 0, .new (some false) 0 0), (.ifc false 1, .new (some false) 1 0),
      (.ifc true 0, .new (some true) 0 0), (.ifc true 1, .new (some true) 1 0),
      (.new (some false) 0 1, .new (some true) 0 1),
      (.new (some false) 0 2, .new (some true) 1 1),
      (.new (some false) 1 1, .new (some true) 0 2),
      (.new (some false) 1 2, .new (some true) 1 2)]).toX

private def op2Number (op : Op) (first : BitVec 64) : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some false, 0), .op1 op first)]
    [(.ifc false 0, .new (some false) 0 0), (.ifc false 1, .new (some false) 0 1)]).toX

private def op1Number (op : Op) (first second : BitVec 64) : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some false, 0), .num (applyOp op first second))]
    [(.ifc false 0, .new (some false) 0 0)]).toX

private def op2Sup (op : Op) (label : Nat) : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some false, 0), .sup label), ((some false, 1), .dupL label),
      ((some true, 0), .op2 op), ((some true, 1), .op2 op)]
    [(.ifc false 0, .new (some false) 1 0), (.ifc false 1, .new (some false) 0 0),
      (.ifc true 0, .new (some true) 0 0), (.ifc true 1, .new (some true) 1 0),
      (.new (some false) 1 1, .new (some true) 0 1),
      (.new (some false) 1 2, .new (some true) 1 1),
      (.new (some true) 0 2, .new (some false) 0 1),
      (.new (some true) 1 2, .new (some false) 0 2)]).toX

private def op1Sup (op : Op) (first : BitVec 64) (label : Nat) : INet.XRhs Symbol :=
  (INet.RuleData.ofWires
    [((some false, 0), .sup label), ((some true, 0), .op1 op first),
      ((some true, 1), .op1 op first)]
    [(.ifc false 0, .new (some false) 0 0),
      (.ifc true 0, .new (some true) 0 0), (.ifc true 1, .new (some true) 1 0),
      (.new (some true) 0 1, .new (some false) 0 1),
      (.new (some true) 1 1, .new (some false) 0 2)]).toX

/-- Finite labelled rule table in executable inets form. -/
def executableRule : Symbol → Symbol → Option (INet.XRhs Symbol)
  | .dupL left, .sup right =>
      if left = right then some project else some (differentLabels left right)
  | .sup right, .dupL left =>
      if left = right then some project.swap else some (differentLabels left right).swap
  | .dupL _, .num value => some (duplicateNumber value)
  | .num value, .dupL _ => some (duplicateNumber value).swap
  | .dup, .num value => some (duplicateNumber value)
  | .num value, .dup => some (duplicateNumber value).swap
  | .dupL _, .era => some duplicateErase
  | .era, .dupL _ => some duplicateErase.swap
  | .era, .num _ => some eraseNumber
  | .num _, .era => some eraseNumber.swap
  | .era, .sup label => some (eraseSup label)
  | .sup label, .era => some (eraseSup label).swap
  | .op2 op, .num first => some (op2Number op first)
  | .num first, .op2 op => some (op2Number op first).swap
  | .op1 op first, .num second => some (op1Number op first second)
  | .num second, .op1 op first => some (op1Number op first second).swap
  | .op2 op, .sup label => some (op2Sup op label)
  | .sup label, .op2 op => some (op2Sup op label).swap
  | .op1 op first, .sup label => some (op1Sup op first label)
  | .sup label, .op1 op first => some (op1Sup op first label).swap
  | _, _ => none

/-- Declarative rules represented by `executableRule`. -/
def rule : Symbol → Symbol → Option (INet.Rhs Symbol) :=
  fun left right => (executableRule left right).map INet.XRhs.toRhs

private theorem rhsSwapInvolutive (R : INet.Rhs Symbol) : R.swap.swap = R := by
  cases R
  simp [INet.Rhs.swap]

private theorem duplicateNumberWF (value : BitVec 64) (left : Symbol)
    (hleft : arity left = 2) : (duplicateNumber value).toRhs.WF arity left (.num value) := by
  cases left <;> simp [arity] at hleft
  all_goals
    exact (INet.RuleData.checkWF_sound (by
      simp [INet.RuleData.checkWF, INet.RuleData.ofWires, INet.RuleData.ports,
        INet.ifcPorts, INet.assoc, arity]
      all_goals
        intro x hx
        cases x with
        | zero => simp
        | succ x =>
          cases x with
          | zero => simp
          | succ x => omega)).2

macro "solvePortCover" : tactic =>
  `(tactic|
    exact (by
      intro x hx
      by_cases hx0 : x = 0
      · subst x
        simp_all
      · by_cases hx1 : x = 1
        · subst x
          simp_all
        · by_cases hx2 : x = 2
          · subst x
            simp_all
          · omega))

macro "solveRuleDataCheck" : tactic =>
  `(tactic|
    (simp [INet.RuleData.checkWF, INet.RuleData.ofWires, INet.RuleData.ports,
      INet.ifcPorts, INet.assoc, arity]
      <;> repeat' (first | constructor | solvePortCover)))

private theorem duplicateEraseWF (label : Nat) :
    duplicateErase.toRhs.WF arity (.dupL label) .era :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem eraseNumberWF (value : BitVec 64) :
    eraseNumber.toRhs.WF arity .era (.num value) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem eraseSupWF (label : Nat) :
    (eraseSup label).toRhs.WF arity .era (.sup label) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem projectWF (label : Nat) :
    project.toRhs.WF arity (.dupL label) (.sup label) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem differentLabelsWF (left right : Nat) :
    (differentLabels left right).toRhs.WF arity (.dupL left) (.sup right) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem op2NumberWF (op : Op) (first : BitVec 64) :
    (op2Number op first).toRhs.WF arity (.op2 op) (.num first) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem op1NumberWF (op : Op) (first second : BitVec 64) :
    (op1Number op first second).toRhs.WF arity (.op1 op first) (.num second) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem op2SupWF (op : Op) (label : Nat) :
    (op2Sup op label).toRhs.WF arity (.op2 op) (.sup label) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

private theorem op1SupWF (op : Op) (first : BitVec 64) (label : Nat) :
    (op1Sup op first label).toRhs.WF arity (.op1 op first) (.sup label) :=
  (INet.RuleData.checkWF_sound (by solveRuleDataCheck)).2

/-- The labelled DUP/SUP and plain revision-three I64 interaction rules. -/
def system : INet.System Symbol where
  ar := arity
  rule := rule
  rule_wf := by
    intro α β R h
    cases α <;> cases β <;> simp [rule, executableRule] at h
    all_goals try split at h
    case dupL.sup.isTrue =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      subst_vars
      exact projectWF _
    case dupL.sup.isFalse =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      exact differentLabelsWF _ _
    case sup.dupL.isTrue =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      subst_vars
      rw [INet.XRhs.toRhs_swap]
      exact (projectWF _).swap
    case sup.dupL.isFalse =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      rw [INet.XRhs.toRhs_swap]
      exact (differentLabelsWF _ _).swap
    all_goals
      subst_vars
      try simp only [INet.XRhs.toRhs_swap]
      first
      | exact duplicateNumberWF _ _ rfl
      | exact (duplicateNumberWF _ _ rfl).swap
      | exact duplicateEraseWF _
      | exact (duplicateEraseWF _).swap
      | exact eraseNumberWF _
      | exact (eraseNumberWF _).swap
      | exact eraseSupWF _
      | exact (eraseSupWF _).swap
      | exact projectWF _
      | exact (projectWF _).swap
      | exact differentLabelsWF _ _
      | exact (differentLabelsWF _ _).swap
      | exact op2NumberWF _ _
      | exact (op2NumberWF _ _).swap
      | exact op1NumberWF _ _ _
      | exact (op1NumberWF _ _ _).swap
      | exact op2SupWF _ _
      | exact (op2SupWF _ _).swap
      | exact op1SupWF _ _ _
      | exact (op1SupWF _ _ _).swap

  rule_symm := by
    intro α β R h
    cases α <;> cases β <;> simp [rule, executableRule] at h
    all_goals try split at h
    case dupL.sup.isTrue =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      subst_vars
      refine ⟨project.toRhs.swap, ?_, INet.Rhs.mirror_swap _⟩
      simp [rule, executableRule, INet.XRhs.toRhs_swap]
    case dupL.sup.isFalse dupLabel supLabel hne =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      refine ⟨(differentLabels dupLabel supLabel).toRhs.swap, ?_,
        INet.Rhs.mirror_swap (differentLabels dupLabel supLabel).toRhs⟩
      simp_all [rule, executableRule, INet.XRhs.toRhs_swap]
    case sup.dupL.isTrue =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      subst_vars
      refine ⟨project.toRhs, ?_, ?_⟩
      · simp [rule, executableRule]
      · simpa only [INet.XRhs.toRhs_swap] using INet.Rhs.swap_mirror project.toRhs
    case sup.dupL.isFalse supLabel dupLabel hne =>
      rcases h with ⟨a, ha, hR⟩
      simp only [Option.some.injEq] at ha
      subst a
      subst R
      refine ⟨(differentLabels dupLabel supLabel).toRhs, ?_, ?_⟩
      · simp_all [rule, executableRule]
      · simpa only [INet.XRhs.toRhs_swap] using
          INet.Rhs.swap_mirror (differentLabels dupLabel supLabel).toRhs
    all_goals
      refine ⟨R.swap, ?_, INet.Rhs.mirror_swap R⟩
      rw [← h]
      simp [rule, executableRule, INet.XRhs.toRhs_swap, rhsSwapInvolutive]

private theorem ruleDataSlots (D : INet.RuleData Symbol) {o : Option Bool} {j : Nat}
    {γ : Symbol} (h : D.toX.lab o j = some γ) : (o, j) ∈ D.toX.slots := by
  change INet.assoc (o, j) D.agents = some γ at h
  change (o, j) ∈ D.agents.map (·.1)
  exact List.mem_map.2 ⟨((o, j), γ), INet.assoc_mem h, rfl⟩

/-- Executable presentation of the labelled system, with slot coverage proved. -/
def executableSystem : INet.XSys Symbol where
  sys := system
  rule := executableRule
  rule_spec := by
    intro left right
    rfl
  slots_spec := by
    intro left right R owner slot γ hRule hLabel
    cases left <;> cases right <;> simp [executableRule] at hRule
    all_goals try split at hRule
    all_goals
      try simp only [Option.some.injEq] at hRule
      rw [← hRule] at hLabel ⊢
      first
      | exact ruleDataSlots _ hLabel
      | exact INet.XRhs.swap_slots (ruleDataSlots _) hLabel

/-- Strong confluence for every well-formed labelled net, by inets' generic theorem. -/
theorem labelledStrongConfluent :
    INet.StrongConfluent (fun N : INet.Net Symbol => N.WF arity) (INet.Step system) :=
  INet.strongConfluent system

/-- #105 confluence corollary, with equal total lengths for the completing paths. -/
theorem labelled_confluence {N N₁ N₂ : INet.Net Symbol} {n m : Nat}
    (hN : N.WF arity)
    (h₁ : INet.Steps (INet.Step system) n N N₁)
    (h₂ : INet.Steps (INet.Step system) m N N₂) :
    ∃ N' k l, INet.Steps (INet.Step system) k N₁ N' ∧
      INet.Steps (INet.Step system) l N₂ N' ∧ n + k = m + l :=
  INet.confluence hN h₁ h₂

/-- Any two normal forms reached from one labelled net are equal, with equal
reduction lengths. -/
theorem labelled_normal_form_unique {N N₁ N₂ : INet.Net Symbol} {n m : Nat}
    (hN : N.WF arity)
    (h₁ : INet.Steps (INet.Step system) n N N₁)
    (h₂ : INet.Steps (INet.Step system) m N N₂)
    (hn₁ : INet.Normal (INet.Step system) N₁)
    (hn₂ : INet.Normal (INet.Step system) N₂) :
    N₁ = N₂ ∧ n = m :=
  INet.normal_form_unique hN h₁ h₂ hn₁ hn₂

/-- Every competing reduction of a net with a normal form is bounded by its
normalizing path and extends to that same normal form. -/
theorem labelled_reduction_bounded {N N₁ N₂ : INet.Net Symbol} {n m : Nat}
    (hN : N.WF arity)
    (h₁ : INet.Steps (INet.Step system) n N N₁)
    (hNormal : INet.Normal (INet.Step system) N₁)
    (h₂ : INet.Steps (INet.Step system) m N N₂) :
    m ≤ n ∧ INet.Steps (INet.Step system) (n - m) N₂ N₁ :=
  INet.reduction_bounded hN h₁ hNormal h₂

/-- Frozen admission caps for the model profile. -/
def maxLabels : Nat := 16
def maxAgents : Nat := 64
def maxWireEntries : Nat := 192
def maxBoundaryPorts : Nat := 16
def maxRewriteSteps : Nat := 256
def maxNesting : Nat := 8
def maxCollapsedValues : Nat := 256

/-- Labelled agents must use one of the net's declared labels. -/
def labelAllowed (labels : List Nat) : Symbol → Bool
  | .dupL label => decide (label ∈ labels)
  | .sup label => decide (label ∈ labels)
  | _ => true

/-- Structural and finite-label admission for a labelled executable net. -/
def validate (labels : List Nat) (X : INet.XNet Symbol) : Bool :=
  X.check arity && decide labels.Nodup && decide (labels.length ≤ maxLabels) &&
    decide (X.agents.length ≤ maxAgents) && decide (X.wires.length ≤ maxWireEntries) &&
    decide (X.free.length ≤ maxBoundaryPorts) &&
    X.agents.all (fun agent => labelAllowed labels agent.2)

/-- Validation establishes inets well-formedness, a duplicate-free bounded
label declaration, resource caps, and membership for every labelled agent. -/
theorem validate_sound {labels : List Nat} {X : INet.XNet Symbol}
    (h : validate labels X = true) :
    X.Ok arity ∧ labels.Nodup ∧ labels.length ≤ maxLabels ∧
      X.agents.length ≤ maxAgents ∧ X.wires.length ≤ maxWireEntries ∧
      X.free.length ≤ maxBoundaryPorts ∧
      ∀ agent ∈ X.agents, labelAllowed labels agent.2 = true := by
  simp only [validate, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq] at h
  rcases h with
    ⟨⟨⟨⟨⟨⟨hNet, hLabels⟩, hLabelCount⟩, hAgentCount⟩, hWireCount⟩, hBoundaryCount⟩,
      hAgents⟩
  exact ⟨INet.XNet.check_iff_ok.1 hNet, hLabels, hLabelCount, hAgentCount,
    hWireCount, hBoundaryCount, hAgents⟩

/-- Finite superposition value trees used for bounded read-back. -/
inductive ValueTree where
  | num (value : BitVec 64)
  | sup (label : Nat) (left right : ValueTree)
  deriving DecidableEq, Repr

/-- Maximum number of nested SUP nodes on a path. -/
def ValueTree.depth : ValueTree → Nat
  | .num _ => 0
  | .sup _ left right => max left.depth right.depth + 1

/-- Number of agents represented by a finite value tree. -/
def ValueTree.nodeCount : ValueTree → Nat
  | .num _ => 1
  | .sup _ left right => 1 + left.nodeCount + right.nodeCount

/-- Labels occurring in a read-back tree, with duplicates retained. -/
def ValueTree.labels : ValueTree → List Nat
  | .num _ => []
  | .sup label left right => label :: (left.labels ++ right.labels)

/-- Ordered collapse emits the left branch before the right branch. -/
def collapse : ValueTree → List (BitVec 64)
  | .num value => [value]
  | .sup _ left right => collapse left ++ collapse right

private def declaredTreeLabels (declared : List Nat) (treeLabels : List Nat) : Bool :=
  treeLabels.all (fun label => decide (label ∈ declared))

/-- Read-back admission also enforces the frozen nesting and output-size limits. -/
def validateTree (declared : List Nat) (tree : ValueTree) : Bool :=
  decide declared.Nodup && decide (tree.depth ≤ maxNesting) &&
    declaredTreeLabels declared tree.labels &&
    decide ((collapse tree).length ≤ maxCollapsedValues)

/-- Whole-boundary read-back must meet the output-count and aggregate collapse caps. -/
def validateReadback (declared : List Nat) (trees : List ValueTree) : Bool :=
  decide (trees.length ≤ maxBoundaryPorts) && trees.all (validateTree declared) &&
    decide ((trees.map collapse).foldl (fun count values => count + values.length) 0 ≤
      maxCollapsedValues)

/-- Read-back admission proves every per-tree limit and the whole-boundary caps. -/
theorem validateReadback_sound {declared : List Nat} {trees : List ValueTree}
    (h : validateReadback declared trees = true) :
    trees.length ≤ maxBoundaryPorts ∧
      (∀ tree ∈ trees, validateTree declared tree = true) ∧
      (trees.map collapse).foldl (fun count values => count + values.length) 0 ≤
        maxCollapsedValues := by
  simp only [validateReadback, Bool.and_eq_true, List.all_eq_true,
    decide_eq_true_eq] at h
  rcases h with ⟨⟨hCount, hTrees⟩, hCollapsed⟩
  exact ⟨hCount, hTrees, hCollapsed⟩

/-- A valid value tree uses declared labels and stays within depth/output caps. -/
theorem validateTree_sound {declared : List Nat} {tree : ValueTree}
    (h : validateTree declared tree = true) :
    declared.Nodup ∧ tree.depth ≤ maxNesting ∧
      (∀ label ∈ tree.labels, label ∈ declared) ∧
      (collapse tree).length ≤ maxCollapsedValues := by
  simp only [validateTree, Bool.and_eq_true, List.all_eq_true, decide_eq_true_eq,
    declaredTreeLabels] at h
  rcases h with ⟨⟨⟨hLabels, hDepth⟩, hTreeLabels⟩, hCollapsed⟩
  exact ⟨hLabels, hDepth, hTreeLabels, hCollapsed⟩

/-- Read a normal-form value from a free boundary port. Cyclic or too-deep
graphs fail closed when the fixed nesting fuel is exhausted. -/
def readbackFuel : Nat → INet.XNet Symbol → INet.Port → Option ValueTree
  | 0, _, _ => none
  | fuel + 1, X, root =>
    match INet.assoc root X.wires with
    | none => none
    | some (agentId, port) =>
      if port = 0 then
        match INet.assoc agentId X.agents with
        | some (.num value) => some (.num value)
        | some (.sup label) => do
          let left ← readbackFuel fuel X (agentId, 1)
          let right ← readbackFuel fuel X (agentId, 2)
          pure (.sup label left right)
        | _ => none
      else
        none

/-- The profile's read-back fuel allows exactly its maximum SUP depth. -/
def readback (X : INet.XNet Symbol) (root : INet.Port) : Option ValueTree :=
  readbackFuel (maxNesting + 1) X root

/-- Read ordered values at all requested free boundary ports. -/
def readbackOutputs (X : INet.XNet Symbol) (roots : List INet.Port) : Option (List ValueTree) :=
  roots.mapM (readback X)

/-- Reduce to a checked normal form, then read the complete ordered boundary.
The agent-count check refuses hidden or unobserved normal-form agents. -/
def observe (labels : List Nat) (fuel : Nat) (X : INet.XNet Symbol) : Option (List ValueTree) :=
  if decide (fuel ≤ maxRewriteSteps) && validate labels X then
    match INet.XNet.normalize executableSystem fuel X with
    | none => none
    | some (normal, _) => do
      let trees ← readbackOutputs normal normal.free
      if decide (trees.foldl (fun count tree => count + tree.nodeCount) 0 =
        normal.agents.length) && validateReadback labels trees then
        some trees
      else
        none
  else
    none

/-- Read back and collapse each ordered boundary value. -/
def collapseObserve (labels : List Nat) (fuel : Nat) (X : INet.XNet Symbol) :
    Option (List (List (BitVec 64))) :=
  (observe labels fuel X).map (List.map collapse)

private def word (value : Nat) : BitVec 64 := BitVec.ofNat 64 value

private def nestedTree : Nat → ValueTree
  | 0 => .num (word 0)
  | depth + 1 => .sup 7 (nestedTree depth) (.num (word 0))

private def emptyFixture : INet.XNet Symbol := INet.XNet.ofWires [] [] []

theorem declared_label_limit_accepted :
    validate (List.range maxLabels) emptyFixture = true := by decide

theorem declared_label_limit_exceeded_refused :
    validate (List.range (maxLabels + 1)) emptyFixture = false := by decide

theorem nesting_limit_accepted : validateTree [7] (nestedTree maxNesting) = true := by decide

theorem nesting_limit_exceeded_refused :
    validateTree [7] (nestedTree (maxNesting + 1)) = false := by decide

theorem rewrite_fuel_limit_at_completes_empty_net :
    observe [] maxRewriteSteps emptyFixture = some [] := by decide

theorem rewrite_fuel_limit_exceeded_refused :
    observe [] (maxRewriteSteps + 1) emptyFixture = none := by decide

/-- Correct labelled DUP semantics on finite value trees. -/
def duplicateValue (label : Nat) : ValueTree → ValueTree × ValueTree
  | .num value => (.num value, .num value)
  | .sup other left right =>
    if label = other then
      (left, right)
    else
      let (left0, left1) := duplicateValue label left
      let (right0, right1) := duplicateValue label right
      (.sup other left0 right0, .sup other left1 right1)

/-- Mutant that inverts the DUP/SUP label-equality guard. -/
def duplicateValueWrongEquality (label : Nat) : ValueTree → ValueTree × ValueTree
  | .num value => (.num value, .num value)
  | .sup other left right =>
    if label ≠ other then
      (left, right)
    else
      let (left0, left1) := duplicateValueWrongEquality label left
      let (right0, right1) := duplicateValueWrongEquality label right
      (.sup other left0 right0, .sup other left1 right1)

/-- Mutant that omits the required SUP constructor at different labels. -/
def duplicateValueMissingNest (label : Nat) : ValueTree → ValueTree × ValueTree
  | .num value => (.num value, .num value)
  | .sup other left right =>
    if label = other then
      (left, right)
    else
      let (left0, _) := duplicateValueMissingNest label left
      let (right0, _) := duplicateValueMissingNest label right
      (left0, right0)

/-- Mutant that swaps the two equal-label projection branches. -/
def duplicateValueSwapped (label : Nat) (tree : ValueTree) : ValueTree × ValueTree :=
  let (left, right) := duplicateValue label tree
  (right, left)

/-- Distribute a completed first arithmetic operand through an ordered SUP tree. -/
def applyOpTree (op : Op) (first : BitVec 64) : ValueTree → ValueTree
  | .num second => .num (applyOp op first second)
  | .sup label left right =>
    .sup label (applyOpTree op first left) (applyOpTree op first right)

/-- Mutant that erases a SUP and keeps only its left arithmetic result. -/
def applyOpTreeErasingSup (op : Op) (first : BitVec 64) : ValueTree → ValueTree
  | .num second => .num (applyOp op first second)
  | .sup _ left _ => applyOpTreeErasingSup op first left

/-- The four seeded semantic faults are distinguished by the frozen oracle. -/
theorem wrong_label_equality_seed_caught :
    duplicateValueWrongEquality 4 (.sup 5 (.num (word 3)) (.num (word 7))) ≠
      duplicateValue 4 (.sup 5 (.num (word 3)) (.num (word 7))) := by decide

theorem swapped_branches_seed_caught :
    duplicateValueSwapped 4 (.sup 4 (.num (word 3)) (.num (word 7))) ≠
      duplicateValue 4 (.sup 4 (.num (word 3)) (.num (word 7))) := by decide

theorem missing_nest_seed_caught :
    duplicateValueMissingNest 4 (.sup 5 (.num (word 3)) (.num (word 7))) ≠
      duplicateValue 4 (.sup 5 (.num (word 3)) (.num (word 7))) := by decide

theorem erased_sup_seed_caught :
    collapse (applyOpTree .add (word 2) (.sup 7 (.num (word 3)) (.num (word 5)))) ≠
      collapse (applyOpTreeErasingSup .add (word 2)
        (.sup 7 (.num (word 3)) (.num (word 5)))) := by decide

/-- Equal-label DUP/SUP projection with two ordered scalar outputs. -/
def equalLabelFixture : INet.XNet Symbol :=
  INet.XNet.ofWires
    [([0], .dupL 4), ([1], .sup 4), ([2], .num (word 3)), ([3], .num (word 7))]
    [([4], 0), ([5], 0)]
    [(([0], 0), ([1], 0)), (([0], 1), ([2], 0)), (([0], 2), ([3], 0)),
      (([1], 1), ([4], 0)), (([1], 2), ([5], 0))]

/-- Different-label DUP/SUP nesting, with two independent ordered outputs. -/
def differentLabelFixture : INet.XNet Symbol :=
  INet.XNet.ofWires
    [([0], .dupL 4), ([1], .sup 5), ([2], .num (word 3)), ([3], .num (word 7))]
    [([4], 0), ([5], 0)]
    [(([0], 0), ([1], 0)), (([0], 1), ([4], 0)), (([0], 2), ([5], 0)),
      (([1], 1), ([2], 0)), (([1], 2), ([3], 0))]

/-- One OP1/SUP interaction followed by both scalar branches. -/
def opSupFixture : INet.XNet Symbol :=
  INet.XNet.ofWires
    [([0], .op1 .add (word 2)), ([1], .sup 7),
      ([2], .num (word 3)), ([3], .num (word 5))]
    [([4], 0)]
    [(([0], 0), ([1], 0)), (([0], 1), ([4], 0)),
      (([1], 1), ([2], 0)), (([1], 2), ([3], 0))]

/-- OP2/SUP distributes the first operand and duplicates the other operand. -/
def op2SupFixture : INet.XNet Symbol :=
  INet.XNet.ofWires
    [([0], .op2 .add), ([1], .sup 7), ([2], .num (word 3)),
      ([3], .num (word 5)), ([4], .num (word 10))]
    [([5], 0)]
    [(([0], 0), ([1], 0)), (([0], 1), ([4], 0)), (([0], 2), ([5], 0)),
      (([1], 1), ([2], 0)), (([1], 2), ([3], 0))]

/-- The plain revision-three `op2`/`num`/`op1` path, with no SUP agent. -/
def plainV3Fixture : INet.XNet Symbol :=
  INet.XNet.ofWires
    [([0], .op2 .add), ([1], .num (word 4)), ([2], .num (word 5))]
    [([3], 0)]
    [(([0], 0), ([1], 0)), (([0], 1), ([2], 0)), (([0], 2), ([3], 0))]

/-- A two-output normal form whose second boundary is an unobserved eraser. -/
def partialBoundaryFixture : INet.XNet Symbol :=
  INet.XNet.ofWires
    [([0], .num (word 9)), ([2], .era)]
    [([1], 0), ([3], 0)]
    [(([0], 0), ([1], 0)), (([2], 0), ([3], 0))]

/-- Symbols admitted to the SUP-free v3 rule table, including DUP-L as the
same fanout operation when no SUP is present. -/
def supFreeSymbol : Symbol → Bool
  | .num _ | .op2 _ | .op1 _ _ | .dup | .dupL _ | .era => true
  | .sup _ => false

/-- The v3 scalar rules, extended only by label-insensitive DUP-L behaviour. -/
def plainV3Rule : Symbol → Symbol → Option (INet.Rhs Symbol)
  | .dup, .num value => some (duplicateNumber value).toRhs
  | .num value, .dup => some (duplicateNumber value).toRhs.swap
  | .dupL _, .num value => some (duplicateNumber value).toRhs
  | .num value, .dupL _ => some (duplicateNumber value).toRhs.swap
  | .dupL _, .era => some duplicateErase.toRhs
  | .era, .dupL _ => some duplicateErase.toRhs.swap
  | .era, .num _ => some eraseNumber.toRhs
  | .num _, .era => some eraseNumber.toRhs.swap
  | .op2 op, .num first => some (op2Number op first).toRhs
  | .num first, .op2 op => some (op2Number op first).toRhs.swap
  | .op1 op first, .num second => some (op1Number op first second).toRhs
  | .num second, .op1 op first => some (op1Number op first second).toRhs.swap
  | _, _ => none

/-- Any interaction in a SUP-free net uses exactly the plain v3 scalar rule. -/
theorem sup_free_rule_table_exact {left right : Symbol}
    (hLeft : supFreeSymbol left = true) (hRight : supFreeSymbol right = true) :
    rule left right = plainV3Rule left right := by
  cases left <;> cases right <;>
    simp [rule, executableRule, plainV3Rule, supFreeSymbol, INet.XRhs.toRhs_swap] at *

/-- Every validated input net inherits the inets reduction-bound theorem. -/
theorem validated_net_reduction_bound {labels : List Nat} {X : INet.XNet Symbol}
    (hValid : validate labels X = true) {N₁ N₂ : INet.Net Symbol} {n m : Nat}
    (h₁ : INet.Steps (INet.Step system) n X.toNet N₁)
    (hNormal : INet.Normal (INet.Step system) N₁)
    (h₂ : INet.Steps (INet.Step system) m X.toNet N₂) :
    m ≤ n ∧ INet.Steps (INet.Step system) (n - m) N₂ N₁ := by
  exact INet.reduction_bounded ((validate_sound hValid).1.1) h₁ hNormal h₂

theorem equal_label_fixture_validates : validate [4] equalLabelFixture = true := by decide

theorem different_label_fixture_validates :
    validate [4, 5] differentLabelFixture = true := by decide

theorem op_sup_fixture_validates : validate [7] opSupFixture = true := by decide

theorem op2_sup_fixture_validates : validate [7] op2SupFixture = true := by decide

theorem plain_v3_fixture_validates : validate [] plainV3Fixture = true := by decide

theorem partial_boundary_fixture_validates : validate [] partialBoundaryFixture = true := by decide

theorem equal_label_projection_fixture :
    observe [4] 2 equalLabelFixture =
      some [.num (word 3), .num (word 7)] := by decide

theorem different_label_nesting_fixture :
    observe [4, 5] 4 differentLabelFixture =
      some [.sup 5 (.num (word 3)) (.num (word 7)),
        .sup 5 (.num (word 3)) (.num (word 7))] := by decide

theorem op_sup_fixture_ordered_result :
    observe [7] 4 opSupFixture =
      some [.sup 7 (.num (word 5)) (.num (word 7))] := by decide

theorem op_sup_fixture_fuel_exhaustion_refuses :
    observe [7] 2 opSupFixture = none := by decide

theorem op_sup_fixture_ordered_collapse :
    collapseObserve [7] 4 opSupFixture = some [[word 5, word 7]] := by decide

theorem op2_sup_fixture_ordered_result :
    observe [7] 8 op2SupFixture =
      some [.sup 7 (.num (word 13)) (.num (word 15))] := by decide

theorem op2_sup_fixture_ordered_collapse :
    collapseObserve [7] 8 op2SupFixture = some [[word 13, word 15]] := by decide

theorem plain_v3_fixture_matches_scalar_rule_table :
    observe [] 3 plainV3Fixture = some [.num (word 9)] := by decide

theorem incomplete_boundary_readback_refuses_without_prefix :
    observe [] 0 partialBoundaryFixture = none := by decide

theorem validate_rejects_unknown_labels :
    validate [4] differentLabelFixture = false := by decide

end NobleFoundation.NetLabelled
