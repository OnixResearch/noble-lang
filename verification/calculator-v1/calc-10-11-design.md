# CALC-10 and CALC-11: authoring-tool boundary and benchmark integrity (design note)

Status: **design only.** Nothing in this note has run. CALC-10 and CALC-11
remain `implementation: absent`, `execution: not-run`, `proof: open`,
`trust: unassessed` with `evidence: []`
([cases L133-L159](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/conformance/calculator-cases.json#L133-L159)).
The note does not promote either case and runs no AI benchmark. It identifies
which tool surfaces apply to calculator edits, which routes are unsupported,
what a run must record, and what has to exist before execution. All links are
pinned to `44329dc`.

## What gets edited

CALC-AI-04 is about authoring Noble programs, so the subject of an edit is the
calculator's Noble guest source: the eleven declared modules embedded by
[`calculator.rs` L27-L36](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L27-L36)
(`crates/noble-contracts/src/fixtures/calc-errors.noble` and
`calculator-*.noble`). It is not a calculator input line. A calculator line is
data to the guest. Its `provenance=unavailable` diagnostics
([`calculator.rs` L4982](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4982)) belong to
CALC-PARSE-02, not to an authoring tool.

The independent acceptance boundary for an edit is the same for every route
(CALC-AI-04, `same_acceptance_boundary: true`):

1. **Source acceptance.** The edited modules pass the selected
   `ModuleSession::new_calculator` checks and independent kernel acceptance,
   which is the same path the `noble calculator` binary takes at startup.
2. **Runtime acceptance.** The #19 runner runs the rebuilt binary
   ([`../README.md`](README.md)) on the canonical vectors.
3. **Independent oracle comparison.** The CALC-12 lane compares the same
   processes against the independent oracle ([`oracle/`](oracle/README.md)).
4. **Held-out cases.** A task-owned held-out set is selected outside producer
   control (CALC-AI-03).

A route may differ only in how the candidate source is produced. It may never
add its own checker (CALC-AI-04: "This benchmark adds no alternative trusted
checker").

## CALC-10: authoring-tool variants

| Variant (expected outcome) | Candidate surface at 44329dc | Applies to calculator sources? | Status |
|---|---|---|---|
| `stale-base-revision` (reject before publication) | Live-slot CAS `stale-reject` before contract checks ([slot-registry.mjs L354-L370](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/runtime/slot-registry.mjs#L354-L370)); live repl `--expect-generation` ([live.rs L259-L282, L483-L494](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/live.rs#L259-L282)) | No. The Calculator-v1 runtime profile is disjoint from live and declared profiles ([engine-host.mjs L8-L13](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/runtime/engine-host.mjs#L8-L13)). The calculator has no source-base identity or edit-publication operation; its modules are compile-time `include_bytes!`. LSLOT-04 and LIVE-09/10 are themselves absent/not-run. | **unsupported** |
| `text-edit-invalid-stack` (acceptance reject) | Editing a `calc*.noble` module, rebuilding, and letting `Session::new` source preparation refuse it ([calculator.rs L273-L303](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L273-L303)) | Partly. The refusal is real, but it needs a binary rebuild. It prints `source_error` (`{stage} [start,end): message`, [L4628-L4632](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/calculator.rs#L4628-L4632)) without the DX-DIAG-01 fields: word or join, required stack, actual stack, constraint, value origin or unavailable. | **partial**; no recorded case |
| `structured-edit-invalid-stack` (acceptance reject) | `noble editor admit` ([editor.rs L211-L231](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/editor.rs#L211-L231)) | No. Editor nodes are only integer, Boolean, word, quotation and hole ([contracts editor.rs L9-L15](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/source/editor.rs#L9-L15)). It has no module, `def`, `signature`, `variant` or Text forms, and it analyzes against the empty Core-Bootstrap namespace, so `calc_*@1` words cannot resolve. | **unsupported** |
| `editor-hole` (incomplete, not executable) | DX-02 (passed): `admitted_source` refuses any hole ([contracts editor.rs L206-L217](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-contracts/src/source/editor.rs#L206-L217)) | The refusal itself does not depend on the namespace, but no calculator module can be expressed or analyzed in Editor-Draft, so there is no calculator-bound observation. | **generic route only**; calculator binding open |
| `unavailable-origin-span` (provenance unavailable) | Checker field `value_origin_or_unavailable` ([output.rs L229-L233](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/crates/noble-cli/src/core/output.rs#L229-L233); DX-01 passed, Core-Bootstrap) | Not for calculator module edits: calculator source-check errors omit value origin. Calculator *lines* always print `provenance=unavailable`, which is the runtime rule, not an authoring-tool observation. | **open** for calculator sources |

**Prerequisites for executing CALC-10.**

- **Source-base identity and stale-base check.** The calculator source set
  needs a base identity, such as the SHA-256 of each fixture (the CALC-12
  evidence controls already compute this manifest). It also needs an
  edit-publication operation that compares the expected base and rejects a
  stale one before publication. The DX-IDENTITY-01/DX-CACHE-01 contracts
  (DX-05, absent) would own that identity.
- **DX-DIAG-01 diagnostics.** Calculator module acceptance must emit the
  DX-DIAG-01 diagnostic fields, including explicit `unavailable` value origins,
  instead of the bare `source_error` string.
- **Structured edits.** These need a versioned structured-edit schema that can
  express declared-module forms (module, `signature`, `def`, `variant`,
  Text). It must be analyzed in the calculator namespace and rendered to
  source that passes the same acceptance boundary. The DX-HOLE-01 rules
  carry over: holes stay incomplete, every admission path rejects them, and
  analysis never executes them.
- **Tool schemas.** The concrete tool schemas themselves; CALC-AI-04 names
  them an implementation-entry gate.
- **Separate lanes.** The #19 runner and the CALC-12 oracle must exist as
  separate acceptance lanes. This branch delivers the oracle lane; the runner
  is #19.

Until these exist, the `stale-base-revision` and
`structured-edit-invalid-stack` variants are **unsupported**, and the other
three have **no calculator-bound evidence**. None may be reported as passing.

## CALC-11: benchmark integrity

### Routes

| Authoring route (CALC-AI-03) | Status for calculator tasks | Why |
|---|---|---|
| `stack-only` | **supported route, blocked execution** | The guest modules are stack-only Noble today. A text edit can pass the acceptance boundary above. Execution still needs the harness and records below. |
| `local-names` | **unsupported** | DX-07 (DX-LOCAL-01/02) is absent/not-run ([language-workflow-cases.json L7-L12](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/conformance/language-workflow-cases.json#L7-L12)). |
| `structured-edits` | **unsupported** | Editor-Draft cannot express calculator modules (see CALC-10 above). |

An unsupported route appears in every report as `unsupported-visible` with
this reason. It never counts as a failure or a pass. The roadmap records the
same restriction: "MA3 compares only supported authoring routes"
([ROADMAP.md L94-L98](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/ROADMAP.md?plain=1#L94-L98)).

### Task families

These are the five families from CALC-AI-01. Each task must bind an immutable
starting revision, the requested change, and an independently owned
acceptance set:

| Family | Independent acceptance it can reuse |
|---|---|
| exact-evaluator | CALC-01/02/03 vectors through the #19 runner, plus CALC-12 oracle comparison of seeded generated inputs |
| integer-powers | CALC-01 power vectors, CALC-03 `0^-1` and `4^(1/2)`, plus oracle-generated power chains and signed exponents |
| lexical-functions | CALC-06 session and CALC-07 admission variants (five CLI-supported, two unsupported-on-cli) |
| parser-repair | CALC-02 spans plus held-out syntax shapes; the seeded `left-associated-power` patch is a ready supplied-defect starting point |
| numeric-capacity | CALC-01 I64-boundary vectors, CALC-08 budgets (#20) and the seeded `I64-wrapping` control |

The CALC-12 seeded-fault patches ([`seeded-faults/`](seeded-faults/README.md))
can serve as supplied defects for the parser-repair family. They must not be
reused as held-out acceptance.

### Required record (CALC-AI-02, case `required_record_fields`)

| Field | Source of truth |
|---|---|
| `model_version` | provider model identifier and version, as reported |
| `task_revision` | SHA-256 of the task file (requested change, family, route, budgets, acceptance-set hash) |
| `starting_revision` | git commit, plus the guest fixture SHA-256 manifest |
| `prompts`, `supplied_context` | the exact bytes, or an explicit `replay-limited` marker where retention is not authorized |
| `tool_interfaces` | tool schema names and versions offered to the producer |
| `budgets` | token, time and attempt budgets, plus the seven guest calculator budgets used by acceptance |
| `attempts`, `patches` | ordered list; each patch is a unified diff with its SHA-256 |
| `compiler_diagnostics` | the DX-DIAG-01 records returned per attempt |
| `compiler_backend_revision` | `noble` binary SHA-256, build profile, `--internal-calculator-wat-version`, and the Node/wasm-tools pins that the #19 receipt already records |
| `numeric_library_revision` | SHA-256 of `calculator-bigint/-bits/-decimal/-numeric.noble` (and `calc-errors.noble`) at the attempt |
| `outcomes` | per attempt: acceptance-runner receipt, oracle comparison report, held-out result, and integrity-check result |

### Metrics (case `metrics`)

| Metric | Definition |
|---|---|
| `first_attempt_acceptance` | the first patch passes the full independent acceptance |
| `final_acceptance` | the final patch passes it within the attempt budget |
| `repair_attempts` | the number of attempts after the first |
| `total_tokens` | provider-reported input plus output tokens over all attempts |
| `elapsed_time` | wall clock from task start to final outcome, with acceptance time reported separately |
| `regressions` | previously passing acceptance vectors that fail after the patch |
| `incorrect_success_claims` | attempts where the producer reported success but independent acceptance (including held-out cases) rejected the result |

### Integrity controls (case variants)

| Variant | Mechanism | Outcome |
|---|---|---|
| `producer-removes-failing-test` | The acceptance set is hashed into `task_revision` and stored outside the producer's writable tree. Any patch touching `specs/conformance/calculator-cases.json`, `verification/calculator-v1/**` or held-out files is rejected before acceptance runs. | `reject-task-integrity` |
| `producer-strengthens-precondition` | The patch is diffed against the task's pinned interfaces: guest `signature` effect rows, budget defaults in `calculator.rs`, CLI flags, spec preconditions. Narrowing an input domain (for example rejecting `0^0`, or adding a nonzero-divisor guard that changes the outcome category) fails the unchanged acceptance or the interface diff. | `reject-task-integrity` |
| `producer-adds-authority` | Any change to worker imports, `engine-host.mjs` profiles, host bindings, or a guest `!` effect row other than `pure` is rejected. The calculator has no host capabilities (CALC-BOUND-01). | `reject-task-integrity` |
| `producer-reports-success-but-hidden-test-fails` | The held-out set runs only after the producer's final claim, and the claim is recorded before acceptance runs. | `incorrect-success-claim` (counted, never dropped) |
| `unsupported-authoring-route` | A route outside `stack-only` (see the routes table). | `unsupported-visible` |
| `run-timeout` | The #19 runner `timeout` classification, or the producer budget wall clock | `timeout-visible`, never a pass or a `ResourceLimit` |
| `harness-error` | The runner `native-failure`/`host-limit` classes, or a harness exception | `harness-error-visible`; the run is retained, not omitted |

Reports must keep every failed, unsupported, timed-out and harness-error run
(`failed_runs_omitted: false`). One model or a small task set never
establishes universal AI suitability.

## Execution prerequisites (both cases)

1. **Acceptance runner.** The #19 runner is merged and has receipts on the
   current tree.
2. **Oracle lane.** The CALC-12 oracle and evidence lanes are merged (this
   branch).
3. **Tool schemas and stale-base check.** The authoring tool schemas, the
   calculator source-base identity and the stale-base check exist (CALC-10
   prerequisites above).
4. **Calculator-namespace diagnostics.** DX-DIAG-01 diagnostics are available
   for calculator module acceptance.
5. **Task portfolio.** Immutable starting revisions and held-out sets exist,
   owned outside the producer.
6. **Acceptance cost.** At 44329dc one calculator line takes several minutes
   (release binary: 6–13 min per CALC-01 vector under shared load in the #19
   sweep). A task's acceptance costs hours, which makes `elapsed_time`
   dominated by acceptance. The calculator speed issues (#16–#18) are a
   practical prerequisite, not a correctness one.
7. **Authorization.** Model access, plus authorization to retain prompts and
   context, or an explicit replay limitation for sensitive inputs
   (CALC-AI-02).

No benchmark harness, record schema or metric computation exists in
`crates/`, `tools/` or `verification/` at 44329dc. Only the ledger-shape
checks in
[`tools/check-specs.mjs` L412-L426](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/tools/check-specs.mjs#L412-L426) exist,
and STATUS records `benchmark_execution: not-run` and
`authoring_tool_schema: open`
([STATUS.json L975-L991](https://github.com/OnixResearch/noble-lang/blob/44329dcbe0c0d3c0e83d4d154aab8a357a06f9a2/specs/STATUS.json#L975-L991)).
