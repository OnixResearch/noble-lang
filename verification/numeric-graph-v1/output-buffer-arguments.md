# `borrowed_argument_types` disposition in `noble-ngraph`

Status: reviewed code decision with measured local results. This does not close
issue #25.

## Decision

The flagged functions are growable output buffers or capacity-sensitive
measurement helpers. A borrowed slice cannot replace them:

- `check::retained_bytes::vec_bytes<T>(&Vec<T>)` measures `Vec::capacity`.
- `extract::admission::push_shape` appends to a traversal worklist.
- `extract::symbolic::flatten` appends the flattened node sequence.
- `format::graph::shape_text::tree` appends canonical shape text to a `String`.
- `sem::Value::write_abi` appends a variable number of ABI bytes.
- `format::push_fmt` / `push_line` are the single text-emission boundary.

Each function carries a function-level
`#[expect(tigerstyle::borrowed_argument_types, reason = "...")]` naming the
growth or capacity requirement. No architecture-policy waiver is used and no
slice replaceable signature is left suppressed.

## Measurement

With the pinned Octet collector on `main` at `286f492`:

- `noble_ngraph` `borrowed_argument_types`: **10 -> 0**.
- Workspace total architecture findings: **4117 -> 4107**.

The five remaining sites are exactly the output-buffer functions above; the
formatting helpers were already covered by the `ignored_result` change.

## Verification

```sh
cargo test -p noble-ngraph
cargo clippy -p noble-ngraph --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo octet check -- --all-targets --all-features
```

Octet still exits 1 on the other pre-existing families. No behavior, format,
identity or evaluator semantics changed.

## Non-claims

This is a lint-family disposition, not a source-coverage or native-assurance
pass. `sentinel_fallback` (24), `unchecked_narrowing` (20), the large
`platform_dependent_cast` (184) and `mutating_input_in_pure` (160) families
remain open.
