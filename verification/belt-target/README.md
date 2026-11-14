# Fixed target schedule evidence (issue #114)

This lane is an optional target-model specialization over the checked I64/Bool
belt graph. It does not replace the sequential belt semantics or select a
production backend. T1 and T2 are fixed abstract descriptors; the lowerer is
untrusted, and target artifacts are independently checked before execution.

## Reproduction

Run the ignored fixed-workload report from a clean source commit. Set `report`
to a new absolute file outside the repository; the Cargo configuration entries
pass both explicit values to the test process:

```sh
report="$HOME/model-cost.json"
source_commit=$(git rev-parse HEAD)
cargo test \
  --config "env.NOBLE_TARGET_REPORT=\"$report\"" \
  --config "env.NOBLE_TARGET_SOURCE_COMMIT=\"$source_commit\"" \
  -p noble-ngraph --test schedule_correspondence --offline --locked -- \
  --ignored --nocapture belt_target_measure::fixed_model_cost_report
```

The protocol is frozen in the Cairn design: two fixed workloads (a six-node
reorderable graph and an eight-node preservation-pressure graph), each run on
both profiles in the Rust test profile, with 20 warm-up calls and 200 measured
lower/check pairs. The JSON records graph/output sizes, modeled cycles, issue
width and utilization, preservation traffic, event count, scheduler work,
encoded bytes, peak in-flight event records, the configured pending bound, and
mean nanoseconds for lower and independent checking. The retained report is
[`model-cost.json`](model-cost.json); it binds the measured source commit.

The cycle/utilization/preservation fields are abstract-machine model outputs.
Nanosecond fields measure this local Rust test-profile implementation path, not
optimized production code. Neither is hardware performance evidence: no
processor was measured and no frequency, power, area, or speedup claim follows.
The target descriptors are not an ISA specification. The Lean theorem proves only latency-derived output availability and ordered
mapping; the symbolic run obtains node values from the graph reference and
does not independently execute arithmetic or belt/save transitions. Rust
artifact and operand-age correctness are covered by separate finite tests, not
a Rust-to-Lean refinement theorem. Outputs wider than a target belt are
refused rather than streamed.
