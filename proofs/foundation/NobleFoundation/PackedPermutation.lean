  /-
  A separate witnessed within-layer permutation profile for total packed I64/Bool graphs.

  The given preload table and operation stream are executed, not reconstructed before
  execution. The graph, register map, layer assignment, layer ranges, witness, bodies
  and ordered output registers are all arguments to the checker. Table entries may
  occur in any order, but each must name one distinct graph input or constant,
  and an operation belongs to exactly one declared instruction range. Layer metadata
  is supplied as part of the model artifact. This proof does not establish that a Rust checker derived the same
  layer metadata, interpreted the bytes as these model constructors, or executes the
  operations according to this model.
  -/
import NobleFoundation.PackedSchedule
import NobleFoundation.FirstProjection

namespace NobleFoundation.PackedPermutation

open NobleFoundation.PackedSchedule

/-- The model graph comes from the actual finite node list. Output selection is
handled by the separate ordered output list, not this unused Graph.output field. -/
def graph (nodes : List Node) : Graph := FirstProjection.ofList nodes 0

/-- A checked register list is read at the same node index in every obligation. -/
def regOf (regs : List Nat) (index : Nat) : Nat := regs.getD index 0

/-- Table entries are inputs and exact I64 constants. -/
def isTable : Node → Bool
  | .input _ | .constant _ => true
  | .add _ _ | .sub _ _ | .mul _ _ | .eq _ _ => false

/-- All graph nodes that must be initialized by the tables. -/
def tableNodes (nodes : List Node) : List Nat :=
  (List.range nodes.length).filter fun index => isTable (nodes.getD index (.constant 0))

/-- Every operation node must be witnessed, even if no output observes it. -/
def emittingNodes (nodes : List Node) : List Nat :=
  (List.range nodes.length).filter fun index => !isTable (nodes.getD index (.constant 0))

/-- Recover the claimed table node by matching an actual preload body against the
graph, using the caller's declared register map. A missing match is out of range. -/
def tableNodeFor (nodes : List Node) (regs : List Nat) (instr : Instr) : Nat :=
  ((tableNodes nodes).find? (fun index =>
    decide (emitNodeWith (graph nodes) (regOf regs) index = instr))).getD nodes.length

/-- The order of actual table entries, never a regenerated graph-node order. -/
def tableWitness (nodes : List Node) (regs : List Nat) (preloads : List Instr) : List Nat :=
  preloads.map (tableNodeFor nodes regs)

/-- A witnessed list covers exactly its required nodes, with no duplicate or omission. -/
def coversExactly (required witness : List Nat) : Bool :=
  decide witness.Nodup &&
    witness.all (fun index => decide (index ∈ required)) &&
    required.all (fun index => decide (index ∈ witness))

/-- All input, constant and arithmetic nodes produce I64; equality produces Bool. -/
def producesI64 : Node → Bool
  | .eq _ _ => false
  | _ => true

/-- Every operand must be an I64-producing node. This excludes graphs whose
`Option Val` observation could be empty solely because a Bool is used as I64. -/
def operandsI64 (nodes : List Node) : Bool :=
  (List.range nodes.length).all fun index =>
    ((nodes.getD index (.constant 0)).operands).all fun operand =>
      producesI64 (nodes.getD operand (.constant 0))

/-- For every dependency, the declared layer of the operand precedes the node.
The old packed validator additionally checks actual definition-before-use. -/
def dependenciesEarlierLayer (nodes : List Node) (nodeLayers : List Nat) : Bool :=
  (List.range nodes.length).all fun index =>
    ((nodes.getD index (.constant 0)).operands).all fun operand =>
      decide (nodeLayers.getD operand 0 < nodeLayers.getD index 0)

/-- Ranges tile the entire operation stream; each witnessed node in a range has
exactly that range's layer number. Empty layers are allowed. -/
def layersMatch (nodeLayers : List Nat) (witness : List Nat) :
    Nat → Nat → List (Nat × Nat) → Bool
  | _, offset, [] => decide (offset = witness.length)
  | layer, offset, (start, stop) :: rest =>
      decide (start = offset ∧ start ≤ stop ∧ stop ≤ witness.length) &&
        ((witness.drop start).take (stop - start)).all
          (fun index => decide (nodeLayers.getD index 0 = layer)) &&
        layersMatch nodeLayers witness (layer + 1) stop rest

