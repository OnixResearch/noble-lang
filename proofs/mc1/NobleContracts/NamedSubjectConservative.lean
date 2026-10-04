import NobleContracts.NamedSubject

namespace NobleContracts.NamedV1

/-- For builtin-only code, each named-model run projects back to a frozen MC1
run.  The two proof motives track both Run and Step, so an unaccounted call
constructor cannot occur even inside run, dip, cases, or list branches. -/
private def RunBack (_env : Environment) (code : List Op) (before after : Stack) : Prop :=
  ∀ oldCode oldBefore, code = liftCode oldCode → before = liftStack oldBefore →
    ∃ oldAfter, after = liftStack oldAfter ∧
      NobleContracts.Run oldCode oldBefore oldAfter
private def StepBack (_env : Environment) (op : Op) (before after : Stack) : Prop :=
  ∀ oldOp oldBefore, op = liftOp oldOp → before = liftStack oldBefore →
    ∃ oldAfter, after = liftStack oldAfter ∧
      NobleContracts.Step oldOp oldBefore oldAfter

private theorem liftStack_cons {v : Value} {s : Stack}
    {xs : NobleContracts.Stack} (h : v :: s = liftStack xs) :
    ∃ oldV oldS, xs = oldV :: oldS ∧ v = liftValue oldV ∧ s = liftStack oldS := by
  cases xs with
  | nil => simp [liftStack] at h
  | cons x rest =>
    simp only [liftStack, List.map_cons, List.cons.injEq] at h
    exact ⟨x, rest, rfl, h.1, h.2⟩

private theorem liftValue_program {p : List Op} {v : NobleContracts.Value}
    (h : Value.program p = liftValue v) :
    ∃ oldP, v = .program oldP ∧ p = liftCode oldP := by
  cases v <;> simp [liftValue] at h
  rename_i oldP
  exact ⟨oldP, rfl, h⟩

