/-
  Issue #81 closure extractor library.

  Prints, as one JSON object, the roots declared by a subject's handwritten refinement modules and,
  for each root, the generated constants its transitive constant closure mentions. The environment
  is Noble's compiled Lean environment, never a lean4export file and never `grep`.

  Scope decision (reviewed once, change deliberately): the root set is "every theorem declared by
  the chosen modules". Naming individual roots by hand would let a lane claim coverage of a theorem
  it never looked at, and would need editing whenever a theorem is added.

  Canonical statement form: binder names and `mdata` dropped (alpha equivalence), binder kinds kept,
  names dotted and unescaped.

  This file imports no subject module, because MC1 and M3 are separate lake projects: the subject
  module is imported at run time from `NOBLE_CLOSURE_MODULES`, which needs the subject's
  build directory and this lane directory on `LEAN_PATH`. Run it from the subject's proof root with
  `LEAN_PATH`.
-/
import Lean
open Lean

namespace NobleRefinementClosure

/-- Default root-module scope, overridable with `NOBLE_CLOSURE_MODULES` so one lane can describe both
    subjects. -/
def defaultRootModules : List String := ["NobleContractImpl.Projection", "NobleContractImpl.Statement"]

def defaultGeneratedPrefix : String := "noble_contracts"

def nameText (n : Name) : String := n.toString (escape := false)

partial def exprConsts (e : Expr) : List Name :=
  match e with
  | .const n _ => [n]
  | .app f a => exprConsts f ++ exprConsts a
  | .lam _ t b _ => exprConsts t ++ exprConsts b
  | .forallE _ t b _ => exprConsts t ++ exprConsts b
  | .letE _ t v b _ => exprConsts t ++ exprConsts v ++ exprConsts b
  | .mdata _ e => exprConsts e
  | .proj _ _ e => exprConsts e
  | .lit _ => []
  | .sort _ => []
  | .bvar _ => []
  | .fvar _ => []
  | .mvar _ => []

def declConsts (info : ConstantInfo) : List Name :=
  match info with
  | .defnInfo v => exprConsts v.type ++ exprConsts v.value
  | .thmInfo v => exprConsts v.type ++ exprConsts v.value
  | .opaqueInfo v => exprConsts v.type ++ exprConsts v.value
  | .axiomInfo v => exprConsts v.type
  | .inductInfo v => exprConsts v.type ++ v.ctors
  | .ctorInfo v => exprConsts v.type
  | .recInfo v => exprConsts v.type
  | .quotInfo _ => []

def isThm (info : ConstantInfo) : Bool :=
  match info with
  | .thmInfo _ => true
  | _ => false

/-- Auxiliary declarations Lean generates (`_proof_1_2`, `_simp_1`, …) and Aeneas' own library
    theorems are not handwritten refinement roots: keeping them would let a lane count machinery as
    coverage. -/
def isAuxiliary (declName : Name) : Bool :=
  let text := nameText declName
  text.startsWith "Aeneas." ||
    (text.splitOn ".").any fun part => part.startsWith "_"

/-- Every constant reachable from `starts` through declaration types and values, transitively. -/
partial def reachable (env : Environment) (pending : List Name) (seen : Std.HashSet Name)
    (fuel : Nat) : Std.HashSet Name :=
  match pending, fuel with
  | [], _ => seen
  | _, 0 => seen
  | current :: rest, fuel + 1 =>
      if seen.contains current then
        reachable env rest seen fuel
      else
        let seen := seen.insert current
        match env.find? current with
        | none => reachable env rest seen fuel
        | some info =>
            let next := (declConsts info).filter fun n => !seen.contains n
            reachable env (next ++ rest) seen fuel

partial def renderLevel : Level → String
  | .zero => "0"
  | .succ l => s!"(s {renderLevel l})"
  | .max a b => s!"(max {renderLevel a} {renderLevel b})"
  | .imax a b => s!"(imax {renderLevel a} {renderLevel b})"
  | .param n => s!"(u {nameText n})"
  | .mvar _ => "(mvar)"

def renderBinder : BinderInfo → String
  | .default => "d"
  | .implicit => "i"
  | .strictImplicit => "s"
  | .instImplicit => "c"

