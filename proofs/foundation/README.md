# Noble mathematical correctness foundation (issue #15)

This Lake project is a narrow, inspectable correctness chain for **signed
wrapping I64 arithmetic plus a small quote/compose/run fragment** of the
`Core-Bootstrap` profile. It connects three things:

- the independent Mathlib `ZMod (2^64)` oracle used by the comparator
  ([`proofs/m3/MathlibI64.lean`](../m3/MathlibI64.lean));
- the MC1 typed operational model
  ([`proofs/mc1/NobleContracts`](../mc1/NobleContracts));
- the executable evidence for the compiled Wasm.

Both Lean sources are compiled from their existing files through
`srcDir`. Nothing is copied, so the theorems are about exactly those
definitions.

**Every theorem here is about a model.** None of them is an
implementation theorem about the Rust frontend, `noble-wasm` lowering,
runtime helpers, `wasm-opt`, V8 or the loader. The tables below show
where each claim stops.

The separate input-specific EMI lane adds one scalar `if` or I64 sum `case`
through [`NobleFoundation/Branch.lean`](NobleFoundation/Branch.lean). Its
branch theorems do not extend the arithmetic comparator's whitelist or
claim universal equivalence of the generated original and variant.

## Reproduce

Prerequisites:
- the policy-selected Lean 4.31.0 (`policy/tool-selection.json`
  `tool_paths.lean.output`; build it with `nix build .#lean`);
- the Node at `tool_paths.node.output`.

Pins are identical to `proofs/m3` and `proofs/mc1`. The checker refuses
any difference: Lean `v4.31.0`, Aeneas `505b6ca3`, Mathlib4 `fabf563a`.

```sh
cd proofs/foundation
PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
  lake exe cache get        # downloads Mathlib .oleans (not rebuilt)
cd ../..
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
OUT=$HOME/.cache/noble-foundation-$(date +%s); mkdir -p "$OUT"
# 1. proofs, axioms, inventory -> foundation-check.json
env -i "$NODE" verification/foundation/check.mjs --output "$OUT/check"
# 2. Phase 3 slice (needs the user systemd manager for the sandboxed Lean consumer)
env -i XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" "$NODE" verification/foundation/slice.mjs \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/slice"
# 3. Phase 4 differential comparison (+ kernel 1000/200 checker lanes)
env -i "$NODE" verification/foundation/differential.mjs --seed 15 --count 80 \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/differential"
# 4. reject stale evidence: succeeds only on the clean tree the receipt names
env -i "$NODE" verification/foundation/differential.mjs --verify "$OUT/differential/differential.json"
# 5. Input-specific branch EMI, with independent admission and fresh Lean checks
env -i "$NODE" --no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up \
  --stack-size=1024 --max-old-space-size=256 \
  verification/foundation/differential.mjs --emi --seed 53 --count 16 \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/emi"
env -i "$NODE" verification/foundation/differential.mjs --emi --verify "$OUT/emi/emi.json"
```

The existing fixed-vector comparator
(`verification/mathlib-i64/compare.mjs`, 12 vectors, opt off) is
unchanged. It needs `proofs/m3/.lake` populated the same way
(`lake exe cache get` in `proofs/m3`).

`check.mjs` does the following:
- refuses a dirty worktree (`--allow-dirty` records `clean_source:false`)
  and ambient `LEAN*`/`LAKE*`/`NODE*`/`GIT_*`/`NIX_*`/loader overrides;
- requires every Lake package checkout to be clean at its manifest revision;
- scans `NobleFoundation/` for `sorry`, `admit`, `native_decide`,
  `implemented_by`, `extern`, `unsafe`, `opaque`, `partial` and `axiom`;
- requires the declared theorems and [`inventory.json`](inventory.json) to
  be the same set;
- builds `NobleFoundation` and fails on any warning;
- for every inventoried theorem, prints `#check` and `#print axioms` through
  `lean --json`, and accepts only `propext`, `Classical.choice` and
  `Quot.sound`;
- writes `foundation-check.json`, which records:
  - source commit/tree;
  - the SHA-256 of every Lean source, the MC1 model, the oracle, the
    pins and the checker;
  - the Lean binary hash;
  - package revisions and trees;
  - each theorem's printed statement and axioms.

**Trust assumption:** the Mathlib `.olean` files come from the Lake cache
download and are not rebuilt. The Mathlib *source* checkout is clean at the
pin. The Lean kernel checks this project's proofs against those imported
`.olean` files.

## Selected profile and whitelist

| Item | Selected | Notes |
|---|---|---|
| Profile | `Core-Bootstrap`, backend `managed-linear-memory` | Calculator-v1 (`44329dc`, `crates/noble-cli/src/calculator.rs`, `runtime/calculator*.wat`) is a disjoint experimental profile. Its comparison, div/rem and guest exact arithmetic inherit **nothing** from this fragment |
| Values | `I64` only on the operand stack. Program values are built by the fragment | Bool/Text/pairs/sums/lists, resources and host effects (`test.emit` 22, `test.abort` 23) are outside it |
| Literals | decimal `[-]digits` accepted iff the value is in `[-2^63, 2^63)` | out-of-range literals are **rejected** (`integer literal is outside I64`) and never wrapped |
| Words | `+` (4), `-` (5), `*` (6), `quote` (8), `compose` (9), `run` (10), `reflect` (11, recipe observation only), literal blocks `[ … ]` | `=` (7), `dip`, `swap`, `dup`, `drop` and branching are outside the theorems. MC1 models them, but they are not part of this fragment |
| Definitions/imports | none | named calls and declared modules belong to #7 |
| Optimization | the model is mode-independent | off/on behavior is a Wasm/optimizer claim, see Phase 3/4 |

This table describes the original arithmetic/Program fragment. The bounded
EMI branch extension and its excluded observations are specified separately
under Phase 4 below.

### Representation relation and observations

- `NobleFoundation.Represents v n :≡ I64Range n ∧ v = .i64 (BitVec.ofInt 64 n)`
  with `I64Range n :≡ -2^63 ≤ n < 2^63`. This relation is functional, and
  every well-typed MC1 I64 value represents exactly its `BitVec.toInt`.
