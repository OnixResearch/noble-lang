# Issue #57 Owi evaluation decision

## Decision

**Evaluation in progress; no final go/no-go decision yet.** The patched engine previously rejected
the separate `noble` host module with `unknown module noble` (exit 49). Per the decision route, this
is the reason to implement the additional-module registration patch, not a final blocker until that
patch is attempted. Merging modules would change the subject bytes and is permitted only as a
separately labelled experiment, not as acceptance evidence.

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
- The patched symbolic attempt recorded in `receipts/2026-11-09/patched.json` reports
  `unknown module noble`, exit 49. It predates the required module-registration patch and is not
  treated as a final blocker or solver result.
- No I64-input optimized/unoptimized matrix, symbolic driver, counterexample, replay, timeout
  control, or performance measurement exists in the retained lane evidence. No final decision is
  recorded until the module-registration and execution route is attempted.
- Unit 1: DONE in `220c0aef`; verification used the pinned Node 24 with `env -i`, a private
  temporary directory, pinned wasm-tools 1.245.1, and `verification/owi/host.mjs`; result
  `{"clean":true,"imports":27,"globals":{"i32":23,"i64":2}}`. The receipt is
  `receipts/2026-11-13/host.json`.
- Unit 2: DONE — clean build at source commit `1e218e15`; `nix build --no-link --print-out-paths
  --option min-free 0 --option max-free 0 --option connect-timeout 10 --option fallback true -L
  ./verification/owi/nix` exited 0 and returned `/nix/store/37fb74k0fd9wj3gkbh32i8agydwzwili-ocaml5.4.1-owi-0.2-unstable-2026-05-22`.
  Derivation, recipe/patch hashes and binary SHA-256 are bound in
  `receipts/2026-11-13/owi-build.json`.
- Unit 3: NOT DONE. No `--opt on` I64-input emitted artifact or `-`, `swap -`, `+` symbolic fixtures
  were produced.
- Unit 4: IN PROGRESS. The second patch adds validated, ordered auxiliary-module registration via
  `OWI_REGISTER_MODULES`; the patched recipe build and real host+subject+driver run remain pending.
- Unit 5: NOT DONE. Required refusal controls, symbolic measurements, and a complete bound receipt
  do not exist. No final go/no-go is recorded yet.

## Decision criteria and next step

Complete the source-pinned registration patch and build, register host then unchanged subject,
and run the driver through the target entry point before making the final decision. Complete units
3 and 5 as well. If the patched engine fails on the runtime itself, record that exact reproducible
blocker and make NO-GO. If registration cannot be made without replacing or rewriting the subject,
record that attempt and keep any merged-module result explicitly secondary.
