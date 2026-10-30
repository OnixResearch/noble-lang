/-!
# Checked interaction-net contract (issue #37)

This file is a finite, kernel-reducible model of the *net contract* for a
checked I64 interaction-net backend.  It names agent kinds, checks natural-number
ports against their agent arity, represents wiring as a finite association, and
keeps the output boundary in declared order.

The validator is structural.  It does not reduce a net, evaluate constants, or
read a result back into a stack.  In particular, the accepted `x - y` fixture
only records that the two ordered inputs reach the two ordered subtraction
ports and that the subtraction result reaches its output boundary.

The interaction rules, termination or confluence, readback, the graph-to-net
translation, budgets, and the owner's continuation criteria remain open.
-/

namespace NobleFoundation.InteractionNet

/-- The finite agent vocabulary of the checked net contract.

`input` carries the source-order input slot.  Binary arithmetic/comparison
agents use port `0` as their principal port and ports `1` and `2` as the
ordered operands.  `dup` is the fanout agent and `era` is the erase agent;
both are structural only in this contract model.
-/
inductive AgentKind where
  | input (slot : Nat)
  | const (value : Int)
  | add
  | sub
  | mul
  | eq
  | dup
  | era
  | output
deriving DecidableEq, Repr

/-- A natural-number port.  `validPort` below is the arity check; using `Nat`
keeps the serialized contract explicit while retaining the same invariant a
`Fin arity` representation would carry.
-/
structure Port where
  agent : Nat
  slot : Nat
deriving DecidableEq, Repr

/-- A finite net declaration.  `wiring` is an association list and contains
both directions of each wire.  `boundary` is the ordered output boundary.
-/
structure Net where
  version : Nat
  agents : List AgentKind
  wiring : List (Port × Port)
  boundary : List Port
deriving DecidableEq, Repr

/-- The contract revision checked by this model. -/
def contractVersion : Nat := 1

/-- Number of ports declared by an agent kind. -/
def arity : AgentKind → Nat
  | .input _ => 1
  | .const _ => 1
  | .add => 3
  | .sub => 3
  | .mul => 3
  | .eq => 3
  | .dup => 3
  | .era => 1
  | .output => 1

/-- A readable port constructor used by the finite fixtures. -/
def port (agent slot : Nat) : Port := ⟨agent, slot⟩

/-- The kind at an agent identifier, if the identifier is declared. -/
def kindAtAux : List AgentKind → Nat → Option AgentKind
  | [], _ => none
  | kind :: _, 0 => some kind
  | _ :: rest, agent + 1 => kindAtAux rest agent

def kindAt (net : Net) (agent : Nat) : Option AgentKind := kindAtAux net.agents agent

/-- The ports belonging to one declared agent. -/
def portsForAgent (agent : Nat) (kind : AgentKind) : List Port :=
  (List.range (arity kind)).map (port agent)

/-- Enumerate every declared port, preserving agent and port order. -/
def allPortsAux : Nat → List AgentKind → List Port
  | _, [] => []
  | agent, kind :: rest => portsForAgent agent kind ++ allPortsAux (agent + 1) rest

/-- The finite port list against which all association obligations are checked. -/
def allPorts (net : Net) : List Port := allPortsAux 0 net.agents

/-- Natural-number arity validation for a port. -/
def validPort (net : Net) (candidate : Port) : Bool :=
  match kindAt net candidate.agent with
  | some kind => candidate.slot < arity kind
  | none => false

/-- Principal/auxiliary classification.  Port `0` is principal; every other
port is auxiliary.  The checker separately verifies that the port is in range.
-/
def isAuxiliary (candidate : Port) : Bool := candidate.slot != 0

/-- The source-order slot carried by an input agent. -/
def inputSlot (net : Net) (candidate : Port) : Option Nat :=
  match kindAt net candidate.agent with
  | some (.input slot) => some slot
  | _ => none

/-- The ordered operand index carried by a binary agent's auxiliary port. -/
def operandSlot (net : Net) (candidate : Port) : Option Nat :=
  match kindAt net candidate.agent with
  | some kind =>
      match kind, candidate.slot with
      | .add, 1 => some 0
      | .add, 2 => some 1
      | .sub, 1 => some 0
      | .sub, 2 => some 1
      | .mul, 1 => some 0
      | .mul, 2 => some 1
      | .eq, 1 => some 0
      | .eq, 2 => some 1
      | _, _ => none
  | none => none