- `toZ : BitVec 64 → ZMod (2^64)` and `ofZ` are mutually inverse ring
  maps. `MathlibI64.signed (toZ x) = x.toInt`.
- `MathlibI64.wrap z` is related to the other descriptions as follows:
  - `= Int.bmod z (2^64)`;
  - `= (BitVec.ofInt 64 z).toInt`;
  - `= ((z + 2^63) mod 2^64) - 2^63`, which is the K-NUM-01 formula.
- Stack orientation:
  - *Public* stacks (`Exec`, typing) are bottom-first, with the last
    element on top. In `x y -`, `x` is deeper (left) and `y` is the top
    (right).
  - *Internal* `Run`/`Step` stacks are top-first, so subtraction matches
    `.i64 y :: .i64 x :: s ↦ x - y` (`exec_sub_is_internal_step`).
  - The runtime helper `$op_sub` pops `$right` first, then `$left`, and
    computes `i64.sub left right`
    (`crates/noble-wasm/runtime/operations.wat:18-21`).
- Observations:
  - the ordered final stack, including an arbitrary preserved prefix;
  - normal return versus no normal return (MC1 is partial correctness);
  - `reflect`'s exact recipe;
  - frontend literal acceptance/rejection.

## Claim table (Phase 1)

Status words:
- **proved**: machine-checked here.
- **tested**: finite executable evidence.
- **assumed**: trusted.
- **open**: no evidence.

| Claim | Model / theorem(s) | Implementation boundary | Status |
|---|---|---|---|
| ZMod/BitVec/K-NUM-01 signed decoding, range, round trips | `toZ_ofZ`, `ofZ_toZ`, `toZ_add/sub/mul/neg`, `signed_eq_toInt`, `signed_toZ`, `wrap_eq_bmod`, `wrap_eq_toInt_ofInt`, `wrap_eq_knum01`, `toInt_range`, `signed_range`, `intCast_signed`, `toInt_ofInt_eq_iff`, `signed_intCast_eq_iff`, `ofInt_toInt` | none (mathematics only) | **proved** |
| MC1 BitVec add/sub/mul decode to the comparator oracle | `toInt_add/sub/mul`, `oracle_add/sub/mul` | none | **proved** |
| Literal acceptance is exact range rejection, not wrapping | `acceptLiteral` (spec); `accumulate_eq_acceptLiteral` over a *transcription* of `Accumulator::digit`; `accepted_literal_exact`; `ofInt_collapses_out_of_range`; `literal_boundary_regressions` | `crates/noble-contracts/src/syntax/integer.rs`, token classification in `source/lexer/tokens.rs` | transcription **proved**. Rust ↔ transcription **open** (the Aeneas extraction exists in `proofs/mc1/NobleContractImpl/Funs.lean`, but there is no refinement theorem). CLI rejection **tested**: `verification/m4/acceptance.json` and the Phase 4 comparator |
| Universal instruction semantics with a preserved typed prefix | `add_spec`, `sub_spec`, `mul_spec` (`ArithSpec`); `exec_arith_toInt`; `add_needs_i64` | `crates/noble-wasm/src/operations.rs:21-23` (word → helper), `runtime/operations.wat:14-25`, `runtime/stack.wat` (`$pop_kind`, `$take`, `$push_i64`) | model **proved**. Lowering/helpers **open** (PO-17), finite Wasm **tested** |
| Public/internal orientation, deeper-minus-top subtraction | `internal_of_public`, `exec_sub_is_internal_step`, `sub_operand_order`, `wrap_sub_noncommutative` | `$op_sub` pop order | model **proved**. Helper **tested** (comparator `sub-order`) |
| Finite sequences equal an independent reference machine | `exec_lowerAll`, `run_lowerAll`, `refRun_range`, `depthRun_of_refRun`, `codeTyped_lowerAll` | lowering of literal pushes (`emit/functions.rs` `call $push_i64`) | model **proved**. Implementation **tested** |
| quote/compose/run: construction is pure, run executes the composed body, reflect observes the exact recipe | `PExpr.exec_code`, `PExpr.exec_code_run`, `PExpr.exec_code_reflect`, `PExpr.exec_run_ref`, `PExpr.codeTyped(_run)`, `exec_quote_program_run_run`, `compose_order_matters`, `quote_compose_run_examples` | `$op_quote_typed`, `$op_compose`, `$op_run` (`runtime/programs.wat`), `emit/functions.rs` | model **proved**. Implementation **tested** |
| MC1 `CodeTyped` agrees with kernel acceptance | — | `noble-kernel` acceptance (PO-09/10/11 relate the Rust checker to the *NobleM2* declarative judgments, not to MC1 `CodeTyped`) | **open** |

Typing premises are stated exactly:
- Sequences require `depthRun ops k = some k'` (static depth) for typing,
  and `refRun ops zs = some zs'` for behavior. `refRun` additionally
  refuses out-of-range pushes.
- Programs constructed by `PExpr` are typed from `base ++ k×I64` to
  `base ++ k'×I64`, and `run` requires that input interface to be the
  whole current stack (MC1 `OpTyped.run`).
- No eligibility/resource premise is needed, because the fragment has no
  resources.

The full list with printed statements and axioms is in
[`inventory.json`](inventory.json) and in every `foundation-check.json`
receipt.

## Phase 3: one checked chain to the loaded bytes

Subject: `[ quote [ 1 + ] compose run ]`. It is one closed, inert quotation
root with no definitions or resources, which is the
`MC2-pure-quotation@1` shape that #10 selected for its PAA-1 admission
envelope (`.cairn/changes/artifact-correspondence-extension/design.md` on
`design/contracts-8-9-10`).

PAA-1 is a design and is **not implemented**. Instead of a second
admission route, `verification/foundation/slice.mjs` runs the two
existing routes that PAA-1 joins, over the same consumer-selected source
bytes:
- Route B: `noble build --require-proof`;
- Route A: `noble admit-artifact`.

