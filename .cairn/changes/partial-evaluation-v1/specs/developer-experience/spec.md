# Numeric graph partial evaluation

## MODIFIED Requirements

### Requirement: NGRAPH-CLI-01
r[NGRAPH-CLI-01]

**NGRAPH-CLI-01.**

`Numeric-Graph-v1` MUST expose exactly the `noble ngraph` subcommands `check`, `extract`,
`eval`, `train`, `verify` and `specialize`.

**Effects.**

- Each subcommand MUST read only the files named by its arguments, each under a stated byte
  bound checked before parsing, and MUST write only its explicitly named output file.
- It MUST NOT spawn a process, execute Wasm or open a network connection.
- The `noble-ngraph` library MUST NOT execute any filesystem, process, environment, clock,
  thread or output effect, and MUST NOT depend on a third-party package.
- Identity digests MUST be computed through a hashing scheme that the composition root
  supplies.

**Reports.**

- Every outcome other than a usage error MUST be reported as a JSON line with the schema
  `noble-ngraph-report/v1`. A usage error MUST print its usage on standard error and nothing
  on standard output.
- A refusal MUST carry one stable code and a stable site. The codes are the D6 codes of
  `NumericGraph-Design`, `E-FORMAT`, `E-SNAPSHOT-INTEGRITY`, `E-SNAPSHOT-ID` and
  `E-SNAPSHOT-REPLAY`. Run-time observations that have no site (host-fuel exhaustion and
  cancellation) and host conditions MAY report a null site.
- A refusal that belongs to admission, snapshot binding or input validation MUST occur before
  any graph node is evaluated.

**Exit status.** The exit status MUST follow the outcome class:

- 0 for `normal`;
- 1 for `trap` (a guard failure or a non-finite update);
- 2 for `reject` (malformed or inconsistent inputs, snapshot binding refusals, usage errors,
  host I/O failures, exceeded input bounds and an existing output path);
- 4 for `unsupported` (effect, resource, dynamic, control, aggregate, reflection,
  Program-output, recursion and default-refused derivative classes);
- 5 for `exhausted` (every `E-BUDGET-<kind>`).

For `eval`, the exit status MUST be that of the first non-normal vector in file order.

**Budgets.** Every D5 budget MUST be settable by an explicit flag whose default is the `/0`
value. A flag MAY only lower a budget; a raised value MUST be refused with `E-PROFILE`.

**Output.**

- For identical arguments and input bytes, standard output MUST be byte-identical across runs.
- An `F32` value MUST be rendered as its exact binary32 encoding, except that every NaN
  encoding MUST be rendered as the single NaN class.
- The compiled evaluator MUST be the default, and the reference interpreter MUST be
  selectable. Both MUST produce identical output except for the evaluator-specific point of
  host-fuel exhaustion.

## ADDED Requirements

### Requirement: NGRAPH-PE-01
r[NGRAPH-PE-01]

`Numeric-Graph-v1` MUST expose bounded static-input specialization of a **checked** graph
through exactly one library entry point in `noble-ngraph` and one `noble ngraph specialize`
subcommand. No source syntax, kernel word, type or effect MAY change, and a specialization
request MUST NOT modify, re-publish or reinterpret the original graph or Program.

**Admitted subject slice.** The entry point MUST accept a checked `ngraph-i64/0` graph whose
interface is flat scalar slots, with no param slot, no interface shape tree, no fact record
and no `Guard` or `F32Index` node. A subject outside that slice MUST be refused with
`E-PROFILE` (profile, param, fact, guard), `E-AGGREGATE` (shape tree) or `E-INTERFACE`
(static-map arity), before any residual node is created.

**Effects.** The specializer MUST perform zero host requests, zero filesystem, process,
environment, clock, thread or output effects, and zero graph-node evaluations. It MUST be
reachable only through the pure library and the existing `noble ngraph` composition root.

#### Scenario: NGRAPH-PE-CASE-01 admitted slice

- GIVEN flat-interface `ngraph-i64/0` graphs extracted from ordinary Core roots and the
  authored flat-interface control graphs, each with a static map of input-leaf arity
- WHEN the entry point specializes each graph
- THEN every request MUST produce exactly one independently checked residual graph, and the
  report MUST name the accounting revision, both graph identities, the accepted static map
  and the node counts

#### Scenario: NGRAPH-PE-CASE-02 refused slice

