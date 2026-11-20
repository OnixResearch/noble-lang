import NobleContracts.NamedSubject
import NobleContracts.NamedV2
import NobleContracts.Obligation

/-! Revision-3 reusable-callee obligations over the NamedV1 semantics
(`PREUSE-CALL-01` / `PREUSE-LEMMA-01`). A separate module on purpose: revisions
1 and 2 (`NamedSubject`, `NamedV2`) are imported byte-for-byte unchanged and
nothing here changes their behavior. This is a semantic model, not source
acceptance, correspondence, or guest authority. -/
namespace NobleContracts.NamedV3
open NobleContracts

/-- T1: concatenation factors through the actual intermediate stack, for the
*named* semantics. This is the NamedV1 analog of `Rules.Run.append_iff` and
`Rules.exec_append_iff`, proved independently here rather than transported,
because the NamedV1 `Exec` reverses its stacks at the boundary. -/
theorem run_append_iff {env : NamedV1.Environment} {a b : List NamedV1.Op}
    {s t : NamedV1.Stack} :
    NamedV1.Run env (a ++ b) s t ↔ ∃ m, NamedV1.Run env a s m ∧ NamedV1.Run env b m t := by
  induction a generalizing s with
  | nil =>
      simp only [List.nil_append]
      exact ⟨fun h => ⟨s, .nil, h⟩, fun ⟨m, hm, hb⟩ => by cases hm; exact hb⟩
  | cons op ops ih =>
      constructor
      · intro h
        cases h with
        | cons hs hr =>
            obtain ⟨m, ha, hb⟩ := ih.mp hr
            exact ⟨m, .cons hs ha, hb⟩
      · rintro ⟨m, ha, hb⟩
        cases ha with
        | cons hs hr => exact .cons hs (ih.mpr ⟨m, hr, hb⟩)

@[simp] theorem exec_append_iff {env : NamedV1.Environment} {a b : List NamedV1.Op}
    {s t : NamedV1.Stack} :
    NamedV1.Exec env (a ++ b) s t ↔ ∃ m, NamedV1.Exec env a s m ∧ NamedV1.Exec env b m t := by
  constructor
  · intro h
    obtain ⟨m, ha, hb⟩ := run_append_iff.mp h
    exact ⟨m.reverse, by simpa [NamedV1.Exec] using ha, by simpa [NamedV1.Exec] using hb⟩
  · rintro ⟨m, ha, hb⟩
    exact run_append_iff.mpr ⟨m.reverse, by simpa [NamedV1.Exec] using ha,
      by simpa [NamedV1.Exec] using hb⟩

/-- T6: the `Holds₂` evaluation lemmas for the revision-3 vocabulary. `true`
always holds, and an ordering comparison holds exactly when the named
evaluation produced `true`, so no assertion is invented for undefined or
non-boolean operands. -/
@[simp] theorem holds_true₂ (env : NamedV1.Environment)
    (before after params : NamedV1.Stack) : NamedV2.Holds₂ env (.bool true) before after params := rfl

@[simp] theorem holds_eq₂_iff (env : NamedV1.Environment) (a b : Term)
    (before after params : NamedV1.Stack) :
    NamedV2.Holds₂ env (.eq a b) before after params ↔
      ∃ x y, NamedV2.evaluate₂ env a before after params = some x ∧
        NamedV2.evaluate₂ env b before after params = some y ∧ x = y := by
  simp only [NamedV2.Holds₂, NamedV2.evaluate₂]
  cases hx : NamedV2.evaluate₂ env a before after params with
  | none => simp
  | some x =>
      cases hy : NamedV2.evaluate₂ env b before after params with
      | none => simp
      | some y => simp

/-! ## T3: the frozen-snapshot embedding

The reviewed vocabulary of Decision 3: `Embedding`, an injective slot renaming
that preserves every row's origin, interface and body, and `Holds₂`
environment-independence for `maps`-free terms. Everything here is stated over
`NamedV1.Environment`, so the caller/callee boundary stays the single
`NamedV2.exportedNamedClaim` convention. -/

/-- Every call occurrence inside a row body resolves through that row's own
table. This is the row half of the `NamedV1.ExactClosure` binding obligation
and it is what makes "execute the callee's accepted body" meaningful without a
second lexical environment. -/
def Resolves (env : NamedV1.Environment) : Prop :=
  ∀ entry ∈ env.entries, ∀ use ∈ NamedV1.usesCode entry.body,
    ∃ target, env.lookup use.slot = some target

/-- The caller table extends the callee table: every row the callee resolves is
resolved by the caller at the same slot to the *identical* record. The caller
may add fresh rows at slots the callee leaves empty; it may not shadow or
rewrite a callee row. This is the frozen import snapshot of Decision 2, and it
is the "preserves origin, body and interface" half of `Embedding`. -/
def Extends (callee caller : NamedV1.Environment) : Prop :=
  ∀ entry ∈ callee.entries, caller.lookup entry.slot = some entry

/-- An entry embedding: the callee's rows form the frozen snapshot under the
caller's table (identity placement), and the accepted subject is placed at the
occurrence's own specialization slot. `rename` is injective *within this
embedding* and `fresh` keeps that slot out of the callee's own table, so the
occurrence's row can neither shadow nor rewrite a snapshot row and no two
occurrences of this embedding alias one slot. Cross-step slot uniqueness is
*not* implied: a chain carries one embedding per step, and distinct steps could
still name one slot. That is a caller/envelope obligation of the revision-3
checker (task 2), recorded in the design amendment of 2026-11-20. -/
structure Embedding (callee caller : NamedV1.Environment) where
  /-- the frozen import snapshot: every callee row resolves identically -/
  snapshot : Extends callee caller
  /-- occurrence identity → fresh per-use specialization slot -/
  rename : Nat → Nat
  /-- distinct occurrences get distinct slots -/
  injective : Function.Injective rename
  /-- a specialization slot is fresh: the callee table leaves it empty -/
  fresh : ∀ occurrence, callee.lookup (rename occurrence) = none

