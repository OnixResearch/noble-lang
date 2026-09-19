/-
Reference model: work accounting, totality, and positive acceptance coverage
for the M2 fragment v0 (PO-10).

The fold that backs `check` runs under two finite budgets: a structural fuel
parameter that bounds every recursion, and the declared work budget that each
charged node draws down. This module proves

* `foldBody_work_bounded`: any successful fold returns a work remainder
  within the budget it started from, strictly below it whenever the body is
  nonempty — every charged node (scheme application plus join) spends at
  least one unit, so the remaining work is a termination measure for the
  fold's charged steps;
* `check_total`: the decision function is total — every request on every
  candidate reaches one of the five outcomes;
* `coverage_positive`: a concrete fragment input — the kernel's
  `sequence_and_literals_accept_arithmetic` fixture — is accepted and
  carries a declarative `Derives` witness at its expected interface.

Equations over the checker's substitution machinery are closed by
evaluation: `Scheme.instantiate` runs under well-founded recursion, which the
kernel's defeq checker does not unfold.
-/

import NobleM2.Check
import NobleM2.Judgment

namespace NobleM2

/-! ## Decidable outcome equality

Structural equality on the outcome domain is decidable; the reference modules
derive it only at the leaves (mirroring `NobleM2.Refinement`). -/

deriving instance DecidableEq for Interface
deriving instance DecidableEq for Derivation
deriving instance DecidableEq for Checked
deriving instance DecidableEq for Constraint
deriving instance DecidableEq for Diagnostic
deriving instance DecidableEq for UnsupportedKind
deriving instance DecidableEq for LimitKind
deriving instance DecidableEq for Outcome

/-! ## The charge and step invariants -/

/-- A successful charge spends exactly its cost: the remainder together with
the cost repays the budget in full. -/
theorem charge_ok {work cost budget : Nat} (h : charge work cost = some budget) :
    budget + cost = work := by
  unfold charge at h
  split at h
  · injection h with hb
    subst hb
    omega
  · injection h

/-- One charged step spends its cost and records one derivation: the returned
fold's work plus the cost repays the input fold's work, and the derivation
record grows by exactly one entry. -/
theorem stepFold_ok {fold : Fold} {cost : Nat} {node : Nat} {iface : Interface}
    {next : Fold} (h : stepFold fold cost node iface = .ok next) :
    next.work + cost = fold.work ∧
      next.derivations.length = fold.derivations.length + 1 := by
  unfold stepFold at h
  split at h
  · injection h
  · rename_i _ hcharge
    injection h with hnext
    subst hnext
    have hc := charge_ok hcharge
    simp only [List.length_append, List.length_cons, List.length_nil]
    exact ⟨hc, trivial⟩

/-- Every scheme application charges at least one unit of work. -/
theorem schemeCost_pos (scheme : Scheme) : 0 < schemeCost scheme := by
  unfold schemeCost
  omega

/-- Every join charges at least one unit of work. -/
theorem joinCost_pos (iface : Interface) : 0 < joinCost iface := by
  unfold joinCost
  omega

/-! ## The work measure -/

