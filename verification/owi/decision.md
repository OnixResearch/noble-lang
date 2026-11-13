# Issue #57 Owi evaluation decision

## Decision

**NO-GO for symbolic execution on this lane at this revision.** The original binary decoder
failure was repaired and patched `owi validate` accepts the exact retained production artifact,
but the patched engine's symbolic CLI rejects the separate `noble` host module with
`unknown module noble` (exit 49). The current `owi sym` interface accepts one source module and
does not register an additional host module. Without that registration, this is not a faithful
runtime wrapper; merging modules would change the subject bytes and is permitted only as a
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
- `receipts/2026-11-13/host.json`: new clean-commit receipt, source `8a78dee0`; 27 imports matched,
  ABI digest and all six limits bound, initial-state comparison passed, and zeroed `step_limit`
  was refused. This is an interface/initial-state check, not execution.
- The patched symbolic attempt recorded in `receipts/2026-11-09/patched.json` reports
  `unknown module noble`, exit 49. This is a module-registration failure, not a solver result.
- No I64-input optimized/unoptimized matrix, symbolic driver, counterexample, replay, timeout
  control, or performance measurement exists in the retained lane evidence. Therefore the decision
  is a NO-GO, not an equivalence result or a successful Owi pilot.
- Unit 1: DONE in `8a78dee0`; verification used the pinned Node 24 with `env -i`, a private
  temporary directory, pinned wasm-tools 1.245.1, and `verification/owi/host.mjs`; result
  `{"clean":true,"imports":27,"globals":{"i32":23,"i64":2}}`. The receipt is
  `receipts/2026-11-13/host.json`.
- Unit 2: NOT DONE. The standalone locked recipe exists, but store path and hashes are not
  recorded. Initial `nix build --no-link --print-out-paths ./verification/owi/nix` failed because
  the historical decoder patch hunk did not match the locked package source; after adapting the
  patch, a rerun exited 1 after cache timeouts with no retained successful output path. Reopen only
  with a successful clean recipe build and a receipt binding its derivation, output and binary
  digest.
- Unit 3: NOT DONE. No `--opt on` I64-input emitted artifact or `-`, `swap -`, `+` symbolic fixtures
  were produced.
- Unit 4: NOT DONE. The driver and second module-registration patch are not implemented; the
  observed `unknown module noble` is the current exact blocker.
- Unit 5: NOT DONE. Required refusal controls, symbolic measurements, and a complete bound receipt
  do not exist. This NO-GO records the known blocker but does not claim the remaining evaluation.

## Reopen conditions

Reopen when a source-pinned Owi registration patch and standalone build reproducibly load the host
and driver alongside the unchanged production-emitted module, and a bounded run reaches the target
entry point. Then complete units 3–5 before changing NO-GO to GO. If registration cannot be made
without replacing or rewriting the subject, keep this lane NO-GO and label any merged-module run as
a secondary experiment only.
