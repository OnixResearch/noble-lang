## Context

This package is grounded at worktree HEAD `fed77835` (2026-11-13). Issue #118
was filed against the same repository state; the cited facts below were
re-read at this HEAD, not inherited from the issue text. The package adds a
design record and no executable behavior.

### #73 is the store

`.cairn/changes/typed-chunk-completion/` already owns the host-owned immutable
chunk library. Its Phase 1 is complete (5/5) and its Phase 2 implementation is
untouched (0/10), so the envelope, request, enumerator and budgets exist as
normative design text only. The facts this package consumes verbatim:

- the `typed-chunk/1` / `pure-i64-bool/1` envelope fixed in
  `design.md:52-96`, including canonical recipe nodes, zero through eight
  immutable scalar captures, resolved `DefinitionRef` dependencies and ordered
  instantiations;
- the rule that `chunk_id` and any digest are lookup aids only, with admission
  recomputing canonical bytes (`design.md:44-46`);
- the `CompletionRequest` field `library_revision: [u8; 32]`
  (`design.md:103-114`) and the requirement that the library snapshot and that
  revision are immutable for one request (`design.md:193-195`);
- the canonical library order `(stack_in, stack_out, effects, chunk_id)` used by
  shortlex enumeration (`design.md:140-152`);
- the shared budget table and the four distinct outcome classes `Invalid`,
  `Unsupported`, `Exhausted` and `NoResult` (`design.md:160-192`,
  `specs/typed-chunk-completion/spec.md` TCHUNK-03);
- TCHUNK-01's rule that equal signatures do not merge behavior and that a
  signature match transfers no evidence and authorizes no substitution.

Extending that library is the only store. This package adds a record layer and
adds no second envelope, library directory, enumerator or admission route.

### Identity is experimental, not canonical

P-ID-04 (`.cairn/specs/language/spec.md:2138-2141`) requires stored IDs and
encodings to be labelled experimental until a canonical byte format, encoding
version, hash algorithm and test vectors are specified, and forbids claiming
cross-implementation stable content hashes. VC-ID-03
(`.cairn/specs/program-contracts/spec.md:469-481`) makes a digest an index or
integrity check under the chosen encoding, never proof validity, provenance or
authority. VC-VALUE-03 (`:983-1008`) requires serialization to export inert
descriptions whose import re-establishes validity in the receiving context, and
invalidates applicability when the policy, a semantic dependency or a relevant
environment fact changes.

This package therefore fixes the *field set and field order* of a record, the
recomputation rules, and the determinism requirements, and deliberately does
not select a canonical language encoding or a language hash algorithm. Each
record and each export carries the encoder identifier and encoding revision it
was produced under; reload determinism is claimed only within one encoder
version and toolchain.

### Admission, evidence and status already exist

- The deterministic companion core decides admission from an opaque request and
  a complete host-check observation; the source-bound CLI runs the isolated
  independent Lean consumer on the exact expected statement and submitted
  evidence bytes
  (`.cairn/specs/program-contracts/spec.md:791`). Its API is
  `admit`, `begin_admission`/`complete_admission`, `replay`, `derive_compose`,
  `instantiate`, `bind`, `release`, `correspond`, `bind_artifact`, `observe`
  and `compose_subject`
  (`crates/noble-contracts/src/companion/core.rs:12-205`,
  `crates/noble-contracts/src/companion/mod.rs:82`).
- The rule set is finite and versioned: `RULESET_V1 = 1` with
  `AdmitLeanV1`, `ComposeV1`, `InstantiateV1`, `GuardV1`, `ProjectV1` and
  `InvokeV1`, and a derivation carries its rule, contract, statement, subject
  and bounded premise list (`crates/noble-contracts/src/companion/rules/mod.rs:10-60`).
- The registry is bounded by the supplied limits and identical entries reuse
  the existing identifier instead of growing
  (`crates/noble-contracts/src/companion/registry/entries.rs:1-60`).
