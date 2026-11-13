/-
The opt-in graph-to-LLVM lowering edge (issue #107), model side.

`crates/noble-ngraph/src/llvm.rs` emits one straight-line LLVM function from an admitted
`ngraph-i64/0` graph and validates the emitted text independently of the emitter. This file is the
semantic claim behind that validator: a machine over the parsed instruction list, a `Bool` model of
the validator, and `validate_sound`, which says that accepting an instruction list and a return
operand for a well-formed graph implies the executed function returns the graph reference's value of
the selected output for *every* pair of `BitVec 64` inputs.

The graph meaning is `NobleFoundation.PackedSchedule.value` — input, exact constant, wrapping
`add`/`sub`/`mul`, one selected output — so this lane asserts nothing new about the graph and
nothing at all about the emitter, the parser or the Rust checker.

What is modelled, and what is not:

* One `Instr` is field for field `noble_ngraph::llvm::Instruction`: the node its destination names,
  its opcode and its two ordered operands. `Operand` is the module's own spelling — parameter,
  exact literal, or `%v<node>`.
* A parameter and a literal are read where they stand, exactly as LLVM reads a function argument or
  a constant; a `%v<node>` operand reads what node `node` wrote, exactly as an SSA name does.
* `validate` walks the graph's nodes in ascending index order and consumes one instruction per
  arithmetic node, so it rejects a missing, extra, reordered, renamed or re-spelled instruction.
* Overflow and poison flags (`nsw`, `nuw`, …), the calling convention, the target triple, the data
  layout and the module's text form are checked by the Rust parser *before* a module becomes an
  instruction list. They are outside this model: the model starts at the parsed form, and the
  parser-to-model step is the trusted translation the lane's README names.
-/
import NobleFoundation.FirstProjection

namespace NobleFoundation.LlvmLowering

open NobleFoundation.PackedSchedule
open NobleFoundation.FirstProjection

/-! ## The parsed module -/

/-- The three arithmetic opcodes the profile emits. -/
inductive Opcode where
  | add
  | sub
  | mul
deriving DecidableEq, Repr

/-- An operand exactly as the module text spells it. -/
inductive Operand where
  /-- `%a<slot>`: a function argument. -/
  | param (slot : Nat)
  /-- An exact signed `i64` literal, as its 64 bits. -/
  | constant (bits : BitVec 64)
  /-- `%v<node>`: the value of the node with that index. -/
  | value (node : Nat)
deriving DecidableEq, Repr

/-- One parsed instruction. Field for field this is `noble_ngraph::llvm::Instruction`. -/
structure Instr where
  /-- The node whose value the destination SSA name carries. -/
  dst : Nat
  /-- The instruction's opcode. -/
  opcode : Opcode
  /-- The left operand. -/
  left : Operand
  /-- The right operand. -/
  right : Operand
deriving DecidableEq, Repr

/-- The value one operation computes on two `i64` values. `BitVec 64` addition, subtraction and
multiplication are two's-complement wrapping, which is what `add`, `sub` and `mul` mean without
`nsw` or `nuw`. -/
def combine : Opcode → BitVec 64 → BitVec 64 → BitVec 64
  | .add => (· + ·)
  | .sub => (· - ·)
  | .mul => (· * ·)

/-- The value an operand names in a machine state. A parameter and a literal are read where they
stand; a `%v<node>` operand reads that node's register. -/
def operandValue (inputs : Nat → BitVec 64) (regs : Nat → Option Val) : Operand → Option Val
  | .param slot => some (.i64 (inputs slot))
  | .constant bits => some (.i64 bits)
  | .value node => regs node

/-- One machine step. Only the instruction's destination register changes, and an operand that holds
no value makes the destination hold no value rather than a guess. -/
def step (inputs : Nat → BitVec 64) (regs : Nat → Option Val) (ins : Instr) : Nat → Option Val :=
  fun register =>
    if register = ins.dst then
      (match operandValue inputs regs ins.left, operandValue inputs regs ins.right with
       | some (.i64 left), some (.i64 right) => some (.i64 (combine ins.opcode left right))
       | _, _ => none)
    else regs register

/-- Execute the parsed instruction list from a register file. -/
def runFrom (inputs : Nat → BitVec 64) (regs : Nat → Option Val) (instrs : List Instr) :
    Nat → Option Val :=
  instrs.foldl (step inputs) regs

/-- The value the emitted function returns: the single `ret` operand, read after the body. -/
def result (inputs : Nat → BitVec 64) (instrs : List Instr) (out : Operand) : Option Val :=
  operandValue inputs (runFrom inputs (fun _ => none) instrs) out

/-- The input function a finite list of slot values denotes. -/
def inputsOf (values : List (BitVec 64)) (slot : Nat) : BitVec 64 :=
  values.getD slot 0

/-! ## The validator model -/

/-- A node is computed by an instruction in this profile. -/
def isArith (graph : Graph) (index : Nat) : Bool :=
  match graph.node index with
  | .input _ | .constant _ | .eq _ _ => false
  | _ => true

/-- A node is computed by an instruction in this profile. -/
def IsArith (graph : Graph) (index : Nat) : Prop := isArith graph index = true

/-- The opcode an arithmetic node demands, if it is arithmetic. -/
def requiredOpcode (graph : Graph) (index : Nat) : Option Opcode :=
  match graph.node index with
  | .add _ _ => some .add
  | .sub _ _ => some .sub
  | .mul _ _ => some .mul
  | _ => none

/-- The node indices an arithmetic node's operands name, left first. -/
def operandsOf (graph : Graph) (index : Nat) : Option (Nat × Nat) :=
  match graph.node index with
  | .add left right => some (left, right)
  | .sub left right => some (left, right)
  | .mul left right => some (left, right)
  | _ => none

/-- `Denotes graph node operand`: the parsed operand, read where it stands, is the value of graph
node `node`. A parameter denotes an `input` node of its slot, a literal denotes a `constant` node of
exactly its bits, and `%v<n>` denotes the arithmetic node `n` itself — the only node whose SSA value
that name can be. -/
def Denotes (graph : Graph) (target : Nat) : Operand → Prop
  | .param slot =>
      match graph.node target with
      | .input declared => declared = slot
      | _ => False
  | .constant bits =>
      match graph.node target with
      | .constant declared => declared = bits
      | _ => False
  | .value index => index = target ∧ IsArith graph target

/-- The decision procedure `Denotes` is the `Bool` spelling of. -/
def denotes (graph : Graph) (target : Nat) : Operand → Bool
  | .param slot =>
      match graph.node target with
      | .input declared => declared == slot
      | _ => false
  | .constant bits =>
      match graph.node target with
      | .constant declared => declared == bits
      | _ => false
  | .value index => (index == target) && isArith graph target

theorem denotes_iff (graph : Graph) (target : Nat) (operand : Operand) :
    denotes graph target operand = true ↔ Denotes graph target operand := by
  cases operand with
  | param slot =>
      cases declared : graph.node target <;> simp [denotes, Denotes, declared]
  | constant bits =>
      cases declared : graph.node target <;> simp [denotes, Denotes, declared]
  | value index =>
      simp [denotes, Denotes, IsArith]

/-- The check one instruction must pass at a node: it names the node, it has the node's opcode, and
its operands spell the node's operand nodes. -/
def spells (graph : Graph) (index : Nat) (ins : Instr) : Bool :=
  match requiredOpcode graph index, operandsOf graph index with
  | some opcode, some (left, right) =>
      (ins.dst == index)
        && ((ins.opcode == opcode) && (denotes graph left ins.left && denotes graph right ins.right))
  | _, _ => false

/-- Walk the graph's nodes `index … index+fuel-1` against the remaining instruction list: each
arithmetic node consumes exactly its own instruction, and a parameter or a literal consumes none.
The walk reports the instructions it did not consume, so only `some []` means the module spelled the
whole body. -/
def checkFrom (graph : Graph) : Nat → Nat → List Instr → Option (List Instr)
  | 0, _, rest => some rest
  | fuel + 1, index, rest =>
      if isArith graph index then
        match rest with
        | [] => none
        | ins :: tail =>
            if spells graph index ins then checkFrom graph fuel (index + 1) tail else none
      else checkFrom graph fuel (index + 1) rest

/-- The model of the validator: the body spells exactly the graph's arithmetic nodes in ascending
node order and leaves no instruction unconsumed, and the single `ret` spells the selected output. -/
def validate (graph : Graph) (instrs : List Instr) (out : Operand) : Bool :=
  (checkFrom graph graph.arity 0 instrs == some []) && denotes graph graph.output out

/-! ## Soundness

The invariant: after the walk has passed the nodes before `index`, every arithmetic node before
`index` already holds its graph value in its own register, and that value is an `I64` (so the
remaining instructions can neither clear it nor change its type). Each instruction writes only its
own node's register, and the graph's well-formedness puts every operand strictly earlier, so every
operand already holds its graph value when it is read. -/

/-- Every arithmetic node before `index` holds its graph value in its own register. -/
def Agrees (graph : Graph) (inputs : Nat → BitVec 64) (regs : Nat → Option Val) (index : Nat) :
    Prop :=
  ∀ node, node < index → IsArith graph node →
    ∃ bits, regs node = some (.i64 bits) ∧ value graph inputs node = some (.i64 bits)

/-- A step writes only its own destination register. -/
theorem step_unwritten (inputs : Nat → BitVec 64) (regs : Nat → Option Val) (ins : Instr)
    (register : Nat) (different : register ≠ ins.dst) :
    step inputs regs ins register = regs register := by
  simp [step, different]

/-- The operand a parsed instruction spells is the graph value of the node it denotes. -/
theorem denotes_value (graph : Graph) (inputs : Nat → BitVec 64) (regs : Nat → Option Val)
    (index target : Nat) (earlier : target < index) (operand : Operand)
    (agrees : Agrees graph inputs regs index) (denoted : Denotes graph target operand) :
    ∃ bits, operandValue inputs regs operand = some (.i64 bits) ∧
      value graph inputs target = some (.i64 bits) := by
  cases operand with
  | param slot =>
      cases declared : graph.node target with
      | input named =>
          have same : named = slot := by simpa [Denotes, declared] using denoted
          have node : graph.node target = .input slot := by simp [declared, same]
          exact ⟨inputs slot, by simp [operandValue], value_input graph inputs target slot node⟩
      | constant bits => simp [Denotes, declared] at denoted
      | add left right => simp [Denotes, declared] at denoted
      | sub left right => simp [Denotes, declared] at denoted
      | mul left right => simp [Denotes, declared] at denoted
      | eq left right => simp [Denotes, declared] at denoted
  | constant bits =>
      cases declared : graph.node target with
      | constant named =>
          have same : named = bits := by simpa [Denotes, declared] using denoted
          have node : graph.node target = .constant bits := by simp [declared, same]
          exact ⟨bits, by simp [operandValue], constant_value graph inputs target bits node⟩
      | input slot => simp [Denotes, declared] at denoted
      | add left right => simp [Denotes, declared] at denoted
      | sub left right => simp [Denotes, declared] at denoted
      | mul left right => simp [Denotes, declared] at denoted
      | eq left right => simp [Denotes, declared] at denoted
  | value index' =>
      obtain ⟨same, arith⟩ := denoted
      subst index'
      exact agrees target earlier arith

/-- A step computes its own node's graph value when the graph puts it at `index`, the instruction
spells that node, and every earlier arithmetic node already holds its value. -/
theorem spells_computes (graph : Graph) (wellFormed : WellFormed graph)
    (inputs : Nat → BitVec 64) (regs : Nat → Option Val) (index : Nat) (ins : Instr)
    (agrees : Agrees graph inputs regs index) (spelled : spells graph index ins = true) :
    ∃ bits, step inputs regs ins index = some (.i64 bits) ∧
      value graph inputs index = some (.i64 bits) := by
  cases shape : graph.node index with
  | input slot => simp [spells, requiredOpcode, shape] at spelled
  | constant bits => simp [spells, requiredOpcode, shape] at spelled
  | eq left right => simp [spells, requiredOpcode, shape] at spelled
  | add left right =>
      have hleft : left < index := wellFormed.2 index left (by simp [shape, Node.operands])
      have hright : right < index := wellFormed.2 index right (by simp [shape, Node.operands])
      simp only [spells, requiredOpcode, operandsOf, shape, Bool.and_eq_true, beq_iff_eq] at spelled
      obtain ⟨named, opcode, operands⟩ := spelled
      obtain ⟨spelledLeft, spelledRight⟩ := operands
      obtain ⟨x, hx, valuex⟩ := denotes_value graph inputs regs index left hleft ins.left agrees
        ((denotes_iff graph left ins.left).1 spelledLeft)
      obtain ⟨y, hy, valuey⟩ := denotes_value graph inputs regs index right hright ins.right agrees
        ((denotes_iff graph right ins.right).1 spelledRight)
      refine ⟨x + y, ?_, value_add graph wellFormed inputs index left right shape x y valuex valuey⟩
      simp [step, named, opcode, hx, hy, combine]
  | sub left right =>
      have hleft : left < index := wellFormed.2 index left (by simp [shape, Node.operands])
      have hright : right < index := wellFormed.2 index right (by simp [shape, Node.operands])
      simp only [spells, requiredOpcode, operandsOf, shape, Bool.and_eq_true, beq_iff_eq] at spelled
      obtain ⟨named, opcode, operands⟩ := spelled
      obtain ⟨spelledLeft, spelledRight⟩ := operands
      obtain ⟨x, hx, valuex⟩ := denotes_value graph inputs regs index left hleft ins.left agrees
        ((denotes_iff graph left ins.left).1 spelledLeft)
      obtain ⟨y, hy, valuey⟩ := denotes_value graph inputs regs index right hright ins.right agrees
        ((denotes_iff graph right ins.right).1 spelledRight)
      refine ⟨x - y, ?_, value_sub graph wellFormed inputs index left right shape x y valuex valuey⟩
      simp [step, named, opcode, hx, hy, combine]
  | mul left right =>
      have hleft : left < index := wellFormed.2 index left (by simp [shape, Node.operands])
      have hright : right < index := wellFormed.2 index right (by simp [shape, Node.operands])
      simp only [spells, requiredOpcode, operandsOf, shape, Bool.and_eq_true, beq_iff_eq] at spelled
      obtain ⟨named, opcode, operands⟩ := spelled
      obtain ⟨spelledLeft, spelledRight⟩ := operands
      obtain ⟨x, hx, valuex⟩ := denotes_value graph inputs regs index left hleft ins.left agrees
        ((denotes_iff graph left ins.left).1 spelledLeft)
      obtain ⟨y, hy, valuey⟩ := denotes_value graph inputs regs index right hright ins.right agrees
        ((denotes_iff graph right ins.right).1 spelledRight)
      refine ⟨x * y, ?_, value_mul graph wellFormed inputs index left right shape x y valuex valuey⟩
      simp [step, named, opcode, hx, hy, combine]

/-- An instruction the validator accepts at a node names that node. -/
theorem spells_dst (graph : Graph) (index : Nat) (ins : Instr)
    (spelled : spells graph index ins = true) : ins.dst = index := by
  cases shape : graph.node index with
  | input slot => simp [spells, requiredOpcode, shape] at spelled
  | constant bits => simp [spells, requiredOpcode, shape] at spelled
  | eq left right => simp [spells, requiredOpcode, shape] at spelled
  | add left right =>
      simp only [spells, requiredOpcode, operandsOf, shape, Bool.and_eq_true, beq_iff_eq] at spelled
      exact spelled.1
  | sub left right =>
      simp only [spells, requiredOpcode, operandsOf, shape, Bool.and_eq_true, beq_iff_eq] at spelled
      exact spelled.1
  | mul left right =>
      simp only [spells, requiredOpcode, operandsOf, shape, Bool.and_eq_true, beq_iff_eq] at spelled
      exact spelled.1

/-- The walk is sound: after it consumes the whole body, every node it passed holds its graph value,
and every register below the starting index is untouched. -/
theorem checkFrom_sound (graph : Graph) (wellFormed : WellFormed graph)
    (inputs : Nat → BitVec 64) :
    ∀ (fuel index : Nat) (instrs : List Instr) (regs : Nat → Option Val),
      Agrees graph inputs regs index →
      checkFrom graph fuel index instrs = some [] →
      (∀ node, node < index → runFrom inputs regs instrs node = regs node) ∧
        (∀ node, index ≤ node → node < index + fuel → IsArith graph node →
          runFrom inputs regs instrs node = value graph inputs node) := by
  intro fuel
  induction fuel with
  | zero =>
      intro index instrs regs _ checked
      have empty : instrs = [] := by simpa [checkFrom] using checked
      subst empty
      refine ⟨?_, ?_⟩
      · intro node _; rfl
      · intro node after before _; omega
  | succ fuel ih =>
      intro index instrs regs agrees checked
      by_cases arithmetic : isArith graph index = true
      · cases instrs with
        | nil => simp [checkFrom, arithmetic] at checked
        | cons ins tail =>
            have walked : checkFrom graph fuel (index + 1) tail = some [] := by
              by_cases present : spells graph index ins = true
              · simpa [checkFrom, arithmetic, present] using checked
              · simp [checkFrom, arithmetic, present] at checked
            have spelled : spells graph index ins = true := by
              cases present : spells graph index ins
              · simp [checkFrom, arithmetic, present] at checked
              · rfl
            have advanced : Agrees graph inputs (step inputs regs ins) (index + 1) := by
              intro node earlier arith
              by_cases same : node = index
              · subst same
                obtain ⟨bits, computed, held⟩ :=
                  spells_computes graph wellFormed inputs regs index ins agrees spelled
                exact ⟨bits, computed, held⟩
              · have below : node < index := by omega
                have named : ins.dst = index := spells_dst graph index ins spelled
                have untouched : step inputs regs ins node = regs node :=
                  step_unwritten inputs regs ins node (by rw [named]; exact same)
                obtain ⟨bits, held, graphheld⟩ := agrees node below arith
                exact ⟨bits, by rw [untouched]; exact held, graphheld⟩
            obtain ⟨keeps, values⟩ := ih (index + 1) tail (step inputs regs ins) advanced walked
            have named : ins.dst = index := spells_dst graph index ins spelled
            refine ⟨?_, ?_⟩
            · intro node earlier
              have reached : node < index + 1 := by omega
              have untouched : step inputs regs ins node = regs node :=
                step_unwritten inputs regs ins node (by rw [named]; omega)
              calc runFrom inputs regs (ins :: tail) node
                  = runFrom inputs (step inputs regs ins) tail node := by
                    simp only [runFrom, List.foldl_cons]
                _ = step inputs regs ins node := keeps node reached
                _ = regs node := untouched
            · intro node after before arith
              by_cases same : node = index
              · subst same
                have reached : index < index + 1 := by omega
                obtain ⟨bits, computed, graphheld⟩ :=
                  spells_computes graph wellFormed inputs regs index ins agrees spelled
                calc runFrom inputs regs (ins :: tail) index
                    = runFrom inputs (step inputs regs ins) tail index := by
                      simp only [runFrom, List.foldl_cons]
                  _ = step inputs regs ins index := keeps index reached
                  _ = some (.i64 bits) := computed
                  _ = value graph inputs index := by rw [graphheld]
              · have later : index + 1 ≤ node := by omega
                have bound : node < index + 1 + fuel := by omega
                calc runFrom inputs regs (ins :: tail) node
                    = runFrom inputs (step inputs regs ins) tail node := by
                      simp only [runFrom, List.foldl_cons]
                  _ = value graph inputs node := values node later bound arith
      · have plain : isArith graph index = false := by
          cases held : isArith graph index
          · rfl
          · exact absurd held arithmetic
        have walked : checkFrom graph fuel (index + 1) instrs = some [] := by
          simpa [checkFrom, plain] using checked
        have advanced : Agrees graph inputs regs (index + 1) := by
          intro node earlier arith
          by_cases same : node = index
          · subst same
            rw [plain] at arith
            exact absurd arith (by decide)
          · exact agrees node (by omega) arith
        obtain ⟨keeps, values⟩ := ih (index + 1) instrs regs advanced walked
        refine ⟨?_, ?_⟩
        · intro node earlier
          exact keeps node (by omega)
        · intro node after before arith
          by_cases same : node = index
          · subst same
            rw [plain] at arith
            exact absurd arith (by decide)
          · exact values node (by omega) (by omega) arith

/-- **The claim.** An instruction list and a return operand the validator accepts for a well-formed
graph make the executed function return the graph reference's value of the selected output, for
every pair of `BitVec 64` inputs. -/
theorem validate_sound (graph : Graph) (wellFormed : WellFormed graph) (instrs : List Instr)
    (out : Operand) (accepted : validate graph instrs out = true) (inputs : Nat → BitVec 64) :
    result inputs instrs out = value graph inputs graph.output := by
  simp only [validate, Bool.and_eq_true, beq_iff_eq] at accepted
  obtain ⟨checked, returned⟩ := accepted
  have denoted : Denotes graph graph.output out := (denotes_iff graph graph.output out).1 returned
  obtain ⟨_, values⟩ :=
    checkFrom_sound graph wellFormed inputs graph.arity 0 instrs (fun _ => none)
      (by intro node earlier; exact absurd earlier (Nat.not_lt_zero node)) checked
  cases out with
  | param slot =>
      cases declared : graph.node graph.output with
      | input named =>
          have same : named = slot := by simpa [Denotes, declared] using denoted
          have node : graph.node graph.output = .input slot := by simp [declared, same]
          simp only [result, operandValue]
          exact (value_input graph inputs graph.output slot node).symm
      | constant bits => simp [Denotes, declared] at denoted
      | add left right => simp [Denotes, declared] at denoted
      | sub left right => simp [Denotes, declared] at denoted
      | mul left right => simp [Denotes, declared] at denoted
      | eq left right => simp [Denotes, declared] at denoted
  | constant bits =>
      cases declared : graph.node graph.output with
      | constant named =>
          have same : named = bits := by simpa [Denotes, declared] using denoted
          have node : graph.node graph.output = .constant bits := by simp [declared, same]
          simp only [result, operandValue]
          exact (constant_value graph inputs graph.output bits node).symm
      | input slot => simp [Denotes, declared] at denoted
      | add left right => simp [Denotes, declared] at denoted
      | sub left right => simp [Denotes, declared] at denoted
      | mul left right => simp [Denotes, declared] at denoted
      | eq left right => simp [Denotes, declared] at denoted
  | value node =>
      obtain ⟨same, arith⟩ := denoted
      subst node
      simp only [result, operandValue]
      exact values graph.output (Nat.zero_le _) (by simpa using wellFormed.1) arith
end NobleFoundation.LlvmLowering
