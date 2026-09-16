# M2 extraction probe: retained failures and the reachable subset

Task 3.1 requires the extraction subject to be regenerated and every failed
probe retained with its exact diagnostic. This record keeps the four
consecutive probe outcomes over the fragment core, the exact Aeneas refusals,
and the redesign each one forces. None of these results is an acceptance
claim; the fragment label stays `M2 fragment v0`.

## Route and configuration

| Input | Value |
|---|---|
| Probe script | `.pi/m2-extraction/toolrun.sh` (offline proof root, Charon, Aeneas, inventory collect) |
| Subject | `crates/noble-kernel` lib, all features, `x86_64-unknown-linux-gnu` |
| Charon / Aeneas | `b104e24fea7d721b71e6c39fd70f26ff20bc0980` / `505b6ca35217e7be5c96c3e2f8045edfbdf47291` |
| Extraction Rust | `nightly-2026-08-18` |
| Aeneas arguments | `-backend lean -abort-on-error -warnings-as-errors -no-progress-bar -emit-json` |
| Retained logs | `.pi/m2-extraction/run1/aeneas.log`, `charon.log`, `toolrun-probe*.log` |

## Probe outcomes (in order)

| Probe | Retained log | Charon | Aeneas | Refusal |
|---|---|---|---|---|
| 1 | `toolrun-probe2.log`, `run1-pre-probe.log` | ok | fail | `Breaks to outer loops are not supported yet` (`acceptance::preflight`) |
| 2 | `toolrun-probe4.log` | ok | fail | same class; kernel still returned from inside loops |
| 3 | `toolrun-probe5.log` | ok | fail | `Detected groups of mixed mutually recursive definitions` (`types::{Ty, ProgramTy}`, `shapes::{Pattern, StackPart, Signature}` × `Clone`/`PartialEq`/`Debug`) |
| 4 | `toolrun-probe6.log`, `run1/aeneas.log` | ok | fail | borrow interpretation: `InterpMatchCtxs` "Could not match the contexts" (`acceptance/parts/instantiate.rs:151-158`), `InterpBorrowsCore` "detected a loop in the chain of ids" (`types/size.rs:96-135`) |

Each refusal was answered by a source change, and every change kept the Octet
catalog at zero findings, the 14 workspace tests green, and `-D warnings`
clippy clean:

1. **Loop exits.** Aeneas cannot translate an LLBC `Break`/`Continue` that
   leaves more than the innermost loop, which is how Charon lowers a `return`
   inside a loop. The kernel now threads each walk as returned state
   (`Machine`/`Fold`/`Walk` state structs, accumulator plus `break`), so no
   loop body returns.
2. **Mutual type families.** Derived `Clone`/`PartialEq`/`Debug` impls of two
   mutually recursive type families form the mixed declaration groups Aeneas
   refuses. `ProgramTy`, `Signature`, and `StackPart` are gone: `Ty::Program`
   and `Pattern::Program` carry their stacks (and effect pattern) directly, so
   each family is self-recursive.
3. **Slice iterators.** Core's slice-iterator adapters carry higher-ranked
   lifetime constraints Aeneas cannot translate. Effect-set construction,
   membership, inclusion, environment lookup, and the first-extra scan are
   explicit index loops again.

## The remaining refusal (probe 4)

```text
[Error] Internal error, please file an issue
Source: 'crates/noble-kernel/src/acceptance/parts/instantiate.rs', lines 151:4-158:5
Compiler source: interp/InterpMatchCtxs.ml, line 230
[Error] Could not match the contexts
Compiler source: interp/InterpMatchCtxs.ml, line 68
[Error] end_abs_aux: detected a loop in the chain of ids: ...
Source: 'crates/noble-kernel/src/types/size.rs', lines 96:0-135:1
Compiler source: interp/InterpBorrowsCore.ml, line 98
```

Both sites keep *references into owned arenas* alive across owned mutations:

- `types/size.rs::queue_children` pushes `&Ty` children of the node under
  inspection onto an explicit work stack, while the caller (`Ty::size`) also
  pushes and pops owned `u32` sizes.
- `acceptance/parts/instantiate.rs::project` clones a value type out of an
  `Inst` binding while the same binding set is borrowed for the stack and
  effect substitutions.

## Required redesign (next probe iteration)

The extraction subset the M2 design names is *owned data, simple control
flow*. The walks that still hold references must hold owned values instead:

- `Ty::is_data`, `Ty::size`, and `size::walk_step`: push owned `Ty` values
  (cloned children) rather than `&Ty` into `walk.todo`.
- `shapes::validate` and `shapes::require_*`: queue owned `Pattern` values
  instead of `&Pattern` slices.
- `acceptance/parts/instantiate.rs::project`: substitute from an owned copy of
  the instantiation (or re-read each binding through `Inst` accessors) so no
  reference outlives the borrow that produced it.
- `acceptance/nodes.rs::node_of`: return a cloned node rather than a
  reference into the candidate arena.

Each change must keep the catalog at zero findings, the workspace tests green,
and the probe rerun; the redesign is only accepted when Aeneas produces the
generated Lean module and the module compiles.
