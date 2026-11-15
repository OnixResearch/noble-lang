import NobleFoundation.NetI64

/-!
# Revision-3 interaction and live-agent resource certificates

This module proves the local unit-cost obligations for every selected NetI64 rule and exposes
the inets-derived count/live-agent theorem for arbitrary legal reduction prefixes and normalizing
schedules. Its certificate envelope is a model of the fields decoded by the Rust byte checker;
the actual binary parser, hash implementation, graph lowering, and Rust/Lean correspondence remain
implementation evidence rather than kernel theorems.
-/

namespace NobleFoundation.NetI64Resource

open INet
open NobleFoundation.NetI64

/-- Wire-format identity shared with the independent Rust byte checker. -/
def resourceCertificateRevision : String := "ngraph-net-resource/1"
def resourceCheckerRevision : String := "ngraph-net-resource-checker/0"
def resourceProofRevision : String := "NobleFoundation.NetI64Resource/0"
def resourceCostRevision : String := "ngraph-net-resource-cost/0"

/-- Time potential paid by one revision-3 agent. The first arithmetic stage carries one
additional unit because it has two firings; the second stage, DUP, and ERA each carry one. -/
def timePotential : NetSym → Nat
  | .num _ => 0
  | .op2 _ => 2
  | .op1 _ _ => 1
  | .dup => 1
  | .era => 1

/-- Committed storage assigns one cell to each live agent; ports, worklists, and arena capacity
are outside this weighted graph-storage model. -/
def spacePotential (_ : NetSym) : Nat := 1

/-- Sum of the weights of the actual replacement agents, including any future selected
rule that creates an agent other than `op1`. -/
def rhsTimePotential (rhs : XRhs NetSym) : Nat :=
  ((rhs.slots.filterMap fun (owner, index) => rhs.lab owner index).map timePotential).sum

/-- Time remaining in the concrete agent population, without a normalization assumption. -/
def agentsTimePotential (agents : List (Id × NetSym)) : Nat :=
  (agents.map fun agent => timePotential agent.2).sum

/-- The number of agents allocated by an executable rule RHS. -/
def rhsSpacePotential (rhs : XRhs NetSym) : Nat := rhs.slots.length

/-- Every selected arithmetic, duplication, and erasure rule spends at most its local time
potential. This is quantified over every 64-bit payload and every operation, not sampled inputs. -/
theorem xRule_time_obligation {left right : NetSym} {rhs : XRhs NetSym}
    (selected : xRule left right = some rhs) :
    timePotential left + timePotential right ≥ 1 + rhsTimePotential rhs := by
  cases left <;> cases right <;> simp only [xRule] at selected
  all_goals cases selected
  all_goals
    simp [rhsTimePotential, timePotential, op2NumData, op1NumData,
      dupNumData, eraNumData, RuleData.toX, RuleData.ofWires, XRhs.swap,
      assoc] <;> omega

/-- Every selected rule's replacement has no more agent cells than its two-agent LHS. The DUP
case is tight (two agents become two); ERA removes both agents. -/
theorem xRule_space_obligation {left right : NetSym} {rhs : XRhs NetSym}
    (selected : xRule left right = some rhs) :
    spacePotential left + spacePotential right ≥ rhsSpacePotential rhs := by
  cases left <;> cases right <;> simp only [xRule] at selected
  all_goals cases selected
  all_goals
    simp [rhsSpacePotential, spacePotential, op2NumData, op1NumData,
      dupNumData, eraNumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc] <;> omega

/-- Arithmetic rules have two stages, and both stages consume one interaction. -/
theorem op2_stage_spends_one (op : Op) (value : BitVec 64) :
    timePotential (.op2 op) + timePotential (.num value) ≥
      1 + rhsTimePotential ((op2NumData op value).toX) := by
  cases op <;> simp [rhsTimePotential, timePotential, op2NumData, RuleData.toX,
    RuleData.ofWires, assoc]

theorem op1_stage_spends_one (op : Op) (first second : BitVec 64) :
    timePotential (.op1 op first) + timePotential (.num second) ≥
      1 + rhsTimePotential ((op1NumData op first second).toX) := by
  cases op <;> simp [rhsTimePotential, timePotential, op1NumData, RuleData.toX,
    RuleData.ofWires, assoc]

