import NobleCertificates.Model

/-!
# MC2 guard templates

Guards are *generated wrapper programs from a finite prechecked template set*.
There is no logical-to-executable compilation machinery: each template v1 has
one fixed wrapper program and one correspondence theorem proved here. A guard is
ordinary guest code; it runs before the subject value is invoked and returns a
defined `Sum` rejection (`.inl (.i64 1)`) instead of trapping. Acceptance tags
the candidate's top result with `inr`, matching the production wrapper.

The wrapper preserves the candidate's stack interface on acceptance: the guard
consumes only its own `dup` copy, so the accepted branch runs the candidate on
exactly the original argument stack. On rejection no candidate-body request is
made at all.

Only three templates exist. Adding a template requires a new constructor and a
new `guard_sound_*` theorem here; unknown templates are refused by the core.
-/

open NobleContracts

namespace NobleCertificates

/-- The signed 64-bit bounds, as the wrapping two's-complement bit patterns. -/
def i64min : BitVec 64 := -9223372036854775808
def i64max : BitVec 64 := 9223372036854775807

/-- The frozen finite v1 template set. `eqI64Literal` is a one-parameter family;
the other two are closed templates. -/
inductive GuardTemplate where
  /-- Accept `x` unless incrementing it would wrap past `i64::MAX`. -/
  | ltI64Max
  /-- Accept `x` unless it is exactly `i64::MIN`. -/
  | neI64Min
  /-- Accept `x` only when it equals the compile-time literal `k`. -/
  | eqI64Literal (k : BitVec 64)

/-- The stated applicability precondition each template must establish before
the candidate body may run. These are exactly the preconditions the checked
wrapper claims; a template that established something weaker would be refused. -/
def GuardPrecondition : GuardTemplate → BitVec 64 → Prop
  | .ltI64Max => fun x => x.toInt < i64max.toInt
  | .neI64Min => fun x => x ≠ i64min
  | .eqI64Literal k => fun x => x = k

/-- Boolean complement program used by the literal template's fixed text. -/
def notProgram : List Op :=
  [.block [.lit (.bool false)], .block [.lit (.bool true)], .word 18]

/-- The fixed executable guard program for a template. It duplicates the runtime
argument, computes a rejection flag from the copy, and then branches. The leading
`dup` is why acceptance cannot disturb the candidate's input stack. -/
def guardProgram : GuardTemplate → List Op
  | .ltI64Max => [.word 0, .lit (.i64 i64max), .word 7]
  | .neI64Min => [.word 0, .lit (.i64 i64min), .word 7]
  | .eqI64Literal k => [.word 0, .lit (.i64 k), .word 7] ++ notProgram

/-- The executable guard's rejection flag: `true` means refuse to run the body.
This is the pure, total scalar applicability check of the selected profile. -/
def guardRejects : GuardTemplate → BitVec 64 → Bool
  | .ltI64Max => fun x => x == i64max
  | .neI64Min => fun x => x == i64min
  | .eqI64Literal k => fun x => x != k

/-- The rejection program: consume the argument and return the defined `Sum`
rejection `.inl (.i64 1)`. It never invokes the candidate. -/
def rejectionProgram : List Op := [.word 1, .lit (.i64 1), .word 15]

/-- Run the candidate on its original stack, then tag its top result. -/
def acceptProgram (subject : List Op) : List Op := [.block subject, .word 10, .word 16]

/-- Normal-form semantics of the wrapper. The equality template uses a boolean
complement here; the production source instead swaps the two branches. Their
execution equivalence is checked separately, not assumed from this definition. -/
def wrapperProgram (template : GuardTemplate) (subject : List Op) : List Op :=
  guardProgram template ++ [.block rejectionProgram, .block (acceptProgram subject), .word 18]

/-- The observable success postcondition includes the production `inr` tag. -/
def successPost (post : Stack → Stack → Prop) (before result : Stack) : Prop :=
  ∃ middle, post before middle ∧ Exec [.word 16] middle result

