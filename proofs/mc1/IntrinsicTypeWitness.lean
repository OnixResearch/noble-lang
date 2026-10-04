import NobleContracts.Expression

namespace NobleContracts.Intrinsic

/-- The checked Type0 domain: no program, syntax, resource or universe code. -/
inductive PureTyCode where
  | unit | bool | i64 | text
  | pair : PureTyCode → PureTyCode → PureTyCode
  | sum : PureTyCode → PureTyCode → PureTyCode
  | list : PureTyCode → PureTyCode

/-- A structural interpretation into exactly the pure part of the reviewed MC1 types. -/
def PureTyCode.tyOf : PureTyCode → Ty
  | .unit => .unit
  | .bool => .bool
  | .i64 => .i64
  | .text => .text
  | .pair a b => .pair a.tyOf b.tyOf
  | .sum a b => .sum a.tyOf b.tyOf
  | .list a => .list a.tyOf

/-- Total semantic values, with Type0 codes rather than an arbitrary Lean Type binder. -/
def El : PureTyCode → Type
  | .unit => Unit
  | .bool => Bool
  | .i64 => BitVec 64
  | .text => String
  | .pair a b => El a × El b
  | .sum a b => Sum (El a) (El b)
  | .list a => List (El a)

/-- Construct the reviewed model value; no cast or program constructor is available. -/
def encode : (A : PureTyCode) → El A → Value
  | .unit, _ => .unit
  | .bool, x => .bool x
  | .i64, x => .i64 x
  | .text, x => .text x
  | .pair a b, x => .pair (encode a x.1) (encode b x.2)
  | .sum a _, .inl x => .inl (encode a x)
  | .sum _ b, .inr x => .inr (encode b x)
  | .list a, xs => .list (xs.map (encode a))

/-- Every decoded Type0 value is genuinely typed in the reviewed MC1 model. -/
theorem encode_hasType : (A : PureTyCode) → (x : El A) →
    HasType (encode A x) A.tyOf
  | .unit, _ => .unit
  | .bool, _ => .bool
  | .i64, _ => .i64
  | .text, _ => .text
  | .pair a b, x => .pair (encode_hasType a x.1) (encode_hasType b x.2)
  | .sum a _, .inl x => .inl (encode_hasType a x)
  | .sum _ b, .inr x => .inr (encode_hasType b x)
  | .list a, xs => .list (by
      intro v hv
      obtain ⟨x, _, rfl⟩ := List.mem_map.mp hv
      exact encode_hasType a x)

private theorem map_injective {α β : Type} (f : α → β) (hf : Function.Injective f) :
    Function.Injective (List.map f) := by
  intro xs ys h
  induction xs generalizing ys with
  | nil =>
    cases ys with
    | nil => rfl
    | cons _ _ => cases h
  | cons x xs ih =>
    cases ys with
    | nil => cases h
    | cons y ys =>
      have ⟨hx, hxs⟩ := List.cons.inj h
      cases hf hx
      cases ih hxs
      rfl

/-- Faithfulness of pure-value equality when observed in the reviewed model. -/
theorem encode_injective : (A : PureTyCode) → Function.Injective (encode A)
  | .unit => by intro x y _; cases x; cases y; rfl
  | .bool => by intro x y h; cases Value.bool.inj h; rfl
  | .i64 => by intro x y h; cases Value.i64.inj h; rfl
  | .text => by intro x y h; cases Value.text.inj h; rfl
  | .pair a b => by
      intro x y h
      have ⟨h₁, h₂⟩ := Value.pair.inj h
      exact Prod.ext (encode_injective a h₁) (encode_injective b h₂)
  | .sum a b => by
      intro x y h
      cases x with
      | inl x =>
        cases y with
        | inl y => exact congrArg Sum.inl (encode_injective a (Value.inl.inj h))
        | inr y => cases h
      | inr x =>
        cases y with
        | inl y => cases h
        | inr y => exact congrArg Sum.inr (encode_injective b (Value.inr.inj h))
  | .list a => by
      intro xs ys h
      have hmap := Value.list.inj h
      exact map_injective (encode a) (encode_injective a) hmap

/-- The CONTRACT-17 source proposition has this predicative, dependent target. -/
def polymorphicReflexivity : Prop :=
  ∀ (A : PureTyCode) (x : El A), x = x

/-- Translation of (intro A (intro x (refl x))); no proof-level axiom or tactic. -/
theorem polymorphicReflexivity_witness : polymorphicReflexivity :=
  fun _ x => Eq.refl x

end NobleContracts.Intrinsic