/-- The DUP obligation charges its firing even though it preserves the agent count. -/
theorem dup_spends_one (value : BitVec 64) :
    timePotential .dup + timePotential (.num value) ≥
      1 + rhsTimePotential ((dupNumData value).toX) := by
  simp [rhsTimePotential, timePotential, dupNumData, RuleData.toX, RuleData.ofWires, assoc]

/-- The ERA obligation charges its firing even though its RHS is empty. -/
theorem era_spends_one (value : BitVec 64) :
    timePotential .era + timePotential (.num value) ≥
      1 + rhsTimePotential eraNumData.toX := by
  simp [rhsTimePotential, timePotential, eraNumData, RuleData.toX, RuleData.ofWires, assoc]

private theorem filter_time_le (agents : List (Id × NetSym))
    (keep : Id × NetSym → Bool) :
    agentsTimePotential (agents.filter keep) ≤ agentsTimePotential agents := by
  induction agents with
  | nil => simp [agentsTimePotential]
  | cons agent rest ih =>
    cases hkeep : keep agent <;>
      simp [agentsTimePotential, hkeep] at * <;> omega

private theorem filter_time_drop (agents : List (Id × NetSym)) (id : Id) (symbol : NetSym)
    (member : (id, symbol) ∈ agents)
    (unique : ∀ agent ∈ agents, agent.1 = id → agent.2 = symbol) :
    timePotential symbol +
      agentsTimePotential (agents.filter fun agent => decide (agent.1 ≠ id)) ≤
        agentsTimePotential agents := by
  induction agents with
  | nil => cases member
  | cons agent rest ih =>
    by_cases head : agent.1 = id
    · have symbolEq := unique agent (by simp) head
      have keep : decide (agent.1 ≠ id) = false := by simp [head]
      simp only [List.filter_cons]
      simp only [keep, ↓reduceIte]
      have tailBound := filter_time_le rest (fun e => decide (e.1 ≠ id))
      simp only [agentsTimePotential, List.map_cons, List.sum_cons] at *
      simpa [symbolEq] using Nat.add_le_add_left tailBound (timePotential symbol)
    · have tailMember : (id, symbol) ∈ rest := by
        rcases List.mem_cons.mp member with equal | tail
        · exact False.elim (head (congrArg Prod.fst equal).symm)
        · exact tail
      have tailUnique : ∀ e ∈ rest, e.1 = id → e.2 = symbol :=
        fun e he => unique e (List.mem_cons_of_mem _ he)
      have tailBound := ih tailMember tailUnique
      have keep : decide (agent.1 ≠ id) = true := by simp [head]
      simp only [List.filter_cons]
      simp only [keep, ↓reduceIte]
      simp only [agentsTimePotential, List.map_cons, List.sum_cons] at *
      omega

private theorem rhs_time_agents (slots : List (Option Bool × Nat))
    (lab : Option Bool → Nat → Option NetSym) (a b : Id) :
    agentsTimePotential
        (slots.filterMap fun s => (lab s.1 s.2).map fun γ => (child a b s.1 s.2, γ)) =
      ((slots.filterMap fun s => lab s.1 s.2).map timePotential).sum := by
  induction slots with
  | nil => rfl
  | cons s rest ih =>
    cases h : lab s.1 s.2 with
    | none => simpa [agentsTimePotential, h] using ih
    | some γ => simpa [agentsTimePotential, h, ih]

private theorem selected_xRule {left right : NetSym} {R : Rhs NetSym}
    (selected : rule left right = some R) :
    ∃ rhs, xRule left right = some rhs ∧ rhs.toRhs = R := by
  unfold rule at selected
  cases h : xRule left right with
  | none => simp [h] at selected
  | some rhs =>
    simp [h] at selected
    exact ⟨rhs, rfl, selected⟩

