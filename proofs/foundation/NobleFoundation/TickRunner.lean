/-!
# Bounded sequential I64 tick protocol (issue #128)

A self-contained finite-DAG reference and single-stream protocol model. Nodes have only
I64 input, exact constant, wrapping add/sub/mul; their operands must precede them.
The ordered interface is [state, input, control] -> [next state, visible output].
A tick selects one entry snapshot; a failed operation returns the *whole* old runner.
The model stores one current output and a logical commit count, not a trace archive.

The proved statements below concern this model only. There is no theorem connecting
Rust graph admission, emitted schedules, evaluator storage, hashes or host cancellation
to this model. The sequential cancellation flag is checked before evaluation; there
is no mid-evaluation interruption, concurrent update, effect, migration or deadline.
-/

namespace NobleFoundation.TickRunner

abbrev Word := BitVec 64

/-- I64-only graph nodes; earlier indices are the only legal node operands. -/
inductive Node where
  | input (slot : Nat)
  | constant (word : Word)
  | add (left right : Nat)
  | sub (left right : Nat)
  | mul (left right : Nat)
deriving DecidableEq, Repr

/-- The ordered graph interface and its selected output node indices. -/
structure Graph where
  inputWords : Nat
  nodes : List Node
  outputs : List Nat
deriving DecidableEq, Repr

/-- Exact, contiguous ordered I64 slot partitions. -/
structure Partition where
  stateWords : Nat
  inputWords : Nat
  controlWords : Nat
  outputWords : Nat
deriving DecidableEq, Repr

/-- Bounds used in this finite model. Byte/trace quotas and host cleanup are external. -/
structure Limits where
  maxNodes : Nat
  maxStateWords : Nat
  maxInputWords : Nat
  maxControlWords : Nat
  maxOutputWords : Nat
  maxTicks : Nat
  maxEpoch : Nat
  maxWorkPerTick : Nat
deriving DecidableEq, Repr

/-- Opaque caller-selected artifact labels: equality here does not authenticate bytes. -/
structure Ids where
  graph : Nat
  controls : Nat
deriving DecidableEq, Repr

/-- An actual node may only read an interface input or earlier node indices. -/
def Node.scoped (node : Node) (index inputWords : Nat) : Bool :=
  match node with
  | .input slot => decide (slot < inputWords)
  | .constant _ => true
  | .add left right => decide (left < index ∧ right < index)
  | .sub left right => decide (left < index ∧ right < index)
  | .mul left right => decide (left < index ∧ right < index)

/-- Check actual nodes and all ordered output positions, not a producer's summary. -/
def Graph.scoped (graph : Graph) : Bool :=
  (List.range graph.nodes.length).all (fun index =>
    match graph.nodes[index]? with
    | none => false
    | some node => node.scoped index graph.inputWords) &&
  graph.outputs.all (fun index => decide (index < graph.nodes.length))

/-- Every declared interface input is a unique position in its exact slot order,
    and later nodes cannot introduce a second, unbound input. -/
def Graph.orderedInputs (graph : Graph) : Bool :=
  decide (graph.nodes.take graph.inputWords =
    (List.range graph.inputWords).map Node.input) &&
  (graph.nodes.drop graph.inputWords).all (fun node =>
    match node with
    | .input _ => false
    | _ => true)

/-- Check a graph/partition and the actual initial state and controls, with word/work caps. -/
def eligible (graph : Graph) (partition : Partition) (state controls : List Word)
    (limits : Limits) : Bool :=
  graph.scoped && graph.orderedInputs &&
  decide (graph.inputWords = partition.stateWords + partition.inputWords +
    partition.controlWords) &&
  decide (graph.outputs.length = partition.stateWords + partition.outputWords) &&
  decide (state.length = partition.stateWords ∧ controls.length = partition.controlWords) &&
  decide (graph.nodes.length ≤ limits.maxNodes ∧
    graph.nodes.length ≤ limits.maxWorkPerTick ∧
    partition.stateWords ≤ limits.maxStateWords ∧
    partition.inputWords ≤ limits.maxInputWords ∧
    partition.controlWords ≤ limits.maxControlWords ∧
    partition.outputWords ≤ limits.maxOutputWords)

