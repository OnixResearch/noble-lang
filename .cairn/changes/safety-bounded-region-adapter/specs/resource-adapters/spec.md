# Bounded immutable region adapter

## MODIFIED Requirements

### Requirement: RA-SCOPE-01
r[RA-SCOPE-01]

Implementations of `Component-Sync-Bootstrap` MUST support synchronous WIT imports/exports and owned resources. Borrow support is limited to adapter-local borrows for imported calls. This is not full `Component-Draft` conformance.

The subset excludes borrowed exports, borrowed values in guest storage, and suspension during a borrowed call. It adds no source borrow type or general lifetime-polymorphic program.

The delivered M5 slice uses the pinned [`noble-test:sync/bootstrap@1.0.0` world](../../../../../crates/noble-wasm/wit/bootstrap.wit). Owners are move-only and noncapturable; imported borrows use adapter-local tokens, not guest values or serializable authority. Normal success and domain errors return the borrowed owner once. Owned transfer instead commits only after preflight and does not implicitly return ownership on a domain error.

The [M5 runtime gate](../../../../../verification/m5/gate.mjs) exercises the production resource table and an independent Rust/Wasmtime 40.0.2 component peer. Its bounded local cancellation and unexpected-suspension control revokes guest access, retains the native pin through actual guest GC, and handles completion, duplicate callbacks and repeated retirement without resurrection or a second release. This control is not native async execution. The peer independently links and converts component values but reuses Noble's resource/authority decision libraries for host policy.

The [strict resource proof lane](../../../../../proofs/m5/M5Resources.lean) relates the extracted Rust transition and checked table-decision functions to their logical transitions, with separate owner-return, busy-owner, cancellation-pin and late-completion properties. The [extraction audit](../../../../../verification/m5/extraction.mjs) binds those seven theorem roots to actual Rust definitions; broader authority and component body/dependency coverage is not refinement. Native release, authenticated host facts and callbacks, and engine/ABI correctness remain external assumptions. Fresh complete extraction/check receipts, not these descriptions or a diagnostic proof audit, determine milestone acceptance.

In addition to the existing M5 bootstrap world, the selected `noble-test:bounded-region/bounded@1.0.0` world provides only an imported `region.read(s64,s64)->result<list<u8>,string>` resource method and explicit `regions.release(own<region>)`, with an exported `read-region(own<region>,s64,s64)->result<list<u8>,string>`. The host invoker alone selects up to 4096 immutable region bytes and creates its owner with context and read right before guest invocation. Distinct simultaneously live host Stores MUST use different TableId and Context identities even if slot and generation coincide; the host allocates identities monotonically without wraparound and fails closed on exhaustion. The borrowed method MUST restore that same owner on both normal result branches and the guest MUST release it once; abnormal exit remains a host cleanup obligation. This does not grant guest allocation, mutation, arbitrary address reads or implicit capability conversion from byte lists.

#### Scenario: S-CASE-02 versioned resource call

- GIVEN a host-selected `00 01 02` region registered for one component invocation and an actual compiled Noble guest whose method borrow is adapter-local
- WHEN the guest enters `read-region` with the canonical offset and length and separately with valid in-bounds controls
- THEN the canonical request is refused before protected access, valid reads return only bytes in the selected region, and ownership returns from borrow and is released or host-retired exactly once

### Requirement: RA-STATE-01
r[RA-STATE-01]

A `Live` to `Busy` transition MUST validate kind, context, generation, and rights before protected work. It consumes guest availability atomically with scope registration. In the selected bounded-region adapter the host MUST also validate signed nonnegative and overflow-free offset/length against the retained byte extent and output quota before taking that borrow/native pin. The host MUST retain the resource through `begin`, validate native access, read only the checked range and `complete` it before the guest's explicit release; invalid arithmetic or claims MUST leave the protected read count at zero.

#### Scenario: S-CASE-02 no protected work on refusal

- GIVEN the exact three-byte host region and the canonical `(3,1)` request, plus forged, cross-context, wrong-kind and retired owner controls
- WHEN the same production adapter validates the owner and request before read
- THEN no rejected request reaches a protected region read or acquires a native pin, and a valid `(1,2)` read returns its borrowed owner before exactly one release
