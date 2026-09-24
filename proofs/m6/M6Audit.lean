import M6Async
import M4Audit
import Lean

open Lean Elab Command
open Aeneas Aeneas.Std noble_kernel
open noble_kernel.async_tasks.domain

set_option Aeneas.customDoElab false
set_option maxHeartbeats 4000000
set_option maxRecDepth 4096

namespace M6Audit

private def transitionContract : Prop :=
  ∀ (record : Snapshot) (claim : Handle) (event : Event),
    async_tasks.transition.transition record claim event = M6Async.step record claim event

private def tableContract : Prop :=
  ∀ (table : async_tasks.table.Table) (claim : Handle) (event : Event),
    async_tasks.table.Table.decide table claim event = M6Async.tableStep table claim event

private def classifyContract : Prop :=
  ∀ (state : async_tasks.domain.State) (event : Event),
    async_tasks.schema.classify state event = Result.ok (M6Async.classify state event)

-- Complete propositions, not dependency mentions: extra assumptions, finite
-- instances, weakened conclusions and wrapper-only proofs cannot satisfy roots.
private def cancellationContract : Prop :=
  ∀ (record : Snapshot) (decision : Decision),
    M6Async.WellFormed record →
    (record.state = .Pending ∨ record.state = .Ready) →
    async_tasks.transition.transition record record.handle .Cancel = .ok (.Ok decision) →
    decision.record.state = .Retiring ∧
    decision.record.native_stopped = record.native_stopped ∧
    decision.record.pins = record.pins ∧ decision.accounting.pins_released = 0#u64 ∧
    decision.accounting.reservation_released = false ∧
    decision.record.returned_inputs = 0#u64 ∧
    decision.record.retiring_inputs = record.retiring_inputs ||| decision.accounting.retired.inputs ∧
    decision.accounting.retired.results = record.results ∧
    decision.accounting.retired.buffers = record.buffers &&& 2#u8 ∧
    decision.accounting.delivered = M6Async.emptyObligations ∧
    decision.record.wake_pending = false

private def completionContract : Prop :=
  ∀ (record : Snapshot) (outcome : Outcome) (completion : Completion) (decision : Decision),
    M6Async.WellFormed record →
    async_tasks.transition.transition record record.handle
      (M6Async.completionEvent record.native outcome completion) = .ok (.Ok decision) →
    decision.record.native_stopped = record.native_stopped ∧
    decision.record.pins = record.pins ∧
    decision.accounting.pins_released = 0#u64 ∧
    decision.accounting.reservation_released = false ∧
    decision.accounting.delivered = M6Async.emptyObligations

private def lateCompletionContract : Prop :=
  ∀ (record : Snapshot) (outcome : Outcome) (completion : Completion) (decision : Decision),
    M6Async.WellFormed record → record.state = .Retiring →
    async_tasks.transition.transition record record.handle
      (M6Async.completionEvent record.native outcome completion) = .ok (.Ok decision) →
    decision.record.state = .Retiring ∧
    decision.accounting.delivered = M6Async.emptyObligations ∧
    decision.accounting.pins_released = 0#u64 ∧
    decision.accounting.reservation_released = false

private def deliveredCancelContract : Prop :=
  ∀ (record : Snapshot), M6Async.WellFormed record → record.state = .Delivered →
    async_tasks.transition.transition record record.handle .Cancel =
      .ok (.Ok (M6Async.unchanged record)) ∧
    (M6Async.unchanged record).accounting = M6Async.emptyAccounting

private def duplicateCompletionContract : Prop :=
  ∀ (record : Snapshot) (outcome : Outcome) (completion : Completion),
    M6Async.WellFormed record →
    (record.state = .Ready ∨ record.state = .Delivered ∨ record.state = .Retired) →
    async_tasks.transition.transition record record.handle
      (M6Async.completionEvent record.native outcome completion) = .ok (.Ok (M6Async.unchanged record)) ∧
    (M6Async.unchanged record).accounting = M6Async.emptyAccounting

private def pinsContract : Prop :=
  ∀ (record : Snapshot) (pins : U64),
    M6Async.WellFormed record → record.state ≠ .Retired → record.native_stopped = false →
    async_tasks.transition.transition record record.handle (.SettlePins record.native pins) =
      .ok (.Err .NativeStillRunning)

private def finishContract : Prop :=
  ∀ (record : Snapshot) (decision : Decision),
    M6Async.WellFormed record → record.finalized = false →
    (record.state = .Delivered ∨ record.state = .Retiring) →
    async_tasks.transition.transition record record.handle .Finish = .ok (.Ok decision) →
    M6Async.settled record = true ∧ decision.record.finalized = true ∧
    decision.accounting.reservation_released = true

