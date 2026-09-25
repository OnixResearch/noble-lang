<!-- Generated compatibility view. Edit .cairn/specs/safety/spec.md instead. -->
# Noble Safety Contract

Document: SPEC-S001  
Revision: 0.1.0-draft.5  
Project: noble  
Status: Canonical working safety contract; implementation and proofs remain absent  
Depends on: SPEC-0001 and SPEC-V001 at 0.1.0-draft.5

[DECISIONS.md](DECISIONS.md) records the selected direction. [EVIDENCE.md](EVIDENCE.md) defines independent implementation, execution, proof, and trust status.

## 1. Purpose

This document defines what Noble means by **language safety**. It strengthens the existing stack/type, effect, resource, host-boundary, and verification requirements without adding a guest `unsafe` sublanguage.

The target is:

> **Every accepted Noble program executes within defined semantics. Guest code cannot opt out of type, stack, memory, resource, effect, or authority safety. Operations requiring machine-level unsafety live outside the guest language behind validated host boundaries.**

Safety is intentionally separated from total correctness, availability, confidentiality of intentionally released information, distributed exactly-once behavior, and application-specific correctness.

## 2. Safety claim classes

Noble SHALL distinguish four claim classes.

| Class | Stable-platform intent |
|---|---|
| **Language safety** | Mandatory: type, stack, memory/representation, and defined-failure safety |
| **Resource and authority safety** | Mandatory: ownership accounting, non-forgeable live authority, checked host boundaries |
| **Standard concurrency safety** | Mandatory for the standard concurrency profile: no guest shared-mutable-memory races, scoped participant state, checked protocol/wire boundaries |
| **Behavioral correctness** | Optional stronger proofs: postconditions, termination, liveness, business invariants, protocol progress |

**S-CLAIM-01.** A release or report using the unqualified phrase **Noble-safe** SHALL identify the exact specification revision and MUST establish all applicable mandatory properties above for the supported subset. Unsupported features and unproved correspondence boundaries MUST remain visible.

**S-CLAIM-02.** Memory safety, type safety, and resource safety MUST NOT be used as shorthand for termination, deadlock freedom, network availability, confidentiality against authorized code, or exactly-once external effects.

## 3. No guest unsafe escape and no language-level undefined behavior

**S-LANG-01.** Noble source SHALL have no construct whose purpose is to disable or bypass the core stack/type checker, resource-eligibility rules, effect accounting, host authorization, or representation invariants. In particular, the standard language SHALL NOT provide a guest `unsafe` mode analogous to unchecked pointer or representation operations.

**S-LANG-02.** Raw machine addresses and unchecked native pointers SHALL NOT be ordinary Noble value types. An integer, byte sequence, text value, syntax node, artifact identifier, or program identity MUST NOT be reinterpret-able as a live resource or native address by guest code.

**S-LANG-03.** The defined semantic outcomes of accepted Noble execution MUST be limited to normal return, explicit modeled waiting/suspension where the execution profile permits it, divergence, a specified trap/abnormal termination, or a specified execution-profile failure such as quota exhaustion or cancellation. **Undefined behavior is not a Noble-language outcome.**

**S-LANG-04.** A bounds violation, invalid variant/tag, malformed external value, stale handle, wrong handle kind/context, or equivalent representation error MUST be rejected before entering trusted Noble state or produce a specified failure. It MUST NOT authorize memory corruption or arbitrary execution.

**S-LANG-05.** Operations that require native unsafety MAY exist inside the compiler, runtime, engine integration, or host adapters. They are outside Noble guest semantics and MUST be covered by explicit implementation boundaries, validated public wrappers, and the verification/trust policy.

## 4. Memory and representation safety

**S-MEM-01.** A conforming host MUST NOT expose a guest operation that permits arbitrary reads or writes to host/native memory by numeric address. Guest-visible memory operations MUST be mediated by typed values, bounded byte regions, validated component memories, or opaque resources with documented contracts.

**S-MEM-02.** Use-after-retirement, double release, forged handles, cross-context handles, and wrong-kind handles MUST NOT produce guest-visible memory unsafety. They MUST be rejected or produce a specified failure at the validated resource boundary.

