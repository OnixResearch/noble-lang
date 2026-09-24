import NobleKernel.Funs
import Lean

-- The backend imports frontend types before its own generated declarations,
-- preserving unique discriminant instances and auditing transitive dependencies.
open Lean Elab Command

set_option maxHeartbeats 1000000
set_option maxRecDepth 2048

namespace M4Audit

private inductive AuditLane
  | contracts
  | wasm

def strictAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

def definingModule (env : Environment) (decl : Name) : Name :=
  match env.getModuleIdxFor? decl with
  | some index => env.header.moduleNames[index.toNat]?.getD .anonymous
  | none => .anonymous

private def projectModule (declName : Name) : Bool :=
  ["NobleKernel.", "NobleContractImpl.", "NobleContracts.", "NobleWasmImpl."].any
    (fun modulePrefix => declName.toString.startsWith modulePrefix)

private def projectName (declName : Name) : Bool :=
  ["noble_kernel.", "noble_contracts.", "noble_wasm.", "NobleContractImpl.", "NobleContracts."].any
    (fun modulePrefix => declName.toString.startsWith modulePrefix)

private def generatedModule (declName : Name) : Bool :=
  [ `NobleKernel.Types, `NobleKernel.Funs, `NobleContractImpl.Types,
    `NobleContractImpl.Funs, `NobleWasmImpl.Types, `NobleWasmImpl.Funs ].contains declName