- GIVEN a non-`ngraph-i64/0` graph, a graph with a param slot, a graph with an interface
  shape tree, a graph with a fact record, a graph with a guard node, and a static map of the
  wrong arity
- WHEN the entry point runs on each
- THEN each MUST report its recorded code with zero residual nodes created and no output

### Requirement: NGRAPH-PE-02
r[NGRAPH-PE-02]

The static map MUST be one optional `I64` per ordered, flattened input leaf (the graph's
`input` slots in slot order), and the residual MUST renumber the remaining inputs by the
number of dynamic input slots strictly below each original slot. The library MUST also expose
the merge function that reconstructs a full original input vector from the static map and a
vector of dynamic inputs, so that a caller can compare the two sides.

**Diagnostics.** A map whose length differs from the input-leaf count MUST be refused with
`E-INTERFACE` and MUST NOT be partially applied.

#### Scenario: NGRAPH-PE-CASE-03 non-prefix static slot

- GIVEN a graph whose only static input leaf is not below every dynamic leaf
- WHEN it is specialized and the merge function reconstructs full input vectors
- THEN the residual MUST index its inputs by the dynamic rank, and treating the static leaf
  as a prefix MUST be reported as a disagreement by the negative control

#### Scenario: NGRAPH-PE-CASE-04 arity

- GIVEN a static map one leaf too short and one leaf too long
- WHEN each is presented
- THEN both MUST be refused with `E-INTERFACE` and no residual

### Requirement: NGRAPH-PE-03
r[NGRAPH-PE-03]

Specialization MUST substitute a static input leaf with an `I64` constant and MUST fold a
node to a constant only when every one of its operands is statically known, and only for
wrapping `I64` addition, subtraction, multiplication and `I64` equality. It MUST NOT
reassociate, commute, apply identity or absorption rewrites, eliminate common subexpressions,
propagate a `Bool` result into an `I64` operand, or reorder nodes. Every node that is not
folded MUST remain residual with its original operation and operand positions.

**Equality extension.** Folding `I64Eq` is not covered by the foundation model's
`specialize_sound`; it is an explicitly recorded extension whose evidence is differential
testing, not proof.

#### Scenario: NGRAPH-PE-CASE-05 folding whitelist

- GIVEN a graph with fully static add, subtract, multiply and equality nodes, mixed nodes and
  a node whose operands are statically known only through a static input
- WHEN it is specialized
- THEN exactly the fully static arithmetic and equality nodes MUST become constants, and no
  other node MUST change operation, order or operand position

#### Scenario: NGRAPH-PE-CASE-06 wraparound

- GIVEN the model's `2^64 - 1 + 1` boundary and a `MIN × -1` multiply
- WHEN the specializer folds them
- THEN the residual constant MUST be the same wrapping value the reference evaluator
  computes for the original graph

### Requirement: NGRAPH-PE-04
r[NGRAPH-PE-04]

A residual MUST be a new graph with its own independently recomputed identity and MUST NOT be
presented as the original. The recorded claim MUST be, for every vector of dynamic inputs,
that the residual's observation and the original's observation under the merged input vector
are equivalent under the profile's observation relation. The implementation MUST NOT assert
that relation as verified; differential tests are finite evidence, not a proof, and no
Program-level refinement claim is made.

**Pruning.** A node that reaches no output MAY be removed and the residual MAY be renumbered
densely, but only because the admitted slice contains no failing node; the derivation record
MUST report how many nodes were folded and pruned.

#### Scenario: NGRAPH-PE-CASE-07 observation agreement

- GIVEN the fixture graphs, seeded Core programs, static maps and input vectors of the
  differential protocol
- WHEN the original is evaluated under the merged vectors and the residual, and the graph
  re-admitted from the residual's source, are evaluated under the dynamic vectors by both
  evaluators
- THEN every pair MUST agree under the observation relation, and the recorded counts MUST
  name the vectors and graphs compared

#### Scenario: NGRAPH-PE-CASE-08 distinct identity

- GIVEN one original graph and any static map
- WHEN the residual is produced
- THEN its subject identity and logical graph identity MUST differ from the original's, its
  closure MUST be the original's, and the derivation record MUST name both identities

### Requirement: NGRAPH-PE-05
r[NGRAPH-PE-05]

