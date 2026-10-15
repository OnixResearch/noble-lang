import NobleFoundation.Sequence
import NobleFoundation.Preservation

/-!
# Construction/execution, composition, quotation and reflection laws (issue #15, Phase 2)

Each law below names its observation relation, and the three relations are
never conflated:

* `BehEq p q` — **behavioral equality** of normal returns, `∀ s t, Exec p s t ↔
  Exec q s t`. MC1 is pure, so this relation has no trace, failure, quota or
  allocation component.
* recipe equality — `p = q` on `List Op`, which is what `reflect` observes;
* definition/proof-subject identity is outside MC1, where code has no names.

Every law is about the pure MC1 model. Effectful laws (ordered traces, host
responses, failures) remain open, as do the resource laws, which belong to
#3/#11.
-/

namespace NobleFoundation

open NobleContracts

/-- Behavioral equality of normal returns (the observation for composition laws). -/
def BehEq (p q : List Op) : Prop := ∀ s t, Exec p s t ↔ Exec q s t

theorem BehEq.refl (p : List Op) : BehEq p p := fun _ _ => Iff.rfl

/-- Behavioral equality is a congruence for sequencing on both sides. -/
theorem BehEq.append {p p' q q' : List Op} (hp : BehEq p p') (hq : BehEq q q') :
    BehEq (p ++ q) (p' ++ q') := by
  intro s t
  simp only [exec_append_iff]
  constructor
  · rintro ⟨m, h1, h2⟩; exact ⟨m, (hp s m).mp h1, (hq m t).mp h2⟩
  · rintro ⟨m, h1, h2⟩; exact ⟨m, (hp s m).mpr h1, (hq m t).mpr h2⟩

/-- Running behaviorally equal programs gives the same normal results. -/
theorem BehEq.run {p q : List Op} (h : BehEq p q) (s t : Stack) :
    Exec [.block p, .word 10] s t ↔ Exec [.block q, .word 10] s t := by
  have hr : ∀ r : List Op, Exec [.block r, .word 10] s t ↔ Exec r s t := fun r => by
    rw [show [Op.block r, .word 10] = [.block r] ++ [.word 10] from rfl, exec_append_iff]
    constructor
    · rintro ⟨m, hm, hx⟩; rw [exec_block_iff.mp hm] at hx; exact exec_run_iff.mp hx
    · intro hx; exact ⟨_, exec_block_iff.mpr rfl, exec_run_iff.mpr hx⟩
  rw [hr, hr]; exact h s t

/-- **Construction/execution separation.** Building a program value pushes
exactly that value whatever the body is. This holds even when the body has no
normal return (here the host word, which MC1 does not model): its latent
behavior is not performed by construction. -/
theorem construction_is_pure (p : List Op) (s t : Stack) :
    Exec [.block p] s t ↔ t = s ++ [.program p] := exec_block_iff

theorem latent_effect_not_performed (s : Stack) :
    Exec [.block [.word 22]] s (s ++ [.program [.word 22]]) ∧
      ∀ t, ¬ Exec [.block [.word 22], .word 10] s t := by
  refine ⟨exec_block_iff.mpr rfl, fun t h => ?_⟩
  rw [show [Op.block [.word 22], .word 10] = [.block [.word 22]] ++ [.word 10] from rfl,
    exec_append_iff] at h
  obtain ⟨m, hm, hr⟩ := h
  rw [exec_block_iff.mp hm, exec_run_iff] at hr
  simp [Exec] at hr

/-- **Composition is recipe-associative** (hence also behaviorally and
reflectively associative): both bracketings push the identical recipe. -/
theorem compose_assoc (p q r : List Op) (s t : Stack) :
    (Exec [.block p, .block q, .word 9, .block r, .word 9] s t ↔
      t = s ++ [.program (p ++ q ++ r)]) ∧
    (Exec [.block p, .block q, .block r, .word 9, .word 9] s t ↔
      t = s ++ [.program (p ++ q ++ r)]) := by
  constructor <;> simp [Exec]

/-- **Identity**: composing with the empty program on either side pushes the
identical recipe. -/
theorem compose_identity (p : List Op) (s t : Stack) :
    (Exec [.block [], .block p, .word 9] s t ↔ t = s ++ [.program p]) ∧
    (Exec [.block p, .block [], .word 9] s t ↔ t = s ++ [.program p]) := by
  constructor <;> simp [Exec]

/-- **Quotation**: running a quotation pushes the quoted value (re-stated from
MC1 `exec_quote_run_iff`), and reflecting it observes the one-literal recipe. -/
theorem quote_laws (v : Value) (s t : Stack) :
    (Exec [.word 8, .word 10] (s ++ [v]) t ↔ t = s ++ [v]) ∧
    (Exec [.word 8, .word 11] (s ++ [v]) t ↔ t = s ++ [.«syntax» [.lit v]]) := by
  refine ⟨exec_quote_run_iff, ?_⟩
  simp [Exec, List.reverse_append]

