/-
Reference model: soundness of the reference decision function (PO-09).

The acceptance theorem (V-CHECK-03 shape): `check` accepting a candidate
implies the checked result is well formed and the candidate's entry body has
a declarative `Derives` derivation at the expected interface.

The proof strengthens the goal to a fold-level lemma, `foldBody_derives`, by
induction on the fold's structural fuel parameter: every successful fold
yields a derivation whose effect index is the right-nested union of the
charged nodes' interfaces, while the fold itself accumulates the same bound
left-nested; `Judgment.foldl_union` (spec §8.2: union is associative as a
summary) reconciles the two at the fold's entry, where the incoming bound is
empty. The closing section keeps, as a regression witness, the input that
separated acceptance from derivability under an exact-chaining reading of
SEQUENCE: a literal consuming a strict segment of the running stack.
-/

import NobleM2.Check
import NobleM2.Judgment
import NobleM2.Soundness
import NobleM2.Termination

namespace NobleM2

/-! ## Decidable outcome equality (for evaluation-closed premises) -/


/-! ## Instantiation and join facts -/

theorem append_nil (l : TyList) : l.append TyList.nil = l := by
  have key : ∀ (n : Nat) (l : TyList), l.length ≤ n → l.append TyList.nil = l := by
    intro n
    induction n with
    | zero =>
        intro l hl
        cases l with
        | nil => rfl
        | cons t ts =>
            have hlen : (TyList.cons t ts).length = 1 + ts.length := rfl
            omega
    | succ n ih =>
        intro l hl
        cases l with
        | nil => rfl
        | cons t ts =>
            have hlen : (TyList.cons t ts).length = 1 + ts.length := rfl
            show TyList.cons t (ts.append TyList.nil) = TyList.cons t ts
            rw [ih ts (by omega)]
  exact key l.length l (Nat.le_refl _)

/-- A quotation witness without its outer stack binding cannot instantiate. -/
theorem quotationScheme_none {inst : Inst} (h0 : inst.stack 0 = none) :
    quotationScheme.instantiate inst = none := by
  simp [quotationScheme, Scheme.instantiate, Scheme.substStack, Scheme.substPattern,
    Scheme.substSignature, Scheme.substEffects, PartList.ofList, SlotList.ofList, h0]

/-- A total quotation witness instantiates to the node's own interface:
`r -- r [Program<a, c, e>]` with an empty construction bound. -/
theorem quotationScheme_some {inst : Inst} {r a c e}
    (h0 : inst.stack 0 = some r) (h1 : inst.stack 1 = some a)
    (h2 : inst.stack 2 = some c) (h3 : inst.effects 3 = some e) :
    quotationScheme.instantiate inst
      = some (r, r.append (TyList.singleton (.program a c e)), EffSet.empty) := by
  simp [quotationScheme, Scheme.instantiate, Scheme.substStack, Scheme.substPattern,
    Scheme.substSignature, Scheme.substEffects, PartList.ofList, SlotList.ofList,
    h0, h1, h2, h3, append_nil, union_empty_right, TyList.singleton]

/-- The eligibility guard decides the declarative side condition. -/
theorem dataOkAt_iff (env : Env) (index : Nat) (inst : Inst) :
    dataOkAt env index inst = true ↔ DataOk env index inst := by
  simp only [dataOkAt, DataOk]
  cases h : dataSlot ((env.kind index).getD .named) with
  | none => simp
  | some var =>
      cases hv : inst.value var with
      | none => simp [hv]
      | some ty =>
          simp only [hv]
          constructor
          · intro hty; exact ⟨ty, rfl, hty⟩
          · rintro ⟨ty', hv', hisdata⟩
            injection hv' with hty
            exact hty ▸ hisdata

/-- A decided inclusion is the declarative subset relation. -/
theorem subsetOf_subset {a b : EffSet} (h : a.subsetOf b = true) : a.Subset b := by
  rw [EffSet.subsetOf, List.all_eq_true] at h
  intro x hx
  simpa using h x hx

/-- A successful join exposes its segment premises: the consumed segment
matched the running stack's top, and the returned stack and bound are the
replacement and the union. -/
theorem joinInterface_ok {stack derived iface limits joined bound}
    (h : joinInterface stack derived iface limits = .ok (joined, bound)) :
    tailEquals stack iface.stackIn = true ∧
      replaceTail stack iface.stackIn iface.stackOut = joined ∧
      bound = EffSet.union derived iface.effects := by
  unfold joinInterface at h
  split at h
  · cases h
  · dsimp only at h
    split at h
    · cases h
    · rename_i hguard _
      injection h with hp
      injection hp with h1 h2
      refine ⟨by simpa using hguard, h1, h2.symm⟩

