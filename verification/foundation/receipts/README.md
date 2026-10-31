# Retained foundation receipts (issue #15)

Each subdirectory holds the receipts of one clean-source run, named by
source commit. The raw run directories (emitted `.noble`/`.wat`/`.wasm`,
Lean audit files, sandbox outputs) stay outside the repository. `index.json`
records their SHA-256.

## 898cfe1 — original foundation run

`898cfe1/` contains these files. Its source tree is `d11cd3c70621`, and it is the
clean run of `verification/foundation/ci.sh`, which reported `overall=0`. This run
supersedes the earlier `ce2e5ce` copy.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim `check.mjs` receipt: 142 theorems, printed statements and axioms |
| `slice-receipt.json` | verbatim `slice.mjs` receipt for the Phase 3 chain and its refusal controls |
| `differential-summary.json` | `differential.mjs` receipt with the 163 per-case rows removed. `compacted_from.sha256` names the raw file |
| `index.json` | raw-file hashes and a result summary |

A receipt in this directory is evidence only for the commit and tree it
names. It is stale for every later commit, and that includes the commit
that adds it. `differential.mjs --verify` therefore refuses it on a
different tree, which is the intended behavior. Re-run the commands to
obtain current evidence.

## 3d5ccc2 — fixed-input branch EMI (issue #53)

`3d5ccc2/` records clean source commit
`3d5ccc2770d3928a0071c143e86fdc6abbad22d4`, tree
`51f9f9b053e25c75f7547f1ec948da3ced14fe78`. The actual invocation was
`verification/foundation/ci.sh NEW_OUTPUT_DIR --skip-slice`; its
[`ci-summary.txt`](3d5ccc2/ci-summary.txt) reports `overall=0`.
The host-service Phase 3 slice was explicitly **skipped**. No fresh slice
result is claimed, and the historical `898cfe1` packet is unchanged.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim Lean inventory/axiom audit: 148 theorems, including the six new branch-model theorems |
| `differential-summary.json` | existing arithmetic comparator: 163 matching rows, all seven controls detected, and the 1,000-oracle/200-malformed kernel checker lanes passed; raw per-case rows remain external |
| `emi.json` | verbatim exact-subject/source/tool/model/artifact receipt for all 19 generated pairs, including the three eligibility refusals |
| `emi-replay.json` | verbatim independent admission, fresh Lean checks and actual wrong-lowering discrepancy replay |
| `minimal-control.json` | replayable fixed-input original/variant pair with the intentional lowering fault |
| `*-verify.json` | actual clean-tree verifier stdout; all fresh receipt checks passed |
| `stale-receipt-refused.json` | actual refusal of the older arithmetic receipt on the new tree |
| `nix-*` | verbatim outputs from the five selected Nix acceptance gates |
| `index.json` | raw and retained-file hashes, result summary, and selected Nix gate evidence |

The EMI run admitted 16 pairs: 12 scalar `if` and four I64 sum `case`.
Each of the four transformation kinds has four eligible pairs. Original
and variant executed with opt off and on: 64 nominal executions, all with
the independently checked ordered result, preserved prefix, sufficient
headroom, and empty host trace/request count. All seven EMI controls were
detected, including the real wrong-selected-branch Wasm lowering in both
optimization modes.

The lowering-control shrink used 13 attempts, retained 12 reductions, and
reached a local minimum without exhausting the 64-attempt bound. It
preserved the complete ordered interface and reduced the mutation site
from 2 to 0. The retained minimal discrepancy replayed successfully.

The exact source commit also passed `octet`, `source-coverage`,
`native-assurance`, `policy`, and `source-inventory-controls`. Octet
reported zero findings, warnings and errors; source coverage matched all
12,053 reviewed subjects. These selected gates are not the entire flake
check set, and inventory/native-assurance success is not refinement or
soundness evidence.

These are finite fixed-input model and execution results, not universal
program equivalence or a compiler theorem. Rust renderer/lowering
refinement remains open; downloaded Mathlib `.olean` files remain a trust
assumption. Like every packet above, this packet is stale for the commit
that adds it: the successful verifier outputs name the tested source tree,
not the later evidence-only commit.

