/-! Claim strength of one coverage citation (#88), spliced after the imports of
the coverage gates' environment checkers. It is computed from the elaborated
statement in the compiled environment, never from source text, and it
describes statement shape only: a strength class does not show that the
reference model is correct.

* `conditional`: some hypothesis is an equation that mentions the classified
  constant, so the statement assumes the refinement it is cited for;
* `fixture`: otherwise, the statement quantifies over no non-proof input;
* `universal`: otherwise, it quantifies over inputs with no such hypothesis.

The axiom class is `native` when a native-evaluation axiom (a per-declaration
`native_decide` auxiliary, `Lean.ofReduceBool` or `Lean.trustCompiler`) is in
the transitive axiom set, so the statement also trusts compiled evaluation. -/
open Lean Meta in
def citationStrength (constant : Name) (type : Expr) : MetaM String :=
  forallTelescope type fun binders _ => do
    let mut inputs := 0
    let mut conditional := false
    for binder in binders do
      let binderType ← inferType binder
      if ← isProp binderType then
        if binderType.isAppOfArity ``Eq 3 && binderType.getUsedConstants.contains constant then
          conditional := true
      else
        inputs := inputs + 1
    return if conditional then "conditional"
      else if inputs == 0 then "fixture"
      else "universal"

def axiomClass (axioms : Array Lean.Name) : String :=
  if axioms.any (fun a =>
      a == `Lean.ofReduceBool || a == `Lean.trustCompiler
        || a.components.any (fun c => c.toString == "_native"))
  then "native" else "strict"

open Lean Elab Command in
/-- Classify one citation and report it on a line the gate parses:
`<tag>: STRENGTH <theorem> <constant> <strength> <axioms>`. -/
def reportStrength (tag : String) (constant : Name) (citation : Name) (type : Expr) :
    CommandElabM Unit := do
  let strength ← liftTermElabM <| citationStrength constant type
  let axioms ← collectAxioms citation
  logInfo m!"{tag}: STRENGTH {citation} {constant} {strength} {axiomClass axioms}"
