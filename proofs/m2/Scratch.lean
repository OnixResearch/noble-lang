import NobleKernel
open noble_kernel Aeneas Aeneas.Std

theorem sizeOf_programType_lt (i o : alloc.vec.Vec types.Ty)
    (e : types.EffSet) :
    sizeOf i.slice.list < sizeOf (types.Ty.ProgramType i o e) := by
  have h3 : sizeOf (types.Ty.ProgramType i o e) = 1 + sizeOf i + sizeOf o + sizeOf e := by
    rw [types.Ty.ProgramType.sizeOf_spec]
  obtain ⟨⟨leng, list, bound⟩⟩ := i
  have hN : sizeOf leng = leng := rfl
  have hv := alloc.vec.Vec.mk.sizeOf_spec (Slice.mk (α := types.Ty) leng list bound)
  have hs := Slice.mk.sizeOf_spec (α := types.Ty) leng list bound
  simp only [alloc.vec.Vec.slice, hv, hs, hN] at h3 ⊢
  omega

mutual
  def gsize : types.Ty → Nat
    | .UnitType => 1
    | .PairType a b => 1 + gsize a + gsize b
    | .ProgramType i o _ => 1 + glistSizeN i.slice.leng i.slice.list + glistSizeN o.slice.leng o.slice.list
    | _ => 1
    termination_by t => sizeOf t
    decreasing_by
      all_goals
        first
          | exact decreasing_tactic
          | exact sizeOf_programType_lt _ _ _
          | omega
  def glistSizeN (n : Nat) (l : Aeneas.Data.ListN.ListN types.Ty n) : Nat :=
    match l with
    | .nil => 1
    | .cons t ts => 1 + gsize t + glistSizeN _ ts
    termination_by sizeOf l
end
