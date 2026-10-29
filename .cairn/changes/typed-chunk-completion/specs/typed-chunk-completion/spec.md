## ADDED Requirements

### Requirement: TCHUNK-01
r[TCHUNK-01]

The initial chunk profile MUST be the immutable, versioned
`typed-chunk/1` / `pure-i64-bool/1` envelope. It MUST retain `format = 1`,
semantic revision `0`, a 16-byte chunk identity, a 32-byte source revision,
exact ordered `I64`/`Bool` input and output stacks, an empty effect set, zero
through eight immutable scalar captures, resolved definition/dependency identities, ordered
instantiation witnesses, and a finite canonical recipe. The recipe MUST be
acyclic, with only I64/Bool literals, pure ordered-stack routing, the published
I64/Bool arithmetic/Boolean operations and bounded quotations. Text, Unit,
Syntax, resources, live references, reflection, dynamic calls, recursion,
`SignedSelf` and `SlotInvoke` MUST be unsupported. A digest or index MUST NOT
be acceptance authority. Every chunk and transitive dependency MUST be checked
by ordinary source preparation and kernel acceptance.

#### Scenario: exact envelope is admitted only as a pure recipe

- GIVEN an envelope with the fixed profile, exact ordered stacks, zero through
  eight immutable I64/Bool captures, an acyclic dependency list and an
  `EffSet::empty()` closure
- WHEN the library admits it
- THEN it MUST retain the body, dependencies and instantiations and MUST
  independently derive the interface rather than trust the supplied metadata

#### Scenario: unsupported capability is not a chunk

- GIVEN a recipe containing a live reference, unresolved dynamic call, resource,
  reflection operation, Text/Unit value, `SignedSelf` or `SlotInvoke`
- WHEN the library admits it
- THEN it MUST return `Unsupported` at the offending recipe field and MUST NOT
  retain the entry

#### Scenario: equal signatures do not merge behavior

- GIVEN two admitted chunks with identical ordered interfaces and empty effects
  but different bodies
- WHEN the library indexes them
- THEN both identities MUST remain distinct and a signature match MUST NOT
  transfer behavioral evidence or authorize substitution

### Requirement: TCHUNK-02
r[TCHUNK-02]

The completion API MUST accept a host-supplied request containing the fixed
profile, an independently supplied expected ordered stack/effect contract, one
editor-format-1 tree with exactly one hole and its bounded path, a pinned
32-byte library revision, an optional host-retained fixture identifier, and a
shared budget token. It MUST enumerate replacement sequences of zero through
four chunks in deterministic shortlex order using canonical library order
`(stack_in, stack_out, effects, chunk_id)`. For each proposal it MUST solve the
complete ordered stack equation, including the surrounding stack tail, emit a
fresh explicit source and kernel candidate, and reparse and check the full
result against the original request. `editor::HoleConstraint::required_output_suffix`
MUST NOT be treated as a promise that an incoming suffix is untouched. Search
MUST be restricted to the explicit editor hole, not ordinary `quote`, `compose`
or `run`.

#### Scenario: two chunks and a stack tail are composed deterministically

- GIVEN one editor hole, an incoming stack with a nonempty surrounding tail and
  two compatible fixed chunks
- WHEN enumeration runs
- THEN it MUST try the shortlex sequence in canonical order, require exact
  intermediate ordered-stack equality, preserve only what the computed equation
  proves, and independently check the final source/candidate

#### Scenario: a nested quotation is bounded rather than guessed

- GIVEN a hole inside an editor quotation and a replacement whose quotation
  depth remains at most 8
- WHEN the proposal is emitted
- THEN the quotation MUST be reparsed and checked as a normal completed source
  tree; a replacement exceeding depth 8 MUST be exhausted or unsupported, never
  silently flattened

#### Scenario: no-result is not a completeness theorem

- GIVEN a finite search in which every enumerated proposal is refused before a
  budget is exhausted
