# Calculator-v1 selected guest design

## Context and scope

The accepted CALC requirements describe an application, not a new language conformance profile. This active design selects a separate `noble calculator` route after M4 and the library/module/text/iteration/error-schema gates. Core-Bootstrap keeps wrapping `I64`; ordinary, cursor, declared-module and LIVE environments do not inherit calculator builtins, memory expansion or preparation allowances. No canonical calculator scenario is promoted by this design. r[CALC-SCOPE-01] r[CALC-GATE-01]

## Decisions

### Pure typed guest, narrow terminal shell

**Choice:** Install checked Noble modules for lexical input, exact numbers, rational normalization, parser/evaluator state, diagnostics and display. Every operation of a line is a freshly checked Noble source submission compiled by the selected Wasm backend. The shell may frame bytes, supply seven nonnegative guest budgets, schedule a guest-certified action kind, observe a bounded typed result and retain accepted line sources. It must not parse an expression, resolve a name, calculate a rational, choose an exact display representation, inspect an opaque guest state, supply filesystem/network/clock/process imports, or convert a native error into a guest result. All guest phases of one candidate line share the same checked ModuleSession, prospective compiler and Wasm worker; their state remains on the guest typed stack. r[CALC-SCOPE-01] r[CALC-BOUND-01]

**Rationale:** A host-only evaluator, separately instantiated one-step stand-ins and host reinjection of private parser/numeric state would satisfy neither the compiled Noble backend nor transactional application contract. Phase boundaries are an implementation device, not a second trust boundary or a reset of an individual compiler preparation's accounting. r[CALC-GATE-01]

### Tentative line and replay boundary

**Choice:** Before interpreting a new line, enforce bounded accepted-command count, retained source bytes and replay effort as explicit host limits. Construct a fresh *candidate* worker from independently checked modules and replay only previously accepted lines in source order. Run all typed phases of the new line in that one candidate worker. Publish its new compiler, worker, typed state and accepted source only after guest acceptance and budgeted exact formatting succeed; otherwise discard it and keep the prior accepted session. A later terminal submission may use a different fresh worker and replay the accepted history. Terminal write failure after a successful commit does not undo that commit. r[CALC-STATE-02]

**Rationale:** A native quota or trap can poison the candidate's Wasm handles. Replaying accepted input into an isolated candidate prevents a failed line from mutating or invalidating the prior worker; explicit replay bounds prevent indefinite reconstruction. Host transport/history limits are labeled separately from the seven guest `ResourceLimit` categories. r[CALC-STATE-02] r[CALC-LIMIT-01]

### Independent selected compiler and memory bounds

**Choice:** The selected source compiler checks borrowed structure, concrete substitutions, complete definition/environment closure, each independent kernel acceptance and identity before publishing a compiler state. It separately bounds 512 reachable named bodies and 768 environment rows, including derived nominal operations; every reachable row retains independent kernel replay. Only during calculator borrowed preflight, before any owned kernel witness clone, each nonzero `Work::charge(amount)` first grants exactly that measured amount through the checked calculator ledger. This preserves the request's original preparation allowance (currently at most 512,000,000 units) for admission and kernel acceptance. The borrowed-preflight grant scope ends before admission; it does not mark the whole submission trusted or enable plan grants across kernel clones. The existing finite environment-work allowance follows environment validation, and only after independent kernel acceptance and identity may plan lowering issue measured shortfalls. All grants and the original allowance share one cumulative ceiling of 2,000,000,000 issued units per preparation. These units bound charged structural operations, **not** a physical CPU/time guarantee. Separate per-type, stack, source, signature and emission caps still reject submissions without raising the work cap. The calculator-only managed runtime imports 16 initial pages and at most 64 pages; 72,362 monotonically retained 48-byte cell handles fill its disjoint regions up to the exclusive 4 MiB end. The per-invocation allocation limit remains 1,376,208 bytes; no handle reclamation/reuse or expanded ordinary memory is implied. Text and transient signature-observation scratch stay distinct and observed signatures are copied before scratch reuse. Ordinary and cursor memory remain at their established 16-page maximum. r[CALC-BOUND-01] r[CALC-LIMIT-01] r[CALC-GATE-01]

**Rationale:** Forged direct submissions and deeply expanded type witnesses must not bypass independent checking or disguise physical preclone work; calculator's larger exact-number closure cannot consume ordinary profile authority or memory. Borrowed per-appearance and named-body scans are charged cumulatively; only the exceptional first stack mismatch is reserved once, against the maximum of each actual join's frame/input pair and each root, named or quotation completion's frame/claimed-output pair, never against unrelated frames or heights. A selected backend exhaustion, assembly refusal, engine trap, timeout or host replay bound is never relabeled as deterministic guest `ResourceLimit`. r[CALC-LIMIT-01]