Specialization MUST be bounded under the named accounting revision `ngraph-pe/0`, with two
budgets: the number of residual nodes created before pruning and the number of residual
variants per request, each defaulting to the `/0` value and each lowering-only through the
existing policy rule. The residual MUST additionally be charged the ordinary checking budgets
when it is re-checked. The profile MUST NOT claim that the original and the residual consume
equal budget, fuel or work: the two sides have different interfaces and visit different
nodes, and the measurement MUST report both sides separately.

#### Scenario: NGRAPH-PE-CASE-09 budget ladder

- GIVEN a fixed graph and static map
- WHEN the residual-node cap is one below the created-node count, exactly equal to it, and
  one above it, and the variant cap is zero, exactly one, and one above the `/0` value
- THEN the first MUST be refused with `E-BUDGET-RESIDUAL-NODES`, the second and third MUST be
  accepted, the variant cap of zero MUST be refused with `E-BUDGET-VARIANTS`, its exact `/0`
  value MUST be accepted, and every raised cap MUST be refused with `E-PROFILE`

#### Scenario: NGRAPH-PE-CASE-10 separate accounting

- GIVEN one measurement record of an original and its residual
- WHEN the record is read
- THEN it MUST carry the two node counts, the folded and pruned counts, per-side evaluator
  node visits, retained bytes, cold first-evaluation time and warmed batch time separately,
  and it MUST NOT carry a normalized or equal-fuel figure

### Requirement: NGRAPH-PE-06
r[NGRAPH-PE-06]

The residual MUST be emitted as ordinary Noble source and MUST be independently re-accepted
by the ordinary frontend and kernel before it is published. Emission MUST produce one
`let`-binding per residual node in canonical order, bind the dynamic inputs first, read
outputs in output order, and render `I64` constants as exact decimal text and `Bool`
constants as `true` or `false`. A re-preparation or re-admission refusal MUST be reported as
that refusal, and then no residual file, graph file or source file may be created.

The specialization path MUST perform zero host requests: the library MUST execute no host
effect, and the command MUST spawn no process, execute no Wasm and open no network connection.
Every `noble ngraph` report MUST expose `guest_requests: 0`, including an extraction refusal,
so the existing host-request accounting makes the no-request result observable.
Effectful, resource, recursive, aggregate and control subjects MUST be refused by the
profile's extraction stage at or before admission, with no graph, no residual and no published
output, and such a subject MUST NOT be able to reach specialization at all: specialization
accepts only an already admitted graph.

#### Scenario: NGRAPH-PE-CASE-11 residual source acceptance

- GIVEN a specialized residual and its emitted source
- WHEN the source is prepared with the ordinary declared-text frontend under the ordinary
  limits and its submission is re-admitted and re-extracted
- THEN the re-admitted graph MUST be checked and MUST agree with the residual under the
  observation relation on the protocol's vectors

#### Scenario: NGRAPH-PE-CASE-12 zero host requests

- GIVEN an effectful subject, a resource-interface subject and a recursive subject
- WHEN the profile's extraction stage runs on each
- THEN extraction MUST refuse each with `E-EFFECT`, `E-RESOURCE` or `E-RECURSION` before any
  graph exists, the profile MUST create no graph file, residual or output, and specialization
  MUST be unreachable because its only entry takes an admitted graph; any CLI report MUST
  record `guest_requests: 0`

### Requirement: NGRAPH-PE-07
r[NGRAPH-PE-07]

Every residual request MUST return a derivation record naming the accounting revision, the
original graph identity, the residual graph identity, the accepted static map, the residual
interface types, the node, folded, pruned and residual counts, and the variant count; the
command MUST additionally name the BLAKE3 digest of the residual source (`source_blake3`) and
the re-admitted graph identity.
A consumer MUST be able to recompute every identity in the record from the files it reads,
and MUST NOT rely on the record's self-description as evidence of the relation between the
two graphs.

#### Scenario: NGRAPH-PE-CASE-13 derivation record

- GIVEN one specialized graph and its published residual source and graph file
- WHEN the record is compared with freshly recomputed identities
- THEN every named identity MUST match its recomputation, and a record whose static map,
  accounting revision or original identity is changed MUST NOT reproduce the residual
  identity

#### Scenario: NGRAPH-PE-CASE-14 cache-key inputs

- GIVEN two requests that differ only in the static map, and two that differ only in the
  original graph
- WHEN their derivation records are compared
- THEN both pairs MUST differ in the residual identity and in the binding fields, so that a
  later cache cannot admit one for the other
