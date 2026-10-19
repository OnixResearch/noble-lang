# `ignored_result` disposition in `noble-ngraph`

Status: reviewed code decision with measured local results. This does not close
issue #25; the remaining architecture families are unchanged.

## Decision

`alloc::string::String` implements `core::fmt::Write` infallibly: its
`write_str` method always returns `Ok(())`. The crate previously discarded the
`fmt::Result` at every canonical-format write with `let _ = write!/writeln!`,
which the Octet `ignored_result` lint correctly flags.

`crates/noble-ngraph/src/format/mod.rs` now owns the single reviewed boundary:

- `push_fmt(&mut String, fmt::Arguments)` calls `write_fmt` and consumes the
  result with `expect("writing to a String cannot fail")`.
- `push_line` appends the line terminator.
- `write_text!` and `write_line!` are crate-private macros preserving the
  existing call-site syntax.

Callers in `format/snapshot.rs`, `format/graph.rs` and the `ir.rs` hex helper
use these helpers. No waiver is used and no result is discarded.

## Measurement

At baseline `c36d3d9`, Octet reported 62 `ignored_result` findings in
`noble_ngraph` (29 source sites, counted once per observing unit). After this
change, the same collection reports **0** `ignored_result` findings in
`noble_ngraph`.

The workspace still reports 77 `ignored_result` findings in other units:
16 in `noble_ngraph_experiment`, 48 in the `noble` CLI package, and the
remaining in other test units. Those are outside this slice.

## Verification

```sh
cargo test -p noble-ngraph
cargo clippy -p noble-ngraph --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo octet check -- --all-targets --all-features
```

The Octet run remains `lint-failure` on the remaining pre-existing families;
the `ignored_result` family in `noble-ngraph` is gone. No production behavior,
file format, identity or evaluator semantics changed; the existing canonical
print/parse round-trip tests exercise the affected paths.

## Non-claims

This is a lint-family disposition, not a source-coverage or native-assurance
pass. The desugaring/unresolved-definition decision and the other issue #25
families remain open. The `expect` relies only on the standard-library
`String` implementation and does not establish anything about external writers.
