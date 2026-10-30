## Context and source grounding

This package is grounded at worktree HEAD `901d0a8c9eb37b74876937e0eb6edc08b073646a`.
The #73 design package is present at `.cairn/changes/typed-chunk-completion/`;
its TCHUNK-01 through TCHUNK-04 are the normative input, but #73 Phase 2 is
not complete. No implementation, training run or benchmark exists to preserve.

The real boundaries named by issue #74 are concrete. `crates/noble-ngraph/src/lib.rs`
exports the checked graph, evaluation, snapshots and training modules and says
that F32 graphs are authored as `ngraph-ir/0` and admitted by `check`; it does
not provide source extraction for F32. `ir.rs` defines `SlotKind::Param`, the
F32 operation set and producer metadata that the checker re-derives. `grad.rs`
provides `grad::admit`, reverse mode and explicit `BooleanPolicy`/
`IndexPolicy`; default hard-boolean and index paths refuse rather than silently
pretend to be exact derivatives. `compiled.rs` schedules a checked graph into
fixed dependency layers and packed registers. Its `F32Select` instruction reads
values from the register file after both branches have been scheduled; a select
chooses between values already computed. Therefore a select gate does **not**
skip branch computation and cannot establish sparse-compute savings.

`crates/noble-contracts/src/source/editor.rs` is editor-only transport. It
represents `Hole`, computes `HoleConstraint` and documents that
`required_output_suffix` is only guaranteed if the known incoming suffix is
retained. The scorer therefore consumes facts from the completed #73 request
and partial queue state, never editor syntax as an alternate authority. Each
proposal still follows #73's ordinary source preparation and kernel acceptance.

## Decisions

### Decision: a frozen corpus and provenance split

The experiment freezes a manifest named `tchunk-ranking-corpus/1` containing
480 pure, acyclic I64/Bool one-hole tasks and the exact #73 library snapshot
identity. The five task families have 96 tasks each:

| Family | Count | Required coverage |
|---|---:|---|
| arithmetic composition | 96 | `+`, `-`, `*`, wrapping boundaries and operand order |
| ordered stack routing | 96 | `dup`, `drop`, `swap`, `over`, nonempty tails |
| Boolean composition | 96 | `not`, `and`, `or`, both Boolean results |
| nested quotation | 96 | bounded quotation depth and `run` recipes |
| equal-signature contrast | 96 | same signature with deliberately different behavior |

Each family is authored in twelve provenance blocks of eight tasks. A block has
one source method and template namespace; its canonical editor tree, expected
ordered interface, independent fixture expectation, recipe/library identity and
source revision are retained. Blocks 00--06 (56 tasks per family, 280 total)
are training, blocks 07--08 (16 per family, 80 total) are validation, and
blocks 09--11 (24 per family, 120 total) are held-out test. The split is by
family and provenance block, never by randomly shuffling rows. A block's
near-duplicate template, generator seed, source revision and chunk-library
revision cannot occur in another split. The test manifest and expected values
are sealed before scorer training and held-out evaluation.

The host freezes the manifest digest, task bytes, expected stacks/effects,
independent fixture values, source revisions, chunk identities, library
revision, tool revisions and split assignment. Exact canonical source/tree
bytes, normalized recipe bytes, ordered interface and expected fixture trace
are deduplicated within and across families. A row is removed as a duplicate
when all of those values match; semantically equivalent templates identified
by normalized recipe and trace are retained once, with the earliest manifest
identifier. If equivalence cannot be established without executing a candidate,
it is not inferred. Test answers, reference implementations, fixture traces,
chunk bodies, chunk identities and test provenance are never copied into
features, target generation code available to the scorer, model-selection logs
or the training queue. The scorer sees no held-out data or evaluator state.

The corpus is not generated in this design package. Phase 2 must materialize
exactly this manifest from #73's independently supplied task constraints and
library. A missing, changed or stale manifest/library identity is an invalid
experiment, not permission to substitute a convenient corpus.

### Decision: bounded feature vector

