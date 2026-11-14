import Mathlib.Data.Fintype.Pi
import Mathlib.Data.Fintype.Pigeonhole
import Mathlib.Order.Interval.Finset.Basic
import Mathlib.Order.OrderIsoNat
import NobleM2.SizeChangeSCT

/-!
# Classical arbitrary-run soundness for the size-change criterion (issue #101)

This foundation lane imports the exact finite matrix/certificate/run model from the strict m3
family. Classical choice is used only for the infinite Ramsey argument; the m3 model remains in its
choice-free axiom lane.
-/

namespace NobleFoundation.SizeChangeSound

open NobleM2.SizeChange

/-- The four size-change relations form a finite type, so matrices over a fixed measure count do too;
the Ramsey argument colours run segments by their composed matrix. -/
instance instFintypeRel4 : Fintype Rel4 :=
  ⟨{Rel4.strict, Rel4.nonIncrease, Rel4.same, Rel4.unknown}, by
    intro relation
    cases relation <;> simp⟩

private structure PairSplit (α : Type) (color : Nat → Nat → α) (s : Set Nat) where
  vertex : Nat
  vertex_mem : vertex ∈ s
  shade : α
  future : Set Nat
  future_infinite : future.Infinite
  future_subset : future ⊆ s
  future_after : ∀ {x}, x ∈ future → vertex < x
  future_color : ∀ {x}, x ∈ future → color vertex x = shade