This is a strengthened trusted build. It is **not** translation
validation.

| Edge | What happens | Status |
|---|---|---|
| E1 Lean specification | `slice_body_exec` (any prefix, `n ↦ n + 1`), `slice_body_signed`/`slice_driver` (oracle `wrap (x + 1)`), `slice_root_inert`, `slice_body_typed`, `slice_exported_claim`, `slice_examples` | **proved** (model) |
| E2 frozen law → regenerated MC1 statement | `noble explain-proof` output must equal [`slice/expected-statement.lean`](../../verification/foundation/slice/expected-statement.lean) byte for byte. This is the PAA-1 `owner_law` analogue | **checked**. The exporter itself is assumed (PO-16/PO-19) |
| E3 regenerated statement ↔ foundation | Lean kernel: `MC1Obligation.program = NobleFoundation.sliceBody := rfl`, and the foundation proves the regenerated `MC1Obligation.claim` | **proved** per artifact |
| E4 source ↔ subject with evidence | Route B: an independent sandboxed recheck of the self-contained MC1 proof [`compose-increment.proof.lean`](../../verification/foundation/slice/compose-increment.proof.lean); `bind_artifact` binds the source | **checked**. `bind_artifact` is trusted Rust (PAA-1 O6 / PO-16 open) |
| E5 `noble-wasm` lowering | source → WAT | **open** (PO-17, PAA-1 O1). Trusted compiler |
| E6 runtime helpers | [`Helper.lean`](NobleFoundation/Helper.lean) transcribes `$op_add/$op_sub/$op_mul`, `$pop_kind`, `$take`, `$push_tag`, `$need`, `$fail`. `helper_refines_step`/`helper_fails_when_stuck` prove they refine MC1 `Step` (operand order, untouched remainder, refusal of non-I64). The text of each transcribed function is pinned in [`helper-transcription.json`](../../verification/foundation/slice/helper-transcription.json) | **proved over a transcription**. Pin **checked**. Transcription fidelity is **assumed** (manual review). `$op_quote_typed`, `$op_compose` and `$op_run` are **open** |
| E7 WAT → Wasm (`wasm-tools parse`) | pinned path and hash | **assumed**. Validation shows format/type only |
| E8 `wasm-opt -O2` (opt on) | separate receipts per mode | **assumed** (PAA-1 O2 open) |
| E9 candidate = consumer rebuild | `admit-artifact` reports `correspondence: byte-exact` for opt off and opt on | **checked** (identity, not semantics) |
| E10 instantiation/loader/V8 | Route A instantiates the compared buffer (code reading) | **assumed** (PO-18, PAA-1 O3 open) |
| E11 execution | drivers `x [ … ] run` for `x ∈ {41, max, min}` and a `Bool`/`I64` preserved prefix, opt off/on, through admission. Expectations are kernel-checked against `MathlibI64.wrap` | **tested** (finite) |
| E12 one PAA-1 envelope joining A and B | — | **open** (#10) |

Refusal controls. Each one must fail for its intended reason, or the run
fails.

| Control | #10 id | Result required |
|---|---|---|
| valid `[1 +]` proof, source `[ quote [ 2 + ] compose run ]` | ACORR-CASE-03 | Route B not released |
| `[2 +]` bytes admitted against the `[1 +]` source | ACORR-CASE-03 | `correspondence-reject` |
| opt-on bytes admitted under consumer opt off | S-CASE-07 option mismatch | `correspondence-reject` |
| `$op_sub` with swapped pops, reassembled for `3 8 -` | ACORR-CASE-08 (changed helper, operand order) | `correspondence-reject`; the Lean transcription pin also refuses (`helper-transcription-stale`) |
| interface/recipe data string `[I64]` → `[I65]`, code unchanged | ACORR-CASE-06 | `correspondence-reject` |
| foundation receipt from another tree | ACORR-CASE-04 (stale version) | `stale-proof-subject` |
| weakened regenerated statement | ACORR-CASE-05 | `law-mismatch` |
| post-check artifact replacement | ACORR-CASE-09 | **not executed**. There is no hook between comparison and instantiation, and the PAA-1 `load` hash is not implemented |

Stale dependencies do not occur in this profile, which has no definitions.
Recorded deviation: the reviewed sandbox `bubblewrap-0.11.2` store path
from `verification/mc2/gate.mjs` was absent on the checking host. The
runner then uses `bubblewrap-0.12.0` and records that path in the receipt.

## Phase 4: differential evidence

`verification/foundation/differential.mjs` works as follows:
- It generates seeded (splitmix64) cases:
  - straight-line arithmetic;
  - `quote`/block/`compose` trees, nested up to depth 2, then `run`;
  - nested quotation `[ … ] quote run run`;
  - preserved `Bool`/`I64` prefixes;
  - three out-of-range-literal refusal cases.
- Constants are boundary-biased: half are drawn from
  {0, ±1, ±2, ±3, max, min, max-1, min+1, ±2^31, 2^32, 2^32-1, ±2^62,
  ±⌈√2^63⌉}, and half are uniform.
- Expected values come from a BigInt transcription of `refRun`. **Lean
  kernel-checks every expectation**: `refRun … = some …` (or `= none`) by
  `decide`, and instantiates the proved MC1 theorem for that case
  (`PExpr.exec_inputs_run`, `exec_lowerAll_from_empty`,
  `exec_inputs_nested`). The implementation under test never supplies an
  expected value.
- Each case runs with `--opt off` and `--opt on`. The run checks:
  - the optimization flag;
  - the executed-bytes hash against the emitted file;
  - the source hash;
  - the ordered stack, including the untouched prefix;
  - an empty request trace;
  - zero guest requests.
- A mismatch is shrunk greedily. At most 48 shrink candidates are
  executed, and every one must stay valid under the reference.
- Mutation controls each have to be detected, and every detection has to
  be attributable to its mutation:
  - reversed subtraction;
  - unsigned decoding;
  - prefix corruption;
  - wrong composition order;
  - phantom host-trace expectation;
  - literal wrapping instead of refusal;
  - stale proof subject (Lean case rebound to another program, foundation
    receipt for another tree).

  The receipt keeps the shrunk witness of each control.
- It runs the existing kernel lanes
  (`cargo test -p noble-kernel --test property`: 1,000 oracle candidates
  and 200 malformed). They are recorded as **checker acceptance**
  coverage, which is distinct from execution semantics.
- The receipt records:
  - source commit/tree;
  - the hashes of the comparator, the Lean sources, the oracle, MC1, the
    runtime helpers, the lowering table, the frontend literal code, the
    config, the policy, `Cargo.lock` and the property test;
  - the CLI and tool hashes;
  - the Lean version and Mathlib revision;
  - the theorem names with their printed statements and axioms (from the
    foundation receipt, which must be for the same tree);
  - the seed, count and modes;
  - claims labelled **proved**, **tested**, **assumed** or **open**.
- `--verify RECEIPT` refuses source drift and stale evidence.

Unsupported modes: none. Both `off` and `on` are supported by
`Core-Bootstrap`.

### Input-specific branch EMI (issue #53)

`differential.mjs --emi` reuses the arithmetic reference, renderers, CLI
builder and scalar observation comparison. Its independent branch model
covers one scalar `if`, or one `inl`/`inr` I64 payload followed by `case`.
Both bodies contain only I64 literals and `+`, `-`, `*`. The selector,
scalar input and Bool/I64 bottom prefix are fixed literals in the complete
source program. Both branches have the same ordered stack interface and
the entire source is independently admitted through the real frontend and
kernel before execution. A valid but unexecuted word outside this model
is still ineligible.

The deterministic corpus has 16–64 positive pairs and three tagged refusal
probes: an out-of-range literal, a wrong branch type, and an admitted word
outside the model whitelist. Only one declared site in the untaken branch
changes: a constant, arithmetic operator, neutral insertion, or neutral
removal. Refused pairs are eligibility outcomes, not compiler disagreements.
Counts distinguish generated and eligible branch/transformation coverage,
refusal classes, opt modes, and checked subjects.

For every eligible pair, fresh Lean declarations bind both complete bodies,
the fixed input, prefix and selector. Kernel-checked `decide` expectations
and `exec_branchRef` establish the ordered scalar result and preserved
prefix in MC1. Structural selected-body equality and the checked visit
vector establish nonexecution of the changed branch in this fragment.
The proof record binds both exact source hashes and the model-pair/input
hashes; changed bytes cannot reuse the original pair's evidence.

Original and variant execute with opt off and on against that independent
expectation, not merely against each other. Every run requires normal
return, the exact ordered scalar stack, unchanged bottom prefix, an empty
request trace, and zero guest requests/protected operations. Reflection,
recipe/identity observations, returned Programs, effects/resources, live
slots and quota-sensitive equivalence claims are excluded.

The selected ABI maxima are supplied explicitly. Admission, assembly,
optimization and execution artifacts are retained, with exact source,
WAT/Wasm, report and tool hashes. Allocation/operand/continuation peaks,
execution steps, structural recipe/depth bounds, and the separate bounded
stack-inspection reflection counter must each leave more than half their
selected limit free. Scalar stack inspection is not a guest `reflect`.
Exhaustion and insufficient headroom are separate discrepancy classes;
neither counts as an equivalent run. Preparation cost and code size are
not claimed equal.

Seven controls must be detected: selected-branch mutation, changed output
order, prefix corruption, an actual host request with a forged empty trace,
stale exact-subject evidence, a stale foundation tree, and an intentionally
reversed selected-branch lowering. The lowering control changes actual
WAT and executes the resulting Wasm in both opt modes. It is not a forged
expected value.

The shrinker changes the program/variant/fixed-input triple, including the
mutation site. Every retained candidate is re-admitted, freshly checked by
Lean and re-executed while preserving eligibility and the discrepancy
class. Its 64-attempt bound, outcomes and local-minimal/bound-exhausted
status are recorded. Replay the retained lowering-control discrepancy with:

```sh
env -i "$NODE" verification/foundation/differential.mjs --emi \
  --replay "$OUT/emi/minimal-control.json" \
  --foundation "$OUT/check/foundation-check.json" --output "$OUT/emi-replay"
```

`--emi --verify` requires the exact clean source tree, unchanged bound
source/model files and every indexed artifact hash. Raw command captures
retain argv, exit, separate stdout/stderr and supplied stdin. Imported
Mathlib `.olean` files retain the cache trust assumption above. The Rust
renderer/lowering refinement remains open: these are finite fixed-input
EMI results, not a compiler theorem or universal program equivalence.

### CI

The CI recipe is [`verification/foundation/ci.sh`](../../verification/foundation/ci.sh)
`NEW_DIR [--skip-slice]`, run on a clean checkout after `lake exe cache get`.
It runs these stages:
1. `check.mjs`;
2. `slice.mjs`;
3. `differential.mjs --seed 15 --count 80`;
4. `--verify` on the fresh receipt;
5. `differential.mjs --emi --seed 53 --count 16`;
6. `--emi --verify` on its fresh receipt;
7. replay of the EMI control's retained minimal discrepancy;
8. verification of the replay receipt;
9. the one-call named linking pilot in both optimization modes;
10. verification of its exact subject/dependency/artifact receipt;
11. a stale-evidence control: the retained `receipts/898cfe1` receipt must be
   refused.

It writes `summary.txt` and exits nonzero if any stage fails.

The recipe is **not** a `flake.nix` `checks` entry, for these reasons:
- **Mathlib needs network or host-bound inputs.** The `.olean` files come
  from `lake exe cache get`, which is networked. In this checkout they
  occupy 2.1 GiB (8,176 files) under `.lake/packages/mathlib/.lake`.
  - The repository's offline route, `nix/offline-inputs.nix` with
    `nix/lean-backend.nix`, names host-captured `nix store add-path`
    sources. On the checking host those store paths are absent, for example
    `/nix/store/mfpq85q33s4hyqy7c5x4prinz45d0341-mathlib`, so a check built
    on them cannot be realised or exercised here.
  - That route also contains sources only. A sandbox check would have to
    compile the imported part of Mathlib from source. The cost has not been
    measured, and that build would replace the current trust in downloaded
    `.olean` files with a rebuild.
- **The slice needs host services.** Its Route B consumer requires a user
  systemd manager, `XDG_RUNTIME_DIR` and bubblewrap, which are not
  available inside a Nix build sandbox.
- **There is no workflow file to extend.** The repository has no
  `.github/workflows`; its CI surface is the flake `checks` set.
  `flake.nix` and `nix/*` are policy-pinned files, so changing them
  requires a re-pin.

A sandbox check becomes feasible once reviewed offline Lean sources and
either a rebuilt Mathlib `.olean` set or an authenticated cache are pinned
as store inputs. The `check` and `differential` stages need only Lean,
Cargo (offline) and Node, so they could move into the sandbox first.

## Conditional named-call linking (issue #52)

[`NobleFoundation/LinkBoundary.lean`](NobleFoundation/LinkBoundary.lean) imports
the existing pinned `NobleContracts.NamedV1` model. Its two-result callee is
`[ - 7 ]`; the one-call caller is `c.split -`. It proves source-model normal
results, models the private operand/continuation ABI, and states conditional
horizontal linking with exact source/dependency/table/artifact bindings.
`vertical_simulation_compose` is a separate local-simulation composition rule.
All new theorem statements and transitive axioms enter the existing inventory
and checker; the checker also hashes `NamedSubject.lean`.

The [executable pilot](../../verification/linking/README.md) maps each boundary
to the actual embedded-callee path, retains exact-byte receipts and exercises
body/order/return-target/dependency/substitution controls. It explicitly models
engine-owned shared memory/table/globals, not a fictional separate module ABI.
The runtime relation projects Bool/I64 operands, continuations, table and
`sp`/`cp`; it does not claim the engine's other accounting globals are unchanged.

The source and helper-transcription theorems are model results. The real
lowering and loading simulations needed to instantiate the conditional linker
remain premises: C8 and O1/O3/O7 are not discharged, and #7/#10 remain completed
designs. No existing conformance or proof-ledger status changes.

## Bounded host-call locality (issue #51)

[`HostLocality.lean`](../m5/HostLocality.lean) models one synchronous
bounded-region adapter, an ordered guest stack with one saved borrowed owner,
exact M5 resource identity and a separate unrelated-owner/state projection.
The adapter may read only its immutable region and request and update only its
own owner, table snapshot and counters. Actual host locality remains an explicit
premise: the native host has process-memory authority, not a Wasm sandbox.

The 43 inventoried model theorems cover normal success, preflight/domain errors,
validation before ordered borrow restoration, consuming release without owner
return, malformed results, trap, cleanup failure and double retirement. The
decision relation reuses actual `M5Resources` completion, table, release and
late-completion lemmas. It does not replace the extracted table with a handwritten
lookalike. Request naturals describe the post-signed-ABI-validation model;
explicit counter-room premises exclude `u64` overflow.

`verification/host-locality/check.mjs` recovers the exact generated Lean sources
from the retained M5 archive and compiles them anew with the pinned Lean. It
cross-checks the archive against its historical extraction receipt and requires
all six current Rust resource-decision files to retain their recorded hashes.
The separate theorem inventory records exact compiled statements and transitive
axioms; only `propext`, `Classical.choice` and `Quot.sound` are allowed. The
historical generated-code naming warnings are retained, not treated as proof
holes. Imported Mathlib/Aeneas libraries remain a separate trust assumption.

The execution runner builds the current CLI with the hash-checked offline
vendor, reusing the worktree's ignored Cargo target. It compiles the selected WIT
guest through the real CLI, then exercises 10 Wasmtime cases: read/empty-end
success, ordinary bounds refusals and emitted-core trap/tag/length faults.
Five direct production-Host tests cover forged identity, cross-invocation use,
owner retention, double release and cleanup refusal/recovery. The malformed
result controls corrupt guest return-area memory after a valid typed host
response; they are not adversarial malformed host `Val` returns.

From a clean committed checkout with the existing foundation dependency cache:

```sh
NODE=/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node
OUT="$HOME/.cache/noble-host-locality-$(date +%s)"
env -i "$NODE" verification/host-locality/run.mjs --output "$OUT"
env -i "$NODE" verification/host-locality/run.mjs --verify "$OUT/locality.json"
```

Select `tool_paths.node.output` from `policy/tool-selection.json` elsewhere.
`--allow-dirty` is available for development smoke runs, never clean-source
acceptance. Separate proof/execution receipts retain raw argv/status/stdout/
stderr, source/tool/WIT/component hashes, assumptions and limited controls.
`--verify` checks source and retained-artifact identity only; it does not replay
Lean or prove Rust/Wasmtime refinement. The CI recipe runs this separate lane.

T5 Rust-wrapper refinement and T6 lowering refinement remain open. Borrow
restoration itself changes compiler `State.stack`, not a separate runtime event;
early-restoration and unrelated-frame mutation controls are model-only. The
one-owner adapter is unchanged. Its v1 report cannot distinguish guest release
from cleanup, and public CLI cleanup failure remains unexercised; the direct
Host refusal test does not close that edge. No arbitrary-host, async, reentrant,
shared-memory or whole-engine isolation claim, conformance promotion, or
proof-ledger change follows from this pilot. The completed #9 design stays closed.

The [retained pilot evidence](../../verification/host-locality/evidence-71e4504.json)
binds clean source `71e4504bb82fdab174a2650280a21e69f3979611`: 43 locality
theorems, five reused M5 lemmas, 10 Wasmtime cases and the direct Host controls.
Its identity check also rejects altered WIT, raw proof output, proof-source
binding and source revision. Replay or verify from that exact clean revision;
the subsequent evidence/documentation commit is not a new execution or proof run.

The pinned gate sweep is **12 passed, 2 failed**, not global acceptance.
`documents` retains 17 inherited local-link diagnostics (15 archived-file
references and two directory links); `boundary-controls` fails the
`acyclic-shell` artifact replay without retaining the early gate's stderr.
Both failures and the earlier remote-builder failures remain in the evidence;
no document, fixture, capacity, policy or acceptance rule was weakened.

## Phase 2: law/obligation matrix (pure MC1 model)

Observation relations are kept separate:
- `BehEq` is normal-return behavioral equality.
- Recipe equality (`List Op` equality) is what `reflect` observes.
- Definition/proof-subject identity has no MC1 counterpart. On the
  executable path it is covered by Route B subject binding and the
  ACORR-CASE-03 controls.

These pure laws are the base cases that #11's resource-aware wiring laws
and #7's law bundles should specialize. Nothing here carries a resource
premise.

| Obligation | Checked here (theorems) | Premises | Remains open |
|---|---|---|---|
| PO-02 preservation | MC1: `exec_preserves`, `Run.preserve`, `Step.preserve` (all 22 pure words, including `dip`/`run`/`case`/`if`/`list.case`, nested executions and list/sum/pair values). Effects layer: `Effects.preserve`, `Effects.preserve_reach` (every small step, including halting host steps) | MC1: `CodeTyped code ts ts'`, `StackTyped s ts`, normal-return `Exec`. Effects: `WT E c` | the whole language, implementation |
| PO-03 progress | MC1: `progress_primitive`, `host_word_outside_model`. Effects layer: `Effects.progress` (a typed running configuration with code steps for **every** host response, `run` included, so there are no forbidden stuck states) and `Effects.halted_no_step` | Effects: `WT E c`, `status = running`, `code ≠ []`, total host oracle `H` | termination (not claimed), quota/exhaustion classes, MC1 big-step control words, the whole language |
| PO-04 effect soundness | Effects layer: `Effects.effect_sound` (every finite prefix: `trace' = trace ++ new`, append-only, and every new label is in the bound `E`, **including denied/failed requests**), `step_trace`, `trace_within_bound`. Negative: `bound_does_not_fix_order` (the same bound allows different orders, so a bound does not justify reordering), `denied_request_recorded_and_halts` | `WT E c` (or a typed program from `init`); `Reach H` for any host oracle | effects beyond emit/acquire/release, real host contracts and `test.emit` implementation, ordered-trace *equivalence* laws |
| PO-05 ownership | Effects layer: `Effects.own_preserve` (stack resources are a permutation of the host live set, no duplicates, fresh ids), `live_change` (the live set changes only by a recorded, permitted `acquire` or `release`; moves are stack permutations), `reachable_sound`, `move_then_release` | `WT E c`, `Own c` | temporaries/heap, borrowing, abnormal-stop cleanup (state is preserved but not cleaned), whole language (#3) |
| PO-06 recursive eligibility | MC1: `program_duplicable`. Effects layer: `ValT.rfree`/`OpT.rfree`/`CodeT.rfree` (typed programs contain no resource at any nesting depth), `typed_rfree` (continuation and every stack program are resource-free), `program_expecting_resource_is_copyable` | `WT E c` | aggregates (pairs/lists) and named captures (MC1 aggregates carry no resources; this layer has no aggregates), #3/#11 vocabulary |
| PO-07 construction/execution | `construction_is_pure`, `latent_effect_not_performed`, `PExpr.exec_code`, `PExpr.exec_code_run`, `BehEq.append`, `BehEq.run`, `compose_assoc`, `compose_identity` | normal returns; `BehEq` observation | effectful laws (same host responses, ordered traces), allocation/quota (deliberately not promised) |
| PO-08 recipe/quotation/reflection | `quote_laws`, `reflect_compose`, `PExpr.exec_code_reflect`, `exec_quote_program_run_run` | — | recipe *normalization* (MC1 does not normalize), named-definition reflection |
| negative: behavior ≠ recipe | `behavior_is_not_recipe` (`1 1 +` vs `2`; `BehEq` is not a congruence under `reflect`) | — | — |
| negative: incompatible ordered interfaces | `compose_requires_exact_middle` | `m ≠ m'` | — |
| negative: wrong-subject evidence | `wrong_subject_evidence` (+ ACORR-CASE-03 executed in Phase 3) | — | — |
| negative: resource duplication/discard/capture | `dup_res_untypable`, `drop_res_untypable`, `quote_res_untypable`, `push_res_untypable`. The untyped runs `untyped_dup_breaks_ownership`, `untyped_drop_breaks_ownership` and `untyped_capture_breaks_ownership` reach states that violate `Own`/`RFreeV`, so the typing premise is load-bearing | — | resource-aware wiring laws (#11) |

`specs/verification/obligations.json` is **unchanged**. Every proved item
above is a pure-MC1-model slice. None discharges a registry obligation as
stated, because those cover the whole language or the implementation.

## Binary32 reference model (issue #38)

`NobleFoundation/Binary32.lean` is a small, explicit, kernel-reducible model of
IEEE 754 binary32 for the Numeric-Graph-v1 slice: raw 32-bit encodings,
classification, an exact scaled-integer interpretation of every finite
encoding, round-to-nearest/ties-to-even with gradual underflow, and
add/subtract/multiply. `NobleFoundation/Binary32Expectations.lean` states the
proved theorems and kernel-checks 26 retained expectations.

The model is *selected*, not proposed: it is Lean 4.31.0 at the Noble pin with
no new dependency, no axiom beyond the foundation's allow-list, and no
`native_decide`, `decide +native` or native `#eval` anywhere in the lane.

| Item | Selected |
|---|---|---|
| Model | explicit `NobleFoundation.Binary32` definitions, no opaque Lean `Float32` |
| Rounding | round-to-nearest, ties-to-even, single rounding per operation |
| Subnormals | retained, gradual underflow at `2^-149` |
| Overflow | rounded magnitude at or above `2^128` becomes an infinity with the operand sign |
| Signed zeros | distinct observations; the xor sign rule for multiplication, and the addition rules of IEEE 754 |
| NaNs | every NaN encoding is one observation class (`obsEq`); arithmetic produces the canonical `0x7FC00000` |
| Observations | `obsEq` equality: NaNs collapse, every other class keeps its exact bytes |
| Correspondence | **none claimed** to the Rust graph evaluator, `wasm-opt`, V8 or any host FPU |

`verification/binary32/propose.py` proposes expected bits from Python integers
and its own rounding search. A proposal is not evidence: `retained_expectations_checked`
is a single theorem the Lean kernel checks over all 26 retained expectations.

Proved here:

| Theorem | Claim |
|---|---|
| `retained_expectations_checked` | every retained expectation is the model's answer for its exact operand encodings |
| `halfway_ties_round_to_even` | halfway ties round to even in both directions around `1.0` |
| `signed_zero_rules` | `(-0)+(-0) = -0`, `(+0)+(-0) = +0`, `(-0)-(+0) = -0`, `x-x = +0` for nonzero `x` |
| `nan_class_and_zero_separation` | all NaNs are one observation class; signed zeros and distinct finite encodings stay separate |
| `encoding_boundaries` | exact scaled interpretation of the smallest subnormal, smallest normal and largest finite encoding |
| `exact_sum_exceeds_the_rounded_result` | the exact sum `1.0 + 2^-24` is not representable yet rounds to exactly `1.0`, so round-to-real reasoning cannot discharge binary32 |
| `reassociation_is_observable` | binary32 addition is not associative, so a fold order is observable |
| `implicit_contraction_is_observable` | rounding the product before the sum differs from one rounding of the exact three-operand sum |
| `multiplication_rounds_once` | multiplication takes the sign exclusive-or and rounds the exact operand product once |

The test-local mutation lane (`NobleFoundation/Binary32Mutations.lean`) declares
six wrong models that are compiled only in this project: away-from-zero ties,
collapsing every zero result to `+0`, flush-to-zero, a zero-sign-insensitive
observation, a commutative subtraction and implicit contraction. Each is caught
by a named retained expectation, and `all_declared_mutations_are_caught` states
that none of the declared variants escapes. Detection is kernel-checked model
disagreement, not a Rust-side mutation run.

The packed Rust graph reference and the compiled graph legs replay all 26
retained expectations (`crates/noble-ngraph-experiment/tests/b2_differential.rs`):
the packed lane calls `noble_ngraph::sem::add/sub/mul`, and the compiled lane
builds a two-input F32 graph per expectation and checks legs agreement plus the
L2 APFloat, L3 reference and L4 compiled observations. NaN expectations match by
observation equality and every other class by its exact encoding. The
receipt `verification/binary32/replay.json` names clean commit `9845a9e9d`,
`rustc 1.96.0-nightly` and host `Linux 6.18.54 x86_64`. The first replay found
that the Rust reference returns `0xFFC00000` where the model produces its own
canonical `0x7FC00000` for `inf + -inf`; both are one NaN observation, which is
exactly why the lane compares observations rather than payload bytes.

A bounded deterministic shrinker in the same test file minimizes one binary32
operation discrepancy: it walks a documented simplification list, accepts only
strictly simpler operand pairs under the documented order, and reports a
`local` minimum inside a documented attempt budget. The flush-to-zero lane
checks determinism, replayability and termination, and
`verification/binary32/shrinker.json` records the witness
(`0x00800000 * 0x3f000000` keeps its subnormal product where flush-to-zero
returns zero), the clean commit, the toolchain and the host. Because a
simplification outside that list is never invented, the lane claims no
reachability beyond the list and no global minimality.

Open, and deliberately not closed by this model:

- correspondence between `crates/noble-ngraph/src/sem.rs` and the model, which
  is a separate trusted-correspondence claim: the replay shows agreement on 26
  retained expectations on one host and target, not a refinement proof;

- the ordered affine fold's individual rounding steps as graph operations, the
  extraction, training, snapshot and codec layers, guard/index rules, software
  helpers and optimizer behavior;
- a universal proof of the rounding function: its correctness is checked by
  kernel-reduced examples and boundary reasoning, not by a general theorem;
- shrinking driven from a retained-expectation discrepancy: the shrinker lane
  minimizes a bare operation discrepancy, so an expectation failure is not yet
  minimized automatically;
- the APFloat differential lane stays secondary finite evidence, and its guard
  facts are not independent.

The retained receipt is
[`verification/foundation/receipts/4be2a68/`](../../verification/foundation/receipts/4be2a68/index.json):
205 theorems, nine of them binary32, `clean_source: true`, axiom sets inside
`propext`/`Classical.choice`/`Quot.sound`.

## Ownership accounting oracle (issue #59)

An independent Lean ownership-accounting oracle for the ordered stack / resource
rules lives in `NobleFoundation/Ownership.lean`. It adapts the
executable-checker-to-declarative-typing architecture rather than Move's
ownership policy: an executable checker `run` and an independent declarative
judgment `Accounts`, with `run_sound` proving that every checker acceptance has
a judgment.

The declared fragment is deliberately small: data and must-use resource
payloads, recursive `wrapped` payloads, the words `dup`/`drop`/`swap`/`quote`,
composition, a finite two-arm branch and the ordinary error-return alternative.
A payload is duplicable exactly when it contains no resource at any depth
(`duplicable_iff`), wrapping never changes that
(`wrapDepth_preserves_eligibility`), a must-use resource as the constrained
operand blocks every word (`resource_operand_blocks_every_word`), dropping and
duplication therefore only move data obligations (`drop_consumes_one`,
`dup_duplicates_one`), quoting retains rather than consumes
(`quote_retains_one`), the opaque segment's must-use obligations are conserved
by every admitted program (`segment_conserved`), a two-arm branch accounts for
the same obligations on both arms (`branch_accounts_both_arms`), and an ordinary
error return is admitted only with no outstanding operand
(`fail_requires_empty_operands`). Eighteen theorems in total, all inventoried.

The retained receipt is
[`verification/foundation/receipts/e7cc00993/`](../../verification/foundation/receipts/e7cc00993/foundation-check.json):
252 theorems, eighteen of them ownership, `clean_source: true`, axiom sets
inside `propext`/`Classical.choice`/`Quot.sound`.

The comparison is finite and deliberately narrow:
[`verification/ownership/differential.json`](../../verification/ownership/differential.json)
records 104 independent cases generated from the model, of which 15 accepted and
6 rejected cases are driven through the actual acceptance machine
(`noble_kernel::acceptance::check`) over the unary `dup`/`drop` fragment, and 83
are model-only. A test-local shadow checker agrees with every expectation and
each of seven declared mutations (duplicating or releasing a must-use resource,
ignoring swap eligibility, consuming a quoted operand, ignoring branch
accounting, permitting an error return with an outstanding operand) is caught by
at least one retained expectation.

Open, and deliberately not closed by this model:

- correspondence between `crates/noble-kernel/src/acceptance` and the model: a
  handwritten comparison, not an extraction or refinement proof;
- retain-in-place production analogues beyond the unary fragment: the production
  quotation constructor wraps its operand into a program value, and the
  two-operand swap value-slot ordering is not exposed as an operand-stack order
  by the acceptance machine, so those cases stay model-only;
- the production rejection category of a resource payload can be a stack-join
  failure rather than eligibility; the lane asserts agreement of the accounting
  decision and reports the split;
- OWN-PO-2 (scope escape) and any future scoped-borrow syntax, package
  retention/reclamation and the host-ledger OWN-PO-4..7 obligations;
- host resource release: no admitted decision in this fragment releases a native
  object, and no runtime claim follows for an unimplemented profile.

## Root-pinned live-dispatch model (issue #47)

`NobleFoundation/LiveDispatch.lean` models the live-reference design's
root-pinning claim with bounded roots, ordered entry maps, single-slot
global-epoch publications and an ordered guest trace whose control events
(denials, publication notices) stay separately identifiable.

Proved: one publication changes only its own slot's selection
(`publish_preserves_other_keys`, `publish_selects_the_published_generation`);
pinning stores the map and nothing replaces it in place (`pin_is_immutable`);
a root's selections come from its own pinned map
(`root_selections_use_pinned_map`) and equal a frozen-entry-map reference for the
same request sequence (`frozen_entry_map_reference_agrees`); two separate
single-slot publications are not one atomic replacement, so a root pinned
between them selects a mixed pair (`mixed_map_selects_old_and_new`); a denied
dispatch produces no guest event (`denied_dispatch_produces_no_guest_event`,
`authorized_dispatch_selects_the_pinned_generation`); and a finite abnormal
prefix keeps its committed events (`abnormal_prefix_preserves_committed_events`).
Nine theorems, all inventoried; receipt
[`verification/foundation/receipts/d2aa7a382/`](../../verification/foundation/receipts/d2aa7a382/foundation-check.json)
(261 theorems, `clean_source: true`, axiom sets inside the allow-list).

Open, and deliberately not closed:

- root-snapshot consistency is proved; *coordinated-version coherence* — that a
  pinned map holds a behaviorally compatible set for one refactoring — is a
  separate premise and is not proved here;
- the interface-compatible two-version fixture and the broken
  per-dispatch-current-map control that must expose the duplicate/omitted event
  are Phase 2 work, bound to an existing supported observation rather than a new
  host API;
- no correspondence claim relates the model to
  `crates/noble-cli/src/core/runtime/slot-registry.mjs`;
- no event-count claim applies when a denied dispatch or effect stopped
  execution before the event, and no rollback is implied by a committed prefix;
- atomic multi-slot replacement, state/resource migration, new
  contextual-effect inference, liveness and exactly-once effects are outside it.

## Status by phase

| Phase | Done (checked) | Open |
|---|---|---|
| 1 | whitelist/representation/observation table; signed ZMod/BitVec/K-NUM-01 correspondence; universal add/sub/mul with preserved typed prefix and orientation; literal refusal vs wrapping (spec and transcription); lifting to sequences and quote/compose/run; regressions; inventory and axiom audit | Rust frontend ↔ transcription refinement; MC1 `CodeTyped` ↔ kernel acceptance |
| 3 | one example from Lean spec to admitted bytes (opt off/on), helper model, ACORR-mapped refusals | E5 lowering, E7 assembler, E8 optimizer, E10 loader/engine, E12 PAA-1 envelope, ACORR-CASE-09, quote/compose/run helper models |
| 4 | seeded differential comparator; separate fixed-input scalar if/case EMI; Lean-checked expectations, opt off/on, shrinking and replay, mutation controls, checker lanes, source-bound receipts, `--verify`, retained in-repo receipts, `ci.sh` recipe | Nix-sandbox CI check (see "CI": networked Mathlib cache, absent offline inputs, host services for the slice); rebuilt Mathlib `.olean` files; Rust renderer/lowering refinement and universal compiler equivalence |
| 2 | MC1: preservation and primitive progress, pure laws and negative cases. Effects/resource layer: preservation, total progress, effect soundness for every prefix including denied/failed, ownership preservation with declared transitions, resource-free capture, resource negative cases | the whole language and implementation, abnormal cleanup, aggregates/borrowing, effectful equivalence laws, normalization |

### Effect and resource layer (`NobleFoundation/Effects.lean`)

This is a separate small-step model; MC1 is unchanged. It has:
- I64, move-only resource tokens, and programs with a latent effect bound
  `prog i o e`;
- the host words `emit`, `acquire` and `release`, which a total host oracle
  `H : Nat → Response` answers with `permitted`, `denied` or `failed`;
- an ordered request trace that records every request, including denied and
  failed ones;
- a continuation `code` that serves as the frame stack (`run` prepends the
  body);
- a host-tracked `live` set and a fresh-id counter.

A non-permitted response halts the configuration with status `denied` or
`failed` and leaves the stack, code and ownership unchanged. Cleanup after
an abnormal stop is **not** modelled.

Stacks are top-first in this file. Every claim is a model claim. It does not
describe the Rust kernel, `test.emit`, or the runtime's handle tables.

The CI recipe and its limits are in the Phase 4 section above. The retained
receipts are under
[`verification/foundation/receipts/`](../../verification/foundation/receipts/README.md).
