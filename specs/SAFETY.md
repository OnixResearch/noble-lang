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

**S-LIVE-01.** `Live-Wasm-Draft` admission MUST treat source files, generated binary, interpreter/VM inputs and claimed manifests as untrusted until checked against a host-selected exact source/import snapshot, independently accepted kernel derivations, selected emitter revision and actual validated bytecode. Standard Wasm module validation alone MUST NOT prove Noble stack/effect typing, executable provenance or host authority. In-process emission MUST use deterministic pinned binary instruction/section rules and independently compare the actual module's imports/exports, signatures, memory/table bounds, feature set and absence of start/active initialization against the checked contract before isolated staging. A reload refusal MUST execute zero candidate-body guest instructions and host requests and make no mutation to prior live VM state, authorizations or stack. Each invocation MUST enforce finite preparation work and module size, engine fuel or equivalent instruction/step budget, stack depth/call recursion, guest memory/table/cells, retained old-generation instances/captures and wall-clock deadline; exceeding any bound reports a distinct refusal or execution failure, never an unchecked fallback. Isolated staging MAY allocate only within an explicit bounded budget and MUST retire staging state on failure. Ordinary host effects remain independently authorized per request, not imported through source or code identity.
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

**S-LANG-03.**

The defined semantic outcomes of accepted Noble execution MUST be limited to normal return, explicit modeled waiting/suspension where the execution profile permits it, divergence, a specified trap/abnormal termination, or a specified execution-profile failure such as quota exhaustion or cancellation. Undefined behavior is not a Noble-language outcome. For the selected S-CASE-13 Wasm-Draft guest, the invoker MUST instantiate a genuine Noble-emitted, import-free Core module with normal 16-page initial memory before setting exactly one unit of Wasmtime guest-call fuel; that call MUST fail specifically from engine fuel exhaustion, not from instantiation or host setup. With independently ample guest fuel, the invoker MUST set a 64-byte generated heap quota measured beyond the guest's 65,536-byte heap baseline, and MUST classify the selected text export as an allocation failure only when its production allocator's private quota diagnostic is set by the guard. A generic unreachable trap, failed artifact load, 64-byte Store memory cap, runtime step budget, or host resource refusal MUST NOT be reported as either specified canonical quota failure. Both canonical variants MUST report unsuccessful execution and zero imported host callbacks; trap cleanup MUST release guest live heap bytes. Positive accepted guest returns, including I64(3), text using exactly 64 generated bytes, and the 65-byte text at quota 76, MUST distinguish the selected limits from a pre-execution refusal. Proof and broad engine/backend correspondence remain open.

Before interpreting a private allocator diagnostic, the production invoker MUST rebuild the complete host-selected Noble WIT/world/export-source recipe with its pinned assembler and MUST byte-match the candidate Core; absent, mismatched, or arbitrary import-free Wasm MUST be refused before guest execution and MUST NOT receive a specified allocation-failure label.

#### Scenario: S-CASE-13 independent fuel and generated-heap failures

- GIVEN the exact S-CASE-13 input, a byte-matched complete Noble source recipe, and separately host-selected one-unit Wasmtime fuel and 64-byte guest heap budgets
- WHEN fresh import-free Core instances execute the arithmetic and text exports after successful instantiation, with ample alternative budget and positive 56/57/65-byte controls
- THEN each exact canonical variant matches every expected field with the matching typed engine trap or diagnosed generated allocator guard, no guest result or callback, and zero live heap after explicit trap cleanup

**S-LANG-04.**

A bounds violation, invalid variant/tag, malformed external value, stale handle, wrong handle kind/context, or equivalent representation error MUST be rejected before entering trusted Noble state or produce a specified failure. It MUST NOT authorize memory corruption or arbitrary execution. In the selected S-CASE-02 versioned bounded-region adapter, the host MUST validate the retained live owner, invocation/context/kind/generation/read right and the nonnegative signed offset and length, checked addition, region extent and output quota before any protected region byte read. Canonical `000102` at offset 3, length 1 MUST return `bounds-reject`, zero protected operations and zero region reads; offset 1, length 2 MUST return bytes `0102` through the same guest-callable operation. Offset 3, length 0 is an empty success; offset 4, length 0 and arithmetic overflow MUST reject. These finite tests do not establish universal native safety.