/-- Exact primitive structure of the source wrapper, after parsing its literal
and subject quotation. This does not postulate correctness of that parsing. -/
def sourceWrapperProgram (template : GuardTemplate) (subject : List Op) : List Op :=
  match template with
  | .ltI64Max => [.word 0, .lit (.i64 i64max), .word 7,
      .block rejectionProgram, .block (acceptProgram subject), .word 18]
  | .neI64Min => [.word 0, .lit (.i64 i64min), .word 7,
      .block rejectionProgram, .block (acceptProgram subject), .word 18]
  | .eqI64Literal k => [.word 0, .lit (.i64 k), .word 7,
      .block (acceptProgram subject), .block rejectionProgram, .word 18]

/-- The invocation wrapper consumes the already supplied program value. No
source reconstruction or replacement program occurs in these primitives. -/
def liveWrapperProgram (template : GuardTemplate) : List Op :=
  let reject := [.word 1, .word 1, .lit (.i64 1), .word 15]
  let accept := [.word 2, .word 10, .word 16]
  match template with
  | .ltI64Max => [.word 2, .word 0, .lit (.i64 i64max), .word 7,
      .block reject, .block accept, .word 18]
  | .neI64Min => [.word 2, .word 0, .lit (.i64 i64min), .word 7,
      .block reject, .block accept, .word 18]
  | .eqI64Literal k => [.word 2, .word 0, .lit (.i64 k), .word 7,
      .block accept, .block reject, .word 18]

/-- The guarded entry precondition of a checked wrapper: the stack carries the
actual `I64` argument on top, the template's exact precondition holds for it, and
the candidate's own precondition holds for the same stack. Acceptance never
alters that stack, so the candidate precondition is exactly the wrapper's. -/
def guardedPre (template : GuardTemplate) (argument : BitVec 64) (innerPre : Stack → Prop) :
    Stack → Prop :=
  fun before => ∃ tail, before = tail ++ [.i64 argument] ∧
    GuardPrecondition template argument ∧ innerPre before

/-- The boolean-complement program pushes the complement of its input flag. -/
@[simp] theorem notProgram_exec_iff (b : Bool) (s : Stack) (v : Value) (result : Stack) :
    Exec notProgram (s ++ [v, .bool b]) result ↔ result = s ++ [v, .bool (!b)] := by
  cases b <;> simp [notProgram, Exec, List.reverse_append]

/-- The `LtI64Max` template's fixed text computes its rejection flag. -/
@[simp] theorem guardProgram_ltI64Max_exec_iff (x : BitVec 64) (tail result : Stack) :
    Exec (guardProgram .ltI64Max) (tail ++ [.i64 x]) result ↔
      result = tail ++ [.i64 x, .bool (x == i64max)] := by
  simp [guardProgram, Exec, List.reverse_append]

/-- The `NeI64Min` template's fixed text computes its rejection flag. -/
@[simp] theorem guardProgram_neI64Min_exec_iff (x : BitVec 64) (tail result : Stack) :
    Exec (guardProgram .neI64Min) (tail ++ [.i64 x]) result ↔
      result = tail ++ [.i64 x, .bool (x == i64min)] := by
  simp [guardProgram, Exec, List.reverse_append]

/-- The `EqI64Literal` template family's fixed text computes its rejection flag. -/
theorem guardProgram_eqI64Literal_exec_iff (k x : BitVec 64) (tail result : Stack) :
    Exec (guardProgram (.eqI64Literal k)) (tail ++ [.i64 x]) result ↔
      result = tail ++ [.i64 x, .bool (!(x == k))] := by
  rw [guardProgram, exec_append_iff]
  constructor
  · rintro ⟨middle, hprefix, hcomplement⟩
    have hmiddle : middle = tail ++ [.i64 x, .bool (x == k)] := by
      simpa [Exec, List.reverse_append] using hprefix
    subst hmiddle
    exact (notProgram_exec_iff (x == k) tail (.i64 x) result).mp hcomplement
  · intro h
    refine ⟨tail ++ [.i64 x, .bool (x == k)], ?_, ?_⟩
    · simp [Exec, List.reverse_append]
    · exact (notProgram_exec_iff (x == k) tail (.i64 x) result).mpr h

