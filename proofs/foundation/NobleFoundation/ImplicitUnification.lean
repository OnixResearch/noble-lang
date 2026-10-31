/-
First-order unification for the `implicit-args/v1` constraint algebra
(`implicit-proof-arguments`, task 13, Lean half).

The design solves the constraint language C by Robinson unification over finite
trees, with structural identity as the only equality. This file models that
solver over a minimal tree algebra (metas, symbols, binary application) into
which C's codes and values encode injectively. It proves what each answer means:

* `solved_mgu`: an answer `solved σ` unifies every constraint, and every unifier
  factors through `σ` (σ is a most general unifier).
* `refused_no_unifier`: after a clash or an occurs failure no unifier exists,
  so `invalid-constraint` never hides a solution.
* `solvable_iff`: the constraints have a unifier exactly when some budget
  answers `solved` (fragment completeness); `solve_complete` is the
  well-founded argument on (remaining metas, size).
* `answers_agree`: two budgets that both answer give the same answer, so the
  budget only decides between an answer and `exhausted`.
* `unique_iff_ground`: all unifiers agree at a meta exactly when the answer is
  ground there; otherwise two concrete solutions differ. This is the design's
  uniqueness criterion: a reported unique solution is the unique ground mgu, and
  `underconstrained` is never a silent choice.
* `Code.enc_injective`, `Val.enc_injective`: structural identity of codes and
  values is equality of their trees.

It is a model claim. It says nothing about a Rust elaborator, which does not
exist yet, about scope (rule 7) or premise positions, or about source-to-Lean
translation.
-/
import Mathlib.Data.Finset.Card

namespace NobleFoundation.ImplicitUnification

/-- Finite trees with metavariables. Constructors of C are symbols applied in
curried form (`Code.enc`), so a meta always stands for a whole term. -/
inductive Tm where
  | mvar : Nat → Tm
  | sym : Nat → Tm
  | app : Tm → Tm → Tm
  deriving DecidableEq, Repr

/-- Apply a substitution to every meta. -/
def Tm.subst (σ : Nat → Tm) : Tm → Tm
  | mvar n => σ n
  | sym s => sym s
  | app f a => app (f.subst σ) (a.subst σ)

/-- The metas of a term. -/
def Tm.vars : Tm → Finset Nat
  | mvar n => {n}
  | sym _ => ∅
  | app f a => f.vars ∪ a.vars

/-- Node count. -/
def Tm.size : Tm → Nat
  | mvar _ => 1
  | sym _ => 1
  | app f a => f.size + a.size + 1

/-- A term without metas. -/
def Tm.Ground (t : Tm) : Prop := t.vars = ∅

theorem Tm.size_pos (t : Tm) : 0 < t.size := by
  cases t <;> simp [size]

theorem Tm.subst_comp (σ θ : Nat → Tm) (t : Tm) :
    (t.subst σ).subst θ = t.subst (fun n => (σ n).subst θ) := by
  induction t with
  | mvar n => rfl
  | sym s => rfl
  | app f a ihf iha => simp [subst, ihf, iha]

theorem Tm.subst_id (t : Tm) : t.subst mvar = t := by
  induction t with
  | mvar n => rfl
  | sym s => rfl
  | app f a ihf iha => simp [subst, ihf, iha]

/-- Substitutions that agree on the metas of a term agree on the term. -/
theorem Tm.subst_congr {σ θ : Nat → Tm} {t : Tm} (h : ∀ n ∈ t.vars, σ n = θ n) :
    t.subst σ = t.subst θ := by
  induction t with
  | mvar n => exact h n (by simp [vars])
  | sym s => rfl
  | app f a ihf iha =>
      simp only [subst]
      rw [ihf (fun n hn => h n (by simp [vars, hn])),
        iha (fun n hn => h n (by simp [vars, hn]))]

/-- A ground term is fixed by every substitution. -/
theorem Tm.subst_ground {t : Tm} (h : t.Ground) (θ : Nat → Tm) : t.subst θ = t := by
  have : t.subst θ = t.subst mvar := subst_congr (by simp [Ground] at h; simp [h])
  rw [this, subst_id]

theorem Tm.mem_vars_subst (σ : Nat → Tm) (t : Tm) (m : Nat) :
    m ∈ (t.subst σ).vars ↔ ∃ n ∈ t.vars, m ∈ (σ n).vars := by
  induction t with
  | mvar n => simp [subst, vars]
  | sym s => simp [subst, vars]
  | app f a ihf iha =>
      simp only [subst, vars, Finset.mem_union, ihf, iha]
      constructor
      · rintro (⟨n, hn, hm⟩ | ⟨n, hn, hm⟩)
        · exact ⟨n, .inl hn, hm⟩
        · exact ⟨n, .inr hn, hm⟩
      · rintro ⟨n, hn | hn, hm⟩
        · exact .inl ⟨n, hn, hm⟩
        · exact .inr ⟨n, hn, hm⟩

/-- A meta inside a term is no larger than the instantiated term, and strictly
smaller when the term is not that meta itself. -/
theorem Tm.size_le_of_mem (θ : Nat → Tm) {x : Nat} {t : Tm} (hx : x ∈ t.vars) :
    (θ x).size ≤ (t.subst θ).size := by
  induction t with
  | mvar n => simp [vars] at hx; subst hx; exact le_refl _
  | sym s => simp [vars] at hx
  | app f a ihf iha =>
      simp only [vars, Finset.mem_union] at hx
      simp only [subst, size]
      rcases hx with hx | hx
      · have := ihf hx; omega
      · have := iha hx; omega

theorem Tm.size_lt_of_mem (θ : Nat → Tm) {x : Nat} {t : Tm} (hx : x ∈ t.vars)
    (hne : t ≠ mvar x) : (θ x).size < (t.subst θ).size := by
  cases t with
  | mvar n => simp [vars] at hx; subst hx; exact absurd rfl hne
  | sym s => simp [vars] at hx
  | app f a =>
      simp only [vars, Finset.mem_union] at hx
      simp only [subst, size]
      rcases hx with hx | hx
      · have := size_le_of_mem θ hx; omega
      · have := size_le_of_mem θ hx; omega

