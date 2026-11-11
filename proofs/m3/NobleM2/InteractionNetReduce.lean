import NobleM2.InteractionNetV2

/-!
# The arithmetic rule of revision 2 (issue #37)

The reducer's core, isolated: a binary agent holds one LIFO stack per operand port and fires when
both are non-empty, popping one value from each and pushing the result onto its result wire.

The contract's decisions are visible in the type: operand order is the **port**, never the source
(`first - second` below); stacks are **LIFO**; and values are `BitVec 64`, so wrapping is the
operation rather than a mode.

Proved here: when the rule fires, when it does not, that `Sub` subtracts in port order, and that each
firing consumes exactly two operands. Not proved and not modelled: the worklist, wire transport,
fanout, erasure, boundary read-back, or any budget.
-/

namespace NobleM2.InteractionNet

/-- 64-bit values, wrapping by construction. -/
abbrev Value := BitVec 64

/-- One binary agent's operand stacks: head is the next value consumed. -/
structure Operands where
  /-- Values waiting on port 1. -/
  first : List Value
  /-- Values waiting on port 2. -/
  second : List Value

/-- The arithmetic rule for one agent kind, firing only when both operands are available. -/
def fire (kind : AgentKind) : Operands → Option (Operands × Value)
  | ⟨a :: as, b :: bs⟩ =>
      match kind with
      | .add => some (⟨as, bs⟩, a + b)
      | .sub => some (⟨as, bs⟩, a - b)
      | .mul => some (⟨as, bs⟩, a * b)
      | .eq => some (⟨as, bs⟩, if a == b then 1 else 0)
      | _ => none
  | _ => none

/-- For an arithmetic agent the rule fires exactly when both operand stacks are non-empty. The
restriction is necessary: a value, eraser or boundary agent never fires however full its stacks, so
the unrestricted statement is false. -/
theorem fire_arithmetic_iff (kind : AgentKind) (hk : kind.isArithmetic = true)
    (ops : Operands) :
    (fire kind ops).isSome ↔ (ops.first ≠ [] ∧ ops.second ≠ []) := by
  cases ops with
  | mk first second =>
      cases first with
      | nil => simp [fire]
      | cons a as =>
          cases second with
          | nil => simp [fire]
          | cons b bs =>
              cases kind <;> simp_all [fire, AgentKind.isArithmetic]

/-- `Sub` subtracts in port order: the first operand is the minuend. -/
theorem fire_sub_is_port_order (a b : Value) (as bs : List Value) :
    fire .sub ⟨a :: as, b :: bs⟩ = some (⟨as, bs⟩, a - b) := rfl

/-- `Add` is exactly wrapping addition. -/
theorem fire_add_is_wrapping (a b : Value) (as bs : List Value) :
    fire .add ⟨a :: as, b :: bs⟩ = some (⟨as, bs⟩, a + b) := rfl

/-- `Eq` yields `1` or `0` in the single value domain. -/
theorem fire_eq_is_zero_or_one (a b : Value) (as bs : List Value) :
    fire .eq ⟨a :: as, b :: bs⟩ = some (⟨as, bs⟩, if a == b then 1 else 0) := rfl

/-- A value agent, eraser or boundary agent never fires, however full its stacks. -/
theorem fire_non_arithmetic (kind : AgentKind) (h : kind.isArithmetic = false)
    (ops : Operands) : fire kind ops = none := by
  obtain ⟨first, second⟩ := ops
  cases first with
  | nil => rfl
  | cons a as =>
      cases second with
      | nil => rfl
      | cons b bs =>
          cases kind <;> simp_all [AgentKind.isArithmetic, fire]

/-- **Stack effect**: a firing pops exactly the two heads and leaves the tails, whatever the
operator. Stated as the shape of the result, which is what a worklist update needs. -/
theorem fire_consumes_heads {kind : AgentKind} (hk : kind.isArithmetic = true)
    (a b : Value) (as bs : List Value) :
    ∃ v : Value, fire kind ⟨a :: as, b :: bs⟩ = some (⟨as, bs⟩, v) := by
  cases kind <;> simp_all [AgentKind.isArithmetic, fire]

