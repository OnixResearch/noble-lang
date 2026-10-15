# Proof-bound artifact admission envelope

## ADDED Requirements

### Requirement: ACORR-ENV-01
r[ACORR-ENV-01]

An opt-in proof-bound artifact admission for the MC2 pure quotation profile MUST be decided by an independent consumer over one envelope. The profile is a resource-free Core-Bootstrap program with one closed quotation root, an empty dependency closure, an MC1 partial-correctness contract, `LeanExact` evidence, and at most one selected guard wrapper.

The envelope MUST bind:
- the owner-frozen law revision and statement;
- the consumer-selected source bytes and canonical recipe events;
- the semantic and IR revisions;
- the full regenerated statement (not only a 64-bit digest);
- the evidence bytes;
- checker binary and library identities;
- policy revision;
- the guard template and wrapper;
- the CLI, every embedded runtime helper, and the Node, wasm-tools and wasm-opt identities, each compared against policy pins;
- the consumer-selected optimization mode and flags;
- the final subject and wrapper Wasm bytes;
- the exact in-memory buffers passed to the engine.

Producer-supplied digests, source claims, tool claims, trust flags and optimization claims MUST be discarded. All checks MUST precede instantiation. Any mismatch MUST refuse with a distinct code and zero guest requests and protected operations. Only the compared buffers MAY be instantiated, in a fresh engine.

Host authorization MUST remain a separate invocation check outside the envelope identity. Ordinary unproved execution MUST remain available and labeled as such.

### Requirement: ACORR-CACHE-01
r[ACORR-CACHE-01]

A cached envelope acceptance MUST be keyed by every envelope field except host authorization, and any field change MUST force full revalidation. Compiled code MUST NOT be reused across admissions.

The optimization mode MUST be a consumer selection bound into the envelope; a candidate built under another mode refuses. Proof-bearing and proof-erased sources MUST compile to identical subject bytes under the same envelope. A guard wrapper MUST NOT be erased: when the precondition is not `true`, only the bound wrapper MAY be invoked as a certified call.

### Requirement: ACORR-CTRL-01
r[ACORR-CTRL-01]

Before any acceptance of the envelope profile, the following negative controls MUST be executed against a frozen consumer and retained with immutable source, tool and proof receipts:
- correct proof with wrong body;
- stale dependency or tool/helper version;
- weakened claim;
- code/recipe divergence;
- changed guard;
- altered runtime helper or import;
- post-check artifact replacement;
- producer metadata forgery;
- an optimization-mode or cache change.

The negative candidates MUST be produced by a preparation path independent of the admission code. Existing S-CASE-07, CONTRACT-13 and CONTRACT-22/26 controls MUST be retained unchanged rather than relabeled as new coverage.
