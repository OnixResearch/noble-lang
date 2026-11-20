import NobleCertificates.GuardLink
import NobleContracts.Expression

/-!
# MC2 v2 guard templates: the bounded `NonEmptyList` extension

`.cairn/changes/bounded-applicability-guards` selects exactly one v2 extension:
a whole-root `(not (is-nil (in x0)))` on a one-element `[List A]` interface with
`A` a pure `Type0` code. The v1 set (`GuardTemplate`, version 1) stays frozen:
`MC2Gate` still pins exactly its three constructors, `guard.rs` keeps its bytes
and the v1 recognizer keeps its five spellings.

This module defines the versioned v2 set as a separate inductive so no v1
witness can be silently reinterpreted. It proves, for the new template:

- **C1** (`nonEmptyList_link`): the MC1 meaning of the contract's own
  `requires` term `(not (is-nil (in x0)))` is exactly `GuardPreconditionV2`,
  i.e. the argument list has a head and a tail. The model-level link is stated
  over the reviewed `NobleContracts.Holds`; the extracted-Rust link for
  `templates_of_v2` is a separate obligation and is not claimed here.
- **C2** (`guardProgramV2_exec_iff`): the fixed guard text computes the total
  rejection flag and leaves the argument untouched underneath it.
- **C3** (`guard_sound_v2`, `nonEmptyList_exact`): a false flag establishes the
  precondition, and for `NonEmptyList` the flag is false exactly when the
  precondition holds.
- **C4** (`wrapperProgramV2_exec_iff`): rejection returns the defined
  `Sum` rejection and starts no body; acceptance runs the candidate on exactly
  the original argument stack and tags its top result.
- **C5** (`sourceWrapperProgramV2_refines`, `liveWrapperProgramV2_refines`):
  the two renderer texts fixed in the design denote that same wrapper
  (primitive structure and refinement only; parsing the rendered text is the
  separate extracted-renderer obligation).

Nothing here asserts a parser, a renderer string equation, a Wasm lowering
(PO-17/PO-18, C6) or that the Rust `Prepared` arena denotes these terms.
-/

open NobleContracts

namespace NobleCertificates

/-- Version tag of this template set. The v1 set is version 1 and is pinned
unchanged by `MC2Gate`; a binding records which version it was made under. -/
def guardTemplateSetVersionV2 : Nat := 2

/-- The pure `Type0` codes: `Unit`, `Bool`, `I64`, `Text` and the recursive
`Pair`, `Sum` and `List` constructions over them. Programs, syntax, contracts,
evidence, certified values, resources and live references are excluded, so a
`List A` element type can never smuggle executable code or authority into a
guard. -/
inductive Type0Code : Ty → Prop where
  | unit : Type0Code .unit
  | bool : Type0Code .bool
  | i64 : Type0Code .i64
  | text : Type0Code .text
  | pair {a b : Ty} : Type0Code a → Type0Code b → Type0Code (.pair a b)
  | sum {a b : Ty} : Type0Code a → Type0Code b → Type0Code (.sum a b)
  | list {a : Ty} : Type0Code a → Type0Code (.list a)

/-- The pure type codes any `List A` guard element type must come from. -/
theorem type0_list_i64 : Type0Code (.list .i64) := .list .i64