/-- One selected rule spends at least one unit of agent time potential. The two live
redex agents are charged separately even if the list also contains other agents. -/
theorem XNet.fire_time_spends_one (X : XNet NetSym) (hX : X.Ok arity)
    {a b : Id} {left right : NetSym} {rhs : XRhs NetSym}
    (redex : Redex system X.toNet a b left right rhs.toRhs)
    (selected : xRule left right = some rhs) :
    1 + agentsTimePotential (X.fire arity a b left right rhs).agents ≤
      agentsTimePotential X.agents := by
  have uniqueLeft : ∀ e ∈ X.agents, e.1 = a → e.2 = left := by
    intro e he hid
    have recorded : X.toNet.agent a e.2 := by simpa [XNet.toNet, ← hid] using he
    exact hX.1.agent_fun recorded redex.ha
  have uniqueRight : ∀ e ∈ X.agents, e.1 = b → e.2 = right := by
    intro e he hid
    have recorded : X.toNet.agent b e.2 := by simpa [XNet.toNet, ← hid] using he
    exact hX.1.agent_fun recorded redex.hb
  have leftMember : (a, left) ∈ X.agents := redex.ha
  have rightMember : (b, right) ∈ X.agents := redex.hb
  have first := filter_time_drop X.agents a left leftMember uniqueLeft
  have rightKept : (b, right) ∈
      X.agents.filter (fun e => decide (e.1 ≠ a)) :=
    List.mem_filter.mpr ⟨rightMember, by simp [redex.ne.symm]⟩
  have second := filter_time_drop
    (X.agents.filter fun e => decide (e.1 ≠ a)) b right rightKept
    (fun e he hid => uniqueRight e (List.mem_filter.mp he).1 hid)
  have filtered :
      (X.agents.filter fun e => decide (e.1 ≠ a)).filter
          (fun e => decide (e.1 ≠ b)) =
        X.agents.filter (fun e => decide (e.1 ≠ a ∧ e.1 ≠ b)) := by
    simp only [List.filter_filter]
    congr 1
    funext e
    simp [Bool.and_comm]
  rw [filtered] at second
  have localCost := xRule_time_obligation selected
  have replacement :
      agentsTimePotential
        (rhs.slots.filterMap fun s =>
          (rhs.lab s.1 s.2).map fun γ => (child a b s.1 s.2, γ)) =
        rhsTimePotential rhs := rhs_time_agents rhs.slots rhs.lab a b
  simp only [XNet.fire, agentsTimePotential, List.map_append, List.sum_append] at ⊢
  change 1 + (agentsTimePotential
      (X.agents.filter fun e => decide (e.1 ≠ a ∧ e.1 ≠ b)) +
      agentsTimePotential
        (rhs.slots.filterMap fun s =>
          (rhs.lab s.1 s.2).map fun γ => (child a b s.1 s.2, γ))) ≤
      agentsTimePotential X.agents
  rw [replacement]
  omega

/-- Every finite legal prefix spends one unit per interaction; both its time
potential and live-agent count are bounded without a terminating run. -/
theorem XNet.steps_resource_le {X : XNet NetSym} (hX : X.Ok arity)
    {steps : Nat} {source target : Net NetSym} (hsource : X.toNet = source)
    (reduction : Steps (CStep system) steps source target) :
    ∃ reached : XNet NetSym, reached.toNet = target ∧ reached.Ok arity ∧
      steps + agentsTimePotential reached.agents ≤ agentsTimePotential X.agents ∧
      reached.agents.length ≤ X.agents.length := by
  induction reduction generalizing X with
  | refl _ => exact ⟨X, hsource, hX, by simp, Nat.le_refl _⟩
  | @head steps source middle target hstep rest ih =>
    have current : CStep system X.toNet middle := by simpa [hsource] using hstep
    rcases current with ⟨fired, ⟨a, b, left, right, R, redex, rfl⟩, compact⟩
    obtain ⟨rhs, selected, rhsEq⟩ := selected_xRule redex.rule
    subst R
    let next := X.fire arity a b left right rhs
    have timeStep : 1 + agentsTimePotential next.agents ≤
        agentsTimePotential X.agents :=
      XNet.fire_time_spends_one X hX redex selected
    have fireSound : next.toNet =
        Net.compact arity (X.toNet.fire a b rhs.toRhs) := by
      simpa [next, xsys, system] using
        (XNet.fire_toNet (xs := xsys) hX redex selected)
    have representation : next.toNet = middle := by
      rw [fireSound]
      simpa [system] using compact.symm
    have nextOk : next.Ok arity := by
      refine ⟨?_, ?_⟩
      · rw [fireSound]
        simpa [system] using
          (Step.wf hX.1
            (show Step system X.toNet (X.toNet.fire a b rhs.toRhs) from
              ⟨a, b, left, right, rhs.toRhs, redex, rfl⟩)).compact
      · rw [fireSound]
        exact Net.compact_idem _ _
    have slots : rhs.slots.length ≤ 2 := by
      have obligation := xRule_space_obligation selected
      simpa [spacePotential, rhsSpacePotential] using obligation
    have spaceStep : next.agents.length ≤ X.agents.length :=
      XNet.fire_agents_le X a b left right rhs redex.ne
        ⟨(a, left), redex.ha, rfl⟩ ⟨(b, right), redex.hb, rfl⟩
        slots
    obtain ⟨reached, final, finalOk, timeRest, spaceRest⟩ := ih nextOk representation
    exact ⟨reached, final, finalOk, by omega, Nat.le_trans spaceRest spaceStep⟩

