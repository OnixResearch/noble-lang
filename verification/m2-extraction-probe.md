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
| 6 | `toolrun-probe8.log`, `run1/aeneas.log` | ok | fail | borrow interpreter internals across the remaining `&`-heavy helpers: `parts.rs:79-91` (`join`), `words.rs:174-179` and `219-224` (`subst_stack`/`subst_effects`), `words/subst.rs:92-99` and `262-270`, `instantiate.rs:160-167`, `preflight.rs:64-71` |

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

## The remaining refusal (probe 6)

```text
[Error] Internal error, please file an issue
Source: 'crates/noble-kernel/src/acceptance/parts.rs', lines 79:4-91:5
Compiler source: interp/InterpMatchCtxs.ml, line 230
[Error] Could not match the contexts
Source: 'crates/noble-kernel/src/words.rs', lines 174:57-179:59
Compiler source: interp/InterpJoin.ml, line 1542
[Error] Internal error, please file an issue
Source: 'crates/noble-kernel/src/words/subst.rs', lines 92:26-99:55
Compiler source: interp/InterpReduceCollapse.ml, line 1137
[Error] Internal error, please file an issue.
Found an already registered mapping: abs@76 -> borrow@2
Source: 'crates/noble-kernel/src/words/subst.rs', lines 262:4-270:5
Compiler source: interp/InterpMatchCtxs.ml, line 68
[Error] Internal error, please file an issue
Source: 'crates/noble-kernel/src/acceptance/parts/instantiate.rs', lines 160:4-167:5
Compiler source: interp/InterpMatchCtxs.ml, line 230
[Error] Internal error, please file an issue
Source: 'crates/noble-kernel/src/acceptance/preflight.rs', lines 64:4-71:5
Compiler source: interp/InterpMatchCtxs.ml, line 230
```

Probes 4-6 answered every refusal that came from a *specific* construct:
reference-carrying walks became owned walks, option-combinator closures became
branches, and the remaining failures are now inside the borrow interpreter's
own passes (`InterpMatchCtxs`, `InterpJoin`, `InterpReduceCollapse`) on
functions whose signatures still thread shared borrows (`&Env`, `&Request`,
`&Scheme`, `&Inst`, `&[Ty]`) while owned state is mutated and returned.

The M2 design already names the subset that avoids this: *owned data, simple
control flow*. The next iteration therefore widens the redesign from the walk
bodies to the kernel's signatures:

- `Scheme::subst_stack`, `Scheme::subst_effects`, `Scheme::instantiate`, and
  the `subst_pattern` walk take owned `Scheme`/`Inst` values and return owned
  results;
- `parts::join`, `parts::limits_of`, `parts::charge`, `parts::invalid`, and
  the `Ctx` holder take owned inputs (or plain scalars) instead of `&Ctx`;
- `instantiate::{apply, project}` and `preflight::check_request` take owned
  schemes and instantiations;
- the machine (`run`, `fold_current`, `open_current`, `close_frame`) then
  threads owned frames, candidate nodes, and context values.

Each change must keep the catalog at zero findings, the workspace tests green,
and the probe rerun; the redesign is only accepted when Aeneas produces the
generated Lean module and the module compiles.
