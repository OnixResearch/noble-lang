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
- **Measured evidence still needed:** `c_disp`, `c_alloc`, `c_pack`, `c_unpack` on the selected CPU route (**now measured below**); `L_h2d`, `L_d2h`, `B_*`, `T_launch`, `T_sync`, `T_pipe` and `c_op` for each lowering variant on pinned hardware and drivers (deferred by the NO-GO decision below).
- **Prerequisites the model exposes:** a packed `I64` batch representation and larger declared limits (coordinate with #3/#5/#14), plus benchmarking straight-line CPU Wasm compilation of `GPU-Map-Draft` kernels against per-word continuation dispatch.

### Phase 2 CPU-side cost study & GO/NO-GO decision

> `OWNER DECISION 2026-11-20` (Decisions 2 & 3): carry out the Phase-2 CPU-side cost study inside `gpu-offload-research`, and complete conditional downstream GPU implementation/device tasks by recording a negative outcome with measured evidence if the CPU-side study refutes their precondition.

**Evidence source [MEASURED]:** Clean-tree run of `verification/gpu-cost/study.mjs` on commit `21f636e6` (`source_revision` `sha256:c1c2a4df64fccbaad42f20826f0a85eff266923d20e874a070b78fb4a55f0711`), retained receipt [`verification/gpu-cost/runs/20261123/receipt.json`](../../../verification/gpu-cost/runs/20261123/receipt.json) (sha256 `cdc61643ffc10893dbe5f2b26981f8eb8e597b968316ad50776ff8199cb9ec1c`), executed on the selected Node 24.13.0 / V8 13.6.233.17-node.37 engine with the pinned `config.json` flags (`--no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up --stack-size=1024 --max-old-space-size=256`), `wasm-tools 1.245.1`, and `wasm-opt 125` (`-O2`), on x86_64 Linux (AMD Ryzen AI Max+ 395). All 390 K-NUM-01 boundary oracle checks (13 eligible kernels × 15 boundary inputs × `{off, on}`), all 10 ceiling controls, and 100% of timed samples (15 samples per cell) passed against the independent arbitrary-precision mathematical oracle `((z + 2^63) mod 2^64) - 2^63`.

#### 1. Empirically verified reachable `List<I64>` and work ceilings [MEASURED]

| Limit regime | At-bound pass | Over-bound refusal (`quota_reason`) | Observed memory / steps |
|---|---|---|---|
| Default `65,536` B, single `List<I64>` | `n = 682` (`status = 0`) | `n = 683` (`quota_reason = 1`, allocation) | `65,520` B (`1,365` cells) |
| Default `65,536` B, input + output `batch.map` | `n = 340` (`status = 0`) | `n = 341` (`quota_reason = 1`, allocation) | `65,376` B (`1,362` cells) |
| Max `196,608` B / `4096` cells, single `List<I64>` (`w2_add`, 6 init cells) | `n = 2,044` (`status = 0`) | `n = 2,045` (`quota_reason = 1`, allocation) | `4,095 / 4,096` cells (`196,272` B; zero-init theoretical max `2,047`, since `Nil` takes 1 cell) |
| Max `196,608` B / `4096` cells, input + output `batch.map` (`w2_add`) | `n = 1,022` (`status = 0`) | `n = 1,023` (`quota_reason = 1`, allocation) | `4,096 / 4,096` cells (`196,320` B) |
| Step limit `100,000` (`w64_arith`, `w = 64`) | `n = 1,562` (`status = 0`) | `n = 1,563` (`quota_reason = 6`, execution work) | `99,968` steps (`100,032` refused) |

Because Core sessions allocate from a fixed 4,096-cell non-reclaiming arena (`96` B/element for boxed `I64` + `List.Cons`), a single `batch.map` that holds both input and output `List<I64>` is capped at **`n <= 340`** under default limits and **`n <= 1,022`** at maximum limits (and even with a pre-existing input list from a prior submission, the total session cell cap still bounds input + output simultaneously to `<= 1,022` elements).

#### 2. Measured CPU helper costs at `n = 682` (medians across 15 samples) [MEASURED]

| Helper term (ns / element) | `opt: off` (IQR) | `opt: on` (`wasm-opt -O2`, IQR) | Notes |
|---|---:|---:|---|
| `c_alloc` (box `I64` + `List.Cons`, 2 cells) | `30.94` (`2.01`) | `29.57` (`2.82`) | Paid on both CPU and GPU (`List<I64>`) paths; cancels in `T_cpu - T_gpu` |
| `c_traverse` (linked-list read `$payload($a(node))`) | `6.38` | `6.32` | Pointer-chasing input walk |
| `c_pack` (traverse + contiguous `i64.store`) | `6.30` (`0.56`) | `5.94` (`0.53`) | Marginal flat store beyond traversal ≈ `0` ns/el (`-0.13` / `-0.28` ns) |
| `c_unpack_total` (flat `i64.load` + 2-cell rebuild) | `31.97` | `31.19` | Dominated by `c_alloc`; `c_load_only` ≈ `0.00` / `-0.01` ns/el |
| `c_unpack` (`max(0, unpack - alloc)`) | `0.62` | `1.32` | Marginal unpack beyond `c_alloc` is ~1 ns/el |

Under TurboFan (`--no-liftoff`), `c_pack + c_unpack` is small (`~6.9` ns/el `opt: off`, `~7.3` ns/el `opt: on`). Unlike naive Liftoff estimates, packing/unpacking does **not** make the break-even denominator negative; instead, the bottleneck is **batch size `n`** (the `340` / `682` / `1,022` linked-list ceiling) combined with **per-element dispatch time** and **GPU fixed launch/transfer/sync overhead**.

#### 3. Measured kernel dispatch `c_disp(w)`, end-to-end map `c_cpu_map(w)` and crossover bounds (`n = 682`)

Per-element medians (`ns/el`) are **[MEASURED]**; `n*(L_fixed)` and `L_max(n)` are derived from the measured denominator `denom_list = c_disp(w) - c_pack - c_unpack` under the **optimistic** assumption of zero GPU execution time and infinite H2D/D2H bandwidth (`1/B_* = c_op = 0`; any real device cost strictly raises `n*` and lowers `L_max`) **[INFERENCE / parametric lower bound]**:

| Kernel (`w` steps/el) | `opt` | `c_disp` (ns/el) | `c_disp/w` (ns/step) | `c_cpu_map` (ns/el) | `denom_list` (ns/el) | Optimistic `n*` at `L_fixed` = `50 / 100 / 200 / 500 µs` | Max affordable `L_fixed` at `n = 340 / 682 / 1,022` (`µs`) |
|---|---|---:|---:|---:|---:|---|---|
| `w2_add` (`[ 1 + ]`, `w=2`) | off / on | `26.41` / `53.46` | `13.2` / `26.7` | `79.30` / `130.74` | `19.49` / `46.20` | `2565/5130/10260/25649` / `1083/2165/4329/10823` | `6.6 / 13.3 / 19.9` / `15.7 / 31.5 / 47.2` |
| `m2_sq` (`[ dup * ]`, `w=2`) | off / on | `50.29` / `27.91` | `25.1` / `14.0` | `128.42` / `84.26` | `43.37` / `20.65` | `1153/2306/4612/11530` / `2422/4843/9685/24211` | `14.7 / 29.6 / 44.3` / `7.0 / 14.1 / 21.1` |
| `w3_sub_order` (`[ 10 swap - ]`, `w=3`) | off / on | `51.77` / `80.38` | `17.3` / `26.8` | `103.51` / `163.52` | `44.85` / `73.13` | `1115/2230/4460/11149` / `684/1368/2736/6838` | `15.3 / 30.6 / 45.8` / `24.9 / 49.9 / 74.7` |
| `w4_add2` (`[ 1 + 1 + ]`, `w=4`) | off / on | `76.68` / `76.90` | `19.2` / `19.2` | `168.29` / `115.30` | `69.76` / `69.65` | `717/1434/2867/7168` / `718/1436/2872/7180` | `23.7 / 47.6 / 71.3` / `23.7 / 47.5 / 71.2` |
| `m4_sub_sq` (`[ 3 - dup * ]`, `w=4`) | off / on | `91.84` / `61.76` | `23.0` / `15.4` | `166.41` / `123.95` | `84.93` / `54.50` | `589/1178/2356/5888` / `918/1835/3670/9175` | `28.9 / 57.9 / 86.8` / `18.5 / 37.2 / 55.7` |
| `w8_add4` (`w=8`) | off / on | `170.94` / `118.30` | `21.4` / `14.8` | `261.00` / `166.03` | `164.02` / `111.04` | `305/610/1220/3049` / `451/901/1802/4503` | `55.8 / 111.9 / 167.6` / `37.8 / 75.7 / 113.5` |
| `br_if` (`[ dup 0 = [ drop 7 ] [ 3 * ] if ]`, `w=8`) | off / on | `177.81` / `146.27` | `22.2` / `18.3` | `280.16` / `181.79` | `170.89` / `139.01` | `293/586/1171/2926` / `360/720/1439/3597` | `58.1 / 116.6 / 174.7` / `47.3 / 94.8 / 142.1` |
| `m9_poly` (`x^2+3x+7`, `w=9`) | off / on | `218.44` / `167.78` | `24.3` / `18.6` | `284.46` / `210.88` | `211.52` / `160.52` | `237/473/946/2364` / `312/623/1246/3115` | `71.9 / 144.3 / 216.2` / `54.6 / 109.5 / 164.1` |
| `w16_arith` (`w=16`) | off / on | `339.40` / `239.26` | `21.2` / `15.0` | `440.53` / `290.59` | `332.48` / `232.00` | `151/301/602/1504` / `216/432/863/2156` | `113.0 / 226.8 / 339.8` / `78.9 / 158.2 / 237.1` |
| `w32_arith` (`w=32`) | off / on | `743.33` / `474.83` | `23.2` / `14.8` | `907.59` / `525.41` | `736.41` / `467.57` | `68/136/272/679` / `107/214/428/1070` | `250.4 / 502.2 / 752.6` / `159.0 / 318.9 / 477.9` |
| `w64_arith` (`w=64`) | off / on | `1621.98` / `919.31` | `25.3` / `14.4` | `1559.78` / `907.87` | `1615.06` / `912.06` | `31/62/124/310` / `55/110/220/549` | `549.1 / 1101.5 / 1650.6` / `310.1 / 622.0 / 932.1` |

*(Full 13-kernel table including `w3_swap_add` and `w6_add3`, plus `n = 64, 256, 1000` sweeps and IQRs, is recorded in `verification/gpu-cost/runs/20261123/receipt.json`.)*

#### 4. GO / NO-GO Decision

**Decision: NO-GO for GPU offload on the current `List<I64>` representation and limits.**

1. **Reachable batches are far too small for typical kernels (`w = 2..9`) [MEASURED + INFERENCE]:** At `w = 2..4`, even under an optimistic warm GPU round-trip `L_fixed = 100 µs` with zero device compute and infinite bandwidth, `n* = 1,178–5,130` exceeds both the default `batch.map` ceiling (`n = 340`), the default single-list ceiling (`n = 682`), and the maximum `batch.map` ceiling (`n = 1,022`). At `w = 8..9`, `L_max` at `n = 340` is only `38–72 µs` (`opt: on` / `off`), and at `n = 1,022` only `113–216 µs` — below realistic warm Vulkan/WebGPU submit + sync + H2D/D2H latency plus kernel execution and cold `T_pipe` compilation.
2. **Wide-kernel `c_disp(w)` is continuation-dispatch overhead, not arithmetic cost [MEASURED]:** `c_disp(w)` scales linearly at **`~14–25 ns/word`** (`~919–1,622 ns/el` at `w = 64`) solely because the current Wasm backend lowers every word to an indirect continuation step (`$enqueue` + `call_indirect` + `$runtime_tick`). For the same straight-line `GPU-Map-Draft` scalar SSA subset, a direct CPU Wasm loop avoids per-word `call_indirect` and would cut CPU time by an order of magnitude without introducing GPU launch latency or an `unsafe` native driver trust boundary [INFERENCE]. Even against the current unspecialized Wasm dispatcher, the entire maximum-size `w = 64` batch (`n = 1,022`, `opt: on`) finishes on CPU in **`~0.93 ms`**, leaving no margin to amortize cold driver pipeline compilation (`T_pipe`) or device round-trips.
3. **Hard prerequisites before reopening GPU offload:**
   - A **packed contiguous `I64` batch representation** and **larger declared allocation/cell/work limits** (coordinate with `#3`/`#5`/`#14`), raising reachable `n` by orders of magnitude (`>= 10^4..10^6`) and removing per-element `List<I64>` cell overhead (`c_alloc`, `c_pack`, `c_unpack`).
   - A **straight-line CPU Wasm baseline** for `GPU-Map-Draft` kernels, so any future GPU candidate is compared against a compiled CPU loop rather than per-word continuation dispatch.
4. **Consequence for Phase 2 tasks 2–4 (Owner Decision 3, 2026-11-20):** Because task 1 refuted the economic precondition for GPU offload at current representations and limits, tasks 2–4 (independent eligibility checker & SPIR-V/WGSL kernel generator, GPU-PLAN-01…12 device runs, and hardware device cost measurements) are **not undertaken** and are closed as recorded negative conditional outcomes without introducing an inventoried native GPU binding.

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

Phase 1 ran only the structural Cairn gates and validation. Under `OWNER DECISION 2026-11-20` (Decisions 2 & 3), Phase 2 executed `verification/gpu-cost/study.mjs` (both `--self-test` and clean-tree run on commit `21f636e6`, receipt [`verification/gpu-cost/runs/20261123/receipt.json`](../../../verification/gpu-cost/runs/20261123/receipt.json), sha256 `cdc61643ffc10893dbe5f2b26981f8eb8e597b968316ad50776ff8199cb9ec1c`), verifying 390 K-NUM-01 boundary oracle evaluations, 10 reachable-ceiling quota controls, and 100% of helper/kernel timing samples on the selected Node/V8 Wasm route. GPU-PLAN-01…12 remain unexecuted because the CPU-side study recorded a NO-GO decision for GPU offload without a packed batch representation and larger limits.

## Claim boundaries

- **CPU-side measurements [MEASURED]:** `c_disp(w)`, `c_alloc`, `c_traverse`, `c_pack`, `c_unpack`, `c_cpu_map(w)` and the empirical `List<I64>` / step ceilings are measured on the selected Node 24.13.0 / V8 Wasm route (`verification/gpu-cost/runs/20261123/receipt.json`, clean commit `21f636e6`).
- **No GPU hardware measurement:** no GPU API, driver compiler, SPIR-V/WGSL/PTX kernel or device run has been executed. Parametric crossover `n*` and `L_max` figures assume zero GPU compute and infinite bandwidth as optimistic lower/upper bounds [INFERENCE].
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
