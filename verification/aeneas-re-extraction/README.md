# Aeneas 1238879f re-extraction blockers (#97)

This directory records the two minimal Aeneas failures that block completing issue #97's subject re-extraction. Both standalone crates were committed as source commit `1fcc664` on branch `iss/reextract-97` (worktree base `01c0df9b`). The checked-in failure records bind the source commit, selected tool revisions, exact argument vectors, exit statuses, diagnostic excerpts, and SHA-256 hashes of the raw temporary logs and LLBC outputs.

## Results

| Reproducer | Charon | Aeneas | Diagnostic |
|---|---:|---:|---|
| [`vec-new`](reproducers/vec-new/src/lib.rs) | 0 | 2 | `Unimplemented` at `alloc/src/vec/mod.rs:464:4-464:30`, `symbolic/SymbolicToPureTypes.ml:411`, while translating `Vec::new` used as a generic function item. |
| [`iterator-find`](reproducers/iterator-find/src/lib.rs) | 0 | 2 | `Unimplemented: found an occurrence of a lifetime constraint relating a higher-ranked lifetime to a free lifetime` at `core/src/slice/iter/macros.rs:338:12-341:46`, `llbc/TypesAnalysis.ml:994`. |

Full records: [`vec-new-failure.json`](vec-new-failure.json) and [`iterator-find-failure.json`](iterator-find-failure.json). Charon was invoked with `--inline-anon-consts` in both cases. That argument addresses the promoted-constant inlining regression; it does not fix either limitation above. Aeneas produced no Lean files for either reproducer, and no Lean proof was attempted. The separate promoted-`'static` negative control passed: Aeneas refused both escaping promoted constants with `There should be no bottoms in the value` (recorded by unit 3, commit `048c3971`).

The attempted full-kernel extraction with a reviewed opaque-item fallback also failed before producing a complete subject: Aeneas exited 2 with `Could not read a borrow` at `crates/noble-kernel/src/shapes.rs:251:17-251:22`, followed by a context mismatch at `crates/noble-kernel/src/contracts/nominal.rs:300:25-300:51`. No opaque exception was adopted, and no downstream Lean check was reached.

The selected revisions were Aeneas and Aeneas Lean library `1238879f9ea550512096e9bf8bd6fe7407b3eaef`, Charon `9e95cf318e3a2964fcf5e03104cde5b718f98ea5`, and extraction Rust `1.100.0-nightly-2026-09-17`; see each record for the resolved store paths and nar hashes.

## Generated subjects and lane receipts

The unit-4 byte comparisons cover the eight checked-in generated files: `Funs.lean` and `Types.lean` for the M2 kernel, M3 kernel, M3-Wasm emitter, and MC1 contract subjects. Those files match output from the selected Aeneas `1238879f` extractor for their recorded inputs; this does not establish a complete full-kernel translation or successful proof-port.

The committed M4 `implementation.json` and `extraction-lock.json`, M3-Wasm `implementation.json`, and MC1 `implementation.json` receipts still identify the earlier Aeneas `505b6ca` run. They are retained as historical evidence and are not current receipts for the `1238879f` selection. Their active producers are bound to the current selection; fresh receipts remain pending until full extraction and the affected Lean lanes pass. The M1 README similarly labels its `505b6ca` / `1238879f` run as historical. Do not use any of these earlier receipts as evidence for the current extractor.

## Re-running

Resolve the selected `charon`, `aeneas`, and extraction-Rust binaries from `policy/tool-selection.json`. From either reproducer directory, use the record's temporary environment and run:

```sh
charon cargo --preset=aeneas --error-on-warnings --inline-anon-consts \
  --dest-file "$OUT/subject.llbc" -- \
  --manifest-path Cargo.toml --lib --all-features \
  --target x86_64-unknown-linux-gnu --locked --offline

aeneas -backend lean -abort-on-error -warnings-as-errors -no-progress-bar \
  -emit-json -split-files -gen-lib-entry -all-computable \
  -dest "$OUT/generated" "$OUT/subject.llbc"
```

Use a private tmpfs work area for `OUT`, `HOME`, `TMPDIR`, `CARGO_HOME`, and `CARGO_TARGET_DIR`; the committed records include the exact values used for the retained run. The raw output lives under that temporary area and is not committed; the failure records preserve the diagnostic excerpts and hashes.
