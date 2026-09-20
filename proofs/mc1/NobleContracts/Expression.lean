import NobleContracts.Rules

namespace NobleContracts

/-- Resolved expression syntax. Bound names and source comments are not propositions. -/
inductive Term where
  | i64 (value : Int)
  | bool (value : Bool)
  | unit
  | input (index : Nat)
  | output (index : Nat)
  | param (index : Nat)
  | definition (index : Nat) (body : Term)
  | not (body : Term)
  | and (left right : Term)
  | or (left right : Term)
  | implies (left right : Term)
  | eq (left right : Term)
  | lt (left right : Term)
  | le (left right : Term)
  | add (left right : Term)
  | sub (left right : Term)
  | mul (left right : Term)
  | pair (left right : Term)
  | first (body : Term)
  | second (body : Term)
  | inl (body : Term)
  | inr (body : Term)
  | isLeft (body : Term)
  | left (body : Term)
  | right (body : Term)
  | nil
  | cons (head tail : Term)
  | isNil (body : Term)
  | head (body : Term)
  | tail (body : Term)
  | length (body : Term)
  | maps (program input output : Term)

structure Context where
  before : List Value
  after : List Value
  params : List Value

@[simp] private def unaryBool (f : Bool → Bool) : Option Value → Option Value
  | some (.bool b) => some (.bool (f b))
  | _ => none

@[simp] private def binaryBool (f : Bool → Bool → Bool) : Option Value → Option Value → Option Value
  | some (.bool a), some (.bool b) => some (.bool (f a b))
  | _, _ => none

@[simp] private def binaryI64 (f : BitVec 64 → BitVec 64 → BitVec 64) :
    Option Value → Option Value → Option Value
  | some (.i64 a), some (.i64 b) => some (.i64 (f a b))
  | _, _ => none

@[simp] private def compareI64 (f : Int → Int → Bool) : Option Value → Option Value → Option Value
  | some (.i64 a), some (.i64 b) => some (.bool (f a.toInt b.toInt))
  | _, _ => none

/-- Logical evaluation is not guest code. Partial projections produce `none`;
    even negation of an undefined expression remains undefined. `maps` is a
    logical partial-correctness claim, not an executable guard or proof search. -/
noncomputable def evaluate : Term → Context → Option Value
  | .i64 n, _ => some (.i64 (BitVec.ofInt 64 n))
  | .bool b, _ => some (.bool b)
  | .unit, _ => some .unit
  | .input i, ctx => ctx.before[i]?
  | .output i, ctx => ctx.after[i]?
  | .param i, ctx => ctx.params[i]?
  | .definition _ body, ctx => evaluate body ctx
  | .not body, ctx => unaryBool Bool.not (evaluate body ctx)
  | .and a b, ctx => binaryBool Bool.and (evaluate a ctx) (evaluate b ctx)
  | .or a b, ctx => binaryBool Bool.or (evaluate a ctx) (evaluate b ctx)
  | .implies a b, ctx => binaryBool (fun x y => !x || y) (evaluate a ctx) (evaluate b ctx)
  | .eq a b, ctx =>
      match evaluate a ctx, evaluate b ctx with
      | some x, some y => some (.bool (@decide (x = y) (Classical.propDecidable _)))
      | _, _ => none
  | .lt a b, ctx => compareI64 (fun x y => decide (x < y)) (evaluate a ctx) (evaluate b ctx)
  | .le a b, ctx => compareI64 (fun x y => decide (x ≤ y)) (evaluate a ctx) (evaluate b ctx)
  | .add a b, ctx => binaryI64 (· + ·) (evaluate a ctx) (evaluate b ctx)
  | .sub a b, ctx => binaryI64 (· - ·) (evaluate a ctx) (evaluate b ctx)
  | .mul a b, ctx => binaryI64 (· * ·) (evaluate a ctx) (evaluate b ctx)
  | .pair a b, ctx =>
      match evaluate a ctx, evaluate b ctx with
      | some x, some y => some (.pair x y)
      | _, _ => none
  | .first body, ctx => match evaluate body ctx with
      | some (.pair a _) => some a
      | _ => none
  | .second body, ctx => match evaluate body ctx with
      | some (.pair _ b) => some b
      | _ => none
  | .inl body, ctx => (evaluate body ctx).map Value.inl
  | .inr body, ctx => (evaluate body ctx).map Value.inr
  | .isLeft body, ctx => match evaluate body ctx with
      | some (.inl _) => some (.bool true)
      | some (.inr _) => some (.bool false)
      | _ => none
  | .left body, ctx => match evaluate body ctx with
      | some (.inl a) => some a
      | _ => none
  | .right body, ctx => match evaluate body ctx with
      | some (.inr b) => some b
      | _ => none
  | .nil, _ => some (.list [])
  | .cons a b, ctx => match evaluate a ctx, evaluate b ctx with
      | some x, some (.list xs) => some (.list (x :: xs))
      | _, _ => none
  | .isNil body, ctx => match evaluate body ctx with
      | some (.list xs) => some (.bool xs.isEmpty)
      | _ => none
  | .head body, ctx => match evaluate body ctx with
      | some (.list xs) => xs.head?
      | _ => none
  | .tail body, ctx => match evaluate body ctx with
      | some (.list (_ :: xs)) => some (.list xs)
      | _ => none
  | .length body, ctx => match evaluate body ctx with
      | some (.list xs) => some (.i64 (BitVec.ofNat 64 xs.length))
      | _ => none
  | .maps p a b, ctx => match evaluate p ctx, evaluate a ctx, evaluate b ctx with
      | some (.program code), some x, some y =>
          some (.bool (@decide (∀ result, Exec code [x] result → result = [y])
            (Classical.propDecidable _)))
      | _, _, _ => none

/-- Undefined logical terms do not establish assertions. -/
def Holds (term : Term) (before after params : List Value) : Prop :=
  evaluate term ⟨before, after, params⟩ = some (.bool true)

@[simp] theorem holds_true (before after params : List Value) :
    Holds (.bool true) before after params := rfl

@[simp] theorem holds_false (before after params : List Value) :
    ¬ Holds (.bool false) before after params := by
  simp [Holds, evaluate]

@[simp] theorem holds_eq (a b : Term) (ctx : Context) (x y : Value)
    (ha : evaluate a ctx = some x) (hb : evaluate b ctx = some y) :
    evaluate (.eq a b) ctx = some (.bool true) ↔ x = y := by
  simp [evaluate, ha, hb]

/-- Initial and final observations are separate even if their display names coincide. -/
theorem snapshot_distinction (x y : BitVec 64) :
    evaluate (.input 0) ⟨[.i64 x], [.i64 y], []⟩ = some (.i64 x) ∧
    evaluate (.output 0) ⟨[.i64 x], [.i64 y], []⟩ = some (.i64 y) := by
  exact ⟨rfl, rfl⟩

/-- In particular, negation does not turn an absent list head into evidence. -/
theorem undefined_not_is_undefined (ctx : Context) :
    evaluate (.not (.head .nil)) ctx = none := rfl

end NobleContracts
