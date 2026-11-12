/-
Propositions and sorts of C, their structural identity and their rebasing
(`implicit-proof-arguments`, task 13, Lean half: the encoding part).

The solver model works over the tree algebra `Tm`. The design's propositions
and sorts are trees over the same symbols, but their encoding was not
modelled. This file models it, so that:

- a constraint over propositions or sorts is representable as a tree, and
- structural identity, the checker's only definitional equality, is exactly
  equality of those trees (`PropT.enc_injective`, `SortT.enc_injective`);
- an encoded proposition is never a metavariable, so it can never be confused
  with a code meta or a value meta (`PropT.enc_ne_mvar`);
- rebasing, the checker's `replace_prop`, acts on exactly the level
  occurrences of a proposition and on nothing else (`PropT.levels_shift`), and
  two rebasings compose (`PropT.shift_comp`). The design's IMPL-CASE-13 exists
  because substituting metas before removing binders rebases twice; the
  composition law is what makes the correct order expressible.

Level occurrences are not only binder indices: `Code.var` inside a
`SortT.value` and inside an `Eq` code is a binder level too, and the checker's
`replace_prop` shifts or replaces it in both modes. `Code.shift`,
`SortT.shift` and `PropT.shift` therefore rewrite those variables as well, and
the level sets include them.

Tags: symbols below 16 name constructors and a level `ℓ` is symbol `16 + ℓ`,
exactly as `Code.enc` and `Val.enc` already do. Sorts use 9, 10, 11 and
propositions 12, 13, 15, so no tag can collide with a level.

The design's `Premise(P)` sort is modelled as a `piPremise` constructor
carrying `P`, so the premise proposition is a field of its binder. `SortT`
itself stays non-recursive, and `PropT` is a single inductive, which is what
lets the two injectivity proofs and the rebasing laws go through directly.

Source-to-Lean translation stays V-INTRINSIC-01.
-/
import NobleFoundation.ImplicitUnification

namespace NobleFoundation.ImplicitPropositions

open NobleFoundation.ImplicitUnification

/-- Sorts of the design: `Type0`, `Value c`, and the premise marker. The
premise proposition is carried by the binder (`PropT.piPremise`). -/
inductive SortT where
  | type0
  | value (c : Code)
  | premise
  deriving DecidableEq, Repr

/-- Propositions of the design: `Pi(level, SortT, body)`, the same with a
premise sort `Premise(P)`, and `Eq c ℓ ℓ`. -/
inductive PropT where
  | pi (level : Nat) (s : SortT) (body : PropT)
  | piPremise (level : Nat) (s : SortT) (premise : PropT) (body : PropT)
  | eq (c : Code) (lo hi : Nat)
  deriving DecidableEq, Repr

def SortT.enc : SortT → Tm
  | .type0 => .sym 9
  | .value c => .app (.sym 10) c.enc
  | .premise => .sym 11

/-- Rebase the variable levels inside a code: the checker's `Code::Variable`
case. Non-variable code is untouched and a code meta is never a level. -/
def codeShift (f : Nat → Nat) : Code → Code
  | .unit => .unit
  | .bool => .bool
  | .i64 => .i64
  | .text => .text
  | .var level => .var (f level)
  | .pair left right => .pair (codeShift f left) (codeShift f right)
  | .sum left right => .sum (codeShift f left) (codeShift f right)
  | .list item => .list (codeShift f item)
  | .mvar n => .mvar n

/-- The variable-level occurrences of a code. -/
def codeLevels : Code → Finset Nat
  | .var level => {level}
  | .pair left right => codeLevels left ∪ codeLevels right
  | .sum left right => codeLevels left ∪ codeLevels right
  | .list item => codeLevels item
  | _ => ∅

/-- Rebase the code a value sort carries. -/
def SortT.shift (f : Nat → Nat) : SortT → SortT
  | .type0 => .type0
  | .value c => .value (codeShift f c)
  | .premise => .premise

