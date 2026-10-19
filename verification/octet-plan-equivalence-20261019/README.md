# Octet architecture plan equivalence restored (2026-10-19)

`noble-octet-gate --workspace --artifact-dir … -- --all-targets --all-features` failed on
`338c3559b44d10c3b088af8123d31db5b16d6a42` before reaching any lint or architecture finding:

```
error: Cargo --all-features plan does not match the explicit architecture policy row
```

## Cause

Commit `8d077ea` added the empty `safety-observe` feature to `crates/noble-cli/Cargo.toml`.
The Octet architecture phase requires the explicit policy row's declared plan to equal the
`--all-features` plan (features equivalence, from the pinned Octet change
`accept-architecture-all-features`); `policy/architecture.ncl` still declared `features = []`.

## Fix

`production.targets` now declares `features = ["safety-observe"]` with `default_features = true`.
No Cargo manifest, lint level, architecture role or gate flag changed; the generated
`policy/architecture.json`, `policy/architecture.manifest.json` and `policy/tool-selection*`
exports were regenerated with the campaign `export-policy.sh` (pinned Nickel and
octet-standards), which also re-pinned `policy/tool-selection-files.ncl`.

## Measured result

- Before (`before-fix.log`): exit 2, `Cargo --all-features plan does not match the explicit
  architecture policy row`, no findings collected.
- After (`after-fix-status.json`, `after-fix-summary.txt`, `after-fix-results.jsonl`): the gate
  reaches the collector and reports the complete deny-all finding set -
  `total_findings 118`, `warning_findings 2`, `error_findings 116`,
  `cargo_process_exit { code: 101 }`.

Those 118 findings are issue #26's remaining work. This receipt covers only the
plan-equivalence regression; it makes no claim about the lints themselves.
