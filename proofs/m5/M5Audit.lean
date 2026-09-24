import M5Resources
import M4Audit
import Lean

open Lean Elab Command
open Aeneas Aeneas.Std noble_kernel

set_option Aeneas.customDoElab false
set_option maxHeartbeats 2000000
set_option maxRecDepth 4096

namespace M5Audit

-- These complete propositions prevent a finite instance, extra premise or
-- weaker result from satisfying a root merely by mentioning its dependencies.
private def transitionContract : Prop :=
  ∀ (record : resources.Snapshot) (claim : resources.Handle) (event : resources.Event),
    resources.transition.transition record claim event = Result.ok (M5Resources.step record claim event)

private def tableContract : Prop :=
  ∀ (table : resources.table.Table) (claim : resources.Handle) (event : resources.Event),
    resources.table.Table.decide table claim event = Result.ok (M5Resources.tableStep table claim event)

private def cancellationContract : Prop :=
  ∀ (record : resources.Snapshot) (serial : U64) (reason : resources.Retirement),
    M5Resources.valid record = true → record.state = .Busy serial →
    M5Resources.scopeCheck record serial = .Ok () →
    resources.transition.transition record record.handle (.Revoke serial reason) =
      Result.ok (.Ok ⟨{ record with state := .Retiring serial, retirement := some reason }, .AccessRevoked⟩) ∧
    resources.Decision.accounting
      ⟨{ record with state := .Retiring serial, retirement := some reason }, .AccessRevoked⟩ =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 0#usize⟩

private def lateCompletionContract : Prop :=
  ∀ (record : resources.Snapshot) (serial : U64) (outcome : resources.Completion),
    M5Resources.valid record = true → record.state = .Retiring serial →
    M5Resources.scopeCheck record serial = .Ok () →
    resources.transition.transition record record.handle (.Complete serial outcome) =
      Result.ok (.Ok ⟨{ record with state := .Retired }, .RetirementCompleted⟩) ∧
    resources.Decision.accounting ⟨{ record with state := .Retired }, .RetirementCompleted⟩ =
      Result.ok ⟨0#usize, 1#usize, 0#usize, 0#usize, 1#usize⟩

private def retiredCompletionContract : Prop :=
  ∀ (record : resources.Snapshot) (serial : U64) (outcome : resources.Completion),
    M5Resources.valid record = true → record.state = .Retired →
    M5Resources.scopeCheck record serial = .Ok () →
    resources.transition.transition record record.handle (.Complete serial outcome) =
      Result.ok (.Ok (M5Resources.unchanged record)) ∧
    resources.Decision.accounting (M5Resources.unchanged record) =
      Result.ok ⟨0#usize, 0#usize, 0#usize, 0#usize, 0#usize⟩

private def normalCompletionContract : Prop :=
  ∀ (record : resources.Snapshot) (serial : U64) (outcome : resources.Completion),
    M5Resources.valid record = true → record.state = .Busy serial →
    M5Resources.scopeCheck record serial = .Ok () →
    resources.transition.transition record record.handle (.Complete serial outcome) =
      Result.ok (.Ok ⟨{ record with state := .Live }, .OwnerReturned outcome⟩) ∧
    resources.Decision.accounting ⟨{ record with state := .Live }, .OwnerReturned outcome⟩ =
      Result.ok ⟨0#usize, 1#usize, 1#usize, 0#usize, 0#usize⟩

private def busyOwnerContract : Prop :=
  ∀ (record : resources.Snapshot) (serial next : U64) (receiver : resources.Context) (generation : U64),
    M5Resources.valid record = true → record.state = .Busy serial →
    let required : resources.Requirement := ⟨record.handle.context, record.handle.kind, record.handle.rights⟩
    resources.transition.transition record record.handle (.Inspect required) = Result.ok (.Err .Busy) ∧
    resources.transition.transition record record.handle (.Release required) = Result.ok (.Err .Busy) ∧
    resources.transition.transition record record.handle (.Begin required next) = Result.ok (.Err .Busy) ∧
    resources.transition.transition record record.handle (.Transfer required receiver generation) = Result.ok (.Err .Busy)

private def implementationRoots : List Name := [
  ``resources.transition.transition, ``resources.table.Table.decide, ``resources.Decision.accounting]