/-- The variable-level occurrences of a sort. -/
def SortT.levels : SortT → Finset Nat
  | .type0 => ∅
  | .value c => codeLevels c
  | .premise => ∅

def PropT.enc : PropT → Tm
  | .pi ℓ s body => .app (.app (.sym 12) (.app (.sym (16 + ℓ)) s.enc)) body.enc
  | .piPremise ℓ s premise body =>
      .app (.app (.sym 13) (.app (.app (.sym (16 + ℓ)) s.enc) premise.enc)) body.enc
  | .eq c lo hi =>
      .app (.app (.sym 15) c.enc) (.app (.sym (16 + lo)) (.sym (16 + hi)))

/-- The level occurrences of a proposition: binder levels, `Eq` levels, and the
variable levels inside the codes a `value` sort or an `Eq` carries. -/
def PropT.levels : PropT → Finset Nat
  | .pi ℓ s body => insert ℓ (s.levels ∪ body.levels)
  | .piPremise ℓ s premise body => insert ℓ (s.levels ∪ premise.levels ∪ body.levels)
  | .eq c lo hi => insert lo (insert hi (codeLevels c))

/-- Capture-avoiding rebasing: apply `f` to every level occurrence. This is
the checker's `replace_prop` once a binder is removed. -/
def PropT.shift (f : Nat → Nat) : PropT → PropT
  | .pi ℓ s body => .pi (f ℓ) (s.shift f) (body.shift f)
  | .piPremise ℓ s premise body =>
      .piPremise (f ℓ) (s.shift f) (premise.shift f) (body.shift f)
  | .eq c lo hi => .eq (codeShift f c) (f lo) (f hi)

/-- The head symbol of an encoded term, if it is a symbol application. -/
def headSym : Tm → Option Nat
  | .mvar _ => none
  | .sym s => some s
  | .app f _ => headSym f

theorem SortT.enc_injective : Function.Injective SortT.enc := by
  intro a b h
  cases a <;> cases b <;> simp_all [SortT.enc]
  exact Code.enc_injective (by simpa [SortT.enc] using h)

theorem PropT.enc_injective : Function.Injective PropT.enc := by
  intro a b h
  induction a generalizing b with
  | pi ℓ₁ s₁ body₁ ih =>
      cases b with
      | pi ℓ₂ s₂ body₂ =>
          simp only [PropT.enc] at h
          injection h with hhead hbody
          injection hhead with _ hsort
          rw [Tm.app.injEq, Tm.sym.injEq] at hsort
          obtain ⟨hlsym, hs⟩ := hsort
          have hl : ℓ₁ = ℓ₂ := by omega
          subst hl
          rw [SortT.enc_injective hs, ih hbody]
      | piPremise _ _ _ _ => simp [PropT.enc] at h
      | eq _ _ _ => simp [PropT.enc] at h
  | piPremise ℓ₁ s₁ premise₁ body₁ ihp ihb =>
      cases b with
      | pi _ _ _ => simp [PropT.enc] at h
      | piPremise ℓ₂ s₂ premise₂ body₂ =>
          simp only [PropT.enc] at h
          injection h with hhead hbody
          injection hhead with _ hrest
          injection hrest with hlevel hpremise
          rw [Tm.app.injEq, Tm.sym.injEq] at hlevel
          obtain ⟨hlsym, hs⟩ := hlevel
          have hl : ℓ₁ = ℓ₂ := by omega
          subst hl
          rw [SortT.enc_injective hs, ihp hpremise, ihb hbody]
      | eq _ _ _ => simp [PropT.enc] at h
  | eq c₁ lo₁ hi₁ =>
      cases b with
      | pi _ _ _ => simp [PropT.enc] at h
      | piPremise _ _ _ _ => simp [PropT.enc] at h
      | eq c₂ lo₂ hi₂ =>
          simp only [PropT.enc] at h
          injection h with hpair hhi
          injection hpair with _ hcode
          injection hhi with hlohis hhihis
          rw [Tm.sym.injEq] at hlohis hhihis
          have hlo : lo₁ = lo₂ := by omega
          have hhiq : hi₁ = hi₂ := by omega
          subst hlo
          subst hhiq
          rw [Code.enc_injective hcode]