#### Scenario: S-CASE-02 checked refusal and actual guest control

- GIVEN the exact S-CASE-02 input and a separate host-selected three-byte resource passed as a typed owner to a compiled guest
- WHEN the guest invokes the versioned region adapter with offset 3 and length 1, and positive/boundary/overflow/invalid-owner controls invoke that same boundary
- THEN the canonical case matches every field of its expected record before any protected read, the valid control returns bytes `0102`, and failures neither read the region nor grant arbitrary native access

**S-LANG-05.** Operations that require native unsafety MAY exist inside the compiler, runtime, engine integration, or host adapters. They are outside Noble guest semantics and MUST be covered by explicit implementation boundaries, validated public wrappers, and the verification/trust policy.

## 4. Memory and representation safety

**S-MEM-01.**

A conforming host MUST NOT expose a guest operation that permits arbitrary reads or writes to host/native memory by numeric address. Guest-visible memory operations MUST be mediated by typed values, bounded byte regions, validated component memories, or opaque resources with documented contracts. The selected versioned bounded-region world MUST supply an immutable host-owned byte region through an opaque invocation-owned resource with an explicit read right. Host table and owner-context identities MUST remain distinct between simultaneously live invocations, with bounded nonwrapping allocation and refusal on exhaustion; an equal slot/generation from another host Store is not authority. Offset/length values are relative to that selected region only; they MUST NOT be interpreted as native addresses or offsets in the separate 16-page Noble Core Wasm memory. A valid read copies only a fully prevalidated bounded subrange, while an invalid read MUST NOT enter the protected region, start native work or consume its owner. A successful or refused borrowed read returns the identical owner for one explicit release or host cleanup on abnormal exit. `native_memory_access=false` for S-CASE-02 means no unchecked host/native-address dereference, not absence of ordinary safe host bookkeeping memory operations.

#### Scenario: S-CASE-02 host-selected three-byte extent

- GIVEN the exact three bytes `00 01 02` separately selected by the host and retained behind the invocation resource, not embedded into guest memory
- WHEN the compiled guest submits the canonical out-of-range read and a positive in-bounds read through the actual host method
- THEN `(3,1)` is `bounds-reject` with zero protected operations and no region bytes accessed, while `(1,2)` returns `01 02` and the owner is released once

**S-MEM-02.**

Use-after-retirement, double release, forged handles, cross-context handles, and wrong-kind handles MUST NOT produce guest-visible memory unsafety. They MUST be rejected or produce a specified failure at the validated resource boundary. For selected S-CASE-08, `Host::new` MUST pre-register a live pending authentic owner plus only the selected mode's necessary fixture after recipe admission. `tokens.issue` MUST select an actual retained host-table claim, not create a new owner or fabricate a label: stale-generation names a genuinely retired earlier generation of the pending owner's reused slot; wrong-kind names one other live owner of a different kind; wrong-context names one other live owner in a distinct bounded, nonwrapping context. Authentic mode needs no extra fixture. The host MUST reject each hostile claim before construction of a Wasmtime `ResourceAny` or trusted Noble owner, before `tokens.consume` or protected work, and MUST settle the pending owner exactly once and any selected fixture separately without leaving live owners or native pins. A matching slot/generation in a distinct context or table namespace does not authorize that owner.

#### Scenario: S-CASE-08 real hostile returned-owner matrix

- GIVEN the prepromotion `Resources-Draft` S-CASE-08 input `callback-handle-matrix` with `stale-generation`, `wrong-kind`, `wrong-context` and the matching compiled guest/WIT recipe
- WHEN each real host-table claim returns from the actual `tokens.issue` callback at the production owned-result boundary
- THEN each mode reports `callback/invalid-result`, `trusted_values_created:0`, zero protected operations, one guest request and one settled invocation owner, with zero live owners and native pins; a separate authentic claim reaches `tokens.consume` once

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

**S-TYPE-02.**