### Selected retained-module source boundary

**Choice:** The explicitly selected `ModuleSession::new_calculator` retains at most 524,288 bytes of module source in aggregate. This is a separate profile bound, not an increase to `Limits.bytes`: each new module source/request still has the 262,144-byte candidate limit, and calculator terminal input remains bounded independently at 65,536 bytes. Registration checks the aggregate before parsing or cloning a candidate module. Ordinary and cursor sessions retain their prior aggregate source limit; the selected profile does not relax source checking, node/depth/Work accounting, source-bound identity or opaque nominal identities. r[CALC-BOUND-01] r[CALC-LIMIT-01]

**Rationale:** The ten separately checked calculator guest modules now exceed 262,144 bytes together while each remains below the individual source cap. A bounded selected retention allowance admits that real guest closure without inflating caller-supplied limits or granting it to unrelated profiles. This source-retention policy is not a completed calculator scenario or compiled-guest execution claim.

### Evaluation Work accounting

**Choice:** `--evaluation-work` (default 4,000,000) is guest Budget slot 5. Every charge is checked quotient-first against the remaining allowance by its own probe before the guarded work runs, and subtracted by its admit. Charges:

- Preparse: 1 per input byte.
- Decimal literal of d digits: estimated width w = ⌊(3322·d + 999)/1000⌋ + 2 = ⌈3.322·d⌉ + 2 for d ≤ 256 (the default NumericDigits budget), else the former 4d + 2. This is at least the bit width of the magnitude and of its 10^f denominator, because 3.322 > log₂10. The literal costs 50·(2w+1)², and w is also the literal's IntermediateIntegerBits estimate.
- Raw rational normalization, with g = width(num) + width(den) + 2: gcd 50·g², numerator finish 25·g², denominator finish 25·g'², where g' = width(reduced num) + width(den) + 2 (`finish_den_measure`), so g' = g when the gcd is 1.
- Power: 100·W² for the projected width W.
- Display: 100·w², where w = width(num) + width(den) + 2.
- Binary operator: 150·C. With a=|n1|, c=|d1|, e=|n2|, f=|d2| and b = a+c+e+f+2, C counts list-element visits of the operator's actual loops. A stepped shift-and-add product with multiplicand width P and multiplier width Q costs M(P,Q) = 4PQ + 2Q² + 9Q + 2. Per operator:
  - × is M(a,e) + M(c,f).
  - ÷ is S = M(a,f) + M(max(c,e), min(c,e)) + c + e + 2.
  - ± is S + M(c,f) + 5b + 9.
  - Every operator adds 2b + 4 for its width measurements.
  - When b exceeds 384, the widths are not packed and C = 8b² + 25b + 19.
  - `op_work_limit` and `op_work_probe` measure the operand widths, and `op_work_route` counts and decides.
- Product roles. The guest loop iterates over the multiplier, so each product's charge follows its stage's operand order:

  | Operator | Product | Stage | Multiplicand | Multiplier | Count |
  |---|---|---|---|---|---|
  | × | n1·n2 | `op_first_stage_seed_mul` | n1 (a) | n2 (e) | M(a,e) |
  | × | d1·d2 | `op_first_stage_second_mul` | d1 (c) | d2 (f) | M(c,f) |
  | ±, ÷ | n1·d2 | `op_first_stage_seed_cross` | n1 (a) | d2 (f) | M(a,f) |
  | ±, ÷ | d1·n2 | `op_first_stage_second_cross` | wider of d1, n2 | narrower; d1 on a tie | M(max(c,e), min(c,e)) + c + e + 2 |
  | ± | d1·d2 | `op_first_stage_sum_den_mul` | d1 (c) | d2 (f) | M(c,f) |

  `op_first_stage_second_cross` measures both widths with `big_width` (c+1 and e+1 positions of `bit_count`, hence c + e + 2) and passes the narrower operand as multiplier. `op_work_probe` orders the same pair the same way, d1 as multiplier on a tie.