## 039b5ee — bounded one-call linking (issue #52)

[`039b5ee/index.json`](039b5ee/index.json) records clean source commit
`039b5ee1473bae02ec3140308d0023353242a486`, tree
`3b3990393ed477348018b033b463f93930392110`. The actual invocation was
`verification/foundation/ci.sh NEW_OUTPUT_DIR --skip-slice`; the retained
[`ci-summary.txt`](039b5ee/ci-summary.txt) reports `overall=0`.
The Phase 3 host-service slice was explicitly **skipped**.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim inventory: 196 Lean-checked theorems, including 48 linking theorems, with exact statements and transitive axioms |
| `linking.json` | verbatim exact-subject, dependency, ABI, tool and artifact receipt for the actual same-artifact named-call/continuation path |
| `linking-summary.json` | verbatim runner stdout: 24 normal executions and all eight controls detected |
| `differential-summary.json` | 163 matching rows, all seven controls detected; per-case rows remain external and their raw receipt is hashed |
| `emi.json`, `emi-replay.json`, `minimal-control.json` | verbatim branch-EMI regression and discrepancy replay evidence |
| `*-verify.json`, `stale-receipt-refused.json` | actual clean-tree receipt verification and stale-tree refusal stdout |
| `nix-*` | verbatim selected gate logs and summary, including the failing document gate |
| `document-failure-attribution.json` | offline baseline attribution of that recorded failure, not a baseline test run |

The linking controls detect a same-signature wrong body, same-signature
identity reuse, reversed arguments, reversed results, a wrong return target,
changed version, changed dependency bytes, and post-check callee substitution.
Both optimization modes execute the ordered scalar results and preserved
typed prefix. The audited transitive axiom union is `Classical.choice`,
`Quot.sound`, and `propext`.

The selected Nix `format`, `cairn`, `policy`, and `tool-selection` gates passed.
**`documents` failed:** 91 tests passed and 18 failed, reporting 17 distinct
local links already present at baseline `62b6362e7c05458cc8c4fbdddf781208c8392db0`.
Fifteen destinations are missing historical files; two are directory-only
links. The packet retains the failure rather than treating attribution as a
pass. These selected gates are not the complete flake check set.

The horizontal linking and separate vertical composition theorems are
conditional model results, not an implementation theorem. C8 and O1/O3/O7
lowering/loading correspondence remain premises; quota/abort and resource
behavior remain outside the selected claim. The evidence-only commit that
adds this packet has a different tree: its historical successful verifier
outputs do not authorize receipt reuse on that later tree.

## 4be2a68 — binary32 reference model (issue #38)

`4be2a68/` records Clean source commit
`4be2a68b1fbc3211c6bc079a435976527bfc96a3`, tree
`ad2727755592f0a7569acfc40a950b179ba55a8e`, from
`verification/foundation/check.mjs --output /var/tmp/f32-check`. It is a
checker run only: the Phase 3 slice, the differential lane and the EMI lane
were **not** re-run, and no fresh slice or comparator result is claimed here.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim Lean inventory/axiom audit: 205 theorems, including nine new binary32 theorems |
| `index.json` | raw-file hash, axiom-set counts and the binary32 theorem list |

The nine new theorems are about `NobleFoundation.Binary32`: raw encodings,
classification, an exact scaled-integer interpretation, round-to-nearest /
ties-to-even with gradual underflow, and add/subtract/multiply including
signed-zero, infinity, NaN and overflow rules. They establish model-level
representation and rounding properties and kernel-check 26 retained
expectations proposed by `verification/binary32/propose.py`. They do not
establish correspondence with the Rust graph evaluator, `wasm-opt`, V8 or any
host FPU, and they close no obligation under #10/#15.

## aa074e1 — binary32 reference with the mutation lane (issue #38)

`aa074e1/` records clean source commit
`aa074e137…`, tree `0fef9fc8df537854c854b90d09c326b7e21746b2`, produced by
`verification/foundation/check.mjs --output /var/tmp/f32-check2`. It supersedes
`4be2a68/` for the binary32 claim only: that earlier packet stays untouched and
remains the evidence for the model without the mutation lane.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim Lean inventory/axiom audit: 212 theorems, sixteen of them binary32 |
| `index.json` | raw-file hash, axiom-set counts, the binary32 theorem list, the six test-local variants |