open Tm

/-- The substitution that binds one meta. -/
def single (x : Nat) (t : Tm) : Nat → Tm := fun n => if n = x then t else .mvar n

/-- Binding `x := t` and then applying `θ` is applying `θ`, when `θ` already
maps `x` to its instance of `t`. -/
theorem subst_single_agree {θ : Nat → Tm} {x : Nat} {t : Tm} (h : θ x = t.subst θ)
    (u : Tm) : (u.subst (single x t)).subst θ = u.subst θ := by
  rw [subst_comp]
  apply subst_congr
  intro n _
  by_cases hn : n = x
  · subst hn; simp [single, h]
  · simp [single, hn, Tm.subst]

theorem vars_subst_single (x : Nat) (t u : Tm) :
    (u.subst (single x t)).vars ⊆ u.vars.erase x ∪ t.vars := by
  intro m hm
  obtain ⟨n, hn, hm⟩ := (mem_vars_subst _ u m).mp hm
  by_cases hnx : n = x
  · subst hnx; simp [single] at hm; exact Finset.mem_union_right _ hm
  · simp [single, hnx, vars] at hm
    subst hm
    exact Finset.mem_union_left _ (Finset.mem_erase.mpr ⟨hnx, hn⟩)

/-! ## Problems, unifiers and the solver -/

/-- Unsolved equations. -/
abbrev Eqns := List (Tm × Tm)

/-- Solved bindings `x := t`. -/
abbrev Solved := List (Nat × Tm)

def substEqns (σ : Nat → Tm) (U : Eqns) : Eqns :=
  U.map (fun p => (p.1.subst σ, p.2.subst σ))

def substSolved (σ : Nat → Tm) (S : Solved) : Solved :=
  S.map (fun q => (q.1, q.2.subst σ))

/-- `θ` unifies every equation. -/
def Unifies (θ : Nat → Tm) (U : Eqns) : Prop := ∀ p ∈ U, p.1.subst θ = p.2.subst θ

/-- `θ` satisfies every solved binding. -/
def Satisfies (θ : Nat → Tm) (S : Solved) : Prop := ∀ q ∈ S, θ q.1 = q.2.subst θ

/-- The metas of a list of equations. -/
def eqnVars : Eqns → Finset Nat
  | [] => ∅
  | p :: U => p.1.vars ∪ p.2.vars ∪ eqnVars U

/-- Total node count of a list of equations. -/
def eqnSize : Eqns → Nat
  | [] => 0
  | p :: U => p.1.size + p.2.size + eqnSize U

/-- The binding a solved form records for a meta, if any. -/
def find : Solved → Nat → Option Tm
  | [], _ => none
  | (k, t) :: S, n => if n = k then some t else find S n

/-- The substitution a solved form denotes: its bindings, identity elsewhere. -/
def toSubst (S : Solved) : Nat → Tm := fun n => (find S n).getD (.mvar n)

/-- What the solver reports. -/
inductive Outcome where
  /-- Unification succeeded; the substitution is a most general unifier. -/
  | solved (σ : Nat → Tm)
  /-- Different symbols or rigid shapes: `invalid-constraint`. -/
  | clash
  /-- A meta equated with a term strictly containing it: `invalid-constraint`. -/
  | cycle
  /-- The step budget ran out: `exhausted`. -/
  | exhausted

/-- The decision for one head equation. -/
inductive Head where
  /-- The two sides are identical. -/
  | skip
  /-- Two applications: equate the heads and the arguments. -/
  | split (f g a b : Tm)
  /-- Bind a meta that does not occur in the other side. -/
  | bind (x : Nat) (t : Tm)
  | clash
  | cycle

/-- Rigid equations decompose or clash; a meta equation binds the meta after
the occurs check. Structural identity is the only equality used. -/
def head : Tm → Tm → Head
  | .mvar x, t => if t = .mvar x then .skip else if x ∈ t.vars then .cycle else .bind x t
  | s, .mvar x => if x ∈ s.vars then .cycle else .bind x s
  | .sym a, .sym b => if a = b then .skip else .clash
  | .app f a, .app g b => .split f g a b
  | _, _ => .clash

/-- Robinson unification in Martelli–Montanari form, one equation per step,
under a step budget. A binding is applied to every remaining equation and to
every earlier binding. -/
def solve : Nat → Eqns → Solved → Outcome
  | 0, _, _ => .exhausted
  | _ + 1, [], S => .solved (toSubst S)
  | fuel + 1, (s, t) :: U, S =>
      match head s t with
      | .skip => solve fuel U S
      | .split f g a b => solve fuel ((f, g) :: (a, b) :: U) S
      | .bind x u => solve fuel (substEqns (single x u) U) ((x, u) :: substSolved (single x u) S)
      | .clash => .clash
      | .cycle => .cycle

/-- The solved-form invariant: distinct bound metas that occur neither in any
binding's right-hand side nor in the remaining equations. -/
structure SolvedForm (U : Eqns) (S : Solved) : Prop where
  nodup : (S.map Prod.fst).Nodup
  rhs : ∀ q ∈ S, ∀ q' ∈ S, q.1 ∉ q'.2.vars
  eqns : ∀ q ∈ S, q.1 ∉ eqnVars U

/-! ## The head decision -/

theorem head_skip {s t : Tm} (h : head s t = .skip) : s = t := by
  cases s <;> cases t <;> simp only [head] at h <;> (try split_ifs at h) <;> simp_all

theorem head_split {s t f g a b : Tm} (h : head s t = .split f g a b) :
    s = .app f a ∧ t = .app g b := by
  cases s <;> cases t <;> simp only [head] at h <;> (try split_ifs at h) <;> simp_all

