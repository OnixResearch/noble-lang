import NobleContractImpl.Projection
import NobleContractImpl.Statement
import Lean

open Lean Elab Command

private def strictAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

private def definingModule (env : Environment) (decl : Name) : Name :=
  match env.getModuleIdxFor? decl with
  | some index => env.header.moduleNames[index.toNat]?.getD .anonymous
  | none => .anonymous

/-- Native computation is confined to the actual generated implementation and
its explicitly named source-equation matrix. It never enters the application
library's separate strict gate. Fresh extraction byte-comparison binds Funs. -/
private def permittedNative (env : Environment) (theoremName axiomName : Name) : Bool :=
  let owner := definingModule env axiomName
  (owner == `NobleContractImpl.Funs &&
    axiomName.toString.contains "._native.decide.ax_") ||
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

elab "check_contract_implementation" : command => do
  let env ← getEnv
  let bridgeTypes := ["VariableKind", "Ty", "Pattern", "EffectSlot", "Scheme",
    "Behavior", "SchemaDecl", "Env", "Binding", "Inst", "Lit", "Node",
    "Candidate", "Limits", "Expected", "Request", "Interface", "Derivation",
    "Checked", "Constraint", "Diagnostic", "UnsupportedKind", "LimitKind",
    "Outcome", "Defect", "InstError"]
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
  for decl in strict do
    let _ ← theoremInfo decl
    for axiomName in ← liftCoreM (collectAxioms decl) do
      unless strictAxioms.contains axiomName do
        throwError "disallowed strict implementation axiom {axiomName} in {decl}"
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
    ("native_axioms", toJson names),
    ("scope", toJson "mc1-statement-export-v1; not universal frontend correctness")]).compress

check_contract_implementation
