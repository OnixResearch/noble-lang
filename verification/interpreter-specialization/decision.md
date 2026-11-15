# Issue #34 first projection: evidence and continuation decision

Decision at source commit `aa8df373a234d384a7f649e413cdcc53221bd145`; frozen criteria were committed earlier at `e5794d7`. The source-bound receipt is `first-projection-aa8df37.json` (SHA-256 `912877dbc7b550e200fec38a7834b41dc8467a9e9228b597c013ae2133792a32`). This is a bounded first projection, not an endorsement of a replacement compiler or self-application.

## First projection and correctness boundary

The source language is **exactly two tagged bytes**, one instruction per byte (`0=add`, `1=sub`, `2=mul`). An I64 accumulator starts at dynamic `x`; each instruction computes `acc op y` in strict order, with wrapping I64 operations. Only the three valid tags and length two are accepted. Stack errors do not exist in this expression family; no branch, trap, host effect, unbounded recursion or lazy evaluation is represented. A change to that profile requires a new semantic admission and correctness argument. The reference directly decodes the tagged bytes and applies wrapping arithmetic, independently of the one-hot interpreter graph.

The interpreter is an ordinarily prepared, kernel-checked and extracted Noble Program with eight I64 inputs (six one-hot static opcode coefficients, then dynamic `x,y`). Its frontend needs the recorded bounded 524,288-node / 512,000,000-work admission cap because constraint expansion of eight live input binders exceeds the normal frontend cap; all graph budgets remain the recorded normal bounded limits. The interpreter runs every arithmetic branch as a total operation and selects results by one-hot multiplication and addition. Issue #32's checked-graph specialization is extended only with total wrapping-I64 zero/one/identity simplifications; operand forwarding removes dead identity nodes and zero annihilation removes unselected branches. A source emitter renders each residual as Noble text, which goes through independent ordinary-path frontend, kernel, extractor and graph checking. #33's positive-only session cache performs fresh admission on both misses and hits; it caches no executable.

The target equation is `Obs(run(readmit(mix(I, static(s))), (x,y))) = Obs(run(I, (static(s),x,y))) = Obs(ref(s,x,y))` for each valid two-byte program and all I64 `(x,y)`, where observations are normal single-I64 equality. This Rust implementation is checked by **finite tests**, not a general proof. The pre-existing foundation `FirstProjection` and `OperandForwarding` theorems establish the abstract packed-graph statement for all nine programs and all bitvector inputs; they do **not** prove Rust/model correspondence, source admission/refinement, the real backend or any cost relation. Arithmetic is total in the admitted profile, but differing admission and runtime budget refusals are not claimed equivalent.

Tests in the existing `extract_b2` target cover all nine ordered programs against 266 boundary/seeded pairs each (2,394 paired rows), comparing the unspecialized interpreter, specialized graph, re-admitted compiled residual, direct ordinarily accepted two-operation Program and independent tagged-byte reference. Malformed lengths/tags, wrong dynamic input arity/type and zero-variant/sub-24-node budget refusals are tested. Wrong static coefficient slot, wrong opcode decoder, an actually re-admitted interpreter with reversed subtraction operands, and a stale cached static dependency all produce a counterexample; the cache then misses under the corrected binding. `noble-ngraph` also tests forwarding through chained identities, zero annihilation, preserving `0-x`, and ordinary re-admission. These are test controls, not mutation coverage of arbitrary compiler defects.

All nine retained residual graph texts have two I64 input nodes and exactly the selected two arithmetic nodes. Example for tags `[0,1]`: `%0=input 0; %1=input 1; %2=i64.add %0 %1; %3=i64.sub %2 %1; out %3`. Every program's emitted source and full graph text are in the receipt, with exact subject IDs, repeated vector agreement and dynamic-input variation. The selector multiplications and unselected branches are absent rather than hidden behind a wrapper.

## Measurements against frozen criteria

The receipt pins the source, toolchain, CPU/OS, graph/backend/semantic/specializer revisions, frontend and graph budgets, seed, cache bounds, optimization profile and stage boundaries. It records 11 setup samples per program and 31 independently timed warm batches of 1,024 identical input pairs per pathway; every distribution has min, p25, median, p75 and max. Preallocated evaluator workspaces and inputs are reused; the host-native direct-arithmetic comparator deliberately does not allocate a graph output and is **not** asserted to match graph interface overhead. The ordinary direct compiler comparison is source admission/extraction plus the layered graph backend; **no Wasm CLI compilation or instantiation was measured**, and graph warm latency must not be called CLI speedup. Initial interpreter admission/extraction/build is separately recorded as a shared cost. Cold first residual execution and direct ordinary build have their own samples.

All times below are **median nanoseconds per warm batch**, not per CLI invocation. Setup is the sum of separately measured median specialization, source emission, independent residual admission and backend build; the common already-built interpreter is not charged twice. Break-even calls are `ceil(setup_ns × 1024 / (interpreter_batch_ns − residual_batch_ns))` **only** when the batch savings are positive; a nonpositive saving would make break-even undefined. Cache-hit lookup/content validation and fresh admission are separately reported, not subtracted from uncached setup. The cache's retained source/manifest bytes are not machine-code bytes.