private theorem reverse_run_aux {env : Environment} {code before after}
    (execution : Run env code before after) : RunBack env code before after := by
  apply Run.recOn
    (motive_1 := fun code before after _ => RunBack env code before after)
    (motive_2 := fun op before after _ => StepBack env op before after) execution
  all_goals dsimp only [RunBack, StepBack]
  all_goals intros
  case nil s oldCode oldBefore hcode hbefore =>
    cases oldCode with
    | nil => exact ⟨oldBefore, hbefore, .nil⟩
    | cons op rest => simp [liftCode] at hcode
  case cons op ops s m t hstep hrun ihstep ihrun oldCode oldBefore hcode hbefore =>
    cases oldCode with
    | nil => simp [liftCode] at hcode
    | cons oldOp oldRest =>
      simp only [liftCode, List.map_cons, List.cons.injEq] at hcode
      obtain ⟨hOp, hRest⟩ := hcode
      obtain ⟨oldMiddle, hMiddle, oldStep⟩ := ihstep oldOp oldBefore hOp hbefore
      obtain ⟨oldAfter, hAfter, oldRun⟩ := ihrun oldRest oldMiddle hRest hMiddle
      exact ⟨oldAfter, hAfter, .cons oldStep oldRun⟩
  case lit v s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    exact ⟨_ :: oldBefore, rfl, .lit⟩
  case block p s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    exact ⟨.program _ :: oldBefore, by simp [liftStack, liftValue], .block⟩
  case call use entry s t hlookup hmatch hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
  case dup v s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hBefore
    subst_vars
    exact ⟨oldV :: oldV :: oldS, rfl, .dup⟩
  case drop v s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hBefore
    exact ⟨oldS, hs, .drop⟩
  case swap a b s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldB, oldRest, rfl, hb, hrest⟩ := liftStack_cons hBefore
    obtain ⟨oldA, oldS, rfl, ha, hs⟩ := liftStack_cons hrest
    subst_vars
    exact ⟨oldA :: oldB :: oldS, by simp [liftStack], .swap⟩
  case dip p v s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldProgram, oldRest, rfl, hProgram, hrest⟩ := liftStack_cons hBefore
    obtain ⟨oldP, hOldProgram, hp⟩ := liftValue_program hProgram
    subst oldProgram
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hrest
    obtain ⟨oldT, ht, oldRun⟩ := ihrun oldP oldS hp hs
    exact ⟨oldV :: oldT, by simp [liftStack, hv, ht], .dip oldRun⟩
  case add a b s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldB, oldRest, rfl, hb, hrest⟩ := liftStack_cons hBefore
    cases oldB <;> simp [liftValue] at hb
    rename_i oldBnum
    obtain ⟨oldA, oldS, rfl, ha, hs⟩ := liftStack_cons hrest
    cases oldA <;> simp [liftValue] at ha
    rename_i oldAnum
    exact ⟨.i64 (oldAnum + oldBnum) :: oldS,
      by simp [liftStack, liftValue, hs, hb, ha], .add⟩
  case sub a b s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldB, oldRest, rfl, hb, hrest⟩ := liftStack_cons hBefore
    cases oldB <;> simp [liftValue] at hb
    rename_i oldBnum
    obtain ⟨oldA, oldS, rfl, ha, hs⟩ := liftStack_cons hrest
    cases oldA <;> simp [liftValue] at ha
    rename_i oldAnum
    exact ⟨.i64 (oldAnum - oldBnum) :: oldS,
      by simp [liftStack, liftValue, hs, hb, ha], .sub⟩
  case mul a b s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldB, oldRest, rfl, hb, hrest⟩ := liftStack_cons hBefore
    cases oldB <;> simp [liftValue] at hb
    rename_i oldBnum
    obtain ⟨oldA, oldS, rfl, ha, hs⟩ := liftStack_cons hrest
    cases oldA <;> simp [liftValue] at ha
    rename_i oldAnum
    exact ⟨.i64 (oldAnum * oldBnum) :: oldS,
      by simp [liftStack, liftValue, hs, hb, ha], .mul⟩
  case equals a b s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldB, oldRest, rfl, hb, hrest⟩ := liftStack_cons hBefore
    cases oldB <;> simp [liftValue] at hb
    rename_i oldBnum
    obtain ⟨oldA, oldS, rfl, ha, hs⟩ := liftStack_cons hrest
    cases oldA <;> simp [liftValue] at ha
    rename_i oldAnum
    exact ⟨.bool (oldAnum == oldBnum) :: oldS,
      by simp [liftStack, liftValue, hs, hb, ha], .equals⟩
  case quote v s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hBefore
    subst_vars
    exact ⟨.program [.lit oldV] :: oldS,
      by simp [liftStack, liftValue, liftOp], .quote⟩
  case compose p q s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldQProgram, oldRest, rfl, hQProgram, hrest⟩ := liftStack_cons hBefore
    obtain ⟨oldQ, hOldQProgram, hq⟩ := liftValue_program hQProgram
    subst oldQProgram
    obtain ⟨oldPProgram, oldS, rfl, hPProgram, hs⟩ := liftStack_cons hrest
    obtain ⟨oldP, hOldPProgram, hp⟩ := liftValue_program hPProgram
    subst oldPProgram
    exact ⟨.program (oldP ++ oldQ) :: oldS,
      by simp [liftStack, liftValue, liftCode, List.map_append, hp, hq, hs], .compose⟩
  case run p s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldProgram, oldS, rfl, hProgram, hs⟩ := liftStack_cons hBefore
    obtain ⟨oldP, hOldProgram, hp⟩ := liftValue_program hProgram
    subst oldProgram
    obtain ⟨oldT, ht, oldRun⟩ := ihrun oldP oldS hp hs
    exact ⟨oldT, ht, .run oldRun⟩
  case reflect p s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hBefore
    cases oldV <;> simp [liftValue] at hv
    rename_i oldP
    subst_vars
    exact ⟨.syntax oldP :: oldS, by simp [liftStack, liftValue], .reflect⟩
  case unit s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    exact ⟨.unit :: oldBefore, by simp [liftStack, liftValue], .unit⟩
  case pair a b s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldB, oldRest, rfl, hb, hrest⟩ := liftStack_cons hBefore
    obtain ⟨oldA, oldS, rfl, ha, hs⟩ := liftStack_cons hrest
    subst_vars
    exact ⟨.pair oldA oldB :: oldS, by simp [liftStack, liftValue], .pair⟩
  case unpair a b s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hBefore
    cases oldV <;> simp [liftValue] at hv
    rename_i oldA oldB
    obtain ⟨ha, hb⟩ := hv
    subst_vars
    exact ⟨oldB :: oldA :: oldS, by simp [liftStack], .unpair⟩
  case inl v s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hBefore
    subst_vars
    exact ⟨.inl oldV :: oldS, by simp [liftStack, liftValue], .inl⟩
  case inr v s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hBefore
    subst_vars
    exact ⟨.inr oldV :: oldS, by simp [liftStack, liftValue], .inr⟩
  case caseLeft p q v s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldQProgram, oldRest1, rfl, hQProgram, hrest1⟩ := liftStack_cons hBefore
    obtain ⟨oldQ, hOldQProgram, hq⟩ := liftValue_program hQProgram
    subst oldQProgram
    obtain ⟨oldPProgram, oldRest2, rfl, hPProgram, hrest2⟩ := liftStack_cons hrest1
    obtain ⟨oldP, hOldPProgram, hp⟩ := liftValue_program hPProgram
    subst oldPProgram
    obtain ⟨oldSum, oldS, rfl, hSum, hs⟩ := liftStack_cons hrest2
    cases oldSum <;> simp [liftValue] at hSum
    rename_i oldV
    subst_vars
    obtain ⟨oldT, ht, oldRun⟩ :=
      ihrun oldP (oldV :: oldS) rfl (by simp [liftStack])
    exact ⟨oldT, ht, .caseLeft oldRun⟩
  case caseRight p q v s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldQProgram, oldRest1, rfl, hQProgram, hrest1⟩ := liftStack_cons hBefore
    obtain ⟨oldQ, hOldQProgram, hq⟩ := liftValue_program hQProgram
    subst oldQProgram
    obtain ⟨oldPProgram, oldRest2, rfl, hPProgram, hrest2⟩ := liftStack_cons hrest1
    obtain ⟨oldP, hOldPProgram, hp⟩ := liftValue_program hPProgram
    subst oldPProgram
    obtain ⟨oldSum, oldS, rfl, hSum, hs⟩ := liftStack_cons hrest2
    cases oldSum <;> simp [liftValue] at hSum
    rename_i oldV
    subst_vars
    obtain ⟨oldT, ht, oldRun⟩ :=
      ihrun oldQ (oldV :: oldS) rfl (by simp [liftStack])
    exact ⟨oldT, ht, .caseRight oldRun⟩
  case ifTrue p q s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldQProgram, oldRest1, rfl, hQProgram, hrest1⟩ := liftStack_cons hBefore
    obtain ⟨oldQ, hOldQProgram, hq⟩ := liftValue_program hQProgram
    subst oldQProgram
    obtain ⟨oldPProgram, oldRest2, rfl, hPProgram, hrest2⟩ := liftStack_cons hrest1
    obtain ⟨oldP, hOldPProgram, hp⟩ := liftValue_program hPProgram
    subst oldPProgram
    obtain ⟨oldBool, oldS, rfl, hBool, hs⟩ := liftStack_cons hrest2
    cases oldBool <;> simp [liftValue] at hBool
    subst_vars
    obtain ⟨oldT, ht, oldRun⟩ := ihrun oldP oldS rfl rfl
    exact ⟨oldT, ht, .ifTrue oldRun⟩
  case ifFalse p q s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldQProgram, oldRest1, rfl, hQProgram, hrest1⟩ := liftStack_cons hBefore
    obtain ⟨oldQ, hOldQProgram, hq⟩ := liftValue_program hQProgram
    subst oldQProgram
    obtain ⟨oldPProgram, oldRest2, rfl, hPProgram, hrest2⟩ := liftStack_cons hrest1
    obtain ⟨oldP, hOldPProgram, hp⟩ := liftValue_program hPProgram
    subst oldPProgram
    obtain ⟨oldBool, oldS, rfl, hBool, hs⟩ := liftStack_cons hrest2
    cases oldBool <;> simp [liftValue] at hBool
    subst_vars
    obtain ⟨oldT, ht, oldRun⟩ := ihrun oldQ oldS rfl rfl
    exact ⟨oldT, ht, .ifFalse oldRun⟩
  case nilList s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    exact ⟨.list [] :: oldBefore, by simp [liftStack, liftValue], .nil⟩
  case consList v vs s oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldL, oldRest, rfl, hL, hrest⟩ := liftStack_cons hBefore
    cases oldL <;> simp [liftValue] at hL
    rename_i oldVs
    obtain ⟨oldV, oldS, rfl, hv, hs⟩ := liftStack_cons hrest
    subst_vars
    exact ⟨.list (oldV :: oldVs) :: oldS,
      by simp [liftStack, liftValue], .cons⟩
  case listNil p q s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldQProgram, oldRest1, rfl, hQProgram, hrest1⟩ := liftStack_cons hBefore
    obtain ⟨oldQ, hOldQProgram, hq⟩ := liftValue_program hQProgram
    subst oldQProgram
    obtain ⟨oldPProgram, oldRest2, rfl, hPProgram, hrest2⟩ := liftStack_cons hrest1
    obtain ⟨oldP, hOldPProgram, hp⟩ := liftValue_program hPProgram
    subst oldPProgram
    obtain ⟨oldL, oldS, rfl, hL, hs⟩ := liftStack_cons hrest2
    cases oldL <;> simp [liftValue] at hL
    rename_i oldVs
    cases oldVs with
    | nil =>
      subst_vars
      obtain ⟨oldT, ht, oldRun⟩ := ihrun oldP oldS rfl rfl
      exact ⟨oldT, ht, .listNil oldRun⟩
    | cons oldV oldVs => simp at hL
  case listCons p q v vs s t hrun ihrun oldOp oldBefore hOp hBefore =>
    cases oldOp <;> simp [liftOp] at hOp
    subst_vars
    obtain ⟨oldQProgram, oldRest1, rfl, hQProgram, hrest1⟩ := liftStack_cons hBefore
    obtain ⟨oldQ, hOldQProgram, hq⟩ := liftValue_program hQProgram
    subst oldQProgram
    obtain ⟨oldPProgram, oldRest2, rfl, hPProgram, hrest2⟩ := liftStack_cons hrest1
    obtain ⟨oldP, hOldPProgram, hp⟩ := liftValue_program hPProgram
    subst oldPProgram
    obtain ⟨oldL, oldS, rfl, hL, hs⟩ := liftStack_cons hrest2
    cases oldL <;> simp [liftValue] at hL
    rename_i oldVs
    cases oldVs with
    | nil => simp at hL
    | cons oldV oldVs =>
      simp only [List.map_cons, List.cons.injEq] at hL
      obtain ⟨hv, hvs⟩ := hL
      subst_vars
      obtain ⟨oldT, ht, oldRun⟩ :=
        ihrun oldQ (.list oldVs :: oldV :: oldS) rfl
          (by simp [liftStack, liftValue])
      exact ⟨oldT, ht, .listCons oldRun⟩

