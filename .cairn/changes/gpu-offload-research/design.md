## Context

This is a research and design record for issue #2. It selects no GPU API and authorizes no implementation. Grounded statements cite base `ffcce53`; unobserved claims are labelled **[INFERENCE]**. CPU execution routes, including the selected Node/V8 path and any Wasmtime/Cranelift alternative, are analysed in the active `cranelift-backend-evaluation` change (#4). This change concerns CPU/GPU dispatch only.

### Baseline facts

| Fact | Evidence |
|---|---|
| `Program(required, result, EffSet)` is a first-class type; the type universe has `Unit, Bool, I64, Text, Syntax, Pair, Sum, List, Program` plus inert contract/evidence kinds; there is no floating-point type | `crates/noble-kernel/src/types.rs` L197-L227 |
| Core-Bootstrap words: `dup drop swap dip + - * = quote compose run reflect unit pair unpair inl inr case if nil cons list.case test.emit test.abort`; no map, fold or loop | `crates/noble-contracts/src/program/words.rs` L87-L112 |
| Recursive definitions remain open, so Core lists cannot be traversed to unbounded length; `iterate`, `i64.cmp` and `i64.div-rem` exist only in the Calculator-v1 profile | README L535; `crates/noble-contracts/src/source/resolution.rs` L229-L233 |
| `+ - *` wrap modulo `2^64` with signed interpretation and deeper-then-top operand order; division and floating point are not selected; results must not depend on optimization | `.cairn/specs/language/spec.md` K-NUM-01 (L557-L569) |
| `if : S Bool Program<S,T,e> Program<S,T,f>` executes only the selected branch, but both branches type-check and contribute effects | spec L1071; K-CONTROL-01 |
| `dip : S a Program<S,T,e> -- T a ! e` | spec L1014 |
| Effect bounds are conservative upper bounds (may widen); an annotation is not authority; an empty bound does not exempt work from limits | K-EFFECT-02, K-EFFECT-04, H-LIMIT-02 |
| Runtime `quote`/`compose`/`run` must not invoke the source compiler | W-EXEC-02 |
| Selected runtime arithmetic: `i64.add/sub/mul` after popping right then left | `crates/noble-wasm/runtime/operations.wat` L14-L25 |
| A `List<I64>` element costs a cons cell plus a boxed scalar head cell (48 bytes each) | `operations.wat` L46-L54; `stack.wat` L46-L52 |
| Default allocation limit 65,536 bytes, maximum 196,608, cell capacity 4096 | `crates/noble-cli/src/core/runtime/config.json` L39-L49 |
| Pure fork/join precedent: only finite, checked, resource-free, host-effect-free, independent branches with precharged bounded work and deterministic join order | active `bend-qcue-scoped-adaptations` DX-PURE-PAR-01 |
| The selected Calculator/Mathlib comparator models I64 as `Int → ZMod (2^64)` with signed reinterpretation | README "Independent Mathlib comparison"; `proofs/m3/MathlibI64.lean` |

**Consequence.** Under the current list representation, a default-limit `List<I64>` holds at most ⌊65536 / 96⌋ = 682 elements, or 2048 at the maximum limit, before any output allocation. Device transfer and launch overheads are therefore unlikely to pay off without a packed batch representation and larger declared limits [INFERENCE: to be measured; see cost model].

## Decisions

### Decision: Explicit typed batch-map boundary first; no inferred parallel regions

**Choice:** The only initial entry point is one explicit operation. The working name `batch.map` is not selected syntax.

```text
batch.map : S List<I64> Program<[I64], [I64], {}> -- S List<I64> ! {}
```

Its **reference semantics** is the sequential left-to-right map: `out[i] = P(in[i])`, with output in input order and P run on a fresh one-element stack. A device may evaluate elements in parallel only because the eligible subset below makes elements independent and trap-free. Automatic inference of parallel regions in ordinary code is deferred and rejected for the initial study. A binary `batch.zip-map` over two equal-length lists is a candidate extension, not part of the initial subset. r[GPU-SCOPE-01]

**Rationale:**

