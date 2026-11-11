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

/-- A set of wires is reciprocal when every wire's reverse is present. -/
def reciprocal (wires : List Wire) : Bool :=
  wires.all fun w => wires.any fun v => decide (v = (w.2, w.1))

/-- Reciprocity gives the reverse wire. -/
theorem reciprocal_reverse {wires : List Wire} {w : Wire} (h : reciprocal wires = true)
    (hmem : w ∈ wires) : (w.2, w.1) ∈ wires := by
  have hall := List.all_eq_true.mp h w hmem
  obtain ⟨v, hv, hve⟩ := List.any_eq_true.mp hall
  rw [decide_eq_true_eq] at hve
  rw [← hve]
  exact hv

/-- Every wired port is touched exactly twice: once by its own wire, once by the reciprocal. -/
def uniqueIncidence (wires : List Wire) : Bool :=
  wires.all fun w => (incidence wires w.1 == 1) && (incidence wires w.2 == 1)

/-- Unique incidence pins the count at one per wire end. -/
theorem uniqueIncidence_left {wires : List Wire} {w : Wire}
    (h : uniqueIncidence wires = true) (hmem : w ∈ wires) : incidence wires w.1 = 1 := by
  have hall := List.all_eq_true.mp h w hmem
  rw [Bool.and_eq_true] at hall
  simpa using hall.1

theorem uniqueIncidence_right {wires : List Wire} {w : Wire}
    (h : uniqueIncidence wires = true) (hmem : w ∈ wires) : incidence wires w.2 = 1 := by
  have hall := List.all_eq_true.mp h w hmem
  rw [Bool.and_eq_true] at hall
  simpa using hall.2

/-- Under unique incidence both ends of a wire are touched exactly once, so a wire accounts for two
port ends. Reciprocity is not needed for this count; it is needed to know the `1` at each end is the
reverse wire rather than a second wire to the same port. -/
theorem unique_incidence_two_ends {wires : List Wire} {w : Wire}
    (huniq : uniqueIncidence wires = true) (hmem : w ∈ wires) :
    incidence wires w.1 + incidence wires w.2 = 2 := by
  rw [uniqueIncidence_left huniq hmem, uniqueIncidence_right huniq hmem]

/-- A revision-2 net: agent kinds, wires, and ordered boundary positions by agent index. -/
structure Net where
  /-- Agent kinds, indexed by position. -/
  agents : List AgentKind
  /-- Wires between ports. -/
  wires : List Wire
  /-- Boundary inputs, in declared order. -/
  inputs : List Nat
  /-- Boundary outputs, in declared order. -/
  outputs : List Nat

/-- The kind at an agent index, if any. -/
def Net.kindAt (net : Net) (i : Nat) : Option AgentKind := net.agents[i]?

/-- Strictly increasing, so boundary positions can be matched to a declared order. -/
def strictlyIncreasing : List Nat → Bool
  | [] => true
  | [_] => true
  | a :: b :: rest => (a < b) && strictlyIncreasing (b :: rest)

/-- Every input is an input agent, every output an output agent, and both lists ascend. -/
def boundaryOk (net : Net) : Bool :=
  (net.inputs.all fun i => net.kindAt i == some .input)
    && (net.outputs.all fun i => net.kindAt i == some .output)
    && strictlyIncreasing net.inputs
    && strictlyIncreasing net.outputs

/-- A scan of mapped values decides each entry against one target. -/
theorem all_beq_map {α β : Type} [BEq β] [LawfulBEq β] {l : List α} {f : α → β} {x : β}
    (h : l.all (fun y => f y == x) = true) : ∀ y ∈ l, f y = x := by
  intro y hy
  have := List.all_eq_true.mp h y hy
  simpa using this