/-- Evaluate one node against only the values of its predecessors. -/
def evalNode (inputs registers : List Word) : Node → Option Word
  | .input slot => inputs[slot]?
  | .constant word => some word
  | .add left right => do
      let x ← registers[left]?
      let y ← registers[right]?
      pure (x + y)
  | .sub left right => do
      let x ← registers[left]?
      let y ← registers[right]?
      pure (x - y)
  | .mul left right => do
      let x ← registers[left]?
      let y ← registers[right]?
      pure (x * y)

/-- Reference DAG interpretation, in node order; no packed schedule is trusted. -/
def evalNodes (inputs : List Word) : List Node → List Word → Option (List Word)
  | [], registers => some registers
  | node :: rest, registers => do
      let word ← evalNode inputs registers node
      evalNodes inputs rest (registers ++ [word])

/-- Evaluate the complete ordered graph output, or refuse an absent node. -/
def evalGraph (graph : Graph) (inputs : List Word) : Option (List Word) := do
  let registers ← evalNodes inputs graph.nodes []
  graph.outputs.mapM (fun index => registers[index]?)

/-- One currently active stream. `commits` counts successful logical tick transitions. -/
structure Runner where
  graph : Graph
  partition : Partition
  limits : Limits
  state : List Word
  controls : List Word
  visibleOutput : List Word
  ids : Ids
  graphEpoch : Nat
  controlEpoch : Nat
  nextTick : Nat
  commits : Nat
deriving DecidableEq, Repr

/-- Prepare does not inherit any previous stream or previous result. -/
def prepare (ids : Ids) (graph : Graph) (partition : Partition)
    (state controls : List Word) (limits : Limits) : Option Runner :=
  if eligible graph partition state controls limits then
    some {
      graph := graph, partition := partition, limits := limits,
      state := state, controls := controls, visibleOutput := [], ids := ids,
      graphEpoch := 0, controlEpoch := 0, nextTick := 0, commits := 0
    }
  else none

/-- A tick ticket is bound to the active graph, controls, and next tick number. -/
structure Ticket where
  graphEpoch : Nat
  controlEpoch : Nat
  tick : Nat
deriving DecidableEq, Repr

/-- A control or reset request is bound to both currently active epochs. -/
structure Epochs where
  graph : Nat
  controls : Nat
deriving DecidableEq, Repr

/-- The complete immutable entry tuple selected for a logical tick. -/
structure Selection where
  graph : Graph
  partition : Partition
  graphArtifact : Nat
  controlArtifact : Nat
  graphEpoch : Nat
  controlEpoch : Nat
  tick : Nat
  state : List Word
  controls : List Word
deriving DecidableEq, Repr

/-- Select *once* at entry; evaluation below uses the same selected graph, state and controls. -/
def select (runner : Runner) : Selection :=
  { graph := runner.graph, partition := runner.partition,
    graphArtifact := runner.ids.graph, controlArtifact := runner.ids.controls,
    graphEpoch := runner.graphEpoch, controlEpoch := runner.controlEpoch,
    tick := runner.nextTick, state := runner.state, controls := runner.controls }

/-- Refusal classes in the sequential protocol (not the Rust error ABI). -/
inductive Refusal where
  | staleTick
  | staleGraph
  | staleControls
  | wrongInput
  | wrongPartition
  | cancelled
  | exhausted
  | evaluation
deriving DecidableEq, Repr

/-- The candidate has not published any state or output. -/
inductive Attempt where
  | refused (why : Refusal)
  | ready (nextState visibleOutput : List Word)
deriving DecidableEq, Repr

/-- Every guard precedes evaluation; the selected snapshot is the only semantic input. -/
def tickCandidate (runner : Runner) (ticket : Ticket) (input : List Word)
    (cancel : Bool) : Attempt :=
  let chosen := select runner
  if ticket.tick ≠ chosen.tick then .refused .staleTick
  else if ticket.graphEpoch ≠ chosen.graphEpoch then .refused .staleGraph
  else if ticket.controlEpoch ≠ chosen.controlEpoch then .refused .staleControls
  else if chosen.tick ≥ runner.limits.maxTicks then .refused .exhausted
  else if input.length ≠ chosen.partition.inputWords then .refused .wrongInput
  else if cancel then .refused .cancelled
  else
    match evalGraph chosen.graph (chosen.state ++ input ++ chosen.controls) with
    | none => .refused .evaluation
    | some values =>
        if values.length ≠ chosen.partition.stateWords + chosen.partition.outputWords then
          .refused .wrongPartition
        else .ready (values.take chosen.partition.stateWords)
          (values.drop chosen.partition.stateWords)