The six variants live in `NobleFoundation/Binary32Mutations.lean` and are
compiled only in the foundation project: an away-from-zero tie rule, collapsing
every zero result to `+0`, flush-to-zero, a zero-sign-insensitive observation, a
commutative subtraction, and implicit contraction. Each is caught by a named
retained expectation, and `all_declared_mutations_are_caught` states that no
declared variant escapes. Detection is kernel-checked model disagreement, not a
Rust-side mutation run; the packed-evaluator replay remains open.

## aa13a54e9 — WITHDRAWN foundation run

`aa13a54e9/` records clean source commit
`aa13a54e996947942e04f1d4e1a3eaba5182bba9`, tree `3b48bbac7af1bfdd8fc66d63095d32ed69cc028e`,
from `verification/foundation/check.mjs --output`. **Its subject was withdrawn at `53c078b3a`**:
the run included a second `RegionTrace` module under `proofs/foundation`, which duplicated the
existing `proofs/m5/RegionTrace.lean` subject, so the module and its inventory rows were deleted.

The receipt bytes are retained unchanged as the record of that commit's run, per the rule that
historical receipts are not rewritten. They are **stale for the current foundation** and must not
be cited: `aa13a54e9/WITHDRAWN.md` states the scope. Region-trace results belong to the m5 module
and its own lanes.

## cc147dd40 — bounded task-interleaving model (issue #46)

`cc147dd40/` records the clean run that added `NobleFoundation.TaskInterleaving` (23 inventoried
theorems; that run reported 428 in total). It is model evidence for `TaskId`/`Lifecycle`/`Slot`/`Table`
step relations, interleavings and lossy observations; it is not a universal theorem and says
nothing about compiler correspondence.

## 4e9ca2acb — proved reduction for the task-interleaving model (issue #46)

`4e9ca2acb/` records the clean run at source `4e9ca2acb` that added the wake budget, the retiring
mark, the proved reduction, the declared-bound counts and the coverage witnesses to
`NobleFoundation.TaskInterleaving` (36 inventoried theorems; that run reported 520 in total). The
commutation, canonical-form and agreement theorems are universal over tables, alphabets and depths;
the counts are kernel-checked by `decide +kernel` at the declared bound only. It is model evidence
and says nothing about correspondence with the Rust table in
`crates/noble-kernel/src/async_tasks/table`.

## 477d24b9e — valid client histories for the task-interleaving model (issue #46)

`477d24b9e/` records the clean run at source `477d24b9e` that added the client semantics, the
valid-client predicate, the documented and lossy client projections, and the client-level reduction
agreement (31 inventoried theorems; that run reported 551 in total). The agreement and
canonical-form theorems are universal; the history counts and the unsound-rule detection are
kernel-checked at the declared bound. It is model evidence, not a weak-memory or linearizability
result, and says nothing about correspondence with the Rust table.

## 9f1288d61 — corrections from the issue audits (issues #38, #43, #45, #48, #56)

`9f1288d61/` records the clean run after the foundation corrections:
- the PackedLayout register-count rule now uses the highest register, with a real Rust layout
  accepted;
- the ServiceWire schema controls are rebuilt on the real record prefix;
- the binary32 contraction mutant is the fused form;
- stale docstrings are corrected.
That run reported 557 theorems. It supersedes nothing: earlier receipts stay as recorded, and
their statements for the corrected theorems are the ones the corrections replace.

## 1d4113e6c — event relocation under root pinning (issue #47)

`1d4113e6c/` records the clean run that added the dispatch run, the relocation fixtures and the
current-map control to `NobleFoundation.LiveDispatch` (6 inventoried theorems; that run reported
563 in total). `denials_only_truncate` is universal over policies, programs, views and stacks;
the emission counts are kernel-checked fixtures. It is model evidence and says nothing about
correspondence with `slot-registry.mjs`.

## 656ec91b4 — whole-program ownership conservation (issue #59)