After #73 validates the request and candidate metadata, each queue entry is
encoded as exactly 64 finite F32 lanes. Values are exact binary32 values after
saturating each nonnegative count to its declared maximum and dividing by that
maximum; booleans are `0` or `1`; categorical types use one-hot lanes. No lane
contains source bytes, expected values, fixture outputs, test membership,
chunk identity bytes, model identity or an evaluator result. The fixed layout
is:

| Lanes | Fields (in order) |
|---|---|
| 0--3 | request input height, output height, hole depth, prefix node count |
| 4--7 | I64 inputs, Bool inputs, I64 outputs, Bool outputs |
| 8--11 | incoming suffix height, required output suffix height, known-effect count, unresolved-effect bit |
| 12--15 | partial sequence length, current queue ordinal, current stack height, residual output height |
| 16--23 | candidate chunk count, expanded-node count, quotation depth, capture count, dependency count, instantiation count, source-byte count, candidate recipe-byte count |
| 24--31 | literal-node count, stack-routing count, I64-arithmetic count, Boolean-op count, quotation-node count, `run` count, exact-stack-match bit, effect-match bit |
| 32--47 | for each of up to four chunk positions: input height, output height, I64-type count and Bool-type count |
| 48--55 | remaining candidate, expansion, checking-work, source-byte, fixture-step, diagnostic and retained-byte fractions, plus library-revision-match bit |
| 56--63 | profile-match bit, format-match bit, semantic-revision-match bit, body-metadata-match bit, dependency-metadata-match bit, instantiation-metadata-match bit, fresh-proposal bit, reserved zero |

The last metadata lanes are required to be `1` for a rankable entry; a failed
identity or metadata check is rejected before scoring, and is also exercised by
controls. Thus these lanes cannot authorize stale or forged chunks. A feature
encoder failure, NaN or infinity is an invalid scorer input and invokes the
same deterministic fallback as a nonfinite score. Feature extraction cost is
charged to the shared #73 budget and reported separately; it is not hidden in
queue work.

The vector is deliberately about the request, partial state and admitted
candidate metadata. It is not a semantic oracle: equal-signature chunks remain
distinct queue entries, and the scorer cannot observe their behavior or answer.

### Decision: fixed scorer, target, loss and training provenance

The scorer is one fixed `ngraph-ir/0` F32 graph with 64 input slots, 65
trainable `Param` slots (64 weights and one bias), 64 F32 multiplies and an
ordered F32-add reduction. Its output is one finite scalar score. No
`F32Select`, index, step, guard, host call or data access occurs in this graph.
The graph identity, IR revision, checker revision and packed-layout/compiler
revision are retained in the model snapshot. The graph is admitted by the
existing ngraph checker; its trainable slots are those represented by
`SlotKind::Param` in `ir.rs`.

For each training candidate, the independent runner records target `y = 1.0`
exactly when the candidate is accepted by the normal #73 path **and** its
independent finite fixture behavior agrees; otherwise `y = 0.0`. The target is
computed only for training blocks. The loss graph is fixed mean squared error,
`(score - y) * (score - y)`, with one F32 subtraction and one F32 multiply per
row, reduced in stored order. It is a ranking surrogate, not a proof and not a
semantic replacement for acceptance.

Training uses `grad::admit` with the default exact-real derivative policy:
there are no Boolean or index derivatives and no surrogate opt-ins. It uses
existing `adam/0` with exact binary64 hyperparameter bits:
`lr=0x3f947ae147ae147b`, `beta1=0x3feccccccccccccd`,
`beta2=0x3feff7ced916872b`, `eps=0x3e45798ee2308c3a`, no weight decay,
clipping or clamp. Parameters and binary64 Adam state are immutable snapshots;
new steps produce fresh snapshots. The fixed training order is the canonical
training-manifest order, batch size 32, 256 optimizer steps, and training seed
`0x74031a7e20261029` under the existing `splitmix64/0` provenance. No
validation or test row is used to update parameters.

