## Context

`noble ngraph eval GRAPH VECTORS --net` lowers each vector row to a fresh revision-3 net and reduces it. Before reduction it re-admits the net and validates it independently (`crates/noble-ngraph/src/net.rs`). The CLI (`crates/noble-cli/src/ngraph/net.rs`) buffers the header and every row line, then writes them in one pass. Any non-normal row replaces the report with a single line. Lattice#14/#22 need enforced ceilings, a stated count basis and stable identities before they can consume this path. The design keeps the pure domain core (`noble-ngraph`, `no_std` + `alloc`) free of host effects. File reads, report construction and the build identity stay in the composition root (`noble-cli`).

## Decisions

### Decision D1: an accounted retained-byte ceiling in the library

**Choice:** Add `net::Limits::max_retained_bytes` (default `net::DEFAULT_MAX_RETAINED_BYTES`, 256 MiB). A private `Meter` charges `size_of::<T>()` × requested element capacity immediately before each buffer is allocated. It covers the lowering's use counts, agents, fanout and eraser tables, use offsets and cursors, use slots, output links, outputs and types. During `reduce`, the validation rebuild charges the same buffers while the candidate net is also charged. Reduction charges its arena slots, output boundary, worklist and read-back buffers. Arithmetic is checked: an overflowing total is reported as `observed = u64::MAX` and refused. The meter releases the validation temporaries when validation returns, and the high-water mark is reported as `Report::retained_bytes_high_water`.

The executor's own arena and worklist growth is pre-charged by a `Growth` step. That step reserves exactly the bytes a checked plan needs before `rewrite::execute` runs, so the executor's amortized `try_reserve` finds capacity in place. If the allocator over-provisions, the excess is visible through `Vec::capacity` and is charged immediately afterwards. Growth beyond the executor's count caps is left to the executor, so the existing `arena-reservation` and `worklist-reservation` refusals keep their precedence. A charge refused in validation or reduction becomes `Obs::Exhausted { limit: RetainedBytes, .. }`, and in lowering it becomes `Refusal::Exhausted`.

**Rationale:** Every buffer size is known from admitted counts before allocation, so the ceiling is enforced, not estimated, and the refusal is deterministic. In the unbounded run, the first refused charge below the high-water mark always lands exactly on that mark. Tests can therefore assert `observed == high_water` at `cap = high_water − 1`. Charging requested capacity, rather than measuring RSS, keeps the core pure.

**Reopen when:** a selected allocator or process-memory measurement can bind a physical ceiling, or the reducer gains buffers that are not sized from admitted counts.

### Decision D2: request input and output ceilings in the CLI

**Choice:** `--max-input-bytes` charges the graph file, then the vector file. Each read is bounded by the smaller of its existing file bound and the bytes the request has left (`files::read_bounded`). A file over the request ceiling is refused as `E-NET-BUDGET-input-bytes` with the stat length (or `limit + 1` if the file grows during the read). A file over its own fixed bound keeps the existing host refusal.

`--max-output-bytes` charges every line's exact encoded length plus its newline. `Json::encoded_len` and `Line::encoded_len` mirror the encoder without building text, and unit tests prove the two agree. A row line is charged as its fixed fields plus the arithmetic length of its `outputs` field before any output value is rendered. A refused charge discards every buffered line and writes only the refusal line.

All three flags need `--net` and may only lower their defaults. The input default (134,217,728) is the sum of the existing file bounds, so it does not change today's behavior. The output default is 67,108,864.

**Rationale:** An exact charge makes success at the limit and refusal one byte below it observable and testable. Charging before rendering bounds construction itself, not just the final write. Replacing the report on refusal follows the existing first-failure convention and never writes a truncated success.

**Reopen when:** reports become streamed, so only one line is retained at a time, or a consumer needs a ceiling on the refusal line itself (today a constant-size line written outside the ceiling).

### Decision D3: executed census check, not a correspondence proof