- Visit count of M(P,Q). One unit is one position of a guest loop pass. Multiplier bit i (0 ≤ i < Q) sees the multiplicand shifted to width P+i and an accumulator below 2^(P+i), so their sum has at most P+i+1 positions. Each bit costs 7 fixed units: route, probe and shift, plus the add's begin and the terminal steps of its continue, trim and reverse passes. A set bit's add also costs 4(P+i+1): digit, continue, trim and reverse over each sum position. Summed over the Q bits, this is 4PQ + 2Q(Q−1) + 4Q + 7Q = 4PQ + 2Q² + 9Q, and the cursor start and the final route on the empty multiplier add 2. The Q² term follows the multiplier: with d1 = 1, the former d1·n2 order cost M(1,e) = 2e² + 13e + 2, and the width-ordered stage costs M(e,1) + e + 3 = 5e + 16.
- Above-384-bit bound. With s = a+c+e+f = b−2 and Q* = min(c,e), so that Q*² ≤ ce:
  - ± is 4(af + ce + cf) + 2Q*² + 4f² + 18f + 9Q* + c + e + 7b + 21.
  - Its quadratic terms are at most 4s², because 4s² ≥ 4f² + 8af + 8ce + 8cf. Its linear terms 18f + 9Q* + c + e are at most 18s. So C(±) ≤ 4s² + 18s + 7b + 21 = 4b² + 9b + 1.
  - ÷ is ± without M(c,f) + 5b + 9, so it is smaller.
  - × is 4ae + 2e² + 4cf + 2f² + 9e + 9f + 2b + 8 ≤ 4s² + 9s + 2b + 8 = 4b² − 5b + 6.
  - Every kind is therefore below 8b² + 25b + 19, and the landed cap stays a sound bound. Its value is unchanged, so admission above 384 bits does not change.

r[CALC-LIMIT-01]

**Rationale:** Each charge is at least the number of guest list-element visits of the work it admits, so admission bounds work deterministically before it runs. The former operator charge, 150·b², charged integer addition as a full multiplication. It also undercounted lopsided products, because the guest shift-and-add iterates over the multiplier and pays a Q² term. The ± and ÷ cross product d1·n2 formerly took n2 as multiplier. Integer ± and ÷ (d1 = 1) therefore paid 2e² in Work and one stepped stage per bit of n2, plus an add for each set bit. The width-ordered stage pays M(max, min) plus the c + e + 2 width count. That is never more than either fixed order plus at most b units, and it adds no guest submission. A fixed swap (d1 always the multiplier) would instead move the Q² term onto d1, and decimal and rational left operands have wide denominators. Charges C per order, with the d1·n2 product's guest submissions in parentheses:

| Operands | a,c,e,f / b | Former n2 multiplier | Fixed d1 multiplier | Width-ordered |
|---|---|---|---|---|
| `9007199254740993 - 9007199254740992` | 54,1,54,1 / 112 | 7,579 (546) | 1,272 (175) | 1,329 (175) |
| `9223372036854775807 + 1` | 63,1,1,1 / 68 | 788 (16) | 788 (16) | 792 (16) |
| `1 + 18014398509481983` | 1,1,54,1 / 59 | 6,996 (4,892) | 689 (175) | 746 (175) |
| `1/3 + 1/6` | 1,2,1,3 / 9 | 227 (19) | 242 (30) | 232 (19) |
| `0.1 + 0.2` | 1,4,1,3 / 11 | 273 (25) | 330 (53) | 280 (25) |
| `0.000001 + 1` | 1,20,1,1 / 25 | 391 (73) | 1,360 (470) | 414 (73) |
| `2^-60 + 3` | 1,61,2,1 / 67 | 1,272 (390) | 9,237 (619) | 1,337 (390) |
| `1.5 + 2.25` | 2,2,4,3 / 13 | 348 (56) | 306 (35) | 314 (35) |
| `1 / 18014398509481983` | 1,1,54,1 / 59 | 6,675 (4,892) | 368 (175) | 425 (175) |
| `0.000001 / 3` | 1,20,2,1 / 26 | 261 (147) | 1,215 (500) | 285 (147) |

The former literal width 4d+2 overestimated bits by about 20%, and that overestimate was squared.

The tighter forms keep the 4,000,000 default: two 16-digit integer operands no longer exhaust it. Runtime quotas (steps, allocation, session cells) depend only on the widths a submission touches. Every value of width X still requires a gate costing at least 100·X², through power, literal or normalization. Operator submissions above 384 bits pay at least 150·b². So the largest width an admitted line can touch is at most max(384, the former frontier) for the same budget, and no new runtime-quota exposure is introduced. A denial remains a deterministic `ResourceLimit EvaluationWork`, never a host timeout.

### Evidence and lifecycle