| Tagged program | Interpreter | Residual | Direct ordinary graph | Host-native | Incremental setup ns | Break-even calls |
|---|---:|---:|---:|---:|---:|---:|
| add, add | 1,000,800 | 444,740 | 436,125 | 7,935 | 4,107,384 | 7,564 |
| add, sub | 824,230 | 370,973 | 377,034 | 6,792 | 3,794,047 | 8,572 |
| add, mul | 996,401 | 475,999 | 455,842 | 7,504 | 4,207,601 | 8,280 |
| sub, add | 901,144 | 401,370 | 397,823 | 6,863 | 4,042,882 | 8,284 |
| sub, sub | 1,006,670 | 442,857 | 438,860 | 8,145 | 3,941,333 | 7,159 |
| sub, mul | 824,800 | 387,995 | 435,123 | 9,278 | 4,117,000 | 9,652 |
| mul, add | 1,610,178 | 733,240 | 743,338 | 14,467 | 7,300,998 | 8,526 |
| mul, sub | 1,164,847 | 599,410 | 647,099 | 11,581 | 7,418,850 | **13,436** |
| mul, mul | 2,172,669 | 924,638 | 737,859 | 13,315 | 6,172,903 | 5,065 |

The specialized sources are 47 bytes each versus the 202-byte interpreter source; all nine residuals have four nodes versus 24. The compiled evaluator retains 104 bytes per residual versus 528 for the interpreter, and needs a 40-byte workspace versus 200. Each request produces one variant; nine distinct source programs are included across the campaign. A fresh cache session per setup repetition stores one positive entry (2,883 accounted bytes) and then records a validated hit; cache hits still pay ordinary admission. These are structural/reported bytes, not peak process heap. Actual peak temporary allocation, Wasm code size, end-to-end CLI prepare/compile/instantiate, and later-stage generation costs are explicitly **unsupported with reasons** in the receipt; none is represented as zero. Shared-host variability and the failure near a 10,000-call threshold preclude generalizing these timings to another workload or host.

## Decision on later projections

**First-projection hard correctness/dispatch gate: PASS** for this finite family and tested implementation. Nine of nine warm residuals exceed the preregistered 20% saving criterion, and source size is below the original. **Second projection: NO-GO.** The preregistered condition required *every* positive-saving program to amortize within 10,000 calls; `(mul,sub)` requires 13,436. More fundamentally, the current specializer is Rust code for Noble graphs, not an accepted Noble representation of `mix` itself. No finite self-specialization/termination bound or reviewed binding-time-quality argument exists. Do not attempt `mix(mix,I)` on these results. Reopen only with (a) a new frozen out-of-sample cost campaign showing the required bound across its workload, (b) an accepted, independently checked specializer-as-subject or justified staging bridge, and (c) explicit code-growth, termination, trusted-boundary and correctness review. This is a feasibility decision, not a statement that second projection is impossible.

**Third projection: NO-GO.** It depends on a valid second projection and, beyond that, an accepted representation of `mix`, measured compiler-generator cost, repeated-use economics and correctness against independently accepted generated compilers. Reopen only after that evidence passes a separately frozen gate. Neither the present graph proof nor a warm graph result makes `mix(mix,mix)` well typed or executable in the accepted profile.

## Acceptance map

| #34 item | Evidence or recorded decision |
|---|---|
| 1. Bounded source, semantics, correctness boundary | Frozen `criteria.md`; interpreter construction and decoded oracle in existing `extract_b2` test module; model distinction above. |
| 2. Differential, malformed and abnormal outcomes | Three interpreter tests plus receipt `records[*].differential_agreement` for 9 × 1,024 campaign rows. No stack errors or traps in this language. |
| 3. Four incorrect variants | The seeded-control test, including actual swapped-interpreter admission and cache rebinding. |
| 4. Eliminate static dispatch | All `records[*].residual_graph_text` and `residual_source`, four live nodes each; structural test covers all nine. |
| 5. Independent stages and resources | Each `records[*].stage_ns`, `cache_miss_ns`, `cache_hit_ns`, `warm_batch_ns`, sizes/variant/workspace fields and explicit unsupported peak heap. |
| 6. Equivalent work and direct/native comparisons | `records[*].warm_batch_ns` for four prewarmed boundaries; ordinary direct source admission/build in stage report; CLI/Wasm unsupported with reason, not a graph speedup claim. |
| 7. Pinning, repetitions, variability | Receipt top-level source/hardware/toolchain/profile/budgets/dataset/cache/repetitions, per-stage and per-path distribution fields. |
| 8. Positive-savings amortization and frozen thresholds | Prior `criteria.md`, positive measured batch savings, formula/table and no-go here. |
| Projection gate | Separate second/third NO-GO decisions and reopen conditions above. |
