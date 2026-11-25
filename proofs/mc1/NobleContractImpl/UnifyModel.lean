import NobleContractImpl.Funs

/-!
# The restated `implicit-args/v1` model fragment

The foundation model of `implicit-proof-arguments` (`ImplicitUnification.lean`) is Mathlib-based
and cannot be imported by the Mathlib-free MC1 project, so this module restates the fragment the
extracted `noble_contracts::inference` solver implements: metavariables with the arena's
`Value`/`Stack` sort, rigid atoms with their identity payloads, the binary constructors and list
cells, substitution, occurrence, size, and the model's head decision. It also proves the
semantics the extracted step's outcomes are checked against: a sort disagreement, differing rigid
atoms or constructors have no sort-preserving unifier, an occurs failure has none either, and the
decomposition and binding cases preserve exactly the model's solution set.
-/

namespace NobleContractImpl.UnifyModel

open Aeneas Aeneas.Std Aeneas.Std.Result

set_option maxHeartbeats 1000000

abbrev ASort := noble_contracts.inference.Sort
abbrev Term := noble_contracts.inference.Term
abbrev Arena := noble_contracts.inference.Arena

/-! ## Terms and their algebra

The `implicit-args/v1` model of `proofs/foundation/NobleFoundation/ImplicitUnification.lean`
does not import into this project (it uses Mathlib), so the fragment the extracted solver
implements is restated here: metavariables with the arena's `Value`/`Stack` sort, rigid atoms
with their identity payloads, the binary constructors, and list cells. A metavariable carries
its arena id, because the extracted solver's fast path compares ids, not terms. -/