theorem head_bind {s t u : Tm} {x : Nat} (h : head s t = .bind x u) :
    x ∉ u.vars ∧ ((s = .mvar x ∧ t = u) ∨ (t = .mvar x ∧ s = u)) := by
  cases s with
  | mvar y =>
      simp only [head] at h
      split_ifs at h with h1 h2
      injection h with h3 h4
      subst h3 h4
      exact ⟨h2, .inl ⟨rfl, rfl⟩⟩
  | sym a =>
      cases t <;> simp only [head] at h <;> (try split_ifs at h with h1) <;> try contradiction
      injection h with h3 h4
      subst h3 h4
      exact ⟨h1, .inr ⟨rfl, rfl⟩⟩
  | app f a =>
      cases t <;> simp only [head] at h <;> (try split_ifs at h with h1) <;> try contradiction
      injection h with h3 h4
      subst h3 h4
      exact ⟨h1, .inr ⟨rfl, rfl⟩⟩

theorem head_cycle {s t : Tm} (h : head s t = .cycle) (θ : Nat → Tm) :
    s.subst θ ≠ t.subst θ := by
  cases s with
  | mvar x =>
      simp only [head] at h
      split_ifs at h with h1 h2
      intro heq
      have := size_lt_of_mem θ h2 h1
      simp only [Tm.subst] at heq
      rw [heq] at this
      exact lt_irrefl _ this
  | sym a =>
      cases t <;> simp only [head] at h <;> (try split_ifs at h) <;> try contradiction
  | app f a =>
      cases t with
      | mvar x =>
          simp only [head] at h
          split_ifs at h with h1
          intro heq
          have := size_lt_of_mem θ h1 (by simp)
          simp only [Tm.subst] at heq
          rw [← heq] at this
          exact lt_irrefl _ this
      | sym b => simp [head] at h
      | app g b => simp [head] at h

theorem head_clash {s t : Tm} (h : head s t = .clash) (θ : Nat → Tm) :
    s.subst θ ≠ t.subst θ := by
  cases s <;> cases t <;> simp only [head] at h <;> (try split_ifs at h) <;>
    simp_all [Tm.subst]

/-! ## Unifiers are preserved step by step -/

theorem unifies_cons {θ : Nat → Tm} {p : Tm × Tm} {U : Eqns} :
    Unifies θ (p :: U) ↔ p.1.subst θ = p.2.subst θ ∧ Unifies θ U := by
  simp [Unifies]

theorem satisfies_cons {θ : Nat → Tm} {q : Nat × Tm} {S : Solved} :
    Satisfies θ (q :: S) ↔ θ q.1 = q.2.subst θ ∧ Satisfies θ S := by
  simp [Satisfies]

theorem mem_eqnVars {n : Nat} {U : Eqns} :
    n ∈ eqnVars U ↔ ∃ p ∈ U, n ∈ p.1.vars ∨ n ∈ p.2.vars := by
  induction U with
  | nil => simp [eqnVars]
  | cons p U ih =>
      simp only [eqnVars, Finset.mem_union, ih, List.mem_cons]
      constructor
      · rintro ((h | h) | ⟨q, hq, h⟩)
        · exact ⟨p, .inl rfl, .inl h⟩
        · exact ⟨p, .inl rfl, .inr h⟩
        · exact ⟨q, .inr hq, h⟩
      · rintro ⟨q, rfl | hq, h⟩
        · rcases h with h | h
          · exact .inl (.inl h)
          · exact .inl (.inr h)
        · exact .inr ⟨q, hq, h⟩

theorem unifies_substEqns_iff {θ : Nat → Tm} {x : Nat} {t : Tm} (h : θ x = t.subst θ)
    (U : Eqns) : Unifies θ (substEqns (single x t) U) ↔ Unifies θ U := by
  constructor
  · intro hu p hp
    have := hu (p.1.subst (single x t), p.2.subst (single x t)) (List.mem_map.mpr ⟨p, hp, rfl⟩)
    simpa only [subst_single_agree h] using this
  · intro hu q hq
    obtain ⟨p, hp, rfl⟩ := List.mem_map.mp hq
    simp only [subst_single_agree h]
    exact hu p hp

theorem satisfies_substSolved_iff {θ : Nat → Tm} {x : Nat} {t : Tm} (h : θ x = t.subst θ)
    (S : Solved) : Satisfies θ (substSolved (single x t) S) ↔ Satisfies θ S := by
  constructor
  · intro hs q hq
    have := hs (q.1, q.2.subst (single x t)) (List.mem_map.mpr ⟨q, hq, rfl⟩)
    simpa only [subst_single_agree h] using this
  · intro hs q hq
    obtain ⟨q₀, hq₀, rfl⟩ := List.mem_map.mp hq
    simp only [subst_single_agree h]
    exact hs q₀ hq₀

/-- Binding `x := t` keeps exactly the unifiers that send `x` to their
instance of `t`. -/
theorem bind_iff {θ : Nat → Tm} {x : Nat} {t : Tm} {U : Eqns} {S : Solved} :
    (θ x = t.subst θ ∧ Unifies θ U ∧ Satisfies θ S) ↔
      (Unifies θ (substEqns (single x t) U) ∧
        Satisfies θ ((x, t) :: substSolved (single x t) S)) := by
  constructor
  · rintro ⟨h, hu, hs⟩
    exact ⟨(unifies_substEqns_iff h U).mpr hu,
      satisfies_cons.mpr ⟨h, (satisfies_substSolved_iff h S).mpr hs⟩⟩
  · rintro ⟨hu, hs⟩
    obtain ⟨h, hs⟩ := satisfies_cons.mp hs
    exact ⟨h, (unifies_substEqns_iff h U).mp hu, (satisfies_substSolved_iff h S).mp hs⟩

/-- A `bind x u` head equation holds under `θ` exactly when `θ` sends `x` to
its instance of `u`, whichever side the meta was on. -/
theorem bind_head_iff {s t u : Tm} {x : Nat} (h : head s t = .bind x u) (θ : Nat → Tm) :
    s.subst θ = t.subst θ ↔ θ x = u.subst θ := by
  rcases (head_bind h).2 with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩
  · rfl
  · exact eq_comm

theorem unifies_split {θ : Nat → Tm} {f g a b : Tm} {U : Eqns} :
    Unifies θ ((.app f a, .app g b) :: U) ↔ Unifies θ ((f, g) :: (a, b) :: U) := by
  simp [Unifies, Tm.subst, and_assoc]