- Refusals are exhaustive and fail-closed, and include
  `LiveCapabilityInEvidence`, `UnsupportedEvidenceClass`, `ForgedStatus`,
  `StaleContext`, `WrongInstantiation`, `CyclicDerivation`, `DuplicateEntry`,
  `MismatchedContext`, `MismatchedSubject`, `UnresolvedImplication`,
  `ExhaustedRegistry` and `ExhaustedReplay`
  (`crates/noble-contracts/src/companion/mod.rs:140-167`).
- Selecting a different policy bumps the context revision so evidence admitted
  under the old policy reports `StaleContext` until revalidated
  (`crates/noble-contracts/src/companion/core/context.rs:19-26`).
- Evidence classes are already the seven named ones — Lean, Aeneas/Lean,
  Verus, validation, test, review and assumption
  (`.cairn/specs/program-contracts/spec.md:483-494`) — and evidence must bind
  subject, claim, source scope, semantic dependencies, policy, toolchain,
  target/features, command, inputs, assumptions and result (EV-BIND-01,
  `.cairn/specs/evidence/spec.md:280-315`).
- Scenario status keeps implementation, execution, proof and trust
  independent (EV-STATE-01, `.cairn/specs/evidence/spec.md:71-105`). Proof
  attachment must not change program identity, and inspection must not fetch a
  proof, start a prover or acquire authority (VC-VALUE-05, `:1033-1052`).
  Tactics, solvers and AI-generated proofs remain untrusted producers
  (VC-TOOL-03, `:1254-1280`).
- No live service capability, resource reference or session handle may hide
  inside supposedly pure evidence (VC-VALUE-04, `:1008-1033`), and no
  `Build`-mode or optional policy may disable a required host or admission
  check (VC-ADMIT-05, `:655-700`).

### What is missing

Nothing in the repository binds, above a chunk: its provenance and producer
tool revision, its per-obligation evidence classes and outcomes, its promotion
state, what it supersedes, its integer cost, its library-level dependency edges,
a deterministic retrieval order, an append-only generation and rollback record,
a restart-identity guarantee and a read-only export for RSI Lab. #74
(`.cairn/changes/typed-chunk-ranking-experiment/`) consumes the same chunk
library for learned ranking and is not redefined here.

## Decisions

### Decision: extend the #73 library; add no second store

One store, two views. A library generation contains the #73 chunk envelopes in
their canonical order. The procedure layer is a total map from admitted
`chunk_id` to one `procedure-record/1` record; a chunk admitted without a
record remains a valid #73 chunk and is not visible to the procedure layer.
Derived revisions are computed from the same generation rather than stored as
competing authorities:

| Value | Computed over | Consumer |
|---|---|---|
| `chunk_library_revision` | ordered chunk envelopes, order `(stack_in, stack_out, effects, chunk_id)` | #73 `CompletionRequest.library_revision`, unchanged |
| `procedure_library_revision` | ordered procedure records, retrieval total order | this package's exports and requests |
| `generation` | monotonic counter | both views |

A procedure-layer request pins `procedure_library_revision`; the #73
enumerator keeps receiving `chunk_library_revision`. Both are recomputed at
load, and both MUST agree with the retained values. This keeps #73's field
meaning intact and prevents two competing revision conventions. The follow-up
implementation MUST expose the ordered admitted chunk envelopes (or their
canonical bytes) so the procedure layer can derive `chunk_library_revision`
without re-encoding a body.

### Decision: identities remain experimental, versioned and library-local

The record fixes field order and recomputation; it selects no canonical byte
encoding and no language hash (P-ID-04). Every record and export carries
`encoder: { id, encoding_revision, experimental: true }`. The identity of a
record is the tuple of its canonical retained fields; the digest is its
experimental representation under the recorded encoder. Consequences:

- a supplied digest, id, signature, name or path is never authority; admission
  recomputes and compares;
- reload determinism is claimed only within one encoder revision and toolchain;
- an encoder change produces a new encoding revision and requires re-admission;
  it MUST NOT be presented as the same identity;
- no export or report may call the digest a content hash of a canonical Noble
  encoding.

