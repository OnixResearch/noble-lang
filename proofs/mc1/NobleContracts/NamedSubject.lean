import NobleContracts.Model

/-!
A separate, versioned semantics for immutable named source definitions.  This
module does not change the MC1-v1 `Op`, its claim, or any historical evidence.
A source consumer must independently prove that each `Use` and `Entry` comes
from one accepted, immutable submission; these records are not acceptance
certificates and are not a replacement for source/compiler correspondence.
-/

namespace NobleContracts.NamedV1

/-- Exact source bytes are retained instead of treating a session-local u64 or
an alias spelling as a portable identity.  The slot identifies the original
definition within that immutable module, not its current alias. -/
structure Provenance where
  moduleName : String
  moduleVersion : Nat
  moduleSource : List UInt8
  definitionSlot : Nat
  definitionName : String
  definitionSource : List UInt8
  deriving DecidableEq, Repr

/-- One monomorphic use record.  `slot` is the intended specialization index
in the accepted submission; `occurrence` is its caller's source node ID.
Only an independent source/accepted-recipe check can authenticate either.
Different uses of a definition may have different interfaces. -/
structure Use where
  slot : Nat
  occurrence : Nat
  caller : Provenance
  origin : Provenance
  input : List NobleContracts.Ty
  output : List NobleContracts.Ty

mutual
  /-- Named source programs and Syntax retain the source-ordered recipe;
`reflect` never silently substitutes an inlined definition body. -/
  inductive Value where
    | unit
    | bool : Bool → Value
    | i64 : BitVec 64 → Value
    | text : String → Value
    | pair : Value → Value → Value
    | inl : Value → Value
    | inr : Value → Value
    | list : List Value → Value
    | program : List Op → Value
    | syntax : List Op → Value

  inductive Op where
    | lit : Value → Op
    | word : Nat → Op
    | block : List Op → Op
    | call : Use → Op
end

abbrev Stack := List Value

mutual
  /-- Source order is preserved by `List`; dependency collection additionally
  visits quotations and first-class literal recipes, not just the root list. -/
  def usesValue : Value → List Use
    | .unit | .bool _ | .i64 _ | .text _ => []
    | .pair left right => usesValue left ++ usesValue right
    | .inl value | .inr value => usesValue value
    | .list values => values.flatMap usesValue
    | .program body | .syntax body => usesCode body

  def usesOp : Op → List Use
    | .lit value => usesValue value
    | .word _ => []
    | .block body => usesCode body
    | .call use => [use]

  def usesCode : List Op → List Use
    | [] => []
    | op :: rest => usesOp op ++ usesCode rest
end

structure Entry where
  slot : Nat
  origin : Provenance
  input : List NobleContracts.Ty
  output : List NobleContracts.Ty
  body : List Op
  /-- Intended accepted definition indices, deduplicated from invocation
  nodes, including nodes inside quotations. -/
  dependencies : List Nat
  rank : Nat

structure Environment where
  entries : List Entry

/-- Resolution cannot identify two distinct call sites as one source node.
They may, however, invoke the very same checked specialization. -/
def Entry.matches (entry : Entry) (use : Use) : Prop :=
  entry.slot = use.slot ∧ entry.origin = use.origin ∧
    entry.input = use.input ∧ entry.output = use.output

/-- Resolution is by the accepted table slot, then the verifier compares the
entire source and interface record.  A matching name alone grants nothing. -/
def Environment.lookup (env : Environment) (slot : Nat) : Option Entry :=
  env.entries.find? (fun entry => entry.slot == slot)

