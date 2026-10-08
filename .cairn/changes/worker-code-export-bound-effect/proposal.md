# WORKER-12 declared code-export effect adapter

## Why

The WORKER-12 design names `test.code-export`, but an inert effect string or a WIT request alone cannot establish that source independently checked a concrete operation. A real declared source adapter must retain its exact Text capture/destination inputs and effect through kernel acceptance and Wasm emission while denial remains independent of source typing. r[H-HOST-02]

## What Changes

- Select a host-owned `worker-test-code-export@1.0.0` bound operation with exact `Text Text -- ! test.code-export` source contract and reserved effect identity. Require the checked owner, exact immutable adapter slot, source dependency, and independent kernel/backend admission before emission. r[H-HOST-02]
- On actual invocation, emit the `noble/test_code_export_bound` Core import `(param i32 adapter_slot) (result i32 status)` only for an invoked bound definition. A denial request receives no Text bytes or pointer, leaves the Wasm invocation poisoned even if the host returns zero, and never implicitly authorizes serialization or publication. r[H-HOST-02]
- Keep WORKER-12 unpromoted until an independently assembled real guest/host scenario proves all selected observations. This source/kernel/Wasm slice neither creates a positive exporter nor changes Worker v1, WIT component ABI, or CLI grants. r[H-HOST-02]

## Impact

- **Files**: scoped source declaration/checker, kernel effect/registration, managed-Wasm source emission and admission, the existing WORKER-12 requirement as an active delta.
- **Verification**: exact source fixture and forged signature/owner/effect refusals, emitted typed import and no-Text denial wrapper, Cairn document validation; actual WORKER-12 scenario execution and promotion remain separate. r[H-HOST-02]
