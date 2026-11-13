## ADDED Requirements

### Requirement: LPL-01
r[LPL-01]

The learned-procedure library MUST be the #73 typed-chunk library extended by
exactly one record layer, `procedure-record/1`. It MUST NOT define a second
body encoding, a second library snapshot, a second admission route or a second
store, and the `.cairn/changes/typed-chunk-completion/` profile
`typed-chunk/1` / `pure-i64-bool/1` and its `library_revision` request field
remain authoritative for chunks.

A procedure record MUST retain: the fixed `format = 1` and semantic revision
`0`; an experimental `procedure_id`; an identity record naming the encoder
identifier, the encoding revision and an explicit experimental flag; the
unmodified `typed-chunk/1` chunk envelope, including its canonical body,
captures, resolved `DefinitionRef` dependencies, ordered instantiation
witnesses, exact ordered `I64`/`Bool` stacks, empty effect set, `chunk_id` and
`source_revision`; producer provenance recording whether the body came from a
chunk, from synthesis or from composition, together with the producer's tool
revision and an optional task identifier; an ordered acyclic list of
library-level dependency edges; per-obligation evidence records, each carrying
one of the existing classes `lean`, `aeneas-lean`, `verus`, `validation`,
`test`, `review` or `assumption`, its own outcome, its toolchain identity, and
its binding to the exact subject and policy revision; a promotion state; an
integer cost profile; a retrieval signature derived only from retained fields;
and an optional supersession or alias reference.

Identity MUST be recomputed from the immutable subject fields — the chunk
envelope (body, captures, resolved dependencies, instantiations, ordered
interface, empty effect set, chunk identity and source revision), the semantic
revision, the encoder identifier and the producer provenance — and compared
with the retained value. Attaching, replacing or erasing an evidence record,
changing a promotion state, a cost profile, a retrieval signature, a
supersession or an alias MUST NOT change `procedure_id` (VC-ID-01,
VC-VALUE-05); those are mutable library state bound to that identity inside a
generation. A supplied digest, `procedure_id`, `chunk_id`, signature, name or
file path MUST NOT be acceptance authority, MUST remain labelled experimental
and library-local, and MUST NOT be presented as a cross-implementation stable
content hash (P-ID-04, VC-ID-03). Every record and every transitive chunk
dependency MUST still be checked by ordinary source preparation and kernel
acceptance, and its effect and capability closure MUST be empty.

A record MUST retain at most 16 library-level dependency edges and at most 16
evidence records, and a library MUST retain at most 64 records inside the
TCHUNK-03 retained-byte maximum. A host MAY lower these bounds but MUST NOT
raise them; exceeding one MUST report `exhausted` rather than truncate, drop or
partially retain.

#### Scenario: the record extends one store rather than replacing it

- GIVEN an existing #73 chunk library snapshot containing admitted chunk
  envelopes
- WHEN a procedure record is admitted
- THEN it MUST retain the chunk envelope by exact identity without re-encoding
  the body, the resulting snapshot MUST still satisfy TCHUNK-01, and the chunk
  view MUST expose exactly the #73 canonical order
  `(stack_in, stack_out, effects, chunk_id)`

#### Scenario: a forged identity is not authority

- GIVEN a record whose claimed digest, `procedure_id`, interface, dependency
  list, capture list or provenance differs from its recomputed identity
- WHEN it is admitted, reloaded or retrieved
- THEN recomputation MUST refuse it at the first differing field, MUST NOT
  retain it, and MUST NOT return it as a retrieval candidate; a forged
  evidence class or outcome is refused separately as `forged-status`

#### Scenario: the experimental label survives every representation

- GIVEN any retained record or exported manifest
- WHEN it is written or inspected
- THEN the encoder identifier, encoding revision and experimental flag MUST be
  present, no cross-implementation stability MAY be claimed, and a digest MUST
  be usable only as an index or integrity check under that encoder

### Requirement: LPL-02
r[LPL-02]

