# Lean Wasm refinement pilot: subject inventory and semantics survey (issue #82, step 1)

Issue #82's first acceptance item needs one of two things: a pinned Lean Wasm semantics covering the
subject artifact's full instruction and section inventory, or a precise list of what it does not
cover, in which case the pilot stops there. This directory records the inventory and the
candidates. Choosing and pinning a semantics is the owner's decision, because every candidate that
comes close is GPL-3.0 or AGPL-3.0.

## The subject artifact

The subject is `verification/owi/receipts/2026-10-26/subject-module-1.wasm`, from #57, 10 474 bytes,
sha256 `261f11d3…cbe5`.

[`inventory.mjs`](inventory.mjs) checks the bytes against the retained `sha256sums.txt`, reads them
with the pinned `wasm-tools` 1.245.1, and writes [`inventory.json`](inventory.json):

| | |
|---|---|
| Sections | types (20), imports (27), functions (123), globals (4), exports (32), elements (2), data count (1), code (123), data (4), custom `name` |
| Imports | one memory (16 to 16 pages), one `funcref` table (16 384), 23 mutable `i32` globals and 2 mutable `i64` globals |
| Segments | passive element segments used by `table.init`, and passive data segments used by `memory.init`, which needs the data count section |
| Instructions | 3063 in 123 function bodies, 52 distinct mnemonics |

The mnemonics:
- **control:** `block`, `loop`, `if`/`else`/`end`, `br`, `br_if`, `return`, `call`, one `call_indirect`, `unreachable`, `drop`;
- **variables:** `local.get`/`set` and `global.get`/`set`;
- **i32:** arithmetic, bitwise and comparison operations;
- **i64:** `add`, `sub`, `mul`, `or`, `shl`, `shr_u`, `eq`;
- **conversions:** `i32.wrap_i64` and `i64.extend_i32_u`;
- **memory:** `i32`/`i64` loads and stores, including `i32.load8_u`, plus `memory.size`, `memory.grow`, `memory.fill` and `memory.init`;
- **table:** `table.init`.

**Which representation.** The artifact uses linear memory. Every production path emits through
`noble_wasm::source::Compiler`, which has no `Representation`: `noble run`, `noble session`,
`noble build`, the calculator and `noble editor admit --emit` (`crates/noble-cli/src/core/artifact.rs`).
`Representation::{WasmGc, ManagedLinearMemory}` is selected only by `noble wasm-experiment`
(`crates/noble-cli/src/backend.rs`). A semantics chosen for this inventory says nothing about
`WasmGc` modules. Those use Release 3.0 GC types and are not produced by production emission.

## Lean 4 candidates, read at pinned commits

| | Talos | T-Brick/lean-wasm | argumentcomputer/Wasm.lean | aionescu/lean-wasm (Utrecht MSc) |
|---|---|---|---|---|
| Commit | [`d7c84ac6`](https://github.com/cajal-technologies/talos/tree/d7c84ac660757953b752bdfc9cb8dae3dab623ed) (2026-11-02) | [`c4606a39`](https://github.com/T-Brick/lean-wasm/tree/c4606a39059256ae1281971cd24180f3e3abef80) (2026-11-02) | [`454172e1`](https://github.com/argumentcomputer/Wasm.lean/tree/454172e1219ce648732211a7804829809dffd64d) (2023-12-13) | [`66f1166f`](https://github.com/aionescu/lean-wasm/tree/66f1166f84a763b23475b0eb660153c72ec54d29) (2024-07-11) |
| License | AGPL-3.0 | GPL-3.0 | MIT | GPL-3.0 |
| Lean | 4.35.0, Mathlib v4.35.0 | 4.35.0, Mathlib v4.35.0 | `nightly-2023-01-10` | unpinned `stable` |
| Reads `.wasm` bytes in Lean | no: it runs the external `wasm-tools print` and parses the text | yes, including the bulk-memory opcodes and the data count section | no (encoder only) | no |
| Semantics | small-step relation and an executable interpreter, with soundness and completeness theorems between them | relational, with memory and most control flow left as `todo` | executable `partial def`, opaque to proofs | relation and evaluator for a small typed fragment |
| Covers the inventory | every mnemonic, bulk memory, imports and passive segments | no: no semantics for control flow, calls, `call_indirect`, loads, stores or `memory.*` | no: no memory or tables | no: no `if`, calls, memory operations or imports |

**Talos comes closest. Its gaps for this pilot:**
1. Lean never reads the bytes. The `.wasm` is decoded by an external `wasm-tools print`, whose version
   Talos does not pin, so the data count and `name` sections are never seen by Lean code. A digest-bound
   theorem about these exact bytes would have to trust that decoder or add a Lean binary decoder.
2. Linking of the imported memory, table and 25 globals is IO harness code, not a pure
   instantiation function. Its `runner` refuses modules with imports, so a proof would build the
   initial store by hand.
3. Its conformance evidence is a self-reported test-suite report, which skips the
   `assert_unlinkable` import-linking cases. That report was not reproduced here.
4. Its default `decode` lowers unknown mnemonics to `unreachable`. `decodeForVerification` refuses
   them, so a pilot would use that entry point.
5. Its README marks the project work in progress, with APIs that may change.

The Talos facts this decision rests on were checked at `d7c84ac6`:
- its toolchain is Lean 4.35.0, and it is AGPL-3.0;
- it decodes by spawning `wasm-tools print` (`interpreter/Interpreter/Runner.lean`);
- its `SmallStep.lean` has `memory.init` rules, including the dropped-segment trap;
- that file states `stepChecked?_sound` and `stepChecked?_complete` and contains no `sorry`.

T-Brick's step relation does stop at `-- todo memory` (`Wasm/Dynamics/Dynamics.lean`). The other
coverage cells come from a read of each repository's source at the listed commits, not from running
anything. WebAssembly's own SpecTec tree has no Lean backend.

## What is left to the owner

- **Whether to adopt Talos at `d7c84ac6` under AGPL-3.0.** The alternative is a Lean binary decoder
  of Noble's own, joined to a Talos-style semantics. A third option is to stop the pilot with the
  gap list above. Each option changes what "the semantics' fidelity" trusts.
- **Whether the decoder may stay external.** If it may, the trust statement must name the external
  `wasm-tools print`, and pin it.

## Non-claims

No semantics is pinned or vendored, no Lean code from any candidate is reused, and nothing was built
or run against a candidate. The inventory is of one artifact. It does not cover other emitted
modules, `WasmGc` modules, or the runtime imports' behaviour.
