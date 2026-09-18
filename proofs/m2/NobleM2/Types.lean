/-
Reference model: fragment v0 types and effect bounds (M2 fragment v0).

This module is an independent restatement of the finite type universe the
checker decides, not a translation of the Rust implementation.
Stacks are encoded as cons lists so recursive definitions stay structural.
-/

namespace NobleM2

/-- Stable identity of a resource kind. -/
abbrev ResourceKind := Nat

/-- Stable identity of one host-operation effect. -/
abbrev EffId := Nat

/-- A finite set of effect identities, kept in ascending order without repeats. -/
structure EffSet where
  ids : List Nat
  deriving Repr, DecidableEq, Inhabited

namespace EffSet

/-- The empty bound. -/
def empty : EffSet := ⟨[]⟩

/-- Membership. -/
def Mem (s : EffSet) (id : EffId) : Prop := id ∈ s.ids

instance (s : EffSet) (id : EffId) : Decidable (Mem s id) :=
  inferInstanceAs (Decidable (id ∈ s.ids))

/-- Least upper bound. -/
def union : EffSet → EffSet → EffSet
  | ⟨[]⟩, b => b
  | a, ⟨[]⟩ => a
  | ⟨x :: xs⟩, ⟨y :: ys⟩ =>
    if x < y then
      let rest := union ⟨xs⟩ ⟨y :: ys⟩
      ⟨x :: rest.ids⟩
    else if y < x then
      let rest := union ⟨x :: xs⟩ ⟨ys⟩
      ⟨y :: rest.ids⟩
    else
      let rest := union ⟨xs⟩ ⟨ys⟩
      ⟨x :: rest.ids⟩

/-- Insert one identity, keeping the list sorted and free of repeats. -/
def insert (id : EffId) : EffSet → EffSet
  | ⟨[]⟩ => ⟨[id]⟩
  | ⟨x :: xs⟩ =>
    if id < x then ⟨id :: x :: xs⟩
    else if id = x then ⟨x :: xs⟩
    else
      let rest := insert id ⟨xs⟩
      ⟨x :: rest.ids⟩

/-- Build from a list, keeping the result sorted and free of repeats. -/
def ofIds (ids : List EffId) : EffSet := ids.foldl (fun acc id => insert id acc) empty

/-- Inclusion: every identity here is present in `other`. -/
def Subset (a b : EffSet) : Prop := ∀ id, id ∈ a.ids → id ∈ b.ids

/-- Decidable inclusion, for the reference checker's guards. -/
def subsetOf (a b : EffSet) : Bool := a.ids.all (fun id => b.ids.elem id)

/-- The number of identities. -/
def card (s : EffSet) : Nat := s.ids.length

end EffSet

/-- The first identity of a derived bound that is absent from the allowed
one, if any. -/
def firstExtra (derived allowed : EffSet) : Option EffId :=
  derived.ids.find? (fun id => !allowed.ids.elem id)

mutual

  /-- A type in the fragment's finite universe. -/
  inductive Ty where
    | unit : Ty
    | bool : Ty
    | i64 : Ty
    | text : Ty
    /-- Inert syntax produced by `reflect`. -/
    | syn : Ty
    | pair : Ty → Ty → Ty
    | sum : Ty → Ty → Ty
    | list : Ty → Ty
    /-- A first-class program type: invocation stack, result stack, latent bound. -/
    | program : TyList → TyList → EffSet → Ty
    | resource : ResourceKind → Ty
    deriving Repr, DecidableEq, Inhabited

  /-- A finite stack of types, bottom first (the head is the stack bottom). -/
  inductive TyList where
    | nil : TyList
    | cons : Ty → TyList → TyList
    deriving Repr, DecidableEq, Inhabited

end

mutual

  /-- The recursive `Data` eligibility predicate. -/
  def Ty.isData : Ty → Bool
    | Ty.unit | Ty.bool | Ty.i64 | Ty.text | Ty.syn => true
    | Ty.pair a b | Ty.sum a b => a.isData && b.isData
    | Ty.list a => a.isData
    | Ty.program _ _ _ => true
    | Ty.resource _ => false

  /-- Whether every stack entry is `Data`. -/
  def TyList.isData : TyList → Bool
    | TyList.nil => true
    | TyList.cons head rest => head.isData && rest.isData

end

mutual

  /-- A size measure used by the declared type-size limit. -/
  def Ty.size : Ty → Nat
    | Ty.unit | Ty.bool | Ty.i64 | Ty.text | Ty.syn => 1
    | Ty.resource _ => 1
    | Ty.pair a b | Ty.sum a b => 1 + a.size + b.size
    | Ty.list a => 1 + a.size
    | Ty.program i o _ => 1 + i.size + o.size

  /-- Total size of every stack entry. -/
  def TyList.size : TyList → Nat
    | TyList.nil => 0
    | TyList.cons head rest => head.size + rest.size

end

namespace TyList

/-- The number of entries. -/
def length : TyList → Nat
  | TyList.nil => 0
  | TyList.cons _ rest => 1 + rest.length

/-- The entries as a list, bottom first. -/
def toList : TyList → List Ty
  | nil => []
  | cons head rest => head :: rest.toList

/-- Concatenation. -/
def append : TyList → TyList → TyList
  | TyList.nil, other => other
  | TyList.cons head rest, other => TyList.cons head (rest.append other)

/-- The one-entry stack. -/
def singleton (ty : Ty) : TyList := TyList.cons ty TyList.nil

/-- The topmost `count` entries, bottom first; the whole stack when shorter. -/
def takeTail (stack : TyList) (count : Nat) : TyList :=
  let length := stack.length
  if count ≤ length then
    let rec drop : Nat → TyList → TyList
      | 0, rest => rest
      | _ + 1, TyList.nil => TyList.nil
      | n + 1, TyList.cons _ rest => drop n rest
    drop (length - count) stack
  else stack

end TyList

/-- The stack prefix of `count` entries. -/
def prefixStack : Nat → TyList → TyList
  | 0, _ => .nil
  | _ + 1, .nil => .nil
  | count + 1, .cons head rest => .cons head (prefixStack count rest)

/-- The top `count` entries of a stack, or the whole stack when it is shorter. -/
def tailOf (stack : TyList) (count : Nat) : TyList := stack.takeTail count

/-- The stack with its top `expected` segment replaced by the result segment. -/
def replaceTail (stack expected out : TyList) : TyList :=
  (prefixStack (stack.length - expected.length) stack).append out

/-- Whether the expected segment is the stack's top segment. -/
def tailEquals (stack expected : TyList) : Bool :=
  stack.length ≥ expected.length && tailOf stack expected.length == expected

/-- Whether every element of a stack is `Data`. -/
def StackIsData (stack : TyList) : Prop := stack.isData = true

end NobleM2