/-- One resolved call occurrence of an accepted callee. The caller's row has the
callee's accepted body and interface verbatim (`body`, `input`, `output`,
`origin`), so the call rule needs no callee body re-derivation and no second
claim form. -/
structure EntryEmbedding (caller : NamedV1.Environment) (subject : NamedV1.Subject)
    (entry : NamedV1.Entry) (use : NamedV1.Use) where
  embedding : Embedding subject.env caller
  /-- the occurrence is placed at its own injective specialization slot -/
  placement : use.slot = embedding.rename use.occurrence
  lookup : caller.lookup use.slot = some entry
  entryMatches : entry.matches use
  body : entry.body = subject.root
  input : entry.input = subject.input
  output : entry.output = subject.output
  origin : entry.origin = subject.provenance

theorem Embedding.slot_fresh {callee caller : NamedV1.Environment} (e : Embedding callee caller)
    (occurrence : Nat) : callee.lookup (e.rename occurrence) = none :=
  e.fresh occurrence

/-- A call occurrence's specialization slot is not a callee row, so installing
the caller's occurrence row at it can neither shadow nor rewrite a snapshot row.
Distinct occurrences of this embedding get distinct slots
(`Embedding.occurrences_distinct`). -/
theorem EntryEmbedding.slot_not_snapshot {caller : NamedV1.Environment}
    {subject : NamedV1.Subject} {entry : NamedV1.Entry} {use : NamedV1.Use}
    (e : EntryEmbedding caller subject entry use) : subject.env.lookup use.slot = none := by
  rw [e.placement]
  exact e.embedding.fresh use.occurrence

/-- Distinct call occurrences of one embedding get distinct specialization
slots. (Across a chain, each step has its own embedding, so a caller that needs
global slot uniqueness must also satisfy the design amendment's envelope
obligation.) -/
theorem Embedding.occurrences_distinct {callee caller : NamedV1.Environment}
    (e : Embedding callee caller) {left right : Nat} (distinct : left ≠ right) :
    e.rename left ≠ e.rename right :=
  fun same => distinct (e.injective same)

theorem lookup_entries {env : NamedV1.Environment} {slot : Nat} {entry : NamedV1.Entry}
    (h : env.lookup slot = some entry) : entry ∈ env.entries :=
  List.mem_of_find?_eq_some h

theorem lookup_slot {env : NamedV1.Environment} {slot : Nat} {entry : NamedV1.Entry}
    (h : env.lookup slot = some entry) : entry.slot = slot := by
  have found := List.find?_some h
  simpa [NamedV1.Environment.lookup, beq_iff_eq] using found

/-- The row half of the closure: every call occurrence in an accepted row body
resolves in the callee's own table. -/
theorem Subject.resolves (subject : NamedV1.Subject) : Resolves subject.env := by
  show ∀ entry ∈ subject.env.entries, ∀ use ∈ NamedV1.usesCode entry.body,
    ∃ target, subject.env.lookup use.slot = some target
  intro entry he use hu
  obtain ⟨target, hlookup, -⟩ := (subject.closure.2.1 entry he).2.2.2 use hu
  exact ⟨target, hlookup⟩

/-- The root half of the closure: every call occurrence in the accepted root
resolves in the callee's own table. -/
theorem Subject.root_resolves (subject : NamedV1.Subject) :
    ∀ use ∈ NamedV1.usesCode subject.root,
      ∃ target, subject.env.lookup use.slot = some target := by
  intro use hu
  obtain ⟨target, hlookup, -⟩ := (subject.closure.2.2.2.2 use hu).2.2
  exact ⟨target, hlookup⟩

/-- T3 (`embed`), forward half: a caller table that resolves every callee row
identically executes the callee's code with the same normal results. -/
theorem run_forward {callee caller : NamedV1.Environment} (h : Extends callee caller)
    {code : List NamedV1.Op} {stack final : NamedV1.Stack}
    (execution : NamedV1.Run callee code stack final) :
    NamedV1.Run caller code stack final := by
  apply NamedV1.Run.recOn
    (motive_1 := fun code s t _ => NamedV1.Run caller code s t)
    (motive_2 := fun op s t _ => NamedV1.Step caller op s t) execution
  all_goals intros
  all_goals first
    | exact .nil
    | exact .cons (by assumption) (by assumption)
    | exact .lit
    | exact .block
    | exact .dup
    | exact .drop
    | exact .swap
    | exact .dip (by assumption)
    | exact .add
    | exact .sub
    | exact .mul
    | exact .equals
    | exact .quote
    | exact .compose
    | exact .run (by assumption)
    | exact .reflect
    | exact .unit
    | exact .pair
    | exact .unpair
    | exact .inl
    | exact .inr
    | exact .caseLeft (by assumption)
    | exact .caseRight (by assumption)
    | exact .ifTrue (by assumption)
    | exact .ifFalse (by assumption)
    | exact .nilList
    | exact .consList
    | exact .listNil (by assumption)
    | exact .listCons (by assumption)
    | (exact .call
        (by
          have matched := (by assumption : NamedV1.Entry.matches _ _).1
          have row := h _ (lookup_entries (by assumption))
          rw [matched] at row
          exact row)
        (by assumption) (by assumption))

/-- A logical term that never mentions `maps`. `maps` is the only `Term`
constructor whose evaluation consults the environment, so a `maps`-free term
evaluates identically in every table. -/
def MapsFree : Term → Prop
  | .i64 _ | .bool _ | .unit | .input _ | .output _ | .param _ | .nil => True
  | .definition _ body => MapsFree body
  | .not body | .first body | .second body | .inl body | .inr body
  | .isLeft body | .left body | .right body | .isNil body | .head body
  | .tail body | .length body => MapsFree body
  | .and a b | .or a b | .implies a b | .eq a b | .lt a b | .le a b
  | .add a b | .sub a b | .mul a b | .pair a b | .cons a b => MapsFree a ∧ MapsFree b
  | .maps _ _ _ => False

