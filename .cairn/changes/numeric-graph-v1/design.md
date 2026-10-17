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

**`no_std`, now also required by policy.** Without `std`, the library cannot reach filesystem, process, environment, clock, thread or output APIs: the compiler enforces the NGRAPH-CLI-01 effect boundary. It also matches the three existing domain-core crates. Q1 was approved as a tightening, and the integrator applied it on `integration/ngraph-v1`: `policy/architecture.ncl` `core_no_std` adds `noble-ngraph` and `dylint.toml` `core_source_scopes` adds `crates/noble-ngraph/src` (`e5decb9`), and `nix/octet-controls.nix` adds it to `requiredCorePackages`, `requiredPackages` and `requiredScopes`, with rejection controls for a missing no-std entry and an omitted package (`ff4e9ba`). Dropping the crate from production or from `core_no_std` is now a policy diagnostic.

**Core-scope lints.** With `crates/noble-ngraph/src` in `core_source_scopes`, the core dylint lints report on the library (warning-only while deny-all stops in `noble-kernel`, #26). Two groups are fixed at the source, with the boring design:

- `interior_mutability_in_core`: the B6 scratch counters were `Cell<u64>` fields shared through `&ScratchCounters`. They are now plain `u64` fields owned by the caller and lent for one call: `Control::scratch` and `TrainControl::scratch` hold `Option<&mut ScratchCounters>`, the evaluators and `train`/`replay` take `&mut Control`/`&mut TrainControl`, and a `ScratchLease` holds the exclusive borrow, so its drop is still the single release. The B6 evidence semantics are unchanged: the caller reads `(acquired, released, live_bytes)` and the peak after every outcome (normal, guard failure, preflight refusal, cancellation, fuel exhaustion, a stopped training step). The reference interpreter's evaluated-node counter is a plain local.
- `concrete_construction_in_core`: `extract::source::prepare` constructed the frontend `Session` and its `Limits`. The caller now injects both (`prepare(Session::new(), Limits::default(), …)` in the CLI, the experiment and the tests).
- `mutating_input_in_pure` stays a measured warning: it reports every `&mut` parameter (builders, encoders, parsers, evaluator workspaces), and the counter change above adds a few more such parameters. Recasting those APIs as returned state transitions is not a contained change.

**Lint style.** The library follows `noble-kernel` and `noble-contracts`: every module imports the `alloc` items it uses explicitly (no glob prelude, so no `#[allow(unused_imports)]`), and no `#[allow]` remains. The pedantic clippy allows (`too_many_lines`, the cast lints) were no-ops, because the workspace gate runs clippy's default lint set with `-D warnings`, so they are deleted rather than turned into `#[expect]`, which would fail as unfulfilled. `sem::index_position` returns the named error `IndexRefused` instead of `()` (clippy `result_unit_err`). The library uses no `tigerstyle::`/`octet::` lint attribute, so, unlike `noble-kernel` and `noble-contracts`, it registers no tools; the CLI keeps its existing `#[expect(tigerstyle::…, reason = "Owner: …")]` convention.

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
   - `trunc_f64`, `trunc_f32`, `floor_f64`, `ceil_f64` and `round_f64` (ties away from zero, as `f64::round` does) work by exponent and mantissa masking and are exact;
   - `pow2_f64(k)` builds the exact binary64 power of two from its bits and replaces `powi(-log2)`; it is exact for every grid `k` the checker admits.
   - `abs` stays core's `f32::abs`/`f64::abs`.
   - The corner-distance sampler (`grad.rs:664`) is used only by the B4 harness, so it moves to the experiment's `suites`.
   - Rejected: `core_float_math` (unstable, and it lowers to platform intrinsics) and a `libm` dependency (a provider edge).
3. **No atomics or shared-mutable globals.**
   - `ScratchCounters` holds plain `u64` fields lent as `&mut` for one call (see "Core-scope lints" above; the first cutover used `Cell<u64>`). The only threaded use (`ngraph-phase-c-l1` W2 modes) moves CLI handles, not library values.
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
| `eval.rs` | `::eval` | Caller-owned scratch counters lent as `&mut` |
| `grad.rs` | `::grad` | The training forward pass enforces facts and guards (D10); the sampler moves out |
| `train.rs` | `::train` | Software `sqrt`; loss admission; parent-aware replay; `TOOL_REVISION` |
| `snapshot.rs` | `::snapshot` | Provenance record and digest |
| `prng.rs` (+ unit tests) | `::prng` | — |
| `text.rs` | `::format::graph` | Identity-complete, symmetric codec (D4) |
| — (new) | `::format::{value, vectors, data, snapshot}` | D5, D6 |
| `extract/{mod,admission,symbolic,source}.rs` | `::extract` | Scheme injection; the frontend session and limits are injected too |
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
- The schema line must have exactly two tokens. A missing schema line (or trailing tokens on it) is `E-GRAPH-MALFORMED`; an unknown sub-profile is `E-PROFILE`.
- `subject <authored|extracted|captured> <64 hex> closure <decimal ids, comma-separated | ->` is required exactly once, anywhere after the schema line.
  - `subject authored-name <name>` is authoring sugar for `Subject::authored(name)` under the injected scheme. The name is the rest of the line: one or more tokens joined by single spaces, so fixture names such as `issue14/MSE point loss` can be spelled. `print` always emits the canonical hex form.
  - A missing, duplicate or malformed subject line is `E-GRAPH-MALFORMED`.
- `shape input|output <trees…>` records `Some(trees)`; an absent line records `None`. Zero trees means `Some([])`. A duplicate or malformed shape line is `E-GRAPH-MALFORMED`.
- **Shape depth.** A shape tree nests at most 127 pairs (`ir::MAX_SHAPE_DEPTH`). The bound is what extraction can produce: the Core frontend limits every value type to 256 structural nodes (`noble-contracts` `TYPE_CAP`, which it sends as the kernel request's `type_size`, and the kernel counts one node per constructor), and a path through `d` nested pairs needs at least `2d + 1` nodes, so an extracted interface item nests at most 127 pairs. The bound holds at construction for every producer:
  - `ShapeTree` is opaque, and `ShapeTree::pair` is its only pair constructor; it refuses a 128th nested pair with `E-GRAPH-MALFORMED` (payload), so the builder can only ever hold shallow trees. Code reads trees through `ShapeTree::node()`. This replaces the public enum variants, a deliberate exception to the IR module's additive-change rule;
  - the parser counts the enclosing pairs while it reads a tree and refuses the 128th `(p` with `E-GRAPH-MALFORMED` at that line before descending any further;
  - extraction refuses a root interface item with more nested pairs as `E-AGGREGATE` at admission (only a forged submission, with type limits above the frontend's, can reach it).
  - Reason: identity hashing, printing, comparison and drop walk shape trees recursively. The independent review found that a 40,000-deep tree in a 240 KB file aborted `check` with a stack overflow (exit 134, no report line); with the bound it is one coded report line, exit 2.
- **Number tokens.** Every number is spelled without a sign other than a leading `-` on signed values (`i64` literals, `grid`): node ids `%N`, slot indices, closure ids and other counts are ASCII digits, an `F32` bit pattern is `0x` plus one to eight hex digits (the vector-file rule, D5), and a `derived` rule escape is `%` plus two hex digits. A leading `+` is `E-GRAPH-MALFORMED`. Rust's integer parsers accept a leading `+` (also after `0x` in `from_str_radix`), which made `+3`, `0x+1` and `%+f` valid by accident; the same strict grammar now applies to every file and every `noble ngraph` flag (D5, D6, D7).
- Node, slot, fact and `out` lines keep today's grammar, with these corrections so that printing and parsing are symmetric for every graph value, including ones the checker refuses:
  - `evidence <hex>` is decoded from hex, and `evidence -` is an empty witness. Odd length or non-hex digits are `E-GRAPH-MALFORMED`. The checker still refuses `evidence` (P5).
  - `derived <rule>` requires its rule token. Rules are printed percent-encoded (every byte outside printable ASCII, and `%`, `#` and `"`, as `%XX`), and the empty rule is printed `""`. A bad escape is `E-GRAPH-MALFORMED`.
  - Range-fact literals are typed: `0x…` is binary32, `true|false` is Bool and `i64:N` is an explicit `I64`; an unprefixed decimal takes the sub-profile's type. `print` writes `F32` as `0x%08x`, an `I64` in an `ngraph-i64/0` graph in decimal and an `I64` in an `ngraph-f32/0` graph as `i64:N`.
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

- **Value tokens.** An `f32` value is either `0x` plus one to eight hex digits (any encoding, including NaN, ±∞ and subnormals) or an exactly representable finite decimal; an inexact decimal is `E-LITERAL-INEXACT` and a `nan`/`inf` word is `E-LITERAL-NONFINITE` (NaN and infinities are given as bits). An `i64` value is ASCII digits within range, optionally after `-`. A `bool` value is `true` or `false`. Any other token is `E-FORMAT`, including a leading `+` (D4 number tokens). Graph files read `F32` literals with the same rules.
- **Decimals.** A decimal is `[-]digits[.digits][(e|E)[+|-]digits]`, where either side of the `.` may be empty but not both (Rust's float grammar without the leading `+` and the `inf`/`nan` words). Exactness is decided in time linear in the token (`sem::exact_f32_decimal`):
  - one pass strips leading zeros and trailing fraction zeros, giving `±D × 10^E` with `D` free of zeros at either end; an overlong exponent saturates;
  - every exact binary32 value `M × 2^q` (`M < 2^24`, `-149 ≤ q ≤ 104`) satisfies `digits(D) ≤ 112` (`(2^24 − 1) × 5^149 < 10^112`), `-149 ≤ E ≤ 38` and `-44 ≤ digits(D) + E ≤ 39` (`2^-149 > 10^-45`, `2^128 < 10^39`); a literal outside that window is `E-LITERAL-INEXACT` before any big-integer work;
  - inside the window, the comparison of `D × 10^E` with the nearest binary32 value works on at most 112 digits and exponents of at most 149.
  - The codes are unchanged for every token: inexact or out-of-range decimals stay `E-LITERAL-INEXACT`, `nan`/`inf` words `E-LITERAL-NONFINITE`. The full expansions of `2^-149` (105 digits), of `(2^24 − 1) × 2^-149` (112 digits) and of the largest finite value are accepted, and one more significant digit is refused.
  - Reason: the previous check built a big integer digit by digit and multiplied it by powers of ten once per unit of exponent, quadratic in the digit count; the review measured 7.1 s for one 400,000-digit token, and a 64 MiB file bound alone does not bound CPU time.
- `types` lists the graph's ordered **input** slot types. Parameters never come from these files (D3 of the archived design): they come only from a snapshot. For data files, the types after `;` are the loss graph's target slot types.
- The `types` line must appear exactly once, before any row; it may list zero types. A data file's `types` and `point` lines contain exactly one `;` (it may touch its neighbours).
- A `types` line that differs from the graph is `E-INTERFACE`, before any evaluation. A wrong count on a row is `E-FORMAT`.
- An unknown or missing schema line is `E-FORMAT`, as is any other syntax error.
- A file is parsed completely before anything is evaluated: any bad row refuses the whole file. Input-fact validation (`E-FACT-INPUT`) stays per vector, because it is a property of a row's values, not of the file.
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
param <slot> 0x<8 hex>       # one line per param slot: interface slot index and bits
optimizer-state <64 hex>
parent <64 hex>
subject <kind> <64 hex> closure <ids|->
profile ngraph-f32/0 ngraph-sem/0
facts <64 hex>               # fact-binding digest of the model graph
loss <64 hex>
data <64 hex>
policy ngraph-deriv-real/0
opt-in %<node> boolean refuse|straight-through|clamped-linear 0x<8 hex> | index refuse|zero|backward-difference
seed 0x<16 hex>
prng splitmix64/0
optimizer adam/0 lr 0x<16 hex> beta1 0x<16 hex> beta2 0x<16 hex> eps 0x<16 hex>
steps <n>
tools noble-ngraph/0; ngraph-ir/0; ngraph-sem/0; ngraph-facts/0; ngraph-layered/0
provenance <64 hex>          # the provenance digest below
```

- **Text form.** Records appear in exactly this order, one per line, with `param` and `opt-in` repeated; anything else (an unknown, missing, reordered or trailing record) is `E-FORMAT`. Hex is lowercase. `param` lines are printed in slot order but may appear in any order when read: their order is the recorded slot order, and binding decides whether it is the graph's layout. `opt-in` lines must be strictly ordered by node and then kind (`boolean` before `index`). `#` comments are accepted as in every other file.
- **Provenance.** The fields are structured, so replay rebuilds `DerivOptions` and `Adam` from them, and the library recomputes the canonical `describe()` strings. For a snapshot trained from the seed, `parent` is the id of S₀ (the snapshot of the seed initialization); `--steps 0` publishes S₀ with `parent = snapshot`.
- **Identity encoding.** Every identity uses the library's canonical stream `H(domain ‖ 0x00 ‖ fields)` under the injected scheme (BLAKE3 in the CLI), where `bytes(x) = u64le(len x) ‖ x`, `str(s) = bytes(utf8 s)`, and integers are fixed-width little-endian:
  - `layout = H("noble.ngraph.param-layout.v0" ‖ u64le(n) ‖ (u32le(slot) ‖ str("f32"))*)` over the `param` slots in record order;
  - `snapshot = H("noble.ngraph.params.v0" ‖ bytes(graph) ‖ u64le(n) ‖ u32le(slot)* ‖ bytes(u32le(bits)*))`;
  - `provenance` is the encoding of every record in file order except the schema and `provenance` lines: `H("noble.ngraph.provenance.v0" ‖ bytes(snapshot) ‖ bytes(graph) ‖ bytes(layout) ‖ u64le(n) ‖ (u32le(slot) ‖ u32le(bits))* ‖ bytes(optimizer-state) ‖ bytes(parent) ‖ str(subject kind) ‖ bytes(subject identity) ‖ u64le(#closure) ‖ u64le(id)* ‖ str(sub-profile) ‖ str(semantic revision) ‖ bytes(facts) ‖ bytes(loss) ‖ bytes(data) ‖ str(policy) ‖ u64le(#opt-ins) ‖ (u32le(node) ‖ str(kind) ‖ str(rule) ‖ [u32le(width bits)])* ‖ u64le(seed) ‖ str(prng) ‖ str(optimizer) ‖ u64le(lr bits) ‖ u64le(beta1 bits) ‖ u64le(beta2 bits) ‖ u64le(eps bits) ‖ u64le(steps) ‖ str(tools))`, where the width is present only for `clamped-linear` and `tools` is the record's text after `tools `.
  - Digest values enter as their 32 bytes, not as hex. `crates/noble-ngraph/tests/snapshot_file.rs` and `crates/noble-cli/tests/ngraph.rs` each re-implement this encoding from this text and reproduce the published identities.
- **Loading** recomputes three things: `layout` from the param slot list, `snapshot` from `graph`, the slots and the bytes, and `provenance` from the records. Any mismatch is `E-SNAPSHOT-INTEGRITY`, raised before use, and so are two records that `train` can never publish, however consistent their digests: a non-finite parameter, and `optimizer` hyperparameters outside the `adam/0` domain (D7). More parameter bytes than `--budget-snapshot-bytes` is `E-BUDGET-SNAPSHOT-BYTES`. Snapshot files are read under a 32 MiB bound; the largest legal snapshot is 1,048,576 params at about 21 bytes per line. Numbers in snapshot files follow the D4 number tokens (no leading `+`; `E-FORMAT`).
- **Binding** to a checked graph runs these checks before any node is evaluated:
  - a `graph` other than the graph's `GraphLogicalId` is `E-SNAPSHOT-STALE`;
  - a `facts` digest other than the graph's fact-binding digest is `E-SNAPSHOT-STALE`, because fact records are not part of `GraphLogicalId`, and a graph whose facts changed must not silently accept parameters trained under other facts;
  - a different layout is `E-SNAPSHOT-LAYOUT`;
  - a caller-pinned `--snapshot-id` that differs is `E-SNAPSHOT-ID`.
- **Publication.** The CLI writes `<out>.partial` with create-new, syncs it, `hard_link`s it to `<out>` (which fails if `<out>` exists) and removes `<out>.partial`.
  - The link decides the outcome. Once it exists, `<out>` holds the complete, immutable bytes, so the command reports its normal line (exit 0) even if removing `<out>.partial` then fails. That leftover is a second name for the same bytes; the CLI warns about it on stderr (`noble ngraph: the output was published, but its .partial file could not be removed: …`), and it matters only to a later publication to the same `--out` after `<out>` is removed, which refuses the existing `.partial` as usual. Reporting a refusal there would claim that nothing was published while `<out>` exists.
  - If writing or syncing `<out>.partial` or the link fails, the CLI removes `<out>.partial`; if that removal fails too, the file is left behind and the refusal says so.
  - No path is ever truncated, renamed over or modified, including the parent snapshot, which is only read.
  - An existing `<out>` or `<out>.partial` is a host refusal (exit 2) that leaves both untouched.
  - A training call that stops publishes nothing; no partial snapshot is kept (D11 of the archived design).
  - `extract --out` uses the same publication.
- **Replay.** `verify --replay` rebuilds the training contract from the model, loss and data files, the recorded seed, policy, opt-ins, optimizer and steps, and an optional `--from` parent.
  - The model, loss, data and parent identities must equal the recorded `graph`, `loss`, `data` and `parent` values. Otherwise `E-SNAPSHOT-STALE`.
  - Replay then admits the recorded contract exactly as `train` admits its flags (D7, D10). A recorded opt-in naming no node of the model, or a node its policy does not apply to, is `E-DERIV-BOOLEAN` or `E-DERIV-INDEX` (stage `replay`, exit 4), as `train --opt-in` with the same opt-in is. Hyperparameters outside the `adam/0` domain never reach replay: loading refuses them. Integrity only proves that the digests match the records, so without this a re-signed forgery with opt-ins that `train` refuses replayed as `replayed: true`.
  - Finally, replay trains and requires byte-identical parameters, `optimizer-state` and `snapshot`. Otherwise `E-SNAPSHOT-REPLAY`.
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

- **Usage.** Every subcommand refuses an unknown, duplicated, missing or inapplicable flag, a missing or extra positional argument and a malformed flag value as a usage error: stdout stays empty, stderr starts with `noble ngraph:` followed by the usage text, and the exit is 2. A bare `noble ngraph` or an unknown subcommand is a usage error too.
- **Streams.** Writes to stderr never panic: a usage error still exits 2, and a report that cannot be written exits 2, when stderr itself fails (`eprintln!` exited 101 there). If stdout is closed rather than failing, Rust's standard output discards the report without an error, so the outcome's exit stands.
- **One line.** `check`, `extract`, `train` and `verify` print exactly one report line for every outcome other than a usage error. `command` is the subcommand name.
- **`eval` lines.** All static work runs before anything is printed: the graph (read, parse, check), the snapshot (load, integrity, binding, or `E-SNAPSHOT-LAYOUT` for a graph with `param` slots and no `--snapshot`) and the vector file (parse, `types` interface). Any such refusal is the only line. Otherwise the header comes first (no `vector` key; `graph`; `snapshot` id or null), then one line per row: `vector` (the 0-based row index) and `outputs` for a normal row, or `code`, `site` and `detail` and no `outputs` otherwise.
- **`check` fields.** `subject` is the subject record text `<kind> <64 hex> closure <ids|->`; `closure` is the array of definition identities; `inputs` and `outputs` are arrays of type names; `params`, `live_nodes`, `layers` and `facts` are counts (`facts` counts fact records); `graph` and `param_layout` are lowercase hex.
- **`extract` source.** Newline-separated submissions with blank lines skipped, or `noble session` framing with `--framed` (a decimal byte-count line, then that many bytes). Every submission before the root must be a definition; otherwise `E-SUBJECT`, like any Core frontend refusal. `--inputs` takes one or more of `i64` and `bool`; a Program-typed input cannot be spelled, so `E-DYNAMIC-RUN` is reachable only through the library (the experiment's B3 controls).
- **`train` values.** `--seed` is decimal digits or `0x` plus one to 16 hex digits, `--steps` decimal digits; no flag value takes a leading `+` (D4 number tokens; usage error). A `--policy` or `--optimizer` other than `ngraph-deriv-real/0` or `adam/0` is `E-PROFILE`. On a training refusal the line also carries `published_steps`, and nothing is published. `loss_initial` and `loss_final` are the first and last recorded binary64 mean losses as `0x%016x`.
- **`--adam-*` values.** Each takes `0x` plus exactly 16 hex digits (the binary64 encoding) or a decimal (D5 grammar) rounded to nearest, ties to even; anything else, including `nan`, `inf` and a short hex encoding, is a usage error. A well-formed value outside the domain that `adam/0` needs is `E-PROFILE` at stage `request`, with the policy and optimizer names and before any file is read (`train::Adam::admit`): every value finite, `lr > 0`, `eps > 0` and `0 ≤ β₁, β₂ < 1`, compared as numbers, so `-0` is a zero β. The archived D11 names `adam/0` but no domain; outside this one the bias correction `1 − βᵗ` or the denominator `√v̂ + eps` can be zero, the moments never decay, or the step ascends. The review found `nan`, `inf`, negative and zero values accepted, and an `lr` of NaN even published S₀ and replayed. `train` itself (and so replay) re-admits the domain, and loading refuses a recorded out-of-domain optimizer (D6).
- **`--opt-in`.** Malformed text, and two policies of one kind on one node, are usage errors. Whether an opt-in applies is training admission in the library, shared with replay (D6, D10): one naming no node of the model, or a node its rule does not apply to (`straight-through` and `refuse`: `step` or `select`; `clamped-linear`: `step`; index rules: `index`), is `E-DERIV-BOOLEAN` or `E-DERIV-INDEX` at stage `admission` (exit 4). These are the codes of the derivative policy, whose default-refused paths they already name; an inapplicable opt-in is a derivative option the policy does not support for that node. The first implementation refused these in the CLI as usage errors, which replay could not apply to a recorded opt-in.
- **`verify` flags.** `--loss`, `--data` and `--from` apply only with `--replay`, which needs both `--loss` and `--data`.
- **Explicit training contract.** `--policy`, `--optimizer`, `--seed` and `--steps` have no defaults. `--steps 0` publishes S₀ from the seed, which is the only way to create an untrained snapshot in v1 (Q4).
- **Values in reports.** `f32` values are `"f32:0x%08x"`, and every NaN encoding renders `"f32:nan"` (the D3 single class). `i64` values are `"i64:<decimal>"` and bools are `"bool:true|false"`.
- **Snapshot evaluation.** A graph with `param` slots and no `--snapshot` is `E-SNAPSHOT-LAYOUT` (the only line, stage `binding`).
- **Dispatch.** A new `if` arm `ngraph` → `ngraph::run(&arguments)` goes in `main.rs`, with the module at `crates/noble-cli/src/ngraph.rs` plus `ngraph/{arguments,files,report}.rs`.
- **Help.** `--help` gains `ngraph::USAGE` and the missing calculator usage line, in the same list.
- **Effects.** The CLI reads only the named files, under their bounds (metadata length first, then a bounded read; refusal messages carry no path). It writes only `--out` and its `.partial` sibling, and runs no Node, Wasm, wasm-opt or subprocess.

r[NGRAPH-CLI-01]

### Decision: D8 Outcomes, diagnostics and exit codes

**Choice:** Every report line starts with `schema`, `command`, `stage` (`request|input|admission|extraction|check|binding|evaluation|training|replay|output`), `outcome`, `code` (string or null) and `site` (string or null), in that order. Refusal lines may also have a `detail` string, which is informative only and not part of the stable contract. Every coded line carries a string `site` (`graph`, `node %N`, `slot K`, `fact I`, `source D:N` or `source root:N`, `line L`), except the run-time observations `E-BUDGET-FUEL` and `C-CANCELLED`, whose site is null. Host conditions have a null code and site. Stages: `request` (budget policy and profile, policy or optimizer names), `input` (reading and parsing files, including the interface of vector and data files), `check`, `admission` (the Core frontend, D7 admission and training admission), `extraction`, `binding`, `evaluation`, `training`, `replay` and `output` (publication). The outcome classes follow `core/output.rs:328-336`:

| Outcome | Exit | Codes |
|---|---|---|
| `normal` | 0 | — |
| `trap` | 1 | `G-FACT-FALSE` (guard failure or a refusing `index` read), `E-NONFINITE-UPDATE`, `C-CANCELLED` (no CLI cancellation source; listed for completeness) |
| `reject` | 2 | `E-GRAPH-MALFORMED`, `E-FORMAT`, `E-LITERAL-INEXACT`, `E-LITERAL-NONFINITE`, `E-TYPE-MIX`, `E-INTERFACE`, `E-SUBJECT` (including a Core frontend refusal of the extraction source), `E-CLOSURE`, `E-PROFILE` (an unknown sub-profile or one the command does not accept, a raised budget, a `--policy` or `--optimizer` other than `ngraph-deriv-real/0` or `adam/0`, or `adam/0` hyperparameters outside its domain, D7), `E-FACT-UNCHECKED`, `E-FACT-INPUT`, `E-PARAM-UNDECLARED`, `E-SNAPSHOT-STALE`, `E-SNAPSHOT-LAYOUT`, `E-SNAPSHOT-ID`, `E-SNAPSHOT-INTEGRITY`, `E-SNAPSHOT-REPLAY`; host conditions (`code: null`): an I/O failure, an exceeded input bound, an existing output |
| `unsupported` | 4 | `E-EFFECT`, `E-RESOURCE`, `E-DYNAMIC-RUN`, `E-LIVE-SLOT`, `E-RECURSION`, `E-CONTROL`, `E-AGGREGATE`, `E-REFLECT`, `E-PROGRAM-OUTPUT`, `E-DERIV-BOOLEAN`, `E-DERIV-INDEX`, `E-DERIV-I64` |
| `exhausted` | 5 | `E-BUDGET-<kind>`, including `FUEL` (runtime) and `STEPS` |

- Usage errors print usage on stderr and exit 2, as every other subcommand does.
- A usage error and a report-output failure exit 2 even when stderr cannot be written.
- For `eval`, the process exit is the exit of the **first** non-`normal` vector in file order, or 0.
- A graph-level refusal (checker or snapshot binding) prints one report line and no vector lines.
- The four new codes are feature-level and do not change any D6 meaning:
  - `E-FORMAT`: a vector, data or snapshot file with a bad schema, syntax or row count;
  - `E-SNAPSHOT-INTEGRITY`: a recorded identity or provenance digest differs from its recomputation, or the file records what `train` never publishes: a non-finite parameter or `optimizer` hyperparameters outside the `adam/0` domain (D6 Loading);
  - `E-SNAPSHOT-ID`: the snapshot differs from a caller-pinned id;
  - `E-SNAPSHOT-REPLAY`: replayed bytes differ.

**Rationale:** The director's mapping: budget → 5, unsupported and effect classes → 4, malformed or usage → 2. Guard failure and a non-finite update are run-time failures of an admitted computation, which is core's `trap` (1). `E-FACT-INPUT` and snapshot binding are static refusals of a request before any node runs, which core classes as `reject` (2). r[NGRAPH-CLI-01]

### Decision: D9 Budgets

**Choice:** Every D5 budget is a flag `--budget-<kind> N`, where kind is one of `work`, `expansion`, `inline-depth`, `live-nodes`, `layers`, `inputs`, `outputs`, `affine-terms`, `table-length`, `table-entries`, `facts`, `graph-bytes`, `snapshot-bytes`, `forward-scratch`, `backward-scratch` or `steps`. Each defaults to its `/0` value.

- A subcommand accepts only the kinds that it charges.
  - `check`: the checker kinds `live-nodes`, `layers`, `inputs`, `outputs`, `affine-terms`, `table-length`, `table-entries`, `facts` and `graph-bytes`;
  - `extract`: `work`, `expansion`, `inline-depth` and the checker kinds;
  - `eval`: the checker kinds, `forward-scratch` and `snapshot-bytes` (and `--fuel`);
  - `train`: the checker kinds (model and loss), `snapshot-bytes`, `backward-scratch` and `steps`;
  - `verify`: the checker kinds, `snapshot-bytes`, and `backward-scratch` and `steps`, which `--replay` charges.
- A budget is exhausted when the observed count exceeds the limit; a limit equal to the observed count or to the default is accepted.
- `backward-scratch` is charged before the first step on the per-step lease, `16 × (model nodes + loss nodes)` bytes.
- A value above the default is `E-PROFILE` (raising needs a new profile policy revision, as in the experiment).
- A value that is not a `u64` written in decimal digits is a usage error (no sign, no leading `+`).
- `--fuel N` is host policy and has no default limit; exhaustion is reported as `E-BUDGET-FUEL`.
- The host input bounds (graph 64 MiB, vectors 64 MiB, data 64 MiB, snapshot 32 MiB, source 1 MiB with each submission at most 65,536 bytes) are fixed and documented. They are host limits, not profile budgets.

r[NGRAPH-CLI-01]

### Decision: D10 Training gaps closed in the library

**Choice:**

- **Forward-pass enforcement.** `grad::forward` skips guards and input facts (callers of the B4 campaigns sample admissible points). The supported trainer (`grad::objective_checked`) first runs the reference preflight, including param facts under the current parameters, then scans the same binary32 forward values for the first failing guard or refusing index read in canonical order, for the model and then for the loss graph (whose input facts are validated too), point by point. The first failing point stops the step with its D8 code (`E-FACT-INPUT` or `G-FACT-FALSE`), and nothing is published. The final-loss evaluation after the last step applies the same rule; a refusal there also publishes nothing (fail closed).
- **Loss admission.** Training admission checks the loss graph: `ngraph-f32/0`, no `param` slot, input slots equal to the model outputs followed by the data target types, and exactly one `F32` output. Otherwise `E-INTERFACE`.
- **Parent-aware replay.** `replay` takes the parent snapshot: `train(c, Some(parent), …)`, with `parent` checked against the recorded id.
- **Replay re-admission.** `replay` trains through `train`, so the recorded contract passes the same admission as a new one: the `adam/0` domain (`E-PROFILE`) and, in `grad::admit`, the applicability of every opt-in (`E-DERIV-BOOLEAN`, `E-DERIV-INDEX`) before the default-refused paths.

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

## Test plan

The default suite needs no Node and no external CLI: CLI tests use `env!("CARGO_BIN_EXE_noble")`, and no ngraph path spawns Node or Wasm. The tests below ran; their results are recorded in [tasks.md](tasks.md). The registered [case file](../../../specs/conformance/numeric-graph-cases.json) now records a 14-passed/1-held candidate: eight cases retain immutable historical acceptance, six additional execution records retain their preliminary packet and bind fresh full-source safety observations, and CASE-02 remains held. The exact integrated-source cold-review and bounded terminal receipts govern the six-record disposition. Passing tests alone are not complete case acceptance.

- **`crates/noble-ngraph/tests`:**
  - `ir_check.rs` and `extract_b1.rs` (moved); `refusals_before_evaluation_evaluate_no_node` directly observes input-fact, interface and missing/stale-snapshot eval refusals at zero evaluated nodes in both library evaluators. The approved NGRAPH-CASE-08 mapping uses the exact three named refusal classes, not CLI counters or CLI guard/F1 bytes. This test does not measure NGRAPH-CASE-04 `check`, guest/protected-operation counters or every NGRAPH-CASE-12 parse/integrity path;
  - `format_graph.rs`: round trip on inline authored graphs and the extracted `I1`–`I4`; `authored-name` sugar; evidence hex symmetry; malformed subject and shape lines; the 127-pair shape bound in the parser and in `ShapeTree::pair`, including the 40,000-deep repro; number tokens without `+` and with at most eight hex digits;
  - `format_values.rs`: vectors and data, with schema, `types`, inexact-decimal, NaN-bits and row-count controls; the decimal window's boundaries (the expansions of `2^-149`, `(2^24 − 1) × 2^-149` and the largest finite value, one more digit, a 100,001-digit token, saturated exponents); no leading `+`;
  - `snapshot_file.rs`: round trip; an independent implementation of the D6 provenance-digest encoding; tampered param, layout and provenance; opt-in order; stale graph or facts; pinned id; swapped params with recomputed identities; re-signed optimizers outside the `adam/0` domain; `+` in records;
  - `train_admission.rs`: data points failing a guard or input fact; loss arity; parent-aware replay; `E-SNAPSHOT-REPLAY` from a provenance-consistent forged file; the `adam/0` domain and opt-in applicability, each refused alike by `train` and by `replay` of a record;
  - `soft_float.rs`: software routines against std on specials, every power of two and 100,000 seeded patterns per routine, with an ignored 10⁷ tier (`cargo test --release -p noble-ngraph --test soft_float -- --ignored`).
  - The test targets link std and use a test-only SipHash identity scheme (`tests/common`), so the library has no third-party dependency of any kind; BLAKE3 identities are pinned by the experiment.
- **`crates/noble-ngraph-experiment/tests`:**
  - the existing Phase B/C tests, retargeted;
  - `identity_agreement.rs` (D3);
  - `format_roundtrip.rs`: the permanent identity round trip over F1–F3 (nine rule pairs), MSE, W3-S/M/L, W4, seeded random graphs 1–8 and the extracted `I1`–`I4`; requires structural equality with origins cleared, the same `GraphLogicalId`, the same fact-binding digest and the same checker outcome.
  - `format_roundtrip.rs` also re-parses every corpus graph with a leading comment, blank lines, trailing comments and runs of spaces, and requires the same graph, identity, fact-binding digest and checker outcome (`comments_blank_lines_and_spacing_do_not_change_any_corpus_graph`; NGRAPH-CASE-06 variants on the whole corpus).
- **Library unit tests:** `extract::admission` refuses a root interface item with 128 nested pairs (`E-AGGREGATE`) and admits 127.
- **`crates/noble-cli/tests/ngraph.rs`** (NGRAPH-CASE-03..13): per subcommand, the success path and each outcome class; snapshot immutability and replay; wrong or stale snapshot refusal. Temporary directories come from `std::env::temp_dir()` with unique names. The first 14 tests were written independently from this design (`feat/ngraph-cli-tests`, 429da01) before the CLI existed and passed unchanged; two tests added at integration cover the forged-snapshot cases (CASE-12 swapped params, CASE-13 replay) with an independent implementation of the D6 encoding, and the recorded B5 identities through the CLI (CASE-10, fixture `tests/ngraph/b5.points`). Four tests cover the boundary review: the 40,000-deep shape repro (one line, exit 2), failing stdout and stderr pipes (exit 2, not a panic), strict numbers and the `adam/0` domain, and re-signed snapshots with out-of-domain optimizers or opt-ins that `train` refuses.
- **`noble-cli` unit test:** `ngraph::files` publication outcomes (a linked output stays published when its `.partial` removal fails).

## Implementation notes

- `CheckedGraph::fact_bindings(ids)` computes the fact-binding digest on demand, so checking a graph costs no extra hash; it is identical to the experiment's B5 `facts` value (D3).
- `extract::source::prepare` refuses an executable submission before the root (it would run in `noble session` but has no meaning for extraction).
- Training admission order is the budget policy, the `adam/0` domain (`E-PROFILE`), the params (`E-PARAM-UNDECLARED`), then the derivative policy (`E-DERIV-I64`, opt-in applicability, the default-refused paths), then the loss interface; the CLI checks the `adam/0` domain at stage `request` and runs the rest before reading the data file, so an `ngraph-i64/0` model is `E-DERIV-I64` whatever its data.
- The readings of the black-box test worker (agent `noble-ngraph-cli-tests`) were settled on 2026-10-16 and are folded into D5–D9: `command` is the subcommand name; coded lines carry a string `site` except the run-time observations; one line per non-usage outcome except `eval`; usage errors keep stdout empty; the `eval` header; whole-file refusal of vector files; `facts` is a count; `param <slot>` is the interface slot index; a seed-trained snapshot's parent is S₀; lowercase hex with `provenance` last; inapplicable flags are usage errors; and a budget is exceeded only when observed > limit.
- **Independent boundary review (2026-10-16).** The review of `20df84c` (agent `noble-ngraph-cli-tests`; repros outside the repository) found two bugs, two specification mismatches and six notes. The eight items selected for this pass are fixed at the source, each with a regression test and a decision recorded above:
  - a 40,000-deep `shape` tree aborted `check` with a stack overflow: the 127-pair bound (D4);
  - decimal exactness was quadratic in the digit count: the linear window (D5);
  - `eprintln!` panicked (exit 101) when stderr failed: non-panicking writes (D7 streams, D8);
  - `--adam-*` accepted `nan`, `inf`, negative and zero values: the strict grammar and the `adam/0` domain in `train`, on load and through replay (D6, D7);
  - replay accepted forged opt-ins that `train` refuses: opt-in applicability moved into training admission (D6, D7, D10);
  - a failed `.partial` removal after a successful link was reported as a refusal: the link decides (D6);
  - a leading `+` was accepted in flags and values: refused everywhere (D4, D5, D9);
  - 39 `#[allow]` in the library, 20 of them on a glob prelude: explicit imports, no `#[allow]` (D2 lint style).
  - With them, the two contained core-scope dylint groups were fixed (D2: `interior_mutability_in_core`, `concrete_construction_in_core`).
  - Before and after, on the reviewer's release build of `20df84c` and on this one: the 40,000-deep tree went from exit 134 with no report to one `E-GRAPH-MALFORMED` line (exit 2); one 400,000-digit token from about 7 s (6.9 and 7.5 s in two runs) to 4 ms (both `E-LITERAL-INEXACT`); a usage error with stderr on `/dev/full` from exit 101 to 2; `--adam-lr nan` from a published S₀ to a usage error and `--adam-lr -1` to `E-PROFILE`; the reviewer's forged snapshots from `replayed: true` to `E-DERIV-BOOLEAN` and `E-DERIV-INDEX` (exit 4) and from bound to `E-SNAPSHOT-INTEGRITY` (NaN `lr`); `--steps +3` from 3 steps to a usage error.
  - Not changed: the `.partial` file is removed by path rather than by handle, the directory is not synced after the link and unlink (D6 requires syncing the file only), and a closed stdout still loses the report with the outcome's exit (D7 streams).

## Integration impacts (for the integrator)

**Measured integration result.** The complete local run at
`bc10df848ed81d53e849c440c1f639979e40d38d` is retained in
`verification/main-gates/receipt.json`: 10/14 checks pass, with boundary-controls
(#26/#27), octet (#26), source-coverage and native-assurance (#25) failing.
`rust-tests` reports 529 passed, 0 failed, 6 ignored in 77 binaries. Historical
publication `0eb5c0478ebf3419de6f94b8144c2ac7520356ca` added the acceptance
runner, inputs and docs without changing the tested Rust/Cargo/policy/Nix
implementation; final acceptance and claims review were then pending.
The later immutable acceptance and independent review support only the
eight recorded cases in the lifecycle section below. They do not rerun or
relabel the historical gate logs. CASE-02 capability closure remains open,
and NG-PO-A..H stay OPEN. The change stays active and is not archived.

- **Workspace and lock.** `Cargo.toml` members add `crates/noble-ngraph`, and `Cargo.lock` changes. There are no new third-party crates: vendoring is unchanged except for the path-crate entries, so re-pin the vendor hash if the lock hash changes.
- **Tool selection.** `nix/tool-selection-files.nix` and `policy/tool-selection-files.ncl`/`.json` gain `crates/noble-ngraph/Cargo.toml` and new hashes for `crates/noble-cli/Cargo.toml` and `crates/noble-ngraph-experiment/Cargo.toml` (repin script).
- **Architecture policy** (`policy/architecture.ncl` and `policy/architecture.json`):
  - `production.packages` and `source_scopes` add `noble-ngraph` and `crates/noble-ngraph/src`;
  - roles add the package and source as `domain-core` and `crates/noble-ngraph/tests` as `test`;
  - the provider comment loses blake3, serde_json and rustc_apfloat as library callers (they stay experiment and CLI callers);
  - the filesystem and observability rationales gain the CLI's ngraph file reads, create-new and hard-link publication, `.partial` removal and report output;
  - no change to rules, provider roles or capability handling.
  - Q1 (approved; applied in `e5decb9` and `ff4e9ba`): `core_no_std` and `nix/octet-controls.nix` `requiredCorePackages`, `requiredPackages` and `requiredScopes` add `noble-ngraph`, and `dylint.toml` `core_source_scopes` adds `crates/noble-ngraph/src`.
- **Native assurance.** `policy/native-assurance.ncl`/`.json`: `required_packages` and the scope note go from six to seven crates. The experiment's provider rows stay, and there is no new row (the library has no external dependency).
- **Source inventory.** Renew `policy/source-inventory.ncl`/`.json` with `tools/source-inventory-renew.mjs` from a complete collection: new `noble-ngraph` paths, removed experiment paths and new `noble-cli` ngraph paths. Update `verification/source-inventory.md` and `verification/native-assurance.md` counts.
- **Octet capacity.**
  - Recompute `max_units`/`max_shards` (78 today). As implemented: `noble-ngraph` adds 9 units (lib, lib+test, and the integration tests `ir_check`, `extract_b1`, `format_graph`, `format_values`, `snapshot_file`, `train_admission`, `soft_float`); the experiment is net 0 (`ir_check` and `extract_b1` moved out, `identity_agreement` and `format_roundtrip` added); `noble-cli` adds `tests/ngraph.rs`. Expected 88 units, to be confirmed by a complete collection.
  - The `noble` bin and bin+test shards were already truncated at 16,384 calls and constructors and were doubled to 32,768 (`architecture.ncl:339-345`), so re-collect and raise `max_calls`, `max_constructors`, `max_spans` and `max_shard_bytes` only from observed complete counts.
- **Lints.** The new CLI module uses `attempt!` instead of `?` and `#[expect(tigerstyle::assertion_density, …)]` on its fallible entry points, following the crate's conventions. The library has no `#[allow]` and fixes the core-scope `interior_mutability_in_core` (6 findings in the integrator's collection of `ff4e9ba`) and `concrete_construction_in_core` (4) groups at the source (D2); `mutating_input_in_pure` stays a measured warning, and the `&mut` scratch counters add a few findings to it. The Octet/tigerstyle gates were not run in this change.
- **Capacity after the review fixes.** The new tests are added to existing targets, so the unit count (88) does not change; the library, CLI and test shards grow slightly, so the call, constructor and span counts need the complete re-collection anyway.
- **Edges.** New normal edges: `noble-ngraph → noble-kernel`, `noble-ngraph → noble-contracts`, `noble-ngraph-experiment → noble-ngraph`, `noble-cli → noble-ngraph`. `noble-ngraph` has no dev-dependency; its tests use a test-only SipHash identity scheme. `dylint.toml` `core_source_scopes` gained `crates/noble-ngraph/src` with the Q1 tightening (`e5decb9`).

## Open questions (recommended answers)

- **Q1** Add `noble-ngraph` to `core_no_std`? This needs `nix/octet-controls.nix` `requiredCorePackages` (and, for symmetry, `requiredPackages`/`requiredScopes`) to include it. **Recommend yes, as a separately approved tightening.** The crate is `no_std` regardless. Approved and applied by the integrator (`e5decb9`, `ff4e9ba`; D2).
- **Q2** The extracted subject identity is BLAKE3 over the kernel's `Debug` rendering (`admission.rs:521-545`), which is stable only within one kernel revision. **Recommend keeping it for v1 and documenting it.** A kernel change changes `GraphLogicalId`, so old snapshots are refused as stale (fail closed). A canonical kernel encoding is a separate change.
- **Q3** Persist the `adam/0` moments so that training can resume? **Recommend no.** `--from` restarts the optimizer state at zero and records the parent, and the snapshot records the state digest only.
- **Q4** Snapshots from caller-written values? **Recommend deferring.** Archived D3 supplies parameters only through snapshots, and `--steps 0` covers S₀.
- **Q5** Keep the last published snapshot when training stops? **Recommend no.** Report `published_steps` in the report line and publish nothing.
- **Q6** Bind fact records into `GraphLogicalId`? **Recommend no.** It would change NGRAPH-SNAP-01 identities and every Phase B id. Binding uses the fact-binding digest instead (D6).
- **Q7** Is the `eval` exit the first non-normal vector or the most severe? **Recommend the first, in file order.** It is deterministic and matches "the first failure", and every outcome is in the report.

## Review

The director reviewed this design (proposal, tasks, D1–D13) on 2026-10-16 under the owner's 2026-10-16 adoption instruction and approved it for implementation; the owner may amend any decision. Answers:

- **Q1:** yes. The integrator adds `noble-ngraph` to `core_no_std` and to `nix/octet-controls.nix` `requiredCorePackages`, `requiredPackages` and `requiredScopes` as a tightening.
- **Q2–Q7:** as recommended above.
- **Q8:** the `NGRAPH` evidence-route row in `tools/check-specs.mjs` (added while promoting `numeric-graph-profile`) is accepted, following the `CB` precedent (commit `9a2e409`).

## Lifecycle record

The registered [NGRAPH-CASE-01..15](../../../specs/conformance/numeric-graph-cases.json)
retain the original immutable [acceptance receipt](../../../verification/numeric-graph-v1/acceptance.json),
[independent review receipt](../../../verification/numeric-graph-v1/review-receipt.json),
and [build provenance](../../../verification/numeric-graph-v1/build-provenance.json).
Those original records support only cases 01/06/07/08/10/13/14/15 at source
`sha256:2f4ebb1dc161687fc9844e9ab69cb1f992caf27bd6fd6ecd83aa0ff9d30f6f9f`;
the build provenance binds the 707 build/policy inputs byte-identical at
published `0eb5c04` and tested `bc10df8`. Their prepromotion records, source
receipts and external raw outputs remain immutable.

The historical safety-observation packet supplies directly measured finite
expected-field evidence for CASE-03/04/05/09/11/12, including guest/protected-
operation counters and CASE-04/12 node-zero observations. Its source-002d
packet and independent preliminary replay remain preserved as historical
support. Separate fresh full-source observations bind the final candidate's
current runner, binaries and build-source manifest; the final cold-review and
bounded terminal receipts bind their exact packet and source hashes. Every
case retains `proof: open`; none of these execution records rewrite or relabel
historical acceptance/review receipts, close global gates, or authorize archive.

CASE-02 remains `absent/not-run/open/unassessed` with `evidence: []`: complete
compiler-authenticated zero-use capability closure is still missing, with
637 relevant unknowns in the historical record. The actual arbitrary `std`
consumer of the `no_std` library produced `I64(7)` while recording 42 reads
and 42 writes; it is a finite adversarial boundary control, not a CASE-02
capability proof. No input, expected field, case kind, profile, policy,
capacity or requirement is weakened to change this boundary.

The current selected-tool boundary is complete for ordinary source-only
coverage: the corrected r8 final index binds all 88 raw shards with zero
canonical mismatches and the archive-after-run closure binds all 2,094
extracted source rows unchanged. The new permanent wrapper verifies the
retained full-88 artifact (`status: valid`, no diagnostics, exit 0). These
receipts establish neither authenticated origin nor Architecture PASS; the
historical producer/runner exit 1, Architecture blocked state, 4,389 open
required unknowns and zero waivers are preserved. CASE-02 and its exact
capability-closure obligation remain open. The selected Octet package's
deliberate `no_unwrap` negative fixture is denied while its clean positive
control passes; this is expected lint behavior, not a compiler blocker.

The [independent claims review](../../../verification/numeric-graph-v1/review.json)
and [lifecycle evidence record](../../../verification/numeric-graph-v1/README.md)
retain the original supplemental runtime packet path and hash: CASE-01
adds 12 actual CORE-03/05/09 runtime inputs per binary to the 31 CORE rows;
CORE-15/16 remain name-bound cargo controls. CASE-08's node-zero evidence is
the approved direct library mapping, not a CLI measurement. CASE-06's
text variants and CASE-14's software-float comparisons have the finite mapped
scope stated in those records. No proof, performance, GPU, backend-selection
or cross-platform claim follows.

Canonical selection and registration do not close the feature. The seven
incomplete contracts and historical four global gate failures (#25/#26/#27)
leave archive unauthorized, and [tasks.md](tasks.md) keeps archive open.
CALC records and `obligations.json` remain unchanged.

## Risks / Trade-offs

- **Library-local software floats** add trusted code. The exhaustive-special and random-pattern tests (NGRAPH-CASE-14) are finite evidence, not a proof.
- **Injected hashing** means a composition root can supply a wrong scheme. The scheme name is recorded in reports, and `identity_agreement` pins the BLAKE3 results.
- **Strict JSON-only reports** make interactive use verbose. This is acceptable for a machine-facing profile and matches core.
- **Performance.** The supported evaluator is 0.06–0.18× host-native (Phase C). Users must not read selection as speed.

## Non-claims

- No performance, speedup, GPU or backend-selection claim. The stop measurement stands.
- No proof: NG-PO-A..H stay open and are not added to `obligations.json`. Agreement, hashes and replay are finite differential evidence.
- No change to K-NUM-01, V-NUM-01, PO-09/10/11, ordinary Programs or any existing CLI output. No cross-platform bit-reproducibility claim.
