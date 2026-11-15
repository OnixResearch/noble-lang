import Lean

/-!
Local equivalents of compiler-lab #217's function/structure eta and native-axiom controls.
These are not upstream shared-corpus fixtures. Each theorem is exported separately; the two
eta counterexamples are made by changing one expression in the resulting valid NDJSON.
-/
namespace Compiler217

theorem functionEta (f g : Nat → Nat) : (fun x => f x) = f := rfl

theorem structureEta (p : Nat × Nat) : (p.1, p.2) = p := rfl

theorem ordinary : (2 : Nat) + 2 = 4 := by decide

theorem nativeDirect : (2 : Nat) + 2 = 4 := by native_decide

private theorem nativeHelper : (2 : Nat) + 2 = 4 := by native_decide

theorem nativeTransitive : (2 : Nat) + 2 = 4 := nativeHelper

end Compiler217