/-- Two rebasings compose: shifting by `f` and then by `g` is shifting by
`g ∘ f`. This is the law the checker's regression exercises. -/
theorem codeShift_comp (c : Code) (f g : Nat → Nat) :
    codeShift g (codeShift f c) = codeShift (fun ℓ => g (f ℓ)) c := by
  induction c with
  | unit => rfl
  | bool => rfl
  | i64 => rfl
  | text => rfl
  | var level => rfl
  | pair left right leftIH rightIH => simp only [codeShift, leftIH, rightIH]
  | sum left right leftIH rightIH => simp only [codeShift, leftIH, rightIH]
  | list item ih => simp only [codeShift, ih]
  | mvar n => rfl

theorem codeShift_id (c : Code) : codeShift id c = c := by
  induction c with
  | unit => rfl
  | bool => rfl
  | i64 => rfl
  | text => rfl
  | var level => rfl
  | pair left right leftIH rightIH => simp only [codeShift, leftIH, rightIH]
  | sum left right leftIH rightIH => simp only [codeShift, leftIH, rightIH]
  | list item ih => simp only [codeShift, ih]
  | mvar n => rfl

theorem codeLevels_shift (c : Code) (f : Nat → Nat) :
    codeLevels (codeShift f c) = (codeLevels c).image f := by
  induction c with
  | unit => simp [codeShift, codeLevels]
  | bool => simp [codeShift, codeLevels]
  | i64 => simp [codeShift, codeLevels]
  | text => simp [codeShift, codeLevels]
  | var level => simp [codeShift, codeLevels]
  | pair left right leftIH rightIH =>
      simp only [codeShift, codeLevels, leftIH, rightIH, Finset.image_union]
  | sum left right leftIH rightIH =>
      simp only [codeShift, codeLevels, leftIH, rightIH, Finset.image_union]
  | list item ih => simp only [codeShift, codeLevels, ih]
  | mvar n => simp [codeShift, codeLevels]

theorem SortT.shift_comp (s : SortT) (f g : Nat → Nat) :
    (s.shift f).shift g = s.shift (fun ℓ => g (f ℓ)) := by
  cases s with
  | type0 => rfl
  | value c => simp only [SortT.shift, codeShift_comp]
  | premise => rfl

theorem SortT.shift_id (s : SortT) : s.shift id = s := by
  cases s with
  | type0 => rfl
  | value c => simp only [SortT.shift, codeShift_id]
  | premise => rfl

theorem SortT.levels_shift (s : SortT) (f : Nat → Nat) :
    (s.shift f).levels = s.levels.image f := by
  cases s with
  | type0 => simp [SortT.shift, SortT.levels]
  | value c => simp only [SortT.shift, SortT.levels, codeLevels_shift]
  | premise => simp [SortT.shift, SortT.levels]

/-- Two rebasings compose: shifting by `f` and then by `g` is shifting by
`g ∘ f`. This is the law the checker's regression exercises. -/
theorem PropT.shift_comp (p : PropT) (f g : Nat → Nat) :
    (p.shift f).shift g = p.shift (fun ℓ => g (f ℓ)) := by
  induction p with
  | pi ℓ s body ih => simp only [PropT.shift, ih, SortT.shift_comp]
  | piPremise ℓ s premise body ihp ihb =>
      simp only [PropT.shift, ihp, ihb, SortT.shift_comp]
  | eq c lo hi => simp only [PropT.shift, codeShift_comp]

