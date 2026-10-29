/-
Validator model for the packed I64/Bool schedule (issue #43, phase 1).

The Rust side (`crates/noble-ngraph/src/compiled.rs`) says agreement between the
packed evaluator and the graph reference is *tested, not proved*, so NG-PO-C stays
open. This file names the model side of a validator lane: the schedule stays
untrusted, and a small relation over it is proved sufficient for complete ordered
result equivalence.

Fragment: finite acyclic graphs over interface input slots, exact `I64`
constants, addition, subtraction, multiplication and equality, with one selected
output. The instruction set mirrors the packed `I` enum — destination register
first, then ordered source registers — and constants and interface slots occupy
their own registers, so bits and slots are validated rather than assumed. Guards,
`F32`, layers-as-reordering, failure order, effects and resources are outside this
file, and so is any binding to the actually emitted Rust artifact.

The schedule relation forbids register aliasing outright in this phase. That is a
restriction, not a claim that the layered scheduler avoids aliasing: the control
`aliasing_loses_the_constant` shows concretely why aliasing needs its own
premise rather than being assumed away.
-/
namespace NobleFoundation.PackedSchedule

/-- A register value in the total I64/Bool fragment. -/
inductive Val where
  | i64 (value : BitVec 64)
  | boolean (value : Bool)
deriving DecidableEq, Repr

/-- One node of the fragment. Operand fields are node indices. -/
inductive Node where
  | input (slot : Nat)
  | constant (bits : BitVec 64)
  | add (a b : Nat)
  | sub (a b : Nat)
  | mul (a b : Nat)
  | eq (a b : Nat)
deriving DecidableEq, Repr

/-- Operand indices in stored order. -/
def Node.operands : Node → List Nat
  | .add a b => [a, b]
  | .sub a b => [a, b]
  | .mul a b => [a, b]
  | .eq a b => [a, b]
  | _ => []

/-- A finite graph with one selected output node. -/
structure Graph where
  arity : Nat
  output : Nat
  node : Nat → Node

/-- Well-formedness: every operand is strictly earlier, and the selected output
is inside the arity. -/
def WellFormed (graph : Graph) : Prop :=
  graph.output < graph.arity ∧
    ∀ index, ∀ operand ∈ (graph.node index).operands, operand < index

/-- The graph reference semantics of one node against the values of the strictly
earlier nodes. An operand type mismatch yields no value rather than a guess. -/
def evalStep (graph : Graph) (values : Nat → Option Val) (inputs : Nat → BitVec 64)
    (index : Nat) : Option Val :=
  match graph.node index with
  | .input slot => some (.i64 (inputs slot))
  | .constant bits => some (.i64 bits)
  | .add a b =>
      match values a, values b with
      | some (.i64 x), some (.i64 y) => some (.i64 (x + y))
      | _, _ => none
  | .sub a b =>
      match values a, values b with
      | some (.i64 x), some (.i64 y) => some (.i64 (x - y))
      | _, _ => none
  | .mul a b =>
      match values a, values b with
      | some (.i64 x), some (.i64 y) => some (.i64 (x * y))
      | _, _ => none
  | .eq a b =>
      match values a, values b with
      | some (.i64 x), some (.i64 y) => some (.boolean (x == y))
      | _, _ => none

/-- The graph reference semantics of the first `count` nodes. -/
def valuesUpTo (graph : Graph) (inputs : Nat → BitVec 64) : Nat → (Nat → Option Val)
  | 0 => fun _ => none
  | count + 1 => fun index =>
      if index = count then evalStep graph (valuesUpTo graph inputs count) inputs count
      else valuesUpTo graph inputs count index

/-- The graph reference value of one node. -/
def value (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat) : Option Val :=
  valuesUpTo graph inputs (index + 1) index

/-- One packed instruction. Destination register first, then ordered sources. -/
inductive Instr where
  | setInput (reg slot : Nat)
  | setConst (reg : Nat) (bits : BitVec 64)
  | add (dst src1 src2 : Nat)
  | sub (dst src1 src2 : Nat)
  | mul (dst src1 src2 : Nat)
  | eq (dst src1 src2 : Nat)

/-- One machine step. A register not written by the instruction keeps its value. -/
def step (inputs : Nat → BitVec 64) (regs : Nat → Option Val) : Instr → (Nat → Option Val)
  | .setInput reg slot => fun r => if r = reg then some (.i64 (inputs slot)) else regs r
  | .setConst reg bits => fun r => if r = reg then some (.i64 bits) else regs r
  | .add dst x y => fun r =>
      if r = dst then
        (match regs x, regs y with
         | some (.i64 a), some (.i64 b) => some (.i64 (a + b))
         | _, _ => none)
      else regs r
  | .sub dst x y => fun r =>
      if r = dst then
        (match regs x, regs y with
         | some (.i64 a), some (.i64 b) => some (.i64 (a - b))
         | _, _ => none)
      else regs r
  | .mul dst x y => fun r =>
      if r = dst then
        (match regs x, regs y with
         | some (.i64 a), some (.i64 b) => some (.i64 (a * b))
         | _, _ => none)
      else regs r
  | .eq dst x y => fun r =>
      if r = dst then
        (match regs x, regs y with
         | some (.i64 a), some (.i64 b) => some (.boolean (a == b))
         | _, _ => none)
      else regs r

/-- The untrusted producer's schedule: an order of nodes, a register for each node,
and the register that carries the selected output. -/
structure Schedule where
  order : List Nat
  regOf : Nat → Nat
  outReg : Nat

/-- One node's instruction, using the schedule's registers. -/
def emitNode (graph : Graph) (schedule : Schedule) (index : Nat) : Instr :=
  match graph.node index with
  | .input slot => .setInput (schedule.regOf index) slot
  | .constant bits => .setConst (schedule.regOf index) bits
  | .add a b => .add (schedule.regOf index) (schedule.regOf a) (schedule.regOf b)
  | .sub a b => .sub (schedule.regOf index) (schedule.regOf a) (schedule.regOf b)
  | .mul a b => .mul (schedule.regOf index) (schedule.regOf a) (schedule.regOf b)
  | .eq a b => .eq (schedule.regOf index) (schedule.regOf a) (schedule.regOf b)

/-- The packed instruction stream for a scheduled order. -/
def emitNodes (graph : Graph) (schedule : Schedule) (order : List Nat) : List Instr :=
  order.map (emitNode graph schedule)

/-- Execute a packed instruction stream. -/
def run (inputs : Nat → BitVec 64) (instrs : List Instr) : Nat → Option Val :=
  instrs.foldl (step inputs) (fun _ => none)

/-- Execute a scheduled order directly, one node at a time. -/
def execSchedule (graph : Graph) (schedule : Schedule) (inputs : Nat → BitVec 64) :
    List Nat → (Nat → Option Val) → (Nat → Option Val)
  | [], regs => regs
  | index :: rest, regs => execSchedule graph schedule inputs rest (step inputs regs (emitNode graph schedule index))

/-- Order correspondence: each node's operands have already been processed. This
is exactly definition-before-use over the untrusted order, with no need to assume
fresh registers or a permutation. -/
def OrderOk (graph : Graph) : List Nat → List Nat → Prop
  | _, [] => True
  | processed, index :: rest =>
      (∀ operand ∈ (graph.node index).operands, operand ∈ processed) ∧
        OrderOk graph (index :: processed) rest

/-! ### Bridge from the packed stream to the scheduled execution -/

theorem run_emitNodes_eq (graph : Graph) (schedule : Schedule) (inputs : Nat → BitVec 64)
    (order : List Nat) :
    run inputs (emitNodes graph schedule order) =
      order.foldl (fun regs index => step inputs regs (emitNode graph schedule index))
        (fun _ => none) := by
  unfold run emitNodes
  exact List.foldl_map

theorem foldl_eq_execSchedule (graph : Graph) (schedule : Schedule)
    (inputs : Nat → BitVec 64) :
    ∀ (order : List Nat) (regs : Nat → Option Val),
      order.foldl (fun regs index => step inputs regs (emitNode graph schedule index)) regs =
        execSchedule graph schedule inputs order regs
  | [], regs => rfl
  | index :: rest, regs => by
      simp only [List.foldl_cons, execSchedule]
      exact foldl_eq_execSchedule graph schedule inputs rest _

/-! ### Reference stability -/

theorem valuesUpTo_eq_value (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ count index, index < count →
      valuesUpTo graph inputs count index = value graph inputs index := by
  intro count
  induction count with
  | zero => intro index h; exact absurd h (Nat.not_lt_zero index)
  | succ count ih =>
      intro index h
      by_cases hlast : index = count
      · rw [hlast]
        simp [value, valuesUpTo]
      · have hlt : index < count := by omega
        simp only [valuesUpTo, if_neg hlast]
        exact ih index hlt

/-! ### Soundness

The invariant: once the scheduled prefix has been executed, every register of a
processed node holds that node's graph value. Each step writes only its own
node's register, and the schedule relation's injectivity keeps that write away
from every processed register. -/

/-- A step writes only its own node's register. -/
theorem step_written (graph : Graph) (schedule : Schedule) (inputs : Nat → BitVec 64)
    (regs : Nat → Option Val) (index r : Nat) (hne : r ≠ schedule.regOf index) :
    step inputs regs (emitNode graph schedule index) r = regs r := by
  unfold emitNode
  split <;> simp [step, hne]

/-! ### Missing operands and unwritten registers (issue #43)

These are properties of the handwritten `Option Val` machine, not checks of Rust instruction bytes
or graph guard operations. An arithmetic or equality instruction with an empty operand writes `none`
to its destination, including when that destination previously held a value. A register no executed
node writes preserves its initial contents; emptiness therefore requires an empty initial register.
Omitting an output node alone is insufficient if another executed node aliases its register.
No layout acceptance or implementation-correspondence claim follows from these statements. -/

/-- In the model, an empty first operand makes `add`, `sub`, `mul` and `eq` write `none`
to their destination, even if that destination previously held a value. -/
theorem a_missing_operand_refuses_rather_than_fabricating (inputs : Nat → BitVec 64)
    (regs : Nat → Option Val) (dst x y : Nat) (missing : regs x = none) :
    step inputs regs (.add dst x y) dst = none
      ∧ step inputs regs (.sub dst x y) dst = none
      ∧ step inputs regs (.mul dst x y) dst = none
      ∧ step inputs regs (.eq dst x y) dst = none := by
  simp [step, missing]

/-- The same model property holds when the second operand register is empty. -/
theorem a_missing_second_operand_refuses_rather_than_fabricating (inputs : Nat → BitVec 64)
    (regs : Nat → Option Val) (dst x y : Nat) (missing : regs y = none) :
    step inputs regs (.add dst x y) dst = none
      ∧ step inputs regs (.sub dst x y) dst = none
      ∧ step inputs regs (.mul dst x y) dst = none
      ∧ step inputs regs (.eq dst x y) dst = none := by
  simp [step, missing]

/-- No executed node writes to a given register. -/
def Unwritten (schedule : Schedule) (order : List Nat) (register : Nat) : Prop :=
  ∀ index ∈ order, schedule.regOf index ≠ register

/-- A register no executed node writes preserves its initial contents through the model stream.
In particular, a nonempty initial register is not cleared merely by omitting its writer. -/
theorem an_unwritten_register_keeps_its_value (graph : Graph) (schedule : Schedule)
    (inputs : Nat → BitVec 64) :
    ∀ (order : List Nat) (register : Nat), Unwritten schedule order register →
      ∀ (regs : Nat → Option Val),
        execSchedule graph schedule inputs order regs register = regs register := by
  intro order
  induction order with
  | nil => intro register _ regs; rfl
  | cons index rest ih =>
      intro register unwritten regs
      have hne : register ≠ schedule.regOf index :=
        fun same => unwritten index (by simp) same.symm
      have advanced := step_written graph schedule inputs regs index register hne
      simp only [execSchedule]
      rw [ih register (fun other member => unwritten other (by simp [member]))
        (step inputs regs (emitNode graph schedule index))]
      exact advanced

/-- Starting from an empty machine, such a register stays empty. -/
theorem an_unwritten_register_stays_empty (graph : Graph) (schedule : Schedule)
    (inputs : Nat → BitVec 64) (order : List Nat) (register : Nat)
    (unwritten : Unwritten schedule order register) :
    execSchedule graph schedule inputs order (fun _ => none) register = none :=
  an_unwritten_register_keeps_its_value graph schedule inputs order register unwritten
    (fun _ => none)

/-- Starting from an empty model machine, omitting every writer of the selected output register
leaves it empty. The premise also excludes writes by nodes aliased onto that register. -/
theorem an_omitted_output_node_yields_no_output (graph : Graph) (schedule : Schedule)
    (inputs : Nat → BitVec 64) (order : List Nat)
    (unwritten : Unwritten schedule order schedule.outReg) :
    execSchedule graph schedule inputs order (fun _ => none) schedule.outReg = none :=
  an_unwritten_register_stays_empty graph schedule inputs order schedule.outReg unwritten

/-- A step computes its own node's graph value whenever its operands' registers
already hold their graph values. -/
theorem step_computes (graph : Graph) (hwf : WellFormed graph) (schedule : Schedule)
    (inputs : Nat → BitVec 64) (regs : Nat → Option Val) (index : Nat)
    (hreads : ∀ operand ∈ (graph.node index).operands,
      regs (schedule.regOf operand) = value graph inputs operand) :
    step inputs regs (emitNode graph schedule index) (schedule.regOf index) =
      value graph inputs index := by
  have hvalue : value graph inputs index =
      evalStep graph (valuesUpTo graph inputs index) inputs index := by
    simp [value, valuesUpTo]
  rw [hvalue]
  cases hn : graph.node index with
  | input slot => simp [emitNode, evalStep, step, hn]
  | constant bits => simp [emitNode, evalStep, step, hn]
  | add a b =>
      have ha : a ∈ (graph.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph.node index).operands := by simp [hn, Node.operands]
      simp [emitNode, evalStep, step, hn, hreads a ha, hreads b hb,
        valuesUpTo_eq_value graph inputs index a (hwf.2 index a ha),
        valuesUpTo_eq_value graph inputs index b (hwf.2 index b hb)]
  | sub a b =>
      have ha : a ∈ (graph.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph.node index).operands := by simp [hn, Node.operands]
      simp [emitNode, evalStep, step, hn, hreads a ha, hreads b hb,
        valuesUpTo_eq_value graph inputs index a (hwf.2 index a ha),
        valuesUpTo_eq_value graph inputs index b (hwf.2 index b hb)]
  | mul a b =>
      have ha : a ∈ (graph.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph.node index).operands := by simp [hn, Node.operands]
      simp [emitNode, evalStep, step, hn, hreads a ha, hreads b hb,
        valuesUpTo_eq_value graph inputs index a (hwf.2 index a ha),
        valuesUpTo_eq_value graph inputs index b (hwf.2 index b hb)]
  | eq a b =>
      have ha : a ∈ (graph.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph.node index).operands := by simp [hn, Node.operands]
      simp [emitNode, evalStep, step, hn, hreads a ha, hreads b hb,
        valuesUpTo_eq_value graph inputs index a (hwf.2 index a ha),
        valuesUpTo_eq_value graph inputs index b (hwf.2 index b hb)]

/-- Executing a scheduled order yields, for every processed node, its graph value
in its own register. `hdisjoint` says the already-processed nodes are not in the
remaining order, which is what keeps every remaining instruction away from their
registers under the schedule's injectivity. -/
theorem execSchedule_sound (graph : Graph) (hwf : WellFormed graph) (schedule : Schedule)
    (inputs : Nat → BitVec 64)
    (hinjective : ∀ a b : Nat, schedule.regOf a = schedule.regOf b → a = b) :
    ∀ (processed order : List Nat) (regs : Nat → Option Val),
      (∀ index ∈ processed, index ∉ order) →
      OrderOk graph processed order →
      order.Nodup →
      (∀ index ∈ processed, regs (schedule.regOf index) = value graph inputs index) →
      ∀ index ∈ order ++ processed,
        execSchedule graph schedule inputs order regs (schedule.regOf index) =
          value graph inputs index := by
  intro processed order
  induction order generalizing processed with
  | nil =>
      intro regs _ _ _ hregs index hmem
      simpa [execSchedule] using hregs index hmem
  | cons head rest ih =>
      intro regs hdisjoint horder hnodup hregs index hmem
      obtain ⟨hoperands, horder'⟩ := horder
      -- the head's operands are all processed here
      have hreads : ∀ operand ∈ (graph.node head).operands,
          regs (schedule.regOf operand) = value graph inputs operand :=
        fun operand hmem => hregs operand (hoperands operand hmem)
      have hhead : head ∉ processed := by
        intro hmem
        exact hdisjoint head hmem (by simp)
      have hnodup' : head ∉ rest := (List.nodup_cons.mp hnodup).1
      have hnodup_rest : rest.Nodup := (List.nodup_cons.mp hnodup).2
      -- registers of processed nodes are untouched by this step
      have hregs' : ∀ index ∈ head :: processed,
          step inputs regs (emitNode graph schedule head) (schedule.regOf index) =
            value graph inputs index := by
        intro index hmem
        rcases List.mem_cons.mp hmem with heq | hmem
        · rw [heq]
          exact step_computes graph hwf schedule inputs regs head hreads
        · have hne : index ≠ head := by
            intro heq
            exact hhead (heq ▸ hmem)
          have hreg_ne : schedule.regOf index ≠ schedule.regOf head := by
            intro heq
            exact hne (hinjective index head heq)
          rw [step_written graph schedule inputs regs head _ hreg_ne]
          exact hregs index hmem
      have hdisjoint' : ∀ index ∈ head :: processed, index ∉ rest := by
        intro index hmem
        rcases List.mem_cons.mp hmem with rfl | hmem
        · exact hnodup'
        · intro hin
          exact hdisjoint index hmem (by simp [hin])
      have hmem' : index ∈ rest ++ head :: processed := by
        rcases List.mem_cons.mp hmem with heq | hnode
        · exact List.mem_append.mpr (Or.inr (by rw [heq]; exact List.mem_cons_self))
        · rcases List.mem_append.mp hnode with hin | hin
          · exact List.mem_append.mpr (Or.inl hin)
          · exact List.mem_append.mpr (Or.inr (List.mem_cons.mpr (Or.inr hin)))
      simpa only [execSchedule] using
        ih (head :: processed) (step inputs regs (emitNode graph schedule head))
          hdisjoint' horder' hnodup_rest hregs' index hmem'

/-- Complete ordered result equivalence for the validated schedule: the selected
output's register holds the graph's value for every admitted input. -/
theorem schedule_sound (graph : Graph) (hwf : WellFormed graph) (schedule : Schedule)
    (inputs : Nat → BitVec 64)
    (hinjective : ∀ a b : Nat, schedule.regOf a = schedule.regOf b → a = b)
    (horder : OrderOk graph [] schedule.order)
    (hnodup : schedule.order.Nodup)
    (houtput : graph.output ∈ schedule.order)
    (hout : schedule.outReg = schedule.regOf graph.output) :
    run inputs (emitNodes graph schedule schedule.order) schedule.outReg =
      value graph inputs graph.output := by
  have hbridge₁ := run_emitNodes_eq graph schedule inputs schedule.order
  have hbridge₂ := foldl_eq_execSchedule graph schedule inputs schedule.order (fun _ => none)
  rw [hbridge₁, hbridge₂, hout]
  exact execSchedule_sound graph hwf schedule inputs hinjective [] schedule.order
    (fun _ => none)
    (fun x (hx : x ∈ ([] : List Nat)) => by simp at hx)
    horder
    hnodup
    (fun x (hx : x ∈ ([] : List Nat)) => by simp at hx)
    graph.output (by simpa using houtput)

/-! ### Fixtures and a control

A small graph: input slot 0, exact constant, one addition and one equality on the
result, with the equality as the selected output. Values under `inputs 0 = 3`:
`3` at slot 0, `5` at the constant, `8` at the addition, `true` at the equality. -/
def exampleGraph : Graph where
  arity := 4
  output := 3
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .constant 5
    else if index = 2 then .add 0 1
    else .eq 2 2

theorem exampleGraph_wellFormed : WellFormed exampleGraph := by
  refine ⟨by decide, ?_⟩
  intro index operand hmem
  by_cases h0 : index = 0
  · subst h0; simp [exampleGraph, Node.operands] at hmem
  by_cases h1 : index = 1
  · subst h1; simp [exampleGraph, Node.operands] at hmem
  by_cases h2 : index = 2
  · subst h2; simp [exampleGraph, Node.operands] at hmem; omega
  · simp [exampleGraph, h0, h1, h2, Node.operands] at hmem; omega

/-- Registers one per node, in dependency order, with the equality's register as
the output: the schedule the validator accepts. -/
def exampleSchedule : Schedule where
  order := [0, 1, 2, 3]
  regOf := fun index => index
  outReg := 3

theorem orderOk_cons (graph : Graph) (processed : List Nat) (index : Nat) (rest : List Nat) :
    OrderOk graph processed (index :: rest) ↔
      ((∀ operand ∈ (graph.node index).operands, operand ∈ processed) ∧
        OrderOk graph (index :: processed) rest) :=
  Iff.rfl

theorem exampleSchedule_valid : OrderOk exampleGraph [] exampleSchedule.order ∧
    exampleSchedule.order.Nodup := by
  refine ⟨?_, by decide⟩
  change OrderOk exampleGraph [] [0, 1, 2, 3]
  rw [orderOk_cons, orderOk_cons, orderOk_cons, orderOk_cons]
  refine ⟨by simp [exampleGraph, Node.operands], by simp [exampleGraph, Node.operands],
    by simp [exampleGraph, Node.operands], by simp [exampleGraph, Node.operands],
    trivial⟩

theorem exampleGraph_values :
    value exampleGraph (fun _ => 3) 0 = some (.i64 3) ∧
      value exampleGraph (fun _ => 3) 2 = some (.i64 8) ∧
      value exampleGraph (fun _ => 3) 3 = some (.boolean true) := by
  refine ⟨?_, ?_, ?_⟩ <;> simp [value, valuesUpTo, evalStep, exampleGraph]

/-- The packed machine agrees with the graph reference on the validated schedule. -/
theorem exampleGraph_schedule_sound :
    run (fun _ => 3) (emitNodes exampleGraph exampleSchedule exampleSchedule.order)
      exampleSchedule.outReg = value exampleGraph (fun _ => 3) 3 :=
  schedule_sound exampleGraph exampleGraph_wellFormed exampleSchedule (fun _ => 3)
    (fun _ _ h => h) exampleSchedule_valid.1 exampleSchedule_valid.2
    (by decide) rfl

/-- The same graph's equality node reading its own register twice is fine, but
**aliasing two distinct nodes onto one register loses a value**: the addition
overwrites the constant's register, so the equality reads `3` twice and the
machine result differs from the graph reference. This is why the schedule relation
carries injectivity as a premise instead of assuming fresh registers. -/
def aliasedSchedule : Schedule where
  order := [0, 1, 2, 3]
  regOf := fun index => if index = 1 then 0 else index
  outReg := 3

def addOutputGraph : Graph :=
  { exampleGraph with output := 2 }

theorem aliasing_loses_the_constant :
    run (fun _ => 3) (emitNodes addOutputGraph aliasedSchedule aliasedSchedule.order) 2 ≠
      value addOutputGraph (fun _ => 3) 2 := by
  decide

end NobleFoundation.PackedSchedule
