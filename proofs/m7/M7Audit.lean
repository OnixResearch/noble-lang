import M7Syndicate
import M4Audit
import Lean

open Lean Elab Command
open Aeneas Aeneas.Std noble_kernel

set_option Aeneas.customDoElab false
set_option maxHeartbeats 1000000

namespace M7Audit

private def publicationContract : Prop :=
  ∀ (current : Option Bool) (requested : Bool),
    dataspace.decide_publication current requested = .ok (M7Syndicate.decide current requested)
private def permissionContract : Prop :=
  ∀ (rights : dataspace.Rights) (operation : dataspace.Operation),
    dataspace.permits rights operation = .ok (M7Syndicate.allowed rights operation)
private def memoryContract : Prop :=
  ∀ (requested : Bool),
    dataspace.admit_shared_memory requested = .ok (M7Syndicate.admit requested)
private def memoryRefusalContract : Prop :=
  dataspace.admit_shared_memory true = .ok (.Err dataspace.Error.Denied)
private def publishRightContract : Prop :=
  ∀ (rights : dataspace.Rights), rights.publish = false →
    dataspace.permits rights .Publish = .ok false
private def observeRightContract : Prop :=
  ∀ (rights : dataspace.Rights), rights.observe = false →
    dataspace.permits rights .Observe = .ok false
private def duplicateContract : Prop :=
  ∀ (ready : Bool), dataspace.decide_publication (some ready) ready = .ok .Unchanged
private def replacementContract : Prop :=
  ∀ (previous requested : Bool), previous ≠ requested →
    dataspace.decide_publication (some previous) requested = .ok .Replaced
private def additionContract : Prop :=
  ∀ (ready : Bool), dataspace.decide_publication none ready = .ok .Added

private def contracts : List (Name × Name × List Name × List Name) := [
  (``M7Syndicate.publication_refines, ``publicationContract,
    [``dataspace.decide_publication, ``M7Syndicate.decide],
    [``dataspace.decide_publication, ``M7Syndicate.decide]),
  (``M7Syndicate.permission_refines, ``permissionContract,
    [``dataspace.permits, ``M7Syndicate.allowed],
    [``dataspace.permits, ``M7Syndicate.allowed]),
  (``M7Syndicate.shared_memory_refines, ``memoryContract,
    [``dataspace.admit_shared_memory, ``M7Syndicate.admit],
    [``dataspace.admit_shared_memory, ``M7Syndicate.admit]),
  (``M7Syndicate.shared_memory_refused, ``memoryRefusalContract,
    [``dataspace.admit_shared_memory],
    [``dataspace.admit_shared_memory, ``M7Syndicate.shared_memory_refines, ``M7Syndicate.admit]),
  (``M7Syndicate.publish_right_required, ``publishRightContract,
    [``dataspace.permits],
    [``dataspace.permits, ``M7Syndicate.permission_refines, ``M7Syndicate.allowed]),
  (``M7Syndicate.observe_right_required, ``observeRightContract,
    [``dataspace.permits],
    [``dataspace.permits, ``M7Syndicate.permission_refines, ``M7Syndicate.allowed]),
  (``M7Syndicate.duplicate_publication_unchanged, ``duplicateContract,
    [``dataspace.decide_publication],
    [``dataspace.decide_publication, ``M7Syndicate.publication_refines, ``M7Syndicate.decide]),
  (``M7Syndicate.changed_publication_replaces, ``replacementContract,
    [``dataspace.decide_publication],
    [``dataspace.decide_publication, ``M7Syndicate.publication_refines, ``M7Syndicate.decide]),
  (``M7Syndicate.first_publication_adds, ``additionContract,
    [``dataspace.decide_publication],
    [``dataspace.decide_publication, ``M7Syndicate.publication_refines, ``M7Syndicate.decide])]

private def implementations : List (Name × Nat) := [
  (``dataspace.admit_shared_memory, 31),
  (``dataspace.decide_publication, 32),
  (``dataspace.permits, 33)]
private def references : List Name :=
  [``M7Syndicate.admit, ``M7Syndicate.decide, ``M7Syndicate.allowed]
