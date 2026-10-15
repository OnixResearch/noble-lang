import NobleFoundation.Instr
import NobleContracts.Composition

/-!
# Type preservation for the pure MC1 model (issue #15, Phase 2)

For every normal-return execution of the pure MC1 model (`Run`/`Step`, all 22
pure words, quotation, composition, invocation, branching, `dip`), a stack typed
at the declared input interface ends typed at the declared output interface.

Scope: this is the MC1 *model* of `proofs/mc1/NobleContracts/Model.lean`. It
covers normal returns only (`Exec` is inductive, so diverging runs have no
derivation) and has no host effects, resources or failures. PO-02 remains open
for the whole language and for the implementation.
-/

namespace NobleFoundation

open NobleContracts

theorem stackTyped_length {s : Stack} {ts : List Ty} (h : StackTyped s ts) : s.length = ts.length := by
  induction h <;> simp_all

/-- Splitting a typed public stack at a boundary of known length. -/
theorem stackTyped_append_inv {xs ys : Stack} {ts us : List Ty}
    (h : StackTyped (xs ++ ys) (ts ++ us)) (hl : ys.length = us.length) :
    StackTyped xs ts ∧ StackTyped ys us := by
  induction xs generalizing ts with
  | nil =>
      have := stackTyped_length h
      simp at this
      have hts : ts.length = 0 := by omega
      rw [List.length_eq_zero_iff] at hts
      subst hts
      exact ⟨.nil, by simpa using h⟩
  | cons x xs ih =>
      cases ts with
      | nil =>
          have := stackTyped_length h
          simp at this; omega
      | cons t ts =>
          cases h with
          | cons hv hrest =>
              obtain ⟨h1, h2⟩ := ih hrest
              exact ⟨.cons hv h1, h2⟩

/-- Internal (top-first) stacks are typed through their public reversal. -/
abbrev ITyped (s : Stack) (ts : List Ty) : Prop := StackTyped s.reverse ts

theorem ityped_cons {v : Value} {s : Stack} {t : Ty} {ts : List Ty} :
    ITyped (v :: s) (ts ++ [t]) ↔ ITyped s ts ∧ HasType v t := by
  unfold ITyped
  simp only [List.reverse_cons]
  constructor
  · intro h
    obtain ⟨h1, h2⟩ := stackTyped_append_inv h rfl
    cases h2 with
    | cons hv _ => exact ⟨h1, hv⟩
  · rintro ⟨h1, hv⟩
    exact h1.append (.cons hv .nil)

theorem ityped_cons2 {a b : Value} {s : Stack} {ta tb : Ty} {ts : List Ty} :
    ITyped (b :: a :: s) (ts ++ [ta, tb]) ↔ ITyped s ts ∧ HasType a ta ∧ HasType b tb := by
  rw [show ts ++ [ta, tb] = (ts ++ [ta]) ++ [tb] by simp, ityped_cons, ityped_cons]
  tauto

theorem ityped_cons3 {a b c : Value} {s : Stack} {ta tb tc : Ty} {ts : List Ty} :
    ITyped (c :: b :: a :: s) (ts ++ [ta, tb, tc]) ↔
      ITyped s ts ∧ HasType a ta ∧ HasType b tb ∧ HasType c tc := by
  rw [show ts ++ [ta, tb, tc] = (ts ++ [ta, tb]) ++ [tc] by simp, ityped_cons, ityped_cons2]
  tauto

