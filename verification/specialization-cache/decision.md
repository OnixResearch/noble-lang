# Specialization-cache decision

## Decision

Implement a bounded, positive-only, in-process cache for the residual product of `ngraph-pe/0`.
Do not persist cache entries. Cache objects are private session state and are populated only by the
current process after the ordinary specializer, source emitter, and independent residual admission
all succeed. A lookup compares the complete specialization manifest, verifies all retained bytes
and derivation fields, and re-admits the residual under the request's current frontend and graph
limits before returning it. The cached residual is never an authorization or admission token.

The CLI exposes reuse only through an explicit `--cache` switch. Its existing `specialize` command
handles one request per process, so its own session contains at most one request; reusable callers
hold the library session-cache object across requests. This first implementation does not pretend
that one-shot CLI invocations share a cache.

## Evidence and criteria

The cache must meet these criteria before reuse is returned:

1. exact, versioned, domain-separated specialization-manifest equality, not digest equality alone;
2. exact retained residual graph/source bytes and a binding provenance record;
3. a build/admission key distinct from semantic specialization identity, including the exact
   residual product and target, representation, backend/runtime/ABI, flags, limits and compiler
   epoch;
4. fresh ordinary-path residual admission under the current request;
5. bounded entry count, variants per original recipe, manifest bytes, retained bytes, lookup work
   and eviction work; failed or interrupted generation adds no entry;
6. no negative caching and no process-local executable handle crossing a cache boundary.

The `ngraph-pe/0` product is the emitted residual Program and its checked graph. This lane does not
cache or persist a machine-code/Wasm artifact: there is no reviewed relocation contract for that
artifact in the specialization path. The separate build key identifies the currently supported
declared-text residual-program/admission product. Any future executable artifact cache needs its
own backend-specific key and validation contract.

## Reopen conditions

Reconsider persistence only after the repository has a reviewed stable codec for the full manifests,
the emitted product and derivation evidence; an accepted trust path for entries written by a prior
process; atomic temp-file-plus-rename publication; bounded load/scratch work; corruption and restart
tests; and a relocation/linkage contract for any executable artifact. Until then, imported or
cross-process entries are unsupported and must not be treated as hits.

Negative caching remains out of scope. Reopen only with a separately justified invalidation rule
for each refusal class, including budget, cancellation, authorization and semantic failures.