/-- The guard program computes the rejection flag from a duplicated argument and
leaves the original argument untouched underneath it. -/
theorem guardProgram_exec_iff (template : GuardTemplate) (x : BitVec 64)
    (tail result : Stack) :
    Exec (guardProgram template) (tail ++ [.i64 x]) result ↔
      result = tail ++ [.i64 x, .bool (guardRejects template x)] := by
  cases template with
  | ltI64Max => rw [guardRejects]; exact guardProgram_ltI64Max_exec_iff x tail result
  | neI64Min => rw [guardRejects]; exact guardProgram_neI64Min_exec_iff x tail result
  | eqI64Literal k => rw [guardRejects]; exact guardProgram_eqI64Literal_exec_iff k x tail result

/-- The rejection program returns a defined `Sum` rejection. -/
@[simp] theorem rejectionProgram_exec_iff (x : BitVec 64) (tail result : Stack) :
    Exec rejectionProgram (tail ++ [.i64 x]) result ↔ result = tail ++ [.inl (.i64 1)] := by
  simp [rejectionProgram, Exec, List.reverse_append]

/-- Acceptance runs the candidate body unchanged, then tags its top output. -/
@[simp] theorem acceptProgram_exec_iff (subject : List Op) (x : BitVec 64)
    (tail result : Stack) :
    Exec (acceptProgram subject) (tail ++ [.i64 x]) result ↔
      ∃ middle, Exec subject (tail ++ [.i64 x]) middle ∧
        Exec [.word 16] middle result := by
  change Exec ([.block subject, .word 10] ++ [.word 16]) _ _ ↔ _
  rw [exec_append_iff]
  simp [Exec, List.reverse_append]

/-- Pushing the two branch programs and branching dispatches on the guard flag.
The branch body runs on the stack underneath the flag, with no other change. -/
theorem branch_exec_iff (accept reject : List Op) (b : Bool) (s : Stack) (v : Value)
    (result : Stack) :
    Exec [.block reject, .block accept, .word 18] (s ++ [v, .bool b]) result ↔
      (if b = true then Exec reject (s ++ [v]) result else Exec accept (s ++ [v]) result) := by
  cases b <;> simp [Exec, List.reverse_append]

/-- The whole wrapper's semantics. Acceptance runs the candidate on exactly the
original argument stack; rejection returns a defined `Sum` and requests no
candidate body. This is a normal-form program theorem; it does not assert a
parser, string renderer or Wasm lowering theorem. -/
theorem wrapperProgram_exec_iff (template : GuardTemplate) (subject : List Op)
    (x : BitVec 64) (tail result : Stack) :
    Exec (wrapperProgram template subject) (tail ++ [.i64 x]) result ↔
      (if guardRejects template x = true then result = tail ++ [.inl (.i64 1)]
       else ∃ middle, Exec subject (tail ++ [.i64 x]) middle ∧
         Exec [.word 16] middle result) := by
  rw [wrapperProgram, exec_append_iff]
  constructor
  · rintro ⟨middle, hguard, hrest⟩
    rw [guardProgram_exec_iff] at hguard
    subst hguard
    have hbranch := (branch_exec_iff (acceptProgram subject) rejectionProgram
      (guardRejects template x) tail (.i64 x) result).mp hrest
    by_cases hflag : guardRejects template x = true
    · rw [if_pos hflag] at hbranch ⊢
      exact (rejectionProgram_exec_iff x tail result).mp hbranch
    · rw [if_neg hflag] at hbranch ⊢
      exact (acceptProgram_exec_iff subject x tail result).mp hbranch
  · intro h
    refine ⟨_, (guardProgram_exec_iff template x tail _).mpr rfl, ?_⟩
    refine (branch_exec_iff (acceptProgram subject) rejectionProgram
      (guardRejects template x) tail (.i64 x) result).mpr ?_
    by_cases hflag : guardRejects template x = true
    · rw [if_pos hflag] at h ⊢
      exact (rejectionProgram_exec_iff x tail result).mpr h
    · rw [if_neg hflag] at h ⊢
      exact (acceptProgram_exec_iff subject x tail result).mpr h