- Core has no recursion or map, so there is nothing for inference to find except finite `list.case` nests and DX-PURE-PAR-01 style pairs.
- An explicit boundary makes data layout, eligibility diagnostics, quota precharge and fallback visible at one site.
- Inference would make quota and timing observations depend on an optimizer decision.

### Decision: Eligible subset `GPU-Map-Draft`

**Choice:** A `batch.map` site is GPU-eligible only if every condition holds, as decided by an independent checker over the kernel-accepted candidate, never by the lowering:

1. **Statically known code.** The Program operand's code is a quotation literal or an exact resolved, nonrecursive definition at that site. Runtime-selected Programs are ineligible: Programs taken from lists, returned by `if`/`case`, or built by `compose` over unknown operands. Runtime *captured values* are allowed (rule 3).
2. **Interface.** `[I64] → [I64]` with an empty latent effect bound. The body uses only `I64` and `Bool`, with `Bool` used only as the condition of `if`.
3. **Captures.** Immutable `I64`/`Bool` scalars captured by `quote` (possibly supplied after compilation) are passed as uniform kernel parameters. Program-, Text-, List-, Pair-, Sum-, Syntax- or resource-valued captures are ineligible.
4. **Words.** Integer literals, `+ - * =`, `dup drop swap`, `dip` and `if` with quotation-literal operands that are themselves eligible, and calls to eligible resolved definitions. Ineligible: `run` of any non-literal Program, `compose` producing a runtime Program, `quote` of anything but scalars, `reflect`, all list/pair/sum words, `test.emit`, `test.abort`, live slots, declared host words and every Calculator-only word (including `i64.div-rem`).
5. **Bounded expansion.** Symbolic stack execution of the body, which inlines `dip`/`if`/definitions and turns `if` into a select over both evaluated branches, yields a straight-line SSA kernel within declared node, depth and register bounds. Exceeding any bound is ineligible.
6. **No traps.** The subset has no division, no bounds-checked access and no `test.abort`, so per-element evaluation is total. Elements are independent because there is no shared mutable state.

Eligibility is a checked decision recorded with the prepared artifact and is not inferred from the effect bound alone. r[GPU-SCOPE-01]

**Rationale:** Each rule removes one source of order-dependent or unbounded behaviour. Effects and traps would make a prefix observable. Dynamic code would require a runtime compiler, which W-EXEC-02 forbids. Aggregates and resources would need device layouts and ownership. Unbounded expansion would break the work bound. The rules mirror DX-PURE-PAR-01 and the #14 extraction boundary.

### Decision: Compile-time eligibility and kernel generation; runtime selection only among prepared kernels

**Choice:**

- **At preparation:** check eligibility, build the SSA kernel, emit device IR (SPIR-V or WGSL text) and bind a kernel identity. The identity covers the kernel IR bytes, the subset revision, the lowering variant (native Int64 or u32-pair) and the generator version. No device is required for this step, and nothing in `run` compiles Noble source.
- **At runtime:** select between the CPU map and a prepared kernel using an explicit host policy (`cpu`, `auto` or `require-gpu`), device availability, a capabilities probe (for example Int64 support), batch size, residency and the cost model.

**Eligibility never implies selection.** The driver's pipeline compilation of already generated IR is a host operation. It is cached by kernel identity and counted as host cost, not guest work. Optionally, a later revision may key prepared kernels by the independently checked template identity of the active `runtime-program-template-identity` change, so a finite set of prepared templates can be selected at runtime. Capture values remain uniforms. r[GPU-DISPATCH-01]

**Rationale:** This separates semantic checking (static, device-independent, reproducible) from economics (dynamic, device-dependent). It also satisfies W-EXEC-02.

### Decision: Device-independent precharged quota and observable equivalence

**Choice:** In this profile, `batch.map` precharges `n × w(P) + a(n)` before choosing a device, where `w` is the checked per-element node count and `a(n)` is the output allocation. The charge is identical for CPU and GPU execution. Insufficient budget refuses before any launch or output, with the same quota class on every device. Device selection, fallback and retry MUST NOT change values, order, quota outcome or report classes. Only diagnostics, such as `executed_on` and the fallback reason, and wall time may differ. r[GPU-DISPATCH-01] r[GPU-FALLBACK-01]

