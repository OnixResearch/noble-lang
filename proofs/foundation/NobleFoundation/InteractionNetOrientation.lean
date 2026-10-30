/-
  What contract revision 1 of the checked interaction net admits (issue #37).

  `InteractionNet.lean` fixes the orientation of a binary agent: port `0` is principal and is the
  result, and ports `1` and `2` are the ordered operands. This module measures what the revision-1
  validator admits under that orientation, before any reduction rule is designed. Contract
  revision 1 and `validate` are unchanged.

  - `orientedOk` is the documented orientation as a check: every wire joins a value producer
    (an input or constant, a binary result, a fanout copy) to a value consumer (a binary operand,
    an output, an eraser, a fanout input).
  - `oriented_active_pair`: in an oriented net every active pair (principal to principal) has an
    output, eraser or fanout on one side. A constant or input therefore never meets an arithmetic
    agent in an active pair, so under revision 1 no numeric rewrite can be an interaction rule.
  - `validate` does not enforce the orientation: it accepts a constant wired into a result port
    (`value_into_result_port_is_accepted`).
  - Its ordered-operand rule compares an input's source slot with the operand position. It rejects
    the direct net for `y - x` (`swappedSubOperands`), yet accepts the same subtraction once each
    input passes through an addition of `0` (`identity_agents_bypass_the_operand_rule`). So the
    rule constrains the wiring shape, not the function the net computes.
  - A fanout copy can never reach an operand: both are auxiliary ports, and the compatibility check
    refuses auxiliary-to-auxiliary links (`fanout_cannot_feed_an_operand`). Under revision 1 a shared
    intermediate value cannot feed two operations.

  These are measurements of the contract model only; no reduction semantics, Rust or runtime claim
  is made.
-/
import NobleFoundation.InteractionNet

namespace NobleFoundation.InteractionNet

/-- The principal-to-principal associations: the active pairs of a net. -/
def activePairs (net : Net) : List (Port × Port) :=
  net.wiring.filter fun edge => edge.1.slot == 0 && edge.2.slot == 0

/-- A port that produces a value: an input or constant, a binary agent's result, or a copy leaving
a fanout agent. -/
def producesValue (net : Net) (candidate : Port) : Bool :=
  match kindAt net candidate.agent with
  | some (.input _) => candidate.slot == 0
  | some (.const _) => candidate.slot == 0
  | some .add => candidate.slot == 0
  | some .sub => candidate.slot == 0
  | some .mul => candidate.slot == 0
  | some .eq => candidate.slot == 0
  | some .dup => candidate.slot != 0
  | _ => false

/-- A port that consumes a value: a binary agent's operand, an output, an eraser, or the input of a
fanout agent. -/
def consumesValue (net : Net) (candidate : Port) : Bool :=
  match kindAt net candidate.agent with
  | some .add => candidate.slot != 0
  | some .sub => candidate.slot != 0
  | some .mul => candidate.slot != 0
  | some .eq => candidate.slot != 0
  | some .output => candidate.slot == 0
  | some .era => candidate.slot == 0
  | some .dup => candidate.slot == 0
  | _ => false

/-- The documented orientation as a check: every wire joins a producer to a consumer. -/
def orientedOk (net : Net) : Bool :=
  net.wiring.all fun edge =>
    (producesValue net edge.1 && consumesValue net edge.2) ||
      (consumesValue net edge.1 && producesValue net edge.2)

/-- A principal port that consumes: an output, an eraser or a fanout input. -/
def consumingPrincipal (net : Net) (candidate : Port) : Prop :=
  candidate.slot = 0 ∧
    (kindAt net candidate.agent = some .output ∨ kindAt net candidate.agent = some .era ∨
      kindAt net candidate.agent = some .dup)

/-- A consuming port at slot `0` is an output, eraser or fanout input: binary agents consume only at
their operand ports. -/
theorem consuming_principal_of_slot_zero (net : Net) (candidate : Port)
    (consumes : consumesValue net candidate = true) (principal : candidate.slot = 0) :
    consumingPrincipal net candidate := by
  refine ⟨principal, ?_⟩
  unfold consumesValue at consumes
  cases kind : kindAt net candidate.agent with
  | none => simp [kind] at consumes
  | some agentKind =>
      cases agentKind <;> simp_all

/-- **Every active pair of an oriented net has an output, eraser or fanout on one side.** A
constant, an input or a binary result can only meet one of those, so no value meets an arithmetic
agent in an active pair, and arithmetic cannot be an interaction rule under revision 1. -/
theorem oriented_active_pair (net : Net) (oriented : orientedOk net = true) (edge : Port × Port)
    (active : edge ∈ activePairs net) :
    consumingPrincipal net edge.1 ∨ consumingPrincipal net edge.2 := by
  unfold activePairs at active
  obtain ⟨wired, principals⟩ := List.mem_filter.mp active
  simp only [Bool.and_eq_true, beq_iff_eq] at principals
  have joined := List.all_eq_true.mp oriented edge wired
  simp only [Bool.or_eq_true, Bool.and_eq_true] at joined
  rcases joined with ⟨_, consumesRight⟩ | ⟨consumesLeft, _⟩
  · exact Or.inr (consuming_principal_of_slot_zero net edge.2 consumesRight principals.2)
  · exact Or.inl (consuming_principal_of_slot_zero net edge.1 consumesLeft principals.1)

/-! ## Fixtures -/

/-- The compiled `x - y` net is oriented, and its only active pair is the subtraction meeting the
output: the subtraction itself has nothing to interact with. -/
theorem deeper_minus_top_has_only_the_output_pair :
    orientedOk deeperMinusTop = true ∧
      activePairs deeperMinusTop = [(port 2 0, port 3 0), (port 3 0, port 2 0)] := by
  decide

/-- A constant wired into an addition's result port, with the output fed by another constant. -/
def valueIntoResult : Net where
  version := contractVersion
  agents := [.const 5, .input 0, .input 1, .add, .output, .const 0]
  wiring :=
    [ (port 0 0, port 3 0), (port 3 0, port 0 0)
    , (port 1 0, port 3 1), (port 3 1, port 1 0)
    , (port 2 0, port 3 2), (port 3 2, port 2 0)
    , (port 4 0, port 5 0), (port 5 0, port 4 0) ]
  boundary := [port 4 0]

/-- **The validator does not enforce the orientation**: it accepts a constant wired into a result
port, which the orientation check refuses. -/
theorem value_into_result_port_is_accepted :
    validate valueIntoResult = true ∧ orientedOk valueIntoResult = false := by
  decide

/-- `y - x` through fanout agents: each input enters a fanout whose second copy is erased. -/
def swapThroughFanout : Net where
  version := contractVersion
  agents := [.input 0, .input 1, .sub, .output, .dup, .era, .dup, .era]
  wiring :=
    [ (port 1 0, port 4 0), (port 4 0, port 1 0)
    , (port 4 1, port 2 1), (port 2 1, port 4 1)
    , (port 4 2, port 5 0), (port 5 0, port 4 2)
    , (port 0 0, port 6 0), (port 6 0, port 0 0)
    , (port 6 1, port 2 2), (port 2 2, port 6 1)
    , (port 6 2, port 7 0), (port 7 0, port 6 2)
    , (port 2 0, port 3 0), (port 3 0, port 2 0) ]
  boundary := [port 3 0]

/-- **A fanout copy can never reach an operand**: the fanout net is oriented, but the validator
refuses it, because a fanout copy and an operand port are both auxiliary. -/
theorem fanout_cannot_feed_an_operand :
    orientedOk swapThroughFanout = true ∧ validate swapThroughFanout = false := by
  decide

/-- `y - x` with each input passing through an addition of `0`: `0 + y` takes `y` at operand
position `1` and `x + 0` takes `x` at operand position `0`, as the slot rule demands. -/
def yMinusXThroughIdentities : Net where
  version := contractVersion
  agents := [.input 0, .input 1, .add, .const 0, .add, .const 0, .sub, .output]
  wiring :=
    [ (port 3 0, port 2 1), (port 2 1, port 3 0)
    , (port 1 0, port 2 2), (port 2 2, port 1 0)
    , (port 0 0, port 4 1), (port 4 1, port 0 0)
    , (port 5 0, port 4 2), (port 4 2, port 5 0)
    , (port 2 0, port 6 1), (port 6 1, port 2 0)
    , (port 4 0, port 6 2), (port 6 2, port 4 0)
    , (port 6 0, port 7 0), (port 7 0, port 6 0) ]
  boundary := [port 7 0]

/-- **The operand rule does not track the computed function**: the direct net for `y - x` is
refused, while the same subtraction with each input passed through an addition of `0` is
accepted and oriented. -/
theorem identity_agents_bypass_the_operand_rule :
    validate swappedSubOperands = false ∧ validate yMinusXThroughIdentities = true ∧
      orientedOk yMinusXThroughIdentities = true := by
  decide

end NobleFoundation.InteractionNet
