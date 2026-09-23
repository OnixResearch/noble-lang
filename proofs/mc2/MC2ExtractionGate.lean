import NobleCertificates.Correspondence
import NobleCertificates.SourceReplay
import M4Audit
import Lean

/-!
Compiled evidence is partitioned into strict actual-source lemmas, renderer
layout equations carrying only generated literal-size bounds, and explicitly
native closed source equations. None of the latter are universal interpreter
refinement or host-checker soundness theorems. Whole-crate extraction, source and
tool hashes, and transparent standard models are checked by the M4 driver.
-/

open Lean Elab Command

set_option Aeneas.customDoElab false
set_option maxHeartbeats 1000000
set_option maxRecDepth 2048

private def strictAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

private def strictSubjects : List (Name × List Name) := [
  (``NobleCertificates.Correspondence.rule_decode_roundtrip,
    [``noble_contracts.companion.rules.RuleId.decode,
     ``NobleCertificates.Correspondence.ruleCode,
     ``NobleCertificates.Correspondence.rustRule]),
  (``NobleCertificates.Correspondence.rule_decode_bad_version,
    [``noble_contracts.companion.rules.RuleId.decode]),
  (``NobleCertificates.Correspondence.rule_decode_unknown,
    [``noble_contracts.companion.rules.RuleId.decode]),
  (``NobleCertificates.Correspondence.rule_code_corresponds,
    [``noble_contracts.companion.rules.RuleId.code,
     ``NobleCertificates.Correspondence.ruleCode]),
  (``NobleCertificates.Correspondence.guard_template_roundtrip,
    [``NobleCertificates.Correspondence.rustGuard,
     ``NobleCertificates.Correspondence.semanticGuard]),
  (``NobleCertificates.Correspondence.release_requires_proved,
    [``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.Correspondence.release_rejects_assumptions,
    [``noble_contracts.companion.core.Core.release])]

private def rendererSubjects : List (Name × List Name) := [
  (``NobleCertificates.Correspondence.wrapper_source_layout,
    [``noble_contracts.companion.guard.wrapper_source,
     ``NobleCertificates.Correspondence.sourceLayout]),
  (``NobleCertificates.Correspondence.invocation_wrapper_layout,
    [``noble_contracts.companion.guard.invocation_wrapper,
     ``NobleCertificates.Correspondence.invocationLayout])]

private def sourceSubjects : List (Name × Name × List Name) := [
  (``NobleCertificates.SourceAdmission.checked_increment_is_proved_and_releasable,
    ``NobleCertificates.SourceAdmission.provedAndReleased,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.rendering.export_lean,
     ``noble_contracts.companion.core.Core.release, ``noble_contracts.companion.core.Core.correspond,
     ``noble_contracts.companion.core.Core.contract_program, ``noble_contracts.companion.core.Core.claim,
     ``noble_contracts.companion.core.Core.evidence_rule]),
  (``NobleCertificates.SourceAdmission.checked_guarded_increment_is_proved_and_releasable,
    ``NobleCertificates.SourceAdmission.provedAndReleased,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.rendering.export_lean,
     ``noble_contracts.companion.core.Core.release, ``noble_contracts.companion.core.Core.correspond,
     ``noble_contracts.companion.core.Core.contract_program, ``noble_contracts.companion.core.Core.claim]),
  (``NobleCertificates.SourceAdmission.raw_claimed_proof_cannot_grant_status,
    ``NobleCertificates.SourceAdmission.rawClaimedProof,
    [``noble_contracts.companion.core.Core.admit, ``noble_contracts.companion.core.Core.release,
     ``noble_contracts.companion.core.Core.claim]),
  (``NobleCertificates.SourceAdmission.checked_verifier_refusal_cannot_release,
    ``NobleCertificates.SourceAdmission.refusingHostObservation,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceAdmission.checked_wrong_evidence_class_cannot_release,
    ``NobleCertificates.SourceAdmission.checkedIncrementWith,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceAdmission.checked_refutation_class_mismatch_cannot_release,
    ``NobleCertificates.SourceAdmission.checkedIncrementWith,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceAdmission.checked_changed_declaration_cannot_release,
    ``NobleCertificates.SourceAdmission.incrementOffer,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.rendering.export_lean,
     ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceAdmission.checked_changed_statement_cannot_release,
    ``NobleCertificates.SourceAdmission.changedStatementAdmission,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.rendering.export_lean,
     ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceAdmission.resource_evidence_is_refused_before_retention,
    ``NobleCertificates.SourceAdmission.capabilityRefused,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.release,
     ``noble_contracts.companion.core.Core.contract_program]),
  (``NobleCertificates.SourceAdmission.service_capability_evidence_is_refused_before_retention,
    ``NobleCertificates.SourceAdmission.capabilityRefused,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.release,
     ``noble_contracts.companion.core.Core.contract_program]),
  (``NobleCertificates.SourceAdmission.authentic_observation_wrong_class_cannot_mint,
    ``NobleCertificates.SourceAdmission.completeIncrementWith,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.companion.admit.observation.CheckObservation.new, ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceAdmission.changed_policy_cannot_complete_pending_request,
    ``NobleCertificates.SourceAdmission.contextChangedAdmission,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.companion.core.context.Core.set_policy, ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceAdmission.restored_policy_cannot_complete_stale_request,
    ``NobleCertificates.SourceAdmission.contextChangedAdmission,
    [``noble_contracts.companion.core.Core.begin_admission, ``noble_contracts.companion.core.Core.complete_admission,
     ``noble_contracts.companion.core.context.Core.set_policy, ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceReplay.composition_replay_retains_certification,
    ``NobleCertificates.SourceReplay.compositionReplay,
    [``noble_contracts.companion.core.Core.derive_compose, ``noble_contracts.companion.core.Core.compose_subject,
     ``noble_contracts.companion.core.Core.replay, ``noble_contracts.companion.core.Core.claim,
     ``noble_contracts.companion.core.Core.evidence_rule, ``noble_contracts.companion.core.Core.bind,
     ``noble_contracts.companion.core.Core.release, ``noble_contracts.companion.core.Core.evidence_guard_templates]),
  (``NobleCertificates.SourceReplay.malformed_replay_refuses_without_minting,
    ``NobleCertificates.SourceReplay.rejectedReplay,
    [``noble_contracts.companion.core.Core.replay, ``noble_contracts.companion.core.Core.derive_compose,
     ``noble_contracts.companion.core.Core.release]),
  (``NobleCertificates.SourceReplay.exact_application_and_context_changes,
    ``NobleCertificates.SourceReplay.applicationTransitions,
    [``noble_contracts.companion.core.Core.bind, ``noble_contracts.companion.core.Core.observe,
     ``noble_contracts.companion.core.Core.contract_program, ``noble_contracts.companion.core.Core.release,
     ``noble_contracts.companion.core.context.Core.set_policy, ``noble_contracts.companion.core.context.Core.set_semantic_revision,
     ``noble_contracts.companion.core.context.Core.set_host_contract, ``noble_contracts.companion.core.context.Core.set_environment_fact]),
  (``NobleCertificates.SourceReplay.guarded_selection_and_wrapper_layout,
    ``NobleCertificates.SourceReplay.guardedSelection,
    [``noble_contracts.companion.core.Core.guard_templates,
     ``noble_contracts.companion.core.Core.evidence_guard_templates,
     ``noble_contracts.companion.core.Core.wrapper_source,
     ``noble_contracts.companion.core.Core.invocation_wrapper,
     ``noble_contracts.companion.core.Core.release, ``noble_contracts.companion.core.context.Core.set_policy])]