partial def renderExpr : Expr → String
  | .bvar i => s!"#{i}"
  | .sort l => s!"(sort {renderLevel l})"
  | .const n us => "(c " ++ nameText n ++ String.join (us.map fun u => " " ++ renderLevel u) ++ ")"
  | .app f a => s!"(@ {renderExpr f} {renderExpr a})"
  | .lam _ t b bi => s!"(fun {renderBinder bi} {renderExpr t} {renderExpr b})"
  | .forallE _ t b bi => s!"(pi {renderBinder bi} {renderExpr t} {renderExpr b})"
  | .letE _ t v b _ => s!"(let {renderExpr t} {renderExpr v} {renderExpr b})"
  | .lit (.natVal n) => s!"(nat {n})"
  | .lit (.strVal s) => s!"(str {s.length})"
  | .mdata _ e => renderExpr e
  | .proj s i e => s!"(proj {nameText s} {i} {renderExpr e})"
  | .fvar _ => "(fvar)"
  | .mvar _ => "(mvar)"

def jsonEscape (s : String) : String :=
  s.foldl (init := "") fun acc c =>
    match c with
    | '"' => acc ++ "\\\""
    | '\\' => acc ++ "\\\\"
    | '\n' => acc ++ "\\n"
    | _ => acc.push c

def jsonString (s : String) : String := "\"" ++ jsonEscape s ++ "\""

def jsonList (items : List String) : String := "[" ++ String.intercalate ", " items ++ "]"

/-- Declaration kind of a generated constant: a `defn` or `opaque` is a function the extraction
    inventory can cover; `induct`, `ctor` and `rec` are type-side constants that never were links. -/
def kindOf (env : Environment) (declName : Name) : String :=
  match env.find? declName with
  | some (.defnInfo _) => "defn"
  | some (.opaqueInfo _) => "opaque"
  | some (.thmInfo _) => "thm"
  | some (.inductInfo _) => "induct"
  | some (.ctorInfo _) => "ctor"
  | some (.recInfo _) => "rec"
  | some (.axiomInfo _) => "axiom"
  | some (.quotInfo _) => "quot"
  | none => "missing"
/-- One root record: canonical statement, transitive generated-constant mentions with their kinds. -/
def rootRecord (env : Environment) (generatedPrefix : String) (declName : Name)
    (info : ConstantInfo) : String :=
  let reachableNames := (reachable env [declName] {} 200_000).toList
  let mentions := reachableNames.filterMap fun n =>
    if generatedPrefix.isPrefixOf (nameText n) then
      some ("{" ++ jsonString "name" ++ ": " ++ jsonString (nameText n) ++ ", " ++
        jsonString "kind" ++ ": " ++ jsonString (kindOf env n) ++ "}")
    else none
  let sorted := mentions.mergeSort (· < ·)
  "{" ++ jsonString "name" ++ ": " ++ jsonString (nameText declName) ++ ", " ++
    jsonString "statement" ++ ": " ++ jsonString (renderExpr info.type) ++ ", " ++
    jsonString "mentions" ++ ": " ++ jsonList sorted ++ ", " ++
    jsonString "constants" ++ ": " ++ toString reachableNames.length ++ "}"

/-- Read the scope overrides, then print the whole closure as one JSON object. -/
def report : CoreM Unit := do
  let envOf (key : String) (fallback : String) : CoreM String := do
    return ((← IO.getEnv key).getD fallback)
  let configured := ((← envOf "NOBLE_CLOSURE_MODULES" "").splitOn " ").filter (· ≠ "")
  let rootModules := if configured.isEmpty then defaultRootModules else configured
  let generatedPrefix ← envOf "NOBLE_CLOSURE_PREFIX" defaultGeneratedPrefix
  initSearchPath (← getBuildDir)
  let env ← importModules ((rootModules.map String.toName).toArray.map fun m => { module := m : Import }) {}
  let moduleNames := rootModules.map String.toName
  let roots := env.constants.toList.filter fun pair =>
    isThm pair.2 && !isAuxiliary pair.1 &&
      moduleNames.any fun module => env.getModuleIdxFor? pair.1 == env.getModuleIdx? module
  let sorted := roots.mergeSort fun a b => nameText a.1 < nameText b.1
  let records := sorted.map fun pair => rootRecord env generatedPrefix pair.1 pair.2
  IO.println ("{" ++ jsonString "roots" ++ ": {" ++
    jsonString "modules" ++ ": " ++ jsonList (rootModules.map jsonString) ++ ", " ++
    jsonString "declared" ++ ": " ++ jsonList (sorted.map fun pair => jsonString (nameText pair.1)) ++ ", " ++
    jsonString "records" ++ ": " ++ jsonList records ++ "}}")

end NobleRefinementClosure

open NobleRefinementClosure in
#eval show CoreM Unit from report