### Decision: promotion reuses existing evidence classes; tested and proved stay visible

Promotion never introduces a second status system. The record keeps one entry
per obligation, each with its own class and outcome from the seven existing
classes, and the promotion state is a summary that a consumer-selected
promotion policy evaluates. A `test` outcome can never be reported as `proved`;
`assumption` and `review` stay visible and are never counted as proof;
`forged-status` is the refusal for a claimed `proved` without a matched
proof-class record. Promotion is not evidence: a retained evidence reference
alone does not establish validity (VC-VALUE-05).

### Decision: capability admission goes through the MC2 companion path

The library is an effectful host shell. It performs IO, canonicalization and
index construction; it makes no admission decision by itself. Every decision
that a record is admissible, composable or invocable is the deterministic
core's, from a complete host observation, under a consumer-selected policy
(`.cairn/specs/program-contracts/spec.md:791`). The library holds references to
contracts and evidence, never handles: a record cannot carry a live capability,
and a policy change revalidates rather than upgrades.

### Decision: the export is a manifest plus a separate metrics record

A byte-comparable identity manifest must not contain time, path, process or
floating-point data, while RSI Lab needs measured counters. The two are
separated: `noble-procedure-library-export/1` is deterministic and is the
artifact whose bytes are compared across restarts;
`noble-procedure-library-metrics/1` is bound to the manifest digest and carries
elapsed time and counters. A transfer claim requires `teacher_calls == 0`.

## Procedure record

`procedure-record/1`, semantic revision `0`, profile `pure-i64-bool/1`
(inherited from TCHUNK-01). Canonical field order:

```
format: u32 = 1
semantic_revision: u32 = 0
encoder: { id: string, encoding_revision: u32, experimental: true }
procedure_id: 32 raw bytes
chunk: typed-chunk/1 envelope (unmodified; TCHUNK-01)
provenance: { producer: chunk|synthesis|composition,
              producer_tool_revision: u32,
              task_id: 32 raw bytes | none,
              grammar_revision: u32 | none }
library_dependencies: [ { procedure_id: 32 bytes, chunk_id: 16 bytes,
                          source_revision: 32 bytes } ]
evidence: [ { obligation: u32,
              class: lean|aeneas-lean|verus|validation|test|review|assumption,
              outcome: proved|disproved|unknown|timeout|unsupported|error|not-run|passed|failed,
              toolchain: [ string ],
              claim: string,
              subject_binding: 32 bytes,
              policy_revision: u32 } ]
status: candidate|tested|promoted|superseded
promotion: { policy_revision: u32, obligations: [ u32 ] } | none
cost: { expanded_nodes: u32, emitted_source_bytes: u32, evaluation_steps: u32 }
retrieval: { stack_in: [I64|Bool], stack_out: [I64|Bool], effects: [],
             status_rank: u8, cost_key: [u32; 3] }
supersedes: procedure_id | none
alias_of: { procedure_id: 32 bytes, certificate: 32 bytes } | none
```

Notes that are normative rather than descriptive:

- `chunk` is the #73 envelope, not a re-encoding of it. `chunk.dependencies`
  (`DefinitionRef`) and `library_dependencies` (procedure edges) are different
  relations and both are retained.
- **Identity scope.** `procedure_id` is recomputed over the immutable subject
  fields only: `encoder`, `semantic_revision`, `chunk` and `provenance`.
  `evidence`, `status`, `promotion`, `cost`, `retrieval`, `supersedes` and
  `alias_of` are mutable library state bound to that identity inside a
  generation. Attaching or replacing a proof of unchanged behavior must not
  change the artifact identity (VC-ID-01, VC-VALUE-05), so a promotion or a new
  evidence record is a new generation of state, not a new artifact.
- `evidence` is ordered by `(obligation, class rank)` where the class rank is
  the order listed above; `subject_binding` and `policy_revision` implement
  EV-BIND-01 and make a changed subject or policy visible instead of reusable.
- `cost` carries integers only, and its key is compared lexicographically;
  measured wall time is never part of a record.
