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
relation. The model exercises the fixtures, but the Lean relation, termination
theorem, claim generator and isolated checker remain open; the selected decision
is **no-go** for integration. It authorizes no automatic merge, publication,
live-slot admission, source-control migration or release.

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

## Impact

- **Files**: `.cairn/changes/semantic-merge-verification/`, the test-only
  `crates/noble-contracts/tests/models/semantic-merge-model.rs` model and
  `verification/semantic-merge-model/run.mjs` plus its receipt. ADDED
  `program-contracts` deltas `SMERGE-REL-01`, `SMERGE-SCOPE-01`,
  `SMERGE-EVIDENCE-01` and ADDED `verification` deltas `SMERGE-CHECK-01`,
  `SMERGE-GATE-01`.
- **Code**: test-only. No production checker, Lean model, claim format,
  evidence envelope, CLI, kernel, ABI or canonical encoding change.
- **Testing**: `cargo test -p noble-contracts --test source -- semantic_merge_model`
  passes. The receipt records the fixture outcomes and the no-go decision. No
  Lean proof is checked and no merge is performed or authorized. Existing
  contract, evidence and source regressions are retained unchanged.
- **Coordination**: #15 owns the correctness foundation and the preservation/
  effect-safety obligations; #10 owns proof-to-executed-artifact correspondence;
  #6 owns proof automation; #9 owns broader behavioral-effect protocols.
  Valence validates identity/linkage, Kiln interprets CI policy, and Octet owns
  a separate Rust implementation rail.
