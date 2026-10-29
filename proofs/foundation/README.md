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
- scans `NobleFoundation/` for `sorry`, `acquire`, `native_decide`,
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

## Closure criterion for issue #15 (owner decision 2026-10-26)

The end-to-end foundation is **complete** for issue #15 once every claim-table
row in this README names either a kernel-checked theorem set carried by a
receipt or an explicitly labelled `open`, `tested` or `assumed` obligation, and
no receipt claims implementation correctness.

That criterion is machine-audited rather than asserted:
[`verification/foundation/claims.mjs`](../../verification/foundation/claims.mjs)
parses every claim table, requires an allowed status word in each row, requires
a named model or theorem set unless the row is explicitly open, checks every
named theorem against [`inventory.json`](inventory.json), and refuses a receipt
that claims an implementation theorem. Names that are specification identifiers
rather than inventoried theorems are declared with their reason in the audit
itself. The retained result is
[`verification/foundation/claims.json`](../../verification/foundation/claims.json):
13 claim-table rows, 12 proved, 5 tested-bearing and 8 open, 484 inventoried
theorems, no diagnostics. The audit is a *document* check: passing it is not a
correctness claim about any implementation, and the rows labelled open stay
open.

`verification/foundation/ci.sh` runs the audit beside the proof check.

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
| Specification-quality obligations are separate from implementation satisfaction | `realizable_of_uninhabited`, `unrealizable_of_contradictory_post`, `not_deterministic_of_two_outputs`, `deterministic_of_unique_output`, `increment_inhabited`, `increment_realizable`, `increment_deterministic`, `contradictory_inhabited`, `contradictory_not_realizable`, `underconstrainedBool_two_outputs`, `underconstrainedBool_realizable`, `underconstrainedBool_not_deterministic`, `emptyOutput_deterministic`, `emptyOutput_not_realizable`, `determinacy_does_not_imply_realizability` | none (model relations only; `NobleContracts.exportedClaim` is not related to this file) | **proved** (model). Renderer/retained-contract correspondence **open** |
| Reverse-mode AD reference over the bounded smooth fragment | `evalStep_hasDerivAt`, `valuesUpTo_hasDerivAt`, `tangent_is_directional_derivative`, `repeated_operand_contributes_twice`, `adjoint_step_adds_contribution`, `adjoint_unchanged_when_operand_absent`, `exampleGraph_wellFormed`, `exampleGraph_value`, `exampleGraph_tangent`, `exampleGraph_derivative`, `unused_input_slot_tangent`, `non_unit_seed_unconsumed` | `crates/noble-ngraph/src/grad.rs` reverse loop and the binary32 operation sequence | model **proved**. Ordered binary32 gradient model, Rust refinement and the transpose identity **open** |
| Packed I64/Bool schedule: result equivalence and missing-register behavior | `schedule_sound`, `execSchedule_sound`, `step_computes`, `step_written`, `run_emitNodes_eq`, `foldl_eq_execSchedule`, `valuesUpTo_eq_value`, `orderOk_cons`, `exampleGraph_wellFormed`, `exampleSchedule_valid`, `exampleGraph_values`, `exampleGraph_schedule_sound`, `aliasing_loses_the_constant`, `a_missing_operand_refuses_rather_than_fabricating`, `a_missing_second_operand_refuses_rather_than_fabricating`, `an_unwritten_register_keeps_its_value`, `an_unwritten_register_stays_empty`, `an_omitted_output_node_yields_no_output` | `crates/noble-ngraph/src/compiled.rs` `Compiled::new`, the packed `I` instruction stream and its evaluator | model **proved** (schedule soundness and the handwritten `Option Val` machine's empty-register behavior). Actual emitted-artifact binding, the Rust transcription, graph guards/`fail_suffix_min`, `F32` and end-to-end checker execution **open** |
| Pure I64 graph rewrite certificates: rule, congruence, lifting and checker soundness | `fold_add_constants`, `add_wraps_at_the_boundary`, `neutral_add_left`, `neutral_mul_right`, `evalStep_congr`, `congruence`, `replacement_lift`, `checked_sound_const_add`, `checked_sound_zero_left`, `checked_sound_one_right`, `constant_value`, `foldGraph_wellFormed`, `foldGraph_reference`, `fold_certificate_accepted`, `wrong_constant_refused`, `swapped_operands_refused`, `wrong_rule_identity_refused`, `wrong_bits_refused` | `crates/noble-ngraph/src/ir.rs` graph producer and any future untrusted transformation | model **proved** (rules, lifting and checker soundness). Rust transformation, derivation encoding/replay budget, endpoint decoding of actual graphs and independent differential execution **open** |
| Bounded graph-pair query model: witness decoding, exhaustive search and refutation | `lookup_returns_recorded`, `decode_uses_fallback`, `decode_replays_recorded`, `not_refuted_iff_agrees`, `search_none_sound`, `search_some_refutes`, `same_sum_search_none`, `same_sum_agrees`, `subtraction_refuted`, `witness_bits_decide`, `unrecorded_slot_falls_back`, `corrupted_witness_changes_the_verdict` | `crates/noble-ngraph/src/extract/symbolic.rs` encoder, `crates/noble-ngraph-experiment/src/b2i64` replay legs and any emitted Wasm | model **proved** (decoding only returns recorded pairs; bounded-universe search soundness). Encoder correspondence, solver, compiled-endpoint replay and witness shrinking **open** |
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
- Route A: `noble acquire-artifact`.

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
| E9 candidate = consumer rebuild | `acquire-artifact` reports `correspondence: byte-exact` for opt off and opt on | **checked** (identity, not semantics) |
| E10 instantiation/loader/V8 | Route A instantiates the compared buffer (code reading) | **assumed** (PO-18, PAA-1 O3 open) |
| E11 execution | drivers `x [ … ] run` for `x ∈ {41, max, min}` and a `Bool`/`I64` preserved prefix, opt off/on, through admission. Expectations are kernel-checked against `MathlibI64.wrap` | **tested** (finite) |
| E12 one PAA-1 envelope joining A and B | — | **open** (#10) |

Refusal controls. Each one must fail for its intended reason, or the run
fails.

| Control | #10 id | Result required |
|---|---|---|
| valid `[1 +]` proof, source `[ quote [ 2 + ] compose run ]` | ACORR-CASE-03 | Route B not released |
| `[2 +]` bytes acquired against the `[1 +]` source | ACORR-CASE-03 | `correspondence-reject` |
| opt-on bytes acquired under consumer opt off | S-CASE-07 option mismatch | `correspondence-reject` |
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
the entire source is independently acquired through the real frontend and
kernel before execution. A valid but unexecuted word outside this model
is still ineligible.

The deterministic corpus has 16–64 positive pairs and three tagged refusal
probes: an out-of-range literal, a wrong branch type, and an acquired word
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
mutation site. Every retained candidate is re-acquired, freshly checked by
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

### The ordered `affine` fold, and the layer classification

`fold` in the same model is the documented `sem.rs` recursion
`v₀ = bias; vᵢ = rn(vᵢ₋₁ + rn(cᵢ × xᵢ))` over the reference operations, so the
fold's order is a statement rather than a comment: `fold_empty`, `fold_append`
(splitting the terms splits the fold) and, as fixtures, the two signed-zero
biases staying distinct in sign (`fold_negative_zero_bias_stays_negative`,
`fold_positive_zero_bias_stays_positive`). `fold_order_is_load_bearing` is the
consequence a reader needs: at `bias = 1.0` the terms `1.0 · 2^24` then
`-1.0 · 2^24` fold to `+0` because the `1.0` is rounded away before the
cancellation, while re-associating the same three values gives `1.0` — so
reassociation, FMA contraction and parallel reduction each change a documented
result.

The remaining `sem.rs` layers are classified rather than left ambiguous:

| Layer | Status |
|---|---|
| per-operation rounding, add/subtract/multiply | **modelled and proved** (above; 26 retained expectations, mutation lane, packed and compiled replay) |
| ordered `affine` fold | **modelled and proved** (`fold`, `fold_append`, `fold_order_is_load_bearing`) |
| activations `relu`, `reglu`, `step` | **not modelled**: they are activation corners and surrogate policies, which the Numeric-Graph-v1 selection excludes; no theorem here says anything about them |
| guard/index rules (`is_bit`, `select`, `index_position`) | **not modelled**: index refusal is a `crates/noble-ngraph` admission rule, not a binary32 operation, and no Lean model exists |
| decimal import / codec (`exact_f32_decimal`) | **not modelled here**: #40 owns exact and RNE conversions and decimal import |
| snapshot / parameter layout (`snapshot.rs`) | **not modelled**: layout and retention are #33/#43 subjects and have no binary32 obligation |
| training admission (`grad.rs`) | **not modelled here**: #45 owns the derivative and training admission |
| software helpers (`pow2`, `log2`, `pow2Int`, `roundPositive`) | **model-internal and structural**: they are the reference's own arithmetic, total and kernel-reducible, so they carry no Rust correspondence claim |
| optimizer slice | **not modelled**: no optimization strength or decision is claimed for binary32 |

Receipt for the fold and the classification:
[verification/foundation/receipts/c33885419/](../../verification/foundation/receipts/c33885419/foundation-check.json)
(463 theorems, `clean_source: true`, axiom sets inside the allow-list).

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
by every acquired program (`segment_conserved`), a two-arm branch accounts for
the same obligations on both arms (`branch_accounts_both_arms`), and an ordinary
error return is acquired only with no outstanding operand
(`fail_requires_empty_operands`), plus the branch-join theorems below. Twenty-six
ownership theorems in total, all inventoried.

### Branch join (issue #59 joins)

The two-arm branch's abstract stack is a join, not "both branches have the same
number of resources". `BranchJoin` carries both arm results at equal length, so a
position one arm lacks is refused rather than padded
(`unequal_arms_cannot_be_joined`), and `mergePosition` records both arms at every
position (`merge_represents_each_arm`). Two different payloads at one position are
recorded as conflicting rather than merged (`distinct_owners_are_not_merged`), so
the seeded join that keeps only the first arm is refuted wherever the arms differ
(`one_armed_join_is_refuted`). The controls that a weaker join fails are proved as
theorems: equal counts with different identities are conflicting
(`same_count_different_identity_is_conflicting`), a wrapped payload is not merged
with its unwrapping although both are eligible
(`nested_payload_is_not_merged_with_its_unwrapping`), and a wrapped must-use
resource is never merged with a plain payload (`nested_resource_is_not_merged`).
The substantive preservation result is `joined_word_admission_agrees`: two stacks
whose ordered positions agree on eligibility are acquired by one word in both arms
or in neither, so the joined abstract stack preserves this fragment's accounting
decision rather than inventing one.

The branch constructor stays outside the join lane on purpose: it compares the two
arms' actual results, so a join can never be used to skip accounting one arm, which
is what `branch_accounts_both_arms` and the mutation controls establish. The join
controls are model-level only; the production acceptance machine has no join
concept, and a join is never presented as an executable production control.

Not proved, and recorded rather than implied: the admission simulation lifted over
a whole word *sequence*, which needs the intermediate stacks to keep agreeing.

The retained receipt is
[`verification/foundation/receipts/e0ec33f66/`](../../verification/foundation/receipts/e0ec33f66/foundation-check.json):
405 theorems, twenty-six of them ownership, `clean_source: true`, axiom sets inside
`propext`/`Classical.choice`/`Quot.sound`.

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
- host resource release: no acquired decision in this fragment releases a native
  object, and no runtime claim follows for an unimplemented profile.

## Checkpoint-scoped authority ledger (issue #66)

`NobleFoundation/AuthorityLedger.lean` models the M5 protected-operation
authority ledger with bounded witnessed requests, an independently supplied
fact bundle (plan, grant, requested identities, source authenticity, checkpoint
validity, policy liveness, credentials, revocation, cost, available quota), a
consumed-witness list, a registered-attempt ledger and checkpoint-local charges.
Denials and preflight refusals leave the ledger definitionally unchanged; only
their separately recorded request/effect counters move.

Proved: initialization of align/unique/quota/at-most-once
(`initial_alignment`, `initial_uniqueness`, `initial_within_quota`,
`initial_started_at_most_once`); one admission consumes exactly one fresh
witness and registers exactly one not-yet-started attempt
(`admission_consumes_exactly_one_witness`, `admission_registers_one_attempt`,
`admission_is_consumption`) while keeping alignment, uniqueness, the
checkpoint quota bound and attempt/charge accounting
(`admission_preserves_alignment`, `admission_preserves_uniqueness`,
`admission_preserves_quota`, `admission_preserves_attempts_charged`); any finite
serialized prefix keeps alignment (`acquirePrefix_preserves_alignment`); an
altered requested plan, revoked or stale facts, or an already started attempt
mint nothing (`altered_plan_cannot_mint`, `stale_facts_cannot_mint`,
`started_attempt_cannot_start_again`). Four mutation models break the
corresponding invariant, which is what makes exact plan equality, checkpoint
validation, freshness and the quota charge load-bearing rather than decorative
(`omitted_quota_charge_breaks_accounting`, `unchecked_reuse_breaks_uniqueness`,
`plan_blind_admission_mints_altered_plan`,
`checkpoint_blind_admission_mints_stale_checkpoint`).
`renewed_snapshot_keeps_quota` states checkpoint renewal as a host premise: a
fresh snapshot must carry prior reservations forward, and host replenishment
that ignores them is outside kernel-enforced safety. Twenty-five inventoried
theorems; receipt
[`verification/foundation/receipts/674b32d9/`](../../verification/foundation/receipts/674b32d9/foundation-check.json)
(286 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- no correspondence claim relates the model to
  `crates/noble-kernel/src/authority`: there is no authority extraction yet, so
  this is finite model evidence rather than an implementation theorem;
- the finite Rust comparisons of Phase 2 are named, measured results over the real
  authority table rather than theorems, and they are partial: two witnesses
  sharing one checkpoint's quota is measured by
  `bindings::same_checkpoint_quota_is_reserved_across_distinct_witnesses`;
  admission order independence by `phase2::both_admission_orders_charge_the_same_quota`
  (both orders commit exactly one and refuse exactly one, with identical
  counters); exhaustion *before any charge* by
  `phase2::quota_exhaustion_stops_before_consumption`; a fresh snapshot that does
  not carry prior reservations by `phase2::a_fresh_snapshot_without_carry_forward_is_refused`,
  which observes `Denial::InconsistentQuota` rather than extra capacity; and
  finalization between admission and start by
  `phase2::finishing_the_invocation_between_admission_and_start_stops_the_start`
  (the start is refused `InvocationFinished`, no protected operation runs, and
  the consumed witness is not restored). Facts changing between authorization
  and admission are measured for a changed quota snapshot (refused
  `InconsistentQuota`) and for revocation
  (`phase2::revocation_between_authorization_and_admission_refuses_the_admission`:
  refused `Denial::Revoked` with no attempt registered, nothing consumed and the
  witness left retirable); a change of credentials, policy or checkpoint between
  those two calls has no separate Rust scenario;
- no concurrency, crash-recovery, remote-revocation atomicity or exactly-once
  claim, and no cross-checkpoint conservation claim without the carry-forward
  premise.

## Reverse-mode AD reference (issue #45)

`NobleFoundation/ReverseAD.lean` models the bounded smooth slice — a
well-formed acyclic graph over the reals with explicit input and parameter slots,
exact finite constants, addition and multiplication, and one selected output.
Guards, indexing, Boolean paths, activation corners, surrogate rules, and the
subtraction and ordered affine terms the issue grants only later are outside the
file, as are dynamic Programs.

Proved: a node's evaluation is differentiable along a direction line whenever its
operands are (`evalStep_hasDerivAt`), and by induction on the stored order every
node's denotation is (`valuesUpTo_hasDerivAt`), so the forward tangent recursion
*is* Mathlib's derivative of the intended-real denotation at the declared
assignment — the JVP specification (`tangent_is_directional_derivative`). The
reverse traversal adds one processed consumer's contribution to an operand's
adjoint (`adjoint_step_adds_contribution`), a repeated operand contributes twice
so accumulation adds rather than overwrites (`repeated_operand_contributes_twice`),
and a node that is not an operand of the processed consumer is left unchanged,
which is what makes the descending order finalize an adjoint before its own step
(`adjoint_unchanged_when_operand_absent`). A contribution multiplies the
**consumer's** own adjoint by its partial-derivative coefficient; the first draft
multiplied the *operand's* adjoint instead, which cannot carry a seed from the
output node to its operands at all, and the transpose identity below refuted it.
The transpose is then proved at two scopes: the reverse-loop **pairing
invariant** (`adjointDot_step`) says pairing the accumulated adjoints with a
direction over the graph's slots adds exactly the processed consumer's
contributions, and the fixture-level **transpose identity**
(`exampleGraph_adjointDot`) says the sweep's accumulated adjoints pair with the
declared direction to exactly the tangent `8`. The overwrite control
(`the_overwriting_update_pairs_to_four`) pairs to `4` on the same slot where the
sweep pairs to `8` (`the_sweep_pairs_to_eight_on_the_second_slot`), so the fixture
discriminates the update rule rather than agreeing with anything.

Fixtures exercise the model: the example graph's well-formedness, denotation
`16`, tangent `8` and its Mathlib derivative statement; a direction on an unused
input slot contributing nothing; a non-unit seed surviving until a consumer
contributes; and the pairing identity with its overwrite control. Receipt
[`verification/foundation/receipts/b91f5504c/`](../../verification/foundation/receipts/b91f5504c/foundation-check.json)
(458 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- no ordered **binary32** gradient model: a real-valued theorem does not
  establish that binary32 gradients equal exact derivatives, and no
  reassociation, FMA contraction or parallel reduction is authorisation;
- the transpose identity is proved for the **one-step pairing and one checked
  fixture**, not for every well-formed graph, and the full summation theorem
  stating each final adjoint as seed plus its consumers stays a named
  obligation;
- no correspondence claim relates this file to
  `crates/noble-ngraph/src/grad.rs`, so the ordered Rust loop's refinement is
  open, and its admission, error and quota behaviour is untouched;
- guards, indexing, Boolean paths, activation corners, surrogate policies,
  dynamic Programs, multi-output seeds and duplicate output-node accumulation
  are outside the selected fragment;
- vanishing checks are absent by construction (no `sorry`, no axiom, no
  `native_decide`), but that is a hygiene fact, not a claim about the Rust side.

## Packed schedule validator model (issue #43, phase 1)

`NobleFoundation/PackedSchedule.lean` names the model side of a validator lane for
the packed `I64`/`Bool` schedule. `crates/noble-ngraph/src/compiled.rs` states that
agreement between the packed evaluator and the graph reference is *tested, not
proved*, so NG-PO-C stays open; the POPL 2008 translation-validation shape keeps
scheduling untrusted and proves a smaller validator correct instead.

The graph fragment is a finite acyclic graph over interface input slots, exact
`I64` constants, addition, subtraction, multiplication and equality, with one
selected output. The instruction set mirrors the packed `I` enum — destination
register first, then ordered source registers — and constants and interface slots
occupy their own registers, so bits and slots are validated rather than assumed.

Proved: the packed fold executes exactly the scheduled order's emitted
instructions (`run_emitNodes_eq`, `foldl_eq_execSchedule`), a step writes only its
own node's register (`step_written`) and computes that node's reference value when
its ordered operands' registers already hold theirs (`step_computes`), reference
values are stable once later nodes are added (`valuesUpTo_eq_value`), and by
induction on the untrusted order every already-processed node's register holds its
reference value (`execSchedule_sound`). The closure is **validator soundness**:
for every acquired input the packed machine's selected output register equals the
graph reference (`schedule_sound`) — acceptance implies complete ordered result
equivalence, with refusal allowed for unsupported valid schedules.

Five fixtures and one control: the four-node example graph (input, exact constant,
addition, equality) with values `3`, `8` and `true` (`exampleGraph_values`); one
register per node in dependency order accepted without aliasing
(`exampleSchedule_valid`) and agreeing with the reference
(`exampleGraph_schedule_sound`); order correspondence decomposing at the head
(`orderOk_cons`); and the control showing that **aliasing two live nodes onto one
register changes the machine's value** (`aliasing_loses_the_constant`), which is
why injectivity is a premise of the relation rather than a fresh-register
assumption. The original thirteen-theorem set is retained in receipt
[`verification/foundation/receipts/0ade1e4e9/`](../../verification/foundation/receipts/0ade1e4e9/foundation-check.json)
(327 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Five further theorems concern **missing operands and unwritten registers in the
handwritten `Option Val` machine**. An empty first or second operand makes
`add`/`sub`/`mul`/`eq` write `none` to the destination, including over an earlier
value (`a_missing_operand_refuses_rather_than_fabricating`,
`a_missing_second_operand_refuses_rather_than_fabricating`). A register with no
executed writer preserves its initial contents
(`an_unwritten_register_keeps_its_value`); it stays empty when the machine starts
empty (`an_unwritten_register_stays_empty`). The selected output is `none` only
when **every** executed node avoids writing its register
(`an_omitted_output_node_yields_no_output`): omitting the output node is not enough
if another node aliases that register. These are model properties, not graph
guard semantics, Rust runtime checks or validation of emitted instruction bodies.

The complete eighteen-theorem `PackedSchedule` set is audited in
[`verification/foundation/receipts/8eef09184/`](../../verification/foundation/receipts/8eef09184/foundation-check.json):
484 total foundation theorems at source `8eef0918484ab1d887ba773f99eab13fa31d3ab3`,
`clean_source: true`, `implementation_theorem: false`. The five additions use
only `propext` and, for the second-operand theorem, `Quot.sound`.

Open, and deliberately not closed:

- **no binding to the actually emitted artifact**: the file models a schedule and
  its instruction stream, not the `Compiled` layout key, layer ranges, register
  count or constant/slot tables that `Compiled::new` produces, so a sound abstract
  checker plus an unproved Rust transcription is not an end-to-end proof;
- graph guards, `fail_suffix_min` canonical-first-failure behaviour, ordered affine
  and `F32` instructions are outside the fragment (`F32` only after #38), as are
  effects, resources, aliasing *after* last use and any multi-output reconstruction;
- refusal classes, bounded checking work, versioned encoding and fail-closed
  preflight are named obligations rather than implemented behaviour here;
- this is not #10's MC2 trusted-build envelope and not a whole-Wasm validation
  path, and finite fixtures are not a universal theorem about the Rust scheduler.

## Graph rewrite certificates model (issue #42, phase 1)

`NobleFoundation/GraphRewrite.lean` (over the `PackedSchedule` model) keeps
optimization and search untrusted and proves the accepted rewrite rules,
congruence, the lifting of one local replacement to the whole graph, and a small
checker whose acceptance implies the declared denotation. The slice is the same
total `I64`/`Bool` one: input/constant, wrapping add/subtract/multiply and
equality. Guards, effects, resources, reflection, `F32` and dynamic calls are
outside the file.

Proved: constant folding denotes the wrapping sum (`fold_add_constants`), including
the boundary case (`add_wraps_at_the_boundary`); `0 + x` and `x * 1` denote `x`'s
`I64` denotation, with the operand's `I64` value as an explicit premise rather than
a total rewrite claim (`neutral_add_left`, `neutral_mul_right`); the step function is
congruent in node identity and operand lookups (`evalStep_congr`), so equal operand
denotations give equal node denotations (`congruence`); and **lifting** carries one
unchanged local denotation to agreement over every node of the graph, which
preserves unaffected shared uses (`replacement_lift`). The checker decodes exact
node shapes, operand positions and constant bits from the graph — it never asks
the reference for the answer — and acceptance implies the declared denotation
(`checked_sound_const_add`, `checked_sound_zero_left`, `checked_sound_one_right`).

Fixtures and controls: the three-constant fixture denotes `8` at both its addition
and its multiplication (`foldGraph_reference`); the true constant-folding claim is
accepted (`fold_certificate_accepted`) and a wrong claimed constant
(`wrong_constant_refused`), swapped operand positions (`swapped_operands_refused`),
a claim naming a node of another kind (`wrong_rule_identity_refused`) and the
identity claim at swapped positions (`wrong_bits_refused`) are refused. Eighteen
inventoried theorems; receipt
[`verification/foundation/receipts/71c02b9bb/`](../../verification/foundation/receipts/71c02b9bb/foundation-check.json)
(345 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- **no Rust transformation or derivation artifact exists**: the checker validates
  claims over the model graph, not a versioned derivation emitted by
  `crates/noble-ngraph`, and there is no replay budget, decoding of the actual
  original/result graph files, or certificate-size measurement;
- no independent differential execution of the original and rewritten endpoints,
  so this is model evidence rather than the issue's implementation test;
- substitution/transitivity closure, a full e-graph, guard/failure-preserving
  rewrites and `F32` laws are outside phase 1; value preservation says nothing
  about fuel, allocation, cancellation behaviour or recipe identity;
- finite fixtures are not a universal claim about any scheduler or transformation.

## Declared packed graph layout (issue #43)

`NobleFoundation/PackedLayout.lean` models the declared packed layout that
`crates/noble-ngraph/src/layout.rs` emits and the obligations its checker applies:
a revision, a declared register count, one declared register per node, the constant
and interface-slot tables, one instruction range per layer, the failure-suffix
prefix minima and the acceptance conjunction of those obligations.

Proved: an inflated register count is refused (`count_inflated_refused`); a missing,
out-of-range or shared register is refused (`missing_register_refused`,
`out_of_range_refused`, `alias_refused`); a constant table with the wrong entry count
is refused (`constant_count_refused`); the layer table must have one ordered range
per layer (`layer_shape_refused`, `layers_unordered_refused`) and its ranges must
cover exactly the instruction nodes (`coverage_refused`); a declared failure-suffix
table that differs from the graph's own prefix minima is refused
(`fail_suffix_mismatch_refused`); and acceptance implies a register per node,
non-aliasing and coverage (`accepted_implies_registers`,
`accepted_implies_distinct`, `accepted_implies_coverage`).

The clean-source kernel check is retained at `verification/foundation/receipts/09a307c64/`
(393 theorems, `clean_source: true`, every axiom set inside the allow-list).

Per-entry exactness is tied to the readable obligation by `constant_predicate_iff` and
`slot_predicate_iff`, and an entry that names no node is refused
(`constant_entry_refused`, `slot_entry_refused`). The clean-source kernel check is
retained at `verification/foundation/receipts/9b1b93055/` (397 theorems, `clean_source: true`).
Not claimed: no decoding of the Rust declaration's bytes, so nothing binds
`serde`/binary output to these tables; the layout build key, compiler revision and
identity scheme are out of scope. The node vocabulary modelled here is the pure-`I64`
fragment, so the slot table obligation is the one for **input** slots: the `ngraph-f32/0`
profile's `Op::Param` slots and its F32 instructions are not modelled. The Rust validator
covers both slot kinds in one table, as issue #68 required the validator to recognise after
it refused the compiler's own parameter layouts.

## Replayable certificate codec (issue #42, codec half)

`NobleFoundation/CertificateCodec.lean` models a token codec for the same
`ngraph-cert/0` derivation: one seven-token chunk per step, a rule token that must
decode through the modelled rule set, decimal bit tokens that must stay inside the
64-bit range, and a revision token the decoder re-checks before any step.

Proved: decoding an encoded step returns the same step (`decode_encode_step_replays`)
and decoding an encoded derivation returns the derivation itself
(`decode_encode_replays`), so acceptance is invariant under a transport round trip
(`acceptance_preserved_by_codec`); a step is exactly seven tokens
(`encodeStep_length`), so a six-token tail is refused (`truncated_step_refused`);
an unknown rule token refuses the step (`unknown_rule_token_refused`); and a stale
revision token refuses decoding before any step is inspected
(`stale_revision_token_refused`).

The clean-source kernel check is retained at `verification/foundation/receipts/26fca0806/`
(381 theorems, `clean_source: true`, every axiom set inside the allow-list).

Not claimed: tokens are modelled as strings and no byte-level envelope is modelled,
so nothing here binds the `serde`/binary bytes the Rust encoder writes; envelope
digests, replay-budget accounting and the identity rules are out of scope.

## Replayable certificate format (issue #42, format half)

`NobleFoundation/Certificate.lean` models the `ngraph-cert/0` derivation format that
`crates/noble-ngraph/src/cert.rs` emits: the revision, the declared step bound, the
ordered step list (rule identity, rewritten node, ordered operands, declared
constant bits, claimed value), the decode of rule identities to the graph
checker's claims, the structural bit comparison against the graph, and acceptance
of a whole derivation.

Proved: a certificate naming another revision is refused
(`stale_revision_refused`) and that refusal kills acceptance even when every step
would be accepted (`stale_revision_kills_acceptance`); a derivation over the
declared bound is refused (`over_bound_refused`); an unknown rule identity cannot
decode or be accepted (`unknown_rule_refused`); non-increasing node order is
refused (`unordered_refused`); acceptance implies the graph checker accepts every
decoded claim (`accepted_implies_checked`); and consequently an accepted
certificate's fold step denotes the claimed constant
(`accepted_fold_step_denotes`), i.e. the encoder's claim is the checker's
denotation rather than taken on faith. Seven inventoried theorems; receipt
[`verification/foundation/receipts/d67efd4a2/`](../../verification/foundation/receipts/d67efd4a2/foundation-check.json)
(374 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- **no byte decoding**: nothing maps `serde`/binary output of the Rust encoder to
  these structures, so a correspondence theorem binding actual certificate bytes
  (or the Rust checker) to this model still does not exist;
- envelope digests, replay-budget accounting, cold/warm comparison and the #32/#33
  identity rules are outside the model, as are `F32`, guards, effects and
  resources;
- the Rust lane's differential execution (`crates/noble-ngraph/tests/cert_differential.rs`)
  is implementation evidence and is not modelled here.

## Bounded graph-pair query model (issue #56, phase 1)

`NobleFoundation/SymbolicPair.lean` models a *declared finite input universe*, the
exhaustive search over it, and the replay of an exact-bit witness, and proves what
such a search can and cannot establish. It contains no SAT/SMT solver, so no
solver status, no certificate and no external UNSAT reasoning is claimed.

Proved: a witness lookup only ever yields a pair the witness recorded
(`lookup_returns_recorded`), an unrecorded slot decodes to exactly the declared
fallback (`decode_uses_fallback`) and a recorded slot replays exactly its recorded
bits (`decode_replays_recorded`), so a witness cannot acquire bits it did not
carry. No refutation is exactly agreement over the ordered outputs
(`not_refuted_iff_agrees`), and therefore a failed search establishes agreement on
the declared universe **and nothing beyond it** (`search_none_sound`), while a
found candidate is a genuine refutation (`search_some_refutes`).

Fixtures: two differently written faces of `3 + 5` (`add 0 1` and `add 1 0` over
reordered constants) find no distinguishing candidate in the declared universe;
a subtraction face is genuinely refuted (`8` versus `2`); and for a pair whose left
face reads input slot zero while the right face is the exact constant seven, the
**decoded witness bits decide the verdict** (`witness_bits_decide`). Controls: an
omitted slot takes the declared fallback rather than inventing bits
(`unrecorded_slot_falls_back`), and a corrupted witness flips the verdict
(`corrupted_witness_changes_the_verdict`), so the recorded bits are load-bearing.
Twelve inventoried theorems; receipt
[`verification/foundation/receipts/9a810d3ff/`](../../verification/foundation/receipts/9a810d3ff/foundation-check.json)
(357 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Open, and deliberately not closed:

- **no encoder correspondence**: nothing relates `extract/symbolic.rs`, a
  serialized formula or the B2 I64 harness to this model, so there is no
  concrete/symbolic simulation theorem and no decoder/binding of actual endpoint
  bytes;
- no solver, no timeout/UNKNOWN budget, no certificate or witness shrinking, and no
  formula-size, solver-time or memory measurement;
- no replay through the actual emitted Wasm of either endpoint and no
  optimization-mode comparison, so this cannot satisfy the issue's compiled-replay
  acceptance on its own;
- budgets, trap/refusal/quota classification, cancellation, guards, `F32`,
  resources, live slots, recursion and dynamic calls are outside the slice.

### Witness encoder correspondence and first-match search (issue #56, phase 2)

`NobleFoundation/SymbolicSolver.lean` closes two of phase 1's open points for the
*witness representation itself*, and one of them only there.

Proved: the encoder records exactly one pair per declared slot
(`encode_length`); every declared slot decodes back to the candidate's own exact
bits (`decode_encode_replays`), so the encoder cannot smuggle a value the
candidate did not carry; and a corrupted witness decodes to the corrupted bits and
not to the candidate's (`corrupted_witness_shifts_bits`). The search is now
first-match: a failed search establishes agreement on the declared universe only
(`firstIndex_none_agrees`), and a reported index comes with a **structural
decomposition** — the universe is a prefix of candidates that do not refute, of
exactly the reported length, followed by a candidate that does
(`firstIndex_some_decomposes`) — so the reported witness can never be a later,
avoidable one, and no positional lookup into the candidate list is needed to say
so. Fixtures rerun through the search: the same-sum pair finds nothing
(`same_sum_first_none`), the subtraction pair and the input-dependent pair are both
refuted at the earliest declared candidate (`subtraction_first_index_zero`,
`input_first_index_zero`, `input_zero_candidate_refutes`), and encoded witnesses
still decide the verdict while a corrupted one does not (`encoded_witness_decides`).
Ten further inventoried theorems (22 for the #56 slice); receipt
[`verification/foundation/receipts/144434256/`](../../verification/foundation/receipts/144434256/foundation-check.json)
(367 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

Still open, and explicitly not claimed here:

- the encoder correspondence above is for the **witness representation**; it does
  **not** relate `extract/symbolic.rs`, a serialized formula or the B2 I64 harness
  to the model, so there is still no concrete/symbolic simulation theorem and no
  binding of actual endpoint bytes;
- the earliest-witness statement is structural over a *declared* universe: it says
  nothing about assigning cost, formula size, solver time or memory, and the
  universe is still the two declared candidates, not a generated search space;
- no replay through the actual emitted Wasm of either endpoint, no certificate or
  external UNSAT reasoning, and no timeout/UNKNOWN budget;
- guards, input facts, `F32`, resources, live slots, recursion and dynamic calls
  remain outside the slice.

### Seeded fault variants for the symbolic query lane (issue #56, acceptance item 4)

`NobleFoundation/SymbolicMutations.lean` is the test-local faulty-variant lane the
bounded query needs, so that "the search found nothing" cannot stand in for the
reference result. Each variant differs from a reference face in exactly one place,
and a named reference expectation catches it:

| Variant | Fault | Caught by |
|---|---|---|
| `opMutated` | subtracts where the reference adds | `wrong_op_is_caught` |
| `operandMutated` | reaches one node twice (`5 + 5`) | `wrong_operand_is_caught` |
| `constantMutated` | holds six where the reference holds three | `wrong_constant_is_caught` |
| `twoOutSwapped` | asks the right face for its ordered outputs in the other order | `wrong_output_order_is_caught` |
| `encodeShifted` | records each slot's exact bits at the next slot | `shifted_encoding_is_caught` |
| `refutedFirstOnly` | inspects only the first ordered result | `output_blind_verdict_is_caught` |

The encoding and verdict variants are the interesting ones because both are shapes
of *agreement that proves nothing*. The shifted encoder **round-trips at the slot it
recorded**, so two consumers of that one corrupted encoding agree the bits are
present; the graph reads a different slot, and the verdict flips from agreement to
refutation against the reference replay of the candidate's own bits. The
output-blind verdict reports agreement on a pair whose **second** ordered result
differs, while the reference comparison over both declared outputs reports the
refutation. `all_declared_mutations_are_caught` closes the lane: no declared variant
escapes.

This lane is model-level detection. It does **not** supply the missing encoder
correspondence to `extract/symbolic.rs`, the B2 I64 harness legs or any emitted
Wasm, and it involves no solver, timeout budget or certificate. Seven inventoried
theorems (29 for the #56 slice).

## Specification-quality obligations (issue #65)

`NobleFoundation/SpecQuality.lean` separates three questions about a retained
contract from the implementation claim. `Relation` abstracts the statement shape
of `NobleContracts.exportedClaim` — typed parameters and inputs, a precondition
over them, and a postcondition over the input, the exact typed output and the
parameters — and the file defines domain inhabitation, relational realizability
and output determinacy over it.

Proved: a universal realizability proposition is provable from a false
precondition alone, which is exactly the vacuity a report must mark
(`realizable_of_uninhabited`); a contradictory postcondition under an inhabited
domain refutes realizability, with the inhabitation witness required so the
refutation is not itself vacuous
(`unrealizable_of_contradictory_post`, `contradictory_inhabited`,
`contradictory_not_realizable`); two distinct satisfying outputs for one
admissible input refute determinacy (`not_deterministic_of_two_outputs`), and
per-input uniqueness entails it (`deterministic_of_unique_output`); the exact
wrapping `I64` increment with a true precondition is inhabited, realizable and
deterministic (`increment_inhabited`, `increment_realizable`,
`increment_deterministic`); an underconstrained `Bool` output is realizable but
not deterministic (`underconstrainedBool_two_outputs`,
`underconstrainedBool_realizable`, `underconstrainedBool_not_deterministic`);
and determinacy does not imply existence — the empty-output relation is
deterministic vacuously and not realizable (`emptyOutput_deterministic`,
`emptyOutput_not_realizable`, `determinacy_does_not_imply_realizability`).
Fifteen inventoried theorems; receipt
[`verification/foundation/receipts/45d658591/`](../../verification/foundation/receipts/45d658591/foundation-check.json)
(301 theorems, `clean_source: true`, axiom sets inside the allow-list,
`implementation_theorem: false`).

### Reporting layer (issue #65)

`noble spec-quality MANIFEST_JSON` reports submitted evidence against the declared
pure-`I64`/`Bool` profile. It declares the bindings the evidence depends on once —
predicate revision 1 (MC1 `Expression.Holds` as pinned by the contract gate),
obligation-shape model revision 1, `exact-scalar-equality` as the observation
relation, and `ordered-exact-i64-bool` as the parameter binding — and refuses a
submission that carries any other one, each with its own diagnostic
(`wrong-predicate-revision`, `wrong-model-revision`,
`wrong-observation-relation`, `wrong-parameter-binding`). A coarser observational
relation cannot be selected by a producer to make determinacy pass.

Obligation kinds are paired, not interchangeable: a specification-quality claim
(`inhabited`, `realizable`, `deterministic`) submitted as implementation
satisfaction, or the implementation claim submitted as specification quality, is
refused as `wrong-obligation` by explicit claim/kind validation.

The selected-CLI lane checks criterion 6 with six artifact controls: predicate
revision `2`, model revision `2`, an unselected `i64-parity-equality` observation
rule, an `unordered-exact-i64-bool` parameter binding, and claim/kind substitution
in both directions (`deterministic` as implementation satisfaction and `satisfied`
as specification quality). Each exits `2` with its specific refusal class; the
matching wrapping-increment submission remains accepted. The four binding
controls alter only one field of that otherwise valid submission, so an
unrelated malformed relation cannot account for their refusal.

The report keeps four obligations apart: `specification_quality` (only
specification-quality rows, with disjoint established/not-established/vacuous
tallies), `partial_correctness` (the submitted status for the unchanged
`exportedClaim`, reported only when that claim is itself checked),
`termination` (never claimed) and `correspondence` (never claimed). Every report
states `guest_requests: 0`; refusals also state `protected_operations: 0`.
These counters alone are not independent evidence of effect isolation.

Each specification-quality row contributes to exactly one tally: an unchecked
claim is not established, a checked vacuous claim is vacuous, and only a checked
non-vacuous claim is established. Implementation claims never appear in these
rows or counters, whether checked, unchecked or vacuous. Selected-CLI regressions
check implementation-only reports, mixed reports, and the invariant that adding
an implementation claim cannot alter the specification-quality block. A false
precondition produces two vacuous universal claims and one unestablished
inhabitation claim, not established specification-quality evidence. These are
report-classification checks, not verification of a submitted implementation
claim or a Lean↔Rust correspondence theorem.

### The propositions are derived, not attested (issue #65)

The submission states the retained relation **once** — ordered exact `I64`/`Bool`
parameter, input and output kinds, a finite declared domain of admissible
assignments, a finite list of permitted output stacks, and `pre`/`post` predicate
terms in a bounded pure-`I64`/`Bool` language — and
`crates/noble-cli/src/core/spec_quality/relation.rs` recomputes inhabitation,
realizability and determinacy from it by enumerating that domain. The submitted
`checked`/`vacuous` flags are then only **compared** with the derivation, so a
manifest cannot attest its own obligations: a disagreement is refused as
`self-attested-obligation`, including a specification-quality claim submitted
without any relation.

Every refusal here fails closed. An index outside the declared stacks, a
mixed-kind comparison, or a predicate that is undefined for an assignment in the
declared domain is `undefined-relation` rather than false; a kind, op, domain
size or permitted-output count outside the bounded profile is
`unsupported-relation`; and `add` is wrapping `I64` arithmetic, so the boundary
case is decided rather than refused.

The selected-CLI lane exercises both sides of the declared bounds: a complete
4,096-byte manifest is accepted, while 4,097 bytes produces `exhausted` (exit
`5`); eight term levels are accepted, while nine produces
`unsupported-relation` (exit `2`). The final assignment of a 64-entry domain
still exposes ambiguity with witness indices `(63,0,1)`; 65 entries are refused.
Undefined preconditions and postconditions produce `undefined-relation`,
including an undefined term behind a false conjunction or true disjunction.
Neither short-circuiting nor an exceeded bound can supply evidence.

A refused proposition is reported with its **measured refutation** — the index of
the admissible assignment that has no permitted output, or the assignment and the
two distinct satisfying outputs — never with "no witness was found". The lane
measures the acceptance criteria end to end against the selected CLI: the exact
wrapping increment across the `I64` boundary derives inhabited, realizable and
deterministic; a contradictory postcondition derives non-realizability with its
admissible assignment; one admissible input with two distinct permitted `Bool`
outputs derives non-determinacy with both outputs named; an uninhabited domain
derives vacuity while its universal proposition holds; and an intentionally
relational report is accepted without a determinacy obligation. This last case
does **not** itself exercise ordinary program admission.

Criterion 5 separately invokes `noble verify` on ordinary `.noble-contract`
source: no inputs, one `Bool` output, and `requires true` / `ensures true`.
Both `[ true ]` and `[ false ]` implementations have
`ordinary_typing.outcome: accepted`, without a determinacy obligation or a
specification-quality artifact. Missing implementation evidence remains
`unknown` (exit `3`) with `independent_recheck: false`; ordinary admission does
not manufacture a proof. Replacing the Bool implementation with `[ 0 ]` is
`not-accepted` / `error` (exit `2`): a permissive postcondition cannot bypass
ordinary typing.

Measured by `crates/noble-cli/tests/spec_quality_report.mjs` (one Rust test
target launches it against the selected CLI): the refusal classes above,
`self-attested-obligation`, `undefined-relation`, `unsupported-relation` and `exhausted`,
the three derived propositions, and the four separated obligations.

Every case also compares the submitted manifest bytes before and after analysis.
At `88a91af8`, a separate full-syscall trace of the 42 `spec-quality` invocations
observed only read-only file opens and report writes to inherited stdout: no
child processes, additional executable images or shared writable mappings.
A positive control writing a
scratch file and launching another executable was detected by the same trace
checks. Together with the scalar-only report dispatch, this is finite evidence
for analysis without guest execution, source mutation or automatic contract
strengthening, not a universal noninterference proof. The tracer is an external
smoke check, not a new test-suite dependency.

The issue #70 regression matrix also checks duplicate and reordered outputs:
`[true,true,false]` and `[true,false,true]` refute determinacy with original
permitted indices `(0,2)` and `(0,1)`, while `[true,true,true]` stays deterministic.
Postcondition filtering can remove the distinct output; when it instead removes
a prefix, the witness still names original list indices `(1,3)`, not positions
in the filtered list. A mixed `I64`/`Bool` stack differing only in its second cell
is distinct. A distinct sixteenth output is examined; seventeen permitted outputs
and a wrong-kind output are refused. All seven accepted regression reports are
inhabited, realizable and non-vacuous. For the duplicate-first ambiguity, `checked: true` is refused as
`self-attested-obligation`, while report submission without a determinacy
obligation remains accepted. This is finite production-CLI evidence, not a
Lean↔Rust correspondence theorem.

The Lean side of the same discipline is in `proofs/mc1/NobleContracts/SpecQuality.lean`:
a `Submission` records the kind, observation relation, parameter binding and
assignment it *claims*, and `Accepts` recomputes all of them from the retained
`Spec` and the raw stacks, so the artifact an accepted submission yields is the
derived witness rather than the submitted claim. The mc1 contract-library gate now
pins five of those refusals under the strict axiom list — a matching increment
submission really rechecks, while a wrong obligation, a wrong observation relation
(`[.bool]` claimed for an `I64`-output relation), a wrong parameter binding, and a
merely asserted satisfaction of a contradictory postcondition all yield no
artifact. Scalar types are compared by explicit traversal rather than a derived
`DecidableEq`, so a silently changed instance cannot make two observation
relations look equal.

Open, and deliberately not closed:

- the producer derives from the relation the submission states; it does not read
  the relation out of a retained contract renderer yet, so a submission is still
  responsible for transcribing it faithfully, and no correspondence claim relates
  the two;
- no correspondence claim relates these relations to the retained contracts or
  to `crates/noble-contracts/src/rendering`: the model fixes the obligation
  shapes, not the renderer, so this is model evidence rather than an
  implementation theorem;
- no obligation is generated from a retained contract yet, no witness is
  replayed during shrinking, and no solver proposal, timeout, `UNKNOWN`,
  unsupported syntax or exhaustion lane is modelled: those are Phase 2 work in
  issue #65;
- determinacy is stated with exact scalar equality only; any observational
  quotient has to be selected, versioned and checked separately;
- relational realizability says nothing about termination, traps, quotas or
  success, and determinacy says nothing about intended meaning. An observed
  relational counterexample is not an implementation failure without separate
  execution evidence.

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

## Bounded canonical ServiceRef codec (issue #48)

`NobleFoundation/ServiceWire.lean` specifies the one admitted Preserves text
record of the dataspace codec, `<service "NAME" #t>` / `<service "NAME" #f>`,
with the 64-byte wire budget, the `1..=4` configured depth and the 1..=32-byte
name alphabet. Bytes are modelled as `List Nat`; the parser matches the record
shape by literal byte patterns and requires the closing suffix to consume the
rest of the input, so full-input consumption and canonicality are structural
rather than checked postconditions.

Proved: value round trip (`decode_encode`) and accepted-byte canonicality
(`encode_of_accepted`); the exact name and wire bounds
(`longest_name_fits_wire_budget`, `overlong_name_is_rejected`); and the decision
precedence — byte budget before the configured depth
(`byte_limit_precedes_depth_scan`), the configured depth before the schema match
(`depth_configuration_precedes_schema`) and the depth scan before it
(`depth_scan_precedes_schema`) — with the readiness tag, trailing bytes, the
empty name and foreign name bytes separated as semantic classes rather than as
diagnostic wording.

The depth scan is **balanced**, like the codec's: `depthScanAux` tracks the
current depth and the largest depth reached, and a closing byte at depth zero
leaves the current depth at zero, so stray openers and separated runs are schema
failures rather than depth failures
(`unbalanced_closers_do_not_count_toward_the_depth`). The earlier scan counted
opening brackets, which was stricter than the codec; the finite comparison below
surfaced that disagreement, and the model was corrected rather than the finding
being recorded and dropped.

The independent comparison is `verification/service-ref/cases.mjs` — a decoder
written from this model rather than from the Rust codec — against the real
`crates/noble-kernel/src/dataspace/wire.rs` functions in the
`service_ref_differential` lanes of `crates/noble-kernel/tests/m7-dataspace.rs`:
271 cases over alphabet edges, name lengths 0/1/32/33, wire lengths 64/65,
configured depths 0/1/4/5, truncation at every offset, appended bytes, one-byte
substitutions and seeded generated values, with three `control-` cases that a
counting-only depth scan would decide differently. The lane agrees on every case.

Nineteen inventoried theorems; receipt
[verification/foundation/receipts/134b215c5/](../../verification/foundation/receipts/134b215c5/foundation-check.json)
(454 theorems, `clean_source: true`).

Open, and deliberately not closed: no extraction or refinement proof relates
`wire.rs` to the model, so the differential is finite evidence on one host and
target rather than a correspondence theorem; the canonicality theorems apply to
this closed schema only, not to JSON documents in general; and no zero-copy,
whole-parser or whole-compiler claim follows.

## Bounded independent-row batch reference (issue #41)

`NobleFoundation/BatchRows.lean` is the *model* end of issue #41's batch entry
point, not an implementation. A row is one wrapping I64 operation over two
explicit operands; `rowOps` builds the row's straight-line fragment, `rowRun` is
one independent single-row run over the existing reference machine (`AOp.ref`,
whose only arithmetic is the comparator oracle's `MathlibI64.wrap`), and
`evalBatch` is the ordered map of those independent runs. It is deliberately not
the optimized Rust loop under another name: it is the same independent machine
the single-row theorems use.

Proved, all over finite input lists:

- the batch **is** the ordered map of independent row semantics
  (`evalBatch_is_the_ordered_map`), and appending batches appends results, so
  partial progress is the ordered map over the prefix (`evalBatch_append`);
- a refused row stays local and cannot change any other row's result
  (`a_refused_row_does_not_affect_later_rows`), and each row's run starts from an
  empty stack, so rows cannot supply or consume each other's operands
  (`a_rows_run_starts_from_an_empty_stack`);
- a validated row's reference result is the wrapping operation on its operands
  (`add_row_result_is_the_wrapping_sum`,
  `sub_row_result_is_the_wrapping_difference`,
  `mul_row_result_is_the_wrapping_product`), and an out-of-range literal row is
  refused at its own position rather than silently coerced
  (`out_of_range_row_is_refused`);
- each batch row's lowered fragment executes exactly as that row's reference run
  (`batch_row_executes_as_its_reference_run`), by reusing the sequence-lifting
  theorem `exec_lowerAll` rather than restating it.

Nine inventoried theorems; receipt
[verification/foundation/receipts/781c2f490/](../../verification/foundation/receipts/781c2f490/foundation-check.json)
(452 theorems, `clean_source: true`).

**Declared cost of staging.** Beside the value theorem, `Cost`, `rowMajorCost` and
`chunkedCost` declare node evaluations and staging/output writes separately, and
`chunksOf`/`chunksOfGo` are the chunked staging of the same rows. Proved:
row-major staging costs exactly one evaluation and one write per row
(`row_major_cost_is_exact`); a chunked pass partitions without reordering or
dropping rows (`chunksOfGo_flatten`) and therefore evaluates exactly the
ordered map of independent row runs (`chunking_does_not_change_the_value`), so
chunking is a staging choice and not a value change; node work is
staging-independent (`node_work_is_staging_independent`); the chunked writes
are exactly the chunk count (`chunked_writes_are_one_per_chunk`); every
committed chunk holds at most one chunk of rows
(`every_chunk_is_bounded_by_the_chunk_size`) and a pass therefore bounds the
live row slots by the chunk size (`live_row_slots_are_bounded_by_the_chunk`);
and a zero chunk size degenerates to one row per chunk instead of dropping rows
(`a_zero_chunk_drops_no_row`).

Nothing in that model measures or predicts elapsed time, process memory or
allocator overhead, and no abstract counter is equated with a measured one.

Receipt for the cost model:
[verification/foundation/receipts/f91cc50f7/](../../verification/foundation/receipts/f91cc50f7/foundation-check.json)
(471 theorems, `clean_source: true`, axiom sets inside the allow-list).

Open, and deliberately not closed by this model: no byte budget beyond the
literal-range rule, no validation rule beyond the literal range is
modelled, no parameter snapshot or output ownership is modelled, and no
implementation correspondence is claimed — a matching Rust batch loop would be
tested-implementation evidence, not a theorem. Performance is untouched: nothing
here measures or predicts throughput, first-result latency or allocation.

## Bounded M6 task-interleaving model (issue #46)

`NobleFoundation/TaskInterleaving.lean` is a two-record subject for a
schedule explorer rather than a scheduler: it names which interleavings are
reachable and what holds at every reachable prefix, so a full-search baseline
exists before any reduction is claimed. Records carry a generation, a pin and
wakeup-reservation units; the table carries a shared admission capacity, a
sticky first abnormal failure, and ordered delivery and settlement records.
Actions are `acquire`, `cancel`, `complete`, `deliver`, `staleCallback`,
`settle`, `cleanup` and `finish`; native stop is not among them because it is
an input observation, not something cancellation proves. Every refusal is a
single guard, an action naming a generation must name the live one, and
same-slot reuse needs a different generation.

Proved, and the enumeration baseline with it: the empty table satisfies the
invariants (`empty_table_satisfies_the_invariants`), every acquired step
preserves them (`step_preserves_invariants`) and therefore every reachable
prefix of any action list does (`reachable_prefixes_satisfy_the_invariants`);
a full table refuses the other id's admission
(`baseline_admission_is_refused`) until finishing a live record releases the
slot (`finishing_a_slot_releases_the_capacity`), after which that other id's
admission is enabled (`releasing_a_slot_enables_another_id`); a stale-generation
callback leaves the table untouched (`stale_generation_callback_changes_nothing`);
delivery after retirement (`late_delivery_after_retirement_is_refused`), a
second delivery of the same generation (`double_delivery_is_refused`) and
settlement before delivery (`settlement_requires_a_delivery`) are refused
rather than recorded; settlement after delivery appends the ordered pair
(`settlement_after_delivery_is_taken`), finishing keeps the delivery record
(`finish_keeps_its_delivery_record`) and cancellation keeps the pin held
(`cancellation_keeps_the_pin`); a recorded abnormal failure survives later
failures, later successes and refused or acquired actions
(`abnormal_failure_survives_a_second_failure`,
`abnormal_failure_survives_a_later_success`,
`abnormal_failure_survives_refused_and_acquired_actions`) and two failures keep
the first code (`first_failure_code_is_kept_as_the_sticky_one`). Twenty-three
theorems, all inventoried, plus the three guard-reading lemmas
(`some_or_none_eq_some`, `none_or_some_eq_some`, `some_or_some_eq_some`).
Receipt
[verification/foundation/receipts/cc147dd40/](../../verification/foundation/receipts/cc147dd40/foundation-check.json)
(428 theorems, `clean_source: true`, axiom sets inside the allow-list).

The observation contract is `observe` — per-id generations, pins and
reservations, the sticky failure, and the ordered delivery and settlement
records. `lossyObserve` deliberately drops the failure and the generations, and
is measured as lossy instead of being used as an equivalence test:
`lossy_observation_hides_the_failure_difference` and
`ordered_delivery_prefixes_are_distinguished` present histories with the same
lossy observation but different documented ones, the second pair differing only
in delivery order.

The deliberately unsound independence rule is refuted, not assumed:
`different_ids_are_not_independent` shows two actions on *different* task ids do
not commute, because the second one's enabledness depends on the first one's
released slot (acquire-then-finish refuses the admission, finish-then-acquire takes
it). Cancellation/delivery races, generation reuse and shared-capacity
conflicts therefore stay dependent.

The independence rules are then **proved or refuted separately at the declared
bound**, so a reduction is only ever proposed for a swap the model supports:

- slot-local actions on the distinct ids **commute**: `cancel_orders_agree_on_distinct_ids`
  and `finish_orders_agree_on_distinct_ids` reach the same documented
  observation in either order, so the swap preserves enabledness, the related
  state and the observation for those actions;
- delivery is **not** independent: `delivery_orders_disagree_on_distinct_ids`
  differs because the documented observation keeps one ordered delivery record,
  while `delivery_orders_agree_under_the_lossy_observation` shows the same two
  orders agreeing under the lossy projection — so an equivalence read from that
  projection would hide a dependence the documented observation retains;
- the shared slot, not the id, decides enabledness:
  `admission_depends_on_which_id_holds_the_shared_slot` reaches two different
  documented observations from one released slot, and
  `composed_admissions_agree_while_the_prefixes_differ` records that both
  *composed* admission orders end refused, so an equality read from the
  composition would hide the reachable-prefix dependence rather than prove
  independence;
- the sticky first failure is shared: `failure_orders_disagree_on_distinct_ids`
  differs across the two failing-completion orders on the two-generation
  `pairedGenerations` table;
- `stale_callback_is_observation_free` states the one action that is
  observation-free at the declared bound.

Two first drafts of the admission and failure witnesses were *refuted* by the
kernel checker because both of their first steps were refused; the statements
were corrected to reachable prefixes and two live generations rather than
weakened into tautologies. Eight further inventoried theorems, receipt
[verification/foundation/receipts/729e5d199/](../../verification/foundation/receipts/729e5d199/foundation-check.json)
(443 theorems, `clean_source: true`).

**The bounded explorer.** `exploreAlphabet`, `sequencesOfLength` and `sequencesUpTo`
populate a baseline observation set from the declared alphabet up to depth three
(`baselineObservations`, `declaredBaseline`), refused steps contributing no
observation rather than a silent state. The enumeration is proved complete for
that bound: it contains every sequence of the declared length over the declared
alphabet (`mem_sequencesOfLength`), a deeper enumeration contains every shorter
one (`sequencesUpTo_contains_shorter`), and therefore every sequence within the
declared depth appears in it (`the_baseline_enumeration_is_complete`) — so a
reduction may not claim agreement against a smaller search than this one.

Full/reduced agreement is then *measured* on the declared pairs: the independent
pair's swap preserves the documented observation in both orders
(`the_independent_pairs_swap_preserves_the_observation`), while the delivery pair
changes it (`the_reduction_must_exclude_the_delivery_pair`), so a reduction that
kept the delivery swap would be unsound rather than merely unproven.

Receipt for the explorer and the witness scan:
[verification/foundation/receipts/e95adf748/](../../verification/foundation/receipts/e95adf748/foundation-check.json)
(479 theorems, `clean_source: true`, axiom sets inside the allow-list).

**Reproducible witness shrinking.** `disagrees` states when a sequence's reachable
observation differs from a reference (or when it is refused where the reference
is reachable), and `scanFrom` walks prefix lengths *upward*, so the first
disagreeing length it finds is the shortest one. Proved: when the scan reports
no disagreement every prefix in its window agrees (`scanFrom_none_all_agree`),
when it reports one that length is the first in its window and every shorter
prefix agrees (`scanFrom_some_finds_the_first`), and therefore a disagreeing
sequence has a shortest prefix witness with every shorter prefix recorded as
agreeing (`every_disagreement_has_a_shortest_witness`) — so a shrunk witness is
replayed from the sequence's own prefix rather than hand-picked.

Open, and deliberately not closed:

- the scan is bounded by one declared sequence and the declared alphabet rather
  than exhaustive over the language, and it shrinks prefixes only (no action
  deletion or operand rewriting);
- this is a bounded subject, not a scheduler and not a new source-level async
  model, and no payload/outcome class outside the listed actions is enumerated;
- no correspondence claim relates the model to the Rust table API in
  `crates/noble-kernel/src/async_tasks/table`; the invariants are model claims;
- the universal reduction theorem is not proved and no reduced search exists:
  the independence fragment above is bounded evidence at the declared bound,
  with concrete witnesses rather than a theorem over every table and action
  list, so it never enables a reduction by itself;
  weak-memory ordering, liveness, fairness, callback-delivery guarantees,
  cleanup after host death and once-only remote effects are outside the model;
- `abnormal_failure_*` are concrete reachable witnesses rather than a universal
  sticky-failure theorem.

## Status by phase

| Phase | Done (checked) | Open |
|---|---|---|
| 1 | whitelist/representation/observation table; signed ZMod/BitVec/K-NUM-01 correspondence; universal add/sub/mul with preserved typed prefix and orientation; literal refusal vs wrapping (spec and transcription); lifting to sequences and quote/compose/run; regressions; inventory and axiom audit | Rust frontend ↔ transcription refinement; MC1 `CodeTyped` ↔ kernel acceptance |
| 3 | one example from Lean spec to acquired bytes (opt off/on), helper model, ACORR-mapped refusals | E5 lowering, E7 assembler, E8 optimizer, E10 loader/engine, E12 PAA-1 envelope, ACORR-CASE-09, quote/compose/run helper models |
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
Receipt [`verification/foundation/receipts/b3655a9cd/`](../../verification/foundation/receipts/b3655a9cd/foundation-check.json)
(435 theorems, `clean_source: true`).
