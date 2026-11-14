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

/-- The sum of time potential on the actual agent slots of an executable rule RHS. -/
def rhsTimePotential (rhs : XRhs NetSym) : Nat :=
  rhs.slots.countP fun (owner, index) =>
    match rhs.lab owner index with
    | some (.op1 _ _) => true
    | _ => false

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
      certificate.declaredInteractions = program.interactionCount ∧
      certificate.declaredLiveAgents = (program.translate inputs).agents.length ∧
      certificate.semantic.graph = program ∧ certificate.semantic.inputs = inputs ∧
      certificate.semantic.declaredInteractions = certificate.declaredInteractions ∧
      certificate.semantic.declaredAgentBound = certificate.declaredLiveAgents ∧
      NobleFoundation.NetI64.checkCertificate program inputs certificate.semantic = true := by
  simp [checkResourceCertificate, Bool.and_eq_true, decide_eq_true_eq, and_assoc]

/-- Any legal finite reduction prefix of an accepted certificate fits its interaction potential.
Every prefix is covered; a terminal normal form has the exact count by inets' unique-normal-form
theorem. The live-agent result uses the independent `XNet` preservation proof for each schedule. -/
theorem accepted_prefix_bounds_every_schedule {program : Program} {inputs : List (BitVec 64)}
    {certificate : Certificate} (accepted : checkCertificate program inputs certificate = true)
    {source : XNet NetSym} (translated : program.translateChecked inputs = some source)
    {steps : Nat} {target : Net NetSym}
    (reduction : Steps (CStep system) steps source.toNet target) :
    steps ≤ certificate.declaredInteractions ∧
      (Normal (CStep system) target → steps = certificate.declaredInteractions) ∧
      ∃ reached : XNet NetSym, reached.toNet = target ∧
        reached.agents.length ≤ certificate.declaredAgentBound := by
  have accepts := (NobleFoundation.NetI64.checkCertificate_iff program inputs certificate).1 accepted
  rcases accepts with ⟨_, _, _, _, _, _, _, _, _, normalized⟩
  cases hnormalize : program.normalize inputs with
  | none => simp [hnormalize] at normalized
  | some result =>
    cases result with
    | mk normal exactSteps =>
      simp [hnormalize] at normalized
      obtain ⟨normalSource, sourceChecked, _, _, _, everyPrefix⟩ :=
        Program.normalize_spec hnormalize
      have sourceEq : normalSource = source :=
        Option.some.inj (sourceChecked.symm.trans translated)
      subst normalSource
      have prefixResult := everyPrefix reduction
      have countEq : exactSteps = certificate.declaredInteractions := normalized.1
      have live :=
        NobleFoundation.NetI64.Certificate.accepted_reduction_live_agents
          accepted translated reduction
      refine ⟨?_, ?_, live⟩
      · exact Nat.le_trans prefixResult.1 (Nat.le_of_eq countEq)
      · intro normalTarget
        exact (prefixResult.2 normalTarget).2.trans countEq

/-- Decoded-byte certificate acceptance carries the same all-schedule bounds as its associated
kernel certificate. The digest equalities remain explicit conclusions rather than a collision
resistance theorem. -/
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
    decoded, _, _, _, _, semanticCount, liveBound, semanticAccepted⟩
  have bounds := accepted_prefix_bounds_every_schedule semanticAccepted translated reduction
  rcases bounds with ⟨prefixBound, normalCount, ⟨reached, htarget, hlive⟩⟩
  exact ⟨decoded, selectedRules, netDigest, ruleDigest,
    Nat.le_trans prefixBound (Nat.le_of_eq semanticCount),
    (fun normalTarget => (normalCount normalTarget).trans semanticCount), reached, htarget,
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
