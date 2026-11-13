import InteractionNets
import NobleFoundation.PackedSchedule

/-!
# Revision-3 exact I64 interaction system (issues #37 and #109)

This module is the kernel model of the selected `ngraph-net-i64/3` contract.
Revision-2's stack reducer in `proofs/m3/NobleM2/InteractionNet{V2,Reduce}.lean`
is a recorded comparison and is not used here.
-/

namespace NobleFoundation.NetI64

open INet

/-- The four exact operations supported by the NetI64 profile. -/
inductive Op where
  | add
  | sub
  | mul
  | eq
  deriving DecidableEq, Repr

/-- The result bits of an operation; equality is represented as I64 zero or one. -/
def Op.apply (op : Op) (a b : BitVec 64) : BitVec 64 :=
  match op with
  | .add => a + b
  | .sub => a - b
  | .mul => a * b
  | .eq => if a == b then 1 else 0

/-- NetI64's symbols. The port conventions are those of inets: port zero is principal. -/
inductive NetSym where
  | num (value : BitVec 64)
  | op2 (op : Op)
  | op1 (op : Op) (first : BitVec 64)
  | dup
  | era
  deriving DecidableEq, Repr

/-- Auxiliary-port count for NetI64 agents. -/
def arity : NetSym → Nat
  | .num _ => 0
  | .op2 _ => 2
  | .op1 _ _ => 1
  | .dup => 2
  | .era => 0

/-- The `op2 ⋈ num` right-hand side: retain operand one in an `op1`, routing its
principal to `op2`'s operand-two port and its auxiliary to the result wire. -/
def op2NumData (op : Op) (first : BitVec 64) : RuleData NetSym :=
  RuleData.ofWires [((some false, 0), .op1 op first)]
    [(.ifc false 0, .new (some false) 0 0),
     (.ifc false 1, .new (some false) 0 1)]

/-- The `op1 ⋈ num` right-hand side. -/
def op1NumData (op : Op) (first second : BitVec 64) : RuleData NetSym :=
  RuleData.ofWires [((some false, 0), .num (op.apply first second))]
    [(.ifc false 0, .new (some false) 0 0)]

/-- The `dup ⋈ num` right-hand side; auxiliary-port order is preserved. -/
def dupNumData (value : BitVec 64) : RuleData NetSym :=
  RuleData.ofWires [((some false, 0), .num value), ((some false, 1), .num value)]
    [(.ifc false 0, .new (some false) 0 0),
     (.ifc false 1, .new (some false) 1 0)]

/-- The `era ⋈ num` right-hand side. -/
def eraNumData : RuleData NetSym := RuleData.ofWires [] []

/-- Executable rule table. Reverse orientations are the inets mirror. -/
def xRule : NetSym → NetSym → Option (XRhs NetSym)
  | .op2 op, .num first => some (op2NumData op first).toX
  | .num first, .op2 op => some (op2NumData op first).toX.swap
  | .op1 op first, .num second => some (op1NumData op first second).toX
  | .num second, .op1 op first => some (op1NumData op first second).toX.swap
  | .dup, .num value => some (dupNumData value).toX
  | .num value, .dup => some (dupNumData value).toX.swap
  | .era, .num _ => some eraNumData.toX
  | .num _, .era => some eraNumData.toX.swap
  | _, _ => none

/-- The relational rule table corresponding to `xRule`. -/
def rule (left right : NetSym) : Option (Rhs NetSym) :=
  (xRule left right).map XRhs.toRhs

private theorem op2NumData_checked (op : Op) (first : BitVec 64) :
    (op2NumData op first).checkWF arity (.op2 op) (.num first) = true := by
  cases op <;>
    dsimp [op2NumData, RuleData.checkWF, RuleData.ofWires, RuleData.ports,
      ifcPorts, arity, List.range, List.range.loop, List.all, assoc] <;>
    simp [List.map, List.all, List.range.loop, assoc]

private theorem op1NumData_checked (op : Op) (first second : BitVec 64) :
    (op1NumData op first second).checkWF arity (.op1 op first) (.num second) = true := by
  cases op <;>
    dsimp [op1NumData, RuleData.checkWF, RuleData.ofWires, RuleData.ports,
      ifcPorts, arity, List.range, List.range.loop, List.all, assoc] <;>
    simp [List.map, List.all, List.range.loop, assoc]

private theorem dupNumData_checked (value : BitVec 64) :
    (dupNumData value).checkWF arity .dup (.num value) = true := by
  dsimp [dupNumData, RuleData.checkWF, RuleData.ofWires, RuleData.ports,
    ifcPorts, arity, List.range, List.range.loop, List.all, assoc]
  simp [List.map, List.all, List.range.loop, assoc]

private theorem eraNumData_checked (value : BitVec 64) :
    eraNumData.checkWF arity .era (.num value) = true := by
  rfl

private theorem op2Num_wf (op : Op) (first : BitVec 64) :
    (op2NumData op first).toX.toRhs.WF arity (.op2 op) (.num first) :=
  (RuleData.checkWF_sound (op2NumData_checked op first)).2

private theorem op1Num_wf (op : Op) (first second : BitVec 64) :
    (op1NumData op first second).toX.toRhs.WF arity (.op1 op first) (.num second) :=
  (RuleData.checkWF_sound (op1NumData_checked op first second)).2

private theorem dupNum_wf (value : BitVec 64) :
    (dupNumData value).toX.toRhs.WF arity .dup (.num value) :=
  (RuleData.checkWF_sound (dupNumData_checked value)).2

private theorem eraNum_wf (value : BitVec 64) :
    eraNumData.toX.toRhs.WF arity .era (.num value) :=
  (RuleData.checkWF_sound (eraNumData_checked value)).2

private theorem rhs_swap_mirror (R : XRhs NetSym) :
    R.toRhs.Mirror R.swap.toRhs := by
  have h := Rhs.mirror_swap R.toRhs
  rw [← XRhs.toRhs_swap R] at h
  exact h

private theorem rhs_mirror_swap (R : XRhs NetSym) :
    R.swap.toRhs.Mirror R.toRhs := by
  have h := Rhs.swap_mirror R.toRhs
  rw [← XRhs.toRhs_swap R] at h
  exact h

private theorem rhs_swap_wf {left right : NetSym} {R : XRhs NetSym}
    (h : R.toRhs.WF arity left right) :
    R.swap.toRhs.WF arity right left := by
  have hs := h.swap
  rw [← XRhs.toRhs_swap R] at hs
  exact hs

private theorem xRule_wf {left right : NetSym} {R : XRhs NetSym}
    (h : xRule left right = some R) : R.toRhs.WF arity left right := by
  cases left with
  | num value =>
    cases right with
    | num other => simp [xRule] at h
    | op2 op =>
      change some (op2NumData op value).toX.swap = some R at h
      cases Option.some.inj h
      exact rhs_swap_wf (op2Num_wf op value)
    | op1 op first =>
      change some (op1NumData op first value).toX.swap = some R at h
      cases Option.some.inj h
      exact rhs_swap_wf (op1Num_wf op first value)
    | dup =>
      change some (dupNumData value).toX.swap = some R at h
      cases Option.some.inj h
      exact rhs_swap_wf (dupNum_wf value)
    | era =>
      change some eraNumData.toX.swap = some R at h
      cases Option.some.inj h
      exact rhs_swap_wf (eraNum_wf value)
  | op2 op =>
    cases right with
    | num value =>
      change some (op2NumData op value).toX = some R at h
      cases Option.some.inj h
      exact op2Num_wf op value
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | op1 op first =>
    cases right with
    | num second =>
      change some (op1NumData op first second).toX = some R at h
      cases Option.some.inj h
      exact op1Num_wf op first second
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | dup =>
    cases right with
    | num value =>
      change some (dupNumData value).toX = some R at h
      cases Option.some.inj h
      exact dupNum_wf value
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | era =>
    cases right with
    | num value =>
      change some eraNumData.toX = some R at h
      cases Option.some.inj h
      exact eraNum_wf value
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h

private theorem xRule_symm {left right : NetSym} {R : XRhs NetSym}
    (h : xRule left right = some R) :
    ∃ R', xRule right left = some R' ∧ R.toRhs.Mirror R'.toRhs := by
  cases left with
  | num value =>
    cases right with
    | num other => simp [xRule] at h
    | op2 op =>
      change some (op2NumData op value).toX.swap = some R at h
      cases Option.some.inj h
      exact ⟨(op2NumData op value).toX, rfl, rhs_mirror_swap _⟩
    | op1 op first =>
      change some (op1NumData op first value).toX.swap = some R at h
      cases Option.some.inj h
      exact ⟨(op1NumData op first value).toX, rfl, rhs_mirror_swap _⟩
    | dup =>
      change some (dupNumData value).toX.swap = some R at h
      cases Option.some.inj h
      exact ⟨(dupNumData value).toX, rfl, rhs_mirror_swap _⟩
    | era =>
      change some eraNumData.toX.swap = some R at h
      cases Option.some.inj h
      exact ⟨eraNumData.toX, rfl, rhs_mirror_swap _⟩
  | op2 op =>
    cases right with
    | num value =>
      change some (op2NumData op value).toX = some R at h
      cases Option.some.inj h
      exact ⟨(op2NumData op value).toX.swap, rfl, rhs_swap_mirror _⟩
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | op1 op first =>
    cases right with
    | num second =>
      change some (op1NumData op first second).toX = some R at h
      cases Option.some.inj h
      exact ⟨(op1NumData op first second).toX.swap, rfl, rhs_swap_mirror _⟩
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | dup =>
    cases right with
    | num value =>
      change some (dupNumData value).toX = some R at h
      cases Option.some.inj h
      exact ⟨(dupNumData value).toX.swap, rfl, rhs_swap_mirror _⟩
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | era =>
    cases right with
    | num value =>
      change some eraNumData.toX = some R at h
      cases Option.some.inj h
      exact ⟨eraNumData.toX.swap, rfl, rhs_swap_mirror _⟩
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h

private theorem ruleData_slots (D : RuleData NetSym) {owner : Option Bool}
    {index : Nat} {symbol : NetSym} (h : D.toX.lab owner index = some symbol) :
    (owner, index) ∈ D.toX.slots :=
  List.mem_map.2 ⟨((owner, index), symbol), assoc_mem h, rfl⟩

private theorem xRule_slots {left right : NetSym} {R : XRhs NetSym}
    (h : xRule left right = some R) {owner : Option Bool} {index : Nat} {symbol : NetSym}
    (hl : R.lab owner index = some symbol) : (owner, index) ∈ R.slots := by
  cases left with
  | num value =>
    cases right with
    | num other => simp [xRule] at h
    | op2 op =>
      change some (op2NumData op value).toX.swap = some R at h
      cases Option.some.inj h
      exact XRhs.swap_slots (ruleData_slots (op2NumData op value)) hl
    | op1 op first =>
      change some (op1NumData op first value).toX.swap = some R at h
      cases Option.some.inj h
      exact XRhs.swap_slots (ruleData_slots (op1NumData op first value)) hl
    | dup =>
      change some (dupNumData value).toX.swap = some R at h
      cases Option.some.inj h
      exact XRhs.swap_slots (ruleData_slots (dupNumData value)) hl
    | era =>
      change some eraNumData.toX.swap = some R at h
      cases Option.some.inj h
      simp_all [eraNumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]
  | op2 op =>
    cases right with
    | num value =>
      change some (op2NumData op value).toX = some R at h
      cases Option.some.inj h
      exact ruleData_slots (op2NumData op value) hl
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | op1 op first =>
    cases right with
    | num second =>
      change some (op1NumData op first second).toX = some R at h
      cases Option.some.inj h
      exact ruleData_slots (op1NumData op first second) hl
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | dup =>
    cases right with
    | num value =>
      change some (dupNumData value).toX = some R at h
      cases Option.some.inj h
      exact ruleData_slots (dupNumData value) hl
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h
  | era =>
    cases right with
    | num value =>
      change some eraNumData.toX = some R at h
      cases Option.some.inj h
      simp_all [eraNumData, RuleData.toX, RuleData.ofWires, assoc]
    | op2 _ => simp [xRule] at h
    | op1 _ _ => simp [xRule] at h
    | dup => simp [xRule] at h
    | era => simp [xRule] at h

