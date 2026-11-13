/-
Guarded packed I64/Bool schedules (issue #43, deliverable 6).

`PackedSchedule.lean` validates an unguarded instruction stream: `bodiesMatch`, definition-before-use,
register injection and the output map, with no guard anywhere. A real emitted stream carries guards
(`crates/noble-ngraph/src/schedule.rs`, `I64Instruction::Guard { node }`), and the Rust observation
distinguishes a refusal from a result: `eval::reference` visits nodes in canonical order and stops at
the first failing guard, while `Compiled::evaluate` executes layers in layer order and stops early
only when `fail_suffix_min` proves no later layer can hold a smaller canonical index.

This file names the model side of that lane, in three parts:

1. **The guarded reference and machine.** `referenceObs` is "the first failing guard in canonical
   order, else the output values". `firstFailure` is the canonical-order scan the reference
   interpreter performs, `failingGraph` lists exactly the failing guards in canonical order, and
   `firstFailure_eq_minimum` proves the scan is the minimum canonical index over that list.
   `machineRun` executes a supplied stream — guards write no register — and `machineObs` reads each
   accepted guard's operand register from the resulting machine state, reporting the smallest
   failing canonical index or the output registers when no guard failed. `validate_machineValue_sound`
   proves the guarded reads match the graph values; `validate_sound` proves the accepted stream's
   observation equals the reference's for every input, under the same explicit `WellFormed` graph
   premise the unguarded model takes.

2. **The layer loop's early exit.** `runLayersWith` is the packed layer loop: it accumulates the
   minimum failing canonical index over the layers it has executed and stops when that minimum is
   below every possible guard identity in the remaining layers. `runLayersWithTable_correct` proves
   that loop, given the graph-derived strict suffix minima, returns exactly the minimum over every
   layer — the `fail_suffix_min` relation `crates/noble-ngraph/src/compiled.rs` relies on. The
   control `example_smoothed_suffix_reports_the_later_guard` shows that an incorrect table can predict
   no later failure, stop early, and report the canonically *later* guard, which is why `layout::check`
   refuses any table other than the graph's own.

3. **The composition.** `layeredObs_sound` composes the two: a validated stream whose layer guards
   flatten back to the stream's guards reports, through the early-exit loop over packed register
   values, the reference's first failing guard (or the stream's output registers).

The graph premise carries one obligation the unguarded model does not need: no operand is a guard.
A guard writes no register, so a computing node that read one would have no reference value to read;
the Rust checker enforces the same thing by type, and `PackedLayout` declares a guard's register as
`u32::MAX`, which `schedule::check` refuses to read.

What is *not* here: `F32` (deferred to an integrator-filed follow-up after #38's promotion, recorded
in `verification/ngraph/README.md`),
refusing `index` reads (they are `F32`-only in the emitted I64 stream), instruction bodies for such
nodes, decoding, and any theorem about the Rust executor. `validate` is a relation over a *supplied*
order and stream; the Rust validator recomputes the node list, so its completeness is structural.
Here guard completeness and placement follow from complete order plus exact node-body matching.
-/
import NobleFoundation.PackedSchedule

namespace NobleFoundation.PackedScheduleGuard

open NobleFoundation.PackedSchedule (Val)

/-! ### The guarded graph -/

/-- One node: the unguarded fragment's nodes plus a guard on another node's value. -/
inductive Node where
  | input (slot : Nat)
  | constant (bits : BitVec 64)
  | add (a b : Nat)
  | sub (a b : Nat)
  | mul (a b : Nat)
  | eq (a b : Nat)
  /-- The guard fails unless its operand's value is a bit (`Fact::Bit`): an `I64` in `{0, 1}` or
  any `Bool`. -/
  | guard (value : Nat)
deriving DecidableEq, Repr

/-- Operand indices in stored order. -/
def Node.operands : Node → List Nat
  | .add a b => [a, b]
  | .sub a b => [a, b]
  | .mul a b => [a, b]
  | .eq a b => [a, b]
  | .guard value => [value]
  | _ => []

/-- A guard writes no register; every other node does. -/
def Node.isGuard : Node → Bool
  | .guard _ => true
  | _ => false

/-- The nodes that hold a register. -/
def Node.holdsRegister (node : Node) : Bool := !node.isGuard

/-- The guarded node of a guard, if this node is one. -/
def Node.guarded : Node → Option Nat
  | .guard value => some value
  | _ => none

theorem holdsRegister_eq_false (node : Node) :
    node.holdsRegister = false ↔ node.isGuard = true := by
  cases node <;> simp [Node.holdsRegister, Node.isGuard]

/-- A finite graph over `count` nodes. -/
structure Graph where
  count : Nat
  node : Nat → Node

/-- Every node below `count` has strictly earlier operands and no guard operand. -/
def wellFormedBool (graph : Graph) : Bool :=
  (List.range graph.count).all fun index =>
    (graph.node index).operands.all fun operand =>
      decide (operand < index) && (graph.node operand).holdsRegister

/-- The graph premise every soundness statement takes. -/
def WellFormed (graph : Graph) : Prop :=
  wellFormedBool graph = true

theorem wellFormed_operands (graph : Graph) (wellFormed : WellFormed graph)
    (index : Nat) (bounded : index < graph.count) :
    ((graph.node index).operands.all fun operand =>
      decide (operand < index) && (graph.node operand).holdsRegister) = true :=
  (List.all_eq_true.mp wellFormed) index (List.mem_range.mpr bounded)

theorem wellFormed_operand_lt (graph : Graph) (wellFormed : WellFormed graph)
    (index operand : Nat) (bounded : index < graph.count)
    (member : operand ∈ (graph.node index).operands) : operand < index :=
  of_decide_eq_true (Bool.and_eq_true_iff.mp
    ((List.all_eq_true.mp (wellFormed_operands graph wellFormed index bounded)) operand member)).1

theorem wellFormed_operand_holds (graph : Graph) (wellFormed : WellFormed graph)
    (index operand : Nat) (bounded : index < graph.count)
    (member : operand ∈ (graph.node index).operands) :
    (graph.node operand).holdsRegister = true :=
  (Bool.and_eq_true_iff.mp
    ((List.all_eq_true.mp (wellFormed_operands graph wellFormed index bounded)) operand member)).2

/-! ### The reference semantics -/

/-- `Fact::Bit` on a value: an `I64` in `{0, 1}`, or any `Bool`. -/
def bitHolds : Val → Bool
  | .i64 value => decide (value = 0 ∨ value = 1)
  | .boolean _ => true

/-- A guard holds when its operand's register holds a bit. -/
def guardHolds (regs : Nat → Option Val) (operand : Nat) : Bool :=
  match regs operand with
  | some value => bitHolds value
  | none => false

/-- The graph reference semantics of one node. A guard's value is `true` when it holds and no value
when it fails, matching the reference interpreter, which returns before evaluating later nodes. -/
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
  | .guard value =>
      match values value with
      | some held => if bitHolds held then some (.boolean true) else none
      | none => none

/-- The graph reference semantics of the first `count` nodes. -/
def valuesUpTo (graph : Graph) (inputs : Nat → BitVec 64) : Nat → (Nat → Option Val)
  | 0 => fun _ => none
  | count + 1 => fun index =>
      if index = count then evalStep graph (valuesUpTo graph inputs count) inputs count
      else valuesUpTo graph inputs count index

/-- The graph reference value of one node. -/
def value (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat) : Option Val :=
  valuesUpTo graph inputs (index + 1) index

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
        simp only [valuesUpTo, ite_eq_right hlast]
        exact ih index hlt

/-- A reference guard holds exactly when its operand's reference value is a bit. -/
def guardPasses (graph : Graph) (inputs : Nat → BitVec 64) (operand : Nat) : Bool :=
  guardHolds (fun index => value graph inputs index) operand

/-- Does the guard at `index` fail? False for every node that is not a guard. -/
def guardFails (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat) : Bool :=
  match (graph.node index).guarded with
  | some operand =>
      match guardPasses graph inputs operand with
      | true => false
      | false => true
  | none => false

/-- A packed guard reads the operand's value register, as `Compiled::guard_fails` does. -/
def guardFailsInRegisters (graph : Graph) (regOf : Nat → Nat)
    (regs : Nat → Option Val) (index : Nat) : Bool :=
  match graph.node index with
  | .guard operand => !guardHolds regs (regOf operand)
  | _ => false

theorem guardFailsInRegisters_isGuard (graph : Graph) (regOf : Nat → Nat)
    (regs : Nat → Option Val) (index : Nat)
    (fails : guardFailsInRegisters graph regOf regs index = true) :
    (graph.node index).isGuard = true := by
  cases node : graph.node index with
  | guard operand => rfl
  | input slot => simp [guardFailsInRegisters, node] at fails
  | constant bits => simp [guardFailsInRegisters, node] at fails
  | add left right => simp [guardFailsInRegisters, node] at fails
  | sub left right => simp [guardFailsInRegisters, node] at fails
  | mul left right => simp [guardFailsInRegisters, node] at fails
  | eq left right => simp [guardFailsInRegisters, node] at fails

/-- The failing guards below `count`, in canonical order: the head of this list is the first failing
guard in canonical order, which is the observation `eval::reference` reports. -/
def failingFrom (graph : Graph) (inputs : Nat → BitVec 64) : Nat → Nat → List Nat
  | 0, _ => []
  | fuel + 1, index =>
      (match guardFails graph inputs index with
       | true => [index]
       | false => []) ++ failingFrom graph inputs fuel (index + 1)

/-- The failing guards of the graph, in canonical order. -/
def failingGraph (graph : Graph) (inputs : Nat → BitVec 64) : List Nat :=
  failingFrom graph inputs graph.count 0

/-- The canonical-order scan the reference interpreter performs: stop at the first failing guard. -/
def firstFailureFrom (graph : Graph) (inputs : Nat → BitVec 64) : Nat → Nat → Option Nat
  | 0, _ => none
  | fuel + 1, index =>
      match guardFails graph inputs index with
      | true => some index
      | false => firstFailureFrom graph inputs fuel (index + 1)

/-- The first failing guard of the graph in canonical order. -/
def firstFailure (graph : Graph) (inputs : Nat → BitVec 64) : Option Nat :=
  firstFailureFrom graph inputs graph.count 0

/-! ### Observations -/

/-- What a run observes: a failing guard's canonical node, or the output values. -/
inductive Obs where
  | failed (node : Nat)
  | normal (outputs : List (Option Val))
deriving DecidableEq, Repr

/-- The reference observation: the first failing guard in canonical order, else the outputs. -/
def referenceObs (graph : Graph) (inputs : Nat → BitVec 64) (outputs : List Nat) : Obs :=
  match firstFailure graph inputs with
  | some node => .failed node
  | none => .normal (outputs.map (value graph inputs))

/-! ### Minima