`Program<S,T,e>` remains the executable interface. `run` MUST only execute a checked/prepared `Program`; `Syntax`, arbitrary bytes, untrusted artifacts, or claimed manifests MUST NOT be directly runnable. On the selected Core-only `noble admit-artifact` path of S-HOST-03, a submitted artifact that passes the pinned validator and engine compilation remains candidate data until its exact final bytes equal the host's pinned compilation of independently accepted, host-invoker-selected source under host-selected options; only then MAY it execute, in a fresh isolated host. Candidate-attached `source`, `digest`, `trusted_correspondence` and `allowed` fields MUST NOT establish correspondence, trust, authority or runnability. That selected path has no trusted-build attestation and no translation-validation input: S-CASE-07's `trusted_build:false` and `translation_validation:false` denote these structural absences, not Boolean inputs, and neither MAY be inferred from candidate metadata. A valid artifact compiled from another program, or whose bytes otherwise differ from that compilation, MUST refuse at admission as `correspondence-reject` before instantiation, with zero guest requests and protected operations. This finite selection does not establish backend lowering or artifact-loading correctness (PO-17/PO-18), cross-host provenance, signature or proof-object admission, declared-binding artifacts or general Wasm loading.

#### Scenario: S-CASE-07 forged correspondence metadata is not runnable

- GIVEN host-selected source `1 2 +` with pinned `--opt off`, and a pinned-validator-valid Core artifact compiled from `2 2 +` whose correct empty effect claim carries forged `source`, `digest`, `trusted_correspondence` and `allowed` metadata
- WHEN the selected admission path validates, reflects and compares its exact final bytes with the host's independent compilation before any instantiation
- THEN it refuses as `admission`/`correspondence-reject` with zero guest requests and protected operations; the same bytes are admitted only when the host selects their actual `2 2 +` source, and the unmodified `1 2 +` artifact executes to `I64(3)`

**S-EFFECT-01.**

For every finite execution prefix, every guest-requested host operation MUST be included in the instantiated effect bound of the executing computation. For the selected Core-only S-CASE-16 Wasm admission path, this bound MUST come from host-invoker-supplied source prepared through actual resolution/checking and independent kernel acceptance, including complete latent quoted program bodies, NOT from an artifact's claimed manifest or effects observed only on the chosen top-level path. The exact final validated artifact MUST match bytes compiled from that checked source under host-selected pinned compiler/assembler/optimizer options. Independently reflect actual binary import module/field/kind before linking: `noble.test_emit` represents Core `test.emit`, `noble.test_abort` represents Core `test.abort`; `noble.test_emit_bound` and `noble.test_clock_bound` have distinct bound `test.emit` and `test.clock` identities but MUST fail closed as unconfigured, not acquire fabricated adapter bindings. Selected import function signatures are assured by final-byte equality with the validated pinned host compilation before instantiation, NOT by an independent pre-correspondence signature decoder. Compiler-emitted but unused Core ABI imports do not by themselves enlarge the checked source's effect bound or establish a guest call; an out-of-bound request MUST NOT be admitted as authorized.

#### Scenario: S-CASE-16 finite manifest/admission footprint

- GIVEN S-CASE-16's canonical `actual_imports:["test.emit"]`, `claimed_effects:[]`, `trusted_correspondence:false` and a real validated artifact representing that claimed footprint
- WHEN the selected admission procedure examines its actual binary imports and supplied claim before any guest execution, and separately checks positive host-supplied source and rebuilt final bytes with latent effects
- THEN the forged empty claim refuses as `effect-manifest-reject` with zero guest requests and protected operations; no manifest or physical import list alone proves the source effect bound, and any admitted execution accounts requests against its checked complete bound

**S-EFFECT-02.**

A program with an empty effect bound MUST NOT issue a guest host request. This does not imply termination, bounded resource use, or freedom from runtime housekeeping outside guest effects. A denied request still counts as a request under S-EFFECT-01. Static effect rejection and forged-metadata admission rejection require zero candidate-body host requests; runtime authority denial with a source-declared effect is distinct but is NOT selected in this Core artifact admission path. A claimed empty set MUST NOT suppress actual binary import reflection or independently checked source/latent effect accounting. The canonical forged footprint MUST reject at admission as `effect-manifest-reject` before candidate instantiation, start, active-segment application, guest request or protected operation, even though independent source/byte correspondence is absent. An honest empty-effect checked source MAY retain unused compiler-emitted Core ABI imports only when exact final-byte correspondence and the separate host gate ensure those imports cannot authorize an out-of-bound guest request. Optional effectful-host policy denial occurs before guest invocation, with zero guest requests and protected operations.