/-- Independent obligations for this profile. `PackedSchedule.validate` checks
actual concatenated table and operation bodies, initialized ordered operands,
register injectivity, and every position of the output map. The other checks
exclude an incomplete or cross-layer stream even if its outputs coincide. -/
def obligations (nodes : List Node) (regs nodeLayers : List Nat)
    (layers : List (Nat × Nat)) (preloads : List Instr)
    (witness : List Nat) (instrs : List Instr) (outputs outRegs : List Nat) : List Bool :=
  [decide (0 < nodes.length),
    decide (regs.length = nodes.length),
    decide (nodeLayers.length = nodes.length),
    FirstProjection.operandsEarlier nodes,
    operandsI64 nodes,
    decide (outputs ≠ []),
    coversExactly (tableNodes nodes) (tableWitness nodes regs preloads),
    coversExactly (emittingNodes nodes) witness,
    (List.range nodes.length).all (fun index =>
      decide (nodeLayers.getD index 0 < layers.length)),
    dependenciesEarlierLayer nodes nodeLayers,
    layersMatch nodeLayers witness 0 0 layers] ++
    [PackedSchedule.validate (graph nodes) (regOf regs)
      (tableWitness nodes regs preloads ++ witness) (preloads ++ instrs) outputs outRegs]

/-- A separate validator for the actual given table, op-only stream, per-instruction
node witness and ordered output map. All data is from the caller; no scheduler is run. -/
def validate (nodes : List Node) (regs nodeLayers : List Nat)
    (layers : List (Nat × Nat)) (preloads : List Instr)
    (witness : List Nat) (instrs : List Instr) (outputs outRegs : List Nat) : Bool :=
  (obligations nodes regs nodeLayers layers preloads witness instrs outputs outRegs).all id

/-- Evaluate the supplied table entries, then the supplied operation stream. -/
def runOutputs (inputs : Nat → BitVec 64) (preloads instrs : List Instr)
    (outRegs : List Nat) : List (Option Val) :=
  PackedSchedule.runOutputs inputs (preloads ++ instrs) outRegs

/-- A model of sufficient instruction-step budget and no cancellation. This does
not assert identical low-budget or cancellation behavior of a Rust evaluator. -/
def runWithBudget (inputs : Nat → BitVec 64) (budget : Nat) (cancelled : Bool)
    (preloads instrs : List Instr) (outRegs : List Nat) : Option (List (Option Val)) :=
  if cancelled then none
  else if decide (preloads.length + instrs.length ≤ budget) then
    some (runOutputs inputs preloads instrs outRegs)
  else none

/-- An accepted actual stream agrees with the graph at every ordered output,
including repetitions, for all assignments of input words. The proof inherits
actual-body, dependency, single-writer and output checks from the packed validator,
while this separate profile also checks full work coverage and layer membership. -/
theorem accepted_stream_sound (nodes : List Node) (regs nodeLayers : List Nat)
    (layers : List (Nat × Nat)) (preloads : List Instr)
    (witness : List Nat) (instrs : List Instr) (outputs outRegs : List Nat)
    (accepted : validate nodes regs nodeLayers layers preloads witness instrs outputs outRegs = true)
    (inputs : Nat → BitVec 64) :
    runOutputs inputs preloads instrs outRegs =
      referenceOutputs (graph nodes) inputs outputs := by
  change (obligations nodes regs nodeLayers layers preloads witness instrs outputs outRegs).all id =
    true at accepted
  have checked := List.all_eq_true.mp accepted
  have nonempty : 0 < nodes.length :=
    of_decide_eq_true (checked (decide (0 < nodes.length)) (by simp [obligations]))
  have earlier : FirstProjection.operandsEarlier nodes = true :=
    checked (FirstProjection.operandsEarlier nodes) (by simp [obligations])
  have body : PackedSchedule.validate (graph nodes) (regOf regs)
      (tableWitness nodes regs preloads ++ witness) (preloads ++ instrs) outputs outRegs = true :=
    checked _ (by
      unfold obligations
      exact List.mem_append.mpr (Or.inr (List.mem_singleton_self _)))
  have wellFormed : WellFormed (graph nodes) :=
    FirstProjection.ofList_wellFormed nodes 0 nonempty earlier
  exact PackedSchedule.validate_sound (graph nodes) wellFormed (regOf regs)
    (tableWitness nodes regs preloads ++ witness) (preloads ++ instrs) outputs outRegs body inputs

/-- Under an explicit sufficient step budget and no cancellation, the accepted
actual table and operation stream has exactly the graph's ordered observation. -/
theorem validate_sound (nodes : List Node) (regs nodeLayers : List Nat)
    (layers : List (Nat × Nat)) (preloads : List Instr)
    (witness : List Nat) (instrs : List Instr) (outputs outRegs : List Nat)
    (accepted : validate nodes regs nodeLayers layers preloads witness instrs outputs outRegs = true)
    (inputs : Nat → BitVec 64) (budget : Nat)
    (enough : preloads.length + instrs.length ≤ budget) (cancelled : Bool)
    (notCancelled : cancelled = false) :
    runWithBudget inputs budget cancelled preloads instrs outRegs =
      some (referenceOutputs (graph nodes) inputs outputs) := by
  simp [runWithBudget, notCancelled, enough,
    accepted_stream_sound nodes regs nodeLayers layers preloads witness instrs outputs outRegs
      accepted inputs]

