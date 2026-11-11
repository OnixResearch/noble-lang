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

end NobleM2.InteractionNet
