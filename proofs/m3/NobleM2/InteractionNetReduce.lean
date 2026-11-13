import NobleM2.InteractionNetV2

/-!
# The arithmetic rule of revision 2 (issue #37)

This is the superseded revision-2 local stack model, retained only as a recorded comparison. The
selected revision-3 I64 interaction system is `NobleFoundation.NetI64.system` over inets; none of the
reducer's node-count, fuel, potential, or certificate bounds are bounds on that real interaction net.

The reducer's core, isolated: a binary agent holds one LIFO stack per operand port and fires when
both are non-empty. A firing pops one value from each operand port and **discards** the value the
rule computes.

The contract's decisions are visible in the type: operand order is the **port**, never the source
(`first - second` below); stacks are **LIFO**; and values are `BitVec 64`, so wrapping is the
operation rather than a mode.

Proved here: when a rule fires and when it does not; that `Sub` subtracts in port order; that a
firing consumes exactly two operands; the fanout and erase value effects; the shape of boundary
read-back; non-wrapping rewrite and retained-byte charges; the stack reducer `stepList`/`run` and its
node-count, fuel and potential bounds; and a certificate checker over a net's own node list.

Not modelled: **delivery**. No agent's result reaches another agent's operand stack, so there is no
whole-net reduction here — `pushNode` and `deliverOutput` are the pieces a delivery-capable reducer
needs and have no call site. Fanout and erase are value-effect formulas only: `stepNode` never fires
them. `run`'s `some` means "no node is reducible", not "the net has produced its outputs"
(`run_some_all_irreducible`), and the "progress" theorem below is a fact about the operand scan, not
a statement that `stepList` keeps going (`anyFireable_disagrees_with_stepList`).
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
arithmetic operator. Stated as the shape of the result, which is what a delivery update needs. -/
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

/-- The change in the number of live values that one firing of each rule causes in a net that
delivers the rule's result onto a wire: arithmetic consumes two and emits one, fanout consumes one
and emits two, erase consumes one and emits none, and a value or boundary agent is not a firing.
This is a per-rule quantity, not the reducer's state change: `stepNode` does not deliver anything,
so the waiting-value count drops by exactly the two consumed operands (`fire_ops_lengths`). -/
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

/-- Every arithmetic rule's value delta is minus one. -/
theorem valueDelta_arithmetic (kind : AgentKind) (hk : kind.isArithmetic = true) :
    valueDelta kind = -1 := by
  cases kind <;> simp_all [AgentKind.isArithmetic, valueDelta]

/-- Fanout's delta is plus one: it is the rule that increases the value count. -/
theorem valueDelta_fanout : valueDelta .fanout = 1 := rfl

/-- Erase's delta is minus one. -/
theorem valueDelta_erase : valueDelta .erase = -1 := rfl

/-- A fanout is the only rule with a positive delta, and an arithmetic agent is never it. -/
theorem arithmetic_delta_not_positive (kind : AgentKind) (hk : kind.isArithmetic = true) :
    valueDelta kind ≤ 0 := by
  rw [valueDelta_arithmetic kind hk]
  decide

/-- An operand pair is fireable when both of its stacks are non-empty. The predicate carries no
agent kind, so it is the firing condition of the arithmetic rules only. -/
def fireable (ops : Operands) : Bool :=
  !ops.first.isEmpty && !ops.second.isEmpty

/-- The operand scan: is any entry's pair of operand stacks fireable? This is not the reducer's
worklist: `stepList` decides with `stepNode`, so a value, fanout or erase agent holding two full
stacks is fireable here and irreducible there (`anyFireable_disagrees_with_stepList`). -/
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

/-- **Scan progress**: if one of the operand pairs is fireable, the scan finds it. This is a fact
about the operand scan alone; it says nothing about `stepList`, which does not consult the scan. -/
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

/-- Read-back of boundary outputs: the complete vector in order, or a refusal. The reducer never
calls it; it is the check a delivery-capable reducer needs. -/
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

/-- Observational state of a net: the operand stacks of every node and the outputs observed so
far. Nothing here produces this structure: `stepList` and `run` work on the node list alone, and
`deliverOutput` is its only writer, so no firing in this file reaches `outputs`. -/
structure Machine where
  /-- Operand stacks, one entry per node. -/
  nodes : List Operands
  /-- Outputs observed in boundary order. -/
  outputs : List Value

/-- Deliver a fired value to a boundary output: the value is appended, so the observation order is
the delivery order. Not called by the reducer; it is the writer a delivery-capable reducer needs. -/
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


/-- Push a delivered value onto one operand stack of a node, LIFO. Not called by the reducer: no
agent's result is routed to another agent here. -/
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

/-- Unwinding accounting a caller may charge for a reduction: rewrites performed and bytes retained,
against declared caps. Every charge is non-wrapping: a charge that would exceed its cap is refused,
never truncated. The reducer does not charge this structure — `stepList` and `run` take no budget —
so these operations are what a charged reducer would call, not what this one does. -/
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


/-- Step one node: fire it if arithmetic and both stacks are ready. The value the rule computes is
dropped; `stepNodeValue` is the variant that reports it. -/
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

