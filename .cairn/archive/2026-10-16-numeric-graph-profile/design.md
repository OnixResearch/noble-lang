## Context

Issue #14 Phase A asks for a bounded design decision. Phase B (the CPU feasibility experiment) and Phase C (the measured continuation decision) start only after the owner approves the design. This document gives the complete Phase A decision. It also writes Phase B and Phase C as concrete experiment plans. The owner approved both on 2026-10-15 ([Phase B approval and decisions](#phase-b-approval-and-decisions)); execution status is in [Acceptance mapping](#acceptance-mapping). r[NGRAPH-SCOPE-01]

### Verified Noble baseline and drift

The issue's links point to `0ea10d3`. This change is based on `ffcce53`, which is `44329dc` (origin/main) plus a `cargo fmt` commit and a vendor re-pin for issue #22. At this base:

- [K-NUM-01](../../specs/language/spec.md) is still at `language/spec.md:557-570`. Its text is unchanged since `0ea10d3`: `+`, `-` and `*` are signed wrapping `I64`; deeper operand first; "Division, floating point, and the detailed checked-arithmetic library are not selected". V-NUM-01 (`verification/spec.md:273`) mirrors it for the model.
- The kernel `Ty` enum (`crates/noble-kernel/src/types.rs:197-249`) has `Unit`, `Bool`, `I64`, `Text`, `Syntax`, `Contract`, `Evidence`, `Certified`, `Pair`, `Sum`, `List`, `Program`, `LiveRef`, `Resource`, `Nominal` and `GenericNominal`. There is **no FP32 type**. This file is unchanged since `0ea10d3`.
- `Ty::is_data` (`crates/noble-kernel/src/types/data.rs:10-60`) walks payloads iteratively. It rejects `Resource` and `LiveRef` at any depth and accepts `Program(..)` as data. A lint attribute forces every constructor to declare its eligibility explicitly (`tigerstyle::fragile_exhaustive_enum_match`). This file is unchanged.
- `crates/noble-wasm/src/source/plan/operations.rs` **drifted** (+32/−4 from `0ea10d3` to `44329dc`, then reformatted). `derivation()` (`:23-57`) still requires one consistent complete `stack_in`/`stack_out`/`effects` interface per checked node. `named_call()` (`:334-347`) still resolves to `submission.definitions[target].identity`. New since `0ea10d3`: Calculator-v1 actions (`:239-247`, definitions 27–31, including `CalculatorDivRem`). Under the calculator profile, literal and quotation-push atoms no longer record per-operation signatures (`:191-201`). This design therefore reads interfaces from the checked derivations and never from the Wasm plan's recorded operation signatures.
- The bootstrap builtin table (`crates/noble-kernel/src/contracts/bootstrap/data.rs:238-264`) orders definitions as 0 `dup`, 1 `drop`, 2 `swap`, 3 `dip`, 4–6 `+ - *` (`Behavior::Arith`), 7 `=`, 8 `quote`, 9 `compose`, 10 `run`, 11 `reflect`, 12–21 data and control words, and 22 `test.emit`. `Behavior::Reserved` keeps builtin identities disjoint between profiles (`contracts.rs:40-100`).
- Accepted proofs PO-09, PO-10 and PO-11 cover only "bootstrap fragment v1" (`specs/verification/obligations.json`). PO-12 (arithmetic refinement), PO-16 (evidence binding), PO-17 (lowering preserves observations) and PO-18 (loading binds executed code) are open.

### What handwired does (read-only at `4ea7fcb`)

Everything in this subsection comes from files read at the pinned revision. Links are listed at the end of this document.

- Handwired is **builder code**, not a compiler: "ordinary JavaScript that runs once and appends the operations to a `Circuit`. No tool converts existing JavaScript source into a program." Evaluation is JavaScript on the CPU; only the JSON-search demo uses WebGPU ([hw-readme:3]).
- There are four instruction kinds: `affine`, `reglu(left, gate) = f(left * max(0, gate))`, `boolean(score) = f(f(f(score) + 8_388_609) - 8_388_609)` and `index` ([hw-exec:19-28]). Here `f` is `Math.fround`. Affine terms run in stored order starting from `f(bias)`, and each product and each partial sum is rounded ([hw-exec:7-17]; reference code [hw-core:1888-1900], [hw-core:1673-1702]).
- Ordinary builder expressions **sort terms by ascending wire**. Only `roundAffine` keeps the caller's order ([hw-exec:17], [hw-exec:32]; `normalizeTerms` [hw-core:1662-1671]).
- Compilation prunes operations that do not reach the requested roots and packs dependency layers ([hw-exec:38-46]). Compiled results are checked against the reference with `===`, "which ignores zero's sign and fails on NaN" ([hw-exec:50], [hw-core:3367-3381]). Gather lowering can return `-0` where the dense sum returns `+0` ([hw-exec:74]).
- Range, grid, one-hot, bit and index-integrality declarations "are promises from the caller, not proofs" ([hw-exec:60]). Only source-cell ranges are checked when the program is evaluated (`validateSourceRanges`, [hw-core:784-795]). Intermediate facts are checked only by test helpers ([hw-exec:60], [hw-core:3194-3270]).
- An index read floors the index in float64, clamps or returns `+0` outside the table, and returns NaN for a NaN index. It requires a declared index range strictly inside ±2^24 ([hw-exec:82-99], [hw-core:1844-1868]).
- `backpropagateReference` "differentiate[s] FP32 arithmetic as real-valued; ReLU uses zero slope at its corner". It accumulates gradients with `Math.fround`, walking the operations in reverse. It throws at a boolean atom unless `booleanGradient: 'straight-through'` is passed. For index reads it offers `indexGradient` values `'reject' | 'zero' | 'backward-difference'`. `'reject'` throws as soon as any nonzero gradient reaches the read, even one destined only for the selected candidate ([hw-core:654-746], [hw-exec:36], [hw-exec:112]).
- The learned gallery trains "244 source cells", which are the coefficients, with Adam in a program of "6,320 operations in 8 layers", using `backpropagateReference` ([hw-gallery:3]). The Adam step overwrites `source[parameter]` in place. It uses float64 moments with `**` bias corrections, gradient-norm limiting and a clamp to ±6 ([hw-geom:739-767]).
- `createTensorEvaluator` reuses work buffers, and "the next call overwrites the same result array" ([hw-exec:294]). The docs measure 2,367,454 built operations taking about 3 s to build and 4–5 GB of memory ([hw-exec:48]).

Noble borrows the explicit graph, bounded cells, a reference evaluator, layer scheduling and differential tests. It deliberately differs on six points: sign-preserving equality, source-order terms with no wire sorting, checked facts instead of promises, static derivative refusal, immutable snapshots instead of in-place updates, and fresh result values.

### Coordination

#8 owns applicability witnesses; its sibling change `bounded-applicability-guards` proposes GUARD-FACT-01 and GUARD-BIND-01. #10 owns artifact correspondence. #11 owns known-graph and wiring laws. #15 owns the mathematical foundation, including any Lean binary32 model. #3 and #5 own ownership and allocation; their sibling changes `ownership-lifetime-contract` (OWN-SCRATCH-01) and `allocation-strategy-contract` (ALLOC-*) are proposals. #2 owns GPU dispatch, #4 alternate CPU backends, and #1 live publication. This design references those proposals and adds no requirement to them.

## Decisions

### Decision: D1 Scope and the exact I64 versus FP32 boundary

**Choice:** `NumericGraph-Design` is a nonselected, opt-in profile with two **disjoint** sub-profiles:

