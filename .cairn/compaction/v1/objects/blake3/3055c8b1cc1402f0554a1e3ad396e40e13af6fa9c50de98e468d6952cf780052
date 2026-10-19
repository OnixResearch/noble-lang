# Numeric graph CPU measurement before backend selection

## ADDED Requirements

### Requirement: NGRAPH-BENCH-01
r[NGRAPH-BENCH-01]

A numeric graph continuation decision MUST be based on a pinned CPU measurement. The measurement MUST compare the CPU reference evaluator, the compiled CPU graph evaluator, the currently available Noble Wasm/V8 path where semantics overlap, and a straightforward host-native reference implementation. It MUST report extraction, admission and build costs, live node count and depth, parameter, activation, scratch and retained bytes, cold and warm latency, repeated-evaluation and training cost, and the source, tool, profile and hardware revisions. Unsupported instrumentation MUST remain explicit rather than be reported as zero. No GPU, direct-native or alternate-CPU backend MAY be selected, and no speedup MAY be claimed, from this measurement alone.