- `retrieval` is derived on admission and MUST equal the value recomputed from
  the other fields.
- `promotion` names the obligations the consumer policy demanded; it is not a
  boolean and it does not imply that unlisted obligations hold.

Library-level bounds (all design maxima; a host may lower, never raise):

| Resource | Maximum |
|---|---:|
| library entries / procedure records | 64 (TCHUNK-01) |
| retained library bytes | 262,144 (TCHUNK-03) |
| library dependency edges per record | 16 |
| transitive dependency steps per admission | 256 (TCHUNK-03) |
| evidence records per record | 16 |
| retained library generations | 16 |
| index entries | 64 |
| index bytes | 65,536 |
| retrieval queries per request | 32 |
| examined candidates per query | 128 |
| retrieval work units per request | 4,096 |
| identity manifest bytes | 262,144 |
| metrics record bytes | 65,536 |

Over-budget results are `Exhausted`, never truncation. The existing TCHUNK-03
counters continue to apply to every expansion and check the library performs;
a record boundary never resets a meter.

## Composition and capability admission

A composition is a fresh candidate, never an edit:

```
record A, record B, explicit consumer context
  -> candidate source composed at exact ordered interfaces
  -> Session::prepare (ordinary parse, resolve, check)
  -> acceptance::check (ordinary kernel admission, same Request limits)
  -> Core::derive_compose(A.evidence, B.evidence, composed subject)
  -> record C (producer = composition, library_dependencies = [A, B])
```

`Core::derive_compose` supplies the prechecked intermediate implication
internally; when the left output interface does not join the right input
interface, `compose_subject` reports `UnresolvedImplication` and no certified
result exists (VC-LIB-02, `crates/noble-contracts/src/companion/mod.rs:82`).
Plain composed programs may still be prepared and run under ordinary interface
matching; they carry no certified claim (`VC-USE-01`, `:1052-1085`).

Authority is the explicit part of the contract that a consumer must allow.
For the initial resource-free profile every record's authority set is empty.
The rule is stated generally so it does not need re-derivation later: the
authority set of a composed or instantiated result MUST be a subset of the
union of its operands' authority sets under the consumer's selected policy.

| Escalation vector | Where it appears | Refusal |
|---|---|---|
| nonempty effect set or host effect | `chunk.interface.effects` | `Unsupported` (TCHUNK-01 closure) |
| live capability, service handle or session in evidence | `evidence[]` | `LiveCapabilityInEvidence` |
| evidence class outside the consumer policy | `evidence[].class` | `UnsupportedEvidenceClass` |
| `proved` claimed on a test/review/assumption record | `evidence[].outcome` | `ForgedStatus` |
| instantiation the independent recheck refuses | `chunk.instantiations` | `WrongInstantiation` |
| later, foreign or cyclic library dependency | `library_dependencies` | `CyclicDerivation` / `Invalid` |
| composed result needs an operand-absent authority or host contract | composed subject | `MismatchedSubject` / `MismatchedContext` |
| consumer keeps old evidence after a policy change | `Core::set_policy` | `StaleContext` |
| restating an existing derivation to mint a second entry | `derive` | `DuplicateEntry` |
| unbounded registry or index growth | admission | `ExhaustedRegistry` |
| guard precondition not established for the actual argument | `invoke` | fail closed before the body (VC-USE-01) |

The library MUST NOT raise its own policy revision, accept a record field as
policy, or convert a signature match into a companion entry. A record that
carries a live capability is refused before retention; a record that merely
claims a class the policy disallows is refused with the exact class and field.

## Promotion

```
candidate --(test evidence for the exact subject/fixtures)--> tested
tested    --(consumer policy obligations all satisfied)-----> promoted
promoted  --(stale policy/toolchain, failed recheck)--------> recorded demotion
any       --(superseded by a newer record)------------------> superseded
```