**S-MEM-03.** Serialization/deserialization, Preserves decoding, portable-code loading, Wasm import/export adaptation, and FFI/host callbacks are hostile-input boundaries unless a stronger trusted provenance contract is explicitly established. Successful byte decoding alone MUST NOT establish a trusted Noble type, live resource, authority, recipe, or proof claim.

**S-MEM-04.** Compiler/runtime representation choices MAY use indexes, pointers, arenas, tables, garbage collection, reference counting, linear ownership, or another strategy internally. Those choices MUST preserve the abstract Noble invariants and MUST NOT be observable as permission for guest representation casts.

### 4.1 Byte views and semantic acceptance

**S-DECODE-01.** Byte-layout checks MUST remain separate from semantic acceptance, artifact correspondence, and authority checks. A decoded representation is untrusted candidate data until the relevant semantic checks pass. Public constructors and deserializers MUST NOT manufacture accepted state from layout validity alone.

Zerocopy is an optional adapter candidate, not a required dependency. `FromBytes` and `TryFromBytes` concern Rust representation validity. They do not establish Noble schemas, effects, recipes, proofs, or live authority.

**S-DECODE-02.** A borrowed byte view MUST retain valid, stable backing storage for its entire use. Across callbacks, suspension, or shared-memory mutation, the adapter MUST copy unless a reviewed ownership contract prevents invalidation. Bounds, offsets, arithmetic overflow, and total work MUST be checked before allocation or traversal.

A Rust `Immutable` marker does not freeze external storage. A copy also requires a safe, bounded read from its source. Copying a header does not stabilize a separately mutable payload.

**S-DECODE-03.** A portable binary format MUST define integer encodings, byte order, tags, padding treatment, versioning, and limits explicitly. Native layouts and `IntoBytes` output MUST NOT define canonical program identity implicitly. Unsupported versions and malformed semantic fields MUST be rejected.

Preserves remains the selected protocol direction. A local binary-layout experiment does not select a new interchange format.

## 5. Type, stack, and effect safety

**S-TYPE-01.** An accepted program executing with conforming host bindings MUST NOT reach a stuck state from wrong operand type, language-level stack underflow, invalid stack shape, or use of a consumed resource. These states are not permitted dynamic fallbacks.

**S-TYPE-02.** `Program<S,T,e>` remains the executable interface. `run` MUST only execute a checked/prepared `Program`; `Syntax`, arbitrary bytes, untrusted artifacts, or claimed manifests MUST NOT be directly runnable.

**S-EFFECT-01.** For every finite execution prefix, every guest-requested host operation MUST be included in the instantiated effect bound of the executing computation.

**S-EFFECT-02.** A program with an empty effect bound MUST NOT issue a guest host request. This does not imply termination, bounded resource use, or freedom from runtime housekeeping outside guest effects.

A denied request still counts as a request under S-EFFECT-01. Static effect rejection and forged-metadata admission rejection require zero candidate-body host requests. Runtime authority denial is a separate case whose declared effect bound already includes the requested operation. See [safety scenarios](conformance/safety-cases.json).

**S-EFFECT-03.** Effect information is descriptive, not authoritative. A type/effect check, `Program` possession, program identity, or successful proof MUST NOT create host authority.

## 6. Resource and capability safety

This section extends the baseline move-only resource discipline.

**S-RES-01.** A live resource/capability has an explicit ownership obligation in Noble. Generic duplication, generic discard, capture by `quote`, persistence, and serialization MUST remain unavailable for resource-bearing values unless a separately specified operation has semantics that preserve the resource contract.

**S-RES-02.** Live authority MUST be non-forgeable from ordinary guest data. A resource may enter guest execution only through an authorized host operation, a checked transfer from another ownership context, or another profile-defined validated authority boundary.

**S-RES-03.** Host resource tables MUST validate at least resource kind, owning/authorized context, liveness/generation, and operation-specific rights before acting on an incoming guest handle representation.

**S-RES-04.** Resource-containing aggregates carry the ownership obligation recursively and MUST NOT satisfy `Data` or `Capture` merely because a particular runtime variant does not currently expose the resource.

