## Context

[numeric-graph-profile](../../archive/2026-10-16-numeric-graph-profile/design.md) (archived 2026-10-16; issue #14) fixed the `NumericGraph-Design` contract, decisions D1–D13 and P1–P6. It also ran Phase B and Phase C in `crates/noble-ngraph-experiment/` ([receipts](../../../verification/ngraph/README.md)). Its nine NGRAPH-* requirements are now canonical. This change selects a supported, opt-in implementation of that contract. It reuses every D-level semantic choice unchanged: the whitelist, binary32 reference, effect exclusions, D5 budgets, D6 codes, D7 extraction, D8 observations, D9 facts, D10 derivative policies, D11 training and snapshots, and D12 ownership. It adds only the library boundary, the user entry point and the durable file formats. r[NGRAPH-SCOPE-01]

**Owner decision (2026-10-16).** Phase C recommended **stop**. Warm W3-L L4 takes 9,711 ns against 1,271 ns host-native (0.131×), with 0.068× (W3-S), 0.061× (W3-M) and 0.182× (W4). Extracted W1/W2 reach 0.0077× and 0.0066× host-native. The threshold was at least 2× faster than both baselines, and GPU (#2) was blocked. The owner adopted the profile as a supported opt-in feature anyway, under four conditions: no performance or GPU claim; B0(b) stays (no kernel `F32`, K-NUM-01 untouched); NG-PO-A..H stay open; and all agreement evidence is finite differential evidence. This design does not reinterpret the measurement (canonical `NGRAPH-BENCH-01`, `NGRAPH-PROOF-01`). r[NGRAPH-SCOPE-01]

### Verified baseline (origin/main `66eb7cd`)

- **Architecture policy.**
  - `policy/architecture.contracts.ncl:17-25` defines the roles `domain-core`, `application-shell`, `inbound-adapter`, `outbound-adapter`, `composition-root`, `test` and `generated`.
  - `policy/architecture.ncl:35-46` has exactly two dependency rules: `domain-core → [domain-core]` and `composition-root → [domain-core]`. A composition-root package such as `noble-cli` may therefore depend only on a domain-core library.
  - Every provider classification (`architecture.ncl:85-142`) has `permitted_caller_roles = ["composition-root"]`. `nix/octet-controls.nix:96-110,167-170` makes that exact list a gate diagnostic (`policy-composition-root-providers-mismatch`).
  - The capability classifications (`architecture.ncl:144-187`) forbid core handling of filesystem, process, environment, time, concurrency, observability and unsafe code.
- **`no_std`.**
  - The policy contract requires only that every `core_no_std` entry be domain-core (`architecture.contracts.ncl:301-303`: `role_for_scope obligation.scope policy.roles == "domain-core"`). It does not require every domain-core package to be `no_std`.
  - `nix/octet-controls.nix:180-182` requires `core_no_std` to equal `requiredCorePackages` = noble-contracts, noble-kernel and noble-wasm exactly (`policy-core-no-std-mismatch`, `policy-core-no-std-missing`). Adding a fourth entry is therefore a control change.
  - All three existing domain-core crates are `#![no_std]` with `alloc`, and none has a third-party dependency.
- **Experiment crate.**
  - Its pure modules (`ir`, `diag`, `check`, `facts`, `sem`, `eval`, `compiled`, `grad`, `train`, `snapshot`, `prng`, `text`, `extract/{mod,admission,symbolic,source}`) contain no `std::fs`, `std::process`, `std::env::var`, `Instant`, thread or print calls.
  - They do use `std::sync::Arc`, `std::sync::atomic::AtomicU64` (scratch counters), `std::cell::Cell`, `std::collections::BTreeMap`, `env!("CARGO_PKG_VERSION")`, and std-only float functions: `f64::sqrt` (`adam/0`), `trunc` (facts), `powi` (grid facts), and `round`/`floor`/`ceil` (the corner-distance sampler).
  - `blake3` is used only through `ir::Canon` and `Subject::derive`. `rustc_apfloat` is used only by `oracle`.
- **Text form (`text.rs`).**
  - `print` (`:357-424`) emits no subject, closure, `input_shape` or `output_shape`, although `Graph::logical_id` (`ir.rs:863-894`) hashes all of them.
  - `parse` (`:174`) takes the subject from an argument.
  - `evidence` provenance is parsed as raw token bytes (`:219`) but printed as hex (`:416`).
  - `parse(print(g))` therefore does not preserve `GraphLogicalId` for any extracted graph.
  - Vectors, data and snapshots have no serialization.
- **CLI.**
  - Dispatch is a hand-written `if` chain (`crates/noble-cli/src/main.rs:37-144`).
  - Machine-facing commands print JSON report lines with a schema id, such as `noble-core-report/v1` (`core/output.rs:153`), through the CLI's own `workflow::encoding`.
  - Core exit codes (`core/output.rs:328-336`) are: 0 for normal, 1 for trap, 4 for unsupported, 5 for exhausted, and 2 for anything else (static rejection, usage, host errors).
  - `--help` (`main.rs:119-133`) omits `calculator`.
- **Docs.** User docs live in `README.md`: "### Experimental calculator (opt-in; acceptance not established)" (`:183-279`) and "## What works today" (`:579`). There is no `docs/` directory and no changelog.

## Decisions

### Decision: D1 Selected profile and entry point

**Choice:** The selected implementation profile is **`Numeric-Graph-v1`**. The hyphenated name follows `Declared-Modules-v1` and `Live-Slot-Design`; the canonical design label `NumericGraph-Design` stays as published, because renaming it would modify every NGRAPH requirement for no semantic gain.

- `Numeric-Graph-v1` is reachable only through `noble ngraph …` and the `noble-ngraph` library.
- It is not selected by `run`, `session`, `compile`, `live`, `calculator`, `build`, `component` or `verify`, and it adds no kernel type, word, frontend syntax or runtime asset.
- `ngraph-f32/0` graphs are authored in `ngraph-ir/0` and admitted only by the graph checker (B0(b)). Extraction exists for `ngraph-i64/0` only.
- `evidence` fact provenance stays refused (`E-FACT-UNCHECKED`, P5) until #8 supplies witness binding.

**Rationale:** This mirrors `Calculator-Design` → `Calculator-v1`: a disjoint, explicitly selected application entry that leaves every ordinary identity unchanged. r[NGRAPH-SCOPE-01] r[NGRAPH-CLI-01]

### Decision: D2 Crate split, roles and the no-dependency library

**Choice:** Create `crates/noble-ngraph/` (package `noble-ngraph`) with role **domain-core**.

- It is `#![no_std]` with `extern crate alloc`, and its only dependencies are the domain-core crates `noble-kernel` and `noble-contracts`. It has no third-party dependency, so no provider classification is involved.
- The experiment stays a composition root, depends on `noble-ngraph`, and keeps `blake3`, `serde_json` and `rustc_apfloat`.
- `noble-cli` (composition root) gains the `noble-cli → noble-ngraph` normal edge, which the existing `composition-root → [domain-core]` rule allows.
- `noble-ngraph → noble-kernel` and `noble-ngraph → noble-contracts` are allowed by `domain-core → [domain-core]`.
- No role rule, provider caller-role list or capability classification changes.

**Why `no_std` although policy does not require it.** Without `std`, the library cannot reach filesystem, process, environment, clock, thread or output APIs: the compiler enforces the NGRAPH-CLI-01 effect boundary. It also matches the three existing domain-core crates. Adding `noble-ngraph` to `core_no_std` is **not** part of this change. That needs `nix/octet-controls.nix` `requiredCorePackages` to change (`:77-81`, checked at `:180-182`), which is an owner decision (open question Q1). The crate is `no_std` either way.

**What `no_std` and the forbidden families require of the moved code:**

1. **Identity digests are injected.** The library defines
   ```rust
   pub trait IdentityHasher { fn update(&mut self, bytes: &[u8]); fn finish(self: Box<Self>) -> [u8; 32]; }
   pub trait IdentityScheme { fn name(&self) -> &'static str; fn start(&self) -> Box<dyn IdentityHasher + '_>; }
   ```
   - `ir::Canon`, `Subject::derive` and every identity function (`Graph::logical_id`, `layout_key`, `snapshot_id`, `data_digest`, the fact-binding digest, the `adam/0` state digest and the new provenance digest) stream exactly the same domain-separated byte sequence as today into the injected hasher.
   - The checker, snapshot constructors, trainer, extractor and parsers take `&dyn IdentityScheme`.
   - `noble-cli` and the experiment each implement `Blake3Scheme` (name `blake3`) over `blake3 =1.8.7` (features `std`, `pure`), so identities stay byte-identical to the experiment's. The CLI already depends on that crate; the experiment keeps it.
   - The library never hashes without a scheme and never names one.
   - **Rejected alternatives.** (a) Vendoring a BLAKE3 implementation into the library: about 1,000 lines of new trusted cryptographic code. (b) Changing provider caller roles: a policy loosening without approval. (c) A non-cryptographic hash: it would change every identity and weaken domain separation.
2. **Software float routines.** The library uses no std float method, and correct rounding is preserved by `sem::soft`:
   - `sqrt_f64` is correctly rounded (roundTiesToEven), computed with an integer square root on the 53-bit significand plus a sticky remainder bit;
   - `trunc_f64`, `floor_f64`, `ceil_f64` and `round_f64` (ties away from zero, as `f64::round` does) work by exponent and mantissa masking and are exact;
   - `pow2_f64(k)` builds the exact binary64 power of two from its bits and replaces `powi(-log2)`; it is exact for every grid `k` the checker admits.
   - `abs` stays core's `f32::abs`/`f64::abs`.
   - The corner-distance sampler (`grad.rs:664`) is used only by the B4 harness, so it moves to the experiment's `suites`.
   - Rejected: `core_float_math` (unstable, and it lowers to platform intrinsics) and a `libm` dependency (a provider edge).
3. **No atomics or shared-mutable globals.**
   - `ScratchCounters` becomes `Cell<u64>` fields. The only threaded use (`ngraph-phase-c-l1` W2 modes) moves CLI handles, not library values.
   - `Arc` becomes `alloc::rc::Rc`.
   - `env!("CARGO_PKG_VERSION")` in `train::tools()` becomes the constant `TOOL_REVISION = "noble-ngraph/0"`.
   - `std::error::Error` becomes `core::error::Error`; `BTreeMap` comes from `alloc`.

**Module map:**

| Experiment source | Destination | Change |
|---|---|---|
| `ir.rs` | `noble-ngraph::ir` | `Canon` streams into `IdentityHasher`; `Subject::derive` takes the scheme |
| `diag.rs` | `::diag` | Adds `E-FORMAT`, `E-SNAPSHOT-INTEGRITY`, `E-SNAPSHOT-ID` and `E-SNAPSHOT-REPLAY`, plus the D8 outcome class (D8 below) |
| `check.rs`, `facts.rs`, `compiled.rs` | `::check`, `::facts`, `::compiled` | `Rc`; software `trunc`/`pow2` |
| `sem.rs` (+ unit tests) | `::sem` | Adds `sem::soft` |
| `eval.rs` | `::eval` | `Cell` counters |
| `grad.rs` | `::grad` | The training forward pass enforces facts and guards (D10); the sampler moves out |
| `train.rs` | `::train` | Software `sqrt`; loss admission; parent-aware replay; `TOOL_REVISION` |
| `snapshot.rs` | `::snapshot` | Provenance record and digest |
| `prng.rs` (+ unit tests) | `::prng` | — |
| `text.rs` | `::format::graph` | Identity-complete, symmetric codec (D4) |
| — (new) | `::format::{value, vectors, data, snapshot}` | D5, D6 |
| `extract/{mod,admission,symbolic,source}.rs` | `::extract` | Scheme injection only |
| `extract/controls.rs` | experiment `controls` | Uses `serde_json` and the live-slot session; harness only |
| `evidence.rs`, `fixtures.rs`, `suites.rs`, `workloads.rs`, `random.rs`, `rejections.rs`, `differential.rs`, `oracle.rs`, `b2i64/`, `l1/`, `bench.rs`, `json.rs`, `bin/*` | stay in the experiment | Import `noble_ngraph::…`; `Blake3Scheme` |

This is a clean cutover: the experiment does not re-export library modules, and every caller imports `noble_ngraph::…`. `rustc_apfloat` is not a library dependency: the library computes binary32 with host `f32` operations (`sem.rs`, no FTZ/DAZ, no contraction), and the oracle exists only as an independent comparison leg. r[NGRAPH-CLI-01] r[NGRAPH-SCOPE-01]

### Decision: D3 Identity agreement with the experiment

**Choice:** Before any code moves, a one-off recorder runs on the base revision `66eb7cd` and writes `crates/noble-ngraph-experiment/tests/fixtures/identities-66eb7cd.txt`. It records:

- the `GraphLogicalId` and fact-binding digest of F1, F2, F3 (all nine rule pairs), the MSE loss, W3-S/M/L, W4, seeded random graphs 1–8 and the extracted `I1`–`I4`;
- the B5 data digest, the S₀ and final snapshot ids, the `adam/0` state digest and the per-step loss bits.

The permanent experiment test `identity_agreement.rs` recomputes all of these through `noble_ngraph` with `Blake3Scheme` and requires byte equality. It also cross-checks the authored ids against `phase-b-core.json`: F1 `6065e93f…d661`, final snapshot `075f58ca…7f9d`, S₀ `a912b177…8046`, data `7c40293c…0692`. Extracted ids are compared only with the recorder, because the kernel's `Debug` rendering may have changed since the receipt revision `f5806a0`.

**Rationale:** Injection and software routines must not change one identity bit. A frozen record from the replaced code is the only independent check once that code is gone. r[NGRAPH-SNAP-02]

### Decision: D4 Graph file format `ngraph-ir/0` (identity-complete text form)

**Choice:** A graph file is UTF-8 text.

- The first non-comment line is the schema line `ngraph-ir/0 <sub-profile>`.
- `#` starts a comment. Blank lines, comments and runs of spaces carry no meaning.
- The record kinds, in the printed order, are:
  ```text
  ngraph-ir/0 ngraph-f32/0
  subject authored 3be1…(64 hex) closure -        # or: subject authored-name f1-demo
  shape input s s                                 # optional; trees: s | u | (p A B)
  shape output s
  slot input f32
  slot param f32
  %0 = input 0
  %1 = param 1
  %2 = f32.mul %0 %1
  fact %0 range 0x00000000 0x3f800000 input-validated
  fact %2 finite derived interval
  out %2
  ```
- `subject <authored|extracted|captured> <64 hex> closure <decimal ids, comma-separated | ->` is required.
  - `subject authored-name <token>` is authoring sugar for `Subject::authored(token)` under the injected scheme, and `print` always emits the canonical hex form.
  - A missing, duplicate or malformed subject line is `E-GRAPH-MALFORMED`.
- `shape input|output <trees…>` records `Some(trees)`; an absent line records `None`. Zero trees means `Some([])`.
- Node, slot, fact and `out` lines keep today's grammar, with two corrections. `evidence <hex>` is decoded from hex, and odd length or non-hex digits are `E-GRAPH-MALFORMED`. `derived <rule>` requires its rule token. The checker still refuses `evidence` (P5).
- Producer origins are not printed and parse as `None`.

**What a graph file's identity covers.**

- `GraphLogicalId` is recomputed from the parsed records alone: the sub-profile with its semantic and fact-rule revisions, the nodes in order, the slots, both shapes, the outputs, and the subject kind, identity and closure (`ir.rs:863-894`).
- Fact records are not identity-bearing (unchanged D7 rule). They are bound through the fact-binding digest (D6, D10).
- Comments, whitespace, file names and paths are never identity-bearing.
- `print` is canonical: fixed record order, `F32` literals as `0x%08x`, `I64` in decimal. For every graph `g`, `parse(print(g))` equals `g` with origins cleared. That implies the same `GraphLogicalId`, the same fact records and the same checker outcome.
- Parsing only builds producer data: admission is still the checker's decision.
- Graph files are read under a 64 MiB bound.

**Rationale:** The scout and the director confirmed that the current form loses identity-bearing fields and is asymmetric for evidence. A user-facing file must round-trip identity, or snapshots bound to it could never be checked from disk. r[NGRAPH-FORMAT-01]

### Decision: D5 Vector and training-data files

**Choice:**

```text
noble-ngraph-vectors/0
types f32 f32
v 0x3fc00000 -0.5
v 0x7fc00000 0x80000000

noble-ngraph-data/0
types f32 f32 ; f32
point 0x3f000000 0x3e800000 ; 0x3f400000
```

- **Value tokens.** An `f32` value is either `0x` plus 8 hex digits (any encoding, including NaN, ±∞ and subnormals) or an exactly representable finite decimal; an inexact decimal is `E-LITERAL-INEXACT`. An `i64` value is a signed decimal within range. A `bool` value is `true` or `false`.
- `types` lists the graph's ordered **input** slot types. Parameters never come from these files (D3 of the archived design): they come only from a snapshot. For data files, the types after `;` are the loss graph's target slot types.
- A `types` line that differs from the graph is `E-INTERFACE`, before any evaluation. A wrong count on a row is `E-FORMAT`.
- An unknown or missing schema line is `E-FORMAT`, as is any other syntax error.
- Row order is preserved; the training-data digest binds it.
- Each file is read under a 64 MiB bound.

r[NGRAPH-FORMAT-01]

### Decision: D6 Snapshot files and immutable publication

**Choice:**

```text
noble-ngraph-snapshot/0
snapshot <64 hex>            # ParamSnapshotId = H(noble.ngraph.params.v0 ‖ graph ‖ slots ‖ LE bytes)
graph <64 hex>               # GraphLogicalId
layout <64 hex>              # param layout key
param <slot> 0x<8 hex>       # one line per param slot, in slot order
optimizer-state <64 hex>
parent <64 hex>
subject <kind> <64 hex> closure <ids|->
profile ngraph-f32/0 ngraph-sem/0
facts <64 hex>               # fact-binding digest of the model graph
loss <64 hex>
data <64 hex>
policy ngraph-deriv-real/0
opt-in %<node> boolean straight-through | boolean clamped-linear 0x<8 hex> | index zero | index backward-difference
seed 0x<16 hex>
prng splitmix64/0
optimizer adam/0 lr 0x<16 hex> beta1 0x<16 hex> beta2 0x<16 hex> eps 0x<16 hex>
steps <n>
tools noble-ngraph/0; ngraph-ir/0; ngraph-sem/0; ngraph-facts/0; ngraph-layered/0
provenance <64 hex>          # H(noble.ngraph.provenance.v0 ‖ every field above except this line)
```

- **Provenance.** The fields are structured, so replay rebuilds `DerivOptions` and `Adam` from them, and the library recomputes the canonical `describe()` strings. `opt-in` lines are sorted by node and policy kind.
- **Loading** recomputes three things: `layout` from the param slot list, `snapshot` from `graph`, the slots and the bytes, and `provenance` from the fields. Any mismatch is `E-SNAPSHOT-INTEGRITY`, raised before use. Snapshot files are read under a 32 MiB bound; the largest legal snapshot is 1,048,576 params at about 21 bytes per line.
- **Binding** to a checked graph runs these checks before any node is evaluated:
  - a `graph` other than the graph's `GraphLogicalId` is `E-SNAPSHOT-STALE`;
  - a `facts` digest other than the graph's fact-binding digest is `E-SNAPSHOT-STALE`, because fact records are not part of `GraphLogicalId`, and a graph whose facts changed must not silently accept parameters trained under other facts;
  - a different layout is `E-SNAPSHOT-LAYOUT`;
  - a caller-pinned `--snapshot-id` that differs is `E-SNAPSHOT-ID`.
- **Publication.** The CLI writes `<out>.partial` with create-new, syncs it, `hard_link`s it to `<out>` (which fails if `<out>` exists) and removes `<out>.partial`.
  - No path is ever truncated, renamed over or modified, including the parent snapshot, which is only read.
  - An existing `<out>` or `<out>.partial` is a host refusal (exit 2) that leaves both untouched.
  - A training call that stops publishes nothing; no partial snapshot is kept (D11 of the archived design).
  - `extract --out` uses the same publication.
- **Replay.** `verify --replay` rebuilds the training contract from the model, loss and data files, the recorded seed, policy, opt-ins, optimizer and steps, and an optional `--from` parent.
  - The model, loss, data and parent identities must equal the recorded `graph`, `loss`, `data` and `parent` values. Otherwise `E-SNAPSHOT-STALE`.
  - Replay then trains and requires byte-identical parameters, `optimizer-state` and `snapshot`. Otherwise `E-SNAPSHOT-REPLAY`.
  - Replay is defined on one pinned platform only.

r[NGRAPH-SNAP-02]

### Decision: D7 `noble ngraph` commands

**Choice:**

| Command | Required arguments | Options | Report (stdout, JSON lines `noble-ngraph-report/v1`) |
|---|---|---|---|
| `noble ngraph check GRAPH` | graph file | checker budgets | one line: `graph`, `profile`, `subject`, `closure`, `inputs` (types), `params` (count), `outputs` (types), `live_nodes`, `layers`, `facts`, `param_layout` |
| `noble ngraph extract SOURCE --inputs T[,T…] --out GRAPH` | Core source file: newline-separated submissions as in `noble session`, definitions first, root last; `--framed` selects session framing | `--profile ngraph-i64/0` (the only accepted value; anything else is `E-PROFILE`), extraction and checker budgets | one line: the `check` fields plus `work_units`, `expanded_nodes`, `pruned_nodes`, `inline_depth` |
| `noble ngraph eval GRAPH VECTORS` | graph file, vector file | `--reference`, `--snapshot FILE`, `--snapshot-id HEX`, `--fuel N`, checker budgets, `--budget-forward-scratch N` | a header line (`graph`, `snapshot` or null), then one line per vector: `vector`, `outcome`, `outputs` or `code`/`site` |
| `noble ngraph train MODEL --loss LOSS --data DATA --policy ngraph-deriv-real/0 --optimizer adam/0 --seed U64 --steps N --out SNAPSHOT` | as listed | `--from PARENT`, repeated `--opt-in %N:boolean=straight-through`, `%N:boolean=clamped-linear:0xBITS`, `%N:index=zero`, `%N:index=backward-difference`; `--adam-lr/-beta1/-beta2/-eps X` (binary64 bits `0x…` or a decimal rounded RNE; the exact bits are recorded; defaults are the `adam/0` values 0.02, 0.9, 0.999, 1e-8); checker budgets, `--budget-backward-scratch`, `--budget-snapshot-bytes`, `--budget-steps` | one line: `snapshot`, `graph`, `parent`, `steps`, `loss_initial`, `loss_final` (binary64 bits) |
| `noble ngraph verify GRAPH SNAPSHOT` | graph file, snapshot file | `--snapshot-id HEX`; `--replay --loss LOSS --data DATA [--from PARENT]` | one line: `snapshot`, `graph`, `bound: true`, `replayed: true|false` |

- **Usage.** Every subcommand refuses an unknown, duplicated, missing or inapplicable flag as a usage error (stderr usage, exit 2).
- **Explicit training contract.** `--policy`, `--optimizer`, `--seed` and `--steps` have no defaults. `--steps 0` publishes S₀ from the seed, which is the only way to create an untrained snapshot in v1 (Q4).
- **Values in reports.** `f32` values are `"f32:0x%08x"`, and every NaN encoding renders `"f32:nan"` (the D3 single class). `i64` values are `"i64:<decimal>"` and bools are `"bool:true|false"`.
- **Snapshot evaluation.** A graph with `param` slots and no `--snapshot` is `E-SNAPSHOT-LAYOUT` (existing preflight).
- **Dispatch.** A new `if` arm `ngraph` → `ngraph::run(&arguments)` goes in `main.rs`, with the module at `crates/noble-cli/src/ngraph.rs` plus `ngraph/{arguments,files,report}.rs`.
- **Help.** `--help` gains `ngraph::USAGE` and the missing calculator usage line, in the same list.
- **Effects.** The CLI reads only the named files, under their bounds and through the existing bounded-read pattern (`core/framing.rs:1-17`). It writes only `--out` and its `.partial` sibling, and runs no Node, Wasm, wasm-opt or subprocess.

r[NGRAPH-CLI-01]

### Decision: D8 Outcomes, diagnostics and exit codes

**Choice:** Every report line has `schema`, `command`, `stage` (`request|input|admission|extraction|check|binding|evaluation|training|replay|output`), `outcome`, `code` (string or null) and `site` (string or null). It may also have a `detail` string, which is informative only and not part of the stable contract. The outcome classes follow `core/output.rs:328-336`:

| Outcome | Exit | Codes |
|---|---|---|
| `normal` | 0 | — |
| `trap` | 1 | `G-FACT-FALSE` (guard failure or a refusing `index` read), `E-NONFINITE-UPDATE`, `C-CANCELLED` (no CLI cancellation source; listed for completeness) |
| `reject` | 2 | `E-GRAPH-MALFORMED`, `E-FORMAT`, `E-LITERAL-INEXACT`, `E-LITERAL-NONFINITE`, `E-TYPE-MIX`, `E-INTERFACE`, `E-SUBJECT` (including a Core frontend refusal of the extraction source), `E-CLOSURE`, `E-PROFILE` (an unknown sub-profile or a raised budget), `E-FACT-UNCHECKED`, `E-FACT-INPUT`, `E-PARAM-UNDECLARED`, `E-SNAPSHOT-STALE`, `E-SNAPSHOT-LAYOUT`, `E-SNAPSHOT-ID`, `E-SNAPSHOT-INTEGRITY`, `E-SNAPSHOT-REPLAY`; host conditions (`code: null`): an I/O failure, an exceeded input bound, an existing output |
| `unsupported` | 4 | `E-EFFECT`, `E-RESOURCE`, `E-DYNAMIC-RUN`, `E-LIVE-SLOT`, `E-RECURSION`, `E-CONTROL`, `E-AGGREGATE`, `E-REFLECT`, `E-PROGRAM-OUTPUT`, `E-DERIV-BOOLEAN`, `E-DERIV-INDEX`, `E-DERIV-I64` |
| `exhausted` | 5 | `E-BUDGET-<kind>`, including `FUEL` (runtime) and `STEPS` |

- Usage errors print usage on stderr and exit 2, as every other subcommand does.
- For `eval`, the process exit is the exit of the **first** non-`normal` vector in file order, or 0.
- A graph-level refusal (checker or snapshot binding) prints one report line and no vector lines.
- The four new codes are feature-level and do not change any D6 meaning:
  - `E-FORMAT`: a vector, data or snapshot file with a bad schema, syntax or row count;
  - `E-SNAPSHOT-INTEGRITY`: a recorded identity or provenance digest differs from its recomputation;
  - `E-SNAPSHOT-ID`: the snapshot differs from a caller-pinned id;
  - `E-SNAPSHOT-REPLAY`: replayed bytes differ.

**Rationale:** The director's mapping: budget → 5, unsupported and effect classes → 4, malformed or usage → 2. Guard failure and a non-finite update are run-time failures of an admitted computation, which is core's `trap` (1). `E-FACT-INPUT` and snapshot binding are static refusals of a request before any node runs, which core classes as `reject` (2). r[NGRAPH-CLI-01]

### Decision: D9 Budgets

**Choice:** Every D5 budget is a flag `--budget-<kind> N`, where kind is one of `work`, `expansion`, `inline-depth`, `live-nodes`, `layers`, `inputs`, `outputs`, `affine-terms`, `table-length`, `table-entries`, `facts`, `graph-bytes`, `snapshot-bytes`, `forward-scratch`, `backward-scratch` or `steps`. Each defaults to its `/0` value.

- A subcommand accepts only the kinds that it charges.
- A value above the default is `E-PROFILE` (raising needs a new profile policy revision, as in the experiment).
- A value that is not a `u64` is a usage error.
- `--fuel N` is host policy and has no default limit; exhaustion is reported as `E-BUDGET-FUEL`.
- The host input bounds (graph 64 MiB, vectors 64 MiB, data 64 MiB, snapshot 32 MiB, source 1 MiB with each submission at most 65,536 bytes) are fixed and documented. They are host limits, not profile budgets.

r[NGRAPH-CLI-01]

### Decision: D10 Training gaps closed in the library

**Choice:**

- **Forward-pass enforcement.** `grad::forward` currently skips guards and input facts (`grad.rs:284-288`: callers sample admissible points). The supported trainer first runs the reference preflight and the guard and index-refusal checks for every data point under the current parameters, in point order. The first failing point stops the step with its D8 code (`E-FACT-INPUT` or `G-FACT-FALSE`), and nothing is published.
- **Loss admission.** Training admission checks the loss graph: `ngraph-f32/0`, no `param` slot, input slots equal to the model outputs followed by the data target types, and exactly one `F32` output. Otherwise `E-INTERFACE`.
- **Parent-aware replay.** `replay` takes the parent snapshot: `train(c, Some(parent), …)`, with `parent` checked against the recorded id.

 r[NGRAPH-SNAP-02]

### Decision: D11 Determinism

**Choice:** For identical arguments and input bytes, stdout is byte-identical across runs.

- Reports have a fixed key order, carry no timing and print no file path.
- The default (compiled) and `--reference` evaluations produce identical bytes for every `normal`, `reject` and `trap` line.
- `exhausted` under `--fuel` may differ, because the observation point is evaluator-specific, as recorded in the archived design's interpretations.
- Two `train` runs with the same inputs write byte-identical snapshot files.

r[NGRAPH-CLI-01]

### Decision: D12 Experiment cutover and receipts

**Choice:**

- The Phase B/C evidence tests stay in the experiment: `b2_differential`, `b3_rejections`, `b4_gradients`, `b5_training`, `b6_ownership`, `c1_workloads`, `extract_b2` (ignored; Node) and `extract_b3`. They depend on the oracle, fixtures, suites, workloads or controls, and the receipts cite them by path.
- `ir_check.rs` and `extract_b1.rs` exercise only library code, so they move to `crates/noble-ngraph/tests/`. Their archived citation (`extract_b1.rs` in B1) stays valid at the receipt's recorded revision.
- The bins keep their names and flags. Their `change` field becomes `.cairn/archive/2026-10-16-numeric-graph-profile`.
- `verification/ngraph/README.md` gains `cargo test -p noble-ngraph` next to `cargo test -p noble-ngraph-experiment` and states that `test-log.txt` is historical. Its reproduce commands are otherwise unchanged.
- The `tools` provenance string changes from `noble-ngraph-experiment 0.0.0; …` to `noble-ngraph/0; …`. Snapshot ids are unaffected, because they do not bind `tools`.

**Deviation from the owner's direction (written reason):** "library tests move with their code" is applied to the tests whose subject is only library code. The Phase B/C tests are evidence tests whose cited paths and harness dependencies belong to the experiment. Moving them would either break the receipt citations or move harness-only fixtures into the library (canonical `NGRAPH-PROOF-01`, `NGRAPH-BENCH-01` evidence stays where it was recorded). r[NGRAPH-CLI-01]

### Decision: D13 Documentation

**Choice:**

- `README.md` gains "### Experimental numeric graphs (opt-in; no performance claim)" after the calculator section, covering the five commands, the formats, the exit-code table, the B0(b) and `evidence` limits, and the non-claims with the Phase C numbers. It also gains a "What works today" row.
- `noble --help` lists both `ngraph` and `calculator`.
- There is no changelog, because the repository has none.

r[NGRAPH-SCOPE-01]

## Test plan (unexecuted)

The default suite needs no Node and no external CLI: CLI tests use `env!("CARGO_BIN_EXE_noble")`, and no ngraph path spawns Node or Wasm. Every case below is in [the draft case file](cases/numeric-graph-cases.json): `absent / not-run / open / unassessed`, `evidence: []`.

- **`crates/noble-ngraph/tests`:**
  - `ir_check.rs` and `extract_b1.rs` (moved);
  - `format_graph.rs`: round trip on inline authored graphs and the extracted `I1`–`I4`; `authored-name` sugar; evidence hex symmetry; malformed subject and shape lines;
  - `format_values.rs`: vectors and data, with schema, `types`, inexact-decimal, NaN-bits and row-count controls;
  - `snapshot_file.rs`: round trip; tampered param, layout and provenance; stale graph or facts; pinned id;
  - `train_admission.rs`: data points failing a guard or input fact; loss arity; parent-aware replay; `E-SNAPSHOT-REPLAY` from a provenance-consistent forged file;
  - `soft_float.rs`: software routines against std on specials and 10⁷ seeded random bit patterns.
  - The test target links std; the library does not.
- **`crates/noble-ngraph-experiment/tests`:**
  - the existing Phase B/C tests, retargeted;
  - `identity_agreement.rs` (D3);
  - `format_roundtrip.rs`: the permanent identity round trip over F1–F3 (nine rule pairs), MSE, W3-S/M/L, W4, seeded random graphs 1–8 and the extracted `I1`–`I4`; requires structural equality with origins cleared, the same `GraphLogicalId`, the same fact-binding digest and the same checker outcome.
- **`crates/noble-cli/tests/ngraph.rs`** (NGRAPH-CASE-03..13): per subcommand, the success path and each outcome class; snapshot immutability and replay; wrong or stale snapshot refusal. Temporary directories come from `std::env::temp_dir()` with unique names.

## Integration impacts (for the integrator)

- **Workspace and lock.** `Cargo.toml` members add `crates/noble-ngraph`, and `Cargo.lock` changes. There are no new third-party crates: vendoring is unchanged except for the path-crate entries, so re-pin the vendor hash if the lock hash changes.
- **Tool selection.** `nix/tool-selection-files.nix` and `policy/tool-selection-files.ncl`/`.json` gain `crates/noble-ngraph/Cargo.toml` and new hashes for `crates/noble-cli/Cargo.toml` and `crates/noble-ngraph-experiment/Cargo.toml` (repin script).
- **Architecture policy** (`policy/architecture.ncl` and `policy/architecture.json`):
  - `production.packages` and `source_scopes` add `noble-ngraph` and `crates/noble-ngraph/src`;
  - roles add the package and source as `domain-core` and `crates/noble-ngraph/tests` as `test`;
  - the provider comment loses blake3, serde_json and rustc_apfloat as library callers (they stay experiment and CLI callers);
  - the filesystem and observability rationales gain the CLI's ngraph file reads, create-new and hard-link publication, `.partial` removal and report output;
  - no change to rules, provider roles or capability handling.
  - `core_no_std` and `nix/octet-controls.nix` `requiredCorePackages` change only if Q1 is approved.
- **Native assurance.** `policy/native-assurance.ncl`/`.json`: `required_packages` and the scope note go from six to seven crates. The experiment's provider rows stay, and there is no new row (the library has no external dependency).
- **Source inventory.** Renew `policy/source-inventory.ncl`/`.json` with `tools/source-inventory-renew.mjs` from a complete collection: new `noble-ngraph` paths, removed experiment paths and new `noble-cli` ngraph paths. Update `verification/source-inventory.md` and `verification/native-assurance.md` counts.
- **Octet capacity.**
  - Recompute `max_units`/`max_shards` (78 today): +2 for the `noble-ngraph` lib and lib+test, plus its integration tests; +1 for `noble-cli/tests/ngraph.rs`; −2 for the moved experiment tests; +2 for `identity_agreement` and `format_roundtrip`.
  - The `noble` bin and bin+test shards were already truncated at 16,384 calls and constructors and were doubled to 32,768 (`architecture.ncl:339-345`), so re-collect and raise `max_calls`, `max_constructors`, `max_spans` and `max_shard_bytes` only from observed complete counts.
- **Lints.** The new CLI module follows the existing `tigerstyle::*` `#[expect]` conventions and `octet::sealed_enum` for closed error enums.

## Open questions (recommended answers)

- **Q1** Add `noble-ngraph` to `core_no_std`? This needs `nix/octet-controls.nix` `requiredCorePackages` (and, for symmetry, `requiredPackages`/`requiredScopes`) to include it. **Recommend yes, as a separately approved tightening.** The crate is `no_std` regardless.
- **Q2** The extracted subject identity is BLAKE3 over the kernel's `Debug` rendering (`admission.rs:521-545`), which is stable only within one kernel revision. **Recommend keeping it for v1 and documenting it.** A kernel change changes `GraphLogicalId`, so old snapshots are refused as stale (fail closed). A canonical kernel encoding is a separate change.
- **Q3** Persist the `adam/0` moments so that training can resume? **Recommend no.** `--from` restarts the optimizer state at zero and records the parent, and the snapshot records the state digest only.
- **Q4** Snapshots from caller-written values? **Recommend deferring.** Archived D3 supplies parameters only through snapshots, and `--steps 0` covers S₀.
- **Q5** Keep the last published snapshot when training stops? **Recommend no.** Report `published_steps` in the report line and publish nothing.
- **Q6** Bind fact records into `GraphLogicalId`? **Recommend no.** It would change NGRAPH-SNAP-01 identities and every Phase B id. Binding uses the fact-binding digest instead (D6).
- **Q7** Is the `eval` exit the first non-normal vector or the most severe? **Recommend the first, in file order.** It is deterministic and matches "the first failure", and every outcome is in the report.

## Risks / Trade-offs

- **Library-local software floats** add trusted code. The exhaustive-special and random-pattern tests (NGRAPH-CASE-14) are finite evidence, not a proof.
- **Injected hashing** means a composition root can supply a wrong scheme. The scheme name is recorded in reports, and `identity_agreement` pins the BLAKE3 results.
- **Strict JSON-only reports** make interactive use verbose. This is acceptable for a machine-facing profile and matches core.
- **Performance.** The supported evaluator is 0.06–0.18× host-native (Phase C). Users must not read selection as speed.

## Non-claims

- No performance, speedup, GPU or backend-selection claim. The stop measurement stands.
- No proof: NG-PO-A..H stay open and are not added to `obligations.json`. Agreement, hashes and replay are finite differential evidence.
- No change to K-NUM-01, V-NUM-01, PO-09/10/11, ordinary Programs or any existing CLI output. No cross-platform bit-reproducibility claim.