/-- The closure proof obligation: declared root and entry edges are neither
missing nor invented, every caller and callee keeps its full immutable source
provenance, no table slot is duplicated, and all edges strictly descend.
The independent source projection must prove this predicate against the
accepted root, transitive body nodes and environment rows; self-supplied
records are not source acceptance evidence. -/
def ExactClosure (env : Environment) (subject : Provenance) (root : List Op)
    (rootDependencies : List Nat) (rootRank : Nat) : Prop :=
  (env.entries.map (fun entry => entry.slot)).Nodup ∧
  (∀ entry ∈ env.entries,
    entry.dependencies.Nodup ∧
    (∀ slot ∈ entry.dependencies, ∃ use ∈ usesCode entry.body, use.slot = slot) ∧
    (∀ use ∈ usesCode entry.body,
      use.caller = entry.origin ∧ use.slot ∈ entry.dependencies) ∧
    ∀ use ∈ usesCode entry.body,
      ∃ target, env.lookup use.slot = some target ∧
        target.matches use ∧ target.rank < entry.rank) ∧
  rootDependencies.Nodup ∧
  (∀ slot ∈ rootDependencies, ∃ use ∈ usesCode root, use.slot = slot) ∧
  (∀ use ∈ usesCode root,
    use.caller = subject ∧ use.slot ∈ rootDependencies ∧
      ∃ target, env.lookup use.slot = some target ∧
        target.matches use ∧ target.rank < rootRank)

mutual
  /-- Semantic execution is normal-return only.  It makes no termination,
  resource, Wasm-correctness or effect-authority claim.  A call executes the
  resolved entry, not a fresh parse by name; a `Subject` additionally requires
  ranked closure and independently derived typing for all environment rows. -/
  inductive Run (env : Environment) : List Op → Stack → Stack → Prop where
    | nil {s} : Run env [] s s
    | cons {op ops s m t} : Step env op s m → Run env ops m t → Run env (op :: ops) s t

  inductive Step (env : Environment) : Op → Stack → Stack → Prop where
    | lit {v s} : Step env (.lit v) s (v :: s)
    | block {p s} : Step env (.block p) s (.program p :: s)
    | call {use entry s t} : env.lookup use.slot = some entry →
        entry.matches use → Run env entry.body s t → Step env (.call use) s t
    | dup {v s} : Step env (.word 0) (v :: s) (v :: v :: s)
    | drop {v s} : Step env (.word 1) (v :: s) s
    | swap {a b s} : Step env (.word 2) (b :: a :: s) (a :: b :: s)
    | dip {p v s t} : Run env p s t →
        Step env (.word 3) (.program p :: v :: s) (v :: t)
    | add {a b : BitVec 64} {s} :
        Step env (.word 4) (.i64 b :: .i64 a :: s) (.i64 (a + b) :: s)
    | sub {a b : BitVec 64} {s} :
        Step env (.word 5) (.i64 b :: .i64 a :: s) (.i64 (a - b) :: s)
    | mul {a b : BitVec 64} {s} :
        Step env (.word 6) (.i64 b :: .i64 a :: s) (.i64 (a * b) :: s)
    | equals {a b : BitVec 64} {s} :
        Step env (.word 7) (.i64 b :: .i64 a :: s) (.bool (a == b) :: s)
    | quote {v s} : Step env (.word 8) (v :: s) (.program [.lit v] :: s)
    | compose {p q s} : Step env (.word 9) (.program q :: .program p :: s)
        (.program (p ++ q) :: s)
    | run {p s t} : Run env p s t → Step env (.word 10) (.program p :: s) t
    | reflect {p s} : Step env (.word 11) (.program p :: s) (.syntax p :: s)
    | unit {s} : Step env (.word 12) s (.unit :: s)
    | pair {a b s} : Step env (.word 13) (b :: a :: s) (.pair a b :: s)
    | unpair {a b s} : Step env (.word 14) (.pair a b :: s) (b :: a :: s)
    | inl {v s} : Step env (.word 15) (v :: s) (.inl v :: s)
    | inr {v s} : Step env (.word 16) (v :: s) (.inr v :: s)
    | caseLeft {p q v s t} : Run env p (v :: s) t →
        Step env (.word 17) (.program q :: .program p :: .inl v :: s) t
    | caseRight {p q v s t} : Run env q (v :: s) t →
        Step env (.word 17) (.program q :: .program p :: .inr v :: s) t
    | ifTrue {p q s t} : Run env p s t →
        Step env (.word 18) (.program q :: .program p :: .bool true :: s) t
    | ifFalse {p q s t} : Run env q s t →
        Step env (.word 18) (.program q :: .program p :: .bool false :: s) t
    | nilList {s} : Step env (.word 19) s (.list [] :: s)
    | consList {v vs s} : Step env (.word 20) (.list vs :: v :: s) (.list (v :: vs) :: s)
    | listNil {p q s t} : Run env p s t →
        Step env (.word 21) (.program q :: .program p :: .list [] :: s) t
    | listCons {p q v vs s t} : Run env q (.list vs :: v :: s) t →
        Step env (.word 21) (.program q :: .program p :: .list (v :: vs) :: s) t
