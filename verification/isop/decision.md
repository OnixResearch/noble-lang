# Issue #35 decision: bounded Boolean ISOP

## Preregistration and amendment

The adoption rule was frozen before measurement in `b55ea7b6`, then amended in `350fb832` to pin the primary lower/upper recurrence and cofactor bounds. The amendment preceded all benchmark invocations; it was a specification correction, not a response to measured results. The operative criteria file is `verification/isop/criteria.md` (SHA-256 `14ea06da05f0d9d874fc6568b58620b3642e22cfb36c617bea865f70f9f21337`).

## #32 residual whitelist census

This is a census by construction, not a scan of runtime traces:

| Boundary | Accepted #32 region | Boolean-composition regions |
|---|---|---:|
| First-projection source model | Exactly two instructions, each one of I64 add/subtract/multiply | 0 |
| Residual graph | The selected arithmetic program's I64 residual; graph vocabulary has no general Boolean AND/OR/NOT operators | 0 |
| Foundation fragment | Branching is explicitly outside the selected whitelist | 0 |

The first-projection opcodes and two-instruction program are in `proofs/foundation/NobleFoundation/FirstProjection.lean:153-167`; its residual construction is at `:400-411`. The graph vocabulary is in `crates/noble-ngraph/src/ir.rs:279-346`, and the fragment boundary is documented under “Selected profile and whitelist” in `proofs/foundation/README.md`. There is no admitted Boolean region or observed dynamic invocation count to evaluate for source integration.

## Frozen synthetic experiment

The committed Rust prototype was measured from source commit `810f397b49d7e162ac9b3423297dbc00e74c4342`, with the operative criteria hash above. The release binary was built with pinned Rust 1.96.0-nightly (`ac7f9ec7d`, 2026-03-20); its SHA-256 and the exact build/run invocations are retained in [`measurement-manifest.json`](measurement-manifest.json). Three fresh, sequential process invocations ran on the same host, each with 30 calibrated samples per arm and a minimum 20,000 ns batch. Requested CPU 2 was confirmed by each process's `Cpus_allowed_list`. All five workloads passed the independent checker in all three runs; no runs were dropped or pooled.

The recorded host was an AMD Ryzen AI MAX+ 395 with Radeon 8060S. The 1/5/15-minute load averages at each run start were 22.67/19.27/20.78. Pinning did not reserve the core or isolate the host, so these are local synthetic observations, not a general performance claim. The raw per-sample summaries, calibration batch sizes, load records, and all arm results are retained separately in [`run-01.json`](run-01.json), [`run-02.json`](run-02.json), and [`run-03.json`](run-03.json).

### Workloads

| Workload | Atoms / rows | DAG / folded nodes | ISOP cubes / literals | Cover storage bytes* |
|---|---:|---:|---:|---:|
| absorption-2-atom | 2 / 4 | 6 / 6 | 1 / 1 | 34 |
| absorption-6-atom-interface | 6 / 64 | 6 / 6 | 1 / 1 | 34 |
| parity-4 | 4 / 16 | 16 / 16 | 8 / 32 | 48 |
| shared-factor-3-atom | 3 / 8 | 8 / 8 | 2 / 2 | 36 |
| constants-repeated-atom-3-atom | 3 / 8 | 6 / 3 | 1 / 1 | 34 |

*Cover storage is the `Cover` struct plus the cube vector's retained capacity; it excludes allocator metadata and is not the optimizer's peak working-set size.

### Per-run medians

Nanoseconds per operation; each value is that run's median across its 30 calibrated samples. Values are rounded to two decimals here; the raw receipts preserve full precision and per-run distributions.