/-- A nested list is still a `Type0` code, so `[List (List I64)]` is accepted
(GUARD-CASE-03's element shape). -/
theorem type0_nested_list : Type0Code (.list (.list .i64)) :=
  .list (.list .i64)

/-- A program is not a `Type0` code: no element type may be executable code. -/
theorem program_is_not_type0 (i o : List Ty) : ¬ Type0Code (.program i o) := by
  intro h
  cases h

/-- A syntax value is not a `Type0` code. -/
theorem syntax_is_not_type0 : ¬ Type0Code .syntax := by
  intro h
  cases h

/-- The versioned v2 template set: the three frozen v1 templates plus exactly
one list template. A v2 binding names the set version, so a v1 witness is never
reinterpreted as a v2 one. -/
inductive GuardTemplateV2 where
  /-- Accept `x` unless incrementing it would wrap past `i64::MAX`. -/
  | ltI64Max
  /-- Accept `x` unless it is exactly `i64::MIN`. -/
  | neI64Min
  /-- Accept `x` only when it equals the compile-time literal `k`. -/
  | eqI64Literal (k : BitVec 64)
  /-- Accept a list only when it has a head, i.e. it is not `nil`. -/
  | nonEmptyList

/-- The v1 template, carried unchanged into the v2 set. -/
def GuardTemplateV2.fromV1 : GuardTemplate → GuardTemplateV2
  | .ltI64Max => .ltI64Max
  | .neI64Min => .neI64Min
  | .eqI64Literal k => .eqI64Literal k

/-- The exact input interface shape each template is formed over. `NonEmptyList`
is formed over `[List A]` for a pure `Type0` element code `A`; the v1 templates
remain formed over the single `I64` input. -/
def GuardTemplateV2.interface : GuardTemplateV2 → List Ty → Prop
  | .ltI64Max => fun s => s = [.i64]
  | .neI64Min => fun s => s = [.i64]
  | .eqI64Literal _ => fun s => s = [.i64]
  | .nonEmptyList => fun s => ∃ a, Type0Code a ∧ s = [.list a]

/-- A well-typed argument value for a template. `NonEmptyList` takes any list;
the v1 templates keep taking an `I64`. -/
def HasInput : GuardTemplateV2 → Value → Prop
  | .ltI64Max => fun v => ∃ x, v = .i64 x
  | .neI64Min => fun v => ∃ x, v = .i64 x
  | .eqI64Literal _ => fun v => ∃ x, v = .i64 x
  | .nonEmptyList => fun v => ∃ xs, v = .list xs

/-- The stated applicability precondition each v2 template establishes. The
three v1 members keep their v1 meaning exactly; `NonEmptyList` means the
argument list is a `cons`. A value of any other shape establishes nothing. -/
def GuardPreconditionV2 : GuardTemplateV2 → Value → Prop
  | .ltI64Max, .i64 x => GuardPrecondition .ltI64Max x
  | .ltI64Max, _ => False
  | .neI64Min, .i64 x => GuardPrecondition .neI64Min x
  | .neI64Min, _ => False
  | .eqI64Literal k, .i64 x => GuardPrecondition (.eqI64Literal k) x
  | .eqI64Literal _, _ => False
  | .nonEmptyList, .list xs => ∃ a as, xs = a :: as
  | .nonEmptyList, _ => False

/-- The executable guard's rejection flag: `true` means refuse to run the body.
The list template's flag is `true` exactly for `nil`; a value of another shape
rejects, so a mistyped input can never be accepted. -/
def guardRejectsV2 : GuardTemplateV2 → Value → Bool
  | .ltI64Max, .i64 x => guardRejects .ltI64Max x
  | .ltI64Max, _ => true
  | .neI64Min, .i64 x => guardRejects .neI64Min x
  | .neI64Min, _ => true
  | .eqI64Literal k, .i64 x => guardRejects (.eqI64Literal k) x
  | .eqI64Literal _, _ => true
  | .nonEmptyList, .list xs => xs.isEmpty
  | .nonEmptyList, _ => true

/-- The fixed executable guard program for a v2 template. The list template is
`[ dup [ true ] [ drop drop false ] list.case ]`: it duplicates the argument,
runs the constant `true` program for `nil` and `[ drop drop false ]` on
`head tail` for `cons`, and leaves the original argument below the flag. -/
def guardProgramV2 : GuardTemplateV2 → List Op
  | .ltI64Max => guardProgram .ltI64Max
  | .neI64Min => guardProgram .neI64Min
  | .eqI64Literal k => guardProgram (.eqI64Literal k)
  | .nonEmptyList =>
      [.word 0, .block [.lit (.bool true)],
        .block [.word 1, .word 1, .lit (.bool false)], .word 21]

/-- The whole v2 wrapper around a candidate body, in normal form. -/
def wrapperProgramV2 (template : GuardTemplateV2) (subject : List Op) : List Op :=
  guardProgramV2 template ++
    [.block rejectionProgram, .block (acceptProgram subject), .word 18]

/-- Exact primitive structure of the v2 source wrapper
`[ dup [ true ] [ drop drop false ] list.case [ drop 1 inl ] [ [ SUBJECT ] run inr ] if ]`
after parsing its literal and subject quotation. As in v1, this does not
postulate correctness of that parsing. -/
def sourceWrapperProgramV2 (template : GuardTemplateV2) (subject : List Op) : List Op :=
  match template with
  | .ltI64Max =>
      [.word 0, .lit (.i64 i64max), .word 7,
        .block rejectionProgram, .block (acceptProgram subject), .word 18]
  | .neI64Min =>
      [.word 0, .lit (.i64 i64min), .word 7,
        .block rejectionProgram, .block (acceptProgram subject), .word 18]
  | .eqI64Literal k =>
      [.word 0, .lit (.i64 k), .word 7,
        .block (acceptProgram subject), .block rejectionProgram, .word 18]
  | .nonEmptyList =>
      guardProgramV2 .nonEmptyList ++
        [.block rejectionProgram, .block (acceptProgram subject), .word 18]

/-- Exact primitive structure of the v2 invocation wrapper
`[ swap dup [ true ] [ drop drop false ] list.case [ drop drop 1 inl ] [ swap run inr ] if ]`
that runs on `[argument, Program]` and consumes the already supplied live
program value. -/
def liveWrapperProgramV2 : GuardTemplateV2 → List Op
  | .ltI64Max =>
      [.word 2, .word 0, .lit (.i64 i64max), .word 7,
        .block [.word 1, .word 1, .lit (.i64 1), .word 15],
        .block [.word 2, .word 10, .word 16], .word 18]
  | .neI64Min =>
      [.word 2, .word 0, .lit (.i64 i64min), .word 7,
        .block [.word 1, .word 1, .lit (.i64 1), .word 15],
        .block [.word 2, .word 10, .word 16], .word 18]
  | .eqI64Literal k =>
      [.word 2, .word 0, .lit (.i64 k), .word 7,
        .block [.word 2, .word 10, .word 16],
        .block [.word 1, .word 1, .lit (.i64 1), .word 15], .word 18]
  | .nonEmptyList =>
      [.word 2] ++ guardProgramV2 .nonEmptyList ++
        [.block [.word 1, .word 1, .lit (.i64 1), .word 15],
          .block [.word 2, .word 10, .word 16], .word 18]

/-! ## C1: the recognized precondition means the template precondition -/

/-- `(not (is-nil (in x0)))`, the only supported `NonEmptyList` spelling. -/
def nonEmptyListTerm : Term := .not (.isNil (.input 0))

/-- **C1, `NonEmptyList`.** The reviewed MC1 meaning of the contract's own
`requires` term is exactly the template precondition: the input list is a
`cons`. -/
theorem nonEmptyList_link (xs : List Value) (params : List Value) :
    Holds nonEmptyListTerm [.list xs] [] params ↔
      GuardPreconditionV2 .nonEmptyList (.list xs) := by
  cases xs with
  | nil => simp [Holds, nonEmptyListTerm, evaluate, GuardPreconditionV2]
  | cons a as => simp [Holds, nonEmptyListTerm, evaluate, GuardPreconditionV2]

/-- **C1 under the typed interface.** Any value that the interface accepts
(`[List A]`, `A` a `Type0` code) satisfies the same link, so the recognizer's
type guard does not weaken the model statement. -/
theorem nonEmptyList_link_of_type {a : Ty} (_ha : Type0Code a) {v : Value}
    (hv : HasType v (.list a)) (params : List Value) :
    Holds nonEmptyListTerm [v] [] params ↔
      GuardPreconditionV2 .nonEmptyList v := by
  cases hv with
  | list _ => exact nonEmptyList_link _ params

/-- The declaration form `[List I64]` of the recognizer's formed interface. -/
theorem nonEmptyList_interface_i64 :
    GuardTemplateV2.interface .nonEmptyList [Ty.list Ty.i64] :=
  ⟨Ty.i64, Type0Code.i64, rfl⟩

/-! ## C2: the guard program computes the flag and keeps the argument -/

/-- The rejection program consumes any top value and returns the defined
`Sum` rejection. -/
@[simp] theorem rejectionProgram_exec_any (v : Value) (tail result : Stack) :
    Exec rejectionProgram (tail ++ [v]) result ↔ result = tail ++ [.inl (.i64 1)] := by
  simp [rejectionProgram, Exec, List.reverse_append]

/-- Acceptance runs the candidate body unchanged, then tags its top output. -/
@[simp] theorem acceptProgram_exec_any (subject : List Op) (v : Value) (tail result : Stack) :
    Exec (acceptProgram subject) (tail ++ [v]) result ↔
      ∃ middle, Exec subject (tail ++ [v]) middle ∧ Exec [.word 16] middle result := by
  change Exec ([.block subject, .word 10] ++ [.word 16]) _ _ ↔ _
  rw [exec_append_iff]
  simp [Exec, List.reverse_append]

/-- **C2, `NonEmptyList`.** `[ dup [ true ] [ drop drop false ] list.case ]`
computes `true` exactly on `nil` and `false` on `cons`, and leaves the original
list below the flag. -/
theorem guardProgramV2_nonEmptyList_exec_iff (xs : List Value) (tail result : Stack) :
    Exec (guardProgramV2 .nonEmptyList) (tail ++ [.list xs]) result ↔
      result = tail ++ [.list xs, .bool (guardRejectsV2 .nonEmptyList (.list xs))] := by
  cases xs with
  | nil =>
      simp [guardProgramV2, guardRejectsV2, Exec, List.reverse_append]
  | cons a as =>
      simp [guardProgramV2, guardRejectsV2, Exec, List.reverse_append]

/-- **C2 for every v2 template**, on a well-typed argument: the fixed guard
text leaves the argument untouched and appends exactly the rejection flag. -/
theorem guardProgramV2_exec_iff (template : GuardTemplateV2) (v : Value)
    (tail result : Stack) (h : HasInput template v) :
    Exec (guardProgramV2 template) (tail ++ [v]) result ↔
      result = tail ++ [v, .bool (guardRejectsV2 template v)] := by
  cases template with
  | ltI64Max =>
      obtain ⟨x, rfl⟩ := h
      simp [guardProgramV2, guardRejectsV2, guardRejects]
  | neI64Min =>
      obtain ⟨x, rfl⟩ := h
      simp [guardProgramV2, guardRejectsV2, guardRejects]
  | eqI64Literal k =>
      obtain ⟨x, rfl⟩ := h
      have hflag : guardRejectsV2 (.eqI64Literal k) (.i64 x) = !(x == k) := by
        cases hh : (x == k)
        · have hne : x ≠ k := beq_eq_false_iff_ne.mp hh
          simp [guardRejectsV2, guardRejects, hne]
        · have heq : x = k := beq_iff_eq.mp hh
          simp [guardRejectsV2, guardRejects, heq]
      change Exec (guardProgram (.eqI64Literal k)) (tail ++ [.i64 x]) result ↔
        result = tail ++ [.i64 x, .bool (guardRejectsV2 (.eqI64Literal k) (.i64 x))]
      rw [hflag, guardProgram_eqI64Literal_exec_iff k x tail result]
  | nonEmptyList =>
      obtain ⟨xs, rfl⟩ := h
      exact guardProgramV2_nonEmptyList_exec_iff xs tail result

/-! ## C3: soundness and exactness of the rejection flag -/

/-- **C3, `NonEmptyList` soundness:** a false flag establishes the precondition. -/
theorem nonEmptyList_sound (xs : List Value)
    (h : guardRejectsV2 .nonEmptyList (.list xs) = false) :
    GuardPreconditionV2 .nonEmptyList (.list xs) := by
  cases xs with
  | nil => simp [guardRejectsV2] at h
  | cons a as => exact ⟨a, as, rfl⟩

/-- **C3, `NonEmptyList` exactness:** the flag is false exactly when the
precondition holds, so no nonempty list is refused and no empty list accepted. -/
theorem nonEmptyList_exact (xs : List Value) :
    guardRejectsV2 .nonEmptyList (.list xs) = false ↔
      GuardPreconditionV2 .nonEmptyList (.list xs) := by
  cases xs with
  | nil => simp [guardRejectsV2, GuardPreconditionV2]
  | cons a as => simp [guardRejectsV2, GuardPreconditionV2]

/-- **C3 for every v2 template** on a well-typed argument: acceptance implies
the stated precondition. -/
theorem guard_sound_v2 (template : GuardTemplateV2) (v : Value)
    (h : guardRejectsV2 template v = false) : GuardPreconditionV2 template v := by
  cases template with
  | ltI64Max =>
      cases v with
      | i64 x => exact guard_sound .ltI64Max x h
      | unit => simp [guardRejectsV2] at h
      | bool _ => simp [guardRejectsV2] at h
      | text _ => simp [guardRejectsV2] at h
      | pair _ _ => simp [guardRejectsV2] at h
      | inl _ => simp [guardRejectsV2] at h
      | inr _ => simp [guardRejectsV2] at h
      | list _ => simp [guardRejectsV2] at h
      | program _ => simp [guardRejectsV2] at h
      | «syntax» _ => simp [guardRejectsV2] at h
  | neI64Min =>
      cases v with
      | i64 x => exact guard_sound .neI64Min x h
      | unit => simp [guardRejectsV2] at h
      | bool _ => simp [guardRejectsV2] at h
      | text _ => simp [guardRejectsV2] at h
      | pair _ _ => simp [guardRejectsV2] at h
      | inl _ => simp [guardRejectsV2] at h
      | inr _ => simp [guardRejectsV2] at h
      | list _ => simp [guardRejectsV2] at h
      | program _ => simp [guardRejectsV2] at h
      | «syntax» _ => simp [guardRejectsV2] at h
  | eqI64Literal k =>
      cases v with
      | i64 x => exact guard_sound (.eqI64Literal k) x h
      | unit => simp [guardRejectsV2] at h
      | bool _ => simp [guardRejectsV2] at h
      | text _ => simp [guardRejectsV2] at h
      | pair _ _ => simp [guardRejectsV2] at h
      | inl _ => simp [guardRejectsV2] at h
      | inr _ => simp [guardRejectsV2] at h
      | list _ => simp [guardRejectsV2] at h
      | program _ => simp [guardRejectsV2] at h
      | «syntax» _ => simp [guardRejectsV2] at h
  | nonEmptyList =>
      cases v with
      | list xs => exact nonEmptyList_sound xs h
      | unit => simp [guardRejectsV2] at h
      | bool _ => simp [guardRejectsV2] at h
      | i64 _ => simp [guardRejectsV2] at h
      | text _ => simp [guardRejectsV2] at h
      | pair _ _ => simp [guardRejectsV2] at h
      | inl _ => simp [guardRejectsV2] at h
      | inr _ => simp [guardRejectsV2] at h
      | program _ => simp [guardRejectsV2] at h
      | «syntax» _ => simp [guardRejectsV2] at h

/-! ## C4: the wrapper's semantics -/

/-- **C4.** Acceptance runs the candidate on exactly the original argument
stack; rejection returns a defined `Sum` and requests no candidate body. -/
theorem wrapperProgramV2_exec_iff (template : GuardTemplateV2) (subject : List Op)
    (v : Value) (tail result : Stack) (hinput : HasInput template v) :
    Exec (wrapperProgramV2 template subject) (tail ++ [v]) result ↔
      (if guardRejectsV2 template v = true then result = tail ++ [.inl (.i64 1)]
       else ∃ middle, Exec subject (tail ++ [v]) middle ∧
         Exec [.word 16] middle result) := by
  rw [wrapperProgramV2, exec_append_iff]
  constructor
  · rintro ⟨middle, hguard, hrest⟩
    rw [guardProgramV2_exec_iff template v tail middle hinput] at hguard
    subst hguard
    have hbranch := (branch_exec_iff (acceptProgram subject) rejectionProgram
      (guardRejectsV2 template v) tail v result).mp hrest
    by_cases hflag : guardRejectsV2 template v = true
    · rw [ite_eq_left hflag] at hbranch ⊢
      exact (rejectionProgram_exec_any v tail result).mp hbranch
    · rw [ite_eq_right hflag] at hbranch ⊢
      exact (acceptProgram_exec_any subject v tail result).mp hbranch
  · intro h
    refine ⟨_, (guardProgramV2_exec_iff template v tail _ hinput).mpr rfl, ?_⟩
    refine (branch_exec_iff (acceptProgram subject) rejectionProgram
      (guardRejectsV2 template v) tail v result).mpr ?_
    by_cases hflag : guardRejectsV2 template v = true
    · rw [ite_eq_left hflag] at h ⊢
      exact (rejectionProgram_exec_any v tail result).mpr h
    · rw [ite_eq_right hflag] at h ⊢
      exact (acceptProgram_exec_any subject v tail result).mpr h

/-- Rejection is decided purely by the flag: `true` refuses the body. -/
theorem wrapperProgramV2_rejects (template : GuardTemplateV2) (subject : List Op)
    (v : Value) (tail result : Stack) (h : HasInput template v)
    (hflag : guardRejectsV2 template v = true) :
    Exec (wrapperProgramV2 template subject) (tail ++ [v]) result ↔
      result = tail ++ [.inl (.i64 1)] := by
  rw [wrapperProgramV2_exec_iff template subject v tail result h, ite_eq_left hflag]

/-- Acceptance runs the candidate body on the original argument stack. -/
theorem wrapperProgramV2_accepts (template : GuardTemplateV2) (subject : List Op)
    (v : Value) (tail result : Stack) (h : HasInput template v)
    (hflag : guardRejectsV2 template v = false) :
    Exec (wrapperProgramV2 template subject) (tail ++ [v]) result ↔
      ∃ middle, Exec subject (tail ++ [v]) middle ∧
        Exec [.word 16] middle result := by
  have hne : ¬ (guardRejectsV2 template v = true) := by rw [hflag]; decide
  rw [wrapperProgramV2_exec_iff template subject v tail result h, ite_eq_right hne]

/-- The theorem the generated v2 wrapper's semantics discharges: a positive
applicability check establishes the exact stated precondition for the actual
input, and the candidate then runs on the untouched argument stack. -/
theorem guard_correspondence_v2 (template : GuardTemplateV2) (subject : List Op)
    (v : Value) (tail result : Stack) (h : HasInput template v)
    (hflag : guardRejectsV2 template v = false)
    (hr : Exec (wrapperProgramV2 template subject) (tail ++ [v]) result) :
    GuardPreconditionV2 template v ∧
      ∃ middle, Exec subject (tail ++ [v]) middle ∧ Exec [.word 16] middle result :=
  ⟨guard_sound_v2 template v hflag,
    (wrapperProgramV2_accepts template subject v tail result h hflag).mp hr⟩

/-! ## C5: the fixed renderer texts denote the same wrapper -/

/-- **C5, source wrapper.** The design's fixed source text parses to a program
with the same execution as the normal-form v2 wrapper, for every template. -/
theorem sourceWrapperProgramV2_refines (template : GuardTemplateV2) (subject : List Op)
    (v : Value) (tail result : Stack) (h : HasInput template v) :
    Exec (sourceWrapperProgramV2 template subject) (tail ++ [v]) result ↔
      Exec (wrapperProgramV2 template subject) (tail ++ [v]) result := by
  cases template with
  | ltI64Max => rfl
  | neI64Min => rfl
  | nonEmptyList => rfl
  | eqI64Literal k =>
      obtain ⟨x, rfl⟩ := h
      by_cases hk : x = k
      · have hflag : guardRejectsV2 (.eqI64Literal k) (.i64 x) = false := by
          simp [guardRejectsV2, guardRejects, hk]
        rw [wrapperProgramV2_accepts (.eqI64Literal k) subject (.i64 x) tail result ⟨x, rfl⟩ hflag]
        have sourceRun :
            Exec (sourceWrapperProgramV2 (.eqI64Literal k) subject) (tail ++ [.i64 x])
                result ↔
              Exec (acceptProgram subject) (tail ++ [.i64 x]) result := by
          simp [sourceWrapperProgramV2, Exec, List.reverse_append, hk]
        exact sourceRun.trans (acceptProgram_exec_any subject (.i64 x) tail result)
      · have hflag : guardRejectsV2 (.eqI64Literal k) (.i64 x) = true := by
          simp [guardRejectsV2, guardRejects, hk]
        have hb : (x == k) = false := beq_eq_false_iff_ne.mpr hk
        rw [wrapperProgramV2_rejects (.eqI64Literal k) subject (.i64 x) tail result ⟨x, rfl⟩ hflag]
        simp [sourceWrapperProgramV2, Exec, rejectionProgram, List.reverse_append, hb]

/-- **C5, invocation wrapper.** The design's fixed live text, running on
`[argument, Program]`, is observationally the source wrapper around the exact
supplied program, for every subject body and normal result. -/
theorem liveWrapperProgramV2_refines (template : GuardTemplateV2) (subject : List Op)
    (v : Value) (tail result : Stack) (h : HasInput template v) :
    Exec (liveWrapperProgramV2 template) (tail ++ [v, .program subject]) result ↔
      Exec (sourceWrapperProgramV2 template subject) (tail ++ [v]) result := by
  cases template with
  | ltI64Max =>
      obtain ⟨x, rfl⟩ := h
      cases hh : (x == i64max) <;>
        simp [liveWrapperProgramV2, sourceWrapperProgramV2, Exec, acceptProgram,
          rejectionProgram, List.reverse_append, hh]
  | neI64Min =>
      obtain ⟨x, rfl⟩ := h
      cases hh : (x == i64min) <;>
        simp [liveWrapperProgramV2, sourceWrapperProgramV2, Exec, acceptProgram,
          rejectionProgram, List.reverse_append, hh]
  | eqI64Literal k =>
      obtain ⟨x, rfl⟩ := h
      cases hh : (x == k) <;>
        simp [liveWrapperProgramV2, sourceWrapperProgramV2, Exec, acceptProgram,
          rejectionProgram, List.reverse_append, hh]
  | nonEmptyList =>
      obtain ⟨xs, rfl⟩ := h
      cases xs with
      | nil =>
          simp [liveWrapperProgramV2, sourceWrapperProgramV2, guardProgramV2, Exec,
            acceptProgram, rejectionProgram, List.reverse_append]
      | cons a as =>
          simp [liveWrapperProgramV2, sourceWrapperProgramV2, guardProgramV2, Exec,
            acceptProgram, rejectionProgram, List.reverse_append]

/-! ## Frozen set version -/

/-- The v2 set version tag is exactly 2. -/
theorem guardTemplateSetVersionV2_eq : guardTemplateSetVersionV2 = 2 := rfl

end NobleCertificates
