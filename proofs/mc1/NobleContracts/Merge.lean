import NobleContracts.Rules

/-!
The `SMERGE/1` four-version relation over the pure MC1 reference semantics.

`MergeRel` relates an exact base program `O`, two variants `A` and `B` and a
supplied candidate `M` that share one complete ordered input stack type and one
complete ordered output stack type.  For every well-typed input vector the four
programs must return normally and the candidate's full output stack must be the
per-observable three-way merge of the variants against the base.  The relation
quantifies over every normal return, so it does not assume a deterministic
evaluator; for the admitted fragment the reference semantics is deterministic
per concrete subject, which the fixture theorems below check explicitly.

Value equality is structural: `I64` is a 64-bit bit pattern with wrapping
arithmetic, so a mathematical-integer reading is never used.

The module also states and proves the terminating-fragment theorem.  The
fragment proved here is a strict sub-fragment of the design's: leaf code is
straight-line first-order dataflow with no invocation at all, and outer code
adds statically embedded leaf blocks, quotation of first-order data and the
eliminators `dip`/`run`/`case`/`if`/`listCase`.  Every invoked program is thus
a leaf, so termination follows from the finite program, the finite embedding
tree and the total builtins - never from purity or an empty effect set.  The
design's named definitions and deeper quotations are not modeled here; that
narrowing is recorded in the change's design.

This file proves the source-model obligation.  It is not a claim about Wasm
execution, a compiler, the host or an installed merge tool.
-/

namespace NobleContracts

/-! ## Observables -/

/-- First-order data types: the only observable types of `SMERGE/1`. -/
inductive ObservableTy : Ty → Prop where
  | unit : ObservableTy .unit
  | bool : ObservableTy .bool
  | i64 : ObservableTy .i64
  | text : ObservableTy .text
  | pair {a b : Ty} : ObservableTy a → ObservableTy b → ObservableTy (.pair a b)
  | sum {a b : Ty} : ObservableTy a → ObservableTy b → ObservableTy (.sum a b)
  | list {a : Ty} : ObservableTy a → ObservableTy (.list a)

/-- First-order data values; no program, quotation or syntax value occurs. -/
inductive FirstOrderValue : Value → Prop where
  | unit : FirstOrderValue .unit
  | bool {b : Bool} : FirstOrderValue (.bool b)
  | i64 {n : BitVec 64} : FirstOrderValue (.i64 n)
  | text {s : String} : FirstOrderValue (.text s)
  | pair {a b : Value} : FirstOrderValue a → FirstOrderValue b →
      FirstOrderValue (.pair a b)
  | inl {v : Value} : FirstOrderValue v → FirstOrderValue (.inl v)
  | inr {v : Value} : FirstOrderValue v → FirstOrderValue (.inr v)
  | list {vs : List Value} : (∀ v ∈ vs, FirstOrderValue v) →
      FirstOrderValue (.list vs)

/-! ## The per-observable three-way merge -/

/-- `MergeVal o a b m` is the three-way merge of one observable: the base value
`o`, the variant values `a` and `b`, and the merged value `m`.  The four
constructors are exactly the four non-conflicting rows of the design's
`merge`; no row lets variant order decide. -/
inductive MergeVal : Value → Value → Value → Value → Prop where
  | unchanged {o : Value} : MergeVal o o o o
  | first {o a : Value} (a_changed : a ≠ o) : MergeVal o a o a
  | second {o b : Value} (b_changed : b ≠ o) : MergeVal o o b b
  | agree {o a b : Value} (a_changed : a ≠ o) (b_changed : b ≠ o) (same : a = b) :
      MergeVal o a b a

/-- No value satisfies the merge: both variants changed the observable to
different values. -/
def Conflict (o a b : Value) : Prop := ∀ m : Value, ¬ MergeVal o a b m

