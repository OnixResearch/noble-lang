## Why

Issue #118 asks for a verified, content-addressed library of learned procedures:
synthesized Noble programs that become reusable, composable, checked artifacts,
without conflating stored code with demonstrated lifelong learning.

Four inputs already exist. This package builds on them instead of restating
them.

- #73 `typed-chunk-completion` already defines the host-owned immutable chunk
  library: the `typed-chunk/1` / `pure-i64-bool/1` envelope with exact
  canonical bodies, zero through eight immutable scalar captures, resolved
  `DefinitionRef` dependencies, ordered instantiation witnesses, exact ordered
  `I64`/`Bool` interfaces and an empty effect set; a `library_revision:
  [u8; 32]` pinned in every `CompletionRequest`; deterministic shortlex
  one-hole enumeration; one shared fail-closed budget; and four distinct
  refusal classes `Invalid`, `Unsupported`, `Exhausted` and `NoResult`
  (`.cairn/changes/typed-chunk-completion/design.md:44-114`,
  `.cairn/changes/typed-chunk-completion/specs/typed-chunk-completion/spec.md`
  TCHUNK-01 through TCHUNK-04). Its Phase 1 is complete (5/5) and its Phase 2
  implementation is untouched (0/10), so the envelope and library snapshot are
  designs, not code, and an index key or digest is explicitly not acceptance
  authority.
- P-ID-04 (`.cairn/specs/language/spec.md:2138-2141`) forbids claiming
  cross-implementation stable content hashes before a canonical byte format,
  encoding version, hash algorithm and test vectors are specified. VC-ID-03
  (`.cairn/specs/program-contracts/spec.md:469-481`) makes a digest an index or
  integrity check under the chosen encoding, never proof validity, provenance
  or authority.
- MC2 companions already implement the admission path a library entry needs: a
  deterministic core with `admit`, `begin_admission`/`complete_admission`
  (decisions from complete authentic host observations, not an effectful
  verifier), `replay` over a finite versioned rule set, `derive_compose`,
  `instantiate`, `bind`, `release`, `correspond` and `bind_artifact`
  (`crates/noble-contracts/src/companion/core.rs:12-205`), a bounded registry
  (`crates/noble-contracts/src/companion/registry/entries.rs:1-60`), a
  `RULESET_V1` with six named rules
  (`crates/noble-contracts/src/companion/rules/mod.rs:10-48`), and an
  exhaustive fail-closed `Refusal` set naming `LiveCapabilityInEvidence`,
  `UnsupportedEvidenceClass`, `ForgedStatus`, `StaleContext`,
  `WrongInstantiation`, `CyclicDerivation`, `DuplicateEntry`,
  `ExhaustedRegistry` and `UnresolvedImplication`
  (`crates/noble-contracts/src/companion/mod.rs:140-167`). The source-bound CLI
  already exposes `compose`, `instantiate`, `invoke`, `policy` and `derive`
  (`crates/noble-cli/src/core/companions.rs:25-38`).
- Evidence classes and their independence are already normative: Lean,
  Aeneas/Lean, Verus, validation, test, review and assumption
  (`.cairn/specs/program-contracts/spec.md:490`), independent
  implementation/execution/proof/trust fields (EV-STATE-01), and evidence
  binding of subject, claim, scope, toolchain, command, inputs and result
  (EV-BIND-01). A library promotion must reuse these, not invent a second
  status system.

What does not exist anywhere in the repository is the layer above a chunk: any
record binding where an artifact came from, which obligations were tested and
which were proved, whether it is currently promoted, which entries it
supersedes, what it costs, how it is retrieved deterministically, and what an
external research harness may read. Without that layer a learned-procedure
library could only be a directory of chunks plus folklore, and acceptance
criteria 2 and 3 of #118 would have nothing exact to compare.

r[LPL-01] r[LPL-02] r[LPL-03] r[LPL-04] r[LPL-05] r[LPL-06] r[LPL-07] r[LPL-08]

## What Changes

