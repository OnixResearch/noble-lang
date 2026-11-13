import NobleM2.SizeChangeChecker

/-!
# Guarded execution at the argument layer (issue #101)

`SizeChangeChecker` classifies a self-call's argument and proves that a `decrease` classification
drops the measure **given the site's positivity guard** `0 < value`. `SizeChangeSites` then decides
a site family and rules out infinite runs that respect the classified relations, but its `Run` has
no guard field: it never checks that a site is taken only while its measure is positive. This module
closes that gap on the execution side.

A `GuardedRun` is the actual argument evaluation: each step replaces the state by the values the
chosen site's arguments compute, and taking a site whose argument is the predecessor requires that
measure to be positive *before* the step. No other assumption is made about the execution.

The main result is `guarded_terminates`: a family the criterion accepts admits no `GuardedRun`.
Unlike `checked_family_terminates`, it does not take the classified relations as an assumption —
they are derived from the arguments by `graphOf_decrease_sound`, and the positivity premise the
classifier needs is exactly the run's `positive` field. `GuardedRun.toRun` also exhibits the bridge
back to the abstract model, so the two models are proved to speak about the same run.

The countdown fixture is restated here at the argument layer **with** its guard, and
`countdown_guard_necessary` shows why the premise is not decoration: at zero the predecessor
argument does not drop the measure.
-/

namespace NobleM2.SizeChange

/-- A guarded run of one site family: the states are the actual argument values, and a site whose
argument is the measure's predecessor is taken only while that measure is positive. -/
structure GuardedRun (k n : Nat) (argsOf : Fin k → Args n) where
  /-- The site chosen at step `t`. -/
  choose : Nat → Fin k
  /-- The measure vector before step `t`. -/
  state : Nat → Fin n → Nat
  /-- One step is the chosen site's argument evaluation, not an abstract relation. -/
  step : ∀ t, state (t + 1) = evalArgs (argsOf (choose t)) (state t)
  /-- The positivity guard the classifier's `decrease` claim needs, per step and measure. -/
  positive : ∀ (t : Nat) (i : Fin n), (argsOf (choose t)) i = .predOfSame → 0 < state t i

/-- The only argument a `decrease` classification admits is the predecessor. -/
theorem arg_pred_of_classify_decrease {arg : Arg} (h : arg.classify = .decrease) :
    arg = .predOfSame := by
  cases arg <;> simp_all [Arg.classify]

/-- A relational `decrease` at a site is a predecessor argument there. -/
theorem arg_ne_pred_of_graph_ne_decrease {n : Nat} {args : Args n} {i : Fin n}
    (h : graphOf args i ≠ .decrease) : args i ≠ .predOfSame := by
  intro same
  apply h
  simp [graphOf, same, Arg.classify]

/-- Under a uniform predecessor site family the selected measure really drops at every step. -/
theorem guarded_run_drops {k n : Nat} {argsOf : Fin k → Args n} {i : Fin n}
    (h : ∀ s, (argsOf s) i = .predOfSame) (run : GuardedRun k n argsOf) (t : Nat) :
    run.state (t + 1) i < run.state t i := by
  have classified : graphOf (argsOf (run.choose t)) i = .decrease := by
    simp [graphOf, Arg.classify, h]
  have drops := graphOf_decrease_sound (args := argsOf (run.choose t)) (state := run.state t)
    (i := i) classified (run.positive t i (h (run.choose t)))
  rw [run.step t]
  exact drops

/-- A guarded run is a run of the classified relations. This is the bridge to `SizeChangeSites`:
the abstract model needs no guard because the relations are assumed there, and this theorem shows
the guarded execution supplies them. -/
def GuardedRun.toRun {k n : Nat} {argsOf : Fin k → Args n} (run : GuardedRun k n argsOf) :
    Run k n (fun s => graphOf (argsOf s)) where
  choose := run.choose
  state := run.state
  respects := by
    intro t i classified
    have pred : (argsOf (run.choose t)) i = .predOfSame := arg_pred_of_classify_decrease classified
    have drops := graphOf_decrease_sound (args := argsOf (run.choose t)) (state := run.state t)
      (i := i) classified (run.positive t i pred)
    rw [run.step t]
    exact drops