#### Scenario: S-CASE-16 zero-execution refusal and honest source controls

- GIVEN the canonical forged empty effect claim and separately generated honest empty/effectful sources under the same selected host configuration
- WHEN each submitted final binary is checked without instantiation and only admitted bytes are installed in an isolated fresh host
- THEN the forged candidate has zero guest requests and protected operations, honest empty source never requests an effect, effectful host-policy refusal happens at admission with zero requests, and separately allowed source executes its actual guest request in the fresh host

**S-EFFECT-03.**

For the selected bounded filesystem profile, the host MUST derive an `authorization/denied` report solely from its retained resource-Table READ-right failure. A guest-returned `denied` error string, even from a source-matched component, MUST NOT create that host authorization observation. The actual request trace and protected-operation count remain independent observations.

Effect information is descriptive, not authoritative. A type/effect check, `Program` possession, program identity, or successful proof MUST NOT create host authority. For the selected S-CASE-06 `noble-test:authorized-fs/bounded@1.0.0` profile, the host MUST independently prepare and compile the complete invoker-selected WIT/world/export-source recipe and byte-match the candidate component before opening any file or creating a Directory owner/grant; a mismatched recipe rejects at admission with zero guest requests, not as S-CASE-06 denial. The actual matched compiled `"main.rs" fs.read` guest MUST issue exactly one read import even when a live host-owned invocation Directory has no READ grant. The host MUST classify that request as `authorization/denied`, count it in the `fs.read` effect trace and perform zero protected file-content reads. The host invoker preopens only one vetted regular file, mapped under the exact key `main.rs`; the Directory is a **logical host table owner over that File**, not a preopened OS directory. Neither the guest's source effect nor its path grants permission; an independently host-granted READ is required for a positive read of the same compiled guest. This selected finite adapter does not establish general filesystem authority or native filesystem safety.

#### Scenario: S-CASE-06 actual request without a READ grant

- GIVEN the exact canonical `Resources-Draft` input `"main.rs" fs.read`, the declared `fs.read` effect and a live host-owned `Directory` in `invocation-1` with `read_right:false`
- WHEN the compiled Noble component calls the production Wasmtime `fs.read` import against the host's exact preopened-file map
- THEN denial occurs at `authorization` after one counted guest `fs.read` request with zero protected operations and zero file-content reads; a separately authorized run of the same compiled guest returns only the selected file's bounded bytes

**S-RES-01.**

A live resource/capability has an explicit ownership obligation in Noble. Generic duplication, generic discard, capture by `quote`, persistence, and serialization MUST remain unavailable for resource-bearing values unless a separately specified operation has semantics that preserve the resource contract.

For the selected S-CASE-04 static profile, an admitted WIT `own<counter>` returned by an imported host operation MAY form a Noble resource value or be combined **inside Noble** into `Pair<Text,Resource<R>>`, `Sum<Resource<R>,I64>` or `List<Resource<R>>`. The production checker MUST refuse `dup`, generic `drop` and `quote` capture on each complete resource-bearing value before component emission. The selected route to generic serialization is `quote reflect`: its refusal MUST occur at `quote`, before serializable Syntax exists. This does not provide an independent serializer or add WIT aggregate inputs.

#### Scenario: Exact resource matrix refuses before component publication

- GIVEN the real admitted `own<counter>` kind and Noble-built `Resource<R>`, `Pair<Text,Resource<R>>`, `Sum<Resource<R>,I64>` and `List<Resource<R>>`
- WHEN each value is used with `dup`, generic `drop`, `quote` or the `quote reflect` route
- THEN the source/kernel eligibility constraint identifies the whole resource-bearing type and exact Data-requiring word, and every component compile refuses before publication without guest execution or protected operations; a WIT parse, frontend type or lowering error is not this result

**S-RES-02.** Live authority MUST be non-forgeable from ordinary guest data. A resource may enter guest execution only through an authorized host operation, a checked transfer from another ownership context, or another profile-defined validated authority boundary.

**S-RES-03.**