/-- A logical observation includes the unique entry tuple on a successful tick. -/
inductive Observation where
  | refused (why : Refusal)
  | committed (selected : Selection) (nextState visibleOutput : List Word)
  | controlsApplied (epoch : Nat)
  | resetApplied (graphEpoch controlEpoch : Nat)
deriving DecidableEq, Repr

structure Result where
  runner : Runner
  observation : Observation
deriving DecidableEq, Repr

/-- Atomic model publication: both state and visible output change with one commit count. -/
def commit (runner : Runner) (nextState output : List Word) : Runner :=
  { runner with
    state := nextState, visibleOutput := output,
    nextTick := runner.nextTick + 1, commits := runner.commits + 1 }

/-- A refused candidate returns the entire unchanged runner, without a partial output. -/
def tick (runner : Runner) (ticket : Ticket) (input : List Word)
    (cancel : Bool) : Result :=
  match tickCandidate runner ticket input cancel with
  | .refused why => ⟨runner, .refused why⟩
  | .ready nextState output =>
      ⟨commit runner nextState output, .committed (select runner) nextState output⟩

/-- Control admission checks both active epochs and the partition before activation. -/
def updateCheck (runner : Runner) (epochs : Epochs) (controls : List Word) : Option Refusal :=
  if epochs.graph ≠ runner.graphEpoch then some .staleGraph
  else if epochs.controls ≠ runner.controlEpoch then some .staleControls
  else if controls.length ≠ runner.partition.controlWords ∨
      controls.length > runner.limits.maxControlWords then some .wrongPartition
  else if runner.controlEpoch ≥ runner.limits.maxEpoch then some .exhausted
  else none

/-- Controls activate only between sequential calls, not during a running tick. -/
def updateControls (runner : Runner) (epochs : Epochs) (controls : List Word)
    (artifact : Nat) : Result :=
  match updateCheck runner epochs controls with
  | some why => ⟨runner, .refused why⟩
  | none =>
      ⟨{ runner with
          controls := controls,
          ids := { graph := runner.ids.graph, controls := artifact },
          controlEpoch := runner.controlEpoch + 1 },
        .controlsApplied (runner.controlEpoch + 1)⟩

/-- Reset is a separate admission, never a request to transfer the active state. -/
def resetCheck (runner : Runner) (epochs : Epochs) (graph : Graph)
    (partition : Partition) (state controls : List Word) : Option Refusal :=
  if epochs.graph ≠ runner.graphEpoch then some .staleGraph
  else if epochs.controls ≠ runner.controlEpoch then some .staleControls
  else if runner.graphEpoch ≥ runner.limits.maxEpoch ∨
      runner.controlEpoch ≥ runner.limits.maxEpoch then some .exhausted
  else if !eligible graph partition state controls runner.limits then some .wrongPartition
  else none

/-- Explicit new graph and separately checked state/controls, even for identical layouts. -/
def reset (runner : Runner) (epochs : Epochs) (ids : Ids) (graph : Graph)
    (partition : Partition) (state controls : List Word) : Result :=
  match resetCheck runner epochs graph partition state controls with
  | some why => ⟨runner, .refused why⟩
  | none =>
      ⟨{ runner with
          graph := graph, partition := partition, state := state,
          controls := controls, ids := ids, visibleOutput := [],
          graphEpoch := runner.graphEpoch + 1, controlEpoch := runner.controlEpoch + 1,
          nextTick := 0 },
        .resetApplied (runner.graphEpoch + 1) (runner.controlEpoch + 1)⟩

/-- A successful tick records exactly the single graph/control/state tuple selected at entry. -/
theorem tick_ready_pins_entry (runner : Runner) (ticket : Ticket) (input : List Word)
    (cancel : Bool) (nextState output : List Word)
    (ready : tickCandidate runner ticket input cancel = .ready nextState output) :
    (tick runner ticket input cancel).observation =
      .committed (select runner) nextState output := by
  simp [tick, ready]

/-- One successful tick publishes both values, advances the ticket once, and increments the
    logical commit count once. The graph/control tuple is not modified by the commit. -/
