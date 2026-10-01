# S-CASE-06 runtime authorization evidence

## MODIFIED Requirements

### Requirement: EV-CASE-03
r[EV-CASE-03]

Authority denial requires a declared effect that includes the request. A denied request counts toward the effect trace. Denial MUST NOT substitute for static effect rejection. To promote S-CASE-06, a new immutable source-bound prepromotion acceptance MUST show independent preparation/compilation of the host-selected complete WIT/world/export-source recipe and exact component-byte correspondence **before** preopening the file or establishing Directory/READ authority. It MUST show the matched actual `"main.rs" fs.read` Noble component invoking production Wasmtime once with the canonical live host-owned **logical** `Directory`, `read_right:false`, `stage:authorization`, `outcome:denied`, `guest_requests:1`, actual bounded callback `request_trace:["fs.read"]`, `protected_operations:0` and no protected file-content reads. A separately READ-granted run of the **same compiled guest bytes** MUST show exact bounded bytes read from the vetted host-preopened regular File and owner retirement, with hostile path/owner/rights refusals before any protected content read. A mismatched source/WIT recipe MUST refuse with status 2 at admission before file open/owner creation, and MUST NOT masquerade as the canonical one-request denial. An oversized granted file MUST instead be labeled a bounded postauthorization failure and MUST NOT be misreported as zero-read authorization denial. Source, pinned tools, WIT/guest/CLI and fixture identity, command outputs and assumptions MUST be bound; structural/document validation, static rejection, a synthetic callback, and a prior fixture receipt are not substitutes. Only after passing prepromotion acceptance MAY native deltas sync and canonical state/evidence/views change; postpromotion documents/Cairn checks are separate. Proof including owner law, Octet and Aeneas remains open.

#### Scenario: S-CASE-06 prepromotion runtime acceptance

- GIVEN the exact unpromoted canonical S-CASE-06 record, source-bound compiled guest, host-preopened file and independent READ-or-none grants
- WHEN the denied and granted invocations and hostile controls execute against the production host before any case promotion
- THEN the denial matches every canonical expected field after one real request and zero protected reads; the granted control returns only exact vetted bounded bytes, owner obligations settle, and distinct prepromotion and postpromotion receipts remain bound to their respective source states
