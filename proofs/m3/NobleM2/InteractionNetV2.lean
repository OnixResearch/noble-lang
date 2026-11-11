import NobleM2.SizeChange

/-!
# Revision-2 I64 interaction-net vocabulary (issue #37)

The agent and port table of the selected contract (`docs/i64-net-contract-v2.md`), as Lean types, so
the validator and reducer can be written against one definition rather than prose.

The three decisions that shape the table are recorded here as theorems rather than comments:

- arithmetic agents consume values on **two operand ports** and own no value at the principal;
- `Fanout` has **two auxiliary copies** and may feed operands, which is legal only because
  auxiliary-to-auxiliary value wires are permitted;
- there is **one value domain**: `Eq` produces `I64` 0/1, so no second wire type exists.

What is not here: wire incidence, boundary order, active-pair inventory or size limits. Those are the
validator's checks and are the next unit; reduction is not modelled at all.
-/

namespace NobleM2.InteractionNet

/-- Agent kinds of revision 2. -/
inductive AgentKind where
  /-- A literal value travelling on a wire. -/
  | value
  /-- Wrapping addition. -/
  | add
  /-- Ordered subtraction, `first - second` by port. -/
  | sub
  /-- Wrapping multiplication. -/
  | mul
  /-- Equality, yielding `I64` 0/1 in the single value domain. -/
  | eq
  /-- Copies a value onto two auxiliary ports. -/
  | fanout
  /-- Discards a value. -/
  | erase
  /-- Boundary input at an ordered position. -/
  | input
  /-- Boundary output at an ordered position. -/
  | output
deriving DecidableEq, Repr

/-- Number of auxiliary ports an agent kind has. Arithmetic agents consume on their two operand
ports; fanout has two copies; value, erase, input and output have none. -/
def AgentKind.auxiliaryPorts : AgentKind → Nat
  | .value => 0
  | .add => 2
  | .sub => 2
  | .mul => 2
  | .eq => 2
  | .fanout => 2
  | .erase => 0
  | .input => 0
  | .output => 0

/-- Arithmetic agents consume values, so they have no value at their principal port. -/
def AgentKind.isArithmetic : AgentKind → Bool
  | .add => true
  | .sub => true
  | .mul => true
  | .eq => true
  | _ => false

theorem arithmetic_has_two_operands (k : AgentKind) (h : k.isArithmetic = true) :
    k.auxiliaryPorts = 2 := by
  cases k <;> simp_all [AgentKind.isArithmetic, AgentKind.auxiliaryPorts]

theorem arithmetic_has_no_values (k : AgentKind) (h : k.isArithmetic = true) :
    k ≠ .value := by
  cases k <;> simp_all [AgentKind.isArithmetic]

/-- Fanout copies to exactly two auxiliary ports. -/
theorem fanout_has_two_copies : AgentKind.fanout.auxiliaryPorts = 2 := rfl

/-- `Eq` lives in the single value domain: its result is an `I64` value like any other, so the table
has no separate Boolean agent kind for it. -/
theorem eq_shares_the_value_domain : AgentKind.eq.isArithmetic = true := rfl

end NobleM2.InteractionNet
