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
env -i "$NODE" verification/foundation/check.mjs --output "$HOME/.cache/noble-foundation-$(date +%s)"
```

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