/-! ## The solved form denotes a most general unifier -/

theorem find_eq_none {S : Solved} {n : Nat} (h : ∀ q ∈ S, q.1 ≠ n) : find S n = none := by
  induction S with
  | nil => rfl
  | cons q S ih =>
      obtain ⟨k, t⟩ := q
      have hk : n ≠ k := fun e => h (k, t) (List.mem_cons_self ..) e.symm
      simp only [find, hk, if_false]
      exact ih (fun q hq => h q (List.mem_cons_of_mem _ hq))

theorem find_of_mem {S : Solved} (hnd : (S.map Prod.fst).Nodup) {k : Nat} {t : Tm}
    (h : (k, t) ∈ S) : find S k = some t := by
  induction S with
  | nil => simp at h
  | cons q S ih =>
      obtain ⟨k', t'⟩ := q
      simp only [List.map_cons, List.nodup_cons] at hnd
      rcases List.mem_cons.mp h with h' | h'
      · injection h' with h1 h2
        subst h1 h2
        simp [find]
      · have hne : k ≠ k' := by
          intro e
          apply hnd.1
          rw [← e]
          exact List.mem_map.mpr ⟨(k, t), h', rfl⟩
        simp only [find, hne, if_false]
        exact ih hnd.2 h'

theorem find_some {S : Solved} {n : Nat} {t : Tm} (h : find S n = some t) : (n, t) ∈ S := by
  induction S with
  | nil => simp [find] at h
  | cons q S ih =>
      obtain ⟨k, u⟩ := q
      simp only [find] at h
      split_ifs at h with hn
      · subst hn
        injection h with h
        subst h
        exact List.mem_cons_self ..
      · exact List.mem_cons_of_mem _ (ih h)

/-- With no equation left, the denoted substitution satisfies the solved form. -/
theorem toSubst_satisfies {S : Solved} (hsf : SolvedForm [] S) : Satisfies (toSubst S) S := by
  rintro ⟨k, t⟩ hq
  have hlhs : toSubst S k = t := by simp [toSubst, find_of_mem hsf.nodup hq]
  have hrhs : t.subst (toSubst S) = t := by
    have : t.subst (toSubst S) = t.subst Tm.mvar := by
      apply subst_congr
      intro n hn
      have : find S n = none :=
        find_eq_none (fun q' hq' e => hsf.rhs q' hq' (k, t) hq (by rw [e]; exact hn))
      simp [toSubst, this]
    rw [this, subst_id]
  rw [hlhs, hrhs]

/-- Every substitution satisfying a solved form factors through it. -/
theorem toSubst_most_general {S : Solved} {θ : Nat → Tm} (hs : Satisfies θ S) (n : Nat) :
    (toSubst S n).subst θ = θ n := by
  cases hf : find S n with
  | none => simp [toSubst, hf, Tm.subst]
  | some t =>
      simp only [toSubst, hf, Option.getD_some]
      exact (hs (n, t) (find_some hf)).symm

theorem eqnVars_cons_subset {p : Tm × Tm} {U : Eqns} : eqnVars U ⊆ eqnVars (p :: U) := by
  intro n hn
  simp [eqnVars, hn]

theorem SolvedForm.tail {p : Tm × Tm} {U : Eqns} {S : Solved} (h : SolvedForm (p :: U) S) :
    SolvedForm U S :=
  ⟨h.nodup, h.rhs, fun q hq hn => h.eqns q hq (eqnVars_cons_subset hn)⟩

theorem eqnVars_split (f g a b : Tm) (U : Eqns) :
    eqnVars ((f, g) :: (a, b) :: U) = eqnVars ((.app f a, .app g b) :: U) := by
  ext n
  simp only [eqnVars, Tm.vars, Finset.mem_union]
  tauto

theorem SolvedForm.split {f g a b : Tm} {U : Eqns} {S : Solved}
    (h : SolvedForm ((.app f a, .app g b) :: U) S) : SolvedForm ((f, g) :: (a, b) :: U) S :=
  ⟨h.nodup, h.rhs, fun q hq hn => h.eqns q hq (by rwa [eqnVars_split] at hn)⟩

theorem eqnVars_substEqns (x : Nat) (t : Tm) (U : Eqns) :
    eqnVars (substEqns (single x t) U) ⊆ (eqnVars U).erase x ∪ t.vars := by
  intro n hn
  obtain ⟨p, hp, hn⟩ := mem_eqnVars.mp hn
  obtain ⟨p₀, hp₀, rfl⟩ := List.mem_map.mp hp
  have side : ∀ u : Tm, (u = p₀.1 ∨ u = p₀.2) → n ∈ (u.subst (single x t)).vars →
      n ∈ (eqnVars U).erase x ∪ t.vars := by
    intro u hu hn
    rcases Finset.mem_union.mp (vars_subst_single x t u hn) with h | h
    · obtain ⟨hne, hmem⟩ := Finset.mem_erase.mp h
      refine Finset.mem_union_left _ (Finset.mem_erase.mpr ⟨hne, mem_eqnVars.mpr ⟨p₀, hp₀, ?_⟩⟩)
      rcases hu with rfl | rfl
      · exact .inl hmem
      · exact .inr hmem
    · exact Finset.mem_union_right _ h
  rcases hn with hn | hn
  · exact side _ (.inl rfl) hn
  · exact side _ (.inr rfl) hn