-- Packet-internal admission only. The M4 driver hashes checkout files, checks the
-- selected tool policy and reviewed lock, and stages the exact source snapshot.
private def selectedSources : List String := [
  "crates/noble-kernel/src/dataspace/mod.rs",
  "crates/noble-kernel/src/dataspace/table.rs",
  "crates/noble-kernel/src/dataspace/table/actions.rs",
  "crates/noble-kernel/src/dataspace/wire.rs",
  "crates/noble-kernel/tests/m7-dataspace.rs",
  "crates/noble-syndicate/Cargo.toml",
  "crates/noble-syndicate/src/lib.rs",
  "crates/noble-syndicate/tests/service.rs",
  "crates/noble-wasm/wit/syndicate.wit",
  "verification/m7/peer/Cargo.lock",
  "verification/m7/peer/Cargo.toml",
  "verification/m7/peer/src/main.rs",
  "proofs/m7/M7Audit.lean",
  "proofs/m7/M7Syndicate.lean",
  "proofs/m7/lake-manifest.json",
  "proofs/m7/lakefile.toml",
  "proofs/m7/lean-toolchain",
  "verification/m7/extraction.mjs",
  "verification/m7/build.mjs",
  "verification/m7/gate.mjs",
  "verification/m7/assurance.mjs",
  "verification/m7/final-documents.mjs"]
private def hostAdapterSources : List String := [
  "crates/noble-syndicate/Cargo.toml",
  "crates/noble-syndicate/src/lib.rs",
  "crates/noble-syndicate/tests/service.rs"]
private def sha256Digest (digest : String) : Bool :=
  digest.length == 64 &&
    digest.toList.all (fun c => c.isDigit || (decide ('a' ≤ c) && decide (c ≤ 'f')))
private def inDataspace (declName : Name) : Bool :=
  declName.toString.startsWith "noble_kernel.dataspace."

private def field (row : Json) (key : String) : CommandElabM String := do
  match row.getObjValAs? String key with
  | .ok text =>
    if text.isEmpty then throwError "M7-PACKET: empty {key}"
    return text
  | .error error => throwError "M7-PACKET: {key}: {error}"
private def rows (packet : Json) (key : String) : CommandElabM (Array Json) := do
  match packet.getObjValAs? (Array Json) key with
  | .ok value => return value
  | .error error => throwError "M7-PACKET: {key}: {error}"
private def names (row : Json) (key : String) : CommandElabM (List Name) := do
  match row.getObjValAs? (List String) key with
  | .ok value => return value.map String.toName
  | .error error => throwError "M7-PACKET: {key}: {error}"
