/-
Per-word acceptance and derivation: one theorem per bootstrap word.

Each word's canonical fixture (`Coverage.wordCandidate n`) is accepted by
the reference checker at exactly its instantiated interface (closed by
evaluation) and carries a declarative `Derives` witness through the word
rule — the M2 `Coverage` pattern, extended to every one of the 23 table
entries. `Coverage.coverage_positive_word` aggregates the family.
-/

import NobleM3.Coverage

namespace NobleM2.WordCoverage

open NobleM2 NobleM2.Refinement

section PerWord

/-- `dup` (position 0): the canonical fixture is accepted and derivable. -/
theorem coverage_dup : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 0 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 0, wordCandidate 0, wordCheckedOf 0, by native_decide,
    ⟨canonInst 0, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 0 = some dupScheme := by native_decide
  have hinst : dupScheme.instantiate (canonInst 0)
      = some ((wordCheckedOf 0).interface.stackIn, (wordCheckedOf 0).interface.stackOut,
          (wordCheckedOf 0).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 0) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst ⟨canonTy, by native_decide, by native_decide⟩
  rw [union_empty_right] at h
  exact h

/-- `drop` (position 1): the canonical fixture is accepted and derivable. -/
theorem coverage_drop : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 1 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 1, wordCandidate 1, wordCheckedOf 1, by native_decide,
    ⟨canonInst 1, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 1 = some dropScheme := by native_decide
  have hinst : dropScheme.instantiate (canonInst 1)
      = some ((wordCheckedOf 1).interface.stackIn, (wordCheckedOf 1).interface.stackOut,
          (wordCheckedOf 1).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 1) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst ⟨canonTy, by native_decide, by native_decide⟩
  rw [union_empty_right] at h
  exact h

/-- `swap` (position 2): the canonical fixture is accepted and derivable. -/
theorem coverage_swap : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 2 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 2, wordCandidate 2, wordCheckedOf 2, by native_decide,
    ⟨canonInst 2, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 2 = some swapScheme := by native_decide
  have hinst : swapScheme.instantiate (canonInst 2)
      = some ((wordCheckedOf 2).interface.stackIn, (wordCheckedOf 2).interface.stackOut,
          (wordCheckedOf 2).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 2) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `dip` (position 3): the canonical fixture is accepted and derivable. -/
theorem coverage_dip : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 3 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 3, wordCandidate 3, wordCheckedOf 3, by native_decide,
    ⟨canonInst 3, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 3 = some dipScheme := by native_decide
  have hinst : dipScheme.instantiate (canonInst 3)
      = some ((wordCheckedOf 3).interface.stackIn, (wordCheckedOf 3).interface.stackOut,
          (wordCheckedOf 3).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 3) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `add` (position 4): the canonical fixture is accepted and derivable. -/
theorem coverage_add : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 4 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 4, wordCandidate 4, wordCheckedOf 4, by native_decide,
    ⟨canonInst 4, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 4 = some arithScheme := by native_decide
  have hinst : arithScheme.instantiate (canonInst 4)
      = some ((wordCheckedOf 4).interface.stackIn, (wordCheckedOf 4).interface.stackOut,
          (wordCheckedOf 4).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 4) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `sub` (position 5): the canonical fixture is accepted and derivable. -/
theorem coverage_sub : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 5 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 5, wordCandidate 5, wordCheckedOf 5, by native_decide,
    ⟨canonInst 5, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 5 = some arithScheme := by native_decide
  have hinst : arithScheme.instantiate (canonInst 5)
      = some ((wordCheckedOf 5).interface.stackIn, (wordCheckedOf 5).interface.stackOut,
          (wordCheckedOf 5).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 5) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `mul` (position 6): the canonical fixture is accepted and derivable. -/
theorem coverage_mul : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 6 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 6, wordCandidate 6, wordCheckedOf 6, by native_decide,
    ⟨canonInst 6, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 6 = some arithScheme := by native_decide
  have hinst : arithScheme.instantiate (canonInst 6)
      = some ((wordCheckedOf 6).interface.stackIn, (wordCheckedOf 6).interface.stackOut,
          (wordCheckedOf 6).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 6) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `equals` (position 7): the canonical fixture is accepted and derivable. -/
theorem coverage_equals : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 7 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 7, wordCandidate 7, wordCheckedOf 7, by native_decide,
    ⟨canonInst 7, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 7 = some equalsScheme := by native_decide
  have hinst : equalsScheme.instantiate (canonInst 7)
      = some ((wordCheckedOf 7).interface.stackIn, (wordCheckedOf 7).interface.stackOut,
          (wordCheckedOf 7).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 7) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `quote` (position 8): the canonical fixture is accepted and derivable. -/
