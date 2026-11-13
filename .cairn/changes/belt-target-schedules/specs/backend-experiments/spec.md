## ADDED Requirements

### Requirement: NGRAPH-BELT-TARGET-01
r[NGRAPH-BELT-TARGET-01]

A target schedule MUST name an immutable versioned machine descriptor. The
initial descriptor family MUST be `ngraph-belt-machine/0`, with T1 and T2
profiles having these fixed properties:

| Property | T1 | T2 |
|---|---:|---:|
| issue width | 1 | 2 |
| belt capacity | 4 | 8 |
| save slots | 8 | 8 |
| pipelined ALU units | 1 | 2 |
| pipelined multiply units | 1 | 1 |
| pipelined move units | 1 | 2 |
| ALU/equality latency | 1 cycle | 1 cycle |
| multiply latency | 3 cycles | 2 cycles |
| move latency | 1 cycle | 1 cycle |
| initiation interval of every unit | 1 cycle | 1 cycle |

I64 addition, subtraction, and equality MUST use an ALU unit; multiplication
MUST use a multiply unit; input/constant materialization and explicit save/load
MUST use a move unit. Latency and initiation interval MUST NOT be inferred from
an implementation or changed by an unversioned configuration.

#### Scenario: target descriptor identity controls legality

- GIVEN a T1 or T2 schedule
- WHEN an independent checker resolves its machine descriptor
- THEN the descriptor's issue width, belt/save bounds, unit counts, latencies,
  and initiation intervals MUST match this versioned table exactly

#### Scenario: schedule requests an unsupported resource or capacity

- GIVEN a schedule with an unknown machine id, out-of-range unit id, or a
  capacity declaration different from its descriptor
- WHEN legality is checked
- THEN the checker MUST refuse it rather than infer or silently substitute
  another machine model

### Requirement: NGRAPH-BELT-TARGET-02
r[NGRAPH-BELT-TARGET-02]

A target schedule MUST declare a nonnegative issue cycle, issue lane, and
physical unit slot for each issued operation. Issue lanes MUST be in
`0..issue_width`; no lane or physical unit may issue more than once in one
cycle. Each operation MUST use a unit of the class assigned by the descriptor.
Each physical unit has initiation interval one, so a pipelined unit may accept
one new operation each cycle even while earlier operations remain in flight.
An operation of latency `L` issued in cycle `c` completes at the end of cycle
`c + L - 1`.

All operations issued in one cycle MUST read operands from the same belt state
at the start of that cycle. They MUST NOT observe one another's results or
same-cycle completions. Completion events are applied at the end of each cycle
in ascending `(completion cycle, issue cycle, physical unit slot)` order. Each
completion inserts its result at belt age zero and evicts the oldest value if
full. Ties MUST therefore have deterministic insertion order independent of
list representation. Explicit save/load operations are move-unit operations
and are subject to the same issue and completion rules. Final output reads
occur only after all scheduled work and completions; they MUST match the graph
output sequence exactly, including duplicates.

#### Scenario: unequal latencies reorder completion

- GIVEN operations issued in different cycles whose latency makes the later
  issue complete first
- WHEN the schedule is simulated
- THEN completion order MUST follow completion cycle before issue cycle and
  MUST NOT be forced to source-graph order

#### Scenario: same-cycle operations cannot forward results

- GIVEN two dependent operations assigned to the same issue cycle
- WHEN each operation's operands are read
- THEN both MUST read the pre-cycle belt and the dependent schedule MUST be
  refused if its producer was not already present there

#### Scenario: simultaneous completions have stable ordering

- GIVEN multiple operations completing in the same cycle
- WHEN their results are inserted
- THEN insertion MUST use ascending issue cycle and physical unit slot, with
  each result becoming the new age-zero value in that order

#### Scenario: idle cycles do not invent completions

- GIVEN a schedule with an empty issue cycle between operations
- WHEN completion and belt state are derived
- THEN the idle cycle MUST have no issued operation and MUST NOT alter the belt
  except for genuine in-flight completions

### Requirement: NGRAPH-BELT-TARGET-03
r[NGRAPH-BELT-TARGET-03]

An independent target-schedule checker MUST validate machine identity,
resource assignment, issue width, unit occupancy, operation latency, operand
identity and pre-cycle ages, completion order, preservation-slot initialization
and liveness, schedule completion, and exact outputs. It MUST reject schedules
that use a value before its completion, implicitly spill a live value, overwrite
a live save slot, violate a unit or issue bound, omit graph operations, or
produce graph outputs in a different order. For every accepted schedule, a
separate semantic-preservation theorem or finite correspondence lane MUST
establish that the final ordered outputs equal the graph reference. A checker
acceptance result MUST NOT be presented as proof of target performance.

#### Scenario: premature use, omission, and capacity pressure refuse

- GIVEN a schedule that reads a not-yet-completed node, omits a required node,
  loses a live value to eviction, or overwrites a live save slot
- WHEN the checker validates it
- THEN it MUST refuse the schedule without producing an accepted result

#### Scenario: accepted schedule preserves graph outputs

- GIVEN a schedule accepted by the independent checker
- WHEN its completed machine state is compared with the graph reference
- THEN every ordered output value MUST match, including duplicate outputs

### Requirement: NGRAPH-BELT-TARGET-04
r[NGRAPH-BELT-TARGET-04]

Target schedule evidence MUST include unequal-latency, completion-reordering,
same-cycle completion, idle-cycle, unit-conflict, issue-width, belt-capacity,
save-slot, and exact-output controls for both T1 and T2. Mutations MUST cover
machine identity, latency, initiation interval, unit id/class, issue lane/cycle,
operand age/identity, completion order, save/load state, and output order. The
report MUST separate the design/model, independent legality check, semantic
correspondence, finite fixtures, and measurements. Timing-model evidence MUST
NOT be described as a hardware result.

#### Scenario: timing mutations are rejected

- GIVEN a T1 or T2 schedule whose latency, initiation interval, issue width, or
  unit assignment is mutated
- WHEN the independent legality checker runs
- THEN it MUST reject every mutation that violates the named descriptor

#### Scenario: design evidence is not a performance claim

- GIVEN a legal schedule under one of the abstract descriptors
- WHEN model or finite-fixture evidence is reported
- THEN it MUST identify the abstract timing model and MUST NOT claim measured
  hardware speed or native ISA compatibility
