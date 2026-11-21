# Ordinary Result and deferred pure-join design

## MODIFIED Requirements

### Requirement: DX-PROPERTY-02
r[DX-PROPERTY-02]

Shrinking MUST preserve the property's input domain and reproduce the same failure predicate before accepting a smaller counterexample. Shrinking MUST be bounded and retain the original failure if reduction fails. Reports MUST distinguish passed trials, counterexamples, discarded inputs, exhaustion, unsupported cases, and harness errors.

The first properties cover compatible program composition, retained recipe structure, and result/effect agreement with the reference model for bounded bootstrap cases. Observable disagreement requires a counterexample, not merely an exhausted budget. Seeds alone are insufficient replay records. Random testing does not establish a universal theorem, and a shared implementation bug can invalidate a differential oracle.

An optional later MC2 pure-fragment law experiment MAY add a finite semantic oracle independently implemented from Noble's checker, prover and selected Wasm lowerer. Its declared finite input vocabulary and bounds MUST be recorded, including exact normal-return predicate and preconditions, independent expected values/recipes and the empty host-effect trace, hostile false-law and false-premise controls, a reproducible seed or saved minimized counterexample, and allowed source/recipe-preservation transformations that actually preserve resolved identity. Tests compare this model with actual accepted resource-free programs and separately with externally checked proof/admission outcomes; model agreement MUST NOT become an accepted theorem or release authorization. Preserve the immutable owner law and distinct release gate of VC-OWNER-01. This design takes only finite-model and metamorphic testing methods from pinned qcue; it does not import CUE's open live refinement, higher-rank or impredicative types, proof-time native execution, a second kernel, or its Go dependency. qcue itself reports that raw source export/reimport leaves one separately attached live-interface conformance proof pending. Existing DX-10/M4 property execution remains its historical bounded scope, not evidence for this optional experiment.

### Requirement: DX-RESULT-01
r[DX-RESULT-01]

The Result library MUST provide success mapping, error mapping, fallible chaining, and recovery through ordinary checked programs. Result MUST remain an ordinary two-arm variant, not an exception effect, an IO monad, or a new expression form. The selected design is four operations on `Result<A,E> = Ok(A) | Err(E)`: `map-ok` transforms `Ok(A)` by a checked `A -> B` callback and carries `Err(E)` unchanged; `map-error` transforms `Err(E)` by an `E -> F` callback and carries `Ok(A)` unchanged; `and-then` invokes an `A -> Result<B,E>` callback only on `Ok(A)`; and `or-else` invokes an `E -> Result<A,F>` callback only on `Err(E)`. These are schematic stack-interface descriptions, not accepted generic variant declaration syntax or a newly implemented module. Concrete exported names, module/version identity, source signatures, and generic schema instantiation MUST pass the existing declaration/library interface gate before acceptance; DXM1's finite monomorphic two-constructor variants alone do not establish that gate.

### Requirement: DX-RESULT-02
r[DX-RESULT-02]

Each combinator MUST declare its complete ordered stack interface and conservative latent effect bound, including the callback's possible effects even if this invocation bypasses it. It MUST invoke the callback exactly once for the selected arm and never for the other arm; `and-then` and `or-else` MUST preserve an already selected `Err` or `Ok` without unwrapping it into an implicit early return. Both branches MUST account for every owned input and every callback output without implicit duplication or discard, including on ordinary error results and abnormal outcomes. Neither unselected-callback evaluation nor speculative host requests are permitted. The resource-free first slice uses ordinary `Data` payloads; admitting a resource-bearing payload requires the separate ownership/retirement profile, an explicit complete owner transition for each arm, and real boundary evidence. No new propagation syntax, IO monad, hidden exception path, or claim of implementation follows from this design.

## ADDED Requirements

### Requirement: DX-PURE-PAR-01
r[DX-PURE-PAR-01]

A future independent fork/join study MAY consider only finite checked subprograms with an explicit admissibility decision before execution: both branches must be resource-free, host-effect-free, independent under their actual ordered stack/capture interfaces, and have no live task, authority, borrow or native-pin obligation. All branch work and retained results MUST be bounded and precharged against an explicit quota; unknown cost or insufficient reservation rejects before either branch starts. Joining MUST be deterministic in declared left-then-right result order. An ordinary `Result` error is normal data and returns in that branch's ordered slot; for abnormal trap or metering exhaustion, wait for both bounded branches to settle, report the left abnormal outcome if present, otherwise the right, never publish a partial successful join, and retain all work charges until settlement. A precharge refusal precedes both branches. Any admissible execution MUST preserve the sequential reference's successful values, ordered recipe/identity observations and empty host-effect trace, with distinct recorded quota/trap outcomes rather than an unconditional equivalence claim. The study starts only after the language and schema/interface gates needed to express both branches; it adds no general guest spawn, channels, GPU backend, effectful parallel calls or M6 async executor behavior. It MUST NOT treat copyable channel endpoints, unsafe Array aliasing, `IO.within` without cancellation, fail-stop OOM, unreviewed foreign C/JS or process termination as a substitute for Noble's owner/retirement and bounded-admission contracts.

#### Scenario: ADAPT-18 deferred pure join admission

- GIVEN finite checked independent resource-free pure branch pairs, effectful and owner-bearing controls, and precharged bounded work
- WHEN an optional study compares sequential reference and permitted fork/join schedules, errors, traps, quotas, results and retained recipes
- THEN only admissible pairs may run in parallel, their ordered successful observations agree, and rejected or exhausted pairs acquire no task or authority