The packed evaluator reports the smallest canonical index among the failures it saw, so every
statement below is about an `Option Nat` minimum: `none` is "no failure" and absorbs into any value. -/

/-- The smaller of two optional canonical indices. -/
def minNat (left : Option Nat) (right : Option Nat) : Option Nat :=
  match left, right with
  | none, other => other
  | other, none => other
  | some left, some right => some (min left right)

/-- The minimum of a list of canonical indices. -/
def minimum (values : List Nat) : Option Nat :=
  values.foldl (fun acc value => minNat acc (some value)) none

theorem minNat_comm (left right : Option Nat) : minNat left right = minNat right left := by
  cases left <;> cases right <;> simp [minNat, Nat.min_comm]

theorem minNat_assoc (first second third : Option Nat) :
    minNat (minNat first second) third = minNat first (minNat second third) := by
  cases first <;> cases second <;> cases third <;> simp [minNat, Nat.min_assoc]

theorem minNat_none_left (value : Option Nat) : minNat none value = value := rfl

theorem minNat_none_right (value : Option Nat) : minNat value none = value := by
  cases value <;> rfl

theorem minNat_some_some (first second : Nat) :
    minNat (some first) (some second) = some (min first second) := rfl

theorem foldl_minNat (values : List Nat) :
    ∀ acc, values.foldl (fun acc value => minNat acc (some value)) acc =
      minNat acc (values.foldl (fun acc value => minNat acc (some value)) none) := by
  intro acc
  induction values generalizing acc with
  | nil => cases acc <;> rfl
  | cons head rest ih =>
      show rest.foldl (fun acc value => minNat acc (some value)) (minNat acc (some head)) =
        minNat acc (rest.foldl (fun acc value => minNat acc (some value)) (minNat none (some head)))
      rw [minNat_none_left, ih (minNat acc (some head)), ih (some head), minNat_assoc]

theorem minimum_cons (head : Nat) (rest : List Nat) :
    minimum (head :: rest) = minNat (some head) (minimum rest) := by
  unfold minimum
  show rest.foldl (fun acc value => minNat acc (some value)) (minNat none (some head)) =
    minNat (some head) (rest.foldl (fun acc value => minNat acc (some value)) none)
  rw [minNat_none_left, foldl_minNat]

theorem minimum_none_iff {values : List Nat} : minimum values = none ↔ values = [] := by
  constructor
  · intro empty
    cases values with
    | nil => rfl
    | cons head rest =>
        rw [minimum_cons] at empty
        cases hrest : minimum rest with
        | none =>
            rw [hrest, minNat_none_right] at empty
            simp at empty
        | some other =>
            rw [hrest, minNat_some_some] at empty
            simp at empty
  · intro nil
    subst nil
    rfl

theorem minimum_append (first second : List Nat) :
    minimum (first ++ second) = minNat (minimum first) (minimum second) := by
  unfold minimum
  rw [List.foldl_append, foldl_minNat]

theorem minimum_singleton (value : Nat) : minimum [value] = some value := by
  rw [minimum_cons]
  exact minNat_none_right (some value)

theorem minimum_mem {values : List Nat} {least : Nat} (found : minimum values = some least) :
    least ∈ values := by
  induction values with
  | nil => simp [minimum] at found
  | cons head rest ih =>
      rw [minimum_cons] at found
      cases hrest : minimum rest with
      | none =>
          rw [hrest, minNat_none_right] at found
          exact (Option.some.inj found).symm ▸ List.mem_cons_self
      | some other =>
          rw [hrest, minNat_some_some] at found
          have hmin : min head other = least := Option.some.inj found
          by_cases hle : head ≤ other
          · rw [Nat.min_eq_left hle] at hmin
            exact hmin.symm ▸ List.mem_cons_self
          · rw [Nat.min_eq_right (Nat.le_of_lt (Nat.lt_of_not_le hle))] at hmin
            exact List.mem_cons_of_mem _ (ih (hmin.symm ▸ hrest))

theorem minimum_le {values : List Nat} {least value : Nat} (member : value ∈ values)
    (found : minimum values = some least) : least ≤ value := by
  induction values generalizing least with
  | nil => simp at member
  | cons head rest ih =>
      rw [minimum_cons] at found
      cases hrest : minimum rest with
      | none =>
          rw [hrest, minNat_none_right] at found
          have hleast : head = least := Option.some.inj found
          rcases List.mem_cons.mp member with same | later
          · rw [same, hleast]
            exact Nat.le_refl least
          · have empty : rest = [] := minimum_none_iff.mp hrest
            rw [empty] at later
            simp at later
      | some other =>
          rw [hrest, minNat_some_some] at found
          have hmin : min head other = least := Option.some.inj found
          rcases List.mem_cons.mp member with same | later
          · rw [same, ← hmin]
            exact Nat.min_le_left head other
          · have bound : least ≤ other := by
              rw [← hmin]
              exact Nat.min_le_right head other
            exact Nat.le_trans bound (ih later hrest)

/-- A minimum smaller than every element is preserved when a list is folded in. -/
theorem minNat_some_of_gt {values : List Nat} {least : Nat}
    (greater : ∀ value ∈ values, least < value) :
    minNat (some least) (minimum values) = some least := by
  cases hvalues : minimum values with
  | none => rfl
  | some other =>
      have member := minimum_mem hvalues
      have strict := greater other member
      have hmin : min least other = least := Nat.min_eq_left (Nat.le_of_lt strict)
      rw [minNat_some_some, hmin]

/-! ### The failing-guard list -/