| Run | Workload | No-pass eval | Folding build | Folding eval | ISOP build | Independent check | ISOP eval |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | absorption-2-atom | 11.95 | 57.46 | 9.25 | 1,050.72 | 324.36 | 2.48 |
| 1 | absorption-6-atom-interface | 15.62 | 53.73 | 16.95 | 8,192.75 | 2,901.63 | 1.98 |
| 1 | parity-4 | 31.94 | 99.17 | 34.38 | 3,267.38 | 2,133.38 | 4.40 |
| 1 | shared-factor-3-atom | 15.14 | 61.25 | 20.96 | 1,279.88 | 696.77 | 2.65 |
| 1 | constants-repeated-atom-3-atom | 16.82 | 40.06 | 6.14 | 1,526.63 | 568.88 | 1.65 |
| 2 | absorption-2-atom | 14.97 | 48.47 | 14.97 | 837.50 | 281.07 | 2.26 |
| 2 | absorption-6-atom-interface | 10.78 | 51.70 | 8.59 | 5,951.25 | 4,639.88 | 2.44 |
| 2 | parity-4 | 31.20 | 88.95 | 31.84 | 3,350.00 | 1,609.25 | 5.03 |
| 2 | shared-factor-3-atom | 19.82 | 52.27 | 19.63 | 1,710.69 | 719.47 | 2.40 |
| 2 | constants-repeated-atom-3-atom | 15.38 | 41.27 | 10.32 | 1,547.25 | 627.58 | 2.24 |
| 3 | absorption-2-atom | 14.84 | 46.28 | 9.40 | 969.31 | 290.31 | 2.12 |
| 3 | absorption-6-atom-interface | 8.43 | 51.93 | 15.76 | 8,676.25 | 3,799.63 | 2.25 |
| 3 | parity-4 | 19.22 | 85.28 | 31.23 | 3,995.00 | 2,105.81 | 4.41 |
| 3 | shared-factor-3-atom | 18.37 | 33.19 | 18.26 | 1,543.19 | 602.22 | 2.52 |
| 3 | constants-repeated-atom-3-atom | 17.06 | 43.87 | 10.58 | 1,302.44 | 584.84 | 1.59 |

Within this synthetic corpus, ISOP evaluation medians are lower than both no-pass and folding evaluation medians in all 15 run/workload pairs. Interval-cover construction and the independent check add substantial fixed cost, measured separately above. No synthetic invocation count is substituted for the missing dynamic count of a real #32 residual; the frozen source-adoption cost condition is therefore not evaluated from these synthetic results.

## Decision

**Source integration: NO-GO / deferred — no admissible workload.** The frozen rule requires a separately reviewed #32 Boolean/branch extension that preserves K-CONTROL-01 laziness and yields a real residual Boolean-composition region. Neither exists in the selected whitelist, whose Boolean-region census is zero. Accordingly, synthetic speedups do not satisfy the source-workload or source-bound cost criteria. This is a boundary decision, not a claim that the prototype is generally faster or slower.

Reopen only after a reviewed extension preserves branch laziness and produces a real residual corpus containing Boolean-composition regions. Then check every admitted rewrite independently and measure end-to-end analysis, checking, and use cost at observed dynamic invocation counts across three source-bound runs, against both no-pass and folding baselines; the frozen threshold remains at least 10% lower than both.

## Reproduction

From the repository's approved Rust environment, build the committed source with:

```sh
cargo build -p noble-ngraph-experiment --release --bin ngraph-phase-c --offline --locked
```

Run each invocation as a fresh process, replacing the binary and output placeholders; keep outputs outside the repository:

```sh
taskset -c 2 <absolute path of the built ngraph-phase-c benchmark binary> \
  --isop-bench true --samples 30 \
  --source-rev 810f397b49d7e162ac9b3423297dbc00e74c4342 \
  --toolchain "rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)" \
  --pinned-cpu 2 --out "<new directory outside the repository>/run-01.json"
```

Repeat with distinct output names for runs 2 and 3. The retained manifest binds the performed invocations, outputs, criteria, source commit, and executable digest. The implementation is an experimental standalone DAG optimizer; it claims no source lowering, `Program`, compiler, Wasm, runtime, or cache correspondence.