Host resource tables MUST validate at least resource kind, owning/authorized context, liveness/generation, and operation-specific rights before acting on an incoming guest handle representation. The selected S-CASE-06 Directory owner is instead retained entirely by the host, not supplied through a guest WIT resource representation. Its table identity, invocation context, kind, generation and liveness MUST still be checked with the independently selected READ right and exact `main.rs` map membership before the host reads the preopened file. A live owner alone grants no READ; a guest path or effect claim cannot create the missing right. Forged, stale, foreign-context, wrong-kind, retired or no-right host claims and non-mapped paths MUST perform zero protected content reads. Host-injected handle probes are controls, not a claim that the WIT exposes a Directory handle.

#### Scenario: Host-retained Directory and hostile claims

- GIVEN a host-preopened file and live per-invocation Directory owner, with separately varied right, table identity/context/kind/generation and guest path
- WHEN the same production host read boundary validates those facts before accessing file bytes
- THEN no invalid or ungranted claim reads the file, while the exact owner/path/READ combination can return only bytes from the vetted file

#### Scenario: Candidate/recipe mismatch precedes host authority

- GIVEN a candidate compiled from `"main.rs" fs.read` and a distinct host-selected WIT/world/export-source recipe
- WHEN the production `read-fs` route independently compiles that complete recipe and compares final candidate bytes before opening the configured file
- THEN the mismatch rejects at admission with status 2, zero guest requests and no Directory owner or READ grant created; it cannot count as the canonical authorization-stage denial

**S-RES-04.**

Resource-containing aggregates carry the ownership obligation recursively and MUST NOT satisfy `Data` or `Capture` merely because a particular runtime variant does not currently expose the resource.

For the selected S-CASE-04 sum, the static checker MUST inspect both alternatives of `Sum<Resource<R>,I64>` when Noble source selects the I64 alternative with a false conditional. The whole sum remains ineligible for the same generic operations; after checked `case` elimination the I64 payload MAY be used at its own Data type without making the containing sum Data. The canonical `check/eligibility-reject` result is a harness classification of an exact typed production refusal, not a claim that the literal component CLI reports those stage/outcome strings. Source construction and source-only branch controls do not imply support for WIT tuple, resource-bearing result/list ABI, component lowering of every positive aggregate, or runtime execution of the selected branch.

#### Scenario: Unselected resource alternative does not inherit Data from I64

- GIVEN the Noble source `false [ counters.open inl ] [ 7 inr ] if` with a real WIT-bound `counters.open` resource kind
- WHEN the complete sum is passed to the generic operations, and separately a checked `case` exposes the right-hand I64 payload
- THEN the whole sum refuses with its complete `Sum<Resource<R>,I64>` eligibility diagnostic while a source-checked I64 branch may use its own Data operations; the static observations alone do not establish a runtime branch trace

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
**S-CONC-09.** The selected `Choreography-Service-M8` profile SHALL project only
the versioned data descriptor `noble:choreography/service@1.0.0`, never
interpreting it as a WIT world, facet authority or executable guest code.
Its entire descriptor has exactly `protocol` and `rounds` fields; `rounds`
has one or two records with exactly `name`, `ready`, `exit`. Each `name`
MUST be distinct and match M7's ASCII `[A-Za-z0-9_-]{1,32}`; `ready` MUST be
Boolean; `exit` MUST be `withdraw` or `trap`, with `trap` only in the last
round. The selected untrusted descriptor boundary SHALL accept only strict
UTF-8 JSON bytes, not an arbitrary transport or a new M7 Preserves tag.
Before constructing trusted typed descriptor values it MUST enforce at most
512 input bytes, valid UTF-8 and JSON container nesting depth at most three
(root object, `rounds` array, round object; count `{` and `[` outside
strings). It MUST parse one complete JSON value with no trailing content
except JSON whitespace, reject duplicate object keys at either object
level, unknown fields, unsupported versions, invalid types or names, zero
or more than two rounds, duplicate names and nonterminal traps before
admission or guest effects. JSON strings MAY use standard escapes; validate
the *decoded* protocol/name characters and lengths. The JSON bytes do not
assign a facet, role, WIT resource or authority. This boundary defines
descriptor decoding only; it selects no file/network transport, general
JSON protocol or replacement for M7's service-only Preserves decoder.

