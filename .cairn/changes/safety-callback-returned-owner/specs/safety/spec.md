# Callback-returned resource ingress

## MODIFIED Requirements

### Requirement: S-MEM-02
r[S-MEM-02]

Use-after-retirement, double release, forged handles, cross-context handles, and wrong-kind handles MUST NOT produce guest-visible memory unsafety. They MUST be rejected or produce a specified failure at the validated resource boundary. For selected S-CASE-08, `Host::new` MUST pre-register a live pending authentic owner plus only the selected mode's necessary fixture after recipe admission. `tokens.issue` MUST select an actual retained host-table claim, not create a new owner or fabricate a label: stale-generation names a genuinely retired earlier generation of the pending owner's reused slot; wrong-kind names one other live owner of a different kind; wrong-context names one other live owner in a distinct bounded, nonwrapping context. Authentic mode needs no extra fixture. The host MUST reject each hostile claim before construction of a Wasmtime `ResourceAny` or trusted Noble owner, before `tokens.consume` or protected work, and MUST settle the pending owner exactly once and any selected fixture separately without leaving live owners or native pins. A matching slot/generation in a distinct context or table namespace does not authorize that owner.

#### Scenario: S-CASE-08 real hostile returned-owner matrix

- GIVEN the exact absent/not-run `Resources-Draft` S-CASE-08 input `callback-handle-matrix` with `stale-generation`, `wrong-kind`, `wrong-context` and the matching compiled guest/WIT recipe
- WHEN each real host-table claim returns from the actual `tokens.issue` callback at the production owned-result boundary
- THEN each mode reports `callback/invalid-result`, `trusted_values_created:0`, zero protected operations, one guest request and one settled invocation owner, with zero live owners and native pins; a separate authentic claim reaches `tokens.consume` once

### Requirement: S-HOST-01
r[S-HOST-01]

Every public boundary from unverified Rust, FFI, Wasm, external wire data, connector/network input, or host callback into verified/runtime-internal state MUST validate all preconditions not already guaranteed by a proven caller relation. For selected S-CASE-08 `noble component callback-owner COMPONENT MODE WIT WORLD EXPORT=SOURCE`, the host MUST independently prepare/compile the complete invoker-selected `noble-test:callback-owner/bounded@1.0.0` WIT/world/export-source recipe and byte-match the candidate component before `Host::new` registers its mode-selective owner fixture or Store/guest execution. A recipe mismatch MUST refuse at admission with zero guest requests and no created owner, not masquerade as a callback-stage invalid result. At `tokens.issue`, the returned owner claim MUST pass retained table identity/slot/generation/kind/context/liveness/right validation before any trusted resource construction; no unverified callback assertion, numeric slot or guest result can replace those checks.

#### Scenario: Matched recipe enters the real callback boundary

- GIVEN the complete independently host-selected WIT/world/export-source recipe and a byte-identical compiled Noble component containing `tokens.issue tokens.consume`
- WHEN the production host invokes each S-CASE-08 callback mode and separately tries a candidate with a mismatched recipe
- THEN actual matching runs reach the `tokens.issue` host ingress for validation, while recipe mismatch refuses before owner creation or guest requests and is not classified as the canonical invalid result

### Requirement: S-HOST-02
r[S-HOST-02]

Host adapters MUST validate their outputs before making them available as trusted Noble values. A malformed native adapter result is an implementation/binding defect and MUST NOT be accepted as if the guest type system had proved it. In the selected `noble-test:callback-owner/bounded@1.0.0` `issue() -> own<token>` result, the production host MUST check its mode-selected retained table-backed claim against the expected token kind, table namespace/slot, current invocation context, generation, liveness and rights **before** calling Wasmtime's owned-resource constructor (`ResourceAny`) or incrementing `trusted_values_created`. The three hostile returned-owner modes MUST fail at `callback/invalid-result` from that host check with zero trusted values; a host-returned error string, guest result, late trap or postconstruction rejection is not equivalent. Authentic issue MUST create exactly one trusted guest owner, and only that owner can be consumed in the second guest callback request. Reports use `noble-callback-owner/v1`; hostile modes each account `guest_requests:1`, `protected_operations:0`, `released_owners:1`, `fixture_releases:1`, `live_owners:0`, `native_pins:0`, while authentic accounts `guest_requests:2`, `protected_operations:1`, `released_owners:1`, `fixture_releases:0`, `live_owners:0`, `native_pins:0`. The fixture release is stale setup retirement or one live wrong-kind/context fixture cleanup, never an additional release of the pending authentic owner.

#### Scenario: Host output checked before resource construction

- GIVEN mode-selective real table states for the same compiled guest: authentic pending owner alone, retired earlier generation plus pending owner, or pending owner plus one live wrong-kind or foreign-context fixture
- WHEN the `tokens.issue` host callback returns its owner claim to the Wasmtime resource conversion boundary
- THEN three hostile modes reject before trusted construction and the authentic mode creates one trusted owner that reaches `tokens.consume`; every report retains independently measured request, operation and owner/pin settlement counters