/-- The revision-3 NetI64 interaction system. -/
def system : System NetSym where
  ar := arity
  rule := rule
  rule_wf := by
    intro left right R h
    cases hr : xRule left right with
    | none => simp [rule, hr] at h
    | some X =>
      have heq : X.toRhs = R := by simpa [rule, hr] using h
      subst R
      exact xRule_wf hr
  rule_symm := by
    intro left right R h
    cases hr : xRule left right with
    | none => simp [rule, hr] at h
    | some X =>
      have heq : X.toRhs = R := by simpa [rule, hr] using h
      subst R
      obtain ⟨Y, hy, hm⟩ := xRule_symm hr
      exact ⟨Y.toRhs, by simp [rule, hy], hm⟩

/-- The selected revision-3 rule table has well-formed right-hand sides. -/
theorem system_rule_wf {left right : NetSym} {R : Rhs NetSym}
    (h : rule left right = some R) : R.WF arity left right :=
  system.rule_wf h

/-- The selected revision-3 rule table is symmetric by the inets mirror. -/
theorem system_rule_symm {left right : NetSym} {R : Rhs NetSym}
    (h : rule left right = some R) :
    ∃ R', rule right left = some R' ∧ R.Mirror R' :=
  system.rule_symm h

/-- Executable system for kernel-evaluated finite nets. -/
def xsys : XSys NetSym where
  sys := system
  rule := xRule
  rule_spec := by intro left right; rfl
  slots_spec := by
    intro left right R owner index symbol hr hl
    exact xRule_slots hr hl

private theorem xRule_slots_length_le {left right : NetSym} {R : XRhs NetSym}
    (h : xRule left right = some R) : R.slots.length ≤ 2 := by
  cases left <;> cases right <;>
    simp [xRule, op2NumData, op1NumData, dupNumData, eraNumData,
      RuleData.ofWires, RuleData.toX, XRhs.swap] at h
  all_goals subst R
  all_goals simp

private theorem filter_id_drop_one {agents : List (Id × NetSym)} {id : Id}
    (hasAgent : ∃ agent ∈ agents, agent.1 = id) :
    (agents.filter (fun agent => decide (agent.1 ≠ id))).length + 1 ≤ agents.length := by
  induction agents with
  | nil =>
      rcases hasAgent with ⟨agent, member, -⟩
      cases member
  | cons head tail ih =>
      by_cases hhead : head.1 = id
      · simp [hhead]
        exact List.length_filter_le _ _
      · have htail : ∃ agent ∈ tail, agent.1 = id := by
          rcases hasAgent with ⟨agent, member, hid⟩
          rcases List.mem_cons.mp member with heq | hmember
          · subst agent
            contradiction
          · exact ⟨agent, hmember, hid⟩
        have hrec := ih htail
        simp [hhead]
        omega

private theorem filter_two_ids_drop_two {agents : List (Id × NetSym)} {first second : Id}
    (different : first ≠ second)
    (firstAgent : ∃ agent ∈ agents, agent.1 = first)
    (secondAgent : ∃ agent ∈ agents, agent.1 = second) :
    (agents.filter (fun agent => decide (agent.1 ≠ first ∧ agent.1 ≠ second))).length + 2 ≤
      agents.length := by
  have firstDrop := filter_id_drop_one firstAgent
  have secondRemains :
      ∃ agent ∈ agents.filter (fun agent => decide (agent.1 ≠ first)), agent.1 = second := by
    rcases secondAgent with ⟨agent, member, hsecond⟩
    refine ⟨agent, List.mem_filter.2 ⟨member, ?_⟩, hsecond⟩
    simp [hsecond, different.symm]
  have secondDrop := filter_id_drop_one secondRemains
  have filterEq :
      agents.filter (fun agent => decide (agent.1 ≠ first ∧ agent.1 ≠ second)) =
        (agents.filter (fun agent => decide (agent.1 ≠ first))).filter
          (fun agent => decide (agent.1 ≠ second)) := by
    simp only [List.filter_filter]
    congr 1
    funext agent
    simp [Bool.and_comm]
  rw [filterEq]
  omega

/-- Firing any NetI64 rule removes two agents and creates at most two, so it
cannot increase the live-agent population. -/
theorem XNet.fire_agents_le (X : XNet NetSym) (a b : Id) (α β : NetSym)
    (R : XRhs NetSym) (different : a ≠ b)
    (aAgent : ∃ agent ∈ X.agents, agent.1 = a)
    (bAgent : ∃ agent ∈ X.agents, agent.1 = b)
    (slots : R.slots.length ≤ 2) :
    (X.fire arity a b α β R).agents.length ≤ X.agents.length := by
  unfold XNet.fire
  simp only [List.length_append]
  have hRemoved := filter_two_ids_drop_two different aAgent bAgent
  have hAdded : (R.slots.filterMap (fun s =>
      (R.lab s.1 s.2).map fun γ => (child a b s.1 s.2, γ))).length ≤ R.slots.length :=
    List.length_filterMap_le _ _
  omega