**Rationale:** H-LIMIT-03 permits different budgets across backends, but here the *same* profile picks the device at runtime. A device-dependent budget would let selection change whether a program exhausts its quota, which is exactly the silent semantic change the issue forbids.

### Decision: Fallback, launch failure and interruption

**Choice:**

- **Ineligible site:** under `cpu`/`auto`, run the CPU map with an eligibility diagnostic. Under `require-gpu`, refuse statically at preparation with zero guest requests.
- **Device absent, unsupported capability, pipeline-compile failure, launch failure or device loss:** under `auto`, discard every device result, run the CPU map once and record the fallback reason; the logical work is not charged a second time. Under `require-gpu`, report an explicit `device-failure` outcome with no output list published.
- **Deadline or cancellation:** report `deadline`/`cancelled`, distinct from quota and trap, with no partial output. Device buffers and in-flight submissions stay owned and pinned until their fence signals; then they are retired. H-LIMIT-02 states that a deadline does not prove that native work stopped.
- **Empty batch:** returns `nil` immediately with no launch, no device allocation and a zero work charge.

r[GPU-FALLBACK-01]

**Rationale:** The subset is pure and total, so recomputation is semantically safe. The policy makes any substitution explicit, and a forced-GPU mode prevents silent CPU use when a user requires the device.

### Decision: Exact I64 lowering per device API

**Choice:**

- Lower `+ - *` to wrapping 64-bit operations where the API defines them exactly: SPIR-V `OpIAdd/OpISub/OpIMul` with the `Int64` capability, and PTX `add.s64`, `sub.s64`, `mul.lo.s64`.
- Where it does not, use a checked **u32-pair lowering**: WGSL, or Vulkan without `shaderInt64`. Add uses carry, sub uses borrow, mul computes the low 64 bits as the full 32×32 low-word product built from 16-bit limbs plus both cross products modulo `2^32`, and `=` compares both words.
- Signed interpretation applies only at literal encoding and output decoding. All three operations are sign-agnostic on two's-complement bits.
- In C-derived kernel languages (CUDA C++, OpenCL C, MSL, HIP), arithmetic is emitted only on unsigned 64-bit types to avoid signed-overflow undefined behaviour, as in CB-NUM-01.
- Never substitute floating point, fused or approximate arithmetic, or introduce division.

r[GPU-NUM-01]

**Rationale:** The feasibility table below shows that every candidate can implement K-NUM-01 exactly, but only some do so natively. The pair lowering is the portable fallback and an independent cross-check of the native lowering.

### Decision: Selection requires independent evidence

**Choice:** No device backend is selected without the GPU-PLAN tests below. Expected results come from the mathematical model (Lean/Mathlib `ZMod (2^64)` or an independent arbitrary-precision oracle), and CPU-versus-GPU comparison is bit-exact. The existing finite I64 Wasm comparison and proofs are precedent, not evidence. r[GPU-GATE-01]

**Rationale:** A new code generator, driver compiler and device form a new trusted path.

## Explicit versus inferred dispatch

| Criterion | Explicit `batch.map` | Inferred parallel regions |
|---|---|---|
| Where parallelism exists in Core today | One site, by construction | None: no recursion or map; only finite `list.case` nests or independent pairs |
| Eligibility diagnostics | At one site, with a reason | Diffuse; every optimizer change can alter them |
| Quota/timing observability | Precharged, device-independent | Depends on inference result |
| Data layout | Declared batch input | Requires discovering layouts across arbitrary code |
| Interaction with `reflect`/recipes | The recipe shows `batch.map` | Hidden rewrite of recipes (BE-REFLECT-01 risk) |
| Recommendation | **Evaluate first** | Defer; revisit only after explicit evidence and a recursion/loop design |

## Backend feasibility per candidate device API

