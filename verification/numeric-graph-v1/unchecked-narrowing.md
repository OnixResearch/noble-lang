# `unchecked_narrowing` disposition in `noble-ngraph`

Status: reviewed code decision with measured local results. This does not close
issue #25.

## Decision

The ten flagged conversions are masked limbs, bit-width-bounded exponent
fields, or a u64 split with an explicit mask. Each site has a local invariant
that makes the conversion lossless; the invariant is recorded on the function
with `#[expect(tigerstyle::unchecked_narrowing, reason = "...")]` rather than
silently masking an out-of-range value.

| Site | Invariant |
|---|---|
| `Big::from_u64` | low/high masks select exactly 32 bits from a `u64` |
| `Big::mul_small` | `u32 × u32 + u32 carry < 2^64`; masked limb and `>> 32` carry fit `u32` |
| `Big::add_small` | `u32 + u32 carry < 2^33`; masked limb fits `u32` and carry is 0/1 |
| `soft::trunc_f64` | the exponent field is masked to 11 bits before `i32` |
| `soft::sqrt_f64` | exponent mask is 11 bits; the 56-bit integer square root shifted by 3 fits `u64` |
| `Workspace::reset` (F32) | constants are stored from `f32::to_bits` and already fit `u32` |

No architecture-policy waiver is used. No conversion semantics or value range
changed.

## Measurement

With the pinned Octet collector on `main` at `1655461`:

- `noble_ngraph` `unchecked_narrowing`: **20 -> 0**.
- Workspace total architecture findings: **4107 -> 4087**.

## Verification

```sh
cargo test -p noble-ngraph
cargo clippy -p noble-ngraph --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo octet check -- --all-targets --all-features
```

Octet still exits 1 on the remaining families. The soft-float tests, big-integer
literal checks and the existing compiler/evaluator suites exercise these paths.

## Non-claims

This is a lint-family disposition, not a source-coverage or native-assurance
pass. `sentinel_fallback` (24), `platform_dependent_cast` (184) and
`mutating_input_in_pure` (160) remain open in `noble_ngraph`.
