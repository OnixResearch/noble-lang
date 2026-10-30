## ADDED Requirements

### Requirement: TCRANK-01
r[TCRANK-01]

The experiment MUST freeze a `tchunk-ranking-corpus/1` manifest containing 480
pure, acyclic I64/Bool one-hole tasks, the exact #73 typed-chunk library
revision and independent expected contracts/fixture values. It MUST contain 96
tasks in each of arithmetic, ordered-stack-routing, Boolean, nested-quotation
and equal-signature-contrast families. Each family MUST have twelve provenance
blocks of eight tasks, with blocks 00--06 for training, 07--08 for validation
and 09--11 held out for test; row-random splitting is prohibited. Exact source,
normalized recipe, interface, expected trace, generator provenance and library
identity MUST be deduplicated, and test answers, bodies, traces and evaluator
state MUST NOT reach features, targets or model selection.

The candidate feature vector MUST be exactly 64 finite F32 lanes containing
only the declared request, partial queue state and admitted chunk metadata:
heights, type counts, depth, sequence/queue position, node/op counts,
capture/dependency/instantiation counts, bounded byte/work fractions, identity
match flags and the declared one-hot type fields. It MUST exclude source bytes,
expected values, fixture outputs, test membership, chunk identity bytes and
evaluator results. Counts MUST be saturating and normalized by frozen maxima;
booleans MUST be exact 0/1. Feature extraction MUST be bounded and charged to
the shared #73 budget.

#### Scenario: provenance split prevents template leakage

- GIVEN two tasks with the same family but different provenance blocks
- WHEN one belongs to the training, validation or test manifest
- THEN its canonical source, normalized recipe, generator provenance and
  expected trace MUST NOT be duplicated in another split, and the test answer
  MUST be unavailable to feature and target generation

#### Scenario: feature extraction cannot observe behavior

- GIVEN two admitted equal-signature chunks with different bodies and behavior
- WHEN their queue entries are encoded
- THEN each MUST receive only the declared metadata fields and the feature
  vector MUST contain no body bytes, fixture output or observed result

#### Scenario: corpus drift fails closed

- GIVEN a changed task, expected contract, fixture trace or #73 library digest
- WHEN the experiment loads the manifest
- THEN it MUST reject the experiment as stale/invalid rather than substitute a
  new corpus or silently reassign a split

### Requirement: TCRANK-02
r[TCRANK-02]

The scorer MUST be one fixed `ngraph-ir/0` F32 graph with 64 input slots, 65
trainable `Param` slots, 64 F32 multiplies and an ordered F32-add reduction to
one score. It MUST contain no select, index, step, guard, host call or data
access. The graph MUST be admitted by the existing ngraph checker and use
`grad::admit` with the default exact-real derivative policy and no Boolean or
index surrogate. The target MUST be `1.0` exactly for a candidate that passes
ordinary #73 acceptance and independent finite fixture agreement, and `0.0`
otherwise, computed only on training blocks. The loss MUST be fixed mean
squared error `(score - target) * (score - target)` in F32 stored order.

Training MUST use existing `adam/0`, with exact binary64 hyperparameter bits
`lr=0x3f947ae147ae147b`, `beta1=0x3feccccccccccccd`,
`beta2=0x3feff7ced916872b`, `eps=0x3e45798ee2308c3a`, no weight decay,
clipping or clamp; batch size 32, 256 steps and `splitmix64/0`. The three
allowed training seeds MUST be `0x74031a7e20261029`,
`0x74031a7e2026102a` and `0x74031a7e2026102b`. The lowest validation mean
loss MUST win, with the lowest seed as the exact tie-break. The selected graph,
feature revision, parameter bytes, optimizer state/provenance and tool
identities MUST be frozen before held-out evaluation; training cost MUST be
reported separately.

#### Scenario: fixed scorer provenance is replayable

- GIVEN a selected model snapshot
- WHEN it is replayed on a feature vector
- THEN graph identity, feature revision, parameter bytes, optimizer identity,
  seed and tool revisions MUST match the frozen provenance before a score is
  used

#### Scenario: default derivative policy is not a semantic shortcut

- GIVEN a scorer graph with a Boolean or index path, or a surrogate derivative
- WHEN ngraph training admission checks it
- THEN it MUST be refused or classified outside this experiment; no surrogate
  policy may be silently treated as an exact derivative

#### Scenario: held-out rows cannot train the scorer

- GIVEN a test task or its independent fixture result
- WHEN training or validation selection runs
- THEN no parameter update, loss, tie-break or model-selection decision may
  read that task or its result

### Requirement: TCRANK-03
r[TCRANK-03]

The learned mode MUST only reorder the finite candidate queue enumerated by #73.
It MUST not add, remove, mutate, synthesize or skip candidates, alter expected
stacks/effects, contracts, allowed effects, checker/profile, proof obligations,
caller budgets or evaluation policy, or become acceptance/proof authority. The
same request, frozen library, ordinary source preparation, kernel acceptance,
fixture policy and shared cap MUST be used by every mode.

Finite scores MUST sort descending, with the original shortlex ordinal as the
stable tie-break. A NaN, positive infinity, negative infinity or feature
encoding failure MUST sort after every finite score using original ordinal. If
all scores are invalid, the queue MUST equal #73's deterministic fixed-chunk
queue. A stale scorer, graph, feature, parameter, optimizer, tool or library
identity MUST be recorded as `Invalid(scorer_provenance)` and use that explicit
fixed-chunk fallback. An incorrect chunk body, capture, dependency,
instantiation, revision, interface, effect or claimed digest MUST be rejected by
#73 before scoring. Shared-budget exhaustion MUST return `Exhausted`, never
fallback work, truncation or partial publication; finite refusal before
exhaustion remains `NoResult`.

