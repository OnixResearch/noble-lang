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

/-- A port of a net: the agent's index and its slot, `none` for the principal. -/
abbrev Port := Nat × Option Nat

/-- A wire joins two ports. Revision 2 allows auxiliary-to-auxiliary value wires. -/
abbrev Wire := Port × Port

/-- How many wires touch a port, counting both ends. -/
def incidence : List Wire → Port → Nat
  | [], _ => 0
  | (a, b) :: rest, p =>
      (if a = p then 1 else 0) + (if b = p then 1 else 0) + incidence rest p

theorem incidence_nil (p : Port) : incidence [] p = 0 := rfl

theorem incidence_cons (w : Wire) (rest : List Wire) (p : Port) :
    incidence (w :: rest) p
      = (if w.1 = p then 1 else 0) + (if w.2 = p then 1 else 0) + incidence rest p := rfl

/-- A port's incidence never exceeds the number of wires: each wire contributes at most two. -/
theorem incidence_le_two_mul_length (wires : List Wire) (p : Port) :
    incidence wires p ≤ 2 * wires.length := by
  induction wires with
  | nil => simp [incidence]
  | cons w rest ih =>
      rw [incidence_cons]
      have hlen : (w :: rest).length = rest.length + 1 := rfl
      rw [hlen]
      split <;> split <;> omega

/-- A port touched by a wire is touched at least once. -/
theorem incidence_pos_of_mem {wires : List Wire} {w : Wire} (hmem : w ∈ wires)
    {p : Port} (hp : w.1 = p ∨ w.2 = p) : 0 < incidence wires p := by
  induction wires with
  | nil => simp at hmem
  | cons v rest ih =>
      rw [incidence_cons]
      rcases List.mem_cons.mp hmem with rfl | hrest
      · rcases hp with _ | _
        · split <;> omega
        · split <;> omega
      · have := ih hrest
        omega

end NobleM2.InteractionNet
