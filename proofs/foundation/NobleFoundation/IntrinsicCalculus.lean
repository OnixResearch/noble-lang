/-
The typed source proof calculus of `Intrinsic-Proofs-Draft`
(`intrinsic-noble-proofs`, tasks 2 and 3, the model half).

`crates/noble-contracts/src/intrinsic.rs` checks a source proof term with
`proof_walk`: `use`, `apply`, `refl`, `intro` and `subst`, rebasing a
proposition with `replace_prop` whenever a binder is eliminated. This file
models that calculus, so that its acceptance means something:

- `Code` is the checker's code tree, already modelled by
  `NobleFoundation.ImplicitUnification`; `BinderSort`, `Proposition` and
  `Proof` are the checker's sorts, propositions and resolved proof terms.
- `Proposition.rebase` is `replace_prop`/`shifted_prop` at a level map, and
  `Proposition.toPropT_rebase` shows it is exactly
  `ImplicitPropositions.PropT.shift`. That module's encoding, its injectivity
  and its rebasing laws therefore apply to this calculus unchanged
  (`Proposition.enc_injective`, `Proposition.rebase_comp`).
- `Proposition.denote` interprets a proposition over an interpretation of its
  binder context: `Type0` ranges over the total pure `PureTyCode` and a total
  value of a code is an element of `El`, both exactly as
  `IntrinsicTypeWitness` defines them. `Eq` says that two total values of one
  code are equal, so `refl` introduces exactly equality and `subst` eliminates
  it along a well-formed total motive.
- `Infers` and `Checks` are the checker's inference and checking judgements,
  including `apply`'s rebasing substitution and `subst`'s equality motive.
  `Proposition.denote_subst` is the substitution law they rely on.
- `Infers.sound` and `Checks.sound` are the typing-soundness theorems: every
  proposition the calculus establishes is true at every interpretation of the
  context it was established in.

It is a model claim. It says nothing about the Rust parser, resolver or
lowerer, about the Lean text that `lean_text` renders, or about actual Rust
refinement: source-to-Lean translation correctness is V-INTRINSIC-01 and
remains open. The source checker resolves names only, so the code and
proposition formation judgements below are the typed fragment the design
requires and a resolver alone does not check; `use` of a previously accepted
declaration is a trust boundary and enters the model as an explicit premise.
-/
import IntrinsicTypeWitness
import NobleFoundation.ImplicitPropositions

namespace NobleFoundation.IntrinsicCalculus

open NobleFoundation.ImplicitUnification
open NobleFoundation.ImplicitPropositions (PropT SortT codeShift codeLevels)
open NobleContracts.Intrinsic (PureTyCode El)

/-- The code variables a code mentions, as `ImplicitPropositions` collects
them. A local name, because `Proposition.codeLevels` below would shadow it. -/
def codeVars (c : Code) : Finset Nat := NobleFoundation.ImplicitPropositions.codeLevels c

/-! ## The deep embedding -/

/-- The propositions of the source calculus: a dependent Pi over a code
(`Type0`), over a total value of a code, and over a local premise, and equality
of two total values of one code. -/
inductive Proposition where
  | piCodes (level : Nat) (body : Proposition)
  | piValue (level : Nat) (code : Code) (body : Proposition)
  | piPremise (level : Nat) (premise : Proposition) (body : Proposition)
  | eq (code : Code) (lo hi : Nat)
  deriving DecidableEq, Repr

/-- The sort a source binder has: the type layer, a total value of a code, or a
local proof premise. -/
inductive BinderSort where
  | codes
  | value (code : Code)
  | premise (prop : Proposition)
  deriving DecidableEq, Repr

