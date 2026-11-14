import NobleFoundation.NetI64

/-!
# Inductive revision-3 I64 family traces (issue #37)

These proofs establish per-family abstract reductions; they do not establish a
Rust lowering or reducer correspondence.
-/

namespace NobleFoundation.NetI64Families

open NobleFoundation.NetI64 INet

def fanoutTraceSlots (first : Nat) : Nat → List Nat
  | 0 => []
  | count + 1 => first :: fanoutTraceSlots (first + 1) count

def fanoutTracePlan (source level : Nat) (parent : Port) : List Nat → List (Port × Port)
  | [] => []
  | [slot] => [(parent, outputPort slot)]
  | slot :: next :: rest =>
      let duplicate := dupId source level
      (parent, (duplicate, 0)) ::
        ((duplicate, 1), outputPort slot) ::
          fanoutTracePlan source (level + 1) (duplicate, 2) (next :: rest)

def fanoutTraceCopyId (level : Nat) : Id := 0 :: dupId 0 level

def fanoutTracePendingId (level : Nat) : Id := 2 :: dupId 0 level

def fanoutTraceNet (outputs fired : Nat) (input : BitVec 64) : XNet NetSym :=
  let duplicates := (List.range (outputs - 1 - fired)).map fun offset =>
    (dupId 0 (fired + offset), NetSym.dup)
  let copies := (List.range fired).map fun level =>
    (fanoutTraceCopyId level, NetSym.num input)
  let agents := if fired = 0 then
      (valueId 0, NetSym.num input) :: duplicates
    else duplicates ++ copies ++ [(fanoutTracePendingId (fired - 1), NetSym.num input)]
  let parent := if fired = 0 then (valueId 0, 0) else (fanoutTracePendingId (fired - 1), 0)
  let outputsWires := (List.range fired).map fun level =>
    ((fanoutTraceCopyId level, 0), outputPort level)
  XNet.ofWires agents ((List.range outputs).map outputPort)
    (outputsWires ++ fanoutTracePlan 0 fired parent (fanoutTraceSlots fired (outputs - fired)))

/-- The remaining consumer slots are consecutive, in the original output order. -/
theorem fanoutTraceSlots_length (first count : Nat) :
    (fanoutTraceSlots first count).length = count := by
  induction count generalizing first with
  | zero => rfl
  | succ count ih => simp [fanoutTraceSlots, ih]

/-- The fanout continuation has one wire per final copy and two per duplicate. -/
theorem fanoutTracePlan_length (source level : Nat) (parent : Port) (slots : List Nat) :
    (fanoutTracePlan source level parent slots).length =
      if slots.isEmpty then 0 else 2 * slots.length - 1 := by
  induction slots generalizing level parent with
  | nil => rfl
  | cons first rest ih =>
    cases rest with
    | nil => rfl
    | cons second more =>
      simp only [fanoutTracePlan, List.length_cons]
      have htail := ih (level + 1) (dupId source level, 2)
      simp at htail ⊢
      omega

/-- The depth-zero balanced family is the one-input identity net. -/
theorem balancedProgram_zero_translation_checked (input : BitVec 64) :
    (balancedProgram 0).translateChecked [input] =
      some ((balancedProgram 0).translate [input]) := by
  let source := (valueId 0, 0)
  let boundary := outputPort 0
  let X : XNet NetSym := {
    agents := [(valueId 0, .num input)]
    free := [boundary]
    wires := [(source, boundary), (boundary, source)]
  }
  have htranslate : (balancedProgram 0).translate [input] = X := by
    rfl
  have hcheck : X.check arity = true := by
    simp [X, source, boundary, XNet.check, XNet.ends, XNet.ids,
      assoc, endsWith, arity, valueId, outputPort,
      List.range, List.range.loop]
  have hinput : (balancedProgram 0).InputsOk [input] := rfl
  have hwfCheck : (balancedProgram 0).wellFormedCheck = true := by decide
  have hwf := (Program.wellFormedCheck_iff (balancedProgram 0)).1 hwfCheck
  have htypedCheck : (balancedProgram 0).wellTypedCheck = true := by decide
  have htyped := (Program.wellTypedCheck_iff (balancedProgram 0)).1 htypedCheck
  have hadmission : (balancedProgram 0).acceptanceCheck [input] = true :=
    (Program.acceptanceCheck_iff (balancedProgram 0) [input]).2
      ⟨hinput, hwf, htyped⟩
  simp [Program.translateChecked, hadmission, htranslate, hcheck]