theorem boundaryOk_parts {net : Net} (h : boundaryOk net = true) :
    net.inputs.all (fun i => net.kindAt i == some .input) = true
      ∧ net.outputs.all (fun i => net.kindAt i == some .output) = true
      ∧ strictlyIncreasing net.inputs = true
      ∧ strictlyIncreasing net.outputs = true := by
  unfold boundaryOk at h
  rw [Bool.and_eq_true, Bool.and_eq_true, Bool.and_eq_true] at h
  exact ⟨h.1.1.1, h.1.1.2, h.1.2, h.2⟩

theorem boundary_input_kind {net : Net} (h : boundaryOk net = true) {i : Nat}
    (hi : i ∈ net.inputs) : net.kindAt i = some .input := by
  have hin := (boundaryOk_parts h).1
  exact all_beq_map hin i hi

theorem boundary_output_kind {net : Net} (h : boundaryOk net = true) {i : Nat}
    (hi : i ∈ net.outputs) : net.kindAt i = some .output := by
  have hout := (boundaryOk_parts h).2.1
  exact all_beq_map hout i hi

/-- Ascending boundary positions satisfy the adjacent order. -/
theorem strictlyIncreasing_head {a b : Nat} {rest : List Nat}
    (h : strictlyIncreasing (a :: b :: rest) = true) : a < b := by
  rw [strictlyIncreasing] at h
  rw [Bool.and_eq_true] at h
  simpa using h.1

/-- A boundary position list with two adjacent entries declares the first before the second. -/
theorem boundary_inputs_ordered {net : Net} (h : boundaryOk net = true) :
    strictlyIncreasing net.inputs = true :=
  (boundaryOk_parts h).2.2.1

/-- Kinds whose principal port carries a value at admission. -/
def AgentKind.isValueSource : AgentKind → Bool
  | .value => true
  | .input => true
  | _ => false

/-- The wires whose principal end is a value source: what a reducer's worklist starts from. -/
def activePairs (net : Net) : List Wire :=
  net.wires.filter fun w =>
    match net.kindAt w.1.1 with
    | some k => k.isValueSource && w.1.2.isNone
    | none => false

/-- Every wire in the inventory really is a value arriving at a principal port. -/
theorem activePairs_mem {net : Net} {w : Wire} (hmem : w ∈ activePairs net) :
    ∃ k, net.kindAt w.1.1 = some k ∧ k.isValueSource = true ∧ w.1.2 = none := by
  unfold activePairs at hmem
  rw [List.mem_filter] at hmem
  obtain ⟨hw, hpred⟩ := hmem
  cases hk : net.kindAt w.1.1 with
  | none => rw [hk] at hpred; simp at hpred
  | some k =>
      rw [hk] at hpred
      rw [Bool.and_eq_true] at hpred
      exact ⟨k, rfl, hpred.1, by simpa using hpred.2⟩

/-- A filtered list is no longer than its source. -/
theorem filter_length_le {α : Type} (p : α → Bool) :
    ∀ (l : List α), (l.filter p).length ≤ l.length
  | [] => by simp
  | a :: rest => by
      have ih := filter_length_le p rest
      cases hp : p a with
      | true =>
          simp [hp]
          omega
      | false =>
          simp [hp]
          omega

/-- The worklist never exceeds the wire count. -/
theorem activePairs_length_le (net : Net) : (activePairs net).length ≤ net.wires.length :=
  filter_length_le _ net.wires

/-- Size limits for admission, charged before growth. -/
def withinLimits (net : Net) (maxAgents maxWires : Nat) : Bool :=
  (decide (net.agents.length ≤ maxAgents)) && decide (net.wires.length ≤ maxWires)

/-- Validation yields the bounds it claims. -/
theorem withinLimits_bounds {net : Net} {maxAgents maxWires : Nat}
    (h : withinLimits net maxAgents maxWires = true) :
    net.agents.length ≤ maxAgents ∧ net.wires.length ≤ maxWires := by
  unfold withinLimits at h
  rw [Bool.and_eq_true] at h
  exact ⟨of_decide_eq_true h.1, of_decide_eq_true h.2⟩

end NobleM2.InteractionNet