**S-RES-05.** Abnormal termination and cancellation MUST retire invocation-owned local handles according to the execution profile without requiring guest code to resume. This local cleanup guarantee MUST NOT be overstated as exactly-once cleanup of remote external effects.

## 7. Standard concurrency safety

The standard concurrency profile is based on the Syndicated Actor Model / Syndicate/Synit direction selected for Noble, with Preserves as the de facto protocol/schema interchange layer. Concurrency remains outside the language kernel.

**S-CONC-01.** Concurrent Noble participants in the standard profile MUST NOT obtain direct shared mutable Noble memory. Cross-participant interaction occurs through immutable data, typed dataspace assertions/interests/messages, explicit host operations, or profile-defined ownership transfer of resources.

The selected M7 profile MUST reject an actual submitted component/core module
requesting shared mutable guest memory before creating a facet, starting a
guest import or granting a participant. The selected component loader MUST
also reject that shared-memory module. Admission MUST bind the rejection
decision to inspected module memory facts, not only a caller-supplied Boolean
or the observation that separately instantiated Stores did not share memory.

**S-CONC-02.** The absence of guest shared mutable memory SHALL imply **Noble-visible data-race freedom** for the standard concurrency profile. This claim does not automatically prove the Rust runtime itself race-free; runtime synchronization must separately refine the abstract concurrency model.

**S-CONC-03.** Assertions, interests, reactions, and subordinate conversational activity MUST have explicit owners/scopes. Facet/actor termination SHALL retract or retire scope-owned conversational state according to the standard profile, including failure paths.

In the selected M7 local synchronous service, the host binds each participant
instance to a facet and checked rights; guest-provided names and protocol bytes
are not facet identities. An active facet owns its assertions, interests, and
children. Normal exit and trap MUST retire its descendants, assertions and
interests once, without requiring another guest call or retaining an assertion
as visible after its owner has terminated. A different active observer's
matching interest can observe the local retraction. This is local scope
cleanup, not remote rollback or exactly-once external cleanup.

**S-CONC-04.** Network or process-boundary protocol data MUST be decoded under bounded resource limits and validated against its Preserves schema/protocol before it becomes a typed Noble protocol value. Preserves embedded/native extension mechanisms MUST NOT by themselves create Noble resources or authority.

