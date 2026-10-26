# Host-authorized exact filesystem read

## MODIFIED Requirements

### Requirement: S-EFFECT-03
r[S-EFFECT-03]

For the selected bounded filesystem profile, the host MUST derive an `authorization/denied` report solely from its retained resource-Table READ-right failure. A guest-returned `denied` error string, even from a source-matched component, MUST NOT create that host authorization observation. The actual request trace and protected-operation count remain independent observations.

Effect information is descriptive, not authoritative. A type/effect check, `Program` possession, program identity, or successful proof MUST NOT create host authority. For the selected S-CASE-06 `noble-test:authorized-fs/bounded@1.0.0` profile, the host MUST independently prepare and compile the complete invoker-selected WIT/world/export-source recipe and byte-match the candidate component before opening any file or creating a Directory owner/grant; a mismatched recipe rejects at admission with zero guest requests, not as S-CASE-06 denial. The actual matched compiled `"main.rs" fs.read` guest MUST issue exactly one read import even when a live host-owned invocation Directory has no READ grant. The host MUST classify that request as `authorization/denied`, count it in the `fs.read` effect trace and perform zero protected file-content reads. The host invoker preopens only one vetted regular file, mapped under the exact key `main.rs`; the Directory is a **logical host table owner over that File**, not a preopened OS directory. Neither the guest's source effect nor its path grants permission; an independently host-granted READ is required for a positive read of the same compiled guest. This selected finite adapter does not establish general filesystem authority or native filesystem safety.

#### Scenario: S-CASE-06 actual request without a READ grant

- GIVEN the exact canonical `Resources-Draft` input `"main.rs" fs.read`, the declared `fs.read` effect and a live host-owned `Directory` in `invocation-1` with `read_right:false`
- WHEN the compiled Noble component calls the production Wasmtime `fs.read` import against the host's exact preopened-file map
- THEN denial occurs at `authorization` after one counted guest `fs.read` request with zero protected operations and zero file-content reads; a separately authorized run of the same compiled guest returns only the selected file's bounded bytes

### Requirement: S-RES-03
r[S-RES-03]

Host resource tables MUST validate at least resource kind, owning/authorized context, liveness/generation, and operation-specific rights before acting on an incoming guest handle representation. The selected S-CASE-06 Directory owner is instead retained entirely by the host, not supplied through a guest WIT resource representation. Its table identity, invocation context, kind, generation and liveness MUST still be checked with the independently selected READ right and exact `main.rs` map membership before the host reads the preopened file. A live owner alone grants no READ; a guest path or effect claim cannot create the missing right. Forged, stale, foreign-context, wrong-kind, retired or no-right host claims and non-mapped paths MUST perform zero protected content reads. Host-injected handle probes are controls, not a claim that the WIT exposes a Directory handle.

#### Scenario: Host-retained Directory and hostile claims

- GIVEN a host-preopened file and live per-invocation Directory owner, with separately varied right, table identity/context/kind/generation and guest path
- WHEN the same production host read boundary validates those facts before accessing file bytes
- THEN no invalid or ungranted claim reads the file, while the exact owner/path/READ combination can return only bytes from the vetted file

#### Scenario: Candidate/recipe mismatch precedes host authority

- GIVEN a candidate compiled from `"main.rs" fs.read` and a distinct host-selected WIT/world/export-source recipe
- WHEN the production `read-fs` route independently compiles that complete recipe and compares final candidate bytes before opening the configured file
- THEN the mismatch rejects at admission with status 2, zero guest requests and no Directory owner or READ grant created; it cannot count as the canonical authorization-stage denial