Three such fixed-seed training attempts are allowed only to make the
pre-registered selection rule reproducible: seeds
`0x74031a7e20261029`, `0x74031a7e2026102a` and `0x74031a7e2026102b`. Select the
lowest validation mean loss; ties within exact F32 equality select the lowest
seed. Freeze that complete model snapshot, parameter bytes, optimizer identity,
feature revision, graph identity and provenance before test evaluation. The
other snapshots are retained as non-selected training artifacts, not tried on
test tasks. Training cost is never charged as inference work.

### Decision: queue-only authority and failure behavior

For every task, all modes receive the same request, exact chunk snapshot,
independent expectations, source-preparation limits, kernel limits, fixture
limits and total shared cap. #73 enumerates the finite eligible candidate queue
in deterministic shortlex order. The learned mode computes a score for each
eligible entry and reorders only that already finite queue by descending finite
score, then ascending original shortlex ordinal for exact ties. It cannot add,
remove, mutate, synthesize or skip a candidate; cannot alter the expected
stack/effects, checker/profile, proof obligations, allowed effects, caller
budget or evaluation policy; and cannot decide acceptance. The controller
charges every feature, score, queue operation, source preparation, check and
fixture step. Exhaustion remains `Exhausted`, and no-result remains `NoResult`.

A nonfinite score or feature is ordered after all finite scores with key
`(invalid-score, original-ordinal)`. If every score is invalid, the queue is
exactly the #73 shortlex queue. A stale model identity, stale feature/graph/
chunk-library identity or incompatible parameter snapshot disables learned
ordering and records `Invalid(scorer_provenance)` while running the deterministic
fixed-chunk queue as the explicit fallback control. This fallback is not a
learned result. An incorrect chunk body, interface, dependency, capture,
instantiation, source/semantic revision or claimed digest is rejected by #73
before feature extraction. An exhausted shared budget never falls back to
additional work or partial publication.

The compiled scope is intentionally narrow. `compiled.rs` schedules a fixed
DAG into layers and its select instruction chooses between values already
computed. Adding a select/step gate to the scorer or controller does not skip
expensive branch execution and does not prove sparse-compute savings. This
experiment therefore leaves the compiled graph topology, executor, backend and
source admission unchanged and places ranking outside the search work it
prioritizes.

### Decision: matched modes and measurements

The four modes use identical task order, chunk eligibility, source/kernel/
fixture policy, total cap and report schema:

1. **Primitive-only**: the deterministic primitive-only search from #73.
2. **Fixed-chunk**: #73's deterministic shortlex search over the frozen chunk
   library; this is the primary comparator.
3. **Learned ranking**: the fixed chunk queue reordered only by the frozen
   scorer above.
4. **Shuffled control**: the same fixed chunk queue permuted by
   `splitmix64/0` with the evaluation seed, with no score or feature access.

Per task and aggregate reports retain structural acceptance, finite behavioral
fixture success, and any independently replayed behavioral check as distinct
outcomes. They also retain no-result, invalid, unsupported and exhausted
counts; candidates proposed, accepted, refused and checked; expanded nodes;
dependency and instantiation work; source-preparation work; kernel checking
work; fixture execution steps; maximum expanded program size; and total shared
work. Latency is monotonic wall time from request receipt through final outcome
and includes feature extraction, scorer evaluation, queue management, model
load (cold and warm reported separately), source preparation, checking,
behavioral evaluation and report encoding. Report median, p95 and per-task
paired deltas, not only a selected subset. Report peak scratch bytes and peak
retained bytes, including model and feature buffers.

Offline training reports elapsed CPU time, optimizer steps, training rows,
peak training memory, parameter/state bytes and model-selection time
separately. It may be amortized only under a stated deployment count; no total
savings claim may fold training into inference or omit it.

Use ten paired evaluation repetitions over all 120 test tasks. Controller seeds
are `0x7403000000000001` through `0x740300000000000a`; the learned model and
all task order remain frozen, and only the shuffled control's permutation uses
that repetition seed. Report arithmetic medians and paired bootstrap 95%
intervals over task-level deltas, with family-level results for all five
families. No task, family, repetition or metric may be dropped after seeing
results.

