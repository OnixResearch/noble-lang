import NobleContractImpl.Projection
import NobleContractImpl.Statement
import Lean

open Lean Elab Command

private def strictAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

private def definingModule (env : Environment) (decl : Name) : Name :=
  match env.getModuleIdxFor? decl with
  | some index => env.header.moduleNames[index.toNat]?.getD .anonymous
  | none => .anonymous

/-- `decide ((String.toByteArray <string literal>).size ≤ Aeneas.Std.U32.max) = true`:
the type of the axiom the pinned Aeneas `toStr` leaves for one string literal
(`decide +native`). -/
private def literalSizeType (type : Expr) : Bool := Id.run do
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

/-- M4Audit's reviewed `nativeStringBound` shape, restricted to the generated
kernel modules (identical to `generatedLiteralSizeAxiom` of the M2 and M3
proof gates): a `<decl>._native.decide.ax_<n>` axiom owned by
`NobleKernel.Funs` or `NobleKernel.FunsExternal`, without universe
parameters, whose type is exactly `literalSizeType`. The linked checker
reaches such axioms through string literals of the inherited kernel. They are
admitted on strict theorems only as generated literal-size evaluation and
reported separately; a `_native.decide` axiom of any other type or owner still
refuses. -/
private def generatedLiteralSizeAxiom (env : Environment) (decl : Name) : Bool := Id.run do
  unless [`NobleKernel.Funs, `NobleKernel.FunsExternal].contains (definingModule env decl) do
    return false
  match decl with
  | .str (.str (.str _ "_native") "decide") suffix =>
    unless suffix.startsWith "ax_" && suffix.length > 3 &&
        (suffix.toList.drop 3).all Char.isDigit do return false
  | _ => return false
  let some (.axiomInfo info) := env.find? decl | return false
  if info.isUnsafe || !info.levelParams.isEmpty then return false
  return literalSizeType info.type

/-- Native computation is confined to the actual generated implementation and
its explicitly named source-equation matrix. It never enters the application
library's separate strict gate. Fresh extraction byte-comparison binds Funs. -/
private def permittedNative (env : Environment) (theoremName axiomName : Name) : Bool :=
  let owner := definingModule env axiomName
  (owner == `NobleContractImpl.Funs &&
    axiomName.toString.contains "._native.decide.ax_") ||
  generatedLiteralSizeAxiom env axiomName ||
  (owner == `NobleContractImpl.Statement &&
    (axiomName.toString.startsWith (theoremName.toString ++ "._native.decide.ax_") ||
     axiomName.toString.startsWith (theoremName.toString ++ "._native.native_decide.ax_")))

private def theoremInfo (decl : Name) : CommandElabM ConstantInfo := do
  match (← getEnv).find? decl with
  | some info@(.thmInfo _) => pure info
  | _ => throwError "required implementation theorem absent: {decl}"

private def stringValue (decl : Name) : CommandElabM String := do
  match (← getEnv).find? decl with
  | some (.defnInfo info) =>
    match info.value with
    | .lit (.strVal text) => pure text
    | _ => throwError "retained source/statement is not a literal: {decl}"
  | _ => throwError "retained source/statement absent: {decl}"

private def requireConstants (decl : Name) (type : Expr) (required : List Name) :
    CommandElabM Unit := do
  for source in required do
    unless type.getUsedConstants.contains source do
      throwError "substituted implementation subject: {decl} does not bind {source}"

/-- The declarations of one imported module. -/
private def moduleConstants (env : Environment) (module : Name) : Array Name :=
  match env.getModuleIdx? module with
  | some index => (env.header.moduleData[index.toNat]?.map (·.constNames)).getD #[]
  | none => #[]

/-- A generated literal-size axiom of `module`: the right name shape and type. -/
private def literalSizeAxiomOf (env : Environment) (module : Name) : Option Name :=
  (moduleConstants env module).find? fun decl =>
    decl.toString.contains "._native.decide.ax_" &&
      match env.find? decl with
      | some (.axiomInfo info) => literalSizeType info.type
      | _ => false

/-- Negative controls of the literal-size admission, on actual declarations:
an admitted kernel axiom; its type with the literal replaced by a defined
string, or with the `U32.max` bound replaced by `U64.max`, refused by the type
check; and an axiom of the same shape owned by the contracts subject refused
by the owner check. No axiom is declared for them. -/
private def literalSizeControls : CommandElabM Json := do
  let env ← getEnv
  let some kernel := literalSizeAxiomOf env `NobleKernel.Funs
    | throwError "literal-size control: no generated kernel literal-size axiom"
  unless generatedLiteralSizeAxiom env kernel do
    throwError "literal-size control: kernel axiom {kernel} not admitted"
  let some (.axiomInfo info) := env.find? kernel
    | throwError "literal-size control: {kernel} is not an axiom"
  let nonLiteral := info.type.replace fun expression =>
    match expression with
    | .lit (.strVal _) => some (mkConst ``NobleContractImpl.Statement.incrementSource)
    | _ => none
  let wrongBound := info.type.replace fun expression =>
    if expression.isConstOf ``Aeneas.Std.U32.max then some (mkConst ``Aeneas.Std.U64.max)
    else none
  if literalSizeType nonLiteral then
    throwError "literal-size control: a non-literal string is admitted"
  if literalSizeType wrongBound then
    throwError "literal-size control: a U64.max bound is admitted"
  let some foreign := literalSizeAxiomOf env `NobleContractImpl.Funs
    | throwError "literal-size control: no contracts literal-size axiom"
  if generatedLiteralSizeAxiom env foreign then
    throwError "literal-size control: contracts-owned axiom {foreign} admitted"
  pure <| Json.mkObj [
    ("admitted_kernel_axiom", toJson kernel.toString),
    ("refused_non_literal_type", toJson true), ("refused_u64_bound_type", toJson true),
    ("refused_contracts_owner", toJson foreign.toString)]

