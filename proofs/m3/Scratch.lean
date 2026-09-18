import NobleM2.CheckSoundness
open NobleM2
theorem caseScheme_inv {inst : Inst} {stack out : TyList} {effects : EffSet}
    (h : caseScheme.instantiate inst = some (stack, out, effects)) :
    ∃ x0 x1 x2 x3 x4 x5,
      inst.stack 0 = some x5 ∧ inst.value 1 = some x4 ∧ inst.value 2 = some x3 ∧
      inst.stack 3 = some x2 ∧ inst.effects 4 = some x1 ∧ inst.effects 5 = some x0 ∧
      out = x2 ∧ effects = x1.union x0 := by
  rcases h5 : inst.effects 5 with _ | x0 <;>
    rcases h4 : inst.effects 4 with _ | x1 <;>
    rcases h3 : inst.stack 3 with _ | x2 <;>
    rcases h2 : inst.value 2 with _ | x3 <;>
    rcases h1 : inst.value 1 with _ | x4 <;>
    rcases h0 : inst.stack 0 with _ | x5 <;>
    simp_all [caseScheme, Scheme.instantiate, Scheme.substStack, Scheme.substPattern,
      Scheme.substSignature, Scheme.substEffects, PartList.ofList, SlotList.ofList,
      append_nil, union_empty_right]
  exact ⟨x0, x1, x2, x3, x4, x5, h0, h1, h2, h3, h4, h5, rfl, rfl⟩