| API / IR | Exact wrapping 64-bit `+ - *` | Lowering | Host access from Rust (`unsafe_code = "forbid"`) | Platforms / browser | Notes |
|---|---|---|---|---|---|
| Vulkan compute / SPIR-V with `Int64` | Yes. `OpIAdd`, `OpISub`, `OpIMul` produce "the low-order N bits of the correct result" (SPIR-V unified spec). `Int64` in shaders requires the optional Vulkan `shaderInt64` feature ("If this feature is not enabled, 64-bit integer types must not be used in shader code", Vulkan 1.4.365) | Native | Raw bindings require `unsafe`; a safe wrapper crate is a new trusted dependency [INFERENCE: e.g. a wgpu SPIR-V passthrough or Vulkan wrapper; adoption needs VT-NATIVE-03] | Linux/Windows/Android; macOS through a translation layer; no browser | **Recommended first native candidate** on the flake's x86_64 Linux target, conditional on a capabilities probe |
| Vulkan without `shaderInt64` | Not native | u32-pair | Same as above | Same | Required fallback variant; also an independent cross-check of the native lowering |
| WebGPU / WGSL | Not native. WGSL concrete integers are `i32`/`u32` only; overflow produces results "modulo 2^bitwidth" (W3C WGSL Candidate Recommendation Draft, 21 Sept 2026, §6.2.3) | u32-pair | A WebGPU implementation crate is a new dependency [INFERENCE: wgpu exposes a mostly safe API] | Browser-portable and native | Portable candidate; cost ≈ 3–4× ops for add/sub and ≈ 10–20× for mul [INFERENCE] |
| CUDA / PTX | Yes. `add.s64`/`sub.s64` compute `d = a + b`/`a − b` (`.sat` only for `.s32`); `mul.lo.s64` writes `t<n-1..0>` (PTX ISA 9.4 §9.7.1) | Native (emit PTX, or unsigned CUDA C++) | Driver API bindings require `unsafe` | NVIDIA only; proprietary driver; no browser | Optional vendor-specific candidate |
| OpenCL C | `long`/`ulong` 64-bit; signed overflow is undefined in C-derived semantics [INFERENCE: verify against the pinned OpenCL C spec] | Native via `ulong` only | Bindings require `unsafe` | Cross-vendor; driver quality varies | Lower priority |
| Metal / MSL | 64-bit integer types available [INFERENCE: verify against the pinned MSL spec]; use `ulong` | Native via `ulong` | Bindings require `unsafe` (Objective-C runtime) | Apple only | Not on the flake target |
| HIP / ROCm | As CUDA C++; hardware may decompose 64-bit multiply into 32-bit operations [INFERENCE] | Native via unsigned types | Bindings require `unsafe` | AMD Linux | Optional |

Every candidate host binding introduces a new trusted native boundary that needs VT-NATIVE-01/03 inventory. No candidate is admitted by relaxing the workspace `forbid`.

## End-to-end cost model

For a batch of `n` elements, kernel `P` with `w` checked nodes and `k` captured scalars:

```text
T_cpu(n)  = n · (c_disp(w) + c_alloc)                         # selected Wasm map: dispatch + 2 cells/element
T_gpu(n)  = T_pipe·[cold] + T_pack(n) + T_h2d(n) + T_launch
          + T_exec(n, w) + T_sync + T_d2h(n) + T_unpack(n)
T_pack(n) = n · c_pack        # linked cells → [u64; n]; zero if already device-resident/packed
T_h2d(n)  = L_h2d + 8n / B_h2d + (8k uniform bytes)
T_exec    = ceil(n / (lanes · occupancy)) · w · c_op(variant)   # c_op(u32-pair) ≫ c_op(Int64)
T_d2h(n)  = L_d2h + 8n / B_d2h
T_unpack  = n · (c_unpack + c_alloc)                          # rebuild List<I64>
select GPU  ⇔  policy ≠ cpu ∧ eligible ∧ device ok ∧ n ≥ n_min
               ∧ T_gpu(n) < (1 − margin) · T_cpu(n)
```