/-- Binding keeps the solved form. -/
theorem SolvedForm.bind {s t u : Tm} {x : Nat} {U : Eqns} {S : Solved}
    (hh : head s t = .bind x u) (hsf : SolvedForm ((s, t) :: U) S) :
    SolvedForm (substEqns (single x u) U) ((x, u) :: substSolved (single x u) S) := by
  obtain ⟨hxu, hshape⟩ := head_bind hh
  have hx : x ∈ eqnVars ((s, t) :: U) := by
    rcases hshape with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩ <;> simp [eqnVars, Tm.vars]
  have hu : u.vars ⊆ eqnVars ((s, t) :: U) := by
    rcases hshape with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩ <;> intro n hn <;> simp [eqnVars, hn]
  have hkeys : (substSolved (single x u) S).map Prod.fst = S.map Prod.fst := by
    simp [substSolved, List.map_map, Function.comp_def]
  -- Every bound meta, old or new, avoids `u` and the instantiated right-hand sides.
  have hkey : ∀ k, (k = x ∨ ∃ r, (k, r) ∈ S) → k ∉ u.vars ∧
      ∀ q ∈ S, k ∉ (q.2.subst (single x u)).vars := by
    rintro k hk
    have hku : k ∉ u.vars := by
      rcases hk with rfl | ⟨r, hr⟩
      · exact hxu
      · exact fun h => hsf.eqns (k, r) hr (hu h)
    refine ⟨hku, fun q hq hmem => ?_⟩
    rcases Finset.mem_union.mp (vars_subst_single x u q.2 hmem) with h | h
    · obtain ⟨hne, hmem⟩ := Finset.mem_erase.mp h
      rcases hk with rfl | ⟨r, hr⟩
      · exact hne rfl
      · exact hsf.rhs (k, r) hr q hq hmem
    · exact hku h
  refine ⟨?_, ?_, ?_⟩
  · rw [List.map_cons, hkeys, List.nodup_cons]
    refine ⟨fun hmem => ?_, hsf.nodup⟩
    obtain ⟨q, hq, hqx⟩ := List.mem_map.mp hmem
    exact hsf.eqns q hq (hqx ▸ hx)
  · intro q hq q' hq'
    have hk : q.1 = x ∨ ∃ r, (q.1, r) ∈ S := by
      rcases List.mem_cons.mp hq with rfl | hq
      · exact .inl rfl
      · obtain ⟨q₀, hq₀, rfl⟩ := List.mem_map.mp hq
        exact .inr ⟨q₀.2, hq₀⟩
    obtain ⟨hku, hkS⟩ := hkey q.1 hk
    rcases List.mem_cons.mp hq' with rfl | hq'
    · exact hku
    · obtain ⟨q₀, hq₀, rfl⟩ := List.mem_map.mp hq'
      exact hkS q₀ hq₀
  · intro q hq hmem
    have hk : q.1 = x ∨ ∃ r, (q.1, r) ∈ S := by
      rcases List.mem_cons.mp hq with rfl | hq
      · exact .inl rfl
      · obtain ⟨q₀, hq₀, rfl⟩ := List.mem_map.mp hq
        exact .inr ⟨q₀.2, hq₀⟩
    obtain ⟨hku, _⟩ := hkey q.1 hk
    rcases Finset.mem_union.mp (eqnVars_substEqns x u U hmem) with h | h
    · obtain ⟨hne, hmem⟩ := Finset.mem_erase.mp h
      rcases hk with hk | ⟨r, hr⟩
      · exact hne hk
      · exact hsf.eqns (q.1, r) hr (eqnVars_cons_subset hmem)
    · exact hku h

/-! ## What the solver's answers mean -/

/-- An accepted answer unifies the remaining equations and satisfies the
solved form, and every such unifier factors through it. -/
theorem solve_solved : ∀ (fuel : Nat) {U : Eqns} {S : Solved} {σ : Nat → Tm},
    solve fuel U S = .solved σ → SolvedForm U S →
      (Unifies σ U ∧ Satisfies σ S) ∧
        ∀ θ, Unifies θ U → Satisfies θ S → ∀ n, (σ n).subst θ = θ n
  | 0, _, _, _, h, _ => by simp [solve] at h
  | _ + 1, [], S, σ, h, hsf => by
      simp only [solve] at h
      injection h with h
      subst h
      exact ⟨⟨fun p hp => by simp at hp, toSubst_satisfies hsf⟩,
        fun θ _ hs n => toSubst_most_general hs n⟩
  | fuel + 1, (s, t) :: U, S, σ, h, hsf => by
      simp only [solve] at h
      cases hh : head s t with
      | skip =>
          simp only [hh] at h
          obtain ⟨⟨hu, hs⟩, hmg⟩ := solve_solved fuel h hsf.tail
          refine ⟨⟨unifies_cons.mpr ⟨by rw [head_skip hh], hu⟩, hs⟩, ?_⟩
          exact fun θ hθ hsθ => hmg θ (unifies_cons.mp hθ).2 hsθ
      | split f g a b =>
          simp only [hh] at h
          obtain ⟨rfl, rfl⟩ := head_split hh
          obtain ⟨⟨hu, hs⟩, hmg⟩ := solve_solved fuel h hsf.split
          exact ⟨⟨unifies_split.mpr hu, hs⟩, fun θ hθ hsθ => hmg θ (unifies_split.mp hθ) hsθ⟩
      | bind x u =>
          simp only [hh] at h
          obtain ⟨⟨hu, hs⟩, hmg⟩ := solve_solved fuel h (hsf.bind hh)
          obtain ⟨hx, hu', hs'⟩ := bind_iff.mpr ⟨hu, hs⟩
          refine ⟨⟨unifies_cons.mpr ⟨(bind_head_iff hh σ).mpr hx, hu'⟩, hs'⟩, ?_⟩
          intro θ hθ hsθ
          obtain ⟨hθ₁, hθ₂⟩ := unifies_cons.mp hθ
          obtain ⟨h₁, h₂⟩ := bind_iff.mp ⟨(bind_head_iff hh θ).mp hθ₁, hθ₂, hsθ⟩
          exact hmg θ h₁ h₂
      | clash => simp [hh] at h
      | cycle => simp [hh] at h

