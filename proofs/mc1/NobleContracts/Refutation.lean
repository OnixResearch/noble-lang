import NobleContracts.Obligation
import NobleContracts.Expression

/-!
Tactic-free refutation template for unary-`I64` MC1 contracts.

An MC1 contract claim is a *universal* statement over every typed input and
every whole-stack prefix (`exportedClaim`). A refutation is therefore one
concrete normal-return execution on one concrete input whose final stack
violates the postcondition. The host supplies the input; the kernel checks the
execution and the violated predicate. Nothing in this module searches for a
counterexample, reflects on decidable goals, or invokes a tactic.

The template is derived from the reviewed model only: `exportedClaim` (the
public boundary), `Exec` (normal-return execution), and `Holds`/`evaluate`
(assertion semantics over BitVec-64 wrapping arithmetic). It adds no axiom and
no new trusted component. Two reviewed instances are included: the
`monotonic` fixture at the positive signed boundary, and a wrapping program
constant that turns the smallest positive input into the minimum value.

Scope, stated explicitly: partial correctness on *normal returns* only. A
trap, a non-terminating program, an effectful word, or a compiler/runtime
disagreement is outside what this template can refute, and a compiled
observation is separate evidence (`observed-execution`), never a premise here.
-/

namespace NobleContracts

/-! ## Kernel-checked evaluation helpers

These small lemmas turn a computed `false` evaluation into the negation of
`Holds`. They are stated over the definitions, so an instance needs no tactic:
`Holds` and `evaluate` reduce definitionally on concrete values, and only the
final `Option`/`Value` constructor conflict needs a proof. -/

/-- The two `Bool` constructors cannot be equal. -/
theorem value_bool_false_ne_true : ¬ (Value.bool false = Value.bool true) :=
  fun equal => Eq.mp Bool.false_eq_true (Eq.mp (Value.bool.injEq false true) equal)

/-- `some` preserves the `Bool` constructor conflict. -/
theorem some_value_bool_false_ne_true :
    ¬ (some (Value.bool false) = some (Value.bool true)) :=
  fun equal => value_bool_false_ne_true (Option.some.inj equal)

/-- A concrete term that evaluates to `(bool false)` does not hold. The
    evaluation equation is the instance's computed input/output; it is not a
    proof search or a reflection oracle. -/
theorem not_holds_of_evaluate_false {term : Term} {before after params : Stack}
    (evaluated : evaluate term ⟨before, after, params⟩ = some (Value.bool false)) :
    ¬ Holds term before after params :=
  fun holds => some_value_bool_false_ne_true (Eq.trans (Eq.symm evaluated) holds)

/-! ## The template -/

/-- A concrete unary-`I64` counterexample refutes the exported claim.

`run` is the *checked* normal-return execution of the contract's exact code on
the exact input `input` and output `output`; `preHolds` is the asserted
precondition at that input; `postFails` is the violated postcondition at the
same observations. No other premise is needed: the claim's quantifier is
instantiated at the empty prefix, the single typed input, and no parameters,
and the existential suffix it demands is the concrete output stack.

The proof is a term: the claim is applied once to the concrete execution, and
the resulting `Holds post` is contradicted by `postFails` after the claim's own
suffix equation identifies the demanded suffix with the concrete output. -/
theorem exportedClaim_unaryI64_refutation {code : List Op} {pre post : Term}
    {input output : BitVec 64}
    (run : Exec code [.i64 input] [.i64 output])
    (preHolds : Holds pre [.i64 input] [] [])
    (postFails : ¬ Holds post [.i64 input] [.i64 output] []) :
    ¬ exportedClaim code [.i64] [.i64] [] (Holds pre) (Holds post) :=
  fun claim =>
    match claim [] [.i64 input] [] (StackTyped.cons HasType.i64 StackTyped.nil)
        StackTyped.nil preHolds [.i64 output] run with
    | ⟨_after, hfinal, _, hpost⟩ => (hfinal ▸ postFails) hpost

/-! ## Instance: the `monotonic` fixture

`verification/mc1/monotonic.noble-contract` declares `[ 1 + ]` with
`(ensures (lt (in x) (out y)))`. At the maximum signed input the wrapping add
returns the minimum signed value, so `(lt (in x) (out y))` is false. The program
list and the postcondition term below are the ones the frontend emits for that
fixture (`program`, `precondition` and `postcondition` in the generated
`MC1Obligation` module are transparent definitions of these values). -/

