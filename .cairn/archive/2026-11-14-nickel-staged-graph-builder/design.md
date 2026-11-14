## Context

Issue #123 adds an opt-in staged authoring frontend for the existing numeric graph IR. The existing graph parser builds untrusted producer data and `check_with` re-derives types, topology, slots, and graph budgets. Existing `ngraph-i64/0` source extraction and directly authored F32 graphs are not reinterpreted. The work changes neither the Noble language nor runtime semantics, and introduces no CLI command or compilation unit.

The selected evaluator is the repository-pinned Nickel 1.17.0. Nickel constructor functions execute only during authoring; the resulting data describes a graph and is never compiled as an arbitrary function. Native Rust lowering and admission remain necessary before execution.

## Decisions

### Decision D1: distinct authored-Nickel admission route

**Choice:** Nickel-built graphs carry a distinct `authored-nickel` subject kind whose identity is the SHA-256 digest of the exact versioned construction bundle. Admission is limited to `ngraph-i64/0`, concrete I64 `Input` slots, exact signed-I64 constants, `I64Add`, `I64Sub`, and `I64Mul`, with an acyclic finite graph and preserved input, operand, output, and repeated-output order. Graph sharing is represented by repeated references to one node handle. No `I64Eq`, F32 operation, `Param`, fact/guard, effect, resource, reflective, dynamic-call, or recursive-feedback node is admitted by this producer route.

The lowerer rejects unsupported constructor records before simplification or graph normalization. Generic I64 source extraction continues to use `extracted`; manually authored Nickel data cannot be labeled `extracted`. The subject label alone is not proof of source/evaluator correspondence; provenance is separately bound by the receipt decision below.

**Rationale:** `I64` has exact wrapping semantics suitable for the requested arithmetic profile, while a unique producer kind prevents the authoring route from being misreported as kernel extraction. Reusing `ngraph-ir/0` avoids a second runtime IR.

**Reopen when:** a reviewed profile revision authorizes additional operations or profiles, or an independently accepted source/evaluator correspondence contract replaces the finite receipt boundary.

### Decision D2: constructor data, evaluator boundary, and resource limits

**Choice:** The repository owns the Nickel library and `noble-ngraph-nickel/v1` schema. Templates, instances, typed scoped handles, explicit wires, and exposed boundaries are finite constructor data, not a new execution language. Rust lowers the exported JSON in an existing crate and admits the resulting `ngraph-ir/0` graph with `check_with` and explicit input, node, depth, output, and byte limits. Existing graph, reference-evaluator, and packed-evaluator APIs remain the runtime consumers. The Nickel process is an external, pinned Node-lane tool; no Nickel dependency enters the no-std core and no new CLI subcommand or Cargo target is added.

The authoring names `template`, `instance`, scoped `handle`, and explicit `wire` coordinate with Choregraph #6's shared vocabulary. This is a finite, I64-only adapter boundary: it does not import Choregraph's operation registry, effects/resources, feedback, or target semantics, and makes no cross-target equivalence claim.

The replay lane records the declared import closure and independently observes every imported file's resolved path and bytes. Admission refuses if the observed import set differs from the declared one. Because nickel-export's documented issue #1 can make import order ambiguous, the lane binds the ordered closure explicitly. Because issue #2 concerns selected export and delayed contracts, the lane demands and fully serializes the complete constructor record before invoking Rust lowering; a delayed contract failure therefore aborts export before graph creation. The native checker is still run after successful export.

Construction budget is charged before allocating expanded node/handle structures; native budgets then charge the graph. A graph reference is a scoped identifier, never a Nickel number. Instance identities are deterministic from the constructor bundle digest plus instance/local identifiers; two instances remain distinct and repeated references to one handle remain shared. Cross-instance handle escape and duplicate scoped identifiers refuse.

**Rationale:** This keeps evaluation outside the pure graph core and separates authoring from admission. Explicit closure binding and full demanded export address the two known nickel-export qualifications without relying on partial export behavior.

**Reopen when:** Nickel's pinned evaluator contract changes, import discovery can be independently authenticated without the explicit closure, or profile limits are revised by a separately reviewed budget policy.

### Decision D3: parameter and exact-constant semantics

**Choice:** A construction parameter selects graph structure or supplies an exact constant. A runtime numeric argument becomes an ordered I64 `Input` slot. Noble's F32 `Op::Param` remains trainable snapshot state and is never used by Nickel constructor parameters or runtime inputs. I64 constants cross the evaluator boundary as canonical signed-decimal strings, are parsed directly as `i64`, and are never serialized through Nickel or JSON binary64 numbers. Fractions, malformed decimal strings, and values outside `[-2^63, 2^63-1]` refuse.

**Rationale:** Nickel's arithmetic is host authoring computation, not the graph's wrapping-I64 evaluator. Decimal strings preserve values above 2^53 and exact MIN/MAX without a float round trip.

**Reopen when:** the constructor schema gains a separately versioned exact integer representation with proven round-trip semantics.

### Decision D4: evidence and claim boundary

**Choice:** Retained evidence distinguishes: real pinned Nickel export; native graph admission; finite reference/packed evaluation; composition-law observations under ordered outputs and exact wrapping-I64 evaluation; provenance record verification; and any mathematical model. No such observation is a general proof or source-to-implementation correspondence theorem. The receipt binds source/import bytes, construction parameters, library and contract versions, evaluator identity, declared and observed import closure, exported constructor bytes, lowerer revision, profile, graph bytes, and checked graph identity. A changed field invalidates the receipt or consumer binding.

**Rationale:** Stable hashes identify the records used by an experiment but do not establish semantic equivalence or grant authority. The finite tests do not imply effect independence, resource permissions, or arbitrary output reordering.

**Reopen when:** a maintained formal correspondence theorem or a stronger independently accepted artifact-custody format is available.

## Risks / Trade-offs

- Nickel contract success is not enough: all demanded export data must be forced and native graph admission must complete before a graph exists.
- Bundle digests and receipts are provenance bindings only; they are not semantic proofs or permissions.
- Composition laws are qualified only by ordered output values under exact wrapping-I64 evaluation. Output permutation is an explicit transform whose changed output order must be checked, not silently normalized.
- Unsupported constructs must be rejected before any optimization so their erasure cannot broaden the route.
