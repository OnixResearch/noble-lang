# Lean Wasm refinement pilot: inventory, evidence, and decision (issue #82)

Issue #82's first acceptance item needs one of two things: a pinned Lean Wasm semantics covering the
subject artifact's full instruction and section inventory, or a precise list of what it does not
cover, in which case the pilot stops there. This directory records the inventory and candidate
survey. The decision is to adopt Talos at `d7c84ac660757953b752bdfc9cb8dae3dab623ed`, under its
AGPL-3.0 license, as an isolated verification-only Lake project in `lean/`. It will not be placed
under `proofs/` or imported by Noble foundation proofs. The decoder remains external: pinned
`wasm-tools print` output is re-encoded and byte-compared before use. The pilot stops if
`decodeForVerification` refuses any construct in the subject.

### Decision evidence

1. **Toolchain and dependencies.** Talos's `interpreter/lake-manifest.json` and Noble foundation's
`lake-manifest.json` use Lean 4.35.0 and the same Mathlib revision
`56a005855ea17e0b0958f73ec801ae140b0a1679`. All eight transitive package revisions also match:
`plausible`, `LeanSearchClient`, `importGraph`, `proofwidgets`, `aesop`, `Qq`, `batteries` and
`Cli`. This permits reuse of the existing prebuilt packages by reflink-copying the matching package
directories; no Mathlib download or rebuild is allowed.
2. **License precedent.** Noble already uses an AGPL-3.0-or-later dependency in the standalone,
verification-only Woxi helper, which is not linked into a production crate
(`verification/calculator-v1/woxi/README.md`). Talos is likewise isolated from production crates.
3. **Host imports.** The subject has no imported functions: its 27 imports are one memory, one
`funcref` table, 23 mutable `i32` globals and two mutable `i64` globals
(`verification/owi/receipts/2026-10-26/subject-imports-exports.txt`). The runtime import behavior is
not claimed; the proof's explicit initial ABI store remains a trusted premise.
4. **Decoder pin.** `wasm-tools` is pinned by `policy/tool-selection.json`; the existing inventory
script resolves that pinned binary and records version 1.245.1. The same selected decoder will be
used by the byte-bound decode harness.

The source tree is pinned by the commit above. No Talos source and no Mathlib package is stored in
this repository.

**Fetched-source notice.** [`prepare-talos.mjs`](prepare-talos.mjs) fetches the Talos interpreter at
exactly `d7c84ac660757953b752bdfc9cb8dae3dab623ed` into a new work directory outside the repository,
verifies that revision, applies the Noble patches in [`lean/patches/`](lean/patches/), copies the
Noble pilot modules from [`lean/overlay/`](lean/overlay/) into the prepared tree, and copies the
matching cached package directories into it. Every lane script reads that prepared directory. Talos
is distributed under AGPL-3.0; that license applies to the fetched code and to the patches, which
modify it (a dropped-segment `memory.init` source-bounds fix, a decoder extension and the lakefile
targets). The official WebAssembly testsuite files under `lean/interpreter/vendor/testsuite/` are
Apache-2.0 and keep their license there.

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

## Implemented pilot boundary

The selected Talos interpreter sources, its unmodified lock manifest and the matching foundation
package directories are under `lean/`. The project stays separate from all `proofs/` Lake projects;
the foundation does not import it. `prepare-talos.mjs` fetches the pinned Talos commit into a new
work directory outside the repository, applies the retained patches and overlays, checks all package
revisions against the foundation manifest, and copies those package directories with
`cp --reflink=always`. The prepared project therefore uses the exact prebuilt Mathlib oleans without
a package fetch or rebuild; the helper refuses a toolchain or dependency mismatch. It also creates a
workdir-bound runner wrapper for the differential harness. The attempted Lake build of the initial
decoder theorems did not pass: the full-module reductions exhausted 5,000,000 heartbeats after about
1,174 seconds, and a subsequent attempt at greater recursion depth failed at the recursion-depth
limit. Those failed theorem declarations are not retained. The current `Check.lean` performs an
executable decoder check and records source-model lemmas separately; this does not turn decoder
acceptance into a kernel-checked theorem.

