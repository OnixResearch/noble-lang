import NobleM2.SizeChangeMatrix

/-!
# Enumerating matrix space (issue #101)

A composite search needs a finite enumeration of matrices, as the vector lane's bound needed one for
`Fin n → Rel`. The construction is generalised over the codomain: `funAux k codomain` lists every
`Fin k → α` built from a complete enumeration of `α`, and applying it twice enumerates
`MGraph n = Fin n → Fin n → Rel4`.

Completeness is the point: an enumeration is a proof that the search space is exhausted
(`matrixAux_complete`), and `matrixAux_length` counts it: `(4 ^ n) ^ n`.
-/

namespace NobleM2.SizeChange

/-- Extracting: membership in an appending fold names an entry. -/
theorem mem_foldr_append {α β : Type} (g : α → List β) (l : List α) {b : β}
    (hb : b ∈ l.foldr (fun a acc => g a ++ acc) []) : ∃ a ∈ l, b ∈ g a := by
  induction l with
  | nil => simp at hb
  | cons a rest ih =>
      rw [List.foldr_cons] at hb
      rcases List.mem_append.mp hb with ha | hrest
      · exact ⟨a, by simp, ha⟩
      · obtain ⟨c, hc, hbc⟩ := ih hrest
        exact ⟨c, by simp [hc], hbc⟩

/-- Constructing: an entry's image is in the fold. -/
theorem mem_of_mem_foldr_append {α β : Type} (g : α → List β) :
    ∀ (l : List α) (a : α) (b : β), a ∈ l → b ∈ g a →
      b ∈ l.foldr (fun a acc => g a ++ acc) []
  | [], a, b, ha, _ => by simp at ha
  | c :: rest, a, b, ha, hb => by
      rw [List.foldr_cons]
      rcases List.mem_cons.mp ha with hca | har
      · have hca' : c = a := hca.symm
        rw [hca']
        exact List.mem_append.mpr (Or.inl hb)
      · exact List.mem_append.mpr (Or.inr (mem_of_mem_foldr_append g rest a b har hb))

/-- All functions `Fin k → α` for a given enumeration of `α`. -/
def funAux {α : Type} : (k : Nat) → (codomain : List α) → List (Fin k → α)
  | 0, _ => [fun i => Fin.elim0 i]
  | k + 1, codomain =>
      codomain.foldr
        (fun a acc =>
          (funAux k codomain).map (fun f : Fin k → α => (Fin.cases a f : Fin (k + 1) → α)) ++ acc)
        ([] : List (Fin (k + 1) → α))

/-- Extending an enumerated function by one argument stays in the next enumeration. -/
theorem funAux_extend {α : Type} {codomain : List α} (k : Nat) {a : α} (ha : a ∈ codomain)
    {f : Fin k → α} (hf : f ∈ funAux k codomain) :
    Fin.cases a f ∈ funAux (k + 1) codomain := by
  simp only [funAux]
  exact mem_of_mem_foldr_append
    (fun a => (funAux k codomain).map (fun g : Fin k → α => (Fin.cases a g : Fin (k + 1) → α)))
    codomain a (Fin.cases a f) ha (List.mem_map.mpr ⟨f, hf, rfl⟩)

/-- Every function appears, provided the codomain enumeration is complete. -/
theorem funAux_complete {α : Type} {codomain : List α} (hall : ∀ a, a ∈ codomain) :
    ∀ (k : Nat) (f : Fin k → α), f ∈ funAux k codomain := by
  intro k
  induction k with
  | zero =>
      intro f
      have h : f = fun i => Fin.elim0 i := by funext i; exact Fin.elim0 i
      rw [h]
      simp [funAux]
  | succ k ih =>
      intro f
      have hdecomp : f = Fin.cases (f 0) (fun i => f i.succ) := by
        funext i
        cases i using Fin.cases <;> simp
      rw [hdecomp]
      exact funAux_extend k (hall (f 0)) (ih _)

/-- The four relations. -/
def Rel4.all : List Rel4 := [.strict, .nonIncrease, .same, .unknown]

/-- Every relation appears. -/
theorem Rel4.mem_all (r : Rel4) : r ∈ Rel4.all := by
  cases r <;> simp [Rel4.all]

/-- All matrix call graphs on `n` measures. -/
def matrixAux (n : Nat) : List (MGraph n) := funAux n (funAux n Rel4.all)

/-- **Completeness**: every matrix appears in the enumeration. -/
theorem matrixAux_complete {n : Nat} (g : MGraph n) : g ∈ matrixAux n :=
  funAux_complete (fun f => funAux_complete Rel4.mem_all n f) n g

/-- A fold whose entries all have the same length multiplies that length. -/
theorem foldr_length_const {α β : Type} (g : α → List β) (m : Nat) (hm : ∀ a, (g a).length = m) :
    ∀ (l : List α), (l.foldr (fun a acc => g a ++ acc) []).length = l.length * m
  | [] => by simp
  | c :: rest => by
      rw [List.foldr_cons, List.length_append, hm c, foldr_length_const g m hm rest,
        List.length_cons, Nat.succ_mul]
      omega

/-- The enumeration has `codomain.length ^ k` elements. -/
theorem funAux_length {α : Type} (codomain : List α) :
    ∀ k, (funAux k codomain).length = codomain.length ^ k
  | 0 => rfl
  | k + 1 => by
      simp only [funAux]
      rw [foldr_length_const
            (fun a => (funAux k codomain).map (fun g : Fin k → α => (Fin.cases a g : Fin (k + 1) → α)))
            ((funAux k codomain).length) (fun a => by simp) codomain,
        funAux_length codomain k, Nat.pow_succ]
      exact Nat.mul_comm _ _

/-- The relation enumeration has four entries. -/
theorem Rel4.all_length : Rel4.all.length = 4 := rfl

/-- The matrix enumeration has `(4 ^ n) ^ n` elements. -/
theorem matrixAux_length (n : Nat) : (matrixAux n).length = (4 ^ n) ^ n := by
  simp only [matrixAux, funAux_length, Rel4.all_length]

end NobleM2.SizeChange