| Sub-profile | Purpose | Value types | Derivatives |
|---|---|---|---|
| `ngraph-i64/0` | Exact stack-to-graph extraction experiment over existing Core Programs | `I64`, `Bool` | **None.** Training contracts are refused. Modular arithmetic has no meaningful real-valued gradient. |
| `ngraph-f32/0` | Experimental differentiable slice | `F32` (profile-local; see D3) | Policies in D10 |

- A graph belongs to exactly one sub-profile. No graph edge, node or host-side helper converts `I64` to `F32` implicitly. An existing `I64` Program is never evaluated "as if" its values were floats. An `ngraph-i64/0` graph makes no claim of equivalence to handwired's FP32 cells.
- Requesting the profile changes nothing about ordinary checking, Wasm execution, identity, `run`, captures, live publication or K-NUM-01. A refused Program stays executable on its existing path.
- No source syntax, kernel type, word or diagnostic named here is implemented.

**Rationale:** The issue forbids reinterpreting `I64` as FP32. Keeping the sub-profiles disjoint lets Phase B exercise extraction against the real Noble Wasm path (`I64`) without borrowing claims from the differentiable slice, which has no ordinary execution path to compare against. r[NGRAPH-SCOPE-01]

### Decision: D2 Graph IR and operation/type whitelist

**Choice:** The graph IR is versioned `ngraph-ir/0`. It is a topologically ordered DAG of typed value nodes, and each node is defined only from earlier nodes. Its interface is an ordered input list (each input marked as `input` or `param`), ordered outputs, and an optional list of guard roots. The complete whitelist:

| Node | Sub-profile | Inputs → output | Semantics (D3 for FP32) | Facts required |
|---|---|---|---|---|
| `input(k)` / `param(k)` | both | → `T` | Ordered interface slot `k`. `param` only in `f32`. | Facts on inputs come from input validation (D9). |
| `const(v)` | both | → `T` | Range-checked `I64` literal, `Bool`, or exact finite `F32` | — |
| `i64.add/sub/mul(a,b)` | i64 | `I64 I64 → I64` | K-NUM-01, `a` is the deeper operand | — |
| `i64.eq(a,b)` | i64 | `I64 I64 → Bool` | `=` | — |
| `f32.add/sub/mul(a,b)` | f32 | `F32 F32 → F32` | One RNE rounding | — |
| `f32.affine(bias; [(c_i, x_i)])` | f32 | `F32* → F32` | Ordered fold (D3). `x_i` may be the constant-one operand. | — |
| `f32.relu(x)` | f32 | `F32 → F32` | NaN if `x` is NaN; otherwise `x` if `x > 0`, else `+0` (including for `-0`) | — |
| `f32.reglu(l,g)` | f32 | `F32 F32 → F32` | `rn(l × relu(g))` | — |
| `f32.step(s)` | f32 | `F32 → F32` | `s ≥ 0.5 ? 1 : +0` | `s ∈ [0,1]` |
| `f32.select(b, off, on)` | f32 | `F32³ → F32` | Bitwise copy of `on` if `b = 1`, otherwise of `off` (no arithmetic) | `b` is an exact bit |
| `f32.index(i, table, below, above)` | f32 | `F32 + F32ⁿ → F32` | Bitwise copy of `table[i]`, or the out-of-range rule | `i` integral and range-bounded |
| `guard(fact, x)` | both | → (root) | Fails closed if the fact is false (D9) | — |

- **Excluded from `/0`:** division, remainder, sqrt, exp, log, transcendental functions, comparisons other than `step`, `neg`/`abs` as primitives (sign changes use exact `× -1`), integer casts, bitcasts, `I64` index reads, `Bool` operations, control flow, lists or tables as graph inputs or outputs, and Program-valued nodes.
- Builder macros from handwired (`emitCompare`, `emitArgmax`, `emitMinimum`, gather lowering and so on) are **not** part of the IR. Any of them would come later as a separately checked library or rewrite with its own exactness contract.
- `select` is a bitwise choice gated by a checked exact bit. It is not handwired's arithmetic interpolation, which can produce NaN from an unselected `0 × Infinity` ([hw-exec:274-278]).

**Rationale:** This is the smallest set that covers affine layers, products, ReGLU networks, hard decisions and bounded table reads, which are the parts the issue names. It keeps each node's exactness contract local. r[NGRAPH-F32-01] r[NGRAPH-EXTRACT-01]

### Decision: D3 FP32 reference contract

**Choice:** `ngraph-f32/0` is defined directly over IEEE 754 binary32 with roundTiesToEven, written `rn`. It is not defined over JavaScript float64 or real arithmetic. "Exact" means agreement with this contract.

- **Type and ABI.** `F32` is a profile-local scalar: `Data`, duplicable and discardable, never a resource. At the graph ABI, `F32` is 4 bytes of little-endian binary32 bits, `I64` is 8 bytes little-endian two's complement, and `Bool` is one byte, `0` or `1`. Inputs and parameters are ordered by interface slot. A parameter is supplied only as a reference to a snapshot (D11), never as caller-written cells. Outputs are fresh immutable values. Making `F32` a source-level kernel type is a separate, profile-gated decision ([D13](#decision-d13-mapping-to-kernel-admission-source-extraction-runtime-ownership-and-proof-to-artifact-obligations)).
- **Literals and constructors.** These are proposed syntax only. A decimal literal such as `1.5f32` is accepted **only if it is exactly representable** in binary32. Bit form `0x3fc00000f32b` is accepted only if it encodes a finite value. Inexact decimals, NaN and infinite literals are rejected (`E-LITERAL-INEXACT`, `E-LITERAL-NONFINITE`). Literals are never rounded silently. Handwired, by contrast, silently applies `f(coefficient)` to float64 coefficients and only requires them to be finite ([hw-exec:9-15], [hw-core:3429-3433]).
- **Conversions.** The graph has none. A host adapter that turns an external number into `F32` must be explicit and must record either *exact-only* conversion (refuse if inexact; for `I64`, `|x| ≤ 2^24`) or a named RNE conversion. No `F32 → I64` conversion exists.
- **Operations.** `add`, `sub` and `mul` each round once. `affine(bias; terms)`: `v₀ = bias`, then `vᵢ = rn(vᵢ₋₁ + rn(cᵢ × xᵢ))` in **stored order**. Because coefficients are exact, `rn(cᵢ)=cᵢ`. A source sum without a bias uses `bias = -0`. In binary32, `-0` is an exact additive identity (`-0 + x = x` for every `x`, including `±0`). Starting at `+0`, as handwired does when the bias is 0, turns a lone `-0` term into `+0`. `relu(-0) = +0`, `relu(NaN) = NaN`, `reglu(l,g) = rn(l × relu(g))`, so `reglu(-1, 0) = -0`. `step` is defined only on its checked range `[0,1]` and maps `0.5 ↦ 1`. This matches handwired's odd-base formula on `[0,1]` ([hw-core:1914-1920]), but outside `[0,1]` that formula yields other integers, so Noble requires the fact.
- **Order, fusion and reassociation.** Fused multiply-add, contraction, reassociation, distribution, term sorting, moving a constant into the bias, algebraic identities (`x+0`, `x×1`), and keeping a chain in wider precision are **forbidden** whenever they could change any bit. The allowed rewrites are: constant folding by evaluating the same `rn` operation, CSE of structurally identical nodes, and fusing a single-use left-deep source chain into one `affine` node with the same order and the `-0` bias rule. One derived claim is that rounding a binary64-computed single `+`, `−` or `×` of two binary32 operands to binary32 equals `rn`. This holds because binary64 has 53 ≥ 2·24+2 bits, the standard double-rounding-is-innocuous condition [INFERENCE; checked by Phase B control B2-ORACLE]. So handwired-style `Math.fround` evaluators can serve as an independent reference **per operation**, but not for whole chains.
- **Signed zero, infinities, NaN and subnormals.** These follow IEEE propagation: `±0` are preserved, `±∞` are produced on overflow, NaN comes from `∞ × 0` or `∞ − ∞`, and subnormals are neither flushed nor treated as zero. A backend with flush-to-zero or denormals-are-zero does **not** conform (handwired notes such backends exist: [hw-exec:246]).
- **Index reads.** The index must have a checked *integral* fact and a checked range fact with `|i| < 2^24` (D9). In range, the read copies `table[i]` bitwise, so `-0`, `∞` and NaN candidates pass through. The `below` and `above` rules are each one of `refuse` (the default: guard failure), `clamp` (nearest end) or `zero` (`+0`). There is no silent floor: a fractional or NaN index cannot be selected, because the integral fact excludes it statically or the guard refuses it. Handwired's float64 floor ([hw-exec:82-99]) is available only as a future named policy, not in `/0`. Candidates may repeat. Tables are fixed-length, `1..T_max` entries.
- **Equality.** Observation equality `≡obs` compares binary32 encodings **bitwise**, so `+0 ≢ -0`. All NaN encodings form **one class** whose sign and payload are unobservable. This replaces handwired's `===` comparison ([hw-exec:50]). As a result, gather lowering that changes the sign of zero ([hw-exec:74]) is not a valid `/0` rewrite.