/-- The operand scan is not the reducer's worklist: a non-arithmetic agent with two full operand
stacks is fireable for the scan and irreducible for `stepList`. -/
theorem anyFireable_disagrees_with_stepList :
    anyFireable [⟨[1], [2]⟩] = true ∧ stepList [(.value, ⟨[1], [2]⟩)] = none := ⟨rfl, rfl⟩

/-- Run the reducer for at most `fuel` steps. `some nodes` is a stuck net: no node is reducible.
`none` means a step was still possible when the fuel ran out, so work remains. A stuck net
therefore completes whatever the fuel, including zero. -/
def run : Nat → List (AgentKind × Operands) → Option (List (AgentKind × Operands))
  | 0, nodes =>
      match stepList nodes with
      | none => some nodes
      | some _ => none
  | fuel + 1, nodes =>
      match stepList nodes with
      | none => some nodes
      | some nodes' => run fuel nodes'

/-- With no fuel, a stuck net still completes: `run 0` returns the net, not `none`. -/
theorem run_zero_of_stuck {nodes : List (AgentKind × Operands)}
    (h : stepList nodes = none) : run 0 nodes = some nodes := by
  simp [run, h]

/-- `run` returns `none` exactly when work remains, at zero fuel as at any other fuel. -/
theorem run_zero (nodes : List (AgentKind × Operands)) :
    run 0 nodes = none ↔ stepList nodes ≠ none := by
  constructor
  · intro h
    cases hstep : stepList nodes with
    | none => simp [run_zero_of_stuck hstep] at h
    | some _ => simp
  · intro h
    cases hstep : stepList nodes with
    | none => exact absurd hstep h
    | some _ => simp [run, hstep]

/-- A stuck net is a completed run, whatever fuel remains. -/
theorem run_stuck {nodes : List (AgentKind × Operands)}
    (h : stepList nodes = none) : ∀ (fuel : Nat), run fuel nodes = some nodes := by
  intro fuel
  induction fuel with
  | zero => exact run_zero_of_stuck h
  | succ fuel ih => simp [run, h]