private def readyContract : Prop :=
  ∀ (record : Snapshot), M6Async.WellFormed record → record.state = .Ready →
    async_tasks.transition.transition record record.handle .Deliver = .ok (.Ok (M6Async.deliver record)) ∧
    (M6Async.deliver record).accounting.delivered =
      ⟨record.returned_inputs, record.results, record.buffers &&& 2#u8⟩ ∧
    (M6Async.deliver record).record.returned_inputs = 0#u64 ∧
    (M6Async.deliver record).record.results = 0#u64 ∧
    (M6Async.deliver record).record.native_stopped = record.native_stopped ∧
    (M6Async.deliver record).record.pins = record.pins ∧
    (M6Async.deliver record).accounting.reservation_released = false ∧
    M6Async.cleanupAvailable record =
      some ⟨record.retiring_inputs, 0#u64, record.buffers &&& (~~~2#u8)⟩ ∧
    (∀ (work : Obligations) (decision : Decision),
      async_tasks.transition.transition record record.handle (.Cleanup work) = .ok (.Ok decision) →
      decision.record.results = record.results ∧ decision.accounting.cleaned.results = 0#u64)

private def failureContract : Prop :=
  ∀ (record : Snapshot) (event : Event) (decision : Decision) (primary : Failure),
    M6Async.WellFormed record →
    (record.state = .Retiring ∨ record.state = .Retired) →
    record.failure = some primary →
    async_tasks.transition.transition record record.handle event = .ok (.Ok decision) →
    decision.record.failure = some primary

private def releaseContract : Prop :=
  ∀ (record : Snapshot) (first second : Decision),
    M6Async.WellFormed record → record.finalized = false →
    (record.state = .Delivered ∨ record.state = .Retiring) →
    async_tasks.transition.transition record record.handle .Finish = .ok (.Ok first) →
    async_tasks.transition.transition first.record first.record.handle .Finish = .ok (.Ok second) →
    first.accounting.reservation_released = true ∧
    first.record.finalized = true ∧
    second = M6Async.unchanged first.record ∧
    second.accounting.reservation_released = false

private def recordCheckContract : Prop :=
  ∀ (record : Snapshot), async_tasks.transition.validation.record record = M6Async.recordCheck record

private def wellFormedReference (record : Snapshot) : Prop :=
  M6Async.recordCheck record = .ok (.Ok ())
private def completionEventReference (native : NativeId) (outcome : Outcome) (completion : Completion) : Event :=
  match outcome with
  | .Success => .CompleteSuccess native completion
  | .DomainError => .CompleteDomainError native completion
private def settledReference (record : Snapshot) : Bool :=
  record.native_stopped && record.pins == 0#u64 && record.retiring_inputs == 0#u64 &&
    record.returned_inputs == 0#u64 && record.results == 0#u64 && record.buffers == 0#u8

private def modelContracts : List (Name × Name) := [
  (``M6Async.WellFormed, ``wellFormedReference),
  (``M6Async.completionEvent, ``completionEventReference),
  (``M6Async.settled, ``settledReference)]

private def contracts : List (Name × Name × List Name × List Name) := Id.run do
  let transition := ``async_tasks.transition.transition
  return [
    (``M6Async.transition_refines, ``transitionContract,
      [transition, ``M6Async.step], [transition, ``M6Async.step]),
    (``M6Async.table_decide_refines, ``tableContract,
      [``async_tasks.table.Table.decide, ``M6Async.tableStep],
      [``async_tasks.table.Table.decide, ``M6Async.tableStep, transition, ``M6Async.step]),
    (``M6Async.classify_refines, ``classifyContract,
      [``async_tasks.schema.classify, ``M6Async.classify], [``async_tasks.schema.classify, ``M6Async.classify]),
    (``M6Async.cancellation_preserves_native_pins, ``cancellationContract, [transition], [transition]),
    (``M6Async.completion_does_not_stop_native, ``completionContract, [transition], [transition]),
    (``M6Async.late_completion_not_delivered, ``lateCompletionContract, [transition], [transition]),
    (``M6Async.delivered_cancel_idempotent, ``deliveredCancelContract, [transition], [transition]),
    (``M6Async.duplicate_completion_idempotent, ``duplicateCompletionContract, [transition], [transition]),
    (``M6Async.pins_require_native_stop, ``pinsContract, [transition], [transition]),
    (``M6Async.finish_requires_settlement, ``finishContract, [transition], [transition]),
    (``M6Async.ready_results_owned_until_delivery, ``readyContract, [transition], [transition]),
    (``M6Async.failure_preserves_primary, ``failureContract, [transition], [transition]),
    (``M6Async.release_once, ``releaseContract, [transition], [transition]),
    (``M6Async.recordCheck_refines, ``recordCheckContract,
      [``async_tasks.transition.validation.record, ``M6Async.recordCheck],
      [``async_tasks.transition.validation.record, ``M6Async.recordCheck])]

private def classifierType := async_tasks.domain.State → Event → M6Async.Checked async_tasks.schema.Rule
private def stepType := Snapshot → Handle → Event → Result (M6Async.Checked Decision)
private def tableType := async_tasks.table.Table → Handle → Event → Result (M6Async.Checked Decision)

private def implementationRoots : List Name := [
  ``async_tasks.transition.transition, ``async_tasks.table.Table.decide, ``async_tasks.schema.classify]

private def coverageRoots : List (Name × List Name) := [
  (``async_tasks.schema.coverage.validate_coverage,
    [``async_tasks.schema.coverage.validate_rows, ``async_tasks.schema.coverage.check_row,
     ``async_tasks.schema.classify, ``async_tasks.schema.coverage.EventKind.representative,
     ``async_tasks.schema.STATE_SCHEMA, ``async_tasks.schema.EVENT_SCHEMA]),
  (``async_tasks.schema.coverage.validate_rows,
    [``async_tasks.schema.coverage.check_row, ``async_tasks.schema.classify,
     ``async_tasks.schema.coverage.EventKind.representative]),
  (``async_tasks.schema.coverage.check_row,
    [``async_tasks.schema.classify, ``async_tasks.schema.coverage.EventKind.representative])]

private def referenceRoots : List (Name × Name) := [
  (``M6Async.classify, ``classifierType), (``M6Async.step, ``stepType),
  (``M6Async.tableStep, ``tableType)]

private def stateNames : List String := ["Pending", "Ready", "Delivered", "Retiring", "Retired"]
private def eventNames : List String := ["Inspect", "CompleteSuccess", "CompleteDomainError", "Deliver",
  "Cancel", "Trap", "Deadline", "Budget", "InternalFailure", "NativeStopped", "SettlePins", "Cleanup",
  "Wake", "TakeWake", "Finish"]

private def stateSchema : String :=
  "noble-kernel::async_tasks::domain::State{Pending;Ready;Delivered;Retiring;Retired}"
private def eventSchema : String :=
  "noble-kernel::async_tasks::domain::Event{" ++
  "Inspect;CompleteSuccess{native:NativeId,completion:Completion};" ++
  "CompleteDomainError{native:NativeId,completion:Completion};Deliver;Cancel;" ++
  "Trap;Deadline;Budget;InternalFailure;NativeStopped{native:NativeId};" ++
  "SettlePins{native:NativeId,pins:u64};Cleanup(Obligations);" ++
  "Wake{native:NativeId};TakeWake;Finish};" ++
  "noble-kernel::async_tasks::domain::NativeId(u64);" ++
  "noble-kernel::async_tasks::domain::Completion{inputs:Disposition,produced:u64,bytes:usize};" ++
  "noble-kernel::async_tasks::domain::Disposition{returned:u64,consumed:u64,retired:u64};" ++
  "noble-kernel::async_tasks::domain::Obligations{inputs:u64,results:u64,buffers:u8}"

private def schemaFiles : List String := ["crates/noble-kernel/src/async_tasks/domain.rs",
  "crates/noble-kernel/src/async_tasks/schema.rs", "crates/noble-kernel/src/async_tasks/schema/coverage.rs"]

private def stateConstructors : Aeneas.Std.Array async_tasks.domain.State 5#usize :=
  Aeneas.Std.Array.make 5#usize [.Pending, .Ready, .Delivered, .Retiring, .Retired]
private def eventConstructors : Aeneas.Std.Array async_tasks.schema.EventKind 15#usize :=
  Aeneas.Std.Array.make 15#usize [.Inspect, .CompleteSuccess, .CompleteDomainError, .Deliver,
    .Cancel, .Trap, .Deadline, .Budget, .InternalFailure, .NativeStopped, .SettlePins, .Cleanup,
    .Wake, .TakeWake, .Finish]

