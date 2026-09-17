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
| 5 | `toolrun-probe7.log`, `run1/aeneas.log` | ok | fail | `InterpBorrowsCore` "Can't end abstraction 5 as it is set as non-endable" over the option-combinator getters (`acceptance/nodes.rs:14`, `contracts.rs:79/84`, `words.rs:100`, `words.rs:210-215`) |

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

## The remaining refusal (probe 5)

```text
[Error] Can't end abstraction 5 as it is set as non-endable
Source: 'crates/noble-kernel/src/acceptance/nodes.rs', lines 14:18-14:52
Source: 'crates/noble-kernel/src/contracts.rs', lines 79:28-79:56
Source: 'crates/noble-kernel/src/contracts.rs', lines 84:28-84:57
Source: 'crates/noble-kernel/src/words.rs', lines 100:33-100:65
Source: 'crates/noble-kernel/src/words.rs', lines 210:55-215:59
Compiler source: interp/InterpBorrowsCore.ml, line 98
```

Every remaining site is an option-combinator chain whose closure captures a
borrow of the container (`candidate.nodes.get(index)`, `self.defs.get(index)`,
`self.kinds.get(index)`, `self.bindings.get(index)`, the `inst.effects` read
inside `subst_effects`). Probes 4 and 5 were answered by removing
reference-carrying walks; the same rule applies here:

- replace `.and_then(|index| container.get(index))` with an explicit
  `usize::try_from` match followed by a second `match` on `container.get(...)`
  (the shape `shapes::require_kind` already uses);
- own the patterns carried by `words/subst.rs`'s tasks
  (`Task::Part(Pattern)`, `Finish(Pattern)`, `Expand(Pattern)`) instead of
  borrowing them from the scheme under substitution;
- re-run probe 6 and keep its diagnostic either way.

Each change must keep the catalog at zero findings, the workspace tests green,
and the probe rerun; the redesign is only accepted when Aeneas produces the
generated Lean module and the module compiles.
