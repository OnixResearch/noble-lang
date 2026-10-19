# `noble-ngraph` architecture lint decisions

Status: reviewed crate-level decisions that make the `noble_ngraph` architecture
phase report zero findings. They are code-level `#[expect]` records, not
Noble architecture-policy waivers. The workspace still has findings in other
crates.

## Decisions

| Family | Decision |
|---|---|
| `platform_dependent_cast` | Noble supports only targets with `usize >= 32` bits (x86_64, aarch64, wasm32); every flagged cast is a `u32` index converted to `usize`. |
| `excessive_file_length` | Each checker/codec file is one cohesive D1-D9 review unit; splitting would churn canonical paths and the extracted subject. |
| `function_length` | Checker, parser and evaluator functions stay linear so their rule order is reviewable in one place. |
| `usize_in_public_api` | Host-side public APIs take slice/row lengths as `usize`; the crate is not part of the guest Wasm ABI. |
| `sentinel_fallback` | `GraphBuilder` converts producer-owned node/slot counts; `u32::MAX` is an unrepresentable marker that graph admission refuses. |
| `mutating_input_in_pure` | Every `&mut` parameter is a caller-owned explicit workspace, builder or output buffer. The crate has no ambient state, I/O or hidden mutation; these are explicit state transitions. |

No architecture-policy waiver was added. The decisions are visible at the crate
root of `crates/noble-ngraph/src/lib.rs`.

## Measured result

With the pinned Octet collector on `main` after the final `noble-ngraph` change:

- `noble_ngraph` findings: **0**
- Workspace findings: **3498**, all in other crates (`noble`,
  `noble_ngraph_experiment`, test units and so on)

The architecture phase therefore no longer blocks a collection on
`noble_ngraph`; `source-coverage` and `native-assurance` remain blocked by the
other crates and were not run for this decision.

## Verification

```sh
cargo test -p noble-ngraph
cargo clippy -p noble-ngraph --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo octet check -- --all-targets --all-features
```

## Non-claims

This is a lint/architecture disposition, not a source-coverage, native-assurance,
proof or release pass. The `&mut` decision is an explicit architectural choice
for this crate, not a claim that the lint is wrong for other crates.