end

/-- The complete initial and final stack is retained; no successful execution
can be invented for an unknown word or a missing definition. -/
def Exec (env : Environment) (code : List Op) (before after : Stack) : Prop :=
  Run env code before.reverse after.reverse

mutual
  inductive HasType (env : Environment) : Value → NobleContracts.Ty → Prop where
    | unit : HasType env .unit .unit
    | bool {b} : HasType env (.bool b) .bool
    | i64 {n} : HasType env (.i64 n) .i64
    | text {s} : HasType env (.text s) .text
    | syntax {p} : HasType env (.syntax p) .syntax
    | pair {a b ta tb} : HasType env a ta → HasType env b tb →
        HasType env (.pair a b) (.pair ta tb)
    | inl {v a b} : HasType env v a → HasType env (.inl v) (.sum a b)
    | inr {v a b} : HasType env v b → HasType env (.inr v) (.sum a b)
    | list {vs t} : (∀ v ∈ vs, HasType env v t) → HasType env (.list vs) (.list t)
    | program {p i o} : CodeTyped env p i o → HasType env (.program p) (.program i o)

  inductive CodeTyped (env : Environment) : List Op → List NobleContracts.Ty → List NobleContracts.Ty → Prop where
    | nil {s} : CodeTyped env [] s s
    | cons {op ops s m t} : OpTyped env op s m → CodeTyped env ops m t →
        CodeTyped env (op :: ops) s t

  inductive OpTyped (env : Environment) : Op → List NobleContracts.Ty → List NobleContracts.Ty → Prop where
    | lit {v ty s} : HasType env v ty → OpTyped env (.lit v) s (s ++ [ty])
    | block {p i o s} : CodeTyped env p i o →
        OpTyped env (.block p) s (s ++ [.program i o])
    | call {use entry s} : env.lookup use.slot = some entry → entry.matches use →
        CodeTyped env entry.body (s ++ use.input) (s ++ use.output) →
        OpTyped env (.call use) (s ++ use.input) (s ++ use.output)
    | dup {a s} : OpTyped env (.word 0) (s ++ [a]) (s ++ [a, a])
    | drop {a s} : OpTyped env (.word 1) (s ++ [a]) s
    | swap {a b s} : OpTyped env (.word 2) (s ++ [a, b]) (s ++ [b, a])
    | dip {a s t} : OpTyped env (.word 3) (s ++ [a, .program s t]) (t ++ [a])
    | add {s} : OpTyped env (.word 4) (s ++ [.i64, .i64]) (s ++ [.i64])
    | sub {s} : OpTyped env (.word 5) (s ++ [.i64, .i64]) (s ++ [.i64])
    | mul {s} : OpTyped env (.word 6) (s ++ [.i64, .i64]) (s ++ [.i64])
    | equals {s} : OpTyped env (.word 7) (s ++ [.i64, .i64]) (s ++ [.bool])
    | quote {a s t} : OpTyped env (.word 8) (s ++ [a]) (s ++ [.program t (t ++ [a])])
    | compose {i m o s} : OpTyped env (.word 9)
        (s ++ [.program i m, .program m o]) (s ++ [.program i o])
    | run {s t} : OpTyped env (.word 10) (s ++ [.program s t]) t
    | reflect {i o s} : OpTyped env (.word 11) (s ++ [.program i o]) (s ++ [.syntax])
    | unit {s} : OpTyped env (.word 12) s (s ++ [.unit])
    | pair {a b s} : OpTyped env (.word 13) (s ++ [a, b]) (s ++ [.pair a b])
    | unpair {a b s} : OpTyped env (.word 14) (s ++ [.pair a b]) (s ++ [a, b])
    | inl {a b s} : OpTyped env (.word 15) (s ++ [a]) (s ++ [.sum a b])
    | inr {a b s} : OpTyped env (.word 16) (s ++ [b]) (s ++ [.sum a b])
    | caseWord {a b s t} : OpTyped env (.word 17)
        (s ++ [.sum a b, .program (s ++ [a]) t, .program (s ++ [b]) t]) t
    | ifWord {s t} : OpTyped env (.word 18)
        (s ++ [.bool, .program s t, .program s t]) t
    | nilList {a s} : OpTyped env (.word 19) s (s ++ [.list a])
    | consList {a s} : OpTyped env (.word 20) (s ++ [a, .list a]) (s ++ [.list a])
    | listCase {a s t} : OpTyped env (.word 21)
        (s ++ [.list a, .program s t, .program (s ++ [a, .list a]) t]) t