The library MUST admit a composition only as a fresh candidate. The composed
source MUST be reparsed by ordinary source preparation, admitted by the kernel
acceptance checker, and certified through the finite MC2 companion rule set
using the composition rule with both operand evidence entries as premises and
the joint endpoint interface as the prechecked intermediate implication. An
absent intermediate implication MUST produce `unresolved-implication` and MUST
NOT produce a certified result. Ordinary program composition remains available
when ordinary interfaces match, and it MUST NOT be reported as certified.

Each entry MUST declare an authority set that is a subset of the consumer's
selected policy allowance. The authority set of a composed or instantiated
result MUST be a subset of the union of its operands' authority sets under that
same policy. Any authority, host contract, effect, evidence class or policy
revision required by a result but absent from its operands' evidence MUST be a
capability escalation and MUST be refused with the corresponding named refusal:
`unsupported`, `live-capability-in-evidence`, `unsupported-evidence-class`,
`mismatched-context` or `stale-context`. A record MUST NOT carry a live
capability, resource reference, service handle, session, reflection handle or
dynamic lookup. A signature match MUST NOT mint a companion entry, transfer
evidence or authorize substitution into an existing Program, and restating an
existing derivation MUST refuse `duplicate-entry`. The library MUST NOT raise
its own policy revision and MUST NOT accept any record field as policy.

#### Scenario: escalation is refused at the exact field

- GIVEN a record or a composed result that widens its effect set, adds a live
  capability or service handle, requires an evidence class the consumer policy
  disallows, claims an instantiation the independent recheck refuses, or
  depends on a later, foreign or cyclic entry
- WHEN it is admitted or composed
- THEN it MUST be refused while naming that field and refusal class, the
  retained library and every existing Program MUST remain unchanged, and no
  partial entry or certified result MAY be published

#### Scenario: a policy change invalidates rather than upgrades

- GIVEN evidence admitted under one consumer policy revision and a consumer
  that selects a different revision to permit a wider class
- WHEN the old evidence is reused without revalidation
- THEN it MUST report `stale-context`, and a widened allowance MUST require
  fresh independent observations rather than a record field or a digest

#### Scenario: composition without an intermediate implication is uncertified

- GIVEN two admitted records whose ordered interfaces do not join at the
  composition boundary
- WHEN certified composition is attempted
- THEN it MUST refuse `unresolved-implication`, and the plain composed program
  MAY remain available only with its ordinary interface and without a
  certified claim

### Requirement: LPL-03
r[LPL-03]

A promotion state MUST be exactly one of `candidate`, `tested`, `promoted` or
`superseded`. `tested` MUST require at least one `test`-class evidence record
with an execution outcome bound to the exact subject and the selected fixture
set, and it MUST NOT imply any proof. `promoted` MUST require that the
consumer-selected promotion policy names every obligation it demands, and each
demanded obligation MUST cite an evidence record of an allowed class with its
own outcome, exact subject, toolchain identity, command and input binding, and
policy revision. A `test`, `review` or `assumption` record MUST NOT establish a
`proved` outcome; a record asserting `proved` without a matching proof-class
record MUST be refused as `forged-status`, and an `assumption` record MUST
remain visible and MUST NOT be counted as proved.

A promotion MUST NOT transfer to a different body, capture list,
instantiation, dependency set, source revision, semantic revision, toolchain or
policy revision. Demotion or invalidation MUST be recorded as a new record
state or a new library generation; a retained record MUST NOT be edited, and
deleting a cited evidence reference MUST invalidate the promotion rather than
silently preserve it. Inspection of a promoted record MUST NOT start a prover,
fetch a proof file or acquire authority implicitly.

#### Scenario: tested and proved stay separately visible

- GIVEN two records with the same interface and authority, one carrying only
  passing `test` evidence and one carrying an accepted proof-class record for
  its exact obligation
- WHEN both are exported or retrieved
- THEN the test-only record MUST report `tested` rather than `promoted` under a
  policy that demands proof, both evidence classes MUST remain visible, and no
  export MAY collapse them into one status