mutual
  /-- Preservation for sequences. -/
  theorem Run.preserve : {code : List Op} → {s t : Stack} → Run code s t →
      ∀ {ts ts' : List Ty}, CodeTyped code ts ts' → ITyped s ts → ITyped t ts'
    | _, _, _, .nil, _, _, hc, hs => by cases hc; exact hs
    | _, _, _, .cons hstep hrun, _, _, hc, hs => by
        cases hc with
        | cons hop hrest => exact Run.preserve hrun hrest (Step.preserve hstep hop hs)

  /-- Preservation for one operation (including nested executions). -/
  theorem Step.preserve : {op : Op} → {s t : Stack} → Step op s t →
      ∀ {ts ts' : List Ty}, OpTyped op ts ts' → ITyped s ts → ITyped t ts'
    | _, _, _, .lit, _, _, hop, hs => by
        cases hop with
        | lit hv => exact ityped_cons.mpr ⟨hs, hv⟩
    | _, _, _, .block, _, _, hop, hs => by
        cases hop with
        | block hp => exact ityped_cons.mpr ⟨hs, .program hp⟩
    | _, _, _, .dup, _, _, hop, hs => by
        cases hop with
        | dup =>
            obtain ⟨h1, hv⟩ := ityped_cons.mp hs
            exact ityped_cons2.mpr ⟨h1, hv, hv⟩
    | _, _, _, .drop, _, _, hop, hs => by
        cases hop with
        | drop => exact (ityped_cons.mp hs).1
    | _, _, _, .swap, _, _, hop, hs => by
        cases hop with
        | swap =>
            obtain ⟨h1, ha, hb⟩ := ityped_cons2.mp hs
            exact ityped_cons2.mpr ⟨h1, hb, ha⟩
    | _, _, _, .dip hp, _, _, hop, hs => by
        cases hop with
        | dip =>
            obtain ⟨h1, ha, hprog⟩ := ityped_cons2.mp hs
            cases hprog with
            | program hc => exact ityped_cons.mpr ⟨Run.preserve hp hc h1, ha⟩
    | _, _, _, .add, _, _, hop, hs => by
        cases hop with
        | add => exact ityped_cons.mpr ⟨(ityped_cons2.mp hs).1, .i64⟩
    | _, _, _, .sub, _, _, hop, hs => by
        cases hop with
        | sub => exact ityped_cons.mpr ⟨(ityped_cons2.mp hs).1, .i64⟩
    | _, _, _, .mul, _, _, hop, hs => by
        cases hop with
        | mul => exact ityped_cons.mpr ⟨(ityped_cons2.mp hs).1, .i64⟩
    | _, _, _, .equals, _, _, hop, hs => by
        cases hop with
        | equals => exact ityped_cons.mpr ⟨(ityped_cons2.mp hs).1, .bool⟩
    | _, _, _, .quote, _, _, hop, hs => by
        cases hop with
        | quote =>
            obtain ⟨h1, hv⟩ := ityped_cons.mp hs
            exact ityped_cons.mpr ⟨h1, .program (.cons (.lit hv) .nil)⟩
    | _, _, _, .compose, _, _, hop, hs => by
        cases hop with
        | compose =>
            obtain ⟨h1, hp, hq⟩ := ityped_cons2.mp hs
            cases hp with
            | program cp =>
                cases hq with
                | program cq => exact ityped_cons.mpr ⟨h1, .program (cp.append cq)⟩
    | _, _, _, .run hp, _, _, hop, hs => by
        cases hop with
        | run =>
            obtain ⟨h1, hprog⟩ := ityped_cons.mp hs
            cases hprog with
            | program hc => exact Run.preserve hp hc h1
    | _, _, _, .reflect, _, _, hop, hs => by
        cases hop with
        | reflect => exact ityped_cons.mpr ⟨(ityped_cons.mp hs).1, .«syntax»⟩
    | _, _, _, .unit, _, _, hop, hs => by
        cases hop with
        | unit => exact ityped_cons.mpr ⟨hs, .unit⟩
    | _, _, _, .pair, _, _, hop, hs => by
        cases hop with
        | pair =>
            obtain ⟨h1, ha, hb⟩ := ityped_cons2.mp hs
            exact ityped_cons.mpr ⟨h1, .pair ha hb⟩
    | _, _, _, .unpair, _, _, hop, hs => by
        cases hop with
        | unpair =>
            obtain ⟨h1, hp⟩ := ityped_cons.mp hs
            cases hp with
            | pair ha hb => exact ityped_cons2.mpr ⟨h1, ha, hb⟩
    | _, _, _, .inl, _, _, hop, hs => by
        cases hop with
        | inl =>
            obtain ⟨h1, hv⟩ := ityped_cons.mp hs
            exact ityped_cons.mpr ⟨h1, .inl hv⟩
    | _, _, _, .inr, _, _, hop, hs => by
        cases hop with
        | inr =>
            obtain ⟨h1, hv⟩ := ityped_cons.mp hs
            exact ityped_cons.mpr ⟨h1, .inr hv⟩
    | _, _, _, .caseLeft hp, _, _, hop, hs => by
        cases hop with
        | caseWord =>
            obtain ⟨h1, hsum, hpl, _⟩ := ityped_cons3.mp hs
            cases hsum with
            | inl hv =>
                cases hpl with
                | program hc => exact Run.preserve hp hc (ityped_cons.mpr ⟨h1, hv⟩)
    | _, _, _, .caseRight hp, _, _, hop, hs => by
        cases hop with
        | caseWord =>
            obtain ⟨h1, hsum, _, hpr⟩ := ityped_cons3.mp hs
            cases hsum with
            | inr hv =>
                cases hpr with
                | program hc => exact Run.preserve hp hc (ityped_cons.mpr ⟨h1, hv⟩)
    | _, _, _, .ifTrue hp, _, _, hop, hs => by
        cases hop with
        | ifWord =>
            obtain ⟨h1, _, hpt, _⟩ := ityped_cons3.mp hs
            cases hpt with
            | program hc => exact Run.preserve hp hc h1
    | _, _, _, .ifFalse hp, _, _, hop, hs => by
        cases hop with
        | ifWord =>
            obtain ⟨h1, _, _, hpf⟩ := ityped_cons3.mp hs
            cases hpf with
            | program hc => exact Run.preserve hp hc h1
    | _, _, _, .nil, _, _, hop, hs => by
        cases hop with
        | nil => exact ityped_cons.mpr ⟨hs, .list (by simp)⟩
    | _, _, _, .cons, _, _, hop, hs => by
        cases hop with
        | cons =>
            obtain ⟨h1, hv, hl⟩ := ityped_cons2.mp hs
            cases hl with
            | list hall =>
                refine ityped_cons.mpr ⟨h1, .list ?_⟩
                intro w hw
                rcases List.mem_cons.mp hw with rfl | hw
                · exact hv
                · exact hall w hw
    | _, _, _, .listNil hp, _, _, hop, hs => by
        cases hop with
        | listCase =>
            obtain ⟨h1, _, hpn, _⟩ := ityped_cons3.mp hs
            cases hpn with
            | program hc => exact Run.preserve hp hc h1
    | _, _, _, .listCons hp, _, _, hop, hs => by
        cases hop with
        | listCase =>
            obtain ⟨h1, hl, _, hpc⟩ := ityped_cons3.mp hs
            cases hl with
            | list hall =>
                cases hpc with
                | program hc =>
                    refine Run.preserve hp hc (ityped_cons2.mpr ⟨h1, hall _ (by simp), .list ?_⟩)
                    intro w hw; exact hall w (List.mem_cons_of_mem _ hw)
end

/-- **Preservation (public orientation).** A typed input stack and typed code
yield a typed output stack for every normal return. -/
theorem exec_preserves {code : List Op} {ts ts' : List Ty} {s t : Stack}
    (hc : CodeTyped code ts ts') (hs : StackTyped s ts) (he : Exec code s t) :
    StackTyped t ts' := by
  have := Run.preserve he hc (by simpa [ITyped] using hs)
  simpa [ITyped] using this

end NobleFoundation

namespace NobleFoundation
open NobleContracts

theorem ityped_snoc_inv {s : Stack} {ts : List Ty} {t : Ty} (h : ITyped s (ts ++ [t])) :
    ∃ v r, s = v :: r ∧ ITyped r ts ∧ HasType v t := by
  cases s with
  | nil =>
      have := stackTyped_length h; simp at this
  | cons v r => exact ⟨v, r, rfl, (ityped_cons.mp h).1, (ityped_cons.mp h).2⟩

/-- The non-control pure words: every rule is a single local stack rewrite. -/
def Primitive : Op → Prop
  | .lit _ | .block _ => True
  | .word w => w ∈ [0, 1, 2, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 19, 20]

/-- **Progress for primitive operations.** A well-typed primitive operation on
a well-typed stack always has a normal transition: no forbidden stuck state.
Control words (`dip` 3, `run` 10, `case` 17, `if` 18, `list.case` 21) take a
step exactly when the invoked body has a normal return, which is a termination
question that this big-step model does not decide. Host word 22 has no MC1
rule (host contracts are outside MC1). -/
theorem progress_primitive {op : Op} {ts ts' : List Ty} {s : Stack}
    (hp : Primitive op) (hop : OpTyped op ts ts') (hs : ITyped s ts) : ∃ t, Step op s t := by
  cases hop with
  | lit => exact ⟨_, .lit⟩
  | block => exact ⟨_, .block⟩
  | dup => obtain ⟨v, r, rfl, -, -⟩ := ityped_snoc_inv hs; exact ⟨_, .dup⟩
  | drop => obtain ⟨v, r, rfl, -, -⟩ := ityped_snoc_inv hs; exact ⟨_, .drop⟩
  | swap =>
      obtain ⟨b, r, rfl, hr, -⟩ := ityped_snoc_inv (ts := _ ++ [_]) (by simpa using hs)
      obtain ⟨a, r', rfl, -, -⟩ := ityped_snoc_inv hr
      exact ⟨_, .swap⟩
  | add | sub | mul | equals =>
      obtain ⟨b, r, rfl, hr, hb⟩ := ityped_snoc_inv (ts := _ ++ [_]) (by simpa using hs)
      obtain ⟨a, r', rfl, -, ha⟩ := ityped_snoc_inv hr
      obtain ⟨x, rfl⟩ := (hasType_i64_iff _).mp ha
      obtain ⟨y, rfl⟩ := (hasType_i64_iff _).mp hb
      first | exact ⟨_, .add⟩ | exact ⟨_, .sub⟩ | exact ⟨_, .mul⟩ | exact ⟨_, .equals⟩
  | quote => obtain ⟨v, r, rfl, -, -⟩ := ityped_snoc_inv hs; exact ⟨_, .quote⟩
  | compose =>
      obtain ⟨q, r, rfl, hr, hq⟩ := ityped_snoc_inv (ts := _ ++ [_]) (by simpa using hs)
      obtain ⟨p, r', rfl, -, hpv⟩ := ityped_snoc_inv hr
      cases hpv; cases hq
      exact ⟨_, .compose⟩
  | reflect =>
      obtain ⟨v, r, rfl, -, hv⟩ := ityped_snoc_inv hs
      cases hv; exact ⟨_, .reflect⟩
  | unit => exact ⟨_, .unit⟩
  | pair =>
      obtain ⟨b, r, rfl, hr, -⟩ := ityped_snoc_inv (ts := _ ++ [_]) (by simpa using hs)
      obtain ⟨a, r', rfl, -, -⟩ := ityped_snoc_inv hr
      exact ⟨_, .pair⟩
  | unpair =>
      obtain ⟨v, r, rfl, -, hv⟩ := ityped_snoc_inv hs
      cases hv; exact ⟨_, .unpair⟩
  | inl => obtain ⟨v, r, rfl, -, -⟩ := ityped_snoc_inv hs; exact ⟨_, .inl⟩
  | inr => obtain ⟨v, r, rfl, -, -⟩ := ityped_snoc_inv hs; exact ⟨_, .inr⟩
  | nil => exact ⟨_, .nil⟩
  | cons =>
      obtain ⟨l, r, rfl, hr, hl⟩ := ityped_snoc_inv (ts := _ ++ [_]) (by simpa using hs)
      obtain ⟨v, r', rfl, -, -⟩ := ityped_snoc_inv hr
      cases hl; exact ⟨_, .cons⟩
  | dip | run | caseWord | ifWord | listCase => simp [Primitive] at hp

/-- The host word has no normal MC1 transition at all (host effects are not
modelled; this is not a progress failure of the pure fragment). -/
theorem host_word_outside_model (s t : Stack) : ¬ Step (.word 22) s t := Step.host_unsupported

end NobleFoundation