end

/-- Every listed dependency is an actual call occurrence, and every call
occurrence names a listed dependency.  Checking only one implication would
permit either invented edges or an omitted transitive definition. -/
theorem dependencies_exact (h : ExactClosure env subject root rootDependencies rootRank)
    (entry : Entry) (he : entry ∈ env.entries) (slot : Nat) :
    slot ∈ entry.dependencies ↔
      ∃ use ∈ usesCode entry.body, use.slot = slot := by
  obtain ⟨_, hall, _, _, _⟩ := h
  obtain ⟨_, hsound, hcomplete, _⟩ := hall entry he
  exact ⟨hsound slot, fun ⟨use, hu, hslot⟩ => hslot ▸ (hcomplete use hu).2⟩

theorem root_dependencies_exact
    (h : ExactClosure env subject root rootDependencies rootRank) (slot : Nat) :
    slot ∈ rootDependencies ↔ ∃ use ∈ usesCode root, use.slot = slot := by
  obtain ⟨_, _, _, hsound, hcomplete⟩ := h
  exact ⟨hsound slot, fun ⟨use, hu, hslot⟩ => hslot ▸ (hcomplete use hu).2.1⟩

/-- Named edges resolve to full callee provenance and concrete per-use
interface, retain caller-side occurrences, and strictly descend in the table.
Binding those occurrences to actual accepted source is a separate obligation. -/
theorem resolved_edge (h : ExactClosure env subject root rootDependencies rootRank)
    (entry : Entry) (he : entry ∈ env.entries)
    (use : Use) (hu : use ∈ usesCode entry.body) :
    ∃ target, env.lookup use.slot = some target ∧
      target.matches use ∧ target.rank < entry.rank := by
  exact (h.2.1 entry he).2.2.2 use hu

def Direct (env : Environment) (parent child : Entry) : Prop :=
  parent ∈ env.entries ∧
    ∃ use ∈ usesCode parent.body, env.lookup use.slot = some child

inductive Descends (env : Environment) : Entry → Entry → Prop where
  | direct {parent child} : Direct env parent child → Descends env parent child
  | next {parent middle child} :
      Direct env parent middle → Descends env middle child →
      Descends env parent child

theorem direct_rank (h : ExactClosure env subject root rootDependencies rootRank)
    (hd : Direct env parent child) : child.rank < parent.rank := by
  obtain ⟨hp, use, hu, hlookup⟩ := hd
  obtain ⟨target, htarget, _, hrank⟩ := resolved_edge h parent hp use hu
  rw [hlookup] at htarget
  cases Option.some.inj htarget
  exact hrank