`tested` is an *executed* statement about fixtures and never a proof.
`promoted` is a policy decision over obligations, each with its own evidence
class and outcome. A promotion record names the policy revision and the
obligation identifiers it relied on. A toolchain change, policy change or any
subject change makes the promotion stale until revalidated; the record is
republished at the demoted state (`tested` or `candidate`) in a new generation,
never edited, and its `procedure_id` does not change because the subject did
not. Evidence inspection does not start a prover or fetch proof bytes, and
holding a reference to a proof does not establish validity (VC-VALUE-05).

## Deduplication

Four distinct results, never conflated:

1. **Identical subject and state** — every identity field and the whole
   evidence/promotion/cost state equal under the same encoder: reuse the
   existing `procedure_id`, add no entry, mint no evidence, change no state.
   This mirrors the bounded registry's existing reuse behavior
   (`registry/entries.rs:29-56`).
2. **Identical subject, changed state** — same identity fields with additional
   or different evidence, promotion or cost: keep the same `procedure_id`,
   append evidence records without overwriting a retained one, and publish in a
   new generation. Proof attachment never creates a second artifact.
3. **Restated derivation** — a derivation that restates an existing premise:
   `DuplicateEntry` (`crates/noble-cli/src/core/companions.rs:32-38`).
4. **Semantic alias** — only with a certificate from #56's
   certified-equivalence lane for the exact pair: add an alias edge plus the
   certificate as its own evidence record. Both identities, both dependency
   closures and both retrieval positions remain. No substitution into an
   existing Program, no deletion, no rewrite.

Equal signatures with different bodies are never merged, never deduplicated and
never treated as behaviorally equal (TCHUNK-01). A name, interface, cost,
template identity, path or nearest-body similarity is not a deduplication key.

## Retrieval

Retrieval is a pure function of one generation and one query. Total order:

```
(stack_in tags lexicographically by type tag)
  -> (stack_out tags)
  -> effects
  -> status_rank: promoted 0, tested 1, candidate 2, superseded 3
  -> (expanded_nodes, emitted_source_bytes, evaluation_steps), absent last
  -> procedure_id bytes
```