-- Canonical payloads bind constructor classification only. They are not valid
-- task records, native observations, or a runtime conformance experiment.
private def completion : Completion := ⟨⟨0#u64, 0#u64, 0#u64⟩, 0#u64, 0#usize⟩
private def obligations : Obligations := ⟨0#u64, 0#u64, 0#u8⟩
private def eventFor : async_tasks.schema.EventKind → Event
  | .Inspect => .Inspect
  | .CompleteSuccess => .CompleteSuccess 1#u64 completion
  | .CompleteDomainError => .CompleteDomainError 1#u64 completion
  | .Deliver => .Deliver
  | .Cancel => .Cancel
  | .Trap => .Trap
  | .Deadline => .Deadline
  | .Budget => .Budget
  | .InternalFailure => .InternalFailure
  | .NativeStopped => .NativeStopped 1#u64
  | .SettlePins => .SettlePins 1#u64 0#u64
  | .Cleanup => .Cleanup obligations
  | .Wake => .Wake 1#u64
  | .TakeWake => .TakeWake
  | .Finish => .Finish

private theorem representative_refines (kind : async_tasks.schema.EventKind) :
    async_tasks.schema.coverage.EventKind.representative kind = Result.ok (eventFor kind) := by
  cases kind <;>
    simp only [async_tasks.schema.coverage.EventKind.representative, eventFor, completion,
      obligations, async_tasks.domain.Obligations.empty, bind_tc_ok]