private def contracts : List (Name × Name × List Name × List Name) := Id.run do
  let transition := ``resources.transition.transition
  let accounting := ``resources.Decision.accounting
  let invariant := [transition, accounting, ``M5Resources.valid, ``M5Resources.scopeCheck]
  return [
    (``M5Resources.transition_refines, ``transitionContract,
      [transition, ``M5Resources.step], [transition, ``M5Resources.step]),
    (``M5Resources.table_decide_refines, ``tableContract,
      [``resources.table.Table.decide, ``M5Resources.tableStep],
      [``resources.table.Table.decide, ``M5Resources.tableStep, transition, ``M5Resources.step]),
    (``M5Resources.cancelled_pin_retained, ``cancellationContract, invariant, invariant),
    (``M5Resources.late_completion_once, ``lateCompletionContract, invariant, invariant),
    (``M5Resources.retired_completion_idempotent, ``retiredCompletionContract, invariant, invariant),
    (``M5Resources.normal_completion_owner_once, ``normalCompletionContract, invariant, invariant),
    (``M5Resources.busy_owner_not_reusable, ``busyOwnerContract,
      [transition, ``M5Resources.valid], [transition, ``M5Resources.valid])]

private def scopedKernel (declName : Name) : Bool :=
  ["noble_kernel.resources.", "noble_kernel.authority."].any
    (fun namespacePrefix => declName.toString.startsWith namespacePrefix)