/-- What an eliminated binder is replaced by: a code instantiation, a value
binder, or a premise (which removes the binder's position and writes no level).
This is the checker's `Argument`. -/
inductive Argument where
  | code (ty : Code)
  | value (level : Nat)
  | premise
  deriving DecidableEq, Repr

/-- Source proof terms after name resolution: exactly the forms `proof_walk`
dispatches on. A premise argument carries its own subproof, so `apply` has the
checker's two argument paths. -/
inductive Proof where
  | useLocal (level : Nat)
  | useDeclared (goal : Proposition)
  | apply (fn : Proof) (arg : Argument)
  | applyPremise (fn : Proof) (premise : Proof)
  | refl (level : Nat)
  | intro (sort : BinderSort) (body : Proof)
  | subst (code : Code) (motive : Proposition) (evidence : Proof) (premise : Proof)
  deriving Repr

/-! ## Rebasing, and its reuse of the implicit-propositions theory -/

/-- The sort a binder sort denotes in `ImplicitPropositions`. -/
def BinderSort.toSortT : BinderSort → SortT
  | .codes => .type0
  | .value c => .value c
  | .premise _ => .premise

/-- The `ImplicitPropositions` proposition this calculus's proposition encodes
to. A premise Pi carries its premise proposition in `PropT.piPremise`, which
keeps `SortT` non-recursive. -/
def Proposition.toPropT : Proposition → PropT
  | .piCodes ℓ body => .pi ℓ .type0 body.toPropT
  | .piValue ℓ c body => .pi ℓ (.value c) body.toPropT
  | .piPremise ℓ p body => .piPremise ℓ .premise p.toPropT body.toPropT
  | .eq c lo hi => .eq c lo hi

/-- The inverse of `Proposition.toPropT` on the shapes this calculus's source
grammar can produce. -/
def Proposition.ofPropT : PropT → Option Proposition
  | .pi ℓ .type0 body => (Proposition.ofPropT body).map (.piCodes ℓ)
  | .pi ℓ (.value c) body => (Proposition.ofPropT body).map (.piValue ℓ c)
  | .pi _ .premise _ => none
  | .piPremise ℓ _ p body =>
      match Proposition.ofPropT p, Proposition.ofPropT body with
      | some p', some b' => some (.piPremise ℓ p' b')
      | _, _ => none
  | .eq c lo hi => some (.eq c lo hi)

theorem Proposition.ofPropT_toPropT (p : Proposition) :
    Proposition.ofPropT p.toPropT = some p := by
  induction p with
  | piCodes ℓ body ih => simp [Proposition.toPropT, Proposition.ofPropT, ih]
  | piValue ℓ c body ih => simp [Proposition.toPropT, Proposition.ofPropT, ih]
  | piPremise ℓ p body ihp ihb =>
      simp [Proposition.toPropT, Proposition.ofPropT, ihp, ihb]
  | eq c lo hi => rfl

/-- Two propositions are equal when their `PropT` encodings are, so the
encoding carries the checker's structural identity exactly. -/
theorem Proposition.toPropT_injective : Function.Injective Proposition.toPropT := by
  intro a b h
  have ha : some a = Proposition.ofPropT (Proposition.toPropT b) := by
    rw [← Proposition.ofPropT_toPropT a, h]
  rw [Proposition.ofPropT_toPropT b] at ha
  exact Option.some.inj ha

/-- Capture-avoiding rebasing at a level map: the checker's `replace_prop` in
`Shift` mode at `base` is `rebase (· + base)`. Only binder levels, equality
indices and code variables move. -/
def Proposition.rebase (f : Nat → Nat) : Proposition → Proposition
  | .piCodes ℓ body => .piCodes (f ℓ) (body.rebase f)
  | .piValue ℓ c body => .piValue (f ℓ) (codeShift f c) (body.rebase f)
  | .piPremise ℓ p body => .piPremise (f ℓ) (p.rebase f) (body.rebase f)
  | .eq c lo hi => .eq (codeShift f c) (f lo) (f hi)

/-- Rebasing commutes with the encoding: it is exactly `PropT.shift`. -/
theorem Proposition.toPropT_rebase (p : Proposition) (f : Nat → Nat) :
    (p.rebase f).toPropT = (p.toPropT).shift f := by
  induction p with
  | piCodes ℓ body ih =>
      simp [Proposition.rebase, Proposition.toPropT, ih, PropT.shift, SortT.shift]
  | piValue ℓ c body ih =>
      simp [Proposition.rebase, Proposition.toPropT, ih, PropT.shift, SortT.shift]
  | piPremise ℓ p body ihp ihb =>
      simp [Proposition.rebase, Proposition.toPropT, ihp, ihb, PropT.shift, SortT.shift]
  | eq c lo hi => simp [Proposition.rebase, Proposition.toPropT, PropT.shift]

/-- Two rebasings compose, through `PropT.shift_comp`. -/
theorem Proposition.rebase_comp (p : Proposition) (f g : Nat → Nat) :
    (p.rebase f).rebase g = p.rebase (fun ℓ => g (f ℓ)) :=
  Proposition.toPropT_injective (by
    rw [Proposition.toPropT_rebase, Proposition.toPropT_rebase, PropT.shift_comp,
      Proposition.toPropT_rebase])

/-- Rebasing by the identity changes nothing. -/
theorem Proposition.rebase_id (p : Proposition) : p.rebase id = p :=
  Proposition.toPropT_injective (by
    rw [Proposition.toPropT_rebase]
    exact PropT.shift_id _)

/-- The level occurrences of a proposition, exactly as
`ImplicitPropositions.PropT.levels` computes them. -/
def Proposition.levels (p : Proposition) : Finset Nat := p.toPropT.levels

/-- Rebasing acts on exactly the level occurrences and on nothing else. -/
theorem Proposition.levels_rebase (p : Proposition) (f : Nat → Nat) :
    (p.rebase f).levels = p.levels.image f := by
  simp only [Proposition.levels, Proposition.toPropT_rebase, PropT.levels_shift]

/-- The tree encoding of a proposition, for the structural-identity claim that
the checker's only definitional equality is equality of these trees. -/
def Proposition.enc (p : Proposition) : Tm := p.toPropT.enc

theorem Proposition.enc_injective : Function.Injective Proposition.enc :=
  fun _ _ h => Proposition.toPropT_injective (PropT.enc_injective h)

/-- Whether a proposition is an equality, which is the shape a `subst` motive
must have. -/
def Proposition.IsEq : Proposition → Prop
  | .eq _ _ _ => True
  | _ => False

/-! ## Interpretations of levels -/

/-- What a level names: a code binder's total pure type, a total value of one,
or a premise slot, which names no value. -/
inductive Binding where
  | code (ty : PureTyCode)
  | value (ty : PureTyCode) (v : El ty)
  | premise

/-- Insert `b` at level `n`, moving every later level up one. This is what a Pi
elimination writes into an interpretation; at `n = env.length` it appends. -/
def insertAt : Nat → Binding → List Binding → List Binding
  | 0, b, env => b :: env
  | _ + 1, b, [] => [b]
  | n + 1, b, x :: xs => x :: insertAt n b xs

theorem insertAt_length {n : Nat} {b : Binding} {env : List Binding} (hn : n ≤ env.length) :
    (insertAt n b env).length = env.length + 1 := by
  induction n generalizing env with
  | zero => simp [insertAt]
  | succ n ih =>
      cases env with
      | nil => exact absurd hn (by simp)
      | cons x xs =>
          have hn' : n ≤ xs.length := by
            simp only [List.length_cons] at hn
            omega
          simp only [insertAt, List.length_cons, ih hn']

theorem insertAt_getElem?_lt {n : Nat} {b : Binding} {env : List Binding} (hn : n ≤ env.length)
    {i : Nat} (hi : i < n) : (insertAt n b env)[i]? = env[i]? := by
  induction n generalizing env i with
  | zero => omega
  | succ n ih =>
      cases env with
      | nil => exact absurd hn (by simp)
      | cons x xs =>
          have hn' : n ≤ xs.length := by
            simp only [List.length_cons] at hn
            omega
          cases i with
          | zero => simp [insertAt]
          | succ i =>
              have hi' : i < n := by omega
              have h1 : i + 1 ≠ 0 := by omega
              have h2 : i + 1 - 1 = i := by omega
              simp only [insertAt, List.getElem?_cons, ite_eq_right h1, h2]
              exact ih hn' hi'

theorem insertAt_getElem?_self {n : Nat} {b : Binding} {env : List Binding}
    (hn : n ≤ env.length) : (insertAt n b env)[n]? = some b := by
  induction n generalizing env with
  | zero =>
      cases env with
      | nil => simp [insertAt]
      | cons _ _ => simp [insertAt]
  | succ n ih =>
      cases env with
      | nil => exact absurd hn (by simp)
      | cons x xs =>
          have hn' : n ≤ xs.length := by
            simp only [List.length_cons] at hn
            omega
          have h1 : n + 1 ≠ 0 := by omega
          have h2 : n + 1 - 1 = n := by omega
          simp only [insertAt, List.getElem?_cons, ite_eq_right h1, h2]
          exact ih hn'

theorem insertAt_getElem?_gt {n : Nat} {b : Binding} {env : List Binding} (hn : n ≤ env.length)
    {i : Nat} (hi : n < i) : (insertAt n b env)[i]? = env[i - 1]? := by
  induction n generalizing env i with
  | zero =>
      cases i with
      | zero => omega
      | succ i =>
          cases env with
          | nil => simp [insertAt]
          | cons x xs =>
              have h1 : i + 1 ≠ 0 := by omega
              have h2 : i + 1 - 1 = i := by omega
              simp only [insertAt, List.getElem?_cons, ite_eq_right h1, h2]
  | succ n ih =>
      cases env with
      | nil => exact absurd hn (by simp)
      | cons x xs =>
          have hn' : n ≤ xs.length := by
            simp only [List.length_cons] at hn
            omega
          cases i with
          | zero => omega
          | succ i =>
              have hi' : n < i := by omega
              have h1 : i + 1 ≠ 0 := by omega
              have h2 : i + 1 - 1 = i := by omega
              have h0 : ¬ i = 0 := by omega
              simp only [insertAt, List.getElem?_cons, ite_eq_right h1, h2, ite_eq_right h0]
              exact ih hn' hi'

theorem insertAt_self {b : Binding} {env : List Binding} :
    insertAt env.length b env = env ++ [b] := by
  induction env with
  | nil => rfl
  | cons x xs ih =>
      show x :: insertAt xs.length b xs = x :: (xs ++ [b])
      rw [ih]

theorem insertAt_append {n : Nat} {b : Binding} {env : List Binding} {xs : List Binding}
    (hn : n ≤ env.length) : insertAt n b (env ++ xs) = insertAt n b env ++ xs := by
  induction n generalizing env with
  | zero => rfl
  | succ n ih =>
      cases env with
      | nil => exact absurd hn (by simp)
      | cons x ys =>
          have hn' : n ≤ ys.length := by
            simp only [List.length_cons] at hn
            omega
          show x :: insertAt n b (ys ++ xs) = x :: (insertAt n b ys ++ xs)
          rw [ih hn']

/-- Two insertions at different levels commute, the outer one moving up one. -/
theorem insertAt_comm {b c : Binding} {env : List Binding} {n m : Nat} (hnm : n ≤ m)
    (hm : m ≤ env.length) :
    insertAt n b (insertAt m c env) = insertAt (m + 1) c (insertAt n b env) := by
  induction n generalizing env m with
  | zero =>
      cases env with
      | nil =>
          have hm0 : m = 0 := by simp only [List.length_nil] at hm; omega
          subst hm0
          rfl
      | cons x xs =>
          show b :: insertAt m c (x :: xs) = b :: insertAt m c (x :: xs)
          rfl
  | succ n ih =>
      cases m with
      | zero => omega
      | succ m' =>
          cases env with
          | nil => simp only [List.length_nil] at hm; omega
          | cons x xs =>
              have hmlen : m' ≤ xs.length := by
                simp only [List.length_cons] at hm
                omega
              show x :: insertAt n b (insertAt m' c xs) =
                x :: insertAt (m' + 1) c (insertAt n b xs)
              rw [ih (by omega) hmlen]

/-! ## Denotation -/

/-- The denotation of a code: the total pure type it names, when every code
variable resolves to a code binder. A metavariable and a variable of any other
sort denote nothing, which is the ill-formed fragment's refusal. -/
def codeDenote (env : List Binding) : Code → Option PureTyCode
  | .unit => some .unit
  | .bool => some .bool
  | .i64 => some .i64
  | .text => some .text
  | .var ℓ =>
      match env[ℓ]? with
      | some (.code ty) => some ty
      | _ => none
  | .pair a b =>
      (match codeDenote env a, codeDenote env b with
       | some x, some y => some (.pair x y)
       | _, _ => none)
  | .sum a b =>
      (match codeDenote env a, codeDenote env b with
       | some x, some y => some (.sum x y)
       | _, _ => none)
  | .list a => (codeDenote env a).map .list
  | .mvar _ => none

/-- The denotation of a proposition at an interpretation. A Pi quantifies over
what its binder may name and writes that at the binder's own level; `Eq` says
the two levels name equal total values of one code. -/
def Proposition.denote (env : List Binding) : Proposition → Prop
  | .piCodes ℓ body => ∀ (ty : PureTyCode), body.denote (insertAt ℓ (.code ty) env)
  | .piValue ℓ c body =>
      ∀ (ty : PureTyCode), codeDenote env c = some ty →
        ∀ v : El ty, body.denote (insertAt ℓ (.value ty v) env)
  | .piPremise ℓ p body => p.denote env → body.denote (insertAt ℓ .premise env)
  | .eq c lo hi =>
      ∃ (ty : PureTyCode) (v w : El ty),
        codeDenote env c = some ty ∧ env[lo]? = some (.value ty v) ∧
          env[hi]? = some (.value ty w) ∧ v = w

/-! ## Formation of codes and propositions -/

/-- A code is formed in a context when it has no metavariable and every one of
its variables names a code binder. -/
inductive CodeFormed : List BinderSort → Code → Prop
  | unit (Γ : List BinderSort) : CodeFormed Γ .unit
  | bool (Γ : List BinderSort) : CodeFormed Γ .bool
  | i64 (Γ : List BinderSort) : CodeFormed Γ .i64
  | text (Γ : List BinderSort) : CodeFormed Γ .text
  | var {Γ : List BinderSort} {ℓ : Nat} : Γ[ℓ]? = some .codes → CodeFormed Γ (.var ℓ)
  | pair {Γ : List BinderSort} {a b : Code} :
      CodeFormed Γ a → CodeFormed Γ b → CodeFormed Γ (.pair a b)
  | sum {Γ : List BinderSort} {a b : Code} :
      CodeFormed Γ a → CodeFormed Γ b → CodeFormed Γ (.sum a b)
  | list {Γ : List BinderSort} {a : Code} : CodeFormed Γ a → CodeFormed Γ (.list a)

/-- Code formation only depends on code binders, so it is preserved by any
extension of the context. -/
theorem CodeFormed.mono {Γ : List BinderSort} {c : Code} (h : CodeFormed Γ c)
    (xs : List BinderSort) : CodeFormed (Γ ++ xs) c := by
  induction h with
  | unit => exact CodeFormed.unit _
  | bool => exact CodeFormed.bool _
  | i64 => exact CodeFormed.i64 _
  | text => exact CodeFormed.text _
  | var hlook =>
      have hlt : _ < Γ.length := (List.getElem?_eq_some_iff.1 hlook).1
      exact CodeFormed.var (by rw [List.getElem?_append_left hlt]; exact hlook)
  | pair _ _ iha ihb => exact CodeFormed.pair iha ihb
  | sum _ _ iha ihb => exact CodeFormed.sum iha ihb
  | list _ iha => exact CodeFormed.list iha

/-- Every code variable of a formed code names a code binder. -/
theorem CodeFormed.codes_of_mem {Γ : List BinderSort} {c : Code} (h : CodeFormed Γ c)
    {ℓ : Nat} (hmem : ℓ ∈ codeVars c) : Γ[ℓ]? = some .codes := by
  induction h with
  | unit => simp [codeVars, codeLevels] at hmem
  | bool => simp [codeVars, codeLevels] at hmem
  | i64 => simp [codeVars, codeLevels] at hmem
  | text => simp [codeVars, codeLevels] at hmem
  | var hlook =>
      simp only [codeVars, codeLevels, Finset.mem_singleton] at hmem
      subst hmem
      exact hlook
  | pair _ _ iha ihb =>
      simp only [codeVars, codeLevels, Finset.mem_union] at hmem
      exact hmem.elim iha ihb
  | sum _ _ iha ihb =>
      simp only [codeVars, codeLevels, Finset.mem_union] at hmem
      exact hmem.elim iha ihb
  | list _ iha =>
      simp only [codeVars, codeLevels] at hmem
      exact iha hmem

/-- The code variables a proposition mentions. -/
def Proposition.codeLevels : Proposition → Finset Nat
  | .piCodes _ body => body.codeLevels
  | .piValue _ c body => codeVars c ∪ body.codeLevels
  | .piPremise _ p body => p.codeLevels ∪ body.codeLevels
  | .eq c _ _ => codeVars c

/-- The equality indices of a proposition. -/
def Proposition.eqIndices : Proposition → Finset Nat
  | .piCodes _ body => body.eqIndices
  | .piValue _ _ body => body.eqIndices
  | .piPremise _ p body => p.eqIndices ∪ body.eqIndices
  | .eq _ lo hi => {lo, hi}

/-- A proposition is formed in a context when each of its Pis binds exactly the
next level, each of its codes is formed, each equality index names a total
value, and a premise Pi's premise is formed. -/
inductive PropositionFormed : List BinderSort → Proposition → Prop
  | piCodes {Γ : List BinderSort} {ℓ : Nat} {body : Proposition} :
      ℓ = Γ.length → PropositionFormed (Γ ++ [.codes]) body →
        PropositionFormed Γ (.piCodes ℓ body)
  | piValue {Γ : List BinderSort} {ℓ : Nat} {c : Code} {body : Proposition} :
      ℓ = Γ.length → CodeFormed Γ c → PropositionFormed (Γ ++ [.value c]) body →
        PropositionFormed Γ (.piValue ℓ c body)
  | piPremise {Γ : List BinderSort} {ℓ : Nat} {p body : Proposition} :
      ℓ = Γ.length → PropositionFormed Γ p → PropositionFormed (Γ ++ [.premise p]) body →
        PropositionFormed Γ (.piPremise ℓ p body)
  | eq {Γ : List BinderSort} {c : Code} {lo hi : Nat} :
      CodeFormed Γ c → (∃ c₁, Γ[lo]? = some (.value c₁)) →
        (∃ c₂, Γ[hi]? = some (.value c₂)) → PropositionFormed Γ (.eq c lo hi)

/-- Every code variable of a formed proposition that is bound in the context is
a code binder. A level the proposition's own Pis introduce is not in that
context, which is why the bound is stated. -/
theorem PropositionFormed.codes_of_mem {Γ : List BinderSort} {p : Proposition}
    (h : PropositionFormed Γ p) {ℓ : Nat} (hmem : ℓ ∈ p.codeLevels) (hlt : ℓ < Γ.length) :
    Γ[ℓ]? = some .codes := by
  induction h with
  | @piCodes Γ₀ ℓ₀ body _ _ ih =>
      have hlen : ℓ < (Γ₀ ++ [BinderSort.codes]).length := by
        simp only [List.length_append, List.length_cons, List.length_nil]
        omega
      have hres := ih hmem hlen
      rwa [List.getElem?_append_left (l₂ := [BinderSort.codes]) hlt] at hres
  | @piValue Γ₀ ℓ₀ c body _ hc _ ih =>
      simp only [Proposition.codeLevels, Finset.mem_union] at hmem
      rcases hmem with hmem | hmem
      · exact CodeFormed.codes_of_mem hc hmem
      · have hlen : ℓ < (Γ₀ ++ [BinderSort.value c]).length := by
          simp only [List.length_append, List.length_cons, List.length_nil]
          omega
        have hres := ih hmem hlen
        rwa [List.getElem?_append_left (l₂ := [BinderSort.value c]) hlt] at hres
  | @piPremise Γ₀ ℓ₀ p₀ body _ _ _ ihp ihb =>
      simp only [Proposition.codeLevels, Finset.mem_union] at hmem
      rcases hmem with hmem | hmem
      · exact ihp hmem hlt
      · have hlen : ℓ < (Γ₀ ++ [BinderSort.premise p₀]).length := by
          simp only [List.length_append, List.length_cons, List.length_nil]
          omega
        have hres := ihb hmem hlen
        rwa [List.getElem?_append_left (l₂ := [BinderSort.premise p₀]) hlt] at hres
  | eq hc _ _ => exact CodeFormed.codes_of_mem hc hmem

/-- Every equality index of a formed proposition that is bound in the context
names a total value binder. -/
theorem PropositionFormed.value_of_mem {Γ : List BinderSort} {p : Proposition}
    (h : PropositionFormed Γ p) {ℓ : Nat} (hmem : ℓ ∈ p.eqIndices) (hlt : ℓ < Γ.length) :
    ∃ c, Γ[ℓ]? = some (.value c) := by
  induction h with
  | @piCodes Γ₀ ℓ₀ body _ _ ih =>
      have hlen : ℓ < (Γ₀ ++ [BinderSort.codes]).length := by
        simp only [List.length_append, List.length_cons, List.length_nil]
        omega
      obtain ⟨c, hc⟩ := ih hmem hlen
      exact ⟨c, by rwa [List.getElem?_append_left (l₂ := [BinderSort.codes]) hlt] at hc⟩
  | @piValue Γ₀ ℓ₀ c body _ _ _ ih =>
      have hlen : ℓ < (Γ₀ ++ [BinderSort.value c]).length := by
        simp only [List.length_append, List.length_cons, List.length_nil]
        omega
      obtain ⟨c', hc'⟩ := ih hmem hlen
      exact ⟨c', by rwa [List.getElem?_append_left (l₂ := [BinderSort.value c]) hlt] at hc'⟩
  | @piPremise Γ₀ ℓ₀ p₀ body _ _ _ ihp ihb =>
      simp only [Proposition.eqIndices, Finset.mem_union] at hmem
      rcases hmem with hmem | hmem
      · exact ihp hmem hlt
      · have hlen : ℓ < (Γ₀ ++ [BinderSort.premise p₀]).length := by
          simp only [List.length_append, List.length_cons, List.length_nil]
          omega
        obtain ⟨c', hc'⟩ := ihb hmem hlen
        exact ⟨c', by
          rwa [List.getElem?_append_left (l₂ := [BinderSort.premise p₀]) hlt] at hc'⟩
  | @eq Γ₀ c lo hi _ hlo hhi =>
      simp only [Proposition.eqIndices, Finset.mem_insert, Finset.mem_singleton] at hmem
      exact hmem.elim (fun h => h ▸ hlo) (fun h => h ▸ hhi)

/-- Denotation of a formed code only depends on the levels it reads. -/
theorem codeDenote_agree {Γ : List BinderSort} {c : Code} (h : CodeFormed Γ c)
    {env env' : List Binding} (hagree : ∀ ℓ, ℓ < Γ.length → env[ℓ]? = env'[ℓ]?) :
    codeDenote env c = codeDenote env' c := by
  induction h with
  | unit => rfl
  | bool => rfl
  | i64 => rfl
  | text => rfl
  | var hlook =>
      have hlt : _ < Γ.length := (List.getElem?_eq_some_iff.1 hlook).1
      have hstep := hagree _ hlt
      simp only [codeDenote]
      rw [hstep]
  | pair _ _ iha ihb =>
      simp only [codeDenote, iha, ihb]
  | sum _ _ iha ihb =>
      simp only [codeDenote, iha, ihb]
  | list _ iha =>
      simp only [codeDenote, iha]

/-- Denotation of a formed proposition only depends on the levels it reads. -/
theorem Proposition.denote_agree {Γ : List BinderSort} {p : Proposition}
    (h : PropositionFormed Γ p) :
    ∀ {env env' : List Binding}, Γ.length ≤ env.length → Γ.length ≤ env'.length →
      (∀ ℓ, ℓ < Γ.length → env[ℓ]? = env'[ℓ]?) → (p.denote env ↔ p.denote env') := by
  induction h with
  | @piCodes Γ₀ ℓ body hℓ _ ih =>
      subst hℓ
      rintro env env' h1 h2 hagree
      have hlen : (Γ₀ ++ [BinderSort.codes]).length = Γ₀.length + 1 := by simp
      have hstep : ∀ (ty : PureTyCode), ∀ j, j < (Γ₀ ++ [BinderSort.codes]).length →
          (insertAt Γ₀.length (Binding.code ty) env)[j]? =
            (insertAt Γ₀.length (Binding.code ty) env')[j]? := by
        intro ty j hj
        by_cases hlt : j < Γ₀.length
        · rw [insertAt_getElem?_lt h1 hlt, insertAt_getElem?_lt h2 hlt]
          exact hagree j hlt
        · have hje : j = Γ₀.length := by rw [hlen] at hj; omega
          rw [hje, insertAt_getElem?_self h1, insertAt_getElem?_self h2]
      constructor
      · intro hp ty
        refine (ih (by rw [insertAt_length h1]; rw [hlen]; omega)
          (by rw [insertAt_length h2]; rw [hlen]; omega) (hstep ty)).1 (hp ty)
      · intro hp ty
        refine (ih (by rw [insertAt_length h1]; rw [hlen]; omega)
          (by rw [insertAt_length h2]; rw [hlen]; omega) (hstep ty)).2 (hp ty)
  | @piValue Γ₀ ℓ c body hℓ hc _ ih =>
      subst hℓ
      rintro env env' h1 h2 hagree
      have hcode : codeDenote env c = codeDenote env' c := codeDenote_agree hc hagree
      have hlen : (Γ₀ ++ [BinderSort.value c]).length = Γ₀.length + 1 := by simp
      have hstep : ∀ (ty : PureTyCode) (v : El ty), ∀ j,
          j < (Γ₀ ++ [BinderSort.value c]).length →
            (insertAt Γ₀.length (Binding.value ty v) env)[j]? =
              (insertAt Γ₀.length (Binding.value ty v) env')[j]? := by
        intro ty v j hj
        by_cases hlt : j < Γ₀.length
        · rw [insertAt_getElem?_lt h1 hlt, insertAt_getElem?_lt h2 hlt]
          exact hagree j hlt
        · have hje : j = Γ₀.length := by rw [hlen] at hj; omega
          rw [hje, insertAt_getElem?_self h1, insertAt_getElem?_self h2]
      have hbound : ∀ (ty : PureTyCode) (v : El ty),
          (Γ₀ ++ [BinderSort.value c]).length ≤ (insertAt Γ₀.length (Binding.value ty v) env).length ∧
          (Γ₀ ++ [BinderSort.value c]).length ≤ (insertAt Γ₀.length (Binding.value ty v) env').length := by
        intro ty v
        rw [insertAt_length h1, insertAt_length h2, hlen]
        exact ⟨by omega, by omega⟩
      constructor
      · intro hp ty hden v
        exact (ih (hbound ty v).1 (hbound ty v).2 (hstep ty v)).1
          (hp ty (by rw [hcode]; exact hden) v)
      · intro hp ty hden v
        exact (ih (hbound ty v).1 (hbound ty v).2 (hstep ty v)).2
          (hp ty (by rw [← hcode]; exact hden) v)
  | @piPremise Γ₀ ℓ p₀ body hℓ hform _ ihp ihb =>
      subst hℓ
      rintro env env' h1 h2 hagree
      have hprem : p₀.denote env ↔ p₀.denote env' := ihp h1 h2 hagree
      have hlen : (Γ₀ ++ [BinderSort.premise p₀]).length = Γ₀.length + 1 := by simp
      have hstep : ∀ j, j < (Γ₀ ++ [BinderSort.premise p₀]).length →
          (insertAt Γ₀.length Binding.premise env)[j]? =
            (insertAt Γ₀.length Binding.premise env')[j]? := by
        intro j hj
        by_cases hlt : j < Γ₀.length
        · rw [insertAt_getElem?_lt h1 hlt, insertAt_getElem?_lt h2 hlt]
          exact hagree j hlt
        · have hje : j = Γ₀.length := by rw [hlen] at hj; omega
          rw [hje, insertAt_getElem?_self h1, insertAt_getElem?_self h2]
      have hbound : (Γ₀ ++ [BinderSort.premise p₀]).length ≤
          (insertAt Γ₀.length Binding.premise env).length ∧
          (Γ₀ ++ [BinderSort.premise p₀]).length ≤
            (insertAt Γ₀.length Binding.premise env').length := by
        rw [insertAt_length h1, insertAt_length h2, hlen]
        exact ⟨by omega, by omega⟩
      constructor
      · intro hgoal hpre
        exact (ihb hbound.1 hbound.2 hstep).1 (hgoal (hprem.2 hpre))
      · intro hgoal hpre
        exact (ihb hbound.1 hbound.2 hstep).2 (hgoal (hprem.1 hpre))
  | @eq Γ₀ c lo hi hc hlo hhi =>
      rintro env env' h1 h2 hagree
      have hcode : codeDenote env c = codeDenote env' c := codeDenote_agree hc hagree
      have hlo' : lo < Γ₀.length := (List.getElem?_eq_some_iff.1 hlo.choose_spec).1
      have hhi' : hi < Γ₀.length := (List.getElem?_eq_some_iff.1 hhi.choose_spec).1
      have hlv := hagree lo hlo'
      have hhv := hagree hi hhi'
      constructor
      · rintro ⟨ty, v, w, hden, hl, hh, hvw⟩
        exact ⟨ty, v, w, by rw [← hcode]; exact hden,
          by rw [← hlv]; exact hl, by rw [← hhv]; exact hh, hvw⟩
      · rintro ⟨ty, v, w, hden, hl, hh, hvw⟩
        exact ⟨ty, v, w, by rw [hcode]; exact hden,
          by rw [hlv]; exact hl, by rw [hhv]; exact hh, hvw⟩

/-! ## Interpretations of a binder context -/

/-- A local premise binder's meaning: the proposition it proves holds at an
interpretation of its own context, which agrees with this one below that
context. A premise is checked at the context it was introduced in, so the
model records that context rather than claiming the proposition holds at every
later one by definition. -/
structure PremiseHolds (p : Proposition) (env : List Binding) where
  ctx : List BinderSort
  base : List Binding
  formed : PropositionFormed ctx p
  length : base.length = ctx.length
  depth : ctx.length ≤ env.length
  agree : ∀ ℓ, ℓ < ctx.length → base[ℓ]? = env[ℓ]?
  holds : p.denote base

theorem PremiseHolds.denote {p : Proposition} {env : List Binding} (h : PremiseHolds p env) :
    p.denote env :=
  (Proposition.denote_agree h.formed (by rw [h.length]) h.depth h.agree).1 h.holds

def PremiseHolds.append {p : Proposition} {env : List Binding} (h : PremiseHolds p env)
    (xs : List Binding) : PremiseHolds p (env ++ xs) where
  ctx := h.ctx
  base := h.base
  formed := h.formed
  length := h.length
  depth := by
    have hd := h.depth
    simp only [List.length_append]
    omega
  agree := fun ℓ hℓ => by
    have hb := h.agree ℓ hℓ
    have hd := h.depth
    rw [hb, List.getElem?_append_left (by omega)]
  holds := h.holds

/-- An interpretation of a binder context: every level names what its sort says,
and a premise binder's proposition holds. -/
inductive CtxHolds : List BinderSort → List Binding → Prop
  | nil : CtxHolds [] []
  | codes {Γ : List BinderSort} {env : List Binding} (ty : PureTyCode) :
      CtxHolds Γ env → CtxHolds (Γ ++ [.codes]) (env ++ [.code ty])
  | value {Γ : List BinderSort} {env : List Binding} (c : Code) (ty : PureTyCode) (v : El ty) :
      CtxHolds Γ env → CodeFormed Γ c → codeDenote env c = some ty →
        CtxHolds (Γ ++ [.value c]) (env ++ [.value ty v])
  | premise {Γ : List BinderSort} {env : List Binding} (p : Proposition) :
      CtxHolds Γ env → PremiseHolds p env → CtxHolds (Γ ++ [.premise p]) (env ++ [.premise])

theorem CtxHolds.length {Γ : List BinderSort} {env : List Binding} (h : CtxHolds Γ env) :
    env.length = Γ.length := by
  induction h with
  | nil => rfl
  | codes ty _ ih => simp [ih]
  | value c ty v _ _ _ ih => simp [ih]
  | premise p _ _ ih => simp [ih]

/-- What a level of an interpreted context names, and the sort it has. -/
inductive LevelSort (Γ : List BinderSort) (env : List Binding) : Nat → BinderSort → Prop
  | codes (i : Nat) (ty : PureTyCode) :
      env[i]? = some (.code ty) → LevelSort Γ env i .codes
  | value (i : Nat) (c : Code) (ty : PureTyCode) (v : El ty) :
      CodeFormed Γ c → env[i]? = some (.value ty v) → codeDenote env c = some ty →
        LevelSort Γ env i (.value c)
  | premise (i : Nat) (p : Proposition) :
      env[i]? = some .premise → PremiseHolds p env → LevelSort Γ env i (.premise p)

/-- A level keeps its sort when the context and the interpretation grow
together. -/
theorem LevelSort.mono {Γ : List BinderSort} {env : List Binding} {i : Nat} {s : BinderSort}
    (h : LevelSort Γ env i s) {xs : List BinderSort} {xs' : List Binding}
    (hlt : i < env.length) (hlen : Γ.length ≤ env.length) (_hlen' : xs.length ≤ xs'.length) :
    LevelSort (Γ ++ xs) (env ++ xs') i s := by
  cases h with
  | codes ty hlook =>
      exact LevelSort.codes _ ty (by rw [List.getElem?_append_left hlt]; exact hlook)
  | value c ty v hform hlook hden =>
      refine LevelSort.value _ c ty v (hform.mono xs) ?_ ?_
      · rw [List.getElem?_append_left hlt]; exact hlook
      · have hagree : ∀ ℓ, ℓ < Γ.length → env[ℓ]? = (env ++ xs')[ℓ]? := by
          intro ℓ hℓ
          rw [List.getElem?_append_left (by omega)]
        rw [← codeDenote_agree hform (env := env) (env' := env ++ xs') hagree]
        exact hden
  | premise p hlook hp =>
      exact LevelSort.premise _ p (by rw [List.getElem?_append_left hlt]; exact hlook)
        (hp.append xs')

theorem CtxHolds.at {Γ : List BinderSort} {env : List Binding} (h : CtxHolds Γ env) :
    ∀ {i : Nat} {s : BinderSort}, Γ[i]? = some s → LevelSort Γ env i s := by
  induction h with
  | nil => rintro i s hi; exact absurd hi (by simp)
  | @codes Γ₀ env₀ ty h ih =>
      rintro i s hi
      have hlen : env₀.length = Γ₀.length := CtxHolds.length h
      rw [List.getElem?_append] at hi
      split_ifs at hi with hlt
      · exact LevelSort.mono (ih hi) (by omega) (by omega) (by simp)
      · cases hd : i - Γ₀.length with
        | zero =>
            have hieq : i = Γ₀.length := by omega
            have hs : BinderSort.codes = s := by
              rw [hd] at hi
              simpa using hi
            subst hs
            subst hieq
            exact LevelSort.codes Γ₀.length ty (by
              rw [List.getElem?_append_right (by omega), hlen]
              simp)
        | succ k =>
            rw [hd] at hi
            simp at hi
  | @value Γ₀ env₀ c ty v h hform hden ih =>
      rintro i s hi
      have hlen : env₀.length = Γ₀.length := CtxHolds.length h
      rw [List.getElem?_append] at hi
      split_ifs at hi with hlt
      · refine LevelSort.mono (ih hi) (by omega) (by omega) (by simp)
      · cases hd : i - Γ₀.length with
        | zero =>
            have hieq : i = Γ₀.length := by omega
            have hs : BinderSort.value c = s := by
              rw [hd] at hi
              simpa using hi
            subst hs
            subst hieq
            refine LevelSort.value Γ₀.length c ty v (hform.mono _) ?_ ?_
            · rw [List.getElem?_append_right (by omega), hlen]
              simp
            · rw [← hden]
              exact codeDenote_agree hform (env' := env₀) (fun ℓ hℓ => by
                rw [List.getElem?_append_left (by omega)])
        | succ k =>
            rw [hd] at hi
            simp at hi
  | @premise Γ₀ env₀ p h hp ih =>
      rintro i s hi
      have hlen : env₀.length = Γ₀.length := CtxHolds.length h
      rw [List.getElem?_append] at hi
      split_ifs at hi with hlt
      · exact LevelSort.mono (ih hi) (by omega) (by omega) (by simp)
      · cases hd : i - Γ₀.length with
        | zero =>
            have hieq : i = Γ₀.length := by omega
            have hs : BinderSort.premise p = s := by
              rw [hd] at hi
              simpa using hi
            subst hs
            subst hieq
            refine LevelSort.premise Γ₀.length p ?_ (hp.append [Binding.premise])
            rw [List.getElem?_append_right (by omega), hlen]
            simp
        | succ k =>
            rw [hd] at hi
            simp at hi

/-- A formed code denotes under every interpretation of its context. -/
theorem CodeFormed.denote {Γ : List BinderSort} {c : Code} (h : CodeFormed Γ c) :
    ∀ {env : List Binding}, CtxHolds Γ env → ∃ ty, codeDenote env c = some ty := by
  induction h with
  | unit => rintro env _; exact ⟨.unit, rfl⟩
  | bool => rintro env _; exact ⟨.bool, rfl⟩
  | i64 => rintro env _; exact ⟨.i64, rfl⟩
  | text => rintro env _; exact ⟨.text, rfl⟩
  | var hlook =>
      rintro env hc
      cases CtxHolds.at hc hlook with
      | codes ty hlook' => exact ⟨ty, by simp [codeDenote, hlook']⟩
  | pair _ _ iha ihb =>
      rintro env hc
      obtain ⟨x, hx⟩ := iha hc
      obtain ⟨y, hy⟩ := ihb hc
      exact ⟨.pair x y, by simp [codeDenote, hx, hy]⟩
  | sum _ _ iha ihb =>
      rintro env hc
      obtain ⟨x, hx⟩ := iha hc
      obtain ⟨y, hy⟩ := ihb hc
      exact ⟨.sum x y, by simp [codeDenote, hx, hy]⟩
  | list _ iha =>
      rintro env hc
      obtain ⟨x, hx⟩ := iha hc
      exact ⟨.list x, by simp [codeDenote, hx]⟩

/-! ## Eliminating a binder: the checker's `replace_prop` -/

/-- The level map of an eliminated binder that writes no new level: the binder's
position disappears and every later level moves down one. -/
def upMap (n : Nat) (ℓ : Nat) : Nat := if n < ℓ then ℓ - 1 else ℓ

/-- The level map a replacement applies. A value argument also names, at the
eliminated level, the level it writes there. -/
def Argument.mapLevel (binder : Nat) : Argument → Nat → Nat
  | .value level, ℓ => if ℓ = binder then level else upMap binder ℓ
  | _, ℓ => upMap binder ℓ

theorem upMap_getElem? {n : Nat} {b : Binding} {env : List Binding} (hn : n ≤ env.length)
    {lo : Nat} (hne : lo ≠ n) : env[upMap n lo]? = (insertAt n b env)[lo]? := by
  by_cases hlt : lo < n
  · have hu : upMap n lo = lo := by simp [upMap, show ¬ n < lo by omega]
    rw [hu, insertAt_getElem?_lt hn hlt]
  · have hgt : n < lo := by omega
    have hu : upMap n lo = lo - 1 := by simp [upMap, show n < lo by omega]
    rw [hu, ← insertAt_getElem?_gt hn hgt]

theorem valueMap_getElem? {n : Nat} {b : Binding} {env : List Binding} {level : Nat}
    (hb : env[level]? = some b) (hn : n ≤ env.length) (lo : Nat) :
    env[Argument.mapLevel n (.value level) lo]? = (insertAt n b env)[lo]? := by
  by_cases h : lo = n
  · rw [h]
    have hu : Argument.mapLevel n (.value level) n = level := by simp [Argument.mapLevel]
    rw [hu, insertAt_getElem?_self hn, hb]
  · have hu : Argument.mapLevel n (.value level) lo = upMap n lo := by
      simp [Argument.mapLevel, h]
    rw [hu]
    exact upMap_getElem? hn h

/-- Replace the code variable at level `n` by `ty`, removing level `n`. -/
def substCode (n : Nat) (ty : Code) : Code → Code
  | .unit => .unit
  | .bool => .bool
  | .i64 => .i64
  | .text => .text
  | .var ℓ => if ℓ = n then ty else .var (upMap n ℓ)
  | .pair a b => .pair (substCode n ty a) (substCode n ty b)
  | .sum a b => .sum (substCode n ty a) (substCode n ty b)
  | .list a => .list (substCode n ty a)
  | .mvar k => .mvar k

/-- Remove level `n` from a code, leaving standing any variable the source
checker does not substitute. -/
def removeLevel (n : Nat) : Code → Code
  | .unit => .unit
  | .bool => .bool
  | .i64 => .i64
  | .text => .text
  | .var ℓ => .var (upMap n ℓ)
  | .pair a b => .pair (removeLevel n a) (removeLevel n b)
  | .sum a b => .sum (removeLevel n a) (removeLevel n b)
  | .list a => .list (removeLevel n a)
  | .mvar k => .mvar k

/-- The checker's `replace_prop` on a code: a code argument is written in place
of the binder, and any other argument only removes the binder's position. -/
def codeSubst (n : Nat) : Argument → Code → Code
  | .code ty => substCode n ty
  | .value _ => removeLevel n
  | .premise => removeLevel n

/-- The checker's `replace_prop` on a proposition. -/
def Proposition.subst (n : Nat) (arg : Argument) : Proposition → Proposition
  | .piCodes ℓ body => .piCodes (arg.mapLevel n ℓ) (body.subst n arg)
  | .piValue ℓ c body => .piValue (arg.mapLevel n ℓ) (codeSubst n arg c) (body.subst n arg)
  | .piPremise ℓ p body => .piPremise (arg.mapLevel n ℓ) (p.subst n arg) (body.subst n arg)
  | .eq c lo hi => .eq (codeSubst n arg c) (arg.mapLevel n lo) (arg.mapLevel n hi)

/-- The levels a substitution may not read: the binder's own position in a
position the argument does not write. Formation implies every clause; the
source checker tests none of them. -/
def SubstOK (p : Proposition) (n : Nat) (arg : Argument) : Prop :=
  match arg with
  | .code _ => n ∉ p.eqIndices
  | .value _ => n ∉ p.codeLevels
  | .premise => n ∉ p.eqIndices ∧ n ∉ p.codeLevels

/-- A proposition formed at a context of length `m`: every Pi binds above level
`n`, the outermost Pi binds at most that context's length, and the body is
formed one level deeper. Formation implies it, and a Pi elimination extends an
interpretation of length `m` by exactly the level its first Pi binds. -/
def Proposition.Fits : Proposition → Nat → Nat → Prop
  | .piCodes ℓ body, n, m => n < ℓ ∧ ℓ ≤ m ∧ body.Fits n (m + 1)
  | .piValue ℓ _ body, n, m => n < ℓ ∧ ℓ ≤ m ∧ body.Fits n (m + 1)
  | .piPremise ℓ p body, n, m => n < ℓ ∧ ℓ ≤ m ∧ p.Fits n m ∧ body.Fits n (m + 1)
  | .eq _ _ _, _, _ => True

/-- The bound only weakens. -/
theorem Proposition.Fits.mono : ∀ (p : Proposition) {n m m' : Nat}, p.Fits n m → m ≤ m' →
    p.Fits n m'
  | .piCodes ℓ body, _, _, _, ⟨h1, h2, h3⟩, hmm =>
      ⟨h1, by omega, Proposition.Fits.mono body h3 (by omega)⟩
  | .piValue ℓ c body, _, _, _, ⟨h1, h2, h3⟩, hmm =>
      ⟨h1, by omega, Proposition.Fits.mono body h3 (by omega)⟩
  | .piPremise ℓ p₀ body, _, _, _, ⟨h1, h2, h3, h4⟩, hmm =>
      ⟨h1, by omega, Proposition.Fits.mono p₀ h3 hmm,
        Proposition.Fits.mono body h4 (by omega)⟩
  | .eq c lo hi, _, _, _, _, _ => trivial

/-- A formed proposition fits an interpretation of its own context. -/
theorem Proposition.fits_of_formed : ∀ (p : Proposition) (Γ : List BinderSort),
    PropositionFormed Γ p → ∀ (n : Nat), n < Γ.length → p.Fits n Γ.length
  | .piCodes ℓ body , Γ, hf, n, hn => by
      cases hf with
      | piCodes hℓ hb =>
          refine ⟨by omega, by omega, ?_⟩
          have hlen : (Γ ++ [BinderSort.codes]).length = Γ.length + 1 := by simp
          rw [← hlen]
          exact Proposition.fits_of_formed body _ hb _ (by rw [hlen]; omega)
  | .piValue ℓ c body , Γ, hf, n, hn => by
      cases hf with
      | piValue hℓ _hc hb =>
          refine ⟨by omega, by omega, ?_⟩
          have hlen : (Γ ++ [BinderSort.value c]).length = Γ.length + 1 := by simp
          rw [← hlen]
          exact Proposition.fits_of_formed body _ hb _ (by rw [hlen]; omega)
  | .piPremise ℓ p₀ body , Γ, hf, n, hn => by
      cases hf with
      | piPremise hℓ hp hb =>
          refine ⟨by omega, by omega, ?_, ?_⟩
          · exact Proposition.fits_of_formed p₀ _ hp _ (by
              have hlen : (Γ ++ [BinderSort.premise p₀]).length = Γ.length + 1 := by simp
              omega)
          · have hlen : (Γ ++ [BinderSort.premise p₀]).length = Γ.length + 1 := by simp
            rw [← hlen]
            exact Proposition.fits_of_formed body _ hb _ (by rw [hlen]; omega)
  | .eq c lo hi, Γ, _hf, _n, _hn => trivial

/-- The binding an argument writes at the eliminated level, with the depth its
own context has: an argument is parsed in the context the binder is eliminated
from, so its code is formed there and its value level is below that depth. -/
inductive Argument.Binds (env : List Binding) (n : Nat) : Argument → Binding → Prop
  | code {Γ : List BinderSort} {ty : Code} {ty' : PureTyCode} :
      CodeFormed Γ ty → Γ.length = n → codeDenote env ty = some ty' →
        Argument.Binds env n (.code ty) (.code ty')
  | value {level : Nat} {ty : PureTyCode} {v : El ty} :
      level < n → env[level]? = some (.value ty v) →
        Argument.Binds env n (.value level) (.value ty v)
  | premise : Argument.Binds env n .premise .premise

/-- A binding survives an extension of the interpretation: a formed argument
code reads only levels below its own context. -/
theorem Argument.Binds.append {env : List Binding} {n : Nat} {arg : Argument} {b : Binding}
    (h : Argument.Binds env n arg b) (hn : n ≤ env.length) (xs : List Binding) :
    Argument.Binds (env ++ xs) n arg b := by
  cases h with
  | @code Γ ty ty' hform hlen hden =>
      refine Argument.Binds.code hform hlen ?_
      rw [← codeDenote_agree hform (env := env) (env' := env ++ xs) (fun ℓ hℓ => by
        rw [List.getElem?_append_left (by omega)])]
      exact hden
  | value hlt hlook =>
      refine Argument.Binds.value hlt ?_
      rw [List.getElem?_append_left (by
        rw [List.getElem?_eq_some_iff] at hlook
        omega)]
      exact hlook
  | premise => exact Argument.Binds.premise

/-- A binding survives inserting a new level above the depth its argument was
parsed at. -/
theorem Argument.Binds.insert {env : List Binding} {n : Nat} {arg : Argument} {b : Binding}
    (h : Argument.Binds env n arg b) (_hn : n ≤ env.length) {m : Nat} {c : Binding}
    (hm : m ≤ env.length) (hnm : n ≤ m) : Argument.Binds (insertAt m c env) n arg b := by
  cases h with
  | @code Γ ty ty' hform hlen hden =>
      refine Argument.Binds.code hform hlen ?_
      rw [← codeDenote_agree hform (env := env) (env' := insertAt m c env) (fun ℓ hℓ => by
        exact (insertAt_getElem?_lt hm (by omega)).symm)]
      exact hden
  | value hlt hlook =>
      exact Argument.Binds.value (by omega) (by rw [insertAt_getElem?_lt hm (by omega)]; exact hlook)
  | premise => exact Argument.Binds.premise

/-- A code argument's substitution denotes what the argument denotes. -/
theorem codeDenote_substCode (c : Code) {n : Nat} {ty : Code} {ty' : PureTyCode}
    {env : List Binding} (hb : codeDenote env ty = some ty') (hn : n ≤ env.length) :
    codeDenote env (substCode n ty c) = codeDenote (insertAt n (.code ty') env) c := by
  induction c with
  | unit => rfl
  | bool => rfl
  | i64 => rfl
  | text => rfl
  | var ℓ =>
      by_cases h : ℓ = n
      · rw [h]
        have hs : substCode n ty (.var n) = ty := by simp [substCode]
        rw [hs, hb]
        simp only [codeDenote, insertAt_getElem?_self hn]
      · have hs : substCode n ty (.var ℓ) = .var (upMap n ℓ) := by simp [substCode, h]
        rw [hs]
        simp only [codeDenote]
        rw [upMap_getElem? hn h]
  | pair a b iha ihb => simp only [substCode, codeDenote, iha, ihb]
  | sum a b iha ihb => simp only [substCode, codeDenote, iha, ihb]
  | list a iha => simp only [substCode, codeDenote, iha]
  | mvar k => rfl

/-- Removing a level no code variable names denotes the same code in the
extended interpretation. -/
theorem codeDenote_removeLevel (c : Code) {n : Nat} {env : List Binding} {b : Binding}
    (hn : n ≤ env.length) (hne : n ∉ codeVars c) :
    codeDenote env (removeLevel n c) = codeDenote (insertAt n b env) c := by
  induction c with
  | unit => rfl
  | bool => rfl
  | i64 => rfl
  | text => rfl
  | var ℓ =>
      have hℓ : ℓ ≠ n := by
        intro h
        exact hne (by simp [codeVars, codeLevels, h])
      have hs : removeLevel n (.var ℓ) = .var (upMap n ℓ) := rfl
      rw [hs]
      simp only [codeDenote]
      rw [upMap_getElem? hn hℓ]
  | pair a b iha ihb =>
      have ha : n ∉ codeVars a := by
        intro h
        simp only [codeVars] at h
        exact hne (by simp only [codeVars, codeLevels, Finset.mem_union]; exact Or.inl h)
      have hb : n ∉ codeVars b := by
        intro h
        simp only [codeVars] at h
        exact hne (by simp only [codeVars, codeLevels, Finset.mem_union]; exact Or.inr h)
      simp only [removeLevel, codeDenote, iha ha, ihb hb]
  | sum a b iha ihb =>
      have ha : n ∉ codeVars a := by
        intro h
        simp only [codeVars] at h
        exact hne (by simp only [codeVars, codeLevels, Finset.mem_union]; exact Or.inl h)
      have hb : n ∉ codeVars b := by
        intro h
        simp only [codeVars] at h
        exact hne (by simp only [codeVars, codeLevels, Finset.mem_union]; exact Or.inr h)
      simp only [removeLevel, codeDenote, iha ha, ihb hb]
  | list a iha =>
      have ha : n ∉ codeVars a := by
        intro h
        simp only [codeVars] at h
        exact hne (by simp only [codeVars, codeLevels]; exact h)
      simp only [removeLevel, codeDenote, iha ha]
  | mvar k => rfl

/-- The code a substitution writes denotes the code it replaced. -/
theorem codeDenote_codeSubst {n : Nat} {arg : Argument} {b : Binding} {env : List Binding}
    (hb : Argument.Binds env n arg b) (hn : n ≤ env.length) {c : Code}
    (hne : n ∉ codeVars c) :
    codeDenote env (codeSubst n arg c) = codeDenote (insertAt n b env) c := by
  cases arg with
  | code ty =>
      cases hb with
      | code _ _ hden => exact codeDenote_substCode c hden hn
  | value level =>
      cases hb with
      | value _ => exact codeDenote_removeLevel c hn hne
  | premise =>
      cases hb with
      | premise => exact codeDenote_removeLevel c hn hne

/-- The substitution law: the checker's `replace_prop` at level `n` denotes
exactly the proposition in the interpretation a Pi elimination extends. -/
theorem Proposition.denote_subst (p : Proposition) :
    ∀ {n : Nat} {arg : Argument} {b : Binding} {env : List Binding}, n ≤ env.length →
      p.Fits n (env.length + 1) → SubstOK p n arg → Argument.Binds env n arg b →
      ((p.subst n arg).denote env ↔ p.denote (insertAt n b env)) := by
  induction p with
  | piCodes ℓ body ih =>
      rintro n arg b env hn hbt hok hb
      obtain ⟨hnℓ, hℓm, hbt'⟩ := hbt
      have hℓne : ℓ ≠ n := by omega
      have hup : upMap n ℓ = ℓ - 1 := by simp [upMap, show n < ℓ by omega]
      have hun : upMap n ℓ + 1 = ℓ := by omega
      have hmapl : arg.mapLevel n ℓ = upMap n ℓ := by
        cases arg with
        | code ty => rfl
        | value level => simp [Argument.mapLevel, hℓne]
        | premise => rfl
      have hok' : SubstOK body n arg := by
        cases arg with
        | code ty => simpa [SubstOK, Proposition.eqIndices] using hok
        | value level => simpa [SubstOK, Proposition.codeLevels] using hok
        | premise => simpa [SubstOK, Proposition.codeLevels, Proposition.eqIndices] using hok
      have hlen' : ∀ (ty : PureTyCode),
          n ≤ (insertAt (arg.mapLevel n ℓ) (.code ty) env).length := by
        intro ty
        have hlenins : (insertAt (arg.mapLevel n ℓ) (.code ty) env).length = env.length + 1 :=
          insertAt_length (n := arg.mapLevel n ℓ) (b := .code ty) (env := env)
            (by rw [hmapl, hup]; omega)
        rw [hlenins]
        omega
      have hbt'' : ∀ (ty : PureTyCode),
          body.Fits n ((insertAt (arg.mapLevel n ℓ) (.code ty) env).length + 1) := by
        intro ty
        have hlenins : (insertAt (arg.mapLevel n ℓ) (.code ty) env).length = env.length + 1 :=
          insertAt_length (n := arg.mapLevel n ℓ) (b := .code ty) (env := env)
            (by rw [hmapl, hup]; omega)
        rw [hlenins]
        exact hbt' 
      have hcomm : ∀ (ty : PureTyCode),
          insertAt n b (insertAt (arg.mapLevel n ℓ) (.code ty) env) =
            insertAt ℓ (.code ty) (insertAt n b env) := by
        intro ty
        rw [hmapl, insertAt_comm (by omega) (by rw [hup]; omega), hun]
      constructor
      · intro hp ty
        rw [← hcomm ty]
        exact (ih (hlen' ty) (hbt'' ty) hok'
          (hb.insert hn (by rw [hmapl, hup]; omega) (by rw [hmapl, hup]; omega))).1 (hp ty)
      · intro hp ty
        have hstep : body.denote (insertAt ℓ (.code ty) (insertAt n b env)) := hp ty
        rw [← hcomm ty] at hstep
        exact (ih (hlen' ty) (hbt'' ty) hok'
          (hb.insert hn (by rw [hmapl, hup]; omega) (by rw [hmapl, hup]; omega))).2 hstep
  | piValue ℓ c body ih =>
      rintro n arg b env hn hbt hok hb
      obtain ⟨hnℓ, hℓm, hbt'⟩ := hbt
      have hℓne : ℓ ≠ n := by omega
      have hup : upMap n ℓ = ℓ - 1 := by simp [upMap, show n < ℓ by omega]
      have hun : upMap n ℓ + 1 = ℓ := by omega
      have hmapl : arg.mapLevel n ℓ = upMap n ℓ := by
        cases arg with
        | code ty => rfl
        | value level => simp [Argument.mapLevel, hℓne]
        | premise => rfl
      have hok' : SubstOK body n arg := by
        cases arg with
        | code ty => simpa [SubstOK, Proposition.eqIndices] using hok
        | value level =>
            intro hc
            exact hok (by simp only [Proposition.codeLevels, Finset.mem_union]; exact Or.inr hc)
        | premise =>
            have h := hok
            simp only [SubstOK, Proposition.codeLevels, Proposition.eqIndices,
              Finset.mem_union, not_or] at h
            exact ⟨h.1, h.2.2⟩
      have hcodeid : codeDenote env (codeSubst n arg c) = codeDenote (insertAt n b env) c := by
        cases arg with
        | code ty₀ =>
            cases hb with
            | code _ _ hden => exact codeDenote_substCode c hden hn
        | value level =>
            cases hb with
            | value _ =>
                have hne : n ∉ codeVars c := by
                  have h := hok
                  simp only [SubstOK, Proposition.codeLevels, Finset.mem_union, not_or] at h
                  exact h.1
                exact codeDenote_removeLevel c hn hne
        | premise =>
            cases hb with
            | premise =>
                have hne : n ∉ codeVars c := by
                  have h := hok
                  simp only [SubstOK, Proposition.codeLevels, Proposition.eqIndices,
                    Finset.mem_union, not_or] at h
                  exact h.2.1
                exact codeDenote_removeLevel c hn hne
      have hlen' : ∀ (ty : PureTyCode) (v : El ty),
          n ≤ (insertAt (arg.mapLevel n ℓ) (.value ty v) env).length := by
        intro ty v
        have hlenins : (insertAt (arg.mapLevel n ℓ) (.value ty v) env).length = env.length + 1 :=
          insertAt_length (n := arg.mapLevel n ℓ) (b := .value ty v) (env := env)
            (by rw [hmapl, hup]; omega)
        rw [hlenins]
        omega
      have hbt'' : ∀ (ty : PureTyCode) (v : El ty),
          body.Fits n ((insertAt (arg.mapLevel n ℓ) (.value ty v) env).length + 1) := by
        intro ty v
        have hlenins : (insertAt (arg.mapLevel n ℓ) (.value ty v) env).length = env.length + 1 :=
          insertAt_length (n := arg.mapLevel n ℓ) (b := .value ty v) (env := env)
            (by rw [hmapl, hup]; omega)
        rw [hlenins]
        exact hbt' 
      have hcomm : ∀ (ty : PureTyCode) (v : El ty),
          insertAt n b (insertAt (arg.mapLevel n ℓ) (.value ty v) env) =
            insertAt ℓ (.value ty v) (insertAt n b env) := by
        intro ty v
        rw [hmapl, insertAt_comm (by omega) (by rw [hup]; omega), hun]
      have hrew : ∀ (ty : PureTyCode) (v : El ty),
          (body.subst n arg).denote (insertAt (arg.mapLevel n ℓ) (.value ty v) env) ↔
            body.denote (insertAt ℓ (.value ty v) (insertAt n b env)) := by
        intro ty v
        rw [← hcomm ty v]
        exact ih (hlen' ty v) (hbt'' ty v) hok'
          (hb.insert hn (by rw [hmapl, hup]; omega) (by rw [hmapl, hup]; omega))
      have hmp : ∀ (ty : PureTyCode) (v : El ty),
          codeDenote (insertAt n b env) c = some ty →
            codeDenote env (codeSubst n arg c) = some ty := by
        intro ty v hden
        rw [hcodeid]
        exact hden
      constructor
      · intro hp ty hden v
        exact (hrew ty v).1 (hp ty (hmp ty v hden) v)
      · intro hp ty hden v
        exact (hrew ty v).2 (hp ty (by rw [← hcodeid]; exact hden) v)
  | piPremise ℓ p₀ body ihp ihb =>
      rintro n arg b env hn hbt hok hb
      obtain ⟨hnℓ, hℓm, hbt', hbtb0⟩ := hbt
      have hℓne : ℓ ≠ n := by omega
      have hup : upMap n ℓ = ℓ - 1 := by simp [upMap, show n < ℓ by omega]
      have hun : upMap n ℓ + 1 = ℓ := by omega
      have hmapl : arg.mapLevel n ℓ = upMap n ℓ := by
        cases arg with
        | code ty => rfl
        | value level => simp [Argument.mapLevel, hℓne]
        | premise => rfl
      have hokp : SubstOK p₀ n arg := by
        cases arg with
        | code ty =>
            have h := hok
            simp only [SubstOK, Proposition.eqIndices, Finset.mem_union, not_or] at h
            exact h.1
        | value level =>
            have h := hok
            simp only [SubstOK, Proposition.codeLevels, Finset.mem_union, not_or] at h
            exact h.1
        | premise =>
            have h := hok
            simp only [SubstOK, Proposition.codeLevels, Proposition.eqIndices,
              Finset.mem_union, not_or] at h
            exact ⟨h.1.1, h.2.1⟩
      have hokb : SubstOK body n arg := by
        cases arg with
        | code ty =>
            have h := hok
            simp only [SubstOK, Proposition.eqIndices, Finset.mem_union, not_or] at h
            exact h.2
        | value level =>
            have h := hok
            simp only [SubstOK, Proposition.codeLevels, Finset.mem_union, not_or] at h
            exact h.2
        | premise =>
            have h := hok
            simp only [SubstOK, Proposition.codeLevels, Proposition.eqIndices,
              Finset.mem_union, not_or] at h
            exact ⟨h.1.2, h.2.2⟩
      have hbtp : p₀.Fits n (env.length + 1) := hbt'
      have hbtb : body.Fits n (env.length + 2) := hbtb0
      have hlen' : n ≤ (insertAt (arg.mapLevel n ℓ) .premise env).length := by
        have hlenins : (insertAt (arg.mapLevel n ℓ) .premise env).length = env.length + 1 :=
          insertAt_length (n := arg.mapLevel n ℓ) (b := .premise) (env := env)
            (by rw [hmapl, hup]; omega)
        rw [hlenins]
        omega
      have hbtb' : body.Fits n ((insertAt (arg.mapLevel n ℓ) .premise env).length + 1) := by
        have hlenins : (insertAt (arg.mapLevel n ℓ) .premise env).length = env.length + 1 :=
          insertAt_length (n := arg.mapLevel n ℓ) (b := .premise) (env := env)
            (by rw [hmapl, hup]; omega)
        rw [hlenins]
        exact hbtb0
      have hcomm : insertAt n b (insertAt (arg.mapLevel n ℓ) .premise env) =
          insertAt ℓ .premise (insertAt n b env) := by
        rw [hmapl, insertAt_comm (by omega) (by rw [hup]; omega), hun]
      have hprem : (p₀.denote (insertAt n b env)) → (p₀.subst n arg).denote env :=
        (ihp hn hbtp hokp hb).2
      have hprem' : (p₀.subst n arg).denote env → p₀.denote (insertAt n b env) :=
        (ihp hn hbtp hokp hb).1
      have hrew : (body.subst n arg).denote (insertAt (arg.mapLevel n ℓ) .premise env) ↔
          body.denote (insertAt ℓ .premise (insertAt n b env)) := by
        rw [← hcomm]
        exact ihb hlen' hbtb' hokb
          (hb.insert hn (by rw [hmapl, hup]; omega) (by rw [hmapl, hup]; omega))
      constructor
      · intro hp hpre
        exact hrew.1 (hp (hprem hpre))
      · intro hp hpre
        have hstep : body.denote (insertAt ℓ .premise (insertAt n b env)) := hp (hprem' hpre)
        exact hrew.2 hstep
  | eq c lo hi =>
      rintro n arg b env hn _hbt hok hb
      have hcode : codeDenote env (codeSubst n arg c) = codeDenote (insertAt n b env) c := by
        cases arg with
        | code ty₀ =>
            cases hb with
            | code _ _ hden => exact codeDenote_substCode c hden hn
        | value level =>
            cases hb with
            | value _ =>
                have hne : n ∉ codeVars c := by
                  have h := hok
                  simp only [SubstOK, Proposition.codeLevels] at h
                  exact h
                exact codeDenote_removeLevel c hn hne
        | premise =>
            cases hb with
            | premise =>
                have hne : n ∉ codeVars c := by
                  have h := hok
                  simp only [SubstOK, Proposition.codeLevels] at h
                  exact h.2
                exact codeDenote_removeLevel c hn hne
      have hlev : ∀ l, l = lo ∨ l = hi →
          env[arg.mapLevel n l]? = (insertAt n b env)[l]? := by
        rintro l (rfl | rfl)
        · cases arg with
          | code ty₀ =>
              cases hb with
              | code _ _ _ =>
                  exact upMap_getElem? hn (by
                    intro h
                    subst h
                    have hh := hok
                    simp only [SubstOK, Proposition.eqIndices] at hh
                    exact hh (by simp))
          | value level =>
              cases hb with
              | value hlt hlook => exact valueMap_getElem? hlook hn _
          | premise =>
              cases hb with
              | premise =>
                  exact upMap_getElem? hn (by
                    intro h
                    subst h
                    have hh := hok
                    simp only [SubstOK, Proposition.eqIndices] at hh
                    exact hh.1 (by simp))
        · cases arg with
          | code ty₀ =>
              cases hb with
              | code _ _ _ =>
                  exact upMap_getElem? hn (by
                    intro h
                    subst h
                    have hh := hok
                    simp only [SubstOK, Proposition.eqIndices] at hh
                    exact hh (by simp))
          | value level =>
              cases hb with
              | value hlt hlook => exact valueMap_getElem? hlook hn _
          | premise =>
              cases hb with
              | premise =>
                  exact upMap_getElem? hn (by
                    intro h
                    subst h
                    have hh := hok
                    simp only [SubstOK, Proposition.eqIndices] at hh
                    exact hh.1 (by simp))
      constructor
      · rintro ⟨ty, v, w, hden, hl, hh, hvw⟩
        refine ⟨ty, v, w, ?_, ?_, ?_, hvw⟩
        · rw [← hcode]; exact hden
        · rw [← hlev lo (Or.inl rfl)]; exact hl
        · rw [← hlev hi (Or.inr rfl)]; exact hh
      · rintro ⟨ty, v, w, hden, hl, hh, hvw⟩
        refine ⟨ty, v, w, ?_, ?_, ?_, hvw⟩
        · rw [hcode]; exact hden
        · rw [hlev lo (Or.inl rfl)]; exact hl
        · rw [hlev hi (Or.inr rfl)]; exact hh

/-! ## The checker's judgements -/

/-- One checker judgement: `Judges false Γ t p` is `proof_walk`'s inference of
`p`, and `Judges true Γ t p` is its check of `t` against `p`. The two modes are
one relation because `apply` on a premise Pi checks its argument and the check
mode falls back to inference, so the checker's walk is mutually recursive. -/
inductive Judges : Bool → List BinderSort → Proof → Proposition → Prop
  | localUse {Γ : List BinderSort} {ℓ : Nat} {p : Proposition} :
      Γ[ℓ]? = some (.premise p) → Judges false Γ (.useLocal ℓ) p
  | declaredUse {Γ : List BinderSort} {g : Proposition} :
      (∀ (env : List Binding), env.length = Γ.length →
        (g.rebase (fun ℓ => ℓ + Γ.length)).denote env) →
        Judges false Γ (.useDeclared g) (g.rebase (fun ℓ => ℓ + Γ.length))
  | refl {Γ : List BinderSort} {c : Code} {ℓ : Nat} :
      Γ[ℓ]? = some (.value c) → Judges false Γ (.refl ℓ) (.eq c ℓ ℓ)
  | applyCode {Γ : List BinderSort} {f : Proof} {ty : Code} {ℓ : Nat} {body : Proposition} :
      Judges false Γ f (.piCodes ℓ body) → CodeFormed Γ ty →
        PropositionFormed (Γ ++ [.codes]) body → ℓ = Γ.length →
          Judges false Γ (.apply f (.code ty)) (body.subst ℓ (.code ty))
  | applyValue {Γ : List BinderSort} {f : Proof} {c : Code} {ℓ : Nat} {body : Proposition}
      {i : Nat} :
      Judges false Γ f (.piValue ℓ c body) → Γ[i]? = some (.value c) →
        PropositionFormed (Γ ++ [.value c]) body → ℓ = Γ.length →
          Judges false Γ (.apply f (.value i)) (body.subst ℓ (.value i))
  | applyPremise {Γ : List BinderSort} {f : Proof} {ℓ : Nat} {p body : Proposition}
      {premise : Proof} :
      Judges false Γ f (.piPremise ℓ p body) → Judges true Γ premise p →
        PropositionFormed (Γ ++ [.premise p]) body → ℓ = Γ.length →
          Judges false Γ (.applyPremise f premise) (body.subst ℓ .premise)
  | infer {Γ : List BinderSort} {t : Proof} {p : Proposition} :
      Judges false Γ t p → Judges true Γ t p
  | introCodes {Γ : List BinderSort} {ℓ : Nat} {body : Proof} {rest : Proposition} :
      ℓ = Γ.length → Judges true (Γ ++ [.codes]) body rest →
        Judges true Γ (.intro .codes body) (.piCodes ℓ rest)
  | introValue {Γ : List BinderSort} {c : Code} {ℓ : Nat} {body : Proof} {rest : Proposition} :
      ℓ = Γ.length → CodeFormed Γ c → Judges true (Γ ++ [.value c]) body rest →
        Judges true Γ (.intro (.value c) body) (.piValue ℓ c rest)
  | introPremise {Γ : List BinderSort} {p : Proposition} {ℓ : Nat} {body : Proof}
      {rest : Proposition} :
      ℓ = Γ.length → PropositionFormed Γ p → Judges true (Γ ++ [.premise p]) body rest →
        Judges true Γ (.intro (.premise p) body) (.piPremise ℓ p rest)
  | substEq {Γ : List BinderSort} {c : Code} {motive : Proposition} {evidence premise : Proof}
      {left right : Nat} :
      CodeFormed Γ c → PropositionFormed (Γ ++ [.value c]) motive → motive.IsEq →
        Judges false Γ evidence (.eq c left right) →
          Judges true Γ premise (motive.subst Γ.length (.value left)) →
            Judges true Γ (.subst c motive evidence premise)
              (motive.subst Γ.length (.value right))

/-- The propositions the checker infers. -/
def Infers (Γ : List BinderSort) (t : Proof) (p : Proposition) : Prop :=
  Judges false Γ t p

/-- The propositions the checker checks a term against. -/
def Checks (Γ : List BinderSort) (t : Proof) (p : Proposition) : Prop :=
  Judges true Γ t p

/-- Soundness: every proposition the checker establishes is true at every
interpretation of the context it was established in. -/
theorem Judges.sound {b : Bool} {Γ : List BinderSort} {t : Proof} {p : Proposition}
    (h : Judges b Γ t p) : ∀ {env : List Binding}, CtxHolds Γ env → p.denote env := by
  induction h with
  | localUse hlook =>
      rintro env hc
      cases CtxHolds.at hc hlook with
      | premise p' hlook' hp => exact hp.denote
  | declaredUse hprem =>
      rintro env hc
      exact hprem env (CtxHolds.length hc)
  | refl hlook =>
      rintro env hc
      cases CtxHolds.at hc hlook with
      | value c' ty v hform hlook' hden => exact ⟨ty, v, v, hden, hlook', hlook', rfl⟩
  | @applyCode Γ f ty ℓ body hf hform hbody hℓ ih =>
      rintro env hc
      have hlen : env.length = Γ.length := CtxHolds.length hc
      have hℓe : ℓ = env.length := by rw [hℓ, hlen]
      obtain ⟨ty', hden⟩ := hform.denote hc
      have hpi := ih hc
      have hfits : body.Fits ℓ (env.length + 1) := by
        rw [hℓe]
        have hlen2 : (Γ ++ [BinderSort.codes]).length = Γ.length + 1 := by simp
        rw [hlen, ← hlen2]
        exact Proposition.fits_of_formed body _ hbody (n := Γ.length) (by rw [hlen2]; omega)
      have hbinds : Argument.Binds env ℓ (.code ty) (.code ty') :=
        Argument.Binds.code hform (by rw [hℓ]) hden
      have hok : SubstOK body ℓ (.code ty) := by
        show ℓ ∉ body.eqIndices
        intro hmem
        obtain ⟨c, hc'⟩ := hbody.value_of_mem hmem (by rw [hℓ]; simp)
        have hlook : (Γ ++ [BinderSort.codes])[ℓ]? = some BinderSort.codes := by
          rw [hℓ, List.getElem?_append_right (by omega)]
          simp
        rw [hlook] at hc'
        exact absurd hc' (by simp)
      exact (Proposition.denote_subst body (n := ℓ) (arg := .code ty) (b := .code ty')
        (env := env) (by omega) hfits hok hbinds).2 (hpi ty')
  | @applyValue Γ f c ℓ body i hf hlook hbody hℓ ih =>
      rintro env hc
      have hlen : env.length = Γ.length := CtxHolds.length hc
      have hℓe : ℓ = env.length := by rw [hℓ, hlen]
      have hpi := ih hc
      have hi : i < Γ.length := (List.getElem?_eq_some_iff.1 hlook).1
      cases CtxHolds.at hc hlook with
      | value c' ty v hform hlook' hden =>
          have hbinds : Argument.Binds env ℓ (.value i) (.value ty v) :=
            Argument.Binds.value (by rw [hℓe]; omega) hlook'
          have hfits : body.Fits ℓ (env.length + 1) := by
            rw [hℓe]
            have hlen2 : (Γ ++ [BinderSort.value c]).length = Γ.length + 1 := by simp
            rw [hlen, ← hlen2]
            exact Proposition.fits_of_formed body _ hbody (n := Γ.length) (by rw [hlen2]; omega)
          have hok : SubstOK body ℓ (.value i) := by
            show ℓ ∉ body.codeLevels
            intro hmem
            have hc' := hbody.codes_of_mem hmem (by rw [hℓ]; simp)
            rw [hℓ, List.getElem?_append_right (by omega)] at hc'
            simp at hc' 
          exact (Proposition.denote_subst body (n := ℓ) (arg := .value i) (b := .value ty v)
            (env := env) (by omega) hfits hok hbinds).2 (hpi ty hden v)
  | @applyPremise Γ f ℓ p body premise hf hprem hbody hℓ ih ihp =>
      rintro env hc
      have hlen : env.length = Γ.length := CtxHolds.length hc
      have hℓe : ℓ = env.length := by rw [hℓ, hlen]
      have hpi := ih hc
      have hpf : p.denote env := ihp hc
      have hfits : body.Fits ℓ (env.length + 1) := by
        rw [hℓe]
        have hlen2 : (Γ ++ [BinderSort.premise p]).length = Γ.length + 1 := by simp
        rw [hlen, ← hlen2]
        exact Proposition.fits_of_formed body _ hbody (n := Γ.length) (by rw [hlen2]; omega)
      have hbinds : Argument.Binds env ℓ .premise .premise := Argument.Binds.premise
      have hok : SubstOK body ℓ .premise := by
        constructor
        · intro hmem
          obtain ⟨c', hc'⟩ := hbody.value_of_mem hmem (by rw [hℓ]; simp)
          rw [hℓ, List.getElem?_append_right (by omega)] at hc'
          simp at hc' 
        · intro hmem
          have hc' := hbody.codes_of_mem hmem (by rw [hℓ]; simp)
          rw [hℓ, List.getElem?_append_right (by omega)] at hc'
          simp at hc' 
      exact (Proposition.denote_subst body (n := ℓ) (arg := .premise) (b := .premise)
        (env := env) (by omega) hfits hok hbinds).2 (hpi hpf)
  | infer hf ih => rintro env hc; exact ih hc
  | @introCodes Γ ℓ body rest hℓ hbody ih =>
      rintro env hc
      have hlen : env.length = Γ.length := CtxHolds.length hc
      intro ty
      have hidx : insertAt ℓ (Binding.code ty) env = env ++ [Binding.code ty] := by
        rw [hℓ, ← hlen]
        exact insertAt_self
      rw [hidx]
      exact ih (CtxHolds.codes ty hc)
  | @introValue Γ c ℓ body rest hℓ hform hbody ih =>
      rintro env hc
      have hlen : env.length = Γ.length := CtxHolds.length hc
      intro ty hden v
      have hidx : insertAt ℓ (Binding.value ty v) env = env ++ [Binding.value ty v] := by
        rw [hℓ, ← hlen]
        exact insertAt_self
      rw [hidx]
      exact ih (CtxHolds.value c ty v hc hform hden)
  | @introPremise Γ p ℓ body rest hℓ hform hbody ih =>
      rintro env hc
      have hlen : env.length = Γ.length := CtxHolds.length hc
      intro hpre
      have hidx : insertAt ℓ Binding.premise env = env ++ [Binding.premise] := by
        rw [hℓ, ← hlen]
        exact insertAt_self
      rw [hidx]
      refine ih (CtxHolds.premise p hc ?_)
      exact ⟨Γ, env, hform, hlen, by omega, fun _ _ => rfl, hpre⟩
  | @substEq Γ c motive evidence premise left right hc₀ hmotive hisEq hev hpre ihEv ihPre =>
      rintro env hc
      have hlen : env.length = Γ.length := CtxHolds.length hc
      obtain ⟨ty, v, w, hden, hleft, hright, hvw⟩ := ihEv hc
      have hleft' : left < Γ.length := by
        have := (List.getElem?_eq_some_iff.1 hleft).1
        omega
      have hright' : right < Γ.length := by
        have := (List.getElem?_eq_some_iff.1 hright).1
        omega
      have hbindsL : Argument.Binds env Γ.length (.value left) (.value ty v) :=
        Argument.Binds.value hleft' hleft
      have hbindsR : Argument.Binds env Γ.length (.value right) (.value ty w) :=
        Argument.Binds.value hright' hright
      have hfits : motive.Fits Γ.length (env.length + 1) := by
        have hlen2 : (Γ ++ [BinderSort.value c]).length = Γ.length + 1 := by simp
        rw [hlen, ← hlen2]
        exact Proposition.fits_of_formed motive _ hmotive (n := Γ.length) (by rw [hlen2]; omega)
      have hok : ∀ (l : Nat) (bnd : Binding), Argument.Binds env Γ.length (.value l) bnd →
          SubstOK motive Γ.length (.value l) := by
        intro l bnd _
        show Γ.length ∉ motive.codeLevels
        intro hmem
        have hc' := hmotive.codes_of_mem hmem (by simp)
        rw [List.getElem?_append_right (by omega)] at hc'
        simp at hc' 
      have hbefore : (motive.subst Γ.length (.value left)).denote env := ihPre hc
      have h1 : motive.denote (insertAt Γ.length (.value ty v) env) :=
        (Proposition.denote_subst motive (n := Γ.length) (arg := .value left)
          (b := .value ty v) (env := env) (by omega) hfits (hok left _ hbindsL) hbindsL).1 hbefore
      rw [hvw] at h1
      exact (Proposition.denote_subst motive (n := Γ.length) (arg := .value right)
        (b := .value ty w) (env := env) (by omega) hfits (hok right _ hbindsR) hbindsR).2 h1
