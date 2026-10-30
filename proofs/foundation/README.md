# Noble mathematical correctness foundation (issue #15)

This Lake project is a narrow, inspectable correctness chain for **signed
wrapping I64 arithmetic plus a small quote/compose/run fragment** of the
`Core-Bootstrap` profile. It connects three things:

- the independent Mathlib `ZMod (2^64)` oracle used by the comparator
  ([`proofs/m3/MathlibI64.lean`](../m3/MathlibI64.lean));
- the MC1 typed operational model
  ([`proofs/mc1/NobleContracts`](../mc1/NobleContracts));
- the executable evidence for the compiled Wasm.

Both Lean sources are compiled from their existing files through
`srcDir`. Nothing is copied, so the theorems are about exactly those
definitions.

**Every theorem here is about a model.** None of them is an
implementation theorem about the Rust frontend, `noble-wasm` lowering,
runtime helpers, `wasm-opt`, V8 or the loader. The tables below show
where each claim stops.

The separate input-specific EMI lane adds one scalar `if` or I64 sum `case`
through [`NobleFoundation/Branch.lean`](NobleFoundation/Branch.lean). Its
branch theorems do not extend the arithmetic comparator's whitelist or
claim universal equivalence of the generated original and variant.

## Reproduce

Prerequisites:
- the policy-selected Lean 4.31.0 (`policy/tool-selection.json`
  `tool_paths.lean.output`; build it with `nix build .#lean`);
- the Node at `tool_paths.node.output`.

Pins are identical to `proofs/m3` and `proofs/mc1`. The checker refuses
any difference: Lean `v4.31.0`, Aeneas `505b6ca3`, Mathlib4 `fabf563a`.

```sh
cd proofs/foundation
PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
  lake exe cache get        # downloads Mathlib .oleans (not rebuilt)
cd ../..
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
OUT=$HOME/.cache/noble-foundation-$(date +%s); mkdir -p "$OUT"
# 1. proofs, axioms, inventory -> foundation-check.json
env -i "$NODE" verification/foundation/check.mjs --output "$OUT/check"
# 2. Phase 3 slice (needs the user systemd manager for the sandboxed Lean consumer)
env -i XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" "$NODE" verification/foundation/slice.mjs \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/slice"
# 3. Phase 4 differential comparison (+ kernel 1000/200 checker lanes)
env -i "$NODE" verification/foundation/differential.mjs --seed 15 --count 80 \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/differential"
# 4. reject stale evidence: succeeds only on the clean tree the receipt names
env -i "$NODE" verification/foundation/differential.mjs --verify "$OUT/differential/differential.json"
# 5. Input-specific branch EMI, with independent admission and fresh Lean checks
env -i "$NODE" --no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up \
  --stack-size=1024 --max-old-space-size=256 \
  verification/foundation/differential.mjs --emi --seed 53 --count 16 \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/emi"
env -i "$NODE" verification/foundation/differential.mjs --emi --verify "$OUT/emi/emi.json"
```

The existing fixed-vector comparator
(`verification/mathlib-i64/compare.mjs`, 12 vectors, opt off) is
unchanged. It needs `proofs/m3/.lake` populated the same way
(`lake exe cache get` in `proofs/m3`).

`check.mjs` does the following:
- refuses a dirty worktree (`--allow-dirty` records `clean_source:false`)
  and ambient `LEAN*`/`LAKE*`/`NODE*`/`GIT_*`/`NIX_*`/loader overrides;
- requires every Lake package checkout to be clean at its manifest revision;
- scans `NobleFoundation/` for `sorry`, `acquire`, `native_decide`,
  `implemented_by`, `extern`, `unsafe`, `opaque`, `partial` and `axiom`;
- requires the declared theorems and [`inventory.json`](inventory.json) to
  be the same set;
- builds `NobleFoundation` and fails on any warning;
- for every inventoried theorem, prints `#check` and `#print axioms` through
  `lean --json`, and accepts only `propext`, `Classical.choice` and
  `Quot.sound`;
- writes `foundation-check.json`, which records:
  - source commit/tree;
  - the SHA-256 of every Lean source, the MC1 model, the oracle, the
    pins and the checker;
  - the Lean binary hash;
  - package revisions and trees;
  - each theorem's printed statement and axioms.

**Trust assumption:** the Mathlib `.olean` files come from the Lake cache
download and are not rebuilt. The Mathlib *source* checkout is clean at the
pin. The Lean kernel checks this project's proofs against those imported
`.olean` files.

## Selected profile and whitelist

| Item | Selected | Notes |
|---|---|---|
| Profile | `Core-Bootstrap`, backend `managed-linear-memory` | Calculator-v1 (`44329dc`, `crates/noble-cli/src/calculator.rs`, `runtime/calculator*.wat`) is a disjoint experimental profile. Its comparison, div/rem and guest exact arithmetic inherit **nothing** from this fragment |
| Values | `I64` only on the operand stack. Program values are built by the fragment | Bool/Text/pairs/sums/lists, resources and host effects (`test.emit` 22, `test.abort` 23) are outside it |
| Literals | decimal `[-]digits` accepted iff the value is in `[-2^63, 2^63)` | out-of-range literals are **rejected** (`integer literal is outside I64`) and never wrapped |
| Words | `+` (4), `-` (5), `*` (6), `quote` (8), `compose` (9), `run` (10), `reflect` (11, recipe observation only), literal blocks `[ … ]` | `=` (7), `dip`, `swap`, `dup`, `drop` and branching are outside the theorems. MC1 models them, but they are not part of this fragment |
| Definitions/imports | none | named calls and declared modules belong to #7 |
| Optimization | the model is mode-independent | off/on behavior is a Wasm/optimizer claim, see Phase 3/4 |

This table describes the original arithmetic/Program fragment. The bounded
EMI branch extension and its excluded observations are specified separately
under Phase 4 below.

### Representation relation and observations

- `NobleFoundation.Represents v n :≡ I64Range n ∧ v = .i64 (BitVec.ofInt 64 n)`
  with `I64Range n :≡ -2^63 ≤ n < 2^63`. This relation is functional, and
  every well-typed MC1 I64 value represents exactly its `BitVec.toInt`.
- `toZ : BitVec 64 → ZMod (2^64)` and `ofZ` are mutually inverse ring
  maps. `MathlibI64.signed (toZ x) = x.toInt`.
- `MathlibI64.wrap z` is related to the other descriptions as follows:
  - `= Int.bmod z (2^64)`;
  - `= (BitVec.ofInt 64 z).toInt`;
  - `= ((z + 2^63) mod 2^64) - 2^63`, which is the K-NUM-01 formula.
- Stack orientation:
  - *Public* stacks (`Exec`, typing) are bottom-first, with the last
    element on top. In `x y -`, `x` is deeper (left) and `y` is the top
    (right).
  - *Internal* `Run`/`Step` stacks are top-first, so subtraction matches
    `.i64 y :: .i64 x :: s ↦ x - y` (`exec_sub_is_internal_step`).
  - The runtime helper `$op_sub` pops `$right` first, then `$left`, and
    computes `i64.sub left right`
    (`crates/noble-wasm/runtime/operations.wat:18-21`).
- Observations:
  - the ordered final stack, including an arbitrary preserved prefix;
  - normal return versus no normal return (MC1 is partial correctness);
  - `reflect`'s exact recipe;
  - frontend literal acceptance/rejection.

## Closure criterion for issue #15 (owner decision 2026-10-26)

The end-to-end foundation is **complete** for issue #15 once every claim-table
row in this README names either a kernel-checked theorem set carried by a
receipt or an explicitly labelled `open`, `tested` or `assumed` obligation, and
no receipt claims implementation correctness.

That criterion is machine-audited rather than asserted:
[`verification/foundation/claims.mjs`](../../verification/foundation/claims.mjs)
parses every claim table, requires an allowed status word in each row, requires
a named model or theorem set unless the row is explicitly open, checks every
named theorem against [`inventory.json`](inventory.json), and refuses a receipt
that claims an implementation theorem. Names that are specification identifiers
rather than inventoried theorems are declared with their reason in the audit
itself. The retained result is
[`verification/foundation/claims.json`](../../verification/foundation/claims.json):
16 claim-table rows, 15 proved, 8 tested-bearing and 11 open, 563 inventoried
theorems, no diagnostics. The audit is a *document* check: passing it is not a
correctness claim about any implementation, and the rows labelled open stay
open.

`verification/foundation/ci.sh` runs the audit beside the proof check.

## Claim table (Phase 1)

Status words:
- **proved**: machine-checked here.
- **tested**: finite executable evidence.
- **assumed**: trusted.
- **open**: no evidence.