`656ec91b4/` records the clean run that added `NobleFoundation.OwnershipConservation` (6 inventoried
theorems; that run reported 569 in total, all six new theorems on `propext` alone). The
conservation statement is exact but partly vacuous for resource movement: the ownership grammar
admits no acquire, release or consuming word. It says nothing about `crates/noble-kernel`.

## a8877ff78 — Binary32 normal-range rounding error (issue #44)

`a8877ff78/` records clean source commit
`a8877ff78ef915f099e0faa75dc2bd76289fc3ed`, tree
`e73bc256c9117f03a7a01fc461a8b4c5a3ad7877`, from
`verification/foundation/check.mjs --output /var/tmp/fc44-a8877ff78`.
The run passed with 588 inventoried theorems and `clean_source: true`.

The 19 new `Binary32.Error` theorems prove exact normal-output values,
universal ties-to-even quotient error, normal-bin rounding bounds, the
conditional absolute/relative `roundPositive` rules, and the `1/3`
non-vacuity witness. The scale inequalities remain explicit premises because
their universal derivation from the custom `floorLog2Ratio` is still open;
subnormal rounding, add/mul corollaries, the checker, analyzer comparison,
go/no-go, and implementation correspondence are also open.

| File | Content |
|---|---|
| `foundation-check.json` | verbatim clean-source Lean inventory and per-theorem axiom audit |
| `Audit.lean` | checker-generated theorem audit source |

## 50ec19885 — certificate replay endpoint equivalence (issue #42)

`50ec19885/` records the clean run that added `applyDerivation` and
`accepted_endpoint_equivalence` to `NobleFoundation.Certificate`, with the two-fold fixture and the
substituted-result control (6 inventoried theorems; that run reported 594 in total, all on
`propext` or `propext`/`Quot.sound`). It is a theorem about the Lean replay model; no theorem binds
`crates/noble-ngraph/src/cert.rs` to it.

## 9936fe19c — reverse-AD slot-gradient fixture (issue #45)

`9936fe19c/` records the clean run that added the slot-mapped gradient extraction and the
non-index-aligned `slotGraph` fixture to `NobleFoundation.ReverseAD` (12 inventoried theorems:
fan-out, repeated slot, unit and seed-5 gradients against forward tangents and Mathlib
derivatives, and the wrong-slot control; that run reported 606 in total). It is a real-valued
model result; no theorem relates it to `crates/noble-ngraph/src/grad.rs`.

## 23462be4b — swap moves resources (issue #59)

`23462be4b/` records the clean run after `Ownership.step .swap` dropped its eligibility check to
match the reviewed design and production (`data_slot` constrains only dup/drop/quote). That run
reported 608 theorems, 34 of them ownership, including `swap_moves_a_resource`,
`resource_operand_blocks_non_swap_word` and `swap_moves_resource_conservation`. It is model
evidence only; the production refinement stays open.

## 48a7a4cbd — packed-schedule instruction-stream validator (issue #43)

`48a7a4cbd/` records the clean run that added `validate` over a supplied instruction stream and its
`validate_sound` theorem to `NobleFoundation.PackedSchedule`, with the reordered-stream and refusal
fixtures (32 new theorems; that run reported 622 in total). No Rust instruction-body export
or correspondence exists.

## d3f856d50 — checked I64 reference (issue #39)

`d3f856d50/` records the clean run that added `NobleFoundation.CheckedI64`: exact success and overflow
characterisations, truncating divrem laws and the (MIN, -1) overflow, plus controls (24 new
theorems; that run reported 646 in total). It is a name-neutral reference; API names, error
representation and overflow policy remain owner decisions.

## e417bf841 — interaction-net contract and wiring validator (issue #37)

`e417bf841/` records the clean run that added `NobleFoundation.InteractionNet`: the version-1 net
contract, `validate_sound` (reciprocal unique-incidence wiring, compatible ports, exact ordered
boundary) and accepted/rejected wiring fixtures (11 new theorems; that run reported 657
in total). Reduction, readback and translation remain open.

## 42350bf01 — bounded ISOP Boolean core (issue #35)

