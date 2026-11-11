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

end NobleM2.InteractionNet
