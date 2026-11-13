import NobleFoundation.PackedSchedule

/-!
# Bounded sequential belt model

This file models symbolic register identities carried by a finite newest-first belt
and explicit preservation slots. The graph is the total `I64`/`Bool` fragment in
`PackedSchedule`; this theorem establishes the checker-to-reference relation, not
Rust code refinement, machine timing, or hardware behavior.
-/

namespace NobleFoundation.Belt

open NobleFoundation.PackedSchedule

/-- One target-neutral belt instruction. Age zero denotes the newest value. -/
inductive Instr where
  | input (slot : Nat)
  | constant (bits : BitVec 64)
  | add (leftAge rightAge : Nat)
  | sub (leftAge rightAge : Nat)
  | mul (leftAge rightAge : Nat)
  | eq (leftAge rightAge : Nat)
  | save (age slot : Nat)
  | load (slot : Nat)
  | output (age : Nat)
deriving DecidableEq

/-- Symbolic machine state: each belt position and slot names its graph node. -/
structure State where
  belt : List Nat
  saves : List (Option Nat)
  nextNode : Nat
  nextOutput : Nat
  outputNodes : List Nat
deriving DecidableEq

/-- Replace an existing list position, refusing an out-of-range slot. -/
def setAt {α : Type} : List α → Nat → α → Option (List α)
  | [], _, _ => none
  | _ :: rest, 0, replacement => some (replacement :: rest)
  | first :: rest, Nat.succ index, replacement =>
      (setAt rest index replacement).map (fun changed => first :: changed)

/-- Whether a node is read by a future definition or remaining output. -/
def liveAfter (graph : Graph) (outputs : List Nat) (nextNode nextOutput node : Nat) : Bool :=
  (((List.range graph.arity).drop nextNode).any fun index =>
      (graph.node index).operands.contains node) ||
    (outputs.drop nextOutput).contains node

/-- Insert a new value and refuse an eviction that loses a live node. -/
def insertNode (graph : Graph) (outputs : List Nat) (capacity : Nat)
    (state : State) (node : Nat) : Option State :=
  if state.belt.length < capacity then
    some { state with belt := node :: state.belt }
  else
    match state.belt.getLast? with
    | none => none
    | some evicted =>
        if liveAfter graph outputs state.nextNode state.nextOutput evicted &&
            !(state.saves.contains (some evicted)) then
          none
        else
          some { state with belt := (node :: state.belt).take capacity }

/-- Define the next canonical graph node and record its symbolic identity. -/
def defineNode (graph : Graph) (outputs : List Nat) (capacity : Nat)
    (state : State) : Option State :=
  if state.nextNode < graph.arity then
    let target := state.nextNode
    let advanced := { state with nextNode := target + 1 }
    insertNode graph outputs capacity advanced target
  else
    none

