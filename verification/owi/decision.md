# Issue #57 Owi evaluation decision

## Decision

**NO-GO for the current Noble artifact/interface.** The separate-module registration patch builds,
and a clean symbolic run resolves the host, unchanged production subject, and driver. The driver
then reaches `submit`, which refuses the two pushed symbolic I64 values because the production
subject requires an empty operand stack. The accepted editor AST has no I64-input node, so this lane
cannot produce the parameterized emitted subject required to test arithmetic. This is a bounded
pilot NO-GO, not an Owi arithmetic defect or a counterexample. Merging modules would change the
subject bytes and is permitted only as a separately labelled experiment, not as acceptance
evidence.

## Frozen criteria

The route is the conservative route from the brief: preserve subject bytes and carry an Owi patch
that registers another Wasm module. Go requires a reproducible patched engine to load the real
subject plus the explicit host and driver, execute `begin`/two symbolic `push_i64`/`submit`, and
compare the ordered stack against an independent reference. A GO decision additionally requires
exact-bit incorrect-result witnesses with independent replay, an equivalent fixture without a
manufactured counterexample, stale/wrong-module, wrapper/ABI, missing-import, UNKNOWN/timeout and
replay-failure controls, and receipts binding all inputs, initial state, limits, tool versions and
measurements. `wasm-merge` output is never treated as an unmodified emitted module.

## Evidence and limits

- `receipts/2026-11-09/patched.json`: pinned fork decoder patch accepts subject, reproducer and
  control; unpatched Owi refuses the real subject before linking.
- `receipts/2026-11-13/host.json`: new clean-commit receipt, source `220c0aef`; 27 imports matched,
  ABI digest and all six limits bound, initial-state comparison passed, and zeroed `step_limit`
  was refused. This is an interface/initial-state check, not execution.
- `receipts/2026-11-13/owi-registration-build.json`: clean build source `b1a9b5c5`; the standalone
  recipe applies both decoder and registration patches, with exact derivation, store output, and
  binary hash recorded.
- `receipts/2026-11-13/owi-runtime-probe.json`: clean-run source `149b508b`; Owi linked `noble`
  host, unchanged subject, and driver. It reported `Trap: unreachable`, symbols `0,0`, exit 13.
  The driver's explicit trap follows nonzero `submit`; in the subject's emitted `submit` path,
  `check_input` rejects a nonempty stack (failure code 3) before evaluating the recipe. This is a
  refusal caused by the attempted wrapper contract, not a counterexample.
- `crates/noble-contracts/src/source/editor.rs` defines editor nodes as Integer, Boolean, Word,
  Quotation, and Hole; `crates/noble-cli/src/core/editor.rs` accepts only those terminal kinds.
  No accepted I64-input node is available to emit a parameterized expression. The only retained
  subject is the constant expression `7 3 * 5 +`, whose emitted result is `I64 26`.
- Unit 1: DONE in `220c0aef`; verification used the pinned Node 24 with `env -i`, a private
  temporary directory, pinned wasm-tools 1.245.1, and `verification/owi/host.mjs`; result
  `{"clean":true,"imports":27,"globals":{"i32":23,"i64":2}}`. The receipt is
  `receipts/2026-11-13/host.json`.
- Unit 2: DONE — decoder-only clean build at `1e218e15` and decoder+registration clean build at
  `b1a9b5c5`; both used `nix build --no-link --print-out-paths --option min-free 0 --option
  max-free 0 --option connect-timeout 10 --option fallback true -L ./verification/owi/nix` and
  exited 0. Exact identities are in `owi-build.json` and `owi-registration-build.json`.
- Unit 3: NOT DONE. No `--opt on` I64-input emitted artifact or `-`, `swap -`, `+` symbolic fixtures
  were produced; the editor input grammar has no I64-input node.
- Unit 4: NOT DONE. The registration patch builds and the clean driver reaches the runtime, but
  `submit` rejects its two symbolic pushes at the empty-input-stack check; therefore no ordered
  arithmetic output could be read or compared against the independent reference.
- Unit 5: NOT DONE. Required refusal controls, symbolic measurements, and a complete bound receipt
  do not exist; no valid incorrect-result witness or replay exists.

## Decision criteria and next step

Reopen when the accepted source/emitter can produce a real parameterized pure-I64 subject whose
runtime accepts the documented input contract (without replacing or rewriting the subject during
the symbolic run). Then rerun the optimized/unoptimized `-`, `swap -`, `+` matrix; assert the
ordered output against an independent reference; retain exact-bit witnesses and independent
`noble worker scalar`/BigInt or Lean replay; and run all frozen mismatch, missing-import,
timeout/UNKNOWN, and replay-failure controls with bound receipts and measurements. Any `wasm-merge`
module remains secondary and cannot satisfy this gate.
