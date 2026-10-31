/-
Scope and code discipline for the `implicit-args/v1` constraint algebra
(`implicit-proof-arguments`, task 13, Lean half: the second part).

The unification solver of `NobleFoundation.ImplicitUnification` decides which
answer solves the constraint set. That is not enough: the design's rule 7
also refuses an answer whose bindings would capture a binder, and it refuses
a value meta bound to a level whose code is not exactly the meta's own. This
file models those two refusals and proves why they belong at solution time:

- a level occurrence is rigid: substitution replaces metavariables only, so
  it never retargets a level, and the levels of an instantiation come from
  the original term and from the bindings it actually uses;
- hence a ground binding that mentions a level at or beyond its meta's depth
  keeps every instantiation ill-scoped, and a value meta bound to a level of
  the wrong code keeps every instantiation wrong-coded;
- hence no later elaboration step can repair such a binding, so refusing it
  when it is produced is the only sound point.

The model is deliberately small: metavariables, rigid symbols, level
occurrences and binary application. Source-to-Lean translation stays
V-INTRINSIC-01. Which premise positions the surface grammar may infer is a
policy decision; it is modelled here only to the extent that an accepted
premise argument is a ground explicit term whose witness no substitution can
retarget.
-/
import Mathlib.Data.Finset.Card

namespace NobleFoundation.ImplicitScope

/-- Terms with metavariables, rigid symbols and level occurrences. -/
inductive T where
  | mvar : Nat → T
  | sym : Nat → T
  | lvl : Nat → T
  | app : T → T → T
  deriving DecidableEq, Repr

/-- Apply a substitution to every metavariable. Level occurrences are rigid. -/
def T.subst (σ : Nat → T) : T → T
  | mvar n => σ n
  | sym s => sym s
  | lvl ℓ => lvl ℓ
  | app f a => app (f.subst σ) (a.subst σ)

/-- The metavariables of a term. -/
def T.vars : T → Finset Nat
  | mvar n => {n}
  | sym _ => ∅
  | lvl _ => ∅
  | app f a => f.vars ∪ a.vars

/-- The level occurrences of a term. -/
def T.levels : T → Finset Nat
  | mvar _ => ∅
  | sym _ => ∅
  | lvl ℓ => {ℓ}
  | app f a => f.levels ∪ a.levels

/-- A term with no metavariables. -/
def T.Ground (t : T) : Prop := t.vars = ∅

/-- Rule 7 for a meta created at depth `d`: every level it mentions is below
`d`. A binder introduced inside the goal or the head telescope would be at
depth `d` or deeper, so mentioning it is a capture. -/
def T.Scoped (d : Nat) (t : T) : Prop := ∀ ℓ ∈ t.levels, ℓ < d

/-- Decidable form of `T.Scoped`, for fixtures. -/
def T.scopedB (d : Nat) : T → Bool
  | mvar _ => true
  | sym _ => true
  | lvl ℓ => decide (ℓ < d)
  | app f a => (f.scopedB d) && (a.scopedB d)

/-- A term that is a level occurrence. A value meta must be solved by one. -/
def T.IsLevel (t : T) : Prop := ∃ ℓ, t = lvl ℓ

/-- The environment assigns each level the code of the value it names. -/
def T.CodedLevel (γ : Nat → Option Nat) (t : T) (c : Nat) : Prop :=
  ∃ ℓ, t = lvl ℓ ∧ γ ℓ = some c

/-- An argument position of an application: an explicit term, or inference.
The design refuses inference at a premise position. -/
inductive Arg where
  | term : T → Arg
  | inferred : Arg
  deriving DecidableEq, Repr

/-- A premise argument is a ground explicit term: its witness carries no
metavariable, so no later substitution can retarget its identity. -/
def Arg.SourceBound : Arg → Prop
  | term t => t.Ground
  | inferred => False

theorem T.subst_comp (σ θ : Nat → T) (t : T) :
    (t.subst σ).subst θ = t.subst (fun n => (σ n).subst θ) := by
  induction t with
  | mvar n => rfl
  | sym s => rfl
  | lvl ℓ => rfl
  | app f a ihf iha => simp only [subst, ihf, iha]

theorem T.subst_id (t : T) : t.subst mvar = t := by
  induction t with
  | mvar n => rfl
  | sym s => rfl
  | lvl ℓ => rfl
  | app f a ihf iha => simp only [subst, ihf, iha]

/-- Substitutions that agree on the metavariables of a term agree on the term. -/
theorem T.subst_congr {σ θ : Nat → T} {t : T} (h : ∀ n ∈ t.vars, σ n = θ n) :
    t.subst σ = t.subst θ := by
  induction t with
  | mvar n => exact h n (by simp [vars])
  | sym s => rfl
  | lvl ℓ => rfl
  | app f a ihf iha =>
      simp only [subst]
      rw [ihf (fun n hn => h n (by simp [vars, hn])),
        iha (fun n hn => h n (by simp [vars, hn]))]