**Choice:** Exercise full compiled guest paths and negative controls separately from document validation. Preserve canonical CALC-01 through CALC-13 states and older M1–M4 evidence. Do not infer an accepted case from one arithmetic vector, a source typecheck, successful WAT assembly, a local native delta or proof about wrapping `I64`. This package remains active/unselected until its source and actual guest observations meet their gates. Pinned Cairn lifecycle gates are unrun under the current explicit no-Nix/low-space directive; offline local specification checks, if run, cannot substitute for them. r[CALC-GATE-01]

## Risks and boundaries

- Bounded selected memory is not a general garbage collector. Only at a Calculator-v1 submission boundary, before `begin`, while no continuation is live (`cp = 0`), the runtime's own `collect` export compacts the cells reachable from the operand stack. This is the selected backend runtime, not the shell or candidate code, and it issues no host request. Survivors keep their relative handle order, so every `a`/`b`/`c` edge stays strictly backward. Live dynamic Text slides with its cell. The vacated cell tail, freed Text bytes, the per-submission type-certificate table and observation globals are zeroed. A stale handle above the new cursor therefore fails liveness. Reclamation is safe because no calculator handle survives outside the worker's operand stack: the shell injects only Text/I64/Bool, never reads report handles, and Calculator-v1 refuses `park` and `project`. Compiled bodies keep quotation and callee handles in instance-private globals, so a boundary where any reachable Program runs compiled code (table entry ≥ 5) moves nothing. A malformed root, edge or Text order fails closed and terminates the session. Handles are therefore valid only until the next execute; never retain one across submissions. The 72,362-cell, 64-page and per-submission allocation caps are unchanged. Implementation choice under S-MEM-04; ordinary, cursor, declared and LIVE profiles still never reclaim.
- The shared 16,384-entry function table is reused, never enlarged. Every calculator module re-lowers its own named-definition closure, and its compiled bodies read Program handles only from instance-private globals, so an earlier module's compiled slots are reachable only through a live Program cell whose table entry is at least 5. After a normal submission the runtime's read-only `compiled_programs_reachable` export runs the `collect` mark closure; a live continuation, failure or malformed arena answers reachable. When nothing is reachable, the next preparation restarts its function allocation at base 5 and its `$initialize` `table.init` overwrites only dead slots; the core entries 0..4 are never rewritten. Otherwise allocation continues above every committed slot. Host pushes between submissions are Text, I64 or Bool and cannot introduce a Program. The same quiescent preparation restarts the definition-identity ledger empty: identities are session-stable ids checked against their recipes, plan and emission never read the ledger, and no earlier epoch's compiled body can run again, so each epoch admits at most 512 distinct identities while ordinary preparations still retain and check the whole ledger.
- The 8,192-key signature pool restarts only at a stricter, Program-free boundary. After a normal submission whose `compiled_programs_reachable` answer is 0, the runtime's read-only `programs_reachable` export runs the same `collect` mark closure and answers 1 if any marked cell is a Program (kind 4), recipe atom or concatenation (8, 9) or Syntax (10), or if the arena is malformed or failed; the engine reports both answers and the host refuses a report where a compiled Program is reachable but no Program is. Signature IDs are held at runtime only by those cells: Program interfaces, literal schemas and invocation witnesses in recipe atoms, and certified cells (16) that copy the interface of the Program they reference through a followed child, so a reachable certified cell implies a reachable Program. No other runtime structure carries an ID across a boundary: `$observed_program` and the recipe cursors are instance-private and also cleared by `collect`, which runs before every Calculator-v1 submission and zeroes the shared certificate page, and the host pushes only Text, I64 and Bool. On the compiler side the definition-identity ledger, whose recipes may name signatures, already restarts at every quiescent boundary, which a Program-free one implies, and earlier modules' compiled code is dead by the table-reuse argument. At such a boundary `prepare_program_free` starts the meter and the prospective pool from a fresh calculator pool, empties the published descriptors in the same epoch, and publishes from descriptor 0. The engine's `recordSignatures` keeps only `signatures.slice(0, first)` and refills `[first, end)`, so `first = 0` replaces the whole table, and `readSignature` bounds-checks every ID against it. Transaction provenance compares only the retained signature and descriptor prefixes, which are empty after a restart, and the key limit still applies within each epoch. Static literal Text and `text_end` are never rewound because live data Text may name static addresses. Ordinary-profile WAT is byte-identical.
- Cross-line replay costs native time and memory and has separately identified host ceilings. It does not expand a guest `EvaluationWork` budget or make a failed line successful.
- Complete CALC conformance, independent numeric oracle/negative controls, AI-authoring benchmark and actual-Rust formal refinement require separate evidence. No result of those gates is claimed here.
