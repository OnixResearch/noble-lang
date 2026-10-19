## Context

M5 retains its synchronous memory32 Canonical ABI path. The resource kernel
separates revocation from pinned native retirement, and the authority kernel
consumes a one-shot witness before protected work. M6 now implements the
separate `Component-Async-Bootstrap` boundary and has retained native execution
evidence. Source-bound extraction and its independent fresh check passed;
the corrected full MC2 regression and eleven production Nix checks passed.
Completion requires the separately bound two publication checks and lifecycle
evidence.
Neither the M5 controls, native execution nor extraction substitutes for them.
ND-56–58 constrain this extension.

## Decisions

### Decision: Preserve sequential guest execution

**Choice:** Suspend only at the component boundary. An imported word completes
before the next source word starts. Task records stay host-owned; exposed
future/stream values use the existing non-Data/non-Capture resource discipline,
including when nested in a Noble `Pair`. There is no guest scheduler,
spawn/channel language or new `async`/`await` syntax.

**Rationale:** Standard native async supplies an ABI mechanism, not authority,
cleanup, speculative execution or permission to retry.

### Decision: Separate deterministic decisions and engine execution

**Choice:** A safe sequential kernel owns Pending/Ready/Delivered/Retiring/
Retired decisions, exact identity checks and bounded reservations. The
[production classifier](../../../crates/noble-kernel/src/async_tasks/schema.rs)
covers all five states and fifteen event constructors, including invalid and
duplicate outcomes; payload guards are not replaced by the 75-pair inventory.
The shell serializes admission, completion, delivery and retirement and owns
physical native objects. Namespace, owner context, generation and native
operation identity are checked before a callback can alter a retained task.
Witness consumption and input ownership remain distinct obligations at the
same protected admission boundary. Late authentic operation success can
justify an operation receipt without uncancelling the invocation.

**Rationale:** Refine the actual decision core through Charon/Aeneas/Lean
without claiming callback authenticity, physical cleanup or engine correctness.

### Decision: Select the executed native ABI

**Choice:** [The pinned selection](../../../verification/m6/pins.json) uses
`noble-test:async-boundary/bootstrap@1.0.0`, memory32/UTF-8 native async
lowering, stackful async lifting, waitable-set/subtask primitives and
`task.return`. `wasm-tools` 1.245.1 enables `cm-async`, `cm-async-stackful`
and `cm-async-builtins`; the independent peer uses Wasmtime and
`wasmtime-wit-bindgen` 40.0.2 with WIT parser/component tooling 0.243.0.
The [emitter](../../../crates/noble-wasm/src/component/emit.rs) and
[suspension helper](../../../crates/noble-wasm/src/component/async.wat) preserve
sequential source execution rather than interpreting source or recipes.
Async results are lifted by `task.return` before guest cleanup; mixed
synchronous members retain their synchronous ABI. Borrow-retaining async
calls remain unsupported and are refused before emission.

The selected world executes `future<result<s64,string>>` and `stream<u8>`
operations, explicit future cancellation and stream read-end closure.
Additional compiled worlds exercise `future<s64>`, nested live-value
eligibility, owned-resource return/domain errors and five-parameter calls.
Stream bytes, producer terminal success/domain error, reader closure and
invocation cancellation have separate dispositions.

**Rationale:** Host-only futures and synchronous lowering cannot establish the
requested boundary. Cooperative scheduling alone cannot establish progress.

### Decision: Bound payloads and isolate interruption

**Choice:** The [driver](../../../verification/m6/peer/src/driver.rs) runs one
cooperative engine thread with 100,000 guest fuel, a 1,000-fuel yield quantum,
fuel exhaustion and epoch interruption enabled. It awaits native task exit
before successful export observation. Blocking work runs on bounded external
workers, not on the cooperative engine thread. The selected linear-memory
limit is 4,194,304 bytes; retained native jobs, live values and observed events
are bounded to 32, eight and 256 respectively.

