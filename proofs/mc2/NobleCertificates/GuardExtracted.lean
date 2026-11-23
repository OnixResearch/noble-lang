import NobleCertificates.GuardLink
import NobleCertificates.GuardsV2
import NobleCertificates.Correspondence

/-!
# C1 for the extracted guard recognizers

`.cairn/archive/2026-11-23-bounded-applicability-guards`, task "Prove C1 recognizer-link theorems": the
extracted-Rust correspondence for `noble_contracts::companion::guard::templates_of` (v1) and
`guard_v2::templates_of`, as translated by the pinned whole-crate Charon/Aeneas lane
(`NobleContractImpl.Funs`).

`arenaTerm` reads the prepared expression arena structurally: an `I64` literal node is `Term.i64`
of the same integer, input `i` is `Term.input i`, and `not`, `eq`, `lt` and `is-nil` nodes are the
same MC1 constructors over their operands' readings. `expression_eq` shows the extracted arena
lookup is exactly that arena read. Each extracted recognizer helper returns `ok` of a pure function
of the arena (`is_input_zero_eq` through `recognize_eq`, and `is_nil_input_zero_eq`), so none of
them can fail or diverge.

Under that reading, `templates_of_sound` shows that the extracted v1 recognizer returns at most one
template, and returns `t` only when the whole `requires` expression reads as one of the five
spellings `recognizeTerm` accepts for `t`. `templates_of_link` composes this with the model-level
C1 theorems of `GuardLink`: the selected template's precondition is exactly what the contract's own
`requires` means on the single `I64` input. `templates_of_complete` is the converse on an `[I64]`
interface. For v2, `templates_of_v2_sound`/`templates_of_v2_link` cover both branches (the v1
recognizer lifted on `[I64]`, and `(not (is-nil x0))` as `NonEmptyList` on `[List A]`, linked
through `GuardsV2.nonEmptyList_link`); `templates_of_v2_complete_i64` and
`templates_of_v2_complete_list` are the converses, the latter for an element type the bounded
`is_type0` check accepts.

Not proved here: that the structural reading agrees with the Lean statement the MC1 export renders
for the same arena (the source-bound statement equations cover fixed examples only), and that the
bounded `is_type0` check accepts every `Type0` element code. This is scoped PO-21 evidence only.
-/

open Aeneas Aeneas.Std Aeneas.Std.Result NobleContracts
open NobleCertificates.Correspondence (rustGuard semanticGuard guard_template_roundtrip)

namespace NobleCertificates.GuardExtracted

set_option maxHeartbeats 1000000
set_option maxRecDepth 2048
set_option Aeneas.customDoElab false

/-! ## The structural reading of the arena -/

/-- The arena node at an index: the prepared expression vector's element, if any. -/
def node (prepared : noble_contracts.Prepared) (index : U32) : Option noble_contracts.Expr :=
  prepared.expressions.val[index.val]?

/-- The extracted arena lookup is exactly that read: its index conversion from `u32` to `usize`
always succeeds, and the slice read is bounds-checked. -/
theorem expression_eq (prepared : noble_contracts.Prepared) (index : U32) :
    noble_contracts.companion.admit.recognize.expression prepared index =
      ok (node prepared index) := by
  have fits : index.val ≤ UScalar.max .Usize := by scalar_tac
  unfold noble_contracts.companion.admit.recognize.expression node
  simp [noble_contracts.Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from,
    noble_kernel.Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from, core.num.tryFromUScalar,
    fits, noble_contracts.Prepared.impl.expressions, core.slice.Slice.get, alloc.vec.Vec.deref]

/-- The structural reading of one node kind, given the reading of its operands: an `I64` literal
is `Term.i64` of the same integer, input `i` is `Term.input i`, and `not`, `eq`, `lt` and `is-nil`
are the same MC1 constructors. Only the kinds the v1 and v2 recognizers inspect are read. -/
def readKind (read : U32 → Option Term) : noble_contracts.ExprKind → Option Term
  | .I64Expr value => some (.i64 value.val)
  | .InputExpr slot => some (.input slot.val)
  | .NotExpr inner => (read inner).map .not
  | .EqExpr left right => (read left).bind fun l => (read right).map fun r => .eq l r
  | .LtExpr left right => (read left).bind fun l => (read right).map fun r => .lt l r
  | .IsNilExpr inner => (read inner).map .isNil
  | _ => none

/-- The structural reading of the prepared expression arena as MC1 terms, to depth `fuel`. A
missing node, a kind `readKind` does not read, or a deeper node has no term here. -/
def arenaTerm (prepared : noble_contracts.Prepared) : Nat → U32 → Option Term
  | 0, _ => none
  | fuel + 1, index =>
    (node prepared index).bind fun n => readKind (arenaTerm prepared fuel) n.kind

/-! ## The five spellings and their meaning -/

/-- The five whole-precondition spellings the v1 recognizer accepts, as MC1 terms. -/
def recognizeTerm : Term → Option GuardTemplate
  | .lt (.input 0) (.i64 k) => if k = 9223372036854775807 then some .ltI64Max else none
  | .eq (.input 0) (.i64 k) => some (.eqI64Literal (BitVec.ofInt 64 k))
  | .eq (.i64 k) (.input 0) => some (.eqI64Literal (BitVec.ofInt 64 k))
  | .not (.eq (.input 0) (.i64 k)) =>
      if k = -9223372036854775808 then some .neI64Min else none
  | .not (.eq (.i64 k) (.input 0)) =>
      if k = -9223372036854775808 then some .neI64Min else none
  | _ => none

/-- C1 over the accepted spellings: a term `recognizeTerm` maps to `t` means exactly `t`'s
precondition on the single `I64` input. -/
theorem recognizeTerm_link {term : Term} {t : GuardTemplate} (h : recognizeTerm term = some t)
    (x : BitVec 64) (params : List Value) :
    Holds term [.i64 x] [] params ↔ GuardPrecondition t x := by
  unfold recognizeTerm at h
  split at h
  · split at h
    · cases h
      subst_vars
      exact ltI64Max_link x params
    · cases h
  · cases h
    exact eqI64Literal_link _ x params
  · cases h
    exact eqI64Literal_link_swapped _ x params
  · split at h
    · cases h
      subst_vars
      exact neI64Min_link x params
    · cases h
  · split at h
    · cases h
      subst_vars
      exact neI64Min_link_swapped x params
    · cases h
  · cases h

