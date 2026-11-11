import NobleM2.SizeChangeCriterion

/-!
# Conformance fixtures (issue #101)

The issue names four cases for its conformance criterion. Each is a named fixture with exactly the
required verdict, so the checker's behaviour is a theorem rather than a sample run.

| Fixture | Stated at | Verdict |
|---|---|---|
| structural list recursion | argument layer, passed as its predecessor | accepted |
| countdown under a guard | relation layer, strictly decreasing | accepted |
| non-decreasing self-call | relation layer, passed through | refused, cycle named |
| wrapping `I64` arithmetic as a measure | argument layer, unclassifiable | refused, cycle named |

Wrapping arithmetic may *increase* a value, so accepting it would be unsound; the checker classifies
the argument as `unknown` and refuses. Both refusals are `certifiedAll_refusal` instances, so the
offending cycle is named rather than the verdict being a bare `false`.
-/

namespace NobleM2.SizeChange

/-- **Fixture 1**: structural list recursion - the length passed as its predecessor, at one site. -/
def listRecursionArgs : Fin 1 → Args 2 := fun _ i => if i = 0 then .predOfSame else .opaque

theorem listRecursionFixture_accepted :
    certifiedAll (fun s => graphOf (listRecursionArgs s)) = true :=
  checked_family_certified (fun _ => by
    show listRecursionArgs _ 0 = .predOfSame
    simp [listRecursionArgs])

/-- **Fixture 2**: a countdown counter under a guard. -/
def countdownFixture : Sites 1 1 := fun _ _ => .decrease

theorem countdownFixture_accepted : certifiedAll countdownFixture = true := by
  unfold certifiedAll
  exact List.any_eq_true.mpr ⟨0, List.mem_finRange 0,
    List.all_eq_true.mpr (fun _ _ => by simp [countdownFixture])⟩

/-- **Fixture 3**: a non-decreasing self-call. -/
def nonDecreasingFixture : Sites 1 1 := fun _ _ => .equal

theorem nonDecreasingFixture_refused : certifiedAll nonDecreasingFixture = false := by decide

/-- Its refusal names the cycle: no measure decreases at every site. -/
theorem nonDecreasingFixture_cycle :
    ¬ ∃ i, ∀ s, nonDecreasingFixture s i = .decrease :=
  certifiedAll_refusal nonDecreasingFixture_refused

/-- **Fixture 4**: wrapping `I64` arithmetic used as a measure - unclassifiable at the argument
layer. -/
def wrappingArgs : Fin 1 → Args 1 := fun _ _ => .opaque

theorem wrappingFixture_refused :
    certifiedAll (fun s => graphOf (wrappingArgs s)) = false := by
  cases h : certifiedAll (fun s => graphOf (wrappingArgs s)) with
  | false => rfl
  | true =>
      obtain ⟨i, hi⟩ := certifiedAll_sound h
      have := hi 0
      simp [graphOf, wrappingArgs, Arg.classify] at this

/-- Its refusal names the cycle as well. -/
theorem wrappingFixture_cycle :
    ¬ ∃ i, ∀ s, graphOf (wrappingArgs s) i = .decrease :=
  certifiedAll_refusal wrappingFixture_refused

end NobleM2.SizeChange