theorem descends_rank (h : ExactClosure env subject root rootDependencies rootRank)
    (path : Descends env parent child) : child.rank < parent.rank := by
  induction path with
  | direct edge => exact direct_rank h edge
  | next edge _ ih => exact Nat.lt_trans ih (direct_rank h edge)

/-- Finite rank decrease excludes direct and transitive recursive calls. -/
theorem no_recursive_named_call (h : ExactClosure env subject root rootDependencies rootRank)
    (entry : Entry) : ¬ Descends env entry entry := by
  intro path
  exact Nat.lt_irrefl entry.rank (descends_rank h path)

/-- A distinct NamedV1 proposition: no MC1-v1 export is implied.  Independent
source→IR correspondence and independent checking of each specialized entry
remain admission requirements before this proposition describes an accepted
source subject, rather than a manually assembled semantic fixture. -/
structure Subject where
  provenance : Provenance
  env : Environment
  root : List Op
  dependencies : List Nat
  rank : Nat
  closure : ExactClosure env provenance root dependencies rank
  checkedEntries : ∀ entry ∈ env.entries,
    CodeTyped env entry.body entry.input entry.output
  input : List NobleContracts.Ty
  output : List NobleContracts.Ty
  typed : CodeTyped env root input output

/-- A partial-correctness contract for a *NamedV1* subject only.  Its stack
predicate is separate from the frozen MC1-v1 `Holds`/`Obligation` predicates. -/
def Satisfies (subject : Subject) (pre : Stack → Prop)
    (post : Stack → Stack → Prop) : Prop :=
  ∀ before after, pre before → Exec subject.env subject.root before after →
    post before after

mutual
  def liftValue : NobleContracts.Value → Value
    | .unit => .unit
    | .bool b => .bool b
    | .i64 n => .i64 n
    | .text text => .text text
    | .pair left right => .pair (liftValue left) (liftValue right)
    | .inl value => .inl (liftValue value)
    | .inr value => .inr (liftValue value)
    | .list values => .list (values.map liftValue)
    | .program body => .program (body.map liftOp)
    | .syntax body => .syntax (body.map liftOp)

  def liftOp : NobleContracts.Op → Op
    | .lit value => .lit (liftValue value)
    | .word id => .word id
    | .block body => .block (body.map liftOp)
end

def liftCode (code : List NobleContracts.Op) : List Op := code.map liftOp
def liftStack (stack : NobleContracts.Stack) : Stack := stack.map liftValue

/-- Every normal return in the frozen builtin-only model is a normal return
in this additive model, including runs through quotations and lists. -/
theorem lift_run {env : Environment} {code : List NobleContracts.Op}
    {before after : NobleContracts.Stack}
    (execution : NobleContracts.Run code before after) :
    Run env (liftCode code) (liftStack before) (liftStack after) := by
  apply NobleContracts.Run.recOn
    (motive_1 := fun code s t _ => Run env (liftCode code) (liftStack s) (liftStack t))
    (motive_2 := fun op s t _ => Step env (liftOp op) (liftStack s) (liftStack t)) execution
  all_goals intros
  all_goals simp only [liftOp, liftValue, liftCode, liftStack,
    List.map_cons, List.map_nil, List.map_append] at *
  all_goals first
    | exact .nil
    | exact .cons (by assumption) (by assumption)
    | exact .lit
    | exact .block
    | exact .dup
    | exact .drop
    | exact .swap
    | exact .dip (by assumption)
    | exact .add
    | exact .sub
    | exact .mul
    | exact .equals
    | exact .quote
    | exact .compose
    | exact .run (by assumption)
    | exact .reflect
    | exact .unit
    | exact .pair
    | exact .unpair
    | exact .inl
    | exact .inr
    | exact .caseLeft (by assumption)
    | exact .caseRight (by assumption)
    | exact .ifTrue (by assumption)
    | exact .ifFalse (by assumption)
    | exact .nilList
    | exact .consList
    | exact .listNil (by assumption)
    | exact .listCons (by assumption)

end NobleContracts.NamedV1