Queries: exact interface; composition-compatible interfaces for a one-hole
context (the #73 stack equation, not a suffix assumption); dependency closure;
promotion state. Filters are applied to the canonically ordered sequence, so a
filtered result is always an ordered subsequence. The index is rebuilt from the
records at load and is never persisted as authority; the persisted index-order
manifest exists only for the reload comparison.

Retrieval returns identities, the examined-entry count and nothing else: no
program, no admission decision, no applicability guarantee, no capability. A
retrieved candidate still passes `Session::prepare`, `acceptance::check` and
the companion checks for the actual composition.

## Generations, replacement and rollback

```
GenerationRecord {
  generation: u64,
  kind: publish | rollback,
  parent_generation: u64 | none,
  rollback_of: u64 | none,
  chunk_library_revision: 32 bytes,
  procedure_library_revision: 32 bytes,
  closure: [ { procedure_id: 32 bytes, state_digest: 32 bytes } ],
  encoder: { id, encoding_revision }
}
```

Generations are append-only and retained up to the bound above. Replacement
adds a record with `supersedes` and keeps the older record addressable. A
rollback republishes a retained generation's exact closure as a new generation
with `kind = rollback`, `parent_generation = restored`, `rollback_of =
abandoned`. Because the closure binds a state digest as well as an identity, a
rollback restores the retained evidence sets and promotion states of that
generation without changing any `procedure_id`. It refuses `Invalid` if any
recomputed identity, state digest, dependency closure, encoder revision or
evidence toolchain identity differs from the retained value or is unavailable.
It mutates no Program, record, generation, export or consumer evidence. A
generation referenced by a retained export or by a consumer's selected closure
is not deleted.

## Reload and restart identity

Persisted: canonical records, generation records, encoder/profile/semantic
identifiers. Load recomputes, in this order: each record's canonical bytes and
`procedure_id`; each identity digest under the recorded encoder; each evidence
set and promotion state against its generation's `state_digest`; each
`library_dependencies` order and closure; `procedure_library_revision` and
`chunk_library_revision`; the retrieval order. Each value must equal the
retained value, else `Invalid` naming the first differing field, with no
partial publication.

Forbidden inputs in the recorded form: wall-clock or monotonic time, absolute
or relative path spelling, process id, host name, hash-container iteration
order, thread scheduling, locale, floating-point values. Collections are
serialized in declared canonical order (dependencies retained order; evidence
by obligation then class rank; entries by retrieval order). The guarantee is
within one encoder revision and toolchain; no cross-implementation stability is
claimed (P-ID-04).

Acceptance test shape: process 1 writes the library and exports the manifest;
processes 2 and 3 load it independently and export; the three manifests are
compared byte for byte. Negative controls: flip one byte in a retained body,
reorder two dependency edges on disk, change the encoder revision, and delete
an evidence record — each must refuse at the first differing field and leave
the selected generation unchanged.

## RSI Lab export

`noble-procedure-library-export/1` (deterministic identity manifest):

```json
{
  "schema": "noble-procedure-library-export/1",
  "schema_revision": 0,
  "profile": "pure-i64-bool/1",
  "semantic_revision": 0,
  "encoder": {"id": "lpl-id/0", "encoding_revision": 0, "experimental": true},
  "library": {
    "generation": 3,
    "kind": "publish",
    "parent_generation": 2,
    "chunk_library_revision": "<64 hex>",
    "procedure_library_revision": "<64 hex>",
    "closure": [{"procedure_id": "<64 hex>", "state_digest": "<64 hex>"}]
  },
  "entries": [
    {
      "procedure_id": "<64 hex>",
      "identity_digest": "<64 hex>",
      "chunk_id": "<32 hex>",
      "source_revision": "<64 hex>",
      "interface": {"stack_in": ["I64"], "stack_out": ["I64"], "effects": []},
      "captures": 1,
      "dependencies": ["<64 hex>"],
      "instantiations": 0,
      "provenance": {"producer": "synthesis", "tool_revision": 1, "task_id": null},
      "status": "tested",
      "evidence": [
        {"obligation": 2, "class": "test", "outcome": "passed"},
        {"obligation": 3, "class": "lean", "outcome": "not-run"}
      ],
      "cost": {"expanded_nodes": 12, "emitted_source_bytes": 240, "evaluation_steps": 64},
      "supersedes": null,
      "alias_of": null
    }
  ],
  "retrieval_order": ["<64 hex>"],
  "non_claims": ["experimental library-local identity; not a canonical Noble hash"]
}
```

`noble-procedure-library-metrics/1` (measured, bound to the manifest digest):

```json
{
  "schema": "noble-procedure-library-metrics/1",
  "schema_revision": 0,
  "manifest_digest": "<64 hex>",
  "generation": 3,
  "index": {"entries": 12, "bytes": 2048, "build_steps": 120, "build_monotonic_ns": 0},
  "queries": {"count": 8, "examined_total": 96, "max_examined": 12},
  "growth": {"entries_added": 3, "bytes_added": 512},
  "acquisition": [{"family": "arith", "entries": 2, "tasks": 40}],
  "retention": [{"family": "arith", "surviving_generations": 2}],
  "transfer": [{"arm": "accumulated", "held_out_tasks": 24, "passed": 0}],
  "regression": [{"family": "arith", "earlier_tasks": 40, "failed": 0}],
  "restart": {"identity_match": true},
  "proposer_calls": 0,
  "teacher_calls": 0
}
```

Both records are read-only and contain no training policy, capability, captured
secret or live handle. A transfer field is only valid when `proposer_calls` and
`teacher_calls` are zero and the harness record shows proposer and teacher
disabled; an attempted teacher call during such a run fails closed and
invalidates the transfer field, while growth is still reported. Model and
synthesis behavior, executed library mechanics, artifact correspondence and
explicit assumptions stay in separate fields.

## Test plan for acceptance 1–3

The plan names the exact interfaces it consumes. It is a design; no row is
executed by this package, and the implementation rows are blocked as noted.

Consumed interfaces:

| Interface | Where | Used for |
|---|---|---|
| `typed_chunk` library snapshot, `ChunkEnvelope`, `CompletionRequest.library_revision` | #73 `design.md:44-114` | the record's `chunk` and the chunk view of a generation |
| one-hole enumerator and shared budget report | #73 TCHUNK-02/TCHUNK-03 | reuse and composition examples; candidate and expansion counts |
| `Session::prepare`, `Prepared::checked`, `Prepared::output` | `crates/noble-contracts/src/source.rs:452-520` | ordinary reparse and interface derivation |
| `Session::commit` | `crates/noble-contracts/src/source.rs:1155` | explicit publication only; never called by completion or retrieval |
| `acceptance::check` | `crates/noble-kernel/src/acceptance/mod.rs:55` | kernel admission of each final candidate |
| `Core::admit`, `begin_admission`, `complete_admission` | `crates/noble-contracts/src/companion/core.rs:12-41` | record admission from complete host observations |
| `Core::replay`, `Core::derive_compose`, `Core::instantiate`, `Core::bind`, `Core::release`, `Core::correspond` | `crates/noble-contracts/src/companion/core.rs:42-200` | certified composition, instantiation, applicability |
| `RuleId` / `RULESET_V1` | `crates/noble-contracts/src/companion/rules/mod.rs:10-48` | rule identity in every derivation |
| `Refusal` variants | `crates/noble-contracts/src/companion/mod.rs:140-167` | rejection classes named in the fixtures |
| synthesis producer record (candidate source, grammar revision, budget, witness/counterexample trace, verifier status) | #116 scope | provenance of synthesized entries and held-out task generation |
| certified equivalence certificate | #56 lane | the only semantic deduplication input |

Acceptance 1 — reuse and composition examples, rejection tests:

| Fixture | Setup | Expected |
|---|---|---|
| `LPL-T01` reuse | one admitted record retrieved by exact interface and used in a #73 one-hole task | `Accepted`; the emitted source reparsed and `acceptance::check` called; record and library unchanged |
| `LPL-T02` two-record composition | two records with joining ordered interfaces | `derive_compose` returns a derived evidence entry; `bind` returns the canonical composed subject; new record has `library_dependencies = [A, B]` |
| `LPL-T03` invalid contract | composed boundary does not join, or consumer's expected interface differs | `UnresolvedImplication` / `MismatchedSubject`; no certified result; plain program may exist uncertified |
| `LPL-T04` effect widening | record or result declares a nonempty effect set | `Unsupported` at `chunk.interface.effects` |
| `LPL-T05` capability escalation | live capability in `evidence`; disallowed class; `proved` on a test record | `LiveCapabilityInEvidence`; `UnsupportedEvidenceClass`; `ForgedStatus` |
| `LPL-T06` policy laundering | evidence admitted under a different policy revision | `StaleContext` until fresh observations; a record field cannot change policy |
| `LPL-T07` dedup split | byte-identical admission; then two equal-signature different-body records | same `procedure_id` and unchanged count; then two distinct ids |
| `LPL-T08` dependency abuse | forward or cyclic `library_dependencies`; forged dependency digest | `CyclicDerivation` / `Invalid` at the exact edge |

Acceptance 2 — reload/restart identity:

| Fixture | Setup | Expected |
|---|---|---|
| `LPL-T09` two-process equality | writer process plus two reader processes | identity manifests byte-identical, including digests, dependency order, both revisions, retrieval order |
| `LPL-T10` byte mutation | one body byte changed on disk | `Invalid` at the first differing field; no partial publication; selected generation unchanged |
| `LPL-T11` dependency reorder | two dependency edges swapped on disk | `Invalid` at `library_dependencies`; neither order accepted silently |
| `LPL-T12` encoder drift | encoder revision changed without re-admission | `Invalid`; no identity reuse across encoding revisions |

Acceptance 3 — transfer on held-out composed tasks:

| Fixture | Setup | Expected |
|---|---|---|
| `LPL-T13` preregistration | frozen protocol before any run: task families, held-out composition split, four arms, paired seeds, success threshold | committed criterion, no post-hoc threshold |
| `LPL-T14` arms | no library; frozen hand-written library; accumulated learned library; size-matched non-transfer control | per-arm held-out success, growth, retrieval overhead, wall and candidate counts |
| `LPL-T15` no teacher fallback | evaluation with proposer and teacher disabled; a deliberately attempted call | run reports `teacher_calls = 0`; the attempt fails closed; a nonzero count invalidates the transfer claim |
| `LPL-T16` regression and restart | earlier families re-run after each update; restart before follow-on tasks | regression counts and restart identity match reported per arm |

Status of the plan: `LPL-T01`–`LPL-T12` require #73 Phase 2 (TCHUNK-01/02
implementation) plus the record layer; `LPL-T13`–`LPL-T16` additionally require
#116's synthesis producer or an equivalent producer contract. No row is
claimed as executed.