`42350bf01/` records the clean run that added `NobleFoundation.Isop`: the exact Minato–Morreale
cover, `isop_bounds`, the independent `equivOn` checker with its soundness theorem, fixtures and
wrong-cofactor/complement controls (17 new theorems; that run reported 674 in total).
Irredundancy is fixture-only; integration remains open.

## 84753c99e — static-input specialization model (issue #32)

`84753c99e/` records the clean run that added `NobleFoundation.Specialize`: index-preserving
specialization with constant folding, `specialize_wellFormed` and the universal
`specialize_sound`, fixtures and the prefix-merge control (18 new theorems; that run reported
692 in total). The binding-time contract and Rust entry point remain owner decisions.

## 58aaefd05 — exact and RNE decimal import model (issue #40)

`58aaefd05/` records the clean run that added `NobleFoundation.Binary32.DecimalImport`: exact
decimal ratios, single-rounding RNE, the exact-import characterisation and its agreement with RNE,
and the 0.1/0.5/-0/1e39/subnormal/tie fixtures (15 new theorems; that run reported 707 in
total). The overflow policy and syntax remain owner decisions.

## d23d0c485 — collision-safe cache lookup and key encoding (issue #33)

`d23d0c485/` records the clean run that added `NobleFoundation.CacheLookup`: manifest-exact lookup
over a possibly colliding index, stale/corrupt refusals, erase-never-creates-a-hit, and the
bounded injectivity of the length-prefixed part encoding with a naive-concatenation collision
control (21 new theorems; that run reported 728 in total). Manifest fields, persistence
and trust path remain owner decisions.

## 9c93d6cc8 — constructor reuse under uniqueness (issue #36)

`9c93d6cc8/` records the clean run that added `NobleFoundation.ConstructorReuse`: size-checked
in-place reuse, the unique-root replacement theorem, the shared-alias control and the
wrapped-resource eligibility control (9 new theorems; that run reported 737 in total). It
claims nothing about the allocator, runtime or Wasm.

## 620b43395 — restricted SGD reference (issue #58)

`620b43395/` records the clean run that added `NobleFoundation.Binary32.Sgd`: the published two-rounding
update over the #38 model, the pilot admission rule with lane-indexed refusals, universal shape,
per-lane and nonfinite-refusal theorems, boundary expectations and the fused-update control (15
new theorems; that run reported 752 in total). No candidate checker or Rust correspondence exists.

## 14a7793c9 — first Futamura projection, restricted family (issue #34)

`14a7793c9/` records the clean run that added `NobleFoundation.FirstProjection`: the one-hot
two-instruction interpreter, `interpreter_agrees`, `first_projection` through #32's
`specialize_sound`, sound zero annihilation, and the measured dead-branch and remaining-overhead
fixtures (23 new theorems; that run reported 775 in total). Identity forwarding is open.

## 3b1841f35 — binary32 scale premises derived (issue #44)

`3b1841f35/` records the clean run that added `NobleFoundation.Binary32.Scale`: the model `log2`
bracket, the lower scale premise for every positive input, the upper premise off the power
boundary, its refutation at `1/2`, the exact carry on the boundary, and the premise-free
normal-range absolute and relative bounds (17 new theorems; that run reported 792 in total).

## 309f46c02 — reverse-AD mutated-update controls (issue #45)

`309f46c02/` records the clean run that added `Node.sub` to `NobleFoundation.ReverseAD` and the
`NobleFoundation.ReverseAD.Mutations` controls: the parameterised sweep equal to
`adjointAfter`, four seeded update-rule faults caught on `slotGraph`, and the subtraction fixture
with its sign control (12 new theorems; that run reported 804 in total).

## 0cf855674 — ownership per-word rules and join lift (issue #59)

`0cf855674/` records the clean run that added `NobleFoundation.OwnershipJudgment`: independent
per-word rules equivalent to `step`, the judgment on them equal to `Accounts`, checker
completeness, and the eligibility-agreement lift over straight-line word sequences (9 new
theorems; that run reported 813 in total).

## e1eca8b19 — binary32 subnormal rounding error (issue #44)

`e1eca8b19/` records the clean run that added `NobleFoundation.Binary32.Subnormal`: the subnormal
branch of `roundPositive` within `2^-150`, the finite absolute error rule over every binade, the
attained bound at the smallest normal, and the relative-error control (15 new theorems; that run
reported 828 in total).