- **ADDED** `LPL-01`: a `procedure-record/1` layer over the unmodified
  `typed-chunk/1` envelope. It retains provenance, library-level dependency
  edges, per-obligation evidence class and outcome, promotion state,
  supersession and a cost profile. Identity is recomputed from the retained
  canonical fields, labelled experimental and library-local, and never
  authorizes admission. Extending #73 is the only store.
- **ADDED** `LPL-02`: composition and capability admission. A composed
  procedure is a fresh candidate admitted by ordinary source preparation and
  kernel acceptance and certified through the MC2 `ComposeV1` rule; the
  authority of a result MUST be a subset of its operands' authority under the
  consumer's policy. Capability escalation, effect widening, class escalation,
  forged status and policy-revision laundering are named refusals.
- **ADDED** `LPL-03`: promotion under a consumer-selected promotion policy that
  reuses the existing evidence classes and keeps tested and proved visibly
  distinct. A test is never relabelled a proof and a promotion never transfers
  to a changed subject.
- **ADDED** `LPL-04`: deduplication. Byte-identical records reuse one identity;
  equal signatures with different bodies stay distinct; semantic
  deduplication requires a certified equivalence for the exact pair from #56's
  lane and produces an alias edge plus a retained certificate, never a merge
  that erases an identity or a dependency.
- **ADDED** `LPL-05`: deterministic retrieval by signature and contract with a
  total order independent of map, file, insertion and wall-clock order, a
  bounded index and counted per-query examination. Retrieval yields candidate
  identities, never authority.
- **ADDED** `LPL-06`: append-only generations, versioned replacement and
  rollback. A superseded record stays addressable; a rollback republishes a
  retained closure as a new generation and refuses when any identity or
  toolchain binding no longer recomputes.
- **ADDED** `LPL-07`: reload/restart identity. Load recomputes identities,
  dependency order, library revision and retrieval order from the persisted
  canonical records and refuses on any drift; a two-process comparison of
  exported manifests is the acceptance test.
- **ADDED** `LPL-08`: read-only `noble-procedure-library-export/1` and
  `noble-procedure-library-metrics/1` records for RSI Lab, carrying
  acquisition, retention, transfer, regression, growth, retrieval overhead and
  restart-reuse measurements bound to one exact generation, with a fail-closed
  refusal of any runtime teacher fallback. No training policy enters the
  compiler.

This is a design-only change. It adds no Rust, no Lean, no canonical
specification, no generated view, no inventory, no receipt and no production
behavior. #73's chunk envelope, request, enumerator, budgets and outcome
classes are consumed verbatim rather than redefined.

## Impact

- **Files**: `.cairn/changes/learned-procedure-library/` only.
- **Rust subjects are design references, not edits**:
  `crates/noble-kernel/src/acceptance/mod.rs`,
  `crates/noble-contracts/src/source.rs`,
  `crates/noble-contracts/src/companion/core.rs`,
  `crates/noble-contracts/src/companion/mod.rs`,
  `crates/noble-contracts/src/companion/rules/mod.rs`,
  `crates/noble-contracts/src/companion/registry/entries.rs`,
  `crates/noble-cli/src/core/companions.rs`.
- **Specifications**: ADDED change-local requirements `LPL-01` through
  `LPL-08`. No `.cairn/specs/` file and no generated compatibility view
  changes.
- **Coordination**: #73 owns the chunk envelope, library snapshot and one-hole
  enumerator; #116 owns program synthesis; #56 owns the only
  certified-equivalence route that may support semantic deduplication; #74
  (`typed-chunk-ranking-experiment`) consumes the same chunk library for
  learned ranking and must not be redefined here; RSI Lab #64/#66 own the
  research protocol the export serves.

## Non-goals

No implementation, canonical encoding or hash-algorithm selection,
cross-implementation identity claim, behavioral-equivalence claim for equal
signatures, substitution of a new entry into an existing Program, deletion of
retained evidence, trainer or neural policy inside the compiler,
teacher-dependent acquisition claim, lifelong-learning claim from library
growth, performance or speedup claim, or archive of #73/#116 work.