theorem coverage_quote : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 8 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 8, wordCandidate 8, wordCheckedOf 8, by native_decide,
    ⟨canonInst 8, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 8 = some quoteScheme := by native_decide
  have hinst : quoteScheme.instantiate (canonInst 8)
      = some ((wordCheckedOf 8).interface.stackIn, (wordCheckedOf 8).interface.stackOut,
          (wordCheckedOf 8).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 8) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst ⟨canonTy, by native_decide, by native_decide⟩
  rw [union_empty_right] at h
  exact h

/-- `compose` (position 9): the canonical fixture is accepted and derivable. -/
theorem coverage_compose : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 9 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 9, wordCandidate 9, wordCheckedOf 9, by native_decide,
    ⟨canonInst 9, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 9 = some composeScheme := by native_decide
  have hinst : composeScheme.instantiate (canonInst 9)
      = some ((wordCheckedOf 9).interface.stackIn, (wordCheckedOf 9).interface.stackOut,
          (wordCheckedOf 9).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 9) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `run` (position 10): the canonical fixture is accepted and derivable. -/
theorem coverage_run : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 10 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 10, wordCandidate 10, wordCheckedOf 10, by native_decide,
    ⟨canonInst 10, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 10 = some runScheme := by native_decide
  have hinst : runScheme.instantiate (canonInst 10)
      = some ((wordCheckedOf 10).interface.stackIn, (wordCheckedOf 10).interface.stackOut,
          (wordCheckedOf 10).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 10) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `reflect` (position 11): the canonical fixture is accepted and derivable. -/
theorem coverage_reflect : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 11 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 11, wordCandidate 11, wordCheckedOf 11, by native_decide,
    ⟨canonInst 11, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 11 = some reflectScheme := by native_decide
  have hinst : reflectScheme.instantiate (canonInst 11)
      = some ((wordCheckedOf 11).interface.stackIn, (wordCheckedOf 11).interface.stackOut,
          (wordCheckedOf 11).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 11) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `unit` (position 12): the canonical fixture is accepted and derivable. -/
theorem coverage_unit : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 12 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 12, wordCandidate 12, wordCheckedOf 12, by native_decide,
    ⟨canonInst 12, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 12 = some unitScheme := by native_decide
  have hinst : unitScheme.instantiate (canonInst 12)
      = some ((wordCheckedOf 12).interface.stackIn, (wordCheckedOf 12).interface.stackOut,
          (wordCheckedOf 12).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 12) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `pair` (position 13): the canonical fixture is accepted and derivable. -/
theorem coverage_pair : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 13 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 13, wordCandidate 13, wordCheckedOf 13, by native_decide,
    ⟨canonInst 13, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 13 = some pairScheme := by native_decide
  have hinst : pairScheme.instantiate (canonInst 13)
      = some ((wordCheckedOf 13).interface.stackIn, (wordCheckedOf 13).interface.stackOut,
          (wordCheckedOf 13).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 13) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `unpair` (position 14): the canonical fixture is accepted and derivable. -/
theorem coverage_unpair : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 14 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 14, wordCandidate 14, wordCheckedOf 14, by native_decide,
    ⟨canonInst 14, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 14 = some unpairScheme := by native_decide
  have hinst : unpairScheme.instantiate (canonInst 14)
      = some ((wordCheckedOf 14).interface.stackIn, (wordCheckedOf 14).interface.stackOut,
          (wordCheckedOf 14).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 14) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `inl` (position 15): the canonical fixture is accepted and derivable. -/
theorem coverage_inl : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 15 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 15, wordCandidate 15, wordCheckedOf 15, by native_decide,
    ⟨canonInst 15, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 15 = some inlScheme := by native_decide
  have hinst : inlScheme.instantiate (canonInst 15)
      = some ((wordCheckedOf 15).interface.stackIn, (wordCheckedOf 15).interface.stackOut,
          (wordCheckedOf 15).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 15) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `inr` (position 16): the canonical fixture is accepted and derivable. -/
theorem coverage_inr : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 16 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 16, wordCandidate 16, wordCheckedOf 16, by native_decide,
    ⟨canonInst 16, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 16 = some inrScheme := by native_decide
  have hinst : inrScheme.instantiate (canonInst 16)
      = some ((wordCheckedOf 16).interface.stackIn, (wordCheckedOf 16).interface.stackOut,
          (wordCheckedOf 16).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 16) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `case` (position 17): the canonical fixture is accepted and derivable. -/
theorem coverage_case : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 17 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 17, wordCandidate 17, wordCheckedOf 17, by native_decide,
    ⟨canonInst 17, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 17 = some caseScheme := by native_decide
  have hinst : caseScheme.instantiate (canonInst 17)
      = some ((wordCheckedOf 17).interface.stackIn, (wordCheckedOf 17).interface.stackOut,
          (wordCheckedOf 17).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 17) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `if` (position 18): the canonical fixture is accepted and derivable. -/
