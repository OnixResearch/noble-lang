import NobleCertificates.GuardsV2
import Lean

/-!
# MC2 v2 strict gate

This gate audits the versioned v2 guard template set. It is separate from
`MC2Gate`, which stays frozen on the three-constructor v1 set and must still
pass unchanged. Both gates restrict every theorem's transitive axioms to
`propext`, `Classical.choice` and `Quot.sound`.

The gate checks that:

- every required v2 theorem exists and is axiom-clean;
- `GuardTemplateV2` has exactly the four v2 constructors (the three frozen v1
  templates plus `nonEmptyList`);
- `GuardTemplate` still has exactly the three v1 constructors, and the `Type0`
  code predicate has exactly the seven pure constructors;
- the recorded set version is 2.

It does not audit a parser, a renderer string equation or the extracted-Rust
correspondence for `templates_of_v2`; those are separate obligations.
-/

open Lean Elab Command

private def strictAxioms : List Name := [``propext, ``Classical.choice, ``Quot.sound]

private def requiredTheorems : List Name := [
  -- the pure Type0 element-code fragment
  ``NobleCertificates.type0_list_i64,
  ``NobleCertificates.type0_nested_list,
  ``NobleCertificates.program_is_not_type0,
  ``NobleCertificates.syntax_is_not_type0,
  -- exact interface formation
  ``NobleCertificates.nonEmptyList_interface_i64,
  -- C1 at model level for the v2 list template
  ``NobleCertificates.nonEmptyList_link,
  ``NobleCertificates.nonEmptyList_link_of_type,
  -- C2: the guard text computes the flag and keeps the argument
  ``NobleCertificates.rejectionProgram_exec_any,
  ``NobleCertificates.acceptProgram_exec_any,
  ``NobleCertificates.guardProgramV2_nonEmptyList_exec_iff,
  ``NobleCertificates.guardProgramV2_exec_iff,
  -- C3: soundness and exactness
  ``NobleCertificates.nonEmptyList_sound,
  ``NobleCertificates.nonEmptyList_exact,
  ``NobleCertificates.guard_sound_v2,
  -- C4: the wrapper
  ``NobleCertificates.wrapperProgramV2_exec_iff,
  ``NobleCertificates.wrapperProgramV2_rejects,
  ``NobleCertificates.wrapperProgramV2_accepts,
  ``NobleCertificates.guard_correspondence_v2,
  -- C5: the fixed renderer texts refine C4
  ``NobleCertificates.sourceWrapperProgramV2_refines,
  ``NobleCertificates.liveWrapperProgramV2_refines,
  -- frozen set version
  ``NobleCertificates.guardTemplateSetVersionV2_eq]

private def requiredDefinitions : List Name := [
  ``NobleCertificates.guardTemplateSetVersionV2,
  ``NobleCertificates.GuardTemplateV2.fromV1,
  ``NobleCertificates.GuardTemplateV2.interface,
  ``NobleCertificates.HasInput,
  ``NobleCertificates.GuardPreconditionV2,
  ``NobleCertificates.guardRejectsV2,
  ``NobleCertificates.guardProgramV2,
  ``NobleCertificates.wrapperProgramV2,
  ``NobleCertificates.sourceWrapperProgramV2,
  ``NobleCertificates.liveWrapperProgramV2,
  ``NobleCertificates.nonEmptyListTerm]

private def requiredInductives : List Name := [
  ``NobleCertificates.Type0Code, ``NobleCertificates.GuardTemplateV2]

private def constructorCount (name : Name) : CommandElabM Nat := do
  let env ← getEnv
  match env.find? name with
  | some (.inductInfo value) => pure value.ctors.length
  | _ => throwError "required inductive absent: {name}"

elab "check_mc2_v2_gate" : command => do
  let env ← getEnv
  for name in requiredTheorems do
    match env.find? name with
    | some (.thmInfo _) => pure ()
    | _ => throwError "required MC2 v2 theorem absent: {name}"
    for axiomName in ← liftCoreM (collectAxioms name) do
      unless strictAxioms.contains axiomName do
        throwError "disallowed MC2 v2 axiom {axiomName} in {name}"
  for name in requiredDefinitions do
    match env.find? name with
    | some (.defnInfo _) => pure ()
    | _ => throwError "required MC2 v2 definition absent: {name}"
  for name in requiredInductives do
    match env.find? name with
    | some (.inductInfo _) => pure ()
    | _ => throwError "required MC2 v2 inductive absent: {name}"
  -- The v2 set is finite and frozen: exactly the three v1 templates plus
  -- `nonEmptyList`. The v1 set itself must still have exactly three.
  let v2Templates ← constructorCount ``NobleCertificates.GuardTemplateV2
  unless v2Templates == 4 do
    throwError "v2 guard template set is not the frozen four-constructor set: {v2Templates}"
  let v1Templates ← constructorCount ``NobleCertificates.GuardTemplate
  unless v1Templates == 3 do
    throwError "frozen v1 guard template set changed: {v1Templates}"
  let type0Codes ← constructorCount ``NobleCertificates.Type0Code
  unless type0Codes == 7 do
    throwError "Type0 code predicate is not the frozen seven-constructor set: {type0Codes}"
  logInfo <| "MC2-V2-GATE " ++ (Json.mkObj [
    ("schema", toJson "mc2-v2-gate/v1"),
    ("result", toJson "passed"),
    ("template_set_version", toJson (2 : Nat)),
    ("required_theorems", toJson requiredTheorems.length),
    ("required_definitions", toJson requiredDefinitions.length),
    ("guard_templates_v2", toJson v2Templates),
    ("guard_templates_v1", toJson v1Templates),
    ("type0_codes", toJson type0Codes),
    ("strict_axioms", toJson (strictAxioms.map Name.toString)),
    ("scope", toJson "compiled v2 semantic guard theorems; extracted-Rust correspondence, renderer parsing and Wasm lowering are separate required audits")]).compress

check_mc2_v2_gate