/-- Rejection is decided purely by the flag: `true` refuses the body. -/
theorem wrapperProgram_rejects (template : GuardTemplate) (subject : List Op)
    (x : BitVec 64) (tail result : Stack) (h : guardRejects template x = true) :
    Exec (wrapperProgram template subject) (tail ++ [.i64 x]) result ↔
      result = tail ++ [.inl (.i64 1)] := by
  rw [wrapperProgram_exec_iff, if_pos h]

/-- Acceptance runs the candidate body on the original argument stack. -/
theorem wrapperProgram_accepts (template : GuardTemplate) (subject : List Op)
    (x : BitVec 64) (tail result : Stack) (h : guardRejects template x = false) :
    Exec (wrapperProgram template subject) (tail ++ [.i64 x]) result ↔
      ∃ middle, Exec subject (tail ++ [.i64 x]) middle ∧
        Exec [.word 16] middle result := by
  have hne : ¬ (guardRejects template x = true) := by rw [h]; decide
  rw [wrapperProgram_exec_iff, if_neg hne]

/-- Swapping the equality-template branches is exactly the rejection-flag
normal form. Both success tagging and rejection payload are preserved. -/
theorem sourceWrapperProgram_refines (template : GuardTemplate) (subject : List Op)
    (x : BitVec 64) (tail result : Stack) :
    Exec (sourceWrapperProgram template subject) (tail ++ [.i64 x]) result ↔
      Exec (wrapperProgram template subject) (tail ++ [.i64 x]) result := by
  cases template with
  | ltI64Max => rfl
  | neI64Min => rfl
  | eqI64Literal k =>
    by_cases h : x = k
    · have hflag : guardRejects (.eqI64Literal k) x = false := by simp [guardRejects, h]
      rw [wrapperProgram_accepts _ _ _ _ _ hflag]
      have sourceRun :
          Exec (sourceWrapperProgram (.eqI64Literal k) subject) (tail ++ [.i64 x]) result ↔
            Exec (acceptProgram subject) (tail ++ [.i64 x]) result := by
        simp [sourceWrapperProgram, Exec, List.reverse_append, h]
      exact sourceRun.trans (acceptProgram_exec_iff subject x tail result)
    · have hflag : guardRejects (.eqI64Literal k) x = true := by simp [guardRejects, h]
      have hb : (x == k) = false := beq_eq_false_iff_ne.mpr h
      rw [wrapperProgram_rejects _ _ _ _ _ hflag]
      simp [sourceWrapperProgram, Exec, rejectionProgram, List.reverse_append, hb]

/-- Live invocation is observationally the source wrapper around the exact
program supplied on the input stack, for every subject body and normal result. -/
theorem liveWrapperProgram_refines (template : GuardTemplate) (subject : List Op)
    (x : BitVec 64) (tail result : Stack) :
    Exec (liveWrapperProgram template) (tail ++ [.i64 x, .program subject]) result ↔
      Exec (sourceWrapperProgram template subject) (tail ++ [.i64 x]) result := by
  cases template with
  | ltI64Max =>
    cases h : (x == i64max) <;>
      simp [liveWrapperProgram, sourceWrapperProgram, Exec, acceptProgram,
        rejectionProgram, List.reverse_append, h]
  | neI64Min =>
    cases h : (x == i64min) <;>
      simp [liveWrapperProgram, sourceWrapperProgram, Exec, acceptProgram,
        rejectionProgram, List.reverse_append, h]
  | eqI64Literal k =>
    cases h : (x == k) <;>
      simp [liveWrapperProgram, sourceWrapperProgram, Exec, acceptProgram,
        rejectionProgram, List.reverse_append, h]

