# #105 labelled DUP/SUP model gate

**Decision (2026-11-13): model-only GO; runtime HOLD.** The labelled DUP/SUP rules are a well-formed instance of the vendored `INet.System`; inets' strong-confluence theorem is instantiated for this system, and the finite fixtures and seeded semantic mutants are kernel checked. This result does not select a Rust/Wasm backend, change the default v3 path, or widen #37's source whitelist.

The prerequisite is the owner's 2026-11-13 rebase of the interaction-net issues on `ngraph-net-i64/3`, recorded in [`i64-net-contract-v3.md`](../../docs/i64-net-contract-v3.md). The runtime arm remains on hold until #37's `nets-rust` backend records its GO/NO-GO; that record is necessary but does not replace #105's own runtime gate.

## Frozen thresholds and result

These thresholds were committed before any performance or runtime measurement. The Lean model enforces the bounded structural/read-back rows; runtime resource rows are not claimed as implemented.

| Criterion | Frozen limit | Model result |
|---|---:|---|
| Declared natural labels | 16 per net, duplicate-free | Enforced by `validate`; at-limit and over-limit theorems |
| Agents | 64 | Enforced by `validate_sound` |
| Stored directed wire records | 192 | Enforced by `validate_sound` |
| Ordered boundary ports | 16 | Enforced by `validate_sound` |
| Rewrite steps | 256 | `observe` refuses larger requested fuel; at-limit and over-limit theorems |
| SUP nesting | depth 8 | Enforced by bounded read-back and `validateTree`; at-limit/over-limit theorems |
| Collapsed scalar values | 256 across the complete boundary | Enforced by `validateReadback_sound` |
| Host requests | 0 in the pure Lean model | No host interface is present; this is not runtime evidence |
| Runtime allocation / scratch | No claim | Not represented or measured in this model |

## Kernel-checked evidence

- `system` proves both `rule_wf` and `rule_symm`; `executableSystem` proves executable slot coverage.
- `labelledStrongConfluent` and `labelled_confluence` instantiate inets strong confluence; `labelled_normal_form_unique` proves uniqueness and equal lengths for reached normal forms. `labelled_reduction_bounded` and `validated_net_reduction_bound` instantiate the conditional bound.
- `validate_sound` establishes inets net well-formedness, a unique bounded declared label set, all structural caps, and declared-label membership for every `dupL`/`sup` agent. `validateTree_sound` and `validateReadback_sound` establish nesting, output-count, per-tree and aggregate collapse bounds.
- `equal_label_projection_fixture` proves equal-label branch projection; `different_label_nesting_fixture` proves the different-label SUP nesting; `op_sup_fixture_ordered_result` and `op_sup_fixture_ordered_collapse` prove OP1-SUP's ordered `[5,7]`; `op2_sup_fixture_ordered_result` and `op2_sup_fixture_ordered_collapse` prove OP2-SUP's ordered `[13,15]` with the other operand duplicated.
- `sup_free_rule_table_exact` proves the scalar rule table is the plain v3 table for SUP-free symbols; `plain_v3_fixture_matches_scalar_rule_table` normalizes the addition fixture to `9`.
- `wrong_label_equality_seed_caught`, `swapped_branches_seed_caught`, `missing_nest_seed_caught`, and `erased_sup_seed_caught` each prove that the corresponding seeded semantic fault differs from the expected model result. Fuel exhaustion and limit overrun refuse without a partial observation; `incomplete_boundary_readback_refuses_without_prefix` pins all-or-none readback when one normal-form boundary is not a value tree.

## Still open / runtime gate

No Rust reducer, source-to-net compiler, graph identity binding, Wasm/graph/packed differential, multi-schedule runtime run, runtime allocation/scratch accounting, cancellation path, or performance measurement was implemented in this slice. HVM3 `INTERS.md` remains prior art, not evidence. Any runtime GO requires the unchanged #105 acceptance criteria, exact v3 identities, and a separate reviewed GO/NO-GO after #37's backend decision. A #37 NO-GO leaves this runtime arm unavailable.

Re-open this model decision if the v3 rule contract or vendored inets pin changes, the labelled rule table/read-back relation changes, or a new runtime/translation claim is added.

## Runtime decision (2026-11-14): NO-GO

The runtime gate above made #37's backend decision a prerequisite and stated that a #37 NO-GO leaves this runtime arm unavailable. #37 has since recorded **NO-GO for continued runtime adoption** in [`net-decision.md`](../ngraph/net-decision.md): the opt-in revision-3 net passes its finite correctness gates but runs about 15x slower than packed execution against the frozen 1.15 speed threshold. There is therefore no adopted net runtime to extend with labelled DUP/SUP agents, and the #105 runtime arm is recorded as **NO-GO** under the unchanged criteria. No reducer, lowerer, differential or measurement was added for it, and none is claimed.

The model-only GO above is unchanged. Reopen the runtime arm only if #37 reopens under a new pre-registered workload or implementation hypothesis and records a runtime GO; the unchanged #105 acceptance criteria and exact v3 identities then apply, with a separate reviewed GO/NO-GO.

## Observation-contract change (2026-11-17, #135)

**Selected: option (a), per-root collapse only.** The model does not promise a
joint observation of several roots. The
[contract](../../docs/labelled-dup-sup-v1.md#selected-multi-root-contract-135-per-root-collapse-only)
now distinguishes labelled tree readback, label-erasing per-root marginals,
and the unsupported joint-compositionality and no-nested-label-preservation
claims. The different-label fixture's outputs carry label 5 and are no longer
described as independent. This corrects documentation, not the reduction rules
or any existing theorem.

Eight additional finite-example theorems are closed with kernel-checked
`decide` and inventoried. A focused pinned Lean 4.35 build of NetLabelled
completed without warnings; an external throwaway script printed each new
theorem's axioms (only propext and Quot.sound) and executed these observations:

| Net | Observed per-root scalar alternatives |
|---|---|
| differentLabelFixture | some [[3,7],[3,7]] |
| separateSupRootsFixture | some [[3,7],[3,7]] |
| corrAdd | some [[6,14]] |
| separateSupAddFixture | some [[6,14]] |
| nestedEqualAddFixture | some [[11,12,23]] |

The printed observations are execution smoke evidence; the corresponding
inventoried theorems are the kernel-checked finite evidence. Neither establishes
a universal joint semantics, a joint projection law, or preservation of an
absence of nested equal labels. Reopen those claims only under the explicit
predicates/semantics and proof or counterexample obligations in the contract.
Option (b)'s joint-collapse fixtures are not applicable to this selection.

Reproduce the focused proof check in the pinned environment with
`cd proofs/foundation && lake build NobleFoundation.NetLabelled`; print axioms
of the eight theorems named in the foundation README's #135 row from an external
Lean file importing NobleFoundation.NetLabelled. The foundation claim-table
audit also passed with no diagnostics. Source-bound receipt refresh and the
shared full-foundation audit remain integration checks, not evidence produced
by the focused smoke. No immutable receipt, frozen cap, rule, observation
implementation, #37 backend gate, or Rust/Wasm path was changed.
