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

### CI

The CI recipe is [`verification/foundation/ci.sh`](../../verification/foundation/ci.sh)
`NEW_DIR [--skip-slice]`, run on a clean checkout after `lake exe cache get`.
It runs these stages:
1. `check.mjs`;
2. `slice.mjs`;
3. `differential.mjs --seed 15 --count 80`;
4. `--verify` on the fresh receipt;
5. a stale-evidence control: the retained `receipts/ce2e5ce` receipt must be
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

## Status by phase

| Phase | Done (checked) | Open |
|---|---|---|
| 1 | whitelist/representation/observation table; signed ZMod/BitVec/K-NUM-01 correspondence; universal add/sub/mul with preserved typed prefix and orientation; literal refusal vs wrapping (spec and transcription); lifting to sequences and quote/compose/run; regressions; inventory and axiom audit | Rust frontend ↔ transcription refinement; MC1 `CodeTyped` ↔ kernel acceptance |
| 3 | one example from Lean spec to admitted bytes (opt off/on), helper model, ACORR-mapped refusals | E5 lowering, E7 assembler, E8 optimizer, E10 loader/engine, E12 PAA-1 envelope, ACORR-CASE-09, quote/compose/run helper models |
| 4 | seeded differential comparator, Lean-checked expectations, opt off/on, shrinking, mutation controls, checker lanes, source-bound receipts, `--verify`, retained in-repo receipts, `ci.sh` recipe | Nix-sandbox CI check (see "CI": networked Mathlib cache, absent offline inputs, host services for the slice); rebuilt Mathlib `.olean` files |
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