/-! ### Hand-authored finite fixtures

The three schedules below share one graph, register map and table but contain
three separately written operation bodies. The two noncanonical witnesses are
not copies of the canonical one, or of each other. -/

def fixtureNodes : List Node :=
  [.input 0, .constant 5, .constant 2, .sub 0 1, .add 0 2, .mul 0 1,
    .eq 3 4, .eq 5 5]

def fixtureRegs : List Nat := [0, 1, 2, 3, 4, 5, 6, 7]

def fixtureNodeLayers : List Nat := [0, 0, 0, 1, 1, 1, 2, 2]

def fixtureLayers : List (Nat × Nat) := [(0, 0), (0, 3), (3, 5)]

def fixturePreloads : List Instr := [.setInput 0 0, .setConst 1 5, .setConst 2 2]

/-- The same actual table contents in constant-first, then input order. -/
def constantFirstPreloads : List Instr := [.setConst 2 2, .setConst 1 5, .setInput 0 0]

def fixtureOutputs : List Nat := [3, 3, 6, 7]

def canonicalWitness : List Nat := [3, 4, 5, 6, 7]

def canonicalInstrs : List Instr :=
  [.sub 3 0 1, .add 4 0 2, .mul 5 0 1, .eq 6 3 4, .eq 7 5 5]

def firstPermutation : List Nat := [4, 3, 5, 7, 6]

def firstInstrs : List Instr :=
  [.add 4 0 2, .sub 3 0 1, .mul 5 0 1, .eq 7 5 5, .eq 6 3 4]

def secondPermutation : List Nat := [5, 4, 3, 6, 7]

def secondInstrs : List Instr :=
  [.mul 5 0 1, .add 4 0 2, .sub 3 0 1, .eq 6 3 4, .eq 7 5 5]

theorem fixture_witnesses_distinct :
    canonicalWitness ≠ firstPermutation ∧ canonicalWitness ≠ secondPermutation ∧
      firstPermutation ≠ secondPermutation := by decide

theorem fixture_canonical_accepted :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      canonicalWitness canonicalInstrs fixtureOutputs fixtureOutputs = true := by decide

theorem fixture_first_permutation_accepted :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      firstPermutation firstInstrs fixtureOutputs fixtureOutputs = true := by decide

theorem fixture_second_permutation_accepted :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      secondPermutation secondInstrs fixtureOutputs fixtureOutputs = true := by decide

theorem fixture_constant_first_table_accepted :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers constantFirstPreloads
      firstPermutation firstInstrs fixtureOutputs fixtureOutputs = true := by decide

/-- The first noncanonical stream is sound for every dynamic input and any budget
large enough to execute the three preload entries and five operations. -/
theorem fixture_first_sound (inputs : Nat → BitVec 64) (budget : Nat)
    (enough : fixturePreloads.length + firstInstrs.length ≤ budget) :
    runWithBudget inputs budget false fixturePreloads firstInstrs fixtureOutputs =
      some (referenceOutputs (graph fixtureNodes) inputs fixtureOutputs) :=
  validate_sound fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
    firstPermutation firstInstrs fixtureOutputs fixtureOutputs
    fixture_first_permutation_accepted inputs budget enough false rfl

/-- The second noncanonical stream is sound for every dynamic input and enough budget. -/
theorem fixture_second_sound (inputs : Nat → BitVec 64) (budget : Nat)
    (enough : fixturePreloads.length + secondInstrs.length ≤ budget) :
    runWithBudget inputs budget false fixturePreloads secondInstrs fixtureOutputs =
      some (referenceOutputs (graph fixtureNodes) inputs fixtureOutputs) :=
  validate_sound fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
    secondPermutation secondInstrs fixtureOutputs fixtureOutputs
    fixture_second_permutation_accepted inputs budget enough false rfl

/-- A valid topological stream outside a declared layer is rejected by the new
profile even though the earlier graph-only stream checker accepts its bodies. -/
def crossLayerWitness : List Nat := [4, 3, 6, 5, 7]

def crossLayerInstrs : List Instr :=
  [.add 4 0 2, .sub 3 0 1, .eq 6 3 4, .mul 5 0 1, .eq 7 5 5]