/-- A refusal (clash or occurs failure) means no unifier exists. -/
theorem solve_refused : ∀ (fuel : Nat) {U : Eqns} {S : Solved},
    (solve fuel U S = .clash ∨ solve fuel U S = .cycle) →
      ∀ θ, ¬ (Unifies θ U ∧ Satisfies θ S)
  | 0, _, _, h, _, _ => by simp [solve] at h
  | _ + 1, [], _, h, _, _ => by simp [solve] at h
  | fuel + 1, (s, t) :: U, S, h, θ, ⟨hu, hs⟩ => by
      simp only [solve] at h
      obtain ⟨hst, hU⟩ := unifies_cons.mp hu
      cases hh : head s t with
      | skip =>
          simp only [hh] at h
          exact solve_refused fuel h θ ⟨hU, hs⟩
      | split f g a b =>
          simp only [hh] at h
          obtain ⟨rfl, rfl⟩ := head_split hh
          exact solve_refused fuel h θ ⟨unifies_split.mp hu, hs⟩
      | bind x u =>
          simp only [hh] at h
          exact solve_refused fuel h θ (bind_iff.mp ⟨(bind_head_iff hh θ).mp hst, hU, hs⟩)
      | clash => exact head_clash hh θ hst
      | cycle => exact head_cycle hh θ hst

/-! ## Termination and completeness -/

theorem lex_of {a b c d : Nat} (h : a < c ∨ (a ≤ c ∧ b < d)) :
    Prod.Lex (· < ·) (· < ·) (a, b) (c, d) := by
  rcases h with h | ⟨h₁, h₂⟩
  · exact Prod.Lex.left _ _ h
  · rcases Nat.lt_or_eq_of_le h₁ with h₁ | rfl
    · exact Prod.Lex.left _ _ h₁
    · exact Prod.Lex.right _ h₂

theorem measure_tail (p : Tm × Tm) (U : Eqns) :
    Prod.Lex (· < ·) (· < ·) ((eqnVars U).card, eqnSize U)
      ((eqnVars (p :: U)).card, eqnSize (p :: U)) := by
  apply lex_of
  refine .inr ⟨Finset.card_le_card eqnVars_cons_subset, ?_⟩
  have := p.1.size_pos
  simp only [eqnSize]
  omega

theorem measure_split (f g a b : Tm) (U : Eqns) :
    Prod.Lex (· < ·) (· < ·) ((eqnVars ((f, g) :: (a, b) :: U)).card,
        eqnSize ((f, g) :: (a, b) :: U))
      ((eqnVars ((.app f a, .app g b) :: U)).card, eqnSize ((.app f a, .app g b) :: U)) := by
  apply lex_of
  refine .inr ⟨by rw [eqnVars_split], ?_⟩
  simp only [eqnSize, Tm.size]
  omega

theorem measure_split_of {s t f g a b : Tm} {U : Eqns} (hh : head s t = .split f g a b) :
    Prod.Lex (· < ·) (· < ·) ((eqnVars ((f, g) :: (a, b) :: U)).card,
        eqnSize ((f, g) :: (a, b) :: U))
      ((eqnVars ((s, t) :: U)).card, eqnSize ((s, t) :: U)) := by
  obtain ⟨rfl, rfl⟩ := head_split hh
  exact measure_split f g a b U

theorem measure_bind {s t u : Tm} {x : Nat} {U : Eqns} (hh : head s t = .bind x u) :
    Prod.Lex (· < ·) (· < ·)
      ((eqnVars (substEqns (single x u) U)).card, eqnSize (substEqns (single x u) U))
      ((eqnVars ((s, t) :: U)).card, eqnSize ((s, t) :: U)) := by
  obtain ⟨hxu, hshape⟩ := head_bind hh
  have hx : x ∈ eqnVars ((s, t) :: U) := by
    rcases hshape with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩ <;> simp [eqnVars, Tm.vars]
  have hu : u.vars ⊆ eqnVars ((s, t) :: U) := by
    rcases hshape with ⟨rfl, rfl⟩ | ⟨rfl, rfl⟩ <;> intro n hn <;> simp [eqnVars, hn]
  have hsub : eqnVars (substEqns (single x u) U) ⊆ (eqnVars ((s, t) :: U)).erase x := by
    intro n hn
    rcases Finset.mem_union.mp (eqnVars_substEqns x u U hn) with h | h
    · obtain ⟨hne, hmem⟩ := Finset.mem_erase.mp h
      exact Finset.mem_erase.mpr ⟨hne, eqnVars_cons_subset hmem⟩
    · exact Finset.mem_erase.mpr ⟨fun e => hxu (e ▸ h), hu h⟩
  apply lex_of
  left
  calc (eqnVars (substEqns (single x u) U)).card
      ≤ ((eqnVars ((s, t) :: U)).erase x).card := Finset.card_le_card hsub
    _ < (eqnVars ((s, t) :: U)).card := Finset.card_erase_lt_of_mem hx

/-- If the equations and solved form have a common unifier, some budget
yields an answer `solved`. -/
theorem solve_complete (U : Eqns) (S : Solved) (hsf : SolvedForm U S)
    (h : ∃ θ, Unifies θ U ∧ Satisfies θ S) : ∃ fuel σ, solve fuel U S = .solved σ := by
  match U, hsf, h with
  | [], _, _ => exact ⟨1, toSubst S, rfl⟩
  | (s, t) :: U, hsf, ⟨θ, hu, hs⟩ =>
      obtain ⟨hst, hU⟩ := unifies_cons.mp hu
      cases hh : head s t with
      | skip =>
          obtain ⟨fuel, σ, hsol⟩ := solve_complete U S hsf.tail ⟨θ, hU, hs⟩
          exact ⟨fuel + 1, σ, by simp only [solve, hh, hsol]⟩
      | split f g a b =>
          obtain ⟨hs', ht'⟩ := head_split hh
          have hsf' : SolvedForm ((f, g) :: (a, b) :: U) S := by
            subst hs' ht'
            exact hsf.split
          have hu' : Unifies θ ((f, g) :: (a, b) :: U) := by
            subst hs' ht'
            exact unifies_split.mp hu
          obtain ⟨fuel, σ, hsol⟩ := solve_complete ((f, g) :: (a, b) :: U) S hsf' ⟨θ, hu', hs⟩
          exact ⟨fuel + 1, σ, by simp only [solve, hh, hsol]⟩
      | bind x u =>
          obtain ⟨fuel, σ, hsol⟩ :=
            solve_complete (substEqns (single x u) U) ((x, u) :: substSolved (single x u) S)
              (hsf.bind hh) ⟨θ, bind_iff.mp ⟨(bind_head_iff hh θ).mp hst, hU, hs⟩⟩
          exact ⟨fuel + 1, σ, by simp only [solve, hh, hsol]⟩
      | clash => exact absurd hst (head_clash hh θ)
      | cycle => exact absurd hst (head_cycle hh θ)