theorem guardFails_isGuard (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (fails : guardFails graph inputs index = true) : (graph.node index).isGuard = true := by
  cases hn : graph.node index with
  | guard operand => rfl
  | input slot => simp [guardFails, Node.guarded, hn] at fails
  | constant bits => simp [guardFails, Node.guarded, hn] at fails
  | add a b => simp [guardFails, Node.guarded, hn] at fails
  | sub a b => simp [guardFails, Node.guarded, hn] at fails
  | mul a b => simp [guardFails, Node.guarded, hn] at fails
  | eq a b => simp [guardFails, Node.guarded, hn] at fails

theorem guardFails_guard (graph : Graph) (inputs : Nat → BitVec 64) (index operand : Nat)
    (hn : graph.node index = .guard operand) :
    guardFails graph inputs index = !(guardPasses graph inputs operand) := by
  cases holds : guardPasses graph inputs operand <;> simp [guardFails, Node.guarded, hn, holds]

theorem guardFails_of_holdsRegister (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (held : (graph.node index).holdsRegister = true) : guardFails graph inputs index = false := by
  cases fails : guardFails graph inputs index
  · rfl
  · have guard := guardFails_isGuard graph inputs index fails
    rw [Node.holdsRegister, guard] at held
    simp at held

theorem failingFrom_lt (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ fuel start value, value ∈ failingFrom graph inputs fuel start → value < start + fuel := by
  intro fuel
  induction fuel with
  | zero => intro start value member; simp [failingFrom] at member
  | succ fuel ih =>
      intro start value member
      have unfold : failingFrom graph inputs (fuel + 1) start =
          (match guardFails graph inputs start with
           | true => [start]
           | false => []) ++ failingFrom graph inputs fuel (start + 1) := rfl
      rw [unfold] at member
      rcases List.mem_append.mp member with head | later
      · cases fails : guardFails graph inputs start with
        | true =>
            rw [fails] at head
            have same : value = start := List.mem_singleton.mp head
            omega
        | false =>
            rw [fails] at head
            simp at head
      · have := ih (start + 1) value later
        omega

theorem failingFrom_ge (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ fuel start value, value ∈ failingFrom graph inputs fuel start → start ≤ value := by
  intro fuel
  induction fuel with
  | zero => intro start value member; simp [failingFrom] at member
  | succ fuel ih =>
      intro start value member
      have unfold : failingFrom graph inputs (fuel + 1) start =
          (match guardFails graph inputs start with
           | true => [start]
           | false => []) ++ failingFrom graph inputs fuel (start + 1) := rfl
      rw [unfold] at member
      rcases List.mem_append.mp member with head | later
      · cases fails : guardFails graph inputs start with
        | true =>
            rw [fails] at head
            have same : value = start := List.mem_singleton.mp head
            omega
        | false =>
            rw [fails] at head
            simp at head
      · exact Nat.le_trans (Nat.le_succ start) (ih (start + 1) value later)

theorem failingFrom_fails (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ fuel start value, value ∈ failingFrom graph inputs fuel start →
      guardFails graph inputs value = true := by
  intro fuel
  induction fuel with
  | zero => intro start value member; simp [failingFrom] at member
  | succ fuel ih =>
      intro start value member
      have unfold : failingFrom graph inputs (fuel + 1) start =
          (match guardFails graph inputs start with
           | true => [start]
           | false => []) ++ failingFrom graph inputs fuel (start + 1) := rfl
      rw [unfold] at member
      rcases List.mem_append.mp member with head | later
      · cases fails : guardFails graph inputs start with
        | true =>
            rw [fails] at head
            have same : value = start := List.mem_singleton.mp head
            rw [same]
            exact fails
        | false =>
            rw [fails] at head
            simp at head
      · exact ih (start + 1) value later

theorem failingFrom_isGuard (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ fuel start value, value ∈ failingFrom graph inputs fuel start →
      (graph.node value).isGuard = true := by
  intro fuel start value member
  exact guardFails_isGuard graph inputs value (failingFrom_fails graph inputs fuel start value member)

theorem failingFrom_mem (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ fuel start value, start ≤ value → value < start + fuel →
      guardFails graph inputs value = true → value ∈ failingFrom graph inputs fuel start := by
  intro fuel
  induction fuel with
  | zero => intro start value lower upper fails; omega
  | succ fuel ih =>
      intro start value lower upper fails
      have unfold : failingFrom graph inputs (fuel + 1) start =
          (match guardFails graph inputs start with
           | true => [start]
           | false => []) ++ failingFrom graph inputs fuel (start + 1) := rfl
      rw [unfold]
      by_cases same : value = start
      · subst same
        rw [fails]
        exact List.mem_append.mpr (Or.inl (List.mem_singleton_self value))
      · exact List.mem_append.mpr (Or.inr (ih (start + 1) value (by omega) (by omega) fails))

theorem failingGraph_mem (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (bounded : index < graph.count) (fails : guardFails graph inputs index = true) :
    index ∈ failingGraph graph inputs :=
  failingFrom_mem graph inputs graph.count 0 index (Nat.zero_le index) (by omega) fails

theorem failingGraph_lt (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (member : index ∈ failingGraph graph inputs) : index < graph.count := by
  have := failingFrom_lt graph inputs graph.count 0 index member
  omega

theorem failingGraph_fails (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (member : index ∈ failingGraph graph inputs) : guardFails graph inputs index = true :=
  failingFrom_fails graph inputs graph.count 0 index member

theorem failingGraph_isGuard (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (member : index ∈ failingGraph graph inputs) : (graph.node index).isGuard = true :=
  failingFrom_isGuard graph inputs graph.count 0 index member

theorem failingFrom_cons_of_fails (graph : Graph) (inputs : Nat → BitVec 64)
    (fuel start : Nat) (fails : guardFails graph inputs start = true) :
    failingFrom graph inputs (fuel + 1) start =
      start :: failingFrom graph inputs fuel (start + 1) := by
  show (match guardFails graph inputs start with
        | true => [start]
        | false => []) ++ failingFrom graph inputs fuel (start + 1) = _
  rw [fails]
  rfl

theorem failingFrom_cons_of_holds (graph : Graph) (inputs : Nat → BitVec 64)
    (fuel start : Nat) (holds : guardFails graph inputs start = false) :
    failingFrom graph inputs (fuel + 1) start =
      failingFrom graph inputs fuel (start + 1) := by
  show (match guardFails graph inputs start with
        | true => [start]
        | false => []) ++ failingFrom graph inputs fuel (start + 1) = _
  simp [holds]

theorem firstFailureFrom_cons_of_fails (graph : Graph) (inputs : Nat → BitVec 64)
    (fuel start : Nat) (fails : guardFails graph inputs start = true) :
    firstFailureFrom graph inputs (fuel + 1) start = some start := by
  show (match guardFails graph inputs start with
        | true => some start
        | false => firstFailureFrom graph inputs fuel (start + 1)) = _
  rw [fails]

theorem firstFailureFrom_cons_of_holds (graph : Graph) (inputs : Nat → BitVec 64)
    (fuel start : Nat) (holds : guardFails graph inputs start = false) :
    firstFailureFrom graph inputs (fuel + 1) start =
      firstFailureFrom graph inputs fuel (start + 1) := by
  show (match guardFails graph inputs start with
        | true => some start
        | false => firstFailureFrom graph inputs fuel (start + 1)) = _
  rw [holds]

theorem firstFailureFrom_eq_minimum (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ fuel start,
      firstFailureFrom graph inputs fuel start =
        minimum (failingFrom graph inputs fuel start) := by
  intro fuel
  induction fuel with
  | zero => intro start; rfl
  | succ fuel ih =>
      intro start
      by_cases fails : guardFails graph inputs start = true
      · rw [firstFailureFrom_cons_of_fails graph inputs fuel start fails,
            failingFrom_cons_of_fails graph inputs fuel start fails,
            minimum_cons]
        have absent : ∀ value ∈ failingFrom graph inputs fuel (start + 1), start < value := by
          intro value member
          have lower := failingFrom_ge graph inputs fuel (start + 1) value member
          omega
        exact (minNat_some_of_gt absent).symm
      · have holds : guardFails graph inputs start = false := by
          cases equal : guardFails graph inputs start
          · rfl
          · exact absurd equal fails
        rw [firstFailureFrom_cons_of_holds graph inputs fuel start holds,
            failingFrom_cons_of_holds graph inputs fuel start holds]
        exact ih (start + 1)

/-- The canonical-order scan and the minimum over the failing-guard list are the same observation. -/
theorem firstFailure_eq_minimum (graph : Graph) (inputs : Nat → BitVec 64) :
    firstFailure graph inputs = minimum (failingGraph graph inputs) :=
  firstFailureFrom_eq_minimum graph inputs graph.count 0

/-! ### Supplied guarded schedules -/

/-- One instruction in the guarded I64/Bool stream. Guard instructions name their canonical node
and write no register. -/
inductive Instr where
  | setInput (reg slot : Nat)
  | setConst (reg : Nat) (bits : BitVec 64)
  | add (dst left right : Nat)
  | sub (dst left right : Nat)
  | mul (dst left right : Nat)
  | eq (dst left right : Nat)
  | guard (node : Nat)
deriving DecidableEq, Repr

/-- One guarded packed-machine step. A guard observes its named canonical node in the guard
observation below and writes no register; every value-producing instruction has the ordinary
packed I64/Bool register effect. -/
def machineStep (inputs : Nat → BitVec 64) (regs : Nat → Option Val) : Instr → Nat → Option Val
  | .setInput reg slot => NobleFoundation.PackedSchedule.step inputs regs (.setInput reg slot)
  | .setConst reg bits => NobleFoundation.PackedSchedule.step inputs regs (.setConst reg bits)
  | .add dst left right => NobleFoundation.PackedSchedule.step inputs regs (.add dst left right)
  | .sub dst left right => NobleFoundation.PackedSchedule.step inputs regs (.sub dst left right)
  | .mul dst left right => NobleFoundation.PackedSchedule.step inputs regs (.mul dst left right)
  | .eq dst left right => NobleFoundation.PackedSchedule.step inputs regs (.eq dst left right)
  | .guard _ => regs

/-- Execute the supplied instructions from an empty register file. Guards themselves are no-ops;
their pass/fail observation is accumulated separately from register execution. -/
def machineRun (inputs : Nat → BitVec 64) (instrs : List Instr) : Nat → Option Val :=
  instrs.foldl (fun regs instr => machineStep inputs regs instr) (fun _ => none)

/-- The values returned from the supplied output-register list. -/
def machineOutputs (inputs : Nat → BitVec 64) (instrs : List Instr) (outRegs : List Nat) :
    List (Option Val) :=
  outRegs.map (machineRun inputs instrs)

/-- Register renaming used only to reuse the unguarded machine theorem below. Real registers map
to even numbers and ghost guard registers to odd numbers, so a ghost constant at a guard cannot
affect a real output register. -/
def soundnessReg (reg : Nat) : Nat := 2 * reg + 2

theorem soundnessReg_injective {first second : Nat}
    (same : soundnessReg first = soundnessReg second) : first = second := by
  unfold soundnessReg at same
  omega

def soundnessGuardReg (node : Nat) : Nat := 2 * node + 1

def soundnessRegOf (graph : Graph) (regOf : Nat → Nat) (index : Nat) : Nat :=
  if (graph.node index).isGuard then soundnessGuardReg index else soundnessReg (regOf index)

/-- Replace guard nodes by ghost constants for the unguarded register-soundness proof. Guard
operands are excluded by `WellFormed`; out-of-range indices are harmless input nodes. -/
def soundnessNode : Node → NobleFoundation.PackedSchedule.Node
  | .input slot => .input slot
  | .constant bits => .constant bits
  | .add left right => .add left right
  | .sub left right => .sub left right
  | .mul left right => .mul left right
  | .eq left right => .eq left right
  | .guard _ => .constant 0

def soundnessGraph (graph : Graph) (output : Nat) : NobleFoundation.PackedSchedule.Graph where
  arity := graph.count
  output := output
  node := fun index =>
    if index < graph.count then soundnessNode (graph.node index) else .input 0

/-- Convert a real guarded instruction to the unguarded proof machine's register numbering. A
guard is ghost-written to its own odd register; this has no effect on even real registers. -/
def soundnessInstr : Instr → NobleFoundation.PackedSchedule.Instr
  | .setInput reg slot => .setInput (soundnessReg reg) slot
  | .setConst reg bits => .setConst (soundnessReg reg) bits
  | .add dst left right => .add (soundnessReg dst) (soundnessReg left) (soundnessReg right)
  | .sub dst left right => .sub (soundnessReg dst) (soundnessReg left) (soundnessReg right)
  | .mul dst left right => .mul (soundnessReg dst) (soundnessReg left) (soundnessReg right)
  | .eq dst left right => .eq (soundnessReg dst) (soundnessReg left) (soundnessReg right)
  | .guard node => .setConst (soundnessGuardReg node) 0

theorem soundnessStep_relation (inputs : Nat → BitVec 64) (regs : Nat → Option Val)
    (proofRegs : Nat → Option Val) (related : ∀ reg, proofRegs (soundnessReg reg) = regs reg)
    (instr : Instr) (reg : Nat) :
    NobleFoundation.PackedSchedule.step inputs proofRegs (soundnessInstr instr) (soundnessReg reg) =
      machineStep inputs regs instr reg := by
  cases instr with
  | setInput dst slot =>
      by_cases same : reg = dst
      · subst reg
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step]
      · have different : soundnessReg reg ≠ soundnessReg dst := by
          intro equal
          exact same (soundnessReg_injective equal)
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step, same, different,
          related reg]
  | setConst dst bits =>
      by_cases same : reg = dst
      · subst reg
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step]
      · have different : soundnessReg reg ≠ soundnessReg dst := by
          intro equal
          exact same (soundnessReg_injective equal)
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step, same, different,
          related reg]
  | add dst left right =>
      by_cases same : reg = dst
      · subst reg
        have leftRelated : proofRegs (soundnessReg left) = regs left := related left
        have rightRelated : proofRegs (soundnessReg right) = regs right := related right
        simp only [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step]
        rw [leftRelated, rightRelated]
        rfl
      · have different : soundnessReg reg ≠ soundnessReg dst := by
          intro equal
          exact same (soundnessReg_injective equal)
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step, same, different,
          related reg]
  | sub dst left right =>
      by_cases same : reg = dst
      · subst reg
        have leftRelated : proofRegs (soundnessReg left) = regs left := related left
        have rightRelated : proofRegs (soundnessReg right) = regs right := related right
        simp only [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step]
        rw [leftRelated, rightRelated]
        rfl
      · have different : soundnessReg reg ≠ soundnessReg dst := by
          intro equal
          exact same (soundnessReg_injective equal)
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step, same, different,
          related reg]
  | mul dst left right =>
      by_cases same : reg = dst
      · subst reg
        have leftRelated : proofRegs (soundnessReg left) = regs left := related left
        have rightRelated : proofRegs (soundnessReg right) = regs right := related right
        simp only [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step]
        rw [leftRelated, rightRelated]
        rfl
      · have different : soundnessReg reg ≠ soundnessReg dst := by
          intro equal
          exact same (soundnessReg_injective equal)
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step, same, different,
          related reg]
  | eq dst left right =>
      by_cases same : reg = dst
      · subst reg
        have leftRelated : proofRegs (soundnessReg left) = regs left := related left
        have rightRelated : proofRegs (soundnessReg right) = regs right := related right
        simp only [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step]
        rw [leftRelated, rightRelated]
        rfl
      · have different : soundnessReg reg ≠ soundnessReg dst := by
          intro equal
          exact same (soundnessReg_injective equal)
        simp [soundnessInstr, machineStep, NobleFoundation.PackedSchedule.step, same, different,
          related reg]
  | guard node =>
      change (if soundnessReg reg = soundnessGuardReg node then
        some (Val.i64 (0 : BitVec 64)) else proofRegs (soundnessReg reg)) = regs reg
      have different : soundnessReg reg ≠ soundnessGuardReg node := by
        unfold soundnessReg soundnessGuardReg
        omega
      simp [different, related reg]

theorem soundnessRun_relation (inputs : Nat → BitVec 64) (instrs : List Instr)
    (regs : Nat → Option Val) (proofRegs : Nat → Option Val)
    (related : ∀ reg, proofRegs (soundnessReg reg) = regs reg) :
    ∀ reg,
      (instrs.map soundnessInstr).foldl
          (fun state instr => NobleFoundation.PackedSchedule.step inputs state instr) proofRegs
          (soundnessReg reg) =
        instrs.foldl (fun state instr => machineStep inputs state instr) regs reg := by
  induction instrs generalizing regs proofRegs with
  | nil => exact related
  | cons instr rest ih =>
      intro reg
      simp only [List.map_cons, List.foldl_cons]
      apply ih (machineStep inputs regs instr)
        (NobleFoundation.PackedSchedule.step inputs proofRegs (soundnessInstr instr))
      intro index
      exact soundnessStep_relation inputs regs proofRegs related instr index

theorem soundnessRun_eq_machineRun (inputs : Nat → BitVec 64) (instrs : List Instr) :
    ∀ reg,
      NobleFoundation.PackedSchedule.run inputs (instrs.map soundnessInstr) (soundnessReg reg) =
        machineRun inputs instrs reg := by
  have related : ∀ reg, (fun _ : Nat => (none : Option Val)) (soundnessReg reg) =
      (fun _ : Nat => (none : Option Val)) reg := by
    intro reg
    rfl
  simpa [NobleFoundation.PackedSchedule.run, machineRun] using
    soundnessRun_relation inputs instrs (fun _ => none) (fun _ => none) related

/-- The instruction body prescribed by a node and the supplied register map. -/
def emitNode (graph : Graph) (regOf : Nat → Nat) (index : Nat) : Instr :=
  match graph.node index with
  | .input slot => .setInput (regOf index) slot
  | .constant bits => .setConst (regOf index) bits
  | .add left right => .add (regOf index) (regOf left) (regOf right)
  | .sub left right => .sub (regOf index) (regOf left) (regOf right)
  | .mul left right => .mul (regOf index) (regOf left) (regOf right)
  | .eq left right => .eq (regOf index) (regOf left) (regOf right)
  | .guard _ => .guard index

/-- Every supplied body is paired with the exact node it claims to execute. -/
def bodiesMatch : Graph → (Nat → Nat) → List Nat → List Instr → Bool
  | _, _, [], [] => true
  | graph, regOf, index :: rest, instr :: instrs =>
      decide (instr = emitNode graph regOf index) && bodiesMatch graph regOf rest instrs
  | _, _, _, _ => false

/-- Every node operand, including a guard's value, has already appeared in the order. -/
def operandsDefined : Graph → List Nat → List Nat → Bool
  | _, _, [] => true
  | graph, processed, index :: rest =>
      (graph.node index).operands.all (fun operand => decide (operand ∈ processed)) &&
        operandsDefined graph (index :: processed) rest

/-- Non-guard nodes have distinct registers. Guards are excluded because their register is the
packed layout's `u32::MAX` sentinel and they write no register. -/
def registersInjectiveOn (graph : Graph) (regOf : Nat → Nat) (order : List Nat) : Bool :=
  order.all fun first => order.all fun second =>
    decide ((graph.node first).isGuard = true ∨ (graph.node second).isGuard = true ∨
      first = second ∨ regOf first ≠ regOf second)

/-- Outputs are retained positionally and must name register-holding graph nodes. -/
def outputsMatch (graph : Graph) (regOf : Nat → Nat) (order outputs outRegs : List Nat) : Bool :=
  match outputs, outRegs with
  | [], [] => true
  | output :: rest, outReg :: outRest =>
      decide (output ∈ order) && decide ((graph.node output).holdsRegister = true) &&
        decide (regOf output = outReg) && outputsMatch graph regOf order rest outRest
  | _, _ => false

/-- The complete node order is checked explicitly; it may be reordered but cannot omit or repeat
a node. -/
def completeOrder (graph : Graph) (order : List Nat) : Bool :=
  (List.range graph.count).all fun index => decide (index ∈ order)

/-- Independent acceptance obligations for one supplied guarded stream. -/
def validateObligations (graph : Graph) (regOf : Nat → Nat) (order : List Nat)
    (instrs : List Instr) (outputs outRegs : List Nat) : List Bool :=
  [decide order.Nodup,
    order.all (fun index => decide (index < graph.count)),
    completeOrder graph order,
    bodiesMatch graph regOf order instrs,
    operandsDefined graph [] order,
    registersInjectiveOn graph regOf order,
    outputsMatch graph regOf order outputs outRegs]

/-- Validate a supplied guarded instruction stream without regenerating its order. -/
def validate (graph : Graph) (regOf : Nat → Nat) (order : List Nat)
    (instrs : List Instr) (outputs outRegs : List Nat) : Bool :=
  (validateObligations graph regOf order instrs outputs outRegs).all id

theorem soundnessNode_operands (node : Node) :
    (soundnessNode node).operands = if node.isGuard then [] else node.operands := by
  cases node <;> rfl

theorem soundnessRegOf_nonGuard (graph : Graph) (regOf : Nat → Nat) (index : Nat)
    (holds : (graph.node index).holdsRegister = true) :
    soundnessRegOf graph regOf index = soundnessReg (regOf index) := by
  have notGuard : (graph.node index).isGuard ≠ true := by
    intro guard
    have empty := (holdsRegister_eq_false (graph.node index)).mpr guard
    rw [holds] at empty
    cases empty
  cases isGuard : (graph.node index).isGuard with
  | false => simp [soundnessRegOf, isGuard]
  | true => exact False.elim (notGuard isGuard)

theorem soundnessRegOf_guard (graph : Graph) (regOf : Nat → Nat) (index : Nat)
    (guard : (graph.node index).isGuard = true) :
    soundnessRegOf graph regOf index = soundnessGuardReg index := by
  simp [soundnessRegOf, guard]

theorem soundnessGuardReg_injective {first second : Nat}
    (same : soundnessGuardReg first = soundnessGuardReg second) : first = second := by
  unfold soundnessGuardReg at same
  omega

theorem soundnessRegOf_injective (graph : Graph) (regOf : Nat → Nat) (order : List Nat)
    (injective : registersInjectiveOn graph regOf order = true)
    (first : Nat) (firstMember : first ∈ order) (second : Nat) (secondMember : second ∈ order) :
    first = second ∨ soundnessRegOf graph regOf first ≠ soundnessRegOf graph regOf second := by
  cases firstGuard : (graph.node first).isGuard with
  | true =>
      cases secondGuard : (graph.node second).isGuard with
      | true =>
          by_cases same : first = second
          · exact Or.inl same
          · right
            intro equal
            have sameGuard : soundnessGuardReg first = soundnessGuardReg second := by
              simpa [soundnessRegOf, firstGuard, secondGuard] using equal
            exact same (soundnessGuardReg_injective sameGuard)
      | false =>
          right
          intro equal
          have differentParity : soundnessGuardReg first = soundnessReg (regOf second) := by
            simpa [soundnessRegOf, firstGuard, secondGuard] using equal
          unfold soundnessGuardReg soundnessReg at differentParity
          omega
  | false =>
      cases secondGuard : (graph.node second).isGuard with
      | true =>
          right
          intro equal
          have differentParity : soundnessReg (regOf first) = soundnessGuardReg second := by
            simpa [soundnessRegOf, firstGuard, secondGuard] using equal
          unfold soundnessGuardReg soundnessReg at differentParity
          omega
      | false =>
          have firstChecks := (List.all_eq_true.mp injective) first firstMember
          have bothChecks := (List.all_eq_true.mp firstChecks) second secondMember
          have cases := of_decide_eq_true bothChecks
          rcases cases with firstIsGuard | secondIsGuard | same | differentRegisters
          · simp [firstGuard] at firstIsGuard
          · simp [secondGuard] at secondIsGuard
          · exact Or.inl same
          · right
            intro equal
            have equalRegisters : soundnessReg (regOf first) = soundnessReg (regOf second) := by
              simpa [soundnessRegOf, firstGuard, secondGuard] using equal
            exact differentRegisters (soundnessReg_injective equalRegisters)

theorem soundnessRegistersInjective (graph : Graph) (regOf : Nat → Nat) (order : List Nat)
    (injective : registersInjectiveOn graph regOf order = true) :
    NobleFoundation.PackedSchedule.registersInjectiveOn (soundnessRegOf graph regOf) order =
      true := by
  unfold NobleFoundation.PackedSchedule.registersInjectiveOn
  apply List.all_eq_true.mpr
  intro first firstMember
  apply List.all_eq_true.mpr
  intro second secondMember
  have distinct := soundnessRegOf_injective graph regOf order injective first firstMember
    second secondMember
  exact decide_eq_true_iff.mpr distinct

theorem soundnessOutputsMatch (graph : Graph) (regOf : Nat → Nat) (order outputs outRegs : List Nat)
    (matched : outputsMatch graph regOf order outputs outRegs = true) :
    NobleFoundation.PackedSchedule.outputsMatch (soundnessRegOf graph regOf) order outputs
      (outputs.map (soundnessRegOf graph regOf)) = true := by
  induction outputs generalizing outRegs with
  | nil =>
      cases outRegs <;> simp [NobleFoundation.PackedSchedule.outputsMatch]
  | cons output rest ih =>
      cases outRegs with
      | nil => simp [outputsMatch] at matched
      | cons outReg outRest =>
          have parts :
              ((decide (output ∈ order) = true ∧
                decide ((graph.node output).holdsRegister = true) = true) ∧
                decide (regOf output = outReg) = true) ∧
              outputsMatch graph regOf order rest outRest = true := by
            simpa only [outputsMatch, Bool.and_eq_true] using matched
          have member : output ∈ order := of_decide_eq_true parts.1.1.1
          simp [NobleFoundation.PackedSchedule.outputsMatch, member, ih outRest parts.2]

theorem soundnessOutputRegisters (graph : Graph) (regOf : Nat → Nat) (order outputs outRegs : List Nat)
    (matched : outputsMatch graph regOf order outputs outRegs = true) :
    outRegs.map soundnessReg = outputs.map (soundnessRegOf graph regOf) := by
  induction outputs generalizing outRegs with
  | nil =>
      cases outRegs <;> simp [outputsMatch] at matched ⊢
  | cons output rest ih =>
      cases outRegs with
      | nil => simp [outputsMatch] at matched
      | cons outReg outRest =>
          have parts :
              ((decide (output ∈ order) = true ∧
                decide ((graph.node output).holdsRegister = true) = true) ∧
                decide (regOf output = outReg) = true) ∧
              outputsMatch graph regOf order rest outRest = true := by
            simpa only [outputsMatch, Bool.and_eq_true] using matched
          have holds : (graph.node output).holdsRegister = true := of_decide_eq_true parts.1.1.2
          have sameReg : regOf output = outReg := of_decide_eq_true parts.1.2
          simp only [List.map_cons]
          rw [soundnessRegOf_nonGuard graph regOf output holds, sameReg]
          exact congrArg (fun values => soundnessReg outReg :: values) (ih outRest parts.2)

theorem soundnessGraph_wellFormed (graph : Graph) (output : Nat)
    (outputBounded : output < graph.count) (wellFormed : WellFormed graph) :
    NobleFoundation.PackedSchedule.WellFormed (soundnessGraph graph output) := by
  refine ⟨outputBounded, ?_⟩
  intro index operand member
  by_cases bounded : index < graph.count
  · have mapped : (soundnessGraph graph output).node index = soundnessNode (graph.node index) := by
      simp [soundnessGraph, bounded]
    rw [mapped, soundnessNode_operands] at member
    by_cases guard : (graph.node index).isGuard = true
    · simp [guard] at member
    · have original : operand ∈ (graph.node index).operands := by simpa [guard] using member
      exact wellFormed_operand_lt graph wellFormed index operand bounded original
  · simp [soundnessGraph, bounded,
      NobleFoundation.PackedSchedule.Node.operands] at member

theorem soundnessValue_eq_value (graph : Graph) (output : Nat)
    (wellFormed : WellFormed graph) (inputs : Nat → BitVec 64) :
    ∀ index, index < graph.count → (graph.node index).holdsRegister = true →
      NobleFoundation.PackedSchedule.value (soundnessGraph graph output) inputs index =
        value graph inputs index := by
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      intro bounded holds
      have mapped : (soundnessGraph graph output).node index = soundnessNode (graph.node index) := by
        simp [soundnessGraph, bounded]
      have baseValue :
          NobleFoundation.PackedSchedule.value (soundnessGraph graph output) inputs index =
            NobleFoundation.PackedSchedule.evalStep (soundnessGraph graph output)
              (NobleFoundation.PackedSchedule.valuesUpTo (soundnessGraph graph output) inputs index)
              inputs index := by
        simp [NobleFoundation.PackedSchedule.value, NobleFoundation.PackedSchedule.valuesUpTo]
      have referenceValue : value graph inputs index =
          evalStep graph (valuesUpTo graph inputs index) inputs index := by
        simp [value, valuesUpTo]
      cases node : graph.node index with
      | input slot =>
          rw [baseValue, referenceValue]
          unfold NobleFoundation.PackedSchedule.evalStep evalStep
          rw [mapped, node]
          simp [soundnessNode]
      | constant bits =>
          rw [baseValue, referenceValue]
          unfold NobleFoundation.PackedSchedule.evalStep evalStep
          rw [mapped, node]
          simp [soundnessNode]
      | add left right =>
          have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
          have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
          have leftBound := wellFormed_operand_lt graph wellFormed index left bounded leftMember
          have rightBound := wellFormed_operand_lt graph wellFormed index right bounded rightMember
          have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
          have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
          have leftEq := ih left leftBound (Nat.lt_trans leftBound bounded) leftHolds
          have rightEq := ih right rightBound (Nat.lt_trans rightBound bounded) rightHolds
          have leftBase :
              NobleFoundation.PackedSchedule.valuesUpTo (soundnessGraph graph output) inputs index left =
                NobleFoundation.PackedSchedule.value (soundnessGraph graph output) inputs left :=
            NobleFoundation.PackedSchedule.valuesUpTo_eq_value _ _ index left leftBound
          have rightBase :
              NobleFoundation.PackedSchedule.valuesUpTo (soundnessGraph graph output) inputs index right =
                NobleFoundation.PackedSchedule.value (soundnessGraph graph output) inputs right :=
            NobleFoundation.PackedSchedule.valuesUpTo_eq_value _ _ index right rightBound
          have leftReference : valuesUpTo graph inputs index left = value graph inputs left :=
            valuesUpTo_eq_value graph inputs index left leftBound
          have rightReference : valuesUpTo graph inputs index right = value graph inputs right :=
            valuesUpTo_eq_value graph inputs index right rightBound
          have stepEq :
              NobleFoundation.PackedSchedule.evalStep (soundnessGraph graph output)
                  (NobleFoundation.PackedSchedule.valuesUpTo (soundnessGraph graph output) inputs index)
                  inputs index =
                evalStep graph (valuesUpTo graph inputs index) inputs index := by
            unfold NobleFoundation.PackedSchedule.evalStep evalStep
            rw [mapped, node]
            simp only [soundnessNode]
            rw [leftBase, rightBase, leftReference, rightReference, leftEq, rightEq]
            rfl
          exact baseValue.trans (stepEq.trans referenceValue.symm)
      | sub left right =>
          have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
          have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
          have leftBound := wellFormed_operand_lt graph wellFormed index left bounded leftMember
          have rightBound := wellFormed_operand_lt graph wellFormed index right bounded rightMember
          have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
          have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
          have leftEq := ih left leftBound (Nat.lt_trans leftBound bounded) leftHolds
          have rightEq := ih right rightBound (Nat.lt_trans rightBound bounded) rightHolds
          have leftBase := NobleFoundation.PackedSchedule.valuesUpTo_eq_value
            (soundnessGraph graph output) inputs index left leftBound
          have rightBase := NobleFoundation.PackedSchedule.valuesUpTo_eq_value
            (soundnessGraph graph output) inputs index right rightBound
          have leftReference := valuesUpTo_eq_value graph inputs index left leftBound
          have rightReference := valuesUpTo_eq_value graph inputs index right rightBound
          have stepEq :
              NobleFoundation.PackedSchedule.evalStep (soundnessGraph graph output)
                  (NobleFoundation.PackedSchedule.valuesUpTo (soundnessGraph graph output) inputs index)
                  inputs index =
                evalStep graph (valuesUpTo graph inputs index) inputs index := by
            unfold NobleFoundation.PackedSchedule.evalStep evalStep
            rw [mapped, node]
            simp only [soundnessNode]
            rw [leftBase, rightBase, leftReference, rightReference, leftEq, rightEq]
            rfl
          exact baseValue.trans (stepEq.trans referenceValue.symm)
      | mul left right =>
          have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
          have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
          have leftBound := wellFormed_operand_lt graph wellFormed index left bounded leftMember
          have rightBound := wellFormed_operand_lt graph wellFormed index right bounded rightMember
          have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
          have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
          have leftEq := ih left leftBound (Nat.lt_trans leftBound bounded) leftHolds
          have rightEq := ih right rightBound (Nat.lt_trans rightBound bounded) rightHolds
          have leftBase := NobleFoundation.PackedSchedule.valuesUpTo_eq_value
            (soundnessGraph graph output) inputs index left leftBound
          have rightBase := NobleFoundation.PackedSchedule.valuesUpTo_eq_value
            (soundnessGraph graph output) inputs index right rightBound
          have leftReference := valuesUpTo_eq_value graph inputs index left leftBound
          have rightReference := valuesUpTo_eq_value graph inputs index right rightBound
          have stepEq :
              NobleFoundation.PackedSchedule.evalStep (soundnessGraph graph output)
                  (NobleFoundation.PackedSchedule.valuesUpTo (soundnessGraph graph output) inputs index)
                  inputs index =
                evalStep graph (valuesUpTo graph inputs index) inputs index := by
            unfold NobleFoundation.PackedSchedule.evalStep evalStep
            rw [mapped, node]
            simp only [soundnessNode]
            rw [leftBase, rightBase, leftReference, rightReference, leftEq, rightEq]
            rfl
          exact baseValue.trans (stepEq.trans referenceValue.symm)
      | eq left right =>
          have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
          have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
          have leftBound := wellFormed_operand_lt graph wellFormed index left bounded leftMember
          have rightBound := wellFormed_operand_lt graph wellFormed index right bounded rightMember
          have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
          have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
          have leftEq := ih left leftBound (Nat.lt_trans leftBound bounded) leftHolds
          have rightEq := ih right rightBound (Nat.lt_trans rightBound bounded) rightHolds
          have leftBase := NobleFoundation.PackedSchedule.valuesUpTo_eq_value
            (soundnessGraph graph output) inputs index left leftBound
          have rightBase := NobleFoundation.PackedSchedule.valuesUpTo_eq_value
            (soundnessGraph graph output) inputs index right rightBound
          have leftReference := valuesUpTo_eq_value graph inputs index left leftBound
          have rightReference := valuesUpTo_eq_value graph inputs index right rightBound
          have stepEq :
              NobleFoundation.PackedSchedule.evalStep (soundnessGraph graph output)
                  (NobleFoundation.PackedSchedule.valuesUpTo (soundnessGraph graph output) inputs index)
                  inputs index =
                evalStep graph (valuesUpTo graph inputs index) inputs index := by
            unfold NobleFoundation.PackedSchedule.evalStep evalStep
            rw [mapped, node]
            simp only [soundnessNode]
            rw [leftBase, rightBase, leftReference, rightReference, leftEq, rightEq]
            rfl
          exact baseValue.trans (stepEq.trans referenceValue.symm)
      | guard operand =>
          have notHolds : (graph.node index).holdsRegister = false :=
            (holdsRegister_eq_false (graph.node index)).mpr (by simp [node, Node.isGuard])
          rw [holds] at notHolds
          cases notHolds

theorem soundnessOutputValues (graph : Graph) (output : Nat) (wellFormed : WellFormed graph)
    (inputs : Nat → BitVec 64) (regOf : Nat → Nat) (order outputs outRegs : List Nat)
    (bounded : ∀ index ∈ order, index < graph.count)
    (matched : outputsMatch graph regOf order outputs outRegs = true) :
    outputs.map (NobleFoundation.PackedSchedule.value (soundnessGraph graph output) inputs) =
      outputs.map (value graph inputs) := by
  induction outputs generalizing outRegs with
  | nil => rfl
  | cons node rest ih =>
      cases outRegs with
      | nil => simp [outputsMatch] at matched
      | cons outReg outRest =>
          have parts :
              ((decide (node ∈ order) = true ∧
                decide ((graph.node node).holdsRegister = true) = true) ∧
                decide (regOf node = outReg) = true) ∧
              outputsMatch graph regOf order rest outRest = true := by
            simpa only [outputsMatch, Bool.and_eq_true] using matched
          have member : node ∈ order := of_decide_eq_true parts.1.1.1
          have nodeBounded := bounded node member
          have holds : (graph.node node).holdsRegister = true := of_decide_eq_true parts.1.1.2
          have valueEq := soundnessValue_eq_value graph output wellFormed inputs node nodeBounded holds
          simp only [List.map_cons, valueEq]
          exact congrArg (fun values => value graph inputs node :: values)
            (ih outRest parts.2)

theorem soundnessEmitNode (graph : Graph) (wellFormed : WellFormed graph)
    (output : Nat) (regOf : Nat → Nat) (index : Nat) (bounded : index < graph.count) :
    soundnessInstr (emitNode graph regOf index) =
      NobleFoundation.PackedSchedule.emitNodeWith (soundnessGraph graph output)
        (soundnessRegOf graph regOf) index := by
  cases node : graph.node index with
  | input slot =>
      simp [emitNode, soundnessInstr, NobleFoundation.PackedSchedule.emitNodeWith,
        NobleFoundation.PackedSchedule.emitNode, soundnessGraph, bounded, soundnessNode,
        soundnessRegOf, Node.isGuard, node, soundnessReg]
  | constant bits =>
      simp [emitNode, soundnessInstr, NobleFoundation.PackedSchedule.emitNodeWith,
        NobleFoundation.PackedSchedule.emitNode, soundnessGraph, bounded, soundnessNode,
        soundnessRegOf, Node.isGuard, node, soundnessReg]
  | add left right =>
      have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
      have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
      have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
      have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
      simp [emitNode, soundnessInstr, NobleFoundation.PackedSchedule.emitNodeWith,
        NobleFoundation.PackedSchedule.emitNode, soundnessGraph, bounded, soundnessNode,
        soundnessRegOf_nonGuard graph regOf index (by simp [node, Node.holdsRegister, Node.isGuard]),
        soundnessRegOf_nonGuard graph regOf left leftHolds,
        soundnessRegOf_nonGuard graph regOf right rightHolds, node, soundnessReg]
  | sub left right =>
      have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
      have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
      have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
      have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
      simp [emitNode, soundnessInstr, NobleFoundation.PackedSchedule.emitNodeWith,
        NobleFoundation.PackedSchedule.emitNode, soundnessGraph, bounded, soundnessNode,
        soundnessRegOf_nonGuard graph regOf index (by simp [node, Node.holdsRegister, Node.isGuard]),
        soundnessRegOf_nonGuard graph regOf left leftHolds,
        soundnessRegOf_nonGuard graph regOf right rightHolds, node, soundnessReg]
  | mul left right =>
      have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
      have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
      have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
      have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
      simp [emitNode, soundnessInstr, NobleFoundation.PackedSchedule.emitNodeWith,
        NobleFoundation.PackedSchedule.emitNode, soundnessGraph, bounded, soundnessNode,
        soundnessRegOf_nonGuard graph regOf index (by simp [node, Node.holdsRegister, Node.isGuard]),
        soundnessRegOf_nonGuard graph regOf left leftHolds,
        soundnessRegOf_nonGuard graph regOf right rightHolds, node, soundnessReg]
  | eq left right =>
      have leftMember : left ∈ (graph.node index).operands := by simp [node, Node.operands]
      have rightMember : right ∈ (graph.node index).operands := by simp [node, Node.operands]
      have leftHolds := wellFormed_operand_holds graph wellFormed index left bounded leftMember
      have rightHolds := wellFormed_operand_holds graph wellFormed index right bounded rightMember
      simp [emitNode, soundnessInstr, NobleFoundation.PackedSchedule.emitNodeWith,
        NobleFoundation.PackedSchedule.emitNode, soundnessGraph, bounded, soundnessNode,
        soundnessRegOf_nonGuard graph regOf index (by simp [node, Node.holdsRegister, Node.isGuard]),
        soundnessRegOf_nonGuard graph regOf left leftHolds,
        soundnessRegOf_nonGuard graph regOf right rightHolds, node, soundnessReg]
  | guard operand =>
      simp [emitNode, soundnessInstr, NobleFoundation.PackedSchedule.emitNodeWith,
        NobleFoundation.PackedSchedule.emitNode, soundnessGraph, bounded, soundnessNode,
        soundnessRegOf, soundnessGuardReg, node, Node.isGuard]

theorem soundnessBodiesMatch (graph : Graph) (wellFormed : WellFormed graph)
    (output : Nat) (regOf : Nat → Nat) (order : List Nat) (instrs : List Instr)
    (bounded : ∀ index ∈ order, index < graph.count)
    (hMatches : bodiesMatch graph regOf order instrs = true) :
    NobleFoundation.PackedSchedule.bodiesMatch (soundnessGraph graph output)
      (soundnessRegOf graph regOf) order (instrs.map soundnessInstr) = true := by
  induction order generalizing instrs with
  | nil =>
      cases instrs <;> simp [bodiesMatch, NobleFoundation.PackedSchedule.bodiesMatch] at hMatches ⊢
  | cons index rest ih =>
      cases instrs with
      | nil => simp [bodiesMatch] at hMatches
      | cons instr instrs =>
          have parts : decide (instr = emitNode graph regOf index) = true ∧
              bodiesMatch graph regOf rest instrs = true := by
            simpa only [bodiesMatch, Bool.and_eq_true] using hMatches
          have body : instr = emitNode graph regOf index := of_decide_eq_true parts.1
          have mappedBody : soundnessInstr instr =
              NobleFoundation.PackedSchedule.emitNodeWith (soundnessGraph graph output)
                (soundnessRegOf graph regOf) index := by
            rw [body]
            exact soundnessEmitNode graph wellFormed output regOf index (bounded index (by simp))
          have restBounded : ∀ node ∈ rest, node < graph.count := by
            intro node member
            exact bounded node (by simp [member])
          simp [NobleFoundation.PackedSchedule.bodiesMatch, mappedBody,
          ih instrs restBounded parts.2]

theorem soundnessOperandsDefined (graph : Graph) (output : Nat) (processed order : List Nat)
    (bounded : ∀ index ∈ order, index < graph.count)
    (defined : operandsDefined graph processed order = true) :
    NobleFoundation.PackedSchedule.operandsDefined (soundnessGraph graph output) processed order =
      true := by
  induction order generalizing processed with
  | nil => rfl
  | cons index rest ih =>
      have parts :
          (graph.node index).operands.all (fun operand => decide (operand ∈ processed)) = true ∧
            operandsDefined graph (index :: processed) rest = true := by
        simpa only [operandsDefined, Bool.and_eq_true] using defined
      have indexBounded := bounded index (by simp)
      have mapped :
          (soundnessGraph graph output).node index = soundnessNode (graph.node index) := by
        simp [soundnessGraph, indexBounded]
      have headDefined :
          ((soundnessGraph graph output).node index).operands.all
            (fun operand => decide (operand ∈ processed)) = true := by
        rw [mapped, soundnessNode_operands]
        by_cases guard : (graph.node index).isGuard = true
        · simp [guard]
        · simpa [guard] using parts.1
      have restBounded : ∀ node ∈ rest, node < graph.count := by
        intro node member
        exact bounded node (by simp [member])
      simp only [NobleFoundation.PackedSchedule.operandsDefined]
      exact Bool.and_eq_true_iff.mpr ⟨headDefined, ih (index :: processed) restBounded parts.2⟩

theorem soundnessValidateBase (graph : Graph) (wellFormed : WellFormed graph)
    (regOf : Nat → Nat) (order : List Nat) (instrs : List Instr)
    (outputs outRegs : List Nat) (output : Nat)
    (noDuplicates : decide order.Nodup = true)
    (boundedBool : order.all (fun index => decide (index < graph.count)) = true)
    (bodyMatch : bodiesMatch graph regOf order instrs = true)
    (operandsDefinedBool : operandsDefined graph [] order = true)
    (registersInjectiveBool : registersInjectiveOn graph regOf order = true)
    (outputsMatchBool : outputsMatch graph regOf order outputs outRegs = true) :
    NobleFoundation.PackedSchedule.validate (soundnessGraph graph output)
      (soundnessRegOf graph regOf) order (instrs.map soundnessInstr) outputs
      (outputs.map (soundnessRegOf graph regOf)) = true := by
  have bounded : ∀ index ∈ order, index < graph.count := by
    intro index member
    exact of_decide_eq_true ((List.all_eq_true.mp boundedBool) index member)
  have baseBounded :
      order.all (fun index => decide (index < (soundnessGraph graph output).arity)) = true := by
    apply List.all_eq_true.mpr
    intro index member
    exact decide_eq_true_iff.mpr (by simpa [soundnessGraph] using bounded index member)
  have baseBodies :=
    soundnessBodiesMatch graph wellFormed output regOf order instrs bounded bodyMatch
  have baseOperands := soundnessOperandsDefined graph output [] order bounded operandsDefinedBool
  have baseRegisters := soundnessRegistersInjective graph regOf order registersInjectiveBool
  have baseOutputs := soundnessOutputsMatch graph regOf order outputs outRegs outputsMatchBool
  simp [NobleFoundation.PackedSchedule.validate, NobleFoundation.PackedSchedule.validateObligations,
    noDuplicates, baseBounded, baseBodies, baseOperands, baseRegisters, baseOutputs]

/-- Machine outputs of a validated guarded stream equal the guarded graph values, for every input.
The proof embeds real registers at even indices and ghost-writes guards at odd indices, then applies
the unguarded checked-schedule theorem. -/
theorem validate_machineOutputs_sound (graph : Graph) (wellFormed : WellFormed graph)
    (regOf : Nat → Nat) (order : List Nat) (instrs : List Instr)
    (outputs outRegs : List Nat)
    (accepted : validate graph regOf order instrs outputs outRegs = true)
    (inputs : Nat → BitVec 64) :
    machineOutputs inputs instrs outRegs = outputs.map (value graph inputs) := by
  have all := List.all_eq_true.mp accepted
  have noDuplicates : decide order.Nodup = true := by
    simpa using all (decide order.Nodup) (by simp [validateObligations])
  have boundedBool : order.all (fun index => decide (index < graph.count)) = true := by
    simpa using all (order.all (fun index => decide (index < graph.count)))
      (by simp [validateObligations])
  have completeBool : completeOrder graph order = true := by
    simpa using all (completeOrder graph order) (by simp [validateObligations])
  have bodyMatch : bodiesMatch graph regOf order instrs = true := by
    simpa using all (bodiesMatch graph regOf order instrs) (by simp [validateObligations])
  have operandsDefinedBool : operandsDefined graph [] order = true := by
    simpa using all (operandsDefined graph [] order) (by simp [validateObligations])
  have registersInjectiveBool : registersInjectiveOn graph regOf order = true := by
    simpa using all (registersInjectiveOn graph regOf order) (by simp [validateObligations])
  have outputsMatchBool : outputsMatch graph regOf order outputs outRegs = true := by
    simpa using all (outputsMatch graph regOf order outputs outRegs) (by simp [validateObligations])
  induction outputs generalizing outRegs with
  | nil =>
      cases outRegs with
      | nil => rfl
      | cons outReg outRegs => simp [outputsMatch] at outputsMatchBool
  | cons output rest =>
      cases outRegs with
      | nil => simp [outputsMatch] at outputsMatchBool
      | cons outReg outRest =>
          have parts :
              ((decide (output ∈ order) = true ∧
                decide ((graph.node output).holdsRegister = true) = true) ∧
                decide (regOf output = outReg) = true) ∧
              outputsMatch graph regOf order rest outRest = true := by
            simpa only [outputsMatch, Bool.and_eq_true] using outputsMatchBool
          have member : output ∈ order := of_decide_eq_true parts.1.1.1
          have bounded : ∀ index ∈ order, index < graph.count := by
            intro index member
            exact of_decide_eq_true ((List.all_eq_true.mp boundedBool) index member)
          have outputBounded := bounded output member
          have baseWellFormed := soundnessGraph_wellFormed graph output outputBounded wellFormed
          have baseValid := soundnessValidateBase graph wellFormed regOf order instrs
            (output :: rest) (outReg :: outRest) output noDuplicates boundedBool
            bodyMatch operandsDefinedBool registersInjectiveBool outputsMatchBool
          have baseSound := NobleFoundation.PackedSchedule.validate_sound
            (soundnessGraph graph output) baseWellFormed (soundnessRegOf graph regOf) order
            (instrs.map soundnessInstr) (output :: rest)
            ((output :: rest).map (soundnessRegOf graph regOf)) baseValid inputs
          have outputRegs := soundnessOutputRegisters graph regOf order (output :: rest)
            (outReg :: outRest) outputsMatchBool
          have runTransfer :
              NobleFoundation.PackedSchedule.runOutputs inputs (instrs.map soundnessInstr)
                  ((output :: rest).map (soundnessRegOf graph regOf)) =
                machineOutputs inputs instrs (outReg :: outRest) := by
            rw [← outputRegs]
            simp [NobleFoundation.PackedSchedule.runOutputs, machineOutputs,
              soundnessRun_eq_machineRun]
          have valueTransfer := soundnessOutputValues graph output wellFormed inputs regOf order
            (output :: rest) (outReg :: outRest) bounded outputsMatchBool
          calc
            machineOutputs inputs instrs (outReg :: outRest) =
                NobleFoundation.PackedSchedule.runOutputs inputs (instrs.map soundnessInstr)
                  ((output :: rest).map (soundnessRegOf graph regOf)) := runTransfer.symm
            _ = NobleFoundation.PackedSchedule.referenceOutputs (soundnessGraph graph output)
                  inputs (output :: rest) := baseSound
            _ = (output :: rest).map (value graph inputs) := valueTransfer

/-- The accepted schedule computes any in-order, register-holding node in its declared register,
not only the declared outputs. This is used to interpret guard reads from the packed register file. -/
theorem validate_machineValue_sound (graph : Graph) (wellFormed : WellFormed graph)
    (regOf : Nat → Nat) (order : List Nat) (instrs : List Instr)
    (outputs outRegs : List Nat)
    (accepted : validate graph regOf order instrs outputs outRegs = true)
    (inputs : Nat → BitVec 64) (index : Nat) (member : index ∈ order)
    (holds : (graph.node index).holdsRegister = true) :
    machineRun inputs instrs (regOf index) = value graph inputs index := by
  have all := List.all_eq_true.mp accepted
  have noDuplicates : decide order.Nodup = true := by
    simpa using all (decide order.Nodup) (by simp [validateObligations])
  have boundedBool : order.all (fun node => decide (node < graph.count)) = true := by
    simpa using all (order.all (fun node => decide (node < graph.count)))
      (by simp [validateObligations])
  have completeBool : completeOrder graph order = true := by
    simpa using all (completeOrder graph order) (by simp [validateObligations])
  have bodyMatch : bodiesMatch graph regOf order instrs = true := by
    simpa using all (bodiesMatch graph regOf order instrs) (by simp [validateObligations])
  have operandsDefinedBool : operandsDefined graph [] order = true := by
    simpa using all (operandsDefined graph [] order) (by simp [validateObligations])
  have registersInjectiveBool : registersInjectiveOn graph regOf order = true := by
    simpa using all (registersInjectiveOn graph regOf order) (by simp [validateObligations])
  have oneOutputMatches : outputsMatch graph regOf order [index] [regOf index] = true := by
    simp [outputsMatch, member, holds]
  have oneOutputAccepted : validate graph regOf order instrs [index] [regOf index] = true := by
    simp [validate, validateObligations, noDuplicates, boundedBool, completeBool, bodyMatch,
      operandsDefinedBool, registersInjectiveBool, oneOutputMatches]
  have oneOutputSound := validate_machineOutputs_sound graph wellFormed regOf order instrs
    [index] [regOf index] oneOutputAccepted inputs
  simpa [machineOutputs] using oneOutputSound

/-- A packed guard checks its operand register against the same bit predicate as the graph
reference, provided the schedule is accepted. -/
theorem guardFailsInRegisters_eq_reference (graph : Graph) (wellFormed : WellFormed graph)
    (regOf : Nat → Nat) (order : List Nat) (instrs : List Instr)
    (outputs outRegs : List Nat)
    (accepted : validate graph regOf order instrs outputs outRegs = true)
    (inputs : Nat → BitVec 64) (index : Nat) (member : index ∈ order) :
    guardFailsInRegisters graph regOf (machineRun inputs instrs) index =
      guardFails graph inputs index := by
  cases node : graph.node index with
  | input slot => simp [guardFailsInRegisters, guardFails, Node.guarded, node]
  | constant bits => simp [guardFailsInRegisters, guardFails, Node.guarded, node]
  | add left right => simp [guardFailsInRegisters, guardFails, Node.guarded, node]
  | sub left right => simp [guardFailsInRegisters, guardFails, Node.guarded, node]
  | mul left right => simp [guardFailsInRegisters, guardFails, Node.guarded, node]
  | eq left right => simp [guardFailsInRegisters, guardFails, Node.guarded, node]
  | guard operand =>
      have all := List.all_eq_true.mp accepted
      have boundedBool : order.all (fun node => decide (node < graph.count)) = true := by
        simpa using all (order.all (fun node => decide (node < graph.count)))
          (by simp [validateObligations])
      have completeBool : completeOrder graph order = true := by
        simpa using all (completeOrder graph order) (by simp [validateObligations])
      have indexBounded : index < graph.count :=
        of_decide_eq_true ((List.all_eq_true.mp boundedBool) index member)
      have operandMember : operand ∈ (graph.node index).operands := by
        simp [node, Node.operands]
      have operandBounded := wellFormed_operand_lt graph wellFormed index operand indexBounded
        operandMember
      have operandWithin := Nat.lt_trans operandBounded indexBounded
      have operandHolds := wellFormed_operand_holds graph wellFormed index operand indexBounded
        operandMember
      have operandInOrder : operand ∈ order :=
        of_decide_eq_true ((List.all_eq_true.mp completeBool) operand
          (List.mem_range.mpr operandWithin))
      have operandValue := validate_machineValue_sound graph wellFormed regOf order instrs
        outputs outRegs accepted inputs operand operandInOrder operandHolds
      have passEq : guardPasses graph inputs operand =
          guardHolds (machineRun inputs instrs) (regOf operand) := by
        simp [guardPasses, guardHolds, operandValue]
      rw [guardFails_guard graph inputs index operand node]
      simp only [guardFailsInRegisters, node]
      rw [← passEq]

/-- Failures of guard nodes in a supplied order, read from the stream's packed register file. -/
def scheduledFailures (graph : Graph) (regOf : Nat → Nat) (regs : Nat → Option Val)
    (order : List Nat) : List Nat :=
  order.filter (guardFailsInRegisters graph regOf regs)

/-- The supplied stream's observation: guard instructions read operand registers from the executed
stream, and the least failing canonical guard is reported; otherwise its output registers are read. -/
def machineObs (graph : Graph) (inputs : Nat → BitVec 64) (regOf : Nat → Nat)
    (order : List Nat) (instrs : List Instr) (outRegs : List Nat) : Obs :=
  match minimum (scheduledFailures graph regOf (machineRun inputs instrs) order) with
  | some node => .failed node
  | none => .normal (machineOutputs inputs instrs outRegs)

theorem minimum_eq_of_mem_iff {firstList secondList : List Nat}
    (sameMembers : ∀ value, value ∈ firstList ↔ value ∈ secondList) :
    minimum firstList = minimum secondList := by
  cases hleft : minimum firstList with
  | none =>
      have leftEmpty := minimum_none_iff.mp hleft
      cases secondList with
      | nil => rfl
      | cons head rest =>
          have headMember : head ∈ head :: rest := by simp
          have : head ∈ firstList := (sameMembers head).mpr headMember
          simp [leftEmpty] at this
  | some least =>
      have memberLeft := minimum_mem hleft
      have memberRight : least ∈ secondList := (sameMembers least).mp memberLeft
      have hright : minimum secondList = some least := by
        cases hmin : minimum secondList with
        | none =>
            have rightEmpty := minimum_none_iff.mp hmin
            simp [rightEmpty] at memberRight
        | some other =>
            have memberOther := minimum_mem hmin
            have otherLe := minimum_le memberRight hmin
            have leastLe := minimum_le ((sameMembers other).mpr memberOther) hleft
            have equal : least = other := Nat.le_antisymm leastLe otherLe
            rw [equal]
      exact hright.symm

theorem failingGraph_mem_iff (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat) :
    index ∈ failingGraph graph inputs ↔ index < graph.count ∧ guardFails graph inputs index = true := by
  constructor
  · intro member
    exact ⟨failingGraph_lt graph inputs index member, failingGraph_fails graph inputs index member⟩
  · rintro ⟨bounded, fails⟩
    exact failingGraph_mem graph inputs index bounded fails

theorem scheduledFailures_mem_iff (graph : Graph) (inputs : Nat → BitVec 64)
    (regOf : Nat → Nat) (regs : Nat → Option Val) (order : List Nat)
    (bounded : ∀ index ∈ order, index < graph.count)
    (complete : ∀ index, index < graph.count → index ∈ order)
    (sameFailures : ∀ index ∈ order,
      guardFailsInRegisters graph regOf regs index = guardFails graph inputs index) (index : Nat) :
    index ∈ scheduledFailures graph regOf regs order ↔ index ∈ failingGraph graph inputs := by
  constructor
  · intro member
    have pair := List.mem_filter.mp member
    exact failingGraph_mem graph inputs index (bounded index pair.1)
      ((sameFailures index pair.1).symm.trans pair.2)
  · intro member
    have ⟨within, fails⟩ := (failingGraph_mem_iff graph inputs index).mp member
    have ordered := complete index within
    have machineFails : guardFailsInRegisters graph regOf regs index = true := by
      rw [sameFailures index ordered]
      exact fails
    exact List.mem_filter.mpr ⟨ordered, machineFails⟩

/-- An accepted stream's abstract guarded observation equals canonical reference evaluation. -/
theorem validate_sound (graph : Graph) (wellFormed : WellFormed graph)
    (regOf : Nat → Nat) (order : List Nat)
    (instrs : List Instr) (outputs outRegs : List Nat)
    (accepted : validate graph regOf order instrs outputs outRegs = true)
    (inputs : Nat → BitVec 64) :
    machineObs graph inputs regOf order instrs outRegs = referenceObs graph inputs outputs := by
  have all := List.all_eq_true.mp accepted
  have boundedBool : order.all (fun index => decide (index < graph.count)) = true := by
    simpa using all (order.all (fun index => decide (index < graph.count)))
      (by simp [validateObligations])
  have completeBool : completeOrder graph order = true := by
    simpa using all (completeOrder graph order) (by simp [validateObligations])
  have bounded : ∀ index ∈ order, index < graph.count := by
    intro index member
    exact of_decide_eq_true ((List.all_eq_true.mp boundedBool) index member)
  have complete : ∀ index, index < graph.count → index ∈ order := by
    intro index within
    exact of_decide_eq_true ((List.all_eq_true.mp completeBool) index (List.mem_range.mpr within))
  have sameFailures : ∀ index ∈ order,
      guardFailsInRegisters graph regOf (machineRun inputs instrs) index =
        guardFails graph inputs index := by
    intro index member
    exact guardFailsInRegisters_eq_reference graph wellFormed regOf order instrs outputs outRegs
      accepted inputs index member
  have sameMembers := scheduledFailures_mem_iff graph inputs regOf (machineRun inputs instrs)
    order bounded complete sameFailures
  have sameMinimum : minimum (scheduledFailures graph regOf (machineRun inputs instrs) order) =
      minimum (failingGraph graph inputs) := minimum_eq_of_mem_iff sameMembers
  have outputSound :=
    validate_machineOutputs_sound graph wellFormed regOf order instrs outputs outRegs accepted inputs
  unfold machineObs referenceObs
  rw [sameMinimum, ← firstFailure_eq_minimum]
  cases firstFailure graph inputs <;> simp [outputSound]

/-! ### Layered failure loop and `fail_suffix_min` -/

/-- One packed layer's possible guard identities and the subset that fail for these inputs. -/
structure GuardLayer where
  guards : List Nat
  failures : List Nat
  failures_are_guards : ∀ node ∈ failures, node ∈ guards

/-- Whether the least failure already observed precedes every possible failure in the remaining
layers. `none` in the suffix table is the model counterpart of the `u32::MAX` sentinel. -/
def earlierThan (observed suffix : Option Nat) : Bool :=
  match observed, suffix with
  | some _, none => true
  | some first, some later => decide (first < later)
  | none, _ => false

/-- The strict suffix minima consumed after each layer, with one trailing `none` entry. Rust stores
the inclusive suffix minimum at each table index and tests `fail_suffix_min[layer + 1]` after that
layer; consequently its exported table's tail, truncated to the number of layers, is this list. -/
def failSuffixMin : List GuardLayer → List (Option Nat)
  | [] => [none]
  | _ :: rest => minimum (rest.flatMap GuardLayer.guards) :: failSuffixMin rest

/-- The packed loop after each layer: retain the least failure seen and stop only when it is below
the exact lower bound for every later guard. -/
def runLayersWithTable : List (Option Nat) → List GuardLayer → Option Nat → Option Nat
  | _, [], observed => observed
  | suffix :: suffixes, layer :: rest, observed =>
      let next := minNat observed (minimum layer.failures)
      if earlierThan next suffix = true then next else runLayersWithTable suffixes rest next
  | [], _ :: _, observed => observed

/-- The loop using the graph-derived suffix minima, rather than a caller-supplied table. -/
def runLayersWith (layers : List GuardLayer) : Option Nat :=
  runLayersWithTable (failSuffixMin layers) layers none

theorem minNat_of_earlier (guards failures : List Nat)
    (subset : ∀ node ∈ failures, node ∈ guards) (observed : Option Nat)
    (earlier : earlierThan observed (minimum guards) = true) :
    minNat observed (minimum failures) = observed := by
  cases observed with
  | none => simp [earlierThan] at earlier
  | some first =>
      cases hguards : minimum guards with
      | none =>
          have guardsEmpty := minimum_none_iff.mp hguards
          have failuresEmpty : failures = [] := by
            cases failures with
            | nil => rfl
            | cons node rest =>
                have member : node ∈ node :: rest := by simp
                have inGuards := subset node member
                simp [guardsEmpty] at inGuards
          simp [minimum_none_iff.mpr failuresEmpty, minNat]
      | some leastGuard =>
          have firstLess : first < leastGuard := by
            simpa [earlierThan, hguards] using earlier
          have greater : ∀ node ∈ failures, first < node := by
            intro node member
            have inGuards := subset node member
            have leastLe := minimum_le inGuards hguards
            omega
          exact minNat_some_of_gt greater

theorem runLayersWithTable_correct (layers : List GuardLayer) (observed : Option Nat) :
    runLayersWithTable (failSuffixMin layers) layers observed =
      minNat observed (minimum (layers.flatMap GuardLayer.failures)) := by
  induction layers generalizing observed with
  | nil =>
      change observed = minNat observed none
      exact (minNat_none_right observed).symm
  | cons layer rest ih =>
      simp only [failSuffixMin, runLayersWithTable]
      let next := minNat observed (minimum layer.failures)
      by_cases stop : earlierThan next (minimum (rest.flatMap GuardLayer.guards)) = true
      · simp [stop, next]
        have noLaterChanges := minNat_of_earlier (rest.flatMap GuardLayer.guards)
          (rest.flatMap GuardLayer.failures)
          (by
            intro node member
            rcases List.mem_flatMap.mp member with ⟨entry, entryMem, nodeMem⟩
            exact List.mem_flatMap.mpr ⟨entry, entryMem, entry.failures_are_guards node nodeMem⟩)
          next stop
        rw [minimum_append, ← minNat_assoc, noLaterChanges]
      · simp [stop, next]
        rw [ih (minNat observed (minimum layer.failures))]
        rw [minimum_append, ← minNat_assoc]

/-- A deliberately smoothed suffix table can stop at the later guard and therefore report the
wrong canonical first failure. The exact table keeps the earlier possible guard visible. -/
def laterGuardLayer : GuardLayer where
  guards := [3]
  failures := [3]
  failures_are_guards := by simp

def earlierPackedGuardLayer : GuardLayer where
  guards := [7]
  failures := [7]
  failures_are_guards := by simp

theorem example_smoothed_suffix_reports_the_later_guard :
    runLayersWithTable [none, none, none] [earlierPackedGuardLayer, laterGuardLayer] none = some 7 ∧
      runLayersWith [earlierPackedGuardLayer, laterGuardLayer] = some 3 := by
  decide

/-! ### Layer-to-reference composition -/

/-- Build one layer's guard and failure sets from the packed machine's register file. -/
def graphGuardLayer (graph : Graph) (regOf : Nat → Nat) (regs : Nat → Option Val)
    (nodes : List Nat) : GuardLayer :=
  { guards := nodes.filter (fun index => (graph.node index).isGuard),
    failures := nodes.filter (guardFailsInRegisters graph regOf regs),
    failures_are_guards := by
      intro index member
      have pair := List.mem_filter.mp member
      exact List.mem_filter.mpr
        ⟨pair.1, guardFailsInRegisters_isGuard graph regOf regs index pair.2⟩ }

theorem filter_append_bool (predicate : Nat → Bool) (first second : List Nat) :
    (first ++ second).filter predicate = first.filter predicate ++ second.filter predicate := by
  induction first with
  | nil => rfl
  | cons head rest ih =>
      simp only [List.cons_append, List.filter_cons, ih]
      by_cases holds : predicate head <;> simp [holds]

/-- The observation from packed layers after the `fail_suffix_min` early-exit rule. Guard checks
read the values in the stream's packed register file, not the reference graph's values. -/
def layeredObs (graph : Graph) (inputs : Nat → BitVec 64) (layers : List (List Nat))
    (regOf : Nat → Nat) (instrs : List Instr) (outRegs : List Nat) : Obs :=
  match runLayersWith
      (layers.map (graphGuardLayer graph regOf (machineRun inputs instrs))) with
  | some node => .failed node
  | none => .normal (machineOutputs inputs instrs outRegs)

theorem flatMap_filter_eq_filter_flatten (layers : List (List Nat))
    (predicate : Nat → Bool) :
    layers.flatMap (fun layer => layer.filter predicate) =
      layers.flatten.filter predicate := by
  induction layers with
  | nil => rfl
  | cons layer rest ih =>
      simp [List.flatten, List.flatMap, List.filter_append]

theorem graphGuardLayers_failures (graph : Graph) (regOf : Nat → Nat)
    (regs : Nat → Option Val) (layers : List (List Nat)) :
    (layers.map (graphGuardLayer graph regOf regs)).flatMap GuardLayer.failures =
      scheduledFailures graph regOf regs layers.flatten := by
  induction layers with
  | nil => rfl
  | cons layer rest ih =>
      simp only [List.map_cons, List.flatMap_cons, graphGuardLayer, List.flatten,
        scheduledFailures]
      have ih' :
          List.flatMap GuardLayer.failures (List.map (graphGuardLayer graph regOf regs) rest) =
            rest.flatten.filter (guardFailsInRegisters graph regOf regs) := by
        simpa [scheduledFailures] using ih
      rw [ih']
      exact (filter_append_bool (guardFailsInRegisters graph regOf regs) layer rest.flatten).symm

theorem layeredObs_sound (graph : Graph) (wellFormed : WellFormed graph)
    (inputs : Nat → BitVec 64) (layers : List (List Nat))
    (outputs : List Nat) (regOf : Nat → Nat) (instrs : List Instr) (outRegs : List Nat)
    (accepted : validate graph regOf layers.flatten instrs outputs outRegs = true) :
    layeredObs graph inputs layers regOf instrs outRegs =
      referenceObs graph inputs outputs := by
  have fullMinimum :=
    runLayersWithTable_correct
      (layers.map (graphGuardLayer graph regOf (machineRun inputs instrs))) none
  have failureLists := graphGuardLayers_failures graph regOf (machineRun inputs instrs) layers
  have machineEquality :
      runLayersWith (layers.map (graphGuardLayer graph regOf (machineRun inputs instrs))) =
        minimum (scheduledFailures graph regOf (machineRun inputs instrs) layers.flatten) := by
    rw [runLayersWith, fullMinimum, failureLists]
    rfl
  have validSound := validate_sound graph wellFormed regOf layers.flatten instrs outputs outRegs
    accepted inputs
  have outputSound :=
    validate_machineOutputs_sound graph wellFormed regOf layers.flatten instrs outputs outRegs
      accepted inputs
  simpa [layeredObs, machineObs, machineEquality, outputSound] using validSound

end NobleFoundation.PackedScheduleGuard
