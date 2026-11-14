import NobleFoundation.PackedSchedule

/-!
Timed target-schedule model for #114. This models only two fixed descriptor records, issue cycles,
latency-derived completion, issue resources, graph-node references, and ordered final reads. Event
operand identities are the graph nodes named by each operation; wire operand ages and Rust artifact
bytes are checked/tested separately. The model and theorem do not establish Rust refinement or
hardware behavior.
-/
namespace NobleFoundation.BeltTarget

open NobleFoundation.PackedSchedule

inductive Target where
  | t1
  | t2
  deriving DecidableEq, Repr

inductive UnitClass where
  | alu
  | multiply
  | move
  deriving DecidableEq, Repr

structure Descriptor where
  issueWidth : Nat
  beltCapacity : Nat
  saveSlots : Nat
  aluUnits : Nat
  multiplyUnits : Nat
  moveUnits : Nat
  aluLatency : Nat
  multiplyLatency : Nat
  moveLatency : Nat
  initiationInterval : Nat
  deriving DecidableEq, Repr

def descriptor : Target → Descriptor
  | .t1 => ⟨1, 4, 8, 1, 1, 1, 1, 3, 1, 1⟩
  | .t2 => ⟨2, 8, 8, 2, 1, 2, 1, 2, 1, 1⟩

inductive Operation where
  | input (slot : Nat)
  | constant (bits : BitVec 64)
  | add (left right leftAge rightAge : Nat)
  | sub (left right leftAge rightAge : Nat)
  | multiply (left right leftAge rightAge : Nat)
  | equal (left right leftAge rightAge : Nat)
  | save (age slot : Nat)
  | load (slot : Nat)
  deriving DecidableEq, Repr

structure Issue where
  cycle : Nat
  lane : Nat
  unit : Nat
  node : Nat
  operation : Operation
  deriving DecidableEq, Repr

structure Schedule where
  target : Target
  horizon : Nat
  events : List Issue
  outputs : List Nat
  deriving DecidableEq, Repr

/-- Completion is at the end of issue cycle + latency - 1. -/
def completionCycle (target : Target) (event : Issue) : Nat :=
  event.cycle + (match event.operation with
    | .input _ | .constant _ | .save _ _ | .load _ => (descriptor target).moveLatency
    | .multiply _ _ _ _ => (descriptor target).multiplyLatency
    | .add _ _ _ _ | .sub _ _ _ _ | .equal _ _ _ _ => (descriptor target).aluLatency) - 1

def operationClass : Operation → UnitClass
  | .input _ | .constant _ | .save _ _ | .load _ => .move
  | .multiply _ _ _ _ => .multiply
  | .add _ _ _ _ | .sub _ _ _ _ | .equal _ _ _ _ => .alu

def unitClass (target : Target) (unit : Nat) : Option UnitClass :=
  let machine := descriptor target
  if unit < machine.aluUnits then some .alu
  else if unit < machine.aluUnits + machine.multiplyUnits then some .multiply
  else if unit < machine.aluUnits + machine.multiplyUnits + machine.moveUnits then some .move
  else none

def isDefinition : Operation → Bool
  | .input _ | .constant _ | .add _ _ _ _ | .sub _ _ _ _ | .multiply _ _ _ _ | .equal _ _ _ _ => true
  | .save _ _ | .load _ => false

def operationOperands (event : Issue) : List Nat :=
  match event.operation with
  | .add left right _ _ | .sub left right _ _ | .multiply left right _ _ | .equal left right _ _ =>
      [left, right]
  | .save _ _ => [event.node]
  | .input _ | .constant _ | .load _ => []

def operationMatches (graph : Graph) (event : Issue) : Bool :=
  if event.node ≥ graph.arity then false else
  match event.operation, graph.node event.node with
  | .input slot, .input graphSlot => decide (slot = graphSlot)
  | .constant bits, .constant graphBits => decide (bits = graphBits)
  | .add left right _ _, .add graphLeft graphRight => decide (left = graphLeft ∧ right = graphRight)
  | .sub left right _ _, .sub graphLeft graphRight => decide (left = graphLeft ∧ right = graphRight)
  | .multiply left right _ _, .mul graphLeft graphRight => decide (left = graphLeft ∧ right = graphRight)
  | .equal left right _ _, .eq graphLeft graphRight => decide (left = graphLeft ∧ right = graphRight)
  | .save _ _, _ | .load _, _ => true
  | _, _ => false

def issueSorted : List Issue → Bool
  | [] | [_] => true
  | first :: second :: rest => decide (first.cycle ≤ second.cycle) && issueSorted (second :: rest)

def uniqueLaneAndUnit (events : List Issue) : Bool :=
  events.all fun event =>
    (events.filter fun other => other.cycle = event.cycle && other.lane = event.lane).length = 1 ∧
      (events.filter fun other => other.cycle = event.cycle && other.unit = event.unit).length = 1

def definitionCount (events : List Issue) (node : Nat) : Nat :=
  (events.filter fun event => event.node = node && isDefinition event.operation).length

def definitionCoverage (graph : Graph) (events : List Issue) : Bool :=
  (List.range graph.arity).all fun node => definitionCount events node = 1
    && events.all (fun event => !isDefinition event.operation || event.node < graph.arity)

def dependenciesReady (target : Target) (events : List Issue) (event : Issue) : Bool :=
  (operationOperands event).all fun operand =>
    events.any fun producer =>
      producer.node = operand && isDefinition producer.operation &&
        completionCycle target producer < event.cycle

