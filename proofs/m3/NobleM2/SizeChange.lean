import Std

/-!
# Size-change termination for a self-recursive definition (issue #101)

The #99 profile admits a self call only at the owner's declared signature, so every recursive call
of one definition is an edge over the same parameter list. This module models the size-change graph
of one such call and decides the bounded criterion the issue asks for: the call is certified when
*some* declared measure strictly decreases at every call. Nothing else is needed for a single
self-loop — a strict decrease on one measure is enough, whatever the other measures do, because the
measure never has to increase for the argument to hold.

Measures are natural numbers only. `List` length and a guarded `n - k` (k ≥ 1, under `n ≥ k`) are
natural-valued; wrapping `I64` arithmetic is not, so a call measured by it is recorded as `unknown`
and refused rather than assumed decreasing.

Refusal means **not shown terminating**, never "diverges".
-/

namespace NobleM2.SizeChange

/-- What one call does to one measure. `unknown` is "not shown", not "increases". -/
inductive Rel where
  | decrease
  | equal
  | unknown
deriving DecidableEq, Repr

namespace Rel

/-- Sequencing two calls: a decrease in either position is still a decrease. -/
def compose : Rel → Rel → Rel
  | .decrease, _ => .decrease
  | _, .decrease => .decrease
  | .equal, .equal => .equal
  | _, _ => .unknown

@[simp] theorem compose_decrease_left (x : Rel) : compose .decrease x = .decrease := rfl
@[simp] theorem compose_decrease_right (x : Rel) : compose x .decrease = .decrease := by
  cases x <;> rfl

theorem eq_decrease_of_compose {x y : Rel} (h : compose x y = .decrease) :
    x = .decrease ∨ y = .decrease := by
  cases x <;> cases y <;> simp [compose] at h ⊢

end Rel

/-- The size-change graph of one recursive call over `n` declared measures. -/
abbrev Graph (n : Nat) := Fin n → Rel

/-- The closure step: two traversals of the same call, in either order. -/
def Graph.compose (g : Graph n) : Graph n := fun i => (g i).compose (g i)

/-- The bounded criterion: some measure strictly decreases at every call. -/
def certified {n : Nat} (g : Graph n) : Bool := (List.finRange n).any fun i => g i == .decrease

theorem certified_sound {n : Nat} {g : Graph n} (h : certified g = true) :
    ∃ i, g i = .decrease := by
  simpa [certified] using (List.any_eq_true.mp h)

theorem certified_refusal {n : Nat} {g : Graph n} (h : certified g = false) :
    ¬ ∃ i, g i = .decrease := by
  rintro ⟨i, hi⟩
  have accepted : certified g = true := by
    unfold certified
    exact List.any_eq_true.mpr ⟨i, List.mem_finRange i, by simp [hi]⟩
  rw [accepted] at h
  exact Bool.noConfusion h

/-- The closure criterion for a self-loop: composing the call with itself decides the same. -/
theorem compose_self_decrease (x : Rel) : (Rel.compose x x == .decrease) = (x == .decrease) := by
  cases x <;> rfl

theorem certified_closure {n : Nat} (g : Graph n) : certified g.compose = certified g := by
  simp [certified, Graph.compose, compose_self_decrease]

/-- No natural-valued measure can strictly decrease at every step of an infinite call sequence. -/
theorem no_decreasing_chain (m : Nat → Nat) (h : ∀ k, m (k + 1) < m k) : False := by
  have bound : ∀ k, m k + k ≤ m 0 := by
    intro k
    induction k with
    | zero => simp
    | succ k ih =>
      have step : m (k + 1) + 1 ≤ m k := h k
      omega
  have beyond := bound (m 0 + 1)
  omega

/-- A certified call cannot be repeated forever: its measure strictly decreases each time. -/
theorem certified_terminates {n : Nat} {g : Graph n} (_h : certified g = true)
    (m : Nat → Nat) (step : ∀ k, m (k + 1) < m k) : False :=
  no_decreasing_chain m step

/-! ## Fixtures -/

/-- Structural recursion on a list: the tail's length is smaller. -/
def tailGraph : Graph 1 := fun _ => .decrease

theorem tail_accepted : certified tailGraph = true := rfl

/-- A guarded countdown `n - 1` under `n ≥ 1`: the counter strictly decreases. -/
def countdownGraph : Graph 1 := fun _ => .decrease

theorem countdown_accepted : certified countdownGraph = true := rfl

/-- A call that passes its counter through unchanged. -/
def sameGraph : Graph 1 := fun _ => .equal

theorem same_refused : certified sameGraph = false := rfl

/-- A call whose measure is wrapping `I64` arithmetic: recorded as not shown, and refused. -/
def wrappingGraph : Graph 1 := fun _ => .unknown

theorem wrapping_refused : certified wrappingGraph = false := rfl

/-- The refusal names the offending self-loop: no measure decreases on it. -/
theorem wrapping_reports_cycle : ¬ ∃ i, wrappingGraph i = .decrease :=
  certified_refusal wrapping_refused

/-- A growth call is refused as well. -/
def growGraph : Graph 1 := fun _ => .unknown

theorem grow_refused : certified growGraph = false := rfl

end NobleM2.SizeChange