## 6c77ad023 — reverse-AD transpose identity over every graph (issue #45)

`6c77ad023/` records the clean run that added `NobleFoundation.ReverseAD.Transpose`: prefix
stability, the coefficient-tangent linearity per node kind, the frontier invariant, the transpose
identity over every well-formed graph, and the reverse sweep as the Mathlib derivative (10 new
theorems; that run reported 838 in total).

## 31caad931 — reverse-AD final adjoints node by node (issue #45)

`31caad931/` records the clean run that added `NobleFoundation.ReverseAD.Adjoint`: the unrolled
sweep, the final-once-processed frame, each final adjoint as seed plus its consumers'
contributions over every well-formed graph, and the interior-seed fixture (9 new theorems;
that run reported 847 in total).

## 956d8ca07 — reverse-AD ordered binary32 reference (issue #45)

`956d8ca07/` records the clean run that added `NobleFoundation.ReverseAD.Ordered`: the `grad.rs`
operation order over the binary32 reference, the stored-order cancellation fixture, the
ascending-traversal, presummed-occurrence and seed controls, and the real-valued counterpart
(7 new theorems; that run reported 854 in total).

## 79a394db9 — binary32 rounding error of whole operation sequences (issue #44)

`79a394db9/` records the clean run that added `NobleFoundation.Binary32.Sequence`: the sign-bit
negation, one signed rounding, the relative error of one model addition and multiplication, the
ordered affine fold bound and the cancellation fixture (34 new theorems; that run reported
888 in total).

## 7445a3696 — binary32 sequence error with subnormal intermediates (issue #44)

`7445a3696/` records the clean run that added `NobleFoundation.Binary32.SequenceFinite`: the mixed
relative-plus-absolute rule for every finite rounding, one model addition and multiplication, the
finite fold bound with its absolute term, and the subnormal fixture (14 new theorems; that run
reported 902 in total).

## b6cce7a71 — ownership join lift over the whole fragment (issue #59)

`b6cce7a71/` records the clean run that characterised the eligibility lift over branches and the
error return: result agreement for every program, admission agreement for every branch-free
program, and the branch counterexample (6 new theorems; that run reported 908 in total).

## 80ccf5670 — binary32 roundoff certificate checker (issue #44)

`80ccf5670/` records the clean run that added `NobleFoundation.Binary32.Checker`: the overflow and
finiteness bridges, the total add/multiply certificate checker with `check_sound` and
`check_output_bound`, and its decided fixtures and controls (33 new theorems; that run reported
941 in total).

## 9864cb89b — identity forwarding by operand rewriting (issue #34)

`9864cb89b/` records the clean run that added `NobleFoundation.OperandForwarding`: forwarding
soundness, the twice-forwarded first-projection residual that is exactly the program, and its
controls (16 new theorems; that run reported 957 in total).

## 9afcfada9 — RegionProtocol-v1 model, T1–T3 (issue #9)

`9afcfada9/` records the clean run that added `NobleFoundation.RegionProtocol`: the transition table,
the selected subject's normal-return postconditions, prefix safety P1–P6 through the monitor, and
sequential composition with its counter and set-union controls (42 new theorems; that run reported
999 in total).

## d2a5393c2 — interaction-net revision-1 orientation analysis (issue #37)

`d2a5393c2/` records the clean run that added `NobleFoundation.InteractionNetOrientation`: the
active-pair characterisation under the documented orientation and the three validator findings
(6 new theorems; that run reported 1005 in total).

## 0d59c4105 — effects-model frames, dip and branches (OWN-PO-1, issue #3)

`0d59c4105/` records the clean run that added `dip` frames and `ifte` branches to the effects
model and proved OWN-PO-1 there over stack and frame-held resources (14 new theorems; that
run reported 1019 in total).

## 588403e92 — first-order unification for implicit proof arguments (issue #12)

`588403e92/` records the clean run that proved the implicit-argument unification model: most
general answers, sound refusals, fragment completeness, budget determinism and uniqueness
exactly at ground answers (59 new theorems; that run reported 1078 in total).
