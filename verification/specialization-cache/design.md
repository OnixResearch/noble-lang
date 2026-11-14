# Exact-key specialization cache (`ngraph-pe/0`)

## Scope and trust boundary

The cache reuses only the residual product produced by #32's bounded static-input specializer.
It is not a calculator cache, a general artifact registry, a symbolic-analysis cache, or an
authorization mechanism. Entries live in one process, are inserted only after positive local
generation and successful ordinary-path re-admission, and are never deserialized from disk.
Hash digests select candidate entries; full manifest equality and content/provenance checks decide
whether a candidate can be considered. Every hit passes fresh `readmit` under the request's current
source limits and graph budgets. A cache hit cannot bypass current checking, policy, effects,
capture or protected-execution requirements.

Only successful entries are retained. A failed specialization, source emission, or independent
admission publishes nothing. A cache miss or capacity refusal falls back to the ordinary
specialization path; cache resource limits therefore affect reuse, not semantics. Negative results
are not cached, so a low-budget refusal cannot poison a later request. Eviction removes only the
cache's owned bytes; a returned `Residual` owns an independent reference to its immutable graph.

## Shared encoding

Both keys use the encoding proved by `NobleFoundation.CacheLookup.encodeParts`: fixed ASCII domain
tag followed by each part as an unsigned 64-bit little-endian byte length and the exact part bytes.
No concatenation without boundaries, Rust `Debug` rendering, platform-sized integer, or implicit
default participates in a key. Key manifests are retained in full. The injected identity scheme
produces a 32-byte lookup digest, but a digest collision is not a hit.

Integer subfields inside a part are fixed-width little-endian. Lists start with a `u64` element
count. Scalar interface tags are `I64 = 1`, `Bool = 2`, and other types are refused by this profile.
The static map is in ordered input-leaf order; each entry has an explicit dynamic/static tag, and a
static I64 stores all eight two's-complement little-endian value bytes. All profile/revision strings
are UTF-8 byte strings. The full original recipe bytes supplied by the caller and the checked graph's
logical identity are both bound.

## Specialization identity

Domain tag: `noble.ngraph.specialization-key.v1`.

Length-prefixed parts, in order:

1. exact original graph/recipe bytes;
2. checked original `GraphLogicalId`;
3. ordered original interface (slot kind and scalar type);
4. ordered static-position/value map (dynamic positions included);
5. ordered resolved subject/capture closure as carried by the checked graph;
6. semantic, fact, graph-IR and effect/profile revisions;
7. the checked assumptions (the current PE slice rejects graph facts and effects, so the admitted
   assumption set is explicitly empty rather than omitted);
8. specializer/accounting revision and the explicit option set (empty in `ngraph-pe/0`);
9. every graph/source/specialization budget that can affect admission or output.

The manifest conservatively includes all `Budgets` fields and all source `Limits` fields, not just
the caps currently charged by #32. This can cause safe false misses when an unrelated limit changes;
it prevents reuse across any stricter admission decision. Static values are typed and positional,
so equal bytes at different positions or under a different interface are not interchangeable.
Immutable parameter snapshots are not part of this profile because static specialization currently
accepts input I64 values only; a future snapshot-as-static profile must bind the snapshot identity
and bytes explicitly.

## Program-build identity

Domain tag: `noble.ngraph.program-build-key.v1`.

Length-prefixed parts, in order:

1. exact canonical residual graph bytes and emitted residual source bytes;
2. target and representation (`noble-core-v1`, `declared-text-v1`);
3. builder/backend identity and version (`noble-kernel/residual-readmit-v1`);
4. exact backend/admission flags and all source/graph budgets and limits;
5. runtime contract (`noble-core-effect-profile-v1`);
6. ordered residual kernel ABI (the interface type tags above, bottom-first);
7. compiler epoch (`ngraph-ir/0`, `ngraph-sem/0`, `ngraph-facts/0`, `ngraph-pe/0`,
   `ngraph-layered/0`, and the readmission contract revision).

This is distinct from the specialization key: identical residual semantics under different targets,
representations, flags, runtime/ABI or compiler epochs have different build identities. The current
cache product is the source-level residual Program and its freshly admitted graph. It does not
retain a raw process-local executable handle or claim to reuse a machine-code artifact. Any future
backend artifact cache must add its actual backend flags, runtime helpers, compiler/assembler/
optimizer versions and relocation/linkage state before admitting artifact reuse.

## Pinned deterministic fixtures