private def ruleFor : String → Option (M6Async.Checked async_tasks.schema.Rule)
  | "ok:Inspect" => some (.Ok .Inspect)
  | "ok:CompleteSuccess" => some (.Ok (.Complete .Success completion))
  | "ok:CompleteDomainError" => some (.Ok (.Complete .DomainError completion))
  | "ok:Deliver" => some (.Ok .Deliver)
  | "ok:Retire:Cancelled" => some (.Ok (.Retire .Cancelled))
  | "ok:Retire:Trap" => some (.Ok (.Retire .Trap))
  | "ok:Retire:Deadline" => some (.Ok (.Retire .Deadline))
  | "ok:Retire:Budget" => some (.Ok (.Retire .Budget))
  | "ok:Retire:Internal" => some (.Ok (.Retire .Internal))
  | "ok:ObserveStop" => some (.Ok .ObserveStop)
  | "ok:SettlePins" => some (.Ok (.SettlePins 0#u64))
  | "ok:Cleanup" => some (.Ok (.Cleanup obligations))
  | "ok:Wake" => some (.Ok .Wake)
  | "ok:TakeWake" => some (.Ok .TakeWake)
  | "ok:Finish" => some (.Ok .Finish)
  | "ok:Duplicate" => some (.Ok .Duplicate)
  | "error:Pending" => some (.Err .Pending)
  | "error:Ready" => some (.Err .Ready)
  | "error:Delivered" => some (.Err .Delivered)
  | "error:Retiring" => some (.Err .Retiring)
  | "error:Retired" => some (.Err .Retired)
  | _ => none

private def scopedKernel (declName : Name) : Bool :=
  declName.toString.startsWith "noble_kernel.async_tasks."

private def proofOwned (env : Environment) (declName : Name) : Bool :=
  M4Audit.definingModule env declName == `M6Async || declName.toString.startsWith "M6Async."

private def field (value : Json) (key : String) (code : String := "M6-PACKET") : CommandElabM String := do
  match value.getObjValAs? String key with
  | .ok text =>
    if text.isEmpty then throwError "{code}: empty {key}"
    return text
  | .error error => throwError "{code}: {key}: {error}"

private def entries (packet : Json) (key : String) (code : String := "M6-PACKET") : CommandElabM (Array Json) := do
  match packet.getObjValAs? (Array Json) key with
  | .ok rows => return rows
  | .error error => throwError "{code}: {key}: {error}"

private def names (packet : Json) (key : String) : CommandElabM (List Name) := do
  match packet.getObjValAs? (List String) key with
  | .ok rows => return rows.map String.toName
  | .error error => throwError "M6-PACKET: {key}: {error}"

private def declaration (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  match env.find? declName with
  | some info => return info
  | none => throwError "M6-DECLARATION: missing compiled declaration {declName}"

private def safeDeclaration (env : Environment) (declName : Name) : CommandElabM ConstantInfo := do
  let info ← declaration env declName
  match info with
  | .axiomInfo _ => throwError "M6-AXIOM: postulated project/model declaration {declName}"
  | .opaqueInfo _ => throwError "M6-TRANSPARENT: opaque project/model declaration {declName}"
  | .defnInfo value =>
    unless value.safety == .safe do throwError "M6-TRANSPARENT: unsafe/partial definition {declName}"
  | .inductInfo value =>
    if value.isUnsafe then throwError "M6-TRANSPARENT: unsafe type {declName}"
  | _ => pure ()
  return info

private def nameJson (values : Array Name) : Json := toJson (values.map Name.toString)

private def typeRecord (info : ConstantInfo) : CommandElabM (List (String × Json)) := do
  let rendered ← liftTermElabM (Meta.ppExpr info.type)
  return [("type", toJson rendered.pretty), ("type_expression", toJson (reprStr info.type))]

private def defEq (left right : Expr) : CommandElabM Bool :=
  liftTermElabM (Meta.withTransparency .all (Meta.isDefEq left right))

private def body (env : Environment) (declName : Name) : CommandElabM Expr := do
  let info ← safeDeclaration env declName
  match info with
  | .defnInfo value => return value.value
  | _ => throwError "M6-TRANSPARENT: expected transparent definition {declName}"

private def requireDependencies (declName : Name) (reachable : Array Name)
    (required : List Name) : CommandElabM Unit := do
  for dependency in required do
    unless reachable.contains dependency do throwError "M6-DEPENDENCY: {declName} detached from {dependency}"

private def requireContract (env : Environment) (declName contract : Name)
    (required : List Name) : CommandElabM ConstantInfo := do
  let info ← declaration env declName
  match info with
  | .thmInfo theoremInfo =>
    unless theoremInfo.levelParams.isEmpty do throwError "M6-TYPE: unexpected theorem universes {declName}"
  | _ => throwError "M6-THEOREM: strict root is not a compiled theorem {declName}"
  let expected ← body env contract
  unless (← liftTermElabM (Meta.withTransparency .reducible (Meta.isDefEq info.type expected))) do
    throwError "M6-TYPE: finite, weakened or substituted async proposition {declName}"
  for dependency in required do
    unless info.type.getUsedConstants.contains dependency do
      throwError "M6-SUBJECT: {declName} does not directly bind actual subject {dependency}"
  return info

private def referenceDependencies (env : Environment) (declName : Name) : CommandElabM (Array Name) := do
  let reachable ← M4Audit.reachable env declName
  for dependency in reachable do
    if scopedKernel dependency && M4Audit.definingModule env dependency == `NobleKernel.Funs then
      throwError "M6-INDEPENDENCE: reference {declName} calls extracted async implementation {dependency}"
  return reachable

private def requireModel (env : Environment) (declName contract : Name) : CommandElabM Unit := do
  unless (← defEq (← body env declName) (← body env contract)) do
    throwError "M6-MODEL: vacuous/weakened invariant qualifier or payload mapping {declName}"

-- Adversarial commands exercise the same contract/dependency checks on fresh
-- declarations. They cannot substitute a root in check_m6_async: that command
-- independently enforces the fixed root identities and owning modules.
elab "check_m6_contract_control" candidate:str expected:str : command => do
  let env := (← getEnv).setExporting false
  let some (_, contract, required, dependencies) :=
      contracts.find? (fun (rootName, _, _, _) => rootName == expected.getString.toName)
    | throwError "M6-ROOT: unknown control contract"
  let declName := candidate.getString.toName
  let _ ← requireContract env declName contract required
  requireDependencies declName (← M4Audit.reachable env declName) dependencies
  let _ ← M4Audit.auditAxioms env declName true (code := "M6-AXIOM")
  pure ()

elab "check_m6_dependency_control" candidate:str dependency:str : command => do
  let env := (← getEnv).setExporting false
  let declName := candidate.getString.toName
  let _ ← body env declName
  requireDependencies declName (← M4Audit.reachable env declName) [dependency.getString.toName]

elab "check_m6_reference_control" candidate:str : command => do
  let env := (← getEnv).setExporting false
  let declName := candidate.getString.toName
  let _ ← body env declName
  let _ ← referenceDependencies env declName
  pure ()

elab "check_m6_model_control" candidate:str expected:str : command => do
  let env := (← getEnv).setExporting false
  let some (_, contract) := modelContracts.find? (fun (rootName, _) => rootName == expected.getString.toName)
    | throwError "M6-ROOT: unknown model control contract"
  requireModel env candidate.getString.toName contract

private def checkConstructor (env : Environment) (declName parent : Name) (fields : List Name)
    (fieldNames : List String := []) : CommandElabM Unit := do
  let info ← declaration env declName
  match info with
  | .ctorInfo value =>
    unless value.induct == parent && value.numParams == 0 && value.numFields == fields.length do
      throwError "M6-SCHEMA: changed constructor payload {declName}"
  | _ => throwError "M6-SCHEMA: missing constructor {declName}"
  let mut type := info.type
  for (expected, index) in fields.zipIdx do
    match type with
    | .forallE fieldName fieldType rest _ =>
      unless (← defEq fieldType (mkConst expected)) do
        throwError "M6-SCHEMA: changed payload type {declName}.{fieldName}"
      if let some expectedName := fieldNames[index]? then
        unless fieldName.toString == expectedName do
          throwError "M6-SCHEMA: changed payload field {declName}.{fieldName}"
      type := rest
    | _ => throwError "M6-SCHEMA: missing payload field {declName}"
  unless (← defEq type (mkConst parent)) do
    throwError "M6-SCHEMA: changed constructor result {declName}"

private def checkEnum (env : Environment) (declName : Name) (constructors : List (String × List Name)) : CommandElabM Unit := do
  let info ← safeDeclaration env declName
  unless M4Audit.definingModule env declName == `NobleKernel.Types do
    throwError "M6-SCHEMA: substituted schema type {declName}"
  match info with
  | .inductInfo value =>
    unless value.numParams == 0 && value.numIndices == 0 &&
        value.ctors == constructors.map (fun (constructor, _) => declName.mkStr constructor) do
      throwError "M6-SCHEMA: missing/extra/substituted constructor in {declName}"
  | _ => throwError "M6-SCHEMA: schema is not a compiled datatype {declName}"
  for (constructor, fields) in constructors do checkConstructor env (declName.mkStr constructor) declName fields

private def checkLiteral (env : Environment) (declName : Name) (expected : String) : CommandElabM Unit := do
  unless M4Audit.definingModule env declName == `NobleKernel.Funs do
    throwError "M6-SCHEMA: substituted schema literal {declName}"
  let expression := (← body env declName).consumeMData
  unless expression.isAppOfArity ``Aeneas.Std.toStr 2 do
    throwError "M6-SCHEMA: schema is not the actual generated literal {declName}"
  unless expression.getAppArgs[0]! == mkStrLit expected do
    throwError "M6-SCHEMA: stale schema bytes in {declName}"

private def checkSchema (env : Environment) (packet binding : Json) : CommandElabM Json := do
  let domain ← match packet.getObjVal? "domain" with
    | .ok value => pure value
    | .error error => throwError "M6-SCHEMA: {error}"
  unless domain.getObjValAs? String "schema" == .ok "noble-m6-state-event-schema/v1" &&
      domain.getObjValAs? String "state_schema" == .ok stateSchema &&
      domain.getObjValAs? String "event_schema" == .ok eventSchema &&
      domain.getObjValAs? (List String) "states" == .ok stateNames &&
      domain.getObjValAs? (List String) "events" == .ok eventNames &&
      domain.getObjValAs? String "state_declaration" == .ok "noble_kernel.async_tasks.domain.State" &&
      domain.getObjValAs? String "event_declaration" == .ok "noble_kernel.async_tasks.domain.Event" &&
      domain.getObjValAs? String "classifier" == .ok "noble_kernel.async_tasks.schema.classify" &&
      domain.getObjValAs? String "coverage_admission" == .ok "noble_kernel.async_tasks.schema.coverage.validate_coverage" &&
      domain.getObjValAs? String "classification" == .ok "source-bound-compiled-constructor-coverage; not runtime-conformance" do
    throwError "M6-SCHEMA: stale/invented constructor, field, classifier or claim"
  let sources ← match domain.getObjVal? "source_files" with
    | .ok value => pure value
    | .error error => throwError "M6-SCHEMA: missing source binding: {error}"
  let boundSources ← match binding.getObjVal? "source_files" with
    | .ok value => pure value
    | .error error => throwError "M6-SCHEMA: missing inventory sources: {error}"
  for file in schemaFiles do
    let digest ← field sources file "M6-SCHEMA"
    unless digest.length == 64 && digest.toList.all (fun c => c.isDigit || (decide ('a' ≤ c) && decide (c ≤ 'f'))) &&
        boundSources.getObjValAs? String file == .ok digest do
      throwError "M6-SCHEMA: schema bytes detached from compiler inventory: {file}"
  checkEnum env ``async_tasks.domain.State (stateNames.map (fun constructor => (constructor, [])))
  checkEnum env ``async_tasks.schema.EventKind (eventNames.map (fun constructor => (constructor, [])))
  checkEnum env ``Event [
    ("Inspect", []), ("CompleteSuccess", [``NativeId, ``Completion]),
    ("CompleteDomainError", [``NativeId, ``Completion]), ("Deliver", []), ("Cancel", []),
    ("Trap", []), ("Deadline", []), ("Budget", []), ("InternalFailure", []),
    ("NativeStopped", [``NativeId]), ("SettlePins", [``NativeId, ``U64]),
    ("Cleanup", [``Obligations]), ("Wake", [``NativeId]), ("TakeWake", []), ("Finish", [])]
  unless M4Audit.definingModule env ``NativeId == `NobleKernel.Types &&
      (← defEq (← body env ``NativeId) (mkConst ``U64)) do
    throwError "M6-SCHEMA: changed NativeId payload"
  checkConstructor env ``Completion.mk ``Completion [``Disposition, ``U64, ``Usize] ["inputs", "produced", "bytes"]
  checkConstructor env ``Disposition.mk ``Disposition [``U64, ``U64, ``U64] ["returned", "consumed", "retired"]
  checkConstructor env ``Obligations.mk ``Obligations [``U64, ``U64, ``U8] ["inputs", "results", "buffers"]
  checkLiteral env ``async_tasks.schema.STATE_SCHEMA stateSchema
  checkLiteral env ``async_tasks.schema.EVENT_SCHEMA eventSchema
  for (actual, expected) in [( ``async_tasks.schema.STATE_CONSTRUCTORS, ``stateConstructors),
      (``async_tasks.schema.EVENT_CONSTRUCTORS, ``eventConstructors)] do
    unless M4Audit.definingModule env actual == `NobleKernel.Funs &&
        (← defEq (mkConst actual) (mkConst expected)) do
      throwError "M6-SCHEMA: stale production constructor array {actual}"
  -- Aeneas bind reduction is theorem-backed, not definitional equality.
  -- Audit the kernel-checked equation before using its canonical payloads.
  let representativeAxioms ← M4Audit.auditAxioms env ``representative_refines true (code := "M6-AXIOM")
  for event in eventNames do
    let kind := mkConst ((``async_tasks.schema.EventKind).mkStr event)
    let representative := mkApp (mkConst ``eventFor) kind
    let expected ← liftTermElabM (Meta.mkAppM ``Result.ok #[representative])
    let actual := mkApp (mkConst ``async_tasks.schema.coverage.EventKind.representative) kind
    let equation ← liftTermElabM (Meta.mkEq actual expected)
    let proved ← liftTermElabM (Meta.inferType (mkApp (mkConst ``representative_refines) kind))
    unless (← defEq proved equation) do throwError "M6-SCHEMA: changed production event payload {event}"
  let pairs ← entries packet "pairs" "M6-PAIR"
  unless pairs.size == 75 do throwError "M6-PAIR: constructor coverage must contain exactly 75 pairs"
  let mut seen : List (String × String) := []
  for row in pairs do
    let state ← field row "state" "M6-PAIR"
    let event ← field row "event" "M6-PAIR"
    unless stateNames.contains state && eventNames.contains event do
      throwError "M6-PAIR: invented constructor {state}/{event}"
    if seen.contains (state, event) then throwError "M6-PAIR: duplicate pair {state}/{event}"
    seen := (state, event) :: seen
    let rule ← field row "rule" "M6-PAIR"
    let parsed ← liftTermElabM (Meta.withTransparency .all (Meta.whnf (mkApp (mkConst ``ruleFor) (mkStrLit rule))))
    unless parsed.isAppOfArity ``Option.some 2 do throwError "M6-PAIR: invented disposition {rule}"
    let expected ← liftTermElabM (Meta.mkAppM ``Result.ok #[parsed.getAppArgs[1]!])
    let stateExpr := mkConst ((``async_tasks.domain.State).mkStr state)
    let eventExpr := mkApp (mkConst ``eventFor) (mkConst ((``async_tasks.schema.EventKind).mkStr event))
    let actual := mkApp2 (mkConst ``async_tasks.schema.classify) stateExpr eventExpr
    let reference ← liftTermElabM (Meta.mkAppM ``Result.ok
      #[mkApp2 (mkConst ``M6Async.classify) stateExpr eventExpr])
    let equation ← liftTermElabM (Meta.mkEq actual reference)
    let proved ← liftTermElabM (Meta.inferType
      (mkApp2 (mkConst ``M6Async.classify_refines) stateExpr eventExpr))
    unless (← defEq proved equation) && (← defEq reference expected) do
      throwError "M6-PAIR: wrong production disposition {state}/{event}: {rule}"
  for state in stateNames do
    for event in eventNames do
      unless seen.contains (state, event) do throwError "M6-PAIR: missing pair {state}/{event}"
  return Json.mkObj [("schema", toJson "noble-m6-compiled-constructor-coverage/v1"),
    ("domain", domain), ("pairs", toJson pairs), ("pair_count", toJson pairs.size),
    ("representative_correspondence", Json.mkObj [
      ("declaration", toJson (``representative_refines).toString),
      ("axioms", nameJson representativeAxioms)]),
    ("classifier_correspondence", toJson "M6Async.classify_refines"),
    ("method", toJson "kernel-checked generated-representative equation and strict universal actual-classifier correspondence, followed by complete reference classification; not runtime conformance"),
    ("runtime_conformance", toJson false)]

elab "check_m6_async" : command => do
  let env := (← getEnv).setExporting false
  let some inputFile ← liftIO (IO.getEnv "NOBLE_M6_EXTRACTION_SUBJECTS")
    | throwError "M6-PACKET: run verification/m4/implementation.mjs"
  let text ← liftIO (IO.FS.readFile inputFile)
  let packet ← match Json.parse text with
    | .ok packet => pure packet
    | .error error => throwError "M6-PACKET: {error}"
  unless packet.getObjValAs? String "schema" == .ok "noble-m6-async-audit/v1" do
    throwError "M6-PACKET: unreviewed schema"
  let binding ← match packet.getObjVal? "binding" with
    | .ok binding => pure binding
    | .error error => throwError "M6-PACKET: missing compiler/source binding: {error}"
  for key in ["source_revision", "inventory_sha256", "canonical_audit_input_sha256", "async_subjects_sha256"] do
    let _ ← field binding key
    pure ()
  for key in ["source_files", "staged_source_files", "tools", "dependencies", "extractions"] do
    unless (binding.getObjVal? key).isOk do throwError "M6-PACKET: missing binding {key}"
  let roots ← entries packet "strict_roots"
  unless roots.size == contracts.length do throwError "M6-ROOT: missing/extra strict theorem"
  for (row, (declName, _, required, dependencies)) in roots.toList.zip contracts do
    unless (← field row "declaration").toName == declName &&
        (← field row "owner") == "M6Async" &&
        (← names row "type_dependencies") == required &&
        (← names row "required_dependencies") == dependencies do
      throwError "M6-ROOT: substituted strict theorem contract {declName}"
  let actualRoots ← entries packet "implementation_roots"
  unless actualRoots.size == implementationRoots.length do throwError "M6-ROOT: missing/extra actual implementation root"
  for (row, declName) in actualRoots.toList.zip implementationRoots do
    unless (← field row "declaration").toName == declName &&
        (← field row "owner") == "NobleKernel.Funs" do
      throwError "M6-ROOT: substituted actual async implementation {declName}"
  let coverageRows ← entries packet "coverage_roots"
  unless coverageRows.size == coverageRoots.length do throwError "M6-ROOT: missing/extra production coverage root"
  for (row, (declName, required)) in coverageRows.toList.zip coverageRoots do
    unless (← field row "declaration").toName == declName &&
        (← field row "owner") == "NobleKernel.Funs" &&
        (← names row "required_dependencies") == required do
      throwError "M6-ROOT: substituted production schema coverage contract {declName}"
  unless (← names packet "reference_roots") == referenceRoots.map Prod.fst do
    throwError "M6-ROOT: missing/extra independent reference root"
  unless (← names packet "qualified_models") == modelContracts.map Prod.fst do
    throwError "M6-ROOT: missing/extra audited invariant qualifier"

  let declarations ← entries packet "declarations"
  if declarations.isEmpty then throwError "M6-COVERAGE: empty production declarations"
  let mut seen : NameSet := {}
  let mut owners : Array (Name × Name) := #[]
  for row in declarations do
    let declName := (← field row "declaration").toName
    let owner := (← field row "owner").toName
    if seen.contains declName then throwError "M6-COVERAGE: duplicate production declaration {declName}"
    seen := seen.insert declName
    unless scopedKernel declName && [`NobleKernel.Types, `NobleKernel.Funs].contains owner &&
        M4Audit.definingModule env declName == owner do
      throwError "M6-OWNER: missing/substituted generated async declaration {declName}"
    let info ← safeDeclaration env declName
    if (← field row "kind") == "functions" then
      unless M4Audit.kind info == "definition" do throwError "M6-TRANSPARENT: not an actual body {declName}"
    owners := owners.push (declName, owner)
  -- Every compiled source-owned async type/body is independently accounted,
  -- including admission, callbacks, ownership, inspection and schema loops.
  let compiled := env.constants.fold (init := (#[] : Array Name)) fun result declName _ =>
    if scopedKernel declName && [`NobleKernel.Types, `NobleKernel.Funs].contains
        (M4Audit.definingModule env declName) then result.push declName else result
  for declName in compiled do
    unless owners.any (fun (parent, owner) => owner == M4Audit.definingModule env declName &&
        parent.isPrefixOf declName) do
      throwError "M6-COVERAGE: omitted compiled async declaration {declName}"

  let mut closure : NameSet := {}
  let mut actualRecords : Array Json := #[]
  for declName in implementationRoots do
    unless seen.contains declName do throwError "M6-COVERAGE: omitted actual implementation {declName}"
    let _ ← M4Audit.transparent env declName
    let reachable ← M4Audit.reachable env declName
    if declName == ``async_tasks.table.Table.decide &&
        !reachable.contains ``async_tasks.transition.transition then
      throwError "M6-DEPENDENCY: Table.decide bypasses actual extracted transition"
    if declName == ``async_tasks.transition.transition &&
        !reachable.contains ``async_tasks.schema.classify then
      throwError "M6-DEPENDENCY: actual transition bypasses the production classifier"
    for dependency in reachable do closure := closure.insert dependency
    let axioms ← M4Audit.auditAxioms env declName true (code := "M6-AXIOM")
    actualRecords := actualRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson "NobleKernel.Funs"),
      ("axioms", nameJson axioms), ("reachable_declarations", nameJson reachable)] ++
      (← typeRecord (← declaration env declName))))

  let mut referenceRecords : Array Json := #[]
  for (declName, expectedType) in referenceRoots do
    unless M4Audit.definingModule env declName == `M6Async do
      throwError "M6-OWNER: substituted independent reference {declName}"
    let _ ← body env declName
    let info ← declaration env declName
    unless (← defEq info.type (← body env expectedType)) do
      throwError "M6-TYPE: reference is not the complete total function {declName}"
    let reachable ← referenceDependencies env declName
    for dependency in reachable do closure := closure.insert dependency
    let axioms ← M4Audit.auditAxioms env declName true (code := "M6-AXIOM")
    referenceRecords := referenceRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson "M6Async"),
      ("classification", toJson "transparent-total-independent-reference; checked-machine-failures-retained"),
      ("axioms", nameJson axioms), ("reachable_declarations", nameJson reachable)] ++ (← typeRecord info)))

  let mut strictRecords : Array Json := #[]
  for (declName, contract, required, dependencies) in contracts do
    unless M4Audit.definingModule env declName == `M6Async do
      throwError "M6-OWNER: substituted async theorem {declName}"
    let info ← requireContract env declName contract required
    let reachable ← M4Audit.reachable env declName
    requireDependencies declName reachable dependencies
    for dependency in reachable do closure := closure.insert dependency
    let axioms ← M4Audit.auditAxioms env declName true (code := "M6-AXIOM")
    strictRecords := strictRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson "M6Async"),
      ("classification", toJson "strict-universal-actual-Rust-async-correspondence-or-qualified-invariant"),
      ("type_dependencies", toJson (required.map Name.toString)),
      ("required_dependencies", toJson (dependencies.map Name.toString)),
      ("axioms", nameJson axioms), ("reachable_declarations", nameJson reachable)] ++ (← typeRecord info)))

  let mut modelRecords : Array Json := #[]
  for (declName, contract) in modelContracts do
    unless M4Audit.definingModule env declName == `M6Async do
      throwError "M6-OWNER: substituted invariant qualifier {declName}"
    requireModel env declName contract
    let reachable ← referenceDependencies env declName
    for dependency in reachable do closure := closure.insert dependency
    let axioms ← M4Audit.auditAxioms env declName true (code := "M6-AXIOM")
    modelRecords := modelRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson "M6Async"),
      ("classification", toJson "exact-record-validation-qualifier-or-constructor-mapping"),
      ("value_expression", toJson (reprStr (← body env declName))),
      ("axioms", nameJson axioms), ("reachable_declarations", nameJson reachable)] ++
      (← typeRecord (← declaration env declName))))

  -- String-size obligations in generated schema literals use only M4's exact
  -- existing literal allowance. They are never granted to strict proof roots.
  let mut coverageRecords : Array Json := #[]
  for (declName, required) in coverageRoots do
    unless seen.contains declName do throwError "M6-COVERAGE: omitted production coverage body {declName}"
    let _ ← M4Audit.transparent env declName
    let reachable ← M4Audit.reachable env declName
    requireDependencies declName reachable required
    let axioms ← M4Audit.auditAxioms env declName false (code := "M6-AXIOM")
    coverageRecords := coverageRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson "NobleKernel.Funs"),
      ("classification", toJson "source-bound-compiled-schema-admission; not runtime-conformance"),
      ("required_dependencies", toJson (required.map Name.toString)),
      ("axioms", nameJson axioms), ("reachable_declarations", nameJson reachable)] ++
      (← typeRecord (← declaration env declName))))
  let constructorCoverage ← checkSchema env packet binding

  let mut declarationRecords : Array Json := #[]
  for row in declarations do
    let declName := (← field row "declaration").toName
    let info ← declaration env declName
    let axioms ← M4Audit.auditAxioms env declName false (!closure.contains declName) (code := "M6-AXIOM")
    declarationRecords := declarationRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson (M4Audit.definingModule env declName).toString),
      ("kind", toJson (M4Audit.kind info)), ("axioms", nameJson axioms),
      ("reachable_from_async_roots", toJson (closure.contains declName))])
  let owned := env.constants.fold (init := (#[] : Array Name)) fun result declName _ =>
    if proofOwned env declName then result.push declName else result
  let mut proofRecords : Array Json := #[]
  for declName in owned.qsort Name.lt do
    let info ← safeDeclaration env declName
    let axioms ← M4Audit.auditAxioms env declName true (code := "M6-AXIOM")
    proofRecords := proofRecords.push (Json.mkObj ([
      ("declaration", toJson declName.toString), ("owner", toJson (M4Audit.definingModule env declName).toString),
      ("kind", toJson (M4Audit.kind info)), ("axioms", nameJson axioms),
      ("reachable_from_async_roots", toJson (closure.contains declName))] ++ (← typeRecord info)))
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
  logInfo <| "M6-ASYNC " ++ (Json.mkObj [
    ("schema", toJson "noble-m6-async-audit/v1"), ("result", toJson "passed"),
    ("packet", packet), ("strict_axioms", toJson (M4Audit.strictAxioms.map Name.toString)),
    ("strict_roots", toJson strictRecords), ("implementation_roots", toJson actualRecords),
    ("reference_roots", toJson referenceRecords), ("coverage_roots", toJson coverageRecords),
    ("qualified_models", toJson modelRecords),
    ("constructor_coverage", constructorCoverage),
    ("declarations", toJson declarationRecords), ("project_declarations", toJson proofRecords),
    ("transitive_dependencies", toJson dependencyRecords),
    ("scope", toJson "Universal extracted Rust transition/Table.decide/classifier correspondence to independent total checked-machine references, exact qualified async ownership invariants and complete source-bound 75-pair constructor coverage."),
    ("non_claims", toJson ([
      "No arithmetic panic-freedom theorem: checked-machine failures remain explicit in the independent total reference.",
      "No native execution, callback authenticity, physical pin release, destructor or external-effect correctness theorem.",
      "No authority, WIT parser, Canonical ABI, component compiler, engine, host storage or peer runtime correctness theorem.",
      "Finite classifier enumeration and compiled coverage-admission dependencies are not runtime conformance.",
      "Runtime protocol controls are independently executed by verification/m6/peer/src/controls/schema.rs.",
      "M4/M5 source coverage, strict proofs, peer independence and runtime conformance gates remain mandatory."] : List String))]).compress

end M6Audit
