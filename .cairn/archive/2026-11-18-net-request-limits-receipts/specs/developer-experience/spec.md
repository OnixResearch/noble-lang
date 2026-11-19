## ADDED Requirements

### Requirement: NGRAPH-NET-LIMITS-01
r[NGRAPH-NET-LIMITS-01]

The opt-in `noble ngraph eval GRAPH VECTORS --net` route MUST enforce three explicit whole-request ceilings with bounded defaults. A ceiling MUST NOT be raised above its default, and a ceiling flag MUST NOT be accepted without `--net`. Each such misuse, and a malformed value, MUST be a usage error.

- `--max-input-bytes` MUST charge the graph file and then the vector file before each is read. No read may extend past the bytes the request has left.
- `--max-retained-bytes` MUST bound the accounted bytes of every buffer that the lowering, the independent validation rebuild, the reduction arena, the output boundary, the active-pair worklist and the read-back hold at the same time. Each buffer MUST be charged at its element size times its requested capacity before it is allocated or grown. Accounting MUST use non-wrapping arithmetic, and any capacity the allocator grants beyond the request MUST be charged before use continues.
- `--max-output-bytes` MUST bound the encoded report. Each line, newline included, MUST be charged at its exact length before its input-proportional values are constructed.

Exceeding a ceiling MUST produce exactly one `exhausted` report line, exit 5, with the code `E-NET-BUDGET-input-bytes`, `E-NET-BUDGET-retained-bytes` or `E-NET-BUDGET-output-bytes`. The line MUST carry the `limit`, the `cap` and the `observed` amount, and MUST replace every header and vector line. The route MUST NOT write a truncated success or a partial output row. A request at a ceiling MUST succeed exactly as it does above the ceiling. These accounted ceilings MUST NOT be reported as process RSS bounds or sandbox limits.

### Requirement: NGRAPH-NET-RECEIPT-01
r[NGRAPH-NET-RECEIPT-01]

A complete net reduction MUST be reported as normal only when the reducer's interaction counter equals the validated net's census `2·#ops + #dups + #eras`. A mismatch MUST be an `internal-invalid` refusal of class `interaction-census` with no outputs. Each normal `--net` vector line MUST report `interactions`, `expected_interactions` and `count_basis: "census-checked"`, plus its accounted `retained_bytes_high_water`. This check is an executed per-run equality. It MUST NOT be described as a refinement or correspondence proof of the lowering, the reducer or the read-back.

The `--net` header line MUST bind:

- the producer package and version, the source revision the build declared or an explicit `unavailable` status, and a null executable digest;
- the contract (`ngraph-net-i64/3`), the lowering revision, the rule-table and plan, checker and executor revisions, the schedule, the storage policy and the identity scheme;
- every effective limit and the input bytes read;
- a `semantic_count` object naming the semantic unit, the counter, the observation boundary, the census, the count basis, the fully qualified Lean theorem declarations with their instantiated system, and the pinned proof sources and retained foundation receipt identities.

The `semantic_count` object MUST state its assurance as model-only, with an explicit boundary statement. A test MUST re-measure the pinned proof sources and receipt and MUST fail when they drift.
