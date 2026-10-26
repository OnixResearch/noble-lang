## ADDED Requirements

### Requirement: NGRAPH-OPT-01
r[NGRAPH-OPT-01]

A candidate optimizer program MUST be a pure, fixed-shape transition of the form
`(parameters, gradients, optimizer state, bounded step descriptor) → (fresh
parameters, fresh optimizer state)` over a minimal typed IR. The transition MUST
declare its exact input shapes and its own persistent state, and MUST NOT possess
an ambient handle to the model graph, loss, dataset, evaluator, accepted history
or any host capability. The IR MUST declare a finite operation whitelist, a
finite node budget, a state-byte budget, a work budget and an output budget.
Graphs, losses, derivative policies, datasets, evaluators and accepted histories
MUST remain immutable across every candidate transition.

#### Scenario: limits are declared before any candidate is admitted

- GIVEN a candidate envelope that omits one of nodes, state bytes, work or outputs
- WHEN admission inspects the envelope
- THEN the candidate MUST be refused as malformed and no snapshot MUST be
  published

#### Scenario: the transition cannot name foreign state

- GIVEN a candidate whose declared inputs do not include the optimizer state
- WHEN the candidate attempts to read or write optimizer state
- THEN the operation MUST be outside the whitelist and the candidate MUST be
  refused

### Requirement: NGRAPH-OPT-02
r[NGRAPH-OPT-02]

The pilot MUST select a deliberately `F32`-only SGD-like reference update as the
first checked algorithm and MUST state its exact arithmetic, rounding,
nonfinite and boundary rules. `adam/0` MUST NOT be reused to certify host binary64
arithmetic: because `adam/0` keeps binary64 moments and converts them into
binary32 parameters, a binary32-only candidate is a distinct algorithm rather
than an equivalent lowering. A binary64-moment candidate MAY be specified later
only as its own requirement with its own model, and MUST NOT silently inherit the
`F32` reference.

#### Scenario: numeric mismatch is resolved rather than assumed

- GIVEN a proposal whose parameters and gradients are `F32` but whose moments are
  binary64
- WHEN the candidate is classified
- THEN it MUST be recorded as the binary64 algorithm, not as an `F32` candidate,
  and MUST NOT be admitted under the `F32` reference

#### Scenario: rounding and nonfinite rules are explicit

- GIVEN a boundary rounding value or a nonfinite gradient
- WHEN the reference update evaluates it
- THEN the outcome MUST be determined by the declared rounding and nonfinite rules
  rather than by host defaults, and the classification MUST be recorded

### Requirement: NGRAPH-OPT-03
r[NGRAPH-OPT-03]

Admission MUST distinguish `malformed`, `unsupported`, `exhausted` and `internal`
outcomes from an admitted candidate, and each refusal MUST carry the exact
offending location in the candidate envelope. An over-budget candidate MUST be
refused rather than truncated or partially applied. Shape mismatch, aliasing
attempts, nonfinite values where the reference declares finiteness, and invalid
optimizer-state transitions MUST each be refused. Lean kernel-checked semantics
MUST be the reference for the selected slice; checker obligations MUST state that
acceptance implies the model's well-formedness premises; finite Rust comparisons
MUST be labeled implementation evidence and MUST NOT be reported as proof.

#### Scenario: budgets refuse instead of truncating

- GIVEN a candidate exceeding the declared node or state-byte budget
- WHEN admission classifies it
- THEN the outcome MUST be `exhausted`, nothing MUST be partially published, and
  the parent snapshot MUST stay byte-stable

#### Scenario: invalid state transition

- GIVEN a candidate whose step counter decreases or whose moments have the wrong
  arity
- WHEN admission checks the state-transition predicate
- THEN the candidate MUST be refused and the refusal MUST NOT be reported as a
  budget refusal

### Requirement: NGRAPH-OPT-04
r[NGRAPH-OPT-04]

An admitted candidate MUST produce fresh immutable parameter and optimizer-state
snapshots bound to the exact optimizer code identity, parent parameter and state
identities, graph/loss/derivative identities, seed, data order, numeric semantics
and tool revisions. Previously published snapshots MUST remain byte-stable.
Whole-training rollback MUST be host custody: RNG state, data cursor and counters
MUST be restored together with parameters and optimizer state, an unavailable
checkpoint MUST fail closed, and append-only attempt records MUST survive the
rollback.

#### Scenario: a later-update fault restores the whole trial

- GIVEN a rejected trial that fails after several accepted updates
- WHEN the host rolls back
- THEN parameters, optimizer state, RNG state, data cursor and counters MUST be
  restored together, MUST NOT be partially restored, and the attempt records MUST
  remain

#### Scenario: unavailable checkpoint

- GIVEN a required checkpoint that is unavailable
- WHEN a rollback is requested
- THEN the rollback MUST fail closed and MUST NOT resume from a partial snapshot

### Requirement: NGRAPH-OPT-05
r[NGRAPH-OPT-05]

Evaluation MUST run on frozen exported artifacts outside proposer and candidate
authority. Held-out data, evaluator internals and secrets MUST be inaccessible to
the proposer and the candidate, and evaluation-tampering controls MUST detect an
attempt to reach them. Any comparison against a tuned fixed optimizer or a
schedule search MUST be preregistered over at least three paired seeds with equal
total budgets including proposal cost, admission, checkpointing and failed trials,
and MUST be reported separately from semantic validation.

#### Scenario: proposer reaches for held-out data

- GIVEN a candidate or proposer that attempts to read held-out data, evaluator
  internals or secrets
- WHEN evaluation runs
- THEN the attempt MUST be refused and recorded, and no evaluation result MUST be
  produced from the tampered lane

#### Scenario: performance is not semantic validation

- GIVEN a completed semantic validation of the selected slice
- WHEN a stop/continue recommendation is recorded
- THEN any performance comparison MUST remain a separate preregistered experiment
  and MUST NOT be cited as evidence for the semantic claim
