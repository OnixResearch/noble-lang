# `sentinel_fallback` disposition in `noble-ngraph`

Status: partial clearance with an explicit remaining API decision. This does not
close issue #25.

## Cleared

Eighteen of the twenty-four findings are removed in code:

| Site | Disposition |
|---|---|
| `extract/admission.rs` resource slot | `u32::try_from(k).map_or(Site::Graph, Site::Slot)` |
| `extract/admission.rs` live-slot node | `at` now takes `Option<u32>`; an unrepresentable node reports `Site::Graph` |
| `extract/symbolic.rs` input slot | checked `index()` returning a `Diagnostic` |
| `extract/symbolic.rs` pending node | checked `index()` returning a `Diagnostic` |
| `extract/symbolic.rs` frame current | checked `index()` returning a `Diagnostic` |
| `facts.rs` interval minimum/maximum | direct four-element `min`/`max`, no fallback |
| `sem.rs` decimal lengths | `i64::try_from(..).ok()?` propagation |

No value is silently mapped to `u32::MAX`/`i64::MAX` in these paths. An
unrepresentable index in the symbolic extractor is an `E-FORMAT` diagnostic at
`Site::Graph`; an unrepresentable diagnostic slot degrades to the graph site.

## Remaining decision

Six findings remain in `ir.rs` at `GraphBuilder::push`, `slot` and
`declare_slot`:

```rust
NodeId(u32::try_from(self.graph.nodes.len()).unwrap_or(u32::MAX))
```

These are infallible builder methods used by the graph parser, extractor,
experiment fixtures and tests. Removing the fallback requires a public,
fallible builder API (`Result<NodeId, Diagnostic>` or a checked push that the
parser and callers propagate). That is an API migration, not a local edit; this
slice does not suppress the findings with an expectation.

## Measurement

With the pinned Octet collector on `main` at `15a8156`:

- `noble_ngraph` `sentinel_fallback`: **24 -> 6** (18 removed).
- Workspace total architecture findings: **4087 -> 4069**.

## Verification

```sh
cargo test -p noble-ngraph
cargo clippy -p noble-ngraph --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo octet check -- --all-targets --all-features
```

Octet still exits 1 on the remaining families and the six builder findings.

## Non-claims

This is a partial lint-family disposition, not a source-coverage or
native-assurance pass. `platform_dependent_cast` (184) and
`mutating_input_in_pure` (160) remain open, as does the fallible builder API.