**Rationale:** A bitwise contract that ignores only NaN payloads is portable across IEEE implementations. Wasm `f32` likewise leaves NaN payloads nondeterministic [INFERENCE]. The contract makes signed-zero and subnormal errors visible instead of hiding them under `===`. Requiring exact literals removes a silent rounding step at the source boundary. r[NGRAPH-F32-01]

### Decision: D4 Effect exclusions

**Choice:** A graph is pure and total apart from named guard failures. The following are excluded:

- every host operation, including `test.emit`, bound emit and clock slots, `live.*`, `code.prepare`, files, WASI and calculator host-facing words;
- resources and `LiveRef` anywhere in the interface or captures;
- live-slot `slot.invoke`;
- clocks, randomness and environment reads.

Event collection, state installation, rendering, I/O, authorization, data loading, the training loop, PRNG use and optimizer arithmetic stay in the **host**. A stateful use is exposed as a pure transition `(state, input, params) → (next_state, output)`. The host decides whether to install `next_state`. The graph contains no DOM, no effects and no persistent cell.

**Rationale:** An empty effect bound is necessary, but as the issue notes it establishes neither termination nor bounded expansion. Those are handled in D5 and D7. r[NGRAPH-EXTRACT-01]

### Decision: D5 Resource and expansion limits

**Choice:** Each budget is explicit, with the policy defaults below. A policy may lower a budget; raising one requires a new profile policy revision. Exceeding a budget refuses before any evaluation with `E-BUDGET-<kind>`. The diagnostic reports the observed count up to the point of refusal.

| Budget | `/0` default | Charged on |
|---|---|---|
| Extraction work units (symbolic steps, including repeated expansions) | 4,194,304 | Every processed source node |
| Expanded nodes, **before** pruning, CSE or fusion | 262,144 | Every node created |
| Inline depth (named call, `run`, `dip` nesting) | 64 | Expansion stack |
| Live nodes after normalization | 262,144 | Graph checker |
| Layers (critical path) | 16,384 | Graph checker |
| Inputs and params / outputs | 65,536 / 65,536 | Interface |
| Terms per `affine` | 4,096 | Node |
| Index table length / total table entries | 65,536 / 1,048,576 | Node / graph |
| Facts | 262,144 | Fact phase |
| Retained graph bytes / snapshot bytes | 32 MiB / 4 MiB (1,048,576 `F32`) | Build / snapshot |
| Forward scratch / backward scratch per evaluation | 64 MiB / 128 MiB | Evaluation preflight |
| Optimizer steps per host call | Explicit argument, ≤ 1,000,000 | Training call |

Wall-clock limits and cancellation are host policy, checked at layer boundaries (D12).

**Rationale:** Handwired measures 2.37M built operations taking about 3 s and 4–5 GB ([hw-exec:48]). These defaults stay one order of magnitude below that and still exceed the learned-gallery scale (6,320 operations, [hw-gallery:3]). Charging the **unpruned** expansion means pruning cannot launder an exponential expansion. r[NGRAPH-EXTRACT-01]

### Decision: D6 Unsupported diagnostics

**Choice:** Every refusal has a stable code. None executes any graph node, and none changes the ordinary path.

| Code | Stage | Trigger |
|---|---|---|
| `E-PROFILE` | request | Profile or sub-profile not opted in or unknown |
| `E-SUBJECT` | admission | Subject not independently kernel-accepted under the requested profile |
| `E-CLOSURE` | admission | Supplied dependency or capture closure differs from the recomputed exact closure |
| `E-INTERFACE` | admission/extraction | Missing or inconsistent checked derivation for a node |
| `E-EFFECT` | admission/extraction | Nonempty effect bound or a host-operation word |
| `E-RESOURCE` | admission/extraction | `Resource` or `LiveRef` in an interface, capture or nested payload |
| `E-DYNAMIC-RUN` | extraction | `run`, `dip` or `compose` on a Program not statically known |
| `E-LIVE-SLOT` | extraction | Live-slot dispatch |
| `E-RECURSION` | extraction | Definition identity already on the expansion stack (direct or mutual) |
| `E-CONTROL` | extraction | `if`, `case`, `list.case`, `inl`, `inr` |
| `E-AGGREGATE` | extraction | `Text`, `Syntax`, `Sum`, nominal, a list that is not a static table, or a table reaching anything other than an `index` table operand |
| `E-REFLECT` | extraction | `reflect` or any reflection-dependent execution |
| `E-PROGRAM-OUTPUT` | extraction | Program-valued output (its recipe is observable; V-OBS-01) |
| `E-TYPE-MIX` | extraction/check | Mixed sub-profile edge or attempted reinterpretation |
| `E-LITERAL-INEXACT`, `E-LITERAL-NONFINITE` | extraction | D3 literal rules |
| `E-BUDGET-<kind>` | any static stage | D5 |
| `E-FACT-UNCHECKED` | fact phase | A required fact lacks an allowed provenance |
| `E-FACT-INPUT` | pre-evaluation | Input validation rejects an input-fact cell |
| `G-FACT-FALSE(node,fact)` | evaluation | Guard failure; no outputs |
| `E-GRAPH-MALFORMED` | graph check | Cycle, forward reference, type error, dangling or unknown node, duplicate slot |
| `E-DERIV-BOOLEAN`, `E-DERIV-INDEX` | training admission | Default-refused derivative path (D10) |
| `E-DERIV-I64` | training admission | Training contract on `ngraph-i64/0` |
| `E-SNAPSHOT-STALE`, `E-SNAPSHOT-LAYOUT` | pre-evaluation | Snapshot graph or layout identity mismatch (D11) |
| `E-PARAM-UNDECLARED` | training admission | Gradient or update requested for a non-`param` slot |
| `E-NONFINITE-UPDATE` | training step | NaN or ∞ gradient or updated parameter; no snapshot is published |
| `C-CANCELLED` | evaluation/training | Host cancellation; scratch released (D12) |

**Rationale:** Named, stage-scoped refusals turn "unsupported" into observable and testable outcomes. Phase B (B3) gives each code a control. r[NGRAPH-EXTRACT-01]

### Decision: D7 Admission and stack-to-graph extraction algorithm

