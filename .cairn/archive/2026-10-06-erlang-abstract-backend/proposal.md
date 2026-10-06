## Why

Gleam's Erlang target now emits annotated Erlang abstract forms for the OTP
compiler rather than generating `.erl` text. Noble can study that documented
route without claiming a direct BEAM emitter, changing its selected Wasm
target, or mistaking BEAM processes for an OS sandbox. No Noble abstract-form
backend, admitted BEAM execution, or backend correspondence evidence exists.
r[EA-SCOPE-01] r[EA-FORMS-01]

## What Changes

- Add SPEC-BE003, a separate, nonselected `Erlang-Abstract-Draft` contract:
  independently accepted, resource-free Core-Bootstrap **subset** only.
  The first positive slice is finite signed `I64` literals and `+`, `-`, `*`,
  `=`; unsupported words and profiles refuse explicitly. A later full
  bootstrap target requires its own separately scoped acceptance.
  r[EA-SCOPE-01] r[EA-I64-01]
- Propose explicit `noble build SOURCE --target erlang-abstract --out DIR
  --contract CONTRACT_JSON --policy POLICY_JSON` and separately admitted
  `noble execute --target erlang-abstract --artifact DIR --input INPUT_JSON`.
  Keep existing `noble build --require-proof` and selected `noble run` /
  `session` unchanged. The host supplies complete expected interface, exact
  immutable environment and bounded policy; an effect-free first slice has
  no `test.emit` grant. r[EA-BUILD-01] r[EA-SCOPE-01]
- Lower checked operations to whitelisted, source-annotated Erlang abstract
  forms; use bounded ETF only as a transport for the list of forms, then
  `compile:noenv_forms(Forms, pinned_options)` under pinned OTP. Reject
  unapproved atoms, compiler attributes, parse transforms and ambient
  compiler options. An independent admission boundary binds exact source,
  candidate, forms, OTP output and policy before a disposable OS-confined VM
  may load any BEAM bytes. r[EA-FORMS-01] r[EA-ADMIT-01] r[EA-ISOLATE-01]
- Require finite actual compiled BEAM versus selected managed-linear-memory
  Wasm observations and separately recorded assumptions before any future
  runnable or production selection. r[EA-GATE-01]

## Impact

- **Files**: this change's SPEC-BE003 delta; new native
  `.cairn/specs/erlang-abstract-backend/spec.md`, registered family and
  `specs/conformance/erlang-abstract-cases.json`; supporting Cairn README,
  roadmap and status, plus regenerated views and requirement ledger.
- **Testing**: Cairn proposal/design/tasks gates and validation, sync preview
  and execution, document self-tests and scoped document tests only. Every
  new scenario stays absent/not-run/proof-open/trust-unassessed with no
  evidence; no OTP runtime was exercised by authoring this change. No
  compiler, loader, sandbox, historical receipt or proof is added.
