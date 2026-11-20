import NobleContracts
import NobleContracts.Refutation
import NobleContracts.NamedV3
import NobleContracts.SpecQuality
import NobleContracts.Wire
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
    ``NobleContracts.TypedPC.compose, ``NobleContracts.increment_missing_implication,
    ``NobleContracts.NamedV3.exec_append_iff, ``NobleContracts.NamedV3.holds_true₂,
    ``NobleContracts.NamedV3.holds_eq₂_iff,
    ``NobleContracts.NamedV3.embed, ``NobleContracts.NamedV3.run_forward,
    ``NobleContracts.NamedV3.run_backward, ``NobleContracts.NamedV3.callFrame,
    ``NobleContracts.NamedV3.liftMc1Claim, ``NobleContracts.NamedV3.chainClaim,
    ``NobleContracts.NamedV3.chainExec, ``NobleContracts.NamedV3.run_reflect,
    ``NobleContracts.NamedV3.exec_reflect, ``NobleContracts.NamedV3.stackTyped_lift,
    ``NobleContracts.NamedV3.holds₂_env_free, ``NobleContracts.NamedV3.evaluate₂_env_free,
    ``NobleContracts.NamedV3.Subject.resolves, ``NobleContracts.NamedV3.Subject.root_resolves,
    ``NobleContracts.NamedV3.callsResolve_of_resolves, ``NobleContracts.NamedV3.run_nil_iff,
    ``NobleContracts.NamedV3.EntryEmbedding.slot_not_snapshot, ``NobleContracts.NamedV3.Embedding.slot_fresh,
    -- The exact reviewed rule identities of `V-INTRINSIC-01` / the
    -- `Named-Contract-Calls-v3` profile. They are Lean core/Std theorems, not
    -- axioms, and are named here rather than in a `MODEL` module so no
    -- historical revision digest moves; `Rules.lean` stays byte-identical.
    ``List.append_assoc, ``List.nil_append, ``List.append_nil,
    ``BitVec.add_assoc, ``BitVec.add_comm, ``BitVec.mul_add,
    ``NobleContracts.SpecQuality.increment_inhabited,
    ``NobleContracts.SpecQuality.increment_realizable,
    ``NobleContracts.SpecQuality.increment_deterministic,
    ``NobleContracts.SpecQuality.contradictory_not_realizable,
    ``NobleContracts.SpecQuality.false_precondition_is_vacuous,
    ``NobleContracts.SpecQuality.underconstrained_has_two_outputs,
    ``NobleContracts.SpecQuality.undefined_predicate_never_satisfied,
    ``NobleContracts.SpecQuality.undefined_post_not_realizable,
    ``NobleContracts.SpecQuality.obligation_kinds_distinct,
    ``NobleContracts.SpecQuality.separate_evidence_for_separate_obligations,
    ``NobleContracts.SpecQuality.witness_recheck_recovers_the_assignment,
    ``NobleContracts.SpecQuality.witness_is_not_uniqueness,
    ``NobleContracts.SpecQuality.accepted_submission_yields_the_derived_witness,
    ``NobleContracts.SpecQuality.wrong_obligation_cannot_be_accepted,
    ``NobleContracts.SpecQuality.wrong_observation_relation_cannot_be_accepted,
    ``NobleContracts.SpecQuality.wrong_parameter_binding_cannot_be_accepted,
    ``NobleContracts.SpecQuality.asserted_satisfaction_yields_no_artifact,
    ``NobleContracts.SpecQuality.admissible_pins_its_lengths,
    ``NobleContracts.SpecQuality.a_shrunk_assignment_never_rechecks,
    ``NobleContracts.SpecQuality.a_prefix_candidate_is_refused,
    ``NobleContracts.SpecQuality.every_shorter_candidate_is_refused,
    ``NobleContracts.Wire.exec_frame,
    ``NobleContracts.Wire.then_left_id, ``NobleContracts.Wire.then_right_id,
    ``NobleContracts.Wire.then_assoc, ``NobleContracts.Wire.apply_quote,
    ``NobleContracts.Wire.apply_then, ``NobleContracts.Wire.under_fuse,
    ``NobleContracts.Wire.pair_unpair, ``NobleContracts.Wire.unpair_pair,
    ``NobleContracts.Wire.swap_pair_invol, ``NobleContracts.Wire.assoc_pair_iso,
    ``NobleContracts.Wire.assoc_pair_iso_symm, ``NobleContracts.Wire.first_fuse,
    ``NobleContracts.Wire.split_def, ``NobleContracts.Wire.fanout_def,
    ``NobleContracts.Wire.left_beta_inl, ``NobleContracts.Wire.left_beta_inr,
    ``NobleContracts.Wire.join_natural_inl, ``NobleContracts.Wire.join_natural_inr,
    ``NobleContracts.Wire.if_beta_true, ``NobleContracts.Wire.if_beta_false,
    ``NobleContracts.Wire.recipes_run_alike, ``NobleContracts.Wire.recipe_distinct,
    -- Tactic-free unary-I64 refutation template (PAUTO-FO-4), kept outside `MODEL`:
    -- a concrete normal-return counterexample refutes the exported universal claim.
    ``NobleContracts.value_bool_false_ne_true,
    ``NobleContracts.some_value_bool_false_ne_true,
    ``NobleContracts.not_holds_of_evaluate_false,
    ``NobleContracts.exportedClaim_unaryI64_refutation,
    ``NobleContracts.monotonic_counterexample_run,
    ``NobleContracts.monotonic_postcondition_false,
    ``NobleContracts.monotonic_refutation,
    ``NobleContracts.wrap_constant_counterexample_run,
    ``NobleContracts.wrap_constant_postcondition_false,
    ``NobleContracts.wrap_constant_refutation]
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