**Choice:** Admission and extraction form a deterministic pipeline whose stages run strictly in order.

1. **Admission.** Inputs: an independently accepted submission and its checked derivations; the root (a definition identity or a closed Program value); the claimed closure `C`; the sub-profile `P`; and the budget `B`.
   - (a) Kernel acceptance under `P` must be on record; otherwise `E-SUBJECT`.
   - (b) Recompute the exact reachable definition identities and capture values from resolved identities, as in `named_call`, and compare them with `C`; otherwise `E-CLOSURE`.
   - (c) Read the root's ordered interface from its derivation. Each type must be a `P` scalar.
   - (d) The root effect bound must be empty.
2. **Symbolic execution.** The symbolic stack `Σ` holds `Scalar(id, T)`, `Known(program)`, `Table([id])`, `PairSym(a, b)` or `Unit`. Initialize `Σ` bottom-first from the input slots. For each body node, in source order:
   - Fetch its interface with a `derivation()`-style consistency check (`E-INTERFACE`), then require empty effects and that the top of `Σ` matches `stack_in`.
   - Dispatch on the node kind:
     - **Literals:** `I64`/`Bool` → `const`. Proposed `F32` literals → exact check, then `const`.
     - **Quotation literal:** → `Known` with empty captures. No graph node is created; retained-recipe budget is charged.
     - **`dup`:** pushes the **same** id; the value is immutable, so sharing is exact.
     - **`drop`:** pops.
     - **`swap`:** reorders ids.
     - **`x [q] dip`:** expands the `Known` `q` below `x` (`E-DYNAMIC-RUN` if `q` is not known).
     - **`+ - *` and `=`:** create `i64.*` nodes with the deeper operand first.
     - **`quote`:** wraps the popped symbolic value as a `Known` capture. The same id is captured, so no copy is made.
     - **`compose`:** concatenates two `Known`s; their interfaces were already unified by K-PROG-05.
     - **`run`:** expands `Known` bodies with their captures (`inline depth + 1`). Unknown Programs, such as Program inputs, give `E-DYNAMIC-RUN`.
     - **Named call:** resolves the exact identity. If the identity is already on the expansion stack, `E-RECURSION`; otherwise push it, expand, and pop.
     - **`pair`/`unpair`/`unit`:** symbolic routing only.
     - **`nil`/`cons`** over `F32` scalars of static length: `Table`.
     - **Profile `F32` words:** the corresponding D2 node.
     - **Anything else:** the D6 code.
   - Every step charges work units, every created node charges expanded nodes, and every expansion charges inline depth. The pipeline stops at the first exceeded budget.
   - At the end, `Σ` must match the root output interface. Pairs flatten into ordered scalar outputs, with their shape recorded. `Known` or `Table` outputs give `E-PROGRAM-OUTPUT` or `E-AGGREGATE`.
3. **Fact phase.** Derive facts by transfer rules (D9). For every operation that needs a fact, use a derived, input-validated or applicable evidence-backed fact. If none exists and policy allows guards, insert a `guard` root before the dependent node; otherwise refuse with `E-FACT-UNCHECKED`.
4. **Normalization.** Only the D3 allowed rewrites, then pruning of nodes that reach no output **and** no guard. Guards and their dependencies are never pruned. No source-unchecked node exists to prune, because every node came from checked source.
5. **Independent graph check.** A separate checker re-derives node types from the whitelist signatures and verifies acyclicity and topological order, the D3 payload rules (exact constants, coefficient finiteness), fact provenance records (D9), and post-normalization budgets. It also recomputes a saturating upper bound on named-call expansion from the closure's call graph, memoized per definition, so it does not trust the extractor's count (V-CHECK-01). The extractor's charge on unpruned `compose`/`run` expansion stays inside the trusted extraction boundary until NG-PO-A is discharged (D13).
6. **Identity and layout.**
   - `GraphLogicalId = H("noble.ngraph.graph.v0" ‖ profile+semantic rev ‖ fact-rule rev ‖ canonical nodes in creation order ‖ interface ‖ outputs ‖ root subject identity ‖ closure identities)`. Including the subject means two Programs that produce identical graphs still get distinct identities, so evidence never transfers silently.
   - `PackedLayoutKey = H(GraphLogicalId ‖ scheduler/compiler rev ‖ layout options)`. This is a build key, not semantic identity, in the same way handwired's `wireOrder` packing ([hw-exec:44]) is not part of program meaning.
   - Layering follows handwired's depth rule ([hw-exec:40-42]) and may reorder only independent nodes.

Runtime-built Programs are never extracted by ordinary `run`. Supporting them later would need a separate explicit preparation step after their bounded closure is known (K-PROG-04, P-PREPARE-01).

**Rationale:** This reuses Noble's existing boundary of complete checked derivations and exact definition identities (`operations.rs:23-57`, `:334-347`) without treating the Wasm plan as a differentiable IR. Stack routing turns into id plumbing, which is exactly the "known graph" view that #11's wiring laws may later reason about. r[NGRAPH-EXTRACT-01]

### Decision: D8 Forward observations

**Choice:** `Obs(G, inputs, snapshot)` is one of:

- `Normal(outputs)`, with outputs compared by `≡obs` (D3);
- `Refused(code)`, before evaluation: admission, snapshot mismatch or input validation;
- `GuardFailed(node, fact)`, with no outputs;
- `Exhausted(kind)`;
- `Cancelled`.

Pruning, CSE, fusion, layering and packing must preserve `Obs` exactly, including which guard fails first. Guards evaluate in canonical node order, and the reference evaluator fixes that order.

For `ngraph-i64/0` graphs extracted from a Program, `Normal(outputs)` must equal the ordinary Noble Wasm/V8 output stack for the same inputs. The pass-through prefix (K-TYPE-01) is not part of the graph. The ordinary path's own engine-quota failures are not reproduced: the graph refuses statically instead, and the report keeps those classes distinct (BE-LIMIT-01). A graph never yields a reflectable recipe and is not a Program replacement (V-OBS-01).

**Rationale:** Making failure classes and guard order part of the observation stops optimization from hiding a failure. r[NGRAPH-EXTRACT-01] r[NGRAPH-F32-01]

### Decision: D9 Checked facts and provenance

**Choice:** The fact kinds are `range[lo,hi]`, `grid(2^k)`, `bit`, `one-hot(group, allow-zero)`, `finite`, `shape(n)` and `integral`. Each fact record holds `(GraphLogicalId, node, kind, payload, provenance, profile rev, fact-rule rev)` and has exactly one provenance:

1. `derived(rule)`: a sound transfer rule over D3 semantics. For example, intervals propagate through `rn` with monotone endpoint rounding, as handwired's `evaluateAffineRange` does ([hw-core:1126-1147]). NaN-capable ranges become unknown. `step` yields `bit`. `select` of two bits yields `bit`. Integral × integral and integral ± integral within `2^24` yield `integral`.
2. `input-validated`: checked on the actual input or parameter cell **before** evaluation. Failure gives `E-FACT-INPUT`. Handwired checks only source-cell ranges ([hw-core:784-795]); `/0` checks every input fact kind.
3. `evidence(witness)`: an accepted proof whose applicability witness is bound, under #8, to this exact subject, inputs, semantic revision and graph identity. A stale or partial witness does not apply.
4. `guard`: a runtime `guard` node that fails closed before any dependent node.

Caller assertions without one of these provenances are refused with `E-FACT-UNCHECKED`. Validating inputs establishes nothing about intermediate values except through rule 1. Facts are attached to value ids. They therefore survive `dup`, `swap`, `dip` and capture, and must be re-derived after arithmetic. This matches the proposed #8 GUARD-FACT-01 without depending on it. Any change of subject, closure, profile or fact-rule revision invalidates every fact.

