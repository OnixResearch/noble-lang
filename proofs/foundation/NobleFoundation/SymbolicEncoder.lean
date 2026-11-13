/-
The `noble-symbolic-qfbv/v1` encoder and its simulation theorem (issue #56, plan step 4).

`verification/symbolic/solver.mjs` encodes a graph pair as one shared-DAG QF_BV query: every node
becomes one `define-fun` whose body is the node's operator applied to the earlier nodes' symbols,
and the assertion is "some listed output differs". This file is the model of that encoder and the
theorem that its body semantics *is* the graph's denotation.

* `bodyValue` is the semantics of one emitted body, node by node, over the model's `Val` fragment:
  `BitVec 64` arithmetic that wraps at `2^64`, and `Bool` equality.
* `value_eq_bodyValue` is the fixed point the emitted `define-fun` bodies rely on: a node's value
  is its body evaluated with the values of the earlier nodes.
* `bodyValue_simulates` states that fixed point on a node list, and `claim_simulates` states the
  assertion of the query: "some listed output differs", evaluated over the emitted bodies, is
  exactly the model's `refuted` predicate — for every 64-bit input, not for a finite universe.

The text itself is bound separately: the generated `SymbolicEncoderCorpus` decides, for every
query of the retained receipt, that `renderQuery` produces the exact bytes the solvers read.

What is trusted, and what is not claimed. SMT-LIB's standard semantics of the rendered text — that
`bvadd`, `bvsub` and `bvmul` on `(_ BitVec 64)` and `=` on `Bool` denote what `bodyValue` says, and
that the emitted `define-fun` and `assert` lines denote those terms — is a reading of the standard,
not a mechanized parser. What is checked is the term-level simulation on every input plus byte
equality of the text. Nothing here is a claim about `extract/symbolic.rs`, about either solver, or
about emitted Wasm.
-/
import NobleFoundation.SymbolicPair
import NobleFoundation.FirstProjection

namespace NobleFoundation.SymbolicEncoder

open NobleFoundation.PackedSchedule
open NobleFoundation.FirstProjection
open NobleFoundation.SymbolicPair

/-! ### The emitted query text -/

/-- One lowercase hexadecimal digit. -/
def hexDigit (value : Nat) : Char :=
  "0123456789abcdef".toList.getD (value % 16) '0'

/-- A 64-bit constant as `encode` writes it: `#x` and sixteen lowercase hexadecimal digits. -/
def hex64 (bits : BitVec 64) : String :=
  "#x" ++ String.ofList (List.ofFn fun (index : Fin 16) =>
    hexDigit (bits.toNat / 16 ^ (15 - index.val) % 16))

/-- The sort of one node in the emitted query. -/
def sortOf : Node → String
  | .eq _ _ => "Bool"
  | _ => "(_ BitVec 64)"

/-- One `define-fun` body, byte for byte as `encode` writes it. -/
def bodyOf (side : String) : Node → String
  | .input slot => s!"s{slot}"
  | .constant bits => hex64 bits
  | .add a b => s!"(bvadd {side}{a} {side}{b})"
  | .sub a b => s!"(bvsub {side}{a} {side}{b})"
  | .mul a b => s!"(bvmul {side}{a} {side}{b})"
  | .eq a b => s!"(= {side}{a} {side}{b})"

/-- One `define-fun` line, byte for byte as `encode` writes it. -/
def defineFunOf (side : String) (index : Nat) (node : Node) : String :=
  s!"(define-fun {side}{index} () {sortOf node} {bodyOf side node})"

/-- A seeded encoder fault: the `sub` body with its operands swapped. -/
def bodyOfSubSwapped (side : String) : Node → String
  | .sub a b => s!"(bvsub {side}{b} {side}{a})"
  | node => bodyOf side node

/-- The seeded `sub`-operand-swap fault changes the emitted body. -/
theorem bodyOf_sub_swap_differs :
    bodyOfSubSwapped "L" (.sub 0 1) ≠ bodyOf "L" (.sub 0 1) := by
  decide

/-- The node indices the listed outputs read. The fuel bounds the worklist: every step consumes
one pending index and pushes at most two operand indices, so `3 * nodes.length + 3` steps cover
any admitted graph; a truncated cone would make `renderQuery` differ from the lane's bytes, which
the generated corpus decides per pair. -/
def scan (nodes : List Node) : Nat → List Nat → List Nat → List Nat
  | 0, _, seen => seen
  | _, [], seen => seen
  | fuel + 1, index :: pending, seen =>
      if index ∈ seen then scan nodes fuel pending seen
      else scan nodes fuel (pending ++ (nodes.getD index (.constant 0)).operands) (index :: seen)

/-- The cone of the listed outputs, in index order. -/
def coneOf (nodes : List Node) (outputs : List Nat) : List Nat :=
  (List.range nodes.length).filter fun index =>
    index ∈ scan nodes (3 * nodes.length + 3) outputs []

/-- The `(index, node)` pairs a query defines, in index order: the whole graph, or its cone. -/
def definesOf (nodes : List Node) (slice : Bool) (outputs : List Nat) : List (Nat × Node) :=
  let keep := if slice then coneOf nodes outputs else List.range nodes.length
  (List.range nodes.length).filterMap fun index =>
    if index ∈ keep then some (index, nodes.getD index (.constant 0)) else none

/-- Pair of graphs the lane encodes: two node lists and the ordered outputs it compares. -/
structure Subject where
  /-- The left graph's nodes, in index order. -/
  left : List Node
  /-- The right graph's nodes, in index order. -/
  right : List Node
  /-- The ordered outputs the claim compares, not necessarily one per graph. -/
  outputs : List Nat
deriving DecidableEq

/-- The input slots the defined nodes read, in definition order. -/
def slotList (subject : Subject) (slice : Bool) : List Nat :=
  (((definesOf subject.left slice subject.outputs).map (·.2)) ++
    ((definesOf subject.right slice subject.outputs).map (·.2))).filterMap fun node =>
    match node with
    | .input slot => some slot
    | _ => none

/-- Insert one slot into an ascending list. -/
def insertSlot (slot : Nat) : List Nat → List Nat
  | [] => [slot]
  | head :: rest => if slot < head then slot :: head :: rest else head :: insertSlot slot rest

/-- Ascending order, as `encode` sorts the declared symbols. -/
def sortSlots : List Nat → List Nat
  | [] => []
  | head :: rest => insertSlot head (sortSlots rest)

/-- Adjacent duplicates removed, on an ascending list. -/
def dedupeSlots : List Nat → List Nat
  | [] => []
  | [slot] => [slot]
  | head :: next :: rest =>
      if head = next then dedupeSlots (next :: rest) else head :: dedupeSlots (next :: rest)

/-- The declared symbol names, ascending. -/
def slotNames (subject : Subject) (slice : Bool) : List String :=
  (dedupeSlots (sortSlots (slotList subject slice))).map fun slot => s!"s{slot}"

/-- One compared output of the claim. A pair whose two outputs have different sorts always
differs, which is what `encode` writes as `true`. -/
def differsAt (subject : Subject) (out : Nat) : String :=
  if sortOf (subject.left.getD out (.constant 0)) = sortOf (subject.right.getD out (.constant 0))
    then s!"(not (= L{out} R{out}))"
    else "true"

/-- The assertion's claim, byte for byte as `encode` writes it. -/
def claimOf (subject : Subject) : String :=
  match subject.outputs.map (differsAt subject) with
  | [] => "false"
  | [only] => only
  | many => s!"(or {String.intercalate " " many})"

/-- Lines joined by a newline. -/
def joinLines : List String → String
  | [] => ""
  | [line] => line
  | line :: rest => line ++ "\n" ++ joinLines rest

/-- The model pair a subject denotes. -/
def subjectPair (subject : Subject) : Pair where
  left := ofList subject.left 0
  right := ofList subject.right 0
  outputs := subject.outputs

/-- The whole query text, byte for byte as `encode` writes it for a fault-free encoding: sliced
defines only the cone, unsliced defines every node. -/
def renderQuery (subject : Subject) (slice : Bool) : String :=
  let names := slotNames subject slice
  let declares := names.map fun name => s!"(declare-const {name} (_ BitVec 64))"
  let defines := (definesOf subject.left slice subject.outputs).map (fun entry => defineFunOf "L" entry.1 entry.2)
    ++ (definesOf subject.right slice subject.outputs).map (fun entry => defineFunOf "R" entry.1 entry.2)
  let values := if names.isEmpty then [] else [s!"(get-value ({String.intercalate " " names}))"]
  joinLines (["(set-logic QF_BV)", "(set-option :produce-models true)"]
    ++ declares ++ defines ++ [s!"(assert {claimOf subject})", "(check-sat)"] ++ values ++ ["(exit)"]) ++ "\n"

/-! ### The semantics of an emitted body -/

/-- The value of one emitted body: the node's operator over the earlier nodes' values, on the
`BitVec 64` / `Bool` fragment, with no value for a type-mismatched operand. -/
def bodyValue (values : Nat → Option Val) (inputs : Nat → BitVec 64) : Node → Option Val
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

/-- The semantics of a query body with `sub`'s operands swapped: the seeded encoder fault. -/
def bodyValueSubSwapped (values : Nat → Option Val) (inputs : Nat → BitVec 64) : Node → Option Val
  | .sub a b =>
      match values a, values b with
      | some (.i64 x), some (.i64 y) => some (.i64 (y - x))
      | _, _ => none
  | node => bodyValue values inputs node

/-- The graph evaluator of one node is the value of that node's emitted body. -/
theorem evalStep_eq_bodyValue (graph : Graph) (values : Nat → Option Val)
    (inputs : Nat → BitVec 64) (index : Nat) :
    evalStep graph values inputs index = bodyValue values inputs (graph.node index) := by
  unfold evalStep
  cases graph.node index <;> rfl

/-- `evalStep` reads only the node's operands. -/
theorem evalStep_congr (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (values other : Nat → Option Val)
    (Agree : ∀ operand ∈ (graph.node index).operands, values operand = other operand) :
    evalStep graph values inputs index = evalStep graph other inputs index := by
  cases h : graph.node index with
  | input slot => simp [evalStep, h]
  | constant bits => simp [evalStep, h]
  | add a b =>
      simp only [evalStep, h]
      rw [Agree a (by simp [h, Node.operands]), Agree b (by simp [h, Node.operands])]
  | sub a b =>
      simp only [evalStep, h]
      rw [Agree a (by simp [h, Node.operands]), Agree b (by simp [h, Node.operands])]
  | mul a b =>
      simp only [evalStep, h]
      rw [Agree a (by simp [h, Node.operands]), Agree b (by simp [h, Node.operands])]
  | eq a b =>
      simp only [evalStep, h]
      rw [Agree a (by simp [h, Node.operands]), Agree b (by simp [h, Node.operands])]

/-- The value of the first `count + 1` nodes, at `count`. -/
theorem valuesUpTo_succ (graph : Graph) (inputs : Nat → BitVec 64) (count index : Nat) :
    valuesUpTo graph inputs (count + 1) index =
      (if index = count then evalStep graph (valuesUpTo graph inputs count) inputs count
       else valuesUpTo graph inputs count index) := by
  simp [valuesUpTo]

/-- An earlier node's value does not change once the value list has reached a later index. -/
theorem valuesUpTo_stable (graph : Graph) (inputs : Nat → BitVec 64) :
    ∀ (bound index : Nat), index < bound →
      valuesUpTo graph inputs bound index = valuesUpTo graph inputs (index + 1) index
  | 0, index, h => absurd h (Nat.not_lt_zero index)
  | bound + 1, index, h => by
      rw [valuesUpTo_succ]
      by_cases hhit : index = bound
      · subst hhit
        simp [valuesUpTo_succ]
      · rw [ite_eq_right hhit]
        exact valuesUpTo_stable graph inputs bound index (by omega)

/-- **The `define-fun` fixed point.** A node's value is its emitted body evaluated with the values
of the earlier nodes — which is exactly how a query's bodies are read, one symbol per node. -/
theorem value_eq_bodyValue (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (Earlier : ∀ operand ∈ (graph.node index).operands, operand < index) :
    value graph inputs index = bodyValue (fun j => value graph inputs j) inputs (graph.node index) := by
  unfold value
  rw [valuesUpTo_succ]
  rw [ite_eq_left rfl]
  rw [evalStep_congr graph inputs index (valuesUpTo graph inputs index) (fun j => value graph inputs j)
    (fun operand hmem => valuesUpTo_stable graph inputs index operand (Earlier operand hmem))]
  exact evalStep_eq_bodyValue graph (fun j => value graph inputs j) inputs index

/-- **The encoded node bodies simulate the graph, node by node, for every input.** -/
theorem bodyValue_simulates (nodes : List Node) (out : Nat) (inputs : Nat → BitVec 64)
    (index : Nat)
    (Earlier : ∀ operand ∈ (nodes.getD index (.constant 0)).operands, operand < index) :
    bodyValue (fun j => value (ofList nodes out) inputs j) inputs (nodes.getD index (.constant 0)) =
      value (ofList nodes out) inputs index := by
  have hfixed := value_eq_bodyValue (ofList nodes out) inputs index (Earlier := by
    intro operand hmem
    exact Earlier operand hmem)
  exact hfixed.symm

/-- The value of a node in a subject's graph. -/
def nodeValues (nodes : List Node) (inputs : Nat → BitVec 64) : Nat → Option Val :=
  fun index => value (ofList nodes 0) inputs index

/-- One compared output's value: the two emitted bodies' values differ. -/
def claimAt (subject : Subject) (inputs : Nat → BitVec 64) (out : Nat) : Bool :=
  decide (bodyValue (nodeValues subject.left inputs) inputs (subject.left.getD out (.constant 0)) ≠
    bodyValue (nodeValues subject.right inputs) inputs (subject.right.getD out (.constant 0)))

/-- The assertion's value: some listed output differs. -/
def claimValue (subject : Subject) (inputs : Nat → BitVec 64) : Bool :=
  subject.outputs.any (claimAt subject inputs)

/-- `List.any` respects a pointwise equality. -/
theorem any_congruence (list : List Nat) (first second : Nat → Bool)
    (Agree : ∀ value ∈ list, first value = second value) : list.any first = list.any second := by
  induction list with
  | nil => rfl
  | cons head rest ih =>
      simp only [List.any_cons]
      rw [Agree head (by simp), ih fun value hmem => Agree value (by simp [hmem])]

/-- One compared output of the query is one compared output of the model. -/
theorem claimAt_simulates (subject : Subject) (inputs : Nat → BitVec 64) (out : Nat)
    (EarlierLeft : ∀ operand ∈ (subject.left.getD out (.constant 0)).operands, operand < out)
    (EarlierRight : ∀ operand ∈ (subject.right.getD out (.constant 0)).operands, operand < out) :
    claimAt subject inputs out =
      decide (value (ofList subject.left 0) inputs out ≠ value (ofList subject.right 0) inputs out) := by
  unfold claimAt nodeValues
  rw [bodyValue_simulates subject.left 0 inputs out EarlierLeft,
    bodyValue_simulates subject.right 0 inputs out EarlierRight]

/-- The admitted-fragment condition on one node list, in the finite form `decide` checks: every
operand of every listed node is an earlier node. -/
def EarlierWithin (nodes : List Node) : Bool :=
  (List.range nodes.length).all fun index =>
    ((nodes.getD index (.constant 0)).operands).all fun operand => operand < index

/-- A checked `EarlierWithin` is the operand-order premise of `claim_simulates`. -/
theorem EarlierWithin.mp {nodes : List Node} (checked : EarlierWithin nodes = true) :
    ∀ index, index < nodes.length →
      ∀ operand ∈ (nodes.getD index (.constant 0)).operands, operand < index := by
  intro index hin operand hmem
  have hall := List.all_eq_true.mp checked index (List.mem_range.mpr hin)
  simpa using List.all_eq_true.mp hall operand hmem

/-- The admitted-fragment condition on a subject's outputs, in the finite form `decide` checks:
every listed output is a node of both graphs. -/
def OutputsWithin (subject : Subject) : Bool :=
  subject.outputs.all fun out => out < subject.left.length && out < subject.right.length

/-- A checked `OutputsWithin` is the output-range premise of `claim_simulates`. -/
theorem OutputsWithin.mp {subject : Subject} (checked : OutputsWithin subject = true) :
    ∀ out ∈ subject.outputs, out < subject.left.length ∧ out < subject.right.length := by
  intro out hmem
  simpa using List.all_eq_true.mp checked out hmem

/-- **The query's assertion is the refutation predicate, for every 64-bit input.** For a subject
inside the admitted fragment — every operand an earlier node on both sides, every listed output a
node of both graphs — the claim the encoded assertion makes, read through the emitted bodies, is
exactly `refuted` of the model pair the subject denotes. -/
theorem claim_simulates (subject : Subject) (inputs : Nat → BitVec 64)
    (EarlierLeft : ∀ index, index < subject.left.length →
      ∀ operand ∈ (subject.left.getD index (.constant 0)).operands, operand < index)
    (EarlierRight : ∀ index, index < subject.right.length →
      ∀ operand ∈ (subject.right.getD index (.constant 0)).operands, operand < index)
    (OutputsIn : ∀ out ∈ subject.outputs, out < subject.left.length ∧ out < subject.right.length) :
    claimValue subject inputs = refuted (subjectPair subject) inputs := by
  unfold claimValue refuted subjectPair
  apply any_congruence
  intro out hmem
  exact claimAt_simulates subject inputs out
    (EarlierLeft out (OutputsIn out hmem).1) (EarlierRight out (OutputsIn out hmem).2)

end NobleFoundation.SymbolicEncoder