/-- Compatibility of the two ends of one directed association entry.  Ordered
input/operand links are checked by their semantic slot; all other principal /
auxiliary links use the declared interaction-net shape and reject auxiliary /
auxiliary links.
-/
def roleCompatible (net : Net) (left right : Port) : Bool :=
  match inputSlot net left, operandSlot net right with
  | some input, some operand => input == operand
  | some _, none => true
  | none, some _ => true
  | none, none => !(isAuxiliary left && isAuxiliary right)

/-- A directed association entry is well-typed when both ports are declared
and its principal/auxiliary and ordered-operand roles agree. -/
def edgeCompatibleBool (net : Net) (edge : Port × Port) : Bool :=
  validPort net edge.1 && validPort net edge.2 && roleCompatible net edge.1 edge.2

def edgeCompatible (net : Net) (edge : Port × Port) : Prop :=
  edgeCompatibleBool net edge = true

/-- Every association entry obeys the port-kind contract. -/
def WiringCompatible (net : Net) : Prop :=
  ∀ edge ∈ net.wiring, edgeCompatible net edge

/-- The ports incident to a source in the association list. -/
def incidence (wiring : List (Port × Port)) (candidate : Port) : List Port :=
  wiring.filterMap fun edge => if edge.1 = candidate then some edge.2 else none

/-- One port's reciprocal obligation.  The singleton shape states unique
incidence; the second lookup states reciprocity; the inequality rules out a
fixed point.
-/
def reciprocalCheckAt (net : Net) (candidate : Port) : Bool :=
  match incidence net.wiring candidate with
  | [partner] => (partner != candidate) && (incidence net.wiring partner == [candidate])
  | _ => false

def reciprocalAt (net : Net) (candidate : Port) : Prop :=
  reciprocalCheckAt net candidate = true

/-- The wiring is a fixed-point-free involution with one incidence per port. -/
def Reciprocal (net : Net) : Prop :=
  ∀ candidate ∈ allPorts net, reciprocalAt net candidate

/-- Boolean checker for the finite reciprocal obligation. -/
def reciprocityOk (net : Net) : Bool :=
  (allPorts net).all (reciprocalCheckAt net)

/-- Boolean checker for the finite edge-kind obligation. -/
def compatibilityOk (net : Net) : Bool :=
  net.wiring.all (edgeCompatibleBool net)

/-- The output principal ports, in the order induced by the declaration. -/
def expectedBoundary (net : Net) : List Port :=
  (allPorts net).filter fun candidate =>
    match kindAt net candidate.agent with
    | some .output => candidate.slot == 0
    | _ => false

/-- Exact ordered boundary equality. -/
def BoundaryExact (net : Net) : Prop := net.boundary = expectedBoundary net

/-- Boolean checker for the exact ordered output boundary. -/
def boundaryOk (net : Net) : Bool :=
  if net.boundary = expectedBoundary net then true else false

/-- The declaration must select this version of the net contract. -/
def Versioned (net : Net) : Prop := net.version = contractVersion

/-- Boolean checker for the selected contract revision. -/
def versionOk (net : Net) : Bool := net.version == contractVersion

/-- The validator's explicit finite obligations. -/
def obligations (net : Net) : List Bool :=
  [versionOk net, reciprocityOk net, compatibilityOk net, boundaryOk net]

/-- A net is accepted precisely when every declared obligation is true. -/
def validate (net : Net) : Bool := (obligations net).all id

/-- Acceptance of the finite reciprocal checker yields the stated involution,
unique-incidence, and no-fixed-point property. -/
theorem reciprocity_ok_sound (net : Net) (checked : reciprocityOk net = true) :
    Reciprocal net := by
  intro candidate member
  unfold reciprocityOk at checked
  change reciprocalCheckAt net candidate = true
  exact List.all_eq_true.mp checked candidate member

/-- Acceptance of the revision checker yields the selected contract version. -/
theorem version_ok_sound (net : Net) (checked : versionOk net = true) :
    Versioned net := by
  exact beq_iff_eq.mp checked

/-- Acceptance of the finite edge checker yields valid ports and compatible
principal/auxiliary and ordered-operand roles for every association entry. -/
theorem compatibility_ok_sound (net : Net) (checked : compatibilityOk net = true) :
    WiringCompatible net := by
  intro edge member
  unfold compatibilityOk at checked
  change edgeCompatibleBool net edge = true
  exact List.all_eq_true.mp checked edge member

/-- Acceptance of the boundary checker yields exactly the declared ordered
output ports. -/
theorem boundary_ok_sound (net : Net) (checked : boundaryOk net = true) :
    BoundaryExact net := by
  unfold boundaryOk at checked
  unfold BoundaryExact
  split at checked
  · exact ‹net.boundary = expectedBoundary net›
  · simp at checked