/-- Every successful fold stays within its work budget and strictly spends
from it on a nonempty body. The proof is structural induction on the fold's
fuel parameter — the recursion's structural measure — while the work
remainder is the semantic measure the charged steps draw down: a nonempty
body charges its first node at least one unit (a scheme cost or a join
cost), and a quotation's nested body starts from the same budget, charges
its join, and hands the outer body the remainder. -/
theorem foldBody_work_bounded {env : Env} {cand : Candidate} {limits : Limits} :
    ∀ (fuel : Nat) (depth : Nat) (body : List Nat) (stack : TyList)
      (derived : EffSet) (fold : Fold) (final : TyList) (finalBound : EffSet)
      (next : Fold),
    foldBody env cand limits depth fuel body stack derived fold
        = .ok final finalBound next →
    next.work ≤ fold.work ∧ (body ≠ [] → next.work < fold.work) := by
  intro fuel
  induction fuel with
  | zero =>
    intro depth body stack derived fold final finalBound next h
    simp only [foldBody] at h
    cases h
  | succ fuel ih =>
    intro depth body stack derived fold final finalBound next h
    cases body with
    | nil =>
      simp only [foldBody] at h
      injection h with _ _ hnext
      subst hnext
      exact ⟨Nat.le_refl _, fun hne => absurd rfl hne⟩
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
            cases hj : joinInterface stack derived iface limits with
            | error problem => simp only [hap, hj] at h; cases h
            | ok pair =>
              obtain ⟨joined, bound⟩ := pair
              cases hst : stepFold fold (schemeCost (litScheme lit)
                  + joinCost iface) id iface with
              | error problem => simp only [hap, hj, hst] at h; cases h
              | ok stepped =>
                simp only [hap, hj, hst] at h
                obtain ⟨hle, _⟩ := ih depth rest joined bound stepped final
                  finalBound next h
                obtain ⟨hwork, _⟩ := stepFold_ok hst
                have hpos : 0 < schemeCost (litScheme lit) + joinCost iface := by
                  have := schemeCost_pos (litScheme lit)
                  have := joinCost_pos iface
                  omega
                exact ⟨Nat.le_trans hle (by omega), fun _ => by omega⟩
        | invocation index inst =>
          simp only [hnode] at h
          cases hscheme : env.scheme index with
          | none => simp only [hscheme] at h; cases h
          | some scheme =>
            simp only [hscheme] at h
            cases hap : applyScheme env limits scheme inst with
            | error problem => simp only [hap] at h; cases h
            | ok pair =>
              obtain ⟨iface, _⟩ := pair
              simp only [hap] at h
              split at h
              · cases h
              · cases hj : joinInterface stack derived iface limits with
                | error problem => simp only [hap, hj] at h; cases h
                | ok pair =>
                  obtain ⟨joined, bound⟩ := pair
                  cases hst : stepFold fold (schemeCost scheme
                      + joinCost iface) id iface with
                  | error problem => simp only [hap, hj, hst] at h; cases h
                  | ok stepped =>
                    simp only [hap, hj, hst] at h
                    obtain ⟨hle, _⟩ := ih depth rest joined bound stepped final
                      finalBound next h
                    obtain ⟨hwork, _⟩ := stepFold_ok hst
                    have hpos : 0 < schemeCost scheme + joinCost iface := by
                      have := schemeCost_pos scheme
                      have := joinCost_pos iface
                      omega
                    exact ⟨Nat.le_trans hle (by omega), fun _ => by omega⟩
        | quotation qbody inst =>
          simp only [hnode] at h
          cases hap : applyScheme env limits quotationScheme inst with
          | error problem => simp only [hap] at h; cases h
          | ok pair =>
            obtain ⟨iface, resolved⟩ := pair
            simp only [hap] at h
            split at h
            · cases h
            · cases hs1 : resolved.stack 1 with
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
                    | failed problem => simp only [hinner] at h; cases h
                    | ok innerFinal innerBound inner =>
                      simp only [hinner] at h
                      split at h
                      · cases h
                      · split at h
                        · cases h
                        · cases hj : joinInterface stack derived iface limits with
                          | error problem => simp only [hj] at h; cases h
                          | ok pair =>
                            obtain ⟨joined, outerBound⟩ := pair
                            cases hst : stepFold inner (joinCost iface) id iface with
                            | error problem => simp only [hj, hst] at h; cases h
                            | ok stepped =>
                              simp only [hj, hst] at h
                              have h1 : inner.work ≤ fold.work :=
                                (ih (depth + 1) qbody start EffSet.empty
                                  ⟨fold.work, fold.derivations⟩ innerFinal
                                  innerBound inner hinner).1
                              obtain ⟨h2, _⟩ := stepFold_ok hst
                              have h3 : next.work ≤ stepped.work :=
                                (ih depth rest joined outerBound stepped final
                                  finalBound next h).1
                              have h4 : 0 < joinCost iface := joinCost_pos iface
                              exact
                                ⟨Nat.le_trans h3 (by omega), fun _ => by omega⟩

/-- The decision function is total: the entry fold's recursion is structural
in its fuel parameter, so every request on every candidate reaches one of the
five outcomes; `foldBody_work_bounded` bounds the work that run can spend. -/
theorem check_total (env : Env) (request : Request) (cand : Candidate) :
    ∃ out, check env request cand = out :=
  ⟨check env request cand, rfl⟩