/-- The exported `program` of the `monotonic` fixture: the word list `[ 1 + ]`. -/
def monotonic_program : List Op := [.lit (.i64 (BitVec.ofInt 64 1)), .word 4]

/-- The counterexample input: the maximum signed 64-bit value. -/
def monotonic_input : BitVec 64 := BitVec.ofInt 64 9223372036854775807

/-- The observed output: wrapping `input + 1`, i.e. the minimum signed value. -/
def monotonic_output : BitVec 64 := monotonic_input + BitVec.ofInt 64 1

/-- The concrete normal-return execution, built from the reviewed primitive
    rules (`exec_lit_iff` then `exec_add_iff`) and their sequencing. -/
theorem monotonic_counterexample_run :
    Exec monotonic_program [.i64 monotonic_input] [.i64 monotonic_output] :=
  exec_append_iff.mpr
    ⟨[.i64 monotonic_input, .i64 (BitVec.ofInt 64 1)],
      exec_lit_iff (v := .i64 (BitVec.ofInt 64 1)) (s := [.i64 monotonic_input]) |>.mpr rfl,
      exec_add_iff (a := monotonic_input) (b := BitVec.ofInt 64 1) (s := []) |>.mpr rfl⟩

/-- At the boundary the postcondition `(lt (in x) (out y))` evaluates to false:
    the maximum signed value is not below the minimum signed value. -/
theorem monotonic_postcondition_false :
    ¬ Holds (.lt (.input 0) (.output 0)) [.i64 monotonic_input] [.i64 monotonic_output] [] :=
  not_holds_of_evaluate_false rfl

/-- The `monotonic` fixture's exported claim is refuted at the maximum input. -/
theorem monotonic_refutation :
    ¬ exportedClaim monotonic_program [.i64] [.i64] []
        (Holds (.bool true)) (Holds (.lt (.input 0) (.output 0))) :=
  exportedClaim_unaryI64_refutation monotonic_counterexample_run
    (holds_true [.i64 monotonic_input] [] []) monotonic_postcondition_false

/-! ## Instance: a wrapping program constant

The same postcondition with a different unary program, `[ 9223372036854775807 + ]`,
at input `1`. The program constant and the input are both boundary values, and
their wrapping sum is the minimum signed value, so the claim that the result is
strictly above the input fails. This instance shows the template is not tied to
`[ 1 + ]` and that program literals participate in the same wrapping arithmetic
as inputs. -/

/-- The exported `program`: the word list `[ 9223372036854775807 + ]`. -/
def wrap_constant_program : List Op :=
  [.lit (.i64 (BitVec.ofInt 64 9223372036854775807)), .word 4]

/-- The counterexample input: the smallest positive signed value. -/
def wrap_constant_input : BitVec 64 := BitVec.ofInt 64 1

/-- The observed output: wrapping `input + 9223372036854775807`, the minimum
    signed value. -/
def wrap_constant_output : BitVec 64 :=
  wrap_constant_input + BitVec.ofInt 64 9223372036854775807

/-- The concrete normal-return execution, built from the reviewed primitive
    rules and their sequencing. -/
theorem wrap_constant_counterexample_run :
    Exec wrap_constant_program [.i64 wrap_constant_input] [.i64 wrap_constant_output] :=
  exec_append_iff.mpr
    ⟨[.i64 wrap_constant_input, .i64 (BitVec.ofInt 64 9223372036854775807)],
      exec_lit_iff (v := .i64 (BitVec.ofInt 64 9223372036854775807))
        (s := [.i64 wrap_constant_input]) |>.mpr rfl,
      exec_add_iff (a := wrap_constant_input) (b := BitVec.ofInt 64 9223372036854775807)
        (s := []) |>.mpr rfl⟩

/-- At `1` the postcondition `(lt (in x) (out y))` evaluates to false: the
    minimum signed value is not above `1`. -/
theorem wrap_constant_postcondition_false :
    ¬ Holds (.lt (.input 0) (.output 0)) [.i64 wrap_constant_input]
        [.i64 wrap_constant_output] [] :=
  not_holds_of_evaluate_false rfl

/-- The wrapping-program claim is refuted at the smallest positive input. -/
theorem wrap_constant_refutation :
    ¬ exportedClaim wrap_constant_program [.i64] [.i64] []
        (Holds (.bool true)) (Holds (.lt (.input 0) (.output 0))) :=
  exportedClaim_unaryI64_refutation wrap_constant_counterexample_run
    (holds_true [.i64 wrap_constant_input] [] []) wrap_constant_postcondition_false

end NobleContracts