The M7 adapter accepts only the selected, canonical Preserves text subset for
`service(name:Text,ready:Bool)`: `<service "NAME" #t>` or
`<service "NAME" #f>`, with exactly the shown delimiters and spaces, and an
ASCII `NAME` of one to 32 characters from `[A-Za-z0-9_-]`. Admission MUST
enforce at most 64 input bytes and nesting depth four, reject malformed UTF-8,
overlong input, excess nesting, noncanonical spelling, escaped names,
annotations, embedded/native extensions, trailing content and schema
mismatches, and create a typed `Service` only after all checks succeed.
Encoding an admitted `Service` MUST emit that same canonical subset and
respect the same byte and depth limits. The source
[Preserves: Text Syntax, v0.996.3 (June 2025)](https://preserves.dev/preserves-text.html)
defines records as `<` followed by label `Value` and field `Value`s then
`>`, permits bare symbol `service`, JSON-style strings and `#t`/`#f`
Booleans; it also permits syntax this profile intentionally rejects.
These limits bound the selected adapter's admitted input and traversal; they
are not a heap bound or general Preserves implementation.

**S-CONC-05.** Typed protocol validity and runtime authority are distinct. A participant may only publish, observe, message, or transfer what both its static protocol interface and its runtime capability permit.

For the selected two-participant service, the publisher's assertion and the
subscriber's exact-pair interest belong to distinct host-bound facets.
Publication and observation require separate checked facet rights. The
`observe(name,ready)` result is exact assertion membership, not the `ready`
field: querying a missing `(name,false)` returns false; querying a published
`(name,false)` returns true. A valid wire value or WIT signature cannot supply
the rights, impersonate another facet, or turn an untrusted resource-shaped
payload into a resource.

**S-CONC-06.** Deadlock freedom, global progress, fairness, and service availability MUST remain separate claims. They require explicit protocol/profile assumptions or proofs and are not implied by race freedom or type safety.

The bounded M7 local synchronous service promises serialized local decisions,
not progress for absent participants, fairness, transport delivery, durability,
distributed exactly-once publication, or native-async task semantics.

**S-CONC-07.**

The selected M7 dataspace SHALL serialize local facet creation, checked
interest registration, assertion publication, exact-pair observation,
retraction and facet termination. Only an active host-bound facet with the
applicable runtime right may mutate its owned state; an invalid owner, retired
facet, failed preflight or exceeded finite capacity MUST leave retained state
and its visible assertions unchanged. The selection admits at most eight
facets, eight assertions, eight interests and 32 retained events; capacity
for a state change, immediate notifications and every already-owed future
removal notification MUST be reserved before commitment. This reservation
MUST leave enough event capacity for mandatory facet retirement even when no
further guest call can run; retirement MUST NOT be refused solely because
the event buffer later becomes full.
`observe` registers one interest per `(facet,name,ready)` idempotently even
when that assertion is absent. A registered exact-pair interest belongs to
its observer facet and records a local add when a matching assertion becomes
visible and a local remove when it ceases to be visible. Publishing the same
`(facet,name,ready)` again is idempotent; publishing changed `ready` for the
same `(facet,name)` atomically removes the prior assertion and adds its
replacement after preflight. `retract(name)` removes only that facet's
assertion and reports whether one existed. The host MUST distinguish these
events from an `observe` membership return and MUST NOT report a stale
queued add as a currently live assertion after retraction.

| Local operation | Required state and rights | Serialized outcome |
|---|---|---|
| Open facet | New root or live parent; free facet capacity | Active host-bound facet, child owned by parent when present |
| Observe `(name,ready)` | Active facet with observation right; reserved interest/event capacity | Idempotent exact-pair interest and current membership Boolean |
| Publish `(name,ready)` | Active facet with publication right; validated service and reserved assertion/event capacity | One owned assertion per `(facet,name)`; identical publish does nothing, changed readiness removes old then adds new |
| Retract `name` | Active facet with publication right | Remove only its owned assertion and return true, or preserve state and return false if missing |
| Retire facet, including trap | Active facet and its descendants | Remove descendant interests and assertions once, emit owed removals to still-live matching observers, retire the subtree without guest resumption |

An unadmitted operation, foreign facet, revoked right or retired facet MUST
NOT mutate the dataspace, create a notification or transfer ownership.
Notifications are local serialized observations, not remote delivery
guarantees.

For the first service scenario, two separately compiled Noble component
instances share no mutable Noble memory. A publisher facet publishes
`service(name:Text,ready:Bool)`; a subscriber facet registers an exact-pair
interest and observes publication. A trap after publication terminates the
publisher facet and retracts its assertion, while the still-live subscriber
observes removal. Both `ready=true` and `ready=false` MUST be exercised as
distinct exact-pair assertions. The abstract local transitions do not assert
that a trap rolls back an admitted operation or that a remote peer receives an
event exactly once.
**S-CONC-08.**

The selected Preserves subset adapter MUST reject inputs exceeding 64 bytes,
nesting depth four, the declared service schema or the exact canonical
encoding before creating a typed protocol value. Its input traversal and
output encoding MUST be bounded by those limits, with overflow and UTF-8
validation before allocation or typed admission. An authenticated host-bound
facet and its rights remain distinct from decoded `Service` data. A payload
field, embedded/native extension, or successful byte roundtrip MUST NOT
manufacture a facet, WIT resource, authority or proof claim. A positive
roundtrip MUST exercise actual published values from compiled Noble
components, not only a host-only test vector.
## 8. Host and artifact boundary safety

**S-HOST-01.** Every public boundary from unverified Rust, FFI, Wasm, external wire data, connector/network input, or host callback into verified/runtime-internal state MUST validate all preconditions not already guaranteed by a proven caller relation.

**S-HOST-02.** Host adapters MUST validate their outputs before making them available as trusted Noble values. A malformed native adapter result is an implementation/binding defect and MUST NOT be accepted as if the guest type system had proved it.

**S-HOST-03.** A valid Wasm artifact, digest, signature, recipe hash, source identity, or attached proof object is not by itself authorization and is not by itself sufficient evidence of source/recipe correspondence. Loading policy MUST separately establish artifact validity, interface compatibility, provenance/correspondence policy, and concrete runtime authority.

**S-HOST-04.** Unsafe/native implementation code MUST be kept behind narrow reviewed boundaries. The project SHALL inventory such boundaries for any release that advertises the corresponding safety claim.

## 9. Verification obligations

The reviewed Lean model remains the reference mathematical model for the safety claims. The existing distinction between language metatheory, Rust implementation refinement, optional program proofs, and source-to-Wasm correspondence remains mandatory.

The safety workstream adds these theorem/implementation obligations:

| Obligation | Required claim |
|---|---|
| **SO-01** | Accepted core executions have no forbidden stuck state and no undefined-behavior semantic outcome |
| **SO-02** | Representation/eligibility rules prevent resource or authority forgery through ordinary data |
| **SO-03** | Effect soundness covers every finite execution prefix, including denied/failed requests |
| **SO-04** | Whole-configuration resource ownership is preserved modulo declared host transitions |
| **SO-05** | Checked preparation cannot turn `Syntax`/untrusted metadata directly into an executable interface without validation |
| **SO-06** | Public host/resource wrappers establish all verified internal preconditions from hostile representations |
| **SO-07** | The selected Noble-to-Wasm lowering preserves the safety-relevant observation and failure relation for the claimed subset |
| **SO-08** | Standard concurrency semantics preserve participant isolation, scope-owned conversational state, protocol typing, and capability checks |
| **SO-09** | The concrete concurrency/runtime state machinery refines SO-08 under its documented synchronization assumptions |

**S-VERIFY-01.** A stable-core safety claim MUST include reviewed Lean-kernel-checked evidence for the complete subset advertised as stable, together with the actual acceptance-checker correspondence required by SPEC-V001.

**S-VERIFY-02.** An end-to-end Noble-safe executable claim requires evidence for the relevant checked-source-to-artifact and loader/runtime boundaries, or those boundaries MUST be explicitly listed as trusted assumptions.

**S-VERIFY-03.** No proof result may convert an unsupported feature, timeout, solver unknown, runtime validation failure, or unverified boundary into a safety success.

## 10. Required safety outcomes and diagnostics

The implementation SHALL distinguish at least:

- static parse/resolution/type/stack rejection;
- resource/eligibility rejection;
- effect-bound rejection;
- authority denial;
- malformed/untrusted input rejection;
- stale/forged/wrong-context resource rejection;
- ordinary domain `Result` errors;
- specified guest trap/abnormal termination;
- execution budget/quota/cancellation failures;
- implementation/binding defect reports.

These categories MAY share user-facing presentation, but they MUST NOT be collapsed internally in a way that converts a safety failure into successful execution.

## 11. Explicit non-goals of the word “safe”

The mandatory safety claim does not by itself establish:

- termination or total correctness;
- deadlock freedom or distributed progress;
- availability of a host, network, filesystem, or remote service;
- confidentiality against code that legitimately possesses authority to disclose data;
- constant-time execution or side-channel freedom;
- exactly-once remote cleanup or business effects;
- correctness of application/business logic;
- correctness of Wasmtime, rustc, the OS, hardware, cryptography, or unproved host adapters.

Each such property requires a separately named model, contract, profile, or proof.

## 12. Stable safety release gate

**S-GATE-01.** A stable Noble core SHALL NOT be advertised as Noble-safe until the stable subset has: (a) executed conformance tests covering the safety families, (b) reviewed Lean safety metatheory, (c) an actual acceptance checker related to that model, (d) explicit host-boundary validation policy, and (e) a published trust/assumption ledger.

**S-GATE-02.** A standard-concurrency Noble-safe claim MUST additionally include the profile's isolation/scope/protocol/capability invariants and a concrete runtime correspondence result or explicitly disclosed trusted-runtime boundary.

**S-GATE-03.** Future advanced type-system features—including dependent types, higher-rank polymorphism, principled subtyping, refinement facilities, or user-extensible inference—are not rejected by this safety contract. They MAY be added only with rules that preserve or deliberately strengthen the safety invariants and with corresponding proof obligations before joining a stable-safe subset.
