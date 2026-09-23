import NobleCertificates
import CertifiedInvariant
import Lean

/-!
# MC2 strict gate

This gate is distinct from checking any individual application theorem. It
audits the required MC2 rule/certificate theorems, restricts every theorem's
transitive axioms to `propext`, `Classical.choice`, `Quot.sound`, and checks the
frozen guard template set is finite with exactly the three v1 constructors.

The extracted-Rust correspondence for `noble_contracts::companion` is audited in
`MC2ExtractionGate` once the deterministic core is extracted; this gate covers
the handwritten rule/certificate layer only and never substitutes for it.
-/

open Lean Elab Command

private def strictAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

private def requiredTheorems : List Name := [
  -- companion model identity specification
  ``NobleCertificates.changed_capture_changes_identity,
  ``NobleCertificates.changed_code_changes_identity,
  -- guard templates: per-template correspondence and boundaries
  ``NobleCertificates.guard_sound,
  ``NobleCertificates.guard_sound_ltI64Max,
  ``NobleCertificates.guard_sound_neI64Min,
  ``NobleCertificates.guard_sound_eqI64Literal,
  ``NobleCertificates.guard_correspondence,
  ``NobleCertificates.wrapperProgram_exec_iff,
  ``NobleCertificates.wrapperProgram_rejects,
  ``NobleCertificates.wrapperProgram_accepts,
  ``NobleCertificates.ltI64Max_boundary,
  ``NobleCertificates.i64max_toInt,
  ``NobleCertificates.neI64Min_accepts_max,
  ``NobleCertificates.guardProgram_exec_iff,
  ``NobleCertificates.sourceWrapperProgram_refines,
  ``NobleCertificates.liveWrapperProgram_refines,
  -- finite rule set: per-rule soundness
  ``NobleCertificates.admitLean_sound,
  ``NobleCertificates.compose_sound,
  ``NobleCertificates.instantiate_sound,
  ``NobleCertificates.guard_rule_sound,
  ``NobleCertificates.project_sound,
  ``NobleCertificates.invoke_sound,
  ``NobleCertificates.rule_sound_of_supported,
  ``NobleCertificates.compose_missing_implication,
  ``NobleCertificates.composeSubject_matches_plain_compose,
  ``NobleCertificates.constructedAdd_exec_iff,
  -- certified-status invariant (PO-20)
  ``NobleCertificates.certified_evidence_accepted,
  ``NobleCertificates.accepted_cases,
  ``NobleCertificates.replay_evidence_class,
  ``NobleCertificates.flag_does_not_establish_certified,
  ``NobleCertificates.changed_context_invalidates,
  ``NobleCertificates.changed_semantic_invalidates,
  ``NobleCertificates.project_preserves_subject,
  ``NobleCertificates.erasure_preserves_observation,
  ``NobleCertificates.evidence_attachment_preserves_identity]

private def requiredDefinitions : List Name := [
  ``NobleCertificates.Satisfies, ``NobleCertificates.guardRejects,
  ``NobleCertificates.guardProgram, ``NobleCertificates.wrapperProgram,
  ``NobleCertificates.composeSubject, ``NobleCertificates.instantiateSubject]

private def requiredInductives : List Name := [
  ``NobleCertificates.Replay, ``NobleCertificates.Accepted, ``NobleCertificates.Certified]

elab "check_mc2_gate" : command => do
  let env ← getEnv
  for name in requiredTheorems do
    match env.find? name with
    | some (.thmInfo _) => pure ()
    | _ => throwError "required MC2 theorem absent: {name}"
    for axiomName in ← liftCoreM (collectAxioms name) do
      unless strictAxioms.contains axiomName do
        throwError "disallowed MC2 axiom {axiomName} in {name}"
  for name in requiredDefinitions do
    match env.find? name with
    | some (.defnInfo _) => pure ()
    | _ => throwError "required MC2 definition absent: {name}"
  for name in requiredInductives do
    match env.find? name with
    | some (.inductInfo _) => pure ()
    | _ => throwError "required MC2 inductive absent: {name}"
  -- The guard template set is finite and frozen: exactly the three v1 templates.
  let templates ← match env.find? ``NobleCertificates.GuardTemplate with
    | some (.inductInfo value) => pure value.ctors
    | _ => throwError "guard template set absent"
  unless templates.length == 3 do
    throwError "guard template set is not the frozen three-constructor v1 set: {templates.length}"
  logInfo <| "MC2-GATE " ++ (Json.mkObj [
    ("schema", toJson "mc2-gate/v1"),
    ("result", toJson "passed"),
    ("required_theorems", toJson requiredTheorems.length),
    ("required_definitions", toJson requiredDefinitions.length),
    ("guard_templates", toJson templates.length),
    ("strict_axioms", toJson (strictAxioms.map Name.toString)),
    ("scope", toJson "compiled semantic certificate theorems; actual Rust correspondence is a separate required audit"),
    ("extracted_correspondence", toJson "separate-required-MC2ExtractionGate")]).compress

check_mc2_gate