elab "check_contract_implementation" : command => do
  let env ← getEnv
  let bridgeTypes := ["VariableKind", "Ty", "Pattern", "EffectSlot", "Scheme",
    "Behavior", "SchemaDecl", "Env", "Binding", "Inst", "Lit", "Node",
    "Candidate", "Limits", "Expected", "Request", "Interface", "Derivation",
    "Checked", "Constraint", "Diagnostic", "UnsupportedKind", "LimitKind",
    "Outcome", "Defect", "InstError", "GenericVariantDecl", "BoundClockRegistration",
    "BoundCodeExportRegistration", "BoundSendRegistration", "LiveSite"]
  let bridge := `noble_contracts.KernelBridge
  let strict := bridgeTypes.flatMap (fun kind =>
      [bridge.mkStr s!"from{kind}_to{kind}", bridge.mkStr s!"to{kind}_from{kind}"]) ++
    [``noble_contracts.KernelBridge.mapListN_toList,
     ``noble_contracts.KernelBridge.mapVec_val,
     ``noble_contracts.KernelBridge.toTy_injective,
     ``noble_contracts.KernelBridge.toCandidate_injective,
     ``noble_contracts.KernelBridge.toRequest_injective,
     ``noble_contracts.KernelBridge.check_from_inputs,
     ``NobleContractImpl.Projection.lower_body_spec,
     ``NobleContractImpl.Projection.lower_node_refines,
     ``NobleContractImpl.Projection.rejects_nonpure_word]
  let mut literalSize : List Name := []
  for decl in strict do
    let _ ← theoremInfo decl
    let mut reached : List Name := []
    for axiomName in ← liftCoreM (collectAxioms decl) do
      unless strictAxioms.contains axiomName do
        unless generatedLiteralSizeAxiom env axiomName do
          throwError "disallowed strict implementation axiom {axiomName} in {decl}"
        reached := axiomName :: reached
        unless literalSize.contains axiomName do
          literalSize := axiomName :: literalSize
    unless reached.isEmpty do
      logInfo <| "MC1-LITERAL-SIZE " ++ (Json.mkObj [
        ("theorem", toJson decl.toString),
        ("axioms", toJson ((reached.map Name.toString).mergeSort (· ≤ ·))),
        ("classification", toJson "generated literal-size evaluation (not strict)")]).compress
  let controls ← literalSizeControls
  for (decl, sources) in [
      (``NobleContractImpl.Projection.lower_body_spec, [``noble_contracts.wire.lower_body]),
      (``NobleContractImpl.Projection.lower_node_refines, [``noble_contracts.wire.lower_node]),
      (``noble_contracts.KernelBridge.check_from_inputs,
        [``noble_contracts.noble_kernel.acceptance.check, ``noble_kernel.acceptance.check])] do
    requireConstants decl (← theoremInfo decl).type sources
  let cases := ["increment", "composed", "family", "structural", "syntax", "wrap"]
  let mut observedNative : List Name := []
  for label in cases do
    let space := `NobleContractImpl.Statement
    let decl := space.mkStr (label ++ "_source_statement")
    let info ← theoremInfo decl
    requireConstants decl info.type [``noble_contracts.prepare, ``noble_contracts.rendering.export_lean]
    for axiomName in ← liftCoreM (collectAxioms decl) do
      unless strictAxioms.contains axiomName do
        unless permittedNative env decl axiomName do
          throwError "disallowed source-equation axiom {axiomName} in {decl}"
        unless observedNative.contains axiomName do
          observedNative := axiomName :: observedNative
    let source ← stringValue (space.mkStr (label ++ "Source"))
    let statement ← stringValue (space.mkStr (label ++ "Statement"))
    let fixture ← liftIO <| IO.FS.readFile s!"../../verification/mc1/{label}.noble-contract"
    unless source == fixture do
      throwError "source-equation fixture drift: {label}"
    logInfo <| "MC1-SOURCE-EQUATION " ++ (Json.mkObj [
      ("case", toJson label), ("theorem", toJson decl.toString),
      ("source", toJson source), ("statement", toJson statement)]).compress
  let names := observedNative.map Name.toString |>.mergeSort (· ≤ ·)
  logInfo <| "MC1-IMPLEMENTATION " ++ (Json.mkObj [
    ("strict_theorems", toJson strict.length), ("source_equations", toJson cases.length),
    ("strict_axioms", toJson (strictAxioms.map Name.toString)),
    ("literal_size_axioms", toJson ((literalSize.map Name.toString).mergeSort (· ≤ ·))),
    ("literal_size_controls", controls),
    ("native_axioms", toJson names),
    ("scope", toJson "mc1-statement-export-v1; not universal frontend correctness")]).compress

check_contract_implementation