These fixtures pin the shared Rust/model `encodeParts` bytes (the index digest is intentionally not
the key). The string parts below are ASCII; the expected hex includes the literal domain tag and
each eight-byte little-endian part length.

### Specialization-key fixture

Domain: `noble.ngraph.specialization-key.v1`.

Parts, exactly: `ngraph-ir/0 ngraph-i64/0 graph bytes`; `original-interface=i64,i64`;
`static-map=0:i64:7,1:dynamic`; `ngraph-sem/0`; `ngraph-facts/0`; `ngraph-pe/0`;
`budgets=default/0`.

Expected encoded bytes (lowercase hex):

```text
6e6f626c652e6e67726170682e7370656369616c697a6174696f6e2d6b65792e7631
24000000000000006e67726170682d69722f30206e67726170682d6936342f30206772617068206279746573
1a000000000000006f726967696e616c2d696e746572666163653d6936342c693634
1c000000000000007374617469632d6d61703d303a6936343a372c313a64796e616d6963
0c000000000000006e67726170682d73656d2f30
0e000000000000006e67726170682d66616374732f30
0b000000000000006e67726170682d70652f30
1100000000000000627564676574733d64656661756c742f30
```

### Program-build-key fixture

Domain: `noble.ngraph.program-build-key.v1`.

Parts, exactly: `residual graph bytes`; `target=noble-core-v1`;
`representation=declared-text-v1`; `backend=noble-kernel/0`;
`flags=default-limits-and-budgets/0`; `runtime=noble-core/0`; `abi=i64->i64`;
`compiler=ngraph-layered/0`.

Expected encoded bytes (lowercase hex):

```text
6e6f626c652e6e67726170682e70726f6772616d2d6275696c642d6b65792e7631
1400000000000000726573696475616c206772617068206279746573
14000000000000007461726765743d6e6f626c652d636f72652d7631
1f00000000000000726570726573656e746174696f6e3d6465636c617265642d746578742d7631
16000000000000006261636b656e643d6e6f626c652d6b65726e656c2f30
2200000000000000666c6167733d64656661756c742d6c696d6974732d616e642d627564676574732f30
140000000000000072756e74696d653d6e6f626c652d636f72652f30
0c000000000000006162693d6936342d3e693634
1900000000000000636f6d70696c65723d6e67726170682d6c6179657265642f30
```

These fixtures pin the encoding schema and do not claim a Rust/Lean theorem about cache behavior,
partial-evaluation correctness, or runtime equivalence.

The Rust test also pins the shared Lean `encodeParts` fixture under its exact model tag,
`noble.ngraph.subject.v0`, with parts `specialization-cache-fixture`, `v1`, and `bytes`. The
expected bytes are:

```text
6e6f626c652e6e67726170682e7375626a6563742e7630
1c000000000000007370656369616c697a6174696f6e2d63616368652d66697874757265
02000000000000007631
05000000000000006279746573
```

This cross-language fixture is the concrete Rust-to-model encoding correspondence; the two cache
key fixtures above additionally pin their distinct production domains and field order.

## Resource and stage accounting

The cache bounds entry count, variants per original recipe, manifest bytes, retained canonical
graph/source/key/provenance bytes, lookup candidates examined and eviction candidates examined.
Scratch manifests exceeding their bound bypass caching before key construction; insertion that
cannot fit evicts bounded old entries or declines insertion. Returned graph owners are independent
of eviction. Cache-hit admission and historical specialization/build/execution costs are separate
stages. A hit does not claim that its work or resource use equals the first generation's.

The differential receipt reports cache-disabled, cold-miss and warm-hit execution agreement, with
lookup, specialization, source emission, current admission, cache publication, backend compilation,
execution and retained-byte costs reported as distinct observations. It is finite implementation evidence, not a specialization
equivalence proof or a performance generalization beyond the measured corpus.

The opt-in integration-test receipt producer lives under the existing B2 test target and records
the exact source revision only after verifying that its checkout is clean. The library's
`Session::specialize_with_observer` reports synchronous begin/end events for lookup, #32
specialization, source emission, independent admission and publication; the harness times those
boundaries separately from backend compilation and execution. Reproduce it with the repository's pinned Rust toolchain:

```sh
NOBLE_CACHE_RECEIPT_OUT="<new directory outside the repository>/specialization-cache.json" \
  cargo test --offline --locked -p noble-ngraph-experiment --test extract_b2 \
  source_bound_cache_receipt -- --ignored
```

The output is measurement evidence for this retained three-fixture, 3,000-vector corpus only.