/-- A fanout firing copies one value to its two auxiliary ports. -/
def fireFanout (v : Value) : List Value := [v, v]

/-- Both copies are the original value: fanout duplicates, it does not transform. -/
theorem fireFanout_copies (v : Value) : ∀ x ∈ fireFanout v, x = v := by
  intro x hx
  simpa [fireFanout] using hx

/-- An erase firing consumes one value and produces none. -/
def fireErase (_v : Value) : List Value := []

theorem fireErase_empty (v : Value) : fireErase v = [] := rfl

/-- Net change in the number of live values caused by one firing of each agent kind. Arithmetic
consumes two and produces one; fanout consumes one and produces two; erase consumes one and produces
none; a value or boundary agent is not a firing. -/
def valueDelta : AgentKind → Int
  | .add => -1
  | .sub => -1
  | .mul => -1
  | .eq => -1
  | .fanout => 1
  | .erase => -1
  | .value => 0
  | .input => 0
  | .output => 0

/-- Every arithmetic firing reduces the value count by one. -/
theorem valueDelta_arithmetic (kind : AgentKind) (hk : kind.isArithmetic = true) :
    valueDelta kind = -1 := by
  cases kind <;> simp_all [AgentKind.isArithmetic, valueDelta]

/-- Fanout is the only rule that increases the value count. -/
theorem valueDelta_fanout : valueDelta .fanout = 1 := rfl

/-- Erase reduces the value count by one. -/
theorem valueDelta_erase : valueDelta .erase = -1 := rfl

/-- A fanout is the only rule with a positive delta, and an arithmetic agent is never it. -/
theorem arithmetic_delta_not_positive (kind : AgentKind) (hk : kind.isArithmetic = true) :
    valueDelta kind ≤ 0 := by
  rw [valueDelta_arithmetic kind hk]
  decide

/-- A node is fireable when both operand stacks are non-empty. -/
def fireable (ops : Operands) : Bool :=
  !ops.first.isEmpty && !ops.second.isEmpty

/-- The worklist scan: is any node fireable? -/
def anyFireable (nodes : List Operands) : Bool :=
  nodes.any fireable

/-- The scan's decision matches the stacks. -/
theorem fireable_iff (ops : Operands) :
    fireable ops = true ↔ (ops.first ≠ [] ∧ ops.second ≠ []) := by
  obtain ⟨first, second⟩ := ops
  cases first <;> cases second <;> simp [fireable]

/-- The scan is true exactly when some node is fireable. -/
theorem anyFireable_iff (nodes : List Operands) :
    anyFireable nodes = true ↔ ∃ ops ∈ nodes, fireable ops = true := by
  unfold anyFireable
  exact List.any_eq_true

/-- **Progress**: if a fireable node exists, the scan finds one, so the reducer never stalls while
work remains. -/
theorem anyFireable_of_mem {nodes : List Operands} {ops : Operands}
    (hmem : ops ∈ nodes) (h : fireable ops = true) : anyFireable nodes = true := by
  unfold anyFireable
  induction nodes with
  | nil => simp at hmem
  | cons head rest ih =>
      rw [List.any_cons]
      rcases List.mem_cons.mp hmem with rfl | hrest
      · rw [h]
        simp
      · rw [ih hrest]
        simp

/-- A scan over an empty node list finds nothing to do. -/
theorem anyFireable_nil : anyFireable [] = false := rfl

/-- Stacking a fireable node first makes the scan true immediately. -/
theorem anyFireable_cons_true {ops : Operands} {nodes : List Operands}
    (h : fireable ops = true) : anyFireable (ops :: nodes) = true := by
  rw [anyFireable, List.any_cons, h]
  simp