For `(name,ready)=(n,b)`, a withdrawal round expands, in order, to
`subscriber.observer(n,b)=false`, `publisher.publisher(n,b)=true`,
`subscriber.observer(n,b)=true`, `publisher.withdraw(n)=true`,
`subscriber.observer(n,b)=false`. A terminal trap round expands to
`subscriber.observer(n,b)=false`,
`publisher.publish-and-trap(n,b)=trap-after-publication`,
`subscriber.observer(n,b)=false`. The trap MUST observe a successful
`publish` import followed by `fail`, commit the publication, produce
typed add/remove on a real invocation error and M7 retirement, and leave the active
subscriber seeing absence, without reporting publisher success. False
readiness is an exact-pair field, not absence. Role-local projections MUST
be the stable ordered subsequences of these global actions, preserving
arguments, expected results and global/round ordinals. Projection is static
data; execution, admission, fairness and compiler correspondence are
separate claims.

Only one M8 session MAY occupy an initially empty M7 dataspace, using
exactly two separately compiled participant instances and host-issued
publisher/subscriber facets. Before creating facets or invoking a guest,
admission MUST validate the complete descriptor, projections, exact
`noble:syndicate/service@1.0.0` WIT signatures, distinct host-bound roles,
rights and capacity for two facets, one simultaneous assertion, two
interests and four retained add/remove events, including immediate
notifications and owed removal/cleanup on trap. M7's preflight still
applies. No competing observer/session/activity is admitted.

The trusted host SHALL pre-admit each export's identity, arguments,
caller role/facet, next global step, exact pair and expected pre-state
before invoking the export. This MUST reject a wrong `publisher` export
where `publish-and-trap` was required even though both first import
`publish`. Pre-export refusal MUST leave the session cursor, dataspace,
events, rights and guest-request/protected-operation counts unchanged.
Every imported operation MUST additionally pass an owner-bound,
phase-specific check before forwarding to M7. The monitor MUST hold at most
one in-flight export with expected import substeps and refuse interleaved
exports. Pre-export admission, each import precheck/M7 commit/phase update
and post-call cursor finalization MUST use the same serialized critical
section, released while component code runs to permit host callbacks.
The lock order MUST be monitor then M7 dataspace for admission, imports and
retirement; it MUST NOT be reversed or held across a guest call. The
pre-export check atomically reserves the in-flight phase, each import
updates its pending substate atomically with its M7 effect, and finalization
after `function.call` plus `post_return` success or trap plus mandatory
retirement commits the next global cursor. There is no unmonitored M8
route to M7. Wrong order/role/replay, forged facet/rights,
wrong pair or an unexpected import MUST NOT cause an unauthorized M7
operation, protected effect or session cursor advance. An in-flight refusal
MAY trap and trigger mandatory M7 retirement of previously owned state and
typed removal events; it MUST NOT be misreported as unchanged dataspace
or as a completed choreography. Only an observed expected import sequence,
return value (or successful compiled `publish` import, subsequent `fail`
import, invocation error and facet retirement/removal, not an earlier
authorization failure) and
final subscriber absence advance the corresponding step and finish the plan.
An unexpected successful return, including one omitting `fail` after a
successful trap publication, MUST terminate the publisher facet as a
protocol failure, retract its assertion and not advance or report success.
No durability, remote transport, global progress or general Syndicate
claim follows.
## 8. Host and artifact boundary safety

**S-HOST-01.**

Every public boundary from unverified Rust, FFI, Wasm, external wire data, connector/network input, or host callback into verified/runtime-internal state MUST validate all preconditions not already guaranteed by a proven caller relation. For selected S-CASE-08 `noble component callback-owner COMPONENT MODE WIT WORLD EXPORT=SOURCE`, the host MUST independently prepare/compile the complete invoker-selected `noble-test:callback-owner/bounded@1.0.0` WIT/world/export-source recipe and byte-match the candidate component before `Host::new` registers its mode-selective owner fixture or Store/guest execution. A recipe mismatch MUST refuse at admission with zero guest requests and no created owner, not masquerade as a callback-stage invalid result. At `tokens.issue`, the returned owner claim MUST pass retained table identity/slot/generation/kind/context/liveness/right validation before any trusted resource construction; no unverified callback assertion, numeric slot or guest result can replace those checks.

