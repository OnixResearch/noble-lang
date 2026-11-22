# GUARD-CASE acceptance gate

`gate.mjs` builds the production `noble` binary and a `live-test-barrier` build from a clean
checkout, then runs GUARD-CASE-01..12 (`specs/conformance/guard-cases.json`) through
`noble companions` sessions and the GUARD-CASE-02 core control. The receipt binds the source
snapshot, both binaries (hashed before and after the run), the case projection and every raw
output it wrote, with each file's sha256. Promotion verdicts (`promotable`, `unmet`, `failed`)
are derived from the observations: a case is promotable only when every registered expectation
was observed. A failing case still writes the receipt, with the failure. It promotes nothing by
itself.

GUARD-CASE-11 runs under the harness fault model of owner decisions 33 and 33a: the
`live-test-barrier` build can exhaust or trap exactly one guard execution, selected by the
SHA-256 of its checked source. The shipped binary neither defines nor forwards the hook.

## Retained runs

| Run | Command | Result |
|---|---|---|
| [`runs/20261122-b4`](runs/20261122-b4) | `node verification/guard-cases/gate.mjs --out <new dir>` at commit `f5c4ccec` (Cairn batch-4 tree) | `completed` / `incomplete`; GUARD-CASE-01, 02 and 04-12 promotable; GUARD-CASE-03 unmet on its tail clause only, a gate reading error (below) |
| [`runs/20261122-b4r`](runs/20261122-b4r) | `node verification/guard-cases/gate.mjs --out <new dir>` at commit `db0fe933` (the same tree with the gate's stack-cell reading fixed) | `completed` / `all-passed`; GUARD-CASE-01..12 promotable |

The binaries are not retained; their digests are in each receipt, and both runs executed the
same `noble` binary (`587b3b71...`) and fault build (`d2ec6346...`). In the first run the gate
read a stack cell's `value` field, but a cell carries its value as the JSON text of `detail`, so
GUARD-CASE-03's `I64 41` tail read as absent before and after the invocations; its transcript
keeps the tail at the bottom of all 21 stacks. `db0fe933` reads cell values from `detail`.

The promoted case records cite [`runs/20261122-b4r/acceptance.json`](runs/20261122-b4r/acceptance.json)
(sha256 `da95ab5a...`, tested commit `db0fe933`, source revision `sha256:cc9dda44...`). Proof
stays open for every case: C1 for `templates_of`, C6 (Wasm lowering of the wrappers) and PO-21
are separate obligations.
