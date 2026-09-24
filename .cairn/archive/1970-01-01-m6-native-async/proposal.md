## Why

M5 supplies synchronous components, resource retirement and protected host
admission. M6 must extend those contracts across actual native Component Model
suspension, future/stream transfer and cancellation without changing Noble's
sequential source semantics or substituting a host-only model.

## What Changes

- Compile direct-style async WIT calls and typed live future/stream boundary
  values, preserving move-only eligibility and source effect order.
- Add a bounded, serialized production Rust lifecycle with complete explicit
  state/event coverage, terminal reservations, ownership accounting and late
  native retirement.
- Couple one-shot authorization and input transfer at admission, retaining
  separate invocation outcomes and authenticated operation receipts.
- Select ABI, driver, buffering and interruption bounds through executed
  compatibility work; retain source-bound independent-peer and refinement evidence.

## Impact

- **Files**: kernel async ownership, existing component frontend/compiler/CLI,
  selected WIT, independent Rust peer, M6 proof/evidence records, exact source
  and tool inventories, canonical profile contracts and supporting status.
- **Testing**: compiled WI-11/WI-12/WI-16 matrix; applicable WORKER-01/08/09
  controls; both cancellation/delivery orders; resource-bearing results,
  domain errors, quota/abnormal exits, blocking native work and runnable Wasm;
  complete schema-bound lifecycle coverage; actual Charon/Aeneas/Lean
  correspondence; M5 and earlier regressions; unchanged deny-all/architecture
  gates and every declared Nix check.
- **Non-goals**: general async borrowing, all WASI or Component-Draft
  interfaces, guest concurrency syntax, MW1/MW2 completion, Syndicate
  implementation, distributed retries or universal backend/host refinement.