private def declaration (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  match env.find? declName with
  | some info => return info
  | none => throwError "M7-ROOT: missing compiled declaration {declName}"
private def transparent (env : Environment) (declName : Name) : CommandElabM Expr := do
  let info ← declaration env declName
  match info with
  | .defnInfo value =>
    unless value.safety == .safe do throwError "M7-TRANSPARENT: unsafe/partial {declName}"
    return value.value
  | .axiomInfo _ => throwError "M7-AXIOM: postulated {declName}"
  | _ => throwError "M7-TRANSPARENT: not a total definition {declName}"
private def dependency (declName : Name) (reachable : Array Name) (required : List Name) : CommandElabM Unit := do
  for needed in required do
    unless reachable.contains needed do throwError "M7-DEPENDENCY: {declName} detached from {needed}"
private def contract (env : Environment) (declName expected : Name) (typeDeps : List Name) : CommandElabM ConstantInfo := do
  let info ← declaration env declName
  match info with
  | .thmInfo value =>
    unless value.levelParams.isEmpty do throwError "M7-TYPE: universe-quantified {declName}"
  | _ => throwError "M7-ROOT: strict root is not a theorem {declName}"
  unless (← liftTermElabM (Meta.withTransparency .reducible
      (Meta.isDefEq info.type (← transparent env expected)))) do
    throwError "M7-TYPE: weakened, finite or assumed theorem {declName}"
  for needed in typeDeps do
    unless info.type.getUsedConstants.contains needed do
      throwError "M7-TYPE: theorem does not directly bind {needed}"
  return info
private def independent (env : Environment) (declName : Name) : CommandElabM (Array Name) := do
  let reachable ← M4Audit.reachable env declName
  for used in reachable do
    if inDataspace used && M4Audit.definingModule env used == `NobleKernel.Funs then
      throwError "M7-INDEPENDENCE: reference {declName} calls extracted dataspace body {used}"
  return reachable

-- The controls run the same checks on an adversarial compiled candidate; they
-- cannot replace any fixed root in the source-bound packet check below.
elab "check_m7_contract_control" candidate:str expected:str : command => do
  let env := (← getEnv).setExporting false
  let some (_, expectedType, typeDeps, needed) :=
      contracts.find? (fun (declName, _, _, _) => declName == expected.getString.toName)
    | throwError "M7-ROOT: unknown strict contract"
  let declName := candidate.getString.toName
  let _ ← contract env declName expectedType typeDeps
  dependency declName (← M4Audit.reachable env declName) needed
  let _ ← M4Audit.auditAxioms env declName true (code := "M7-AXIOM")
  pure ()
elab "check_m7_dependency_control" candidate:str required:str : command => do
  let env := (← getEnv).setExporting false
  let declName := candidate.getString.toName
  let _ ← transparent env declName
  dependency declName (← M4Audit.reachable env declName) [required.getString.toName]
elab "check_m7_reference_control" candidate:str : command => do
  let env := (← getEnv).setExporting false
  let declName := candidate.getString.toName
  let _ ← transparent env declName
  let _ ← independent env declName
  pure ()

elab "check_m7_syndicate" : command => do
  let env := (← getEnv).setExporting false
  let some path ← liftIO (IO.getEnv "NOBLE_M7_EXTRACTION_SUBJECTS")
    | throwError "M7-PACKET: source-bound audit input required"
  let text ← liftIO (IO.FS.readFile path)
  let packet ← match Json.parse text with
    | .ok packet => pure packet
    | .error error => throwError "M7-PACKET: {error}"
  unless packet.getObjValAs? String "schema" == .ok "noble-m7-syndicate-audit/v1" do
    throwError "M7-PACKET: unreviewed schema"
  let binding ← match packet.getObjVal? "binding" with
    | .ok value => pure value
    | .error error => throwError "M7-PACKET: missing binding: {error}"
  for key in ["source_revision", "inventory_sha256", "canonical_audit_input_sha256", "syndicate_subjects_sha256"] do
    let _ ← field binding key
  for key in ["tools", "dependencies", "extractions"] do
    unless (binding.getObjVal? key).isOk do throwError "M7-PACKET: missing binding {key}"
  let sourceFiles ← match binding.getObjVal? "source_files" with
    | .ok sources =>
      unless sources.getObj?.isOk do throwError "M7-PACKET: invalid source file map"
      pure sources
    | _ => throwError "M7-PACKET: missing source file map"
  let stagedFiles ← match binding.getObjVal? "staged_source_files" with
    | .ok staged =>
      unless staged.getObj?.isOk do throwError "M7-PACKET: invalid staged source file map"
      pure staged
    | _ => throwError "M7-PACKET: missing staged source file map"
  for file in selectedSources do
    let digest ← field sourceFiles file
    unless sha256Digest digest do
      throwError "M7-PACKET: malformed selected source digest {file}"
    unless (← field stagedFiles file) == digest do
      throwError "M7-PACKET: selected staged source differs from the source snapshot: {file}"
  let mut hosts : List (String × Json) := []
  for file in hostAdapterSources do
    hosts := (file, Json.str (← field sourceFiles file)) :: hosts
  let hostFiles ← match packet.getObjVal? "host_adapter_source_files" with
    | .ok value => pure value
    | _ => throwError "M7-PACKET: missing host adapter source map"
  unless hostFiles == Json.mkObj hosts do
    throwError "M7-PACKET: host adapter sources differ from the source snapshot"
  let strictRows ← rows packet "strict_roots"
  unless strictRows.size == contracts.length do throwError "M7-ROOT: missing/extra strict theorem"
  for (row, (declName, _, typeDeps, required)) in strictRows.toList.zip contracts do
    unless (← field row "declaration").toName == declName &&
        (← field row "owner") == "M7Syndicate" &&
        (← names row "type_dependencies") == typeDeps &&
        (← names row "required_dependencies") == required do
      throwError "M7-ROOT: substituted theorem contract {declName}"
  let implementationRows ← rows packet "implementation_roots"
  unless implementationRows.size == implementations.length do
    throwError "M7-ROOT: missing/extra generated implementation"
  for (row, (declName, id)) in implementationRows.toList.zip implementations do
    unless (← field row "declaration").toName == declName &&
        (← field row "owner") == "NobleKernel.Funs" &&
        row.getObjValAs? Nat "def_id" == .ok id do
      throwError "M7-ROOT: substituted implementation {declName}"
  let referenceRows ← rows packet "reference_roots"
  unless referenceRows.size == references.length do throwError "M7-ROOT: missing/extra reference"
  for (row, declName) in referenceRows.toList.zip references do
    unless (← field row "declaration").toName == declName &&
        (← field row "owner") == "M7Syndicate" do
      throwError "M7-ROOT: substituted reference {declName}"
  let declarationRows ← rows packet "declarations"
  if declarationRows.isEmpty then throwError "M7-COVERAGE: empty generated dataspace"
  let mut seen : NameSet := {}
  for row in declarationRows do
    let declName := (← field row "declaration").toName
    let owner := (← field row "owner").toName
    if seen.contains declName then throwError "M7-COVERAGE: duplicate {declName}"
    seen := seen.insert declName
    unless inDataspace declName && [`NobleKernel.Types, `NobleKernel.Funs].contains owner &&
        M4Audit.definingModule env declName == owner do
      throwError "M7-OWNER: substituted generated declaration {declName}"
    let info ← declaration env declName
    if (← field row "kind") == "functions" then
      unless M4Audit.kind info == "definition" do throwError "M7-TRANSPARENT: missing body {declName}"
  let compiled := env.constants.fold (init := (#[] : Array Name)) fun values declName _ =>
    if inDataspace declName && [`NobleKernel.Types, `NobleKernel.Funs].contains
        (M4Audit.definingModule env declName) then values.push declName else values
  for declName in compiled do
    unless declarationRows.any (fun row =>
        match row.getObjValAs? String "declaration" with
        | .ok path => path.toName.isPrefixOf declName
        | .error _ => false) do
      throwError "M7-COVERAGE: omitted generated declaration {declName}"
  let mut implementationRecords : Array Json := #[]
  for (declName, id) in implementations do
    unless seen.contains declName && M4Audit.definingModule env declName == `NobleKernel.Funs do
      throwError "M7-OWNER: missing extracted implementation {declName}"
    let _ ← transparent env declName
    let axioms ← M4Audit.auditAxioms env declName true (code := "M7-AXIOM")
    implementationRecords := implementationRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("def_id", toJson id),
      ("owner", toJson "NobleKernel.Funs"), ("axioms", toJson (axioms.map Name.toString))])
  let mut referenceRecords : Array Json := #[]
  for declName in references do
    unless M4Audit.definingModule env declName == `M7Syndicate do
      throwError "M7-OWNER: substituted independent reference {declName}"
    let _ ← transparent env declName
    let reachable ← independent env declName
    let axioms ← M4Audit.auditAxioms env declName true (code := "M7-AXIOM")
    referenceRecords := referenceRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson "M7Syndicate"),
      ("axioms", toJson (axioms.map Name.toString)),
      ("reachable_declarations", toJson (reachable.map Name.toString))])
  let mut strictRecords : Array Json := #[]
  for (declName, expected, typeDeps, needed) in contracts do
    unless M4Audit.definingModule env declName == `M7Syndicate do
      throwError "M7-OWNER: substituted strict theorem {declName}"
    let _ ← contract env declName expected typeDeps
    let reachable ← M4Audit.reachable env declName
    dependency declName reachable needed
    let axioms ← M4Audit.auditAxioms env declName true (code := "M7-AXIOM")
    strictRecords := strictRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson "M7Syndicate"),
      ("type_dependencies", toJson (typeDeps.map Name.toString)),
      ("required_dependencies", toJson (needed.map Name.toString)),
      ("axioms", toJson (axioms.map Name.toString)),
      ("reachable_declarations", toJson (reachable.map Name.toString))])
  let mut declarationRecords : Array Json := #[]
  for row in declarationRows do
    let declName := (← field row "declaration").toName
    let info ← declaration env declName
    declarationRecords := declarationRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString),
      ("owner", toJson (M4Audit.definingModule env declName).toString),
      ("kind", toJson (M4Audit.kind info))])
  -- An unused proof axiom or hole is still part of this reviewed proof module.
  -- Checking just the nine strict roots would silently admit such additions.
  let owned := env.constants.fold (init := (#[] : Array Name)) fun values declName _ =>
    if M4Audit.definingModule env declName == `M7Syndicate ||
        declName.toString.startsWith "M7Syndicate." then values.push declName else values
  for declName in owned do
    let info ← declaration env declName
    match info with
    | .axiomInfo _ => throwError "M7-AXIOM: postulated proof declaration {declName}"
    | .opaqueInfo _ => throwError "M7-TRANSPARENT: opaque proof declaration {declName}"
    | .defnInfo value =>
      unless value.safety == .safe do throwError "M7-AXIOM: partial/unsafe proof declaration {declName}"
    | .inductInfo value =>
      if value.isUnsafe then throwError "M7-AXIOM: unsafe proof datatype {declName}"
    | _ => pure ()
    let _ ← M4Audit.auditAxioms env declName true (code := "M7-AXIOM")
  logInfo <| "M7-SYNDICATE " ++ (Json.mkObj [
    ("schema", toJson "noble-m7-syndicate-audit/v1"), ("result", toJson "passed"),
    ("packet", packet), ("strict_axioms", toJson (M4Audit.strictAxioms.map Name.toString)),
    ("strict_roots", toJson strictRecords),
    ("implementation_roots", toJson implementationRecords),
    ("reference_roots", toJson referenceRecords),
    ("declarations", toJson declarationRecords),
    ("scope", toJson "Pure extracted dataspace admission, rights and publication correspondence and qualified refusal/invariance; not host or component-runtime correctness.")]).compress

end M7Audit
