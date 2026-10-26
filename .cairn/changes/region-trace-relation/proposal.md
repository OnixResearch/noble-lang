## Why

Issue #9's accepted design separates guest requests, host responses and committed
effects and leaves its T1–T6 obligations open. Nothing yet relates that protocol
to the events an execution of the existing bounded-region subject actually
produces: the host reports several abnormal exits as one `adapter-trap` class,
cleanup can fail before a report is emitted, and host retirement is a commit
that happens after an abnormal exit. Trace-relating compilation correctness
supplies the discipline: define the source/target relation first and derive
which properties transfer, because non-identical traces do not preserve
arbitrary properties automatically.

r[WIT-TRACE-01] r[WIT-TRACE-02] r[WIT-TRACE-03] r[WIT-TRACE-04]
r[WIT-TRACE-05] r[WIT-TRACE-06]

## What Changes

- **ADDED** `WIT-TRACE-01`: source and target event alphabets, and an explicit
  relation for the existing synchronous bounded-region subject, covering request
  operation and arguments, exact resource identity, response value or error, the
  protected commit, return, the abnormal-exit class and the host-cleanup commit.
- **ADDED** `WIT-TRACE-02`: the stuttering/erasure discipline. Only identified
  internal ABI bookkeeping steps may be erased or stuttered, each with a proved
  justification; observable ordering, response-to-request pairing, resource
  identity and every protected commit are preserved; denials are not erased and
  host retirement is never converted into a guest release.
- **ADDED** `WIT-TRACE-03`: the preservation corollaries the accepted protocol
  needs: permitted ordered prefixes, no forged identity, no unexplained read
  commit, at-most-once release/retirement under declared assumptions, and a
  normal-return postcondition conditional on normal return.
- **ADDED** `WIT-TRACE-04`: quota/fuel boundaries through an explicit
  cost/accounting relation or a justified abstraction, preserving the committed
  prefix and the precise termination/cleanup status; an unrestricted "the target
  may trap anywhere" clause is refused.
- **ADDED** `WIT-TRACE-05`: a finite observable pilot at the existing WIT
  interface with the instrumentation needed to distinguish trap, quota and
  cleanup failure; absent observations stay unknown rather than being
  synthesized into a successful protocol outcome.
- **ADDED** `WIT-TRACE-06`: the correspondence route from the target model to the
  actual lowering/runtime steps with pinned source, where a theorem about two
  handwritten machines stays a model theorem and Wasmtime/Canonical-ABI
  assumptions stay visible.

This is a design change. It reopens no accepted design, adds no effect, and
claims no liveness, rollback, cancellation support or cleanup after host death.

## Impact

- Rust source subjects: none. No instrumentation is added by this package; the
  pilot names what instrumentation the evidence would require.
- Affected specs: `.cairn/specs/wit-wasi/spec.md` (delta only).
- Dependencies: #9's accepted design (`RegionProtocol-v1`) and #10's artifact and
  loading obligations stay authoritative; this package relates traces to them.
- Adoption: none. The pilot is opt-in evidence, not a gate.
