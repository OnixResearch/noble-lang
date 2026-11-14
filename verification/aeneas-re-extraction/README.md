# Aeneas 1238879f re-extraction blockers (#97)

This directory records the two minimal Aeneas failures that block completing issue #97's subject re-extraction. Both standalone crates were committed as source commit `1fcc664` on branch `iss/reextract-97` (worktree base `01c0df9b`). The checked-in failure records bind the source commit, selected tool revisions, exact argument vectors, exit statuses, diagnostic excerpts, and SHA-256 hashes of the raw temporary logs and LLBC outputs.

## Results

| Reproducer | Charon | Aeneas | Diagnostic |
|---|---:|---:|---|
| [`vec-new`](reproducers/vec-new/src/lib.rs) | 0 | 2 | `Unimplemented` at `alloc/src/vec/mod.rs:464:4-464:30`, `symbolic/SymbolicToPureTypes.ml:411`, while translating `Vec::new` used as a generic function item. |
| [`iterator-find`](reproducers/iterator-find/src/lib.rs) | 0 | 2 | `Unimplemented: found an occurrence of a lifetime constraint relating a higher-ranked lifetime to a free lifetime` at `core/src/slice/iter/macros.rs:338:12-341:46`, `llbc/TypesAnalysis.ml:994`. |

Full records: [`vec-new-failure.json`](vec-new-failure.json) and [`iterator-find-failure.json`](iterator-find-failure.json). Charon was invoked with `--inline-anon-consts` in both cases. That argument addresses the promoted-constant inlining regression; it does not fix either limitation above. Aeneas produced no Lean files for either reproducer, and no Lean proof was attempted. The separate promoted-`'static` negative control passed: Aeneas refused both escaping promoted constants with `There should be no bottoms in the value` (recorded by unit 3, commit `048c3971`).

The selected revisions were Aeneas and Aeneas Lean library `1238879f9ea550512096e9bf8bd6fe7407b3eaef`, Charon `9e95cf318e3a2964fcf5e03104cde5b718f98ea5`, and extraction Rust `1.100.0-nightly-2026-09-17`; see each record for the resolved store paths and nar hashes.

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
