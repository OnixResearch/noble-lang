import NobleContracts
import Lean

open Lean Elab Command

/-- This gate is distinct from checking any individual application theorem. -/
elab "check_contract_library" : command => do
  let env ← getEnv
  let required : List Name := [
    ``NobleContracts.exec_append_iff, ``NobleContracts.pc_sequence,
    ``NobleContracts.exec_lit_iff, ``NobleContracts.exec_block_iff,
    ``NobleContracts.exec_dup_iff, ``NobleContracts.exec_drop_iff,
    ``NobleContracts.exec_swap_iff, ``NobleContracts.exec_dip_iff,
    ``NobleContracts.exec_add_iff, ``NobleContracts.exec_sub_iff,
    ``NobleContracts.exec_mul_iff, ``NobleContracts.exec_equals_iff,
    ``NobleContracts.exec_quote_iff, ``NobleContracts.exec_compose_iff,
    ``NobleContracts.exec_run_iff, ``NobleContracts.exec_reflect_iff,
    ``NobleContracts.exec_unit_iff, ``NobleContracts.exec_pair_iff,
    ``NobleContracts.exec_unpair_iff, ``NobleContracts.exec_inl_iff,
    ``NobleContracts.exec_inr_iff, ``NobleContracts.exec_nil_word_iff,
    ``NobleContracts.exec_cons_iff, ``NobleContracts.exec_case_left_iff,
    ``NobleContracts.exec_case_right_iff, ``NobleContracts.exec_if_true_iff,
    ``NobleContracts.exec_if_false_iff, ``NobleContracts.exec_list_nil_iff,
    ``NobleContracts.exec_list_cons_iff, ``NobleContracts.pc_if_join,
    ``NobleContracts.pc_case_join, ``NobleContracts.pc_listcase_join,
    ``NobleContracts.pc_run, ``NobleContracts.pc_dip,
    ``NobleContracts.increment_exported, ``NobleContracts.twoIncrements_exported,
    ``NobleContracts.captureBuilder_family, ``NobleContracts.captureBuilder_exported,
    ``NobleContracts.maps_append, ``NobleContracts.snapshot_distinction,
    ``NobleContracts.undefined_not_is_undefined,
    ``NobleContracts.TypedPC.compose, ``NobleContracts.increment_missing_implication]
  for name in required do
    match env.find? name with
    | some (.thmInfo _) => pure ()
    | _ => throwError "required contract theorem absent: {name}"
    let axioms ← liftCoreM (collectAxioms name)
    for ax in axioms do
      unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
        throwError "disallowed contract axiom {ax} in {name}"
  logInfo m!"MC1-CONTRACT-LIBRARY: {required.length} required theorems; strict axioms accepted"

check_contract_library