theorem mergeVal_functional {o a b m m' : Value}
    (left : MergeVal o a b m) (right : MergeVal o a b m') : m = m' := by
  cases left with
  | unchanged =>
      cases right with
      | unchanged => rfl
      | first h => exact absurd rfl h
      | second h => exact absurd rfl h
      | agree h _ _ => exact absurd rfl h
  | first h =>
      cases right with
      | unchanged => exact absurd rfl h
      | first _ => rfl
      | second _ => exact absurd rfl h
      | agree _ hb _ => exact absurd rfl hb
  | second h =>
      cases right with
      | unchanged => exact absurd rfl h
      | first _ => exact absurd rfl h
      | second _ => rfl
      | agree ha _ _ => exact absurd rfl ha
  | agree ha hb _ =>
      cases right with
      | unchanged => exact absurd rfl ha
      | first _ => exact absurd rfl hb
      | second _ => exact absurd rfl ha
      | agree _ _ _ => rfl

theorem mergeVal_total (o a b : Value) :
    (∃ m : Value, MergeVal o a b m) ∨ Conflict o a b := by
  by_cases ha : a = o
  · cases ha
    by_cases hb : b = o
    · cases hb
      exact Or.inl ⟨o, .unchanged⟩
    · exact Or.inl ⟨b, .second hb⟩
  · by_cases hb : b = o
    · cases hb
      exact Or.inl ⟨a, .first ha⟩
    · by_cases hab : a = b
      · cases hab
        exact Or.inl ⟨a, .agree ha hb rfl⟩
      · refine Or.inr ?_
        intro m hm
        cases hm with
        | unchanged => exact ha rfl
        | first _ => exact hb rfl
        | second _ => exact ha rfl
        | agree _ _ same => exact hab same

/-- The relation is symmetric in the two variants: variant order never decides. -/
theorem mergeVal_comm {o a b m : Value} (h : MergeVal o a b m) :
    MergeVal o b a m := by
  cases h with
  | unchanged => exact .unchanged
  | first h => exact .second h
  | second h => exact .first h
  | agree ha hb same =>
      cases same
      exact .agree hb ha rfl

/-- `MergeStack` lifts the observable merge to the complete ordered output
stack: the same length at every position, never a per-position skip. -/
inductive MergeStack : Stack → Stack → Stack → Stack → Prop where
  | nil : MergeStack [] [] [] []
  | cons {o a b m : Value} {os as_ bs ms : Stack} :
      MergeVal o a b m → MergeStack os as_ bs ms →
      MergeStack (o :: os) (a :: as_) (b :: bs) (m :: ms)

theorem mergeStack_functional {o a b m m' : Stack}
    (left : MergeStack o a b m) (right : MergeStack o a b m') : m = m' := by
  induction left generalizing m' with
  | nil => cases right; rfl
  | cons hv _ ih =>
      cases right with
      | cons hv' rest =>
          rw [mergeVal_functional hv hv']
          simpa using ih rest

theorem mergeStack_comm {o a b m : Stack} (h : MergeStack o a b m) :
    MergeStack o b a m := by
  induction h with
  | nil => exact .nil
  | cons hv _ ih => exact .cons (mergeVal_comm hv) ih

theorem mergeStack_lengths {o a b m : Stack} (h : MergeStack o a b m) :
    a.length = o.length ∧ b.length = o.length ∧ m.length = o.length := by
  induction h with
  | nil => exact ⟨rfl, rfl, rfl⟩
  | cons _ _ ih => exact ⟨by simp [ih.1], by simp [ih.2.1], by simp [ih.2.2]⟩

/-- Positional extraction: a merge stack is the merge at every position. -/
theorem mergeStack_position {o a b m : Stack} (h : MergeStack o a b m)
    {i : Nat} {ov av bv mv : Value} (ho : o[i]? = some ov) (ha : a[i]? = some av)
    (hb : b[i]? = some bv) (hm : m[i]? = some mv) : MergeVal ov av bv mv := by
  induction h generalizing i with
  | nil => cases ho
  | cons hv _ ih =>
      cases i with
      | zero =>
          simp only [List.getElem?_cons_zero, Option.some.injEq] at ho ha hb hm
          subst ho; subst ha; subst hb; subst hm
          exact hv
      | succ j =>
          simp only [List.getElem?_cons_succ] at ho ha hb hm
          exact ih ho ha hb hm

theorem mergeStack_append {o a b m o' a' b' m' : Stack}
    (left : MergeStack o a b m) (right : MergeStack o' a' b' m') :
    MergeStack (o ++ o') (a ++ a') (b ++ b') (m ++ m') := by
  induction left with
  | nil => exact right
  | cons hv _ ih => exact .cons hv ih

theorem mergeVal_no_conflict {o a b m : Value} (h : MergeVal o a b m) :
    ¬ Conflict o a b := fun conflict => conflict m h

/-! ## Wrapping `I64` observables -/

/-- Structural equality of `I64` values is bit equality of the 64-bit word. -/
theorem i64_value_eq {a b : BitVec 64} : (.i64 a : Value) = .i64 b ↔ a = b :=
  ⟨fun h => Value.i64.inj h, fun h => by rw [h]⟩

/-- The wrapping reading: the largest word plus one is zero, not 2^64. -/
theorem wrap_max_add_one : ((18446744073709551615 : BitVec 64) + 1) = 0 := by decide

theorem max_ne_zero : (18446744073709551615 : BitVec 64) ≠ 0 := by decide

theorem add_one_ne_self (y : BitVec 64) : y + 1 ≠ y := by
  intro h
  have h2 := congrArg BitVec.toNat h
  simp [BitVec.toNat_add] at h2
  omega

theorem add_two_ne_self (y : BitVec 64) : y + 2 ≠ y := by
  intro h
  have h2 := congrArg BitVec.toNat h
  simp [BitVec.toNat_add] at h2
  omega

theorem add_one_ne_add_two (y : BitVec 64) : y + 1 ≠ y + 2 := by
  intro h
  have h2 := congrArg BitVec.toNat h
  simp [BitVec.toNat_add] at h2
  omega

/-! ## Stack typing helpers -/

theorem stackTyped_append {s t : Stack} {i j : List Ty}
    (hs : StackTyped s i) (ht : StackTyped t j) : StackTyped (s ++ t) (i ++ j) := by
  induction hs with
  | nil => simpa using ht
  | cons hv _ ih => simpa using StackTyped.cons hv ih

/-- A typed stack splits at any point of its type list. -/
theorem stackTyped_split {s : Stack} {i j : List Ty} (h : StackTyped s (i ++ j)) :
    ∃ t u, s = t ++ u ∧ StackTyped t i ∧ StackTyped u j := by
  induction i generalizing s with
  | nil => exact ⟨[], s, rfl, .nil, by simpa using h⟩
  | cons ty tys ih =>
      obtain ⟨v, vs, rfl, hv, hvs⟩ := (stackTyped_cons_iff _ _ _).mp h
      obtain ⟨t, u, rfl, ht, hu⟩ := ih hvs
      exact ⟨v :: t, u, rfl, .cons hv ht, hu⟩

theorem stackTyped_singleton_shape {s : Stack} {ty : Ty} (h : StackTyped s [ty]) :
    ∃ v, s = [v] ∧ HasType v ty := by
  obtain ⟨v, vs, rfl, hv, hvs⟩ := (stackTyped_cons_iff _ _ _).mp h
  obtain rfl := (stackTyped_nil_iff _).mp hvs
  exact ⟨v, rfl, hv⟩

theorem stackTyped_pair_shape {s : Stack} {a b : Ty} (h : StackTyped s [a, b]) :
    ∃ x y, s = [x, y] ∧ HasType x a ∧ HasType y b := by
  obtain ⟨x, vs, rfl, hx, hvs⟩ := (stackTyped_cons_iff _ _ _).mp h
  obtain ⟨y, rfl, hy⟩ := stackTyped_singleton_shape hvs
  exact ⟨x, y, rfl, hx, hy⟩

theorem stackTyped_append_singleton {s : Stack} {i : List Ty} {a : Ty}
    (h : StackTyped s (i ++ [a])) :
    ∃ t x, s = t ++ [x] ∧ StackTyped t i ∧ HasType x a := by
  obtain ⟨t, u, rfl, ht, hu⟩ := stackTyped_split h
  obtain ⟨x, rfl, hx⟩ := stackTyped_singleton_shape hu
  exact ⟨t, x, rfl, ht, hx⟩

theorem stackTyped_append_pair {s : Stack} {i : List Ty} {a b : Ty}
    (h : StackTyped s (i ++ [a, b])) :
    ∃ t x y, s = t ++ [x, y] ∧ StackTyped t i ∧ HasType x a ∧ HasType y b := by
  obtain ⟨t, u, rfl, ht, hu⟩ := stackTyped_split h
  obtain ⟨x, y, rfl, hx, hy⟩ := stackTyped_pair_shape hu
  exact ⟨t, x, y, rfl, ht, hx, hy⟩

theorem stackTyped_append_three {s : Stack} {i : List Ty} {a b c : Ty}
    (h : StackTyped s (i ++ [a, b, c])) :
    ∃ t x y z, s = t ++ [x, y, z] ∧ StackTyped t i ∧ HasType x a ∧
      HasType y b ∧ HasType z c := by
  obtain ⟨t, u, rfl, ht, hu⟩ := stackTyped_split h
  obtain ⟨x, v, rfl, hx, hv⟩ := (stackTyped_cons_iff _ _ _).mp hu
  obtain ⟨y, z, rfl, hy, hz⟩ := stackTyped_pair_shape hv
  exact ⟨t, x, y, z, rfl, ht, hx, hy, hz⟩

theorem hasType_program_shape {v : Value} {i o : List Ty}
    (h : HasType v (.program i o)) :
    ∃ p, v = .program p ∧ CodeTyped p i o := by
  cases h with
  | program hcode => exact ⟨_, rfl, hcode⟩

theorem hasType_pair_shape {v : Value} {a b : Ty} (h : HasType v (.pair a b)) :
    ∃ x y, v = .pair x y ∧ HasType x a ∧ HasType y b := by
  cases h with
  | pair hx hy => exact ⟨_, _, rfl, hx, hy⟩

theorem hasType_sum_shape {v : Value} {a b : Ty} (h : HasType v (.sum a b)) :
    (∃ x, v = .inl x ∧ HasType x a) ∨ (∃ y, v = .inr y ∧ HasType y b) := by
  cases h with
  | inl hx => exact .inl ⟨_, rfl, hx⟩
  | inr hy => exact .inr ⟨_, rfl, hy⟩

theorem hasType_list_shape {v : Value} {a : Ty} (h : HasType v (.list a)) :
    ∃ vs, v = .list vs ∧ ∀ x ∈ vs, HasType x a := by
  cases h with
  | list hv => exact ⟨_, rfl, hv⟩

/-! ## The terminating fragment -/

mutual
  /-- Straight-line dataflow.  No program value is invoked, reflected or
  composed, so one op consumes one constructor.  A literal may quote a leaf
  value; a literal program is admitted only when its body is a leaf. -/
  inductive CoreOp : Op → Prop where
    | lit {v : Value} : LeafValue v → CoreOp (.lit v)
    | dup : CoreOp (.word 0)
    | drop : CoreOp (.word 1)
    | swap : CoreOp (.word 2)
    | add : CoreOp (.word 4)
    | sub : CoreOp (.word 5)
    | mul : CoreOp (.word 6)
    | equals : CoreOp (.word 7)
    | unit : CoreOp (.word 12)
    | pair : CoreOp (.word 13)
    | unpair : CoreOp (.word 14)
    | inl : CoreOp (.word 15)
    | inr : CoreOp (.word 16)
    | nil : CoreOp (.word 19)
    | cons : CoreOp (.word 20)

  inductive LeafCode : List Op → Prop where
    | nil : LeafCode []
    | cons {op : Op} {ops : List Op} : CoreOp op → LeafCode ops → LeafCode (op :: ops)

  /-- A value every program inside which is a leaf: first-order data possibly
  containing leaf quotations and leaf blocks. -/
  inductive LeafValue : Value → Prop where
    | unit : LeafValue .unit
    | bool {b : Bool} : LeafValue (.bool b)
    | i64 {n : BitVec 64} : LeafValue (.i64 n)
    | text {s : String} : LeafValue (.text s)
    | pair {a b : Value} : LeafValue a → LeafValue b → LeafValue (.pair a b)
    | inl {v : Value} : LeafValue v → LeafValue (.inl v)
    | inr {v : Value} : LeafValue v → LeafValue (.inr v)
    | list {vs : List Value} : (∀ v ∈ vs, LeafValue v) → LeafValue (.list vs)
    | program {p : List Op} : LeafCode p → LeafValue (.program p)
end

/-- Every stack value is a leaf value. -/
def AllLeaf (s : Stack) : Prop := ∀ v ∈ s, LeafValue v

theorem firstOrder_leafValue {v : Value} (h : FirstOrderValue v) : LeafValue v := by
  cases h with
  | unit => exact .unit
  | bool => exact .bool
  | i64 => exact .i64
  | text => exact .text
  | pair ha hb => exact .pair (firstOrder_leafValue ha) (firstOrder_leafValue hb)
  | inl hv => exact .inl (firstOrder_leafValue hv)
  | inr hv => exact .inr (firstOrder_leafValue hv)
  | list hvs => exact .list (fun v hv => firstOrder_leafValue (hvs v hv))

theorem leafValue_list_shape {vs : List Value} (h : LeafValue (.list vs)) :
    ∀ v ∈ vs, LeafValue v := by
  cases h with
  | list hv => exact hv

theorem leafValue_program_shape {p : List Op} (h : LeafValue (.program p)) :
    LeafCode p := by
  cases h with
  | program hcode => exact hcode

theorem allLeaf_nil : AllLeaf [] := by intro v hv; cases hv

theorem allLeaf_cons {v : Value} {s : Stack} (hv : LeafValue v) (hs : AllLeaf s) :
    AllLeaf (v :: s) := by
  intro w hw
  cases hw with
  | head => exact hv
  | tail _ h => exact hs w h

theorem allLeaf_append_iff {s t : Stack} : AllLeaf (s ++ t) ↔ AllLeaf s ∧ AllLeaf t := by
  constructor
  · intro h
    exact ⟨fun v hv => h v (List.mem_append_left _ hv),
      fun v hv => h v (List.mem_append_right _ hv)⟩
  · rintro ⟨hs, ht⟩ v hv
    cases List.mem_append.mp hv with
    | inl h => exact hs v h
    | inr h => exact ht v h

theorem allLeaf_append {s t : Stack} (hs : AllLeaf s) (ht : AllLeaf t) :
    AllLeaf (s ++ t) := allLeaf_append_iff.mpr ⟨hs, ht⟩

theorem allLeaf_singleton {v : Value} (hv : LeafValue v) : AllLeaf [v] :=
  allLeaf_cons hv allLeaf_nil

theorem allLeaf_singleton_inv {v : Value} (h : AllLeaf [v]) : LeafValue v :=
  h v (by simp)

theorem allLeaf_pair_inv {x y : Value} {rest : Stack}
    (h : AllLeaf (x :: y :: rest)) : LeafValue x ∧ LeafValue y ∧ AllLeaf rest := by
  refine ⟨h x (by simp), h y (by simp), ?_⟩
  intro v hv
  exact h v (by simp [hv])

theorem leafValue_pair_inv {x y : Value} (h : LeafValue (.pair x y)) :
    LeafValue x ∧ LeafValue y := by
  cases h with
  | pair hx hy => exact ⟨hx, hy⟩

theorem leafValue_inl_inv {x : Value} (h : LeafValue (.inl x)) : LeafValue x := by
  cases h with
  | inl hv => exact hv

theorem leafValue_inr_inv {x : Value} (h : LeafValue (.inr x)) : LeafValue x := by
  cases h with
  | inr hv => exact hv

theorem core_step {op : Op} {i o : List Ty} {s : Stack} (hcore : CoreOp op)
    (htyped : OpTyped op i o) (hstack : StackTyped s i) (hall : AllLeaf s) :
    ∃ t, Exec [op] s t ∧ StackTyped t o ∧ AllLeaf t := by
  cases hcore with
  | lit hval =>
      cases htyped with
      | lit hv =>
          refine ⟨s ++ [_], exec_lit_iff.mpr rfl, stackTyped_append hstack (.cons hv .nil),
            allLeaf_append hall (allLeaf_singleton hval)⟩
  | dup =>
      cases htyped with
      | dup =>
          obtain ⟨t, x, rfl, ht, hx⟩ := stackTyped_append_singleton hstack
          have hallx := allLeaf_append_iff.mp hall
          have hxl : LeafValue x := allLeaf_singleton_inv hallx.2
          refine ⟨t ++ [x, x], exec_dup_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using
              stackTyped_append (stackTyped_append ht (.cons hx .nil)) (.cons hx .nil)
          · exact allLeaf_append hallx.1 (allLeaf_cons hxl (allLeaf_singleton hxl))
  | drop =>
      cases htyped with
      | drop =>
          obtain ⟨t, x, rfl, ht, _⟩ := stackTyped_append_singleton hstack
          exact ⟨t, exec_drop_iff.mpr rfl, ht, (allLeaf_append_iff.mp hall).1⟩
  | swap =>
      cases htyped with
      | swap =>
          obtain ⟨t, x, y, rfl, ht, hx, hy⟩ := stackTyped_append_pair hstack
          have hallx := allLeaf_append_iff.mp hall
          obtain ⟨hxl, hyl, _⟩ := allLeaf_pair_inv hallx.2
          refine ⟨t ++ [y, x], exec_swap_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using
              stackTyped_append (stackTyped_append ht (.cons hy .nil)) (.cons hx .nil)
          · exact allLeaf_append hallx.1
              (allLeaf_append (allLeaf_singleton hyl) (allLeaf_singleton hxl))
  | add =>
      cases htyped with
      | add =>
          obtain ⟨t, x, y, rfl, ht, hx, hy⟩ := stackTyped_append_pair hstack
          obtain ⟨xa, rfl⟩ := (hasType_i64_iff _).mp hx
          obtain ⟨ya, rfl⟩ := (hasType_i64_iff _).mp hy
          refine ⟨t ++ [.i64 (xa + ya)], exec_add_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using stackTyped_append ht (.cons .i64 .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1 (allLeaf_singleton .i64)
  | sub =>
      cases htyped with
      | sub =>
          obtain ⟨t, x, y, rfl, ht, hx, hy⟩ := stackTyped_append_pair hstack
          obtain ⟨xa, rfl⟩ := (hasType_i64_iff _).mp hx
          obtain ⟨ya, rfl⟩ := (hasType_i64_iff _).mp hy
          refine ⟨t ++ [.i64 (xa - ya)], exec_sub_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using stackTyped_append ht (.cons .i64 .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1 (allLeaf_singleton .i64)
  | mul =>
      cases htyped with
      | mul =>
          obtain ⟨t, x, y, rfl, ht, hx, hy⟩ := stackTyped_append_pair hstack
          obtain ⟨xa, rfl⟩ := (hasType_i64_iff _).mp hx
          obtain ⟨ya, rfl⟩ := (hasType_i64_iff _).mp hy
          refine ⟨t ++ [.i64 (xa * ya)], exec_mul_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using stackTyped_append ht (.cons .i64 .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1 (allLeaf_singleton .i64)
  | equals =>
      cases htyped with
      | equals =>
          obtain ⟨t, x, y, rfl, ht, hx, hy⟩ := stackTyped_append_pair hstack
          obtain ⟨xa, rfl⟩ := (hasType_i64_iff _).mp hx
          obtain ⟨ya, rfl⟩ := (hasType_i64_iff _).mp hy
          refine ⟨t ++ [.bool (xa == ya)], exec_equals_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using stackTyped_append ht (.cons .bool .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1 (allLeaf_singleton .bool)
  | unit =>
      cases htyped with
      | unit =>
          refine ⟨s ++ [.unit], exec_unit_iff.mpr rfl, ?_, ?_⟩
          · simpa using stackTyped_append hstack (.cons .unit .nil)
          · exact allLeaf_append hall (allLeaf_singleton .unit)
  | pair =>
      cases htyped with
      | pair =>
          obtain ⟨t, x, y, rfl, ht, hx, hy⟩ := stackTyped_append_pair hstack
          have hallx := allLeaf_append_iff.mp hall
          obtain ⟨hxl, hyl, _⟩ := allLeaf_pair_inv hallx.2
          refine ⟨t ++ [.pair x y], exec_pair_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using stackTyped_append ht (.cons (.pair hx hy) .nil)
          · exact allLeaf_append hallx.1 (allLeaf_singleton (.pair hxl hyl))
  | unpair =>
      cases htyped with
      | unpair =>
          obtain ⟨t, x, rfl, ht, hx⟩ := stackTyped_append_singleton hstack
          obtain ⟨a, b, rfl, ha, hb⟩ := hasType_pair_shape hx
          have hallx := (allLeaf_append_iff.mp hall).2
          obtain ⟨hla, hlb⟩ := leafValue_pair_inv (allLeaf_singleton_inv hallx)
          refine ⟨t ++ [a, b], exec_unpair_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using
              stackTyped_append (stackTyped_append ht (.cons ha .nil)) (.cons hb .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1
              (allLeaf_append (allLeaf_singleton hla) (allLeaf_singleton hlb))
  | inl =>
      cases htyped with
      | inl =>
          obtain ⟨t, x, rfl, ht, hx⟩ := stackTyped_append_singleton hstack
          have hallx := (allLeaf_append_iff.mp hall).2
          have hxl : LeafValue x := allLeaf_singleton_inv hallx
          refine ⟨t ++ [.inl x], exec_inl_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using stackTyped_append ht (.cons (.inl hx) .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1
              (allLeaf_singleton (.inl hxl))
  | inr =>
      cases htyped with
      | inr =>
          obtain ⟨t, x, rfl, ht, hx⟩ := stackTyped_append_singleton hstack
          have hallx := (allLeaf_append_iff.mp hall).2
          have hxl : LeafValue x := allLeaf_singleton_inv hallx
          refine ⟨t ++ [.inr x], exec_inr_iff.mpr rfl, ?_, ?_⟩
          · simpa [List.append_assoc] using stackTyped_append ht (.cons (.inr hx) .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1
              (allLeaf_singleton (.inr hxl))
  | nil =>
      cases htyped with
      | nil =>
          refine ⟨s ++ [.list []], exec_nil_word_iff.mpr rfl, ?_, ?_⟩
          · refine stackTyped_append hstack (.cons ?_ .nil)
            exact .list (by intro v hv; cases hv)
          · exact allLeaf_append hall (allLeaf_singleton (.list (by intro v hv; cases hv)))
  | cons =>
      cases htyped with
      | cons =>
          obtain ⟨t, x, y, rfl, ht, hx, hy⟩ := stackTyped_append_pair hstack
          obtain ⟨vs, rfl, hvs⟩ := hasType_list_shape hy
          have hallx := allLeaf_append_iff.mp hall
          obtain ⟨hxl, hlist, _⟩ := allLeaf_pair_inv hallx.2
          have htail : ∀ z ∈ vs, LeafValue z := leafValue_list_shape hlist
          refine ⟨t ++ [.list (x :: vs)], exec_cons_iff.mpr rfl, ?_, ?_⟩
          · refine stackTyped_append ht (.cons (.list ?_) .nil)
            intro z hz
            cases hz with
            | head => exact hx
            | tail _ h => exact hvs z h
          · exact allLeaf_append hallx.1 (allLeaf_singleton (.list (by
              intro z hz
              cases hz with
              | head => exact hxl
              | tail _ h => exact htail z h)))

theorem leaf_exec {c : List Op} {i o : List Ty} {s : Stack} (htyped : CodeTyped c i o)
    (hleaf : LeafCode c) (hstack : StackTyped s i) (hall : AllLeaf s) :
    ∃ t, Exec c s t ∧ StackTyped t o ∧ AllLeaf t := by
  revert i o s
  induction c with
  | nil =>
      intro i o s htyped hstack hall
      cases htyped with
      | nil => exact ⟨s, exec_nil_iff.mpr rfl, hstack, hall⟩
  | cons op ops ih =>
      intro i o s htyped hstack hall
      cases htyped with
      | cons hop hrest =>
          cases hleaf with
          | cons hcore htail =>
              obtain ⟨m, hm, hmt, hallm⟩ := core_step hcore hop hstack hall
              obtain ⟨t, ht, htt, hallt⟩ := ih htail hrest hmt hallm
              exact ⟨t, exec_append_iff.mpr ⟨m, hm, ht⟩, htt, hallt⟩

theorem allLeaf_three_inv {a b c : Value} {rest : Stack}
    (h : AllLeaf (a :: b :: c :: rest)) :
    LeafValue a ∧ LeafValue b ∧ LeafValue c ∧ AllLeaf rest := by
  refine ⟨h a (by simp), h b (by simp), h c (by simp), ?_⟩
  intro v hv
  exact h v (by simp [hv])

/-! Outer code: leaf code, statically embedded leaf blocks, quotation and the
eliminators whose programs are stack values.  Quotation has no static payload:
the quoted value is a stack value, so `AllLeaf` requires it to be a leaf and
the quoted body is admitted only then.  Every reachable program value is thus a
leaf, which is the fragment's termination premise. -/
inductive OuterOp : Op → Prop where
  | core {op : Op} : CoreOp op → OuterOp op
  | block {p : List Op} : LeafCode p → OuterOp (.block p)
  | quote : OuterOp (.word 8)
  | dip : OuterOp (.word 3)
  | run : OuterOp (.word 10)
  | caseWord : OuterOp (.word 17)
  | ifWord : OuterOp (.word 18)
  | listCase : OuterOp (.word 21)

inductive OuterCode : List Op → Prop where
  | nil : OuterCode []
  | cons {op : Op} {ops : List Op} : OuterOp op → OuterCode ops → OuterCode (op :: ops)

theorem outer_step {op : Op} {i o : List Ty} {s : Stack} (houter : OuterOp op)
    (htyped : OpTyped op i o) (hstack : StackTyped s i) (hall : AllLeaf s) :
    ∃ t, Exec [op] s t ∧ StackTyped t o ∧ AllLeaf t := by
  cases houter with
  | core hcore => exact core_step hcore htyped hstack hall
  | block hleaf =>
      cases htyped with
      | block hcode =>
          refine ⟨s ++ [.program _], exec_block_iff.mpr rfl, ?_, ?_⟩
          · exact stackTyped_append hstack (.cons (.program hcode) .nil)
          · exact allLeaf_append hall (allLeaf_singleton (.program hleaf))
  | quote =>
      cases htyped with
      | quote =>
          obtain ⟨t, v, rfl, ht, hv⟩ := stackTyped_append_singleton hstack
          refine ⟨t ++ [.program [.lit v]], exec_quote_iff.mpr rfl, ?_, ?_⟩
          · exact stackTyped_append ht (.cons (.program (.cons (.lit hv) .nil)) .nil)
          · exact allLeaf_append (allLeaf_append_iff.mp hall).1
              (allLeaf_singleton (.program (.cons (.lit (allLeaf_singleton_inv (allLeaf_append_iff.mp hall).2)) .nil)))
  | dip =>
      cases htyped with
      | dip =>
          obtain ⟨t, v, prog, rfl, ht, hv, hprog⟩ := stackTyped_append_pair hstack
          obtain ⟨p, rfl, hcode⟩ := hasType_program_shape hprog
          have hallx := allLeaf_append_iff.mp hall
          obtain ⟨hvl, hprogl, _⟩ := allLeaf_pair_inv hallx.2
          obtain ⟨u, hu, htu, hallu⟩ :=
            leaf_exec hcode (leafValue_program_shape hprogl) ht hallx.1
          refine ⟨u ++ [v], exec_dip_iff.mpr ⟨u, hu, rfl⟩, ?_, ?_⟩
          · exact stackTyped_append htu (.cons hv .nil)
          · exact allLeaf_append hallu (allLeaf_singleton hvl)
  | run =>
      cases htyped with
      | run =>
          obtain ⟨t, prog, rfl, ht, hprog⟩ := stackTyped_append_singleton hstack
          obtain ⟨p, rfl, hcode⟩ := hasType_program_shape hprog
          have hallx := allLeaf_append_iff.mp hall
          obtain ⟨u, hu, htu, hallu⟩ :=
            leaf_exec hcode (leafValue_program_shape (allLeaf_singleton_inv hallx.2)) ht hallx.1
          exact ⟨u, exec_run_iff.mpr hu, htu, hallu⟩
  | caseWord =>
      cases htyped with
      | caseWord =>
          obtain ⟨t, value, p, q, rfl, ht, hv, hp, hq⟩ := stackTyped_append_three hstack
          obtain ⟨hvl, hpl, hql, _⟩ := allLeaf_three_inv (allLeaf_append_iff.mp hall).2
          obtain ⟨pc, rfl, hcodep⟩ := hasType_program_shape hp
          obtain ⟨qc, rfl, hcodeq⟩ := hasType_program_shape hq
          rcases hasType_sum_shape hv with ⟨x, rfl, hx⟩ | ⟨y, rfl, hy⟩
          · obtain ⟨u, hu, htu, hallu⟩ := leaf_exec hcodep
              (leafValue_program_shape hpl) (stackTyped_append ht (.cons hx .nil))
              (allLeaf_append (allLeaf_append_iff.mp hall).1 (allLeaf_singleton (leafValue_inl_inv hvl)))
            exact ⟨u, exec_case_left_iff.mpr hu, htu, hallu⟩
          · obtain ⟨u, hu, htu, hallu⟩ := leaf_exec hcodeq
              (leafValue_program_shape hql) (stackTyped_append ht (.cons hy .nil))
              (allLeaf_append (allLeaf_append_iff.mp hall).1 (allLeaf_singleton (leafValue_inr_inv hvl)))
            exact ⟨u, exec_case_right_iff.mpr hu, htu, hallu⟩
  | ifWord =>
      cases htyped with
      | ifWord =>
          obtain ⟨t, value, p, q, rfl, ht, hv, hp, hq⟩ := stackTyped_append_three hstack
          have hallx := allLeaf_append_iff.mp hall
          obtain ⟨hvl, hpl, hql, _⟩ := allLeaf_three_inv hallx.2
          obtain ⟨pc, rfl, hcodep⟩ := hasType_program_shape hp
          obtain ⟨qc, rfl, hcodeq⟩ := hasType_program_shape hq
          obtain ⟨b, rfl⟩ := (hasType_bool_iff _).mp hv
          cases b with
          | true =>
              obtain ⟨u, hu, htu, hallu⟩ := leaf_exec hcodep
                (leafValue_program_shape hpl) ht hallx.1
              exact ⟨u, exec_if_true_iff.mpr hu, htu, hallu⟩
          | false =>
              obtain ⟨u, hu, htu, hallu⟩ := leaf_exec hcodeq
                (leafValue_program_shape hql) ht hallx.1
              exact ⟨u, exec_if_false_iff.mpr hu, htu, hallu⟩
  | listCase =>
      cases htyped with
      | listCase =>
          obtain ⟨t, value, p, q, rfl, ht, hv, hp, hq⟩ := stackTyped_append_three hstack
          have hallx := allLeaf_append_iff.mp hall
          obtain ⟨hvl, hpl, hql, _⟩ := allLeaf_three_inv hallx.2
          obtain ⟨pc, rfl, hcodep⟩ := hasType_program_shape hp
          obtain ⟨qc, rfl, hcodeq⟩ := hasType_program_shape hq
          obtain ⟨vs, rfl, hvs⟩ := hasType_list_shape hv
          have hallvs : ∀ z ∈ vs, LeafValue z := leafValue_list_shape hvl
          cases vs with
          | nil =>
              obtain ⟨u, hu, htu, hallu⟩ := leaf_exec hcodep
                (leafValue_program_shape hpl) ht hallx.1
              exact ⟨u, exec_list_nil_iff.mpr hu, htu, hallu⟩
          | cons v vs =>
              have htyv : HasType v _ := hvs v (by simp)
              have htylist : HasType (.list vs) _ :=
                .list (vs := vs) (fun z hz => hvs z (by simp [hz]))
              have hvleaf : LeafValue v := hallvs v (by simp)
              have hlistleaf : LeafValue (.list vs) :=
                .list (vs := vs) (fun z hz => hallvs z (by simp [hz]))
              obtain ⟨u, hu, htu, hallu⟩ := leaf_exec hcodeq
                (leafValue_program_shape hql)
                (stackTyped_append ht (.cons htyv (.cons htylist .nil)))
                (allLeaf_append hallx.1 (allLeaf_cons hvleaf (allLeaf_singleton hlistleaf)))
              exact ⟨u, exec_list_cons_iff.mpr hu, htu, hallu⟩

theorem outer_exec {c : List Op} {i o : List Ty} {s : Stack} (htyped : CodeTyped c i o)
    (hcode : OuterCode c) (hstack : StackTyped s i) (hall : AllLeaf s) :
    ∃ t, Exec c s t ∧ StackTyped t o ∧ AllLeaf t := by
  revert i o s
  induction c with
  | nil =>
      intro i o s htyped hstack hall
      cases htyped with
      | nil => exact ⟨s, exec_nil_iff.mpr rfl, hstack, hall⟩
  | cons op ops ih =>
      intro i o s htyped hstack hall
      cases htyped with
      | cons hop hrest =>
          cases hcode with
          | cons houter htail =>
              obtain ⟨m, hm, hmt, hallm⟩ := outer_step houter hop hstack hall
              obtain ⟨t, ht, htt, hallt⟩ := ih htail hrest hmt hallm
              exact ⟨t, exec_append_iff.mpr ⟨m, hm, ht⟩, htt, hallt⟩

/-- A first-order typed stack never holds a program value, so the observable
fragment's inputs are leaf stacks. -/
theorem firstOrder_of_observable {v : Value} {ty : Ty} (hty : ObservableTy ty)
    (hv : HasType v ty) : FirstOrderValue v := by
  induction hty generalizing v with
  | unit => cases hv with | unit => exact .unit
  | bool => cases hv with | bool => exact .bool
  | i64 => cases hv with | i64 => exact .i64
  | text => cases hv with | text => exact .text
  | pair ha hb iha ihb =>
      obtain ⟨x, y, rfl, hx, hy⟩ := hasType_pair_shape hv
      exact .pair (iha hx) (ihb hy)
  | sum ha hb iha ihb =>
      rcases hasType_sum_shape hv with ⟨x, rfl, hx⟩ | ⟨y, rfl, hy⟩
      · exact .inl (iha hx)
      · exact .inr (ihb hy)
  | list ha iha =>
      obtain ⟨vs, rfl, hvs⟩ := hasType_list_shape hv
      exact .list (fun x hx => iha (hvs x hx))

theorem observable_allLeaf {s : Stack} {i : List Ty} (hs : StackTyped s i)
    (hobs : ∀ t ∈ i, ObservableTy t) : AllLeaf s := by
  induction hs with
  | nil => exact allLeaf_nil
  | cons hv _ ih =>
      exact allLeaf_cons
        (firstOrder_leafValue (firstOrder_of_observable (hobs _ (by simp)) hv))
        (ih (fun t ht => hobs t (by simp [ht])))

/-! ## The four-version relation -/

/-- A normal return of `p` on the public input stack `xs` at the declared
output stack type. -/
def Observes (outputTys : List Ty) (p : List Op) (xs out : Stack) : Prop :=
  Exec p xs out ∧ StackTyped out outputTys

/-- `SMERGE/1`: for every well-typed input vector the four subjects return
normally, the merge is defined at every output position, and every normal
return of the candidate is the merge of the variants' normal returns.  The
relation quantifies over all normal returns, so it never chooses one output of
a nondeterministic evaluator; for the fragment the reference semantics is
deterministic per concrete subject, which the fixture theorems check. -/
def MergeRel (inputTys outputTys : List Ty) (O A B M : List Op) : Prop :=
  ∀ xs, StackTyped xs inputTys →
    (∃ mo, Observes outputTys O xs mo) ∧
    (∃ ma, Observes outputTys A xs ma) ∧
    (∃ mb, Observes outputTys B xs mb) ∧
    (∃ mm, Observes outputTys M xs mm) ∧
    (∀ mo ma mb mm, Observes outputTys O xs mo → Observes outputTys A xs ma →
      Observes outputTys B xs mb → Observes outputTys M xs mm →
      MergeStack mo ma mb mm)

theorem mergeRel_returns {i o : List Ty} {O A B M : List Op} (h : MergeRel i o O A B M)
    {xs : Stack} (hxs : StackTyped xs i) :
    (∃ mo, Observes o O xs mo) ∧ (∃ ma, Observes o A xs ma) ∧
      (∃ mb, Observes o B xs mb) ∧ (∃ mm, Observes o M xs mm) :=
  ⟨(h xs hxs).1, (h xs hxs).2.1, (h xs hxs).2.2.1, (h xs hxs).2.2.2.1⟩

theorem mergeRel_values {i o : List Ty} {O A B M : List Op} (h : MergeRel i o O A B M)
    {xs : Stack} (hxs : StackTyped xs i) :
    ∀ mo ma mb mm, Observes o O xs mo → Observes o A xs ma → Observes o B xs mb →
      Observes o M xs mm → MergeStack mo ma mb mm :=
  (h xs hxs).2.2.2.2

/-- The relation is symmetric in the two variants: no variant order decides. -/
theorem mergeRel_symm {i o : List Ty} {O A B M : List Op} (h : MergeRel i o O A B M) :
    MergeRel i o O B A M := by
  intro xs hxs
  obtain ⟨hO, hA, hB, hM, hvalues⟩ := h xs hxs
  exact ⟨hO, hB, hA, hM, fun mo mb ma mm hO' hB' hA' hM' =>
    mergeStack_comm (hvalues mo ma mb mm hO' hA' hB' hM')⟩

/-- A fragment subject's four normal returns are established by the
terminating-fragment theorem, so the relation is never vacuous for an admitted
subject. -/
theorem mergeRel_terminating {i o : List Ty} {O A B M : List Op}
    (hinput : ∀ t ∈ i, ObservableTy t)
    (hO : CodeTyped O i o) (hFO : OuterCode O)
    (hA : CodeTyped A i o) (hFA : OuterCode A)
    (hB : CodeTyped B i o) (hFB : OuterCode B)
    (hM : CodeTyped M i o) (hFM : OuterCode M) {xs : Stack}
    (hxs : StackTyped xs i) :
    (∃ mo, Observes o O xs mo) ∧ (∃ ma, Observes o A xs ma) ∧
      (∃ mb, Observes o B xs mb) ∧ (∃ mm, Observes o M xs mm) := by
  have hall := observable_allLeaf hxs hinput
  obtain ⟨mo, hmo, htmo, _⟩ := outer_exec hO hFO hxs hall
  obtain ⟨ma, hma, htma, _⟩ := outer_exec hA hFA hxs hall
  obtain ⟨mb, hmb, htmb, _⟩ := outer_exec hB hFB hxs hall
  obtain ⟨mm, hmm, htmm, _⟩ := outer_exec hM hFM hxs hall
  exact ⟨⟨mo, hmo, htmo⟩, ⟨ma, hma, htma⟩, ⟨mb, hmb, htmb⟩, ⟨mm, hmm, htmm⟩⟩

/-- A conflict at one validated input and position refutes any candidate: the
relation must hold for the observations, and no value satisfies a conflicting
position. -/
theorem mergeRel_conflict_absurd {i o : List Ty} {O A B M : List Op}
    (h : MergeRel i o O A B M) {xs : Stack} (hxs : StackTyped xs i)
    {mo ma mb mm : Stack} (hmO : Observes o O xs mo) (hmA : Observes o A xs ma)
    (hmB : Observes o B xs mb) (hmM : Observes o M xs mm)
    {n : Nat} {ov av bv mv : Value} (ho : mo[n]? = some ov) (ha : ma[n]? = some av)
    (hb : mb[n]? = some bv) (hm : mm[n]? = some mv) (conflict : Conflict ov av bv) :
    False :=
  conflict mv (mergeStack_position (mergeRel_values h hxs mo ma mb mm hmO hmA hmB hmM)
    ho ha hb hm)

theorem codeTyped_append {a b : List Op} {i m o : List Ty} (ha : CodeTyped a i m)
    (hb : CodeTyped b m o) : CodeTyped (a ++ b) i o := by
  revert i m o
  induction a with
  | nil =>
      intro i m o ha hb
      cases ha with
      | nil => exact hb
  | cons op ops ih =>
      intro i m o ha hb
      cases ha with
      | cons h1 h2 => exact .cons h1 (ih h2 hb)

/-! ## Fixture subjects -/

def incrementOne : List Op := [.lit (.i64 1), .word 4]
def incrementTwo : List Op := [.lit (.i64 2), .word 4]
def incrementBottom : List Op := [.word 2, .lit (.i64 1), .word 4, .word 2]
def incrementBoth : List Op := incrementBottom ++ incrementOne
def identityCode : List Op := []

theorem typed_identity : CodeTyped identityCode [.i64, .i64] [.i64, .i64] := .nil

theorem typed_incrementOne : CodeTyped incrementOne [.i64, .i64] [.i64, .i64] :=
  CodeTyped.cons (m := [.i64, .i64, .i64])
    (OpTyped.lit (v := .i64 1) (ty := .i64) (s := [.i64, .i64]) .i64)
    (CodeTyped.cons (m := [.i64, .i64]) (OpTyped.add (s := [.i64])) CodeTyped.nil)

theorem typed_incrementTwo : CodeTyped incrementTwo [.i64, .i64] [.i64, .i64] :=
  CodeTyped.cons (m := [.i64, .i64, .i64])
    (OpTyped.lit (v := .i64 2) (ty := .i64) (s := [.i64, .i64]) .i64)
    (CodeTyped.cons (m := [.i64, .i64]) (OpTyped.add (s := [.i64])) CodeTyped.nil)

theorem typed_incrementBottom : CodeTyped incrementBottom [.i64, .i64] [.i64, .i64] :=
  CodeTyped.cons (m := [.i64, .i64]) (OpTyped.swap (a := .i64) (b := .i64) (s := []))
    (CodeTyped.cons (m := [.i64, .i64, .i64])
      (OpTyped.lit (v := .i64 1) (ty := .i64) (s := [.i64, .i64]) .i64)
      (CodeTyped.cons (m := [.i64, .i64]) (OpTyped.add (s := [.i64]))
        (CodeTyped.cons (m := [.i64, .i64])
          (OpTyped.swap (a := .i64) (b := .i64) (s := [])) CodeTyped.nil)))

theorem typed_incrementBoth : CodeTyped incrementBoth [.i64, .i64] [.i64, .i64] :=
  codeTyped_append typed_incrementBottom typed_incrementOne

theorem leaf_identity : LeafCode identityCode := .nil

theorem leaf_incrementOne : LeafCode incrementOne :=
  .cons (.lit .i64) (.cons .add .nil)

theorem leaf_incrementTwo : LeafCode incrementTwo :=
  .cons (.lit .i64) (.cons .add .nil)

theorem leaf_incrementBottom : LeafCode incrementBottom :=
  .cons .swap (.cons (.lit .i64) (.cons .add (.cons .swap .nil)))

theorem leaf_incrementBoth : LeafCode incrementBoth :=
  .cons .swap (.cons (.lit .i64) (.cons .add (.cons .swap
    (.cons (.lit .i64) (.cons .add .nil)))))

theorem typed_i64_pair {x y : BitVec 64} : StackTyped [.i64 x, .i64 y] [.i64, .i64] :=
  .cons .i64 (.cons .i64 .nil)

theorem stackTyped_i64_pair {s : Stack} (h : StackTyped s [.i64, .i64]) :
    ∃ x y, s = [.i64 x, .i64 y] := by
  obtain ⟨x0, y0, rfl, hx, hy⟩ := stackTyped_pair_shape h
  obtain ⟨x, rfl⟩ := (hasType_i64_iff _).mp hx
  obtain ⟨y, rfl⟩ := (hasType_i64_iff _).mp hy
  exact ⟨x, y, rfl⟩

theorem i64_ne_of_ne {a b : BitVec 64} (h : a ≠ b) : (.i64 a : Value) ≠ .i64 b :=
  fun hv => h (i64_value_eq.mp hv)

theorem not_mergeVal_of_distinct {o a b m : Value} (ha : a ≠ o) (hb : b ≠ o)
    (hab : a ≠ b) : ¬ MergeVal o a b m := by
  intro h
  cases h with
  | unchanged => exact ha rfl
  | first _ => exact hb rfl
  | second _ => exact ha rfl
  | agree _ _ same => exact hab same

/-! ### `incrementOne` on `[I64 I64]` -/

theorem exec_incrementOne {x y : BitVec 64} :
    Exec incrementOne [.i64 x, .i64 y] [.i64 x, .i64 (y + 1)] := by
  have hlit : Exec [.lit (.i64 1)] [.i64 x, .i64 y] [.i64 x, .i64 y, .i64 1] :=
    (exec_lit_iff (v := .i64 1) (s := [.i64 x, .i64 y])).mpr rfl
  have hadd : Exec [.word 4] [.i64 x, .i64 y, .i64 1] [.i64 x, .i64 (y + 1)] :=
    (exec_add_iff (a := y) (b := 1) (s := [.i64 x])).mpr rfl
  exact (exec_append_iff (a := [.lit (.i64 1)]) (b := [.word 4])).mpr ⟨_, hlit, hadd⟩

theorem exec_incrementOne_unique {x y : BitVec 64} {t : Stack}
    (h : Exec incrementOne [.i64 x, .i64 y] t) : t = [.i64 x, .i64 (y + 1)] := by
  have h' : Exec ([.lit (.i64 1)] ++ [.word 4]) [.i64 x, .i64 y] t := h
  obtain ⟨m, h1, h2⟩ :=
    (exec_append_iff (a := [.lit (.i64 1)]) (b := [.word 4])).mp h'
  have hm : m = [.i64 x, .i64 y, .i64 1] :=
    (exec_lit_iff (v := .i64 1) (s := [.i64 x, .i64 y])).mp h1
  subst hm
  exact (exec_add_iff (a := y) (b := 1) (s := [.i64 x])).mp h2

/-! ### `incrementTwo` on `[I64 I64]` -/

theorem exec_incrementTwo {x y : BitVec 64} :
    Exec incrementTwo [.i64 x, .i64 y] [.i64 x, .i64 (y + 2)] := by
  have hlit : Exec [.lit (.i64 2)] [.i64 x, .i64 y] [.i64 x, .i64 y, .i64 2] :=
    (exec_lit_iff (v := .i64 2) (s := [.i64 x, .i64 y])).mpr rfl
  have hadd : Exec [.word 4] [.i64 x, .i64 y, .i64 2] [.i64 x, .i64 (y + 2)] :=
    (exec_add_iff (a := y) (b := 2) (s := [.i64 x])).mpr rfl
  exact (exec_append_iff (a := [.lit (.i64 2)]) (b := [.word 4])).mpr ⟨_, hlit, hadd⟩

theorem exec_incrementTwo_unique {x y : BitVec 64} {t : Stack}
    (h : Exec incrementTwo [.i64 x, .i64 y] t) : t = [.i64 x, .i64 (y + 2)] := by
  have h' : Exec ([.lit (.i64 2)] ++ [.word 4]) [.i64 x, .i64 y] t := h
  obtain ⟨m, h1, h2⟩ :=
    (exec_append_iff (a := [.lit (.i64 2)]) (b := [.word 4])).mp h'
  have hm : m = [.i64 x, .i64 y, .i64 2] :=
    (exec_lit_iff (v := .i64 2) (s := [.i64 x, .i64 y])).mp h1
  subst hm
  exact (exec_add_iff (a := y) (b := 2) (s := [.i64 x])).mp h2

/-! ### `incrementBottom` on `[I64 I64]` -/

theorem exec_incrementBottom {x y : BitVec 64} :
    Exec incrementBottom [.i64 x, .i64 y] [.i64 (x + 1), .i64 y] := by
  have hswap : Exec [.word 2] [.i64 x, .i64 y] [.i64 y, .i64 x] :=
    (exec_swap_iff (a := .i64 x) (b := .i64 y) (s := [])).mpr rfl
  have hlit : Exec [.lit (.i64 1)] [.i64 y, .i64 x] [.i64 y, .i64 x, .i64 1] :=
    (exec_lit_iff (v := .i64 1) (s := [.i64 y, .i64 x])).mpr rfl
  have hadd : Exec [.word 4] [.i64 y, .i64 x, .i64 1] [.i64 y, .i64 (x + 1)] :=
    (exec_add_iff (a := x) (b := 1) (s := [.i64 y])).mpr rfl
  have hswap2 : Exec [.word 2] [.i64 y, .i64 (x + 1)] [.i64 (x + 1), .i64 y] :=
    (exec_swap_iff (a := .i64 y) (b := .i64 (x + 1)) (s := [])).mpr rfl
  have htail : Exec [.lit (.i64 1), .word 4, .word 2] [.i64 y, .i64 x]
      [.i64 (x + 1), .i64 y] :=
    (exec_append_iff (a := [.lit (.i64 1)]) (b := [.word 4, .word 2])).mpr
      ⟨_, hlit, (exec_append_iff (a := [.word 4]) (b := [.word 2])).mpr
        ⟨_, hadd, hswap2⟩⟩
  exact (exec_append_iff (a := [.word 2]) (b := [.lit (.i64 1), .word 4, .word 2])).mpr
    ⟨_, hswap, htail⟩

theorem exec_incrementBottom_unique {x y : BitVec 64} {t : Stack}
    (h : Exec incrementBottom [.i64 x, .i64 y] t) :
    t = [.i64 (x + 1), .i64 y] := by
  have h' : Exec ([.word 2] ++ [.lit (.i64 1), .word 4, .word 2]) [.i64 x, .i64 y] t := h
  obtain ⟨m, h1, h2⟩ :=
    (exec_append_iff (a := [.word 2]) (b := [.lit (.i64 1), .word 4, .word 2])).mp h'
  have hm : m = [.i64 y, .i64 x] :=
    (exec_swap_iff (a := .i64 x) (b := .i64 y) (s := [])).mp h1
  subst hm
  have htail : Exec ([.lit (.i64 1)] ++ [.word 4, .word 2]) [.i64 y, .i64 x] t := h2
  obtain ⟨m, h2, h3⟩ :=
    (exec_append_iff (a := [.lit (.i64 1)]) (b := [.word 4, .word 2])).mp htail
  have hm : m = [.i64 y, .i64 x, .i64 1] :=
    (exec_lit_iff (v := .i64 1) (s := [.i64 y, .i64 x])).mp h2
  subst hm
  have htail : Exec ([.word 4] ++ [.word 2]) [.i64 y, .i64 x, .i64 1] t := h3
  obtain ⟨m, h3, h4⟩ :=
    (exec_append_iff (a := [.word 4]) (b := [.word 2])).mp htail
  have hm : m = [.i64 y, .i64 (x + 1)] :=
    (exec_add_iff (a := x) (b := 1) (s := [.i64 y])).mp h3
  subst hm
  exact (exec_swap_iff (a := .i64 y) (b := .i64 (x + 1)) (s := [])).mp h4

/-! ### `incrementBoth` on `[I64 I64]` -/

theorem exec_incrementBoth {x y : BitVec 64} :
    Exec incrementBoth [.i64 x, .i64 y] [.i64 (x + 1), .i64 (y + 1)] :=
  (exec_append_iff (a := incrementBottom) (b := incrementOne)).mpr
    ⟨_, exec_incrementBottom, exec_incrementOne⟩

theorem exec_incrementBoth_unique {x y : BitVec 64} {t : Stack}
    (h : Exec incrementBoth [.i64 x, .i64 y] t) :
    t = [.i64 (x + 1), .i64 (y + 1)] := by
  obtain ⟨m, h1, h2⟩ :=
    (exec_append_iff (a := incrementBottom) (b := incrementOne)).mp h
  have hm : m = [.i64 (x + 1), .i64 y] := exec_incrementBottom_unique h1
  subst hm
  exact exec_incrementOne_unique h2

/-! ### Relation instances -/

/-- SMERGE-CASE-04: both variants change identically, so the candidate equals
both. -/
theorem mergeRel_identical_increment :
    MergeRel [.i64, .i64] [.i64, .i64] identityCode incrementOne incrementOne
      incrementOne := by
  intro xs hxs
  obtain ⟨x, y, rfl⟩ := stackTyped_i64_pair hxs
  refine ⟨⟨_, exec_nil_iff.mpr rfl, typed_i64_pair⟩,
    ⟨_, exec_incrementOne, typed_i64_pair⟩,
    ⟨_, exec_incrementOne, typed_i64_pair⟩,
    ⟨_, exec_incrementOne, typed_i64_pair⟩, ?_⟩
  intro mo ma mb mm ho ha hb hm
  have hmo : mo = [.i64 x, .i64 y] := exec_nil_iff.mp ho.1
  have hma : ma = [.i64 x, .i64 (y + 1)] := exec_incrementOne_unique ha.1
  have hmb : mb = [.i64 x, .i64 (y + 1)] := exec_incrementOne_unique hb.1
  have hmm : mm = [.i64 x, .i64 (y + 1)] := exec_incrementOne_unique hm.1
  subst hmo; subst hma; subst hmb; subst hmm
  exact .cons .unchanged (.cons
    (.agree (i64_ne_of_ne (add_one_ne_self y)) (i64_ne_of_ne (add_one_ne_self y)) rfl)
    .nil)

/-- SMERGE-CASE-01: independent changes to the two positions are both
preserved. -/
theorem mergeRel_independent_changes :
    MergeRel [.i64, .i64] [.i64, .i64] identityCode incrementOne incrementBottom
      incrementBoth := by
  intro xs hxs
  obtain ⟨x, y, rfl⟩ := stackTyped_i64_pair hxs
  refine ⟨⟨_, exec_nil_iff.mpr rfl, typed_i64_pair⟩,
    ⟨_, exec_incrementOne, typed_i64_pair⟩,
    ⟨_, exec_incrementBottom, typed_i64_pair⟩,
    ⟨_, exec_incrementBoth, typed_i64_pair⟩, ?_⟩
  intro mo ma mb mm ho ha hb hm
  have hmo : mo = [.i64 x, .i64 y] := exec_nil_iff.mp ho.1
  have hma : ma = [.i64 x, .i64 (y + 1)] := exec_incrementOne_unique ha.1
  have hmb : mb = [.i64 (x + 1), .i64 y] := exec_incrementBottom_unique hb.1
  have hmm : mm = [.i64 (x + 1), .i64 (y + 1)] := exec_incrementBoth_unique hm.1
  subst hmo; subst hma; subst hmb; subst hmm
  exact .cons (.second (i64_ne_of_ne (add_one_ne_self x)))
    (.cons (.first (i64_ne_of_ne (add_one_ne_self y))) .nil)

/-- SMERGE-CASE-07: incompatible changes to one position form a conflict. -/
theorem conflict_incompatible_increments (y : BitVec 64) :
    Conflict (.i64 y) (.i64 (y + 1)) (.i64 (y + 2)) :=
  fun m => not_mergeVal_of_distinct (m := m) (i64_ne_of_ne (add_one_ne_self y))
    (i64_ne_of_ne (add_two_ne_self y)) (i64_ne_of_ne (add_one_ne_add_two y))

theorem no_candidate_for_incompatible_increments {x y : BitVec 64} (M : List Op) :
    ¬ MergeRel [.i64, .i64] [.i64, .i64] identityCode incrementOne incrementTwo M := by
  intro h
  have hxs : StackTyped [.i64 x, .i64 y] [.i64, .i64] := typed_i64_pair
  obtain ⟨⟨mo, hmo⟩, ⟨ma, hma⟩, ⟨mb, hmb⟩, ⟨mm, hmm⟩⟩ := mergeRel_returns h hxs
  obtain ⟨p, q, rfl⟩ := stackTyped_i64_pair hmm.2
  have hmo' : mo = [.i64 x, .i64 y] := exec_nil_iff.mp hmo.1
  have hma' : ma = [.i64 x, .i64 (y + 1)] := exec_incrementOne_unique hma.1
  have hmb' : mb = [.i64 x, .i64 (y + 2)] := exec_incrementTwo_unique hmb.1
  subst hmo'; subst hma'; subst hmb'
  exact mergeRel_conflict_absurd h hxs hmo hma hmb hmm (n := 1)
    (ov := .i64 y) (av := .i64 (y + 1)) (bv := .i64 (y + 2)) (mv := .i64 q)
    rfl rfl rfl rfl (conflict_incompatible_increments y)

/-- SMERGE-CASE-08: at the wrapping boundary the merge reads `0`, not a
mathematical integer, so a candidate computing `1` is not the merge. -/
theorem wrapping_boundary_is_not_merge :
    ¬ MergeVal (.i64 18446744073709551615) (.i64 0) (.i64 18446744073709551615)
      (.i64 1) := by
  intro h
  have hsame := mergeVal_functional h
    (MergeVal.first (i64_ne_of_ne (fun h => max_ne_zero h.symm)) :
      MergeVal (.i64 18446744073709551615) (.i64 0) (.i64 18446744073709551615)
        (.i64 0))
  exact absurd (i64_value_eq.mp hsame) (by decide)

end NobleContracts