- WHEN the API returns
- THEN it MUST return `NoResult` with counts and MUST NOT claim that no valid
  completion exists

### Requirement: TCHUNK-03
r[TCHUNK-03]

The profile MUST enforce one shared fail-closed budget with maxima of 262,144
retained library bytes, 64 library entries, 64 recipe nodes, stack height 32,
256 dependency steps, 64 instantiation bindings, quotation depth 8, 256
expanded nodes, four composed chunks, 4,096 emitted source bytes, 128
candidates, 64 diagnostics, 4,096 checking-work units, 256 host fixture
execution steps and 262,144 request/retained bytes. A host MAY lower but MUST
NOT raise these values. Every attempted candidate and every expanded node MUST
be charged; a chunk boundary MUST NOT reset any counter. The API MUST preserve
four distinct refusal classes: `Invalid`, `Unsupported`, `Exhausted` and
`NoResult`. Invalid, unsupported and exhausted responses MUST identify the
first exact envelope/editor location and observed/limit values. Exhaustion MUST
never truncate or partially publish a proposal, and no-result MUST never be
reported as exhaustion.

#### Scenario: each budget boundary has its own exhaustion

- GIVEN a request whose retained library, dependency walk, expansion, source,
  candidate, diagnostic, checking or fixture execution reaches its fixed limit
- WHEN the next bounded step is charged
- THEN the API MUST return `Exhausted` naming that resource and MUST publish no
  partial completion or mutate the existing Program

#### Scenario: malformed and unsupported remain distinct

- GIVEN a missing envelope field, malformed node reference, unsupported opcode,
  effect widening, stale definition revision or dynamic/live construct
- WHEN the proposal is classified
- THEN malformed structure or stale metadata MUST be `Invalid`, an out-of-profile
  construct MUST be `Unsupported`, and neither MUST be relabeled `NoResult`

#### Scenario: an exhausted search is not no-result

- GIVEN the 128th candidate has been charged and a 129th candidate would be
  required
- WHEN enumeration would continue
- THEN the API MUST return `Exhausted(candidates)` rather than `NoResult`, with
  the report retaining the 128 attempted-candidate count

### Requirement: TCHUNK-04
r[TCHUNK-04]

The implementation plan MUST retain independent fixtures for primitive and
fixed-chunk completion, composition of at least two chunks, a supported nested
quotation, wrapping-I64 boundary inputs (`i64::MIN`, `-1`, `0`, `1`,
`i64::MAX`), subtraction operand order, and both Boolean results. It MUST also
exercise fresh instantiation, nonempty stack-tail constraints,
equal-signature/different-behavior chunks, changed body/capture/dependency,
stale source or semantic revisions, wrong ordered interfaces, effect widening,
forged metadata, unsupported constructs and malformed proposals. Expected
values and contracts MUST remain host-supplied and independent. Fixture
agreement MUST be reported separately from model proofs, checker obligations,
behavioral-equivalence claims and any performance result.

#### Scenario: independent fixtures cover values and composition

- GIVEN the primitive, fixed two-chunk, nested-quotation, wrapping-I64,
  operand-order and Boolean fixtures
- WHEN the implementation reports them
- THEN it MUST include the expected input/output values, ordered interfaces and
  separate fixture-agreement labels, with no claim that agreement is a proof

#### Scenario: mutation controls reject stale or forged chunks

- GIVEN a changed body, capture list, dependency, source/semantic revision,
  ordered interface, effect bound or claimed identity/digest
- WHEN the chunk is looked up or composed
- THEN revalidation MUST reject it before acceptance and MUST leave the retained
  library and existing Programs unchanged

#### Scenario: measurements expose hidden expansion

- GIVEN a completion attempt using reusable chunks
- WHEN the report is produced
- THEN it MUST include enumerated/checked candidates, expanded nodes,
  dependency and instantiation work, fixture execution steps, total monotonic
  latency and retained bytes; the report MUST NOT claim a speedup or universal
  correctness without separately retained evidence