/-- Resource bytes decoded by the Rust checker. `semantic` is the checked Lean model associated
with the decoded net; the envelope contains the two raw-byte digests and resource claims. -/
structure ResourceCertificate where
  formatRevision : String
  checkerRevision : String
  proofRevision : String
  costRevision : String
  hashScheme : String
  netDigest : List Nat
  ruleDigest : List Nat
  costDigest : List Nat
  declaredInteractions : Nat
  declaredLiveAgents : Nat
  semantic : Certificate
  deriving DecidableEq, Repr

/-- A digest has the fixed 32-byte width of the Rust wire format. -/
def DigestBytes (digest : List Nat) : Bool :=
  (digest.length == 32) && digest.all (fun byte => decide (byte < 256))

/-- The caller supplies a bounded independent decoder and exact selected rule/cost bytes. Its
success is checked rather than inferred from a digest. The hash algorithm and decoder's
correspondence to the Rust implementation are explicit trust boundaries of this model. -/
def checkResourceCertificate (hash : List Nat → List Nat) (hashScheme : String)
    (decode : List Nat → Option (Program × List (BitVec 64)))
    (netBytes ruleBytes selectedRuleBytes selectedCostBytes : List Nat)
    (program : Program) (inputs : List (BitVec 64))
    (certificate : ResourceCertificate) : Bool :=
  decide (certificate.formatRevision = resourceCertificateRevision) &&
    decide (certificate.checkerRevision = resourceCheckerRevision) &&
    decide (certificate.proofRevision = resourceProofRevision) &&
    decide (certificate.costRevision = resourceCostRevision) &&
    decide (certificate.hashScheme = hashScheme) &&
    DigestBytes certificate.netDigest &&
    DigestBytes certificate.ruleDigest &&
    DigestBytes certificate.costDigest &&
    decide (certificate.netDigest = hash netBytes) &&
    decide (certificate.ruleDigest = hash ruleBytes) &&
    decide (certificate.costDigest = hash selectedCostBytes) &&
    decide (ruleBytes = selectedRuleBytes) &&
    decide (decode netBytes = some (program, inputs)) &&
    decide (agentsTimePotential (program.translate inputs).agents ≤
      certificate.declaredInteractions) &&
    decide (certificate.declaredInteractions = program.interactionCount) &&
    decide (certificate.declaredLiveAgents = (program.translate inputs).agents.length) &&
    decide (certificate.semantic.graph = program) &&
    decide (certificate.semantic.inputs = inputs) &&
    decide (certificate.semantic.declaredInteractions = certificate.declaredInteractions) &&
    decide (certificate.semantic.declaredAgentBound = certificate.declaredLiveAgents) &&
    NobleFoundation.NetI64.checkCertificate program inputs certificate.semantic

