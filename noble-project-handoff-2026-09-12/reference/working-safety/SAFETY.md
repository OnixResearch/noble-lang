# Noble Safety Contract

Document: SPEC-S001  
Revision: 0.1.0-draft.1  
Date: 2026-09-12  
Project: noble  
Status: Working safety specification; normative goals selected, implementation/proofs not claimed  
Baseline: SPEC-0001, 0.1.0-draft.1, with verification extension SPEC-V001  
Surface terminology: uses the selected `run` and `reflect` names; older drafts spell these `call` and `reify`

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

**S-LANG-03.** The defined semantic outcomes of accepted Noble execution are limited to normal return, explicit modeled waiting/suspension where the execution profile permits it, divergence, a specified trap/abnormal termination, or a specified execution-profile failure such as quota exhaustion or cancellation. **Undefined behavior is not a Noble-language outcome.**

**S-LANG-04.** A bounds violation, invalid variant/tag, malformed external value, stale handle, wrong handle kind/context, or equivalent representation error MUST be rejected before entering trusted Noble state or produce a specified failure. It MUST NOT authorize memory corruption or arbitrary execution.

**S-LANG-05.** Operations that require native unsafety MAY exist inside the compiler, runtime, engine integration, or host adapters. They are outside Noble guest semantics and MUST be covered by explicit implementation boundaries, validated public wrappers, and the verification/trust policy.

## 4. Memory and representation safety

**S-MEM-01.** A conforming host MUST NOT expose a guest operation that permits arbitrary reads or writes to host/native memory by numeric address. Guest-visible memory operations MUST be mediated by typed values, bounded byte regions, validated component memories, or opaque resources with documented contracts.

**S-MEM-02.** Use-after-retirement, double release, forged handles, cross-context handles, and wrong-kind handles MUST NOT produce guest-visible memory unsafety. They MUST be rejected or produce a specified failure at the validated resource boundary.

**S-MEM-03.** Serialization/deserialization, Preserves decoding, portable-code loading, Wasm import/export adaptation, and FFI/host callbacks are hostile-input boundaries unless a stronger trusted provenance contract is explicitly established. Successful byte decoding alone MUST NOT establish a trusted Noble type, live resource, authority, recipe, or proof claim.

**S-MEM-04.** Compiler/runtime representation choices MAY use indexes, pointers, arenas, tables, garbage collection, reference counting, linear ownership, or another strategy internally. Those choices MUST preserve the abstract Noble invariants and MUST NOT be observable as permission for guest representation casts.

## 5. Type, stack, and effect safety

**S-TYPE-01.** For an accepted program executing with conforming host bindings, wrong operand type, language-level stack underflow, invalid stack shape, and use of a consumed resource are forbidden stuck states, not permitted dynamic fallbacks.

**S-TYPE-02.** `Program<S,T,e>` remains the executable interface. `run` MUST only execute a checked/prepared `Program`; `Syntax`, arbitrary bytes, untrusted artifacts, or claimed manifests MUST NOT be directly runnable.

**S-EFFECT-01.** For every finite execution prefix, every guest-requested host operation MUST be included in the instantiated effect bound of the executing computation.

**S-EFFECT-02.** A program with an empty effect bound MUST NOT issue a guest host request. This does not imply termination, bounded resource use, or freedom from runtime housekeeping outside guest effects.

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

**S-CONC-02.** The absence of guest shared mutable memory SHALL imply **Noble-visible data-race freedom** for the standard concurrency profile. This claim does not automatically prove the Rust runtime itself race-free; runtime synchronization must separately refine the abstract concurrency model.

**S-CONC-03.** Assertions, interests, reactions, and subordinate conversational activity MUST have explicit owners/scopes. Facet/actor termination SHALL retract or retire scope-owned conversational state according to the standard profile, including failure paths.

**S-CONC-04.** Network or process-boundary protocol data MUST be decoded under bounded resource limits and validated against its Preserves schema/protocol before it becomes a typed Noble protocol value. Preserves embedded/native extension mechanisms MUST NOT by themselves create Noble resources or authority.

**S-CONC-05.** Typed protocol validity and runtime authority are distinct. A participant may only publish, observe, message, or transfer what both its static protocol interface and its runtime capability permit.

**S-CONC-06.** Deadlock freedom, global progress, fairness, and service availability are separate claims. They require explicit protocol/profile assumptions or proofs and are not implied by race freedom or type safety.

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

**S-VERIFY-01.** A stable-core safety claim requires reviewed Lean-kernel-checked evidence for the complete subset advertised as stable, together with the actual acceptance-checker correspondence required by SPEC-V001.

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

**S-GATE-02.** A standard-concurrency Noble-safe claim additionally requires the profile's isolation/scope/protocol/capability invariants and a concrete runtime correspondence result or explicitly disclosed trusted-runtime boundary.

**S-GATE-03.** Future advanced type-system features—including dependent types, higher-rank polymorphism, principled subtyping, refinement facilities, or user-extensible inference—are not rejected by this safety contract. They MAY be added only with rules that preserve or deliberately strengthen the safety invariants and with corresponding proof obligations before joining a stable-safe subset.
