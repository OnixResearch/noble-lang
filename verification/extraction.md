# Bounded kernel extraction

**Task 3.1 provides the entry point.** Task 3.2 adds the rejection controls; task 4.4 owns the Nix `extraction` check.

Successful extraction and Lean compilation are not a refinement proof and not M1 acceptance.

## Command

```sh
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' \
  run .#extract-kernel -- --root "$ROOT" --selection "$ROOT/policy/tool-selection.json" \
    --inventory "$EVIDENCE/inventory/inventory.json" --artifact-dir "$EVIDENCE/extraction" \
    --proof-dir "$EVIDENCE/proof"
```

`--proof-dir` is optional and defaults to `<artifact-dir>/proof`.
A warm backend is reused only after the Lean dependency identity check passes; otherwise the entry point clones the pinned backend.

## Phases

| Phase | Command |
|---|---|
| backend bootstrap | `lake --no-cache env true` in the proof root |
| backend cache | `lake exe cache get` |
| backend sources | `noble-lean-dependencies-check` over the proof root |
| charon | `charon cargo --preset=aeneas --error-on-warnings --dest-file llbc/noble_kernel.llbc -- --manifest-path crates/noble-kernel/Cargo.toml --lib --all-features --target … --locked --offline` |
| aeneas | `aeneas -backend lean -abort-on-error -warnings-as-errors -no-progress-bar -emit-json -dest lean llbc/noble_kernel.llbc` |
| lean build | `lake build NobleKernel` in the proof root |
| lean explicit | `lake env lean -DmaxHeartbeats=… -DmaxRecDepth=… -o lean/NobleKernel.olean NobleKernel.lean` |
| byte comparison | `cmp lean/NobleKernel.lean proof/NobleKernel.lean` |

The explicit Lean command writes a fresh object even when Lake reports the target as up to date.
Lean requires the module inside its working root, so the byte comparison connects the translator output to the compiled copy.
Keep the `noble_kernel.llbc` basename during regeneration comparisons.

## Source binding

`extraction.json` records the kernel source BLAKE3, the source tree, the inventory IR and coverage identities, the selected target, and the pinned Rust, Charon, Aeneas, and Lean revisions.
It also records the generated function names and the requested function, which must appear in the compiler-derived inventory first.
The requested function is rejected before extraction if the inventory does not contain it.

## Rejection controls

`extract-kernel verify` recomputes the source, inventory, selection, target, and tool-revision bindings and re-hashes every recorded output.
It rejects, in an observed run:

| Control | Mutation | Diagnostic |
|---|---|---|
| Positive baseline | Untouched record | `binding and outputs consistent` |
| Missing function | `--function noble_kernel::missing_budget` | `function-not-in-inventory` |
| Proof-required claim | `--claim proof-required` | `proof-required-unresolved:no-refinement-proof-exists` |
| Edited generated output | Append bytes to `lean/NobleKernel.lean` | `output-blake3-mismatch:lean/NobleKernel.lean` |
| Missing output object | Delete `lean/NobleKernel.olean` | `output-missing:lean/NobleKernel.olean` |
| Stale kernel source | Append a comment to the kernel root | `kernel-source-mismatch` |
| Changed selection policy | Change the pinned Aeneas revision | `selection-mismatch` |
| Changed inventory | Change the coverage identity | `inventory-coverage-mismatch` |
| Unrelated fixture | Separate repository, identical kernel bytes | `source-tree-mismatch` |
| Incomplete translation metadata | Rename the recorded `rust_name` | `function-not-in-generated-output` |
| Dependency drift | Change `Cargo.lock` | `workspace-lock-mismatch` |
| Macro drift | Add a `macro_rules!` body to the kernel | `kernel-source-mismatch` |
| Failed translation | Run the app on an `async` subject | non-zero exit, coroutine diagnostic, no record written |

Thirteen rejections plus two positive baselines were observed across the two control sets.
The dependency lock, kernel manifest, selection policy, target, tool revisions, source tree, kernel source, inventory identity, generated translation, and output ledger are all bound.
A changed feature set or argument list changes the selection policy and rejects as `selection-mismatch`.

### Unsupported body: established

Charon rejects an `async` body in the kernel crate:

```
error: Coroutine types are not supported yet
error: Coroutines are not supported
ERROR Charon failed to translate this code (2 errors)
```

`charon` exits 101 on that fixture, so the unsupported-body rejection is established for coroutine-producing bodies.

Supporting evidence, retained: a second fixture with `dyn` trait-object dispatch, `dyn Fn` dispatch, a capturing closure, direct recursion, and an iterator-adapter chain was **accepted** (`charon` exit 0).
Those constructs are therefore not the exclusion, and earlier hypotheses about nested loops and mutable references were wrong.

### Generated-output binding

`verify` also requires the requested function to appear in `lean/translation.json` as a local generated function.
An absent function, or a missing translation record, is rejected as `function-not-in-generated-output` or `translation-record-missing`.

## Limits

The entry point always regenerates Charon and Aeneas output into a fresh artifact directory, so a stale artifact cannot be presented as current.
Freshness rejection, unsupported-body rejection, edited-output rejection, and proof-required rejection belong to task 3.2.
The generated Lean module compiles against the pinned backend; this does not establish refinement, Wasm correspondence, or cache authenticity.