theorem cross_layer_base_accepts :
    PackedSchedule.validate (graph fixtureNodes) (regOf fixtureRegs)
      (tableWitness fixtureNodes fixtureRegs fixturePreloads ++ crossLayerWitness)
      (fixturePreloads ++ crossLayerInstrs) fixtureOutputs fixtureOutputs = true := by decide

theorem cross_layer_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      crossLayerWitness crossLayerInstrs fixtureOutputs fixtureOutputs = false := by decide

/-- An attempted read of node 3 before its instruction is rejected. -/
def earlyReadWitness : List Nat := [6, 4, 3, 5, 7]

def earlyReadInstrs : List Instr :=
  [.eq 6 3 4, .add 4 0 2, .sub 3 0 1, .mul 5 0 1, .eq 7 5 5]

theorem early_read_base_refused :
    PackedSchedule.validate (graph fixtureNodes) (regOf fixtureRegs)
      (tableWitness fixtureNodes fixtureRegs fixturePreloads ++ earlyReadWitness)
      (fixturePreloads ++ earlyReadInstrs) fixtureOutputs fixtureOutputs = false := by decide

theorem early_read_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      earlyReadWitness earlyReadInstrs fixtureOutputs fixtureOutputs = false := by decide

theorem wrong_witness_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      canonicalWitness firstInstrs fixtureOutputs fixtureOutputs = false := by decide

theorem repeated_writer_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      [4, 3, 4, 7, 6] firstInstrs fixtureOutputs fixtureOutputs = false := by decide

theorem omitted_writer_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      [4, 3, 7, 6] [.add 4 0 2, .sub 3 0 1, .eq 7 5 5, .eq 6 3 4]
      fixtureOutputs fixtureOutputs = false := by decide

/-- Node 7 is absent from this output list; omitting it would still pass the
older body's output-only acceptance check, but not exact emitting-node coverage. -/
def selectedOutputs : List Nat := [3, 3, 6]

theorem dead_writer_base_accepts :
    PackedSchedule.validate (graph fixtureNodes) (regOf fixtureRegs)
      (tableWitness fixtureNodes fixtureRegs fixturePreloads ++ [3, 4, 5, 6])
      (fixturePreloads ++ [.sub 3 0 1, .add 4 0 2, .mul 5 0 1, .eq 6 3 4])
      selectedOutputs selectedOutputs = true := by decide

theorem dead_writer_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      [3, 4, 5, 6]
      [.sub 3 0 1, .add 4 0 2, .mul 5 0 1, .eq 6 3 4]
      selectedOutputs selectedOutputs = false := by decide

theorem swapped_subtraction_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      canonicalWitness
      [.sub 3 1 0, .add 4 0 2, .mul 5 0 1, .eq 6 3 4, .eq 7 5 5]
      fixtureOutputs fixtureOutputs = false := by decide

def aliasedRegs : List Nat := [0, 1, 2, 3, 4, 3, 6, 7]

theorem duplicate_destination_body_matches :
    PackedSchedule.bodiesMatch (graph fixtureNodes) (regOf aliasedRegs)
      (tableWitness fixtureNodes aliasedRegs fixturePreloads ++ canonicalWitness)
      (fixturePreloads ++
        [.sub 3 0 1, .add 4 0 2, .mul 3 0 1, .eq 6 3 4, .eq 7 3 3]) = true := by decide

theorem duplicate_destination_refused :
    validate fixtureNodes aliasedRegs fixtureNodeLayers fixtureLayers fixturePreloads
      canonicalWitness
      [.sub 3 0 1, .add 4 0 2, .mul 3 0 1, .eq 6 3 4, .eq 7 3 3]
      fixtureOutputs fixtureOutputs = false := by decide

theorem changed_constant_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers
      [.setInput 0 0, .setConst 1 6, .setConst 2 2]
      canonicalWitness canonicalInstrs fixtureOutputs fixtureOutputs = false := by decide

theorem changed_input_slot_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers
      [.setInput 0 1, .setConst 1 5, .setConst 2 2]
      canonicalWitness canonicalInstrs fixtureOutputs fixtureOutputs = false := by decide

theorem duplicate_table_entry_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers
      [.setInput 0 0, .setConst 1 5, .setConst 1 5, .setConst 2 2]
      canonicalWitness canonicalInstrs fixtureOutputs fixtureOutputs = false := by decide

theorem changed_output_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      canonicalWitness canonicalInstrs fixtureOutputs [3, 4, 6, 7] = false := by decide

theorem empty_output_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      canonicalWitness canonicalInstrs [] [] = false := by decide

theorem empty_witness_refused :
    validate fixtureNodes fixtureRegs fixtureNodeLayers fixtureLayers fixturePreloads
      [] [] fixtureOutputs fixtureOutputs = false := by decide

end NobleFoundation.PackedPermutation