#### Scenario: a forged promotion is refused

- GIVEN a record that sets `promoted` while its evidence records are a test
  outcome, a review note or an assumption, or whose toolchain identity is
  unavailable
- WHEN the promotion is admitted or verified
- THEN it MUST be refused as `forged-status` or `stale-context` for the exact
  obligation, and the previous state MUST remain authoritative

#### Scenario: promotion does not travel with a name

- GIVEN a promoted record and a new record with a changed body, capture or
  dependency but the same interface and name
- WHEN the new record is admitted
- THEN it MUST start at `candidate` and MUST NOT inherit any evidence,
  promotion or certification from the name, interface or template identity

### Requirement: LPL-04
r[LPL-04]

Two admitted records MUST share one `procedure_id` exactly when their immutable
subject fields — body, captures, ordered interface, effect set, dependency list
and order, instantiations, source and semantic revisions, toolchain identity
and producer provenance — are equal under the same encoder and encoding
revision. Equality MUST NOT be inferred from a name, interface, cost, template
identity, file location or nearest-body similarity. Re-admitting an equal
subject with an identical evidence set, promotion state and cost MUST return
the existing identity without adding an entry, minting evidence or changing
state. Re-admitting an equal subject with different or additional evidence MUST
retain the same identity, append evidence records without overwriting or
deleting a retained one, and publish the change only in a new generation.

Records with identical ordered interfaces and different bodies MUST remain
distinct, and a signature match MUST NOT transfer behavioral evidence.
Semantic deduplication MUST require an equivalence certificate produced by
#56's certified-equivalence lane for the exact pair. The resulting alias
relation MUST retain both identities and both complete dependency closures,
MUST record the certificate as its own evidence record naming its class, scope,
checker identity, toolchain and outcome, and MUST NOT substitute the alias into
an existing Program, delete or rewrite a record, or change the retrieval order
of either identity.

#### Scenario: exact duplicates reuse one identity

- GIVEN two admissions of byte-identical records under the same encoder
- WHEN the second admission completes
- THEN it MUST return the existing identity, the entry count MUST stay
  unchanged, and no evidence, promotion or generation MAY be minted

#### Scenario: attaching evidence is not a new artifact

- GIVEN an admitted record and a later admission of the same subject with one
  additional evidence record
- WHEN the later admission completes
- THEN the `procedure_id` MUST stay unchanged, the earlier evidence record MUST
  remain present and unmodified, and the new evidence MUST appear only in a new
  generation

#### Scenario: equal signatures stay distinct

- GIVEN two records with identical ordered stacks, empty effects and different
  bodies
- WHEN the library indexes and retrieves them
- THEN both identities MUST remain retrievable and distinct, the dependency
  closures MUST remain separate, and no behavioral equivalence MAY be claimed

#### Scenario: semantic deduplication needs a certificate for the exact pair

- GIVEN two distinct records and no certificate for that pair
- WHEN semantic deduplication is attempted
- THEN it MUST be refused; with a certificate for the exact pair it MUST add
  only an alias relation and MUST preserve both identities, both closures and
  the certificate's own evidence binding

### Requirement: LPL-05
r[LPL-05]

Retrieval MUST be a pure function of one library generation. Candidates MUST be
ordered by the total order: ordered input stack type tags, ordered output stack
type tags, effect set, promotion-state rank `promoted < tested < candidate <
superseded`, the integer cost key `(expanded_nodes, emitted_source_bytes,
evaluation_steps)` with an absent key ordered last, and finally the
`procedure_id` bytes. The order MUST NOT depend on hash-container iteration,
directory enumeration, insertion history, wall-clock time, process or host
identity, locale, thread scheduling or floating point.

