import NobleM2.CheckSoundness

namespace NobleM2

/-- The `caseScheme` scheme's instantiation exposes its eliminator shape:
every branch variable is bound, the result stack is the branches' common
output, and the derived bound is the union of the two branch bounds
(B-CHECK-06, K-EFFECT-01/02). -/
theorem caseScheme_inv {inst : Inst} {stack out : TyList} {effects : EffSet}
    (h : caseScheme.instantiate inst = some (stack, out, effects)) :
    ∃ x0 x1 x2 x3 x4 x5,
       inst.stack 0 = some x0 ∧ inst.value 1 = some x1 ∧ inst.value 2 = some x2 ∧ inst.stack 3 = some x3 ∧ inst.effects 4 = some x4 ∧ inst.effects 5 = some x5 ∧
      out = x3 ∧ effects = x4.union x5 := by
   rcases h0 : inst.stack 0 with _ | x0 <;> rcases h1 : inst.value 1 with _ | x1 <;> rcases h2 : inst.value 2 with _ | x2 <;> rcases h3 : inst.stack 3 with _ | x3 <;> rcases h4 : inst.effects 4 with _ | x4 <;> rcases h5 : inst.effects 5 with _ | x5 <;>
    simp_all [caseScheme, Scheme.instantiate, Scheme.substStack, Scheme.substPattern,
      Scheme.substSignature, Scheme.substEffects, PartList.ofList, SlotList.ofList,
      append_nil, union_empty_right]

/-- The `ifScheme` scheme's instantiation exposes its eliminator shape:
every branch variable is bound, the result stack is the branches' common
output, and the derived bound is the union of the two branch bounds
(B-CHECK-06, K-EFFECT-01/02). -/
theorem ifScheme_inv {inst : Inst} {stack out : TyList} {effects : EffSet}
    (h : ifScheme.instantiate inst = some (stack, out, effects)) :
    ∃ x0 x1 x2 x3,
       inst.stack 0 = some x0 ∧ inst.stack 1 = some x1 ∧ inst.effects 2 = some x2 ∧ inst.effects 3 = some x3 ∧
      out = x1 ∧ effects = x2.union x3 := by
   rcases h0 : inst.stack 0 with _ | x0 <;> rcases h1 : inst.stack 1 with _ | x1 <;> rcases h2 : inst.effects 2 with _ | x2 <;> rcases h3 : inst.effects 3 with _ | x3 <;>
    simp_all [ifScheme, Scheme.instantiate, Scheme.substStack, Scheme.substPattern,
      Scheme.substSignature, Scheme.substEffects, PartList.ofList, SlotList.ofList,
      append_nil, union_empty_right]

/-- The `listCaseScheme` scheme's instantiation exposes its eliminator shape:
every branch variable is bound, the result stack is the branches' common
output, and the derived bound is the union of the two branch bounds
(B-CHECK-06, K-EFFECT-01/02). -/
theorem listCaseScheme_inv {inst : Inst} {stack out : TyList} {effects : EffSet}
    (h : listCaseScheme.instantiate inst = some (stack, out, effects)) :
    ∃ x0 x1 x2 x3 x4,
       inst.stack 0 = some x0 ∧ inst.value 1 = some x1 ∧ inst.stack 2 = some x2 ∧ inst.effects 3 = some x3 ∧ inst.effects 4 = some x4 ∧
      out = x2 ∧ effects = x3.union x4 := by
   rcases h0 : inst.stack 0 with _ | x0 <;> rcases h1 : inst.value 1 with _ | x1 <;> rcases h2 : inst.stack 2 with _ | x2 <;> rcases h3 : inst.effects 3 with _ | x3 <;> rcases h4 : inst.effects 4 with _ | x4 <;>
    simp_all [listCaseScheme, Scheme.instantiate, Scheme.substStack, Scheme.substPattern,
      Scheme.substSignature, Scheme.substEffects, PartList.ofList, SlotList.ofList,
      append_nil, union_empty_right]

end NobleM2