/-- Read-back of boundary outputs: the complete vector in order, or a refusal. -/
def readBack (expected : Nat) (values : List Value) : Option (List Value) :=
  if values.length = expected then some values else none

/-- Read-back succeeds exactly when the count matches the declared boundary. -/
theorem readBack_complete_iff (expected : Nat) (values : List Value) :
    (readBack expected values).isSome ↔ values.length = expected := by
  unfold readBack
  split <;> simp_all

/-- On success the output is the observed values themselves: read-back never reorders. -/
theorem readBack_order {expected : Nat} {values out : List Value}
    (h : readBack expected values = some out) : out = values := by
  unfold readBack at h
  split at h
  · injection h with hout
    exact hout.symm
  · simp at h

/-- **No partial success**: an incomplete read-back is a refusal, not a prefix. -/
theorem readBack_refuses_incomplete {expected : Nat} {values : List Value}
    (h : values.length < expected) : readBack expected values = none := by
  unfold readBack
  split
  · omega
  · rfl

/-- Read-back never invents or drops a value. -/
theorem readBack_length {expected : Nat} {values out : List Value}
    (h : readBack expected values = some out) : out.length = expected := by
  have hlen := (readBack_complete_iff expected values).mp (h ▸ Option.isSome_some)
  simpa [readBack_order h] using hlen

/-- Reducer state: the operand stacks of every arithmetic node and the outputs observed so far. -/
structure Machine where
  /-- Operand stacks, one entry per arithmetic node. -/
  nodes : List Operands
  /-- Outputs observed in boundary order. -/
  outputs : List Value

/-- Deliver a fired value to a boundary output: the value is appended, so the observation order is
the delivery order. -/
def Machine.deliverOutput (m : Machine) (v : Value) : Machine :=
  { m with outputs := m.outputs ++ [v] }

theorem deliverOutput_outputs (m : Machine) (v : Value) :
    (m.deliverOutput v).outputs = m.outputs ++ [v] := rfl

/-- Delivery to an output adds exactly one observation. -/
theorem deliverOutput_length (m : Machine) (v : Value) :
    (m.deliverOutput v).outputs.length = m.outputs.length + 1 := by
  rw [deliverOutput_outputs, List.length_append]
  rfl

/-- Delivery preserves the observations already made and their order. -/
theorem deliverOutput_prefix (m : Machine) (v : Value) :
    ∀ x ∈ m.outputs, x ∈ (m.deliverOutput v).outputs := by
  intro x hx
  rw [deliverOutput_outputs]
  exact List.mem_append.mpr (Or.inl hx)

/-- Delivery does not disturb the operand stacks. -/
theorem deliverOutput_nodes (m : Machine) (v : Value) :
    (m.deliverOutput v).nodes = m.nodes := rfl

/-- Observing a value cannot reduce the set of fireable nodes. -/
theorem deliverOutput_fireable (m : Machine) (v : Value) :
    anyFireable (m.deliverOutput v).nodes = anyFireable m.nodes := by
  rw [deliverOutput_nodes]


/-- Push a delivered value onto one operand stack of a node, LIFO. -/
def Operands.push (ops : Operands) (secondPort : Bool) (v : Value) : Operands :=
  match secondPort with
  | true => { ops with second := v :: ops.second }
  | false => { ops with first := v :: ops.first }

theorem push_first_head (ops : Operands) (v : Value) :
    (ops.push false v).first = v :: ops.first := rfl

theorem push_second_head (ops : Operands) (v : Value) :
    (ops.push true v).second = v :: ops.second := rfl

/-- Deliver to the node at an index; `none` when the index is out of range. -/
def pushNode : List Operands → Nat → Bool → Value → Option (List Operands)
  | [], _, _, _ => none
  | ops :: rest, 0, secondPort, v => some (ops.push secondPort v :: rest)
  | ops :: rest, n + 1, secondPort, v =>
      (pushNode rest n secondPort v).map (fun updated => ops :: updated)