termination_by ((eqnVars U).card, eqnSize U)
decreasing_by
  all_goals first
    | exact measure_tail _ _
    | exact measure_split_of (by assumption)
    | exact measure_bind (by assumption)

/-! ## The budget -/

/-- An answer, once given, does not change with one more step of budget. -/
theorem solve_succ : ∀ (fuel : Nat) {U : Eqns} {S : Solved} {r : Outcome},
    solve fuel U S = r → r ≠ .exhausted → solve (fuel + 1) U S = r
  | 0, _, _, _, h, hr => by
      simp only [solve] at h
      exact absurd h.symm hr
  | _ + 1, [], _, _, h, _ => by
      simp only [solve] at h ⊢
      exact h
  | fuel + 1, (s, t) :: U, S, r, h, hr => by
      simp only [solve] at h ⊢
      cases hh : head s t with
      | skip =>
          simp only [hh] at h ⊢
          exact solve_succ fuel h hr
      | split f g a b =>
          simp only [hh] at h ⊢
          exact solve_succ fuel h hr
      | bind x u =>
          simp only [hh] at h ⊢
          exact solve_succ fuel h hr
      | clash =>
          simp only [hh] at h ⊢
          exact h
      | cycle =>
          simp only [hh] at h ⊢
          exact h

theorem solve_add {fuel : Nat} {U : Eqns} {S : Solved} {r : Outcome}
    (h : solve fuel U S = r) (hr : r ≠ .exhausted) (k : Nat) : solve (fuel + k) U S = r := by
  induction k with
  | zero => exact h
  | succ k ih => exact solve_succ _ ih hr

