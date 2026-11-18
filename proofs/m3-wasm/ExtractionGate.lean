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
    `NobleWasmImpl.KernelBridge, `NobleWasmImpl.ContractsBridge,
    `NobleWasmImpl.FunsExternal ].contains declName

/-- The generated modules whose string literals the pinned `toStr` sizes with
native evaluation; the same owners `proofs/m4/M4Audit.lean` reviews. -/
private def nativeOwner (env : Environment) (decl : Name) : Bool :=
  [ `NobleKernel.Funs, `NobleKernel.FunsExternal, `NobleContractImpl.Funs,
    `NobleWasmImpl.Funs ].contains (definingModule env decl)

/-- The complete proposition shape of a literal UTF-8 byte-array size bound:
`decide ((String.toByteArray <string literal>).size ≤ Aeneas.Std.U32.max) = true`. -/
private def literalSizeShape (type : Expr) : Bool := Id.run do
  let type := type.consumeMData
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

/-- Only the generated/inherited `toStr` literal byte-array bound may use native
evaluation. This is not a namespace-wide axiom whitelist: the complete
proposition shape, literal subject, bound, generated name, and owning module are
all checked; it is `proofs/m4/M4Audit.lean`'s reviewed `nativeStringBound`.
Fresh extraction separately byte-binds the generated Funs modules. -/
private def nativeStringBound (env : Environment) (decl : Name) : Bool := Id.run do
  unless nativeOwner env decl do return false
  match decl with
  | .str (.str (.str _ "_native") "decide") suffix =>
    unless suffix.startsWith "ax_" && suffix.length > 3 &&
        (suffix.toList.drop 3).all Char.isDigit do return false
  | _ => return false
  let some (.axiomInfo info) := env.find? decl | return false
  if info.isUnsafe || !info.levelParams.isEmpty then return false
  return literalSizeShape info.type

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

/-- External Rust types have safe data representations or reducible aliases, as
`proofs/m4/M4Audit.lean` reviews: an inductive representation is not an opaque
type or an axiom permission. Functions and globals stay transparent definitions. -/
private def modelType (env : Environment) (declName : Name) : CommandElabM Unit := do
  match ← declaration env declName with
  | .inductInfo value =>
    if value.isUnsafe then throwError "unsafe external type model: {declName}"
  | _ =>
    let _ ← transparentDefinition env declName
    pure ()

private def requireConstants (subject : Name) (expression : Expr) (required : List Name) :
    CommandElabM Unit := do
  for declName in required do
    unless expression.getUsedConstants.contains declName do
      throwError "substituted extraction subject: {subject} does not reference {declName}"

/-- The audited axioms of `declName`. Strict declarations admit only
`strictAxioms` and the generated literal-size evaluations, which are returned
separately and never count as strict; everything else rejects. -/
private def axiomAudit (env : Environment) (declName : Name) (strict : Bool)
    (allowFormatter : Bool := false) :
    CommandElabM (Array Name × Array Name) := do
  let axioms ← liftCoreM (collectAxioms declName)
  let mut literalSize : Array Name := #[]
  for axiomName in axioms do
    unless strictAxioms.contains axiomName do
      if nativeStringBound env axiomName then
        literalSize := literalSize.push axiomName
      else unless !strict && allowFormatter && abstractFormatter env axiomName do
        let info ← declaration env axiomName
        throwError "disallowed extraction axiom {axiomName} in {declName}: {info.type}"
  return (axioms.filter (fun axiomName => !nativeStringBound env axiomName),
    literalSize.qsort Name.lt)

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

private def textField (record : Json) (key : String) : CommandElabM String := do
  match record.getObjValAs? String key with
  | .ok text =>
    if text.isEmpty then throwError "empty boundary record field: {key}"
    pure text
  | .error message => throwError "invalid boundary record field {key}: {message}"

private def declarationKind : ConstantInfo → String
  | .defnInfo _ => "definition"
  | .thmInfo _ => "theorem"
  | .axiomInfo _ => "axiom"
  | .opaqueInfo _ => "opaque-definition"
  | .inductInfo _ => "inductive"
  | .ctorInfo _ => "constructor"
  | .recInfo _ => "recursor"
  | .quotInfo _ => "quotient"

/-- Negative and positive controls of the literal-size shape, evaluated on every
run so the admission cannot silently widen: only the exact proposition over a
string literal and `Aeneas.Std.U32.max`, decided `true`, is accepted. -/
private def literalSizeControls : List (String × Bool × Expr) :=
  let decision (proposition : Expr) (value : Name) : Expr :=
    mkApp3 (mkConst ``Eq [Level.one]) (mkConst ``Bool)
      (mkApp2 (mkConst ``decide) proposition (mkConst ``instDecidableTrue)) (mkConst value)
  let sizeOf (text : Expr) : Expr :=
    mkApp (mkConst ``ByteArray.size) (mkApp (mkConst ``String.toByteArray) text)
  let bounded (text : Expr) (bound : Name) : Expr :=
    mkApp4 (mkConst ``LE.le [Level.zero]) (mkConst ``Nat) (mkConst ``instLENat) (sizeOf text)
      (mkConst bound)
  let literal := mkStrLit "m3-wasm literal-size control"
  [ ("generated literal against U32.max", true, decision (bounded literal ``Aeneas.Std.U32.max) ``Bool.true),
    ("bound other than U32.max", false, decision (bounded literal ``Aeneas.Std.U64.max) ``Bool.true),
    ("non-literal string subject", false,
      decision (bounded (mkApp2 (mkConst ``String.append) literal literal) ``Aeneas.Std.U32.max)
        ``Bool.true),
    ("decision equated to false", false, decision (bounded literal ``Aeneas.Std.U32.max) ``Bool.false),
    ("bare bound without decide", false,
      mkApp3 (mkConst ``Eq [Level.one]) (mkSort Level.zero) (bounded literal ``Aeneas.Std.U32.max)
        (mkConst ``True)) ]

elab "check_m3_extraction" : command => do
  let env := (← getEnv).setExporting false
  let some inputFile ← liftIO (IO.getEnv "NOBLE_M3_EXTRACTION_SUBJECTS")
    | throwError "run verification/m3-wasm/implementation.mjs to supply fresh translation subjects"
  let text ← liftIO (IO.FS.readFile inputFile)
  let packet ← match Json.parse text with
    | .ok value => pure value
    | .error message => throwError "invalid extraction subject packet: {message}"
  unless packet.getObjValAs? String "schema" == .ok "m3-extraction-audit/v2" do
    throwError "unreviewed extraction subject schema"

  let mut controlRecords : Array Json := #[]
  for (label, accepted, proposition) in literalSizeControls do
    unless literalSizeShape proposition == accepted do
      throwError "literal-size admission control failed: {label}"
    controlRecords := controlRecords.push (Json.mkObj [
      ("control", toJson label), ("accepted", toJson accepted)])
  for axiomName in strictAxioms do
    if nativeStringBound env axiomName then
      throwError "literal-size admission control failed: {axiomName} admitted"

  let locals ← namesField packet "local_definitions"
  let kernelFunctions ← namesField packet "kernel_boundary_functions"
  let kernelTypes ← namesField packet "kernel_boundary_types"
  let boundaries ← match packet.getObjValAs? (Array Json) "boundaries" with
    | .ok rows =>
      if rows.isEmpty then throwError "empty extraction inventory: boundaries"
      pure rows
    | .error message => throwError "invalid extraction inventory boundaries: {message}"
  for text in locals do
    let declName := text.toName
    let _ ← transparentDefinition env declName
    unless definingModule env declName == `NobleWasmImpl.Funs do
      throwError "local Rust body replaced by a model: {declName}"
  let mut boundaryNames : NameSet := {}
  let mut boundaryRecords : Array Json := #[]
  for record in boundaries do
    let declName := (← textField record "name").toName
    let kind ← textField record "kind"
    let model := (← textField record "model").toName
    if boundaryNames.contains declName then throwError "duplicate boundary record: {declName}"
    boundaryNames := boundaryNames.insert declName
    unless modelModule model do throwError "unreviewed boundary model module: {model}"
    match kind with
    | "type" => modelType env declName
    | "function" | "global" | "local-opaque" =>
      let _ ← transparentDefinition env declName
    | _ => throwError "unreviewed boundary kind {kind}: {declName}"
    unless definingModule env declName == model do
      throwError "boundary implemented outside its reviewed model module: {declName}"
    boundaryRecords := boundaryRecords.push record
  for text in kernelFunctions do
    let declName := text.toName
    let _ ← transparentDefinition env declName
    unless definingModule env declName == `NobleKernel.FunsExternal do
      throwError "unreviewed inherited kernel boundary owner: {declName}"
  for text in kernelTypes do
    let declName := text.toName
    modelType env declName
    unless definingModule env declName == `NobleKernel.TypesExternal do
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
  let mut observedNative : NameSet := {}
  for declName in strict do
    let info ← declaration env declName
    match info with
    | .thmInfo _ => pure ()
    | _ => throwError "strict bridge theorem absent: {declName}"
    let (axioms, literalSize) ← axiomAudit env declName true
    for axiomName in literalSize do observedNative := observedNative.insert axiomName
    strictRecords := strictRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("axioms", toJson (axioms.map Name.toString)),
      ("generated_literal_size_evaluation_not_strict", toJson (literalSize.map Name.toString))])
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
  let mut observedAbstractTypes : NameSet := {}
  for declName in roots do
    let closure ← if declName == ``noble_wasm.compile then pure compileReachable else reachable env declName
    let (axioms, literalSize) ← axiomAudit env declName false
    for axiomName in literalSize do observedNative := observedNative.insert axiomName
    let mut modules : NameSet := {}
    for dependency in closure do modules := modules.insert (definingModule env dependency)
    rootRecords := rootRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString),
      ("module", toJson (definingModule env declName).toString),
      ("axioms", toJson ((axioms ++ literalSize).map Name.toString)),
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
    let (axioms, literalSize) ←
      axiomAudit env declName isStrictTheorem (!compileReachable.contains declName)
    for axiomName in literalSize do observedNative := observedNative.insert axiomName
    for axiomName in axioms do
      if abstractFormatter env axiomName then
        observedAbstractTypes := observedAbstractTypes.insert axiomName
    ownedRecords := ownedRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("module", toJson owner.toString),
      ("kind", toJson (declarationKind info)),
      ("reachable_from_compile", toJson (compileReachable.contains declName)),
      ("strict_bridge_theorem", toJson isStrictTheorem),
      ("axioms", toJson ((axioms ++ literalSize).map Name.toString))])
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
    ("schema", toJson "m3-extraction-audit/v2"), ("result", toJson "passed"),
    ("local_definitions", toJson locals), ("boundaries", toJson boundaryRecords),
    ("kernel_boundary_functions", toJson kernelFunctions),
    ("kernel_boundary_types", toJson kernelTypes),
    ("strict_axioms", toJson (strictAxioms.map Name.toString)),
    ("literal_size_controls", toJson controlRecords),
    ("required_strict_theorems", toJson strictRecords),
    ("implementation_subjects", toJson rootRecords),
    ("project_declarations", toJson ownedRecords),
    ("native_evaluation_dependencies", toJson nativeRecords),
    ("abstract_standard_types", toJson abstractTypeRecords),
    ("scope", toJson "actual compiled source dependency/axiom audit and strict representation bridge equations; no lowering/Wasm refinement or runtime result")]).compress

check_m3_extraction