/-- A successful delivery preserves the node count. -/
theorem pushNode_length : ∀ (nodes : List Operands) (index : Nat) (secondPort : Bool)
    (v : Value) (updated : List Operands),
    pushNode nodes index secondPort v = some updated → updated.length = nodes.length
  | [], _, _, _, updated, h => by simp [pushNode] at h
  | ops :: rest, 0, _, _, updated, h => by
      simp only [pushNode] at h
      injection h with hupd
      subst hupd
      simp
  | ops :: rest, n + 1, secondPort, v, updated, h => by
      simp only [pushNode] at h
      cases hrest : pushNode rest n secondPort v with
      | none => rw [hrest] at h; simp at h
      | some t =>
          rw [hrest] at h
          injection h with hupd
          subst hupd
          simp [pushNode_length rest n secondPort v t hrest]

/-- Unwinding accounting for one reduction: rewrites performed and bytes retained, against declared
caps. Every charge is non-wrapping: a charge that would exceed its cap is refused, never truncated. -/
structure Budget where
  /-- Rewrites performed so far. -/
  rewrites : Nat
  /-- Bytes retained so far. -/
  bytes : Nat
  /-- Declared rewrite cap. -/
  maxRewrites : Nat
  /-- Declared retained-byte cap. -/
  maxBytes : Nat

/-- Charge one rewrite, refusing at the cap. -/
def Budget.rewrite (b : Budget) : Option Budget :=
  if b.rewrites < b.maxRewrites then some { b with rewrites := b.rewrites + 1 } else none

/-- Charge retained bytes, refusing when the charge would cross the cap. Charges before growth. -/
def Budget.retain (b : Budget) (n : Nat) : Option Budget :=
  if b.bytes + n ≤ b.maxBytes then some { b with bytes := b.bytes + n } else none

/-- A rewrite charge succeeds exactly below the cap. -/
theorem rewrite_succeeds_iff (b : Budget) :
    (b.rewrite).isSome ↔ b.rewrites < b.maxRewrites := by
  unfold Budget.rewrite
  split <;> simp_all

/-- Rewrites never exceed their cap. -/
theorem rewrite_bounded {b b' : Budget} (h : b.rewrite = some b') :
    b'.rewrites ≤ b'.maxRewrites := by
  unfold Budget.rewrite at h
  split at h
  · injection h with hb
    subst hb
    simp
    omega
  · simp at h

/-- A byte charge succeeds exactly when it fits, and then fits afterwards. -/
theorem retain_succeeds_iff (b : Budget) (n : Nat) :
    (b.retain n).isSome ↔ b.bytes + n ≤ b.maxBytes := by
  unfold Budget.retain
  split <;> simp_all

/-- Retained bytes never exceed their cap. -/
theorem retain_bounded {b b' : Budget} {n : Nat} (h : b.retain n = some b') :
    b'.bytes ≤ b'.maxBytes := by
  unfold Budget.retain at h
  split at h
  · injection h with hb
    subst hb
    simpa using ‹b.bytes + n ≤ b.maxBytes›
  · simp at h

/-- A charge over the cap is a refusal, not a truncated charge. -/
theorem retain_refuses_over_cap {b : Budget} {n : Nat} (h : b.maxBytes < b.bytes + n) :
    b.retain n = none := by
  unfold Budget.retain
  split
  · omega
  · rfl


/-- Step one node: fire it if arithmetic and both stacks are ready. -/
def stepNode : AgentKind × Operands → Option (AgentKind × Operands)
  | (kind, ops) =>
      match fire kind ops with
      | some (ops', _v) => some (kind, ops')
      | none => none

/-- A node is reducible exactly when stepping it does something. -/
def reducible (node : AgentKind × Operands) : Bool := (stepNode node).isSome