/-- A ground term is fixed by every substitution. -/
theorem T.subst_ground {t : T} (h : t.Ground) (θ : Nat → T) : t.subst θ = t := by
  have : t.subst θ = t.subst mvar :=
    subst_congr (by simp [Ground] at h; simp [h])
  rw [this, subst_id]

/-- Metavariables of a substituted term. -/
theorem T.mem_vars_subst (σ : Nat → T) (t : T) (m : Nat) :
    m ∈ (t.subst σ).vars ↔ ∃ n ∈ t.vars, m ∈ (σ n).vars := by
  induction t with
  | mvar n => simp [subst, vars]
  | sym s => simp [subst, vars]
  | lvl ℓ => simp [subst, vars]
  | app f a ihf iha =>
      simp only [subst, vars, Finset.mem_union, ihf, iha]
      constructor
      · rintro (⟨n, hn, hm⟩ | ⟨n, hn, hm⟩)
        · exact ⟨n, .inl hn, hm⟩
        · exact ⟨n, .inr hn, hm⟩
      · rintro ⟨n, hn | hn, hm⟩
        · exact .inl ⟨n, hn, hm⟩
        · exact .inr ⟨n, hn, hm⟩

/-- A witness of either side's metavariables is a witness of the union, and
conversely: the two sides of an application contribute independently. -/
theorem T.exists_over_vars {p : Nat → Prop} (f a : T) :
    (∃ n, (n ∈ f.vars ∨ n ∈ a.vars) ∧ p n) ↔
      (∃ n ∈ f.vars, p n) ∨ ∃ n ∈ a.vars, p n := by
  constructor
  · rintro ⟨n, hn | hn, hp⟩
    · exact Or.inl ⟨n, hn, hp⟩
    · exact Or.inr ⟨n, hn, hp⟩
  · rintro (⟨n, hn, hp⟩ | ⟨n, hn, hp⟩)
    · exact ⟨n, Or.inl hn, hp⟩
    · exact ⟨n, Or.inr hn, hp⟩

/-- Level occurrences of a substituted term come from the term itself and
from the bindings it actually uses. Substitution adds no level of its own. -/
theorem T.mem_levels_subst (σ : Nat → T) (t : T) (ℓ : Nat) :
    ℓ ∈ (t.subst σ).levels ↔
      ℓ ∈ t.levels ∨ ∃ n ∈ t.vars, ℓ ∈ (σ n).levels := by
  induction t with
  | mvar n => simp [subst, levels, vars]
  | sym s => simp [subst, levels, vars]
  | lvl m => simp [subst, levels, vars]
  | app f a ihf iha =>
      simp only [subst, levels, vars, Finset.mem_union]
      rw [exists_over_vars (p := fun n => ℓ ∈ (σ n).levels) f a, ihf, iha]
      simp only [or_assoc, or_left_comm]

theorem T.scoped_iff {d : Nat} (t : T) : t.Scoped d ↔ t.scopedB d = true := by
  induction t with
  | mvar n => simp [Scoped, scopedB, levels]
  | sym s => simp [Scoped, scopedB, levels]
  | lvl m =>
      simp only [Scoped, scopedB, levels, Finset.mem_singleton, forall_eq]
      exact ⟨decide_eq_true, of_decide_eq_true⟩
  | app f a ihf iha =>
      have h : (app f a).Scoped d ↔ f.Scoped d ∧ a.Scoped d := by
        simp only [Scoped, levels, Finset.forall_mem_union]
      rw [h, scopedB, Bool.and_eq_true, ihf, iha]

/-- Scoping is preserved by any substitution with scoped bindings. -/
theorem T.scoped_subst {d : Nat} {t : T} {σ : Nat → T} (ht : t.Scoped d)
    (hσ : ∀ n ∈ t.vars, (σ n).Scoped d) : (t.subst σ).Scoped d := by
  intro ℓ hℓ
  rcases (mem_levels_subst σ t ℓ).1 hℓ with hl | ⟨n, hn, hm⟩
  · exact ht ℓ hl
  · exact hσ n hn ℓ hm

/-- A scoped instantiation has scoped bindings at every metavariable the term
uses, so an ill-scoped binding cannot be hidden by a later step. -/
theorem T.scoped_of_subst {d : Nat} {t : T} {σ : Nat → T}
    (h : (t.subst σ).Scoped d) : ∀ n ∈ t.vars, (σ n).Scoped d := by
  intro n hn ℓ hℓ
  exact h ℓ ((mem_levels_subst σ t ℓ).2 (.inr ⟨n, hn, hℓ⟩))

/-- Ill-scoping is exactly the presence of a level at or beyond the depth. -/
theorem T.not_scoped_iff {d : Nat} {t : T} :
    ¬ t.Scoped d ↔ ∃ ℓ ∈ t.levels, d ≤ ℓ := by
  simp only [Scoped, not_forall, exists_prop]
  constructor
  · rintro ⟨ℓ, hℓ, hge⟩
    exact ⟨ℓ, hℓ, Nat.le_of_not_lt hge⟩
  · rintro ⟨ℓ, hℓ, hge⟩
    exact ⟨ℓ, hℓ, Nat.not_lt_of_ge hge⟩

