# Bounded sequential I64 tick profile

`noble-tick-i64/v1` is an opt-in host-driven stream over a separately checked acyclic `ngraph-i64/0` DAG. A single step is `(state, input, controls) -> (next_state, visible_output)`. It has no graph feedback edge, asynchronous caller, effects or wall-clock deadline. Its frozen continuation rule is [continuation-criteria.md](continuation-criteria.md); it must be committed before measurements, and measured outcomes must not modify its cutoffs.

## Interface and transition

A partition `(S,I,C,O)` consists of ordered nonnegative counts `state_words`, `input_words`, `control_words`, `output_words`. Graph inputs are exactly `S+I+C` **ordinary I64 Input slots** in contiguous `[state[0..S], input[0..I], control[0..C]]` order; graph outputs are exactly `S+O` ordered I64 positions `[next_state[0..S], visible_output[0..O]]`. Independently check the actual interface, node op whitelist, topology, schedule and limits, not only the caller's declared counts. Repeated references to a graph value in outputs are permitted. Constants are exact signed I64; arithmetic is wrapping `i64.add`, `i64.sub` (left minus right), `i64.mul`. Require `S >= 1`, `I >= 1`, `O >= 1`; `C` may be zero. The arithmetic of a selected graph is total after admission. `Op::Param` is F32-only and is not a control slot here. No specialization, graph-feedback opcode, F32, guards/facts, effects/resources, recursive nodes, dynamic calls, reflection, multi-clock, concurrency or implicit state migration is introduced.

`Runner::prepare(ids, Start { graph, partition, state, controls }, limits)` admits the complete graph, selected schedule, initial state and controls before exposing a stream. Successful prepare sets graph epoch 0, control epoch 0, `next_tick = 0`, retained output `None`. Supported graphs require the first `S+I+C` nodes to be precisely ordered `Input(0..S+I+C)`, with only I64 constants and wrapping Add/Sub/Mul thereafter. The identifiers must distinguish semantic profile, exact graph logical/artifact identity and selected schedule identity; a digest alone is not authority or a semantic checker.

`Runner::tick(expected_graph_epoch, expected_control_epoch, expected_tick, input, &mut eval::Control)` pins all selected identities and state at entry and computes one step. On success it publishes both next state and visible output as **one** transition, increments `next_tick` exactly once, and returns the state/output of that tick; a duplicate sequence/epoch or an interrupted evaluation does not commit.

