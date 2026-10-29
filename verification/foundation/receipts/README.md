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
