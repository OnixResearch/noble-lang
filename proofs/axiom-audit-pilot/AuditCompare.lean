import AxiomAudit

open Lean AxiomAudit

-- This executable is an experimental comparison, not a replacement for Lean's
-- proof checking. Its candidate arm uses upstream's shared traversal verbatim;
-- its reference arm calls Lean's independent collectAxioms for each constant.
private def names (text : String) : Array Name :=
  ((text.splitOn ",").filter (!·.isEmpty)).toArray.map String.toName

private def declarations (env : Environment) (root : Name) : Array Name := Id.run do
  let modules := env.allImportedModuleNames
  let found := env.constants.fold (init := #[]) fun acc decl _ =>
    match env.getModuleIdxFor? decl with
    | some idx =>
      match modules[idx.toNat]? with
      | some moduleName => if inAuditedLib root moduleName then acc.push decl else acc
      | none => acc
    | none => acc
  return found.qsort Name.lt

private def row (decl : Name) (axioms : Array Name) : Json :=
  Json.mkObj [
    ("name", Json.str (freshStr decl)),
    ("axioms", Lean.toJson ((axioms.qsort Name.lt).map freshStr))
  ]

private def inspect (roots : Array Name) (candidate : Bool) : CoreM Json := do
  let env ← getEnv
  let imported := env.allImportedModuleNames
  let mut reports := #[]
  for root in roots do
    let selected := declarations env root
    let values ← if candidate then
      pure (Id.run ((selected.mapM axiomsOf |>.run env).run' {}))
    else
      selected.mapM Lean.collectAxioms
    let mut rows := #[]
    for i in [0:selected.size] do
      rows := rows.push (row selected[i]! values[i]!)
    reports := reports.push (Json.mkObj [
      ("root", Json.str (freshStr root)),
      ("count", Lean.toJson selected.size),
      ("rows", Json.arr rows)
    ])
  return Json.mkObj [
    ("mode", Json.str (if candidate then "candidate" else "stock")),
    ("imported_modules", Lean.toJson (imported.map freshStr)),
    ("roots", Json.arr reports)
  ]

def main (args : List String) : IO UInt32 := do
  let [rosterPath, mode] := args
    | IO.eprintln "usage: noble-axiom-compare ROSTER candidate|stock"; return 2
  if mode != "candidate" && mode != "stock" then
    IO.eprintln "mode must be candidate or stock"
    return 2
  let entries := (← IO.FS.readFile rosterPath).splitOn "\n"
  let modules := names (entries[0]!)
  let roots := names (entries[1]!)
  if modules.isEmpty || roots.isEmpty then
    IO.eprintln "empty module or library roster"
    return 2
  let result ← withImportedEnv modules (inspect roots (mode == "candidate"))
  IO.println result.compress
  return 0