/-! ## The extracted recognizer, step by step -/

/-- A monadic bind that returns `ok` ran its first computation to `ok`. -/
theorem bind_eq_ok {α β : Type} {x : Result α} {f : α → Result β} {y : β}
    (h : Std.bind x f = ok y) : ∃ a, x = ok a ∧ f a = ok y := by
  cases x with
  | ret a => exact ⟨a, rfl, by simpa using h⟩
  | vis i k => simp at h
  | div => simp at h

/-- `is_input_zero`: the node is input `0`. -/
def inputZero (prepared : noble_contracts.Prepared) (index : U32) : Bool :=
  match node prepared index with
  | some n =>
    match n.kind with
    | .InputExpr slot => decide (slot = 0#u32)
    | _ => false
  | none => false

theorem is_input_zero_eq (prepared : noble_contracts.Prepared) (index : U32) :
    noble_contracts.companion.guard.is_input_zero prepared index =
      ok (inputZero prepared index) := by
  unfold noble_contracts.companion.guard.is_input_zero inputZero
  rw [expression_eq]
  simp only [bind_ok]
  cases node prepared index with
  | none => rfl
  | some n =>
    cases hk : n.kind <;> simp [noble_contracts.companion.admit.recognize.is_input_zero, hk]

/-- `literal_of`: the node is an `I64` literal. -/
def literal (prepared : noble_contracts.Prepared) (index : U32) : Option I64 :=
  match node prepared index with
  | some n =>
    match n.kind with
    | .I64Expr value => some value
    | _ => none
  | none => none

theorem literal_of_eq (prepared : noble_contracts.Prepared) (index : U32) :
    noble_contracts.companion.guard.literal_of prepared index = ok (literal prepared index) := by
  unfold noble_contracts.companion.guard.literal_of literal
  rw [expression_eq]
  simp only [bind_ok]
  cases node prepared index with
  | none => rfl
  | some n =>
    cases hk : n.kind <;> simp [noble_contracts.companion.admit.recognize.literal, hk]

/-- `input_equality`: one operand is input `0` and the other an `I64` literal, left first. -/
def inputEquality (prepared : noble_contracts.Prepared) (left right : U32) : Option I64 :=
  if inputZero prepared left then literal prepared right
  else if inputZero prepared right then literal prepared left
  else none

theorem input_equality_eq (prepared : noble_contracts.Prepared) (left right : U32) :
    noble_contracts.companion.guard.input_equality prepared left right =
      ok (inputEquality prepared left right) := by
  unfold noble_contracts.companion.guard.input_equality inputEquality
  simp only [is_input_zero_eq, literal_of_eq, bind_ok]
  split <;> (try split) <;> rfl

/-- The model of `Option<i64>` equality the extracted code calls. -/
theorem option_i64_eq (o : Option I64) (m : I64) :
    noble_contracts.core.option.Option.Insts.CoreCmpPartialEqOption.eq core.cmp.PartialEqI64 o
        (some m) =
      ok (decide (o = some m)) := by
  cases o <;> simp [noble_contracts.core.option.Option.Insts.CoreCmpPartialEqOption.eq]

/-- `is_minimum_equality`: the node is an equality whose input-literal reading is `i64::MIN`. -/
def minimumEquality (prepared : noble_contracts.Prepared) (index : U32) : Bool :=
  match node prepared index with
  | some n =>
    match n.kind with
    | .EqExpr left right => decide (inputEquality prepared left right = some core.num.I64.MIN)
    | _ => false
  | none => false

theorem is_minimum_equality_eq (prepared : noble_contracts.Prepared) (index : U32) :
    noble_contracts.companion.guard.is_minimum_equality prepared index =
      ok (minimumEquality prepared index) := by
  unfold noble_contracts.companion.guard.is_minimum_equality minimumEquality
  rw [expression_eq]
  simp only [bind_ok]
  cases node prepared index with
  | none => rfl
  | some n =>
    cases hk : n.kind <;> simp [hk, input_equality_eq, option_i64_eq]

/-- `recognize`, as a pure function of the arena. -/
def recognizeRust (prepared : noble_contracts.Prepared) :
    noble_contracts.ExprKind → Option noble_contracts.companion.GuardTemplate
  | .NotExpr inner =>
      if minimumEquality prepared inner then some .NeI64Min else none
  | .EqExpr left right => (inputEquality prepared left right).map .EqI64Literal
  | .LtExpr left right =>
      if inputZero prepared left then
        if literal prepared right = some core.num.I64.MAX then some .LtI64Max else none
      else none
  | _ => none

theorem recognize_eq (prepared : noble_contracts.Prepared) (kind : noble_contracts.ExprKind) :
    noble_contracts.companion.guard.recognize prepared kind = ok (recognizeRust prepared kind) := by
  cases kind with
  | NotExpr inner =>
    simp only [noble_contracts.companion.guard.recognize, recognizeRust, is_minimum_equality_eq,
      bind_ok]
    split <;> rfl
  | EqExpr left right =>
    simp only [noble_contracts.companion.guard.recognize, recognizeRust, input_equality_eq,
      bind_ok]
    rcases inputEquality prepared left right with _ | value
    · rfl
    · simp [core.option.Option.map,
        noble_contracts.P.Insts.CoreOpsFunctionFnOnceTupleI64GuardTemplate.call_once,
        noble_contracts.companion.GuardTemplate.EqI64Literal.constructor]
  | LtExpr left right =>
    simp only [noble_contracts.companion.guard.recognize, recognizeRust, is_input_zero_eq,
      literal_of_eq, option_i64_eq, bind_ok]
    split <;> (try split) <;> simp_all
  | _ => rfl

/-! ## Soundness of the extracted recognizer -/

private theorem read_input {prepared : noble_contracts.Prepared} {index : U32} {fuel : Nat}
    (h : inputZero prepared index = true) :
    arenaTerm prepared (fuel + 1) index = some (.input 0) := by
  unfold inputZero at h
  cases hn : node prepared index with
  | none => simp [hn] at h
  | some n =>
    rw [hn] at h
    cases hk : n.kind <;> simp [hk] at h
    subst h
    simp [arenaTerm, hn, readKind, hk]

private theorem read_literal {prepared : noble_contracts.Prepared} {index : U32} {fuel : Nat}
    {value : I64} (h : literal prepared index = some value) :
    arenaTerm prepared (fuel + 1) index = some (.i64 value.val) := by
  unfold literal at h
  cases hn : node prepared index with
  | none => simp [hn] at h
  | some n =>
    rw [hn] at h
    cases hk : n.kind <;> simp [hk] at h
    subst h
    simp [arenaTerm, hn, readKind, hk]

/-- An input-literal equality reads as one of the two `eq` spellings. -/
private theorem read_equality {prepared : noble_contracts.Prepared} {left right : U32}
    {fuel : Nat} {value : I64} (h : inputEquality prepared left right = some value) :
    readKind (arenaTerm prepared (fuel + 1)) (.EqExpr left right) =
        some (.eq (.input 0) (.i64 value.val)) ∨
      readKind (arenaTerm prepared (fuel + 1)) (.EqExpr left right) =
        some (.eq (.i64 value.val) (.input 0)) := by
  unfold inputEquality at h
  split at h
  · rename_i hzero
    left
    simp [readKind, read_input (fuel := fuel) hzero, read_literal (fuel := fuel) h]
  · split at h
    · rename_i hzero
      right
      simp [readKind, read_input (fuel := fuel) hzero, read_literal (fuel := fuel) h]
    · cases h

private theorem i64_max_val : core.num.I64.MAX.val = 9223372036854775807 := by decide

private theorem i64_min_val : core.num.I64.MIN.val = -9223372036854775808 := by decide

/-- Soundness of `recognize`: a template it returns is the Rust form of the template
`recognizeTerm` assigns to the kind's reading. -/
theorem recognizeRust_sound {prepared : noble_contracts.Prepared}
    {kind : noble_contracts.ExprKind} {template : noble_contracts.companion.GuardTemplate}
    (h : recognizeRust prepared kind = some template) :
    ∃ t term, template = rustGuard t ∧ readKind (arenaTerm prepared 2) kind = some term ∧
      recognizeTerm term = some t := by
  cases kind with
  | NotExpr inner =>
    simp only [recognizeRust] at h
    split at h
    · rename_i hmin
      cases h
      unfold minimumEquality at hmin
      cases hn : node prepared inner with
      | none => simp [hn] at hmin
      | some n =>
        rw [hn] at hmin
        cases hk : n.kind <;> simp [hk] at hmin
        rename_i left right
        have hread : arenaTerm prepared 2 inner =
            readKind (arenaTerm prepared 1) (.EqExpr left right) := by
          simp [arenaTerm, hn, hk]
        rcases read_equality (fuel := 0) hmin with hr | hr
        · refine ⟨.neI64Min, .not (.eq (.input 0) (.i64 core.num.I64.MIN.val)), rfl, ?_, ?_⟩
          · have hr' : readKind (arenaTerm prepared 1) (.EqExpr left right) =
                some (.eq (.input 0) (.i64 core.num.I64.MIN.val)) := hr
            show (arenaTerm prepared 2 inner).map Term.not = _
            rw [hread, hr']
            rfl
          · rw [i64_min_val]
            rfl
        · refine ⟨.neI64Min, .not (.eq (.i64 core.num.I64.MIN.val) (.input 0)), rfl, ?_, ?_⟩
          · have hr' : readKind (arenaTerm prepared 1) (.EqExpr left right) =
                some (.eq (.i64 core.num.I64.MIN.val) (.input 0)) := hr
            show (arenaTerm prepared 2 inner).map Term.not = _
            rw [hread, hr']
            rfl
          · rw [i64_min_val]
            rfl
    · cases h
  | EqExpr left right =>
    simp only [recognizeRust, Option.map_eq_some_iff] at h
    obtain ⟨value, hvalue, rfl⟩ := h
    rcases read_equality (fuel := 1) hvalue with hr | hr
    · refine ⟨.eqI64Literal (BitVec.ofInt 64 value.val), .eq (.input 0) (.i64 value.val), ?_,
        hr, ?_⟩
      · simp [rustGuard]
      · simp [recognizeTerm]
    · refine ⟨.eqI64Literal (BitVec.ofInt 64 value.val), .eq (.i64 value.val) (.input 0), ?_,
        hr, ?_⟩
      · simp [rustGuard]
      · simp [recognizeTerm]
  | LtExpr left right =>
    simp only [recognizeRust] at h
    split at h
    · split at h
      · rename_i hzero hlit
        cases h
        have ha : arenaTerm prepared 2 left = some (.input 0) := read_input hzero
        have hb : arenaTerm prepared 2 right = some (.i64 core.num.I64.MAX.val) := read_literal hlit
        refine ⟨.ltI64Max, .lt (.input 0) (.i64 core.num.I64.MAX.val), rfl, ?_, ?_⟩
        · simp [readKind, ha, hb]
        · rw [i64_max_val]
          rfl
      · cases h
    · cases h
  | _ => simp [recognizeRust] at h

/-- `Vec.push` onto the empty vector returns the one-element vector. -/
private theorem push_empty {T : Type} {t : T} {found : alloc.vec.Vec T}
    (h : alloc.vec.Vec.push (alloc.vec.Vec.with_capacity T 1#usize) t = ok found) :
    found.val = [t] := by
  unfold alloc.vec.Vec.push at h
  simp [alloc.vec.Vec.with_capacity] at h
  have fits : 1 ≤ U32.max ∨ 1 ≤ Usize.max := Or.inl (by scalar_tac)
  rw [dite_eq_left fits, Result.ok.injEq] at h
  subst h
  rfl

/-- Soundness of the extracted `templates_of`: it returns at most one template, and returns `t`
only when the whole `requires` expression reads as a spelling `recognizeTerm` maps to `t`. -/
theorem templates_of_sound (prepared : noble_contracts.Prepared)
    (found : alloc.vec.Vec noble_contracts.companion.GuardTemplate)
    (h : noble_contracts.companion.guard.templates_of prepared = ok found) :
    found.val = [] ∨ ∃ t term, found.val = [rustGuard t] ∧
      arenaTerm prepared 3 prepared.requires = some term ∧ recognizeTerm term = some t := by
  unfold noble_contracts.companion.guard.templates_of at h
  simp only [noble_contracts.Prepared.impl.checked, noble_contracts.Prepared.impl.requires,
    expression_eq, recognize_eq, bind_ok] at h
  obtain ⟨s, -, h⟩ := bind_eq_ok h
  obtain ⟨b, -, h⟩ := bind_eq_ok h
  split at h
  · left
    simp only [Result.ok.injEq] at h
    subst h
    rfl
  · cases hn : node prepared prepared.requires with
    | none =>
      simp only [hn, Result.ok.injEq] at h
      left
      subst h
      rfl
    | some slot =>
      simp only [hn] at h
      cases hr : recognizeRust prepared slot.kind with
      | none =>
        simp only [hr, Result.ok.injEq] at h
        left
        subst h
        rfl
      | some template =>
        simp only [hr] at h
        right
        obtain ⟨t, term, rfl, hterm, hrec⟩ := recognizeRust_sound hr
        refine ⟨t, term, push_empty h, ?_, hrec⟩
        show (node prepared prepared.requires).bind
          (fun n => readKind (arenaTerm prepared 2) n.kind) = some term
        rw [hn]
        exact hterm

/-- **C1 for the extracted recognizer**: whenever the extracted `templates_of` selects a
template, the contract's own `requires` reads as an MC1 term that, on the single `I64` input,
holds exactly when the selected template's precondition holds. -/
theorem templates_of_link (prepared : noble_contracts.Prepared)
    (found : alloc.vec.Vec noble_contracts.companion.GuardTemplate)
    (h : noble_contracts.companion.guard.templates_of prepared = ok found)
    {selected : noble_contracts.companion.GuardTemplate} (hsel : selected ∈ found.val) :
    ∃ term, arenaTerm prepared 3 prepared.requires = some term ∧
      ∀ x params, Holds term [.i64 x] [] params ↔
        GuardPrecondition (semanticGuard selected) x := by
  rcases templates_of_sound prepared found h with hempty | ⟨t, term, hval, hterm, hrec⟩
  · simp [hempty] at hsel
  · rw [hval, List.mem_singleton] at hsel
    subst hsel
    refine ⟨term, hterm, fun x params => ?_⟩
    rw [guard_template_roundtrip]
    exact recognizeTerm_link hrec x params

/-! ## Completeness on an `[I64]` interface -/

/-- The kernel's type equality on `I64` and `I64`: one comparison pass, then the empty work
list. -/
theorem ty_eq_i64 : noble_kernel.types.impls.ty_eq .I64Type .I64Type = ok true := by
  unfold noble_kernel.types.impls.ty_eq noble_kernel.types.impls.ty_eq_loop0
  have fits : 1 ≤ U32.max ∨ 1 ≤ Usize.max := Or.inl (by scalar_tac)
  simp only [alloc.vec.Vec.with_capacity]
  unfold alloc.vec.Vec.push
  simp [alloc.vec.Vec.new]
  rw [dite_eq_left fits, bind_ok, loop]
  simp [noble_kernel.types.impls.ty_eq_loop0.body, noble_kernel.alloc.vec.Vec.is_empty,
    noble_kernel.alloc.vec.Vec.pop, noble_kernel.types.WORK_CAP]
  rw [loop]
  simp

/-- The extracted interface check reports an `[I64]` interface equal to `[I64]`. -/
theorem interface_i64 (s : Slice noble_contracts.noble_kernel.types.Ty)
    (hs : s.val = [.I64Type]) (proof) :
    noble_contracts.Shared0Slice.Insts.CoreCmpPartialEqArray.ne
      noble_contracts.noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy s
      (Array.make 1#usize [noble_contracts.noble_kernel.types.Ty.I64Type] proof) = ok false := by
  simp [noble_contracts.Shared0Slice.Insts.CoreCmpPartialEqArray.ne,
    noble_contracts.Slice.Insts.CoreCmpPartialEqArray.ne,
    noble_contracts.Slice.Insts.CoreCmpPartialEqArray.eq, core.slice.cmp.PartialEqSlice.eq, hs,
    Array.to_slice, Array.make, noble_contracts.noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq,
    core.cmp.PartialEq.ne.default, noble_contracts.KernelBridge.toTy,
    noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq, ty_eq_i64]

/-- A node that reads as input `0` is input `0` for the extracted helper. -/
private theorem input_of_read {prepared : noble_contracts.Prepared} {index : U32} {fuel : Nat}
    (h : arenaTerm prepared (fuel + 1) index = some (.input 0)) :
    inputZero prepared index = true := by
  simp only [arenaTerm, Option.bind_eq_some_iff] at h
  obtain ⟨n, hn, hk⟩ := h
  unfold inputZero
  rw [hn]
  cases hkind : n.kind <;>
    simp [readKind, hkind, Option.bind_eq_some_iff, Option.map_eq_some_iff] at hk ⊢
  scalar_tac

/-- A node that reads as a literal is that literal for the extracted helper, and not input `0`. -/
private theorem literal_of_read {prepared : noble_contracts.Prepared} {index : U32} {fuel : Nat}
    {k : Int} (h : arenaTerm prepared (fuel + 1) index = some (.i64 k)) :
    (∃ value, literal prepared index = some value ∧ value.val = k) ∧
      inputZero prepared index = false := by
  simp only [arenaTerm, Option.bind_eq_some_iff] at h
  obtain ⟨n, hn, hk⟩ := h
  unfold literal inputZero
  rw [hn]
  cases hkind : n.kind <;>
    simp [readKind, hkind, Option.bind_eq_some_iff, Option.map_eq_some_iff] at hk ⊢
  exact hk

private theorem readKind_lt {read : U32 → Option Term} {kind : noble_contracts.ExprKind}
    {l r : Term} (h : readKind read kind = some (.lt l r)) :
    ∃ a b, kind = .LtExpr a b ∧ read a = some l ∧ read b = some r := by
  cases kind <;> simp [readKind, Option.bind_eq_some_iff, Option.map_eq_some_iff] at h
  exact ⟨_, _, rfl, h.1, h.2⟩

private theorem readKind_eq {read : U32 → Option Term} {kind : noble_contracts.ExprKind}
    {l r : Term} (h : readKind read kind = some (.eq l r)) :
    ∃ a b, kind = .EqExpr a b ∧ read a = some l ∧ read b = some r := by
  cases kind <;> simp [readKind, Option.bind_eq_some_iff, Option.map_eq_some_iff] at h
  exact ⟨_, _, rfl, h.1, h.2⟩

private theorem readKind_not {read : U32 → Option Term} {kind : noble_contracts.ExprKind}
    {body : Term} (h : readKind read kind = some (.not body)) :
    ∃ inner, kind = .NotExpr inner ∧ read inner = some body := by
  cases kind <;> simp [readKind, Option.bind_eq_some_iff, Option.map_eq_some_iff] at h
  exact ⟨_, rfl, h⟩

private theorem arenaTerm_kind {prepared : noble_contracts.Prepared} {index : U32} {fuel : Nat}
    {term : Term} (h : arenaTerm prepared (fuel + 1) index = some term) :
    ∃ n, node prepared index = some n ∧
      readKind (arenaTerm prepared fuel) n.kind = some term := by
  simpa [arenaTerm, Option.bind_eq_some_iff] using h

/-- Completeness of `recognize`: a kind whose reading is an accepted spelling of `t` is
recognized as the Rust form of `t`. -/
theorem recognizeRust_complete {prepared : noble_contracts.Prepared}
    {kind : noble_contracts.ExprKind} {term : Term} {t : GuardTemplate}
    (hterm : readKind (arenaTerm prepared 2) kind = some term)
    (hrec : recognizeTerm term = some t) :
    recognizeRust prepared kind = some (rustGuard t) := by
  unfold recognizeTerm at hrec
  split at hrec
  · split at hrec
    · rename_i k hk
      cases hrec
      obtain ⟨a, b, rfl, ha, hb⟩ := readKind_lt hterm
      have hzero := input_of_read ha
      obtain ⟨⟨v, hv, hval⟩, -⟩ := literal_of_read hb
      have hmax : v = core.num.I64.MAX := IScalar.eq_of_val_eq (by rw [hval, hk]; decide)
      simp [recognizeRust, hzero, hv, hmax, rustGuard]
    · cases hrec
  · cases hrec
    obtain ⟨a, b, rfl, ha, hb⟩ := readKind_eq hterm
    have hzero := input_of_read ha
    obtain ⟨⟨v, hv, hval⟩, -⟩ := literal_of_read hb
    subst hval
    simp [recognizeRust, inputEquality, hzero, hv, rustGuard]
  · cases hrec
    obtain ⟨a, b, rfl, ha, hb⟩ := readKind_eq hterm
    obtain ⟨⟨v, hv, hval⟩, hnz⟩ := literal_of_read ha
    have hzero := input_of_read hb
    subst hval
    simp [recognizeRust, inputEquality, hnz, hzero, hv, rustGuard]
  · split at hrec
    · rename_i k hk
      cases hrec
      obtain ⟨inner, rfl, hinner⟩ := readKind_not hterm
      obtain ⟨n, hn, hkind⟩ := arenaTerm_kind hinner
      obtain ⟨a, b, hk2, ha, hb⟩ := readKind_eq hkind
      have hzero := input_of_read ha
      obtain ⟨⟨v, hv, hval⟩, -⟩ := literal_of_read hb
      have hmin : v = core.num.I64.MIN := IScalar.eq_of_val_eq (by rw [hval, hk]; decide)
      simp [recognizeRust, minimumEquality, hn, hk2, inputEquality, hzero, hv, hmin, rustGuard]
    · cases hrec
  · split at hrec
    · rename_i k hk
      cases hrec
      obtain ⟨inner, rfl, hinner⟩ := readKind_not hterm
      obtain ⟨n, hn, hkind⟩ := arenaTerm_kind hinner
      obtain ⟨a, b, hk2, ha, hb⟩ := readKind_eq hkind
      obtain ⟨⟨v, hv, hval⟩, hnz⟩ := literal_of_read ha
      have hzero := input_of_read hb
      have hmin : v = core.num.I64.MIN := IScalar.eq_of_val_eq (by rw [hval, hk]; decide)
      simp [recognizeRust, minimumEquality, hn, hk2, inputEquality, hnz, hzero, hv, hmin,
        rustGuard]
    · cases hrec
  · cases hrec

/-- **Completeness of the extracted `templates_of`** on an `[I64]` interface: when the whole
`requires` expression reads as a spelling `recognizeTerm` maps to `t`, the extracted recognizer
returns exactly the Rust form of `t`. -/
theorem templates_of_complete (prepared : noble_contracts.Prepared)
    (hiface : prepared.checked.interface.stack_in.val = [.I64Type])
    {term : Term} {t : GuardTemplate}
    (hterm : arenaTerm prepared 3 prepared.requires = some term)
    (hrec : recognizeTerm term = some t) :
    ∃ found, noble_contracts.companion.guard.templates_of prepared = ok found ∧
      found.val = [rustGuard t] := by
  obtain ⟨n, hn, hkind⟩ := arenaTerm_kind hterm
  have hr := recognizeRust_complete hkind hrec
  have fits : 1 ≤ U32.max ∨ 1 ≤ Usize.max := Or.inl (by scalar_tac)
  unfold noble_contracts.companion.guard.templates_of
  simp only [noble_contracts.Prepared.impl.checked, noble_contracts.Prepared.impl.requires,
    expression_eq, recognize_eq, bind_ok, hn, hr, noble_contracts.alloc.vec.Vec.as_slice,
    noble_kernel.alloc.vec.Vec.as_slice]
  rw [interface_i64 _ hiface]
  simp only [bind_ok, Bool.false_eq_true, ↓reduceIte]
  unfold alloc.vec.Vec.push
  simp only [alloc.vec.Vec.with_capacity, alloc.vec.Vec.new]
  simp [fits]

/-! ## The v2 recognizer: soundness -/

/-- The Rust form of a v2 template. -/
def rustGuardV2 : GuardTemplateV2 → noble_contracts.companion.guard_v2.GuardTemplateV2
  | .ltI64Max => .LtI64Max
  | .neI64Min => .NeI64Min
  | .eqI64Literal k => .EqI64Literal ⟨k⟩
  | .nonEmptyList => .NonEmptyList

/-- The whole-precondition spellings the v2 recognizer accepts: `(not (is-nil x0))` for
`NonEmptyList`, and the five v1 spellings carried into the v2 set. -/
def recognizeTermV2 : Term → Option GuardTemplateV2
  | .not (.isNil (.input 0)) => some .nonEmptyList
  | term => (recognizeTerm term).map GuardTemplateV2.fromV1

/-- C1 over the v2 spellings, for every input the template takes: a term `recognizeTermV2` maps
to `t` means exactly `t`'s precondition. -/
theorem recognizeTermV2_link {term : Term} {t : GuardTemplateV2}
    (h : recognizeTermV2 term = some t) {v : Value} (hv : HasInput t v) (params : List Value) :
    Holds term [v] [] params ↔ GuardPreconditionV2 t v := by
  unfold recognizeTermV2 at h
  split at h
  · cases h
    obtain ⟨xs, rfl⟩ := hv
    exact nonEmptyList_link xs params
  · obtain ⟨t1, h1, rfl⟩ := Option.map_eq_some_iff.mp h
    cases t1 <;> obtain ⟨x, rfl⟩ := hv <;>
      simpa [GuardPreconditionV2, GuardTemplateV2.fromV1] using recognizeTerm_link h1 x params

/-- `GuardTemplateV2::from_v1`, as a pure function. -/
def fromV1Rust : noble_contracts.companion.GuardTemplate →
    noble_contracts.companion.guard_v2.GuardTemplateV2
  | .LtI64Max => .LtI64Max
  | .NeI64Min => .NeI64Min
  | .EqI64Literal value => .EqI64Literal value

theorem from_v1_eq (template : noble_contracts.companion.GuardTemplate) :
    noble_contracts.companion.guard_v2.GuardTemplateV2.from_v1 template =
      ok (fromV1Rust template) := by
  cases template <;> rfl

theorem fromV1Rust_rustGuard (t : GuardTemplate) :
    fromV1Rust (rustGuard t) = rustGuardV2 (GuardTemplateV2.fromV1 t) := by
  cases t <;> rfl

/-- `is_nil_input_zero`: the node is `is-nil` of input `0`. -/
def nilInputZero (prepared : noble_contracts.Prepared) (index : U32) : Bool :=
  match node prepared index with
  | some n =>
    match n.kind with
    | .IsNilExpr body => inputZero prepared body
    | _ => false
  | none => false

theorem is_nil_input_zero_eq (prepared : noble_contracts.Prepared) (index : U32) :
    noble_contracts.companion.guard_v2.is_nil_input_zero prepared index =
      ok (nilInputZero prepared index) := by
  unfold noble_contracts.companion.guard_v2.is_nil_input_zero nilInputZero
  rw [expression_eq]
  simp only [bind_ok]
  cases node prepared index with
  | none => rfl
  | some n =>
    cases hk : n.kind <;> simp only [hk]
    rename_i body
    rw [expression_eq]
    simp only [bind_ok]
    unfold inputZero
    cases node prepared body with
    | none => rfl
    | some m =>
      cases hm : m.kind <;> simp [noble_contracts.companion.admit.recognize.is_input_zero, hm]

/-- `saturating_add` of `0` and `1` on `usize` is `1`. -/
private theorem saturating_zero_one : (core.num.Usize.saturating_add 0#usize 1#usize).val = 1 := by
  have h : 1 ≤ UScalar.max .Usize := by scalar_tac
  have hpow : 1 < 2 ^ System.Platform.numBits := by
    rcases System.Platform.numBits_eq with h' | h' <;> rw [h'] <;> decide
  unfold core.num.Usize.saturating_add UScalar.saturating_add
  show (BitVec.ofNat _ (min (UScalar.max .Usize) ((0#usize).val + (1#usize).val))).toNat = 1
  simp only [BitVec.toNat_ofNat]
  have h01 : (0#usize).val + (1#usize).val = 1 := by scalar_tac
  rw [h01, Nat.min_eq_right h]
  exact Nat.mod_eq_of_lt hpow

/-- The v2 recognizer's lifting loop copies the (at most one) v1 template into the v2 set. -/
theorem templates_of_loop_eq (lifted : alloc.vec.Vec noble_contracts.companion.GuardTemplate)
    (hlen : lifted.val.length ≤ 1) :
    ∃ out, noble_contracts.companion.guard_v2.templates_of_loop
        (alloc.vec.Vec.with_capacity _ 1#usize) lifted 0#usize = ok out ∧
      out.val = lifted.val.map fromV1Rust := by
  have fits : 1 ≤ U32.max ∨ 1 ≤ Usize.max := Or.inl (by scalar_tac)
  rcases hv : lifted.val with _ | ⟨x, _ | ⟨y, ys⟩⟩
  · unfold noble_contracts.companion.guard_v2.templates_of_loop
    rw [loop]
    simp [noble_contracts.companion.guard_v2.templates_of_loop.body, alloc.vec.Vec.len, hv,
      alloc.vec.Vec.with_capacity]
  · have hs : lifted.slice.val = [x] := hv
    have hidx : lifted.slice.index_usize 0#usize = ok x := by simp [Slice.index_usize, hs]
    unfold noble_contracts.companion.guard_v2.templates_of_loop
    rw [loop]
    simp only [noble_contracts.companion.guard_v2.templates_of_loop.body]
    unfold alloc.vec.Vec.push
    simp [alloc.vec.Vec.len, hv, from_v1_eq, alloc.vec.Vec.index, alloc.vec.Vec.with_capacity,
      fits, hidx, lift]
    rw [loop]
    simp [saturating_zero_one]
  · simp [hv] at hlen

/-- A node that reads as `(not (is-nil x0))` is recognized as such by the extracted helper. -/
private theorem read_nonEmpty {prepared : noble_contracts.Prepared} {inner : U32}
    (h : nilInputZero prepared inner = true) :
    readKind (arenaTerm prepared 2) (.NotExpr inner) = some nonEmptyListTerm := by
  unfold nilInputZero at h
  cases hn : node prepared inner with
  | none => simp [hn] at h
  | some n =>
    rw [hn] at h
    cases hk : n.kind <;> simp [hk] at h
    rename_i body
    have hbody : arenaTerm prepared 1 body = some (.input 0) := read_input h
    show (arenaTerm prepared 2 inner).map Term.not = _
    have hinner : arenaTerm prepared 2 inner = some (.isNil (.input 0)) := by
      show (node prepared inner).bind (fun n => readKind (arenaTerm prepared 1) n.kind) = _
      rw [hn]
      show readKind (arenaTerm prepared 1) n.kind = _
      rw [hk]
      show (arenaTerm prepared 1 body).map Term.isNil = _
      rw [hbody]
      rfl
    rw [hinner]
    rfl

/-- **Soundness of the extracted v2 `templates_of`**: every template it returns is the Rust form
of a template that `recognizeTermV2` assigns to the reading of the whole `requires`. -/
theorem templates_of_v2_sound (prepared : noble_contracts.Prepared)
    (found : alloc.vec.Vec noble_contracts.companion.guard_v2.GuardTemplateV2)
    (h : noble_contracts.companion.guard_v2.templates_of prepared = ok found) :
    ∀ selected ∈ found.val, ∃ t term, selected = rustGuardV2 t ∧
      arenaTerm prepared 3 prepared.requires = some term ∧ recognizeTermV2 term = some t := by
  intro selected hsel
  unfold noble_contracts.companion.guard_v2.templates_of at h
  simp only [noble_contracts.Prepared.impl.checked, bind_ok] at h
  obtain ⟨s, -, h⟩ := bind_eq_ok h
  obtain ⟨b, -, h⟩ := bind_eq_ok h
  split at h
  · -- an `[I64]` interface: the v1 recognizer, lifted
    obtain ⟨lifted, hlifted, h⟩ := bind_eq_ok h
    have hsound := templates_of_sound prepared lifted hlifted
    have hlen : lifted.val.length ≤ 1 := by
      rcases hsound with h0 | ⟨t, term, hv, -, -⟩
      · simp [h0]
      · simp [hv]
    obtain ⟨out, hout, hval⟩ := templates_of_loop_eq lifted hlen
    rw [hout, Result.ok.injEq] at h
    subst h
    rw [hval] at hsel
    obtain ⟨t1, ht1, rfl⟩ := List.mem_map.mp hsel
    rcases hsound with h0 | ⟨t, term, hv, hterm, hrec⟩
    · simp [h0] at ht1
    · rw [hv, List.mem_singleton] at ht1
      subst ht1
      refine ⟨GuardTemplateV2.fromV1 t, term, fromV1Rust_rustGuard t, hterm, ?_⟩
      unfold recognizeTermV2
      split
      · simp [recognizeTerm] at hrec
      · simp [hrec]
  · -- otherwise only a one-element `[List A]` interface with `A` passing `is_type0`
    simp only [noble_contracts.alloc.vec.Vec.as_slice, noble_kernel.alloc.vec.Vec.as_slice,
      bind_ok] at h
    split at h
    · rw [Result.ok.injEq] at h
      subst h
      simp [alloc.vec.Vec.with_capacity] at hsel
    · obtain ⟨ty, -, h⟩ := bind_eq_ok h
      cases ty <;> simp only at h
      all_goals
        try (rw [Result.ok.injEq] at h; subst h; simp [alloc.vec.Vec.with_capacity] at hsel)
      obtain ⟨b1, -, h⟩ := bind_eq_ok h
      split at h
      · simp only [noble_contracts.Prepared.impl.requires, expression_eq, bind_ok] at h
        cases hn : node prepared prepared.requires with
        | none =>
          simp only [hn, Result.ok.injEq] at h
          subst h
          simp [alloc.vec.Vec.with_capacity] at hsel
        | some slot =>
          simp only [hn] at h
          cases hk : slot.kind <;> simp only [hk] at h
          all_goals
            try (rw [Result.ok.injEq] at h; subst h; simp [alloc.vec.Vec.with_capacity] at hsel)
          rename_i inner
          rw [is_nil_input_zero_eq] at h
          simp only [bind_ok] at h
          split at h
          · rename_i hnil
            have hval := push_empty h
            rw [hval, List.mem_singleton] at hsel
            subst hsel
            refine ⟨.nonEmptyList, nonEmptyListTerm, rfl, ?_, rfl⟩
            show (node prepared prepared.requires).bind
              (fun n => readKind (arenaTerm prepared 2) n.kind) = _
            rw [hn]
            show readKind (arenaTerm prepared 2) slot.kind = _
            rw [hk]
            exact read_nonEmpty hnil
          · rw [Result.ok.injEq] at h
            subst h
            simp [alloc.vec.Vec.with_capacity] at hsel
      · rw [Result.ok.injEq] at h
        subst h
        simp [alloc.vec.Vec.with_capacity] at hsel

/-- **C1 for the extracted v2 recognizer**: whenever the extracted `guard_v2::templates_of`
selects a template, the contract's own `requires` reads as an MC1 term that, on every input the
template takes, holds exactly when the selected template's precondition holds. -/
theorem templates_of_v2_link (prepared : noble_contracts.Prepared)
    (found : alloc.vec.Vec noble_contracts.companion.guard_v2.GuardTemplateV2)
    (h : noble_contracts.companion.guard_v2.templates_of prepared = ok found)
    {selected : noble_contracts.companion.guard_v2.GuardTemplateV2} (hsel : selected ∈ found.val) :
    ∃ t term, selected = rustGuardV2 t ∧ arenaTerm prepared 3 prepared.requires = some term ∧
      ∀ v params, HasInput t v → (Holds term [v] [] params ↔ GuardPreconditionV2 t v) := by
  obtain ⟨t, term, hselected, hterm, hrec⟩ := templates_of_v2_sound prepared found h selected hsel
  exact ⟨t, term, hselected, hterm, fun v params hv => recognizeTermV2_link hrec hv params⟩

/-- The kernel's type equality on a list type and `I64`: one comparison pass returns `false`. -/
theorem ty_eq_list_i64 (item : noble_kernel.types.Ty) :
    noble_kernel.types.impls.ty_eq (.ListType item) .I64Type = ok false := by
  unfold noble_kernel.types.impls.ty_eq noble_kernel.types.impls.ty_eq_loop0
  have fits : 1 ≤ U32.max ∨ 1 ≤ Usize.max := Or.inl (by scalar_tac)
  simp only [alloc.vec.Vec.with_capacity]
  unfold alloc.vec.Vec.push
  simp [alloc.vec.Vec.new]
  rw [dite_eq_left fits, bind_ok, loop]
  simp [noble_kernel.types.impls.ty_eq_loop0.body, noble_kernel.alloc.vec.Vec.is_empty,
    noble_kernel.alloc.vec.Vec.pop, noble_kernel.types.WORK_CAP]

/-- The v2 interface check reports an `[I64]` interface equal to `[I64]`. -/
theorem interface_eq_i64 (s : Slice noble_contracts.noble_kernel.types.Ty)
    (hs : s.val = [.I64Type]) (proof) :
    noble_contracts.Shared0Slice.Insts.CoreCmpPartialEqArray.eq
      noble_contracts.noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy s
      (Array.make 1#usize [noble_contracts.noble_kernel.types.Ty.I64Type] proof) = ok true := by
  simp [noble_contracts.Shared0Slice.Insts.CoreCmpPartialEqArray.eq,
    noble_contracts.Slice.Insts.CoreCmpPartialEqArray.eq, core.slice.cmp.PartialEqSlice.eq, hs,
    Array.to_slice, Array.make, noble_contracts.noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq,
    core.cmp.PartialEq.ne.default, noble_contracts.KernelBridge.toTy,
    noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq, ty_eq_i64, pure_tc_eq]

/-- The v2 interface check reports a one-element list interface different from `[I64]`. -/
theorem interface_eq_list (s : Slice noble_contracts.noble_kernel.types.Ty)
    {item : noble_contracts.noble_kernel.types.Ty} (hs : s.val = [.ListType item]) (proof) :
    noble_contracts.Shared0Slice.Insts.CoreCmpPartialEqArray.eq
      noble_contracts.noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy s
      (Array.make 1#usize [noble_contracts.noble_kernel.types.Ty.I64Type] proof) = ok false := by
  simp [noble_contracts.Shared0Slice.Insts.CoreCmpPartialEqArray.eq,
    noble_contracts.Slice.Insts.CoreCmpPartialEqArray.eq, core.slice.cmp.PartialEqSlice.eq, hs,
    Array.to_slice, Array.make, noble_contracts.noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq,
    core.cmp.PartialEq.ne.default, noble_contracts.KernelBridge.toTy,
    noble_kernel.types.Ty.Insts.CoreCmpPartialEqTy.eq, ty_eq_list_i64, pure_tc_eq]

/-- **Completeness of the extracted v2 `templates_of`** on an `[I64]` interface: a v1 spelling of
`t` is recognized as the v2 form of `t`. -/
theorem templates_of_v2_complete_i64 (prepared : noble_contracts.Prepared)
    (hiface : prepared.checked.interface.stack_in.val = [.I64Type])
    {term : Term} {t : GuardTemplate}
    (hterm : arenaTerm prepared 3 prepared.requires = some term)
    (hrec : recognizeTerm term = some t) :
    ∃ found, noble_contracts.companion.guard_v2.templates_of prepared = ok found ∧
      found.val = [rustGuardV2 (GuardTemplateV2.fromV1 t)] := by
  obtain ⟨lifted, hlifted, hval⟩ := templates_of_complete prepared hiface hterm hrec
  have hlen : lifted.val.length ≤ 1 := by simp [hval]
  obtain ⟨out, hout, hvalout⟩ := templates_of_loop_eq lifted hlen
  refine ⟨out, ?_, by rw [hvalout, hval]; simp [fromV1Rust_rustGuard]⟩
  unfold noble_contracts.companion.guard_v2.templates_of
  simp only [noble_contracts.Prepared.impl.checked, noble_contracts.alloc.vec.Vec.as_slice,
    noble_kernel.alloc.vec.Vec.as_slice, bind_ok]
  rw [interface_eq_i64 _ hiface]
  simp only [bind_ok, ↓reduceIte, hlifted, hout]

/-- **Completeness of the extracted v2 `templates_of`** on a one-element `[List A]` interface
whose element passes the bounded `is_type0` check: `(not (is-nil x0))` is recognized as
`NonEmptyList`. The bounded check itself may refuse a very large `Type0` code; that refusal is
not a recognizer gap but the check's stated budget. -/
theorem templates_of_v2_complete_list (prepared : noble_contracts.Prepared)
    {item : noble_contracts.noble_kernel.types.Ty}
    (hiface : prepared.checked.interface.stack_in.val = [.ListType item])
    (htype : noble_contracts.companion.guard_v2.is_type0 item = ok true)
    (hterm : arenaTerm prepared 3 prepared.requires = some nonEmptyListTerm) :
    ∃ found, noble_contracts.companion.guard_v2.templates_of prepared = ok found ∧
      found.val = [rustGuardV2 .nonEmptyList] := by
  obtain ⟨n, hn, hkind⟩ := arenaTerm_kind hterm
  obtain ⟨inner, hk, hinner⟩ := readKind_not hkind
  obtain ⟨m, hm, hmkind⟩ := arenaTerm_kind hinner
  have hnil : nilInputZero prepared inner = true := by
    unfold nilInputZero
    rw [hm]
    cases hmk : m.kind <;> simp [readKind, hmk, Option.bind_eq_some_iff, Option.map_eq_some_iff]
      at hmkind ⊢
    exact input_of_read (fuel := 0) hmkind
  have fits : 1 ≤ U32.max ∨ 1 ≤ Usize.max := Or.inl (by scalar_tac)
  unfold noble_contracts.companion.guard_v2.templates_of
  simp only [noble_contracts.Prepared.impl.checked, noble_contracts.alloc.vec.Vec.as_slice,
    noble_kernel.alloc.vec.Vec.as_slice, bind_ok]
  rw [interface_eq_list _ hiface]
  simp only [bind_ok, Bool.false_eq_true, ↓reduceIte]
  have hs : prepared.checked.interface.stack_in.slice.val = [.ListType item] := hiface
  have hlen : (prepared.checked.interface.stack_in.slice.len != 1#usize) = false := by
    simp [Slice.len, hs]
    apply UScalar.eq_of_val_eq
    simp [UScalar.ofNatCore_val_eq]
  have hidx : prepared.checked.interface.stack_in.slice.index_usize 0#usize =
      ok (.ListType item) := by
    simp [Slice.index_usize, hs]
  rw [hlen, hidx]
  simp only [Bool.false_eq_true, ↓reduceIte, bind_ok, htype, noble_contracts.Prepared.impl.requires,
    expression_eq, hn, hk, is_nil_input_zero_eq, hnil]
  unfold alloc.vec.Vec.push
  simp only [alloc.vec.Vec.with_capacity, alloc.vec.Vec.new]
  simp [fits, rustGuardV2]

end NobleCertificates.GuardExtracted