/-- One executable reducer step cannot increase the live-agent population. -/
theorem XNet.step_agents_le {X X' : XNet NetSym}
    (hstep : X.step xsys = some X') :
    X'.agents.length ≤ X.agents.length := by
  unfold XNet.step at hstep
  cases hredex : X.findRedex xsys with
  | none => simp [hredex] at hstep
  | some redex =>
      rw [hredex] at hstep
      obtain ⟨a, b, α, β, R⟩ := redex
      have hresult : X.fire arity a b α β R = X' := by
        apply Option.some.inj
        simpa [xsys, system] using hstep
      subst X'
      obtain ⟨⟨agent, α₀⟩, hmem, hredexAt⟩ := List.exists_of_findSome?_eq_some hredex
      obtain ⟨r, hRule⟩ := X.redexAt_sound hredexAt hmem
      have haMem : (a, α) ∈ X.agents := r.ha
      have hbMem : (b, β) ∈ X.agents := r.hb
      have ha : ∃ e ∈ X.agents, e.1 = a := ⟨(a, α), haMem, rfl⟩
      have hb : ∃ e ∈ X.agents, e.1 = b := ⟨(b, β), hbMem, rfl⟩
      exact XNet.fire_agents_le X a b α β R r.ne ha hb (xRule_slots_length_le hRule)

/-- Every prefix of the sequential reducer run stays within its initial
live-agent count. -/
theorem XNet.run_agents_le {X : XNet NetSym} (hX : X.Ok arity) (fuel : Nat) :
    (X.run xsys fuel).1.agents.length ≤ X.agents.length := by
  induction fuel generalizing X with
  | zero => simp [XNet.run]
  | succ fuel ih =>
      unfold XNet.run
      cases hstep : X.step xsys with
      | none => simp
      | some next =>
          simp
          have hnext : next.Ok arity := (XNet.step_sound hX hstep).2
          exact Nat.le_trans (ih hnext) (XNet.step_agents_le hstep)

private theorem rule_to_xRule {left right : NetSym} {R : Rhs NetSym}
    (h : rule left right = some R) :
    ∃ XR, xRule left right = some XR ∧ XR.toRhs = R := by
  unfold rule at h
  cases hx : xRule left right with
  | none => simp [hx] at h
  | some XR =>
      simp [hx] at h
      exact ⟨XR, rfl, h⟩

/-- Every abstract inets step from a finite checked net has a finite executable
representative, and that representative does not increase the agent list. -/
theorem XNet.cstep_agent_bound {X : XNet NetSym} (hX : X.Ok arity)
    {target : Net NetSym} (hstep : CStep system X.toNet target) :
    ∃ next : XNet NetSym, next.toNet = target ∧ next.Ok arity ∧
      next.agents.length ≤ X.agents.length := by
  rcases hstep with ⟨fired, ⟨a, b, α, β, R, redex, rfl⟩, hcompact⟩
  rcases rule_to_xRule redex.rule with ⟨XR, hXR, hR⟩
  subst R
  let next := X.fire arity a b α β XR
  have hfire := XNet.fire_toNet (xs := xsys) hX redex hXR
  have hfire' : next.toNet = Net.compact arity (X.toNet.fire a b XR.toRhs) := by
    simpa [next, xsys, system] using hfire
  have htarget : next.toNet = target := by
    rw [hfire']
    simpa [system] using hcompact.symm
  have hab : Step system X.toNet (X.toNet.fire a b XR.toRhs) :=
    ⟨a, b, α, β, XR.toRhs, redex, rfl⟩
  have hnextOk : next.Ok arity := by
    refine ⟨?_, ?_⟩
    · rw [hfire']
      simpa [system] using (Step.wf hX.1 hab).compact
    · rw [hfire']
      exact Net.compact_idem _ _
  have hcount := XNet.fire_agents_le X a b α β XR redex.ne
    ⟨(a, α), redex.ha, rfl⟩ ⟨(b, β), redex.hb, rfl⟩ (xRule_slots_length_le hXR)
  exact ⟨next, htarget, hnextOk, by simpa [next] using hcount⟩

/-- Every finite prefix of every inets reduction from a checked executable net
has a finite representative whose live-agent list is no larger than the
initial list. -/
theorem XNet.steps_agents_le {X : XNet NetSym} (hX : X.Ok arity)
    {steps : Nat} {source target : Net NetSym} (hsource : X.toNet = source)
    (hsteps : Steps (CStep system) steps source target) :
    ∃ reached : XNet NetSym, reached.toNet = target ∧ reached.Ok arity ∧
      reached.agents.length ≤ X.agents.length := by
  induction hsteps generalizing X with
  | refl _ => exact ⟨X, hsource, hX, Nat.le_refl _⟩
  | @head steps source middle target hstep htail ih =>
      have hstep' : CStep system X.toNet middle := by simpa [hsource] using hstep
      obtain ⟨next, hnext, hnextOk, hnextLen⟩ := XNet.cstep_agent_bound hX hstep'
      obtain ⟨reached, hfinal, hfinalOk, hfinalLen⟩ := ih hnextOk hnext
      exact ⟨reached, hfinal, hfinalOk, Nat.le_trans hfinalLen hnextLen⟩

/-- No interaction rule is invented for two numeric agents or two binary
operation agents. -/
theorem no_rule_num_num (left right : BitVec 64) :
    rule (.num left) (.num right) = none := rfl

theorem no_rule_op2_op2 (left right : Op) :
    rule (.op2 left) (.op2 right) = none := rfl

/-- The two-stage arithmetic rules are present for every selected operation. -/
theorem rule_op2_num (op : Op) (first : BitVec 64) :
    rule (.op2 op) (.num first) = some (op2NumData op first).toX.toRhs := rfl

theorem rule_op1_num (op : Op) (first second : BitVec 64) :
    rule (.op1 op first) (.num second) =
      some (op1NumData op first second).toX.toRhs := rfl

/-- Duplication and erasure consume a numeric agent with the contract's exact RHS. -/
theorem rule_dup_num (value : BitVec 64) :
    rule .dup (.num value) = some (dupNumData value).toX.toRhs := rfl

theorem rule_era_num (value : BitVec 64) :
    rule .era (.num value) = some eraNumData.toX.toRhs := rfl

/-- Deliberately ambiguous table: one ordered pair has distinct addition and
subtraction right-hand sides, each accompanied by a mirror entry. -/
def ambiguousRuleSysData : SysData NetSym where
  ar := arity
  rules := [
    ((.op2 .sub, .num 3), op2NumData .sub 3),
    ((.op2 .sub, .num 3), op2NumData .add 3),
    ((.num 3, .op2 .sub), (op2NumData .sub 3).swap),
    ((.num 3, .op2 .sub), (op2NumData .add 3).swap)]

/-- Inets' checked data format rejects competing right-hand sides for the same
ordered symbol pair. -/
theorem conflicting_rule_pair_rejected :
    ambiguousRuleSysData.check = false := by
  decide

/-- Subtraction preserves the contract's operand order. -/
theorem sub_operand_order (first second : BitVec 64) :
    Op.apply .sub first second = first - second := rfl

/-- Equality's numeric wire encoding is one for equal operands and zero otherwise. -/
theorem eq_one_of_equal (value : BitVec 64) : Op.apply .eq value value = 1 := by
  simp [Op.apply]

theorem eq_zero_of_ne {left right : BitVec 64} (different : left ≠ right) :
    Op.apply .eq left right = 0 := by
  simp [Op.apply, different]

/-! ## Bounded graph profile and canonical translation -/

/-- Output type in the admitted fragment. -/
inductive ValueKind where
  | i64
  | boolean
  deriving DecidableEq, Repr

/-- A finite, ordered-output I64 graph. `inputArity` is the input-vector length;
`nodes` are stored in definition-before-use order. -/
structure Program where
  inputArity : Nat
  nodes : List PackedSchedule.Node
  outputs : List Nat
  deriving DecidableEq, Repr

open PackedSchedule

/-- The node at an index; out-of-range indices have a harmless default and are
excluded by `Program.WellFormed`. -/
def Program.nodeAt (program : Program) (index : Nat) : Node :=
  program.nodes.getD index (.constant 0)

/-- Static output kind of one node. -/
def nodeKind : Node → ValueKind
  | .eq _ _ => .boolean
  | _ => .i64

/-- The graph semantics for one selected output, reusing the schedule reference. -/
def Program.graphFor (program : Program) (output : Nat) : Graph where
  arity := program.nodes.length
  output := output
  node := program.nodeAt

/-- Input vector has exactly the declared arity. -/
def Program.InputsOk (program : Program) (inputs : List (BitVec 64)) : Prop :=
  inputs.length = program.inputArity

/-- Every dependency precedes its consumer; input slots and outputs are in
bounds. -/
def Program.WellFormed (program : Program) : Prop :=
  (∀ index, index < program.nodes.length →
    ∀ operand ∈ (program.nodeAt index).operands, operand < index) ∧
  (∀ index, index < program.nodes.length →
    match program.nodeAt index with
    | .input slot => slot < program.inputArity
    | _ => True) ∧
  (∀ output, output ∈ program.outputs → output < program.nodes.length)

/-- Arithmetic and equality operands are I64. Node kinds are syntactically
determined, so this is the complete typing premise for the admitted fragment. -/
def Program.WellTyped (program : Program) : Prop :=
  ∀ index, index < program.nodes.length →
    match program.nodeAt index with
    | .add left right | .sub left right | .mul left right | .eq left right =>
        nodeKind (program.nodeAt left) = ValueKind.i64 ∧
          nodeKind (program.nodeAt right) = ValueKind.i64
    | _ => True

private def Program.nodeInputCheck (program : Program) : Node → Bool
  | .input slot => decide (slot < program.inputArity)
  | _ => true

private def Program.nodeInputBound (program : Program) : Node → Prop
  | .input slot => slot < program.inputArity
  | _ => True

private theorem Program.nodeInputCheck_iff (program : Program) (node : Node) :
    program.nodeInputCheck node = true ↔ program.nodeInputBound node := by
  cases node <;> simp [Program.nodeInputCheck, Program.nodeInputBound]

private def Program.nodeTypedCheck (program : Program) : Node → Bool
  | .add left right | .sub left right | .mul left right | .eq left right =>
      decide (nodeKind (program.nodeAt left) = ValueKind.i64) &&
        decide (nodeKind (program.nodeAt right) = ValueKind.i64)
  | _ => true

private def Program.nodeTyped (program : Program) : Node → Prop
  | .add left right | .sub left right | .mul left right | .eq left right =>
      nodeKind (program.nodeAt left) = ValueKind.i64 ∧
        nodeKind (program.nodeAt right) = ValueKind.i64
  | _ => True

private theorem Program.nodeTypedCheck_iff (program : Program) (node : Node) :
    program.nodeTypedCheck node = true ↔ program.nodeTyped node := by
  cases node <;> simp [Program.nodeTypedCheck, Program.nodeTyped,
    Bool.and_eq_true, decide_eq_true_eq]

/-- Bounded executable reflection of `Program.WellFormed`. -/
def Program.wellFormedCheck (program : Program) : Bool :=
  ((List.range program.nodes.length).all fun index =>
    (program.nodeAt index).operands.all (fun operand => decide (operand < index)) &&
      program.nodeInputCheck (program.nodeAt index)) &&
    program.outputs.all (fun output => decide (output < program.nodes.length))

/-- Bounded executable reflection of `Program.WellTyped`. -/
def Program.wellTypedCheck (program : Program) : Bool :=
  (List.range program.nodes.length).all fun index =>
    program.nodeTypedCheck (program.nodeAt index)

/-- Executable admission check for exact input arity and the selected graph
fragment's well-formedness and type premises. -/
def Program.acceptanceCheck (program : Program) (inputs : List (BitVec 64)) : Bool :=
  decide (inputs.length = program.inputArity) &&
    program.wellFormedCheck && program.wellTypedCheck

theorem Program.wellFormedCheck_iff (program : Program) :
    program.wellFormedCheck = true ↔ program.WellFormed := by
  constructor
  · intro h
    unfold Program.wellFormedCheck at h
    have hparts := h
    rw [Bool.and_eq_true] at hparts
    have hnodes := List.all_eq_true.mp hparts.1
    have houtputs := List.all_eq_true.mp hparts.2
    refine ⟨?_, ?_, ?_⟩
    · intro index hindex operand hmember
      have hnode := hnodes index (List.mem_range.mpr hindex)
      rw [Bool.and_eq_true] at hnode
      exact of_decide_eq_true ((List.all_eq_true.mp hnode.1) operand hmember)
    · intro index hindex
      have hnode := hnodes index (List.mem_range.mpr hindex)
      rw [Bool.and_eq_true] at hnode
      exact (Program.nodeInputCheck_iff program (program.nodeAt index)).mp hnode.2
    · exact fun output hmember => of_decide_eq_true (houtputs output hmember)
  · rintro ⟨hoperands, hinputs, houtputs⟩
    unfold Program.wellFormedCheck
    rw [Bool.and_eq_true]
    constructor
    · apply List.all_eq_true.mpr
      intro index hindex
      rw [Bool.and_eq_true]
      constructor
      · apply List.all_eq_true.mpr
        intro operand hmember
        exact decide_eq_true_iff.mpr
          (hoperands index (List.mem_range.mp hindex) operand hmember)
      · exact (Program.nodeInputCheck_iff program (program.nodeAt index)).mpr
          (hinputs index (List.mem_range.mp hindex))
    · apply List.all_eq_true.mpr
      intro output hmember
      exact decide_eq_true_iff.mpr (houtputs output hmember)

theorem Program.wellTypedCheck_iff (program : Program) :
    program.wellTypedCheck = true ↔ program.WellTyped := by
  simp [Program.wellTypedCheck, Program.WellTyped, Program.nodeTypedCheck_iff,
    Program.nodeTyped, List.all_eq_true, List.mem_range]

theorem Program.acceptanceCheck_iff (program : Program) (inputs : List (BitVec 64)) :
    program.acceptanceCheck inputs = true ↔
      program.InputsOk inputs ∧ program.WellFormed ∧ program.WellTyped := by
  simp [Program.acceptanceCheck, Program.InputsOk, Program.wellFormedCheck_iff,
    Program.wellTypedCheck_iff, Bool.and_eq_true, decide_eq_true_eq, and_assoc]

/-- Input lookup used by the total graph evaluator. -/
def Program.inputValue (inputs : List (BitVec 64)) (slot : Nat) : BitVec 64 :=
  inputs.getD slot 0

/-- The graph reference value for an output node. -/
def Program.referenceValue (program : Program) (inputs : List (BitVec 64))
    (output : Nat) : Option Val :=
  PackedSchedule.value (program.graphFor output) (Program.inputValue inputs) output

inductive Consumer where
  | arg1 (node : Nat)
  | arg2 (node : Nat)
  | output (slot : Nat)
  deriving DecidableEq, Repr

/-- Stable, pairwise-disjoint identifier families for graph, fanout, eraser,
and interface entities. -/
def valueId (node : Nat) : Id := [0, 0, node]
def opId (node : Nat) : Id := [0, 1, node]
def dupId (source level : Nat) : Id := [0, 2, source, level]
def eraId (source : Nat) : Id := [0, 3, source]
def outputPort (slot : Nat) : Port := ([9, 9, slot], 0)

private def Consumer.port : Consumer → Port
  | .arg1 node => (opId node, 0)
  | .arg2 node => (opId node, 1)
  | .output slot => outputPort slot

private def nodeConsumers (nodeIndex source : Nat) (node : Node) : List Consumer :=
  match node with
  | .add left right | .sub left right | .mul left right | .eq left right =>
      (if left == source then [.arg1 nodeIndex] else []) ++
        (if right == source then [.arg2 nodeIndex] else [])
  | _ => []

private def outputConsumers : List Nat → Nat → Nat → List Consumer
  | [], _, _ => []
  | output :: rest, source, slot =>
      (if output == source then [.output slot] else []) ++ outputConsumers rest source (slot + 1)

private def nodeUses (source : Nat) : Nat → List Node → List Consumer
  | _, [] => []
  | index, node :: rest =>
      nodeConsumers index source node ++ nodeUses source (index + 1) rest

private theorem nodeUses_append (source index : Nat) (left right : List Node) :
    nodeUses source index (left ++ right) =
      nodeUses source index left ++ nodeUses source (index + left.length) right := by
  induction left generalizing index with
  | nil => simp [nodeUses]
  | cons node rest ih =>
      simp only [List.cons_append, nodeUses]
      rw [ih, ← List.append_assoc]
      simp only [List.length_cons]
      rw [show (index + 1) + rest.length = index + (rest.length + 1) by omega]

/-- Canonical order: node index, argument one before argument two, then output
position. -/
def Program.usesOf (program : Program) (source : Nat) : List Consumer :=
  nodeUses source 0 program.nodes ++
      outputConsumers program.outputs source 0

private theorem Program.usesOf_length (program : Program) (source : Nat) :
    (program.usesOf source).length =
      (nodeUses source 0 program.nodes).length +
        (outputConsumers program.outputs source 0).length := by
  simp [Program.usesOf]

private def fanoutWires (source : Nat) : Nat → Port → List Consumer → List (Port × Port)
  | _, _, [] => []
  | level, parent, consumer :: rest =>
      match rest with
      | [] => [(parent, consumer.port)]
      | next :: remaining =>
          let duplicate := dupId source level
          (parent, (duplicate, 0)) ::
            ((duplicate, 1), consumer.port) ::
              fanoutWires source (level + 1) (duplicate, 2) (next :: remaining)

private def Program.sourcePort (program : Program) (source : Nat) : Port :=
  match program.nodeAt source with
  | .input _ | .constant _ => (valueId source, 0)
  | _ => (opId source, 2)

private def Program.sourceRoute (program : Program) (source : Nat) : List (Port × Port) :=
  let uses := program.usesOf source
  match uses with
  | [] => [(program.sourcePort source, (eraId source, 0))]
  | _ => fanoutWires source 0 (program.sourcePort source) uses

private def Program.baseAgent (program : Program) (inputs : List (BitVec 64))
    (index : Nat) : Id × NetSym :=
  match program.nodeAt index with
  | .input slot => (valueId index, .num (Program.inputValue inputs slot))
  | .constant value => (valueId index, .num value)
  | .add _ _ => (opId index, .op2 .add)
  | .sub _ _ => (opId index, .op2 .sub)
  | .mul _ _ => (opId index, .op2 .mul)
  | .eq _ _ => (opId index, .op2 .eq)

private def Program.extraAgents (program : Program) (source : Nat) : List (Id × NetSym) :=
  let uses := program.usesOf source
  if uses.isEmpty then [(eraId source, .era)]
  else if uses.length < 2 then []
  else (List.range (uses.length - 1)).map fun level => (dupId source level, .dup)

/-- Total canonical lowering into the executable inets net representation. -/
def Program.translate (program : Program) (inputs : List (BitVec 64)) : XNet NetSym :=
  let base := (List.range program.nodes.length).map (program.baseAgent inputs)
  let extra := (List.range program.nodes.length).flatMap program.extraAgents
  let wires := (List.range program.nodes.length).flatMap program.sourceRoute
  XNet.ofWires (base ++ extra) ((List.range program.outputs.length).map outputPort) wires

/-- Publish only after the executable inets invariant checker accepts the net.
Refusal means the lowering was not established, not that the graph diverges. -/
def Program.translateChecked (program : Program) (inputs : List (BitVec 64)) :
    Option (XNet NetSym) :=
  if program.acceptanceCheck inputs then
    let net := program.translate inputs
    if net.check arity then some net else none
  else none

/-- The exact static number of binary-operation agents in the graph. -/
def Program.opCount (program : Program) : Nat :=
  program.nodes.countP fun node =>
    match node with
    | .add _ _ | .sub _ _ | .mul _ _ | .eq _ _ => true
    | _ => false

/-- The number of duplicators required by the canonical consumer chains. -/
def Program.dupCount (program : Program) : Nat :=
  (List.range program.nodes.length).foldl (fun total source =>
    let count := (program.usesOf source).length
    total + if count < 2 then 0 else count - 1) 0

/-- One eraser is used for every value with no consumers. -/
def Program.eraCount (program : Program) : Nat :=
  (List.range program.nodes.length).countP fun source =>
    (program.usesOf source).isEmpty

/-- Exact cost prescribed by the revision-3 profile. -/
def Program.interactionCount (program : Program) : Nat :=
  2 * program.opCount + program.dupCount + program.eraCount

/-- Initial live-agent bound: one base agent per graph node, plus fanout and erasers. -/
def Program.agentBound (program : Program) : Nat :=
  program.nodes.length + program.dupCount + program.eraCount

/-- Locate the value connected to one output interface port. -/
private def readBackBits (net : XNet NetSym) (slot : Nat) : Option (BitVec 64) := do
  let (agent, port) ← assoc (outputPort slot) net.wires
  if port != 0 then none else pure ()
  let symbol ← assoc agent net.agents
  match symbol with
  | .num bits => some bits
  | _ => none

/-- Read back a normal form only when every output is a numeric agent and
there are no leftover operation, duplicator, or eraser agents. Boolean output
nodes are represented by the canonical I64 encoding `v = 1`. -/
def Program.readBack (program : Program) (net : XNet NetSym) : Option (List Val) := do
  let expectedFree := (List.range program.outputs.length).map outputPort
  if net.free != expectedFree || net.agents.length != program.outputs.length then none else pure ()
  if !net.agents.all (fun agent => match agent.2 with | .num _ => true | _ => false) then none else pure ()
  (List.range program.outputs.length).mapM fun slot => do
    let bits ← readBackBits net slot
    let output := program.outputs.getD slot 0
    match nodeKind (program.nodeAt output) with
    | .i64 => some (.i64 bits)
    | .boolean => some (.boolean (bits == 1))

/-- Reference outputs in interface order, using the packed-schedule graph semantics. -/
def Program.referenceOutputs (program : Program) (inputs : List (BitVec 64)) : Option (List Val) :=
  program.outputs.mapM (program.referenceValue inputs)

/-- Execute exactly the declared profile budget and return only a completed
normal form; `none` never denotes a partial result. -/
def Program.normalize (program : Program) (inputs : List (BitVec 64)) :
    Option (XNet NetSym × Nat) :=
  match program.translateChecked inputs with
  | none => none
  | some net => net.normalize xsys program.interactionCount

/-- A single admitted binary-operation family, parameterized by the rule. -/
def binaryProgram (op : Op) : Program where
  inputArity := 2
  nodes := [.input 0, .input 1,
    match op with
    | .add => .add 0 1
    | .sub => .sub 0 1
    | .mul => .mul 0 1
    | .eq => .eq 0 1]
  outputs := [2]

/-- A fanout family with one input and any number of ordered output copies. -/
def fanoutProgram (outputs : Nat) : Program where
  inputArity := 1
  nodes := [.input 0]
  outputs := List.replicate outputs 0

/-- Negative-control graph with a forward dependency. -/
def forwardReferenceProgram : Program where
  inputArity := 1
  nodes := [.input 0, .add 1 0]
  outputs := [1]

/-- Negative-control graph that feeds a Boolean equality result to integer add. -/
def illTypedProgram : Program where
  inputArity := 1
  nodes := [.input 0, .eq 0 0, .add 1 0]
  outputs := [2]

/-- Negative-control graph whose input node names a slot outside its interface. -/
def invalidInputSlotProgram : Program where
  inputArity := 1
  nodes := [.input 1]
  outputs := [0]

/-- Node list of a linear add chain over one input. Each appended operation
adds the preceding result to that input. -/
def chainNodes : Nat → List PackedSchedule.Node
  | 0 => []
  | count + 1 => chainNodes count ++ [.add count 0]

private theorem chainNodes_length (operations : Nat) :
    (chainNodes operations).length = operations := by
  induction operations with
  | zero => rfl
  | succ operations ih => simp [chainNodes, ih]

/-- A growing, definition-before-use operation chain over an arbitrary I64 input. -/
def chainProgram (operations : Nat) : Program where
  inputArity := 1
  nodes := [.input 0] ++ chainNodes operations
  outputs := [operations]

private theorem chainNodes_opCount (operations : Nat) :
    (chainNodes operations).countP (fun node =>
      match node with
      | .add _ _ | .sub _ _ | .mul _ _ | .eq _ _ => true
      | _ => false) = operations := by
  induction operations with
  | zero => simp [chainNodes]
  | succ operations ih =>
      simp [chainNodes, List.countP_append, ih]

theorem chainProgram_opCount (operations : Nat) :
    (chainProgram operations).opCount = operations := by
  simp [Program.opCount, chainProgram, chainNodes_opCount]

private theorem chainNodes_uses_length (operations source : Nat) :
    (nodeUses source 1 (chainNodes operations)).length =
      if source = 0 then (if operations = 0 then 0 else operations + 1)
      else if source < operations then 1 else 0 := by
  induction operations with
  | zero => simp [chainNodes, nodeUses]
  | succ operations ih =>
      rw [chainNodes, nodeUses_append]
      simp [ih, chainNodes_length, nodeUses, nodeConsumers]
      by_cases hzero : source = 0
      · subst source
        cases operations with
        | zero => simp
        | succ operations => simp
      · by_cases hlt : source < operations
        · have hne : operations ≠ source := by omega
          have hne0 : 0 ≠ source := by omega
          simp [hzero, hlt, hne, hne0] <;> omega
        · by_cases heq : source = operations
          · subst source
            simp [hzero] <;> omega
          · have hgt : operations < source := by omega
            have hne : operations ≠ source := by omega
            have hnotlt : ¬ source < operations + 1 := by omega
            have hne0 : 0 ≠ source := by omega
            simp [hzero, hlt, hne, hnotlt, hne0]

theorem chainProgram_uses_count (operations source : Nat) :
    ((chainProgram operations).usesOf source).length =
      if source = 0 then operations + 1 else if source ≤ operations then 1 else 0 := by
  rw [Program.usesOf_length]
  simp [chainProgram, nodeUses, nodeConsumers, outputConsumers,
    chainNodes_uses_length]
  by_cases hzero : source = 0
  · subst source
    by_cases hops : operations = 0
    · simp [hops]
    · have hpos : 0 < operations := Nat.pos_of_ne_zero hops
      simp [hops] <;> omega
  · by_cases heq : source = operations
    · subst source
      simp [hzero]
    · by_cases hlt : source < operations
      · have hne : operations ≠ source := by omega
        simp [hzero, hlt, hne] <;> omega
      · have hgt : operations < source := by omega
        have hne : operations ≠ source := by omega
        simp [hzero, hlt, hgt, hne] <;> omega

private theorem range_fold_zero (length weight : Nat) :
    (List.range length).foldl
        (fun total source => total + if source = 0 then weight else 0) 0 =
      if length = 0 then 0 else weight := by
  induction length with
  | zero => simp
  | succ length ih =>
      rw [List.range_succ]
      by_cases hzero : length = 0
      · subst length
        simp
      · have hprefix :
            (List.range length).foldl
              (fun total source => total + if source = 0 then weight else 0) 0 = weight := by
          simpa [hzero] using ih
        simp [hzero, hprefix]

private theorem range_countP_zero (predicate : Nat → Bool) :
    ∀ length, (∀ source, source < length → predicate source = false) →
      (List.range length).countP predicate = 0 := by
  intro length
  induction length with
  | zero => intro _; simp
  | succ length ih =>
      intro hfalse
      rw [List.range_succ, List.countP_append]
      have hprefix : (List.range length).countP predicate = 0 := by
        apply ih
        intro source hsource
        exact hfalse source (by omega)
      rw [hprefix]
      simp [hfalse length (by omega)]

theorem chainProgram_dupCount (operations : Nat) :
    (chainProgram operations).dupCount = operations := by
  unfold Program.dupCount
  have hlength : (chainProgram operations).nodes.length = operations + 1 := by
    simp [chainProgram, chainNodes_length]
  rw [hlength]
  simp only [chainProgram_uses_count]
  have hcost (source : Nat) :
      (if (if source = 0 then operations + 1
        else if source ≤ operations then 1 else 0) < 2 then 0
        else (if source = 0 then operations + 1
          else if source ≤ operations then 1 else 0) - 1) =
        if source = 0 then operations else 0 := by
    by_cases hzero : source = 0
    · subst source
      cases operations with
      | zero => simp
      | succ operations => simp <;> omega
    · by_cases hwithin : source ≤ operations <;> simp [hzero, hwithin]
  simp only [hcost]
  simpa using range_fold_zero (operations + 1) operations

private theorem chainProgram_use_isEmpty (operations source : Nat)
    (hsource : source ≤ operations) :
    ((chainProgram operations).usesOf source).isEmpty = false := by
  have hlength := chainProgram_uses_count operations source
  have hpositive : 0 < ((chainProgram operations).usesOf source).length := by
    rw [hlength]
    by_cases hzero : source = 0
    · subst source
      simp
    · simp [hzero, hsource]
  cases huses : (chainProgram operations).usesOf source with
  | nil => simp [huses] at hpositive
  | cons first rest => rfl

theorem chainProgram_eraCount (operations : Nat) :
    (chainProgram operations).eraCount = 0 := by
  unfold Program.eraCount
  have hlength : (chainProgram operations).nodes.length = operations + 1 := by
    simp [chainProgram, chainNodes_length]
  rw [hlength]
  apply range_countP_zero
  intro source hsource
  have hwithin : source ≤ operations := by omega
  exact chainProgram_use_isEmpty operations source hwithin

theorem chainProgram_interactionCount (operations : Nat) :
    (chainProgram operations).interactionCount = 3 * operations := by
  rw [Program.interactionCount, chainProgram_opCount,
    chainProgram_dupCount, chainProgram_eraCount]
  omega

theorem chainProgram_agentBound (operations : Nat) :
    (chainProgram operations).agentBound = 2 * operations + 1 := by
  rw [Program.agentBound, chainProgram_dupCount, chainProgram_eraCount]
  have hlength : (chainProgram operations).nodes.length = operations + 1 := by
    simp [chainProgram, chainNodes_length]
  rw [hlength]
  omega

/-- Build a perfect balanced add tree in postorder, returning its appended
nodes, next free node index and root. -/
def balancedSubtree : Nat → Nat → Nat → List PackedSchedule.Node × Nat × Nat
  | 0, firstLeaf, next => ([], next, firstLeaf)
  | depth + 1, firstLeaf, next =>
      let (leftNodes, afterLeft, leftRoot) := balancedSubtree depth firstLeaf next
      let (rightNodes, afterRight, rightRoot) :=
        balancedSubtree depth (firstLeaf + 2 ^ depth) afterLeft
      (leftNodes ++ rightNodes ++ [.add leftRoot rightRoot], afterRight + 1, afterRight)

/-- A perfect balanced tree of `2^depth` arbitrary I64 input leaves. -/
def balancedProgram (depth : Nat) : Program :=
  let leaves := 2 ^ depth
  let (operations, _, root) := balancedSubtree depth 0 leaves
  { inputArity := leaves
    nodes := (List.range leaves).map Node.input ++ operations
    outputs := [root] }

private theorem balancedSubtree_length_add_one (depth firstLeaf next : Nat) :
    ((balancedSubtree depth firstLeaf next).1).length + 1 = 2 ^ depth := by
  induction depth generalizing firstLeaf next with
  | zero => simp [balancedSubtree]
  | succ depth ih =>
      have hleft := ih firstLeaf next
      have hright := ih (firstLeaf + 2 ^ depth)
        ((balancedSubtree depth firstLeaf next).2.1)
      simp [balancedSubtree, hright, List.length_append, Nat.pow_succ,
        Nat.add_assoc]
      omega

private theorem balancedSubtree_length (depth firstLeaf next : Nat) :
    ((balancedSubtree depth firstLeaf next).1).length = 2 ^ depth - 1 := by
  have h := balancedSubtree_length_add_one depth firstLeaf next
  omega

private theorem balancedSubtree_opCount_add_one (depth firstLeaf next : Nat) :
    ((balancedSubtree depth firstLeaf next).1).countP (fun node =>
      match node with
      | .add _ _ | .sub _ _ | .mul _ _ | .eq _ _ => true
      | _ => false) + 1 = 2 ^ depth := by
  induction depth generalizing firstLeaf next with
  | zero => simp [balancedSubtree]
  | succ depth ih =>
      have hleft := ih firstLeaf next
      have hright := ih (firstLeaf + 2 ^ depth)
        ((balancedSubtree depth firstLeaf next).2.1)
      simp [balancedSubtree, hright, List.countP_append, Nat.pow_succ]
      omega

private theorem balancedSubtree_opCount (depth firstLeaf next : Nat) :
    ((balancedSubtree depth firstLeaf next).1).countP (fun node =>
      match node with
      | .add _ _ | .sub _ _ | .mul _ _ | .eq _ _ => true
      | _ => false) = 2 ^ depth - 1 := by
  have h := balancedSubtree_opCount_add_one depth firstLeaf next
  omega

private theorem inputNodes_opCount (leaves : Nat) :
    ((List.range leaves).map Node.input).countP (fun node =>
      match node with
      | .add _ _ | .sub _ _ | .mul _ _ | .eq _ _ => true
      | _ => false) = 0 := by
  induction leaves with
  | zero => simp
  | succ leaves ih => simp [List.range_succ, List.countP_append, ih]

private theorem balancedSubtree_next_eq (depth firstLeaf next : Nat) :
    (balancedSubtree depth firstLeaf next).2.1 =
      next + ((balancedSubtree depth firstLeaf next).1).length := by
  induction depth generalizing firstLeaf next with
  | zero => simp [balancedSubtree]
  | succ depth ih =>
      have hleft := ih firstLeaf next
      have hright := ih (firstLeaf + 2 ^ depth)
        ((balancedSubtree depth firstLeaf next).2.1)
      rw [hleft] at hright
      simp [balancedSubtree, hleft, hright, List.length_append, Nat.add_assoc]

private theorem balancedSubtree_root_next_eq (depth firstLeaf next : Nat)
    (hdepth : 0 < depth) :
    (balancedSubtree depth firstLeaf next).2.2 + 1 =
      next + ((balancedSubtree depth firstLeaf next).1).length := by
  cases depth with
  | zero => omega
  | succ depth =>
      have hnext := balancedSubtree_next_eq (depth + 1) firstLeaf next
      have hroot : (balancedSubtree (depth + 1) firstLeaf next).2.1 =
          (balancedSubtree (depth + 1) firstLeaf next).2.2 + 1 := by
        simp [balancedSubtree]
      exact hroot.symm.trans hnext

private theorem balancedSubtree_root_data (depth firstLeaf next : Nat) :
    (depth = 0 → (balancedSubtree depth firstLeaf next).2.2 = firstLeaf) ∧
    (0 < depth → (balancedSubtree depth firstLeaf next).2.2 + 1 =
      next + ((balancedSubtree depth firstLeaf next).1).length) := by
  constructor
  · intro hzero
    subst depth
    simp [balancedSubtree]
  · intro hpositive
    exact balancedSubtree_root_next_eq depth firstLeaf next hpositive

private theorem balanced_leaf_range_partition (first width source : Nat) :
    (if first ≤ source ∧ source < first + width then 1 else 0) +
      (if first + width ≤ source ∧ source < first + width + width then 1 else 0) =
        if first ≤ source ∧ source < first + (width + width) then 1 else 0 := by
  by_cases hstart : first ≤ source
  · by_cases hleft : source < first + width
    · have hwhole : source < first + (width + width) := by omega
      have hright : ¬(first + width ≤ source ∧ source < first + width + width) := by omega
      simp [hstart, hleft, hwhole, hright]
    · by_cases hrightEnd : source < first + width + width
      · have hrightStart : first + width ≤ source := by omega
        have hwhole : source < first + (width + width) := by omega
        simp [hstart, hleft, hrightStart, hrightEnd, hwhole]
      · have hwhole : ¬source < first + (width + width) := by omega
        simp [hstart, hleft, hwhole, hrightEnd]
  · have hright : ¬(first + width ≤ source ∧ source < first + width + width) := by omega
    simp [hstart, hright]

private theorem balanced_op_range_partition
    (start leftLen rightLen parentLen leftRoot rightRoot source : Nat)
    (hleftPositive : 0 < leftLen) (hrightPositive : 0 < rightLen)
    (hleftRoot : leftRoot + 1 = start + leftLen)
    (hrightRoot : rightRoot + 1 = start + leftLen + rightLen)
    (hparentLength : parentLen = leftLen + rightLen + 1) :
    (if start ≤ source ∧ source + 1 < start + leftLen then 1 else 0) +
      (if start + leftLen ≤ source ∧ source + 1 < start + leftLen + rightLen
        then 1 else 0) +
      (if leftRoot = source then 1 else 0) +
      (if rightRoot = source then 1 else 0) =
        if start ≤ source ∧ source + 1 < start + parentLen then 1 else 0 := by
  rw [hparentLength]
  by_cases hstart : start ≤ source
  · by_cases hparent : source + 1 < start + (leftLen + rightLen + 1)
    · by_cases hleftInternal : source + 1 < start + leftLen
      · have hnotRightStart : ¬(start + leftLen ≤ source) := by omega
        have hnotLeftRoot : leftRoot ≠ source := by omega
        have hnotRightRoot : rightRoot ≠ source := by omega
        simp [hstart, hparent, hleftInternal, hnotRightStart,
          hnotLeftRoot, hnotRightRoot] <;> omega
      · by_cases hrightStart : start + leftLen ≤ source
        · by_cases hrightInternal : source + 1 < start + leftLen + rightLen
          · have hnotLeftRoot : leftRoot ≠ source := by omega
            have hnotRightRoot : rightRoot ≠ source := by omega
            simp [hstart, hparent, hleftInternal, hrightStart,
              hrightInternal, hnotLeftRoot, hnotRightRoot] <;> omega
          · have hrightAt : rightRoot = source := by omega
            have hnotLeftRoot : leftRoot ≠ source := by omega
            simp [hstart, hparent, hleftInternal, hrightStart,
              hrightInternal, hrightAt, hnotLeftRoot] <;> omega
        · have hleftAt : leftRoot = source := by omega
          have hnotRightRoot : rightRoot ≠ source := by omega
          simp [hstart, hparent, hleftInternal, hrightStart,
            hleftAt, hnotRightRoot] <;> omega
    · have hnotLeftRange :
          ¬(source + 1 < start + leftLen) := by omega
      have hnotRightRange :
          ¬(start + leftLen ≤ source ∧ source + 1 < start + leftLen + rightLen) := by omega
      have hnotLeftRoot : leftRoot ≠ source := by omega
      have hnotRightRoot : rightRoot ≠ source := by omega
      simp [hstart, hparent, hnotRightRange,
        hnotLeftRoot, hnotRightRoot] <;> omega
  · have hnotLeftRoot : leftRoot ≠ source := by omega
    have hnotRightRoot : rightRoot ≠ source := by omega
    have hnotRightRange :
        ¬(start + leftLen ≤ source ∧ source + 1 < start + leftLen + rightLen) := by omega
    simp [hstart, hnotRightRange, hnotLeftRoot, hnotRightRoot] <;> omega

private def balancedSubtree_uses_expected
    (depth firstLeaf next source : Nat) : Nat :=
  (if 0 < depth ∧ firstLeaf ≤ source ∧ source < firstLeaf + 2 ^ depth
    then 1 else 0) +
  (if next ≤ source ∧ source + 1 < next +
      ((balancedSubtree depth firstLeaf next).1).length then 1 else 0)

private theorem balancedSubtree_uses_length (depth firstLeaf next source : Nat) :
    (nodeUses source next (balancedSubtree depth firstLeaf next).1).length =
      balancedSubtree_uses_expected depth firstLeaf next source := by
  induction depth generalizing firstLeaf next source with
  | zero =>
      by_cases hnext : next ≤ source
      · have hinner : ¬ source + 1 < next := by omega
        simp [balancedSubtree_uses_expected, balancedSubtree, nodeUses, hnext, hinner]
      · simp [balancedSubtree_uses_expected, balancedSubtree, nodeUses, hnext]
  | succ depth ih =>
      let left := balancedSubtree depth firstLeaf next
      let right := balancedSubtree depth (firstLeaf + 2 ^ depth) left.2.1
      by_cases hdepthZero : depth = 0
      · subst depth
        by_cases hfirst : source = firstLeaf
        · subst source
          simp [balancedSubtree_uses_expected, balancedSubtree, nodeUses, nodeConsumers]
        · by_cases hsecond : source = firstLeaf + 1
          · subst source
            simp [balancedSubtree_uses_expected, balancedSubtree, nodeUses, nodeConsumers]
          · have hnotRange : ¬ (firstLeaf ≤ source ∧ source < firstLeaf + 2) := by omega
            have hleft : firstLeaf ≠ source := by omega
            have hright : firstLeaf + 1 ≠ source := by omega
            simp [balancedSubtree_uses_expected, balancedSubtree, nodeUses, nodeConsumers,
              hnotRange, hleft, hright]
      · have hdepthPositive : 0 < depth := by omega
        have hleftNext : left.2.1 = next + left.1.length := by
          simpa [left] using balancedSubtree_next_eq depth firstLeaf next
        have hleft :
            (nodeUses source next left.1).length =
              balancedSubtree_uses_expected depth firstLeaf next source := by
          simpa [left] using ih firstLeaf next source
        have hright :
            (nodeUses source left.2.1 right.1).length =
              balancedSubtree_uses_expected depth (firstLeaf + 2 ^ depth) left.2.1 source := by
          simpa [left, right] using ih (firstLeaf + 2 ^ depth) left.2.1 source
        have hleftLen : left.1.length = 2 ^ depth - 1 := by
          simpa [left] using balancedSubtree_length depth firstLeaf next
        have hrightLen : right.1.length = 2 ^ depth - 1 := by
          simpa [right] using balancedSubtree_length depth (firstLeaf + 2 ^ depth) left.2.1
        have hparentLen :
            ((balancedSubtree (depth + 1) firstLeaf next).1).length =
              left.1.length + right.1.length + 1 := by
          change (left.1 ++ right.1 ++
            [PackedSchedule.Node.add left.2.2 right.2.2]).length = _
          simp [List.length_append, Nat.add_assoc, right]
        have hleftRootNext : left.2.2 + 1 = next + left.1.length := by
          simpa [left] using
            balancedSubtree_root_next_eq depth firstLeaf next hdepthPositive
        have hrightRootNext : right.2.2 + 1 = next + left.1.length + right.1.length := by
          rw [← hleftNext]
          simpa [right] using
            balancedSubtree_root_next_eq depth (firstLeaf + 2 ^ depth) left.2.1
              hdepthPositive
        have hleftPositive : 0 < left.1.length := by
          rw [hleftLen]
          cases depth with
          | zero => omega
          | succ depth =>
              rw [Nat.pow_succ]
              have hpow : 0 < 2 ^ depth := Nat.pow_pos (by decide : 0 < (2 : Nat))
              omega
        have hrightPositive : 0 < right.1.length := by
          rw [hrightLen]
          cases depth with
          | zero => omega
          | succ depth =>
              rw [Nat.pow_succ]
              have hpow : 0 < 2 ^ depth := Nat.pow_pos (by decide : 0 < (2 : Nat))
              omega
        have hpower : 2 ^ (depth + 1) = 2 ^ depth + 2 ^ depth := by
          simp [Nat.pow_succ]
          omega
        change (nodeUses source next
          (left.1 ++ right.1 ++ [PackedSchedule.Node.add left.2.2 right.2.2])).length =
            balancedSubtree_uses_expected (depth + 1) firstLeaf next source
        rw [nodeUses_append, nodeUses_append]
        simp only [List.length_append]
        have hrootUses :
            (nodeUses source (next + (left.1.length + right.1.length))
              [PackedSchedule.Node.add left.2.2 right.2.2]).length =
              (if left.2.2 = source then 1 else 0) +
                (if right.2.2 = source then 1 else 0) := by
          by_cases hleftRoot : left.2.2 = source <;>
            by_cases hrightRoot : right.2.2 = source <;>
            simp [nodeUses, nodeConsumers, hleftRoot, hrightRoot]
        rw [hleft, ← hleftNext, hright, hleftNext, hrootUses]
        simp [balancedSubtree_uses_expected, hdepthPositive]
        rw [hpower]
        rw [balancedSubtree_length depth firstLeaf next]
        rw [← hleftLen]
        rw [balancedSubtree_length depth (firstLeaf + 2 ^ depth)
          (next + left.1.length)]
        rw [← hrightLen]
        rw [hparentLen]
        have hleaf := balanced_leaf_range_partition firstLeaf (2 ^ depth) source
        have hop := balanced_op_range_partition next left.1.length right.1.length
          (left.1.length + right.1.length + 1)
          left.2.2 right.2.2 source hleftPositive hrightPositive hleftRootNext
          hrightRootNext rfl
        omega

theorem balancedProgram_opCount (depth : Nat) :
    (balancedProgram depth).opCount = 2 ^ depth - 1 := by
  simp only [Program.opCount, balancedProgram, List.countP_append]
  rw [inputNodes_opCount, balancedSubtree_opCount]
  omega

private theorem inputNodes_uses_empty (count source : Nat) :
    nodeUses source 0 ((List.range count).map Node.input) = [] := by
  induction count with
  | zero => rfl
  | succ count ih =>
      rw [List.range_succ, List.map_append, List.map_singleton, nodeUses_append]
      simp [nodeUses, nodeConsumers, ih]

private theorem balancedProgram_uses_once (depth source : Nat)
    (hsource : source < (balancedProgram depth).nodes.length) :
    ((balancedProgram depth).usesOf source).length = 1 := by
  cases depth with
  | zero =>
      have hsource0 : source = 0 := by
        have hlt : source < 1 := by simpa [balancedProgram, balancedSubtree] using hsource
        omega
      subst source
      simp [balancedProgram, Program.usesOf_length, outputConsumers,
        nodeUses, nodeConsumers, balancedSubtree]
  | succ depth =>
      let leaves := 2 ^ (depth + 1)
      let tree := balancedSubtree (depth + 1) 0 leaves
      have hsourceBound : source < leaves + tree.1.length := by
        simpa [balancedProgram, leaves, tree] using hsource
      have htreeUses := balancedSubtree_uses_length (depth + 1) 0 leaves source
      have htreeUses' : (nodeUses source leaves tree.1).length =
          balancedSubtree_uses_expected (depth + 1) 0 leaves source := by
        simpa [tree] using htreeUses
      have htreeLen := balancedSubtree_length (depth + 1) 0 leaves
      have htreeRoot : tree.2.2 + 1 = leaves + tree.1.length := by
        simpa [tree] using
          balancedSubtree_root_next_eq (depth + 1) 0 leaves (by omega)
      have htreeRootRaw : tree.2.2 + 1 = leaves +
          ((balancedSubtree (depth + 1) 0 leaves).1).length := by
        simpa [tree] using htreeRoot
      have htreeLenPositive : 0 < tree.1.length := by
        rw [htreeLen]
        have hpow : 0 < 2 ^ (depth + 1) := Nat.pow_pos (by decide : 0 < (2 : Nat))
        omega
      have hrootAtOrAfterLeaves : leaves ≤ tree.2.2 := by omega
      rw [Program.usesOf_length]
      change (nodeUses source 0 ((List.range leaves).map Node.input ++ tree.1)).length +
          (outputConsumers [tree.2.2] source 0).length = 1
      rw [nodeUses_append]
      simp only [inputNodes_uses_empty, List.nil_append, Nat.zero_add,
        List.length_map, List.length_range]
      rw [htreeUses']
      simp only [outputConsumers]
      by_cases hleaf : source < leaves
      · have hnotRoot : tree.2.2 ≠ source := by omega
        have hnotAfter : ¬leaves ≤ source := by omega
        simp [balancedSubtree_uses_expected, leaves, hleaf, hnotAfter, hnotRoot]
      · have hleafFalse : ¬source < leaves := hleaf
        have hafterLeaves : leaves ≤ source := by omega
        by_cases hbelowRoot : source < tree.2.2
        · have hnotRoot : tree.2.2 ≠ source := by omega
          have hbeforeEnd : source + 1 < leaves +
              ((balancedSubtree (depth + 1) 0 leaves).1).length := by
            rw [← htreeRootRaw]
            omega
          simp [balancedSubtree_uses_expected, leaves, hleafFalse, hafterLeaves,
            hbeforeEnd, hnotRoot]
        · by_cases hroot : source = tree.2.2
          · simp [balancedSubtree_uses_expected, hroot]
            constructor
            · simpa [leaves] using hrootAtOrAfterLeaves
            · intro _
              rw [← htreeRootRaw]
              exact Nat.le_refl _
          · have haboveRoot : tree.2.2 < source := by omega
            rw [← htreeRoot] at hsourceBound
            omega

private theorem range_fold_uses_once_zero (length : Nat) (uses : Nat → Nat)
    (huses : ∀ source, source < length → uses source = 1) :
    (List.range length).foldl
        (fun total source => total + if uses source < 2 then 0 else uses source - 1) 0 = 0 := by
  induction length with
  | zero => rfl
  | succ length ih =>
      rw [List.range_succ, List.foldl_append]
      have hprefix := ih (fun source hsource => huses source (by omega))
      rw [hprefix]
      simp [huses length (by omega)]

theorem balancedProgram_dupCount (depth : Nat) :
    (balancedProgram depth).dupCount = 0 := by
  unfold Program.dupCount
  apply range_fold_uses_once_zero
  intro source hsource
  exact balancedProgram_uses_once depth source hsource

theorem balancedProgram_eraCount (depth : Nat) :
    (balancedProgram depth).eraCount = 0 := by
  unfold Program.eraCount
  apply range_countP_zero
  intro source hsource
  have hlength := balancedProgram_uses_once depth source hsource
  have hnotEmpty : ((balancedProgram depth).usesOf source).isEmpty = false := by
    cases huses : (balancedProgram depth).usesOf source with
    | nil => simp [huses] at hlength
    | cons first rest => rfl
  exact hnotEmpty

theorem balancedProgram_interactionCount (depth : Nat) :
    (balancedProgram depth).interactionCount = 2 * (2 ^ depth - 1) := by
  rw [Program.interactionCount, balancedProgram_opCount,
    balancedProgram_dupCount, balancedProgram_eraCount]
  omega

theorem balancedProgram_agentBound (depth : Nat) :
    (balancedProgram depth).agentBound = 2 ^ (depth + 1) - 1 := by
  rw [Program.agentBound, balancedProgram_dupCount, balancedProgram_eraCount]
  simp only [balancedProgram, List.length_append, List.length_map, List.length_range,
    balancedSubtree_length]
  simp [Nat.pow_succ]
  omega

private theorem outputConsumers_replicate_length (count slot : Nat) :
    (outputConsumers (List.replicate count 0) 0 slot).length = count := by
  induction count generalizing slot with
  | zero => rfl
  | succ count ih =>
      change ([Consumer.output slot] ++ outputConsumers (List.replicate count 0) 0 (slot + 1)).length =
        count + 1
      simp [ih]

private theorem fanoutProgram_uses_length (count : Nat) :
    ((fanoutProgram count).usesOf 0).length = count := by
  change ([] ++ outputConsumers (List.replicate count 0) 0 0).length = count
  simp [outputConsumers_replicate_length]

private theorem fanoutProgram_uses_empty (count : Nat) :
    ((fanoutProgram count).usesOf 0) = [] ↔ count = 0 := by
  cases hUses : (fanoutProgram count).usesOf 0 with
  | nil =>
      have hCount := fanoutProgram_uses_length count
      rw [hUses, List.length_nil] at hCount
      simp [hCount]
  | cons first rest =>
      have hCount := fanoutProgram_uses_length count
      rw [hUses] at hCount
      have hpos : 0 < count := by simp at hCount; omega
      simp [Nat.ne_of_gt hpos]

private theorem fanoutProgram_uses_isEmpty (count : Nat) :
    ((fanoutProgram count).usesOf 0).isEmpty = (count == 0) := by
  cases hUses : (fanoutProgram count).usesOf 0 with
  | nil =>
      have hCount := fanoutProgram_uses_length count
      rw [hUses, List.length_nil] at hCount
      simp [hCount]
  | cons first rest =>
      have hCount := fanoutProgram_uses_length count
      rw [hUses] at hCount
      have hpos : 0 < count := by simp at hCount; omega
      simp [Nat.ne_of_gt hpos]

private theorem fanoutProgram_dupCount (count : Nat) :
    (fanoutProgram count).dupCount = if count < 2 then 0 else count - 1 := by
  change (List.range 1).foldl (fun total source => total +
    if ((fanoutProgram count).usesOf source).length < 2 then 0
    else ((fanoutProgram count).usesOf source).length - 1) 0 = _
  simp [List.range_one, fanoutProgram_uses_length]

private theorem fanoutProgram_eraCount (count : Nat) :
    (fanoutProgram count).eraCount = if count = 0 then 1 else 0 := by
  change (List.range 1).countP (fun source => ((fanoutProgram count).usesOf source).isEmpty) = _
  simp [List.range_one, fanoutProgram_uses_isEmpty]

/-- All four arithmetic/equality members have exactly two principal-pair firings. -/
theorem binaryProgram_interactionCount (op : Op) :
    (binaryProgram op).interactionCount = 2 := by
  cases op <;> decide

/-- Fanout-only interaction count: no output means the input is erased; each
additional output copy consumes one duplicator interaction. -/
theorem fanoutProgram_interactionCount (outputs : Nat) :
    (fanoutProgram outputs).interactionCount = if outputs = 0 then 1 else outputs - 1 := by
  rw [Program.interactionCount, fanoutProgram_dupCount outputs, fanoutProgram_eraCount outputs]
  simp only [Program.opCount, fanoutProgram, List.countP_cons, List.countP_nil]
  by_cases hz : outputs = 0
  · simp [hz]
  · by_cases hlt : outputs < 2
    · have hone : outputs = 1 := by omega
      simp [hone]
    · simp [hz, hlt]

/-- The fanout family's closed-form initial-agent bound is the numeric agent,
plus one duplicator per additional output, or an eraser when it has no use. -/
theorem fanoutProgram_agentBound (outputs : Nat) :
    (fanoutProgram outputs).agentBound =
      if outputs = 0 then 2 else if outputs = 1 then 1 else outputs := by
  rw [Program.agentBound, fanoutProgram_dupCount outputs, fanoutProgram_eraCount outputs]
  simp only [fanoutProgram, List.length_cons, List.length_nil]
  by_cases hzero : outputs = 0
  · simp [hzero]
  · by_cases hone : outputs = 1
    · simp [hone]
    · have hmany : 2 ≤ outputs := by omega
      simp [hzero, hone, Nat.not_lt.mpr hmany]
      omega

/-- The first nontrivial chain fixture has three operation firings, three
fanout firings, and the exact initial live-agent count. -/
theorem chain_three_family_counts :
    (chainProgram 3).opCount = 3 ∧
      (chainProgram 3).dupCount = 3 ∧
      (chainProgram 3).eraCount = 0 ∧
      (chainProgram 3).interactionCount = 9 ∧
      (chainProgram 3).agentBound = 7 := by
  decide

/-- The depth-two balanced tree has three operation agents, no fanout or
erasers, six interactions, and seven initial live agents. -/
theorem balanced_depth_two_family_counts :
    (balancedProgram 2).opCount = 3 ∧
      (balancedProgram 2).dupCount = 0 ∧
      (balancedProgram 2).eraCount = 0 ∧
      (balancedProgram 2).interactionCount = 6 ∧
      (balancedProgram 2).agentBound = 7 := by
  decide

/-- Kernel-evaluated three-step chain trace: nine firings, graph-reference
agreement, and the expected wrapping-I64 result. -/
theorem chain_three_reference :
    ((chainProgram 3).normalize [5]).map Prod.snd = some 9 ∧
      ((chainProgram 3).normalize [5]).bind
        (fun (normal, _) => (chainProgram 3).readBack normal) = some [.i64 20] ∧
      (chainProgram 3).referenceOutputs [5] = some [.i64 20] := by
  decide

/-- Kernel-evaluated depth-two balanced-tree trace: six firings and agreement
between its read-back and the ordered packed-schedule reference. -/
theorem balanced_depth_two_reference :
    ((balancedProgram 2).normalize [1, 2, 3, 4]).map Prod.snd = some 6 ∧
      ((balancedProgram 2).normalize [1, 2, 3, 4]).bind
        (fun (normal, _) => (balancedProgram 2).readBack normal) = some [.i64 10] ∧
      (balancedProgram 2).referenceOutputs [1, 2, 3, 4] = some [.i64 10] := by
  decide

/-- The arbitrary-size fanout family computes three ordered copies, with one
interaction per DUP and the expected initial-agent peak. -/
theorem fanout_three_reference :
    ((fanoutProgram 3).normalize [7]).map Prod.snd = some 2 ∧
      ((fanoutProgram 3).normalize [7]).bind
        (fun (normal, _) => (fanoutProgram 3).readBack normal) =
          some [.i64 7, .i64 7, .i64 7] ∧
      (fanoutProgram 3).referenceOutputs [7] = some [.i64 7, .i64 7, .i64 7] := by
  decide

/-- An unused input takes exactly one ERA interaction and produces no values. -/
theorem fanout_zero_reference :
    ((fanoutProgram 0).normalize [7]).map Prod.snd = some 1 ∧
      ((fanoutProgram 0).normalize [7]).bind
        (fun (normal, _) => (fanoutProgram 0).readBack normal) = some [] ∧
      (fanoutProgram 0).referenceOutputs [7] = some [] := by
  decide

/-- Two independent ordered subtraction nodes, used to expose schedule choice
without changing either rule or graph. -/
def parallelSubProgram : Program where
  inputArity := 4
  nodes := [.input 0, .input 1, .input 2, .input 3,
    .sub 0 1, .sub 2 3]
  outputs := [4, 5]

def parallelSubInputs : List (BitVec 64) := [7, 3, 3, 7]

def parallelSubNet : XNet NetSym := parallelSubProgram.translate parallelSubInputs

/-- Reversing the finite representation's agent scan order selects the other
independent active pair first; the represented abstract net is unchanged. -/
def parallelSubReversedNet : XNet NetSym :=
  { parallelSubNet with agents := parallelSubNet.agents.reverse }

def parallelSubNormalize (reverseAgents : Bool) : Option (XNet NetSym × Nat) :=
  XNet.normalize xsys 4
    (if reverseAgents then parallelSubReversedNet else parallelSubNet)

def parallelSubScheduleResult (reverseAgents : Bool) :
    Option (Nat × List Val × List Val) :=
  match parallelSubNormalize reverseAgents with
  | none => none
  | some (normal, steps) =>
      match parallelSubProgram.readBack normal,
          parallelSubProgram.referenceOutputs parallelSubInputs with
      | some output, some reference => some (steps, output, reference)
      | _, _ => none

/-- Reordering the finite agent inventory changes neither the abstract net nor
its output boundary. -/
theorem parallel_sub_initial_net_equal :
    parallelSubNet.toNet = parallelSubReversedNet.toNet := by
  apply Net.ext'
  · intro id symbol
    simp [XNet.toNet, parallelSubReversedNet, List.mem_reverse]
  · intro left right
    rfl
  · intro port
    rfl

/-- The two explicit agent-scan orders start by firing different independent
subtraction pairs. -/
theorem parallel_sub_schedules_start_differ :
    (XNet.findRedex xsys parallelSubNet).map
        (fun redex => (redex.1, redex.2.1)) =
        some (valueId 0, opId 4) ∧
      (XNet.findRedex xsys parallelSubReversedNet).map
        (fun redex => (redex.1, redex.2.1)) = some (opId 5, valueId 2) := by
  decide

/-- Inets' normal-form theorem identifies the abstract result and exact count
of any successful normalization under either agent-scan order. -/
theorem parallel_sub_schedules_same_normal_form
    {left right : XNet NetSym} {leftCount rightCount : Nat}
    (leftResult : parallelSubNormalize false = some (left, leftCount))
    (rightResult : parallelSubNormalize true = some (right, rightCount)) :
    left.toNet = right.toNet ∧ leftCount = rightCount := by
  unfold parallelSubNormalize at leftResult rightResult
  change XNet.normalize xsys 4 parallelSubNet = some (left, leftCount) at leftResult
  change XNet.normalize xsys 4 parallelSubReversedNet = some (right, rightCount) at rightResult
  obtain ⟨leftOk, _, leftSteps, leftNormal, _⟩ := XNet.normalize_spec leftResult
  obtain ⟨_, _, rightSteps, rightNormal, _⟩ := XNet.normalize_spec rightResult
  rw [← parallel_sub_initial_net_equal] at rightSteps
  exact cnormal_form_unique leftOk.1 leftSteps rightSteps leftNormal rightNormal

/-- Both scan orders terminate within the declared four-interaction fuel. -/
theorem parallel_sub_schedules_normalize :
    (parallelSubNormalize false).isSome = true ∧
      (parallelSubNormalize true).isSome = true := by
  decide

/-- Kernel-evaluated compiled x-y traces, for `7 - 3` and the wrapping
`3 - 7` case. Both distinct schedules reach the same graph-reference
read-back in exactly four firings (`K = 2 * 2`). -/
theorem parallel_sub_schedules_agree :
    parallelSubScheduleResult false =
        some (4, [.i64 4, .i64 (3 - 7)], [.i64 4, .i64 (3 - 7)]) ∧
      parallelSubScheduleResult true =
        some (4, [.i64 4, .i64 (3 - 7)], [.i64 4, .i64 (3 - 7)]) := by
  decide

/-- A deliberately rejected FIFO control: two producers deliver the operands
to one subtraction port, and queue order is the delivery schedule. -/
inductive FifoControlState where
  | start
  | receivedSeven
  | receivedThree
  | queuedSevenThree
  | queuedThreeSeven
  | result (value : BitVec 64)
  deriving DecidableEq, Repr

inductive fifoControlStep : FifoControlState → FifoControlState → Prop
  | deliverSeven : fifoControlStep .start .receivedSeven
  | deliverThree : fifoControlStep .start .receivedThree
  | deliverThreeAfterSeven : fifoControlStep .receivedSeven .queuedSevenThree
  | deliverSevenAfterThree : fifoControlStep .receivedThree .queuedThreeSeven
  | consumeSevenThree :
      fifoControlStep .queuedSevenThree (.result ((7 : BitVec 64) - 3))
  | consumeThreeSeven :
      fifoControlStep .queuedThreeSeven (.result ((3 : BitVec 64) - 7))

/-- The FIFO variant's two producer-delivery steps have no one-step join, so
this explicit control fails the diamond property. It is not the selected system. -/
theorem fifo_control_fork_not_diamond :
    fifoControlStep .start .receivedSeven ∧
      fifoControlStep .start .receivedThree ∧
      ¬ ∃ join, fifoControlStep .receivedSeven join ∧
        fifoControlStep .receivedThree join := by
  refine ⟨.deliverSeven, .deliverThree, ?_⟩
  rintro ⟨join, hleft, hright⟩
  cases hleft
  cases hright

/-- Completing each FIFO schedule produces its respective ordered subtraction;
the results differ, unlike the selected inets schedule fixture. -/
theorem fifo_control_schedules_differ :
    fifoControlStep .start .receivedSeven ∧
      fifoControlStep .receivedSeven .queuedSevenThree ∧
      fifoControlStep .queuedSevenThree (.result ((7 : BitVec 64) - 3)) ∧
      fifoControlStep .start .receivedThree ∧
      fifoControlStep .receivedThree .queuedThreeSeven ∧
      fifoControlStep .queuedThreeSeven (.result ((3 : BitVec 64) - 7)) ∧
      ((7 : BitVec 64) - 3) ≠ ((3 : BitVec 64) - 7) := by
  refine ⟨.deliverSeven, .deliverThreeAfterSeven, .consumeSevenThree,
    .deliverThree, .deliverSevenAfterThree, .consumeThreeSeven, ?_⟩
  decide

/-- Exact executable properties included in a certificate's resource-function
table. The interaction potential and agent bound are graph-derived; wire count
is the exact two-orientation `XNet` representation size. -/
structure ResourceFunctions where
  interactionPotential : Nat
  liveAgentBound : Nat
  initialAgents : Nat
  wireCount : Nat
  inputSlots : Nat
  deriving DecidableEq, Repr

/-- The resource-function outputs for one admitted graph and input vector. -/
def Program.resourceFunctions (program : Program) (inputs : List (BitVec 64)) :
    ResourceFunctions :=
  { interactionPotential := program.interactionCount
    liveAgentBound := program.agentBound
    initialAgents := (program.translate inputs).agents.length
    wireCount := (program.translate inputs).wires.length
    inputSlots := inputs.length }

/-- The pinned semantic/lowering revisions bound by revision-3 certificates. -/
def i64RuleRevision : String := "ngraph-net-i64/3"
def i64LoweringRevision : String := "noble-net-i64-lowering/1"
def i64CertificateRevision : String := "ngraph-net-certificate/2"

/-- A certificate binds the source graph and input vector, both revisions, the
exact cost and live-agent claims, and the resource-function results. -/
structure Certificate where
  graph : Program
  inputs : List (BitVec 64)
  certificateRevision : String
  ruleRevision : String
  loweringRevision : String
  declaredInteractions : Nat
  declaredAgentBound : Nat
  resources : ResourceFunctions
  deriving DecidableEq, Repr

/-- Construct the claims derived from one exact graph and input vector. -/
def Certificate.forProgram (program : Program) (inputs : List (BitVec 64)) : Certificate :=
  { graph := program
    inputs := inputs
    certificateRevision := i64CertificateRevision
    ruleRevision := i64RuleRevision
    loweringRevision := i64LoweringRevision
    declaredInteractions := program.interactionCount
    declaredAgentBound := program.agentBound
    resources := program.resourceFunctions inputs }

/-- Declarative acceptance condition. The returned normalization must consume
exactly the formula cost and agree with the graph reference read-back. -/
def Certificate.Accepts (program : Program) (inputs : List (BitVec 64))
    (certificate : Certificate) : Prop :=
  certificate.graph = program ∧
  certificate.inputs = inputs ∧
  certificate.certificateRevision = i64CertificateRevision ∧
  certificate.ruleRevision = i64RuleRevision ∧
  certificate.loweringRevision = i64LoweringRevision ∧
  certificate.declaredInteractions = program.interactionCount ∧
  certificate.declaredAgentBound = program.agentBound ∧
  certificate.resources = program.resourceFunctions inputs ∧
  certificate.declaredAgentBound = certificate.resources.initialAgents ∧
  match program.normalize inputs with
  | none => False
  | some (normal, steps) =>
      steps = certificate.declaredInteractions ∧
        program.readBack normal = program.referenceOutputs inputs

/-- Executable, fail-closed certificate checker. -/
def checkCertificate (program : Program) (inputs : List (BitVec 64))
    (certificate : Certificate) : Bool :=
  decide (certificate.graph = program) &&
    decide (certificate.inputs = inputs) &&
    decide (certificate.certificateRevision = i64CertificateRevision) &&
    decide (certificate.ruleRevision = i64RuleRevision) &&
    decide (certificate.loweringRevision = i64LoweringRevision) &&
    decide (certificate.declaredInteractions = program.interactionCount) &&
    decide (certificate.declaredAgentBound = program.agentBound) &&
    decide (certificate.resources = program.resourceFunctions inputs) &&
    decide (certificate.declaredAgentBound = certificate.resources.initialAgents) &&
    match program.normalize inputs with
    | none => false
    | some (normal, steps) =>
        decide (steps = certificate.declaredInteractions) &&
          decide (program.readBack normal = program.referenceOutputs inputs)

/-- The checker is complete and sound for the declared certificate predicate. -/
private theorem Program.translateChecked_parts {program : Program} {inputs : List (BitVec 64)}
    {net : XNet NetSym} (h : program.translateChecked inputs = some net) :
    program.acceptanceCheck inputs = true ∧
      program.translate inputs = net ∧ net.check arity = true := by
  by_cases haccepted : program.acceptanceCheck inputs = true
  · by_cases hcheck : (program.translate inputs).check arity = true
    · have heq : program.translate inputs = net := by
        simpa [Program.translateChecked, haccepted, hcheck] using h
      exact ⟨haccepted, heq, by simpa [heq] using hcheck⟩
    · simp [Program.translateChecked, haccepted, hcheck] at h
  · simp [Program.translateChecked, haccepted] at h

theorem Program.translateChecked_accepted {program : Program} {inputs : List (BitVec 64)}
    {net : XNet NetSym} (h : program.translateChecked inputs = some net) :
    program.InputsOk inputs ∧ program.WellFormed ∧ program.WellTyped :=
  (Program.acceptanceCheck_iff program inputs).1 (Program.translateChecked_parts h).1

theorem checkCertificate_iff (program : Program) (inputs : List (BitVec 64))
    (certificate : Certificate) :
    checkCertificate program inputs certificate = true ↔
      Certificate.Accepts program inputs certificate := by
  cases hnormalize : program.normalize inputs with
  | none => simp [checkCertificate, Certificate.Accepts, hnormalize]
  | some result =>
      cases result with
      | mk normal steps =>
          simp [checkCertificate, Certificate.Accepts, hnormalize,
            Bool.and_eq_true, decide_eq_true_eq, and_assoc]

/-- A successful checked translation has the executable invariant required by
`XNet.normalize` and the sequential live-agent theorem. -/
theorem Program.translateChecked_ok {program : Program} {inputs : List (BitVec 64)}
    {net : XNet NetSym} (h : program.translateChecked inputs = some net) :
    net.Ok arity := by
  obtain ⟨_, heq, hc⟩ := Program.translateChecked_parts h
  subst net
  exact XNet.Checked.ok (XNet.check_iff.mp hc)

/-- Every completed normalization has the inets well-formedness, reduction,
normality, and schedule-independent exact-length guarantees. -/
theorem Program.normalize_spec {program : Program} {inputs : List (BitVec 64)}
    {normal : XNet NetSym} {steps : Nat}
    (h : program.normalize inputs = some (normal, steps)) :
    ∃ source, program.translateChecked inputs = some source ∧
      source.toNet.WF arity ∧
      Steps (CStep system) steps source.toNet normal.toNet ∧
      Normal (CStep system) normal.toNet ∧
      (∀ {m : Nat} {target : Net NetSym},
        Steps (CStep system) m source.toNet target →
        m ≤ steps ∧ (Normal (CStep system) target → target = normal.toNet ∧ m = steps)) := by
  unfold Program.normalize at h
  cases hsource : program.translateChecked inputs with
  | none => simp [hsource] at h
  | some source =>
      simp [hsource] at h
      obtain ⟨hSource, hNormalNet, hSteps, hNormal, hEvery⟩ := XNet.normalize_spec h
      exact ⟨source, rfl, hSource.1, hSteps, hNormal, hEvery⟩

/-- Inets strong confluence gives one normal form and one step count for every
reduction from a checked NetI64 source that reaches a normal form. -/
theorem i64_normal_form_unique {source : XNet NetSym} {n m : Nat}
    {normal₁ normal₂ : Net NetSym} (hsource : source.Ok arity)
    (first : Steps (CStep system) n source.toNet normal₁)
    (second : Steps (CStep system) m source.toNet normal₂)
    (firstNormal : Normal (CStep system) normal₁)
    (secondNormal : Normal (CStep system) normal₂) :
    normal₁ = normal₂ ∧ n = m :=
  cnormal_form_unique hsource.1 first second firstNormal secondNormal

/-- Once one schedule establishes the formula count `K`, every schedule from
the same checked translation to a normal form has exactly that count. This is
the NetI64 corollary of inets' schedule-independent normal-form theorem. -/
theorem i64_equal_interaction_count {program : Program} {inputs : List (BitVec 64)}
    {source : XNet NetSym} (translated : program.translateChecked inputs = some source)
    {normal target : Net NetSym} {steps : Nat}
    (canonical : Steps (CStep system) program.interactionCount source.toNet normal)
    (normalForm : Normal (CStep system) normal)
    (alternative : Steps (CStep system) steps source.toNet target)
    (alternativeNormal : Normal (CStep system) target) :
    target = normal ∧ steps = program.interactionCount :=
  cnormal_form_unique (Program.translateChecked_ok translated).1
    alternative canonical alternativeNormal normalForm

/-- An accepted certificate proves the formula interaction count for every
reduction of its exact translated net to a normal form. This uses the inets
normalizer specification, whose final clause is the unique-normal-form length
theorem, not the reducer's chosen schedule as a proxy. -/
theorem Certificate.accepted_exact_count
    {program : Program} {inputs : List (BitVec 64)} {certificate : Certificate}
    (accepted : checkCertificate program inputs certificate = true)
    {source : XNet NetSym} (hsource : program.translateChecked inputs = some source)
    {m : Nat} {target : Net NetSym}
    (hsteps : Steps (CStep system) m source.toNet target)
    (hnormal : Normal (CStep system) target) :
    m = program.interactionCount := by
  have haccept := (checkCertificate_iff program inputs certificate).1 accepted
  rcases haccept with ⟨_, _, _, _, _, hFormula, _, _, _, hRun⟩
  cases hnormalize : program.normalize inputs with
  | none => simp [hnormalize] at hRun
  | some result =>
      cases result with
      | mk normal steps =>
          simp [hnormalize] at hRun
          obtain ⟨source', hsource', hWf, hCanonical, hNormal, hEvery⟩ :=
            Program.normalize_spec hnormalize
          have hsourceEq : source' = source :=
            Option.some.inj (hsource'.symm.trans hsource)
          subst source'
          obtain ⟨_, hFinal⟩ := hEvery hsteps
          obtain ⟨_, hLength⟩ := hFinal hnormal
          exact hLength.trans (hRun.1.trans hFormula)

/-- Every prefix of the executable reduction used by an accepted certificate
stays within its declared live-agent bound. The checker binds that bound both
to the formula and to the exact checked translation. -/
theorem Certificate.accepted_run_live_agents
    {program : Program} {inputs : List (BitVec 64)} {certificate : Certificate}
    (accepted : checkCertificate program inputs certificate = true)
    {source : XNet NetSym} (hsource : program.translateChecked inputs = some source)
    (fuel : Nat) :
    (source.run xsys fuel).1.agents.length ≤ certificate.declaredAgentBound := by
  have haccept := (checkCertificate_iff program inputs certificate).1 accepted
  rcases haccept with ⟨_, _, _, _, _, _, _, hResources, hDeclaredActual, _⟩
  have hTranslation : program.translate inputs = source :=
    (Program.translateChecked_parts hsource).2.1
  have hBoundSource : certificate.declaredAgentBound = source.agents.length := by
    calc
      certificate.declaredAgentBound = (program.resourceFunctions inputs).initialAgents :=
        hDeclaredActual.trans (congrArg ResourceFunctions.initialAgents hResources)
      _ = (program.translate inputs).agents.length := rfl
      _ = source.agents.length := by rw [hTranslation]
  rw [hBoundSource]
  exact XNet.run_agents_le (Program.translateChecked_ok hsource) fuel

/-- Every finite prefix of every abstract reduction of an accepted net has a
finite representative within the certificate's live-agent bound. -/
theorem Certificate.accepted_reduction_live_agents
    {program : Program} {inputs : List (BitVec 64)} {certificate : Certificate}
    (accepted : checkCertificate program inputs certificate = true)
    {source : XNet NetSym} (hsource : program.translateChecked inputs = some source)
    {steps : Nat} {target : Net NetSym}
    (hsteps : Steps (CStep system) steps source.toNet target) :
    ∃ reached : XNet NetSym, reached.toNet = target ∧
      reached.agents.length ≤ certificate.declaredAgentBound := by
  obtain ⟨reached, htarget, _, hbound⟩ :=
    XNet.steps_agents_le (Program.translateChecked_ok hsource) rfl hsteps
  have haccept := (checkCertificate_iff program inputs certificate).1 accepted
  rcases haccept with ⟨_, _, _, _, _, _, _, hResources, hDeclaredActual, _⟩
  have hBoundSource : certificate.declaredAgentBound = source.agents.length := by
    calc
      certificate.declaredAgentBound = (program.resourceFunctions inputs).initialAgents :=
        hDeclaredActual.trans (congrArg ResourceFunctions.initialAgents hResources)
      _ = (program.translate inputs).agents.length := rfl
      _ = source.agents.length := by
        rw [(Program.translateChecked_parts hsource).2.1]
  exact ⟨reached, htarget, Nat.le_trans hbound (Nat.le_of_eq hBoundSource.symm)⟩

/-- Concrete two-input arithmetic fixture; the checker accepts only if its
kernel-evaluated normal form reads back the reference result. -/
def binaryAddCertificate (left right : BitVec 64) : Certificate :=
  let program := binaryProgram .add
  let inputs := [left, right]
  Certificate.forProgram program inputs

/-- Certificate fixture for arbitrary-size output-only fanout. -/
def fanoutCertificate (outputs : Nat) (value : BitVec 64) : Certificate :=
  Certificate.forProgram (fanoutProgram outputs) [value]

/-- A concrete accepted certificate demonstrates that the checker is not
refusal-only. -/
theorem binaryAddCertificate_accepted :
    checkCertificate (binaryProgram .add) [1, 2]
      (binaryAddCertificate 1 2) = true := by
  decide

/-- Certificate checking has only a successful result or an explicit
`uncertified` result; refusal does not claim divergence. -/
inductive CertificateOutcome where
  | certified
  | uncertified
  deriving DecidableEq, Repr

def certificateOutcome (program : Program) (inputs : List (BitVec 64))
    (certificate : Certificate) : CertificateOutcome :=
  if checkCertificate program inputs certificate then .certified else .uncertified

theorem certificateOutcome_iff (program : Program) (inputs : List (BitVec 64))
    (certificate : Certificate) :
    certificateOutcome program inputs certificate = .certified ↔
      checkCertificate program inputs certificate = true := by
  simp [certificateOutcome]

/-- Mutation controls: each identity, revision, cost, bound, and resource-field
change fails closed before a normal form can be published. -/
theorem changed_graph_refused :
    checkCertificate (fanoutProgram 1) [1, 2] (binaryAddCertificate 1 2) = false := by
  decide

theorem changed_inputs_refused :
    checkCertificate (binaryProgram .add) [1, 3] (binaryAddCertificate 1 2) = false := by
  decide

theorem changed_rule_revision_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with ruleRevision := "ngraph-net-i64/2" } = false := by
  decide

theorem changed_certificate_revision_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with certificateRevision := "ngraph-net-certificate/1" } = false := by
  decide

theorem changed_lowering_revision_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with loweringRevision := "other-lowering" } = false := by
  decide

theorem insufficient_interaction_count_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with declaredInteractions := 1 } = false := by
  decide

theorem insufficient_interaction_potential_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with
        resources := { (binaryAddCertificate 1 2).resources with interactionPotential := 1 } } = false := by
  decide

theorem insufficient_agent_bound_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with declaredAgentBound := 2 } = false := by
  decide

theorem changed_resource_function_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with
        resources := { (binaryAddCertificate 1 2).resources with inputSlots := 3 } } = false := by
  decide

