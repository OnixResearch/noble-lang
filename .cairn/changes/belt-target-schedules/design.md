## Scope

This package answers only the target-machine description and legality relation
requested by #114. It depends on the target-neutral graph and artifact contract
in `.cairn/changes/belt-backend-v0`; it does not implement or benchmark a
scheduler. The target schedules are optional specializations over the same
checked graph, never replacements for its semantic baseline.

## Decisions

### Versioned `ngraph-belt-machine/0` descriptors

The design fixes T1 to issue width 1, belt capacity 4, eight save slots, one
pipelined ALU, one pipelined multiply unit, and one pipelined move unit. ALU and
equality latency is one cycle, multiply latency three cycles, and move latency
one cycle. T2 fixes issue width 2, belt capacity 8, eight save slots, two
pipelined ALUs, one pipelined multiply unit, and two pipelined move units. Its
ALU/equality latency is one cycle, multiply latency two cycles, and move
latency one cycle. Every unit has initiation interval one cycle. Descriptors
are immutable/versioned data, not caller-tunable hints.

ALU handles add, subtract, and equality; multiply handles multiply; move
handles input/constant materialization and explicit save/load. The checker
rejects a schedule whose declared machine or bounds differ from its named
descriptor.

### Issue and resource semantics

Every event declares a cycle, issue lane, and globally unique physical unit
slot. Issue lanes are numbered from zero to `issue_width - 1`; a lane and a
physical unit can each accept at most one event per cycle. Distinct pipelined
units can overlap in-flight work. An event issued at cycle `c` with latency `L`
completes at the end of cycle `c + L - 1`.

Within one cycle all issue events read from the same belt snapshot at the
beginning of that cycle. No same-cycle issue or completion can be forwarded to
another operation. At each cycle end, all actual completions are ordered by
ascending `(completion_cycle, issue_cycle, physical_unit_slot)` and inserted in
that order. Each result goes to age zero; a full belt evicts its oldest item.
The total order gives stable final belt state even when several units complete
together. An idle issue cycle creates no events, while prior in-flight events
may still complete.

Input and constant materialization, save, and load all use the move unit and
obey its latency, initiation interval, belt insertion, and preservation rules.
An output is not an issued operation: after the last completion, the checker
reads the requested graph output nodes in declared order. A value held only in
a save slot must be explicitly loaded before that read.

### Independent legality and semantic evidence

The schedule checker is separate from the optimizer and validates the declared
machine, every issue lane and unit assignment, resource occupancy, operation
class, producer completion, pre-cycle operand identity/age, ordered completion,
belt liveness, save-slot state, graph operation coverage, and exact output
sequence. A model theorem or a separate finite correspondence lane must show
that a legal schedule's output values equal the graph-reference values.
Neither checker acceptance nor timing arithmetic proves that a real processor
has these characteristics.

### Controls and future work

A later implementation must exercise each descriptor against unequal
latencies, out-of-order and simultaneous completions, idle cycles, same-cycle
no-forwarding, issue/unit conflicts, belt pressure, save/load liveness, and
multiple ordered outputs. Mutations cover the descriptor and every legality
field, especially latency, initiation interval, unit identity/class, issue
cycle/lane, completion order, operand ages, save slots, and outputs.

This package contains no implementation or measured result. The machine model
is an explicit experiment choice, not Mill ISA compatibility, hardware
semantics, or an assumption about expected speed. #37 remains the interaction
net owner and #43 remains the guard-aware packed evaluator owner.