/-- Acceptance exposes the exact envelope-field checks, including both supplied-byte digest
equalities, and the existing kernel-checked semantic certificate. -/
theorem checkResourceCertificate_iff (hash : List Nat → List Nat) (hashScheme : String)
    (decode : List Nat → Option (Program × List (BitVec 64)))
    (netBytes ruleBytes selectedRuleBytes selectedCostBytes : List Nat)
    (program : Program) (inputs : List (BitVec 64))
    (certificate : ResourceCertificate) :
    checkResourceCertificate hash hashScheme decode netBytes ruleBytes selectedRuleBytes
      selectedCostBytes program inputs certificate = true ↔
      certificate.formatRevision = resourceCertificateRevision ∧
      certificate.checkerRevision = resourceCheckerRevision ∧
      certificate.proofRevision = resourceProofRevision ∧
      certificate.costRevision = resourceCostRevision ∧
      certificate.hashScheme = hashScheme ∧
      DigestBytes certificate.netDigest = true ∧ DigestBytes certificate.ruleDigest = true ∧
      DigestBytes certificate.costDigest = true ∧
      certificate.netDigest = hash netBytes ∧ certificate.ruleDigest = hash ruleBytes ∧
      certificate.costDigest = hash selectedCostBytes ∧ ruleBytes = selectedRuleBytes ∧
      decode netBytes = some (program, inputs) ∧
      agentsTimePotential (program.translate inputs).agents ≤
        certificate.declaredInteractions ∧
      certificate.declaredInteractions = program.interactionCount ∧
      certificate.declaredLiveAgents = (program.translate inputs).agents.length ∧
      certificate.semantic.graph = program ∧ certificate.semantic.inputs = inputs ∧
      certificate.semantic.declaredInteractions = certificate.declaredInteractions ∧
      certificate.semantic.declaredAgentBound = certificate.declaredLiveAgents ∧
      NobleFoundation.NetI64.checkCertificate program inputs certificate.semantic = true := by
  simp [checkResourceCertificate, Bool.and_eq_true, decide_eq_true_eq, and_assoc]

/-- Any legal finite schedule prefix fits the budget checked against its exact admitted source.
No normalization, terminal normal form, or successful semantic certificate is required. -/
theorem accepted_prefix_bounds_every_schedule {program : Program} {inputs : List (BitVec 64)}
    {certificate : Certificate}
    {source : XNet NetSym} (translated : program.translateChecked inputs = some source)
    (sourceTime : agentsTimePotential source.agents ≤ certificate.declaredInteractions)
    (sourceLive : source.agents.length ≤ certificate.declaredAgentBound)
    {steps : Nat} {target : Net NetSym}
    (reduction : Steps (CStep system) steps source.toNet target) :
    steps ≤ certificate.declaredInteractions ∧
      ∃ reached : XNet NetSym, reached.toNet = target ∧
        reached.agents.length ≤ certificate.declaredAgentBound := by
  obtain ⟨reached, htarget, _, timeBound, liveBound⟩ :=
    XNet.steps_resource_le (Program.translateChecked_ok translated) rfl reduction
  exact ⟨by omega, reached, htarget, Nat.le_trans liveBound sourceLive⟩

/-- Exact interaction count is a distinct, conditional consequence of a successful
semantic normalization and a terminal normal form, not of the prefix potential. -/
theorem accepted_exact_count_every_normal_schedule
    {program : Program} {inputs : List (BitVec 64)}
    {certificate : Certificate} (accepted : checkCertificate program inputs certificate = true)
    {source : XNet NetSym} (translated : program.translateChecked inputs = some source)
    {steps : Nat} {target : Net NetSym}
    (reduction : Steps (CStep system) steps source.toNet target)
    (normal : Normal (CStep system) target) :
    steps = certificate.declaredInteractions := by
  have fields :=
    (NobleFoundation.NetI64.checkCertificate_iff program inputs certificate).1 accepted
  rcases fields with ⟨_, _, _, _, _, declared, _, _, _, _⟩
  exact (Certificate.accepted_exact_count accepted translated reduction normal).trans
    declared.symm