**Choice:** `net::census_check(interactions, net)` compares the counter of a complete reduction with `InteractionGraph::expected_interactions` (`2·#ops + #dups + #eras` of the validated candidate). `reduce` returns `Normal` only on equality; otherwise it returns `InternalInvalid(InteractionCensus)` (`interaction-census`). `Report::expected_interactions` carries the census whenever validation admitted the candidate, so a fuel-exhausted row reports both the partial counter and the census. Normal CLI rows state `count_basis: "census-checked"`.

**Rationale:** The census is the formula that `Certificate.accepted_exact_count` fixes for every normalizing reduction in the model. Comparing it with the native counter on every run catches a reducer that skips, repeats or miscounts firings in that run. A refinement proof would cover every run; this check covers only the run it executed. The mismatch control is the pure function's negative test, because the reducer offers no supported way to forge its own counter.

**Reopen when:** an accepted Rust-to-model refinement theorem for the lowering, reducer and read-back exists. The basis may then name that theorem instead.

### Decision D4: receipt fields and proof binding

**Choice:** The `--net` header adds `producer`, `net_contract`, `limits`, `input_bytes` and `semantic_count`, as fixed in `docs/i64-net-contract-v3.md` §9. `semantic_count.theorems` lists the three fully qualified Lean declarations, and `lean_system` names `NobleFoundation.NetI64.system`. `proof_source` gives the module and vendored inets file (path, BLAKE3, SHA-256 as recorded by the receipt), the inets upstream commit, the Lean toolchain, and the retained foundation receipt `verification/foundation/receipts/518be7fd5/foundation-check.json` (path, BLAKE3, source commit). `assurance` is `model-only`, and `model_boundary` states that the theorems are not a refinement proof. A CLI unit test re-measures those files with BLAKE3, checks the receipt's recorded hashes, its source commit and the `proved` status of each theorem, and fails on drift. The rule-table identity is the existing `PLAN_RULES_REVISION`, and no digest of a description is invented. The report schema stays `noble-ngraph-report/v1`: the fields are additive, and clients ignore unknown fields.

**Rationale:** A consumer that promotes a count to `checked` must bind exact theorem declarations and proof sources. Pinning them in the producer and testing them against the retained receipt prevents silent drift when the Lean sources change.

**Reopen when:** the foundation receipt is renewed or `NetI64.lean` changes. The test then fails, and the constants and receipt path must be renewed together.

### Decision D5: producer identity without a binary digest

**Choice:** `producer.package` and `producer.version` come from Cargo. `producer.source_revision` comes from compile-time `NOBLE_SOURCE_REVISION` and is reported only as a full lowercase 40-hex commit with status `build-declared`; otherwise it is `null` with status `unavailable`. The flake's `noble` derivation sets `NOBLE_SOURCE_REVISION = self.rev or ""`. `producer.executable_digest` is always `null`.

**Rationale:** A process cannot honestly attest its own executable bytes, so the report names the source and lets a consumer measure the executable. A dirty tree has no `self.rev` and must not claim one.

**Reopen when:** a measured, signed executable artifact is selected, for example through Mantle or Lattice's admitted route.

## Risks / Trade-offs

- Setting `NOBLE_SOURCE_REVISION` from `self.rev` makes the `noble` derivation, and every check that consumes it, rebuild on each commit, even when the source is unchanged. The owner may prefer to declare the revision only in a release derivation.
- The accounted ceiling counts the larger of the lowering peak and the validation-plus-reduction peak for each row. The decoded graph and vector rows are bounded separately, by `--max-input-bytes` and the checker budgets, and are not part of the per-row accounting.
- The output ceiling covers the success report. A refusal line is constant-size and is written outside the ceiling.
- New and changed functions need source-inventory renewal and an Octet run. Tigerstyle findings in the new code are fixed at the source, with no added exemptions.

## Open questions

- Should `--max-input-bytes` and `--max-output-bytes` become ordinary-path flags as well? This change scopes them to `--net`.
- Should the revision binding move from `self.rev` to a dedicated release derivation (see Risks)?