/-! ## Positive acceptance coverage -/

namespace Coverage

/-- One-entry-per-type stack builder (the kernel fixtures' `tyList`). -/
def ty : List Ty → TyList
  | [] => .nil
  | t :: ts => .cons t (ty ts)

/-- The arithmetic-sequence candidate of the kernel's
`sequence_and_literals_accept_arithmetic` acceptance test: two `i64`
literals followed by the resolved `add` word (bootstrap definition 4). -/
def cand : Candidate :=
  ⟨0, 0,
    [ .literal (.i64 41) ⟨[.stack (ty [])]⟩,
      .literal (.i64 1) ⟨[.stack (ty [.i64])]⟩,
      .invocation 4 ⟨[.stack (ty [])]⟩ ],
    [0, 1, 2] ⟩

/-- Its request: empty entry stack, `[i64]` result stack, empty allowed
bound, and the kernel tests' default limits. -/
def req : Request :=
  ⟨64, ⟨ty [], ty [.i64], EffSet.empty⟩,
    ⟨65536, 256, 32, 64, 16, 10000, 64⟩⟩

/-- The accepted result the kernel test asserts: interface `[] -- [i64]`
with an empty bound and three node derivations. -/
def checked : Checked :=
  ⟨⟨ty [], ty [.i64], EffSet.empty⟩,
    [ ⟨0, ⟨ty [], ty [.i64], EffSet.empty⟩⟩,
      ⟨1, ⟨ty [.i64], ty [.i64, .i64], EffSet.empty⟩⟩,
      ⟨2, ⟨ty [.i64, .i64], ty [.i64], EffSet.empty⟩⟩ ]⟩

/-- The checker accepts the fixture (closed by evaluation, as in
`NobleM2.Refinement`). -/
theorem accepted : check bootstrapEnv req cand = .accepted checked := by
  native_decide

/-! The declarative witness, built by hand from the judgment's constructors.
Each node's premise is its scheme instantiation, closed by evaluation. -/

theorem lit41 : (litScheme (.i64 41)).instantiate ⟨[.stack .nil]⟩
    = some (ty [], ty [.i64], EffSet.empty) := by
  native_decide

theorem lit1 : (litScheme (.i64 1)).instantiate ⟨[.stack (ty [.i64])]⟩
    = some (ty [.i64], ty [.i64, .i64], EffSet.empty) := by
  native_decide

/-- The bootstrap `add` word's rank-1 scheme, `S -- S i64 i64` to `S -- S i64`. -/
def addScheme : Scheme :=
  schemeOf [Kind.stack]
    (PartList.ofList [stackVar 0, patternPart .i64, patternPart .i64])
    (PartList.ofList [stackVar 0, patternPart .i64])
    SlotList.nil

theorem addLook : bootstrapEnv.scheme 4 = some addScheme := by
  native_decide

theorem addInst : addScheme.instantiate ⟨[.stack .nil]⟩
    = some (ty [.i64, .i64], ty [.i64], EffSet.empty) := by
  native_decide

/-- The fixture's entry body derives its checked interface: EMPTY closes the
body after the three nodes, and every union is the empty bound. -/
theorem derives : Derives bootstrapEnv cand cand.body req.expected.stackIn
    req.expected.stackOut checked.interface.effects := by
  have h3 : Derives bootstrapEnv cand [2] (ty [.i64, .i64]) (ty [.i64])
      (EffSet.empty.union EffSet.empty) :=
    Derives.sequence rfl
      (NodeDerives.word addLook (resolvesTo_refl _ (by decide)) addInst trivial) rfl rfl
      (Derives.empty (ty [.i64]))
  have h2 : Derives bootstrapEnv cand [1, 2] (ty [.i64]) (ty [.i64])
      (EffSet.empty.union (EffSet.empty.union EffSet.empty)) :=
    Derives.sequence rfl (NodeDerives.literal (resolvesTo_refl _ (by decide)) lit1)
      rfl rfl h3
  have h1 : Derives bootstrapEnv cand [0, 1, 2] (ty []) (ty [.i64])
      (EffSet.empty.union (EffSet.empty.union (EffSet.empty.union EffSet.empty))) :=
    Derives.sequence rfl (NodeDerives.literal (resolvesTo_refl _ (by decide)) lit41)
      rfl rfl h2
  simp only [cand, req, checked] at h1 ⊢
  rw [union_empty_left, union_empty_left, union_empty_left] at h1
  exact h1

end Coverage

/-! ## The v1 work measure (PO-10, V-CHECK-05, fragment v1)

The resolution walk charges before every hop and stays inside its hop
budget (B-CHECK-05); the dependency walk and schema scan charge before they
traverse and fail closed (B-CHECK-02); the fold bounds its charged steps as
in v0; and every word's application charges at least one unit. The v0
theorem names (`check_total`, `foldBody_work_bounded`) are restated over
the extended checker by the v1 names below. -/

/-- One chain walk spends at most the budget it is handed. -/
theorem followFrom_work_bounded (witness : Inst) :
    ∀ (remaining current hops : Nat) (target left : Nat),
      witness.followFrom current remaining hops = .ok (target, left) →
      left ≤ remaining
  | 0, _, _, _, _, h => by
      rw [Inst.followFrom] at h
      split at h
      · exact absurd h (by simp)
      · exact absurd h (by simp)
  | remaining + 1, current, hops, target, left, h => by
      rw [Inst.followFrom] at h
      split at h
      · exact absurd h (by simp)
      · split at h
        · omega
        · cases hbind : witness.bindings[current]? with
          | none => rw [hbind] at h; exact absurd h (by simp)
          | some binding =>
              rw [hbind] at h
              cases binding with
              | stack _ | value _ | effect _ =>
                  injection h with hp
                  injection hp with htarget hleft
                  subst htarget
                  subst hleft
                  exact Nat.le_refl _
              | ref next =>
                  exact Nat.le_trans
                    (followFrom_work_bounded witness remaining next (hops + 1) target left
                      (by simpa using h))
                    (by omega)

/-- One binding's resolution spends at most the budget it is given. -/
theorem resolveOne_work_bounded {kinds : List Kind} {witness : Inst} {index : Nat}
    {binding : Binding} {remaining : Nat} {b : Binding} {left : Nat}
    (h : Inst.resolveOne kinds witness index binding remaining = .ok (b, left)) :
    left ≤ remaining := by
  simp only [Inst.resolveOne] at h
  cases binding with
  | stack _ | value _ | effect _ =>
      injection h with pe
      injection pe with hb hl
      omega
  | ref v =>
      simp only [] at h
      cases hf : witness.followFrom v remaining 0 with
      | error => simp only [hf] at h; exact absurd h (by simp)
      | ok pair =>
          obtain ⟨terminal, left1⟩ := pair
          simp only [hf] at h
          have hb := followFrom_work_bounded witness remaining v 0 terminal left1 hf
          cases hk : kinds[index]? with
          | none => simp only [hk] at h; exact absurd h (by simp)
          | some kind =>
              simp only [hk] at h
              cases ht : witness.bindings[terminal]? with
              | none => simp only [ht] at h; exact absurd h (by simp)
              | some term =>
                  simp only [ht] at h
                  split at h
                  · injection h with pe
                    injection pe with hb' hl'
                    subst hb'
                    subst hl'
                    omega
                  · exact absurd h (by simp)

/-- One resolution pass over a witness's bindings stays inside the budget. -/
theorem resolveList_work_bounded (kinds : List Kind) (witness : Inst) :
    ∀ (bs : List Binding) (index remaining : Nat)
      (result : List Binding) (left : Nat),
      Inst.resolveList kinds witness index remaining bs = .ok (result, left) →
      left ≤ remaining
  | [], _, _, _, _, h => by
      simp only [Inst.resolveList] at h
      injection h with pe
      injection pe with re hl
      subst re
      subst hl
      exact Nat.le_refl _
  | binding :: rest, index, remaining, _, _, h => by
      simp only [Inst.resolveList] at h
      cases hone : Inst.resolveOne kinds witness index binding remaining with
      | error => simp only [hone] at h; exact absurd h (by simp)
      | ok pair1 =>
          obtain ⟨b, left1⟩ := pair1
          simp only [hone] at h
          have hb := resolveOne_work_bounded hone
          cases hrec : Inst.resolveList kinds witness (index + 1) left1 rest with
          | error => simp only [hrec] at h; exact absurd h (by simp)
          | ok pair2 =>
              obtain ⟨bs', left2⟩ := pair2
              simp only [hrec] at h
              injection h with pe
              injection pe with re hl
              subst re
              subst hl
              exact Nat.le_trans
                (resolveList_work_bounded kinds witness rest (index + 1) left1 bs' left2 hrec)
                hb

/-- The resolution walk stays inside its hop budget: a successful
resolution returns no more leftover than the fuel it started with. -/
theorem resolve_work_bounded {kinds : List Kind} {witness resolved : Inst}
    {fuel left : Nat}
    (h : Inst.resolve kinds witness fuel = .ok (resolved, left)) : left ≤ fuel := by
  rw [Inst.resolve] at h
  cases hres : Inst.resolveList kinds witness 0 fuel witness.bindings with
  | error => simp only [hres] at h; exact absurd h (by simp)
  | ok pair =>
      obtain ⟨bs, left'⟩ := pair
      simp only [hres] at h
      injection h with pe
      injection pe with inst_eq left_eq
      subst inst_eq
      subst left_eq
      exact resolveList_work_bounded kinds witness witness.bindings 0 fuel bs left' hres

/-- The schema scan stays inside its charged budget: a successful scan
returns no more leftover than the work it started with — every inspected
declaration charges one unit before it is looked at, and the `.exhausted`
outcome is taken exactly when the budget runs out. -/
theorem validateSchemas_ok_bounded (env : Env) :
    ∀ (work : Nat) (decls : List SchemaDecl) {left : Nat},
      validateSchemas env work decls = .ok left → left ≤ work := by
  intro work decls
  induction decls generalizing work with
  | nil =>
      intro left h
      simp only [validateSchemas] at h
      injection h with hl
      subst hl
      exact Nat.zero_le work
  | cons decl rest ih =>
      intro left h
      simp only [validateSchemas] at h
      split at h
      · exact absurd h (by simp)
      · split at h
        · exact absurd h (by simp)
        · split at h
          · exact absurd h (by simp)
          · exact Nat.le_trans (ih (work - 1) h) (Nat.pred_le work)

/-- Totality over fragment v1: every request on every candidate reaches one
of the five outcomes, and both external-data validation walks terminate
(structurally bounded by their fuel). -/
theorem check_total_v1 (env : Env) (request : Request) (cand : Candidate) :
    ∃ out, check env request cand = out ∧
      (∃ r, validateDeps env request.limits.work = r) ∧
      (∃ r2, validateSchemas env request.limits.work env.schemas = r2) :=
  ⟨check env request cand, rfl, ⟨_, rfl⟩, ⟨_, rfl⟩⟩

/-- Per-word cost positivity: every one of the 23 table entries charges at
least one work unit per application. -/
theorem word_cost_positive (n : Nat) (h : n < 23) :
    ∃ word, wordTable[n]? = some word ∧ 0 < schemeCost word.scheme := by
  obtain ⟨word, hw, _⟩ := wordScheme_lookup n h
  exact ⟨word, hw, schemeCost_pos word.scheme⟩

/-- The v1 work bound of the fold, restated over the extended checker: the
v0 statement, with each application's witness resolution bounded by
`resolve_work_bounded` above. -/
theorem foldBody_work_bounded_v1 {env : Env} {cand : Candidate} {limits : Limits} :
    ∀ (fuel : Nat) (depth : Nat) (body : List Nat) (stack : TyList)
      (derived : EffSet) (fold : Fold) (final : TyList) (finalBound : EffSet)
      (next : Fold),
    foldBody env cand limits depth fuel body stack derived fold
        = .ok final finalBound next →
    next.work ≤ fold.work ∧ (body ≠ [] → next.work < fold.work) :=
  foldBody_work_bounded


/-- Positive acceptance coverage (PO-10): a fragment input exists that the
checker accepts and that carries a declarative derivation at its expected
interface. -/
theorem coverage_positive :
    ∃ env req cand checked,
      check env req cand = .accepted checked ∧
      TypingDerivation env req cand checked :=
  ⟨bootstrapEnv, Coverage.req, Coverage.cand, Coverage.checked,
    Coverage.accepted, Coverage.derives⟩

end NobleM2
