# Numeric graph partial evaluation

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
  one above it, and when the variant cap is zero
- THEN the first MUST be refused with `E-BUDGET-RESIDUAL-NODES`, the second and third MUST be
  accepted, the fourth MUST be refused with `E-BUDGET-VARIANTS`, and every raised cap MUST be
  refused with `E-PROFILE`

#### Scenario: NGRAPH-PE-CASE-10 separate accounting

- GIVEN one measurement record of an original and its residual
- WHEN the record is read
- THEN it MUST carry the two node counts, the folded and pruned counts and per-side evaluator
  node visits separately, and it MUST NOT carry a normalized or equal-fuel figure

### Requirement: NGRAPH-PE-06
r[NGRAPH-PE-06]

The residual MUST be emitted as ordinary Noble source and MUST be independently re-accepted
by the ordinary frontend and kernel before it is published. Emission MUST produce one
`let`-binding per residual node in canonical order, bind the dynamic inputs first, read
outputs in output order, and render `I64` constants as exact decimal text and `Bool`
constants as `true` or `false`. A re-preparation or re-admission refusal MUST be reported as
that refusal, and then no residual file, graph file or source file may be created.

The specialization path MUST perform zero host requests: effectful, resource, recursive,
aggregate and control subjects MUST be refused at or before admission, with no graph, no
residual and no published output.

#### Scenario: NGRAPH-PE-CASE-11 residual source acceptance

- GIVEN a specialized residual and its emitted source
- WHEN the source is prepared with the ordinary declared-text frontend under the ordinary
  limits and its submission is re-admitted and re-extracted
- THEN the re-admitted graph MUST be checked and MUST agree with the residual under the
  observation relation on the protocol's vectors

#### Scenario: NGRAPH-PE-CASE-12 zero host requests

- GIVEN an effectful subject, a resource-interface subject and a recursive subject
- WHEN the specialize command runs on each
- THEN each MUST be refused at the admission stage with `E-EFFECT`, `E-RESOURCE` or
  `E-RECURSION`, the ordinary-path reports for the same sources MUST record zero guest and
  zero host requests, and no output file MUST exist afterwards

### Requirement: NGRAPH-PE-07
r[NGRAPH-PE-07]

Every residual request MUST return a derivation record naming the accounting revision, the
original graph identity, the residual graph identity, the accepted static map, the residual
interface types, the node, folded, pruned and residual counts, and the variant count; the
command MUST additionally name the residual source digest and the re-admitted graph identity.
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