**Rationale:** Handwired documents that a false declaration can corrupt an optimization or GPU code ([hw-exec:60]). Noble makes the same information sound or refused. r[NGRAPH-FACT-01]

### Decision: D10 Derivative policies

**Choice:** A training contract names `ngraph-deriv-real/0` plus per-node options. This policy is **the derivative of the intended real-valued arithmetic, ignoring FP32 rounding**. It is not the derivative of the floating-point function.

- `add`/`sub`: `±g`. `mul`: `g·b`, `g·a`. `affine`: `g·cᵢ`.
- `relu`, and the `reglu` gate: derivative `0` at `gate ≤ 0`, including `-0`. This is the corner convention, as in handwired ([hw-core:737]). `reglu` left: `g·relu(gate)`.
- `select`: `g` goes to the selected operand; the gate is a hard-boolean path.
- `step` (hard boolean): **default refuse**. If any declared parameter reaches the loss through a `step` or a `select` gate, training admission refuses **statically** with `E-DERIV-BOOLEAN`. Handwired refuses dynamically when a nonzero gradient arrives ([hw-core:719-721]). The opt-ins are `straight-through` (identity) and `clamped-linear(w)` (slope `1/w` on `|s−0.5| < w/2`, otherwise 0).
- `index`: two separate policies. The **candidate gradient** goes to the selected candidate (after `clamp`; none on the `zero` side), and repeated reads accumulate it. It is the exact real derivative for a fixed index and is allowed by default. The **index gradient** defaults to static refuse (`E-DERIV-INDEX`) when a parameter reaches an index operand. The opt-ins are `zero` (explicitly no gradient) and `backward-difference` (`read(i) − read(i−1)`, labelled a **surrogate**; compare handwired's `indexGradient` at [hw-core:690-715]). Handwired's `'reject'` also blocks candidate gradients; Noble separates the two.
- **Reference backward arithmetic:** binary32 `rn`, visiting nodes in reverse canonical order. Within a node, contributions are accumulated in stored term order into each input's gradient, so gradients are bitwise reproducible. A compiled backward pass must match the reference under `≡obs` or be reported as a distinct, non-reference evaluator.
- Every opt-in, with its parameters, is recorded in training provenance (D11). No surrogate is described as an exact derivative.

**Rationale:** These are the issue's three distinctions (real versus FP32, corner, hard boolean and index), made static and named. r[NGRAPH-DERIV-01]

### Decision: D11 Training contract, immutable snapshots and provenance

**Choice:**

- **The training contract** binds the `GraphLogicalId`, the subset of `F32` input slots marked `param`, a separately admitted **loss graph** (`outputs, targets → F32`), the derivative policy and options, an optimizer revision with exact hyperparameter bits, `seed: u64` with PRNG `splitmix64/0` (host-side initialization and shuffling), and the ordered training-data digest. Constants, captured values, `input` slots (state, event bits, indices) and graph structure are never trainable.
- **Optimizers.** `sgd/0` computes `p' = rn(p − rn(lr·g))` in binary32. `adam/0` uses binary64 RNE in a fixed order with no FMA. It keeps running products `β₁ᵗ` and `β₂ᵗ` as state instead of calling `pow`, because handwired's `**` relies on `Math.pow` ([hw-geom:744-745]), and `pow` is not required to be correctly rounded [INFERENCE]. `sqrt` is correctly rounded. Each parameter is converted to binary32 once, at the store. Weight decay, gradient-norm limiting and clamping (all present in handwired at [hw-geom:747-767]) are named optimizer options, never implicit.
- **Snapshots.** Each published step yields a **new** immutable snapshot `ParamSnapshotId = H("noble.ngraph.params.v0" ‖ GraphLogicalId ‖ ordered param layout ‖ exact LE bytes)`, together with an immutable optimizer-state digest. The working state of a step is private and is discarded on failure (`E-NONFINITE-UPDATE`, `C-CANCELLED`); the last published pair stays current. Handwired, by contrast, overwrites `source[parameter]` in place ([hw-geom:763-767]).
- **Provenance** binds the parent snapshot, subject and closure identities, profile and derivative revisions with their options, fact bindings, loss graph, data digest, seed and PRNG, optimizer with its bits, step count, tool revisions and the resulting snapshot. Replaying from provenance on the same pinned platform must reproduce the snapshot bytes.
- **Use.** Evaluating `G` with snapshot `S` requires `S.graph = GraphLogicalId(G)` and a matching layout; otherwise `E-SNAPSHOT-STALE` or `E-SNAPSHOT-LAYOUT`. Older snapshots keep evaluating exactly as before. A hash identifies bytes; it does not prove correctness.
- **Capture.** Capturing snapshot values into a Program (`quote`, K-PROG-07) creates a **new subject**. Its evidence must be checked again for applicability (#8, #10). Proofs never transfer automatically, and captured constants are never updated in place.

**Rationale:** Learning changes designated parameter values, not structure or semantics. Identity and provenance make that checkable. r[NGRAPH-SNAP-01] r[NGRAPH-DERIV-01]

### Decision: D12 Ownership and backend boundary

**Choice:**

- **Retained owners:** graph code (immutable, shareable), parameter snapshots (immutable bytes), optimizer-state snapshots and captures. Duplicate owners may release in any order.
- **Per-evaluation scratch:** activations, the gradient tape and workspace. An evaluator may keep a reusable workspace, but results are **copied out** as fresh immutable values. Handwired's evaluator instead returns a result array that the next call overwrites ([hw-exec:294]).
- **Cleanup.** On normal return, `G-FACT-FALSE`, `E-*` during preflight, exhaustion and `C-CANCELLED`, scratch is released or reset exactly once. No partial snapshot is published and no retained owner changes. Cancellation is checked at layer boundaries.
- **Scratch reset must never invalidate a saved Program, graph or snapshot.** This is the numeric-graph instance of the proposed #3 OWN-SCRATCH-01. Allocation strategy and failure classes for these owners are deferred to #5 (proposed ALLOC-*) and S-MEM-04.
- **Device buffers and queues** are explicitly managed, move-only host resources (S-RES-01). They are never `Data`, graph nodes or captures. GPU eligibility and dispatch belong to #2.
- **Backends.** The normative executor is a straightforward **CPU reference interpreter** over `ngraph-ir/0`. A compiled CPU layered evaluator is optional and must match it under `≡obs`. Ordinary Program execution on the selected Wasm/V8 path is unchanged and stays the baseline. Measurements compare against that path and a straightforward host-native implementation (D13, Phase C). A direct-native Noble backend is not a prerequisite; alternate CPU backends are #4.

**Rationale:** Separating owners makes cleanup and snapshot stability testable (Phase B, B6). r[NGRAPH-OWN-01] r[NGRAPH-BENCH-01]

### Decision: D13 Mapping to kernel admission, source extraction, runtime ownership and proof-to-artifact obligations

**Choice:**

| Concern | Existing anchor (at base) | Profile obligation | Coordination |
|---|---|---|---|
| Kernel admission (`I64`) | Accepted PO-09/10/11 for bootstrap fragment v1; `Behavior` table `bootstrap/data.rs:238-264` | Graph admission requires prior independent acceptance and reuses the accepted subject. No new kernel rule. | — |
| Kernel admission (`F32` source, optional) | `Ty` has no `F32` (`types.rs:197-249`); explicit-arm eligibility lint (`types/data.rs:10-60`); `Behavior::Reserved` keeps profile identities disjoint, as Calculator-v1 does with 27–31 (`operations.rs:239-247`) | **Phase B decision B0:** either (a) add a profile-gated `F32` type and words behind an explicit environment flag with an explicit `Data` arm, outside the accepted PO-09/10/11 fragment until it is re-proved, or (b) author `F32` graphs directly in `ngraph-ir/0`, checked only by the graph checker, and use source extraction only for `I64`. K-NUM-01 and V-NUM-01 stay unchanged either way. | #15 (binary32 model) |
| Source extraction | `derivation()` `operations.rs:23-57`; `named_call` `:334-347`; drift `:191-201` | D7. Interfaces come from checked derivations, never from Wasm-plan signatures or producer metadata (V-CHECK-01). | #11 (wiring analysis) |
| Runtime ownership | S-MEM-04, S-RES-01; proposed OWN-SCRATCH-01, ALLOC-* | D12 and NGRAPH-OWN-01 | #3, #5, #2 |
| Proof-to-artifact | PO-12, PO-16, PO-17, PO-18 open | Proposed **open** obligations (named here, **not** added to `obligations.json`): **NG-PO-A** extraction preserves `Obs` on the `I64` overlap (PO-17 analogue); **NG-PO-B** graph-checker soundness; **NG-PO-C** compiled evaluator refines the reference (PO-17/18 analogue); **NG-PO-D** the reference model corresponds to IEEE binary32 (#15); **NG-PO-E** fact-transfer soundness (#8, PO-21 analogue); **NG-PO-F** `ngraph-deriv-real/0` computes the real derivative away from corners; **NG-PO-G** snapshot and provenance binding (PO-16 analogue); **NG-PO-H** executed compiled-graph bytes correspond to the admitted graph (#10) | #8, #10, #15 |

Producer metadata (extractor node types, fact records, counts, hashes) is **never** independent validation. The graph checker re-derives it. Matching hashes and agreement between evaluators are reported only as what they are (NGRAPH-PROOF-01, V-CLAIM-01, EV-STATE-01).

**Rationale:** This places every new trust edge next to an existing open obligation instead of inheriting the bootstrap fragment's accepted proofs. r[NGRAPH-PROOF-01] r[NGRAPH-EXTRACT-01]

## Phase B approval and decisions

The owner approved Phase B and Phase C on #14 on 2026-10-15. The decisions under that approval:

- **P1:** D1–D13 and the nine NGRAPH-* requirements are the basis for Phase B.
- **P2:** Option **B0(b)**. `F32` graphs are authored directly in `ngraph-ir/0` (builder API or the text form) and checked only by the graph checker. Source extraction is for `I64` only. No kernel type, word or frontend changes; K-NUM-01 and the accepted PO-09/10/11 fragment are untouched.
- **P3:** The isolated workspace crate `crates/noble-ngraph-experiment/`. It does not modify noble-kernel, noble-contracts, noble-wasm or noble-cli sources; it may depend on them as a consumer (the `I64` extraction leg does).
- **P4:** The pinned independent binary32 oracle is `rustc_apfloat =0.2.3+llvm-462a31f5a5ab` (crates.io, pure Rust port of LLVM APFloat, `#![forbid(unsafe_code)]`).
- **P5:** `evidence` facts are excluded from Phase B. The checker refuses an `evidence` provenance with `E-FACT-UNCHECKED`.
- **P6:** #3 OWN-SCRATCH-01 is **proposed** (active change `ownership-lifetime-contract`), not canonical. B6 tests the numeric-graph instance only.
- **Phase C thresholds kept** as proposed in C3: stop unless the compiled CPU graph is at least 2× faster than both baselines on warm `W3`; admission at most 100 ms at `W3` scale; at least a 2× margin over compiled CPU before any scoped #2 GPU experiment.

**Interpretations made while executing Phase B.** These follow from the design; none changes a decision.

- `affine` coefficients are constants (D2), so trainable layers in `F1` combine `param` values with `mul`/`add` in the affine order (bias first, then terms in stored order).
- `index` under `refuse` with an out-of-range index is reported as `G-FACT-FALSE` at the index node with the fact `range[0, n−1]`. Evaluators report the first failure point (guard or refusing index read) in canonical node order.
- Host fuel (`Exhausted`) and cancellation (`C-CANCELLED`) are host policy checked at layer boundaries; their observation point is evaluator-specific and they are not compared across evaluators.
- The B4 G2 bound `4·depth·2^-24` is applied relative to the absolute-value magnitude of the computation (the same reverse pass over |values| and |local derivatives|). A first run scaled by |forward values| alone failed 20 of 1,440 slot checks from cancellation in `d = y − t`; this was investigated, not loosened silently ([receipts](../../../verification/ngraph/README.md)).
- A `select` gate reached by a param is refused (`E-DERIV-BOOLEAN`) unless that node opts into the `straight-through` surrogate, defined as `(on − off)`. `clamped-linear` applies to `step` only.
- Applicability of old-subject evidence is checked on run records (subject, `GraphLogicalId`, snapshot). D6 has no code for it; it is reported as `NotApplicable(OtherSubject)`. #8 and #10 own witness binding.
- B6 instrumentation is the crate's scratch-lease counters. A counting global allocator would need `unsafe`, which the workspace forbids.

**Execution status.** Receipts and commands are in [verification/ngraph/README.md](../../../verification/ngraph/README.md). B1–B6 are executed. The core receipt (`phase-b-core.json`) covers `F1`–`F3`, the `F32` legs L2, B2-ORACLE, L3 and L4, the rejection controls on authored IR, and B4–B6. The extraction receipt (`phase-b-extract.json`) covers `I1`–`I4` extracted from Core source, L1 (`--opt off` and `on`), the BigInt L2, L3 and L4, and 16 source-level controls including the `k = 20` expansion chain. In the extraction leg, 24 L1 vectors of one random program are ordinary-path quota refusals, reported as a separate limit class. The receipts README lists that leg's deviations from the design text: no `(block …)` syntax, the admission-stage placement of `E-DYNAMIC-RUN`, `E-PROGRAM-OUTPUT`, `E-LIVE-SLOT` and `E-RECURSION`, forged recursion subjects and a hand-assembled `k = 20` subject (Core source cannot express either), chunked L1 sessions, code choices where the design names none, and a producer-digest subject identity. NG-PO-A..H stay open.

**Phase C status.** Phase C was measured on the pinned, shared host; see the Phase C section of the [receipts README](../../../verification/ngraph/README.md). On warm W3 at learned-gallery scale (6,270 operations, 243 params), the compiled CPU graph runs at 0.13× the speed of a straightforward host-native Rust implementation. Admission takes 1.1 ms. Under the kept thresholds the recommendation is **stop**, and a #2 GPU experiment is blocked. Noble's ordinary Wasm/V8 path has no `F32` under B0(b), so for W3 the stop test is applied to the host-native baseline alone. W1/W2 were extracted from Core source and measured against both baselines (`phase-c-l1.json`). Extraction from source to the first compiled result takes about 0.2 ms. The compiled graph is about 2×10^5 to 5×10^5 times faster than the ordinary path's per-submission `noble session` interface, and 0.0066–0.0077× host-native. It is therefore not 2× faster than both baselines on any workload, and the recommendation is unchanged.

**Promotion (owner decision, 2026-10-16).** The owner adopted the numeric graph profile as a supported opt-in feature despite the Phase C **stop** recommendation. The adoption makes no performance or GPU claim (GPU stays with #2), keeps B0(b) (no kernel `F32` type or word; K-NUM-01 and the accepted PO-09/10/11 fragment are untouched), keeps NG-PO-A..H open, and treats all agreement evidence as finite differential evidence. This answers the promotion question for this change: its nine NGRAPH-* requirements are synced into the canonical specifications unchanged, and this change is archived. The supported surface (library crate, `noble ngraph` CLI, on-disk formats) is designed in the separate change `numeric-graph-v1`. Sync and archive are document operations; they promote no scenario case, implementation status or proof obligation.

## Phase B plan: CPU-only feasibility experiment (approved 2026-10-15)

**Prerequisites:**

- **P1:** The owner approves this design on #14.
- **P2:** The owner selects option B0(a) or B0(b) from D13.
- **P3:** An isolated experiment crate or tool (for example `crates/noble-ngraph-experiment/`) is added. It does not touch the ordinary compiler path. The pinned toolchain from the existing devShell is used, and the CPU host is recorded.
- **P4:** A pinned independent binary32 oracle is chosen (Berkeley SoftFloat 3e or `rustc_apfloat`).
- **P5:** Either #8 witness binding is available, or `evidence` facts are excluded from Phase B.
- **P6:** The status of #3 OWN-SCRATCH-01 is recorded.

Every result becomes an evidence record whose implementation, execution, proof and trust fields are independent (EV-STATE-01). Each run records the BE-CONFIG-01 fields.

- **B1 Lowering.** Sources are written in current fixture syntax; `F32` words are proposed only.
  - `I1` `(block (I64 I64) (I64) [ dup * swap 3 * + 7 - ])`: `(x=5, y=4) → 24`. Exercises `dup`/`swap`, constants and operand order.
  - `I2` `(block (I64) (I64) [ 2 quote [ + ] compose dup compose run ])`: `40 → 44`. Captures, composition and one shared capture id used twice.
  - `I3` with a definition `sq = [ dup * ]` and root `[ sq swap sq + ]`: `(3,4) → 25`. A repeated named call to the same identity.
  - `I4` transition `(block (I64 I64 I64) (I64 I64) [ swap [ * ] dip + dup ])`: `(s,i,k) → (s·k+i, s·k+i)`. Run for 1,000 host steps.
  - `F1`: a 2→3→1 ReGLU network with fixed parameters.
  - `F2`: the transition `(e, v; m₀..m₃) → (rn(rn(m₀e)+rn(m₁v)), rn(rn(m₂e)+rn(m₃v)))` (compare [hw-exec:178]).
  - `F3`: `index` over `[a, a, b, -0]` with every out-of-range rule.
- **B2 Differential legs.**
  - `L1` ordinary Noble Wasm/V8 (I-programs only), with optimization off and on.
  - `L2` independent profile reference: a BigInt modulo-2^64 oracle for `I64`, and the pinned softfloat oracle for `F32`. `B2-ORACLE` adds a per-operation `Math.fround` leg to check the D3 double-rounding inference.
  - `L3` the graph reference interpreter.
  - `L4` the compiled layered evaluator.

  Expected result: all legs agree under `≡obs` on:
  - **Operand order:** `5 3 - = 2`.
  - **`I64` overflow:** `MAX 1 + = MIN` (CORE-14), `MIN -1 * = MIN`.
  - **Reassociation:** `affine(-0; 2^24, 1, 1) = 2^24`, versus `2^24+2` if reassociated.
  - **FMA detection:** `a=b=1+2^-12`, `c=-(1+2^-11)` gives `rn(rn(ab)+c) = +0`; a fused evaluation gives `2^-24`.
  - **Signed zero:** a lone `-0` term stays `-0`; `relu(-0)=+0`; `reglu(-1,0)=-0`; `select` and `index` return a `-0` operand bitwise.
  - **Nonfinite values:** `2^127·2=+∞`; `reglu(∞,0)=NaN` (class); a NaN candidate passes through `index`.
  - **Subnormals:** `2^-126·0.5 = 2^-127`. Flush-to-zero is detected as nonconforming.
  - **Boundary indices:** `-1, 0, n−1, n` under `refuse`/`clamp`/`zero`; `±(2^24−1)`.
  - **Repeated reads** of the same candidate.
  - **Seeded random graphs** (seed recorded) with ≥ 10,000 input vectors per graph.
- **B3 Rejections before protected execution.** Expected for every case: the D6 code, zero evaluated nodes, and the ordinary path unchanged.
  - False facts. A caller-asserted range, grid, bit or one-hot fact gives `E-FACT-UNCHECKED`. A violating input with an input fact gives `E-FACT-INPUT`. A violating intermediate under a guard gives `G-FACT-FALSE`, with `0.5` held in a "bit".
  - Bad indices. A fractional index `2.5`, an out-of-range index under `refuse`, and a NaN index under a guard.
  - Forbidden constructs. `test.emit`, a resource input, `LiveRef`, live-slot dispatch, `run` of a Program input, self and mutual recursion, `reflect`, `if`/`case`/`list.case`, and a Program output.
  - Malformed IR given directly to the checker. A cycle, a forward reference, a type mix, a dangling node, an unknown operation, an inexact or NaN constant, and a duplicate slot.
  - Budgets. An exponential named-call chain `dₖ = [ dₖ₋₁ dₖ₋₁ ]` with `k = 20` must give `E-BUDGET-EXPANSION` even when the result is dropped, which shows that pruning cannot hide it.
- **B4 Gradients.**
  - (G1) Binary64 reverse mode under the same policy versus binary64 central differences. Use `h = 2^-20·max(1,|p|)` and tolerance `1e-6·max(1,|∂|)`. Justification: roundoff is about `2^-52/h ≈ 2^-32` and truncation is about `h²`, both far below the tolerance.
  - (G2) FP32 reference backward versus G1. Use the rounding bound `4·depth·2^-24` relative. A failure is investigated, never loosened silently.
  - Sample only points more than `4h` from any ReLU corner, the `step` threshold or an integer index boundary.
  - Separate cases: the corner at `gate = 0` and at `-0` gives a zero gate gradient. A boolean path is refused statically, and `straight-through` is recorded. Index candidate gradients accumulate (`[a,a,b]` read at `0` and `1` gives `a ← 2g`). The `reject`, `zero` and `backward-difference` index policies are each tested separately, and the last is labelled a surrogate.
- **B5 Training.** Train `F1` against a fixed 64-point target set (recorded data digest) with an MSE loss graph, `adam/0`, a fixed recorded `seed` and 500 steps.
  - Expected: the loss falls to ≤ 10% of its initial value. This is a smoke threshold, not a convergence claim.
  - The `GraphLogicalId` and every non-param byte are unchanged.
  - Snapshot `S₀` evaluates bitwise-identically after training.
  - Replay reproduces the final snapshot bytes on the same platform.
  - A wrong graph, a wrong layout or a stale snapshot is refused. Evidence for the old subject is refused for a newly captured subject.
- **B6 Ownership.**
  - A retained snapshot is used after the invocation exits.
  - Two owners are released in both orders.
  - Cancellation is injected at layer `k`, alongside a guard failure and exhaustion. Instrumented allocator counters must show scratch released exactly once, no partial snapshot, and retained digests unchanged.

## Phase C plan: measured continuation decision (approved 2026-10-15)

**Prerequisites:** Phase B is complete, and its evidence has been reviewed.

- **C1 Measurement.** Workloads:
  - `W1` = `I1` (small).
  - `W2` = `I4` for 10^5 steps.
  - `W3` = `F32` ReGLU networks at three sizes, up to the learned-gallery scale (about 6,320 operations and 244 parameters, [hw-gallery:3]).
  - `W4` = a 10,000-entry index table (compare [hw-exec:114]).

  Report the NGRAPH-BENCH-01 metrics:
  - extraction, admission and build time;
  - live nodes and depth;
  - parameter, activation, scratch and retained bytes;
  - cold latency (admission, build and first evaluation);
  - warm latency (median and IQR over N ≥ 1,000);
  - repeated-evaluation throughput;
  - training step cost.

  The baselines are the ordinary Noble Wasm/V8 path (where semantics overlap) and a straightforward host-native Rust implementation with contraction disabled. Pinned handwired under pinned Bun may be run for information only.
- **C2 Pinning and claim separation.** Record source, tool, profile, oracle and hardware revisions. Report reference agreement, finite coverage, theorem obligations (NG-PO-A..H, all open unless separately discharged) and trusted compiler/runtime boundaries as separate fields. Unsupported instrumentation is reported as such, never as zero.
- **C3 Recommendation.** Proposed thresholds, which the owner may adjust at approval:
  - **Stop** if the compiled CPU graph is not at least 2× faster than both baselines on warm `W3`, or if admission cost exceeds 100 ms at `W3` scale with no training value demonstrated.
  - **CPU specialization** if it is ≥ 2× faster and B5 succeeded.
  - **A separately scoped GPU experiment (#2)** only if measured or explicitly modelled transfer and launch costs, against the batch size of a real workload, leave a margin of at least 2× over compiled CPU. Missing GPU costs block that recommendation.

## Acceptance mapping

| Issue #14 checkbox | Section | Status |
|---|---|---|
| A1 Whitelist, FP32/I64 boundary, effect exclusions, limits, diagnostics | [D1](#decision-d1-scope-and-the-exact-i64-versus-fp32-boundary), [D2](#decision-d2-graph-ir-and-operationtype-whitelist), [D3](#decision-d3-fp32-reference-contract), [D4](#decision-d4-effect-exclusions), [D5](#decision-d5-resource-and-expansion-limits), [D6](#decision-d6-unsupported-diagnostics) | Designed |
| A2 Forward observations, checked assumption provenance, derivative policy, immutable snapshot identity | [D8](#decision-d8-forward-observations), [D9](#decision-d9-checked-facts-and-provenance), [D10](#decision-d10-derivative-policies), [D11](#decision-d11-training-contract-immutable-snapshots-and-provenance) | Designed |
| A3 Map to kernel admission, source extraction, runtime ownership, proof-to-artifact; producer metadata is not validation | [D7](#decision-d7-admission-and-stack-to-graph-extraction-algorithm), [D12](#decision-d12-ownership-and-backend-boundary), [D13](#decision-d13-mapping-to-kernel-admission-source-extraction-runtime-ownership-and-proof-to-artifact-obligations) | Designed |
| B1 Lower pure program and state transition with captures, composition, repeated inputs | [Phase B](#phase-b-plan-cpu-only-feasibility-experiment-approved-2026-10-15) B1 | **Executed** (`I1`–`I4` extracted from source; `F1`–`F3` authored) |
| B2 Differential: original, independent reference, compiled graph | Phase B B2 | **Executed** (`I64` L1 off/on, BigInt L2, L3, L4; `F32` L2, B2-ORACLE, L3, L4) |
| B3 Reject false facts, bad indices, effects/resources, malformed topology, budgets | Phase B B3 | **Executed** (16 source-level and 35 authored-IR controls) |
| B4 Gradients versus analytic and finite differences; policies separately | Phase B B4 | **Executed** |
| B5 Train a fixed-shape example; snapshots and identity | Phase B B5 | **Executed** |
| B6 Retained snapshots, owner orders, failure/cancellation cleanup | Phase B B6 | **Executed** |
| C1 Costs and baselines | [Phase C](#phase-c-plan-measured-continuation-decision-approved-2026-10-15) C1 | **Executed** (W3 three sizes and W4 against host-native; W1/W2 from source, with extraction time, against host-native and ordinary Wasm/V8 `--opt off`/`on`) |
| C2 Pin revisions; separate agreement, coverage, theorems, trust | Phase C C2 | **Executed** (separate `c2` fields) |
| C3 Stop / CPU specialization / scoped GPU recommendation | Phase C C3 | **Executed: stop** (compiled W3-L at 0.13× host-native; GPU blocked) |

## Risks / Trade-offs

- **The `F32` source path costs proof scope.** Option B0(a) widens the kernel type universe outside the accepted PO-09/10/11 fragment. Option B0(b) leaves `F32` graphs without Noble source extraction. Either way, the differentiable slice has no ordinary-execution leg to compare against; only independent references.
- **Strict bitwise semantics forbid profitable rewrites.** No FMA, no reassociation and no sign-changing gather lowering. Loosening any of these would need a separately named profile whose own equality relation is stated.
- **Bound placement.** Static derivative refusal and fail-closed guards can reject training setups that handwired would run. That is intended, but it may make the gallery-scale workload need explicit opt-ins.
- **Trust in the extractor.** Unpruned-expansion accounting for `compose`/`run` stays inside the extractor's trust boundary until NG-PO-A is discharged.
- **Unknowns.** Budgets and Phase C thresholds are proposals, not measurements. Pinned-platform replay does not establish cross-platform bit reproducibility.

## Non-claims

- No proof, benchmark or GPU claim. Phase B results are finite tests in the isolated experiment crate ([receipts](../../../verification/ngraph/README.md)); they discharge none of NG-PO-A..H.
- No change to K-NUM-01 or ordinary Programs.
- No automatic whole-language compilation, general tensor language, structure evolution, live publication, effects inside the graph, or default surrogate gradients.
- Finite differential tests and matching hashes do not establish a verified compiler.
- The description of handwired is limited to the pinned files cited.

[hw-readme:3]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/README.md?plain=1#L3
[hw-exec:7-17]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L7-L17
[hw-exec:9-15]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L9-L15
[hw-exec:17]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L17
[hw-exec:19-28]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L19-L28
[hw-exec:32]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L32
[hw-exec:36]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L36
[hw-exec:38-46]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L38-L46
[hw-exec:40-42]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L40-L42
[hw-exec:44]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L44
[hw-exec:48]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L48
[hw-exec:50]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L50
[hw-exec:60]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L60
[hw-exec:74]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L74
[hw-exec:82-99]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L82-L99
[hw-exec:112]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L112
[hw-exec:114]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L114
[hw-exec:178]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L178
[hw-exec:246]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L246
[hw-exec:274-278]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L274-L278
[hw-exec:294]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/docs/execution-model.md?plain=1#L294
[hw-core:654-746]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L654-L746
[hw-core:690-715]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L690-L715
[hw-core:719-721]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L719-L721
[hw-core:737]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L737
[hw-core:784-795]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L784-L795
[hw-core:1126-1147]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L1126-L1147
[hw-core:1662-1671]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L1662-L1671
[hw-core:1673-1702]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L1673-L1702
[hw-core:1844-1868]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L1844-L1868
[hw-core:1888-1900]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L1888-L1900
[hw-core:1914-1920]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L1914-L1920
[hw-core:3194-3270]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L3194-L3270
[hw-core:3367-3381]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L3367-L3381
[hw-core:3429-3433]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/runtime/numeric-ui-core.ts#L3429-L3433
[hw-gallery:3]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/learned-gallery/README.md?plain=1#L3
[hw-geom:739-767]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/learned-gallery/trainable-geometry.ts#L739-L767
[hw-geom:744-745]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/learned-gallery/trainable-geometry.ts#L744-L745
[hw-geom:747-767]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/learned-gallery/trainable-geometry.ts#L747-L767
[hw-geom:763-767]: https://github.com/chenglou/handwired/blob/4ea7fcb27ebe93acf49b9f29a93e162b975bad46/src/learned-gallery/trainable-geometry.ts#L763-L767