theorem changed_wire_count_refused :
    checkCertificate (binaryProgram .add) [1, 2]
      { binaryAddCertificate 1 2 with
        resources := { (binaryAddCertificate 1 2).resources with wireCount := 0 } } = false := by
  decide

/-- The certificate cannot omit a DUP's unit cost. -/
theorem omitted_dup_cost_refused :
    checkCertificate (fanoutProgram 3) [7]
      { fanoutCertificate 3 7 with declaredInteractions := 1 } = false := by
  decide

/-- The certificate cannot omit the ERA cost of an unused input. -/
theorem omitted_era_cost_refused :
    checkCertificate (fanoutProgram 0) [7]
      { fanoutCertificate 0 7 with declaredInteractions := 0 } = false := by
  decide

/-- The checked translation refuses a forward edge rather than assigning it a value. -/
theorem forward_reference_refused :
    forwardReferenceProgram.translateChecked [7] = none := by
  decide

/-- The checked translation refuses an I64/Bool type mismatch. -/
theorem ill_typed_graph_refused :
    illTypedProgram.translateChecked [7] = none := by
  decide

/-- The checked translation refuses an out-of-range input slot. -/
theorem invalid_input_slot_refused :
    invalidInputSlotProgram.translateChecked [7] = none := by
  decide