private def sourceOwners : List Name := [
  `NobleCertificates.SourceSupport, `NobleCertificates.SourceAdmission,
  `NobleCertificates.SourceReplay]

private def owner (env : Environment) (declName : Name) : Name :=
  match env.getModuleIdxFor? declName with
  | some index => env.header.moduleNames[index.toNat]?.getD .anonymous
  | none => .anonymous

private def nativeDecision? (env : Environment) (declName : Name) : Option Expr := do
  let .str (.str (.str _ "_native") evaluator) suffix := declName | none
  unless evaluator == "decide" || evaluator == "native_decide" do none
  unless suffix.startsWith "ax_" && suffix.length > 3 &&
      (suffix.toList.drop 3).all (fun character => character.isDigit || character == '_') do none
  let some (.axiomInfo info) := env.find? declName | none
  if info.isUnsafe || !info.levelParams.isEmpty then none else do
    let type := info.type.consumeMData
    unless type.isAppOfArity ``Eq 3 do none
    let equation := type.getAppArgs
    unless equation[0]!.isConstOf ``Bool && equation[2]!.isConstOf ``Bool.true do none
    let decision := equation[1]!.consumeMData
    unless decision.isAppOfArity ``decide 2 do none
    some decision.getAppArgs[0]!

/-- No native axiom may assert anything except a listed, closed source equation
or a literal byte-size bound. Matching a theorem name alone is insufficient. -/
private def auxiliaryAxiomKind (env : Environment) (declName : Name) :
    CommandElabM (Option String) := do
  if M4Audit.isGeneratedLiteralSizeAxiom env declName then
    return some "generated-literal-byte-size"
  unless sourceOwners.contains (owner env declName) do return none
  let some proposition := nativeDecision? env declName | return none
  for (subject, _, _) in sourceSubjects do
    if subject == declName.getPrefix.getPrefix.getPrefix then
      if let some (.thmInfo info) := env.find? subject then
        if owner env subject == owner env declName &&
            (← liftTermElabM (Meta.withTransparency .reducible
              (Meta.isDefEq proposition info.type))) then
          return some "closed-source-equation-native-evaluation"
  let proposition := proposition.consumeMData
  unless proposition.isAppOfArity ``LE.le 4 do return none
  let bound := proposition.getAppArgs
  unless bound[0]!.isConstOf ``Nat && bound[3]!.isConstOf ``Aeneas.Std.U32.max do return none
  let size := bound[2]!.consumeMData
  unless size.isAppOfArity ``ByteArray.size 1 do return none
  let bytes := size.appArg!.consumeMData
  unless bytes.isAppOfArity ``String.toByteArray 1 do return none
  let literal ← liftTermElabM (Meta.whnf bytes.appArg!)
  match literal.consumeMData with
  | .lit (.strVal _) => return some "source-fixture-literal-byte-size"
  | _ => return none

/-- Traverse compiled scenario helpers and actual extracted production bodies.
This establishes declaration dependencies, not an execution trace. -/
private def sourceDependencies (env : Environment) (expression : Expr) :
    CommandElabM NameSet := do
  let mut seen : NameSet := {}
  let mut pending := expression.getUsedConstants
  while !pending.isEmpty do
    let current := pending.back!
    pending := pending.pop
    unless seen.contains current do
      seen := seen.insert current
      if sourceOwners.contains (owner env current) ||
          owner env current == `NobleContractImpl.Funs then
        let some info := env.find? current | throwError "MC2-SOURCE: missing helper {current}"
        pending := pending ++ info.type.getUsedConstants
        if let some body := info.value? (allowOpaque := true) then
          pending := pending ++ body.getUsedConstants
  return seen

private def closedObservation (expression : Expr) : Bool := Id.run do
  let type := expression.consumeMData
  unless type.isAppOfArity ``Eq 3 do return false
  let equation := type.getAppArgs
  return equation[0]!.isConstOf ``Bool && equation[2]!.isConstOf ``Bool.true &&
    equation[1]!.isAppOfArity ``NobleCertificates.SourceSupport.observed 1

elab "check_mc2_extraction" : command => do
  let env := (← getEnv).setExporting false
  let mut records : Array Json := #[]
  let mut sourceRecords : Array Json := #[]
  let mut auxiliary : NameSet := {}
  for (isRenderer, subjects) in [(false, strictSubjects), (true, rendererSubjects)] do
    for (declName, required) in subjects do
      let info ← match env.find? declName with
        | some (.thmInfo info) => pure info
        | _ => throwError "MC2-CORRESPONDENCE: missing theorem {declName}"
      unless owner env declName == `NobleCertificates.Correspondence do
        throwError "MC2-CORRESPONDENCE: substituted theorem owner {declName}"
      for dependency in required do
        unless info.type.getUsedConstants.contains dependency do
          throwError "MC2-CORRESPONDENCE: {declName} does not state a result about {dependency}"
      let axioms ← liftCoreM (collectAxioms declName)
      for axiomName in axioms do
        unless strictAxioms.contains axiomName do
          unless isRenderer && M4Audit.isGeneratedLiteralSizeAxiom env axiomName do
            throwError "MC2-CORRESPONDENCE: unsupported axiom {axiomName} in {declName}"
          auxiliary := auxiliary.insert axiomName
      let rendered ← liftTermElabM (Meta.ppExpr info.type)
      records := records.push (Json.mkObj [
        ("declaration", toJson declName.toString),
        ("classification", toJson (if isRenderer then "renderer-layout-with-generated-literal-size-bounds" else "strict-actual-source-correspondence")),
        ("type", toJson rendered.pretty),
        ("axioms", toJson ((axioms.qsort Name.lt).map Name.toString))])
  for (declName, scenario, required) in sourceSubjects do
    let info ← match env.find? declName with
      | some (.thmInfo info) => pure info
      | _ => throwError "MC2-SOURCE: missing theorem {declName}"
    unless sourceOwners.contains (owner env declName) && info.levelParams.isEmpty &&
        closedObservation info.type && info.type.getUsedConstants.contains scenario do
      throwError "MC2-SOURCE: not the required closed observation {declName}"
    let dependencies ← sourceDependencies env info.type
    let production := [``noble_contracts.prepare,
      ``noble_contracts.Limits.Insts.CoreDefaultDefault.default,
      ``noble_contracts.companion.core.context.Core.new] ++ required
    for dependency in production do
      unless dependencies.contains dependency do
        throwError "MC2-SOURCE: {declName} detached from extracted {dependency}"
      unless owner env dependency == `NobleContractImpl.Funs do
        throwError "MC2-SOURCE: substituted extracted dependency {dependency}"
    let axioms ← liftCoreM (collectAxioms declName)
    for axiomName in axioms do
      unless strictAxioms.contains axiomName do
        unless (← auxiliaryAxiomKind env axiomName).isSome do
          throwError "MC2-SOURCE: unsupported axiom {axiomName} in {declName}"
        auxiliary := auxiliary.insert axiomName
    let rendered ← liftTermElabM (Meta.ppExpr info.type)
    sourceRecords := sourceRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString),
      ("classification", toJson "native-closed-extracted-source-equation"),
      ("type", toJson rendered.pretty),
      ("required_production_dependencies", toJson (production.map Name.toString)),
      ("axioms", toJson ((axioms.qsort Name.lt).map Name.toString))])
  let owned := env.constants.fold (init := (#[] : Array Name)) fun declarations declName _ =>
    if owner env declName == `NobleCertificates.Correspondence ||
        sourceOwners.contains (owner env declName) then declarations.push declName else declarations
  let mut ownedRecords : Array Json := #[]
  for declName in owned.qsort Name.lt do
    let some info := env.find? declName | throwError "MC2-CORRESPONDENCE: missing declaration {declName}"
    match info with
    | .axiomInfo _ =>
      unless (← auxiliaryAxiomKind env declName).isSome do
        throwError "MC2-CORRESPONDENCE: project axiom {declName}"
    | .opaqueInfo _ => throwError "MC2-CORRESPONDENCE: opaque project declaration {declName}"
    | .defnInfo info =>
      unless info.safety == .safe do
        throwError "MC2-CORRESPONDENCE: unsafe/partial project definition {declName}"
    | _ => pure ()
    let axioms ← liftCoreM (collectAxioms declName)
    for axiomName in axioms do
      unless strictAxioms.contains axiomName do
        let kind ← auxiliaryAxiomKind env axiomName
        unless kind.isSome && (owner env declName != `NobleCertificates.Correspondence ||
            kind == some "generated-literal-byte-size") do
          throwError "MC2-CORRESPONDENCE: unsupported unused/helper axiom {axiomName} in {declName}"
        auxiliary := auxiliary.insert axiomName
    ownedRecords := ownedRecords.push (Json.mkObj [
      ("declaration", toJson declName.toString),
      ("axioms", toJson ((axioms.qsort Name.lt).map Name.toString))])
  let mut stringRecords : Array Json := #[]
  let mut nativeSourceRecords : Array Json := #[]
  for declName in auxiliary.toArray.qsort Name.lt do
    let some info := env.find? declName | throwError "MC2-CORRESPONDENCE: missing native obligation"
    let some kind ← auxiliaryAxiomKind env declName | throwError "MC2-CORRESPONDENCE: unclassified native obligation"
    let rendered ← liftTermElabM (Meta.ppExpr info.type)
    let record := Json.mkObj [
      ("declaration", toJson declName.toString), ("owner", toJson (owner env declName).toString),
      ("classification", toJson kind), ("type", toJson rendered.pretty)]
    if kind == "closed-source-equation-native-evaluation" then
      nativeSourceRecords := nativeSourceRecords.push record
    else stringRecords := stringRecords.push record
  logInfo <| "MC2-EXTRACTION " ++ (Json.mkObj [
    ("schema", toJson "mc2-extraction-correspondence/v1"), ("result", toJson "passed"),
    ("theorems", toJson records), ("strict_theorems", toJson strictSubjects.length),
    ("renderer_theorems", toJson rendererSubjects.length),
    ("source_equations", toJson sourceRecords), ("source_equation_count", toJson sourceSubjects.length),
    ("strict_axioms", toJson (strictAxioms.map Name.toString)),
    ("project_declarations", toJson ownedRecords),
    ("native_string_obligations", toJson stringRecords),
    ("native_source_equation_obligations", toJson nativeSourceRecords),
    ("source_fixtures", toJson ([(false, "increment"), (true, "guarded")].map
      (fun (guarded, fixture) => Json.mkObj [
        ("fixture", toJson fixture),
        ("source", toJson (NobleCertificates.SourceAdmission.incrementSource guarded)),
        ("declaration", toJson (NobleCertificates.SourceAdmission.incrementDeclaration guarded)),
        ("expected_statement", toJson (NobleCertificates.SourceAdmission.incrementStatement guarded))]))),
    ("host_checker_boundary", toJson "The two exact named MC1 statement/declaration observations are assumed authentic results from the sound external host checker. Constructing CheckObservation is not a proof of authenticity or checker soundness."),
    ("scope", toJson "Strict finite-rule decoding, release eligibility and wrapper layout; closed extracted admission, replay, applicability, stale-context and guard/release transitions; separate strict semantic wrapper execution."),
    ("non_claims", toJson ([
      "No universal Rust replay-to-PC or checker soundness theorem.",
      "No collision-free finite digest, generic decimal parser, source parser or Wasm lowering theorem.",
      "The native closed source equations are not strict universal semantic theorems.",
      "Authenticity and soundness of host-produced CheckObservation values are explicit boundary assumptions."] : List String))]).compress

check_mc2_extraction