theorem tick_ready_commits_once (runner : Runner) (ticket : Ticket) (input : List Word)
    (cancel : Bool) (nextState output : List Word)
    (ready : tickCandidate runner ticket input cancel = .ready nextState output) :
    (tick runner ticket input cancel).runner = commit runner nextState output ∧
    (tick runner ticket input cancel).runner.state = nextState ∧
    (tick runner ticket input cancel).runner.visibleOutput = output ∧
    (tick runner ticket input cancel).runner.nextTick = runner.nextTick + 1 ∧
    (tick runner ticket input cancel).runner.commits = runner.commits + 1 ∧
    (tick runner ticket input cancel).runner.graph = runner.graph ∧
    (tick runner ticket input cancel).runner.controls = runner.controls ∧
    (tick runner ticket input cancel).runner.ids = runner.ids ∧
    (tick runner ticket input cancel).runner.graphEpoch = runner.graphEpoch ∧
    (tick runner ticket input cancel).runner.controlEpoch = runner.controlEpoch := by
  simp [tick, ready, commit]

/-- Any refused tick, including stale tickets, cancellation and failed evaluation, preserves
    *all* active data and counters, not merely the state words. -/
theorem tick_refusal_preserves_active (runner : Runner) (ticket : Ticket)
    (input : List Word) (cancel : Bool) (why : Refusal)
    (refused : (tick runner ticket input cancel).observation = .refused why) :
    (tick runner ticket input cancel).runner = runner := by
  cases attempt : tickCandidate runner ticket input cancel with
  | refused reason => simp [tick, attempt]
  | ready nextState output => simp [tick, attempt] at refused

/-- An accepted control update changes only controls, their label and control epoch;
    no tick, state, output or graph epoch is advanced by activation. -/
theorem update_success_keeps_state (runner : Runner) (epochs : Epochs)
    (controls : List Word) (artifact : Nat)
    (validUpdate : updateCheck runner epochs controls = none) :
    (updateControls runner epochs controls artifact).runner.state = runner.state ∧
    (updateControls runner epochs controls artifact).runner.visibleOutput = runner.visibleOutput ∧
    (updateControls runner epochs controls artifact).runner.nextTick = runner.nextTick ∧
    (updateControls runner epochs controls artifact).runner.graphEpoch = runner.graphEpoch ∧
    (updateControls runner epochs controls artifact).runner.controlEpoch =
      runner.controlEpoch + 1 ∧
    (updateControls runner epochs controls artifact).runner.controls = controls := by
  simp [updateControls, validUpdate]

/-- Rejected updates do not activate a partial control tuple. -/
theorem update_refusal_preserves_active (runner : Runner) (epochs : Epochs)
    (controls : List Word) (artifact : Nat) (why : Refusal)
    (refused : (updateControls runner epochs controls artifact).observation = .refused why) :
    (updateControls runner epochs controls artifact).runner = runner := by
  cases admission : updateCheck runner epochs controls with
  | none => simp [updateControls, admission] at refused
  | some reason => simp [updateControls, admission]

/-- Reset explicitly installs the supplied state, clears old output and resets the ticket;
    layout equality is not a migration permission. Both epochs advance together. -/
theorem reset_uses_separate_state (runner : Runner) (epochs : Epochs) (ids : Ids)
    (graph : Graph) (partition : Partition) (state controls : List Word)
    (validReset : resetCheck runner epochs graph partition state controls = none) :
    (reset runner epochs ids graph partition state controls).runner.state = state ∧
    (reset runner epochs ids graph partition state controls).runner.controls = controls ∧
    (reset runner epochs ids graph partition state controls).runner.visibleOutput = [] ∧
    (reset runner epochs ids graph partition state controls).runner.nextTick = 0 ∧
    (reset runner epochs ids graph partition state controls).runner.graphEpoch =
      runner.graphEpoch + 1 ∧
    (reset runner epochs ids graph partition state controls).runner.controlEpoch =
      runner.controlEpoch + 1 := by
  simp [reset, validReset]

/-- A failed reset cannot switch graph or smuggle old state into a replacement. -/
theorem reset_refusal_preserves_active (runner : Runner) (epochs : Epochs) (ids : Ids)
    (graph : Graph) (partition : Partition) (state controls : List Word) (why : Refusal)
    (refused : (reset runner epochs ids graph partition state controls).observation =
      .refused why) :
    (reset runner epochs ids graph partition state controls).runner = runner := by
  cases admission : resetCheck runner epochs graph partition state controls with
  | none => simp [reset, admission] at refused
  | some reason => simp [reset, admission]

/-! Concrete nonempty executions ensure the conditional claims have reachable premises. -/

def fixturePartition : Partition :=
  { stateWords := 1, inputWords := 1, controlWords := 1, outputWords := 1 }

