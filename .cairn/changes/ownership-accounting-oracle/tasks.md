## Phase 1: Checker, judgment and accounting theorems

- [x] [serial] Select the declared fragment: data and must-use resource payloads, recursive wrapping, `dup`/`drop`/`swap`/`quote`, composition, a finite two-arm branch and the ordinary error-return alternative, with an executable checker and an independent declarative judgment. r[VC-OWN-01]
- [x] [serial] Prove checker soundness, the eligibility equivalence at every wrapping depth, that a resource operand blocks each data-constraining word, that swap moves either operand without an eligibility check, that dropping consumes only a duplicable operand, that duplication preserves both copies, that quoting retains, the opaque-segment conservation, the two-arm accounting rule and the empty-operand requirement for an error return. r[VC-OWN-01]
- [x] [serial] Inventory the initial eighteen theorems and retain the clean-source kernel check: the historical `verification/foundation/receipts/e7cc00993/` records 252 theorems, `clean_source: true` and axiom sets inside the allow-list. The swap correction adds two fixture theorems and requires a fresh receipt. r[VC-OWN-03]
- [x] [serial] Retain a clean-source foundation receipt binding `Ownership.lean`, `OwnershipTrace.lean` and `OwnershipCorpus.lean`, with all 116 corpus equations inventoried: `verification/foundation/receipts/2e932250/foundation-check.json` (source commit `2e932250`, 1,417 theorem checks). r[VC-OWN-03]
  Note (2026-10-29): the conservation follow-up adds the independently
  checked `OwnershipConservation` unit, proves whole-program filtered-resource
  conservation and arbitrary successful-configuration segment preservation,
  and retains a decided data-word/resource-tail fixture. The statement is
  exact but the earlier source had an incorrect swap eligibility guard; the
  Phase 3 correspondence task below remains owner-reserved and unchanged.

  Note (2026-10-29): the issue #59 swap correction removes that guard, retains
  the `swap_permutes_two` name with its stronger no-premise statement, adds the
  decided `swap_moves_a_resource` and
  `swap_moves_resource_conservation` fixtures, and renames the false
  `resource_operand_blocks_every_word` theorem to
  `resource_operand_blocks_non_swap_word`. The corpus now renders tagged
  resources and recursive wrapping, adds branch loss/doubling, wrong-identity
  and resource-move controls, and has 108 cases (15 accepted, 6 rejected and
  87 model-only). The swap mutation now drops an operand; the owner records
  fresh foundation and differential receipts after this one-commit change.

  Note (2026-10-30): `OwnershipJudgment` states one independent rule per word
  and proves `step` agrees with it exactly (`step_iff_wordRule`); the judgment
  on those rules coincides with `Accounts`, and `run_complete` makes the
  checker decide it exactly. The join admission simulation is lifted over
  whole straight-line word sequences (`joined_sequence_admission_agrees`).
  Branches and the error return stay outside the lift; the Phase 3
  correspondence task stays owner-reserved.

## Phase 2: Independent corpus, production comparison and mutations

- [x] [serial] Generate 116 corpus cases from the model over nine programmes and operand shapes plus branch/resource controls; state a reason for every model-only row and emit a Lean corpus whose `run` and `runTrace` equations are kernel-decided. r[VC-OWN-02]
- [x] [serial] Drive 28 accepted and 17 rejected cases through `noble_kernel::acceptance::check` one word prefix at a time, compare the production stack with every model trace entry and pin each rejection class. r[VC-OWN-02]
- [x] [serial] Compare a test-local shadow checker against every row, with the unmutated control agreeing everywhere and each of six rule mutations caught by a retained expectation. r[VC-OWN-02]
- [x] [serial] Minimise any differential failure into a reproducible fixture; exercise the deterministic prefix/operand minimiser with a synthetic probe when the lane has no natural divergence. r[VC-OWN-02]
- [x] [serial] Retain `verification/ownership/differential-d6e6f99a.json` (source commit `d6e6f99a08d00fc3d723f32dcfb5d0fc837955ec`): clean commit, toolchain, host, source/corpus/lane digests, coverage counts, model-only counts by reason and the open Rust-to-model edge. r[VC-OWN-03]
- [x] [serial] Mutate production ownership rules in scratch copies and retain `verification/ownership/production-mutants-d6e6f99a.json`: the unmutated lane passes, five mutants are caught and the quotation mutant survives because it has no executable control. r[VC-OWN-03]

## Phase 3: Promotion (not selected)

- [x] [serial] Decide whether to relate `crates/noble-kernel/src/acceptance` to the model through an extraction or refinement route; until then the differential stays finite evidence and the correspondence edge stays open. r[VC-OWN-03] **Decided 2026-11-13: not selected.** The edge stays open and labelled. A refinement route would need a named extraction subject for the acceptance machine's candidate derivation, a re-audited axiom set and a clean-source receipt for that route (see the decision, evidence, criteria and reopen conditions in `design.md`); this change does not attempt one, and the issue's deliverable 5 explicitly leaves refinement open. No implementation theorem is claimed.

## Note (2026-11-13): per-word comparison, kernel-decided corpus, production mutants

The corpus generator remains a handwritten JavaScript mirror of the Lean model. It emits
both `cases.txt` and `proofs/foundation/NobleFoundation/OwnershipCorpus.lean`, where the
kernel `decide`s `run` and the new per-word `runTrace` for every proposed row: 116 cases (28
accepted and 17 rejected through the production acceptance machine, 71 model-only), every
model-only row carrying a reason (`branch-accounting`, `error-return`,
`quotation-program-value`, `wrapped-payload`, `operand-arity`). `cases.mjs --check` fails
on any drift between the committed artefacts and the emission.

`OwnershipTrace.lean` adds `straightLine`, `runWords`, `traceWords` and `runTrace` with eight
public theorems (`run_eq_runWords`, `traceWords_length`, `traceWords_take`,
`runWords_prefix_accepted`, `traceWords_last`, `runTrace_getLast`, `trace_dup_drop`,
`trace_stops_at_refused_word`) and two private helpers, all inventoried.

The production lane drives one candidate per word prefix: after `k` words the machine's
derived stack must equal trace entry `k`, so the first diverging word is reported (60 prefix
comparisons, `first-divergence=none`). `swap` is drivable through the machine's per-word
scheme instantiation; owner identity stays unobservable and its controls stay model-only. A
failing comparison writes the shortest failing word prefix and trims trailing
operands while the divergence persists to `verification/ownership/minimised.txt`;
leading operands stay because they may be the head a word inspects. The minimiser
is exercised by a synthetic probe, and an accepted-case guard prevents shrinking
into a model-only arity refusal. The current comparison receipt is
`verification/ownership/differential-d6e6f99a.json`; its paired production-mutation
receipt is `verification/ownership/production-mutants-d6e6f99a.json`. The immutable legacy
`differential.json` remains byte-for-byte unchanged; its `419c0144` source/model
binding is stale and it is not current evidence.

Two mutation lanes are separated: the corpus mutations (all six caught by the retained rows,
the unmutated copy agreeing everywhere) and `verification/ownership/production-mutants.mjs`,
which mutates a production file in a scratch copy of the clean commit and requires the lane to
fail. Five mutants are caught (releasing the `Drop` or `Dup` data slot, dropping a `swap`
operand, making `swap` an identity, making `dup` produce one copy); releasing the `Quote`
data slot survives because every `quote` case is model-only, and the receipt records the
survivor.