theorem coverage_if : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 18 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 18, wordCandidate 18, wordCheckedOf 18, by native_decide,
    ⟨canonInst 18, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 18 = some ifScheme := by native_decide
  have hinst : ifScheme.instantiate (canonInst 18)
      = some ((wordCheckedOf 18).interface.stackIn, (wordCheckedOf 18).interface.stackOut,
          (wordCheckedOf 18).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 18) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `nil` (position 19): the canonical fixture is accepted and derivable. -/
theorem coverage_nil : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 19 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 19, wordCandidate 19, wordCheckedOf 19, by native_decide,
    ⟨canonInst 19, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 19 = some nilScheme := by native_decide
  have hinst : nilScheme.instantiate (canonInst 19)
      = some ((wordCheckedOf 19).interface.stackIn, (wordCheckedOf 19).interface.stackOut,
          (wordCheckedOf 19).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 19) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `cons` (position 20): the canonical fixture is accepted and derivable. -/
theorem coverage_cons : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 20 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 20, wordCandidate 20, wordCheckedOf 20, by native_decide,
    ⟨canonInst 20, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 20 = some consScheme := by native_decide
  have hinst : consScheme.instantiate (canonInst 20)
      = some ((wordCheckedOf 20).interface.stackIn, (wordCheckedOf 20).interface.stackOut,
          (wordCheckedOf 20).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 20) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `list_case` (position 21): the canonical fixture is accepted and derivable. -/
theorem coverage_list_case : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 21 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 21, wordCandidate 21, wordCheckedOf 21, by native_decide,
    ⟨canonInst 21, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 21 = some listCaseScheme := by native_decide
  have hinst : listCaseScheme.instantiate (canonInst 21)
      = some ((wordCheckedOf 21).interface.stackIn, (wordCheckedOf 21).interface.stackOut,
          (wordCheckedOf 21).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 21) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h

/-- `test_emit` (position 22): the canonical fixture is accepted and derivable. -/
theorem coverage_test_emit : ∃ (req : Request) (cand : Candidate) (checked : Checked),
    NobleM2.check bootstrapEnv req cand = .accepted checked ∧
    (∃ wit, cand.nodes[0]? = some (.invocation 22 wit)) ∧
    Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
      checked.interface.effects := by
  refine ⟨wordRequestOf 22, wordCandidate 22, wordCheckedOf 22, by native_decide,
    ⟨canonInst 22, rfl⟩, ?_⟩
  have hlook : bootstrapEnv.scheme 22 = some emitScheme := by native_decide
  have hinst : emitScheme.instantiate (canonInst 22)
      = some ((wordCheckedOf 22).interface.stackIn, (wordCheckedOf 22).interface.stackOut,
          (wordCheckedOf 22).interface.effects) := by native_decide
  have h := invoke_derives (cand := wordCandidate 22) rfl rfl hlook
    (resolvesTo_refl _ (by native_decide)) hinst trivial
  rw [union_empty_right] at h
  exact h
/-- Positive coverage for every table entry (PO-10, V-CHECK-05): each of the
23 bootstrap words has an accepted, derivable fixture that exercises the
word at its own table position — the per-word family above. -/
theorem coverage_positive_word : ∀ (n : Nat), n < 23 →
    ∃ (req : Request) (cand : Candidate) (checked : Checked),
      NobleM2.check bootstrapEnv req cand = .accepted checked ∧
      (∃ wit, cand.nodes[0]? = some (.invocation n wit)) ∧
      Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
        checked.interface.effects := by
  intro n hn
  match n with
  | 0 => exact coverage_dup
  | 1 => exact coverage_drop
  | 2 => exact coverage_swap
  | 3 => exact coverage_dip
  | 4 => exact coverage_add
  | 5 => exact coverage_sub
  | 6 => exact coverage_mul
  | 7 => exact coverage_equals
  | 8 => exact coverage_quote
  | 9 => exact coverage_compose
  | 10 => exact coverage_run
  | 11 => exact coverage_reflect
  | 12 => exact coverage_unit
  | 13 => exact coverage_pair
  | 14 => exact coverage_unpair
  | 15 => exact coverage_inl
  | 16 => exact coverage_inr
  | 17 => exact coverage_case
  | 18 => exact coverage_if
  | 19 => exact coverage_nil
  | 20 => exact coverage_cons
  | 21 => exact coverage_list_case
  | 22 => exact coverage_test_emit
  | (m + 23) => omega

end PerWord

end NobleM2.WordCoverage
