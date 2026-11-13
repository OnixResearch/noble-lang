## ADDED Requirements

### Requirement: NGRAPH-BELT-01
r[NGRAPH-BELT-01]

The opt-in `ngraph-belt/0` backend MUST admit only checked total I64/Bool graphs:
I64 inputs, I64 constants, wrapping I64 addition/subtraction/multiplication,
I64 equality, and ordered graph outputs. Graph facts/guards, F32 nodes, parameter
slots, unsupported operations, and profile mismatches MUST be refused before
lowering. Each artifact MUST declare a belt capacity in `2..=64` and at most
8 explicit preservation slots; capacity 8 is nominal, not mandatory. The
backend MUST bound graph nodes, outputs, instructions, and encoded bytes, and
MUST refuse rather than truncate or partially emit when any bound is exceeded.

#### Scenario: only the total I64/Bool fragment is admitted

- GIVEN a checked graph using only I64 inputs/constants, wrapping arithmetic,
  equality, and ordered outputs
- WHEN the belt admission boundary runs
- THEN the graph MUST be eligible for lowering only when it has no facts,
  guards, parameters, unsupported node kinds, or non-I64 profile obligations

#### Scenario: unsupported graph features fail before lowering

- GIVEN a graph with a fact/guard, an F32 operation, a parameter, or an
  unsupported operation
- WHEN the belt lowerer receives the checked graph
- THEN it MUST return an explicit refusal and MUST NOT emit a partial artifact

#### Scenario: capacity and explicit preservation bounds are enforced

- GIVEN a capacity outside `2..=64`, more than 8 save slots, or a graph or
  instruction stream above its declared bound
- WHEN the artifact is lowered or decoded
- THEN the request MUST be refused without truncation or partial execution

### Requirement: NGRAPH-BELT-02
r[NGRAPH-BELT-02]

The `ngraph-belt/0` instruction semantics MUST be target-neutral and
unambiguous. Belt age zero MUST name the newest value. Binary operands MUST be
read from the pre-insertion belt in their declared left/right order; reading
MUST NOT consume a value. A definition MUST insert its result at the newest
position and evict the oldest position when full. Preservation MUST use only
explicit `Save(age, slot)` and `Load(slot)` instructions. A live value MUST NOT
be lost by eviction or slot overwrite. Outputs MUST be produced only by
explicit final `Output(age)` instructions and MUST match graph output order,
including repeated outputs. Arithmetic MUST use wrapping 64-bit I64 semantics;
equality MUST produce Bool.

#### Scenario: asymmetric operands are not interchangeable

- GIVEN a subtraction instruction reading two belt ages
- WHEN the graph node is `left - right`
- THEN the checker MUST preserve that operand order and MUST refuse a swapped
  pair of graph-node identities

#### Scenario: reads precede insertion and do not consume operands

- GIVEN an accepted binary operation whose operands are at distinct ages
- WHEN the operation executes
- THEN both operands MUST be read from the same pre-insertion belt and the
  result MUST be inserted only after both reads

#### Scenario: explicit saves preserve live values

- GIVEN a live graph value at the oldest belt position
- WHEN an insertion would evict it
- THEN the checker MUST require an initialized save slot containing that exact
  graph-node identity; implicit spills are forbidden

#### Scenario: final reads preserve exact output multiplicity and order

- GIVEN ordered graph outputs containing a repeated node
- WHEN the belt program completes
- THEN it MUST perform the same number of final reads in the same order and
  return every repeated value

### Requirement: NGRAPH-BELT-03
r[NGRAPH-BELT-03]

A belt artifact MUST be bound to its format, lowerer and checker revisions,
checked graph identity, declared capacity and save-slot count, instruction count,
instruction length, and instruction-byte digest. The binding digest MUST cover
those fields and the instruction digest. The independent checker MUST decode
and validate bounds, revisions, graph identity, operation bodies, operand ages
and order, preservation-slot initialization/overwrite, liveness, completion,
and exact output order. A checker-issued token MUST borrow the exact bytes it
accepted and snapshot their instruction digest and artifact binding; checked
execution MUST recheck both against those snapshots before running. Refusal
MUST produce no output values.

#### Scenario: body bytes changed after encoding are refused

- GIVEN a valid artifact whose instruction byte is changed without recomputing
  the declared instruction digest
- WHEN the checker or checked execution decodes it
- THEN it MUST refuse the artifact before interpreting its instructions

#### Scenario: header fields are bound together

- GIVEN an artifact whose capacity, slot count, graph identity, revision, or
  length is changed without recomputing its binding
- WHEN artifact decoding runs
- THEN decoding MUST refuse the mismatched binding rather than accepting the
  changed declaration

#### Scenario: malformed machine state is fail-closed

- GIVEN a stale age, uninitialized load, live eviction, live-slot overwrite,
  incomplete node stream, or wrong output order
- WHEN the independent checker validates the artifact
- THEN it MUST return a distinct refusal and MUST NOT issue a checked token

### Requirement: NGRAPH-BELT-04
r[NGRAPH-BELT-04]

Evidence for `ngraph-belt/0` MUST keep kernel-checked model theorems,
source-to-artifact correspondence, finite Rust differential tests, and
performance/cost measurements distinct. The finite differential lane MUST
compare real lowered programs with the graph reference, packed `Compiled`,
exact I64 expectations, and ordinary Wasm plus the BigInt oracle where those
paths are available. Cost reports MUST separate admission, lowering, checking,
execution, and output readback, record encoded bytes and preservation traffic,
and report bounded peak storage beside a warm `Compiled` baseline and a direct
scalar baseline. Test or measurement success MUST NOT be called a general proof
or an ISA/hardware result.

#### Scenario: source and finite evidence are reported independently

- GIVEN a Lean soundness theorem, Rust correspondence tests, and finite B2
  comparisons
- WHEN the implementation evidence is summarized
- THEN each evidence class MUST be identified separately and no finite run MUST
  be described as a proof of general implementation correspondence

#### Scenario: costs do not substitute for semantics

- GIVEN measured belt costs and a warm compiled/scalar baseline
- WHEN a result is recorded
- THEN the report MUST retain each stage and resource count and MUST NOT infer
  semantic correctness or target-hardware speed from timing alone