- **Compile-time costs** (eligibility, SSA and IR generation) are paid at preparation and amortized. `T_pipe` is a cold per-process driver compile, cached by kernel identity.
- **Break-even:** output allocation `c_alloc` is paid on both paths and cancels, giving `n* ≈ (T_pipe·[cold] + L_h2d + L_d2h + T_launch + T_sync) / (c_disp(w) − c_pack − c_unpack − 8/B_h2d − 8/B_d2h − w·c_op/lanes)`. With the current linked-list representation, `c_pack + c_unpack` is paid on the GPU path every time, so the denominator is small or negative unless `w` is large. Together with the 682/2048-element ceiling, this suggests the GPU path cannot win at current limits [INFERENCE].
- **Measured evidence still needed:** `c_disp`, `c_alloc`, `c_pack`, `c_unpack` on the selected CPU route; `L_h2d`, `L_d2h`, `B_*`, `T_launch`, `T_sync`, `T_pipe` and `c_op` for each lowering variant on pinned hardware and drivers; and the values of `n*` for representative `w`.
- **Prerequisites the model exposes:** a packed `I64` batch representation and larger declared limits (coordinate with #3/#5/#14).

## Correctness test plan (unexecuted)

**Oracles:** expected values come from the mathematical model, `((z + 2^63) mod 2^64) − 2^63` (K-NUM-01), evaluated by the Lean/Mathlib I64 model or an independent arbitrary-precision implementation. The CPU and GPU implementations under test are never used as the oracle. Every case runs CPU-reference, GPU-native-Int64 (if available) and GPU-u32-pair. All runs are compared bit-for-bit with the oracle and with each other, with optimization off and on.

| ID | Given | When | Then |
|---|---|---|---|
| GPU-PLAN-01 overflow boundaries | Inputs `[MAX, MIN, −1, 0, 1, 2^31, 2^32−1, 2^32, −2^32, MAX−1]`; P ∈ `[1 +]`, `[1 -]`, `[-1 *]`, `[dup *]`, `[4294967296 *]`, `[4294967295 *]` | map on each device/variant | Every element equals the oracle (e.g. `MAX 1 +` → `MIN`, `MIN -1 *` → `MIN`, `2^32·2^32` → `0`); 32-bit carry/borrow boundaries agree |
| GPU-PLAN-02 operand order | P = `[10 -]` vs `[10 swap -]`; `[dup 3 - *]` | map | Results equal `x−10` and `10−x` respectively (deeper − top), never swapped |
| GPU-PLAN-03 captures | `c quote [*] compose` with `c` supplied after compilation; the same captured Program reused for two batches; two different `c` values | map | Correct per-capture results; one prepared kernel identity; capture passed as a uniform; no recompilation |
| GPU-PLAN-04 nested control | P using `dip` and `if` on `=` (e.g. `[dup 0 = [drop 7] [3 *] if]`), and nested resolved nonrecursive definitions | map | Matches the oracle; both branches evaluated as a select with no observable difference |
| GPU-PLAN-05 empty/small batches | `nil`; n = 1; n < `n_min`; forced device in test mode | map under `auto` and `require-gpu` | `nil` with zero launches/device allocations; small batches select CPU under `auto`; forced device gives identical values |
| GPU-PLAN-06 quota bound | n at the declared batch/work/allocation bound and one over | map | At-bound succeeds identically; one-over refuses pre-launch with the same quota class on CPU and GPU; no partial list |
| GPU-PLAN-07 eligibility rejection | Bodies with `test.emit`, `Text`, `reflect`, `run` of a runtime Program, list/pair/sum words, `i64.div-rem`, non-empty effect bound, Program chosen from a list or by `if`, expansion one over bound | prepare under `require-gpu`; run under `auto` | Static `eligibility` rejection with zero guest requests/launches under `require-gpu`; CPU map with a recorded reason under `auto` |
| GPU-PLAN-08 fallback | Device absent; `shaderInt64` absent; injected pipeline-compile failure, launch failure and device loss mid-batch | map under `auto` and `require-gpu` | `auto`: values identical to CPU, a fallback diagnostic, work charged once. `require-gpu`: `device-failure`, no output |
| GPU-PLAN-09 interruption | Deadline and cancellation during a long kernel | map | `deadline`/`cancelled` distinct from quota/trap; no output; device buffers retired only after the fence |
| GPU-PLAN-10 differential random | Pinned PRNG and seeds; n up to the declared bound; generated eligible P up to the node bound | map on all variants | Bit-identical to the oracle; workgroup-size variation does not change results |
| GPU-PLAN-11 order | Index-encoding inputs (e.g. `x_i = i·2^32 + i`) | map | Output order equals input order |
| GPU-PLAN-12 determinism | The same batch launched repeatedly and across process restarts | map | Identical outputs and identical reports apart from timing and `executed_on` |

## Rejected alternatives

- **Inferring parallel regions automatically:** rejected initially (see the table above).
- **Using an empty effect bound as the eligibility test:** rejected. It says nothing about termination, bounded memory, data layout or independence (H-LIMIT-02).
- **Compiling runtime-composed Programs to kernels on first use:** rejected. It makes `run` a compiler service (W-EXEC-02).
- **Device-dependent quotas:** rejected. Runtime selection would change exhaustion outcomes.
- **FP32/FP64 or approximate arithmetic:** rejected. Not selected by K-NUM-01; #14 owns any FP profile.
- **Silent CPU fallback under a device requirement:** rejected. `require-gpu` must fail explicitly.

## Tests

This change runs only the structural Cairn gates and validation. GPU-PLAN-01…12 are designs with positive controls (01–05, 10–12) and negative controls (06–09). None has executed.

## Claim boundaries

- No GPU feasibility *measurement* and no performance claim. Every cost figure is [INFERENCE].
- API facts are quoted from the cited spec revisions (SPIR-V unified, Vulkan 1.4.365, WGSL CRD 21 Sept 2026, PTX ISA 9.4). OpenCL, MSL and HIP facts are [INFERENCE] pending pinned spec review.
- Observational equivalence between sequential and parallel evaluation of the subset is a design argument, not a proof. It needs a separate proof obligation.
- No change to canonical semantics, conformance state, evidence or proofs.

## Risks / Trade-offs

- Driver compilers are opaque trusted components. Bit-exact differential testing reduces but does not remove that trust.
- The u32-pair multiply is easy to get wrong at carry boundaries. GPU-PLAN-01 includes the 2^32 boundaries specifically.
- The explicit boundary may never be profitable at current list limits. The recommended outcome may be "no GPU until a packed batch type exists".
- New native GPU bindings conflict with `unsafe_code = "forbid"` unless confined in a separately reviewed boundary.

## Acceptance mapping

| Issue #2 research outcome / question | Section |
|---|---|
| Proposed subset (eligibility: operations/types, checked dependencies and captures, independent bounded work, device layout, exclusions) | [Decision: Eligible subset `GPU-Map-Draft`](#decision-eligible-subset-gpu-map-draft); r[GPU-SCOPE-01] |
| Explicit versus inferred dispatch (entry point) | [Decision: Explicit typed batch-map boundary first](#decision-explicit-typed-batch-map-boundary-first-no-inferred-parallel-regions); [Explicit versus inferred dispatch](#explicit-versus-inferred-dispatch) |
| Compile time versus runtime; eligibility ≠ selection | [Decision: Compile-time eligibility and kernel generation](#decision-compile-time-eligibility-and-kernel-generation-runtime-selection-only-among-prepared-kernels); r[GPU-DISPATCH-01] |
| Backend feasibility, incl. I64 wrapping per device API; no division/FP | [Backend feasibility per candidate device API](#backend-feasibility-per-candidate-device-api); [Decision: Exact I64 lowering](#decision-exact-i64-lowering-per-device-api); r[GPU-NUM-01] |
| End-to-end cost model | [End-to-end cost model](#end-to-end-cost-model) |
| Fallback and validation (unavailable, unsupported, uneconomic, launch failure, interruption, quota) | [Decision: Device-independent precharged quota](#decision-device-independent-precharged-quota-and-observable-equivalence); [Decision: Fallback, launch failure and interruption](#decision-fallback-launch-failure-and-interruption); r[GPU-FALLBACK-01] |
| Independent correctness tests: mathematical expectations, CPU/GPU comparison, overflow, operand order, captures, empty/small batches, eligibility rejection, fallback | [Correctness test plan](#correctness-test-plan-unexecuted) GPU-PLAN-01…12; r[GPU-GATE-01] |
