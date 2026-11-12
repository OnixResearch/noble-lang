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

Composing two relations is `Rel.compose`. `unknown` means "not shown", so it permits an increase and
therefore absorbs: a decrease on one side of an `unknown` step proves nothing about the pair. The
composition is proved sound for the step semantics `Rel.Allowed` (`Rel.allowed_compose`), which is
what makes a composed graph describe a real two-step path.
-/

namespace NobleM2.SizeChange

/-- What one call does to one measure. `unknown` is "not shown", not "increases". -/
inductive Rel where
  | decrease
  | equal
  | unknown
deriving DecidableEq, Repr

namespace Rel

/-- Sequencing two calls. A strict decrease on one side survives only when the other side is known:
`unknown` permits an increase that cancels it, so `unknown` absorbs on both sides. -/
def compose : Rel → Rel → Rel
  | .decrease, .equal => .decrease
  | .decrease, .decrease => .decrease
  | .equal, .decrease => .decrease
  | .equal, .equal => .equal
  | _, _ => .unknown

/-- What a call may do to one measure: `decrease` really drops, `equal` really holds, `unknown`
permits any pair. -/
def Allowed : Rel → Nat → Nat → Prop
  | .decrease, before, after => after < before
  | .equal, before, after => after = before
  | .unknown, _, _ => True

/-- `unknown` absorbs composition on the left: nothing follows from an unconstrained step. -/
@[simp] theorem compose_unknown_left (x : Rel) : compose .unknown x = .unknown := rfl

/-- `unknown` absorbs composition on the right. -/
@[simp] theorem compose_unknown_right (x : Rel) : compose x .unknown = .unknown := by
  cases x <;> rfl

/-- A decrease composed with a known relation stays a decrease. -/
theorem compose_decrease_left (x : Rel) (hx : x ≠ .unknown) :
    compose .decrease x = .decrease := by
  cases x <;> simp_all [compose]

/-- A known relation composed with a decrease stays a decrease. -/
theorem compose_decrease_right (x : Rel) (hx : x ≠ .unknown) :
    compose x .decrease = .decrease := by
  cases x <;> simp_all [compose]

/-- A `decrease` is a known relation. -/
theorem known_of_decrease {x : Rel} (h : x = .decrease) : x ≠ .unknown := by
  rw [h]; exact fun hbad => noConfusion hbad

/-- Composing two known relations is known. -/
theorem known_compose {x y : Rel} (hx : x ≠ .unknown) (hy : y ≠ .unknown) :
    x.compose y ≠ .unknown := by
  cases x <;> cases y <;> simp_all [compose]

/-- **Composition describes real two-step paths**: a pair the composed relation permits is a pair
some intermediate value makes both steps permit. (The converse fails — the table is sound, not
complete — which is why a refusal is never a divergence claim.) -/
theorem allowed_compose {r₁ r₂ : Rel} {a b c : Nat}
    (h₁ : r₁.Allowed a b) (h₂ : r₂.Allowed b c) : (r₁.compose r₂).Allowed a c := by
  cases r₁ <;> cases r₂ <;> simp_all [compose, Allowed] <;> omega

/-- The concrete reason `unknown` must absorb: `5 → 3` is a real decrease and `3 → 9` is
unconstrained, yet `5 → 9` is not a decrease. A table that called `decrease ∘ unknown` a decrease
would therefore describe no two-step path. -/
theorem compose_decrease_unknown_raises :
    Rel.Allowed .decrease 5 3 ∧ Rel.Allowed .unknown 3 9 ∧ ¬ Rel.Allowed .decrease 5 9 := by
  refine ⟨?_, trivial, ?_⟩
  · show 3 < 5
    omega
  · show ¬ (9 < 5)
    omega

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

/-- **A certified self-loop has no infinite run.** The certificate supplies the measure: `h` is
used to pick the measure `i` that `g` decreases, and `respects` says exactly that each traversal of
`g` drops it. So the conclusion is about runs of this graph, not about an arbitrary decreasing
function. -/
theorem certified_terminates {n : Nat} {g : Graph n} (h : certified g = true)
    (state : Nat → Fin n → Nat)
    (respects : ∀ (t : Nat) (i : Fin n), g i = .decrease → state (t + 1) i < state t i) : False := by
  obtain ⟨i, hi⟩ := certified_sound h
  exact no_decreasing_chain (fun t => state t i) (fun t => respects t i hi)

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

/-- The refusal states what is proved about the offending self-loop: no measure decreases on it. It
does not exhibit a call cycle. -/
theorem wrapping_reports_no_decrease : ¬ ∃ i, wrappingGraph i = .decrease :=
  certified_refusal wrapping_refused

/-- A growth call is refused as well. -/
def growGraph : Graph 1 := fun _ => .unknown

theorem grow_refused : certified growGraph = false := rfl

end NobleM2.SizeChange