/-- T3, second half: `Holds₂` is environment-independent for `maps`-free
terms. Only `maps` consults the named table, so two environments agree on the
whole evaluated value, not merely on its truth. -/
theorem evaluate₂_env_free {term : Term} (h : MapsFree term) :
    ∀ (env env' : NamedV1.Environment) (before after params : NamedV1.Stack),
      NamedV2.evaluate₂ env term before after params =
        NamedV2.evaluate₂ env' term before after params := by
  induction term with
  | i64 n => intro _ _ _ _ _; rfl
  | bool b => intro _ _ _ _ _; rfl
  | unit => intro _ _ _ _ _; rfl
  | input i => intro _ _ _ _ _; rfl
  | output i => intro _ _ _ _ _; rfl
  | param i => intro _ _ _ _ _; rfl
  | nil => intro _ _ _ _ _; rfl
  | definition i body ih =>
      intro env env' before after params
      exact ih h env env' before after params
  | not body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | first body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | second body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | inl body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | inr body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | isLeft body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | left body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | right body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | isNil body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | head body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | tail body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | length body ih =>
      intro env env' before after params
      have hb := ih h env env' before after params
      simp only [NamedV2.evaluate₂, hb]
  | and a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | or a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | implies a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | eq a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | lt a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | le a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | add a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | sub a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | mul a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | pair a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | cons a b iha ihb =>
      intro env env' before after params
      have ha := iha h.1 env env' before after params
      have hb := ihb h.2 env env' before after params
      simp only [NamedV2.evaluate₂, ha, hb]
  | maps p a b =>
      intro _ _ _ _ _
      exact absurd h (by simp [MapsFree])

/-- T3, second half, as an assertion: a `maps`-free assertion holds in one
named table exactly when it holds in another. -/
theorem holds₂_env_free {env env' : NamedV1.Environment} {term : Term}
    (h : MapsFree term) {before after params : NamedV1.Stack} :
    NamedV2.Holds₂ env term before after params ↔
      NamedV2.Holds₂ env' term before after params := by
  unfold NamedV2.Holds₂
  rw [evaluate₂_env_free h]

/-- A scalar value: the bounded profile's literals are `I64`/`Bool` (with the
`Unit`/`Text` constants the model already has) and never programs or syntax. -/
def Scalar : NamedV1.Value → Prop
  | .unit => True
  | .bool _ => True
  | .i64 _ => True
  | .text _ => True
  | .pair _ _ | .inl _ | .inr _ | .list _ | .program _ | .syntax _ => False

/-- The bounded profile's accepted body (Decision 1): scalar literals, the pure
scalar builtins `dup drop swap + - * =`, and named calls. Quotations, program
values and the higher-order words are outside the profile. -/
def Sequential : List NamedV1.Op → Prop
  | [] => True
  | .lit value :: rest => Scalar value ∧ Sequential rest
  | .word id :: rest =>
      (id = 0 ∨ id = 1 ∨ id = 2 ∨ id = 4 ∨ id = 5 ∨ id = 6 ∨ id = 7) ∧ Sequential rest
  | .call _ :: rest => Sequential rest
  | .block _ :: _rest => False

/-- Every call occurrence of the code resolves in the table. This is the
profile's static resolution obligation; it is what `NamedV1.ExactClosure` and
an accepted source submission establish for real code. -/
inductive CallsResolve (callee : NamedV1.Environment) : List NamedV1.Op → Prop where
  | nil : CallsResolve callee []
  | lit {value rest} : CallsResolve callee rest → CallsResolve callee (.lit value :: rest)
  | word {id rest} : CallsResolve callee rest → CallsResolve callee (.word id :: rest)
  | block {body rest} : CallsResolve callee rest → CallsResolve callee (.block body :: rest)
  | call {use rest} : (∃ target, callee.lookup use.slot = some target) →
      CallsResolve callee rest → CallsResolve callee (.call use :: rest)

/-- Static resolution of every call occurrence follows from the closure facts
`NamedV1.ExactClosure` binds, for any profile body. -/
theorem callsResolve_of_resolves {env : NamedV1.Environment} :
    ∀ {code : List NamedV1.Op}, Sequential code →
      (∀ use ∈ NamedV1.usesCode code, ∃ target, env.lookup use.slot = some target) →
      CallsResolve env code := by
  intro code
  induction code with
  | nil => intro _ _; exact .nil
  | cons op rest ih =>
      intro profile codeResolved
      cases op with
      | lit value =>
          exact .lit (ih profile.2 fun use hu => codeResolved use (by simp [NamedV1.usesCode, hu]))
      | word id =>
          exact .word (ih profile.2 fun use hu => codeResolved use (by simp [NamedV1.usesCode, hu]))
      | block body => exact profile.elim
      | call use =>
          exact .call (codeResolved use (by simp [NamedV1.usesCode, NamedV1.usesOp]))
            (ih profile fun use hu => codeResolved use (by simp [NamedV1.usesCode, hu]))

/-- T3 (`embed`), backward half on the bounded profile: when the callee
resolves every call occurrence, and every body is a profile body, the caller
cannot invent a different normal result. Without the profile and resolution
hypotheses this direction is false: a caller may add a row that a callee leaves
empty, and a stack program the callee cannot resolve can then run in the caller
table only. -/
theorem run_backward {callee caller : NamedV1.Environment} (h : Extends callee caller)
    (rows : Resolves callee)
    (bodies : ∀ entry ∈ callee.entries, Sequential entry.body) :
    ∀ {code : List NamedV1.Op} {stack final : NamedV1.Stack}, Sequential code →
      CallsResolve callee code →
      NamedV1.Run caller code stack final → NamedV1.Run callee code stack final := by
  intro code stack final codeProfile codeResolved execution
  revert codeProfile codeResolved
  apply NamedV1.Run.recOn
    (motive_1 := fun code s t _ => Sequential code → CallsResolve callee code →
      NamedV1.Run callee code s t)
    (motive_2 := fun op s t _ => Sequential [op] → CallsResolve callee [op] →
      NamedV1.Step callee op s t)
    (t := execution)
  · exact fun _ _ => .nil
  · intro op ops s m t step rest ihStep ihRest codeProfile codeResolved
    cases op with
    | lit value =>
        cases codeResolved with
        | lit hrest =>
            exact .cons
              (‹Sequential [NamedV1.Op.lit value] → CallsResolve callee [NamedV1.Op.lit value] →
                  NamedV1.Step callee (NamedV1.Op.lit value) s m›
                ⟨codeProfile.1, trivial⟩ (.lit .nil))
              (‹Sequential ops → CallsResolve callee ops → NamedV1.Run callee ops m t›
                codeProfile.2 hrest)
    | word id =>
        cases codeResolved with
        | word hrest =>
            exact .cons
              (‹Sequential [NamedV1.Op.word id] → CallsResolve callee [NamedV1.Op.word id] →
                  NamedV1.Step callee (NamedV1.Op.word id) s m›
                ⟨codeProfile.1, trivial⟩ (.word .nil))
              (‹Sequential ops → CallsResolve callee ops → NamedV1.Run callee ops m t›
                codeProfile.2 hrest)
    | call use =>
        cases codeResolved with
        | call huse hrest =>
            exact .cons
              (‹Sequential [NamedV1.Op.call use] → CallsResolve callee [NamedV1.Op.call use] →
                  NamedV1.Step callee (NamedV1.Op.call use) s m› trivial
                (.call huse .nil))
              (‹Sequential ops → CallsResolve callee ops → NamedV1.Run callee ops m t›
                codeProfile hrest)
    | block body =>
        exact absurd codeProfile (by simp [Sequential])
  -- the profile's scalar and arithmetic words
  all_goals intros
  all_goals first
    | (cases ‹CallsResolve callee [NamedV1.Op.call _]› with
       | call huse hrest =>
           obtain ⟨entry₀, hlookup₀⟩ := huse
           have callerLookup := ‹caller.lookup _ = some _›
           have callerMatches := ‹NamedV1.Entry.matches _ _›
           have mem₀ : entry₀ ∈ callee.entries := lookup_entries hlookup₀
           have slot₀ := lookup_slot hlookup₀
           have callerRow : caller.lookup entry₀.slot = some entry₀ := h entry₀ mem₀
           have callerRow' := callerRow
           rw [slot₀] at callerRow'
           have entryEq := Option.some.inj (callerRow'.symm.trans callerLookup)
           cases entryEq
           refine .call hlookup₀ callerMatches ?_
           exact ‹_ → _ → NamedV1.Run callee _ _ _› (bodies _ mem₀)
               (callsResolve_of_resolves (bodies _ mem₀)
                 (fun use hu => by
                   obtain ⟨target, hlookup⟩ := rows _ mem₀ use hu
                   exact ⟨target, hlookup⟩)))
    | exact .lit
    | exact .dup
    | exact .drop
    | exact .swap
    | exact .add
    | exact .sub
    | exact .mul
    | exact .equals
    | (simp_all [Sequential])


/-- T3 (`embed`): the frozen snapshot is exact on the bounded profile. The
caller and the callee execute the accepted body to the same normal results; no
stack, value or body is renamed, because a call occurrence inside an accepted
body is a source node of the callee module and keeps its table identity, and a
specialization slot is fresh in the callee's own table. -/
theorem embed {callee caller : NamedV1.Environment} (e : Embedding callee caller)
    (rows : Resolves callee) (bodies : ∀ entry ∈ callee.entries, Sequential entry.body)
    {code : List NamedV1.Op} (profile : Sequential code) (resolved : CallsResolve callee code)
    {stack final : NamedV1.Stack} :
    NamedV1.Run callee code stack final ↔ NamedV1.Run caller code stack final :=
  ⟨run_forward e.snapshot, run_backward e.snapshot rows bodies profile resolved⟩

/-- T2 (`callFrame`): an entry embedding plus the callee's exported claim give
the caller's frame discipline for one call occurrence. The claim is used at the
caller's own `tail`; no callee body is re-derived and no second claim form is
introduced. The profile and resolution premises are the bounded profile's
static obligations for the callee's own code. -/
theorem callFrame {caller : NamedV1.Environment} (subject : NamedV1.Subject)
    (entry : NamedV1.Entry) (use : NamedV1.Use)
    (e : EntryEmbedding caller subject entry use)
    (pre post : NamedV1.Stack → NamedV1.Stack → NamedV1.Stack → Prop)
    (claim : NamedV2.exportedNamedClaim subject [] pre post)
    (bodies : ∀ row ∈ subject.env.entries, Sequential row.body)
    (profile : Sequential subject.root) :
    ∀ frame args final,
      NamedV2.StackTyped subject.env args entry.input →
      pre args [] [] →
      NamedV1.Exec caller [.call use] (frame ++ args) final →
      ∃ res, final = frame ++ res ∧
        NamedV2.StackTyped subject.env res entry.output ∧ post args res [] := by
  intro frame args final hargs hpre execution
  have run : NamedV1.Run caller [.call use] (frame ++ args).reverse final.reverse := execution
  cases run with
  | cons step rest =>
      cases step with
      | call lookupCaller matchesCaller inner =>
          cases rest with
          | nil =>
              rw [e.lookup] at lookupCaller
              injection lookupCaller with sameEntry
              rw [← sameEntry, e.body] at inner
              have innerCallee : NamedV1.Run subject.env subject.root
                  (frame ++ args).reverse final.reverse :=
                (embed e.embedding (Subject.resolves subject) bodies profile
                  (callsResolve_of_resolves profile (Subject.root_resolves subject))).mpr inner
              rw [e.input] at hargs
              obtain ⟨res, hfinal, htyped, hpost⟩ :=
                claim frame args [] hargs .nil hpre final innerCallee
              exact ⟨res, hfinal, by rw [e.output]; exact htyped, hpost⟩


/-- One justified call in a chain: a resolved occurrence of an accepted callee
with its `callFrame` data, plus the caller's own bridges. `bridge` is the pure
logical step into the callee's precondition (with the callee-side argument
typing the claim needs); `join` is the caller's step out of the callee's
postcondition. Neither field can smuggle a program call: both are propositions
about the caller's own assertion, never a `pc-call` slice. -/
structure ChainStep (caller : NamedV1.Environment) (P Q : NamedV1.Stack → Prop) where
  subject : NamedV1.Subject
  entry : NamedV1.Entry
  use : NamedV1.Use
  embedding : EntryEmbedding caller subject entry use
  bodies : ∀ row ∈ subject.env.entries, Sequential row.body
  profile : Sequential subject.root
  pre : NamedV1.Stack → NamedV1.Stack → NamedV1.Stack → Prop
  post : NamedV1.Stack → NamedV1.Stack → NamedV1.Stack → Prop
  claim : NamedV2.exportedNamedClaim subject [] pre post
  /-- the caller's bridge into the callee precondition and argument typing -/
  bridge : ∀ args, P args →
    pre args [] [] ∧ NamedV2.StackTyped subject.env args entry.input
  /-- the caller's join out of the callee postcondition -/
  join : ∀ args res, P args → post args res [] →
    NamedV2.StackTyped subject.env res entry.output → Q res

/-- A chain of independently justified call steps: the empty chain is the
identity on the caller's assertion, and a step composes with the rest through
its own `join`. -/
inductive Chain (caller : NamedV1.Environment) : (NamedV1.Stack → Prop) →
    (NamedV1.Stack → Prop) → Type where
  | nil (P) : Chain caller P P
  | cons {P Q R} : ChainStep caller P Q → Chain caller Q R → Chain caller P R

/-- The caller code a chain justifies: exactly one call per step, in order. -/
def Chain.code {caller : NamedV1.Environment} {P Q : NamedV1.Stack → Prop} :
    Chain caller P Q → List NamedV1.Op
  | .nil _ => []
  | .cons step rest => .call step.use :: rest.code

@[simp] theorem run_nil_iff {env : NamedV1.Environment} {s t : NamedV1.Stack} :
    NamedV1.Run env [] s t ↔ t = s := by
  constructor
  · intro h
    cases h
    rfl
  · intro h
    subst h
    exact .nil

/-- T5 (`chainClaim`) execution half: a chain of call steps, each justified by
its resolved entry and the callee's own claim, transports the caller's
assertion through the whole sequence. No callee body is re-derived. -/
theorem chainExec {caller : NamedV1.Environment} :
    ∀ {P Q : NamedV1.Stack → Prop}, (chain : Chain caller P Q) →
      ∀ tail args final, P args →
        NamedV1.Exec caller (Chain.code chain) (tail ++ args) final →
        ∃ res, final = tail ++ res ∧ Q res := by
  intro P Q chain
  induction chain with
  | nil P =>
      intro tail args final hP execution
      refine ⟨args, ?_, hP⟩
      have empty : final.reverse = (tail ++ args).reverse := by
        have h : NamedV1.Run caller [] ((tail ++ args).reverse) final.reverse := by
          simpa [NamedV1.Exec, Chain.code] using execution
        simpa using (run_nil_iff.mp h)
      simpa using congrArg List.reverse empty
  | @cons P Q R step rest ih =>
      intro tail args final hP execution
      obtain ⟨mid, hcall, hrest⟩ :=
        (exec_append_iff (a := [NamedV1.Op.call step.use]) (b := Chain.code rest)).mp
          (by simpa [Chain.code] using execution)
      obtain ⟨res, hmid, htyped, hpost⟩ :=
        callFrame step.subject step.entry step.use step.embedding step.pre step.post
          step.claim step.bodies step.profile
          tail args mid (step.bridge args hP).2 (step.bridge args hP).1 hcall
      obtain ⟨res', hfinal, hQ⟩ :=
        ih tail res final (step.join args res hP hpost htyped)
          (by rw [← hmid]; exact hrest)
      exact ⟨res', hfinal, hQ⟩

/-- T5 (`chainClaim`): a chain of independently justified call steps implies the
caller's exported named claim. The two-step rule re-derived both callee bodies;
here each step is one resolved entry plus the callee's own claim. The landed
shape is narrower than "n-ary generalization of `twoStepClaim`" and the design
amendment of 2026-11-20 records it: `Chain.code` emits only calls (no literal or
builtin segments, so S04's `[ swap d.sub2 ]` is not expressible yet), each step
consumes the whole stack above the chain's single fixed `tail`, and the post is
output-only (`post := fun _ after _ => Q after`), so a relational claim such as
`out0 = in0 + 2` needs per-instance glue. -/
theorem chainClaim (subject : NamedV1.Subject) {P Q : NamedV1.Stack → Prop}
    (chain : Chain subject.env P Q) (root : subject.root = Chain.code chain)
    (typed : ∀ res, Q res → NamedV2.StackTyped subject.env res subject.output) :
    NamedV2.exportedNamedClaim subject []
      (fun before _ _ => P before) (fun _ after _ => Q after) := by
  intro tail before params _ _ hpre final execution
  rw [root] at execution
  obtain ⟨res, hfinal, hQ⟩ := chainExec chain tail before final hpre execution
  exact ⟨res, hfinal, typed res hQ, hQ⟩


/-! ## T4: the MC1 claim lifts along `liftCode` -/

/-- A scalar MC1 value: the literals the bounded profile admits. -/
def ScalarMC1 : NobleContracts.Value → Prop
  | .unit | .bool _ | .i64 _ | .text _ => True
  | .pair _ _ | .inl _ | .inr _ | .list _ | .program _ | .«syntax» _ => False

/-- MC1 profile code: scalar literals and the pure scalar builtins. MC1 has no
named calls; quotations and program values are outside the profile. -/
def ProfileCode : List NobleContracts.Op → Prop
  | [] => True
  | .lit value :: rest => ScalarMC1 value ∧ ProfileCode rest
  | .word id :: rest =>
      (id = 0 ∨ id = 1 ∨ id = 2 ∨ id = 4 ∨ id = 5 ∨ id = 6 ∨ id = 7) ∧ ProfileCode rest
  | .block _ :: _rest => False

@[simp] theorem liftStack_nil : NamedV1.liftStack ([] : NobleContracts.Stack) = [] := rfl

theorem liftValue_i64_iff {v : NobleContracts.Value} {n : BitVec 64} :
    NamedV1.liftValue v = .i64 n ↔ v = .i64 n := by
  cases v <;> simp [NamedV1.liftValue]

theorem step_lit {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {value : NobleContracts.Value} {m : NamedV1.Stack}
    (h : NamedV1.Step env (.lit (NamedV1.liftValue value)) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.lit value) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | lit => exact ⟨value :: before, by
      rw [← hb]
      simp only [NamedV1.liftStack, List.map_cons], .lit⟩

theorem step_dup {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {m : NamedV1.Stack}
    (h : NamedV1.Step env (.word 0) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.word 0) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | dup =>
      cases before with
      | nil => simp [NamedV1.liftStack] at hb
      | cons v vs =>
          refine ⟨v :: v :: vs, ?_, .dup⟩
          simp only [NamedV1.liftStack, List.map_cons, List.cons.injEq] at hb
          obtain ⟨h1, h2⟩ := hb
          simp only [NamedV1.liftStack, List.map_cons]
          rw [h1, h2]

theorem step_drop {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {m : NamedV1.Stack}
    (h : NamedV1.Step env (.word 1) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.word 1) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | drop =>
      cases before with
      | nil => simp [NamedV1.liftStack] at hb
      | cons v vs =>
          refine ⟨vs, ?_, .drop⟩
          simp only [NamedV1.liftStack, List.map_cons, List.cons.injEq] at hb
          exact hb.2.symm

theorem step_swap {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {m : NamedV1.Stack}
    (h : NamedV1.Step env (.word 2) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.word 2) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | swap =>
      cases before with
      | nil => simp [NamedV1.liftStack] at hb
      | cons x xs =>
          cases xs with
          | nil => simp [NamedV1.liftStack] at hb
          | cons y ys =>
              refine ⟨y :: x :: ys, ?_, .swap⟩
              simp only [NamedV1.liftStack, List.map_cons, List.cons.injEq] at hb
              obtain ⟨h1, h2, h3⟩ := hb
              simp only [NamedV1.liftStack, List.map_cons]
              rw [h1, h2, h3]

theorem step_add {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {m : NamedV1.Stack}
    (h : NamedV1.Step env (.word 4) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.word 4) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | add =>
      cases before with
      | nil => simp [NamedV1.liftStack] at hb
      | cons x xs =>
          cases xs with
          | nil => simp [NamedV1.liftStack] at hb
          | cons y ys =>
              simp only [NamedV1.liftStack, List.map_cons, List.cons.injEq] at hb
              obtain ⟨hx, hy, hs⟩ := hb
              rw [liftValue_i64_iff.mp hx, liftValue_i64_iff.mp hy] at *
              exact ⟨.i64 (_ + _) :: ys, by
                simp only [NamedV1.liftStack, List.map_cons, NamedV1.liftValue]
                rw [← hs], .add⟩

theorem step_sub {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {m : NamedV1.Stack}
    (h : NamedV1.Step env (.word 5) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.word 5) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | sub =>
      cases before with
      | nil => simp [NamedV1.liftStack] at hb
      | cons x xs =>
          cases xs with
          | nil => simp [NamedV1.liftStack] at hb
          | cons y ys =>
              simp only [NamedV1.liftStack, List.map_cons, List.cons.injEq] at hb
              obtain ⟨hx, hy, hs⟩ := hb
              rw [liftValue_i64_iff.mp hx, liftValue_i64_iff.mp hy] at *
              exact ⟨.i64 (_ - _) :: ys, by
                simp only [NamedV1.liftStack, List.map_cons, NamedV1.liftValue]
                rw [← hs], .sub⟩

theorem step_mul {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {m : NamedV1.Stack}
    (h : NamedV1.Step env (.word 6) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.word 6) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | mul =>
      cases before with
      | nil => simp [NamedV1.liftStack] at hb
      | cons x xs =>
          cases xs with
          | nil => simp [NamedV1.liftStack] at hb
          | cons y ys =>
              simp only [NamedV1.liftStack, List.map_cons, List.cons.injEq] at hb
              obtain ⟨hx, hy, hs⟩ := hb
              rw [liftValue_i64_iff.mp hx, liftValue_i64_iff.mp hy] at *
              exact ⟨.i64 (_ * _) :: ys, by
                simp only [NamedV1.liftStack, List.map_cons, NamedV1.liftValue]
                rw [← hs], .mul⟩

theorem step_equals {env : NamedV1.Environment} {before : NobleContracts.Stack}
    {m : NamedV1.Stack}
    (h : NamedV1.Step env (.word 7) (NamedV1.liftStack before) m) :
    ∃ μ, m = NamedV1.liftStack μ ∧ NobleContracts.Step (.word 7) before μ := by
  generalize hb : NamedV1.liftStack before = stack
  rw [hb] at h
  cases h with
  | equals =>
      cases before with
      | nil => simp [NamedV1.liftStack] at hb
      | cons x xs =>
          cases xs with
          | nil => simp [NamedV1.liftStack] at hb
          | cons y ys =>
              simp only [NamedV1.liftStack, List.map_cons, List.cons.injEq] at hb
              obtain ⟨hx, hy, hs⟩ := hb
              rw [liftValue_i64_iff.mp hx, liftValue_i64_iff.mp hy] at *
              exact ⟨.bool (_ == _) :: ys, by
                simp only [NamedV1.liftStack, List.map_cons, NamedV1.liftValue]
                rw [← hs], .equals⟩

/-- T4's reflection half: a named normal return of a lifted *profile* program
from a lifted stack is the lift of an MC1 normal return, so the named model adds
no behavior inside that fragment. This is not a claim about arbitrary named code
(quotations, program values and the higher-order words are excluded). -/
theorem run_reflect {env : NamedV1.Environment} :
    ∀ {code : List NobleContracts.Op} {before : NobleContracts.Stack}
      {final : NamedV1.Stack}, ProfileCode code →
      NamedV1.Run env (NamedV1.liftCode code) (NamedV1.liftStack before) final →
      ∃ after, final = NamedV1.liftStack after ∧ NobleContracts.Run code before after := by
  intro code
  induction code with
  | nil =>
      intro before final _ execution
      refine ⟨before, ?_, .nil⟩
      have h : NamedV1.Run env ([] : List NamedV1.Op) (NamedV1.liftStack before) final := by
        simpa [NamedV1.liftCode] using execution
      simpa using (run_nil_iff.mp h)
  | cons op rest ih =>
      intro before final profile execution
      cases execution with
      | cons first rest' =>
          cases op with
          | lit value =>
              simp only [NamedV1.liftOp] at first
              obtain ⟨μ, hμ, hstep⟩ := step_lit first
              rw [hμ] at rest'
              obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
              exact ⟨after, hfinal, .cons hstep hrun⟩
          | block body => exact profile.elim
          | word id =>
              simp only [NamedV1.liftOp] at first
              rcases profile.1 with rfl | rfl | rfl | rfl | rfl | rfl | rfl
              · obtain ⟨μ, hμ, hstep⟩ := step_dup first
                rw [hμ] at rest'
                obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
                exact ⟨after, hfinal, .cons hstep hrun⟩
              · obtain ⟨μ, hμ, hstep⟩ := step_drop first
                rw [hμ] at rest'
                obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
                exact ⟨after, hfinal, .cons hstep hrun⟩
              · obtain ⟨μ, hμ, hstep⟩ := step_swap first
                rw [hμ] at rest'
                obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
                exact ⟨after, hfinal, .cons hstep hrun⟩
              · obtain ⟨μ, hμ, hstep⟩ := step_add first
                rw [hμ] at rest'
                obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
                exact ⟨after, hfinal, .cons hstep hrun⟩
              · obtain ⟨μ, hμ, hstep⟩ := step_sub first
                rw [hμ] at rest'
                obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
                exact ⟨after, hfinal, .cons hstep hrun⟩
              · obtain ⟨μ, hμ, hstep⟩ := step_mul first
                rw [hμ] at rest'
                obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
                exact ⟨after, hfinal, .cons hstep hrun⟩
              · obtain ⟨μ, hμ, hstep⟩ := step_equals first
                rw [hμ] at rest'
                obtain ⟨after, hfinal, hrun⟩ := ih profile.2 rest'
                exact ⟨after, hfinal, .cons hstep hrun⟩

/-! The named model accepts every MC1 type derivation under `liftCode`. -/
mutual
  theorem hasType_lift {env : NamedV1.Environment} {v : NobleContracts.Value} {ty : Ty}
      (h : NobleContracts.HasType v ty) :
      NamedV1.HasType env (NamedV1.liftValue v) ty := by
    cases h with
    | unit => simp only [NamedV1.liftValue]; exact .unit
    | bool => simp only [NamedV1.liftValue]; exact .bool
    | i64 => simp only [NamedV1.liftValue]; exact .i64
    | text => simp only [NamedV1.liftValue]; exact .text
    | «syntax» => simp only [NamedV1.liftValue]; exact .«syntax»
    | pair hv hw =>
        simp only [NamedV1.liftValue]
        exact .pair (hasType_lift hv) (hasType_lift hw)
    | inl hv => simp only [NamedV1.liftValue]; exact .inl (hasType_lift hv)
    | inr hv => simp only [NamedV1.liftValue]; exact .inr (hasType_lift hv)
    | list mem =>
        simp only [NamedV1.liftValue]
        refine .list fun w hw => ?_
        obtain ⟨v', hv', rfl⟩ := List.mem_map.mp hw
        exact hasType_lift (mem v' hv')
    | program hc => simp only [NamedV1.liftValue]; exact .program (codeTyped_lift hc)

  theorem codeTyped_lift {env : NamedV1.Environment} {code : List NobleContracts.Op}
      {i o : List Ty} (h : NobleContracts.CodeTyped code i o) :
      NamedV1.CodeTyped env (NamedV1.liftCode code) i o := by
    cases h with
    | nil => exact .nil
    | cons hop hrest =>
        exact .cons (opTyped_lift hop) (codeTyped_lift hrest)

  theorem opTyped_lift {env : NamedV1.Environment} {op : NobleContracts.Op}
      {i o : List Ty} (h : NobleContracts.OpTyped op i o) :
      NamedV1.OpTyped env (NamedV1.liftOp op) i o := by
    cases h with
    | lit hv => simp only [NamedV1.liftOp]; exact .lit (hasType_lift hv)
    | block hc => simp only [NamedV1.liftOp]; exact .block (codeTyped_lift hc)
    | dup => simp only [NamedV1.liftOp]; exact .dup
    | drop => simp only [NamedV1.liftOp]; exact .drop
    | swap => simp only [NamedV1.liftOp]; exact .swap
    | dip => simp only [NamedV1.liftOp]; exact .dip
    | add => simp only [NamedV1.liftOp]; exact .add
    | sub => simp only [NamedV1.liftOp]; exact .sub
    | mul => simp only [NamedV1.liftOp]; exact .mul
    | equals => simp only [NamedV1.liftOp]; exact .equals
    | quote => simp only [NamedV1.liftOp]; exact .quote
    | compose => simp only [NamedV1.liftOp]; exact .compose
    | run => simp only [NamedV1.liftOp]; exact .run
    | reflect => simp only [NamedV1.liftOp]; exact .reflect
    | unit => simp only [NamedV1.liftOp]; exact .unit
    | pair => simp only [NamedV1.liftOp]; exact .pair
    | unpair => simp only [NamedV1.liftOp]; exact .unpair
    | inl => simp only [NamedV1.liftOp]; exact .inl
    | inr => simp only [NamedV1.liftOp]; exact .inr
    | caseWord => simp only [NamedV1.liftOp]; exact .caseWord
    | ifWord => simp only [NamedV1.liftOp]; exact .ifWord
    | nil => simp only [NamedV1.liftOp]; exact .nilList
    | cons => simp only [NamedV1.liftOp]; exact .consList
    | listCase => simp only [NamedV1.liftOp]; exact .listCase
end

theorem stackTyped_lift {env : NamedV1.Environment} {stack : NobleContracts.Stack}
    {types : List Ty} (h : NobleContracts.StackTyped stack types) :
    NamedV2.StackTyped env (NamedV1.liftStack stack) types := by
  induction h with
  | nil => exact .nil
  | cons hv _ ih =>
      simp only [NamedV1.liftStack, List.map_cons]
      exact .cons (hasType_lift hv) ih

/-- T4, `Exec` form of `run_reflect`: a named normal return of a lifted profile
program from a lifted stack is the lift of an MC1 normal return. -/
theorem exec_reflect {env : NamedV1.Environment} {code : List NobleContracts.Op}
    {before : NobleContracts.Stack} {final : NamedV1.Stack} (profile : ProfileCode code)
    (h : NamedV1.Exec env (NamedV1.liftCode code) (NamedV1.liftStack before) final) :
    ∃ after, final = NamedV1.liftStack after ∧ NobleContracts.Exec code before after := by
  have hrev : (NamedV1.liftStack before).reverse = NamedV1.liftStack before.reverse := by
    simp [NamedV1.liftStack, List.map_reverse]
  have hrun : NamedV1.Run env (NamedV1.liftCode code)
      (NamedV1.liftStack before.reverse) final.reverse := by
    rw [← hrev]
    exact h
  obtain ⟨after, hfinal, hrun⟩ := run_reflect profile hrun
  refine ⟨after.reverse, ?_, ?_⟩
  · have := congrArg List.reverse hfinal
    simpa [NamedV1.liftStack, List.map_reverse] using this
  · show NobleContracts.Run code before.reverse (List.reverse after).reverse
    rw [List.reverse_reverse]
    exact hrun

/-- T4 (`liftMc1Claim`): an MC1 exported claim for `ProfileCode` lifts to the
named semantics along `liftCode`. The named model supplies the MC1 preimages of
its stacks and receives the lifted MC1 result, with the named `StackTyped`
boundary and the MC1 postcondition. The execution is at a *lifted* tail: the
tail of `exportedNamedClaim` is an arbitrary named stack, and a named stack
holding call-bearing programs has no MC1 preimage, so this statement quantifies
only over the tails the MC1 claim itself quantifies. It is therefore **not** the
`exportedNamedClaim` that `callFrame`/`ChainStep` instantiate at a caller's
arbitrary frame; the design amendment of 2026-11-20 records what closing that
gap requires. -/
theorem liftMc1Claim {code : List NobleContracts.Op} {inputTypes outputTypes paramTypes : List Ty}
    {pre post : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop}
    (claim : NobleContracts.exportedClaim code inputTypes outputTypes paramTypes pre post)
    (profile : ProfileCode code)
    (subject : NamedV1.Subject) (hroot : subject.root = NamedV1.liftCode code)
    (hinput : subject.input = inputTypes) (houtput : subject.output = outputTypes) :
    ∀ (tail b p : NobleContracts.Stack) (final : NamedV1.Stack),
      NobleContracts.StackTyped b subject.input →
      NobleContracts.StackTyped p paramTypes →
      pre b [] p →
      NamedV1.Exec subject.env subject.root
        (NamedV1.liftStack tail ++ NamedV1.liftStack b) final →
      ∃ a, final = NamedV1.liftStack tail ++ NamedV1.liftStack a ∧
        NamedV2.StackTyped subject.env (NamedV1.liftStack a) subject.output ∧
        post b a p := by
  intro tail b p final hb hp hpre execution
  rw [hroot] at execution
  rw [hinput] at hb
  have hstack : NamedV1.liftStack tail ++ NamedV1.liftStack b =
      NamedV1.liftStack (tail ++ b) := by
    simp [NamedV1.liftStack, List.map_append]
  rw [hstack] at execution
  obtain ⟨after, hfinal, hexec⟩ := exec_reflect profile execution
  obtain ⟨a, ha, htyped, hpost⟩ := claim tail b p hb hp hpre after hexec
  refine ⟨a, ?_, ?_, hpost⟩
  · rw [hfinal, ha]
    simp [NamedV1.liftStack, List.map_append]
  · rw [houtput]
    exact stackTyped_lift htyped

end NobleContracts.NamedV3