A query MAY filter by exact interface, by composition-compatible interfaces for
a one-hole context, by dependency closure and by promotion state; every filter
MUST be applied after the canonical order. Retrieval MUST return candidate
identities plus the examined-entry count and MUST NOT return a completed
program, an admission decision, an applicability guarantee, evidence validity
or a capability. Index construction and per-query examination MUST be bounded
and counted, and every returned candidate MUST still pass ordinary admission
and interface checking for the actual composition.

The index MUST be bounded at 64 entries and 65,536 bytes, with at most 128
examined entries per query, 32 queries and 4,096 retrieval work units per
request. A host MAY lower but MUST NOT raise these values; over-budget MUST
report `exhausted` rather than return a truncated or unordered result.

#### Scenario: the same generation retrieves the same order everywhere

- GIVEN one library generation retrieved twice, in separate processes and from
  differently ordered storage
- WHEN identical queries run
- THEN the candidate identity sequences and examined-entry counts MUST be equal

#### Scenario: retrieval is not authority

- GIVEN a retrieved candidate that was admitted for one interface and evidence
  set
- WHEN it is offered into a composition whose interface, capture or effect
  requirement differs
- THEN the composition MUST recheck and refuse as ordinary admission dictates,
  and the retrieval result itself MUST NOT have authorized anything

#### Scenario: filters do not change the canonical order

- GIVEN the same query with and without a promotion-state filter
- WHEN the filtered result is produced
- THEN it MUST be an ordered subsequence of the unfiltered result for that
  generation

### Requirement: LPL-06
r[LPL-06]

The library MUST retain generations append-only. Each publication MUST record a
monotonic generation number, the recomputed library revision, the parent
generation, the kind `publish` or `rollback`, and the exact record closure.
The closure MUST bind each record's identity together with the digest of that
generation's mutable state (evidence set, promotion state and cost), so a
restored generation restores state without changing any identity. Replacement
MUST add a new record carrying a `supersedes` reference and MUST keep the
replaced record addressable by its exact identity.

A rollback MUST republish a retained generation's exact closure as a new
generation whose parent is the restored generation. It MUST refuse `invalid`
when any record's recomputed identity, state digest, dependency closure,
encoder revision or evidence toolchain identity differs from the retained value
or is unavailable.
A rollback MUST NOT mutate an existing Program, a retained record, a retained
generation, an exported manifest or a consumer's retained evidence. A retained
generation referenced by an export or by a consumer's selected closure MUST NOT
be deleted, and a rollback MUST be visible in the generation record rather than
presented as an ordinary publish. Retention MUST be bounded at 16 generations;
a publication or rollback that would exceed the bound MUST refuse `exhausted`
rather than delete a referenced generation.

#### Scenario: rollback restores exactly the retained closure

- GIVEN three published generations and a request to restore the first
- WHEN the rollback completes
- THEN a new generation MUST name the first generation as its parent and carry
  exactly that closure with recomputed identities equal to the retained values,
  and the abandoned generations MUST remain retained and addressable

#### Scenario: a drifted closure refuses rollback

- GIVEN a retained generation whose encoder revision, toolchain identity or a
  record's recomputed digest no longer matches
- WHEN rollback is requested
- THEN it MUST refuse `invalid` at the first differing field and the currently
  selected generation MUST remain in effect

#### Scenario: supersession is additive

- GIVEN a record replaced by a newer record with a `supersedes` reference
- WHEN retrieval or composition runs against the same generation
- THEN the older record MUST remain addressable by its exact identity, its
  dependency edges MUST remain resolvable, and no consumer's existing evidence
  MAY be invalidated by the supersession alone

### Requirement: LPL-07
r[LPL-07]

The persisted form MUST be the canonical records plus generation metadata and
the encoder, profile and semantic identifiers. Load MUST recompute each
`procedure_id`, identity digest, evidence set and promotion state against the
generation's state digest, ordered dependency list, library revision and
retrieval order from the persisted canonical fields, and MUST produce values
identical to those observed before restart for the same encoder and toolchain.
A mismatch MUST be `invalid`, MUST name the first differing field, and MUST NOT
publish a partial or repaired library.

