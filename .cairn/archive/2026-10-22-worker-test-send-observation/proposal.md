# WORKER-10 bounded send observation

## Why

WORKER-10 requires a guest that declares `test.send` and a host record that keeps requested, denied, admitted, completed and externally unknown operation outcomes apart under cancellation and output limits. No `test.send` effect, adapter or correlated host record existed. The only bound source effect that a guest could call after a denial was none: `test.emit` ends the invocation on any denial, and `test.code-export` always denies. Relabelling either one as `test.send` would change the case input. r[H-OUTCOME-02]

## What Changes

- Reserve kernel effect identity `EffId(7)` for `test.send` and register only an exact host-selected `worker-test-send@1.0.0` binding with `Text -- ! test.send`, a module-owned immutable slot, independent Wasm replay and typed emission. r[H-OUTCOME-02]
- Emit `noble/test_send_bound: (offset: i32, length: i32, adapter_slot: i32) -> i32` for an invoked bound operation. Status 0 means only that the host recorded the request: the guest learns no operation outcome. Any other status ends the invocation. r[H-OUTCOME-02]
- Add a host-only test embedder (`noble worker program test-host send-observe*`) that authorizes, admits, starts and observes each request through `noble_kernel::authority` against its own exact-plan grant table and a synthetic peer, and returns one bounded observation record with invocation and attempt correlation, kernel-retained outcomes, omitted scope and a bounded diagnostic. r[H-OUTCOME-02]

## Impact

- **Files**: kernel effect and registration, source declaration and checker, Worker WIT selection, Wasm plan, admission and emission, the CLI Worker host and its new `worker_send` record module, `verification/worker10/`.
- **Testing**: scoped kernel, contracts, Wasm and CLI suites; the WORKER-10 gate running all three variants and a changed-source control on the production binary; throwaway host mutations that hide truncation, report the unknown outcome as completed or erase rows on cancellation must fail the gate. r[H-OUTCOME-02]