def fixtureLimits : Limits :=
  { maxNodes := 8, maxStateWords := 2, maxInputWords := 2,
    maxControlWords := 2, maxOutputWords := 2, maxTicks := 8,
    maxEpoch := 8, maxWorkPerTick := 8 }

/-- s' = s + c*x, visible output s'; control is the third interface input. -/
def additive : Graph :=
  { inputWords := 3,
    nodes := [.input 0, .input 1, .input 2, .mul 1 2, .add 0 3],
    outputs := [4, 4] }

/-- Same layout, different meaning and ordered outputs: s' = s-x, out = s'-c. -/
def subtractive : Graph :=
  { inputWords := 3,
    nodes := [.input 0, .input 1, .input 2, .sub 0 1, .sub 3 2],
    outputs := [3, 4] }

def fixture : Runner :=
  { graph := additive, partition := fixturePartition, limits := fixtureLimits,
    state := [5], controls := [2], visibleOutput := [], ids := { graph := 17, controls := 19 },
    graphEpoch := 0, controlEpoch := 0, nextTick := 0, commits := 0 }

def firstTicket : Ticket := { graphEpoch := 0, controlEpoch := 0, tick := 0 }
def initialEpochs : Epochs := { graph := 0, controls := 0 }
def replacementIds : Ids := { graph := 23, controls := 29 }

theorem fixture_prepares :
    prepare { graph := 17, controls := 19 } additive fixturePartition [5] [2] fixtureLimits =
      some fixture := by
  decide

/-- Neither a permuted prefix nor an extra late Input is part of this profile,
    even though both may point to in-range, already declared interface slots. -/
theorem out_of_order_or_late_input_refuses :
    prepare { graph := 17, controls := 19 }
      { additive with
        nodes := [.input 0, .input 2, .input 1, .mul 1 2, .add 0 3] }
      fixturePartition [5] [2] fixtureLimits = none ∧
    prepare { graph := 17, controls := 19 }
      { additive with
        nodes := [.input 0, .input 1, .input 2, .input 0, .add 0 3] }
      fixturePartition [5] [2] fixtureLimits = none := by
  decide

theorem actual_tick_commits_once :
    tickCandidate fixture firstTicket [4] false = .ready [13] [13] ∧
    (tick fixture firstTicket [4] false).runner.state = [13] ∧
    (tick fixture firstTicket [4] false).runner.commits = 1 := by
  decide

/-- Changing controls changes the next result; activation is not applied one tick late. -/
theorem changed_control_is_selected :
    (updateControls fixture initialEpochs [3] 31).runner.controlEpoch = 1 ∧
    (tick (updateControls fixture initialEpochs [3] 31).runner
      { graphEpoch := 0, controlEpoch := 1, tick := 0 } [4] false).runner.state = [17] := by
  decide

/-- A duplicate tick ticket and a cancellation refuse without publishing another result. -/
theorem duplicate_and_cancel_preserve :
    (tick (tick fixture firstTicket [4] false).runner firstTicket [4] false).observation =
      .refused .staleTick ∧
    (tick fixture firstTicket [4] true).runner = fixture := by
  decide

/-- An explicitly checked same-layout replacement starts from 99, not the old state 5;
    its ordered subtraction output is distinct from its next-state word. -/
theorem same_layout_reset_never_migrates :
    (reset fixture initialEpochs replacementIds subtractive fixturePartition [99] [3]).runner.state =
      [99] ∧
    (tick (reset fixture initialEpochs replacementIds subtractive fixturePartition [99] [3]).runner
      { graphEpoch := 1, controlEpoch := 1, tick := 0 } [4] false).runner.state = [95] ∧
    (tick (reset fixture initialEpochs replacementIds subtractive fixturePartition [99] [3]).runner
      { graphEpoch := 1, controlEpoch := 1, tick := 0 } [4] false).runner.visibleOutput = [92] := by
  decide

/-- Wrong interface widths and exhausted tick counters are actual refusal paths. -/
theorem partition_and_tick_bound_refuse :
    prepare { graph := 17, controls := 19 } additive
      { fixturePartition with controlWords := 2 } [5] [2] fixtureLimits = none ∧
    (tick { fixture with nextTick := 8 }
      { graphEpoch := 0, controlEpoch := 0, tick := 8 } [4] false).observation =
      .refused .exhausted := by
  decide

end NobleFoundation.TickRunner