/-- Rigid leaves. Payloads are unpacked so that identity is the extracted comparison:
`RigidValue`/`RigidStack`/`Resource` compare `u32`s and `Nominal`/`GenericNominal` compare the
`NominalTypeId` fields the generated `PartialEq` compares. -/
inductive Atom where
  | rigidValue (id : Std.U32)
  | rigidStack (id : Std.U32)
  | unit
  | boolean
  | i64
  | text
  | syntax
  | contract
  | evidence
  | certified
  | resource (kind : Std.U32)
  | nominal (module' : Std.U64) (ordinal : Std.U32)
  | empty
  deriving DecidableEq, Repr

/-- Binary constructors. `generic` carries the `NominalTypeId` the extracted comparison reads. -/
inductive Kind where
  | pair
  | sum
  | program
  | push
  | generic (module' : Std.U64) (ordinal : Std.U32)
  deriving DecidableEq, Repr

/-- Model terms. A meta is an arena hole, identified by its id and its sort. -/
inductive Tm where
  | mvar (id : Std.U32) (sort : ASort)
  | atom (atom : Atom)
  | bin (kind : Kind) (left right : Tm)
  | list (item : Tm)

/-- The model's sorts, decided from the term alone. -/
def Tm.sortOf : Tm → ASort
  | .mvar _ s => s
  | .atom (.rigidStack _) => .Stack
  | .atom .empty => .Stack
  | .atom _ => .Value
  | .bin .push _ _ => .Stack
  | .bin _ _ _ => .Value
  | .list _ => .Value

/-- The metas of a term. -/
def Tm.vars : Tm → List Std.U32
  | .mvar id _ => [id]
  | .atom _ => []
  | .bin _ l r => l.vars ++ r.vars
  | .list x => x.vars

/-- Node count. -/
def Tm.size : Tm → Nat
  | .mvar _ _ => 1
  | .atom _ => 1
  | .bin _ l r => l.size + r.size + 1
  | .list x => x.size + 1

/-- A term without metas. -/
def Tm.Ground (t : Tm) : Prop := t.vars = []

/-- Apply a substitution to every meta. -/
def Tm.subst (σ : Std.U32 → Tm) : Tm → Tm
  | .mvar id _ => σ id
  | .atom a => .atom a
  | .bin k l r => .bin k (l.subst σ) (r.subst σ)
  | .list x => .list (x.subst σ)

/-- A substitution that preserves the sort of every meta. -/
def SortPreserving (σ : Std.U32 → Tm) : Prop :=
  ∀ id s, (Tm.mvar id s).subst σ = σ id ∧ (σ id).sortOf = s

/-- A cheaper, equivalent form: the substitution maps every meta to a term of its own sort. -/
def SortPreserving' (σ : Std.U32 → Tm) : Prop := ∀ id s, (σ id).sortOf = s

theorem sortPreserving_iff (σ : Std.U32 → Tm) :
    SortPreserving σ ↔ SortPreserving' σ := by
  constructor
  · intro h id s; exact (h id s).2
  · intro h id s; exact ⟨rfl, h id s⟩

/-- Structural equality of sorts as a `Bool`. -/
def sortEq : ASort → ASort → Bool
  | .Value, .Value => true
  | .Stack, .Stack => true
  | _, _ => false

theorem sortEq_iff (a b : ASort) : sortEq a b = true ↔ a = b := by
  cases a <;> cases b <;> simp [sortEq]

theorem sortEq_self (a : ASort) : sortEq a a = true := (sortEq_iff a a).mpr rfl

/-- Structural equality of atoms as a `Bool`, mirroring the extracted comparisons. -/
def atomEq (a b : Atom) : Bool :=
  match a, b with
  | .rigidValue x, .rigidValue y => x = y
  | .rigidStack x, .rigidStack y => x = y
  | .unit, .unit => true
  | .boolean, .boolean => true
  | .i64, .i64 => true
  | .text, .text => true
  | .syntax, .syntax => true
  | .contract, .contract => true
  | .evidence, .evidence => true
  | .certified, .certified => true
  | .resource x, .resource y => x = y
  | .nominal m o, .nominal m' o' => m = m' && o = o'
  | .empty, .empty => true
  | _, _ => false

theorem atomEq_iff (a b : Atom) : atomEq a b = true ↔ a = b := by
  cases a <;> cases b <;> simp [atomEq]

/-- Structural equality of binary constructors as a `Bool`. -/
def kindEq (a b : Kind) : Bool :=
  match a, b with
  | .pair, .pair => true
  | .sum, .sum => true
  | .program, .program => true
  | .push, .push => true
  | .generic m o, .generic m' o' => m = m' && o = o'
  | _, _ => false

theorem kindEq_iff (a b : Kind) : kindEq a b = true ↔ a = b := by
  cases a <;> cases b <;> simp [kindEq]

/-- The model's decision for one head equation, mirroring `ImplicitUnification.head` with the
arena's id fast path and its sort restriction. -/
inductive Head where
  /-- The ids coincide: the step consumes the equation. -/
  | skip
  /-- Two equal binary constructors: equate the children. -/
  | split (left1 right1 left2 right2 : Tm)
  /-- Two equal lists: equate the items. -/
  | splitList (left right : Tm)
  /-- Bind a meta (on `x`'s side) that does not occur in `t`. -/
  | bind (x : Std.U32) (sort : ASort) (t : Tm)
  /-- The two sides have different sorts. -/
  | sortClash
  /-- Rigid shapes with no unifier. -/
  | shapeClash
  /-- A meta equated with a term containing it. -/
  | cycle (x : Std.U32) (t : Tm)

/-- The model's head decision: identical ids skip; otherwise the sorts must agree, a meta binds
after the occurs check, equal rigid shapes decompose or skip, different rigid shapes clash. -/
def head (left right : Std.U32) (l r : Tm) : Head :=
  if left = right then .skip
  else if sortEq l.sortOf r.sortOf then
    match l, r with
    | .mvar x _, t => if x ∈ t.vars then .cycle x t else .bind x l.sortOf t
    | _, .mvar y _ => if y ∈ l.vars then .cycle y l else .bind y r.sortOf l
    | .atom a, .atom b => if atomEq a b then .skip else .shapeClash
    | .bin ka l1 l2, .bin kb r1 r2 =>
        if kindEq ka kb then .split l1 r1 l2 r2 else .shapeClash
    | .list x, .list y => .splitList x y
    | _, _ => .shapeClash
  else .sortClash

/-- The reading of a term from the arena's children readings: holes read as metas of their own
id and sort, rigid shapes read structurally, and a link reads as the target. -/
def toTm (read : Std.U32 → Option Tm) (id : Std.U32) : Term → Option Tm
  | .HoleTerm s => some (.mvar id s)
  | .RigidValueTerm n => some (.atom (.rigidValue n))
  | .RigidStackTerm n => some (.atom (.rigidStack n))
  | .LinkTerm (next : Std.U32) => read next
  | .UnitTerm => some (.atom .unit)
  | .BoolTerm => some (.atom .boolean)
  | .I64Term => some (.atom .i64)
  | .TextTerm => some (.atom .text)
  | .SyntaxTerm => some (.atom .syntax)
  | .ContractTerm => some (.atom .contract)
  | .EvidenceTerm => some (.atom .evidence)
  | .CertifiedTerm => some (.atom .certified)
  | .ResourceTerm k => some (.atom (.resource k))
  | .NominalTerm n => some (.atom (.nominal n.module n.ordinal))
  | .GenericNominalTerm n a b =>
      (read a).bind fun x => (read b).map fun y => .bin (.generic n.module n.ordinal) x y
  | .PairTerm a b => (read a).bind fun x => (read b).map fun y => .bin .pair x y
  | .SumTerm a b => (read a).bind fun x => (read b).map fun y => .bin .sum x y
  | .ListTerm item => (read item).map fun x => .list x
  | .ProgramTerm a b => (read a).bind fun x => (read b).map fun y => .bin .program x y
  | .EmptyTerm => some (.atom .empty)
  | .PushTerm a b => (read a).bind fun x => (read b).map fun y => .bin .push x y

/-- The fuel-indexed reading of an arena id: a link is chased (one fuel per link and per term
layer), and a non-link term reads through `toTm` at the children's fuel. -/
def read (self : Arena) : Nat → Std.U32 → Option Tm
  | 0, _ => none
  | fuel + 1, id =>
      match self.terms.val[id.val]? with
      | none => none
      | some (.LinkTerm next) => read self fuel next
      | some t => toTm (read self fuel) id t

/-- A term is link-free: not a link. -/
def Term.linkFree : Term → Prop
  | .LinkTerm _ => False
  | _ => True

instance (t : Term) : Decidable (Term.linkFree t) := by
  cases t with
  | LinkTerm _ => exact isFalse (fun h => h)
  | _ => exact isTrue trivial


/-! ## The model's semantics of the solver's outcomes -/

/-- The sort of a substituted term is the sort of the term under a sort-preserving
substitution. -/
theorem sortOf_subst {σ : Std.U32 → Tm} (hσ : SortPreserving' σ) (t : Tm) :
    (t.subst σ).sortOf = t.sortOf := by
  cases t with
  | mvar id s => exact hσ id s
  | atom a => rfl
  | bin k l r => cases k <;> rfl
  | list x => rfl

/-- A meta's image is no larger than any term that contains it, and is smaller unless the term is
that very meta. -/
theorem size_lt_of_mem {σ : Std.U32 → Tm} {x : Std.U32} :
    ∀ {t : Tm}, x ∈ t.vars → t ≠ .mvar x t.sortOf → (σ x).size < (t.subst σ).size := by
  intro t
  induction t with
  | mvar id s =>
      intro hx hne
      simp [Tm.vars] at hx
      subst hx
      exact absurd rfl hne
  | atom a => intro hx hne; simp [Tm.vars] at hx
  | bin k l r ihl ihr =>
      intro hx hne
      simp only [Tm.vars, List.mem_append] at hx
      simp only [Tm.subst, Tm.size]
      have h1 : (σ x).size ≤ (l.subst σ).size ∨ (σ x).size ≤ (r.subst σ).size := by
        rcases hx with hx | hx
        · left
          by_cases h : l = .mvar x l.sortOf
          · rw [h]; simp [Tm.subst]
          · exact Nat.le_of_lt (ihl hx h)
        · right
          by_cases h : r = .mvar x r.sortOf
          · rw [h]; simp [Tm.subst]
          · exact Nat.le_of_lt (ihr hx h)
      omega
  | list x' ih =>
      intro hx hne
      simp only [Tm.vars] at hx
      simp only [Tm.subst, Tm.size]
      by_cases h : x' = .mvar x x'.sortOf
      · rw [h]; simp [Tm.subst]
      · have := ih hx h; omega

/-- An occurs failure has no sort-preserving unifier: a meta bound to a term that contains it
already forces the two sides apart by size. -/
theorem occurs_no_unifier {x : Std.U32} {s : ASort} {t : Tm} {σ : Std.U32 → Tm}
    (hx : x ∈ t.vars) (hne : ∀ s', t ≠ .mvar x s') :
    (Tm.mvar x s).subst σ ≠ t.subst σ := by
  intro heq
  have hlt := size_lt_of_mem (σ := σ) hx (hne t.sortOf)
  simp only [Tm.subst] at heq
  rw [heq] at hlt
  exact Nat.lt_irrefl _ hlt

/-- A sort disagreement has no sort-preserving unifier. -/
theorem sort_mismatch_no_unifier {l r : Tm} {σ : Std.U32 → Tm} (hσ : SortPreserving' σ)
    (h : l.sortOf ≠ r.sortOf) : l.subst σ ≠ r.subst σ := by
  intro heq
  have := congrArg Tm.sortOf heq
  simp only [sortOf_subst hσ] at this
  exact h this

/-- Different rigid atoms stay different under every substitution. -/
theorem atom_mismatch_no_unifier {a b : Atom} (h : a ≠ b) (σ : Std.U32 → Tm) :
    (Tm.atom a).subst σ ≠ (Tm.atom b).subst σ := by
  intro heq
  simp only [Tm.subst] at heq
  exact h (Tm.atom.inj heq)

/-- Different binary constructors stay different under every substitution. -/
theorem bin_kind_mismatch_no_unifier {k k' : Kind} (h : k ≠ k') (l1 l2 r1 r2 : Tm)
    (σ : Std.U32 → Tm) :
    (Tm.bin k l1 l2).subst σ ≠ (Tm.bin k' r1 r2).subst σ := by
  intro heq
  simp only [Tm.subst] at heq
  exact h (Tm.bin.inj heq).1

/-- Equal binary constructors decompose: the children's equations are exactly the parent's. -/
theorem bin_unifies_iff {k : Kind} {l1 l2 r1 r2 : Tm} {σ : Std.U32 → Tm} :
    (Tm.bin k l1 l2).subst σ = (Tm.bin k r1 r2).subst σ ↔
      l1.subst σ = r1.subst σ ∧ l2.subst σ = r2.subst σ := by
  simp [Tm.subst]

/-- Equal list constructors decompose. -/
theorem list_unifies_iff {l r : Tm} {σ : Std.U32 → Tm} :
    (Tm.list l).subst σ = (Tm.list r).subst σ ↔ l.subst σ = r.subst σ := by
  simp [Tm.subst]

/-- Binding a meta to a term is exactly requiring the substitution to send the meta to the term's
instance. -/
theorem bind_unifies_iff {x : Std.U32} {s : ASort} {t : Tm} {σ : Std.U32 → Tm} :
    (Tm.mvar x s).subst σ = t.subst σ ↔ σ x = t.subst σ := by
  simp [Tm.subst]

/-! ## The head decision on model terms

These connect the model's `head` decision to the branches the extracted step distinguishes. -/

/-- Identical ids skip. -/
theorem head_self (left : Std.U32) (l : Tm) : head left left l l = .skip := by
  simp [head]

/-- A sort disagreement is the model's sort clash. -/
theorem head_sortClash {left right : Std.U32} {l r : Tm} (hne : left ≠ right)
    (h : sortEq l.sortOf r.sortOf = false) : head left right l r = .sortClash := by
  simp [head, *]

/-- Differing rigid atoms are the model's shape clash. -/
theorem head_atomClash {left right : Std.U32} {a b : Atom} (hne : left ≠ right)
    (hs : sortEq (Tm.atom a).sortOf (Tm.atom b).sortOf = true)
    (h : atomEq a b = false) : head left right (.atom a) (.atom b) = .shapeClash := by
  simp [head, *]

/-- Equal binary constructors split into the children's equations. -/
theorem head_bin {left right : Std.U32} {k : Kind} {l1 l2 r1 r2 : Tm} (hne : left ≠ right)
    (hs : sortEq (Tm.bin k l1 l2).sortOf (Tm.bin k r1 r2).sortOf = true)
    (hk : kindEq k k = true) : head left right (.bin k l1 l2) (.bin k r1 r2) =
      .split l1 r1 l2 r2 := by
  simp [head, *]

/-- Equal list constructors split into the items' equation. -/
theorem head_list {left right : Std.U32} {l r : Tm} (hne : left ≠ right)
    (hs : sortEq (Tm.list l).sortOf (Tm.list r).sortOf = true) :
    head left right (.list l) (.list r) = .splitList l r := by
  simp [head, *]

end NobleContractImpl.UnifyModel
