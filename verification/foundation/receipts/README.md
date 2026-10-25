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
