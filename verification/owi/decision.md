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
- `receipts/2026-11-10/host.json`: historical import-shape evidence only; it predates ABI initial
  state checks added by this change and does not prove execution.
- The patched symbolic attempt recorded in `receipts/2026-11-09/patched.json` reports
  `unknown module noble`, exit 49. This is a module-registration failure, not a solver result.
- No I64-input optimized/unoptimized matrix, symbolic driver, counterexample, replay, timeout
  control, or performance measurement exists in the retained lane evidence. Therefore the decision
  is a NO-GO, not an equivalence result or a successful Owi pilot.
- Unit 1: ABI quota initialization and zero-step-limit refusal check are implemented here; its new
  receipt remains to be generated from a clean commit.
- Unit 2: standalone locked Nix recipe is implemented here; store path and hashes remain to be
  generated from a clean commit.
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
