import NobleWasmImpl
import NobleWasmImpl.KernelBridge
import Lean

open Lean Elab Command

set_option maxHeartbeats 1000000
set_option maxRecDepth 2048

private def strictAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

private def definingModule (env : Environment) (decl : Name) : Name :=
  match env.getModuleIdxFor? decl with
  | some index => env.header.moduleNames[index.toNat]?.getD .anonymous
  | none => .anonymous

private def projectModule (declName : Name) : Bool :=
  declName.toString.startsWith "NobleWasmImpl." || declName.toString.startsWith "NobleKernel."

private def modelModule (declName : Name) : Bool :=
  [ `NobleWasmImpl.TypesExternal, `NobleWasmImpl.StdModels,
    `NobleWasmImpl.KernelBridge, `NobleWasmImpl.FunsExternal ].contains declName

/-- Only the inherited `toStr` literal byte-array bound may use native evaluation.
This is not a namespace-wide axiom whitelist: the complete proposition shape,
literal subject, bound, generated name, and owning module are all checked.
Fresh extraction separately byte-binds both generated Funs modules. -/
private def nativeStringBound (env : Environment) (decl : Name) : Bool := Id.run do
  unless [ `NobleKernel.Funs, `NobleKernel.FunsExternal, `NobleWasmImpl.Funs ].contains
      (definingModule env decl) do return false
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
  let proposition := decision.getAppArgs[0]!.consumeMData
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

/-- The pinned library leaves formatting state abstract. This type-only boundary
is allowed solely outside `compile` and is never a strict theorem assumption. -/
private def abstractFormatter (env : Environment) (decl : Name) : Bool :=
  decl == ``Aeneas.Std.core.fmt.Formatter &&
  definingModule env decl == `Aeneas.Std.Core.Fmt &&
  match env.find? decl with
  | some (.axiomInfo info) =>
    !info.isUnsafe && info.levelParams.isEmpty && info.type == mkSort (.succ .zero)
  | _ => false

private def declaration (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  match env.find? declName with
  | some info => pure info
  | none => throwError "required extraction declaration absent: {declName}"

private def transparentDefinition (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  match ← declaration env declName with
  | info@(.defnInfo value) =>
    unless value.safety == .safe do
      throwError "unsafe/partial replacement for extracted definition: {declName}"
    pure info
  | _ => throwError "required transparent extraction definition absent: {declName}"

private def requireConstants (subject : Name) (expression : Expr) (required : List Name) :
    CommandElabM Unit := do
  for declName in required do
    unless expression.getUsedConstants.contains declName do
      throwError "substituted extraction subject: {subject} does not reference {declName}"

private def axiomAudit (env : Environment) (declName : Name) (strict : Bool)
    (allowFormatter : Bool := false) :
    CommandElabM (Array Name) := do
  let axioms ← liftCoreM (collectAxioms declName)
  for axiomName in axioms do
    unless strictAxioms.contains axiomName do
      unless !strict && (nativeStringBound env axiomName ||
          (allowFormatter && abstractFormatter env axiomName)) do
        let info ← declaration env axiomName
        throwError "disallowed extraction axiom {axiomName} in {declName}: {info.type}"
  return axioms

/-- Inspect both types and values, including theorem/opaque values and inductive
constructors. This records logical dependencies, not dynamic execution coverage. -/
private def reachable (env : Environment) (root : Name) : CommandElabM (Array Name) := do
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
      if let .inductInfo value := info then
        pending := pending ++ value.ctors.toArray
  return seen.toArray.qsort Name.lt

private def namesField (packet : Json) (key : String) : CommandElabM (Array String) := do
  match packet.getObjValAs? (Array String) key with
  | .ok names =>
    if names.isEmpty then throwError "empty extraction inventory: {key}"
    let mut seen : NameSet := {}
    for text in names do
      let declName := text.toName
      if text.isEmpty || seen.contains declName then
        throwError "invalid/duplicate extraction inventory name in {key}: {text}"
      seen := seen.insert declName
    return names
  | .error message => throwError "invalid extraction inventory {key}: {message}"

private def declarationKind : ConstantInfo → String
  | .defnInfo _ => "definition"
  | .thmInfo _ => "theorem"
  | .axiomInfo _ => "axiom"
  | .opaqueInfo _ => "opaque-definition"
  | .inductInfo _ => "inductive"
  | .ctorInfo _ => "constructor"
  | .recInfo _ => "recursor"
  | .quotInfo _ => "quotient"

elab "check_m3_extraction" : command => do
  let env := (← getEnv).setExporting false
  let some inputFile ← liftIO (IO.getEnv "NOBLE_M3_EXTRACTION_SUBJECTS")
    | throwError "run verification/m3-wasm/implementation.mjs to supply fresh translation subjects"
  let text ← liftIO (IO.FS.readFile inputFile)
  let packet ← match Json.parse text with
    | .ok value => pure value
    | .error message => throwError "invalid extraction subject packet: {message}"
  unless packet.getObjValAs? String "schema" == .ok "m3-extraction-audit/v1" do
    throwError "unreviewed extraction subject schema"
  let locals ← namesField packet "local_definitions"
  let externals ← namesField packet "external_functions"
  let externalTypes ← namesField packet "external_types"
  let kernelBoundary ← namesField packet "kernel_boundary"
  for text in locals do
    let declName := text.toName
    let _ ← transparentDefinition env declName
    unless definingModule env declName == `NobleWasmImpl.Funs do
      throwError "local Rust body replaced by a model: {declName}"
  for text in externals ++ externalTypes do
    let declName := text.toName
    let _ ← transparentDefinition env declName
    unless modelModule (definingModule env declName) do
      throwError "unreviewed emitter boundary implementation owner: {declName}"
  for text in kernelBoundary do
    let declName := text.toName
    let _ ← transparentDefinition env declName
    unless [ `NobleKernel.FunsExternal, `NobleKernel.TypesExternal ].contains
        (definingModule env declName) do
      throwError "unreviewed inherited kernel boundary owner: {declName}"

  -- These names are deliberately the stable source entry points, not private
  -- Rust class/loop names. Every other local declaration comes from translation.json.
  let roots : List Name := [
    ``noble_wasm.compile, ``noble_wasm.admission.check,
    ``noble_wasm.lowering.lower, ``noble_wasm.signatures.walk.encode,
    ``noble_wasm.emit.module, ``noble_kernel.acceptance.check ]
  for declName in roots do
    let _ ← transparentDefinition env declName
    let expected := if declName == ``noble_kernel.acceptance.check then `NobleKernel.Funs
      else `NobleWasmImpl.Funs
    unless definingModule env declName == expected do
      throwError "substituted implementation owner: {declName}"
  requireConstants ``noble_wasm.compile
    (← transparentDefinition env ``noble_wasm.compile).value!
    [``noble_wasm.admission.check, ``noble_wasm.lowering.lower, ``noble_wasm.emit.module]
  requireConstants ``noble_wasm.admission.check
    (← transparentDefinition env ``noble_wasm.admission.check).value!
    [``noble_wasm.noble_kernel.acceptance.check, ``noble_wasm.noble_kernel.contracts.environment]
  requireConstants ``noble_wasm.lowering.lower
    (← transparentDefinition env ``noble_wasm.lowering.lower).type
    [``noble_wasm.noble_kernel.untrusted.Checked]
  for (model, actual) in [
      (``noble_wasm.noble_kernel.acceptance.check, ``noble_kernel.acceptance.check),
      (``noble_wasm.noble_kernel.contracts.environment, ``noble_kernel.contracts.environment),
      (``noble_wasm.noble_kernel.types.Ty.Insts.CoreCloneClone.clone,
        ``noble_kernel.types.Ty.Insts.CoreCloneClone.clone),
      (``noble_wasm.noble_kernel.types.EffSet.is_empty, ``noble_kernel.types.EffSet.is_empty)] do
    requireConstants model (← transparentDefinition env model).value! [actual]

  let bridge := `noble_wasm.KernelBridge
  let strict := ["Ty", "Candidate", "Request", "Env", "Outcome"].flatMap (fun kind =>
    [bridge.mkStr s!"from{kind}_to{kind}", bridge.mkStr s!"to{kind}_from{kind}"]) ++
    [``noble_wasm.KernelBridge.check_from_inputs]
  let mut strictRecords : Array Json := #[]
  for declName in strict do
    let info ← declaration env declName
    match info with
    | .thmInfo _ => pure ()
    | _ => throwError "strict bridge theorem absent: {declName}"
    let axioms ← axiomAudit env declName true
    strictRecords := strictRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("axioms", toJson (axioms.map Name.toString))])
  for kind in ["Ty", "Candidate", "Request", "Env", "Outcome"] do
    for theoremName in [bridge.mkStr s!"from{kind}_to{kind}", bridge.mkStr s!"to{kind}_from{kind}"] do
      requireConstants theoremName (← declaration env theoremName).type
        [bridge.mkStr s!"from{kind}", bridge.mkStr s!"to{kind}"]
  requireConstants ``noble_wasm.KernelBridge.check_from_inputs
    (← declaration env ``noble_wasm.KernelBridge.check_from_inputs).type
    [``noble_wasm.noble_kernel.acceptance.check, ``noble_kernel.acceptance.check]

  let compileReachable ← reachable env ``noble_wasm.compile
  for declName in roots do
    unless compileReachable.contains declName do
      throwError "required implementation dependency absent from actual compile: {declName}"
  let mut rootRecords : Array Json := #[]
  let mut observedNative : NameSet := {}
  let mut observedAbstractTypes : NameSet := {}
  for declName in roots do
    let closure ← if declName == ``noble_wasm.compile then pure compileReachable else reachable env declName
    let axioms ← axiomAudit env declName false
    for axiomName in axioms do
      unless strictAxioms.contains axiomName do observedNative := observedNative.insert axiomName
    let mut modules : NameSet := {}
    for dependency in closure do modules := modules.insert (definingModule env dependency)
    rootRecords := rootRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString),
      ("module", toJson (definingModule env declName).toString),
      ("axioms", toJson (axioms.map Name.toString)),
      ("reachable_declarations", toJson closure.size),
      ("reachable_modules", toJson ((modules.toArray.qsort Name.lt).map Name.toString)),
      ("reachable_project_declarations", toJson
        ((closure.filter (fun decl => projectModule (definingModule env decl))).map Name.toString))])

  -- Also inspect declarations outside compile's dependency closure, so an unused
  -- admitted model or theorem cannot hide behind the successful entrypoint audit.
  let owned := env.constants.fold (init := (#[] : Array Name)) fun names declName _ =>
    if projectModule (definingModule env declName) then names.push declName else names
  let mut ownedRecords : Array Json := #[]
  for declName in owned.qsort Name.lt do
    let info ← declaration env declName
    let owner := definingModule env declName
    let isStrictTheorem := owner == `NobleWasmImpl.KernelBridge &&
      match info with | .thmInfo _ => true | _ => false
    if let .axiomInfo _ := info then
      unless nativeStringBound env declName do throwError "new project/model axiom: {declName}"
    let axioms ← axiomAudit env declName isStrictTheorem (!compileReachable.contains declName)
    for axiomName in axioms do
      if nativeStringBound env axiomName then
        observedNative := observedNative.insert axiomName
      else if abstractFormatter env axiomName then
        observedAbstractTypes := observedAbstractTypes.insert axiomName
    ownedRecords := ownedRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("module", toJson owner.toString),
      ("kind", toJson (declarationKind info)),
      ("reachable_from_compile", toJson (compileReachable.contains declName)),
      ("strict_bridge_theorem", toJson isStrictTheorem),
      ("axioms", toJson (axioms.map Name.toString))])
  let mut nativeRecords : Array Json := #[]
  for declName in observedNative.toArray.qsort Name.lt do
    let info ← declaration env declName
    let rendered ← liftTermElabM (Meta.ppExpr info.type)
    nativeRecords := nativeRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString),
      ("module", toJson (definingModule env declName).toString),
      ("type", toJson rendered.pretty),
      ("reachable_from_compile", toJson (compileReachable.contains declName)),
      ("classification", toJson "generated/inherited literal UTF-8 byte-array size evaluation; not strict")])
  let mut abstractTypeRecords : Array Json := #[]
  for declName in observedAbstractTypes.toArray.qsort Name.lt do
    unless !compileReachable.contains declName do
      throwError "abstract formatter entered the actual compile dependency graph"
    abstractTypeRecords := abstractTypeRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString),
      ("module", toJson (definingModule env declName).toString),
      ("type", toJson "Type"), ("reachable_from_compile", toJson false),
      ("classification", toJson "pinned abstract formatting-state type; no formatting fidelity or strict proof claim")])
  logInfo <| "M3-EXTRACTION " ++ (Json.mkObj [
    ("schema", toJson "m3-extraction-audit/v1"), ("result", toJson "passed"),
    ("local_definitions", toJson locals), ("external_functions", toJson externals),
    ("external_types", toJson externalTypes), ("kernel_boundary", toJson kernelBoundary),
    ("strict_axioms", toJson (strictAxioms.map Name.toString)),
    ("required_strict_theorems", toJson strictRecords),
    ("implementation_subjects", toJson rootRecords),
    ("project_declarations", toJson ownedRecords),
    ("native_evaluation_dependencies", toJson nativeRecords),
    ("abstract_standard_types", toJson abstractTypeRecords),
    ("scope", toJson "actual compiled source dependency/axiom audit and strict representation bridge equations; no lowering/Wasm refinement or runtime result")]).compress

check_m3_extraction