### Preregistered decision rule

The following rule is frozen before opening held-out test results. Let `W` be
per-task total charged work and `L` scorer-inclusive warm latency, both compared
with the fixed-chunk baseline on the same task and repetition. The learned
ranking is **GO** only if every condition holds:

- median paired `W` is at least 15% lower, with the paired-bootstrap 95% lower
  bound on the reduction above 5%;
- median paired warm `L` is at least 10% lower, with its 95% lower bound above
  0%;
- finite behavioral success is no more than 1 percentage point below the
  fixed-chunk mode overall and no more than 5 points below it in any family;
- structural acceptance and independent replay outcomes are not worse than
  fixed-chunk by more than 1 point, each reported separately;
- peak retained bytes are no more than 1.25 times fixed-chunk, and invalid
  score/fallback rows are no more than 1% of candidate rows;
- the learned result beats the shuffled control on both primary reductions;
  otherwise fewer proposals are not treated as a useful improvement.

Failure of any condition is **NO-GO**: retain the simpler fixed-chunk baseline
and do not widen the scorer, corpus or search. The rule distinguishes fewer
proposals from lower total work and finite fixture success from proof. A
negative result is an expected valid outcome.

## Controls and proof obligations

Phase 2 must run all controls under the same report and shared cap:

- force NaN, positive infinity and negative infinity scores, and nonfinite
  feature lanes; verify deterministic fallback and unchanged acceptance;
- supply adversarial rankings (reverse queue, always-last successful candidate,
  and equal scores); verify only finite queue reordering and stable tie-breaks;
- change scorer graph, parameter, feature revision, model snapshot, optimizer
  provenance and #73 library identity; verify stale invalidation and fallback;
- mutate chunk body, capture, dependency, source/semantic revision, ordered
  interface, effect metadata and claimed identity/digest; verify #73 rejection;
- exhaust candidate, checking-work, expansion, fixture-step and retained-byte
  budgets; verify fail-closed exhaustion rather than fallback or truncation;
- use equal-signature chunks with opposite behavior; verify both remain entries,
  behavior is checked independently and signatures do not leak into scores;
- verify source preparation, contracts, allowed effects, checker/profile,
  proof obligations and caller budgets are byte-for-byte policy-equivalent in
  every mode;
- verify cold-load and warm-operation reports, scorer-inclusive latency and
  separately retained training cost; compare all five families and all ten
  repetitions.

This design does not prove a model theorem, scorer correctness, behavioral
completeness, semantic equivalence, sparse execution or generalization. Future
implementation evidence must separately show that #73 re-admits each proposal,
that shared counters are not reset at chunk boundaries, that model snapshots
are immutable and replayable, and that fixture agreement is not promoted to
proof. The ngraph graph and gradient policies are reused as existing contracts;
this package does not create a second training substrate.

## Acceptance mapping

| Issue #74 acceptance criterion | Design location |
|---|---|
| bounded runner, three baselines and held-out expectations | corpus and matched modes |
| unchanged checkers/contracts/effects/budgets; scorer not authority | queue-only authority and controls |
| provenance, leakage controls, deterministic fallback | corpus, feature vector and failure behavior |
| scorer-inclusive work/latency/memory and training cost | measurements |
| repetitions, seeds and no cherry-picking | measurements and decision rule |
| invalid/adversarial/stale/metadata/budget/equal-signature controls | controls |
| explicit go/no-go and finite-success/proof distinction | preregistered decision rule |
| compiled select does not skip computation | compiled scope decision |

## Non-claims

No implementation, training, benchmark, speedup, completeness, proof, semantic
equivalence, learned chunk discovery, parser learning, conditional execution,
GPU acceleration, topology learning or production adoption is claimed. The
Cartesia H-Net/BLT analogy motivates bounded effort allocation only; it is not
reproduced and its external compute results are not Noble evidence.
