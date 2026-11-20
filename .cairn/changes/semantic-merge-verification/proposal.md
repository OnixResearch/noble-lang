## Why

Noble already distinguishes normal-return partial correctness, prefix safety
and total correctness (`.cairn/specs/program-contracts/spec.md:110,167-174`),
and it requires a named preservation relation for a source transformation
(VC-REWRITE-01/02). It binds evidence to exact subjects, captures, interfaces
and dependencies (VC-ID-02/03) and checks the intended claim under a
consumer-selected policy (VC-EVIDENCE-01/02/04). It does not define a
four-version semantic merge relation.

Issue #30 asks for a bounded design of such a relation over a base O, variants
A and B, and a supplied candidate M, plus an evaluated language-owned checking
path. A test corpus cannot establish the universal claim, and an artifact or
recipe match is not a semantic merge proof. The first experiment must be pure,
resource-free and terminally decidable, with compatible complete ordered-stack
interfaces.

Existing evidence is deliberately two-version and conditional on normal return.
VC-LOGIC-01 says a postcondition is conditional on normal return and is not a
termination claim; VC-LOGIC-03 requires a termination argument that a pure
effect set does not provide. A merge claim that simply assumes four normal
returns would accept vacuously when a version aborts or diverges. This change selects a demonstrably terminating fragment, states the relation,
the claim/evidence binding, the checking and outcome discipline, the fixtures
and the go/no-go criteria, and implements a bounded test-only model of the
relation. Phase 2 exercises the fixtures; Phase 3 (OWNER DECISION 2026-11-20)
adds the Mathlib-free Lean relation and termination theorem, the untrusted
claim generator, consumer regeneration and the isolated pinned Lean check, and
records the obligation dispositions and a separately reviewable integration
plan. The selected decision remains **no-go** for integration: SM-3, SM-5 and
SM-6 are tested only, SM-4 is narrowed and the plan is unreviewed. It
authorizes no automatic merge, publication, live-slot admission, source-control
migration or release.

## What Changes

- Select `SMERGE/1`: four closed pure resource-free programs with identical
  complete ordered input/output stacks; observables are the full output stack
  over first-order data; `I64` is compared as wrapping 64-bit bits.
  r[SMERGE-REL-01]
- Define the per-observable three-way merge: if A changes an observable from O,
  M preserves A; if B changes it, M preserves B; if neither changes it, M
  preserves O; if both change it identically, M preserves the common value; if
  both change it incompatibly, the claim is `conflict` and no candidate is
  verified. Absent outputs, incompatible interfaces and abnormal outcomes are
  distinct nonverified outcomes, never vacuous acceptance. r[SMERGE-REL-01]
- Select a syntactically terminating fragment: nonrecursive acyclic closed
  definitions and quotations, pure first-order data words, wrapping `I64`, no
  host effects, resources, live dispatch, async or runtime-generated Programs.
  Termination is proved from the static dependency DAG and terminating builtins;
  purity alone is not the argument. r[SMERGE-SCOPE-01]
- Specify `noble-merge-claim/v1` elaboration, independently regenerated expected
  subject/claim binding, isolated pinned Lean checking, optional bounded
  counterexample search, and the outcome lattice `verified`,
  `validated-counterexample`, `unknown`, `unsupported`, `timeout-exhausted`,
  `internal-failure`. All nonverified states remain visibly nonverified.
  r[SMERGE-CHECK-01]
- Specify `noble-merge-evidence/v1` binding of O/A/B/M exact subjects,
  dependencies, captures, instantiated interfaces, selected observables,
  preconditions, semantic model, proof/toolchain, outcome and scope; do not
  reduce identity to names, locations, template IDs or equal effect sets.
  Source-model proof and installed Wasm correspondence remain separate.
  r[SMERGE-EVIDENCE-01]
- Specify positive/negative fixtures, a reproducible experimental plan and an
  explicit go/no-go decision before integration. r[SMERGE-GATE-01]
- Implement the test-only `crates/noble-contracts/tests/models/semantic-merge-model.rs`
  model and `verification/semantic-merge-model/run.mjs` receipt lane. The
  10-fixture model records 4 verified, 1 conflict, 3 counterexamples, 1
  incompatible interface and 1 abnormal outcome, and selects **no-go** because
  the Lean obligations remain open. No production checker or merge action
  changes. r[SMERGE-CHECK-01] r[SMERGE-GATE-01]
- Phase 3 (OWNER DECISION 2026-11-20): add `proofs/mc1/NobleContracts/Merge.lean`
  (`MergeRel`, the wrapping-`I64` three-way merge and the narrowed terminating
  fragment), the claim/consumer/checker lane and its receipt. A submission is
  elaborated in a **producer process** whose only output is an `.olean`; a
  fresh **replay checker** that never elaborates producer text reads that
  `.olean` as data, replays the declaration closure through the kernel with
  checks on, compares the replayed type with its own regeneration of the claim
  and computes the axiom closure there. Acceptance is never taken from
  producer-printable output. r[SMERGE-CHECK-01]
  r[SMERGE-EVIDENCE-01] r[SMERGE-GATE-01]

## Impact

- **Files**: `.cairn/changes/semantic-merge-verification/`,
  `proofs/mc1/NobleContracts/Merge.lean`, `verification/mc1/implementation.mjs`
  (the MC1 gate builds Merge and pins 121 theorems), the canonical
  `.cairn/specs/verification-toolchain/spec.md` and its
  `specs/VERIFICATION-TOOLCHAIN.md` view, the test-only merge lane under
  `crates/noble-contracts/tests/models/merge/`, the
  `verification/semantic-merge-verification/` runner and receipts, and the
  test-only `semantic-merge-model` model and receipt lane. ADDED
  `program-contracts` deltas `SMERGE-REL-01`, `SMERGE-SCOPE-01`,
  `SMERGE-EVIDENCE-01` and ADDED `verification` deltas `SMERGE-CHECK-01`,
  `SMERGE-GATE-01`.
- **Code**: test-only. The Lean model and the claim/consumer/checker lane are
  not wired into `prepare`, the CLI or any admission path; no production
  checker, claim format, evidence envelope, kernel, ABI or canonical encoding
  changes.
- **Testing**: `cargo test -p noble-contracts --test source -- semantic_merge_model`
  passes, the pinned Lean model builds and accepts the gate, and the lane's
  ignored Lean check verifies the CASE-04/CASE-01 claims through the consumer
  path while the forged submissions stay nonverified. The receipt records the
  observations, the raw transcripts and the no-go assessment. No merge is
  performed or authorized. Existing contract, evidence and source regressions
  are retained unchanged.
- **Coordination**: #15 owns the correctness foundation and the preservation/
  effect-safety obligations; #10 owns proof-to-executed-artifact correspondence;
  #6 owns proof automation; #9 owns broader behavioral-effect protocols.
  Valence validates identity/linkage, Kiln interprets CI policy, and Octet owns
  a separate Rust implementation rail.
