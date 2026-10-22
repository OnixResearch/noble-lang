## Context

`noble worker typed` runs one transition in the Node/V8 engine. The emitted runtime charges `$runtime_tick` for every dispatched operation, initialization item and captured-data traversal node against a 4,096-step limit, and `$observation_tick` for reflection and type-observation nodes against the same limit on a separate counter. Exhaustion is checked before each charge, so the budget is never exceeded. The transition is one synchronous `submit` call: no host code runs until it returns, so the host can neither cancel it nor apply a deadline inside it, and the engine reports every JavaScript exception, including host bugs, as `trap`. The JavaScript protocol, not the Rust host, chooses the 4,096-step limit.

## Decisions

### Decision: Vehicle

**Choice:** Implement the profile for the local Worker-Typed engine only. Worker Program v2 keeps its Wasmtime fuel and epoch budgets and its modules are unchanged.

**Rationale:** The typed engine runs the MW1 `State Event -- State List<Action>` transitions. Isolating the change behind a compiler mode keeps every other profile's modules byte-identical.

### Decision: Accounting revision and units

**Choice:** Revision `worker-typed/step-4096/interrupt-8`. The units are the existing ones: a dispatched operation, an initialization item or a captured-data traversal node on the execution counter (ceiling 4,096), and a reflection or type-observation node on the observation counter (ceiling 4,096). The Rust host selects the 4,096-step ceiling and sends it with each execution. WORKER-09's `symbolic-test-profile-v1` is the case's harness label; its interval of eight is this revision's interval.

**Rationale:** H-LIMIT-03 requires a named revision. Reusing the existing units keeps exhaustion semantics unchanged.

### Decision: Interruption points

**Choice:** A new worker compiler mode renames the existing charging ticks to `$runtime_charge` and `$observation_charge` and defines `$runtime_tick` and `$observation_tick` as charge-then-check. A shared counter calls the imported `noble/worker_interrupt_check(work)` after every eighth charged unit of either kind, passing the total charged work. The host answers 0 to continue, 1 to cancel (status 7), 2 for a host deadline (status 8); any other answer makes the guest execute `unreachable`. The engine allows this import only for the typed worker, and records the number of checks and the largest work difference between consecutive checks, including the start and the tail after the last check.

**Rationale:** Only a host call can interrupt a synchronous Wasm call. Taking the work value at each check lets the host measure the interval rather than trusting a reported metric. The tail bound covers work after the last check.

### Decision: Outcomes

**Choice:** Status 0 is `normal`; statuses 1 and 2 are `runtime-exhausted` (deterministic work, allocation, recipe or observation limits); 7 is `cancelled`; 8 is `host-deadline`; a `WebAssembly.RuntimeError` is `trap`; any other exception during typed execution is `internal-failure`. The Rust host refuses an observed interval above eight as `internal-failure`, and only `normal` yields a typed output. The production deadline is 60 seconds of wall-clock time, checked only at interruption points and below the 120-second reply deadline that still guards a hung engine.

**Rationale:** H-OUTCOME-01 and H-LIMIT-03 require these outcomes to stay distinct; a host fault must not read as a guest trap.

### Decision: Test controls

**Choice:** In-process callers may select cancel-at-check and fault-at-check (guest trap or host exception) controls. The CLI always uses the production control with neither. A zero deadline exercises the host-deadline outcome deterministically at the first check.

**Rationale:** Pure builtins are total, so a guest trap and a host fault need a host-selected fault point. None of these controls is candidate input.

### Decision: WORKER-09 rows

**Choice:** MW1 rows: finite pure step, domain error, pure-loop work limit (an over-budget non-recursive call tree, since pure source has no unbounded loop), runtime-builder limit (recipe leaves), nested-invocation budget (`run`), cancellation, guest trap and runtime internal failure, plus a zero host deadline. The domain-error row uses a schema whose `Action` has an error arm: the four-type worker catalog does not admit generic `result@1.Result`. The host-deadline-with-native-work and permitted-suspension rows stay MW2 obligations.

**Rationale:** The synchronous typed worker has no native task or suspension; reporting those rows as covered would be false.

## Risks / Trade-offs

- A host check every eight units adds one host call per eight units; budgets of 4,096 units keep that bounded.
- Equal programs need not consume equal budgets across engines; the revision binds this engine only.
- WORKER-09 is not promoted: two rows remain MW2 obligations, and WORKER-11 keeps `bounded-execution-profile` open until milestone review.