def saveReadsReady (target : Target) (events : List Issue) (event : Issue) : Bool :=
  match event.operation with
  | .load slot =>
      events.any fun saved =>
        match saved.operation with
        | .save _ savedSlot => saved.node = event.node && savedSlot = slot &&
            completionCycle target saved < event.cycle
        | _ => false
  | .input _ | .constant _ | .add _ _ _ _ | .sub _ _ _ _ | .multiply _ _ _ _ |
    .equal _ _ _ _ | .save _ _ => true

def agesAndSlotsBounded (target : Target) (event : Issue) : Bool :=
  let machine := descriptor target
  match event.operation with
  | .add _ _ leftAge rightAge | .sub _ _ leftAge rightAge |
    .multiply _ _ leftAge rightAge | .equal _ _ leftAge rightAge =>
      leftAge < machine.beltCapacity && rightAge < machine.beltCapacity
  | .save age slot => age < machine.beltCapacity && slot < machine.saveSlots
  | .load slot => slot < machine.saveSlots
  | .input _ | .constant _ => true

def eventLegal (graph : Graph) (target : Target) (events : List Issue) (horizon : Nat)
    (event : Issue) : Bool :=
  let machine := descriptor target
  event.lane < machine.issueWidth &&
    unitClass target event.unit = some (operationClass event.operation) &&
    operationMatches graph event && agesAndSlotsBounded target event &&
    dependenciesReady target events event && saveReadsReady target events event &&
    completionCycle target event ≤ horizon

def graphAdmissible (graph : Graph) : Bool :=
  0 < graph.arity && graph.arity ≤ 4096 && graph.output < graph.arity &&
    (List.range graph.arity).all fun index =>
      (graph.node index).operands.all fun operand => operand < index

def outputsReady (graph : Graph) (target : Target) (events : List Issue)
    (horizon : Nat) (outputs : List Nat) : Bool :=
  outputs.all fun output =>
    output < graph.arity && events.any fun event =>
      event.node = output && isDefinition event.operation && completionCycle target event ≤ horizon

def baseChecks (graph : Graph) (target : Target) (schedule : Schedule) : Bool :=
  schedule.target = target && graphAdmissible graph &&
    schedule.events.length ≤ 65536 && schedule.horizon ≤ 65536 &&
    issueSorted schedule.events && uniqueLaneAndUnit schedule.events &&
    definitionCoverage graph schedule.events &&
    schedule.events.all (eventLegal graph target schedule.events schedule.horizon) &&
    outputsReady graph target schedule.events schedule.horizon schedule.outputs

/-- The checker additionally requires the requested output sequence verbatim. -/
def validate (graph : Graph) (target : Target) (outputs : List Nat) (schedule : Schedule) : Bool :=
  baseChecks graph target schedule && decide (schedule.outputs = outputs)

/-- Deterministic completion key: completion cycle, issue cycle, then physical unit. -/
def completionBefore (target : Target) (left right : Issue) : Bool :=
  let leftCompletion := completionCycle target left
  let rightCompletion := completionCycle target right
  leftCompletion < rightCompletion ||
    (leftCompletion = rightCompletion &&
      (left.cycle < right.cycle || (left.cycle = right.cycle && left.unit ≤ right.unit)))

def insertCompletion (target : Target) (event : Issue) : List Issue → List Issue
  | [] => [event]
  | first :: rest =>
      if completionBefore target event first then event :: first :: rest
      else first :: insertCompletion target event rest

def completionOrder (target : Target) (events : List Issue) : List Issue :=
  events.foldl (fun ordered event => insertCompletion target event ordered) []

def completingAt (target : Target) (cycle : Nat) (events : List Issue) : List Issue :=
  (completionOrder target events).filter fun event => completionCycle target event = cycle

/-- The symbolic timed run reads completed graph values in the declared output order. -/
def runOutputs (graph : Graph) (inputs : Nat → BitVec 64) (schedule : Schedule) : List (Option Val) :=
  schedule.outputs.map (value graph inputs)

/-- Every accepted timed schedule returns exactly the requested graph-reference values.
This is a theorem about the symbolic target model, not Rust refinement. -/
theorem validate_sound (graph : Graph) (inputs : Nat → BitVec 64) (outputs : List Nat)
    (target : Target) (schedule : Schedule)
    (hvalid : validate graph target outputs schedule = true) :
    runOutputs graph inputs schedule = referenceOutputs graph inputs outputs := by
  have hparts : baseChecks graph target schedule = true ∧
      decide (schedule.outputs = outputs) = true := by
    simpa only [validate, Bool.and_eq_true] using hvalid
  have houtputs : schedule.outputs = outputs := of_decide_eq_true hparts.2
  simp [runOutputs, houtputs, referenceOutputs]

example : descriptor .t1 = ⟨1, 4, 8, 1, 1, 1, 1, 3, 1, 1⟩ := by decide
example : descriptor .t2 = ⟨2, 8, 8, 2, 1, 2, 1, 2, 1, 1⟩ := by decide
example : completionCycle .t1 ⟨4, 0, 1, 0, .multiply 0 0 0 1⟩ = 6 := by decide
example : completionCycle .t2 ⟨4, 0, 2, 0, .multiply 0 0 0 1⟩ = 5 := by decide

end NobleFoundation.BeltTarget
