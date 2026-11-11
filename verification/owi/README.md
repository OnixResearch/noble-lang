# Owi downstream Wasm bug-finding lane (issue #57, pilot)

Evaluation record for a **bounded pilot**, not a proof route and not a
replacement for the production engine. Owi is not a Lean proof authority, and
nothing here claims source-to-Wasm correctness.

## Decision

**No-go on the pinned Owi build.** The pinned Owi cannot decode the instructions
that Noble's real emitted modules actually contain, so the lane cannot start
without an upstream fix. This is not a Noble defect: the same artifact passes
`wasm-tools validate`, and the failure is in Owi's binary decoder before
validation, imports or wrapping are reached.

Per issue #57, a reproducible compatibility blocker that documents why the lane
cannot start is a valid outcome. No counterexample was found, no wrapper was
built and no `certified`/`replayed counterexample` status is claimed.

## Update (2026-11-09): the decoder is fixed in a fork

The blocker is a four-line decoder defect, so it is carried rather than waited on.

- **Fork.** [`OnixResearch/owi`](https://github.com/OnixResearch/owi), branch
  `noble/memory-init-decode`, commit `44eadcdc`: `memory.init` is `0xFC 8 dataidx memidx`, and the
  decoder read a third, zero byte, which consumed the next instruction's opcode. Wasm 2.0 spells
  memidx as the single byte `0x00`; 3.0 permits other indices, so dropping the extra read is correct
  for both.
- **Patch over the pin.** [`nix/owi-memory-init-decode.patch`](../../nix/owi-memory-init-decode.patch)
  is the same change, applied to the pinned nixpkgs source with `overrideAttrs`, as the Aeneas and
  Octet patches are. `verification/owi/patched.mjs` records the comparison on the committed
  artifacts.
- **Observed.** Unpatched, `owi validate` refuses the reproducer and the real emitted subject with
  `data count section required` (exit 30) and accepts the control. Patched, all three validate with
  exit 0 and no diagnostic.
- **What is still open.** `owi sym` now reaches module instantiation and stops at
  `unknown module noble`: the lane needs a bounded host-module wrapper for `noble`'s memory, table
  and globals, plus the entry point and quota bridge this record already lists. That is Noble-side
  work, no longer an upstream block. The no-go above still describes what was executed before the
  fix: nothing was run symbolically then, and nothing is claimed now beyond validation.

## Pinned toolchain and subject

| Item | Value |
|---|---|
| Noble source | `411cda75` (this commit's parent tree for the emitted artifact; the emitter and module are unchanged by this record) |
| Emitter invocation | `noble editor admit subject-ast.json --emit NEW_DIR` |
| Subject AST | `receipts/2026-10-26/subject-ast.json` (`7 3 * 5 +`, `{"format":1,…}` integer/word nodes) |
| Emitted module | `receipts/2026-10-26/subject-module-1.wasm`, 10474 bytes |
| `wasm-tools` | 1.245.1 (pinned by `policy/tool-selection.json`) |
| `binaryen`/`wasm-opt` | 125 (pinned by `policy/tool-selection.json`) |
| Owi | 0.2-unstable-2026-01-29 from the flake's pinned nixpkgs (`ocaml5.3.0-owi`) |
| Emit outcome | `noble-core-report/v1` `stage:"wasm"`, `outcome:"normal"`, stack `I64 26` |

Exact bytes are bound by `receipts/2026-10-26/sha256sums.txt`.

## Actual emitted module inventory (not inferred from the paper)

From `wasm-tools objdump` and `wasm-tools print` on the emitted bytes:

- sections: types(20), imports(27), functions(123), globals(4), exports(32),
  elements(2), **data count**, code(123), data(4), custom `"name"`;
- **27 imports from module `"noble"`**: `memory` (16..16 pages), `table`
  (16384..16384 `funcref`), and 25 mutable globals — 23 `i32`
  (`failure`, `quota_reason`, `phase`, `heap_cursor`, `heap_baseline`,
  `allocation_limit`, `recipe_limit`, `depth_limit`, `operand_limit`,
  `continuation_limit`, `step_limit`, `allocation_peak`, `operand_peak`,
  `continuation_peak`, `steps`, `quote_invocations`, `sp`, `cp`,
  `observed_program`, `recipe_count`, `reflection_steps`, `rp`, `generation`)
  and 2 `i64` (`allocated_total`, `released_total`);
- 32 exports: `memory`, `metric`, `reflect`, `recipe_length`, `recipe_kind`,
  `recipe_value`, `begin`, `push_i64`, `push_bool`, `push_unit`, … (full list in
  `receipts/2026-10-26/subject-imports-exports.txt`);
- no `start` section and no `_start`/`main` export;
- code uses bulk-memory instructions at the recorded offsets:
  `table.init $core`, `table.init $code`, `memory.init $d0`…`$d3`
  (`receipts/2026-10-26/subject-bulk-memory-ops.txt`).

So a pure scalar source fixture is **not** a standalone Wasm function with no
runtime state: it needs the host's memory, table and 25 ABI globals, and its
entry convention is Noble's own export set. Any faithful wrapper would have to
supply all of that.

## Reproducible blocker

| Command | Result |
|---|---|
| `wasm-tools validate subject-module-1.wasm` | **accepts** (exit 0) |
| `owi validate subject-module-1.wasm` | `owi: [ERROR] data count section required (got \016 instead)`, exit 30 |
| `owi validate control-passive-data-only.wasm` | accepts (exit 0) |
| `owi validate reproducer-memory-init.wasm` | `owi: [ERROR] data count section required (got B instead)`, exit 30 |

The control and reproducer isolate the cause to the **bulk-memory instruction
decoding**, not to passive segments, the data-count section or the imports:

- `control-passive-data-only.wat` has a passive data segment, the data-count
  section and a `"noble"` memory import, and Owi decodes it;
- `reproducer-memory-init.wat` is 40 bytes of module and differs only by using
  `memory.init`; Owi's decoder refuses it with the same message class;
- a `memory.fill` module with no passive data decodes (Owi's validator then
  reports an ordinary type mismatch), so the prefixed-opcode family is the
  differentiator rather than the `0xFC` prefix as such;
- moving the data-count section after the code section does not change the
  outcome, so this is not a section-ordering mismatch on Noble's side.

Consequence: building a wrapper cannot unblock the lane, because the refusal
happens before imports, instantiation or entry-point selection. A future lane
needs an Owi build whose decoder accepts `memory.init`/`table.init`/`data.drop`/
`elem.drop` (or an emitter variant whose code avoids bulk-memory instructions,
which would be a different artifact and is not substituted here).

## What this record does not claim

- No wrong-subtraction/constant/output-order witness and no replayed
  counterexample: no artifact was executed symbolically at all.
- No wrapper, ABI initial state or quota-global bridge was built, because the
  decoder refusal precedes them; nothing is stubbed, and no semantic dependency
  was replaced by a handwritten surrogate.
- No equality/`Bool` fixture, no shrinking, no witness/receipt format from #56,
  no solver result classification (certified/replayed counterexample/
  inconclusive/unsupported), no cost measurements (wall time, solver time, peak
  memory, explored paths) because no run happened.
- Wasm behaviour could not be observed, so no source-level undefined-behaviour,
  frontend, emitter, optimizer, engine or loader claim is made either way.
- The module digest binds identity, not semantics.

## Replay

```
noble editor admit verification/owi/receipts/2026-10-26/subject-ast.json --emit /tmp/owi-emit
wasm-tools validate /tmp/owi-emit/engine/module-1.wasm     # exit 0
owi validate /tmp/owi-emit/engine/module-1.wasm            # exit 30, decoder refusal
```

`noble` requires the pinned `wasm-tools 1.245.1` and `binaryen 125` binaries from
`policy/tool-selection.json` to be present and realpath-identical; missing or
incompatible tools fail closed with `incompatible wasm_tools`, as intended.

The retained run is
[`receipts/2026-11-09/patched.json`](receipts/2026-11-09/patched.json), from the clean commit
`5f883144`: unpatched `30/0/30`, patched `0/0/0` for reproducer, control and subject.

## The `noble` host module

`host/noble-host.wat` is the host module a live subject imports. The committed subject imports one memory (`16 16`), one table (`16384 16384 funcref`) and twenty-five mutable globals from `noble` — twenty-three `i32` counters and pointers, two `i64` byte totals — and no host function, so a pure-data module satisfies it.

`host.mjs` keeps the stub tied to the subject: it validates the stub, parses the subject's imports from the committed dump, and requires the exports to match by name, kind, memory and table limits, and global types. A control removes one global and requires a refusal. The run is retained as [`receipts/2026-11-10/host.json`](receipts/2026-11-10/host.json), from the clean commit `36fe625a`: 27 imports matched, 23 `i32` and 2 `i64` globals, and the control refused.

A faithful host module is not a faithful runtime: every global starts at zero and there is no host function to call. Whether the engine can link the stub with the subject for `owi sym` is the next step, and it is engine-side, not an interface question.