/-- **Reflection of composition** observes the concatenated recipe, unnormalized. -/
theorem reflect_compose (p q : List Op) (s t : Stack) :
    Exec [.block p, .block q, .word 9, .word 11] s t ↔ t = s ++ [.«syntax» (p ++ q)] := by
  simp [Exec]

/-- **Do not conflate behavior with recipes**: `1 1 +` and `2` are behaviorally
equal but reflect to different recipes, so behavioral equality is not a
congruence under `reflect`. -/
theorem behavior_is_not_recipe :
    BehEq [.lit (i64v 1), .lit (i64v 1), .word 4] [.lit (i64v 2)] ∧
    [Op.lit (i64v 1), .lit (i64v 1), .word 4] ≠ [.lit (i64v 2)] ∧
    ∀ s t, Exec [.block [.lit (i64v 1), .lit (i64v 1), .word 4], .word 11] s t →
      ¬ Exec [.block [.lit (i64v 2)], .word 11] s t := by
  refine ⟨?_, by simp, ?_⟩
  · intro s t
    have h1 : refRun [.push 1, .push 1, .add] [] = some [2] := by decide
    have h2 : refRun [.push 2] [] = some [2] := by decide
    have e1 := exec_lowerAll_from_empty h1 s t
    have e2 := exec_lowerAll_from_empty h2 s t
    simp only [lowerAll, List.map, AOp.lower] at e1 e2
    rw [e1, e2]
  · intro s t h1 h2
    have r1 : t = s ++ [.«syntax» [.lit (i64v 1), .lit (i64v 1), .word 4]] := by
      simpa [Exec, List.reverse_append] using h1
    have r2 : t = s ++ [.«syntax» [.lit (i64v 2)]] := by
      simpa [Exec, List.reverse_append] using h2
    rw [r1] at r2
    simp at r2

private theorem snoc2_inj {xs ys : List Ty} {a b c d : Ty}
    (h : xs ++ [a, b] = ys ++ [c, d]) : xs = ys ∧ a = c ∧ b = d := by
  obtain ⟨h1, h2⟩ := List.append_inj h (by
    have := congrArg List.length h; simp at this; omega)
  simp at h2
  exact ⟨h1, h2⟩

/-- **Incompatible ordered interfaces are refused**: `compose` is typable only
when the first program's output interface is exactly the second's input. -/
theorem compose_requires_exact_middle {s : List Ty} {i m m' o : List Ty} {out : List Ty}
    (hne : m ≠ m') : ¬ OpTyped (.word 9) (s ++ [.program i m, .program m' o]) out := by
  intro h
  generalize hin : s ++ [Ty.program i m, Ty.program m' o] = input at h
  cases h with
  | compose =>
      obtain ⟨-, h1, h2⟩ := snoc2_inj hin
      cases h1; cases h2; exact hne rfl

/-- **Wrong-subject evidence**: the increment claim holds for `[ 1 + ]` and is
false for the same-interface body `[ 2 + ]`. A claim is never transferable
by interface match. -/
theorem wrong_subject_evidence :
    exportedClaim [.lit (i64v 1), .word 4] [.i64] [.i64] [] (fun _ _ _ => True)
      (fun before after _ => ∃ n, before = [.i64 n] ∧ after = [.i64 (n + 1)]) ∧
    ¬ exportedClaim [.lit (i64v 2), .word 4] [.i64] [.i64] [] (fun _ _ _ => True)
      (fun before after _ => ∃ n, before = [.i64 n] ∧ after = [.i64 (n + 1)]) := by
  constructor
  · apply exportedClaim_unaryI64
    intro tail n final h
    simpa [Exec, List.reverse_append, i64v] using h
  · intro h
    obtain ⟨after, hfinal, -, n, hb, ha⟩ :=
      h [] [.i64 0] [] (.cons .i64 .nil) .nil trivial [.i64 (0 + BitVec.ofInt 64 2)]
        (by simp [Exec, i64v])
    simp at hb; subst hb
    simp at hfinal; subst hfinal
    simp at ha

/-- Programs are duplicable data: `dup` is typable and steps on any program
value, so an immutable program can be copied regardless of its interface. -/
theorem program_duplicable (p : List Op) (i o : List Ty) (s : List Ty) (r : Stack) :
    OpTyped (.word 0) (s ++ [.program i o]) (s ++ [.program i o, .program i o]) ∧
      Step (.word 0) (.program p :: r) (.program p :: .program p :: r) :=
  ⟨.dup, .dup⟩

end NobleFoundation