/-- One checked symbolic machine transition. -/
def stepBody (graph : Graph) (outputs : List Nat) (capacity : Nat)
    (state : State) : Instr → Option State
  | .input slot =>
      match graph.node state.nextNode with
      | .input expected =>
          if slot == expected then defineNode graph outputs capacity state else none
      | _ => none
  | .constant bits =>
      match graph.node state.nextNode with
      | .constant expected =>
          if bits == expected then defineNode graph outputs capacity state else none
      | _ => none
  | .add leftAge rightAge =>
      match graph.node state.nextNode with
      | .add left right =>
          match state.belt.get? leftAge, state.belt.get? rightAge with
          | some actualLeft, some actualRight =>
              if actualLeft == left && actualRight == right then
                defineNode graph outputs capacity state
              else none
          | _, _ => none
      | _ => none
  | .sub leftAge rightAge =>
      match graph.node state.nextNode with
      | .sub left right =>
          match state.belt.get? leftAge, state.belt.get? rightAge with
          | some actualLeft, some actualRight =>
              if actualLeft == left && actualRight == right then
                defineNode graph outputs capacity state
              else none
          | _, _ => none
      | _ => none
  | .mul leftAge rightAge =>
      match graph.node state.nextNode with
      | .mul left right =>
          match state.belt.get? leftAge, state.belt.get? rightAge with
          | some actualLeft, some actualRight =>
              if actualLeft == left && actualRight == right then
                defineNode graph outputs capacity state
              else none
          | _, _ => none
      | _ => none
  | .eq leftAge rightAge =>
      match graph.node state.nextNode with
      | .eq left right =>
          match state.belt.get? leftAge, state.belt.get? rightAge with
          | some actualLeft, some actualRight =>
              if actualLeft == left && actualRight == right then
                defineNode graph outputs capacity state
              else none
          | _, _ => none
      | _ => none
  | .save age slot =>
      match state.belt.get? age, state.saves.get? slot with
      | some node, some previous =>
          let otherSlots := state.saves.take slot ++ state.saves.drop (slot + 1)
          let overwritesLive := match previous with
            | none => false
            | some old =>
                liveAfter graph outputs state.nextNode state.nextOutput old &&
                  !(state.belt.contains old || otherSlots.contains (some old))
          if overwritesLive then none else
            (setAt state.saves slot (some node)).map fun saves =>
              { state with saves := saves }
      | _, _ => none
  | .load slot =>
      match state.saves.get? slot with
      | some (some node) => insertNode graph outputs capacity state node
      | _ => none
  | .output age =>
      if state.nextNode != graph.arity then none else
        match outputs.get? state.nextOutput, state.belt.get? age with
        | some expected, some actual =>
            if expected == actual then
              some { state with
                nextOutput := state.nextOutput + 1
                outputNodes := state.outputNodes ++ [actual] }
            else none
        | _, _ => none

/-- Completed programs reject every trailing instruction. -/
def step (graph : Graph) (outputs : List Nat) (capacity : Nat)
    (state : State) (instruction : Instr) : Option State :=
  if state.nextNode == graph.arity && state.nextOutput == outputs.length then
    none
  else
    stepBody graph outputs capacity state instruction

/-- Consume a complete instruction list, refusing on the first invalid transition. -/
def run (graph : Graph) (outputs : List Nat) (capacity : Nat) :
    List Instr → State → Option State
  | [], state => some state
  | instruction :: rest, state =>
      match step graph outputs capacity state instruction with
      | none => none
      | some next => run graph outputs capacity rest next

/-- Finite well-formedness and I64-input typing checks for the model fragment. -/
def graphAdmissible (graph : Graph) : Bool :=
  decide (graph.output < graph.arity) &&
    (List.range graph.arity).all fun index =>
      ((graph.node index).operands).all fun operand => decide (operand < index)

/-- Node result type induced by the restricted graph vocabulary. -/
inductive Ty where
  | i64
  | boolean
deriving DecidableEq

def nodeTy : Node → Ty
  | .eq _ _ => .boolean
  | _ => .i64

/-- Every graph operand is I64; equality is the sole Bool-producing node. -/
def graphTyped (graph : Graph) : Bool :=
  (List.range graph.arity).all fun index =>
    ((graph.node index).operands).all fun operand =>
      decide (nodeTy (graph.node operand) = .i64)

/-- Completion requires every node and every requested output in exact order. -/
def complete (graph : Graph) (outputs : List Nat) (state : State) : Bool :=
  decide (state.nextNode = graph.arity) &&
    decide (state.nextOutput = outputs.length) &&
    decide (state.outputNodes = outputs)

/-- Fixed limits of the proof model, matching the selected Rust backend bounds. -/
def boundsValid (capacity saveSlots : Nat) : Bool :=
  decide (2 ≤ capacity ∧ capacity ≤ 64 ∧ saveSlots ≤ 8)

/-- Independently validate graph, bounds, node order, belt ages, slots and outputs. -/
def validate (graph : Graph) (outputs : List Nat) (capacity saveSlots : Nat)
    (program : List Instr) : Bool :=
  boundsValid capacity saveSlots && graphAdmissible graph && graphTyped graph &&
    outputs.all (fun output => decide (output < graph.arity)) &&
      match run graph outputs capacity program {
        belt := [], saves := List.replicate saveSlots none,
        nextNode := 0, nextOutput := 0, outputNodes := [] } with
      | none => false
      | some state => complete graph outputs state

