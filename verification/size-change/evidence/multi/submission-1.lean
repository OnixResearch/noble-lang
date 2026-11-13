/-
  Generated size-change certificate (issue #101).

  Each `Args` vector is the front end's classification of one self-call site; the theorems
  re-check the model's own decision procedure (`certifiedAll`) by evaluation. This is evidence
  about the classified relations, not about an execution: see `verification/size-change`.
-/
import NobleM2.SizeChangeFixtures

namespace NobleSizeChangeCertificate

open NobleM2.SizeChange

/-- Certified definition 0: 1 self-call site(s) over 1 declared measure(s). -/
def definition_0Args : Fin 1 → Args 1 := fun site measure =>
  match site.val, measure.val with
  | 0, 0 => .predOfSame
  | _, _ => .opaque

def definition_0Measure : Fin 1 := ⟨0, by decide⟩

theorem definition_0Certified : certifiedAll (fun site => graphOf (definition_0Args site)) = true := by
  decide

theorem definition_0Shared : ∀ site : Fin 1, graphOf (definition_0Args site) definition_0Measure = .decrease := by
  decide

end NobleSizeChangeCertificate