/-- Checked decoded bytes bind the initial potential and live population independently of a
normalization proof. Exact terminal count below remains conditional on the semantic checker,
whose acceptance requires a successful normalizer run; digests assert equality, not collision
resistance. -/
theorem accepted_bytes_bound_every_schedule (hash : List Nat → List Nat) (hashScheme : String)
    (decode : List Nat → Option (Program × List (BitVec 64)))
    (netBytes ruleBytes selectedRuleBytes selectedCostBytes : List Nat)
    {program : Program} {inputs : List (BitVec 64)}
    {certificate : ResourceCertificate}
    (accepted : checkResourceCertificate hash hashScheme decode netBytes ruleBytes
      selectedRuleBytes selectedCostBytes program inputs certificate = true)
    {source : XNet NetSym} (translated : program.translateChecked inputs = some source)
    {steps : Nat} {target : Net NetSym} (reduction : Steps (CStep system) steps source.toNet target) :
    decode netBytes = some (program, inputs) ∧ ruleBytes = selectedRuleBytes ∧
      certificate.netDigest = hash netBytes ∧ certificate.ruleDigest = hash ruleBytes ∧
      steps ≤ certificate.declaredInteractions ∧
      (Normal (CStep system) target → steps = certificate.declaredInteractions) ∧
      ∃ reached : XNet NetSym, reached.toNet = target ∧
        reached.agents.length ≤ certificate.declaredLiveAgents := by
  have fields := (checkResourceCertificate_iff hash hashScheme decode netBytes ruleBytes
    selectedRuleBytes selectedCostBytes program inputs certificate).1 accepted
  rcases fields with ⟨_, _, _, _, _, _, _, _, netDigest, ruleDigest, _, selectedRules,
    decoded, sourceTime, _, sourceLive, _, _, semanticCount, liveBound, semanticAccepted⟩
  have sourceEq : program.translate inputs = source := by
    by_cases admitted : program.acceptanceCheck inputs = true
    · by_cases checked : (program.translate inputs).check arity = true
      · simpa [Program.translateChecked, admitted, checked] using translated
      · simp [Program.translateChecked, admitted, checked] at translated
    · simp [Program.translateChecked, admitted] at translated
  have sourceTime' : agentsTimePotential source.agents ≤
      certificate.semantic.declaredInteractions := by
    calc
      agentsTimePotential source.agents =
          agentsTimePotential (program.translate inputs).agents := by rw [sourceEq]
      _ ≤ certificate.declaredInteractions := sourceTime
      _ = certificate.semantic.declaredInteractions := semanticCount.symm
  have sourceLive' : source.agents.length ≤ certificate.semantic.declaredAgentBound := by
    calc
      source.agents.length = (program.translate inputs).agents.length := by rw [sourceEq]
      _ = certificate.declaredLiveAgents := sourceLive.symm
      _ = certificate.semantic.declaredAgentBound := liveBound.symm
  have ⟨prefixBound, reached, htarget, hlive⟩ :=
    accepted_prefix_bounds_every_schedule translated sourceTime' sourceLive' reduction
  exact ⟨decoded, selectedRules, netDigest, ruleDigest,
    Nat.le_trans prefixBound (Nat.le_of_eq semanticCount),
    (fun normalTarget =>
      (accepted_exact_count_every_normal_schedule semanticAccepted translated reduction
        normalTarget).trans semanticCount), reached, htarget,
    Nat.le_trans hlive (Nat.le_of_eq liveBound)⟩

/-- A growing chain has linear interaction and committed-agent bounds. The formula is valid for
all structural sizes; the arbitrary input values are bound by each certificate's `inputs`. -/
theorem chain_family_resources (operations : Nat) :
    (chainProgram operations).interactionCount = 3 * operations ∧
      (chainProgram operations).agentBound = 2 * operations + 1 :=
  ⟨chainProgram_interactionCount operations, chainProgram_agentBound operations⟩

/-- A perfect balanced tree has exponentially growing leaves and linear-in-nodes resource bounds. -/
theorem balanced_family_resources (depth : Nat) :
    (balancedProgram depth).interactionCount = 2 * (2 ^ depth - 1) ∧
      (balancedProgram depth).agentBound = 2 ^ (depth + 1) - 1 :=
  ⟨balancedProgram_interactionCount depth, balancedProgram_agentBound depth⟩

/-- Fanout and erase costs are part of the growing profile formulas, not free administrative work. -/
theorem fanout_family_resources (outputs : Nat) :
    (fanoutProgram outputs).interactionCount =
        (if outputs = 0 then 1 else outputs - 1) ∧
      (fanoutProgram outputs).agentBound =
        (if outputs = 0 then 2 else if outputs = 1 then 1 else outputs) :=
  ⟨fanoutProgram_interactionCount outputs, fanoutProgram_agentBound outputs⟩

end NobleFoundation.NetI64Resource