The focused library targets `Interpreter.WasmPilot.Check` and
`Interpreter.WasmPilot.Runner` built successfully. `lean --run Interpreter/WasmPilot/Check.lean`
executed the full-module `decodeForVerification` check for both retained WAT strings and printed
`subject decoder check passed`. The complete Lake executable build remains blocked: building the
testsuite and runner executables failed at native linking because the prebuilt Mathlib object graph
has an unresolved `runtime_initialize_mathlib_Mathlib_Data_Nat_WithBot` reference from
`SmallDegree.c.o`. Rebuilding Mathlib is prohibited for this pilot, so this is not claimed as a
successful full project build. The direct Lean check avoids that native-link step.

**No-Mathlib rebuild caveat (2026-11-13).** The final Check/Runner-only target completed 17 jobs without building Mathlib; its clean-source evidence is in `receipts/2026-11-13/focused-build-f3547329.json`. A separate broader attempt to build `Semantics`, `Wp.Atomic`, and `Examples.SmallStep` exited successfully but built Mathlib modules, contrary to this pilot's no-rebuild requirement. Its generated work directory was removed and that run is not compliant verification evidence. The older `post-fix/focused-build.json` records the broader 3,158-job build and must not be read as proof that Mathlib was reused. The 1,918-command spec-test receipt remains an observed pass, but it was produced in that broader-build context; no further Mathlib-dependent build or spec-test rerun was attempted.

`decode.mjs` uses the pinned `wasm-tools` binary to print the retained subject, checks its digest
against the #57 receipt, emits `Subject.lean` into the prepared project with both the binary and
printed-text digests, and requires `wasm-tools parse` to reproduce the exact original bytes. The
separate `Check.lean` executable evaluates `decodeForVerification` on the complete
digest-bound subject and new emitted `x y *` module. It prints the axioms of the source-model lemmas,
the decoder declaration, and Talos's checked-step soundness/completeness theorems. The reported
source-model lemmas use only the permitted axiom set; they do not connect the decoded modules to
`NobleContracts.Exec`.

The copied decoder was changed in two areas needed by the actual emitted text: named data-segment
references are resolved for `memory.init` and `data.drop`, and named parameters in type-applied
functions retain their correct local indices. Regression coverage was added for both. The runner's
import check now accepts the reviewed ABI globals by name rather than assuming their declaration
order. `SmallStep.lean` also checks the source-segment range for `memory.init` after `data.drop`:
with an empty segment and zero length, source offset zero is valid, but a nonzero source offset
traps. The big-step `Semantics.lean` and `Wp/Atomic.lean` rules use the same bound, and
`Examples/SmallStep.lean` adds kernel-checked memory32 and memory64 regressions. This addresses the
pinned `memory_init.wast:240` assertion; the clean focused rebuild passes and the four-fixture
official rerun passes all 1,918 commands. The
separate `NobleContracts.Model` library points at the repository's MC1 source to state source-side
execution lemmas; it imports no foundation module.

## Unit status and reproduced evidence