/-- Outputs of the symbolic belt run, interpreted at their checked graph identities. -/
def runOutputs (graph : Graph) (inputs : Nat → BitVec 64) (outputs : List Nat)
    (capacity saveSlots : Nat)
    (program : List Instr) : Option (List (Option Val)) :=
  (run graph outputs capacity program {
    belt := [], saves := List.replicate saveSlots none,
    nextNode := 0, nextOutput := 0, outputNodes := [] }).map fun state =>
      state.outputNodes.map (NobleFoundation.PackedSchedule.value graph inputs)

/-- Any accepted canonical belt run returns exactly the graph-reference outputs. -/
theorem validate_sound (graph : Graph) (inputs : Nat → BitVec 64) (outputs : List Nat)
    (capacity saveSlots : Nat) (program : List Instr)
    (hvalid : validate graph outputs capacity saveSlots program = true) :
    runOutputs graph inputs outputs capacity saveSlots program =
      some (NobleFoundation.PackedSchedule.referenceOutputs graph inputs outputs) := by
  have hparts :
      boundsValid capacity saveSlots = true ∧
      graphAdmissible graph = true ∧
      graphTyped graph = true ∧
      outputs.all (fun output => decide (output < graph.arity)) = true ∧
      (match run graph outputs capacity program {
        belt := [], saves := List.replicate saveSlots none,
        nextNode := 0, nextOutput := 0, outputNodes := [] } with
        | none => false
        | some state => complete graph outputs state) = true := by
    simpa only [validate, Bool.and_eq_true] using hvalid
  cases hrun : run graph outputs capacity program {
      belt := [], saves := List.replicate saveSlots none,
      nextNode := 0, nextOutput := 0, outputNodes := [] } with
  | none => simp [hrun] at hparts
  | some state =>
      have hcomplete : complete graph outputs state = true := by
        simpa only [hrun] using hparts.2.2.2.2
      have hcompleteParts := Bool.and_eq_true.mp hcomplete
      have houtputs : decide (state.outputNodes = outputs) = true :=
        (Bool.and_eq_true.mp hcompleteParts.2).2
      have hnodeOrder : state.outputNodes = outputs := of_decide_eq_true houtputs
      simp [runOutputs, hrun, hnodeOrder,
        NobleFoundation.PackedSchedule.referenceOutputs]

/-- Tiny checked graph with an asymmetric subtraction and two I64 inputs. -/
def controlGraph : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else .sub 1 0

def controlProgram : List Instr :=
  [.input 0, .input 1, .sub 0 1, .output 0]

example : validate controlGraph [2] 2 0 controlProgram = true := by decide

example : validate controlGraph [2] 2 0
    [.input 0, .input 1, .sub 1 0, .output 0] = false := by decide

example : validate controlGraph [2] 2 0
    [.input 0, .input 1, .sub 2 1, .output 0] = false := by decide

example : validate controlGraph [2] 2 1
    [.load 0, .input 0, .input 1, .sub 0 1, .output 0] = false := by decide

example : validate controlGraph [2, 1] 2 0
    [.input 0, .input 1, .sub 0 1, .output 0, .output 0] = false := by decide

/-- A four-node graph whose first input must survive two later definitions. -/
def longLivedGraph : Graph where
  arity := 4
  output := 3
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else if index = 2 then .input 2
    else .add 0 2

example : validate longLivedGraph [3] 2 1
    [.input 0, .input 1, .save 1 0, .input 2, .load 0,
      .add 0 1, .output 0] = true := by decide

example : validate longLivedGraph [3] 2 0
    [.input 0, .input 1, .input 2, .add 0 1, .output 0] = false := by decide

example : validate longLivedGraph [3] 2 1
    [.input 0, .input 1, .save 1 0, .input 2, .save 0 0,
      .load 0, .add 0 1, .output 0] = false := by decide

end NobleFoundation.Belt