The canonical form MUST NOT contain or depend on wall-clock or monotonic time,
absolute or relative path spelling, process or host identity, hash-container
iteration order, thread scheduling, locale or floating-point values. Collections
MUST be serialized in their declared canonical order: dependencies in retained
order, evidence by obligation identifier then class rank, entries by the
retrieval total order. Reload determinism is a guarantee within one encoder
version and toolchain; it MUST NOT be reported as a cross-implementation or
cross-encoding stable content hash (P-ID-04).

#### Scenario: two processes produce one identity manifest

- GIVEN one library directory written by an earlier process
- WHEN two independent later processes load it and export the identity manifest
- THEN the manifests MUST be byte-identical, including every identity digest,
  ordered dependency list, library revision and retrieval order

#### Scenario: a changed byte is refused, not absorbed

- GIVEN the same library with one byte changed in a retained body or dependency
  list
- WHEN it is loaded
- THEN the recomputed identity MUST differ from the retained identity, load
  MUST refuse `invalid` naming the first differing field, and no record MAY be
  silently repaired, dropped or reordered

#### Scenario: nondeterministic inputs are absent from the recorded form

- GIVEN any retained record, generation record or exported identity manifest
- WHEN it is inspected
- THEN it MUST contain no time, path, process, host or floating-point field,
  and two loads on one machine with different directory spelling and process
  identifiers MUST agree

### Requirement: LPL-08
r[LPL-08]

The library MUST export two read-only records. `noble-procedure-library-export/1`
MUST be a deterministic identity manifest containing the schema and schema
revision, the profile, the semantic revision, the encoder identifier and
revision, the generation closure, each entry's canonical identity fields,
ordered dependencies, promotion state, evidence classes and outcomes, integer
cost key, supersession and alias references, and the canonical retrieval order.
It MUST be byte-comparable across restarts and MUST contain no time, path,
process, host or floating-point field.

`noble-procedure-library-metrics/1` MUST carry measured counters bound to that
manifest's digest: index build work and monotonic elapsed time, per-query
examined entries, entry and byte growth, acquisition and retention events per
task family, transfer outcomes on held-out composed tasks, regression outcomes
for earlier tasks, restart-reuse identity match, and the number of
teacher/proposer calls. An evaluation that reports transfer MUST execute with
the proposer and teacher disabled; a nonzero teacher or proposer call count
MUST invalidate a transfer claim, and an attempted teacher call during such an
evaluation MUST fail closed instead of substituting silently. Growth, retrieval
overhead and transfer MUST be reported as separate measurements; library growth
alone MUST NOT be reported as demonstrated lifelong learning.

The export MUST NOT carry training policy, a host capability, captured secrets,
a live handle, or a promoted claim that its evidence records do not bind. It
MUST keep model/synthesis behavior, executed library mechanics, artifact
correspondence and explicit assumptions in separate fields, and a test outcome
MUST remain a test outcome after export. The identity manifest MUST be bounded
at 262,144 bytes and the metrics record at 65,536 bytes; an export exceeding a
bound MUST be refused rather than truncated.

#### Scenario: the identity manifest is reproducible, the metrics are not part of it

- GIVEN one library generation and two restarts
- WHEN the export is produced
- THEN the identity manifest MUST be byte-identical, while measured counters
  MUST appear only in the metrics record and MUST be tied to the manifest
  digest rather than to a timestamp or path

#### Scenario: teacher fallback invalidates a transfer claim

- GIVEN a held-out evaluation run in which a proposer or teacher was called
  once, or in which a call was attempted after the harness disabled it
- WHEN the export reports the run
- THEN the transfer claim MUST be withdrawn or marked invalid, the attempted
  call MUST have failed closed, and the library growth MUST still be reported
  without the transfer claim

#### Scenario: no capability or training policy escapes

- GIVEN a library containing promoted and tested records
- WHEN the export is written
- THEN it MUST contain no capability, handle, secret, teacher configuration or
  training policy, and MUST keep tested and proved evidence visibly distinct