private noncomputable def splitPairColor {α : Type} [Fintype α]
    (color : Nat → Nat → α) (s : Set Nat) (hs : s.Infinite) : PairSplit α color s := by
  classical
  -- `PairSplit` is data, so the witnesses come from `Classical.choose`, not `Exists` elimination.
  let a := Classical.choose (hs.exists_gt 0)
  have ha : a ∈ s := (Classical.choose_spec (hs.exists_gt 0)).1
  let future : Set Nat := {b | b ∈ s ∧ a < b}
  have hfuture : future.Infinite := by
    apply Set.infinite_iff_exists_gt.mpr
    intro m
    obtain ⟨b, hb, hmb⟩ := hs.exists_gt (max a m)
    refine ⟨b, ⟨hb, ?_⟩, ?_⟩
    · exact lt_of_le_of_lt (Nat.le_max_left _ _) hmb
    · exact lt_of_le_of_lt (Nat.le_max_right _ _) hmb
  let f : {b : Nat // b ∈ future} → α := fun b => color a b
  letI : Infinite {b : Nat // b ∈ future} := Set.infinite_coe_iff.mpr hfuture
  let c := Classical.choose (Finite.exists_infinite_fiber f)
  have hc : Infinite (f ⁻¹' {c}) := Classical.choose_spec (Finite.exists_infinite_fiber f)
  letI : Infinite (f ⁻¹' {c}) := hc
  let future' : Set Nat := {b | b ∈ future ∧ color a b = c}
  have hfuture' : future'.Infinite := by
    apply Set.infinite_coe_iff.mp
    apply Infinite.of_injective
      (fun b : f ⁻¹' {c} => ⟨b.1.1, ⟨b.1.2, show color a b.1.1 = c from Set.mem_singleton_iff.mp b.2⟩⟩)
    intro x y h
    apply Subtype.ext
    apply Subtype.ext
    simpa using congrArg Subtype.val h
  refine ⟨a, ha, c, future', hfuture', ?_, ?_, ?_⟩
  · intro b hb
    exact hb.1.1
  · intro b hb
    exact hb.1.2
  · intro b hb
    exact hb.2

private noncomputable def pairStages {α : Type} [Fintype α]
    (color : Nat → Nat → α) : Nat → {s : Set Nat // s.Infinite}
  | 0 => ⟨Set.univ, Set.infinite_univ⟩
  | k + 1 =>
      let previous := pairStages color k
      let next := splitPairColor color previous.1 previous.2
      ⟨next.future, next.future_infinite⟩

private noncomputable def pairStageSplit {α : Type} [Fintype α]
    (color : Nat → Nat → α) (k : Nat) : PairSplit α color (pairStages color k).1 :=
  splitPairColor color (pairStages color k).1 (pairStages color k).2

/-- Every finite coloring of increasing pairs of naturals has an infinite homogeneous sequence. -/
theorem infinite_pair_homogeneous {α : Type} [Fintype α] (color : Nat → Nat → α) :
    ∃ indices : Nat → Nat, StrictMono indices ∧
      ∃ shade, ∀ i j, i < j → color (indices i) (indices j) = shade := by
  classical
  let vertex := fun k => (pairStageSplit color k).vertex
  let shade := fun k => (pairStageSplit color k).shade
  have hvertex_mem (k : Nat) : vertex k ∈ (pairStages color k).1 :=
    (pairStageSplit color k).vertex_mem
  have hstage_succ (k : Nat) : (pairStages color (k + 1)).1 =
      (pairStageSplit color k).future := rfl
  have hstage_subset (k : Nat) : (pairStages color (k + 1)).1 ⊆ (pairStages color k).1 := by
    rw [hstage_succ]
    exact (pairStageSplit color k).future_subset
  have hvertex_succ (k : Nat) : vertex k < vertex (k + 1) := by
    apply (pairStageSplit color k).future_after
    exact hvertex_mem (k + 1)
  have hstrict : StrictMono vertex := strictMono_nat_of_lt_succ hvertex_succ
  have hnested : ∀ i j, i < j → (pairStages color j).1 ⊆ (pairStages color (i + 1)).1 := by
    intro i j hij
    induction j with
    | zero => omega
    | succ j ih =>
        by_cases hEq : i = j
        · subst j
          exact fun _ hx => hx
        · have hij' : i < j := by omega
          exact fun x hx => ih hij' (hstage_subset j hx)
  have hshade (i j : Nat) (hij : i < j) : color (vertex i) (vertex j) = shade i := by
    have hmem : vertex j ∈ (pairStages color (i + 1)).1 :=
      hnested i j hij (hvertex_mem j)
    rw [hstage_succ] at hmem
    exact (pairStageSplit color i).future_color hmem
  let shades : Nat → α := shade
  obtain ⟨common, hcommon⟩ := Finite.exists_infinite_fiber shades
  let sameShade : Set Nat := shades ⁻¹' {common}
  have hsameShade : sameShade.Infinite := Set.infinite_coe_iff.mp hcommon
  let subsequence : Nat ↪o Nat := Nat.orderEmbeddingOfSet sameShade
  have hsub_mem (i : Nat) : subsequence i ∈ sameShade := by
    have hrange := Nat.orderEmbeddingOfSet_range sameShade
    rw [← hrange]
    exact ⟨i, rfl⟩
  refine ⟨vertex ∘ subsequence, hstrict.comp subsequence.strictMono, common, ?_⟩
  intro i j hij
  have hs : subsequence i < subsequence j := subsequence.strictMono hij
  calc
    color (vertex (subsequence i)) (vertex (subsequence j)) = shades (subsequence i) :=
      hshade _ _ hs
    _ = common := by simpa using Set.mem_preimage.mp (hsub_mem i)

/-- A checked closure certificate rules out every satisfying infinite site run. -/
theorem matrix_closure_sct_terminates_classical {k n : Nat}
    {sites : Fin k → MGraph n} (certificate : MatrixClosureCertificate k n sites)
    (run : MatrixSiteRun sites) : False := by
  classical
  let color : Nat → Nat → MGraph n :=
    fun i j => pathMatrix (j - i) run.step i
  obtain ⟨indices, hindices, graph, hhomogeneous⟩ := infinite_pair_homogeneous color
  have hmatrix_segment (i j : Nat) (hij : i < j) :
      pathMatrix (indices j - indices i) run.step (indices i) = graph :=
    hhomogeneous i j hij
  have path_add : ∀ a b t, pathMatrix (a + b) run.step t =
      composeMatrix (pathMatrix a run.step t) (pathMatrix b run.step (t + a)) := by
    intro a
    induction a with
    | zero =>
        intro b t
        rw [Nat.zero_add, Nat.add_zero]
        exact (composeMatrix_id_left _).symm
    | succ a ih =>
        intro b t
        have hlen : Nat.succ a + b = Nat.succ (a + b) := by omega
        rw [hlen]
        change composeMatrix (run.step t) (pathMatrix (a + b) run.step (t + 1)) =
          composeMatrix (composeMatrix (run.step t) (pathMatrix a run.step (t + 1)))
            (pathMatrix b run.step (t + Nat.succ a))
        rw [ih b (t + 1)]
        have hindex : t + Nat.succ a = t + 1 + a := by omega
        rw [hindex]
        exact (composeMatrix_assoc _ _ _).symm
  have hstart_lt : indices 0 < indices 1 := hindices (by omega)
  have hprefix_nonempty :
      stepList run.step (indices 1 - indices 0) (indices 0) ≠ [] := by
    cases hlength : indices 1 - indices 0 with
    | zero => omega
    | succ length => simp [stepList]
  have hprefix_sites : ∀ matrix,
      matrix ∈ stepList run.step (indices 1 - indices 0) (indices 0) →
        ∃ site, matrix = sites site := by
    intro matrix hmember
    exact stepList_sites sites run.site (indices 1 - indices 0) (indices 0) matrix
      (by exact hmember)
  have hprefix_closure := matrixProduct_sites_mem_closure sites
    (stepList run.step (indices 1 - indices 0) (indices 0)) hprefix_nonempty hprefix_sites
  have hprefix_product :
      matrixProduct (stepList run.step (indices 1 - indices 0) (indices 0)) = graph := by
    calc
      _ = pathMatrix (indices 1 - indices 0) run.step (indices 0) :=
        matrixProduct_stepList run.step (indices 1 - indices 0) (indices 0)
      _ = graph := hmatrix_segment 0 1 (by omega)
  rw [hprefix_product] at hprefix_closure
  have hfirst_middle : indices 0 < indices 1 := hindices (by omega)
  have hmiddle_last : indices 1 < indices 2 := hindices (by omega)
  have hsum : (indices 1 - indices 0) + (indices 2 - indices 1) =
      indices 2 - indices 0 := by omega
  have hshift : indices 0 + (indices 1 - indices 0) = indices 1 := by omega
  have hadd : pathMatrix (indices 2 - indices 0) run.step (indices 0) =
      composeMatrix (pathMatrix (indices 1 - indices 0) run.step (indices 0))
        (pathMatrix (indices 2 - indices 1) run.step (indices 1)) := by
    simpa [hsum, hshift] using
      path_add (indices 1 - indices 0) (indices 2 - indices 1) (indices 0)
  have hidempotent : composeMatrix graph graph = graph := by
    calc
      composeMatrix graph graph =
          composeMatrix (pathMatrix (indices 1 - indices 0) run.step (indices 0))
            (pathMatrix (indices 2 - indices 1) run.step (indices 1)) := by
        rw [hmatrix_segment 0 1 (by omega), hmatrix_segment 1 2 (by omega)]
      _ = pathMatrix (indices 2 - indices 0) run.step (indices 0) := hadd.symm
      _ = graph := hmatrix_segment 0 2 (by omega)
  obtain ⟨measure, hstrict⟩ :=
    certificate_closure_criterion certificate hprefix_closure hidempotent
  have hsatisfies : ∀ t, MatrixSatisfies (run.step t) (run.state t) (run.state (t + 1)) := by
    intro t
    simpa [MatrixSiteRun.step] using run.satisfies t
  have hdecrease : ∀ i,
      run.state (indices (i + 1)) measure < run.state (indices i) measure := by
    intro i
    have hsegment := hmatrix_segment i (i + 1) (by omega)
    have hblock := matrixSatisfies_path run.step run.state hsatisfies
      (indices (i + 1) - indices i) (indices i)
    have hlt : indices i < indices (i + 1) := hindices (Nat.lt_succ_self i)
    have hstate : indices i + (indices (i + 1) - indices i) = indices (i + 1) := by omega
    rw [hstate, hsegment] at hblock
    exact MatrixSatisfies.strict hblock hstrict
  let selectedMeasure : Nat → Nat := fun i => run.state (indices i) measure
  have hbound : ∀ i, selectedMeasure i + i ≤ selectedMeasure 0 := by
    intro i
    induction i with
    | zero => simp [selectedMeasure]
    | succ i ih =>
        have hstep := hdecrease i
        dsimp [selectedMeasure] at ih hstep ⊢
        omega
  have hlarge := hbound (selectedMeasure 0 + 1)
  omega

end NobleFoundation.SizeChangeSound