/-! ## The fold carries a derivation -/

/-- Every successful fold derives the same stack transformation: there is a
derivation of the body from the entry stack to the fold's final stack whose
effect index is the right-nested union of the charged nodes' interface
bounds, exactly tracking the fold's own left-nested accumulation. -/
theorem foldBody_derives {env : Env} {cand : Candidate} {limits : Limits} :
    ∀ (fuel depth : Nat) (body : List Nat) (stack : TyList) (derivedIn : EffSet)
      (fold : Fold) (final : TyList) (finalBound : EffSet) (next : Fold),
      foldBody env cand limits depth fuel body stack derivedIn fold
          = .ok final finalBound next →
      ∃ (heads : List EffSet) (d : EffSet),
        Derives env cand body stack final d ∧
          d = heads.foldr EffSet.union EffSet.empty ∧
          finalBound = heads.foldl EffSet.union derivedIn := by
  intro fuel
  induction fuel with
  | zero =>
    intro depth body stack derivedIn fold final finalBound next h
    simp only [foldBody] at h
    cases h
  | succ fuel ih =>
    intro depth body stack derivedIn fold final finalBound next h
    cases body with
    | nil =>
      simp only [foldBody] at h
      injection h with h1 h2 h3
      subst h2
      subst h1
      subst h3
      exact ⟨[], EffSet.empty, Derives.empty stack, rfl, rfl⟩
    | cons id rest =>
      simp only [foldBody] at h
      cases hnode : cand.nodes[id]? with
      | none => simp only [hnode] at h; cases h
      | some node =>
        cases node with
        | literal lit inst =>
          simp only [hnode] at h
          cases hap : applyScheme env limits (litScheme lit) inst with
          | error problem => simp only [hap] at h; cases h
          | ok pair =>
            obtain ⟨iface, _⟩ := pair
            cases hj : joinInterface stack derivedIn iface limits with
            | error problem => simp only [hap, hj] at h; cases h
            | ok pair =>
              obtain ⟨joined, bound⟩ := pair
              have hjoin := joinInterface_ok hj
              cases hst : stepFold fold (schemeCost (litScheme lit)
                  + joinCost iface) id iface with
              | error problem => simp only [hap, hj, hst] at h; cases h
              | ok stepped =>
                simp only [hap, hj, hst] at h
                obtain ⟨heads', d', hd', hdeq', hout'⟩ := ih depth rest joined bound
                  stepped final finalBound next h
                refine ⟨iface.effects :: heads', iface.effects.union d', ?_, by simp only [List.foldr_cons, hdeq'], ?_⟩
                · exact Derives.sequence hnode
                    (NodeDerives.literal (applyScheme_ok_resolves hap)
                      (applyScheme_ok hap)) hjoin.1 hjoin.2.1 hd'
                · rw [hjoin.2.2] at hout'
                  exact hout'
        | invocation index inst =>
          simp only [hnode] at h
          cases hscheme : env.scheme index with
          | none => simp only [hscheme] at h; cases h
          | some scheme =>
            simp only [hscheme] at h
            cases hap : applyScheme env limits scheme inst with
            | error problem => simp only [hap] at h; cases h
            | ok pair =>
              obtain ⟨iface, resolved⟩ := pair
              simp only [hap] at h
              split at h
              · cases h
              · rename_i hdata
                have hdataeq : dataOkAt env index resolved = true := by simpa using hdata
                have hdataok : DataOk env index resolved := (dataOkAt_iff env index resolved).1 hdataeq
                cases hj : joinInterface stack derivedIn iface limits with
                  | error problem => simp only [hap, hj] at h; cases h
                  | ok pair =>
                    obtain ⟨joined, bound⟩ := pair
                    have hjoin := joinInterface_ok hj
                    cases hst : stepFold fold (schemeCost scheme
                        + joinCost iface) id iface with
                    | error problem => simp only [hap, hj, hst] at h; cases h
                    | ok stepped =>
                      simp only [hap, hj, hst] at h
                      obtain ⟨heads', d', hd', hdeq', hout'⟩ := ih depth rest joined
                        bound stepped final finalBound next h
                      refine ⟨iface.effects :: heads', iface.effects.union d', ?_, by simp only [List.foldr_cons, hdeq'], ?_⟩
                      · exact Derives.sequence hnode
                          (NodeDerives.word hscheme (applyScheme_ok_resolves hap)
                          (applyScheme_ok hap) hdataok)
                          hjoin.1 hjoin.2.1 hd'
                      · rw [hjoin.2.2] at hout'
                        exact hout'
        | quotation qbody inst =>
          simp only [hnode] at h
          cases hap : applyScheme env limits quotationScheme inst with
          | error problem => simp only [hap] at h; cases h
          | ok pair =>
            obtain ⟨iface, resolved⟩ := pair
            simp only [hap] at h
            split at h
            · cases h
            · cases hs0 : resolved.stack 0 with
              | none =>
                  have hi := applyScheme_ok hap
                  rw [quotationScheme_none hs0] at hi
                  exact absurd hi (by simp)
              | some r =>
                cases hs1 : resolved.stack 1 with
                | none => simp only [hs1] at h; cases h
                | some start =>
                  cases hs2 : resolved.stack 2 with
                  | none => simp only [hs1, hs2] at h; cases h
                  | some claimed =>
                    cases hs3 : resolved.effects 3 with
                    | none => simp only [hs1, hs2, hs3] at h; cases h
                    | some bound =>
                      simp only [hs1, hs2, hs3] at h
                      cases hinner : foldBody env cand limits (depth + 1) fuel
                          qbody start EffSet.empty ⟨fold.work, fold.derivations⟩ with
                      | failed problem => cases problem <;> simp only [hinner] at h <;> simp at h
                      | ok innerFinal innerBound inner =>
                        simp only [hinner] at h
                        split at h
                        · cases h
                        · rename_i hfc
                          split at h
                          · cases h
                          · rename_i hsub
                            have hsome := quotationScheme_some hs0 hs1 hs2 hs3
                            have hi := applyScheme_ok hap
                            rw [hsome] at hi
                            injection hi with hp
                            injection hp with hi1 hrest
                            injection hrest with hi2 hi3
                            obtain ⟨headsI, dI, hdI, hdeqI, houtI⟩ := ih (depth + 1)
                              qbody start EffSet.empty ⟨fold.work, fold.derivations⟩
                              innerFinal innerBound inner hinner
                            have hdI2 : dI = innerBound := by
                              rw [hdeqI, houtI, foldl_union, union_empty_left]
                            have hfin : innerFinal = claimed := by simpa using hfc
                            have hsub2 : innerBound.Subset bound :=
                              subsetOf_subset (by simpa using hsub)
                            have hq : QuotationDerives env cand qbody resolved r
                                (r.append (TyList.singleton
                                  (.program start claimed bound))) EffSet.empty :=
                              QuotationDerives.mk hsome
                                (by rw [← hfin]; exact hdI)
                                (by rw [hdI2]; exact hsub2)
                            cases hj : joinInterface stack derivedIn iface limits with
                            | error problem => simp only [hj] at h; cases h
                            | ok pair =>
                              obtain ⟨joined, outerBound⟩ := pair
                              have hjoin := joinInterface_ok hj
                              cases hst : stepFold inner (joinCost iface) id iface with
                              | error problem => simp only [hj, hst] at h; cases h
                              | ok stepped =>
                                simp only [hj, hst] at h
                                obtain ⟨heads', d', hd', hdeq', hout'⟩ := ih depth rest
                                  joined outerBound stepped final finalBound next h
                                refine ⟨iface.effects :: heads',
                                  iface.effects.union d', ?_,
                                  by simp only [List.foldr_cons, hdeq'], ?_⟩
                                · refine Derives.quotationSequence hnode (applyScheme_ok_resolves hap) ?_
                                    hjoin.1 hjoin.2.1 hd'
                                  rw [← hi1, ← hi2, ← hi3]
                                  exact hq
                                · rw [hjoin.2.2] at hout'
                                  exact hout'

/-- At a fold's entry the incoming bound is empty, so the derivation's
right-nested index and the fold's left-nested accumulation coincide. -/
theorem foldBody_entry_derives {env : Env} {cand : Candidate} {limits : Limits}
    {depth fuel : Nat} {body : List Nat} {stack : TyList} {fold : Fold}
    {final : TyList} {derivedOut : EffSet} {next : Fold}
    (h : foldBody env cand limits depth fuel body stack EffSet.empty fold
        = .ok final derivedOut next) :
    Derives env cand body stack final derivedOut := by
  obtain ⟨heads, d, hd, hdeq, hout⟩ :=
    foldBody_derives fuel depth body stack EffSet.empty fold final derivedOut next h
  have hd2 : d = derivedOut := by
    rw [hdeq, hout, foldl_union, union_empty_left]
  exact hd2 ▸ hd

/-! ## Acceptance soundness (V-CHECK-03, PO-09) -/

/-- Acceptance implies well-formedness and a typing derivation: the checked
interface is the expected one under an included bound, and the entry body
derives it declaratively. -/
theorem check_soundness {env : Env} {request : Request} {cand : Candidate}
    {checked : Checked} (h : check env request cand = .accepted checked) :
    WellFormed request cand checked ∧
      TypingDerivation env request cand checked := by
  unfold check at h
  split at h
  · cases h
  · split at h
    · cases h
    · split at h
      · cases h
      · split at h
        · cases h
        · -- fragment v1: the dependency walk must validate (B-CHECK-02)
          split at h
          · cases h
          · cases h
          · cases h
          · cases h
          · -- and the schema scan must validate
            split at h
            · cases h
            · cases h
            · cases h
            · cases h
            · split at h
              · cases h
              · cases hfind : request.expected.allowedEffects.ids.find?
                    (fun id => !effectKnown env id) with
                | some id => simp only [hfind] at h; cases h
                | none =>
                  simp only [hfind] at h
                  cases hf : foldBody env cand request.limits 0 request.limits.work
                      cand.body request.expected.stackIn EffSet.empty
                      ⟨request.limits.work, []⟩ with
                  | failed problem => cases problem <;> simp only [hf] at h <;> simp at h
                  | ok final derived fold =>
                    simp only [hf] at h
                    split at h
                    · cases h
                    · rename_i hfc
                      split at h
                      · cases h
                      · rename_i hsub
                        injection h with hchecked
                        subst hchecked
                        have hfin : final = request.expected.stackOut := by
                          simpa using hfc
                        have hsub2 : derived.subsetOf request.expected.allowedEffects
                            = true := by simpa using hsub
                        refine ⟨⟨rfl, by rw [hfin], subsetOf_subset hsub2,
                          fun _ _ => Or.inr trivial⟩, ?_⟩
                        have hentry := foldBody_entry_derives hf
                        rw [hfin] at hentry
                        exact hentry

/-! ## Regression: the segment-extension gap is closed -/

namespace Regression

/-- The input that separated acceptance from derivability under an
exact-chaining SEQUENCE: the literal's instantiation consumes only the empty
segment while the running stack is `[bool]`. -/
def ceCandidate : Candidate := ⟨0, 0, [.literal (.i64 41) ⟨[.stack .nil]⟩], [0]⟩

def ceRequest : Request :=
  ⟨64, ⟨.cons .bool .nil, .cons .bool (.cons .i64 .nil), EffSet.empty⟩,
    ⟨65536, 256, 32, 64, 16, 10000, 64⟩⟩

def ceChecked : Checked :=
  ⟨⟨.cons .bool .nil, .cons .bool (.cons .i64 .nil), EffSet.empty⟩,
    [⟨0, ⟨.nil, .cons .i64 .nil, EffSet.empty⟩⟩]⟩

/-- The checker accepts it (by evaluation, as in `NobleM2.Refinement`). -/
theorem ce_accepted : check bootstrapEnv ceRequest ceCandidate
    = .accepted ceChecked := by
  native_decide

/-- And its acceptance is derivable: the node derives its own `[] -- [i64]`
interface, joined onto `[bool]` by segment replacement. -/
theorem ce_derives : Derives bootstrapEnv ceCandidate ceCandidate.body
    ceRequest.expected.stackIn ceRequest.expected.stackOut
    ceChecked.interface.effects := by
  have h0 : (litScheme (.i64 41)).instantiate ⟨[.stack .nil]⟩
      = some (.nil, .cons .i64 .nil, EffSet.empty) := by native_decide
  have hseq : Derives bootstrapEnv ceCandidate [0] (.cons .bool .nil)
      (.cons .bool (.cons .i64 .nil)) (EffSet.empty.union EffSet.empty) :=
    Derives.sequence rfl (NodeDerives.literal (resolvesTo_refl _ (by decide)) h0) rfl rfl
      (Derives.empty (.cons .bool (.cons .i64 .nil)))
  rw [union_empty_left] at hseq
  exact hseq

end Regression

end NobleM2
