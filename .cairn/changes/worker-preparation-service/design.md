## Context

`noble worker typed PACKAGE --schema SCHEMA --state STATE --event EVENT` admits a closed local package and runs one bounded transition. Admission checks the host-selected `WorkerData@1` schema module, binds the package's schema rows and declared interface (WORKER-02), checks the root module, the exported `transition` owner and its closure, and checks the exact invocation `State Event -- State List<Action> ! {}` before compiling and starting the engine. Nothing produces such a package in production. Tests build packages with an unchecked builder, and the WORKER-02 gate encodes its own. The worker conformance slice says candidate syntax arrives after the coordinator compiles and is prepared against the independent worker interface and exact schema environment, with a separate service trace.

## Decisions

### Decision: Vehicle and command

**Choice:** Add `noble worker prepare SOURCE --schema SCHEMA --out PACKAGE` to the local Worker-Typed v1 profile. Worker Program v2 keeps its in-process `Store::prepare`; it has no State/Event/Action interface yet, so it is not this service.

**Rationale:** v1 is the only path whose admission already holds the worker interface and the schema environment, so preparation and evaluation can share its checks exactly.

### Decision: Inputs and authority

**Choice:** Only the source bytes come from the candidate (at most 65,536 bytes, UTF-8, a regular file). The host selects the schema module (65,536 bytes), the interface, the root export `transition`, the limits (65,536 bytes, 4,096 nodes, depth 32, work 3,000,000 per stage; 131,072 retained schema and root bytes), the encoding and the destination. Identities, schema rows, interface claims, effects, captures and artifacts are never accepted from the candidate; the service derives every package field.

**Rationale:** B-CHECK-07 binds an independently supplied interface and environment. A candidate claim cannot select either.

### Decision: One implementation of the checks

**Choice:** Split typed admission into a schema-module check, a schema catalog, a root-module check, a root closure walk and a transition-interface check. `typed` keeps its order and its WORKER-02 classifications. Preparation runs schema module, catalog, root module, closure, transition interface, one worker compilation and encoding, in that order, and stops at the first refusal. The package encoder moves from test code into production; tests build hostile controls from the production encoder over checked owners.

**Rationale:** A second checker could disagree with admission. Preparation succeeding must imply that admission of the same source under the same schema succeeds.

### Decision: Encoding and publication

**Choice:** Encode `noble-worker-local-package/v1` with the existing canonical serde form (sorted keys, compact, no trailing newline) and parse the result with the admission parser before publishing. Refuse a package over 262,144 bytes as exhausted, and refuse a worker literal arena over the engine's 983,040-byte bound as exhausted. Publish with create-new only after every check; an existing destination is refused as invalid input and never overwritten.

**Rationale:** The encoding is local, not a portable identity. The round trip proves the service only emits what admission accepts. For the accepted WORKER-02 baseline source and schema, the bytes equal the committed baseline package.

### Decision: Separate service trace

**Choice:** Print one `noble-worker-preparation/v1` record. It names the service, the last stage entered, the outcome, the selected interface and limits, the candidate and schema digests, the checked owner, the compiled module's WAT digest, the published package digest and length, a bounded diagnostic, and service-effect counters: input files read, checker stages entered, closure walks, backend compilations and packages written. Counters are incremented where the work happens. The service never starts an engine, so the trace has no evaluation fields; `typed` reports have no preparation fields.

**Rationale:** EV-CASE-02 and B-RESULT-02 keep compiler-service effects in a separate trace. Counting at the call site makes the trace an observation, not a restated constant.

### Decision: Interpretation of the preparation result

**Choice:** In process, the service's successful result is the compiled worker module at the expected interface, whose WAT digest the trace reports. Across invocations it publishes a semantic package whose `artifact` is null. P-STAGE-02 treats the package as untrusted until `typed` re-admits it locally.

**Rationale:** P-PREPARE-02 asks for an executable program at the expected interface; a v1 package may not carry executable bytes. The WAT digest lets a test bind the prepared module to the module `typed` admits.

### Decision: Export authority

**Choice:** The package carries no captured information and derives only from the invoking host's own source and schema. The destination is named by the invoking host. This is a local preparation output, not a P-PACK-01 export of captured data.

**Rationale:** Worker Program v2 keeps capture export behind a host-built grant. v1 packages admit no captures.

## Risks / Trade-offs

- A refused candidate's classification follows typed admission. Typed admission collapses closure exhaustion into `invalid-input`, so preparation does too.
- Identities in the package are session-local counters. They are stable only for the fixed session order of schema then root, and are not portable identities (P-ID-04).
- `typed` still re-checks and recompiles the package source as local admission. That is required by P-STAGE-02 and is not a preparation-service request.
- No guest `code.prepare` word, compiler capability value (P-PREPARE-04), cache, v2 preparation or WORKER promotion is added. WORKER-11 keeps `preparation-service` among its open gates until milestone review.