#### Scenario: nonfinite scores have deterministic fallback

- GIVEN a queue with finite, NaN and infinity scorer results
- WHEN learned ordering is applied
- THEN finite scores MUST precede invalid scores, each invalid entry MUST keep
  original shortlex order, and acceptance/results MUST be identical to trying
  that reordered queue under #73

#### Scenario: stale model cannot authorize a proposal

- GIVEN a model whose graph, parameter bytes, feature revision or library
  revision differs from the frozen identities
- WHEN a task is requested
- THEN the scorer MUST be marked invalid, the deterministic fixed-chunk fallback
  MUST be selected, and no candidate may bypass ordinary re-admission

#### Scenario: queue exhaustion remains exhaustion

- GIVEN a task for which the next candidate would exceed the shared candidate,
  checking-work, expansion or fixture-step cap
- WHEN the controller charges that step
- THEN it MUST return the #73 `Exhausted` outcome with no added scorer work or
  partial completion, rather than claim `NoResult` or use an unbounded fallback

### Requirement: TCRANK-04
r[TCRANK-04]

The experiment MUST compare all four matched modes: primitive-only, #73
fixed-chunk shortlex, learned ranking over the same fixed chunks and a
splitmix64 shuffled ranking control. It MUST run all ten paired evaluation
seeds `0x7403000000000001` through `0x740300000000000a` over all 120 test tasks,
retain all five family results and prohibit post-result task or repetition
dropping. It MUST report structural acceptance, finite fixture success and any
independent replay result separately, along with no-result/invalid/unsupported/
exhausted counts, candidates proposed/checked, expanded nodes, dependency and
instantiation work, source/kernel work, fixture steps, expanded size, peak
scratch/retained bytes and per-task outcomes.

Latency MUST be monotonic request-to-outcome latency including feature
extraction, scorer evaluation, queue management, cold and warm model loading,
source preparation, checking, behavioral evaluation and report encoding. Median
and p95 latency and paired task-level deltas MUST be retained. Offline
training MUST separately report elapsed CPU time, rows, optimizer steps, model
selection time, parameter/state bytes and peak memory; it MUST NOT be hidden in
inference or uncharged savings.

Controls MUST include NaN/infinity scores and invalid features; reverse,
always-last and equal-score adversarial rankings; stale model/graph/feature/
library identities; changed body/capture/dependency/revision/interface/effect
metadata and forged identities; each shared budget boundary; and equal-
signature chunks with opposite behavior. All modes MUST preserve the #73
checker, contracts, effects, budgets and ordinary source/kernel re-admission.

#### Scenario: scorer overhead is visible

- GIVEN a learned request with a successful or refused candidate
- WHEN its report is emitted
- THEN feature extraction, score evaluation, queue management, source/kernel
  work, fixture work and total scorer-inclusive latency MUST each be present,
  and training time MUST be in a separate record

#### Scenario: controls cannot be cherry-picked

- GIVEN ten seeds, 120 test tasks and five task families
- WHEN results are summarized
- THEN every task and repetition MUST contribute to the declared aggregate and
  family summaries, including regressions, fallbacks, refusals and exhaustion

#### Scenario: equal signatures do not become behavioral labels

- GIVEN two eligible equal-signature chunks with different independent fixture
  behavior
- WHEN all four modes rank and try them
- THEN both entries MUST remain eligible, each MUST be independently checked,
  and no signature match or scorer result may transfer behavioral evidence

### Requirement: TCRANK-05
r[TCRANK-05]

Before held-out evaluation, the experiment MUST freeze this decision rule.
Learned ranking is `GO` only when, against fixed-chunk on paired test tasks,
median total charged work is at least 15% lower with a paired-bootstrap 95%
lower bound above 5%; median warm scorer-inclusive latency is at least 10%
lower with its 95% lower bound above 0%; finite fixture success is no more
than 1 percentage point lower overall or 5 points lower in any family; structural
acceptance and independent replay are each no more than 1 point lower; peak
retained bytes are at most 1.25 times fixed-chunk; invalid-score/fallback rows
are at most 1%; and learned ranking beats shuffled control on both primary
reductions. Any failure is `NO-GO`, retaining fixed-chunk as the baseline.

The decision MUST distinguish fewer proposals from lower total charged work,
finite fixture success from proof, and any replay observation from universal
correctness. It MUST NOT claim a speedup from a compiled select or step gate.
`compiled.rs` schedules a fixed DAG and selects between values already
computed; a select gate MUST NOT be described as skipping branch computation
or proving sparse-compute savings. No conditional-execution rewrite, GPU
backend or topology-learning system is in scope.

#### Scenario: a threshold failure is no-go

- GIVEN a learned mode that reduces proposal count but fails either the 15%
  work reduction, 10% scorer-inclusive latency reduction, confidence bound,
  success, memory, fallback or shuffled-control condition
- WHEN the preregistered decision is applied
- THEN the result MUST be `NO-GO` and the fixed-chunk baseline MUST remain
  preferred; proposal reduction alone MUST NOT qualify

#### Scenario: compiled select does not claim skipped work

- GIVEN a scorer or controller expressed with an ngraph select gate
- WHEN compiled evaluation is inspected
- THEN both branch values MUST still be counted as scheduled/computed under
  the fixed DAG, and no sparse-compute or branch-skipping benefit may be
  attributed to the ranking experiment

#### Scenario: finite success is not proof

- GIVEN all held-out fixture values agree for a mode
- WHEN the report records the result
- THEN it MUST label that finite behavioral success separately and MUST NOT
  claim semantic equivalence, completeness, proof or generalization