/-- Exact input-vector arity is an admission premise. -/
theorem short_input_vector_refused :
    (binaryProgram .add).translateChecked [7] = none := by
  decide

/-- A certificate whose lowering violates a structural assumption cannot be
accepted, even when its identity/resource fields are self-consistent. -/
theorem false_structural_assumption_refused :
    checkCertificate illTypedProgram [7]
      (Certificate.forProgram illTypedProgram [7]) = false := by
  decide

/-- The raw translation preserves its output-list interface exactly. -/
theorem Program.translate_interface (program : Program) (inputs : List (BitVec 64)) :
    (program.translate inputs).free = (List.range program.outputs.length).map outputPort := rfl

/-- Any accepted translation is a well-formed inets net. -/
theorem Program.translateChecked_wf {program : Program} {inputs : List (BitVec 64)}
    {net : XNet NetSym} (h : program.translateChecked inputs = some net) :
    net.toNet.WF arity := by
  exact (Program.translateChecked_ok h).1

/-- A checked lowering retains precisely the declared ordered output interface. -/
theorem Program.translateChecked_interface {program : Program} {inputs : List (BitVec 64)}
    {net : XNet NetSym} (h : program.translateChecked inputs = some net) :
    net.free = (List.range program.outputs.length).map outputPort := by
  rw [← (Program.translateChecked_parts h).2.1]
  exact Program.translate_interface program inputs

end NobleFoundation.NetI64