/-- Main validator soundness: acceptance implies reciprocal unique-incidence
wiring without fixed points, compatible connected port kinds, and the exact
ordered output boundary.
-/
theorem validate_sound (net : Net) (checked : validate net = true) :
    Versioned net ∧ Reciprocal net ∧ WiringCompatible net ∧ BoundaryExact net := by
  have allChecked : (obligations net).all id = true := by
    exact checked
  have hversion : versionOk net = true :=
    List.all_eq_true.mp allChecked (versionOk net) (by simp [obligations])
  have hrec : reciprocityOk net = true :=
    List.all_eq_true.mp allChecked (reciprocityOk net) (by simp [obligations])
  have hcompat : compatibilityOk net = true :=
    List.all_eq_true.mp allChecked (compatibilityOk net) (by simp [obligations])
  have hboundary : boundaryOk net = true :=
    List.all_eq_true.mp allChecked (boundaryOk net) (by simp [obligations])
  exact ⟨version_ok_sound net hversion, reciprocity_ok_sound net hrec,
    compatibility_ok_sound net hcompat, boundary_ok_sound net hboundary⟩

/-! ## Finite fixtures

The first fixture is the structural net emitted for the expression `x - y`:
input `0` is the deeper operand, input `1` is the top operand, subtraction's
ordered ports `1` and `2` receive them, and its principal result reaches the
single output.  The remaining fixtures are refusal controls for the same
contract: swapped operand ports, a missing reciprocal entry, duplicate source
incidence, and a permuted two-output boundary.
-/

def deeperMinusTop : Net where
  version := contractVersion
  agents := [.input 0, .input 1, .sub, .output]
  wiring :=
    [ (port 0 0, port 2 1), (port 2 1, port 0 0)
    , (port 1 0, port 2 2), (port 2 2, port 1 0)
    , (port 2 0, port 3 0), (port 3 0, port 2 0) ]
  boundary := [port 3 0]

def swappedSubOperands : Net where
  version := contractVersion
  agents := deeperMinusTop.agents
  wiring :=
    [ (port 0 0, port 2 2), (port 2 2, port 0 0)
    , (port 1 0, port 2 1), (port 2 1, port 1 0)
    , (port 2 0, port 3 0), (port 3 0, port 2 0) ]
  boundary := deeperMinusTop.boundary

def danglingWire : Net where
  version := contractVersion
  agents := deeperMinusTop.agents
  wiring :=
    [ (port 0 0, port 2 1), (port 2 1, port 0 0)
    , (port 1 0, port 2 2)
    , (port 2 0, port 3 0), (port 3 0, port 2 0) ]
  boundary := deeperMinusTop.boundary

def aliasedIncidence : Net where
  version := contractVersion
  agents := deeperMinusTop.agents
  wiring := deeperMinusTop.wiring ++ [(port 0 0, port 2 1)]
  boundary := deeperMinusTop.boundary

def twoOutputNet : Net where
  version := contractVersion
  agents := [.input 0, .input 1, .sub, .output, .output, .const 0]
  wiring :=
    [ (port 0 0, port 2 1), (port 2 1, port 0 0)
    , (port 1 0, port 2 2), (port 2 2, port 1 0)
    , (port 2 0, port 3 0), (port 3 0, port 2 0)
    , (port 4 0, port 5 0), (port 5 0, port 4 0) ]
  boundary := [port 3 0, port 4 0]

def wrongOutputOrder : Net where
  version := contractVersion
  agents := twoOutputNet.agents
  wiring := twoOutputNet.wiring
  boundary := [port 4 0, port 3 0]

/-- The compiled deeper-minus-top net is accepted by the structural validator. -/
theorem deeper_minus_top_is_accepted : validate deeperMinusTop = true := by
  decide

/-- A subtraction net with its ordered operands swapped is rejected. -/
theorem swapped_sub_operands_are_rejected : validate swappedSubOperands = false := by
  decide

/-- A net with a dangling, non-reciprocal association is rejected. -/
theorem dangling_wire_is_rejected : validate danglingWire = false := by
  decide

/-- A net with two associations for one source port is rejected as non-unique. -/
theorem aliased_incidence_is_rejected : validate aliasedIncidence = false := by
  decide

/-- The ordered two-output fixture establishes the expected boundary order. -/
theorem two_output_boundary_is_accepted : validate twoOutputNet = true := by
  decide

/-- Permuting the declared output ports is rejected even when the wiring is
otherwise unchanged.
-/
theorem wrong_output_order_is_rejected : validate wrongOutputOrder = false := by
  decide

end NobleFoundation.InteractionNet