/-- Conservative equivalence on *all* builtin-only programs: the full set of
normal-return output stacks in NamedV1 is exactly the embedding of the frozen
MC1-v1 outcomes, for any named environment.  No named definition can affect
an execution that starts with builtin-only code and embedded input values. -/
theorem builtin_runs_iff {env : Environment} {code : List NobleContracts.Op}
    {before : NobleContracts.Stack} {result : Stack} :
    Run env (liftCode code) (liftStack before) result ↔
      ∃ after, result = liftStack after ∧
        NobleContracts.Run code before after := by
  constructor
  · intro execution
    exact reverse_run_aux execution code before rfl rfl
  · rintro ⟨after, rfl, execution⟩
    exact lift_run execution

/-- Historical MC1-v1 builtin typing derivations also remain valid in any
NamedV1 environment; this includes quoted programs and typed literal lists. -/
theorem lift_typed {env : Environment} {code : List NobleContracts.Op}
    {input output : List NobleContracts.Ty}
    (typed : NobleContracts.CodeTyped code input output) :
    CodeTyped env (liftCode code) input output := by
  apply NobleContracts.CodeTyped.recOn
    (motive_1 := fun value ty _ => HasType env (liftValue value) ty)
    (motive_2 := fun body i o _ => CodeTyped env (liftCode body) i o)
    (motive_3 := fun op i o _ => OpTyped env (liftOp op) i o) typed
  all_goals intros
  all_goals simp only [liftValue, liftOp, liftCode, List.map_cons, List.map_nil] at *
  case list vs ty original ih =>
    apply HasType.list
    intro v hv
    obtain ⟨oldV, hmem, rfl⟩ := List.mem_map.mp hv
    exact ih oldV hmem
  all_goals first
    | exact .unit
    | exact .bool
    | exact .i64
    | exact .text
    | exact .syntax
    | exact .pair (by assumption) (by assumption)
    | exact .inl (by assumption)
    | exact .inr (by assumption)
    | exact .program (by assumption)
    | exact .nil
    | exact .cons (by assumption) (by assumption)
    | exact .lit (by assumption)
    | exact .block (by assumption)
    | exact .dup
    | exact .drop
    | exact .swap
    | exact .dip
    | exact .add
    | exact .sub
    | exact .mul
    | exact .equals
    | exact .quote
    | exact .compose
    | exact .run
    | exact .reflect
    | exact .pair
    | exact .unpair
    | exact .inl
    | exact .inr
    | exact .caseWord
    | exact .ifWord
    | exact .nilList
    | exact .consList
    | exact .listCase

end NobleContracts.NamedV1