private def proofOwned (env : Environment) (declName : Name) : Bool :=
  M4Audit.definingModule env declName == `M5Resources || declName.toString.startsWith "M5Resources."

private def field (value : Json) (key : String) : CommandElabM String := do
  match value.getObjValAs? String key with
  | .ok text =>
    if text.isEmpty then throwError "M5-PACKET: empty {key}"
    return text
  | .error error => throwError "M5-PACKET: {key}: {error}"

private def entries (packet : Json) (key : String) : CommandElabM (Array Json) := do
  match packet.getObjValAs? (Array Json) key with
  | .ok rows => return rows
  | .error error => throwError "M5-PACKET: {key}: {error}"

private def names (packet : Json) (key : String) : CommandElabM (List Name) := do
  match packet.getObjValAs? (List String) key with
  | .ok rows => return rows.map String.toName
  | .error error => throwError "M5-PACKET: {key}: {error}"

private def declaration (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  match env.find? declName with
  | some info => return info
  | none => throwError "M5-DECLARATION: missing compiled declaration {declName}"

private def safeDeclaration (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  let info ← declaration env declName
  match info with
  | .axiomInfo _ => throwError "M5-AXIOM: postulated project/model declaration {declName}"
  | .opaqueInfo _ => throwError "M5-TRANSPARENT: opaque project/model declaration {declName}"
  | .defnInfo value =>
    unless value.safety == .safe do throwError "M5-TRANSPARENT: unsafe/partial definition {declName}"
  | .inductInfo value =>
    if value.isUnsafe then throwError "M5-TRANSPARENT: unsafe type {declName}"
  | _ => pure ()
  return info

private def nameJson (names : Array Name) : Json := toJson (names.map Name.toString)

private def typeRecord (info : ConstantInfo) : CommandElabM (List (String × Json)) := do
  let rendered ← liftTermElabM (Meta.ppExpr info.type)
  return [("type", toJson rendered.pretty), ("type_expression", toJson (reprStr info.type))]

elab "check_m5_resources" : command => do
  let env := (← getEnv).setExporting false
  let some inputFile ← liftIO (IO.getEnv "NOBLE_M5_EXTRACTION_SUBJECTS")
    | throwError "M5-PACKET: run verification/m4/implementation.mjs"
  let text ← liftIO (IO.FS.readFile inputFile)
  let packet ← match Json.parse text with
    | .ok packet => pure packet
    | .error error => throwError "M5-PACKET: {error}"
  unless packet.getObjValAs? String "schema" == .ok "m5-resource-audit/v1" do
    throwError "M5-PACKET: unreviewed schema"
  let binding ← match packet.getObjVal? "binding" with
    | .ok binding => pure binding
    | .error error => throwError "M5-PACKET: missing source/compiler binding: {error}"
  for key in ["source_revision", "inventory_sha256", "canonical_audit_input_sha256", "component_subjects_sha256"] do
    let _ ← field binding key
    pure ()
  for key in ["source_files", "staged_source_files", "tools", "dependencies", "extractions"] do
    unless (binding.getObjVal? key).isOk do throwError "M5-PACKET: missing binding {key}"
  let roots ← entries packet "strict_roots"
  unless roots.size == contracts.length do throwError "M5-ROOT: missing/extra strict theorem"
  for (row, (declName, _, required, dependencies)) in roots.toList.zip contracts do
    unless (← field row "declaration").toName == declName &&
        (← field row "owner") == "M5Resources" &&
        (← names row "type_dependencies") == required &&
        (← names row "required_dependencies") == dependencies do
      throwError "M5-ROOT: substituted strict theorem contract {declName}"
  let actualRoots ← entries packet "implementation_roots"
  unless actualRoots.size == implementationRoots.length do throwError "M5-ROOT: missing/extra implementation root"
  for (row, declName) in actualRoots.toList.zip implementationRoots do
    unless (← field row "declaration").toName == declName && (← field row "owner") == "NobleKernel.Funs" do
      throwError "M5-ROOT: substituted actual resource implementation {declName}"

  let declarations ← entries packet "declarations"
  if declarations.isEmpty then throwError "M5-COVERAGE: empty production declarations"
  let mut seen : NameSet := {}
  let mut owners : Array (Name × Name) := #[]
  for row in declarations do
    let declName := (← field row "declaration").toName
    let owner := (← field row "owner").toName
    if seen.contains declName then throwError "M5-COVERAGE: duplicate production declaration {declName}"
    seen := seen.insert declName
    unless scopedKernel declName && [`NobleKernel.Types, `NobleKernel.Funs].contains owner &&
        M4Audit.definingModule env declName == owner do
      throwError "M5-OWNER: missing/substituted generated declaration {declName}"
    let info ← safeDeclaration env declName
    if (← field row "kind") == "functions" then
      unless M4Audit.kind info == "definition" do throwError "M5-TRANSPARENT: not an actual body {declName}"
    owners := owners.push (declName, owner)
  -- Compiler/translator IDs account every original body. Independently reject
  -- disappearance of compiled scoped names, allowing only subordinate helpers
  -- of a listed body/type, never a different module or handwritten substitute.
  let compiled := env.constants.fold (init := (#[] : Array Name)) fun result declName _ =>
    if scopedKernel declName && [`NobleKernel.Types, `NobleKernel.Funs].contains
        (M4Audit.definingModule env declName) then result.push declName else result
  for declName in compiled do
    unless owners.any (fun (parent, owner) => owner == M4Audit.definingModule env declName &&
        parent.isPrefixOf declName) do
      throwError "M5-COVERAGE: omitted compiled resource/authority declaration {declName}"

  let mut closure : NameSet := {}
  let mut actualRecords : Array Json := #[]
  for declName in implementationRoots do
    unless seen.contains declName do throwError "M5-COVERAGE: omitted implementation body {declName}"
    let _ ← M4Audit.transparent env declName
    let reachable ← M4Audit.reachable env declName
    if declName == ``resources.table.Table.decide && !reachable.contains ``resources.transition.transition then
      throwError "M5-DEPENDENCY: Table.decide bypasses the actual extracted transition"
    for dependency in reachable do closure := closure.insert dependency
    let axioms ← M4Audit.auditAxioms env declName true (code := "M5-AXIOM")
    actualRecords := actualRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson "NobleKernel.Funs"),
      ("axioms", nameJson axioms), ("reachable_declarations", nameJson reachable)] ++
      (← typeRecord (← declaration env declName))))

  let mut strictRecords : Array Json := #[]
  for (declName, contract, required, dependencies) in contracts do
    let info ← declaration env declName
    unless M4Audit.definingModule env declName == `M5Resources do
      throwError "M5-OWNER: substituted resource theorem {declName}"
    match info with
    | .thmInfo theoremInfo =>
      unless theoremInfo.levelParams.isEmpty do throwError "M5-TYPE: unexpected theorem universe parameters {declName}"
    | _ => throwError "M5-THEOREM: root is not a compiled theorem {declName}"
    let some expected := (← declaration env contract).value?
      | throwError "M5-TYPE: missing audited proposition {contract}"
    unless (← liftTermElabM (Meta.withTransparency .reducible (Meta.isDefEq info.type expected))) do
      throwError "M5-TYPE: finite, weakened or substituted resource proposition {declName}"
    for dependency in required do
      unless info.type.getUsedConstants.contains dependency do
        throwError "M5-SUBJECT: {declName} does not directly bind {dependency}"
    let reachable ← M4Audit.reachable env declName
    for dependency in dependencies do
      unless reachable.contains dependency do throwError "M5-DEPENDENCY: {declName} detached from {dependency}"
    for dependency in reachable do closure := closure.insert dependency
    let axioms ← M4Audit.auditAxioms env declName true (code := "M5-AXIOM")
    strictRecords := strictRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson "M5Resources"),
      ("classification", toJson "strict-universal-actual-Rust-resource-correspondence-or-invariant"),
      ("type_dependencies", toJson (required.map Name.toString)),
      ("required_dependencies", toJson (dependencies.map Name.toString)),
      ("axioms", nameJson axioms), ("reachable_declarations", nameJson reachable)] ++ (← typeRecord info)))

  let mut declarationRecords : Array Json := #[]
  for row in declarations do
    let declName := (← field row "declaration").toName
    let info ← declaration env declName
    let axioms ← M4Audit.auditAxioms env declName false (!closure.contains declName) (code := "M5-AXIOM")
    declarationRecords := declarationRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson (M4Audit.definingModule env declName).toString),
      ("kind", toJson (M4Audit.kind info)), ("axioms", nameJson axioms),
      ("reachable_from_resource_roots", toJson (closure.contains declName))])
  let owned := env.constants.fold (init := (#[] : Array Name)) fun result declName _ =>
    if proofOwned env declName then result.push declName else result
  let mut proofRecords : Array Json := #[]
  for declName in owned.qsort Name.lt do
    let info ← safeDeclaration env declName
    let axioms ← M4Audit.auditAxioms env declName true (code := "M5-AXIOM")
    proofRecords := proofRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson (M4Audit.definingModule env declName).toString),
      ("kind", toJson (M4Audit.kind info)), ("axioms", nameJson axioms),
      ("reachable_from_resource_roots", toJson (closure.contains declName))] ++ (← typeRecord info)))
  let mut dependencyRecords : Array Json := #[]
  for declName in closure.toArray.qsort Name.lt do
    let info ← declaration env declName
    let valueDependencies := match info.value? (allowOpaque := true) with
      | some value => value.getUsedConstants
      | none => #[]
    let constructors := match info with
      | .inductInfo value => value.ctors.toArray
      | _ => #[]
    dependencyRecords := dependencyRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson (M4Audit.definingModule env declName).toString),
      ("kind", toJson (M4Audit.kind info)),
      ("type_dependencies", nameJson (info.type.getUsedConstants.qsort Name.lt)),
      ("value_dependencies", nameJson (valueDependencies.qsort Name.lt)),
      ("constructors", nameJson constructors)])
  logInfo <| "M5-RESOURCES " ++ (Json.mkObj [
    ("schema", toJson "m5-resource-audit/v1"), ("result", toJson "passed"),
    ("packet", packet), ("strict_axioms", toJson (M4Audit.strictAxioms.map Name.toString)),
    ("strict_roots", toJson strictRecords), ("implementation_roots", toJson actualRecords),
    ("declarations", toJson declarationRecords), ("project_declarations", toJson proofRecords),
    ("transitive_dependencies", toJson dependencyRecords),
    ("scope", toJson "Universal actual resource transition/Table.decide correspondence and qualified lifecycle/accounting invariants; all new kernel resources/authority declarations separately inventoried."),
    ("non_claims", toJson (["No physical native/host release or arbitrary destructor correctness theorem.",
      "No entire authority-system, authentic observation, remote-effect or quota synchronization refinement theorem.",
      "No WIT parser, Canonical ABI, component compiler, loader, peer execution or runtime correctness theorem.",
      "Peer independence, actual conformance and all-target source-coverage policy remain independent gates."] : List String))]).compress

end M5Audit