/-- Rule 7's refusal cannot be repaired: a ground binding that mentions a
level at or beyond its meta's depth keeps every instantiation ill-scoped. -/
theorem T.ill_scoped_persists {d : Nat} {t : T} {σ θ : Nat → T} {n : Nat}
    (hn : n ∈ t.vars) (hground : (σ n).Ground) (hill : ¬ (σ n).Scoped d) :
    ¬ ((t.subst σ).subst θ).Scoped d := by
  rw [subst_comp]
  intro h
  have hfix : (σ n).subst θ = σ n := subst_ground hground θ
  have hσ := scoped_of_subst (σ := fun k => (σ k).subst θ) h n hn
  rw [hfix] at hσ
  exact hill hσ

/-- Rule 7's refusal is forced: no substitution repairs an ill-scoped
binding, so accepting one would produce an elaborated term whose capture no
later step removes. -/
theorem T.ill_scoped_has_no_repair {d : Nat} {t : T} {σ : Nat → T} {n : Nat}
    (hn : n ∈ t.vars) (hground : (σ n).Ground) (hill : ¬ (σ n).Scoped d) :
    ∀ θ : Nat → T, ¬ ((t.subst σ).subst θ).Scoped d :=
  fun θ => ill_scoped_persists (θ := θ) hn hground hill

/-- A value meta's answer is a level of exactly the meta's own code, and that
stays true under every later substitution. -/
theorem T.coded_level_stable {γ : Nat → Option Nat} {t : T} {c : Nat}
    (hground : t.Ground) (hc : t.CodedLevel γ c) :
    ∀ θ : Nat → T, (t.subst θ).CodedLevel γ c := by
  intro θ
  rw [subst_ground hground θ]
  exact hc

/-- The exact-code requirement is unrepairable too: a value meta bound to a
level of the wrong code keeps every instantiation wrong-coded. -/
theorem T.wrong_code_persists {γ : Nat → Option Nat} {θ : Nat → T} {ℓ c : Nat}
    (hwrong : γ ℓ ≠ some c) : ¬ ((T.lvl ℓ).subst θ).CodedLevel γ c := by
  intro hc
  obtain ⟨m, hm, hcode⟩ := hc
  have hml : m = ℓ := by
    injection hm with h
    exact h.symm
  exact hwrong (hml ▸ hcode)

/-- An accepted premise argument is a ground explicit term, so it carries no
metavariable and no later substitution can retarget its witness identity. -/
theorem Arg.source_bound_stable {a : Arg} (h : a.SourceBound) :
    ∃ t, a = .term t ∧ t.Ground := by
  cases a with
  | term t => exact ⟨t, rfl, h⟩
  | inferred => exact h.elim

/-- The premise argument's witness is unchanged by every substitution. -/
theorem Arg.source_bound_unchanged {t : T} {θ : Nat → T} (h : t.Ground) :
    t.subst θ = t :=
  T.subst_ground h θ

/-- Decidable form of `T.Ground`, for fixtures. -/
def T.groundB : T → Bool
  | mvar _ => false
  | sym _ => true
  | lvl _ => true
  | app f a => (f.groundB) && (a.groundB)

theorem T.ground_iff (t : T) : t.Ground ↔ t.groundB = true := by
  induction t with
  | mvar n => simp [Ground, groundB, vars]
  | sym s => simp [Ground, groundB, vars]
  | lvl m => simp [Ground, groundB, vars]
  | app f a ihf iha =>
      have h : (app f a).Ground ↔ f.Ground ∧ a.Ground := by
        simp only [Ground, vars, Finset.union_eq_empty]
      rw [h, groundB, Bool.and_eq_true, ihf, iha]

/-! Fixtures from the design, decided by evaluation.

IMPL-CASE-12's capture is ill-scoped: a goal binder sits at the meta's own
depth, so a solution naming it is refused. P2's rebasing is well-scoped: the
context's `B` has a level below the meta's depth. -/

/-- A meta at depth 1 solved by a level below 1 is scoped. -/
example : (T.app (T.sym 0) (T.lvl 0)).scopedB 1 = true := by decide

/-- The same meta solved by a level at its own depth is ill-scoped: the
capture of IMPL-CASE-12. -/
example : (T.app (T.sym 0) (T.lvl 1)).scopedB 1 = false := by decide

/-- A meta at depth 2 may name both levels 0 and 1 (P2's rebasing). -/
example : (T.app (T.app (T.sym 0) (T.lvl 0)) (T.lvl 1)).scopedB 2 = true := by
  decide

/-- Raising the depth never turns a scoped term ill-scoped. -/
example : (T.app (T.sym 0) (T.lvl 3)).scopedB 4 = true := by decide

/-- A rigid level is ground. -/
example : (T.lvl 3).groundB = true := by decide

/-- The ill-scoped binding stays ill-scoped in its instantiation: the probe
substitution leaves meta 7 alone, and the level at the meta's own depth
survives. -/
example : ((T.mvar 7).subst (fun _ => T.lvl 1)).scopedB 1 = false := by decide

end NobleFoundation.ImplicitScope