/-- The depth-zero tree has the empty abstract trace and reads back its input. -/
theorem balancedProgram_zero_trace (input : BitVec 64) :
    ∃ normal : XNet NetSym,
      (balancedProgram 0).translateChecked [input] =
        some ((balancedProgram 0).translate [input]) ∧
      (balancedProgram 0).normalize [input] = some (normal, 0) ∧
      Steps (CStep system) 0 ((balancedProgram 0).translate [input]).toNet
        normal.toNet ∧
      Normal (CStep system) normal.toNet ∧
      (balancedProgram 0).readBack normal =
        (balancedProgram 0).referenceOutputs [input] := by
  let source := (valueId 0, 0)
  let boundary := outputPort 0
  let X : XNet NetSym := {
    agents := [(valueId 0, .num input)]
    free := [boundary]
    wires := [(source, boundary), (boundary, source)]
  }
  have htranslate : (balancedProgram 0).translate [input] = X := rfl
  have hcheck : X.check arity = true := by
    simp [X, source, boundary, XNet.check, XNet.ends, XNet.ids,
      assoc, endsWith, arity, valueId, outputPort,
      List.range, List.range.loop]
  have hinput : (balancedProgram 0).InputsOk [input] := rfl
  have hwfCheck : (balancedProgram 0).wellFormedCheck = true := by decide
  have hwf := (Program.wellFormedCheck_iff (balancedProgram 0)).1 hwfCheck
  have htypedCheck : (balancedProgram 0).wellTypedCheck = true := by decide
  have htyped := (Program.wellTypedCheck_iff (balancedProgram 0)).1 htypedCheck
  have hadmission : (balancedProgram 0).acceptanceCheck [input] = true :=
    (Program.acceptanceCheck_iff (balancedProgram 0) [input]).2
      ⟨hinput, hwf, htyped⟩
  have hstep : X.step xsys = none := by
    simp [XNet.step, XNet.findRedex, XNet.redexAt, X, source, boundary,
      assoc, valueId, outputPort]
  have hcheck' : X.check xsys.sys.ar = true := by
    change X.check arity = true
    exact hcheck
  have hrun : X.normalize xsys 0 = some (X, 0) := by
    simp [XNet.normalize, XNet.run, hcheck', hstep]
  have hspec := XNet.normalize_spec hrun
  have hcount : (balancedProgram 0).interactionCount = 0 := by
    rw [balancedProgram_interactionCount]
  have hnormalize : (balancedProgram 0).normalize [input] = some (X, 0) := by
    unfold Program.normalize
    rw [balancedProgram_zero_translation_checked input, htranslate, hcount]
    exact hrun
  refine ⟨X, balancedProgram_zero_translation_checked input, hnormalize, ?_, ?_, ?_⟩
  · simpa [xsys, htranslate] using hspec.2.2.1
  · simpa [xsys] using hspec.2.2.2.1
  · rfl

/-- A chain with no operations is the same normal identity net as one tree leaf. -/
theorem chainProgram_zero_trace (input : BitVec 64) :
    ∃ normal : XNet NetSym,
      (chainProgram 0).translateChecked [input] =
        some ((chainProgram 0).translate [input]) ∧
      (chainProgram 0).normalize [input] = some (normal, 3 * 0) ∧
      Steps (CStep system) (3 * 0) ((chainProgram 0).translate [input]).toNet
        normal.toNet ∧
      Normal (CStep system) normal.toNet ∧
      (chainProgram 0).readBack normal =
        (chainProgram 0).referenceOutputs [input] := by
  simpa [chainProgram, chainNodes, balancedProgram, balancedSubtree] using
    balancedProgram_zero_trace input

/-- One requested fanout copy is already the input net, so no interaction is needed. -/
theorem fanoutProgram_one_trace (input : BitVec 64) :
    ∃ normal : XNet NetSym,
      (fanoutProgram 1).translateChecked [input] =
        some ((fanoutProgram 1).translate [input]) ∧
      (fanoutProgram 1).normalize [input] = some (normal, 0) ∧
      Steps (CStep system) 0 ((fanoutProgram 1).translate [input]).toNet
        normal.toNet ∧
      Normal (CStep system) normal.toNet ∧
      (fanoutProgram 1).readBack normal =
        some (List.replicate 1 (.i64 input)) := by
  obtain ⟨normal, hchecked, hnormalize, hsteps, hnormal, hread⟩ :=
    balancedProgram_zero_trace input
  refine ⟨normal, ?_, ?_, ?_, hnormal, ?_⟩
  · simpa [balancedProgram, balancedSubtree, fanoutProgram] using hchecked
  · simpa [balancedProgram, balancedSubtree, fanoutProgram] using hnormalize
  · simpa [balancedProgram, balancedSubtree, fanoutProgram] using hsteps
  · simpa [balancedProgram, balancedSubtree, fanoutProgram,
      Program.referenceOutputs, Program.referenceValue, Program.graphFor,
      Program.nodeAt, Program.inputValue, PackedSchedule.value,
      PackedSchedule.valuesUpTo, PackedSchedule.evalStep] using hread

/-- With no requested copies, one ERA interaction removes the input value. -/
theorem fanoutProgram_zero_trace (input : BitVec 64) :
    ∃ normal : XNet NetSym,
      (fanoutProgram 0).translateChecked [input] =
        some ((fanoutProgram 0).translate [input]) ∧
      (fanoutProgram 0).normalize [input] = some (normal, 1) ∧
      Steps (CStep system) 1 ((fanoutProgram 0).translate [input]).toNet
        normal.toNet ∧
      Normal (CStep system) normal.toNet ∧
      (fanoutProgram 0).readBack normal = some [] := by
  let source := (valueId 0, 0)
  let eraser := (eraId 0, 0)
  let X : XNet NetSym := {
    agents := [(valueId 0, .num input), (eraId 0, .era)]
    free := []
    wires := [(source, eraser), (eraser, source)]
  }
  let Y : XNet NetSym := { agents := [], free := [], wires := [] }
  have htranslate : (fanoutProgram 0).translate [input] = X := rfl
  have hcheck : X.check arity = true := by
    simp [X, source, eraser, XNet.check, XNet.ends, XNet.ids,
      assoc, endsWith, arity, valueId, eraId,
      List.range, List.range.loop]
  have hinput : (fanoutProgram 0).InputsOk [input] := rfl
  have hwfCheck : (fanoutProgram 0).wellFormedCheck = true := by decide
  have hwf := (Program.wellFormedCheck_iff (fanoutProgram 0)).1 hwfCheck
  have htypedCheck : (fanoutProgram 0).wellTypedCheck = true := by decide
  have htyped := (Program.wellTypedCheck_iff (fanoutProgram 0)).1 htypedCheck
  have hadmission : (fanoutProgram 0).acceptanceCheck [input] = true :=
    (Program.acceptanceCheck_iff (fanoutProgram 0) [input]).2
      ⟨hinput, hwf, htyped⟩
  have hchecked : (fanoutProgram 0).translateChecked [input] =
      some ((fanoutProgram 0).translate [input]) := by
    simp [Program.translateChecked, hadmission, htranslate, hcheck]
  have hstep : X.step xsys = some Y := by
    simp [XNet.step, XNet.findRedex, XNet.redexAt, XNet.fire,
      fireTable, retireTable, spliceAll,
      X, Y, source, eraser, xsys, system, xRule,
      eraNumData, RuleData.ofWires, RuleData.toX, XRhs.swap,
      XRhs.wiresFrom, XRhs.newPorts, ifcPorts, LPort.isNew,
      valueId, eraId, assoc, arity]
  have hcheck' : X.check xsys.sys.ar = true := by
    change X.check arity = true
    exact hcheck
  have hXok : X.Ok xsys.sys.ar := XNet.Checked.ok (XNet.check_iff.mp hcheck')
  have hYstep : Y.step xsys = none := by
    simp [XNet.step, XNet.findRedex, Y]
  have hstepSound := XNet.step_sound hXok hstep
  have hnormal : Normal (CStep system) Y.toNet := by
    simpa [xsys] using XNet.step_complete hstepSound.2 hYstep
  have hstepTrace := hstepSound.1
  have hcount : (fanoutProgram 0).interactionCount = 1 := by
    simpa using fanoutProgram_interactionCount 0
  have hnormalize : (fanoutProgram 0).normalize [input] = some (Y, 1) := by
    unfold Program.normalize
    rw [hchecked, hcount, htranslate]
    simp [XNet.normalize, XNet.run, hstep, hYstep, hcheck']
  refine ⟨Y, hchecked, hnormalize, ?_, hnormal, ?_⟩
  · simpa [xsys, htranslate] using
      Steps.head hstepTrace (Steps.refl Y.toNet)
  · simp [Program.readBack, fanoutProgram, Y]

/-- The first nontrivial fanout fires one duplicator and preserves output order. -/
theorem fanoutProgram_two_trace (input : BitVec 64) :
    ∃ normal : XNet NetSym,
      (fanoutProgram 2).translateChecked [input] =
        some ((fanoutProgram 2).translate [input]) ∧
      (fanoutProgram 2).normalize [input] = some (normal, 1) ∧
      Steps (CStep system) 1 ((fanoutProgram 2).translate [input]).toNet
        normal.toNet ∧
      Normal (CStep system) normal.toNet ∧
      (fanoutProgram 2).readBack normal =
        some (List.replicate 2 (.i64 input)) := by
  let X := fanoutTraceNet 2 0 input
  let Y := fanoutTraceNet 2 1 input
  have htranslate : (fanoutProgram 2).translate [input] = X := by rfl
  have hcheck : X.check arity = true := by
    simp [XNet.check, X, fanoutTraceNet, fanoutTracePlan, fanoutTraceSlots,
      XNet.ofWires, XNet.ends, XNet.ids, assoc, endsWith, arity, valueId, dupId,
      outputPort, List.all]
    constructor
    · intro x hx
      have hcases : x = 0 ∨ x = 1 := by omega
      rcases hcases with rfl | rfl <;> decide
    · intro x hx
      have hcases : x = 0 ∨ x = 1 ∨ x = 2 := by omega
      rcases hcases with rfl | rfl | rfl <;> decide
  have hinput : (fanoutProgram 2).InputsOk [input] := rfl
  have hwf : (fanoutProgram 2).WellFormed :=
    (Program.wellFormedCheck_iff _).1 (by decide)
  have htyped : (fanoutProgram 2).WellTyped :=
    (Program.wellTypedCheck_iff _).1 (by decide)
  have hadmission := (Program.acceptanceCheck_iff (fanoutProgram 2) [input]).2
    ⟨hinput, hwf, htyped⟩
  have hchecked : (fanoutProgram 2).translateChecked [input] =
      some ((fanoutProgram 2).translate [input]) := by
    simp [Program.translateChecked, hadmission, htranslate, hcheck]
  have hstep : X.step xsys = some Y := by
    rfl
  have hYstep : Y.step xsys = none := by
    rfl
  have hcheck' : X.check xsys.sys.ar = true := by
    change X.check arity = true
    exact hcheck
  have hXok : X.Ok xsys.sys.ar :=
    XNet.Checked.ok (XNet.check_iff.mp hcheck')
  have hstepSound := XNet.step_sound hXok hstep
  have hnormal : Normal (CStep system) Y.toNet := by
    simpa [xsys] using XNet.step_complete hstepSound.2 hYstep
  have hnormalize : (fanoutProgram 2).normalize [input] = some (Y, 1) := by
    unfold Program.normalize
    rw [hchecked, fanoutProgram_interactionCount, htranslate]
    simp [XNet.normalize, XNet.run, hstep, hYstep, hcheck']
  refine ⟨Y, hchecked, hnormalize, ?_, hnormal, ?_⟩
  · simpa [xsys, htranslate] using
      Steps.head hstepSound.1 (Steps.refl Y.toNet)
  · rfl

end NobleFoundation.NetI64Families