private def modelModules (lane : String) : List Name :=
  if lane == "kernel" then [ `NobleKernel.FunsExternal, `NobleKernel.TypesExternal ]
  else if lane == "contracts" then [ `NobleContractImpl.FunsExternal,
    `NobleContractImpl.TypesExternal, `NobleContractImpl.StdModels, `NobleContractImpl.KernelBridge ]
  else if lane == "wasm" then [ `NobleWasmImpl.FunsExternal,
    `NobleWasmImpl.TypesExternal, `NobleWasmImpl.StdModels,
    `NobleWasmImpl.KernelBridge, `NobleWasmImpl.ContractsBridge ]
  else []

private def literalStringSize (expression : Expr) : Bool := Id.run do
  let proposition := expression.consumeMData
  unless proposition.isAppOfArity ``LE.le 4 do return false
  let bound := proposition.getAppArgs
  unless bound[0]!.isConstOf ``Nat && bound[3]!.isConstOf ``Aeneas.Std.U32.max do return false
  let size := bound[2]!.consumeMData
  unless size.isAppOfArity ``ByteArray.size 1 do return false
  let bytes := size.appArg!.consumeMData
  unless bytes.isAppOfArity ``String.toByteArray 1 do return false
  match bytes.appArg!.consumeMData with
  | .lit (.strVal _) => return true
  | _ => return false

private def nativeOwner (env : Environment) (decl : Name) : Bool :=
  [ `NobleKernel.Funs, `NobleKernel.FunsExternal, `NobleContractImpl.Funs,
    `NobleWasmImpl.Funs ].contains (definingModule env decl)

/-- Native evaluation is not a strict axiom permission. Only the complete shape
of an actual generated/inherited literal UTF-8 array size bound is admitted. -/
private def nativeStringBound (env : Environment) (decl : Name) : Bool := Id.run do
  unless nativeOwner env decl do return false
  match decl with
  | .str (.str (.str _ "_native") "decide") suffix =>
    unless suffix.startsWith "ax_" && suffix.length > 3 &&
        (suffix.toList.drop 3).all Char.isDigit do return false
  | _ => return false
  let some (.axiomInfo info) := env.find? decl | return false
  if info.isUnsafe || !info.levelParams.isEmpty then return false
  let type := info.type.consumeMData
  unless type.isAppOfArity ``Eq 3 do return false
  let equation := type.getAppArgs
  unless equation[0]!.isConstOf ``Bool && equation[2]!.isConstOf ``Bool.true do return false
  let decision := equation[1]!.consumeMData
  unless decision.isAppOfArity ``decide 2 do return false
  return literalStringSize decision.getAppArgs[0]!

/-- Reuse the exact generated-literal size allowance in the separately labelled
MC2 renderer audit. This does not make it a strict theorem axiom permission. -/
def isGeneratedLiteralSizeAxiom (env : Environment) (decl : Name) : Bool :=
  nativeStringBound env decl

/-- The inherited formatting model also elaborates closed `_proof_` lemmas
for those literal bounds. Report them as string obligations, never as strict
bridge equations; the complete proposition is checked independently of names. -/
private def nativeStringProof (env : Environment) (decl : Name) : Bool :=
  nativeOwner env decl && decl.toString.contains "._proof_" &&
    match env.find? decl with
    | some (.thmInfo info) => info.levelParams.isEmpty && literalStringSize info.type
    | _ => false

private def abstractFormatter (env : Environment) (decl : Name) : Bool :=
  decl == ``Aeneas.Std.core.fmt.Formatter &&
  definingModule env decl == `Aeneas.Std.Core.Fmt &&
  match env.find? decl with
  | some (.axiomInfo info) =>
    !info.isUnsafe && info.levelParams.isEmpty && info.type == mkSort (.succ .zero)
  | _ => false

def declaration (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  match env.find? declName with
  | some info => pure info
  | none => throwError "M4-DECLARATION: required declaration absent: {declName}"

def transparent (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  match ← declaration env declName with
  | info@(.defnInfo value) =>
    unless value.safety == .safe do throwError "M4-TRANSPARENT: unsafe/partial definition: {declName}"
    pure info
  | _ => throwError "M4-TRANSPARENT: extracted/model definition absent: {declName}"

-- External Rust types have safe data representations or reducible aliases.
-- An inductive representation is not an opaque type or an axiom permission.
private def modelDeclaration (env : Environment) (declName : Name) (isType : Bool) :
    CommandElabM Unit := do
  if isType then
    match ← declaration env declName with
    | .inductInfo value =>
      if value.isUnsafe then throwError "M4-TRANSPARENT: unsafe model type: {declName}"
      return
    | _ => pure ()
  let _ ← transparent env declName
  pure ()

private def field (value : Json) (key : String) : CommandElabM String := do
  match value.getObjValAs? String key with
  | .ok text =>
    if text.isEmpty then throwError "M4-PACKET: empty {key}"
    pure text
  | .error error => throwError "M4-PACKET: {key}: {error}"

private def entries (packet : Json) (key : String) : CommandElabM (Array Json) := do
  match packet.getObjValAs? (Array Json) key with
  | .ok rows =>
    if rows.isEmpty then throwError "M4-PACKET: empty {key}"
    pure rows
  | .error error => throwError "M4-PACKET: {key}: {error}"

private def requireConstants (declName : Name) (expression : Expr) (required : List Name) :
    CommandElabM Unit := do
  for dependency in required do
    unless expression.getUsedConstants.contains dependency do
      throwError "M4-STRICT-SUBJECT: {declName} does not bind {dependency}"

/-- Logical dependencies, including declaration types and values. This is not
an execution trace or a universal correspondence result. -/
def reachable (env : Environment) (root : Name) : CommandElabM (Array Name) := do
  let mut seen : NameSet := {}
  let mut pending := #[root]
  while !pending.isEmpty do
    let current := pending.back!
    pending := pending.pop
    unless seen.contains current do
      seen := seen.insert current
      let info ← declaration env current
      pending := pending ++ info.type.getUsedConstants
      if let some body := info.value? (allowOpaque := true) then
        pending := pending ++ body.getUsedConstants
      if let .inductInfo value := info then pending := pending ++ value.ctors.toArray
  return seen.toArray.qsort Name.lt

def auditAxioms (env : Environment) (declName : Name) (strict : Bool)
    (allowFormatter : Bool := false) (code : String := "M4-AXIOM") :
    CommandElabM (Array Name) := do
  let axioms ← liftCoreM (collectAxioms declName)
  for axiomName in axioms do
    unless strictAxioms.contains axiomName do
      unless !strict && (nativeStringBound env axiomName ||
          (allowFormatter && abstractFormatter env axiomName)) do
        throwError "{code}: disallowed axiom {axiomName} in {declName}"
  return axioms.qsort Name.lt

def kind : ConstantInfo → String
  | .defnInfo _ => "definition"
  | .thmInfo _ => "theorem"
  | .axiomInfo _ => "axiom"
  | .opaqueInfo _ => "opaque-definition"
  | .inductInfo _ => "inductive"
  | .ctorInfo _ => "constructor"
  | .recInfo _ => "recursor"
  | .quotInfo _ => "quotient"

private def strictTheorems (lane : AuditLane) : List Name := Id.run do
  let (bridge, types, additional) : Name × List String × List Name := match lane with
    | .contracts => (`noble_contracts.KernelBridge,
      ["VariableKind", "Ty", "Pattern", "EffectSlot", "Scheme", "Behavior",
       "SchemaDecl", "Env", "Binding", "Inst", "Lit", "Node", "Candidate", "Limits", "Expected",
       "Request", "TextLiteral", "Body", "ExecutionDefinition", "Submission",
       "Interface", "Derivation", "Checked", "Constraint", "Diagnostic", "UnsupportedKind",
       "LimitKind", "Outcome", "Defect", "InstError"],
      [`noble_contracts.KernelBridge.mapListN_toList, `noble_contracts.KernelBridge.mapVec_val,
       `noble_contracts.KernelBridge.toTy_injective, `noble_contracts.KernelBridge.toCandidate_injective,
       `noble_contracts.KernelBridge.toPattern_injective, `noble_contracts.KernelBridge.toLit_injective,
       `noble_contracts.KernelBridge.toTextLiteral_injective, `noble_contracts.KernelBridge.toBody_injective,
       `noble_contracts.KernelBridge.toExecutionDefinition_injective, `noble_contracts.KernelBridge.toSubmission_injective,
       `noble_contracts.KernelBridge.toRequest_injective, `noble_contracts.KernelBridge.check_from_inputs,
       `NobleContractImpl.Projection.lower_body_spec, `NobleContractImpl.Projection.lower_node_refines,
       `NobleContractImpl.Projection.rejects_nonpure_word])
    | .wasm => (`noble_wasm.KernelBridge,
      ["Ty", "Candidate", "Request", "Env", "Outcome", "InstError"],
      [`noble_wasm.KernelBridge.check_from_inputs] ++
        ["Type", "Operation", "Stage", "Error"].flatMap (fun type =>
          let frontend := `noble_wasm.ContractsBridge
          [frontend.mkStr s!"from{type}_to{type}", frontend.mkStr s!"to{type}_from{type}"]))
  return types.flatMap (fun type =>
    [bridge.mkStr s!"from{type}_to{type}", bridge.mkStr s!"to{type}_from{type}"]) ++ additional

elab "check_m4_extraction" laneSyntax:str : command => do
  let laneName := laneSyntax.getString
  let lane : AuditLane ← match laneName with
    | "contracts" => pure .contracts
    | "wasm" => pure .wasm
    | _ => throwError "M4-PACKET: unknown audit lane {laneName}"
  let env := (← getEnv).setExporting false
  let some inputFile ← liftIO (IO.getEnv "NOBLE_M4_EXTRACTION_SUBJECTS")
    | throwError "M4-PACKET: run verification/m4/implementation.mjs"
  let text ← liftIO (IO.FS.readFile inputFile)
  let packet ← match Json.parse text with
    | .ok packet => pure packet
    | .error error => throwError "M4-PACKET: {error}"
  unless packet.getObjValAs? String "schema" == .ok "m4-extraction-audit/v1" do
    throwError "M4-PACKET: unreviewed schema"
  unless (← field packet "lane") == laneName do
    throwError "M4-PACKET: audit lane does not match command lane {laneName}"
  let declarations ← entries packet "declarations"
  let boundaries ← entries packet "boundaries"
  let retainedModels ← entries packet "retained_models"
  let bridges ← entries packet "bridges"
  let roots ← entries packet "roots"
  let mut seen : NameSet := {}
  for item in declarations do
    let declName := (← field item "name").toName
    if seen.contains declName then throwError "M4-PACKET: duplicate declaration {declName}"
    seen := seen.insert declName
    let owner := (← field item "owner").toName
    unless generatedModule owner && definingModule env declName == owner do
      throwError "M4-OWNER: local Rust body/type replaced by another owner: {declName}"
    if item.getObjValAs? Bool "transparent" == .ok true then
      let _ ← transparent env declName
      pure ()
    else
      let _ ← declaration env declName
      pure ()
  for item in boundaries ++ retainedModels do
    let declName := (← field item "name").toName
    modelDeclaration env declName (item.getObjValAs? String "kind" == .ok "types")
    unless (modelModules (← field item "lane")).contains (definingModule env declName) do
      throwError "M4-BOUNDARY-OWNER: {declName}"
  for bridge in bridges do
    let model := (← field bridge "model").toName
    let actual := (← field bridge "actual").toName
    let _ ← transparent env actual
    -- Transparent wrappers must actually depend on the separately extracted
    -- body, not on a handwritten copy of its desired outcome.
    unless (← reachable env model).contains actual do
      throwError "M4-BRIDGE: {model} does not depend on actual Rust {actual}"

  let mut rootReachable : NameSet := {}
  let mut rootRecords : Array Json := #[]
  for root in roots do
    let declName := (← field root "declaration").toName
    let owner := (← field root "owner").toName
    unless seen.contains declName && definingModule env declName == owner do
      throwError "M4-ROOT: missing/substituted implementation root {declName}"
    let closure ← reachable env declName
    let isType := root.getObjValAs? String "kind" == .ok "types"
    if !isType then
      let _ ← transparent env declName
      if root.getObjValAs? Bool "requires_acceptance" == .ok true ||
          root.getObjValAs? String "method" == .ok "prepare" ||
          root.getObjValAs? String "rust" == .ok "noble_wasm::compile" then
        unless closure.contains ``noble_kernel.acceptance.check do
          throwError "M4-ACCEPTANCE-DEPENDENCY: {declName} bypasses the actual extracted checker"
    for dependency in closure do rootReachable := rootReachable.insert dependency
    let axioms ← auditAxioms env declName false
    let modules := closure.foldl (fun modules dependency =>
      modules.insert (definingModule env dependency)) ({} : NameSet)
    rootRecords := rootRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson owner.toString),
      ("axioms", toJson (axioms.map Name.toString)),
      ("reachable_declarations", toJson closure.size),
      ("reachable_modules", toJson ((modules.toArray.qsort Name.lt).map Name.toString)),
      ("reachable_project_declarations", toJson ((closure.filter (fun declName =>
        projectModule (definingModule env declName) || projectName declName)).map Name.toString))])

  let mut strictRecords : Array Json := #[]
  for declName in strictTheorems lane do
    match ← declaration env declName with
    | .thmInfo _ => pure ()
    | _ => throwError "M4-STRICT-THEOREM: required inherited theorem absent: {declName}"
    let axioms ← auditAxioms env declName true
    strictRecords := strictRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("axioms", toJson (axioms.map Name.toString))])
  let subjects : List (Name × List Name) := match lane with
    | .contracts => [
      (`noble_contracts.KernelBridge.check_from_inputs,
        [`noble_contracts.noble_kernel.acceptance.check, ``noble_kernel.acceptance.check]),
      (`NobleContractImpl.Projection.lower_body_spec, [`noble_contracts.wire.lower_body]),
      (`NobleContractImpl.Projection.lower_node_refines, [`noble_contracts.wire.lower_node])]
    | .wasm => [
      (`noble_wasm.KernelBridge.check_from_inputs,
        [`noble_wasm.noble_kernel.acceptance.check, ``noble_kernel.acceptance.check])]
  for (declName, dependencies) in subjects do
    requireConstants declName (← declaration env declName).type dependencies
  let (bridge, types) : Name × List String := match lane with
    | .contracts => (`noble_contracts.KernelBridge,
      ["Ty", "Candidate", "Request", "Env", "Outcome", "TextLiteral", "Body", "ExecutionDefinition", "Submission"])
    | .wasm => (`noble_wasm.KernelBridge, ["Ty", "Candidate", "Request", "Env", "Outcome", "InstError"])
  for type in types do
    for declName in [bridge.mkStr s!"from{type}_to{type}", bridge.mkStr s!"to{type}_from{type}"] do
      requireConstants declName (← declaration env declName).type
        [bridge.mkStr s!"from{type}", bridge.mkStr s!"to{type}"]
  if laneName == "wasm" then
    let frontend := `noble_wasm.ContractsBridge
    for type in ["Type", "Operation", "Stage", "Error"] do
      for declName in [frontend.mkStr s!"from{type}_to{type}", frontend.mkStr s!"to{type}_from{type}"] do
        requireConstants declName (← declaration env declName).type
          [frontend.mkStr s!"from{type}", frontend.mkStr s!"to{type}"]

  -- Audit unused project declarations too: dead admitted models or theorems
  -- cannot hide behind a successful prepare dependency closure.
  let owned := env.constants.fold (init := (#[] : Array Name)) fun names declName _ =>
    if projectModule (definingModule env declName) || projectName declName then names.push declName else names
  let mut ownedRecords : Array Json := #[]
  let mut native : NameSet := {}
  let mut abstractTypes : NameSet := {}
  for declName in owned.qsort Name.lt do
    let info ← declaration env declName
    let owner := definingModule env declName
    if let .axiomInfo _ := info then
      unless nativeStringBound env declName do throwError "M4-AXIOM: new project/model axiom {declName}"
    if let .opaqueInfo _ := info then throwError "M4-TRANSPARENT: opaque project/model declaration {declName}"
    let stringProof := nativeStringProof env declName
    let strict := !(generatedModule owner) && !stringProof &&
      (match info with | .thmInfo _ => true | _ => false)
    let axioms ← auditAxioms env declName strict (!rootReachable.contains declName)
    for axiomName in axioms do
      if nativeStringBound env axiomName then native := native.insert axiomName
      else if abstractFormatter env axiomName then abstractTypes := abstractTypes.insert axiomName
    ownedRecords := ownedRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson owner.toString),
      ("kind", toJson (kind info)), ("strict_theorem", toJson strict),
      ("native_string_obligation", toJson stringProof),
      ("reachable_from_roots", toJson (rootReachable.contains declName)),
      ("axioms", toJson (axioms.map Name.toString))])
  let mut nativeRecords : Array Json := #[]
  for declName in native.toArray.qsort Name.lt do
    let info ← declaration env declName
    let rendered ← liftTermElabM (Meta.ppExpr info.type)
    nativeRecords := nativeRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson (definingModule env declName).toString),
      ("type", toJson rendered.pretty),
      ("classification", toJson "generated/inherited literal UTF-8 byte-array size evaluation; not strict")])
  for declName in abstractTypes.toArray do
    if rootReachable.contains declName then throwError "M4-FORMATTER: abstract formatting state entered a required root"
  logInfo <| "M4-EXTRACTION " ++ (Json.mkObj [
    ("schema", toJson "m4-extraction-audit/v1"), ("result", toJson "passed"),
    ("lane", toJson laneName),
    ("declarations", toJson declarations), ("boundaries", toJson boundaries), ("bridges", toJson bridges),
    ("retained_models", toJson retainedModels),
    ("strict_axioms", toJson (strictAxioms.map Name.toString)),
    ("required_strict_theorems", toJson strictRecords), ("roots", toJson roots),
    ("root_audits", toJson rootRecords),
    ("project_declarations", toJson ownedRecords), ("native_string_obligations", toJson nativeRecords),
    ("abstract_standard_types", toJson ((abstractTypes.toArray.qsort Name.lt).map Name.toString)),
    ("scope", toJson "actual-Rust extraction and logical dependency audit; no universal frontend, session, lowering or Wasm refinement")]).compress

end M4Audit