`Runner::prepare_control_update(&self, graph_epoch, control_epoch, words, ids)` validates and binds inactive controls to the exact graph/control/**tick** selection; holding its token makes no state change. `Runner::activate_control_update(&mut self, token)` rechecks that entire boundary and consumes the token; an intervening tick makes it stale. `Runner::update_controls(expected_graph_epoch, expected_control_epoch, controls, ids: &dyn IdentityScheme)` is the convenience form combining both phases, accepting only between ticks; the identity scheme must match the one selected at preparation. On success it replaces the entire ordered typed control vector and increments control epoch exactly once, even if bytes happen to be unchanged. An attempted stale/malformed/oversized update never changes it.

`Runner::reset(ids, Epochs { graph, controls }, Start { graph, partition, state, controls })` separately admits a replacement graph and initial state/control tuple; callers can use `selection.epochs()` to select the current pair. On success it increments **both** epochs once, sets `next_tick=0`, replaces the graph/schedule and state, and clears retained output. Even when dimensions match, reset cannot carry old state. Refusal leaves the *whole* active tuple and retained last output unchanged. Evaluator-reported cancellation/exhaustion before publication has the same no-commit property; cancellation is sampled at layer starts, so a signal arriving after the last probe is not observed for that tick.

The runner is sequential. There is no opportunity to request a control update or reset inside an executing tick; an external concurrent attempt lies outside this profile and must not be reported as race coverage. Both epoch counters and tick counter refuse an increment that would overflow. `next_tick` is the index of the next tick, initially zero and reset to zero; a successful tick at index `t` leaves `next_tick=t+1`. Stale expected epoch/tick, duplicate tick, wrong partition, invalid graph, malformed data, insufficient work/fuel or storage bounds, cancellation and terminal counter exhaustion are refusal classes, not implicit retry or partial success. Keep the implementation's actual diagnostic detail in observations; the export classes below are a coarse mapping. A failed call can be followed by a valid call against the still-active tuple unless the caller's explicit cancellation control itself is terminal. Only accepted operations advance counters.

### Explicit finite bounds

Admission must enforce caller-provided finite limits no greater than the bounded pilot envelope: at most **16 graphs** across a trace, **4096 DAG nodes per graph**, **256 words** in each of state/input/control/output, **256 total input words** and **256 total graph output words**, **4096 successful ticks per reset epoch**, **65536 operations per exported trace**, and **16 MiB UTF-8 export bytes**. I64 vectors cost eight bytes/word; limit input, output, state and update vectors independently **and** their checked aggregate, without overflow in count×8. Set explicit finite caps on checked instruction/operand/destination counts and work/fuel per tick (at most 4096 node evaluations), selected schedule/workspace scratch, retained graph/control versions (one selected graph/control tuple plus staged replacement, at most two while preparing), output storage, replay observations, and cleanup/drop work; if the crate's existing admission limit is tighter, report that tighter value and refuse, never silently broaden it. One `tick` call performs at most one graph step. A reset and update must stage bounded replacement data before publication and release obsolete storage; no unbounded version history belongs in Runner. Trace caps limit **exports**, not just evaluator heap. Exceeding a bound refuses without changing the selected tuple; when a trace-export cap is hit, the exporter stops rather than emitting a truncated trace as complete.

## Independent observation and controls

The model maintains `(graph_key, graph_epoch, control_epoch, next_tick, state, controls, retained_output)` and independently evaluates ordered graph nodes using signed 64-bit wraparound (`BigInt.asIntN(64, x)`). For every operation compare the **entire** observation tuple, not just final state: accepted ticks must equal every model next-state word and ordered output word; update changes only controls/control epoch; reset changes both epochs and clears output; refusal changes nothing. Refusal observations remain part of the retained trace, and accepted operations following them prove reuse. A packed/reference comparison must also compare the full operation-by-operation tuple against the independent recurrence, not only the two engines against each other.

The finite test family must include the one-state multiply-add, a two-state ordered-output asymmetric subtraction graph, wrapping boundaries, control changes at tick boundaries, and a subtracting replacement reset with same-sized but semantically different state. Seeded faulty selectors should inject and detect (individually) mixed control epochs in one tick, stale graph or schedule, wrong partitions, swapped state/output order, duplicate publication, one-tick-early/late activation and state carried across reset. Name which controls actually use a mutated selector and which are graph/admission/refusal controls. This finite sequential evidence does **not** establish concurrency safety, a Rust refinement theorem or general graph soundness. Model theorems, Rust/model differential evidence, backend evidence and executed artifact identity must be reported separately.

## Compiler-lab export (`noble-tick-export/v1`)

From a **clean committed source tree**, produce and independently replay the
bounded finite trace with the opt-in integration test:

```sh
NOBLE_TICK_EXPORT_PATH='<new absolute trace path outside this repository>' \
  cargo test -p noble-ngraph --test schedule_correspondence \
    tick_runner::export::clean_source_trace_when_requested -- --exact --nocapture
node verification/tick-runner/measure.mjs '<the same trace path>' \
  > '<new replay summary path outside this repository>'
```

`producer_sha256` binds the five Rust producer source files
(`export.rs` and `export/{identity,model,rows,sequence}.rs`) as SHA-256 over
ordered, length-framed relative paths and exact file bytes. The Git tree and
test-executable digests bind the broader source and executed artifact. These
identities do not authorize an artifact or establish its semantic correctness.

The producer supplies UTF-8 NDJSON; the first line is a header, followed by one operation per line, in `seq=0,1,...` order. All I64 values are **canonical decimal strings** (`"0"`, `"-1"`, no `+`, leading zero or `-0`); never use JSON numbers for I64. The bounded independent replay command is `node verification/tick-runner/measure.mjs <trace.ndjson>`, which prints a JSON replay summary on stdout and refuses malformed, oversized, missing-identity or divergent traces. It does not run the Rust producer or time a backend. Copy the original NDJSON plus summary to a retained clean-source receipt; source tree, compiler, exact producer and runner executable bytes/digests, selected schedule and logical graph IDs must be captured by the Rust producer. The Node tool independently derives `export_sha256` for each graph from the UTF-8 bytes of `JSON.stringify({partition,nodes,outputs})` in that exact key order. This is an **export model** byte identity, not the original checked-graph or schedule identity; those must be separately bound and independently admitted by the Rust producer. The report binds raw trace SHA-256 and exact bytes; consumers must compare that digest to the transferred original trace, not trust a filename.

Header shape (illustrative field types, not an observation):

```json
{"schema":"noble-tick-export/v1","profile":"noble-tick-i64/v1","source_commit":"<full Git commit hex>","source_tree":"<full Git tree hex>","rustc":"<pinned rustc version>","rustc_sha256":"<exact compiler executable SHA-256>","cargo_sha256":"<exact Cargo executable SHA-256>","identity_scheme":"<actual scheme name>","producer_sha256":"<64 lowercase hex>","runner_artifact_sha256":"<64 lowercase hex>","graphs":[{"key":"add","graph_logical_id":"<actual checked graph identity>","selected_schedule_id":"<actual selected schedule identity>","export_sha256":"<SHA-256 of canonical export model JSON>","partition":{"state_words":1,"input_words":1,"control_words":1,"output_words":1},"nodes":[{"op":"input","index":0},{"op":"input","index":1},{"op":"input","index":2},{"op":"mul","lhs":2,"rhs":1},{"op":"add","lhs":0,"rhs":3}],"outputs":[4,4]}],"initial":{"graph_key":"add","state":["0"],"controls":["1"]},"limits":{"max_graphs":16,"max_nodes":4096,"max_words":256,"max_events":65536,"max_ticks":4096,"max_work_per_tick":4096,"max_trace_bytes":16777216}}
```

Each graph's `nodes` is topologically ordered; the first `S+I+C` nodes must be `input` in slot order, and later nodes are `const` with `value` or binary `add/sub/mul` with prior node indices. `outputs` contains exactly `S+O` node indices. Reject unsupported operators rather than inventing a model. An exporter may choose a different output set but must preserve the actual checked interface order and prove that this model is a faithful projection of the actual checked graph; model-export hashing alone cannot establish that linkage.

An operation row has `{seq, op, expected_graph_epoch, expected_control_epoch, outcome, observed}`. `tick` adds `expected_tick`, `input` (I decimal strings), and optionally `cancel_before_commit:true` or `work_budget` (nonnegative node-evaluation units for this export model); `update_controls` adds `controls` (C strings); `reset` adds `graph_key`, `state` (new graph S strings), and `controls` (new graph C strings). `outcome` is `{kind:"ok"}` or `{kind:"refused",class:"stale"|"malformed"|"budget"|"cancelled"|"exhausted"|"terminal"}`. `observed` is `{graph_epoch,control_epoch,next_tick,graph_key,state,controls,output}` with `output:null` before the first success and after reset, otherwise O ordered decimal strings. Epochs and ticks are nonnegative safe JSON integers; no JSON coercions. Stale epochs/tick, malformed vector shape/value, cancellation, max-tick exhaustion and model work-budget refusal are independently determined where possible; a Rust-only admission/fuel refusal must additionally carry a separate producer-side witness and cannot be declared independently proven by this generic model. The exporter must not skip refused calls, fabricate synthetic state after a failed producer invocation, or collapse repeated identical outputs. Before sharing with compiler-lab #227, retain source/tool/graph/schedule identities, full raw NDJSON and its digest, limits and scenario generation recipe, plus replay command/result. Finite agreement is not a universal proof of compiler correctness or authorization.

## Measurement and historical decisions

Run identical finite scenarios through Runner, ordinarily repeated checked graph evaluation with host-managed state/controls, and direct native wrapping-I64 code. Keep graph preparation/reset, control admission/activation, first and warm tick, full trace, allocations if actually instrumented, scratch/retained memory and cleanup distinct. The frozen file defines the cost categories, break-even against **both** comparators, thresholds and NO-GO conditions; only the linked decision below reports the measured timing and receipts. #14 Phase C STOP and #41 independent-row batch NO-GO are retained, not reopened by this separate sequential pilot.

The [retained comparison and acceptance decision](decision.md) records the
clean-source trace, independent replay, nine matched benchmark cases and raw
allocation observations. Its frozen continuation outcome is **NO-GO** for
further optimization; this does not change the opt-in protocol or either
historical decision.

The opt-in integration test runs only after source and criteria have been committed cleanly, and writes outside the repository:

```sh
NOBLE_TICK_BENCH_PATH='<new absolute receipt path outside this repository>' \
  cargo test -p noble-ngraph --test schedule_correspondence \
    tick_runner::benchmark::probes::opt_in_source_bound_cost_comparison \
    -- --exact --nocapture
```

The receipt records the compile-time Cargo profile, optimization level and
debug-assertion selection; a missing compiler environment value is explicitly
`unknown`, not silently called optimized. On Linux it also records a bounded
`/proc/cpuinfo` CPU-model read and separate whole-trace CPU nanoseconds from
`/proc/thread-self/schedstat` for the executing test-harness thread where
available; this is not an arm-specific process-wide CPU profiler or a
hard-real-time guarantee.

The receipt retains nine scenarios (A/B/C × 32/512/4096 successful ticks), all ordered initial vectors and operation JSON bytes plus SHA-256, 11 interleaved rounds per arm with every raw event-duration sample, the exact test executable and tool digests, graph/schedule selections, untimed complete observation agreement, and distinct stage summaries. The first round is process-warmup, not OS-cache cold; there are ten warm rounds. It marks allocation counts unmeasured until instrumented. `VmRSS` of this shared test process is merely an after-run observation, **not** an arm-specific peak; unknown comparative memory cannot satisfy the frozen GO rule. The optional 32-tick timing ratio is diagnostic, not a GO requirement. No timing-only result overrides a failed correctness check.

An external allocation probe can run the **same previously built test executable** under a reviewed DHAT/Valgrind binary; use `NOBLE_TICK_ALLOC_ARM=runner|graph|native` and `NOBLE_TICK_ALLOC_LENGTH=0|32|512`, and select `tick_runner::benchmark::probes::opt_in_allocation_observation --exact --nocapture`. Keep each arm/length in a separate process, retain exact executable and profiler identities, raw process totals and relevant program points; subtracting zero-length process totals is only a diagnostic, because fixture and harness allocations remain included. Do not claim per-arm allocator counts from timing receipts, or per-tick allocation freedom from register reuse.