/-- Rebasal by the identity changes nothing. -/
theorem PropT.shift_id (p : PropT) : p.shift id = p := by
  induction p with
  | pi ℓ s body ih => simp [PropT.shift, ih, SortT.shift_id]
  | piPremise ℓ s premise body ihp ihb =>
      simp [PropT.shift, ihp, ihb, SortT.shift_id]
  | eq c lo hi => simp [PropT.shift, codeShift_id]

/-- Rebasing acts on exactly the level occurrences, and on nothing else. -/
theorem PropT.levels_shift (p : PropT) (f : Nat → Nat) :
    (p.shift f).levels = p.levels.image f := by
  induction p with
  | pi ℓ s body ih =>
      simp only [PropT.shift, PropT.levels, ih, SortT.levels_shift, Finset.image_insert,
        Finset.image_union]
  | piPremise ℓ s premise body ihp ihb =>
      simp only [PropT.shift, PropT.levels, ihp, ihb, SortT.levels_shift,
        Finset.image_insert, Finset.image_union]
  | eq c lo hi =>
      simp only [PropT.shift, PropT.levels, codeLevels_shift, Finset.image_insert]

/-- An encoded proposition is never a metavariable, so no code meta or value
meta can be solved by a proposition and no proposition can hide a meta. -/
theorem PropT.enc_ne_mvar (p : PropT) (n : Nat) : p.enc ≠ .mvar n := by
  intro h
  cases p with
  | pi _ _ _ =>
      simp only [PropT.enc] at h
      cases h
  | piPremise _ _ _ _ =>
      simp only [PropT.enc] at h
      cases h
  | eq _ _ _ =>
      simp only [PropT.enc] at h
      cases h

/-- Encoded propositions are symbol-headed, which is why they are distinct
from every metavariable encoding. -/
theorem PropT.head_enc (p : PropT) :
    headSym p.enc = some 12 ∨ headSym p.enc = some 13 ∨ headSym p.enc = some 15 := by
  cases p with
  | pi _ _ _ => simp [PropT.enc, headSym]
  | piPremise _ _ _ _ => simp [PropT.enc, headSym]
  | eq _ _ _ => simp [PropT.enc, headSym]

/-! Fixtures from the design, decided by evaluation. -/

/-- P2's head telescope at depth 1:
`Pi 1 Type0 (Pi 2 (Value (Var 1)) (Eq (Var 1) 2 2))`. Its level
occurrences are 1 and 2 and nothing else. -/
def p2 : PropT :=
  .pi 1 .type0 (.pi 2 (.value (.var 1)) (.eq (.var 1) 2 2))

/-- The level occurrences of P2's proposition are exactly 1 and 2. -/
example : p2.levels = {1, 2} := by decide

/-- Rebasing P2 by predecessor maps its levels, and nothing else. -/
example : (p2.shift Nat.pred).levels = {0, 1} := by decide

/-- Rebasing moves the code variables too: after taking the predecessor, `Var 0` names the
`Value` binder. The previous model left `Var 1` here, which then named the `Value` binder itself —
the reviewer's counterexample, and the reason `replace_prop` shifts `Code::Variable`. -/
example : p2.shift Nat.pred =
    .pi 0 .type0 (.pi 1 (.value (.var 0)) (.eq (.var 0) 1 1)) := by decide

/-- An `Eq` code's variable level is a level occurrence, collected with the `Eq` indices. -/
example : (PropT.eq (.var 4) 5 6).levels = {4, 5, 6} := by decide

/-- Rebasing an `Eq` shifts the code's variable along with the two indices. -/
example : (PropT.eq (.var 4) 5 6).shift Nat.succ = .eq (.var 5) 6 7 := by decide

/-- A premise proposition is a proposition, never a metavariable. -/
example : (PropT.eq (.list .unit) 0 0).enc ≠ .mvar 0 := PropT.enc_ne_mvar _ 0

end NobleFoundation.ImplicitPropositions
