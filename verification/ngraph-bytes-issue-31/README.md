# Issue #31: graph-byte accounting covers graph-owned storage

Source revision: `338c3559b44d10c3b088af8123d31db5b16d6a42` (base of the fix).

## What changed

`noble_ngraph::check::retained_bytes` (the estimate charged against `Budgets::graph_bytes`) now covers every graph-owned heap allocation instead of fixed-size headers only:

- `Subject.closure` element storage, charged at `Vec::capacity`;
- the `input_shape`/`output_shape` vectors plus the boxed pair children of every `ShapeTree` (one two-`ShapeTree` allocation per pair node, walked recursively);
- `Node.origin.expansion` vectors;
- fact payloads: `Fact::OneHot` groups and `Provenance::Derived`/`Provenance::Evidence` buffers;
- node operation payloads (`F32Affine` terms, `F32Index` tables, `Guard` one-hot groups) at capacity.

The estimator is conservative and never wraps: every sum and product saturates, so an unrepresentable total is charged as `u64::MAX` and refused rather than admitted. Element storage is charged at capacity, so spare capacity retained by a producer counts. Allocator bookkeeping and the checker's own derived caches (`CheckedGraph` maps and layers) are outside the contract, as documented on the function.

## Measured before/after

Reproduction (see `repro-before-fix.txt`): with `check.rs` at the base revision, the new `tests/retained_bytes.rs` fails 4 of 6 tests. The decisive line:

```
charge 318 must cover the 65536-byte closure above 318
```

After the fix (`test-log.txt`): all six tests pass, including the closure ladder (charge rises from 318 to 65854 for 8,192 closure entries), the shape ladder (2,016 bytes of boxed pairs charged), and the fact/node-payload cases. `cargo test -p noble-ngraph` passes in full, `cargo clippy -p noble-ngraph --all-targets -- -D warnings` is clean, and `cargo fmt -p noble-ngraph -- --check` reports no diff.

## Limits

The tests assert the accounting contract - charge growth, `E-BUDGET-GRAPH-BYTES` refusal under a smaller budget, and admission once the budget covers the charge. They do not measure process memory, allocator overhead, or denial-of-service behaviour, and the `u64` saturation path is not reachable by construction in a test (it needs capacities near `2^61`); saturation is provided by the saturating operators and covered by review.