/-- A completed run preserves the node count: steps never add or drop agents. -/
theorem run_length : ∀ (fuel : Nat) (nodes nodes' : List (AgentKind × Operands)),
    run fuel nodes = some nodes' → nodes'.length = nodes.length := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes nodes' h
      cases hstep : stepList nodes with
      | none =>
          rw [run_zero_of_stuck hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          simp
      | some _ => simp [run, hstep] at h
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

/-- `none` is never returned for a net that cannot step: a completed run always reports the net.
This is the converse of `run_none_step`, and the property the `run` doc states. -/
theorem run_none_work_remains : ∀ (fuel : Nat) {nodes : List (AgentKind × Operands)},
    run fuel nodes = none → stepList nodes ≠ none := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes h
      exact (run_zero nodes).mp h
  | succ fuel ih =>
      intro nodes h
      obtain ⟨nodes', hstep, _⟩ := run_none_step h
      rw [hstep]
      simp

/-- A successful run ends on a stuck net: the reducer stops only when no node is reducible. -/
theorem run_some_stuck : ∀ (fuel : Nat) {nodes : List (AgentKind × Operands)}
    {nodes' : List (AgentKind × Operands)},
    run fuel nodes = some nodes' → stepList nodes' = none := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes nodes' h
      cases hstep : stepList nodes with
      | none =>
          rw [run_zero_of_stuck hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          exact hstep
      | some _ => simp [run, hstep] at h
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

/-- A successful run ends stuck: every node is irreducible at the end. `some` reports that, not
that the net's outputs are ready. -/
theorem run_some_all_irreducible {fuel : Nat} {nodes nodes' : List (AgentKind × Operands)}
    (h : run fuel nodes = some nodes') : ∀ node ∈ nodes', reducible node = false :=
  (stepList_none_iff nodes').mp (run_some_stuck fuel h)

/-- A run that also reports how many interaction steps it took. `some (k, final)` is a net that is
stuck after `k` steps and `none` means a step was still possible when the fuel ran out, exactly as
for `run`; a net needing `k` steps therefore completes with `k` units of fuel (see
`run_runCount_fuel`). -/
def runCount : Nat → List (AgentKind × Operands) →
    Option (Nat × List (AgentKind × Operands))
  | 0, nodes =>
      match stepList nodes with
      | none => some (0, nodes)
      | some _ => none
  | fuel + 1, nodes =>
      match stepList nodes with
      | none => some (0, nodes)
      | some nodes' =>
          match runCount fuel nodes' with
          | some (k, final) => some (k + 1, final)
          | none => none

/-- The counted run's step count never exceeds the fuel, and the bound is reachable:
`run_runCount_fuel` shows the reported count is enough fuel for `run`. -/
theorem runCount_steps_le : ∀ (fuel : Nat) {nodes : List (AgentKind × Operands)}
    {k : Nat} {final : List (AgentKind × Operands)},
    runCount fuel nodes = some (k, final) → k ≤ fuel := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes k final h
      cases hstep : stepList nodes with
      | none =>
          simp [runCount, hstep] at h
          obtain ⟨hk, _⟩ := h
          subst hk
          exact Nat.zero_le _
      | some _ => simp [runCount, hstep] at h
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
      cases hstep : stepList nodes with
      | none =>
          simp [runCount, hstep] at h
          obtain ⟨_, hnodes⟩ := h
          subst hnodes
          exact run_zero_of_stuck hstep
      | some _ => simp [runCount, hstep] at h
  | succ fuel ih =>
      intro nodes k final h
      cases hstep : stepList nodes with
      | none =>
          simp [runCount, hstep] at h
          obtain ⟨_, hnodes⟩ := h
          subst hnodes
          simp [run, hstep]
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

/-- The reported interaction count is exactly enough fuel: a net the counted run finishes in `k`
steps completes under `run` with `k` units of fuel. -/
theorem run_runCount_fuel : ∀ (fuel : Nat) {nodes : List (AgentKind × Operands)}
    {k : Nat} {final : List (AgentKind × Operands)},
    runCount fuel nodes = some (k, final) → run k nodes = some final := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes k final h
      cases hstep : stepList nodes with
      | none =>
          simp [runCount, hstep] at h
          obtain ⟨rfl, rfl⟩ := h
          exact run_zero_of_stuck hstep
      | some _ => simp [runCount, hstep] at h
  | succ fuel ih =>
      intro nodes k final h
      cases hstep : stepList nodes with
      | none =>
          simp [runCount, hstep] at h
          obtain ⟨rfl, rfl⟩ := h
          exact run_zero_of_stuck hstep
      | some next =>
          cases hrc : runCount fuel next with
          | none => simp [runCount, hstep, hrc] at h
          | some p =>
              obtain ⟨k', final'⟩ := p
              simp [runCount, hstep, hrc] at h
              obtain ⟨rfl, rfl⟩ := h
              have := ih hrc
              simp only [run, hstep]
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
    runCount 1 [(.add, ⟨[a], [b]⟩)] = some (1, [(.add, ⟨[], []⟩)]) := rfl

/-- A net needing one interaction completes with exactly one unit of fuel, and reports work
remaining at zero: the strict bound is reachable, not merely an inequality. -/
theorem run_single_add_exact_fuel (a b : BitVec 64) :
    run 1 [(.add, ⟨[a], [b]⟩)] = some [(.add, ⟨[], []⟩)] ∧
      run 0 [(.add, ⟨[a], [b]⟩)] = none := ⟨rfl, rfl⟩

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

/-- Fire the node at index `k`, in place: the rest of the net is untouched. Refuses when that node
cannot fire or the index is out of range. -/
def fireAt (k : Nat) : List (AgentKind × Operands) → Option (List (AgentKind × Operands))
  | [] => none
  | node :: rest =>
      match k with
      | 0 =>
          match stepNode node with
          | some node' => some (node' :: rest)
          | none => none
      | k + 1 =>
          match fireAt k rest with
          | some rest' => some (node :: rest')
          | none => none

/-- Firing at index zero agrees with the abstract step whenever the head is the reducible node. The
abstract reducer skips an irreducible head; an in-place plan at index zero refuses instead. -/
theorem fireAt_zero_of_reducible (node : AgentKind × Operands)
    (rest : List (AgentKind × Operands)) (h : reducible node = true) :
    fireAt 0 (node :: rest) = stepList (node :: rest) := by
  cases hstep : stepNode node with
  | none =>
      rw [reducible, hstep] at h
      simp at h
  | some node' => simp [fireAt, stepList, hstep]
/-- An in-place firing preserves the node count: one node is rewritten where it stands. -/
theorem fireAt_length : ∀ (k : Nat) (nodes nodes' : List (AgentKind × Operands)),
    fireAt k nodes = some nodes' → nodes'.length = nodes.length := by
  intro k
  induction k with
  | zero =>
      intro nodes nodes' h
      cases nodes with
      | nil => simp [fireAt] at h
      | cons node rest =>
          cases hstep : stepNode node with
          | none => simp [fireAt, hstep] at h
          | some node' =>
              simp [fireAt, hstep] at h
              obtain ⟨_, rfl⟩ := h
              rfl
  | succ k ih =>
      intro nodes nodes' h
      cases nodes with
      | nil => simp [fireAt] at h
      | cons node rest =>
          cases hrec : fireAt k rest with
          | none => simp [fireAt, hrec] at h
          | some rest' =>
              simp [fireAt, hrec] at h
              obtain ⟨_, rfl⟩ := h
              simp [ih rest rest' hrec]

/-- A plan is a list of node indices fired in order. -/
def applyPlan : List (AgentKind × Operands) → List Nat → Option (List (AgentKind × Operands))
  | nodes, [] => some nodes
  | nodes, k :: ks =>
      match fireAt k nodes with
      | some nodes' => applyPlan nodes' ks
      | none => none

/-- Every plan, whatever its index order, preserves the node count: in-place rewriting agrees with
abstract reduction on the size invariant. -/
theorem applyPlan_length : ∀ (ks : List Nat) (nodes nodes' : List (AgentKind × Operands)),
    applyPlan nodes ks = some nodes' → nodes'.length = nodes.length := by
  intro ks
  induction ks with
  | nil =>
      intro nodes nodes' h
      simp [applyPlan] at h
      obtain rfl := h
      rfl
  | cons k ks ih =>
      intro nodes nodes' h
      unfold applyPlan at h
      cases hfire : fireAt k nodes with
      | none => rw [hfire] at h; simp at h
      | some next =>
          rw [hfire] at h
          rw [ih next nodes' h]
          exact fireAt_length k nodes next hfire

/-- In-place firing never increases the number of reducible nodes, at any index. -/
theorem fireAt_reducibleCount_le : ∀ (k : Nat) (nodes nodes' : List (AgentKind × Operands)),
    fireAt k nodes = some nodes' → reducibleCount nodes' ≤ reducibleCount nodes := by
  intro k
  induction k with
  | zero =>
      intro nodes nodes' h
      cases nodes with
      | nil => simp [fireAt] at h
      | cons node rest =>
          cases hstep : stepNode node with
          | none => simp [fireAt, hstep] at h
          | some node' =>
              have hred : reducible node = true := reducible_of_stepNode _ _ hstep
              simp [fireAt, hstep] at h
              obtain ⟨_, rfl⟩ := h
              unfold reducibleCount
              rw [hred]
              have hle : (if reducible node' then 1 else 0) ≤ 1 := by
                split <;> simp
              exact Nat.add_le_add_right hle _
  | succ k ih =>
      intro nodes nodes' h
      cases nodes with
      | nil => simp [fireAt] at h
      | cons node rest =>
          cases hrec : fireAt k rest with
          | none => simp [fireAt, hrec] at h
          | some rest' =>
              simp [fireAt, hrec] at h
              obtain ⟨_, rfl⟩ := h
              unfold reducibleCount
              exact Nat.add_le_add_left (ih rest rest' hrec) _

/-- No plan, in any index order, increases the reducible-node count. -/
theorem applyPlan_reducibleCount_le : ∀ (ks : List Nat) (nodes nodes' : List (AgentKind × Operands)),
    applyPlan nodes ks = some nodes' → reducibleCount nodes' ≤ reducibleCount nodes := by
  intro ks
  induction ks with
  | nil =>
      intro nodes nodes' h
      simp [applyPlan] at h
      obtain rfl := h
      exact Nat.le_refl _
  | cons k ks ih =>
      intro nodes nodes' h
      unfold applyPlan at h
      cases hfire : fireAt k nodes with
      | none => rw [hfire] at h; simp at h
      | some next =>
          rw [hfire] at h
          exact Nat.le_trans (ih next nodes' h) (fireAt_reducibleCount_le k nodes next hfire)


/-- The node named by an index, or nothing when the index is out of range. -/
def nthNode : Nat → List (AgentKind × Operands) → Option (AgentKind × Operands)
  | 0, [] => none
  | 0, node :: _ => some node
  | _ + 1, [] => none
  | k + 1, _ :: rest => nthNode k rest

/-- A plan step refuses exactly when the index is out of range or the node it names cannot fire.
This is what lets a checker accept a plan without trusting its producer: refusal is a fact about the
named node, not about the schedule. -/
theorem fireAt_none_iff : ∀ (k : Nat) (nodes : List (AgentKind × Operands)),
    fireAt k nodes = none ↔
      nthNode k nodes = none ∨ ∃ node, nthNode k nodes = some node ∧ reducible node = false := by
  intro k
  induction k with
  | zero =>
      intro nodes
      cases nodes with
      | nil => simp [fireAt, nthNode]
      | cons node rest =>
          cases hstep : stepNode node with
          | none => simp [fireAt, nthNode, reducible, hstep]
          | some node' => simp [fireAt, nthNode, reducible, hstep]
  | succ k ih =>
      intro nodes
      cases nodes with
      | nil => simp [fireAt, nthNode]
      | cons node rest =>
          have hget : nthNode (k + 1) (node :: rest) = nthNode k rest := by simp [nthNode]
          rw [hget]
          cases hrec : fireAt k rest with
          | none =>
              have hyes : nthNode k rest = none ∨
                  ∃ node, nthNode k rest = some node ∧ reducible node = false := (ih rest).mp hrec
              simp [fireAt, hrec]
              simpa using hyes
          | some rest' =>
              have hne : ¬ (nthNode k rest = none ∨
                  ∃ node, nthNode k rest = some node ∧ reducible node = false) := by
                intro h
                rw [(ih rest).mpr h] at hrec
                simp at hrec
              simp [fireAt, hrec]
              simpa using hne

/-- A step that also reports the value it produced. -/
def stepNodeValue : AgentKind × Operands → Option ((AgentKind × Operands) × BitVec 64)
  | (kind, ops) =>
      match fire kind ops with
      | some (ops', v) => some ((kind, ops'), v)
      | none => none

/-- Reporting the value does not change the step: the same node is rewritten. -/
theorem stepNodeValue_stepNode {node node' : AgentKind × Operands} {v : BitVec 64}
    (h : stepNodeValue node = some (node', v)) : stepNode node = some node' := by
  obtain ⟨kind, ops⟩ := node
  simp only [stepNodeValue, stepNode] at h ⊢
  cases hfire : fire kind ops with
  | none => rw [hfire] at h; simp at h
  | some p =>
      rw [hfire] at h
      obtain ⟨ops', w⟩ := p
      simp only [Option.some.injEq, Prod.mk.injEq] at h
      obtain ⟨rfl, _⟩ := h
      rfl

/-- Fire the node at index `k`, reporting the produced value. -/
def fireAtValue (k : Nat) (nodes : List (AgentKind × Operands)) :
    Option (List (AgentKind × Operands) × BitVec 64) :=
  match nthNode k nodes with
  | none => none
  | some node =>
      match stepNodeValue node with
      | none => none
      | some (_node', v) =>
          match fireAt k nodes with
          | some nodes' => some (nodes', v)
          | none => none

/-- The value-reporting plan step agrees with the plain one on the net it produces. -/
theorem fireAtValue_fireAt {k : Nat} {nodes nodes' : List (AgentKind × Operands)}
    {v : BitVec 64} (h : fireAtValue k nodes = some (nodes', v)) :
    fireAt k nodes = some nodes' := by
  cases hn : nthNode k nodes with
  | none =>
      have hnone : fireAtValue k nodes = none := by simp [fireAtValue, hn]
      rw [hnone] at h
      simp at h
  | some node =>
      cases hv : stepNodeValue node with
      | none =>
          have hnone : fireAtValue k nodes = none := by simp [fireAtValue, hn, hv]
          rw [hnone] at h
          simp at h
      | some p =>
          cases hf : fireAt k nodes with
          | none =>
              have hnone : fireAtValue k nodes = none := by simp [fireAtValue, hn, hv, hf]
              rw [hnone] at h
              simp at h
          | some r =>
              have hsome : fireAtValue k nodes = some (r, p.2) := by
                simp [fireAtValue, hn, hv, hf]
              rw [hsome] at h
              simp only [Option.some.injEq, Prod.mk.injEq] at h
              exact congrArg some h.1

/-- A plan that reports every value it produces, in order. -/
def applyPlanValues : List (AgentKind × Operands) → List Nat →
    Option (List (BitVec 64) × List (AgentKind × Operands))
  | nodes, [] => some ([], nodes)
  | nodes, k :: ks =>
      match fireAtValue k nodes with
      | some (nodes', v) =>
          match applyPlanValues nodes' ks with
          | some (vs, final) => some (v :: vs, final)
          | none => none
      | none => none

/-- The value log records one value per firing, so its length is bounded by the plan length. -/
theorem applyPlanValues_length : ∀ (ks : List Nat) (nodes : List (AgentKind × Operands))
    (vs : List (BitVec 64)) (final : List (AgentKind × Operands)),
    applyPlanValues nodes ks = some (vs, final) → vs.length ≤ ks.length := by
  intro ks
  induction ks with
  | nil =>
      intro nodes vs final h
      simp [applyPlanValues] at h
      obtain ⟨rfl, _⟩ := h
      simp
  | cons k ks ih =>
      intro nodes vs final h
      cases hfire : fireAtValue k nodes with
      | none => simp [applyPlanValues, hfire] at h
      | some p =>
          obtain ⟨next, v⟩ := p
          simp only [applyPlanValues, hfire] at h
          cases hrec : applyPlanValues next ks with
          | none => simp [hrec] at h
          | some q =>
              obtain ⟨vs', final'⟩ := q
              simp only [hrec, Option.some.injEq, Prod.mk.injEq] at h
              obtain ⟨rfl, _⟩ := h
              simp only [List.length_cons]
              exact Nat.succ_le_succ (ih next vs' final' hrec)

/-- The value-logging plan produces exactly the net the plain plan produces. -/
theorem applyPlanValues_applyPlan : ∀ (ks : List Nat) (nodes : List (AgentKind × Operands))
    (vs : List (BitVec 64)) (final : List (AgentKind × Operands)),
    applyPlanValues nodes ks = some (vs, final) → applyPlan nodes ks = some final := by
  intro ks
  induction ks with
  | nil =>
      intro nodes vs final h
      simp [applyPlanValues] at h
      obtain ⟨_, rfl⟩ := h
      simp [applyPlan]
  | cons k ks ih =>
      intro nodes vs final h
      cases hfire : fireAtValue k nodes with
      | none => simp [applyPlanValues, hfire] at h
      | some p =>
          obtain ⟨next, v⟩ := p
          simp only [applyPlanValues, hfire] at h
          cases hrec : applyPlanValues next ks with
          | none => simp [hrec] at h
          | some q =>
              obtain ⟨vs', final'⟩ := q
              simp only [hrec, Option.some.injEq, Prod.mk.injEq] at h
              obtain ⟨_, hfinal⟩ := h
              subst hfinal
              have hstep : fireAt k nodes = some next := fireAtValue_fireAt hfire
              unfold applyPlan
              rw [hstep]
              exact ih next vs' final' hrec

/-- Time potential: the work still available at a node, counted as the values waiting on its two
ports. A node with empty ports has no work left. This is a per-node quantity: the only theorem about
it is the one-node obligation `timePotential_obligation`, and no run's interaction count is derived
from it — the only proved count bound is the fuel (`runCount_steps_le`). -/
def timePotential : AgentKind × Operands → Nat
  | (kind, ops) =>
      if kind.isArithmetic then ops.first.length + ops.second.length else 0

/-- Firing consumes exactly one value from each port. -/
theorem fire_ops_lengths {kind : AgentKind} {ops ops' : Operands} {v : BitVec 64}
    (h : fire kind ops = some (ops', v)) :
    ops'.first.length + ops'.second.length + 2 = ops.first.length + ops.second.length := by
  cases ops with
  | mk first second =>
      cases first with
      | nil => simp [fire] at h
      | cons a as =>
          cases second with
          | nil => simp [fire] at h
          | cons b bs =>
              cases kind with
              | add => (simp only [fire, Option.some.injEq, Prod.mk.injEq] at h; obtain ⟨h1, _⟩ := h; subst h1; simp <;> omega)
              | sub => (simp only [fire, Option.some.injEq, Prod.mk.injEq] at h; obtain ⟨h1, _⟩ := h; subst h1; simp <;> omega)
              | mul => (simp only [fire, Option.some.injEq, Prod.mk.injEq] at h; obtain ⟨h1, _⟩ := h; subst h1; simp <;> omega)
              | eq => (simp only [fire, Option.some.injEq, Prod.mk.injEq] at h; obtain ⟨h1, _⟩ := h; subst h1; simp <;> omega)
              | value => simp [fire] at h
              | fanout => simp [fire] at h
              | erase => simp [fire] at h
              | input => simp [fire] at h
              | output => simp [fire] at h

/-- The local time obligation for one arithmetic rule instance: the potential at the node before
firing covers the charge of one interaction plus the potential after firing. -/
theorem timePotential_obligation (kind : AgentKind) (hk : kind.isArithmetic = true)
    (ops ops' : Operands) (v : BitVec 64) (h : fire kind ops = some (ops', v)) :
    timePotential (kind, ops) ≥ 1 + timePotential (kind, ops') := by
  have hlen := fire_ops_lengths h
  simp only [timePotential, hk, ite_true]
  omega

/-- The non-arithmetic rules move the waiting-value count in opposite directions, so a potential
must treat them differently: erasure emits nothing, fanout emits two copies of the value it
consumes. These multiplicities are what a local obligation for either rule has to charge; the
reducer itself currently fires neither, so no obligation over `stepNode` exists for them yet. -/
theorem nonarithmetic_multiplicities (v : Value) :
    (fireErase v).length = 0 ∧ (fireFanout v).length = 2 := by
  constructor
  · rfl
  · rfl

/-- Space potential: one storage cell per node plus one per waiting value. A node's own weight is
the constant one, so a declared node-storage weight of one cell is dominated by construction. -/
def spacePotential : List (AgentKind × Operands) → Nat
  | [] => 0
  | (_, ops) :: rest =>
      (1 + ops.first.length + ops.second.length) + spacePotential rest

/-- The space potential dominates node storage: at least one cell per node. -/
theorem spacePotential_ge_length : ∀ (nodes : List (AgentKind × Operands)),
    spacePotential nodes ≥ nodes.length := by
  intro nodes
  induction nodes with
  | nil => simp [spacePotential]
  | cons node rest ih =>
      obtain ⟨kind, ops⟩ := node
      unfold spacePotential
      simp only [List.length_cons]
      omega

/-- A successful step on a named node rewrites that node in place, and the rule it fired computed
some value. The step does not keep that value; `stepNodeValue` is the variant that returns it. -/
theorem stepNode_some_fire {kind : AgentKind} {ops : Operands} {node' : AgentKind × Operands}
    (h : stepNode (kind, ops) = some node') :
    ∃ ops' v, node' = (kind, ops') ∧ fire kind ops = some (ops', v) := by
  simp only [stepNode] at h
  cases hfire : fire kind ops with
  | none => rw [hfire] at h; simp at h
  | some p =>
      obtain ⟨ops2, v⟩ := p
      rw [hfire] at h
      simp only [Option.some.injEq] at h
      exact ⟨ops2, v, h.symm, rfl⟩

/-- A step rewrites one node in place and consumes exactly two waiting values, so it drops the
space potential by exactly two: the potential after the step plus two is the potential before it. -/
theorem stepList_spacePotential_drop : ∀ (nodes nodes' : List (AgentKind × Operands)),
    stepList nodes = some nodes' → spacePotential nodes' + 2 = spacePotential nodes := by
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
          obtain ⟨ops', v, hshape, hfire⟩ := stepNode_some_fire hstep
          subst hshape
          have hl := fire_ops_lengths hfire
          have hval : stepList ((kind, ops) :: rest) = some ((kind, ops') :: rest) := by
            simp [stepList, hstep]
          rw [hval] at h
          simp only [Option.some.injEq] at h
          subst h
          unfold spacePotential
          omega
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
          have hih := ih t hrest
          unfold spacePotential
          omega

/-- A step cannot increase the space potential; the exact drop is `stepList_spacePotential_drop`. -/
theorem stepList_spacePotential_le : ∀ (nodes nodes' : List (AgentKind × Operands)),
    stepList nodes = some nodes' → spacePotential nodes' ≤ spacePotential nodes := by
  intro nodes nodes' h
  have hdrop := stepList_spacePotential_drop nodes nodes' h
  omega

/-- Values waiting on the net's ports, summed over nodes. -/
def stackValues : List (AgentKind × Operands) → Nat
  | [] => 0
  | (_, ops) :: rest => ops.first.length + ops.second.length + stackValues rest

/-- A step cannot increase the waiting-value count: the fired node loses exactly the two values it
consumed, and every other node is carried through. -/
theorem stepList_stackValues_le : ∀ (nodes nodes' : List (AgentKind × Operands)),
    stepList nodes = some nodes' → stackValues nodes' ≤ stackValues nodes := by
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
          obtain ⟨ops', v, hshape, hfire⟩ := stepNode_some_fire hstep
          subst hshape
          have hl := fire_ops_lengths hfire
          have hval : stepList ((kind, ops) :: rest) = some ((kind, ops') :: rest) := by
            simp [stepList, hstep]
          rw [hval] at h
          simp only [Option.some.injEq] at h
          subst h
          unfold stackValues
          omega
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
          unfold stackValues
          exact Nat.add_le_add_left (ih t hrest) _

/-- Whole-run space bound: a run of any length cannot exceed the net's initial space potential. -/
theorem run_spacePotential_le : ∀ (fuel : Nat) (nodes nodes' : List (AgentKind × Operands)),
    run fuel nodes = some nodes' → spacePotential nodes' ≤ spacePotential nodes := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes nodes' h
      cases hstep : stepList nodes with
      | none =>
          rw [run_zero_of_stuck hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          exact Nat.le_refl _
      | some _ => simp [run, hstep] at h
  | succ fuel ih =>
      intro nodes nodes' h
      unfold run at h
      cases hstep : stepList nodes with
      | none =>
          rw [hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          exact Nat.le_refl _
      | some next =>
          rw [hstep] at h
          exact Nat.le_trans (ih next nodes' h) (stepList_spacePotential_le nodes next hstep)

/-- Whole-run waiting-value bound: a run of any length cannot exceed the net's initial waiting
value count. -/
theorem run_stackValues_le : ∀ (fuel : Nat) (nodes nodes' : List (AgentKind × Operands)),
    run fuel nodes = some nodes' → stackValues nodes' ≤ stackValues nodes := by
  intro fuel
  induction fuel with
  | zero =>
      intro nodes nodes' h
      cases hstep : stepList nodes with
      | none =>
          rw [run_zero_of_stuck hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          exact Nat.le_refl _
      | some _ => simp [run, hstep] at h
  | succ fuel ih =>
      intro nodes nodes' h
      unfold run at h
      cases hstep : stepList nodes with
      | none =>
          rw [hstep] at h
          simp only [Option.some.injEq] at h
          subst h
          exact Nat.le_refl _
      | some next =>
          rw [hstep] at h
          exact Nat.le_trans (ih next nodes' h) (stepList_stackValues_le nodes next hstep)

/-- The certificate revision this checker accepts. -/
def certificateRevision : Nat := 1

/-- A bounded certificate over a net: the revision it is written for, the net it carries, the node
count it claims for that net, and the space it declares. The net is inside the certificate, so
accepting one binds no network outside it. -/
structure NetCertificate where
  /-- Certificate format revision. -/
  revision : Nat
  /-- The net the certificate is about, carried inside the certificate. -/
  nodes : List (AgentKind × Operands)
  /-- Node count claimed for `nodes`, the certificate's own list. -/
  netLength : Nat
  /-- Space the certificate declares sufficient for `nodes`. -/
  declaredSpace : Nat

/-- Accept a certificate only when it is written for this revision, its claimed node count is the
count of its own embedded node list, and its declared space dominates that list's space potential.
Nothing compares the embedded list with an independently supplied net, so acceptance says the
embedded net fits the embedded declaration — not that the certificate describes the net a producer
claims to have. -/
def checkCertificate (c : NetCertificate) : Bool :=
  (c.revision == certificateRevision) && (c.netLength == c.nodes.length) &&
    decide (spacePotential c.nodes ≤ c.declaredSpace)

/-- A checked certificate says exactly what it claims: the revision matches, the node count is the
embedded list's, and the declared space dominates that list's potential. -/
theorem checkCertificate_sound {c : NetCertificate} (h : checkCertificate c = true) :
    c.revision = certificateRevision ∧ c.netLength = c.nodes.length ∧
      spacePotential c.nodes ≤ c.declaredSpace := by
  unfold checkCertificate at h
  simp only [Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq] at h
  exact ⟨h.1.1, h.1.2, h.2⟩

/-- The control: a declared space smaller than the net's potential is refused rather than
accepted with a warning. -/
theorem checkCertificate_refuses_undersized (c : NetCertificate)
    (h : spacePotential c.nodes > c.declaredSpace) : checkCertificate c = false := by
  have hnot : ¬ (spacePotential c.nodes ≤ c.declaredSpace) := Nat.not_le.mpr h
  unfold checkCertificate
  simp [hnot]

/-- A checked certificate bounds the final net of any completed run of its embedded node list.
Since a step never increases the potential (`stepList_spacePotential_le`), every net the run passes
through is no larger either, and `plan_within_declared_space` states the same bound for every
intermediate net an in-place plan produces. -/
theorem run_within_declared_space {c : NetCertificate} {fuel : Nat}
    {nodes' : List (AgentKind × Operands)}
    (hcheck : checkCertificate c = true) (hrun : run fuel c.nodes = some nodes') :
    spacePotential nodes' ≤ c.declaredSpace := by
  have hbound := run_spacePotential_le fuel c.nodes nodes' hrun
  have hdecl := (checkCertificate_sound hcheck).2.2
  omega

/-- Acceptance is exactly the three conditions, in both directions: the checker neither misses a
violation nor refuses a well-formed certificate. -/
theorem checkCertificate_iff (c : NetCertificate) :
    checkCertificate c = true ↔ c.revision = certificateRevision ∧
      c.netLength = c.nodes.length ∧ spacePotential c.nodes ≤ c.declaredSpace := by
  constructor
  · exact checkCertificate_sound
  · intro h
    unfold checkCertificate
    simp only [Bool.and_eq_true, beq_iff_eq, decide_eq_true_eq]
    exact ⟨⟨h.1, h.2.1⟩, h.2.2⟩

/-- Control: a certificate written for a different revision is refused. -/
theorem checkCertificate_refuses_wrong_revision (c : NetCertificate)
    (h : c.revision ≠ certificateRevision) : checkCertificate c = false := by
  cases hcheck : checkCertificate c with
  | false => rfl
  | true =>
      have hs := checkCertificate_sound hcheck
      exact absurd hs.1 h

/-- Control: a certificate claiming a node count the net does not have is refused. -/
theorem checkCertificate_refuses_wrong_count (c : NetCertificate)
    (h : c.netLength ≠ c.nodes.length) : checkCertificate c = false := by
  cases hcheck : checkCertificate c with
  | false => rfl
  | true =>
      have hs := checkCertificate_sound hcheck
      exact absurd hs.2.1 h

/-- The checker accepts a concrete certificate: one empty-stack agent declaring one cell. This is
the acceptance side of the battery - refusal theorems alone would be satisfied by a checker that
rejects everything. -/
theorem certificate_accepts_small_net :
    checkCertificate
        { revision := 1, nodes := [(.add, ⟨[], []⟩)], netLength := 1, declaredSpace := 1 } = true :=
  rfl

/-- In-place firing cannot increase the space potential, at any index. -/
theorem fireAt_spacePotential_le : ∀ (k : Nat) (nodes nodes' : List (AgentKind × Operands)),
    fireAt k nodes = some nodes' → spacePotential nodes' ≤ spacePotential nodes := by
  intro k
  induction k with
  | zero =>
      intro nodes nodes' h
      cases nodes with
      | nil => simp [fireAt] at h
      | cons node rest =>
          obtain ⟨kind, ops⟩ := node
          cases hstep : stepNode (kind, ops) with
          | none => simp [fireAt, hstep] at h
          | some node' =>
              obtain ⟨ops', v, hshape, hfire⟩ := stepNode_some_fire hstep
              subst hshape
              have hl := fire_ops_lengths hfire
              simp [fireAt, hstep] at h
              obtain ⟨_, rfl⟩ := h
              unfold spacePotential
              omega
  | succ k ih =>
      intro nodes nodes' h
      cases nodes with
      | nil => simp [fireAt] at h
      | cons node rest =>
          obtain ⟨kind, ops⟩ := node
          cases hrec : fireAt k rest with
          | none => simp [fireAt, hrec] at h
          | some rest' =>
              simp [fireAt, hrec] at h
              obtain ⟨_, rfl⟩ := h
              unfold spacePotential
              exact Nat.add_le_add_left (ih rest rest' hrec) _

/-- No plan, in any index order, increases the space potential. -/
theorem applyPlan_spacePotential_le : ∀ (ks : List Nat) (nodes nodes' : List (AgentKind × Operands)),
    applyPlan nodes ks = some nodes' → spacePotential nodes' ≤ spacePotential nodes := by
  intro ks
  induction ks with
  | nil =>
      intro nodes nodes' h
      simp [applyPlan] at h
      obtain rfl := h
      exact Nat.le_refl _
  | cons k ks ih =>
      intro nodes nodes' h
      unfold applyPlan at h
      cases hfire : fireAt k nodes with
      | none => rw [hfire] at h; simp at h
      | some next =>
          rw [hfire] at h
          exact Nat.le_trans (ih next nodes' h) (fireAt_spacePotential_le k nodes next hfire)

/-- A checked certificate bounds any completed plan over its net, in any index order. -/
theorem plan_within_declared_space {c : NetCertificate} {ks : List Nat}
    {nodes' : List (AgentKind × Operands)}
    (hcheck : checkCertificate c = true) (hplan : applyPlan c.nodes ks = some nodes') :
    spacePotential nodes' ≤ c.declaredSpace := by
  have hbound := applyPlan_spacePotential_le ks c.nodes nodes' hplan
  have hdecl := (checkCertificate_sound hcheck).2.2
  omega

end NobleM2.InteractionNet