/-- **Determinism across budgets**: two budgets that both answer give the same
answer, so the budget decides only whether an answer is given. -/
theorem answers_agree {f₁ f₂ : Nat} {U : Eqns} {S : Solved} {r₁ r₂ : Outcome}
    (h₁ : solve f₁ U S = r₁) (h₂ : solve f₂ U S = r₂) (hr₁ : r₁ ≠ .exhausted)
    (hr₂ : r₂ ≠ .exhausted) : r₁ = r₂ := by
  rcases Nat.le_total f₁ f₂ with h | h
  · have := solve_add h₁ hr₁ (f₂ - f₁)
    rw [Nat.add_sub_cancel' h, h₂] at this
    exact this.symm
  · have := solve_add h₂ hr₂ (f₁ - f₂)
    rw [Nat.add_sub_cancel' h, h₁] at this
    exact this

/-! ## Answers for a fresh problem -/

theorem solvedForm_nil (U : Eqns) : SolvedForm U [] := ⟨by simp, by simp, by simp⟩

theorem satisfies_nil (θ : Nat → Tm) : Satisfies θ [] := by simp [Satisfies]

/-- **Soundness and most generality**: an answer `solved σ` unifies every
constraint, and every unifier factors through `σ`. -/
theorem solved_mgu {fuel : Nat} {U : Eqns} {σ : Nat → Tm} (h : solve fuel U [] = .solved σ) :
    Unifies σ U ∧ ∀ θ, Unifies θ U → ∀ n, (σ n).subst θ = θ n := by
  obtain ⟨⟨hu, _⟩, hmg⟩ := solve_solved fuel h (solvedForm_nil U)
  exact ⟨hu, fun θ hθ n => hmg θ hθ (satisfies_nil θ) n⟩

/-- **Refusals are sound**: after a clash or an occurs failure no unifier
exists, so `invalid-constraint` never hides a solution. -/
theorem refused_no_unifier {fuel : Nat} {U : Eqns}
    (h : solve fuel U [] = .clash ∨ solve fuel U [] = .cycle) (θ : Nat → Tm) :
    ¬ Unifies θ U :=
  fun hu => solve_refused fuel h θ ⟨hu, satisfies_nil θ⟩

/-- **Completeness**: the constraints have a unifier exactly when some budget
answers `solved`. -/
theorem solvable_iff {U : Eqns} : (∃ θ, Unifies θ U) ↔ ∃ fuel σ, solve fuel U [] = .solved σ := by
  constructor
  · rintro ⟨θ, hθ⟩
    exact solve_complete U [] (solvedForm_nil U) ⟨θ, hθ, satisfies_nil θ⟩
  · rintro ⟨fuel, σ, h⟩
    exact ⟨σ, (solved_mgu h).1⟩

theorem subst_single_sym_ne {m : Nat} {u : Tm} (hm : m ∈ u.vars) :
    u.subst (single m (.sym 0)) ≠ u.subst (single m (.sym 1)) := by
  induction u with
  | mvar n =>
      simp only [Tm.vars, Finset.mem_singleton] at hm
      subst hm
      simp [Tm.subst, single]
  | sym s => simp [Tm.vars] at hm
  | app f a ihf iha =>
      simp only [Tm.vars, Finset.mem_union] at hm
      simp only [Tm.subst, ne_eq, Tm.app.injEq, not_and]
      rcases hm with hm | hm
      · exact fun h => absurd h (ihf hm)
      · exact fun _ h => iha hm h

/-- **Uniqueness, the design's criterion**: all unifiers agree at a meta
exactly when the answer is ground there. A non-ground answer is not a silent
choice: two concrete solutions differ at that meta, so `underconstrained` is
the honest refusal. -/
theorem unique_iff_ground {fuel : Nat} {U : Eqns} {σ : Nat → Tm}
    (h : solve fuel U [] = .solved σ) (n : Nat) :
    (∀ θ₁ θ₂, Unifies θ₁ U → Unifies θ₂ U → θ₁ n = θ₂ n) ↔ (σ n).Ground := by
  obtain ⟨hu, hmg⟩ := solved_mgu h
  constructor
  · intro huniq
    by_contra hng
    obtain ⟨m, hm⟩ : (σ n).vars.Nonempty := Finset.nonempty_iff_ne_empty.mpr hng
    let θ : Nat → Nat → Tm := fun i k => (σ k).subst (single m (.sym i))
    have inst : ∀ i, Unifies (θ i) U := by
      intro i p hp
      show p.1.subst (fun k => (σ k).subst (single m (.sym i))) =
        p.2.subst (fun k => (σ k).subst (single m (.sym i)))
      rw [← subst_comp, ← subst_comp, hu p hp]
    exact subst_single_sym_ne hm (huniq (θ 0) (θ 1) (inst 0) (inst 1))
  · intro hg θ₁ θ₂ h₁ h₂
    rw [← hmg θ₁ h₁ n, ← hmg θ₂ h₂ n, subst_ground hg, subst_ground hg]

/-! ## Codes and values of C -/

/-- Codes of C (`c` in the design). -/
inductive Code where
  | unit
  | bool
  | i64
  | text
  | var (level : Nat)
  | pair (a b : Code)
  | sum (a b : Code)
  | list (a : Code)
  | mvar (n : Nat)
  deriving DecidableEq, Repr

/-- Values of C (`v` in the design): a binder level or a value meta. The code a
value meta carries is checked by the scope rule, not by unification. -/
inductive Val where
  | level (l : Nat)
  | mvar (n : Nat)
  deriving DecidableEq, Repr

/-- Symbols below 16 name constructors and a level `ℓ` is symbol `16 + ℓ`.
Code metas are even metas and value metas odd ones, so the two never meet. -/
def Code.enc : Code → Tm
  | .unit => .sym 0
  | .bool => .sym 1
  | .i64 => .sym 2
  | .text => .sym 3
  | .var l => .app (.sym 4) (.sym (16 + l))
  | .pair a b => .app (.app (.sym 5) a.enc) b.enc
  | .sum a b => .app (.app (.sym 6) a.enc) b.enc
  | .list a => .app (.sym 7) a.enc
  | .mvar n => .mvar (2 * n)

def Val.enc : Val → Tm
  | .level l => .app (.sym 8) (.sym (16 + l))
  | .mvar n => .mvar (2 * n + 1)

/-- Structural identity of codes is equality of their trees. -/
theorem Code.enc_injective : Function.Injective Code.enc := by
  intro a b h
  induction a generalizing b with
  | unit => cases b <;> simp_all [Code.enc]
  | bool => cases b <;> simp_all [Code.enc]
  | i64 => cases b <;> simp_all [Code.enc]
  | text => cases b <;> simp_all [Code.enc]
  | var l => cases b <;> simp_all [Code.enc]
  | pair a₁ a₂ ih₁ ih₂ =>
      cases b <;> simp [Code.enc] at h
      rw [ih₁ h.1, ih₂ h.2]
  | sum a₁ a₂ ih₁ ih₂ =>
      cases b <;> simp [Code.enc] at h
      rw [ih₁ h.1, ih₂ h.2]
  | list a ih =>
      cases b <;> simp [Code.enc] at h
      rw [ih h]
  | mvar n =>
      cases b <;> simp_all [Code.enc]
      omega

theorem Val.enc_injective : Function.Injective Val.enc := by
  intro a b h
  cases a <;> cases b <;> simp_all [Val.enc]
  omega

/-! ## Design cases -/

/-- A2 head `Pair ?α I64` against goal `Pair (List Unit) I64`: unique, with
`?α := List Unit`. -/
def pairGoal : Eqns :=
  [((Code.pair (.mvar 0) .i64).enc, (Code.pair (.list .unit) .i64).enc)]

theorem pairGoal_solved : ∃ σ, solve 8 pairGoal [] = .solved σ ∧
    σ 0 = (Code.list .unit).enc ∧ (σ 0).Ground := ⟨_, rfl, rfl, rfl⟩

/-- The same problem under a budget of one step is `exhausted`: the budget
decides only whether an answer is given. -/
theorem pairGoal_exhausted : solve 1 pairGoal [] = .exhausted := rfl

/-- IMPL-CASE-09: `Pair (List ?α) I64 ≐ Bool` clashes, and no unifier exists. -/
theorem case09_clash :
    solve 8 [((Code.pair (.list (.mvar 0)) .i64).enc, Code.bool.enc)] [] = .clash ∧
      ∀ θ, ¬ Unifies θ [((Code.pair (.list (.mvar 0)) .i64).enc, Code.bool.enc)] :=
  ⟨rfl, refused_no_unifier (fuel := 8) (.inl rfl)⟩

/-- The occurs check: `?α ≐ List ?α` is refused, and no unifier exists. -/
theorem occurs_refused :
    solve 8 [((Code.mvar 0).enc, (Code.list (.mvar 0)).enc)] [] = .cycle ∧
      ∀ θ, ¬ Unifies θ [((Code.mvar 0).enc, (Code.list (.mvar 0)).enc)] :=
  ⟨rfl, refused_no_unifier (fuel := 8) (.inr rfl)⟩

/-- IMPL-CASE-08: a meta bound only to another meta. The answer is not ground
there, so two solutions differ and the case is `underconstrained`. -/
theorem case08_underconstrained :
    ∃ σ, solve 8 [((Code.mvar 0).enc, (Code.mvar 1).enc)] [] = .solved σ ∧
      ¬ (∀ θ₁ θ₂, Unifies θ₁ [((Code.mvar 0).enc, (Code.mvar 1).enc)] →
        Unifies θ₂ [((Code.mvar 0).enc, (Code.mvar 1).enc)] → θ₁ 0 = θ₂ 0) := by
  refine ⟨_, rfl, fun huniq => ?_⟩
  have hg := (unique_iff_ground (fuel := 8) rfl 0).mp huniq
  simp [Tm.Ground, toSubst, find, Code.enc, Tm.vars] at hg

end NobleFoundation.ImplicitUnification