| Claim | Model / theorem(s) | Implementation boundary | Status |
|---|---|---|---|
| ZMod/BitVec/K-NUM-01 signed decoding, range, round trips | `toZ_ofZ`, `ofZ_toZ`, `toZ_add/sub/mul/neg`, `signed_eq_toInt`, `signed_toZ`, `wrap_eq_bmod`, `wrap_eq_toInt_ofInt`, `wrap_eq_knum01`, `toInt_range`, `signed_range`, `intCast_signed`, `toInt_ofInt_eq_iff`, `signed_intCast_eq_iff`, `ofInt_toInt` | none (mathematics only) | **proved** |
| MC1 BitVec add/sub/mul decode to the comparator oracle | `toInt_add/sub/mul`, `oracle_add/sub/mul` | none | **proved** |
| Checked I64 scalar arithmetic and explicit truncating division/remainder | `NobleFoundation.CheckedI64.checkedAdd_ok_iff`, `checkedSub_ok_iff`, `checkedMul_ok_iff`, `checkedNeg_ok_iff`, `checkedAbs_ok_iff`, `checkedAdd_overflow_iff`, `checkedSub_overflow_iff`, `checkedMul_overflow_iff`, `checkedNeg_overflow_iff`, `checkedAbs_overflow_iff`, `div_rem_identity`, `trem_natAbs_lt`, `trem_sign`, `checkedDivRem_overflow_iff`, `checkedDivRem_divisionByZero` | none (name-neutral mathematical model; no Rust/API correspondence) | **proved** |
| Literal acceptance is exact range rejection, not wrapping | `acceptLiteral` (spec); `accumulate_eq_acceptLiteral` over a *transcription* of `Accumulator::digit`; `accepted_literal_exact`; `ofInt_collapses_out_of_range`; `literal_boundary_regressions` | `crates/noble-contracts/src/syntax/integer.rs`, token classification in `source/lexer/tokens.rs` | transcription **proved**. Rust ↔ transcription **open** (the Aeneas extraction exists in `proofs/mc1/NobleContractImpl/Funs.lean`, but there is no refinement theorem). CLI rejection **tested**: `verification/m4/acceptance.json` and the Phase 4 comparator |
| Universal instruction semantics with a preserved typed prefix | `add_spec`, `sub_spec`, `mul_spec` (`ArithSpec`); `exec_arith_toInt`; `add_needs_i64` | `crates/noble-wasm/src/operations.rs:21-23` (word → helper), `runtime/operations.wat:14-25`, `runtime/stack.wat` (`$pop_kind`, `$take`, `$push_i64`) | model **proved**. Lowering/helpers **open** (PO-17), finite Wasm **tested** |
| Public/internal orientation, deeper-minus-top subtraction | `internal_of_public`, `exec_sub_is_internal_step`, `sub_operand_order`, `wrap_sub_noncommutative` | `$op_sub` pop order | model **proved**. Helper **tested** (comparator `sub-order`) |
| Finite sequences equal an independent reference machine | `exec_lowerAll`, `run_lowerAll`, `refRun_range`, `depthRun_of_refRun`, `codeTyped_lowerAll` | lowering of literal pushes (`emit/functions.rs` `call $push_i64`) | model **proved**. Implementation **tested** |
| quote/compose/run: construction is pure, run executes the composed body, reflect observes the exact recipe | `PExpr.exec_code`, `PExpr.exec_code_run`, `PExpr.exec_code_reflect`, `PExpr.exec_run_ref`, `PExpr.codeTyped(_run)`, `exec_quote_program_run_run`, `compose_order_matters`, `quote_compose_run_examples` | `$op_quote_typed`, `$op_compose`, `$op_run` (`runtime/programs.wat`), `emit/functions.rs` | model **proved**. Implementation **tested** |
| Specification-quality obligations are separate from implementation satisfaction | `realizable_of_uninhabited`, `unrealizable_of_contradictory_post`, `not_deterministic_of_two_outputs`, `deterministic_of_unique_output`, `increment_inhabited`, `increment_realizable`, `increment_deterministic`, `contradictory_inhabited`, `contradictory_not_realizable`, `underconstrainedBool_two_outputs`, `underconstrainedBool_realizable`, `underconstrainedBool_not_deterministic`, `emptyOutput_deterministic`, `emptyOutput_not_realizable`, `determinacy_does_not_imply_realizability` | none (model relations only; `NobleContracts.exportedClaim` is not related to this file) | **proved** (model). Renderer/retained-contract correspondence **open** |
| Reverse-mode AD reference over the bounded smooth fragment | `line_zero`, `evalStep_hasDerivAt`, `valuesUpTo_hasDerivAt`, `tangent_is_directional_derivative`, `repeated_operand_contributes_twice`, `adjoint_step_adds_contribution`, `adjoint_unchanged_when_operand_absent`, `adjointDot_step`, `exampleGraph_wellFormed`, `exampleGraph_value`, `exampleGraph_tangent`, `exampleGraph_derivative`, `unused_input_slot_tangent`, `non_unit_seed_unconsumed`, `exampleGraph_adjointDot`, `the_overwriting_update_pairs_to_four`, `the_sweep_pairs_to_eight_on_the_second_slot`, `slotGraph_wellFormed`, `slotGraph_slotNode_map`, `slotGraph_value`, `slotGraph_tangent_slot_zero`, `slotGraph_tangent_slot_one`, `slotGraph_derivative_slot_zero`, `slotGraph_derivative_slot_one`, `slotGraph_slotGradient_unit`, `slotGraph_slotGradients_match_unit_tangent`, `slotGraph_slotGradient_five`, `slotGraph_slotGradients_match_five_tangent`, `slotGraph_wrong_slot_control` | `crates/noble-ngraph/src/grad.rs` `slot_grads`/`slot_node` and reverse loop; binary32 operation sequence | model **proved** (JVP, per-step accumulation, slot extraction, unit and seed-5 fixture transpose checks, the wrong-slot control, and five seeded update-rule mutations caught, including the subtraction sign). Ordered binary32 gradient model, Rust refinement and the transpose identity over arbitrary graphs **open** |
| Binary32 reference: rounding, signed zeros, NaN class, boundaries and the ordered fold | `retained_expectations_checked`, `halfway_ties_round_to_even`, `signed_zero_rules`, `nan_class_and_zero_separation`, `encoding_boundaries`, `exact_sum_exceeds_the_rounded_result`, `reassociation_is_observable`, `implicit_contraction_is_observable`, `multiplication_rounds_once`, `fold_append`, `fold_order_is_load_bearing`, `all_declared_mutations_are_caught` | `crates/noble-ngraph/src/sem.rs` add/sub/mul/affine and the compiled legs, on the receipted host and target only | model **proved**. Packed and compiled replay of the 26 retained expectations **tested** (`verification/binary32/replay.json`). `sem.rs` correspondence **open** |
| Conditional normal-range rounding error for positive exact quotients | `roundPositive_normal_absolute_error`, `roundPositive_normal_relative_error`, `flush_to_zero_violates_absolute_bound` | exact `Binary32Error` model only; scale/bin/value premises are explicit checker obligations, and no Rust/host correspondence is claimed | model **proved** under the stated normal-range premises. Subnormal error rule and implementation correspondence **open** |
| Scale premises of the normal-range rounding error, derived (issue #44) | `scale_lower`, `scale_upper`, `scale_at_boundary`, `boundary_rounds_exactly`, `roundPositive_absolute_error`, `roundPositive_relative_error` | exact `Binary32Scale` model only; the carried-exponent range stays a premise, and no Rust/host correspondence is claimed | model **proved**; the upper scale premise is **refuted** on the power boundary, where the result is exact. Subnormal error rule and implementation correspondence **open** |
| Restricted SGD reference update and its admission rule (issue #58) | `update_preserves_shape`, `update_lane`, `checkStep_accepted`, `nonfinite_input_is_never_accepted`, `nonfinite_parameter_is_refused_at_its_lane`, `fused_update_is_detected` | the published `ngraph-opt/1` reference over the #38 model only; no candidate checker, Rust or training correspondence | model **proved**; candidate checker, snapshots, rollback and evaluation **open** |
| First Futamura projection for a one-hot two-instruction bytecode family (issue #34) | `interpreter_agrees`, `first_projection`, `annihilate_sound`, `residual_sound`, `unselected_branches_are_dead`, `remaining_dispatch_overhead` | a deliberately restricted source family over the #32 `PackedSchedule` specializer; no general interpreter, Rust or runtime correspondence | model **proved**; identity forwarding, general source languages and measurements **open** |
| Bounded canonical ServiceRef codec: round trip, canonicality, bounds and error classes | `decode_encode`, `encode_of_accepted`, `accepted_name_is_valid`, `longest_name_round_trips`, `the_full_wire_budget_is_not_a_byte_limit`, `overlong_name_decodes_to_schema`, `byte_limit_precedes_depth_scan`, `depth_configuration_precedes_schema`, `depth_scan_precedes_schema`, `altered_readiness_is_schema`, `empty_name_is_schema`, `foreign_name_byte_is_schema`, `schema_controls_have_accepted_neighbours`, `unbalanced_closers_do_not_count_toward_the_depth` | `crates/noble-kernel/src/dataspace/wire.rs` `decode_service`/`encode_service`; an Aeneas extraction of both exists in `proofs/m3/NobleKernel/Funs.lean` | model **proved**. 271-case differential **tested** (`verification/service-ref/differential.json`). Refinement from the extraction **open** |
| Packed I64/Bool schedule: result equivalence and independent instruction validation | `validate_sound`, `bodiesMatch_eq_emitNodes`, `operandsDefined_sound`, `registersInjectiveOn_sound`, `outputsMatch_runOutputs`, `schedule_sound`, `execSchedule_sound`, `step_computes`, `step_written`, `run_emitNodes_eq`, `foldl_eq_execSchedule`, `valuesUpTo_eq_value`, `orderOk_cons`, `exampleGraph_wellFormed`, `exampleSchedule_valid`, `exampleSchedule_validated`, `reordered_order_not_layer_sorted`, `reorderedGraph_wellFormed`, `exampleGraph_values`, `exampleGraph_schedule_sound`, `reordered_stream_validated_with_repeated_output`, `reordered_stream_sound`, `validate_refuses_operand_before_definition`, `validate_refuses_live_register_alias`, `validate_refuses_wrong_operation`, `validate_refuses_wrong_output_register`, `aliasing_loses_the_constant`, `a_missing_operand_refuses_rather_than_fabricating`, `a_missing_second_operand_refuses_rather_than_fabricating`, `an_unwritten_register_keeps_its_value`, `an_unwritten_register_stays_empty`, `an_omitted_output_node_yields_no_output` | `crates/noble-ngraph/src/compiled.rs` `Compiled::new`, the packed `I` instruction stream and its evaluator | model **proved**. Rust instruction-body export and Lean↔Rust correspondence **open** |
| Explicit exact and RNE decimal import | `decimalRatio_den_pos`, `isNegative_eq_msb`, `rne_negative_sign`, `zero_spelling`, `outcomeClass_overflow_iff`, `exactImport_some_iff`, `exactImport_agrees_with_rne`, `exactImport_exactValue`, `negative_zero_fixture`, `decimal_point_one_fixture`, `decimal_point_five_fixture`, `decimal_one_e39_overflow_fixture`, `decimal_least_subnormal_fixture`, `decimal_tie_fixture`, `decimal_just_above_tie_fixture` | decimal syntax/resource bounds, overflow response policy, and correspondence to `crates/noble-ngraph/src/sem.rs` | model **proved**. Implementation and owner-policy obligations remain **open** |
| Bounded canonical ServiceRef codec: round trip, canonicality, bounds and error classes | `decode_encode`, `encode_of_accepted`, `accepted_name_is_valid`, `longest_name_round_trips`, `the_full_wire_budget_is_not_a_byte_limit`, `overlong_name_decodes_to_schema`, `byte_limit_precedes_depth_scan`, `depth_configuration_precedes_schema`, `depth_scan_precedes_schema`, `altered_readiness_is_schema`, `empty_name_is_schema`, `foreign_name_byte_is_schema`, `schema_controls_have_accepted_neighbours`, `unbalanced_closers_do_not_count_toward_the_depth` | `crates/noble-kernel/src/dataspace/wire.rs` `decode_service`/`encode_service`; an Aeneas extraction of both exists in `proofs/m3/NobleKernel/Funs.lean` | model **proved**. 271-case differential **tested** (`verification…
| Packed I64/Bool schedule: result equivalence and missing-register behavior | `schedule_sound`, `execSchedule_sound`, `step_computes`, `step_written`, `run_emitNodes_eq`, `foldl_eq_execSchedule`, `valuesUpTo_eq_value`, `orderOk_cons`, `exampleGraph_wellFormed`, `exampleSchedule_valid`, `exampleGraph_values`, `exampleGraph_schedule_sound`, `aliasing_loses_the_constant`, `a_missing_operand_refuses_rather_than_fabricating`, `a_missing_second_operand_refuses_rather_than_fabricating`, `an_unwritten_register_keeps_its_value`, `an_unwritten_register_stays_empty`, `an_omitted_output_node_yields_no_output` | `crates/noble-ngraph/src/compiled.rs` `Compiled::new`, the packed `I` instruction stream and its evaluator | model **proved** (schedule soundness and the handwritten `Option Val` machine's empty-register behavior). Actual emitted-artifact binding, the Rust transcription, graph guards/`fail_suffix_min`, `F32` and end-to-end checker execution **open** |
| Pure I64 graph rewrite certificates: rule, congruence, lifting, checker and replay endpoint | `fold_add_constants`, `add_wraps_at_the_boundary`, `neutral_add_left`, `neutral_mul_right`, `evalStep_congr`, `congruence`, `replacement_lift`, `checked_sound_const_add`, `checked_sound_zero_left`, `checked_sound_one_right`, `constant_value`, `foldGraph_wellFormed`, `foldGraph_reference`, `fold_certificate_accepted`, `wrong_constant_refused`, `swapped_operands_refused`, `wrong_rule_identity_refused`, `wrong_bits_refused`, `accepted_endpoint_equivalence`, `multiFold_endpoint`, `substituted_result_differs` | `crates/noble-ngraph/src/ir.rs` graph producer and any future untrusted transformation | model **proved** (rules, lifting, checker soundness and the Lean replay endpoint). Correspondence to `cert.rs`, Rust transformation, derivation encoding/replay budget, endpoint decoding of actual graphs and independent differential execution **open** |
| Bounded graph-pair query model: witness decoding, exhaustive search and refutation | `lookup_returns_recorded`, `decode_uses_fallback`, `decode_replays_recorded`, `not_refuted_iff_agrees`, `search_none_sound`, `search_some_refutes`, `same_sum_search_none`, `same_sum_agrees`, `subtraction_refuted`, `witness_bits_decide`, `unrecorded_slot_falls_back`, `corrupted_witness_changes_the_verdict` | `crates/noble-ngraph/src/extract/symbolic.rs` encoder, `crates/noble-ngraph-experiment/src/b2i64` replay legs and any emitted Wasm | model **proved** (decoding only returns recorded pairs; bounded-universe search soundness). Encoder correspondence, solver, compiled-endpoint replay and witness shrinking **open** |
| Bounded M6 task interleavings: prefix invariants, a proved reduction and full/reduced agreement, for refusal-ending and valid client histories | `step_preserves_invariants`, `reachable_prefixes_satisfy_the_invariants`, `independent_actions_commute`, `every_sequence_has_a_reduced_representative`, `reduced_search_agrees`, `the_declared_reduction_agrees`, `the_declared_observation_counts`, `different_ids_rule_is_unsound`, `the_unsound_reduction_is_detected_only_where_ids_interleave`, `every_disagreement_has_a_shortest_witness`, `independent_is_client_sound`, `client_canonicalize`, `reduced_client_search_agrees`, `the_client_projection_discriminates`, `the_lossy_client_projection_hides_rejected_calls` | `crates/noble-kernel/src/async_tasks/table` (admission, callbacks, ownership) | model **proved** (two-record model; counts and coverage at the declared bound). Rule-level negative controls against the Rust table **tested** (`crates/noble-kernel/tests/m6-interleaving.rs`). Model ↔ Rust table correspondence and witness replay through the table API **open** |
| Checked interaction-net contract: versioned typed agent kinds, arity-checked ports, reciprocal unique-incidence wiring, compatible ordered operand links and ordered output boundary | `version_ok_sound`, `reciprocity_ok_sound`, `compatibility_ok_sound`, `boundary_ok_sound`, `validate_sound`, `deeper_minus_top_is_accepted`, `swapped_sub_operands_are_rejected`, `dangling_wire_is_rejected`, `aliased_incidence_is_rejected`, `two_output_boundary_is_accepted`, `wrong_output_order_is_rejected` | no implementation correspondence; net reduction/readback and graph-to-net translation remain open | **proved** (model) |
| Bounded ISOP Boolean core with an independent checker | `valuation_mem`, `equivOn_sound`, `isop_exact`, `isop_bounds`, `constant_true_checked`, `constant_false_checked`, `contradiction_checked`, `repeated_atom_checked`, `parity3_checked`, `contradiction_irredundant_fixture`, `repeated_atom_irredundant_fixture`, `parity3_irredundant_fixture`, `parity3_cover_growth`, `parity3_orders_differ`, `parity3_orders_equivalent`, `wrong_cofactor_rejected`, `wrong_complement_rejected` | `proofs/foundation/NobleFoundation/Isop.lean` only; no source integration is claimed | model **proved** (exact bounded cofactors, support interval, independent exhaustive checker and fixture controls). Checked Boolean DAG representation, lowering to `Program`, branching extensions, care sets and adoption criteria **open** |
| Constructor reuse under a unique root, matching size, and resource eligibility | `makeCell_requested`, `reuseStep_success`, `reuseStep_refuses_size_mismatch`, `reuseStep_replaces_unique_root`, `reuseStep_initializes_requested_cell`, `eligible_replacement_has_no_resource`, `eligible_replacement_requires_move`, `wrapped_resource_is_not_reuse_eligible`, `shared_alias_control` | none (finite model only; no allocator, runtime or Wasm correspondence) | **proved** |
| MC1 `CodeTyped` agrees with kernel acceptance | — | `noble-kernel` acceptance (PO-09/10/11 relate the Rust checker to the *NobleM2* declarative judgments, not to MC1 `CodeTyped`) | **open** |
| Collision-safe cache lookup and length-prefixed subject encoding | `NobleFoundation.CacheLookup.lookup_some_implies_manifest_eq`, `colliding_index_equal`, `colliding_index_wrong_manifest_misses`, `stale_manifest_entry_refused`, `corrupt_entry_refused`, `lookup_after_erase_subset`, `lengthBytes_injective_of_lt`, `encodeParts_injective_of_lt`, `changed_field_changes_encoding`, `naive_concat_field_collision` | model only; `Subject::derive` byte correspondence and all cache integration boundaries remain open | **proved** (model) |

Typing premises are stated exactly:
- Sequences require `depthRun ops k = some k'` (static depth) for typing,
  and `refRun ops zs = some zs'` for behavior. `refRun` additionally
  refuses out-of-range pushes.
- Programs constructed by `PExpr` are typed from `base ++ k×I64` to
  `base ++ k'×I64`, and `run` requires that input interface to be the
  whole current stack (MC1 `OpTyped.run`).
- No eligibility/resource premise is needed, because the fragment has no
  resources.

The full list with printed statements and axioms is in
[`inventory.json`](inventory.json) and in every `foundation-check.json`
receipt.

## Phase 3: one checked chain to the loaded bytes

Subject: `[ quote [ 1 + ] compose run ]`. It is one closed, inert quotation
root with no definitions or resources, which is the
`MC2-pure-quotation@1` shape that #10 selected for its PAA-1 admission
envelope (`.cairn/changes/artifact-correspondence-extension/design.md` on
`design/contracts-8-9-10`).

PAA-1 is a design and is **not implemented**. Instead of a second
admission route, `verification/foundation/slice.mjs` runs the two
existing routes that PAA-1 joins, over the same consumer-selected source
bytes:
- Route B: `noble build --require-proof`;
- Route A: `noble acquire-artifact`.

This is a strengthened trusted build. It is **not** translation
validation.

| Edge | What happens | Status |
|---|---|---|
| E1 Lean specification | `slice_body_exec` (any prefix, `n ↦ n + 1`), `slice_body_signed`/`slice_driver` (oracle `wrap (x + 1)`), `slice_root_inert`, `slice_body_typed`, `slice_exported_claim`, `slice_examples` | **proved** (model) |
| E2 frozen law → regenerated MC1 statement | `noble explain-proof` output must equal [`slice/expected-statement.lean`](../../verification/foundation/slice/expected-statement.lean) byte for byte. This is the PAA-1 `owner_law` analogue | **checked**. The exporter itself is assumed (PO-16/PO-19) |
| E3 regenerated statement ↔ foundation | Lean kernel: `MC1Obligation.program = NobleFoundation.sliceBody := rfl`, and the foundation proves the regenerated `MC1Obligation.claim` | **proved** per artifact |
| E4 source ↔ subject with evidence | Route B: an independent sandboxed recheck of the self-contained MC1 proof [`compose-increment.proof.lean`](../../verification/foundation/slice/compose-increment.proof.lean); `bind_artifact` binds the source | **checked**. `bind_artifact` is trusted Rust (PAA-1 O6 / PO-16 open) |
| E5 `noble-wasm` lowering | source → WAT | **open** (PO-17, PAA-1 O1). Trusted compiler |
| E6 runtime helpers | [`Helper.lean`](NobleFoundation/Helper.lean) transcribes `$op_add/$op_sub/$op_mul`, `$pop_kind`, `$take`, `$push_tag`, `$need`, `$fail`. `helper_refines_step`/`helper_fails_when_stuck` prove they refine MC1 `Step` (operand order, untouched remainder, refusal of non-I64). The text of each transcribed function is pinned in [`helper-transcription.json`](../../verification/foundation/slice/helper-transcription.json) | **proved over a transcription**. Pin **checked**. Transcription fidelity is **assumed** (manual review). `$op_quote_typed`, `$op_compose` and `$op_run` are **open** |
| E7 WAT → Wasm (`wasm-tools parse`) | pinned path and hash | **assumed**. Validation shows format/type only |
| E8 `wasm-opt -O2` (opt on) | separate receipts per mode | **assumed** (PAA-1 O2 open) |
| E9 candidate = consumer rebuild | `acquire-artifact` reports `correspondence: byte-exact` for opt off and opt on | **checked** (identity, not semantics) |
| E10 instantiation/loader/V8 | Route A instantiates the compared buffer (code reading) | **assumed** (PO-18, PAA-1 O3 open) |
| E11 execution | drivers `x [ … ] run` for `x ∈ {41, max, min}` and a `Bool`/`I64` preserved prefix, opt off/on, through admission. Expectations are kernel-checked against `MathlibI64.wrap` | **tested** (finite) |
| E12 one PAA-1 envelope joining A and B | — | **open** (#10) |

Refusal controls. Each one must fail for its intended reason, or the run
fails.

| Control | #10 id | Result required |
|---|---|---|
| valid `[1 +]` proof, source `[ quote [ 2 + ] compose run ]` | ACORR-CASE-03 | Route B not released |
| `[2 +]` bytes acquired against the `[1 +]` source | ACORR-CASE-03 | `correspondence-reject` |
| opt-on bytes acquired under consumer opt off | S-CASE-07 option mismatch | `correspondence-reject` |
| `$op_sub` with swapped pops, reassembled for `3 8 -` | ACORR-CASE-08 (changed helper, operand order) | `correspondence-reject`; the Lean transcription pin also refuses (`helper-transcription-stale`) |
| interface/recipe data string `[I64]` → `[I65]`, code unchanged | ACORR-CASE-06 | `correspondence-reject` |
| foundation receipt from another tree | ACORR-CASE-04 (stale version) | `stale-proof-subject` |
| weakened regenerated statement | ACORR-CASE-05 | `law-mismatch` |
| post-check artifact replacement | ACORR-CASE-09 | **not executed**. There is no hook between comparison and instantiation, and the PAA-1 `load` hash is not implemented |

Stale dependencies do not occur in this profile, which has no definitions.
Recorded deviation: the reviewed sandbox `bubblewrap-0.11.2` store path
from `verification/mc2/gate.mjs` was absent on the checking host. The
runner then uses `bubblewrap-0.12.0` and records that path in the receipt.

## Phase 4: differential evidence

`verification/foundation/differential.mjs` works as follows:
- It generates seeded (splitmix64) cases:
  - straight-line arithmetic;
  - `quote`/block/`compose` trees, nested up to depth 2, then `run`;
  - nested quotation `[ … ] quote run run`;
  - preserved `Bool`/`I64` prefixes;
  - three out-of-range-literal refusal cases.
- Constants are boundary-biased: half are drawn from
  {0, ±1, ±2, ±3, max, min, max-1, min+1, ±2^31, 2^32, 2^32-1, ±2^62,
  ±⌈√2^63⌉}, and half are uniform.
- Expected values come from a BigInt transcription of `refRun`. **Lean
  kernel-checks every expectation**: `refRun … = some …` (or `= none`) by
  `decide`, and instantiates the proved MC1 theorem for that case
  (`PExpr.exec_inputs_run`, `exec_lowerAll_from_empty`,
  `exec_inputs_nested`). The implementation under test never supplies an
  expected value.
- Each case runs with `--opt off` and `--opt on`. The run checks:
  - the optimization flag;
  - the executed-bytes hash against the emitted file;
  - the source hash;
  - the ordered stack, including the untouched prefix;
  - an empty request trace;
  - zero guest requests.
- A mismatch is shrunk greedily. At most 48 shrink candidates are
  executed, and every one must stay valid under the reference.
- Mutation controls each have to be detected, and every detection has to
  be attributable to its mutation:
  - reversed subtraction;
  - unsigned decoding;
  - prefix corruption;
  - wrong composition order;
  - phantom host-trace expectation;
  - literal wrapping instead of refusal;
  - stale proof subject (Lean case rebound to another program, foundation
    receipt for another tree).

  The receipt keeps the shrunk witness of each control.
- It runs the existing kernel lanes
  (`cargo test -p noble-kernel --test property`: 1,000 oracle candidates
  and 200 malformed). They are recorded as **checker acceptance**
  coverage, which is distinct from execution semantics.
- The receipt records:
  - source commit/tree;
  - the hashes of the comparator, the Lean sources, the oracle, MC1, the
    runtime helpers, the lowering table, the frontend literal code, the
    config, the policy, `Cargo.lock` and the property test;
  - the CLI and tool hashes;
  - the Lean version and Mathlib revision;
  - the theorem names with their printed statements and axioms (from the
    foundation receipt, which must be for the same tree);
  - the seed, count and modes;
  - claims labelled **proved**, **tested**, **assumed** or **open**.
- `--verify RECEIPT` refuses source drift and stale evidence.

Unsupported modes: none. Both `off` and `on` are supported by
`Core-Bootstrap`.

### Input-specific branch EMI (issue #53)

`differential.mjs --emi` reuses the arithmetic reference, renderers, CLI
builder and scalar observation comparison. Its independent branch model
covers one scalar `if`, or one `inl`/`inr` I64 payload followed by `case`.
Both bodies contain only I64 literals and `+`, `-`, `*`. The selector,
scalar input and Bool/I64 bottom prefix are fixed literals in the complete
source program. Both branches have the same ordered stack interface and
the entire source is independently acquired through the real frontend and
kernel before execution. A valid but unexecuted word outside this model
is still ineligible.

The deterministic corpus has 16–64 positive pairs and three tagged refusal
probes: an out-of-range literal, a wrong branch type, and an acquired word
outside the model whitelist. Only one declared site in the untaken branch
changes: a constant, arithmetic operator, neutral insertion, or neutral
removal. Refused pairs are eligibility outcomes, not compiler disagreements.
Counts distinguish generated and eligible branch/transformation coverage,
refusal classes, opt modes, and checked subjects.

For every eligible pair, fresh Lean declarations bind both complete bodies,
the fixed input, prefix and selector. Kernel-checked `decide` expectations
and `exec_branchRef` establish the ordered scalar result and preserved
prefix in MC1. Structural selected-body equality and the checked visit
vector establish nonexecution of the changed branch in this fragment.
The proof record binds both exact source hashes and the model-pair/input
hashes; changed bytes cannot reuse the original pair's evidence.

Original and variant execute with opt off and on against that independent
expectation, not merely against each other. Every run requires normal
return, the exact ordered scalar stack, unchanged bottom prefix, an empty
request trace, and zero guest requests/protected operations. Reflection,
recipe/identity observations, returned Programs, effects/resources, live
slots and quota-sensitive equivalence claims are excluded.

The selected ABI maxima are supplied explicitly. Admission, assembly,
optimization and execution artifacts are retained, with exact source,
WAT/Wasm, report and tool hashes. Allocation/operand/continuation peaks,
execution steps, structural recipe/depth bounds, and the separate bounded
stack-inspection reflection counter must each leave more than half their
selected limit free. Scalar stack inspection is not a guest `reflect`.
Exhaustion and insufficient headroom are separate discrepancy classes;
neither counts as an equivalent run. Preparation cost and code size are
not claimed equal.

Seven controls must be detected: selected-branch mutation, changed output
order, prefix corruption, an actual host request with a forged empty trace,
stale exact-subject evidence, a stale foundation tree, and an intentionally
reversed selected-branch lowering. The lowering control changes actual
WAT and executes the resulting Wasm in both opt modes. It is not a forged
expected value.

The shrinker changes the program/variant/fixed-input triple, including the
mutation site. Every retained candidate is re-acquired, freshly checked by
Lean and re-executed while preserving eligibility and the discrepancy
class. Its 64-attempt bound, outcomes and local-minimal/bound-exhausted
status are recorded. Replay the retained lowering-control discrepancy with:

```sh
env -i "$NODE" verification/foundation/differential.mjs --emi \
  --replay "$OUT/emi/minimal-control.json" \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/emi-replay"
```

`--emi --verify` requires the exact clean source tree, unchanged bound
source/model files and every indexed artifact hash. Raw command captures
retain argv, exit, separate stdout/stderr and supplied stdin. Imported
Mathlib `.olean` files retain the cache trust assumption above. The Rust
renderer/lowering refinement remains open: these are finite fixed-input
EMI results, not a compiler theorem or universal program equivalence.

### CI

The CI recipe is [`verification/foundation/ci.sh`](../../verification/foundation/ci.sh)
`NEW_DIR [--skip-slice]`, run on a clean checkout after `lake exe cache get`.
It runs these stages:
1. `check.mjs`;
2. `slice.mjs`;
3. `differential.mjs --seed 15 --count 80`;
4. `--verify` on the fresh receipt;
5. `differential.mjs --emi --seed 53 --count 16`;
6. `--emi --verify` on its fresh receipt;
7. replay of the EMI control's retained minimal discrepancy;
8. verification of the replay receipt;
9. the one-call named linking pilot in both optimization modes;
10. verification of its exact subject/dependency/artifact receipt;
11. a stale-evidence control: the retained `receipts/898cfe1` receipt must be
   refused.

It writes `summary.txt` and exits nonzero if any stage fails.

The recipe is **not** a `flake.nix` `checks` entry, for these reasons:
- **Mathlib needs network or host-bound inputs.** The `.olean` files come
  from `lake exe cache get`, which is networked. In this checkout they
  occupy 2.1 GiB (8,176 files) under `.lake/packages/mathlib/.lake`.
  - The repository's offline route, `nix/offline-inputs.nix` with
    `nix/lean-backend.nix`, names host-captured `nix store add-path`
    sources. On the checking host those store paths are absent, for example
    `/nix/store/mfpq85q33s4hyqy7c5x4prinz45d0341-mathlib`, so a check built
    on them cannot be realised or exercised here.
  - That route also contains sources only. A sandbox check would have to
    compile the imported part of Mathlib from source. The cost has not been
    measured, and that build would replace the current trust in downloaded
    `.olean` files with a rebuild.
- **The slice needs host services.** Its Route B consumer requires a user
  systemd manager, `XDG_RUNTIME_DIR` and bubblewrap, which are not
  available inside a Nix build sandbox.
- **There is no workflow file to extend.** The repository has no
  `.github/workflows`; its CI surface is the flake `checks` set.
  `flake.nix` and `nix/*` are policy-pinned files, so changing them
  requires a re-pin.

A sandbox check becomes feasible once reviewed offline Lean sources and
either a rebuilt Mathlib `.olean` set or an authenticated cache are pinned
as store inputs. The `check` and `differential` stages need only Lean,
Cargo (offline) and Node, so they could move into the sandbox first.

## Conditional named-call linking (issue #52)

[`NobleFoundation/LinkBoundary.lean`](NobleFoundation/LinkBoundary.lean) imports
the existing pinned `NobleContracts.NamedV1` model. Its two-result callee is
`[ - 7 ]`; the one-call caller is `c.split -`. It proves source-model normal
results, models the private operand/continuation ABI, and states conditional
horizontal linking with exact source/dependency/table/artifact bindings.
`vertical_simulation_compose` is a separate local-simulation composition rule.
All new theorem statements and transitive axioms enter the existing inventory
and checker; the checker also hashes `NamedSubject.lean`.

The [executable pilot](../../verification/linking/README.md) maps each boundary
to the actual embedded-callee path, retains exact-byte receipts and exercises
body/order/return-target/dependency/substitution controls. It explicitly models
engine-owned shared memory/table/globals, not a fictional separate module ABI.
The runtime relation projects Bool/I64 operands, continuations, table and
`sp`/`cp`; it does not claim the engine's other accounting globals are unchanged.

The source and helper-transcription theorems are model results. The real
lowering and loading simulations needed to instantiate the conditional linker
remain premises: C8 and O1/O3/O7 are not discharged, and #7/#10 remain completed
designs. No existing conformance or proof-ledger status changes.

## Bounded host-call locality (issue #51)

[`HostLocality.lean`](../m5/HostLocality.lean) models one synchronous
bounded-region adapter, an ordered guest stack with one saved borrowed owner,
exact M5 resource identity and a separate unrelated-owner/state projection.
The adapter may read only its immutable region and request and update only its
own owner, table snapshot and counters. Actual host locality remains an explicit
premise: the native host has process-memory authority, not a Wasm sandbox.

The 43 inventoried model theorems cover normal success, preflight/domain errors,
validation before ordered borrow restoration, consuming release without owner
return, malformed results, trap, cleanup failure and double retirement. The
decision relation reuses actual `M5Resources` completion, table, release and
late-completion lemmas. It does not replace the extracted table with a handwritten
lookalike. Request naturals describe the post-signed-ABI-validation model;
explicit counter-room premises exclude `u64` overflow.

`verification/host-locality/check.mjs` recovers the exact generated Lean sources
from the retained M5 archive and compiles them anew with the pinned Lean. It
cross-checks the archive against its historical extraction receipt and requires
all six current Rust resource-decision files to retain their recorded hashes.
The separate theorem inventory records exact compiled statements and transitive
axioms; only `propext`, `Classical.choice` and `Quot.sound` are allowed. The
historical generated-code naming warnings are retained, not treated as proof
holes. Imported Mathlib/Aeneas libraries remain a separate trust assumption.

The execution runner builds the current CLI with the hash-checked offline
vendor, reusing the worktree's ignored Cargo target. It compiles the selected WIT
guest through the real CLI, then exercises 10 Wasmtime cases: read/empty-end
success, ordinary bounds refusals and emitted-core trap/tag/length faults.
Five direct production-Host tests cover forged identity, cross-invocation use,
owner retention, double release and cleanup refusal/recovery. The malformed
result controls corrupt guest return-area memory after a valid typed host
response; they are not adversarial malformed host `Val` returns.

From a clean committed checkout with the existing foundation dependency cache:

```sh
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
OUT="$HOME/.cache/noble-host-locality-$(date +%s)"
env -i "$NODE" verification/host-locality/run.mjs --output "$OUT"
env -i "$NODE" verification/host-locality/run.mjs --verify "$OUT/locality.json"
```

Select `tool_paths.node.output` from `policy/tool-selection.json` elsewhere.
`--allow-dirty` is available for development smoke runs, never clean-source
acceptance. Separate proof/execution receipts retain raw argv/status/stdout/
stderr, source/tool/WIT/component hashes, assumptions and limited controls.
`--verify` checks source and retained-artifact identity only; it does not replay
Lean or prove Rust/Wasmtime refinement. The CI recipe runs this separate lane.

T5 Rust-wrapper refinement and T6 lowering refinement remain open. Borrow
restoration itself changes compiler `State.stack`, not a separate runtime event;
early-restoration and unrelated-frame mutation controls are model-only. The
one-owner adapter is unchanged. Its v1 report cannot distinguish guest release
from cleanup, and public CLI cleanup failure remains unexercised; the direct
Host refusal test does not close that edge. No arbitrary-host, async, reentrant,
shared-memory or whole-engine isolation claim, conformance promotion, or
proof-ledger change follows from this pilot. The completed #9 design stays closed.

The [retained pilot evidence](../../verification/host-locality/evidence-71e4504.json)
binds clean source `71e4504bb82fdab174a2650280a21e69f3979611`: 43 locality
theorems, five reused M5 lemmas, 10 Wasmtime cases and the direct Host controls.
Its identity check also rejects altered WIT, raw proof output, proof-source
binding and source revision. Replay or verify from that exact clean revision;
the subsequent evidence/documentation commit is not a new execution or proof run.

The pinned gate sweep is **12 passed, 2 failed**, not global acceptance.
`documents` retains 17 inherited local-link diagnostics (15 archived-file
references and two directory links); `boundary-controls` fails the
`acyclic-shell` artifact replay without retaining the early gate's stderr.
Both failures and the earlier remote-builder failures remain in the evidence;
no document, fixture, capacity, policy or acceptance rule was weakened.

## Phase 2: law/obligation matrix (pure MC1 model)

Observation relations are kept separate:
- `BehEq` is normal-return behavioral equality.
- Recipe equality (`List Op` equality) is what `reflect` observes.
- Definition/proof-subject identity has no MC1 counterpart. On the
  executable path it is covered by Route B subject binding and the
  ACORR-CASE-03 controls.

These pure laws are the base cases that #11's resource-aware wiring laws
and #7's law bundles should specialize. Nothing here carries a resource
premise.

| Obligation | Checked here (theorems) | Premises | Remains open |
|---|---|---|---|
| PO-02 preservation | MC1: `exec_preserves`, `Run.preserve`, `Step.preserve` (all 22 pure words, including `dip`/`run`/`case`/`if`/`list.case`, nested executions and list/sum/pair values). Effects layer: `Effects.preserve`, `Effects.preserve_reach` (every small step, including halting host steps) | MC1: `CodeTyped code ts ts'`, `StackTyped s ts`, normal-return `Exec`. Effects: `WT E c` | the whole language, implementation |
| PO-03 progress | MC1: `progress_primitive`, `host_word_outside_model`. Effects layer: `Effects.progress` (a typed running configuration with code steps for **every** host response, `run` included, so there are no forbidden stuck states) and `Effects.halted_no_step` | Effects: `WT E c`, `status = running`, `code ≠ []`, total host oracle `H` | termination (not claimed), quota/exhaustion classes, MC1 big-step control words, the whole language |
| PO-04 effect soundness | Effects layer: `Effects.effect_sound` (every finite prefix: `trace' = trace ++ new`, append-only, and every new label is in the bound `E`, **including denied/failed requests**), `step_trace`, `trace_within_bound`. Negative: `bound_does_not_fix_order` (the same bound allows different orders, so a bound does not justify reordering), `denied_request_recorded_and_halts` | `WT E c` (or a typed program from `init`); `Reach H` for any host oracle | effects beyond emit/acquire/release, real host contracts and `test.emit` implementation, ordered-trace *equivalence* laws |
| PO-05 ownership | Effects layer: `Effects.own_preserve` (stack resources are a permutation of the host live set, no duplicates, fresh ids), `live_change` (the live set changes only by a recorded, permitted `acquire` or `release`; moves are stack permutations), `reachable_sound`, `move_then_release` | `WT E c`, `Own c` | temporaries/heap, borrowing, abnormal-stop cleanup (state is preserved but not cleaned), whole language (#3) |
| PO-06 recursive eligibility | MC1: `program_duplicable`. Effects layer: `ValT.rfree`/`OpT.rfree`/`CodeT.rfree` (typed programs contain no resource at any nesting depth), `typed_rfree` (continuation and every stack program are resource-free), `program_expecting_resource_is_copyable` | `WT E c` | aggregates (pairs/lists) and named captures (MC1 aggregates carry no resources; this layer has no aggregates), #3/#11 vocabulary |
| PO-07 construction/execution | `construction_is_pure`, `latent_effect_not_performed`, `PExpr.exec_code`, `PExpr.exec_code_run`, `BehEq.append`, `BehEq.run`, `compose_assoc`, `compose_identity` | normal returns; `BehEq` observation | effectful laws (same host responses, ordered traces), allocation/quota (deliberately not promised) |
| PO-08 recipe/quotation/reflection | `quote_laws`, `reflect_compose`, `PExpr.exec_code_reflect`, `exec_quote_program_run_run` | — | recipe *normalization* (MC1 does not normalize), named-definition reflection |
| negative: behavior ≠ recipe | `behavior_is_not_recipe` (`1 1 +` vs `2`; `BehEq` is not a congruence under `reflect`) | — | — |
| negative: incompatible ordered interfaces | `compose_requires_exact_middle` | `m ≠ m'` | — |
| negative: wrong-subject evidence | `wrong_subject_evidence` (+ ACORR-CASE-03 executed in Phase 3) | — | — |
| negative: resource duplication/discard/capture | `dup_res_untypable`, `drop_res_untypable`, `quote_res_untypable`, `push_res_untypable`. The untyped runs `untyped_dup_breaks_ownership`, `untyped_drop_breaks_ownership` and `untyped_capture_breaks_ownership` reach states that violate `Own`/`RFreeV`, so the typing premise is load-bearing | — | resource-aware wiring laws (#11) |

`specs/verification/obligations.json` is **unchanged**. Every proved item
above is a pure-MC1-model slice. None discharges a registry obligation as
stated, because those cover the whole language or the implementation.

## Binary32 reference model (issue #38)

`NobleFoundation/Binary32.lean` is a small, explicit, kernel-reducible model of
IEEE 754 binary32 for the Numeric-Graph-v1 slice: raw 32-bit encodings,
classification, an exact scaled-integer interpretation of every finite
encoding, round-to-nearest/ties-to-even with gradual underflow, and
add/subtract/multiply. `NobleFoundation/Binary32Expectations.lean` states the
proved theorems and kernel-checks 26 retained expectations.

The model is *selected*, not proposed: it is Lean 4.31.0 at the Noble pin with
no new dependency, no axiom beyond the foundation's allow-list, and no
`native_decide`, `decide +native` or native `#eval` anywhere in the lane.

| Item | Selected |
|---|---|---|
| Model | explicit `NobleFoundation.Binary32` definitions, no opaque Lean `Float32` |
| Rounding | round-to-nearest, ties-to-even, single rounding per operation |
| Subnormals | retained, gradual underflow at `2^-149` |
| Overflow | rounded magnitude at or above `2^128` becomes an infinity with the operand sign |
| Signed zeros | distinct observations; the xor sign rule for multiplication, and the addition rules of IEEE 754 |
| NaNs | every NaN encoding is one observation class (`obsEq`); arithmetic produces the canonical `0x7FC00000` |
| Observations | `obsEq` equality: NaNs collapse, every other class keeps its exact bytes |
| Correspondence | **none claimed** to the Rust graph evaluator, `wasm-opt`, V8 or any host FPU |

`verification/binary32/propose.py` proposes expected bits from Python integers
and its own rounding search. A proposal is not evidence: `retained_expectations_checked`
is a single theorem the Lean kernel checks over all 26 retained expectations.

Proved here:

| Theorem | Claim |
|---|---|
| `retained_expectations_checked` | every retained expectation is the model's answer for its exact operand encodings |
| `halfway_ties_round_to_even` | halfway ties round to even in both directions around `1.0` |
| `signed_zero_rules` | `(-0)+(-0) = -0`, `(+0)+(-0) = +0`, `(-0)-(+0) = -0`, `x-x = +0` for nonzero `x` |
| `nan_class_and_zero_separation` | all NaNs are one observation class; signed zeros and distinct finite encodings stay separate |
| `encoding_boundaries` | exact scaled interpretation of the smallest subnormal, smallest normal and largest finite encoding |
| `exact_sum_exceeds_the_rounded_result` | the exact sum `1.0 + 2^-24` is not representable yet rounds to exactly `1.0`, so round-to-real reasoning cannot discharge binary32 |
| `reassociation_is_observable` | binary32 addition is not associative, so a fold order is observable |
| `implicit_contraction_is_observable` | rounding the product before the sum differs from one rounding of the exact three-operand sum |
| `multiplication_rounds_once` | multiplication takes the sign exclusive-or and rounds the exact operand product once |

`NobleFoundation/Binary32Error.lean` adds the exact rational interpretation
`Error.exactValue` for finite encodings. Its integer quotient lemmas are
universal: `quotient_round_error` and `roundNearest_error` prove the
ties-to-even half-denominator rule, while `scaled_exact_ratio` proves that the
model's scale is exactly undone by the rational power. Under explicit premises
that the positive scaled quotient occupies one normal significand bin and the
carried exponent is in `[-126, 127]`,
`roundPositive_normal_absolute_error` proves a half-ulp absolute bound and
`roundPositive_normal_relative_error` derives the relative `2^-24` bound from
the bin's lower endpoint. `exactValue_of_scaledMagnitude` and
`exactValue_normalEncoding` prove the exact-value decoding of every bounded
normal output, so the error theorems no longer assume that rewrite. The scale
and bin inequalities are explicit premises of these two theorems;
`Binary32Scale.lean`, below, derives them. These are model theorems with explicit
arithmetic premises, not implementation correspondence claims. The
kernel-checked `flush_to_zero_violates_absolute_bound` control records the
smallest-normal times one-half witness: the exact product is `2^-127`, while
flushing it to zero exceeds `2^-150`.

`normal_error_applies_to_one_third` is a kernel-checked non-vacuity witness:
the remaining normal-bin and finite-carry premises hold for exact input
`1/3`, and the absolute bound is concluded for that encoded `roundPositive`
call.

`NobleFoundation/Binary32Scale.lean` derives those scale premises (issue #44). It first proves
that the model's fuel-bounded `log2` is the core floor logarithm (`log2_eq`), which brackets
`floorLog2Ratio`: `2^e · b ≤ a` for every positive input (`floorLog2Ratio_lower`), and
`a < 2^(e+1) · b` off one family of inputs (`floorLog2Ratio_upper`). `scale_lower` then
discharges the lower premise for every positive input and `scale_upper` the upper premise off
that family.

The family is the power boundary: ratios below one that are exact powers of two (`1/2`, `1/4`,
…). There `floorLog2Ratio` is one below the true floor logarithm, the scaled quotient is exactly
`2^24` (`scale_at_boundary`), and the upper premise is false; `upper_premise_fails_for_one_half`
checks this at `1/2`. So the obligation as first stated, deriving both premises for every
positive input, cannot be met. The model is still correct there: the rounded significand is
`2^24`, the carry moves it to `2^23` in the next exponent, and `boundary_rounds_exactly` proves
the encoded result is the exact ratio.

With that case split, `roundPositive_absolute_error` and `roundPositive_relative_error` state the
half-ulp and `2^-24` bounds for every positive input whose carried exponent is in `[-126, 127]`.
That exponent range is the only premise left; `absolute_error_covers_one_half` applies the
absolute rule on the boundary. Subnormal results and any implementation correspondence stay open.

The clean-source receipt for the scale-premise unit is
[`verification/foundation/receipts/3b1841f35/`](../../verification/foundation/receipts/3b1841f35/foundation-check.json)
(792 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

### The ordered `affine` fold, and the layer classification

`fold` in the same model is the documented `sem.rs` recursion
`v₀ = bias; vᵢ = rn(vᵢ₋₁ + rn(cᵢ × xᵢ))` over the reference operations, so the
fold's order is a statement rather than a comment: `fold_empty`, `fold_append`
(splitting the terms splits the fold) and, as fixtures, the two signed-zero
biases staying distinct in sign (`fold_negative_zero_bias_stays_negative`,
`fold_positive_zero_bias_stays_positive`). `fold_order_is_load_bearing` is the
consequence a reader needs: at `bias = 1.0` the terms `1.0 · 2^24` then
`-1.0 · 2^24` fold to `+0` because the `1.0` is rounded away before the
cancellation, while re-associating the same three values gives `1.0` — so
reassociating the fold, including a parallel reduction that groups its terms
differently, changes a documented result. Both products in that example are exact,
so it says nothing about FMA contraction; contraction is shown separately on a
three-operand case (`implicit_contraction_is_observable`, caught by
`Mutations.implicit_contraction_is_caught`).

The remaining `sem.rs` layers are classified rather than left ambiguous:

| Layer | Status |
|---|---|
| per-operation rounding, add/subtract/multiply | **modelled and proved** (above; 26 retained expectations, mutation lane, packed and compiled replay) |
| ordered `affine` fold | **modelled and proved** (`fold`, `fold_append`, `fold_order_is_load_bearing`) |
| activations `relu`, `reglu`, `step` | **not modelled**: they are activation corners and surrogate policies, which the Numeric-Graph-v1 selection excludes; no theorem here says anything about them |
| guard/index rules (`is_bit`, `select`, `index_position`) | **not modelled**: index refusal is a `crates/noble-ngraph` admission rule, not a binary32 operation, and no Lean model exists |
| decimal import / codec (`exact_f32_decimal`) | `DecimalImport.rne`, `DecimalImport.exactImport`, `DecimalImport.exactImport_some_iff`, `DecimalImport.exactImport_agrees_with_rne`, `DecimalImport.exactImport_exactValue`, and the retained decimal fixtures | `crates/noble-ngraph/src/sem.rs` exact-import implementation, syntax/resource bounds, overflow response and executable correspondence | model **proved**. Implementation correspondence and owner policy decisions remain **open** |
| snapshot / parameter layout (`snapshot.rs`) | **not modelled**: layout and retention are #33/#43 subjects and have no binary32 obligation |
| training admission (`grad.rs`) | **not modelled here**: #45 owns the derivative and training admission |
| software helpers (`pow2`, `log2`, `pow2Int`, `roundPositive`) | **model-internal and structural**: they are the reference's own arithmetic, total and kernel-reducible, so they carry no Rust correspondence claim. `Binary32Error` additionally proves quotient rounding and conditional normal-range error bounds |
| optimizer slice | **not modelled**: no optimization strength or decision is claimed for binary32 |

Receipt for the fold and the classification:
[verification/foundation/receipts/c33885419/](../../verification/foundation/receipts/c33885419/foundation-check.json)
(463 theorems, `clean_source: true`, axiom sets inside the allow-list).

Receipt for the conditional rounding-error lane:
[verification/foundation/receipts/a8877ff78/](../../verification/foundation/receipts/a8877ff78/foundation-check.json)
(588 theorems, `clean_source: true`; the 19 new error theorems use only the
foundation allow-list). The normal-bin scale inequalities remain explicit
premises until their universal floor-log derivation is proved; subnormal and
implementation correspondence remain open.

The test-local mutation lane (`NobleFoundation/Binary32Mutations.lean`) declares
six wrong models that are compiled only in this project: away-from-zero ties,
collapsing every zero result to `+0`, flush-to-zero, a zero-sign-insensitive
observation, a commutative subtraction and implicit contraction (a fused
multiply-add). Each is caught by a named retained expectation, and
`all_declared_mutations_are_caught` states that none of the six escapes.
Detection is kernel-checked model disagreement, not a Rust-side mutation run.
Until the 2026-10-29 correction, the contraction variant was the reference's own
product-then-sum rounding rather than a fused one, and `all_declared_mutations_are_caught`
left it out; it is now the single-rounding form and is included (receipt
[verification/foundation/receipts/9f1288d61/](../../verification/foundation/receipts/9f1288d61/foundation-check.json)).

The packed Rust graph reference and the compiled graph legs replay all 26
retained expectations (`crates/noble-ngraph-experiment/tests/b2_differential.rs`):
the packed lane calls `noble_ngraph::sem::add/sub/mul`, and the compiled lane
builds a two-input F32 graph per expectation and checks legs agreement plus the
L2 APFloat, L3 reference and L4 compiled observations. NaN expectations match by
observation equality and every other class by its exact encoding. The
receipt `verification/binary32/replay.json` binds the lane, `propose.py`,
`proposals.json`, both Lean model files and the clean foundation-receipt
membership. It also records the source commit/tree, target triple, full
`rustc -vV` output and FP-relevant flags/environment. The digest-only command
`node verification/binary32/replay.mjs --verify verification/binary32/replay.json`
recomputes these bindings without running Cargo. The first replay found
that the Rust reference returns `0xFFC00000` where the model produces its own
canonical `0x7FC00000` for `inf + -inf`; both are one NaN observation, which is
exactly why the lane compares observations rather than payload bytes.

A bounded deterministic shrinker in the same test file minimizes one binary32
operation discrepancy: it walks a documented simplification list, accepts only
strictly simpler operand pairs under the documented order, and reports a
`local` minimum inside a documented attempt budget. The flush-to-zero lane
checks determinism, replayability and termination, and
`verification/binary32/shrinker.json` records the witness
(`0x00800000 * 0x3f000000` keeps its subnormal product where flush-to-zero
returns zero), the clean commit, the toolchain, target metadata and the same
proposal/Lean bindings. Its `shrinker.mjs --verify <receipt>` mode performs
the same digest-only checks without Cargo. Because a simplification outside
that list is never invented, the lane claims no reachability beyond the list
and no global minimality. The source commit/tree check allows only the two
receipt paths to change in the evidence commit that follows receipt generation.

Open, and deliberately not closed by this model:

- correspondence between `crates/noble-ngraph/src/sem.rs` and the model, which
  is a separate trusted-correspondence claim: the replay shows agreement on 26
  retained expectations on one host and target, not a refinement proof;

- the extraction, training, snapshot and codec layers, guard/index rules, software
  helpers and optimizer behavior;
- the fold is modelled and proved as a recursion over the reference operations, but no
  Lean model maps the graph operations (`Op::F32Add`/`Sub`/`Mul`/`Affine`) to them;
- the subnormal branch of the rounding error rule. `Binary32Error.lean` proves
  the universal quotient-rounding rule, exact normal-output values and the
  normal-range absolute/relative bounds, and `Binary32Scale.lean` discharges
  their scale premises, but neither extends them to gradual underflow;
- retained-expectation shrinking is exercised only for the flush-to-zero
  multiplication candidate
  (`retained_expectation_discrepancies_shrink_and_preserve_their_classes` in
  `crates/noble-ngraph-experiment/tests/b2_differential.rs`); no other candidate or
  operation is shrunk;
- the APFloat differential lane stays secondary finite evidence, and its guard
  facts are not independent.

The first retained receipt is
[`verification/foundation/receipts/4be2a68/`](../../verification/foundation/receipts/4be2a68/index.json):
205 theorems, nine of them binary32, `clean_source: true`, axiom sets inside
`propext`/`Classical.choice`/`Quot.sound`. Later foundation receipts cover every
binary32 theorem, including the fold receipt above.

## Checked I64 reference (issue #39, phase 1)

`NobleFoundation/CheckedI64.lean` is a name-neutral reference for checked
signed-I64 scalar arithmetic. It uses `I64Range` from `Repr.lean` and returns
the exact mathematical result on success, or `ArithError.overflow` when the
result is outside that range. Addition, subtraction, multiplication,
negation, and absolute value have universal success and overflow
characterisations (`checkedAdd_ok_iff` through `checkedAbs_overflow_iff`),
including the two signed-minimum controls `neg_min_overflows` and
`abs_min_overflows`.

Division and remainder are defined independently of the library integer
division policies. `tquot` and `trem` use `Int.sign`, `natAbs`, and Nat
`/`/`%`; `div_rem_identity` proves reconstruction, while `trem_natAbs_lt`
and `trem_sign` prove the magnitude and sign laws for a nonzero divisor.
`checkedDivRem` reports division by zero before quotient evaluation and
`checkedDivRem_overflow_iff` isolates the only in-range nonzero-divisor
overflow, signed minimum divided by negative one. The zero-divisor theorem
is `checkedDivRem_divisionByZero`.

The kernel-checked controls are `truncating_quotient_control`,
`truncating_remainder_control`, `signed_order_control`,
`off_by_one_boundary_control`, `swapped_sub_control`,
`swapped_divrem_control`, and `min_neg_one_has_no_quotient`. They distinguish
truncation from floor division, signed from unsigned `BitVec` order, the
strict upper boundary, operand order, and the absent quotient at the
minimum/negative-one pair.

The clean-source receipt for the checked I64 reference is
[`verification/foundation/receipts/d3f856d50/`](../../verification/foundation/receipts/d3f856d50/foundation-check.json)
(646 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- no Rust implementation, frontend, runtime helper, API name, error
  representation, or overflow-policy correspondence is claimed;
- no library division operation is selected by this reference, and no
  compiler or generated-artifact theorem follows from these model laws;
- the signed order definition is a mathematical `BitVec.toInt` relation only.
## Exact and RNE decimal import (issue #40, phase 1)

`NobleFoundation/DecimalImport.lean` is the name-neutral model for an explicit
decimal import boundary. A `Decimal` stores a sign bit, a
most-significant-digit-first list of decimal digits, and an integer power of
ten. `decimalRatio` interprets that spelling as a natural numerator and
denominator, while `exactDecimalValue` gives the corresponding signed rational
for the optional value bridge. `rne` applies `roundPositive` exactly once to
the natural ratio and ORs the explicit sign onto the resulting binary32
magnitude; it does not introduce an intermediate binary format. Zero-valued
spellings retain either zero sign.

`exactImport` is intentionally narrower than `rne`: it returns the RNE bits
only when they are finite and their scaled magnitude cross-multiplies exactly
with the decimal ratio. `outcomeClass` reports an infinity-magnitude result
as `overflow` but does not choose an infinity-versus-error response. The
universal model claims are `decimalRatio_den_pos`, `isNegative_eq_msb`,
`rne_negative_sign`, `zero_spelling`, `outcomeClass_overflow_iff`,
`exactImport_some_iff`, `exactImport_agrees_with_rne`,
and `exactImport_exactValue` (with its explicit sign premise).

The kernel-checked fixtures retain the exact/RNE split for `0.1` and `0.5`,
negative zero, `10^39` overflow classification, `10^-45` least-subnormal
rounding, an exact halfway case, and a decimal just above that tie. The last
fixture states only the single RNE result (`0x3F800001`); it makes no claim
about a binary64 route. The clean-source receipt for the decimal-import model is
[`verification/foundation/receipts/58aaefd05/`](../../verification/foundation/receipts/58aaefd05/foundation-check.json)
(707 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed by this model:

- syntax validation, significant-digit and exponent/resource bounds, and the
  exact-import implementation correspondence in `sem.rs`;
- whether overflow returns infinity or a checked error, and the versioned
  public mode/signature/error table;
- finite-decimal provenance, serialization, graph identity, and executable
  import/export replay;
- I64/F32 adapters, which belong to the separate #39 model and are not defined
  here.

## Ownership accounting oracle (issue #59)

An independent Lean ownership-accounting oracle for the ordered stack / resource
rules lives in `NobleFoundation/Ownership.lean` and
`NobleFoundation/OwnershipConservation.lean`. It adapts the
executable-checker-to-declarative-typing architecture rather than Move's
ownership policy: an executable checker `run` and an independent declarative
judgment `Accounts`, with `run_sound` proving that every checker acceptance has
a judgment.

The declared fragment is deliberately small: data and must-use resource
payloads, recursive `wrapped` payloads, the words `dup`/`drop`/`swap`/`quote`,
composition, a finite two-arm branch and the ordinary error-return alternative.
A payload is duplicable exactly when it contains no resource at any depth
(`duplicable_iff`), wrapping never changes that
(`wrapDepth_preserves_eligibility`), and a must-use resource as the
constrained operand blocks each data-constraining word
(`resource_operand_blocks_non_swap_word`). Dropping and duplication therefore
only move data obligations (`drop_consumes_one`, `dup_duplicates_one`), while
`swap` reorders either data or resource operands without an eligibility check
(`swap_permutes_two`, `swap_moves_a_resource`). Quoting retains rather than
consumes (`quote_retains_one`), a two-arm branch accounts for the same
obligations on both arms (`branch_accounts_both_arms`), and an ordinary error
return is acquired only with no outstanding operand
(`fail_requires_empty_operands`).
The old `segment_conserved` theorem is deliberately narrower than its former
wording suggested: it extracts the operand `run` result from a
`runConfiguration` result whose segment is already syntactically the original
segment. The new `runConfiguration_segment_preserved` theorem proves that
successful configurations cannot return a different segment.

The conservation unit proves `step_conserves_obligations` for one accepted
word, lifts it through the declarative judgment as
`accounts_conserves_obligations`, and exposes the checker corollary
`run_conserves_obligations`. This is exact conservation of the filtered
must-use resources. The grammar has no admitted acquire, release or
resource-consuming word; `swap` can move a resource, while `dup`, `drop` and
`quote` reject one in their constrained position. The decided
`resource_carry_run` and `resource_carry_accounts` fixture demonstrates
carry-through with `dup` followed by `drop` on a data head and a resource tail,
and `swap_moves_resource_conservation` demonstrates a resource changing
position. Thirty-four ownership theorems are inventoried in total.

### Branch join (issue #59 joins)

The two-arm branch's abstract stack is a join, not "both branches have the same
number of resources". `BranchJoin` carries both arm results at equal length, so a
position one arm lacks is refused rather than padded
(`unequal_arms_cannot_be_joined`), and `mergePosition` records both arms at every
position (`merge_represents_each_arm`). Two different payloads at one position are
recorded as conflicting rather than merged (`distinct_owners_are_not_merged`), so
the seeded join that keeps only the first arm is refuted wherever the arms differ
(`one_armed_join_is_refuted`). The controls that a weaker join fails are proved as
theorems: equal counts with different identities are conflicting
(`same_count_different_identity_is_conflicting`), a wrapped payload is not merged
with its unwrapping although both are eligible
(`nested_payload_is_not_merged_with_its_unwrapping`), and a wrapped must-use
resource is never merged with a plain payload (`nested_resource_is_not_merged`).
The substantive preservation result is `joined_word_admission_agrees`: two stacks
whose ordered positions agree on eligibility are acquired by one word in both arms
or in neither, so the joined abstract stack preserves this fragment's accounting
decision rather than inventing one.

The branch constructor stays outside the join lane on purpose: it compares the two
arms' actual results, so a join can never be used to skip accounting one arm, which
is what `branch_accounts_both_arms` and the mutation controls establish. The join
controls are model-level only; the production acceptance machine has no join
concept, and a join is never presented as an executable production control.

`NobleFoundation/OwnershipJudgment.lean` closes two gaps in that structure. First, `Accounts.word`
takes `step w operands = some out` as its premise, so the judgment's word rules delegated to the
checker. `WordRule` states one rule per word from the fragment's prose instead (`dup`, `drop`
and `quote` need an operand with no resource at any depth; `swap` reorders two operands
unconditionally), and `step_iff_wordRule` proves the executable step agrees with it exactly.
`Judged`, the judgment built on those rules, coincides with `Accounts` (`judged_iff_accounts`),
and with the new completeness theorem `run_complete` the checker decides it exactly
(`run_iff_judged`).

Second, the admission simulation is lifted over whole straight-line word sequences. Each step
preserves eligibility agreement (`step_preserves_agreement`), so a straight-line program is
admitted on two agreeing stacks or on neither and leaves stacks that agree again
(`joined_sequence_admission_agrees`); `joined_judgment_transfers` states the same over `Judged`.
`equal_length_does_not_lift` is the control that the lift needs eligibility agreement, not a
count, and `lift_fixture` runs a three-word sequence over stacks with different owners.
Branches and the error return compare actual results and stay outside the lift.

The clean-source receipt for the per-word rules and the join lift is
[`verification/foundation/receipts/0cf855674/`](../../verification/foundation/receipts/0cf855674/foundation-check.json)
(813 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

The retained receipt for the corrected swap rule is
[`verification/foundation/receipts/23462be4b/`](../../verification/foundation/receipts/23462be4b/foundation-check.json):
608 theorems, thirty-four of them ownership (26 `Ownership`, 8 `OwnershipConservation`),
`clean_source: true`, axiom sets inside the allow-list. The conservation-only revision before
the swap correction is retained at `656ec91b4`.

The comparison is finite and deliberately narrow:
[`verification/ownership/differential.json`](../../verification/ownership/differential.json)
records 108 independent cases generated from the model, of which 15 accepted and
6 rejected cases are driven through the actual acceptance machine
(`noble_kernel::acceptance::check`) over the unary `dup`/`drop` fragment, and 87
are model-only. A test-local shadow checker agrees with every expectation and
each of seven declared mutations (duplicating or releasing a must-use resource,
drops one operand during `swap`, consuming a quoted operand, ignoring branch
accounting, permitting an error return with an outstanding operand) is caught by
at least one retained expectation. The model-only controls include a branch
that attempts to lose a resource, a branch that attempts to duplicate one,
equal-count branches with different resource identities, and a resource move.

Open, and deliberately not closed by this model:

- correspondence between `crates/noble-kernel/src/acceptance` and the model: a
  handwritten comparison, not an extraction or refinement proof;
- retain-in-place production analogues beyond the unary fragment: the production
  quotation constructor wraps its operand into a program value, and the
  two-operand swap value-slot ordering is not exposed as an operand-stack order
  by the acceptance machine, so those cases stay model-only;
- the production rejection category of a resource payload can be a stack-join
  failure rather than eligibility; the lane asserts agreement of the accounting
  decision and reports the split;
- OWN-PO-2 (scope escape) and any future scoped-borrow syntax, package
  retention/reclamation and the host-ledger OWN-PO-4..7 obligations;
- host resource release: no acquired decision in this fragment releases a native
  object, and no runtime claim follows for an unimplemented profile.

## Constructor reuse under a unique root (issue #36, phase 1)

`NobleFoundation/ConstructorReuse.lean` is a finite observation model for the
ownership-aware direct-lowering reuse candidate. A heap is a lookup
`Nat → Option Cell`; each `Cell` records a tag, a size and an ordered field
list, and each field records a target reference plus an ownership payload.
`reachableFrom` deliberately has depth one: it recognizes a root itself or a
target named by one of that root's fields, and does not recurse through target
cells. `unique roots r` means that `r` occurs exactly once in the ordered root
list.

`reuseStep` overwrites one present cell only when the old and requested sizes
match. `reuseStep_replaces_unique_root` models consumption of the old value by
the unique root: that root observes the requested replacement while every
other root at a different reference observes the same cell as before.
`reuseStep_initializes_requested_cell` exposes the requested tag, field shape,
size and initialized state, and `reuseStep_refuses_size_mismatch` is the
refusal control. The `shared_alias_control` fixture has two roots at one
address and is checked by `decide`; its second root observes the changed cell,
which is why uniqueness is required rather than inferred from a last use.

The resource control uses `Ownership.containsResource` recursively and
`Ownership.duplicable` for every field payload. A wrapped must-use resource is
therefore detected and is not reuse-eligible
(`wrapped_resource_is_not_reuse_eligible`). This model has no resource-transfer
operation: "moved" means only that the resource is absent from the replacement
fields (`resourceMoved` and `eligible_replacement_requires_move`). Ordinary
immutable data remains eligible when its payloads are duplicable.

The clean-source receipt for the constructor-reuse model is
[`verification/foundation/receipts/9c93d6cc8/`](../../verification/foundation/receipts/9c93d6cc8/foundation-check.json)
(737 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed by this model:

- recording that the post-M3 layout trial is selected remains an owner decision;
- the frozen experiment configuration and decision thresholds remain an owner
  decision;
- any default change remains an owner decision;
- allocator, lowering, runtime and Wasm correspondence is not modelled;
- no ADAPT-17 workload has been executed by this model, including quota,
  cleanup, capture, reflection and ordered-effect controls;
- no allocation, copy, retained-byte, stack-bound or performance claim follows
  from the finite overwrite equations.

## Checkpoint-scoped authority ledger (issue #66)

`NobleFoundation/AuthorityLedger.lean` models the M5 protected-operation
authority ledger with bounded witnessed requests, an independently supplied
fact bundle (plan, grant, requested identities, source authenticity, checkpoint
validity, policy liveness, credentials, revocation, cost, available quota), a
consumed-witness list, a registered-attempt ledger and checkpoint-local charges.
Denials and preflight refusals leave the ledger definitionally unchanged; only
their separately recorded request/effect counters move.

Proved: initialization of align/unique/quota/at-most-once
(`initial_alignment`, `initial_uniqueness`, `initial_within_quota`,
`initial_started_at_most_once`); one admission consumes exactly one fresh
witness and registers exactly one not-yet-started attempt
(`admission_consumes_exactly_one_witness`, `admission_registers_one_attempt`,
`admission_is_consumption`) while keeping alignment, uniqueness, the
checkpoint quota bound and attempt/charge accounting
(`admission_preserves_alignment`, `admission_preserves_uniqueness`,
`admission_preserves_quota`, `admission_preserves_attempts_charged`); any finite
serialized prefix keeps alignment (`acquirePrefix_preserves_alignment`); an
altered requested plan, revoked or stale facts, or an already started attempt
mint nothing (`altered_plan_cannot_mint`, `stale_facts_cannot_mint`,
`started_attempt_cannot_start_again`). Four mutation models break the
corresponding invariant, which is what makes exact plan equality, checkpoint
validation, freshness and the quota charge load-bearing rather than decorative
(`omitted_quota_charge_breaks_accounting`, `unchecked_reuse_breaks_uniqueness`,
`plan_blind_admission_mints_altered_plan`,
`checkpoint_blind_admission_mints_stale_checkpoint`).
`renewed_snapshot_keeps_quota` states checkpoint renewal as a host premise: a
fresh snapshot must carry prior reservations forward, and host replenishment
that ignores them is outside kernel-enforced safety. Twenty-five inventoried
theorems; receipt
[`verification/foundation/receipts/674b32d9/`](../../verification/foundation/receipts/674b32d9/foundation-check.json)
(286 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- no correspondence claim relates the model to
  `crates/noble-kernel/src/authority`: there is no authority extraction yet, so
  this is finite model evidence rather than an implementation theorem;
- the finite Rust comparisons of Phase 2 are named, measured results over the real
  authority table rather than theorems, and they are partial: two witnesses
  sharing one checkpoint's quota is measured by
  `bindings::same_checkpoint_quota_is_reserved_across_distinct_witnesses`;
  admission order independence by `phase2::both_admission_orders_charge_the_same_quota`
  (both orders commit exactly one and refuse exactly one, with identical
  counters); exhaustion *before any charge* by
  `phase2::quota_exhaustion_stops_before_consumption`; a fresh snapshot that does
  not carry prior reservations by `phase2::a_fresh_snapshot_without_carry_forward_is_refused`,
  which observes `Denial::InconsistentQuota` rather than extra capacity; and
  finalization between admission and start by
  `phase2::finishing_the_invocation_between_admission_and_start_stops_the_start`
  (the start is refused `InvocationFinished`, no protected operation runs, and
  the consumed witness is not restored). Facts changing between authorization
  and admission are measured for a changed quota snapshot (refused
  `InconsistentQuota`) and for revocation
  (`phase2::revocation_between_authorization_and_admission_refuses_the_admission`:
  refused `Denial::Revoked` with no attempt registered, nothing consumed and the
  witness left retirable); a change of credentials, policy or checkpoint between
  those two calls has no separate Rust scenario;
- no concurrency, crash-recovery, remote-revocation atomicity or exactly-once
  claim, and no cross-checkpoint conservation claim without the carry-forward
  premise.

## Reverse-mode AD reference (issue #45, phase 1)

`NobleFoundation/ReverseAD.lean` models the bounded smooth slice — a
well-formed acyclic graph over the reals with explicit input and parameter slots,
exact finite constants, addition, subtraction and multiplication, and one
selected output. Guards, indexing, Boolean paths, activation corners, surrogate
rules, and the ordered affine terms the issue grants only later are outside the
file, as are dynamic Programs. Subtraction was admitted after the add/multiply
core was proved; its derivative case is part of `evalStep_hasDerivAt`.

Proved: a node's evaluation is differentiable along a direction line whenever its
operands are (`evalStep_hasDerivAt`), and by induction on the stored order every
node's denotation is (`valuesUpTo_hasDerivAt`), so the forward tangent recursion
*is* Mathlib's derivative of the intended-real denotation at the declared
assignment — the JVP specification (`tangent_is_directional_derivative`). The
direction line starts at the declared assignment (`line_zero`). The reverse
traversal adds one processed consumer's contribution to an operand's adjoint
(`adjoint_step_adds_contribution`), a repeated operand contributes twice so
accumulation adds rather than overwrites (`repeated_operand_contributes_twice`),
and a node that is not an operand of the processed consumer is left unchanged,
which is what makes the descending order finalize an adjoint before its own step
(`adjoint_unchanged_when_operand_absent`). A contribution multiplies the
**consumer's** own adjoint by its partial-derivative coefficient; the first draft
multiplied the *operand's* adjoint instead, which cannot carry a seed from the
output node to its operands at all, and the transpose identity below refuted it.
The transpose is then proved at two scopes: the reverse-loop **pairing
invariant** (`adjointDot_step`) says pairing the accumulated adjoints with a
direction over the graph's slots adds exactly the processed consumer's
contributions, and the fixture-level **transpose identity**
(`exampleGraph_adjointDot`) says the sweep's accumulated adjoints pair with the
declared direction to exactly the tangent `8`. The overwrite control
(`the_overwriting_update_pairs_to_four`) pairs to `4` on the same slot where the
sweep pairs to `8` (`the_sweep_pairs_to_eight_on_the_second_slot`), so the fixture
discriminates the update rule rather than agreeing with anything.

The slot extraction mirrors the implementation boundary: `slotNode` scans the
finite node order for the node implementing a declared input/parameter slot,
and `slotGradient` reads that node's final reverse adjoint, matching the
`slot_grads`/`slot_node` shape in `crates/noble-ngraph/src/grad.rs`. The second
fixture is intentionally non-index-aligned (`slotGraph_slotNode_map`): node `0`
implements slot `1` and node `1` implements slot `0`; node `2` fans out to two
distinct consumers; node `4` repeats the slot node in `mul 0 0`; and node `3`
multiplies distinct operands. Its output is `39`, with unit forward tangents
`17` and `26` (`slotGraph_tangent_slot_zero`,
`slotGraph_tangent_slot_one`) and corresponding Mathlib derivative statements
(`slotGraph_derivative_slot_zero`, `slotGraph_derivative_slot_one`). The mapped
reverse extraction gives those same values for a unit output seed
(`slotGraph_slotGradient_unit`,
`slotGraph_slotGradients_match_unit_tangent`) and gives `85` and `130` for a
seed of `5` (`slotGraph_slotGradient_five`,
`slotGraph_slotGradients_match_five_tangent`). The mutant extraction that reads
the slot number as a node index is rejected by
`slotGraph_wrong_slot_control`. The two fixture well-formedness/value claims are
`exampleGraph_wellFormed`, `exampleGraph_value`, and `slotGraph_wellFormed`,
`slotGraph_value`; the remaining concrete controls are
`exampleGraph_tangent`, `exampleGraph_derivative`, `unused_input_slot_tangent`,
and `non_unit_seed_unconsumed`.

Receipt:
[`verification/foundation/receipts/9936fe19c/`](../../verification/foundation/receipts/9936fe19c/foundation-check.json)
(606 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`). The phase-1 theorems were first retained at
`b91f5504c`.

`NobleFoundation/ReverseADMutations.lean` adds mutated-update controls. It reruns the sweep
with its partial-derivative rule as a parameter; `adjointAfterWith_stepCoefficient` proves
that under the model's rule this is exactly `adjointAfter`, so each mutant differs from the
checked sweep in its one rule only. On `slotGraph`, where the tangents are `17` and `26`:

- a multiplication rule that uses each operand's own value instead of its partner's gives
  `24` and `22` (`slotGraph_mulOwnValue_caught`);
- an addition rule that drops its second operand gives `0` and `1`
  (`slotGraph_addDropsSecond_caught`);
- counting a repeated multiplication operand once gives `17` and `24`: only the slot behind
  `mul 0 0` moves (`slotGraph_repeatedCountedOnce_caught`);
- overwriting adjoints instead of accumulating gives `1` and `1` (`slotGraph_overwrite_caught`).

The subtraction-sign control needs a subtraction node, so `subGraph` computes `(x - y)^2` with
non-index-aligned slots and the operands of `sub 1 0` in reverse node order. Its tangents are
`2` and `-2` (`subGraph_tangents`, with `subGraph_derivative_slot_one` tying the `y` tangent to
the Mathlib derivative), and the model sweep gives exactly those gradients
(`subGraph_slotGradients_match_tangents`). Flipping the sign of the second operand moves the
`y` gradient to `2` (`subGraph_subSignFlipped_caught`). These are seeded faults, not an
exhaustive fault model.

The clean-source receipt for the mutation controls is
[`verification/foundation/receipts/309f46c02/`](../../verification/foundation/receipts/309f46c02/foundation-check.json)
(804 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- no ordered **binary32** gradient model: a real-valued theorem does not
  establish that binary32 gradients equal exact derivatives, and no
  reassociation, FMA contraction or parallel reduction is authorisation;
- the transpose identity is proved for the **one-step pairing and one checked
  fixture**, not for every well-formed graph, and the full summation theorem
  stating each final adjoint as seed plus its consumers stays a named
  obligation;
- no correspondence claim relates this file to
  `crates/noble-ngraph/src/grad.rs`, so the ordered Rust loop's refinement is
  open, and its admission, error and quota behaviour is untouched;
- guards, indexing, Boolean paths, activation corners, surrogate policies,
  dynamic Programs, multi-output seeds and duplicate output-node accumulation
  are outside the selected fragment;
- vanishing checks are absent by construction (no `sorry`, no axiom, no
  `native_decide`), but that is a hygiene fact, not a claim about the Rust side.

## Packed schedule validator model (issue #43, phase 1)

`NobleFoundation/PackedSchedule.lean` now contains an independent validator for a
*given* instruction stream. The producer supplies a node `order`, the `Instr`
list, a node-to-register function `regOf`, and parallel output-node/output-register
lists. `validate` does not call the model scheduler or regenerate the stream.
It checks, in order: a bounded, duplicate-free node order; exact instruction
operation, ordered operands, destination register and constant bits for every
node; definition-before-use for every operand; injectivity of `regOf` restricted
to scheduled nodes; and positional output mapping. Output lists are not sets, so
an output repeated twice must be paired with the same register twice.

`validate_sound` is proved for the fragment model under the explicit premise
`WellFormed graph`: an accepted supplied stream returns the graph reference value
at every requested output, including repeated outputs. The proof reconstructs the
supplied stream from the checked bodies and then reuses the reference-machine
induction with injectivity restricted to the scheduled nodes. The body bridge is
`bodiesMatch_eq_emitNodes`; dependency and register obligations are connected by
`operandsDefined_sound` and `registersInjectiveOn_sound`; output-list equality
is handled by `outputsMatch_runOutputs`.

Fixtures and controls are all kernel-checked with `decide`: the model's own
`exampleSchedule` is accepted (`exampleSchedule_validated`); a creation-order
stream that executes an independent layer-0 input after a layer-1 addition is
accepted even though it is not layer-sorted
(`reordered_order_not_layer_sorted`), with output 4 repeated twice
(`reordered_stream_validated_with_repeated_output`); and the validator refuses
an operand used before definition, an alias between two scheduled live nodes, a
wrong operation, and a wrong output register
(`validate_refuses_operand_before_definition`,
`validate_refuses_live_register_alias`, `validate_refuses_wrong_operation`,
`validate_refuses_wrong_output_register`). The repeated-output execution
fixture is `reordered_stream_sound`.

Open, and deliberately not closed:

- **No Rust instruction-body export yet.** `Compiled::new`'s Rust `I` stream
  remains private; this Lean validator has no decoder or byte-level instruction
  input to consume.
- **No Lean↔Rust correspondence theorem.** The result is a proof about the
  definitions in this file, not a theorem that the Rust evaluator's private
  instruction bodies equal these `Instr` values or that its graph matches this
  `Graph`.
- The selected fragment excludes guards, failure-suffix ordering, F32/affine
  instructions, effects, resources, register reuse after last use, and dynamic
  multi-output reconstruction. The graph well-formedness premise and all
  unsupported Rust behaviours remain explicit rather than inferred from the
  finite fixtures.

The clean-source receipt for the instruction-stream validator is
[`verification/foundation/receipts/48a7a4cbd/`](../../verification/foundation/receipts/48a7a4cbd/foundation-check.json)
(622 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`). The earlier schedule theorem set was first
retained at `8eef09184`.

`PackedLayout.lean` remains a separate declaration-table model. Its register,
constant/slot, layer, count and failure-suffix obligations do not export or
validate instruction bodies, and no correspondence theorem joins that layout
model to Rust bytes or to this validator.

## Graph rewrite certificates model (issue #42, phase 1)

`NobleFoundation/GraphRewrite.lean` (over the `PackedSchedule` model) keeps
optimization and search untrusted and proves the accepted rewrite rules,
congruence, the lifting of one local replacement to the whole graph, and a small
checker whose acceptance implies the declared denotation. The slice is the same
total `I64`/`Bool` one: input/constant, wrapping add/subtract/multiply and
equality. Guards, effects, resources, reflection, `F32` and dynamic calls are
outside the file.

Proved: constant folding denotes the wrapping sum (`fold_add_constants`), including
the boundary case (`add_wraps_at_the_boundary`); `0 + x` and `x * 1` denote `x`'s
`I64` denotation, with the operand's `I64` value as an explicit premise rather than
a total rewrite claim (`neutral_add_left`, `neutral_mul_right`); the step function is
congruent in node identity and operand lookups (`evalStep_congr`), so equal operand
denotations give equal node denotations (`congruence`); and **lifting** carries one
unchanged local denotation to agreement over every node of the graph, which
preserves unaffected shared uses (`replacement_lift`). The checker decodes exact
node shapes, operand positions and constant bits from the graph — it never asks
the reference for the answer — and acceptance implies the declared denotation
(`checked_sound_const_add`, `checked_sound_zero_left`, `checked_sound_one_right`).
The format model also proves the replay endpoint theorem
(`Certificate.accepted_endpoint_equivalence`): for accepted steps whose premises
are checked against the original graph, applying every fold in order (while
recording but not applying identity steps) preserves every node denotation. This
is a theorem about the Lean replay model, not yet about the Rust implementation.

Fixtures and controls: the three-constant fixture denotes `8` at both its addition
and its multiplication (`foldGraph_reference`); the true constant-folding claim is
accepted (`fold_certificate_accepted`) and a wrong claimed constant
(`wrong_constant_refused`), swapped operand positions (`swapped_operands_refused`),
a claim naming a node of another kind (`wrong_rule_identity_refused`) and the
identity claim at swapped positions (`wrong_bits_refused`) are refused. Eighteen
inventoried GraphRewrite theorems. The two-independent-fold certificate fixture
is accepted and has endpoint agreement (`multiFold_derivation_accepted`,
`multiFold_reference`, `multiFold_endpoint`), while a substituted result graph
differs from replay (`substituted_result_differs`). The GraphRewrite theorems are
retained at
[`verification/foundation/receipts/71c02b9bb/`](../../verification/foundation/receipts/71c02b9bb/foundation-check.json)
(345 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`); the endpoint theorem and the two-fold fixture are
retained at [`verification/foundation/receipts/50ec19885/`](../../verification/foundation/receipts/50ec19885/foundation-check.json) (594 theorems).

Open, and deliberately not closed:

- **correspondence to `cert.rs` remains open**: the Lean endpoint theorem above
  is about `Certificate.applyDerivation`, while
  `crates/noble-ngraph/src/cert.rs` emits and checks a versioned `ngraph-cert/0`
  derivation. No theorem yet binds the Rust steps, replay or result graph to the
  Lean structures; there is also no replay budget, Rust byte codec or wire-size
  measurement. The checker now replays folds on the original and refuses any
  other result (`Refusal::ResultMismatch`), but that fix is implementation
  evidence rather than a Lean correspondence result;
- `crates/noble-ngraph/tests/cert_differential.rs` executes both endpoints of
  single-step folds on the compiled and reference evaluators; no multi-step
  simplification is admitted, and its expectations are computed in Rust rather
  than kernel-checked;
- substitution/transitivity closure, a full e-graph, guard/failure-preserving
  rewrites and `F32` laws are outside phase 1; value preservation says nothing
  about fuel, allocation, cancellation behaviour or recipe identity;
- finite fixtures are not a universal claim about any scheduler or transformation.

## Declared packed graph layout (issue #43)

`NobleFoundation/PackedLayout.lean` models the declared packed layout that
`crates/noble-ngraph/src/layout.rs` emits and the obligations its checker applies:
a revision, a declared register count, one declared register per node, the constant
and interface-slot tables, one instruction range per layer, the failure-suffix
prefix minima and the acceptance conjunction of those obligations.

Proved: an inflated register count is refused (`count_inflated_refused`) while a count
one above the highest declared register's count is accepted
(`count_one_above_is_accepted`); a missing,
out-of-range or shared register is refused (`missing_register_refused`,
`out_of_range_refused`, `alias_refused`); a constant table with the wrong entry count
is refused (`constant_count_refused`); the layer table must have one ordered range
per layer (`layer_shape_refused`, `layers_unordered_refused`) and its ranges must
cover exactly the instruction nodes (`coverage_refused`); a declared failure-suffix
table that differs from the graph's own prefix minima is refused
(`fail_suffix_mismatch_refused`); and acceptance implies a register per node,
non-aliasing and coverage (`accepted_implies_registers`,
`accepted_implies_distinct`, `accepted_implies_coverage`).

**Correction (2026-10-29).** `countWithinOne` used to compare the declared count with
*every* declared register (`regs ≤ register + 2` for each), not with the highest one as the
Rust rule does (`regs - (highest + 1) ≤ 1`). The scheduler declares
`regs = non-guard nodes + 1`, so the old rule refused every real layout with two or more
registers, and no positive fixture existed to show it. The model now uses the highest
declared register (`highestRegister`). `exampleLayout_accepted` checks the exact
declaration `Compiled::new` emits for the Rust counterpart of
`PackedSchedule.exampleGraph`, copied from a probe run where `layout::check` accepts it:
`regs = 5`, registers `[0, 1, 2, 3]`, constants `[(1, 5)]`, slots `[(0, 0)]`, layers
`[(0, 0), (0, 1), (1, 2)]` and four `u32::MAX` failure-suffix entries. The old rule
refused that same declaration (checked in a scratch file, not retained). Receipt
[verification/foundation/receipts/9f1288d61/](../../verification/foundation/receipts/9f1288d61/foundation-check.json)
(557 theorems, `clean_source: true`).

The clean-source kernel check is retained at `verification/foundation/receipts/09a307c64/`
(393 theorems, `clean_source: true`, every axiom set inside the allow-list).

Per-entry exactness is tied to the readable obligation by `constant_predicate_iff` and
`slot_predicate_iff`, and an entry that names no node is refused
(`constant_entry_refused`, `slot_entry_refused`). The clean-source kernel check is
retained at `verification/foundation/receipts/9b1b93055/` (397 theorems, `clean_source: true`).
Not claimed: no decoding of the Rust declaration's bytes, so nothing binds
`serde`/binary output to these tables; the layout build key, compiler revision and
identity scheme are out of scope. The node vocabulary modelled here is the pure-`I64`
fragment, so the slot table obligation is the one for **input** slots: the `ngraph-f32/0`
profile's `Op::Param` slots and its F32 instructions are not modelled. The Rust validator
covers both slot kinds in one table, as issue #68 required the validator to recognise after
it refused the compiler's own parameter layouts.

## Replayable certificate codec (issue #42, codec half)

`NobleFoundation/CertificateCodec.lean` models a token codec for the same
`ngraph-cert/0` derivation: one seven-token chunk per step, a rule token that must
decode through the modelled rule set, decimal bit tokens that must stay inside the
64-bit range, and a revision token the decoder re-checks before any step.

Proved: decoding an encoded step returns the same step (`decode_encode_step_replays`)
and decoding an encoded derivation returns the derivation itself
(`decode_encode_replays`), so acceptance is invariant under a transport round trip
(`acceptance_preserved_by_codec`); a step is exactly seven tokens
(`encodeStep_length`), so a six-token tail is refused (`truncated_step_refused`);
an unknown rule token refuses the step (`unknown_rule_token_refused`); and a stale
revision token refuses decoding before any step is inspected
(`stale_revision_token_refused`).

The clean-source kernel check is retained at `verification/foundation/receipts/26fca0806/`
(381 theorems, `clean_source: true`, every axiom set inside the allow-list).

Not claimed: tokens are modelled as strings and no byte-level envelope is modelled,
so nothing here binds the `serde`/binary bytes the Rust encoder writes; envelope
digests, replay-budget accounting and the identity rules are out of scope.

## Replayable certificate format (issue #42, format half)

`NobleFoundation/Certificate.lean` models the `ngraph-cert/0` derivation format that
`crates/noble-ngraph/src/cert.rs` emits: the revision, the declared step bound, the
ordered step list (rule identity, rewritten node, ordered operands, declared
constant bits, claimed value), the decode of rule identities to the graph
checker's claims, the structural bit comparison against the graph, and acceptance
of a whole derivation.

Proved: a certificate naming another revision is refused
(`stale_revision_refused`) and that refusal kills acceptance even when every step
would be accepted (`stale_revision_kills_acceptance`); a derivation over the
declared bound is refused (`over_bound_refused`); an unknown rule identity cannot
decode or be accepted (`unknown_rule_refused`); non-increasing node order is
refused (`unordered_refused`); acceptance implies the graph checker accepts every
decoded claim (`accepted_implies_checked`); and consequently an accepted
certificate's fold step denotes the claimed constant
(`accepted_fold_step_denotes`), i.e. the encoder's claim is the checker's
denotation rather than taken on faith. The replay model applies fold steps in
order while leaving identity steps unchanged, and the endpoint theorem
(`accepted_endpoint_equivalence`) proves agreement of every node denotation
under an accepted derivation whose premises are checked against the original
graph. The two-independent-fold fixture and substituted-result control are
`multiFold_endpoint` and `substituted_result_differs`. Thirteen inventoried
theorems; receipt
[`verification/foundation/receipts/50ec19885/`](../../verification/foundation/receipts/50ec19885/foundation-check.json)
(594 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`). The first seven were first retained at
`d67efd4a2` (374 theorems).

Open, and deliberately not closed:

- **no byte decoding**: nothing maps `serde`/binary output of the Rust encoder to
  these structures, so a correspondence theorem binding actual certificate bytes
  (or the Rust checker) to this model still does not exist;
- envelope digests, replay-budget accounting, cold/warm comparison and the #32/#33
  identity rules are outside the model, as are `F32`, guards, effects and
  resources;
- the Rust lane's differential execution (`crates/noble-ngraph/tests/cert_differential.rs`)
  is implementation evidence and is not modelled here.

## Bounded graph-pair query model (issue #56, phase 1)

`NobleFoundation/SymbolicPair.lean` models a *declared finite input universe*, the
exhaustive search over it, and the replay of an exact-bit witness, and proves what
such a search can and cannot establish. It contains no SAT/SMT solver, so no
solver status, no certificate and no external UNSAT reasoning is claimed.

Proved: a witness lookup only ever yields a pair the witness recorded
(`lookup_returns_recorded`), an unrecorded slot decodes to exactly the declared
fallback (`decode_uses_fallback`) and a recorded slot replays exactly its recorded
bits (`decode_replays_recorded`), so a witness cannot acquire bits it did not
carry. No refutation is exactly agreement over the ordered outputs
(`not_refuted_iff_agrees`), and therefore a failed search establishes agreement on
the declared universe **and nothing beyond it** (`search_none_sound`), while a
found candidate is a genuine refutation (`search_some_refutes`).

Fixtures: two differently written faces of `3 + 5` (`add 0 1` and `add 1 0` over
reordered constants) find no distinguishing candidate in the declared universe;
a subtraction face is genuinely refuted (`8` versus `2`); and for a pair whose left
face reads input slot zero while the right face is the exact constant seven, the
**decoded witness bits decide the verdict** (`witness_bits_decide`). Controls: an
omitted slot takes the declared fallback rather than inventing bits
(`unrecorded_slot_falls_back`), and a corrupted witness flips the verdict
(`corrupted_witness_changes_the_verdict`), so the recorded bits are load-bearing.
Twelve inventoried theorems; receipt
[`verification/foundation/receipts/9a810d3ff/`](../../verification/foundation/receipts/9a810d3ff/foundation-check.json)
(357 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- **no encoder correspondence**: nothing relates `extract/symbolic.rs`, a
  serialized formula or the B2 I64 harness to this model, so there is no
  concrete/symbolic simulation theorem and no decoder/binding of actual endpoint
  bytes;
- no solver, no timeout/UNKNOWN budget, no certificate or witness shrinking, and no
  formula-size, solver-time or memory measurement;
- no replay through the actual emitted Wasm of either endpoint and no
  optimization-mode comparison, so this cannot satisfy the issue's compiled-replay
  acceptance on its own;
- budgets, trap/refusal/quota classification, cancellation, guards, `F32`,
  resources, live slots, recursion and dynamic calls are outside the slice.

### Witness encoder correspondence and first-match search (issue #56, phase 2)

`NobleFoundation/SymbolicSolver.lean` closes two of phase 1's open points for the
*witness representation itself*, and one of them only there.

Proved: the encoder records exactly one pair per declared slot
(`encode_length`); every declared slot decodes back to the candidate's own exact
bits (`decode_encode_replays`), so the encoder cannot smuggle a value the
candidate did not carry; and a corrupted witness decodes to the corrupted bits and
not to the candidate's (`corrupted_witness_shifts_bits`). The search is now
first-match: a failed search establishes agreement on the declared universe only
(`firstIndex_none_agrees`), and a reported index comes with a **structural
decomposition** — the universe is a prefix of candidates that do not refute, of
exactly the reported length, followed by a candidate that does
(`firstIndex_some_decomposes`) — so the reported witness can never be a later,
avoidable one, and no positional lookup into the candidate list is needed to say
so. Fixtures rerun through the search: the same-sum pair finds nothing
(`same_sum_first_none`), the subtraction pair and the input-dependent pair are both
refuted at the earliest declared candidate (`subtraction_first_index_zero`,
`input_first_index_zero`, `input_zero_candidate_refutes`), and encoded witnesses
still decide the verdict while a corrupted one does not (`encoded_witness_decides`).
Ten further inventoried theorems (22 for the #56 slice); receipt
[`verification/foundation/receipts/144434256/`](../../verification/foundation/receipts/144434256/foundation-check.json)
(367 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Still open, and explicitly not claimed here:

- the encoder correspondence above is for the **witness representation**; it does
  **not** relate `extract/symbolic.rs`, a serialized formula or the B2 I64 harness
  to the model, so there is still no concrete/symbolic simulation theorem and no
  binding of actual endpoint bytes;
- the earliest-witness statement is structural over a *declared* universe: it says
  nothing about assigning cost, formula size, solver time or memory, and the
  universe is still the two declared candidates, not a generated search space;
- no replay through the actual emitted Wasm of either endpoint, no certificate or
  external UNSAT reasoning, and no timeout/UNKNOWN budget;
- guards, input facts, `F32`, resources, live slots, recursion and dynamic calls
  remain outside the slice.

### Seeded fault variants for the symbolic query lane (issue #56, acceptance item 4)

`NobleFoundation/SymbolicMutations.lean` is the test-local faulty-variant lane the
bounded query needs, so that "the search found nothing" cannot stand in for the
reference result. Each variant differs from a reference face in exactly one place,
and a named reference expectation catches it:

| Variant | Fault | Caught by |
|---|---|---|
| `opMutated` | subtracts where the reference adds | `wrong_op_is_caught` |
| `operandMutated` | reaches one node twice (`5 + 5`) | `wrong_operand_is_caught` |
| `constantMutated` | holds six where the reference holds three | `wrong_constant_is_caught` |
| `twoOutSwapped` | asks the right face for its ordered outputs in the other order | `wrong_output_order_is_caught` |
| `encodeShifted` | records each slot's exact bits at the next slot | `shifted_encoding_is_caught` |
| `refutedFirstOnly` | inspects only the first ordered result | `output_blind_verdict_is_caught` |

The encoding and verdict variants are the interesting ones because both are shapes
of *agreement that proves nothing*. The shifted encoder **round-trips at the slot it
recorded**, so two consumers of that one corrupted encoding agree the bits are
present; the graph reads a different slot, and the verdict flips from agreement to
refutation against the reference replay of the candidate's own bits. The
output-blind verdict reports agreement on a pair whose **second** ordered result
differs, while the reference comparison over both declared outputs reports the
refutation. `all_declared_mutations_are_caught` closes the lane: no declared variant
escapes.

This lane is model-level detection. It does **not** supply the missing encoder
correspondence to `extract/symbolic.rs`, the B2 I64 harness legs or any emitted
Wasm, and it involves no solver, timeout budget or certificate. Seven inventoried
theorems (29 for the #56 slice).

## Specification-quality obligations (issue #65)

`NobleFoundation/SpecQuality.lean` separates three questions about a retained
contract from the implementation claim. `Relation` abstracts the statement shape
of `NobleContracts.exportedClaim` — typed parameters and inputs, a precondition
over them, and a postcondition over the input, the exact typed output and the
parameters — and the file defines domain inhabitation, relational realizability
and output determinacy over it.

Proved: a universal realizability proposition is provable from a false
precondition alone, which is exactly the vacuity a report must mark
(`realizable_of_uninhabited`); a contradictory postcondition under an inhabited
domain refutes realizability, with the inhabitation witness required so the
refutation is not itself vacuous
(`unrealizable_of_contradictory_post`, `contradictory_inhabited`,
`contradictory_not_realizable`); two distinct satisfying outputs for one
admissible input refute determinacy (`not_deterministic_of_two_outputs`), and
per-input uniqueness entails it (`deterministic_of_unique_output`); the exact
wrapping `I64` increment with a true precondition is inhabited, realizable and
deterministic (`increment_inhabited`, `increment_realizable`,
`increment_deterministic`); an underconstrained `Bool` output is realizable but
not deterministic (`underconstrainedBool_two_outputs`,
`underconstrainedBool_realizable`, `underconstrainedBool_not_deterministic`);
and determinacy does not imply existence — the empty-output relation is
deterministic vacuously and not realizable (`emptyOutput_deterministic`,
`emptyOutput_not_realizable`, `determinacy_does_not_imply_realizability`).
Fifteen inventoried theorems; receipt
[`verification/foundation/receipts/45d658591/`](../../verification/foundation/receipts/45d658591/foundation-check.json)
(301 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

### Reporting layer (issue #65)

`noble spec-quality CONTRACT MANIFEST_JSON` is the native source-bound reporting
interface. `CONTRACT` is read as a `.noble-contract` source and `MANIFEST_JSON`
is the separate v2 manifest. The manifest schema is
`noble-mc2-spec-quality/v2`; its allowed required keys are `schema`,
`source_sha256`, `predicate_revision` (`1`), `model_revision` (`1`),
`observation_relation` (`exact-scalar-equality`),
`parameter_binding` (`ordered-exact-i64-bool`), `domain`, `permitted` and
`obligations`. Source paths and manifest paths are not identities. The report
binds the exact source and manifest bytes with `source_sha256` and
`manifest_sha256`, and reports the prepared contract as
`contract = {name, candidate_format, semantic_revision}`.

The manifest carries only finite observations: every `domain` assignment has
ordered scalar `inputs` and `params` stacks, and `permitted` is an array of
ordered scalar output stacks. Scalars are exactly `{i64: exact-integer}` and
`{bool: boolean}`. Domain entries and the top-level manifest reject unknown
fields. A caller cannot supply `types`, `pre`, `post`, `relation`, or another
legacy relation object: declared interface types and the `requires`/`ensures`
roots come only from the native `Prepared` contract. Every report, including an
implementation-only report, supplies `source_sha256`, `domain` and `permitted`.

Source bytes are read once and hashed before native preparation. A digest mismatch
is `wrong-source-binding` (exit `2`). Native preparation uses the default bounded
`noble_contracts::Limits`: an invalid checked source is `invalid-contract` (exit
`2`), a frontend-unsupported source form is `unsupported-contract`
(exit `2`), and a limit failure is `exhausted` (exit `5`). Native inputs,
outputs and parameters must be scalar `I64`/`Bool`; a non-scalar or effectful
interface, a stack shape/kind mismatch, or a bound outside the finite profile is
`unsupported-relation` (exit `2`). Missing `domain` or `permitted` is
`invalid-manifest` (exit `2`). A manifest remains at most 4,096 bytes, a domain
has 1..64 assignments, and each output list has 1..16 stacks. Native source
remains at most 65,536 bytes; the byte limit applies before preparation.

The producer validates both native `requires` and `ensures` roots before
enumeration. The supported expression fragment is native `I64`, `Bool`, input,
output, parameter and transparent definition references, together with `not`,
`and`, `or`, `eq`, `lt` and wrapping `add`; other native expression forms are
`unsupported-relation`, including when unreachable under a Boolean operand.
Reachable expression depth is bounded at eight levels (root depth zero), and
Boolean operands are evaluated eagerly. Native frontend typing and scope errors
therefore remain `invalid-contract`; an undefined evaluator result is never
silently treated as false.

### The propositions are derived, not attested (issue #65)

`crates/noble-cli/src/core/spec_quality/relation.rs` consumes the prepared native
contract and the finite manifest stacks. It enumerates the declared domain and
derives inhabitation, relational realizability and output determinacy from the
native predicate roots; submitted `checked`/`vacuous` flags are compared with
those derivations rather than trusted. A disagreement is
`self-attested-obligation`. Refutations retain their measured finite witnesses:
the admissible assignment with no satisfying permitted output, or the assignment
and original permitted-list indices of two distinct satisfying output stacks.
Exact whole-stack equality preserves duplicate-first, reordered-index,
postcondition-filtering, mixed-stack and output/domain-boundary behavior. Exact
wrapping `I64` arithmetic keeps the signed boundary finite and decidable.

The report still separates `specification_quality` (specification-quality rows
and established/not-established/vacuous tallies), `partial_correctness` (the
implementation-satisfaction status), `termination` (never claimed), and
`correspondence` (never claimed). Implementation status is reported only; it is
not independently proved by this lane. The report includes the finite derivation
and refutation indices, `guest_requests: 0`, and the existing metadata. The
selected-CLI regression lane retains controls for obligation separation, vacuity,
duplicate and reordered outputs, exact I64 boundaries, output/domain limits and
ordinary `verify` typing; it also controls changed native predicate/body/interface
name/parameters, rebound predicates, native definition references, parameter and
type binding, legacy manifest injection, source/manifest byte preservation and
same-source bytes at different paths. These are finite production-CLI controls,
not a Lean↔Rust correspondence theorem.

The selected Rust wrapper passes against the source-built CLI. Four separate
native CLI probes exercised matching source, stale source, a correctly rebound
false postcondition with the old realizability flag, and its accepted finite
refutation. Their full syscall traces observed only the initial CLI executable,
no child creation, no writable opens or writable shared mappings, and writes
only to report stdout. A deliberate file write and child executable calibrated
the trace detector. Both input files stayed byte-identical. This is finite
effect evidence combined with the scalar-only dispatch; tracing alone does not
rule out in-process execution or establish universal noninterference.

The [source-bound gate and CLI receipt](../../verification/main-gates/receipt-13b5a35.json)
records the cutover at `13b5a352`: 10 gates pass, two fail, tool-version
verification is interrupted and tool-overrides is unrun. The remaining global
gate work is tracked in [#72](https://github.com/OnixResearch/noble-lang/issues/72);
the finite native-contract integration does not waive those failures.

The ordinary `noble verify` controls remain separate from this report: a native
Bool contract with `requires true` / `ensures true` exercises ordinary admission,
while a wrong-typed implementation remains an ordinary contract error. A
specification-quality report does not create implementation evidence, and an
ordinary verify result does not create a specification-quality artifact.

The separate Lean model in `proofs/mc1/NobleContracts/SpecQuality.lean` remains
the retained model-level discipline: its `Submission` records the kind,
observation relation, parameter binding and assignment it claims, while
`Accepts` recomputes them from the model `Spec` and raw stacks. Its existing
receipt and five model-level refusal controls are historical model evidence;
they do not stand in for native source preparation or the finite CLI report.

The earlier syscall trace at `88a91af8` remains historical evidence for the
previous selected reporting lane; it is not reinterpreted as a v2 native source
or correspondence result. The finite native report boundary likewise makes no
claim about universal contract realizability, determinacy, termination, traps,
quotas, runtime success, compiler correctness, lowering, Wasm execution or
source/renderer correspondence. The Lean model and its receipt above remain
separate historical model evidence and are not overwritten by this protocol
description.

## Root-pinned live-dispatch model (issue #47)

`NobleFoundation/LiveDispatch.lean` models the live-reference design's
root-pinning claim with bounded roots, ordered entry maps, single-slot
global-epoch publications and an ordered guest trace whose control events
(denials, publication notices) stay separately identifiable.

Proved: one publication changes only its own slot's selection
(`publish_preserves_other_keys`, `publish_selects_the_published_generation`);
pinning stores the map and nothing replaces it in place (`pin_is_immutable`);
a root's selections come from its own pinned map
(`root_selections_use_pinned_map`) and equal a frozen-entry-map reference for the
same request sequence (`frozen_entry_map_reference_agrees`); two separate
single-slot publications are not one atomic replacement, so a root pinned
between them selects a mixed pair (`mixed_map_selects_old_and_new`); a denied
dispatch produces no guest event (`denied_dispatch_produces_no_guest_event`,
`authorized_dispatch_selects_the_pinned_generation`); and a finite abnormal
prefix keeps its committed events (`abnormal_prefix_preserves_committed_events`).
Nine theorems, all inventoried; receipt
[`verification/foundation/receipts/d2aa7a382/`](../../verification/foundation/receipts/d2aa7a382/foundation-check.json)
(261 theorems, `clean_source: true`, axiom sets inside the allow-list).

Those nine hold by construction. `trace` reads only the pinned map, so
`root_selections_use_pinned_map`, `frozen_entry_map_reference_agrees` and the denial and prefix
theorems are `rfl`, and `mixed_map_selects_old_and_new` checks one selection. **Event relocation**
(VC-LIVE-03, model half) gives the model something a pin could get wrong.

**The run.** `run` executes programs that emit a distinguished event and dispatch to other
slots. Every dispatch selects from a map *view* chosen per dispatch and asks a per-dispatch
policy:
- `pinnedRequest` reads the root's pinned map at every dispatch;
- `currentMapRequest`, the deliberately broken control, reads the registry as published at each
  dispatch.

**The fixtures.** The relocation fixture moves the event from caller to callee between versions:
`A-old` emits and calls `B`, `A-new` only calls `B`, `B-old` is inert, `B-new` emits.
`reverseRelocation` moves it back.

Proved:
- coherent all-old and all-new pins emit the event exactly once, for both fixtures
  (`coherent_pins_emit_exactly_once`);
- with `B-new` published between the caller's and the callee's dispatch, current-map selection
  **duplicates** the event under the relocation and **omits** it under the reverse, while a root
  pinned to the old map before the publication emits it once
  (`current_map_selection_duplicates_and_omits`);
- a root pinned between two single-slot publications still selects from one map, but that map
  is a mixed pair, so it omits or duplicates the event
  (`a_root_pinned_between_sequential_publications_can_mix`). This is the limit of the coherence
  premise, not a root-snapshot bug;
- for **every** policy, program table, view and stack, a run's events are a prefix of the fully
  authorised run's events from the same state (`denials_only_truncate`, with
  `run_extends_the_events`). A denial stops execution; it neither rolls back nor invents an event;
- concretely, a denial before the emitting hop emits nothing and keeps the committed dispatch
  (`a_denial_before_the_emitting_hop_keeps_the_prefix`).

Six further inventoried theorems; receipt
[verification/foundation/receipts/1d4113e6c/](../../verification/foundation/receipts/1d4113e6c/foundation-check.json)
(563 theorems, `clean_source: true`; the LiveDispatch theorems use `propext` at most).

Open, and deliberately not closed:

- root-snapshot consistency is proved; *coordinated-version coherence* — that a
  pinned map holds a behaviorally compatible set for one refactoring — is a
  separate premise and is not proved here;
- the broken per-dispatch-current-map control exists **in the model** only. On the implementation
  side it is not constructible from the runtime's authority shape, so the implementation
  fixtures measure omission alone. The retention step of
  `crates/noble-cli/tests/live_slot_coherent_maps.mjs` used to republish without the CAS fields
  and was refused for that reason, whether or not a root was pinned. It now holds a root before
  its selection, lands a compare-and-set publication, and checks that the held root dispatches
  and emits once from its pinned version
  ([verification/live-slot-coherence/](../../verification/live-slot-coherence/README.md));
- no correspondence claim relates the model to
  `crates/noble-cli/src/core/runtime/slot-registry.mjs`;
- no event-count claim applies when a denied dispatch or effect stopped
  execution before the event, and no rollback is implied by a committed prefix;
- atomic multi-slot replacement, state/resource migration, new
  contextual-effect inference, liveness and exactly-once effects are outside it.

## Bounded canonical ServiceRef codec (issue #48)

`NobleFoundation/ServiceWire.lean` specifies the one admitted Preserves text
record of the dataspace codec, `<service "NAME" #t>` / `<service "NAME" #f>`,
with the 64-byte wire budget, the `1..=4` configured depth and the 1..=32-byte
name alphabet. Bytes are modelled as `List Nat`; the parser matches the record
shape by literal byte patterns and requires the closing suffix to consume the
rest of the input, so full-input consumption is structural; canonicality is a
checked condition, because `decode` re-encodes the parsed value and accepts only
when the encoding equals the input.

Proved: value round trip (`decode_encode`) and accepted-byte canonicality
(`encode_of_accepted`); the exact name and wire bounds
(`longest_name_fits_wire_budget`, `overlong_name_is_rejected`); and the decision
precedence — byte budget before the configured depth
(`byte_limit_precedes_depth_scan`), the configured depth before the schema match
(`depth_configuration_precedes_schema`) and the depth scan before it
(`depth_scan_precedes_schema`) — with the readiness tag, trailing bytes, the
empty name and foreign name bytes separated as semantic classes rather than as
diagnostic wording.

**Correction (2026-10-29).** `altered_readiness_is_schema`, `empty_name_is_schema` and
`foreign_name_byte_is_schema` used inputs that omitted the space byte of the record
prefix, so each was refused at the prefix and established nothing about the case it
names. They are now built from `recordPrefix` and `recordSuffix`.
`schema_controls_have_accepted_neighbours` shows that each control has an accepted
neighbour that differs only in the named part. The decoder's own bounds are now stated
too:
- a record with a 33-byte name fits the wire budget and is refused by the name bound
  (`overlong_name_decodes_to_schema`);
- 64 bytes pass the byte limit and 65 do not (`the_full_wire_budget_is_not_a_byte_limit`);
- every accepted name satisfies the name predicate (`accepted_name_is_valid`).

The depth scan is **balanced**, like the codec's: `depthScanAux` tracks the
current depth and the largest depth reached, and a closing byte at depth zero
leaves the current depth at zero, so stray openers and separated runs are schema
failures rather than depth failures
(`unbalanced_closers_do_not_count_toward_the_depth`). The earlier scan counted
opening brackets, which was stricter than the codec; the finite comparison below
surfaced that disagreement, and the model was corrected rather than the finding
being recorded and dropped.

The independent comparison is `verification/service-ref/cases.mjs` — a decoder
written from this model rather than from the Rust codec — against the real
`crates/noble-kernel/src/dataspace/wire.rs` functions in the
`service_ref_differential` lanes of `crates/noble-kernel/tests/m7-dataspace.rs`:
271 cases over alphabet edges, name lengths 0/1/32/33, wire length 65 (64 only
through two generated inputs),
configured depths 0/1/4/5, truncation at every offset, appended bytes, one-byte
substitutions and seeded generated values, with three `control-` cases that a
counting-only depth scan would decide differently. The lane agrees on every case.
Nine seeded decoder faults run through the same cases in
`service_ref_corpus_detects_each_declared_decoder_mutation`: a counting-only depth
scan, a depth limit one level too strict and one too lenient, a byte budget one
byte too lenient, ignored trailing bytes, a foreign tag read as not ready, ignored
readiness, an admitted empty name and an admitted 33-byte name. Each is the decoder
with one rule changed, simulated from its own decisions, and each disagrees with at
least one expectation; four of them with exactly one case.

Twenty-eight inventoried theorems after the correction, receipt
[verification/foundation/receipts/9f1288d61/](../../verification/foundation/receipts/9f1288d61/foundation-check.json)
(557 theorems, `clean_source: true`); the earlier receipt
[verification/foundation/receipts/134b215c5/](../../verification/foundation/receipts/134b215c5/foundation-check.json)
(454 theorems, `clean_source: true`) predates it.

Open, and deliberately not closed: an Aeneas extraction of `decode_service` and
`encode_service` exists in `proofs/m3/NobleKernel/Funs.lean`, but no refinement
proof relates it to the model, so the differential is finite evidence on one host
and target rather than a correspondence theorem. No whitespace insertion or
deletion family exists, and the seeded faults are not an exhaustive fault model. The canonicality theorems apply to this closed schema
only, not to JSON documents in general, and no zero-copy, whole-parser or
whole-compiler claim follows.

## Bounded independent-row batch reference (issue #41)

`NobleFoundation/BatchRows.lean` is the *model* end of issue #41's batch entry
point, not an implementation. A row is one wrapping I64 operation over two
explicit operands; `rowOps` builds the row's straight-line fragment, `rowRun` is
one independent single-row run over the existing reference machine (`AOp.ref`,
whose only arithmetic is the comparator oracle's `MathlibI64.wrap`), and
`evalBatch` is the ordered map of those independent runs. It is deliberately not
the optimized Rust loop under another name: it is the same independent machine
the single-row theorems use.

Proved, all over finite input lists:

- the batch **is** the ordered map of independent row semantics
  (`evalBatch_is_the_ordered_map`), and appending batches appends results, so
  partial progress is the ordered map over the prefix (`evalBatch_append`);
- a refused row stays local and cannot change any other row's result
  (`a_refused_row_does_not_affect_later_rows`), and each row's run starts from an
  empty stack, so rows cannot supply or consume each other's operands
  (`a_rows_run_starts_from_an_empty_stack`);
- a validated row's reference result is the wrapping operation on its operands
  (`add_row_result_is_the_wrapping_sum`,
  `sub_row_result_is_the_wrapping_difference`,
  `mul_row_result_is_the_wrapping_product`), and an out-of-range literal row is
  refused at its own position rather than silently coerced
  (`out_of_range_row_is_refused`);
- each batch row's lowered fragment executes exactly as that row's reference run
  (`batch_row_executes_as_its_reference_run`), by reusing the sequence-lifting
  theorem `exec_lowerAll` rather than restating it.

Nine inventoried theorems; receipt
[verification/foundation/receipts/781c2f490/](../../verification/foundation/receipts/781c2f490/foundation-check.json)
(452 theorems, `clean_source: true`).

**Declared cost of staging.** Beside the value theorem, `Cost`, `rowMajorCost` and
`chunkedCost` declare node evaluations and staging/output writes separately, and
`chunksOf`/`chunksOfGo` are the chunked staging of the same rows. Proved:
row-major staging costs exactly one evaluation and one write per row
(`row_major_cost_is_exact`); a chunked pass partitions without reordering or
dropping rows (`chunksOfGo_flatten`) and therefore evaluates exactly the
ordered map of independent row runs (`chunking_does_not_change_the_value`), so
chunking is a staging choice and not a value change; node work is
staging-independent (`node_work_is_staging_independent`); the chunked writes
are exactly the chunk count (`chunked_writes_are_one_per_chunk`); every
committed chunk holds at most one chunk of rows
(`every_chunk_is_bounded_by_the_chunk_size`) and a pass therefore bounds the
live row slots by the chunk size (`live_row_slots_are_bounded_by_the_chunk`);
and a zero chunk size degenerates to one row per chunk instead of dropping rows
(`a_zero_chunk_drops_no_row`).

Nothing in that model measures or predicts elapsed time, process memory or
allocator overhead, and no abstract counter is equated with a measured one.

Receipt for the cost model:
[verification/foundation/receipts/f91cc50f7/](../../verification/foundation/receipts/f91cc50f7/foundation-check.json)
(471 theorems, `clean_source: true`, axiom sets inside the allow-list).

The graph-backed Rust batch, `batch::Prepared` in `crates/noble-ngraph/src/batch.rs`,
runs one checked I64 graph over ordered rows and is compared row by row with
independent single-row runs in `crates/noble-ngraph/tests/batch_rows/`. That is
tested-implementation evidence; no theorem here relates it to `evalBatch`.

Open, and deliberately not closed by this model: no byte budget beyond the
literal-range rule, no validation rule beyond the literal range is
modelled, no parameter snapshot or output ownership is modelled, and no
implementation correspondence is claimed — a matching Rust batch loop would be
tested-implementation evidence, not a theorem. Performance is untouched: nothing
here measures or predicts throughput, first-result latency or allocation.

## Bounded M6 task-interleaving model (issue #46)

`NobleFoundation/TaskInterleaving.lean` is a two-record subject for a
schedule explorer rather than a scheduler: it names which interleavings are
reachable and what holds at every reachable prefix, so a full-search baseline
exists before any reduction is claimed. Records carry a generation, a pin,
queued wakeup units, a lifetime wake budget and a retiring mark; the table
carries a shared admission capacity, a sticky first abnormal failure, and
ordered delivery and settlement records. Actions are `acquire`, `cancel`,
`complete`, `deliver`, `staleCallback`, `settle`, `cleanup`, `finish`, `wake`
and `takeWake`; native stop is not among them because it is an input
observation, not something cancellation proves. Every refusal is a single
guard, an action naming a generation must name the live one, and same-slot
reuse needs a different generation. (The wake budget, the retiring mark and the
two wake actions were added with the reduced search below; earlier receipts
cover the model without them.)

Proved, and the enumeration baseline with it: the empty table satisfies the
invariants (`empty_table_satisfies_the_invariants`), every acquired step
preserves them (`step_preserves_invariants`) and therefore every reachable
prefix of any action list does (`reachable_prefixes_satisfy_the_invariants`);
a full table refuses the other id's admission
(`baseline_admission_is_refused`) until finishing a live record releases the
slot (`finishing_a_slot_releases_the_capacity`), after which that other id's
admission is enabled (`releasing_a_slot_enables_another_id`); a stale-generation
callback leaves the table untouched (`stale_generation_callback_changes_nothing`);
delivery after retirement (`late_delivery_after_retirement_is_refused`), a
second delivery of the same generation (`double_delivery_is_refused`) and
settlement before delivery (`settlement_requires_a_delivery`) are refused
rather than recorded; settlement after delivery appends the ordered pair
(`settlement_after_delivery_is_taken`), finishing keeps the delivery record
(`finish_keeps_its_delivery_record`) and cancellation keeps the pin held
(`cancellation_keeps_the_pin`); a recorded abnormal failure survives later
failures, later successes and refused or acquired actions
(`abnormal_failure_survives_a_second_failure`,
`abnormal_failure_survives_a_later_success`,
`abnormal_failure_survives_refused_and_acquired_actions`) and two failures keep
the first code (`first_failure_code_is_kept_as_the_sticky_one`). Twenty-three
theorems, all inventoried, plus the three guard-reading lemmas
(`some_or_none_eq_some`, `none_or_some_eq_some`, `some_or_some_eq_some`).
Receipt
[verification/foundation/receipts/cc147dd40/](../../verification/foundation/receipts/cc147dd40/foundation-check.json)
(428 theorems, `clean_source: true`, axiom sets inside the allow-list).

The observation contract is `observe` — per-id generations, pins and
reservations, the sticky failure, and the ordered delivery and settlement
records. `lossyObserve` deliberately drops the failure and the generations, and
is measured as lossy instead of being used as an equivalence test:
`lossy_observation_hides_the_failure_difference` and
`ordered_delivery_prefixes_are_distinguished` present histories with the same
lossy observation but different documented ones, the second pair differing only
in delivery order.

The deliberately unsound independence rule is refuted, not assumed:
`different_ids_are_not_independent` shows two actions on *different* task ids do
not commute, because the second one's enabledness depends on the first one's
released slot (acquire-then-finish refuses the admission, finish-then-acquire takes
it). Cancellation/delivery races, generation reuse and shared-capacity
conflicts therefore stay dependent.

The independence rules are then **proved or refuted separately at the declared
bound**, so a reduction is only ever proposed for a swap the model supports:

- slot-local actions on the distinct ids **commute**: `cancel_orders_agree_on_distinct_ids`
  and `finish_orders_agree_on_distinct_ids` reach the same documented
  observation in either order, so the swap preserves enabledness, the related
  state and the observation for those actions;
- delivery is **not** independent: `delivery_orders_disagree_on_distinct_ids`
  differs because the documented observation keeps one ordered delivery record,
  while `delivery_orders_agree_under_the_lossy_observation` shows the same two
  orders agreeing under the lossy projection — so an equivalence read from that
  projection would hide a dependence the documented observation retains;
- the shared slot, not the id, decides enabledness:
  `admission_depends_on_which_id_holds_the_shared_slot` reaches two different
  documented observations from one released slot, and
  `composed_admissions_agree_while_the_prefixes_differ` records that both
  *composed* admission orders end refused, so an equality read from the
  composition would hide the reachable-prefix dependence rather than prove
  independence;
- the sticky first failure is shared: `failure_orders_disagree_on_distinct_ids`
  differs across the two failing-completion orders on the two-generation
  `pairedGenerations` table;
- `stale_callback_is_observation_free` states the one action that is
  observation-free at the declared bound.

Two first drafts of the admission and failure witnesses were *refuted* by the
kernel checker because both of their first steps were refused; the statements
were corrected to reachable prefixes and two live generations rather than
weakened into tautologies. Eight further inventoried theorems, receipt
[verification/foundation/receipts/729e5d199/](../../verification/foundation/receipts/729e5d199/foundation-check.json)
(443 theorems, `clean_source: true`).

**The bounded explorer.** `exploreAlphabet`, `sequencesOfLength` and `sequencesUpTo`
populate a baseline observation set from the declared alphabet up to depth three
(`baselineObservations`, `declaredBaseline`), refused steps contributing no
observation rather than a silent state. The enumeration is proved complete for
that bound: it contains every sequence of the declared length over the declared
alphabet (`mem_sequencesOfLength`), a deeper enumeration contains every shorter
one (`sequencesUpTo_contains_shorter`), and therefore every sequence within the
declared depth appears in it (`the_baseline_enumeration_is_complete`) — so a
reduction may not claim agreement against a smaller search than this one.

Full/reduced agreement is then *measured* on the declared pairs: the independent
pair's swap preserves the documented observation in both orders
(`the_independent_pairs_swap_preserves_the_observation`), while the delivery pair
changes it (`the_reduction_must_exclude_the_delivery_pair`), so a reduction that
kept the delivery swap would be unsound rather than merely unproven.

Receipt for the explorer and the witness scan:
[verification/foundation/receipts/e95adf748/](../../verification/foundation/receipts/e95adf748/foundation-check.json)
(479 theorems, `clean_source: true`, axiom sets inside the allow-list).

**Reproducible witness shrinking.** `disagrees` states when a sequence's reachable
observation differs from a reference (or when it is refused where the reference
is reachable), and `scanFrom` walks prefix lengths *upward*, so the first
disagreeing length it finds is the shortest one. Proved: when the scan reports
no disagreement every prefix in its window agrees (`scanFrom_none_all_agree`),
when it reports one that length is the first in its window and every shorter
prefix agrees (`scanFrom_some_finds_the_first`), and therefore a disagreeing
sequence has a shortest prefix witness with every shorter prefix recorded as
agreeing (`every_disagreement_has_a_shortest_witness`) — so a shrunk witness is
replayed from the sequence's own prefix rather than hand-picked.

**Wakeups and the retiring mark.** An acquired record starts with a lifetime
wake budget of one (`wakeBudget`). `wake` queues one wakeup from the budget,
coalesces into a wakeup already queued, or, with the budget spent, records the
sticky failure and marks the record retiring; `takeWake` removes the queued
wakeup. Cancellation now also marks the record retiring, and a retiring record
refuses delivery, so cancelling before and after delivery are different
histories. Both wake actions are refused for a retiring or delivered record, as
the Rust schema refuses `Wake` and `TakeWake` in the `Retiring` and `Delivered`
states. The observation keeps both new fields per id. Every earlier statement is
unchanged and re-checked against the extended model, and the invariant
preservation proof covers the two new actions.

**Proved reduction.** The reduced search is enabled by a proof rather than by
finding fewer traces:

- `independent` relates actions on *different* ids that do not conflict on
  shared state: neither reads the capacity the other moves (admission against
  admission, finish or cleanup), they do not both write the sticky failure
  (failing completions and wakes), and they do not both append to one ordered
  record. Every pair on one id stays dependent, so cancellation/delivery races,
  generation reuse and a stale callback against its own record are never
  reordered.
- `independent_actions_commute` proves that every independent pair reaches the
  same table in either order **on every table**, both orders refused included
  (`independent_is_sound`). It is proved for the left-first order
  (`left_then_right_commute`) and transferred through `independent_symm`.
- `canonicalize` moves actions past related neighbours by adjacent swaps only,
  so the representative is a permutation (`canonicalize_perm`), is canonical
  (`canonicalize_canonical`: no adjacent related pair has the right id first),
  and reaches exactly what the sequence reaches from every start
  (`reaches_canonicalize`).
- `reducedUpTo` enumerates only canonical sequences, pruning while it extends
  rather than filtering a full enumeration (`mem_reducedUpTo`; the full search
  is characterised the same way by `mem_sequencesUpTo`).
- `every_sequence_has_a_reduced_representative` and `reduced_search_agrees`:
  for **every** sound relation, start table, alphabet and depth, every
  full-search history has a replayable representative in the reduced search,
  and the two searches reach the same set of documented observations.
  `the_declared_reduction_agrees` instantiates it at the declared bound.

The declared bound is fifteen actions (`exploreAlphabet`) to depth three from
two starts (`declaredStarts`): `baseline`, one live record against one shared
unit, and `paired`, both records live against two units. Both are empty tables
after admissions (`the_declared_starts_are_reachable`). Kernel-checked
deterministic counts (`decide +kernel`; the kernel, not native evaluation, is
the authority):

| Start | Sequences full / reduced | Reachable histories full / reduced | Distinct observations full / reduced |
|---|---|---|---|
| `baseline` | 3616 / 2717 | 244 / 243 | 44 / 44 |
| `paired` | 3616 / 2717 | 500 / 338 | 87 / 87 |

(`the_declared_sequence_counts`, `the_declared_history_counts`,
`the_declared_observation_counts`.) From `baseline` the right id is rarely
enabled, so the reduction saves almost no reachable history; from `paired` it
saves about a third.

The deliberately unsound rule "different task ids are independent" is the
control. It is refuted as a commuting relation (`different_ids_rule_is_unsound`),
and the reduced search it would enable loses the right-first delivery order
(`the_unsound_reduction_loses_an_observation`). At the declared bound it loses
observations from `paired` but keeps every observation from `baseline`
(`the_unsound_reduction_is_detected_only_where_ids_interleave`): agreement
measured at one start would have accepted it, which is why only the commutation
proof enables a reduction.

Coverage of the behaviours the baseline must include is stated per behaviour.
Each history in `coverageWitnesses` starts at a declared start and is in the
declared full search (`the_coverage_witnesses_are_in_the_full_search`), so its
canonical representative is in the reduced search:

- contention for the shared unit (`contention_is_covered`);
- cancellation against delivery in both orders (`cancel_versus_deliver_is_covered`);
- duplicate and late callbacks (`late_and_duplicate_callbacks_are_covered`);
- a stale handle after same-slot reuse (`stale_handles_are_covered`);
- wakeup coalescing and exhaustion (`wakeup_exhaustion_is_covered`);
- settlement and cleanup ordering (`settlement_and_cleanup_ordering_is_covered`);
- once-only finish (`once_only_finish_is_covered`);
- concurrent distinct-slot histories (`concurrent_distinct_slot_histories_are_covered`).

Thirty-six further inventoried theorems. Receipt
[verification/foundation/receipts/4e9ca2acb/](../../verification/foundation/receipts/4e9ca2acb/foundation-check.json)
(520 theorems, `clean_source: true`; the new theorems depend on `propext` and
`Quot.sound` at most).

**Valid client histories (the lane the Relinche comment on #46 asks for).**
`reaches` ends a history at its first refused step; a client keeps going. Under
the client semantics (`advance`, `accepts`, `respond`, `afterHistory`) the table
answers each call accepted or rejected, and a rejected call leaves it unchanged.
On a history with no rejected call this agrees with `reaches`
(`afterHistory_of_reaches`).

- **Valid-client predicate.** `ValidClient` holds when every call names only
  what the client holds: the handles the start table issued and the handles
  its own accepted admissions returned. A stale handle from an earlier
  generation stays held, so its call is valid and may be rejected. A forged
  handle, and one a rejected admission would have named, are not
  (`valid_client_examples`). Every coverage witness is a valid client history
  (`the_coverage_witnesses_are_valid_clients`).
- **Documented projection.** `projectClient` keeps every id's calls paired
  with their responses, in order: handle identity and refusal outcomes. It
  also keeps the documented observation of the final table: failure, pin,
  reservation, wake, cleanup and the ordered records. The order between calls
  on different ids is observed only through shared state.
- **Positive histories.** Some pairs end at the same table but differ in a
  rejected call: a delivery after cancellation, an old-generation delivery
  after reuse, a settlement after cleanup. `projectClient` distinguishes each
  pair, and it distinguishes the two cancel/deliver orders
  (`the_client_projection_discriminates`).
- **Negative histories.** `lossyClient` drops rejected calls and identifies
  each of those pairs (`the_lossy_client_projection_hides_rejected_calls`).
- **Reduction.** An independent call, accepted or rejected, never changes
  whether the other is accepted (`independent_keeps_acceptance`), and the two
  orders reach the same table (`independent_advance_commute`;
  `independent_is_client_sound`). Canonical representatives therefore keep
  validity and the projection (`client_canonicalize`). For every client-sound
  relation, start, alphabet and depth, the full and reduced searches reach the
  same client projections of valid client histories
  (`reduced_client_search_agrees`, `the_declared_client_reduction_agrees`).
- **Counts and control at the declared bound**, kernel-checked: valid client
  histories are 2384 full and 2168 reduced from `baseline`, and 3616 full and
  2717 reduced from `paired` (`the_declared_client_history_counts`). The
  unsound rule loses client projections from both starts
  (`the_unsound_client_reduction_loses_projections`). From `paired` it loses
  the right-first delivery order. From `baseline` it loses a rejected
  admission followed by the finish that would have enabled it, which the
  proved relation keeps.

This is a sequential explorer, not a weak-memory or linearizability result. No
concurrent wrapper is selected, so weak-memory controls are not applicable
rather than passed. No source/runtime event correspondence is claimed; that
belongs to the #50 trace relation. Thirty-one further inventoried theorems.
Receipt
[verification/foundation/receipts/477d24b9e/](../../verification/foundation/receipts/477d24b9e/foundation-check.json)
(551 theorems, `clean_source: true`; the new theorems depend on `propext` and
`Quot.sound` at most).

Open, and deliberately not closed:

- the scan is bounded by one declared sequence and the declared alphabet rather
  than exhaustive over the language, and it shrinks prefixes only (no action
  deletion or operand rewriting);
- this is a bounded subject, not a scheduler and not a new source-level async
  model, and no payload/outcome class outside the listed actions is enumerated;
- no correspondence claim relates the model to the Rust table API in
  `crates/noble-kernel/src/async_tasks/table`; the invariants and the reduction
  are model claims, and witness histories are not replayed through the Rust
  table API (the mutation controls in `crates/noble-kernel/tests/m6-interleaving.rs`
  are rule-level);
- the independence relation is sound but not complete, canonical forms are not
  unique, and no optimality is claimed for the reduced search;
- the counts and the coverage witnesses are for the declared bound only; the
  wake budget is a model constant of one, and the failure code records which
  generation failed first, not the failure kind;
- the number of *distinct* client projections is not kernel-checked: with
  thousands of projections, quadratic deduplication is too expensive for the
  kernel. Agreement is the universal theorem, and the counts checked are of
  histories and of named projections;
- weak-memory ordering, liveness, fairness, callback-delivery guarantees,
  cleanup after host death and once-only remote effects are outside the model;
- `abnormal_failure_*` are concrete reachable witnesses rather than a universal
  sticky-failure theorem.

## Bounded ISOP Boolean core (issue #35, phase 1)

[`NobleFoundation/Isop.lean`](NobleFoundation/Isop.lean) is a standalone
truth-table model. `Valuation n` is the finite row type for `n` Boolean
variables, `BFun n` is a function on those rows, `Formula` supplies
`const`/`var`/`and`/`or`/`not` syntax, and `Cube`/`Cover` supply optional
literals and their disjunction. The exact mode has `L = U = F`: `isop` splits
on the fixed head-first variable order, recursively covers both cofactors and
attaches the complementary literals. `isop_exact` proves the generated cover
has the input function, and `isop_bounds` proves the support interval
`L ≤ cover ≤ U` pointwise whenever `L ≤ F ≤ U`.

`equivOn` is an independent exhaustive checker over `valuations`; its
soundness theorem does not call `isop` or any recurrence. The checked fixtures
include true and false constants, `x ∧ ¬x`, a repeated atom, three-variable
parity (including its four-cube growth and fixture-only irredundancy), and two
orders whose parity covers differ as lists but are checker-equivalent. The
wrong-cofactor and wrong-complement seeded recurrences are rejected by that
checker on the repeated-atom fixture. These irredundancy results are fixture
computations only; no universal irredundancy or minimization theorem is stated.

The module is deliberately a Boolean core, not source integration. A checked
Boolean DAG representation, lowering to `Program`, branching extensions, care
sets and adoption criteria are open. No theorem here claims correspondence to
the Rust frontend, compiler, runtime or any source-level optimizer. The clean-source receipt for the ISOP core is
[`verification/foundation/receipts/42350bf01/`](../../verification/foundation/receipts/42350bf01/foundation-check.json)
(674 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).


## Restricted SGD reference (issue #58, phase 2)

[`NobleFoundation/SgdReference.lean`](NobleFoundation/SgdReference.lean) is the kernel-checked
reference for the update published in `.cairn/changes/optimizer-kernel-experiment/design.md`. For
stored lanes in order `p_i' = Binary32.sub p_i (Binary32.mul eta g_i)`: the product rounds once and
the difference rounds once, through the #38 model, with no fused multiply-add. `checkStep` applies
the pilot rule. `n` must be `1..=64` (`malformed` at `$.n`, `exhausted` at `$.outputs`), the
gradient vector must have `n` lanes (`unsupported` at the gradient shape), every input must be finite
(`unsupported` at `$.inputs.p[i]`, `$.inputs.g[i]` or `$.inputs.eta`), and so must every output lane
(`unsupported` at `$.outputs.parameters[i]`). Signed zeros and subnormals are finite and admissible.

Universal theorems: `update_preserves_shape` and `update_lane` (lane `i` depends only on `p_i`,
`g_i` and `eta`); `checkStep_accepted` (an accepted step outputs exactly the reference update with
only finite inputs and outputs); `nonfinite_input_is_never_accepted`;
`nonfinite_parameter_is_refused_at_its_lane` with `firstNonfinite_reports_a_nonfinite_lane` (a
reported lane really holds a nonfinite value). Kernel-checked expectations cover a normal step, signed
zeros, a subnormal result, a ties-to-even boundary, an overflowing lane refused at its output, NaN and
infinite inputs refused at their paths, the shape and budget refusals, and `fused_update_is_detected`:
on one lane the reference differs from `Mutations.contractedSum`, which rounds `p - eta·g` once.

The published table fixes each refusal's class and location but not the precedence between classes.
This model checks `n`, the budget and the gradient shape before any value, and the inputs in envelope
order (`p`, `g`, `eta`) before the outputs; that order is the model's reading, and a checker that
reports a different first location for a multiply-refused envelope is not contradicted by it.

Open, and deliberately not closed: the candidate envelope checker and its controls, the empty
`State0` and `StepDescriptor` handling, immutable snapshots, rollback, evaluation isolation, the CPU
comparison, and any Rust correspondence.

The clean-source receipt for the SGD reference is
[`verification/foundation/receipts/620b43395/`](../../verification/foundation/receipts/620b43395/foundation-check.json)
(752 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

## First Futamura projection, restricted family (issue #34, phase 1)

[`NobleFoundation/FirstProjection.lean`](NobleFoundation/FirstProjection.lean) takes one deliberately
restricted source family: two-instruction programs over an accumulator that start at a dynamic `x`
and apply `add`, `sub` or `mul` with a second dynamic `y`. Each opcode is encoded one-hot as three
static I64 coefficients, and the interpreter is one 24-node `PackedSchedule` graph that computes
`c_add·(a + y) + c_sub·(a − y) + c_mul·(a·y)` per instruction with no branch. It is not a general
interpreter.

`interpreter_value` gives the interpreter's value for every input assignment, by the operand-value
lemmas `value_input`/`value_add`/`value_sub`/`value_mul`. `interpreter_agrees` proves agreement with
the independent reference `refEval` for all nine programs and every `x`, `y`. `first_projection`
instantiates #32's `specialize_sound`: specializing the interpreter to a program's six static
coefficients yields a residual over the dynamic inputs (`x` as dynamic slot 0, `y` as slot 1) that
computes the program, for every program and dynamic input.

Dispatch removal is bounded by the graph IR. There is no alias node, so an identity rewrite
(`1·v → v`, `v + 0 → v`) cannot be applied in place; the certificate format records identity steps
without applying them for the same reason. Zero annihilation (`0·v → 0`) can be applied in place:
`annihilate_sound` proves it preserves every node value on well-formed graphs without equality nodes,
and `residual_sound` shows the specialized, annihilated residual still computes the program.
`unselected_branches_are_dead` checks, for every program, that of each instruction's three operation
nodes exactly the selected one is live. `remaining_dispatch_overhead` states the cost that remains:
exactly one live multiplication per instruction still reads a former opcode slot, and that slot is the
constant `1`. Seeded controls detect a wrong opcode encoding and swapped subtraction operands.

Open, and deliberately not closed: identity forwarding (an alias node or operand forwarding), a
general source language and encoding, the second and third projections, specialization and residual
cost measurements, and any Rust or runtime correspondence; continuation thresholds are the owner's.

The clean-source receipt for the first-projection model is
[`verification/foundation/receipts/14a7793c9/`](../../verification/foundation/receipts/14a7793c9/foundation-check.json)
(775 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

## Status by phase

| Phase | Done (checked) | Open |
|---|---|---|
| 1 | whitelist/representation/observation table; signed ZMod/BitVec/K-NUM-01 correspondence; universal add/sub/mul with preserved typed prefix and orientation; literal refusal vs wrapping (spec and transcription); lifting to sequences and quote/compose/run; regressions; inventory and axiom audit | Rust frontend ↔ transcription refinement; MC1 `CodeTyped` ↔ kernel acceptance |
| 3 | one example from Lean spec to acquired bytes (opt off/on), helper model, ACORR-mapped refusals | E5 lowering, E7 assembler, E8 optimizer, E10 loader/engine, E12 PAA-1 envelope, ACORR-CASE-09, quote/compose/run helper models |
| 4 | seeded differential comparator; separate fixed-input scalar if/case EMI; Lean-checked expectations, opt off/on, shrinking and replay, mutation controls, checker lanes, source-bound receipts, `--verify`, retained in-repo receipts, `ci.sh` recipe | Nix-sandbox CI check (see "CI": networked Mathlib cache, absent offline inputs, host services for the slice); rebuilt Mathlib `.olean` files; Rust renderer/lowering refinement and universal compiler equivalence |
| 2 | MC1: preservation and primitive progress, pure laws and negative cases. Effects/resource layer: preservation, total progress, effect soundness for every prefix including denied/failed, ownership preservation with declared transitions, resource-free capture, resource negative cases | the whole language and implementation, abnormal cleanup, aggregates/borrowing, effectful equivalence laws, normalization |

### Effect and resource layer (`NobleFoundation/Effects.lean`)

This is a separate small-step model; MC1 is unchanged. It has:
- I64, move-only resource tokens, and programs with a latent effect bound
  `prog i o e`;
- the host words `emit`, `acquire` and `release`, which a total host oracle
  `H : Nat → Response` answers with `permitted`, `denied` or `failed`;
- an ordered request trace that records every request, including denied and
  failed ones;
- a continuation `code` that serves as the frame stack (`run` prepends the
  body);
- a host-tracked `live` set and a fresh-id counter.

A non-permitted response halts the configuration with status `denied` or
`failed` and leaves the stack, code and ownership unchanged. Cleanup after
an abnormal stop is **not** modelled.

Stacks are top-first in this file. Every claim is a model claim. It does not
describe the Rust kernel, `test.emit`, or the runtime's handle tables.

The CI recipe and its limits are in the Phase 4 section above. The retained
receipts are under
[`verification/foundation/receipts/`](../../verification/foundation/receipts/README.md).
Receipt [`verification/foundation/receipts/b3655a9cd/`](../../verification/foundation/receipts/b3655a9cd/foundation-check.json)
(435 theorems, `clean_source: true`).

## Checked interaction-net contract (issue #37, phase 1)

`NobleFoundation/InteractionNet.lean` is a versioned (contract revision `1`),
structural contract for the bounded checked I64 interaction-net backend.
`AgentKind` names the typed
input, constant, add, sub, mul, equality, dup/fanout, era/erase and output
agents. `Port` uses an explicit natural-number agent and slot; `validPort`
performs the declared arity check. A net's wiring is a finite association list
with both directions represented, and its boundary is an ordered list of output
principal ports.

Proved model claims:

- `version_ok_sound` shows that every accepted declaration selects the pinned
  contract revision;
- `reciprocity_ok_sound` turns the finite reciprocal obligation into a
  fixed-point-free involution with exactly one incidence per declared port;
- `compatibility_ok_sound` turns each accepted association into valid,
  principal/auxiliary-compatible and ordered-operand-compatible connected
  ports, including the source-order deeper/top distinction for binary agents;
- `boundary_ok_sound` and `validate_sound` preserve the exact declared output
  order and combine all three obligations into validator soundness;
- `deeper_minus_top_is_accepted` is the accepted compiled net for the
  structural `x - y` shape (input 0 is deeper and input 1 is top), while
  `swapped_sub_operands_are_rejected`, `dangling_wire_is_rejected`,
  `aliased_incidence_is_rejected` and `wrong_output_order_is_rejected` are
  kernel-checked refusal controls. `two_output_boundary_is_accepted` supplies
  the positive ordered-boundary fixture used by the last control.

The module is a contract and wiring validator only. Open, and deliberately not
closed: reduction rules, termination, confluence, readback correctness, the
graph-to-net translation proof, resource/byte/work budgets, and the owner's
continuation criteria. No Rust or runtime correspondence follows from these
theorems. The clean-source receipt for the net contract is
[`verification/foundation/receipts/e417bf841/`](../../verification/foundation/receipts/e417bf841/foundation-check.json)
(657 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).
## Static-input specialization model (issue #32, phase 1)

`NobleFoundation/Specialize.lean` defines a static map
`Nat → Option (BitVec 64)`, the dynamic-slot rank, and a merge operation. The
specializer keeps graph node indices fixed, replaces static input nodes by
constants, renumbers dynamic inputs by their original order, and folds only
add/sub/mul nodes whose two operands are statically known. Mixed nodes remain
residual nodes with their original operand positions.

The kernel-checked model claims are `staticValuesUpTo_eq_staticValue`,
`staticValue_sound`, `specialize_wellFormed`, `congruenceWithInputs`,
`residual_specialize_sound`, and `specialize_sound`. The fixtures cover no
static inputs, all static inputs, mixed static inputs, a non-prefix static
slot, deeper-minus-top subtraction order, and the `2^64 - 1 + 1` wrapping
boundary. `prefix_merge_control` is a negative control: treating static
slots as a prefix gives a different result on the non-prefix fixture.

| Claim | Model / theorem(s) | Implementation boundary | Status |
|---|---|---|---|
| Bounded static-input specialization over packed graphs preserves values under merge | `staticValue_sound`, `specialize_wellFormed`, `congruenceWithInputs`, `residual_specialize_sound`, `specialize_sound` | Rust certificate emission, effect-aware program semantics, and runtime correspondence remain open | **proved** |

The clean-source receipt for the specialization model is
[`verification/foundation/receipts/84753c99e/`](../../verification/foundation/receipts/84753c99e/foundation-check.json)
(692 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- pairs and `Unit` values, and `quote`/`compose`/`run`, are outside this graph
  model;
- preservation of an arbitrary stack tail is not represented;
- budgets, thresholds, refusal/adoption criteria and other owner-reserved
  contracts are not specified;
- effects, host calls, resource ownership and effect-aware partial execution
  are not modelled here;
- no correspondence claim relates this model to Rust, Wasm, the certificate
  emitter, or any retained runtime artifact.
## Collision-safe cache lookup and subject key encoding (issue #33, phase 1)

`NobleFoundation/CacheLookup.lean` is a generic model of two parts of the
cache design. Model A stores an index key, the stored manifest, content and a
verification bit. Its lookup accepts an entry only when the arbitrary index
function agrees, the stored manifest is exactly the request, and the entry is
verified. The collision fixture has two unequal manifests with one index key;
the stale-manifest and corrupt-entry controls both refuse, and
`lookup_after_erase_subset` states that removing entries cannot create a hit.

Model B mirrors `Subject::derive`'s fixed `noble.ngraph.subject.v0` domain tag,
then an eight-byte little-endian length and the bytes for every part. Under an
explicit shorter-than-`2^64` premise, `encodeParts_injective_of_lt` proves
injectivity. The changed-field fixture differs under this encoding, while
`naive_concat_field_collision` records the boundary collision that length
prefixes prevent.

This unit remains generic over the manifest type and therefore does not settle
issue #32's manifest contract. It is a model statement only: no runtime, Rust
or Wasm correspondence is claimed.

Open, and deliberately not closed:

- #32's manifest fields and their canonical representation;
- the persistence boundary and its failure/recovery behavior;
- the trust path for verification and artifact provenance;
- negative caching and its invalidation policy;
- correspondence between this model and the Rust cache and `Subject::derive`
  implementation.

The clean-source receipt for the cache-lookup model is
[`verification/foundation/receipts/d23d0c485/`](../../verification/foundation/receipts/d23d0c485/foundation-check.json)
(728 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).