Ordinary task-table limits are eight tasks, eight terminal-result reservations,
4,096 reserved payload bytes, eight parked payloads, eight native pins, 64
wakeup credits, 128 retirement-work units and 128 admitted generations.
Admission reserves terminal capacity even for an empty result, and its
reservation stays charged until `Finish`. Each selected live future/stream
reserves 128 result bytes and 256 parked bytes split into 128 local and 128
external worker/channel bytes. Stream production uses a one-byte engine
buffer and a 64-byte bounded consumer payload. Backing storage is reserved
before caller custody or authorization is consumed; oversized results cannot
be retained as successful results. These reservations cover payload storage,
not engine allocation, task metadata, worker stacks or whole-process heap use.

The [progress controls](../../../verification/m6/peer/src/progress.rs) are
separate local probes: the runnable-fuel case uses 10,000 fuel; epoch and
blocking-deadline cases use 100,000,000 fuel, with a 100-fuel yield quantum.
The epoch/deadline trigger is three milliseconds and the blocking native job
runs for a configured 60 milliseconds. The gate measures polls and heartbeat
progress, requires the observed longest poll below 100 milliseconds, and
observes interruption acknowledgement before native completion with a pin
still retained. These probe parameters and observations are not a universal
latency or OS scheduling guarantee. Epoch interruption requires the host to
advance the epoch; it is not an autonomous wall-clock deadline.

Cancellation or abnormal exit revokes guest access through the production
table and an invocation-isolated Wasmtime Store. Host state and native jobs
remain outside that Store until actual native stop and accounted settlement.
Wasmtime 40.0.2 supplies no per-task cancellation API for this selection.
Store destruction, a dropped task token and cancellation acknowledgement do
not establish native retirement.

**Rationale:** A bounded stream buffer alone does not bound parked payloads,
native pins, wakeups or cleanup. Revocation must stop guest access without
pretending that an independently blocked native operation has stopped.

## Evidence and closeout boundary

The retained [native receipt](../../../verification/m6/acceptance.json) passes
WI-11, WI-12, WI-16 and WORKER-08: 38 variants, 62 controls, 13 native kernel
tests and 157 recorded commands. [The gate](../../../verification/m6/gate.mjs)
keeps these scopes separate:

- Actual Noble compilation, independent component validation and peer execution
  establish the selected ordered-import, terminal-value and eligibility cases.
- WORKER-08 and lifecycle/schema/progress probes exercise local production
  decisions and the peer; they are not full worker-service execution.
- A separate hand-written clock component executes official
  `wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for` from the same
  pinned Wasmtime source. This is peer-only compatibility, not Noble output
  or stable/full WASI support. Stable `0.3.0` linkage and disabled async or
  stackful engine features are rejected on their declared paths.

The peer independently supplies linking, bindings and typed conversion but
reuses production Noble task/resource/authority decisions. The local strict
14-root correspondence audit and complete 75-pair schema accounting now pass
the [independent source-bound check](../../../verification/m6/extraction.json)
with 176 M6 refusal controls. The
[formal archives](../../../verification/m6/formal-archives.json) retain the
discovery and checked products. These do not establish native host or backend
refinement. The corrected MC2 regression and eleven production Nix checks
passed; the [completion record](../../../verification/m6/evidence.json) binds
the separately passed publication and lifecycle gates.
The current 25-unit source inventory retains
1,939 open authored-body obligations; historical snapshots keep their counts.

## Risks / Trade-offs

- Engine-native stackful async is experimental in the selected engine pin.
  Executed compatibility is bounded evidence, not engine/ABI refinement or
  support for every Component-Draft/WASI interface.
- A cancellation acknowledgement revokes access, not native activity. Retained
  pins and terminal reservations survive until actual completion/cleanup.
- Stream closure, producer completion and invocation cancellation have
  different ownership effects. Buffer capacity alone does not bound parked
  work, retained payloads, wakeups or retirement.
- Extraction is distinct from proved correspondence. Every changed source
  unit remains inventoried even when its theorem obligation stays open.
- Authentic callbacks/facts, physical native release, clocks and OS scheduling
  remain trusted host boundaries. No universal progress, eventual cleanup,
  full MW-WORKER service or general guest concurrency claim follows.