| Unit | Status | Evidence and boundary |
|---|---|---|
| U1: semantics choice and facts (a)–(d) | DONE | This README and `decision.md`; Talos is fetched at its pinned commit, remains isolated, and no Mathlib/dependency packages were fetched. |
| U2: project, byte binding, decoder acceptance, axiom/source audit | NOT DONE | Exact byte round trip and executable decoder acceptance pass; the focused Check/Runner libraries build and printed axioms stay within the permitted set. The full Lake executable link fails on the unavailable prebuilt Mathlib native symbol above. The attempted full-module decoder reductions exhausted 5,000,000 heartbeats after about 1,174 seconds; a higher recursion-depth attempt also failed. These runs do not prove kernel acceptance. See the original receipts and post-fix `decode.json`, `lean-check.json`, and `focused-build.json`. |
| U3: differential vectors and official spec tests | DONE | From clean commit `e05cc4afce893711298438296f25acca50b7dc9d`, the official pinned subset passes 4 fixtures / 1,918 commands / 0 failures, and the production-artifact differential passes 19 cases (including all 18 arithmetic/linking vectors). Kernel-checked memory32/memory64 regressions cover the corrected dropped-segment source bound. The differential receipt explicitly limits its linking coverage to the retained Bool-prefix/arithmetic projection, not the full declared-module linker. See `receipts/2026-11-13/post-fix/`. |
| U4: digest-bound module-to-MC1 refinement theorem | NOT DONE | No theorem relates decoded artifact execution to `NobleContracts.Exec`; only separate MC1 source-execution lemmas and Talos checked-step soundness/completeness are present. No artifact trace, trap, or quota/out-of-fuel separation is proved. The decoder reduction failures above are the furthest kernel-checking attempt; the dynamic decoder check is not a substitute. |
| U5: stale-binding and wrong-artifact controls | NOT DONE | The post-fix control receipt proves stale-digest refusal before decoding. The correct-digest `+` artifact returns 5 rather than the `x y *` claim's 6 on both V8 and Talos, and the MC1 source model proves the source results differ. No theorem connects the artifact execution result to either MC1 result, so the required kernel-checked wrong-artifact counterexample remains incomplete pending U4. See `receipts/2026-11-13/post-fix/controls.json` and `differential.json`. |
| U6: trust statement and TB+ decision | DONE | Trust boundary, per-fixture proof status, NO-GO, and reopen conditions are recorded below. No measured proof effort is claimed where no proof completed. |

The initial seven retained receipts were produced from the clean source commit
`589f8dcd6cbe6f9303d9d1751d317a09b0f6a7e1` and are retained in `receipts/2026-11-13/`. The
post-fix receipt set was produced from the clean source commit
`e05cc4afce893711298438296f25acca50b7dc9d` before being copied into
`receipts/2026-11-13/post-fix/`: `decode.json`, `emission.json`, `controls.json`,
`differential.json`, `spec-tests.json`, `lean-check.json`, and `focused-build.json`.
Additional clean-source f354 receipts record the prepared package revisions, byte binding,
production emission, controls, source audit, focused Check/Runner target, and final differential
under `receipts/2026-11-13/*-f3547329.json`.

For reproducibility, first prepare a fresh directory outside the repository with the policy-selected
Node 24 executable:

```sh
<policy-selected Node 24 executable> verification/wasm-refinement-pilot/prepare-talos.mjs \
  --work-dir <new directory outside the repository>
<policy-selected Node 24 executable> verification/wasm-refinement-pilot/decode.mjs \
  --work-dir <prepared directory> --output <new directory outside the repository>
<policy-selected Node 24 executable> verification/wasm-refinement-pilot/emit-mul.mjs \
  --write-subject --work-dir <prepared directory> --noble <absolute path to built noble CLI> \
  --output <new directory outside the repository>
cd <prepared directory>/interpreter
lake build Interpreter.WasmPilot.Check Interpreter.WasmPilot.Runner
lake env lean --run Interpreter/WasmPilot/Check.lean
cd <repository root>
<policy-selected Node 24 executable> verification/wasm-refinement-pilot/spec-tests.mjs \
  --work-dir <prepared directory> --output <new directory outside the repository>
<policy-selected Node 24 executable> verification/wasm-refinement-pilot/controls.mjs \
  --work-dir <prepared directory> --output <new directory outside the repository>
<policy-selected Node 24 executable> verification/wasm-refinement-pilot/differential.mjs \
  --work-dir <prepared directory> --noble <absolute path to built noble CLI> \
  --runner <prepared directory>/pilot-runner --output <new directory outside the repository>
```

Each output directory must be new. `spec-tests.mjs` verifies every retained fixture against
`SOURCE.json`, runs the pinned subset, and writes `spec-tests.json` even when a conformance assertion
fails. The decoder, emission, controls and differential receipts identify both the clean Noble
source commit and the exact Talos source commit/tree used by the prepared workdir.

