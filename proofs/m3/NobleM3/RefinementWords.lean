/-
The per-word scheme refinements (task 5.2, design item 1): for each of the
23 bootstrap words, the extracted instantiation path — bounds, reference
witness resolution, substitution, and eligibility — applied to the word's
scheme and its canonical witness yields exactly the reference
instantiation's interface, decided by computation over the generated module.
-/

import NobleKernel
import NobleM2.Embed
import NobleM3.CoverageWords

namespace NobleM2.WordRefinement

open NobleM2 NobleM2.Refinement NobleM2.WordCoverage noble_kernel Aeneas Aeneas.Std

/-- The interface the extracted instantiation path derives, projected onto
the reference domain; a rejected or divergent application is `none`. -/
def extractedIface
    (r : Result (core.result.Result (untrusted.Interface × words.Inst) acceptance.Fail)) :
    Option Interface :=
  match r.match with
  | .ok (core.result.Result.Ok (i, _)) => some (projectInterface i)
  | _ => none

/-- The interface the reference instantiation derives, when accepted. -/
def referenceIface (r : Except Failure (Interface × Inst)) : Option Interface :=
  match r with
  | .ok (i, _) => some i
  | .error _ => none

/-- The extracted instantiation context for word position `n`'s canonical
fixture. -/
def wordCtx (n : Nat) : acceptance.parts.Ctx :=
  ⟨embedRequest (wordRequestOf n), embedEnv bootstrapEnv⟩

/-- The eligibility variable the extracted path reads for word `n`, when the
word's behavior constrains one. -/
def wordDataVar (n : Nat) : Option noble_kernel.words.Variable :=
  match n with
  | 0 | 1 | 8 => some (embedU32 1)
  | _ => none

/-- One word position's instantiation agreement: the extracted path and the
reference substitution both derive the fixture's interface. -/
def wordAgrees (n : Nat) (scheme : Scheme) : Bool :=
  extractedIface (acceptance.parts.instantiate.apply (embedScheme scheme)
      (embedInst (canonInst n)) (wordDataVar n) ⟨none, none⟩ (wordCtx n))
    = some (wordCheckedOf n).interface
    && referenceIface (applyScheme bootstrapEnv (wordRequestOf n).limits scheme
        (canonInst n)) = some (wordCheckedOf n).interface

/-- The 23 per-word instantiation rows, one per table entry, each binding
the extracted instantiation path to the reference instantiation's interface
by evaluation. -/
theorem word_refinement_dup : wordAgrees 0 dupScheme = true := by native_decide
theorem word_refinement_drop : wordAgrees 1 dropScheme = true := by native_decide
theorem word_refinement_swap : wordAgrees 2 swapScheme = true := by native_decide
theorem word_refinement_dip : wordAgrees 3 dipScheme = true := by native_decide
theorem word_refinement_add : wordAgrees 4 arithScheme = true := by native_decide
theorem word_refinement_sub : wordAgrees 5 arithScheme = true := by native_decide
theorem word_refinement_mul : wordAgrees 6 arithScheme = true := by native_decide
theorem word_refinement_equals : wordAgrees 7 equalsScheme = true := by native_decide
theorem word_refinement_quote : wordAgrees 8 quoteScheme = true := by native_decide
theorem word_refinement_compose : wordAgrees 9 composeScheme = true := by native_decide
theorem word_refinement_run : wordAgrees 10 runScheme = true := by native_decide
theorem word_refinement_reflect : wordAgrees 11 reflectScheme = true := by native_decide
theorem word_refinement_unit : wordAgrees 12 unitScheme = true := by native_decide
theorem word_refinement_pair : wordAgrees 13 pairScheme = true := by native_decide
theorem word_refinement_unpair : wordAgrees 14 unpairScheme = true := by native_decide
theorem word_refinement_inl : wordAgrees 15 inlScheme = true := by native_decide
theorem word_refinement_inr : wordAgrees 16 inrScheme = true := by native_decide
theorem word_refinement_case : wordAgrees 17 caseScheme = true := by native_decide
theorem word_refinement_if : wordAgrees 18 ifScheme = true := by native_decide
theorem word_refinement_nil : wordAgrees 19 nilScheme = true := by native_decide
theorem word_refinement_cons : wordAgrees 20 consScheme = true := by native_decide
theorem word_refinement_list_case : wordAgrees 21 listCaseScheme = true := by native_decide
theorem word_refinement_test_emit : wordAgrees 22 emitScheme = true := by native_decide

/-- Every one of the 23 per-word instantiation refinements holds at once. -/
theorem word_refinements :
    (List.range 23).all (fun n =>
      match wordTable[n]? with
      | some word => wordAgrees n word.scheme
      | none => false) = true := by
  native_decide

end NobleM2.WordRefinement