## Acceptance mapping

| Issue #118 criterion | Design location |
|---|---|
| reuse and composition examples | LPL-01, LPL-02; `LPL-T01`, `LPL-T02` |
| rejection tests for invalid contracts | LPL-02; `LPL-T03`, `LPL-T04`, `LPL-T08` |
| rejection tests for capability escalation | LPL-02; `LPL-T05`, `LPL-T06` |
| reload/restart produces identical identities and dependencies | LPL-06, LPL-07; `LPL-T09`–`LPL-T12` |
| transfer on held-out composed tasks, no runtime teacher fallback | LPL-08; `LPL-T13`–`LPL-T15` |
| reporting library growth and retrieval overhead | LPL-05, LPL-08; `LPL-T14`, `LPL-T16` |
| semantic deduplication where checkable | LPL-04 |
| deterministic retrieval, versioned replacement and rollback | LPL-05, LPL-06 |
| exported integration contract, no training policy in the compiler | LPL-08 |

## Proof obligations

This package proves none of the following; each is a planned obligation for a
later slice, and each must keep model proof, implementation test,
correspondence and assumption separate.

| ID | Obligation | Suggested route |
|---|---|---|
| `LPL-O1` | the identity function over the immutable subject fields is well defined and injective on the record shape, and is insensitive to evidence and status changes | Lean model of the subject canonicalization plus an encoder-refinement obligation |
| `LPL-O2` | reload determinism: recomputation from persisted bytes equals the pre-restart values | Lean statement over an abstract canonical encoder, with an executed Rust differential lane |
| `LPL-O3` | deduplication preserves the identity and the dependency closure, never overwrites or deletes an evidence record, and appends evidence only in a new generation | Lean invariant over the dedup relation; Rust fixture as the executed lane |
| `LPL-O4` | rollback restores exactly the retained closure | Lean invariant over the generation relation; Rust fixture for the refusal cases |
| `LPL-O5` | retrieval never returns authority and admission is always re-entered | executed Rust property over generated queries; no theorem claimed |
| `LPL-O6` | the export carries every tested/proved distinction and no capability | schema-level executed check over generated libraries |

VC-LIB-01..04 supply the rule-library route for `LPL-O1`–`LPL-O4`; EV-BIND-01
and VC-TOOL-04 supply the evidence-binding route for their receipts. No Lean
declaration, receipt or conformance case is added by this change.

## Non-claims

No implementation, compiled store, CLI surface or production path. No canonical
Noble encoding, hash algorithm, test vectors or cross-implementation content
hash. No behavioral equivalence of equal-signature records, no substitution of
an entry into an existing Program, no automatic merge, no deletion of retained
evidence. No claim that a promotion proves correctness: a promotion records
that a consumer policy's demanded obligations have cited evidence, and holding a
reference is not validity. No claim that library growth is lifelong learning,
that a library arm transfers without a preregistered paired comparison, or that
any transfer result is teacher-free when a teacher contributed. No benchmark,
speedup, amortization or break-even claim. No progress on #73 Phase 2, #116,
#56 or #74 is implied.
