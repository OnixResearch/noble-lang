import NobleM2.SizeChangeClosure

/-!
# The checker arm: argument classification to call graphs (issue #101)

The models so far take their graphs as input. This module derives them from the arguments a
definition passes at each of its self-call sites, and proves the derivation sound against the
arguments' semantics.

The argument grammar is deliberately small — the forms a bounded checker can classify without
evaluating arbitrary expressions: the measure itself, its predecessor, a reset to zero, and
anything opaque. The soundness claims are only what the classifier proves: an `equal` really
passes the same value, and a `decrease` really drops it **given the site's positivity guard**.
`unknown` claims nothing, which is why an opaque argument is refused rather than trusted.

This is not a parser or an evaluator for arbitrary source: the grammar, the guard and the call
graph are the interface a front end has to fill. Under that interface the decision the criterion
consumes is exactly the classification of the arguments.
-/

namespace NobleM2.SizeChange

/-- What a self-call passes for one measure. -/
inductive Arg where
  /-- The measure itself. -/
  | same
  /-- The measure minus one. -/
  | predOfSame
  /-- A reset, e.g. zero. -/
  | reset
  /-- Anything the bounded checker cannot classify. -/
  | opaque

/-- The relation a classified argument declares. -/
def Arg.classify : Arg → Rel
  | .same => .equal
  | .predOfSame => .decrease
  | .reset => .unknown
  | .opaque => .unknown

/-- What the argument passes for the measure, given the measure's current value. -/
def Arg.eval (value : Nat) : Arg → Nat
  | .same => value
  | .predOfSame => value - 1
  | .reset => 0
  | .opaque => value

/-- An `equal` classification is exact: the argument passes the same value back. -/
theorem classify_equal_sound {value : Nat} {arg : Arg} (h : arg.classify = .equal) :
    arg.eval value = value := by
  cases arg <;> simp_all [Arg.classify, Arg.eval]

/-- A `decrease` classification drops the measure, given the site's positivity guard. -/
theorem classify_decrease_sound {value : Nat} {arg : Arg} (h : arg.classify = .decrease)
    (pos : 0 < value) : arg.eval value < value := by
  cases arg <;> simp_all [Arg.classify, Arg.eval] <;> omega

/-- The arguments a call site passes, one per measure. -/
abbrev Args (n : Nat) := Fin n → Arg

/-- The call graph the checker derives from the arguments. -/
def graphOf {n : Nat} (args : Args n) : Graph n := fun i => (args i).classify

/-- The values the call passes, given the current state. -/
def evalArgs {n : Nat} (args : Args n) (state : Fin n → Nat) : Fin n → Nat :=
  fun i => (args i).eval (state i)

/-- A `decrease` in the derived graph is a real drop at that measure. -/
theorem graphOf_decrease_sound {n : Nat} {args : Args n} {state : Fin n → Nat} {i : Fin n}
    (h : graphOf args i = .decrease) (pos : 0 < state i) :
    evalArgs args state i < state i :=
  classify_decrease_sound h pos

/-- An `equal` in the derived graph passes the value through. -/
theorem graphOf_equal_sound {n : Nat} {args : Args n} {state : Fin n → Nat} {i : Fin n}
    (h : graphOf args i = .equal) : evalArgs args state i = state i :=
  classify_equal_sound h

/-- Arguments that classify as decreasing at one measure make the criterion accept the family. -/
theorem checked_family_certified {k n : Nat} {argsOf : Fin k → Args n} {i : Fin n}
    (h : ∀ s, (argsOf s) i = .predOfSame) :
    certifiedAll (fun s => graphOf (argsOf s)) = true := by
  unfold certifiedAll
  exact List.any_eq_true.mpr ⟨i, List.mem_finRange i,
    List.all_eq_true.mpr (fun s _ => by simp [graphOf, Arg.classify, h s])⟩

/-- **End to end.** Checked arguments, the shared criterion and the run model compose: a family
whose arguments all classify as decreasing at one measure admits no infinite run. -/
theorem checked_family_terminates {k n : Nat} {argsOf : Fin k → Args n} {i : Fin n}
    (h : ∀ s, (argsOf s) i = .predOfSame) :
    ∀ _run : Run k n (fun s => graphOf (argsOf s)), False :=
  certifiedAll_terminates (checked_family_certified h)

/-! ## Fixtures -/

/-- The classic tail call: measure `0` is passed as its predecessor. -/
def tailArgs : Args 2 := fun i => if i = 0 then .predOfSame else .opaque

/-- The graph the checker derives: decreasing at `0`, nothing at `1`. -/
theorem tailArgs_graph : graphOf tailArgs 0 = .decrease ∧ graphOf tailArgs 1 = .unknown := by
  constructor <;> simp [graphOf, tailArgs, Arg.classify]

/-- The criterion accepts the family derived from tail calls. -/
theorem tailArgs_certified : certifiedAll (fun (_ : Fin 1) => graphOf tailArgs) = true :=
  checked_family_certified (k := 1) (n := 2) (argsOf := fun _ => tailArgs) (i := 0)
    (fun _ => by simp [tailArgs])

/-- A call that passes the measure back unchanged is classified `equal`, not `decrease`. -/
def identityArgs : Args 1 := fun _ => .same

theorem identityArgs_classified : graphOf identityArgs 0 = .equal := rfl

theorem identityArgs_not_decreasing : ¬ ∃ i, graphOf identityArgs i = .decrease := by
  rintro ⟨i, hi⟩
  have : i = 0 := Subsingleton.elim i 0
  subst this
  simp [graphOf, identityArgs, Arg.classify] at hi

end NobleM2.SizeChange
