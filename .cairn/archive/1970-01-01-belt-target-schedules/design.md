## Scope

This package answers only the target-machine description and legality relation
requested by #114. It depends on the target-neutral graph and artifact contract
in `.cairn/changes/belt-backend-v0`; target schedules are optional
specializations over the same checked graph, never replacements for its
semantic baseline. Phase 1 selected the model; Phase 2 implements an untrusted
list scheduler, independent legality checker, timed interpreter, Lean model,
finite controls, and model-only cost evidence.

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

The Rust artifact has a separate `ngraph-belt-target/0` revision. Its bytes
bind the machine id, complete canonical descriptor bytes and digest, checked
graph identity, event stream and ordered outputs, schedule digest, and artifact
binding. A schedule therefore cannot be reused under a different target id or
silently inherit an edited timing table. This defines the target identity
needed by #33; changing #33's cache keys is outside #114.

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

The present final-read phase is a bounded snapshot, not a streaming drain: the
entire requested ordered output set must fit in the selected belt. The lowerer
and independent checker refuse a wider output set. A future streaming output
protocol would need explicit read/load interleaving and a revised artifact
contract; it is not inferred from this phase's output-order evidence.

The initial producer is a deterministic canonical-order list scheduler with no
search. It waits for issued definitions to complete and explicitly saves or
reloads live values as the belt requires. It establishes a stable baseline for
both fixed profiles, not an optimization-quality claim. The independent
checker consumes encoded events and never calls the producer; it checks the
event/resource/timing/operand/belt/output premises above. Bounded refusal,
producer exhaustion, and checker-work exhaustion remain distinct outcomes.

### Frozen phase-2 cost protocol

Before retaining any measurement, run two fixed graph workloads through both
T1 and T2: (a) six-node `input,input,constant,add,sub,multiply` with the three
arithmetic nodes as ordered outputs; and (b) an eight-node preservation-pressure
graph with two inputs, four constants, `add(input,input)`, then
`add(sum,last_constant)`, and four ordered outputs. Use the Rust test profile
(debug), 20 warm-up calls, then 200 measured lower/check pairs per
machine/workload. Record modeled cycles, issue width and utilization
(`events / (cycles * width)`), save/load counts, event count, scheduler work,
encoded artifact bytes, peak in-flight event records, and mean nanoseconds per
lower and per independent check. These are local software-path and abstract
machine-model measurements only; no hardware or between-machine speed claim is
permitted. Keep the protocol unchanged for the retained receipt; a changed
workload or iteration count is a new measurement.

### Controls and future work

A later implementation must exercise each descriptor against unequal
latencies, out-of-order and simultaneous completions, idle cycles, same-cycle
no-forwarding, issue/unit conflicts, belt pressure, save/load liveness, and
multiple ordered outputs. Mutations cover the descriptor and every legality
field, especially latency, initiation interval, unit identity/class, issue
cycle/lane, completion order, operand ages, save slots, and outputs.

**Phase 2 scope boundary.** Implementation and model evidence cover only the
checked total-I64/Bool graph fragment. Guards, branches, calls, caches,
variable-latency loads, F32 and host effects remain outside the lane. No graph
mutation, optimization search, production backend selection, #33 cache update,
or Rust-to-Lean correspondence is claimed. Measurements are fixed-workload
simulated model results: no physical machine was measured and no hardware
frequency, power, area, or speed claim follows. The machine model is not Mill
ISA compatibility or hardware semantics. #37 remains the interaction-net
owner and #43 remains the guard-aware packed evaluator owner.