/-- Reducibility is arithmetic plus ready stacks, by the firing condition. -/
theorem reducible_iff (kind : AgentKind) (hk : kind.isArithmetic = true) (ops : Operands) :
    reducible (kind, ops) = true ↔ (ops.first ≠ [] ∧ ops.second ≠ []) := by
  unfold reducible stepNode
  cases hfire : fire kind ops with
  | none =>
      constructor
      · intro h
        simp [hfire] at h
      · intro hf
        have hsome := (fire_arithmetic_iff kind hk ops).mpr hf
        rw [hfire] at hsome
        simp at hsome
  | some p =>
      constructor
      · intro _
        have hsome : (fire kind ops).isSome := by rw [hfire]; rfl
        exact (fire_arithmetic_iff kind hk ops).mp hsome
      · intro _
        show (match fire kind ops with
          | some (ops', _v) => some (kind, ops')
          | none => none).isSome = true
        rw [hfire]
        rfl

/-- Step the first reducible node; `none` when none is reducible. -/
def stepList : List (AgentKind × Operands) → Option (List (AgentKind × Operands))
  | [] => none
  | node :: rest =>
      match stepNode node with
      | some node' => some (node' :: rest)
      | none => (stepList rest).map (fun rest' => node :: rest')

/-- A step preserves the node count. -/
theorem stepList_length : ∀ (nodes nodes' : List (AgentKind × Operands)),
    stepList nodes = some nodes' → nodes'.length = nodes.length := by
  intro nodes
  induction nodes with
  | nil => intro nodes' h; simp [stepList] at h
  | cons node rest ih =>
      intro nodes' h
      unfold stepList at h
      cases hstep : stepNode node with
      | some node' =>
          rw [hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          simp
      | none =>
          rw [hstep] at h
          cases hrest : stepList rest with
          | none => rw [hrest] at h; simp at h
          | some t =>
              rw [hrest] at h
              have ht : t.length = rest.length := ih t hrest
              simp only [Option.map_some, Option.some.injEq] at h
              subst h
              simp [ht]

/-- A step refuses exactly when no node is reducible. -/
theorem stepList_none_iff : ∀ (nodes : List (AgentKind × Operands)),
    stepList nodes = none ↔ ∀ node ∈ nodes, reducible node = false := by
  intro nodes
  induction nodes with
  | nil => simp [stepList]
  | cons node rest ih =>
      cases hstep : stepNode node with
      | some node' =>
          have hval : stepList (node :: rest) = some (node' :: rest) := by
            simp [stepList, hstep]
          have hred : reducible node = true := by simp [reducible, hstep]
          rw [hval]
          simp only [reduceCtorEq, false_iff]
          intro hall
          have hfalse := hall node (by simp)
          rw [hred] at hfalse
          simp at hfalse
      | none =>
          have hval : stepList (node :: rest) = (stepList rest).map (fun rest' => node :: rest') := by
            simp [stepList, hstep]
          have hnode : reducible node = false := by simp [reducible, hstep]
          rw [hval]
          have hmap : ((stepList rest).map (fun rest' => node :: rest') = none)
              ↔ stepList rest = none := by
            cases stepList rest <;> simp
          rw [hmap, ih]
          constructor
          · intro hall n hn
            rcases List.mem_cons.mp hn with rfl | hmem
            · exact hnode
            · exact hall n hmem
          · intro hall n hmem
            exact hall n (by simp [hmem])

/-- Run the reducer for at most `fuel` steps. `some nodes` is a stuck (complete) net; `none` means
the fuel ran out with work remaining. -/
def run : Nat → List (AgentKind × Operands) → Option (List (AgentKind × Operands))
  | 0, _ => none
  | fuel + 1, nodes =>
      match stepList nodes with
      | some nodes' => run fuel nodes'
      | none => some nodes

/-- With no fuel, nothing happens. -/
theorem run_zero (nodes : List (AgentKind × Operands)) : run 0 nodes = none := rfl

/-- A stuck net is a completed run, whatever fuel remains. -/
theorem run_stuck {nodes : List (AgentKind × Operands)} {fuel : Nat}
    (h : stepList nodes = none) : run (fuel + 1) nodes = some nodes := by
  unfold run
  rw [h]

/-- A completed run preserves the node count: steps never add or drop agents. -/
theorem run_length : ∀ (fuel : Nat) (nodes nodes' : List (AgentKind × Operands)),
    run fuel nodes = some nodes' → nodes'.length = nodes.length := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes nodes' h
      simp [run_zero] at h
  | succ fuel ih =>
      intro nodes nodes' h
      unfold run at h
      cases hstep : stepList nodes with
      | none =>
          rw [hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          simp
      | some next =>
          rw [hstep] at h
          have hnext : next.length = nodes.length := stepList_length nodes next hstep
          rw [ih next nodes' h]
          exact hnext

/-- A run that returns `none` had work remaining at the last step: every unit of fuel was spent
stepping. -/
theorem run_none_step {nodes : List (AgentKind × Operands)} {fuel : Nat}
    (h : run (fuel + 1) nodes = none) :
    ∃ nodes', stepList nodes = some nodes' ∧ run fuel nodes' = none := by
  unfold run at h
  cases hstep : stepList nodes with
  | none => rw [hstep] at h; simp at h
  | some nodes' =>
      rw [hstep] at h
      exact ⟨nodes', rfl, h⟩

/-- A successful run ends on a stuck net: the reducer stops only when no node is reducible. -/
theorem run_some_stuck : ∀ (fuel : Nat) {nodes : List (AgentKind × Operands)}
    {nodes' : List (AgentKind × Operands)},
    run fuel nodes = some nodes' → stepList nodes' = none := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes nodes' h
      simp [run_zero] at h
  | succ fuel ih =>
      intro nodes nodes' h
      unfold run at h
      cases hstep : stepList nodes with
      | none =>
          rw [hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          exact hstep
      | some next =>
          rw [hstep] at h
          exact ih h

/-- A successful run is complete: every node is irreducible at the end. -/
theorem run_some_all_irreducible {fuel : Nat} {nodes nodes' : List (AgentKind × Operands)}
    (h : run fuel nodes = some nodes') : ∀ node ∈ nodes', reducible node = false :=
  (stepList_none_iff nodes').mp (run_some_stuck fuel h)

/-- A run that also reports how many interaction steps it took. -/
def runCount : Nat → List (AgentKind × Operands) →
    Option (Nat × List (AgentKind × Operands))
  | 0, _ => none
  | fuel + 1, nodes =>
      match stepList nodes with
      | some nodes' =>
          match runCount fuel nodes' with
          | some (k, final) => some (k + 1, final)
          | none => none
      | none => some (0, nodes)

/-- The counted run's step count never exceeds the fuel. -/
theorem runCount_steps_le : ∀ (fuel : Nat) {nodes : List (AgentKind × Operands)}
    {k : Nat} {final : List (AgentKind × Operands)},
    runCount fuel nodes = some (k, final) → k ≤ fuel := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes k final h
      simp [runCount] at h
  | succ fuel ih =>
      intro nodes k final h
      cases hstep : stepList nodes with
      | none =>
          simp [runCount, hstep] at h
          obtain ⟨hk, _⟩ := h
          subst hk
          exact Nat.zero_le _
      | some next =>
          cases hrc : runCount fuel next with
          | none => simp [runCount, hstep, hrc] at h
          | some p =>
              obtain ⟨k', final'⟩ := p
              simp [runCount, hstep, hrc] at h
              obtain ⟨hk, _⟩ := h
              subst hk
              exact Nat.succ_le_succ (ih hrc)

/-- The counted run computes the same final net as `run`. -/
theorem runCount_snd_run : ∀ (fuel : Nat) {nodes : List (AgentKind × Operands)}
    {k : Nat} {final : List (AgentKind × Operands)},
    runCount fuel nodes = some (k, final) → run fuel nodes = some final := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes k final h
      simp [runCount] at h
  | succ fuel ih =>
      intro nodes k final h
      cases hstep : stepList nodes with
      | none =>
          simp [runCount, hstep] at h
          obtain ⟨_, hnodes⟩ := h
          subst hnodes
          exact run_stuck hstep
      | some next =>
          cases hrc : runCount fuel next with
          | none => simp [runCount, hstep, hrc] at h
          | some p =>
              obtain ⟨k', final'⟩ := p
              simp [runCount, hstep, hrc] at h
              obtain ⟨_, hfinal⟩ := h
              subst hfinal
              have := ih hrc
              unfold run
              rw [hstep]
              exact this

/-- The counted run preserves the node population: the reported interaction count bounds the work
done, not the number of live nodes. -/
theorem runCount_length {fuel : Nat} {nodes : List (AgentKind × Operands)} {k : Nat}
    {final : List (AgentKind × Operands)}
    (h : runCount fuel nodes = some (k, final)) : final.length = nodes.length :=
  run_length fuel nodes final (runCount_snd_run fuel h)

/-- A single fully-fed add agent fires once and is then stuck: one interaction, and the agent
survives with empty stacks. -/
theorem runCount_single_add (a b : BitVec 64) :
    runCount 2 [(.add, ⟨[a], [b]⟩)] = some (1, [(.add, ⟨[], []⟩)]) := rfl

/-- A net whose head is a value agent does not fire; the run completes in zero interactions. -/
theorem runCount_value_head_stuck : runCount 3 [(.value, ⟨[7], []⟩)] = some (0, [(.value, ⟨[7], []⟩)]) := rfl

/-- Number of reducible nodes in a net. -/
def reducibleCount : List (AgentKind × Operands) → Nat
  | [] => 0
  | node :: rest => (if reducible node then 1 else 0) + reducibleCount rest

/-- A node that steps is reducible: reducibility is the step being possible. -/
theorem reducible_of_stepNode (node node' : AgentKind × Operands)
    (h : stepNode node = some node') : reducible node = true := by
  simp [reducible, h]

/-- A step never increases the number of reducible nodes: the fired node may stop being reducible,
and every other node is carried through unchanged. -/
theorem stepList_reducibleCount_le : ∀ (nodes nodes' : List (AgentKind × Operands)),
    stepList nodes = some nodes' → reducibleCount nodes' ≤ reducibleCount nodes := by
  intro nodes
  induction nodes with
  | nil =>
      intro nodes' h
      simp [stepList] at h
  | cons node rest ih =>
      intro nodes' h
      obtain ⟨kind, ops⟩ := node
      cases hstep : stepNode (kind, ops) with
      | some node' =>
          have hred : reducible (kind, ops) = true := reducible_of_stepNode _ _ hstep
          have hval : stepList ((kind, ops) :: rest) = some (node' :: rest) := by
            simp [stepList, hstep]
          rw [hval] at h
          simp only [Option.some.injEq] at h
          subst h
          unfold reducibleCount
          rw [hred]
          have hle : (if reducible node' then 1 else 0) ≤ 1 := by
            split <;> simp
          exact Nat.add_le_add_right hle _
      | none =>
          have hval : stepList ((kind, ops) :: rest) =
              (stepList rest).map (fun r => (kind, ops) :: r) := by
            simp [stepList, hstep]
          rw [hval] at h
          have hmap : ((stepList rest).map (fun r => (kind, ops) :: r) = some nodes')
              ↔ ∃ t, stepList rest = some t ∧ nodes' = (kind, ops) :: t := by
            cases stepList rest with
            | none => simp
            | some t => simp [eq_comm]
          obtain ⟨t, hrest, rfl⟩ := hmap.mp h
          unfold reducibleCount
          exact Nat.add_le_add_left (ih t hrest) _

end NobleM2.InteractionNet