#### Scenario: Matched recipe enters the real callback boundary

- GIVEN the complete independently host-selected WIT/world/export-source recipe and a byte-identical compiled Noble component containing `tokens.issue tokens.consume`
- WHEN the production host invokes each S-CASE-08 callback mode and separately tries a candidate with a mismatched recipe
- THEN actual matching runs reach the `tokens.issue` host ingress for validation, while recipe mismatch refuses before owner creation or guest requests and is not classified as the canonical invalid result

**S-HOST-02.**

Host adapters MUST validate their outputs before making them available as trusted Noble values. A malformed native adapter result is an implementation/binding defect and MUST NOT be accepted as if the guest type system had proved it. In the selected `noble-test:callback-owner/bounded@1.0.0` `issue() -> own<token>` result, the production host MUST check its mode-selected retained table-backed claim against the expected token kind, table namespace/slot, current invocation context, generation, liveness and rights **before** calling Wasmtime's owned-resource constructor (`ResourceAny`) or incrementing `trusted_values_created`. The three hostile returned-owner modes MUST fail at `callback/invalid-result` from that host check with zero trusted values; a host-returned error string, guest result, late trap or postconstruction rejection is not equivalent. Authentic issue MUST create exactly one trusted guest owner, and only that owner can be consumed in the second guest callback request. Reports use `noble-callback-owner/v1`; hostile modes each account `guest_requests:1`, `protected_operations:0`, `released_owners:1`, `fixture_releases:1`, `live_owners:0`, `native_pins:0`, while authentic accounts `guest_requests:2`, `protected_operations:1`, `released_owners:1`, `fixture_releases:0`, `live_owners:0`, `native_pins:0`. The fixture release is stale setup retirement or one live wrong-kind/context fixture cleanup, never an additional release of the pending authentic owner.

#### Scenario: Host output checked before resource construction

- GIVEN mode-selective real table states for the same compiled guest: authentic pending owner alone, retired earlier generation plus pending owner, or pending owner plus one live wrong-kind or foreign-context fixture
- WHEN the `tokens.issue` host callback returns its owner claim to the Wasmtime resource conversion boundary
- THEN three hostile modes reject before trusted construction and the authentic mode creates one trusted owner that reaches `tokens.consume`; every report retains independently measured request, operation and owner/pin settlement counters

**S-HOST-03.**

A valid Wasm artifact, digest, signature, recipe hash, source identity, or attached proof object is not by itself authorization and is not by itself sufficient evidence of source/recipe correspondence. Loading policy MUST separately establish artifact validity, interface compatibility, provenance/correspondence policy, and concrete runtime authority. For selected Core-only S-CASE-16 admission through `noble admit-artifact WASM --effects CLAIMS_JSON [--source HOST_SOURCE] [--allow-effects ...] [--opt off|on]`, the invoker MUST supply source bytes, pinned compilation options, allowed effects and optional effectful-host policy independently of candidate-attached claims; missing source cannot establish correspondence. Validate the final binary and independently reflect import module/name/kind; compare byte-for-byte with the host-controlled validated Core compilation under exactly those options to establish the selected complete interface and function signatures. Neither a matching digest nor a manifest substitutes for equality. Reject submitted starts and active element/data segments before instantiation; the current selected Core compilation has no start and uses passive segments. No untrusted submitted artifact may be instantiated or mutate host state during admission. Only a fully admitted matching artifact MAY execute in a fresh isolated host; effectful-host policy denial MUST precede execution, while actual allowed guest invocation MUST separately establish whether effects execute. This finite selected observation does not discharge declared-binding artifact admission or source-, backend-, engine- or host-universal correctness/refinement.

#### Scenario: S-CASE-16 correspondence-independent manifest rejection

- GIVEN the canonical valid-import/empty-claim/no-correspondence candidate, mismatched source/options/bytes and interface mutation controls, and independently supplied positive source/policy pairs
- WHEN actual final bytes are validated, classified and compared to independently rebuilt source bytes before any candidate instantiation
- THEN the canonical case refuses specifically as `effect-manifest-reject` with zero guest requests and protected operations; unrelated correspondence/interface failures also refuse without side effects, while only matching positive bytes proceed into a fresh host and an actual governed guest invocation

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