/-- Prechecked correspondence for the `LtI64Max` template: equality rejection
establishes the original signed-inequality precondition, not merely a predicate
with a similar name. -/
theorem guard_sound_ltI64Max (x : BitVec 64) (h : guardRejects .ltI64Max x = false) :
    GuardPrecondition .ltI64Max x := by
  have unequal : x.toInt ≠ i64max.toInt := BitVec.toInt_ne.mpr (beq_eq_false_iff_ne.mp h)
  have upper : x.toInt ≤ 9223372036854775807 := BitVec.toInt_le
  have maximum : i64max.toInt = 9223372036854775807 := by decide
  change x.toInt < i64max.toInt
  omega

/-- Prechecked correspondence for the `NeI64Min` template. -/
theorem guard_sound_neI64Min (x : BitVec 64) (h : guardRejects .neI64Min x = false) :
    GuardPrecondition .neI64Min x := beq_eq_false_iff_ne.mp h

/-- Prechecked correspondence for the `EqI64Literal` template family: acceptance
implies the argument equals the compile-time literal, and no other instance
certifies another. -/
theorem guard_sound_eqI64Literal (k x : BitVec 64)
    (h : guardRejects (.eqI64Literal k) x = false) :
    GuardPrecondition (.eqI64Literal k) x := bne_eq_false_iff_eq.mp h

/-- The dispatcher the core uses: every prechecked template's acceptance implies
its stated precondition. -/
theorem guard_sound (template : GuardTemplate) (x : BitVec 64)
    (h : guardRejects template x = false) : GuardPrecondition template x := by
  cases template with
  | ltI64Max => exact guard_sound_ltI64Max x h
  | neI64Min => exact guard_sound_neI64Min x h
  | eqI64Literal k => exact guard_sound_eqI64Literal k x h

/-- VC-USE-01: a positive applicability check establishes the exact stated
precondition for the actual input, and the candidate then runs on the untouched
argument stack. A false guard fails closed with a defined `Sum`, and no
candidate-body request is made. This is the theorem the generated wrapper's
semantics discharge; total correctness is not claimed. -/
theorem guard_correspondence (template : GuardTemplate) (subject : List Op)
    (x : BitVec 64) (tail result : Stack) (h : guardRejects template x = false)
    (hr : Exec (wrapperProgram template subject) (tail ++ [.i64 x]) result) :
    GuardPrecondition template x ∧
      ∃ middle, Exec subject (tail ++ [.i64 x]) middle ∧ Exec [.word 16] middle result :=
  ⟨guard_sound template x h, (wrapperProgram_accepts template subject x tail result h).mp hr⟩

/-- The wrapping `I64` boundary case at `i64::MAX`. Incrementing the maximum
wraps to the minimum, so the `LtI64Max` guard must reject it and its precondition
must fail. This is the exact case a non-wrapping comparison would hide. -/
theorem ltI64Max_boundary :
    guardRejects .ltI64Max i64max = true ∧ ¬ GuardPrecondition .ltI64Max i64max
      ∧ i64max + 1 = i64min := by
  exact ⟨by decide, by change ¬ i64max.toInt < i64max.toInt; omega, by decide⟩

/-- The maximum really is the signed bound, not an off-by-one shift. -/
theorem i64max_toInt : i64max.toInt = 9223372036854775807 := by decide

/-- `NeI64Min` accepts the wrapping maximum, so the two templates genuinely
differ (a template cannot stand in for another). -/
theorem neI64Min_accepts_max : guardRejects .neI64Min i64max = false := by decide

end NobleCertificates