/-- **A family the criterion accepts admits no guarded run.** The criterion supplies the shared
measure, the classifier's soundness supplies the drop, and the run's positivity premise is the
guard the classifier requires. -/
theorem guarded_terminates {k n : Nat} {argsOf : Fin k → Args n}
    (h : certifiedAll (fun s => graphOf (argsOf s)) = true) :
    ∀ _run : GuardedRun k n argsOf, False := by
  intro run
  obtain ⟨i, hi⟩ := certifiedAll_sound h
  exact no_decreasing_chain (fun t => run.state t i) (fun t => by
    have pred : (argsOf (run.choose t)) i = .predOfSame :=
      arg_pred_of_classify_decrease (hi (run.choose t))
    have drops := graphOf_decrease_sound (args := argsOf (run.choose t)) (state := run.state t)
      (i := i) (by simp [graphOf, Arg.classify, pred]) (run.positive t i pred)
    rw [run.step t]
    exact drops)

/-- The classified form the issue asks for: checked arguments that all pass the predecessor at one
measure admit no guarded run. It is the criterion's own acceptance test
(`checked_family_certified`) fed to `guarded_terminates`. -/
theorem guarded_checked_family_terminates {k n : Nat} {argsOf : Fin k → Args n} {i : Fin n}
    (h : ∀ s, (argsOf s) i = .predOfSame) :
    ∀ _run : GuardedRun k n argsOf, False :=
  guarded_terminates (checked_family_certified h)

/-- The abstract criterion and the guarded execution agree: an accepted family's `toRun`
contradicts `certifiedAll_terminates` too, so the bridge is not a weaker statement. -/
theorem guarded_bridge_terminates {k n : Nat} {argsOf : Fin k → Args n}
    (h : certifiedAll (fun s => graphOf (argsOf s)) = true)
    (run : GuardedRun k n argsOf) : False :=
  certifiedAll_terminates h run.toRun

/-! ## Fixtures -/

/-- A guarded countdown at the argument layer: the single measure is passed as its predecessor. -/
def countdownArgs : Args 1 := fun _ => .predOfSame

/-- The predecessor argument drops the measure exactly while the guard holds. -/
theorem countdownArgs_drops (state : Fin 1 → Nat) (h : 0 < state 0) :
    evalArgs countdownArgs state 0 < state 0 :=
  graphOf_decrease_sound (by simp [graphOf, countdownArgs, Arg.classify]) h

/-- With its guard, the countdown has no guarded run. -/
theorem countdown_no_guarded_run : ∀ _run : GuardedRun 1 1 (fun _ => countdownArgs), False :=
  guarded_checked_family_terminates (i := 0) (fun _ => rfl)

/-- Without the guard the classification does not drop the measure. This is why the accepted
countdown fixture needs `0 <` and not a zero test: at zero the predecessor argument is not a
decrease. -/
theorem countdown_guard_necessary :
    evalArgs countdownArgs (fun _ => 0) 0 = 0 ∧ ¬ evalArgs countdownArgs (fun _ => 0) 0 < 0 := by
  constructor
  · simp [evalArgs, countdownArgs, Arg.eval]
  · simp [evalArgs, countdownArgs, Arg.eval]

/-- A pass-through argument at the argument layer: refused, not certified. -/
def staticArgs : Args 1 := fun _ => .same

/-- The pass-through family is not certified. -/
theorem staticArgs_not_certified : certifiedAll (fun (_ : Fin 1) => graphOf staticArgs) = false := by
  decide

/-- A refused argument family has no shared measure, so no certificate and no guarded-run
theorem. -/
theorem staticArgs_no_shared :
    ¬ ∃ i, ∀ _s : Fin 1, graphOf staticArgs i = .decrease :=
  certifiedAll_refusal staticArgs_not_certified

end NobleM2.SizeChange