The testsuite run is an execution check, not a proof of full Wasm conformance. Its retained fixtures
are from WebAssembly/spec commit `f3268fa9dcb7d3a60d09546e743e7281e75cddb0`; their SHA-256 values
and upstream Git blob IDs are in `lean/interpreter/vendor/testsuite/SOURCE.json`.

## Non-claims

This lane is an artifact-specific pilot, not a verified compiler, not a proof of Talos's fidelity to
the Wasm specification, and not a V8 correctness claim. A module digest binds identity, not
semantics. The inventory and any future theorem cover only this exact linear-memory artifact;
`WasmGc` modules are outside scope. Imported memory/table/global behavior and the initial ABI state
remain trust assumptions unless a retained proof discharges them. Unsupported or inconclusive
results are never evidence of a miscompilation.

## Trust statement

The semantics candidate is Talos at
`d7c84ac660757953b752bdfc9cb8dae3dab623ed`, run with Lean 4.35.0 and its matched
Mathlib/dependency revisions, plus the local dropped-segment `memory.init` source-bound correction
described above. Fidelity of the remaining Talos semantics and general correctness of the local
correction remain trusted; `Examples/SmallStep.lean` kernel-checks the stated memory32 and
memory64 zero-length/nonzero-source regressions. The pinned fixture is executable conformance
evidence, not a general proof of source-bound correctness. The specification reference is the
WebAssembly/spec commit
`f3268fa9dcb7d3a60d09546e743e7281e75cddb0`; its `test/core` tree is pinned to the same repository
commit. Compatibility of the selected older module with that revision relies on the specification's
English change-history statement, not a Lean theorem.

The decoder is external `wasm-tools 1.245.1`, selected through `policy/tool-selection.json`; it
produces WAT that is re-encoded and byte-compared with the digest-bound module. Correctness of the
decoder and of its text serialization is trusted beyond that round trip. The finite execution engine
and module loader are pinned Node.js/V8 from the same policy; they are independent observations, not
proof premises discharged by Talos. Only the managed linear-memory representation is in scope;
Wasm GC modules are not covered.

The selected subject has 27 imports: one 16-page memory, one 16,384-entry `funcref` table, and 25
mutable globals. It has no imported functions. The values and limits for the initial ABI state come
from `crates/noble-cli/src/core/runtime/abi.json`; faithful interpretation of that state, memory,
table, and imported globals is a trusted boundary. No claim is made about host-function behavior,
because this subject imports none. The digest binds module identity only; it does not establish
source correspondence or semantic correctness. `NobleContracts.Exec` is the source-side model;
source-to-module correspondence is a separate theorem obligation.

## Go/no-go against TB+

The comparator is the selected strengthened trusted-build route (TB+) in
`.cairn/changes/artifact-correspondence-extension/design.md`: it deterministically rebuilds the
consumer-selected source with policy-pinned tools and compares final bytes. TB+ has low, repeatable
per-build cost, but retains trust in `noble-wasm`, runtime helpers, `wasm-tools`, any selected
optimizer, and V8. This artifact pilot would instead remove the emitter/optimizer from the trust base
for each exact proved module, while adding a Wasm decoder, a large semantics, ABI setup, and a proof
for every admitted fixture.

| Fixture | Exact-module proof effort/result | Decision relative to TB+ |
|---|---|---|
| Retained `7 3 * 5 +` module (10,474 bytes; 3,063 instructions) | Source-side MC1 execution lemma is recorded. A complete Talos run/refinement theorem and its effort are not yet established. | TB+ remains the usable admission choice; no semantic-proof claim. |
| New input module for `x y *` | A universally quantified module-to-`Exec` theorem and per-proof cost are not yet established. | No evidence yet that theorem effort beats TB+'s low fixed rebuild cost. |

**NO-GO** for accepting either fixture as semantically validated or replacing TB+ with this pilot.
The bounded research lane may continue, but the decision reopens only after the complete digest-bound
theorem, trap/exhaustion separation, wrong-artifact and stale-digest controls, and the pinned V8/spec
differential evidence all pass. A failed or timed-out proof remains inconclusive. This is not a
compiler-correctness result and does not make the trusted-build route unnecessary.
