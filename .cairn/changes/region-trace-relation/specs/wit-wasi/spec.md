## ADDED Requirements

### Requirement: WIT-TRACE-01
r[WIT-TRACE-01]

A prefix-safe relation MUST define a source event alphabet and a target event
alphabet for the existing synchronous bounded-region subject, and MUST relate
them explicitly. The source alphabet MUST be `RegionProtocol-v1`'s request,
response and commit classes. The target alphabet MUST cover the request
operation and its arguments, the exact resource identity, the response value or
error, the protected commit, the return, the abnormal-exit class and the
host-cleanup commit. The relation MUST be defined before any property is
derived, and a property that is not derived from it MUST NOT be claimed to
transfer.

#### Scenario: relation before property

- GIVEN a proposed preservation claim over source and target traces
- WHEN the claim is registered
- THEN it MUST name the relation it is derived from, and an underived claim MUST
  be refused rather than recorded as proved

#### Scenario: exact identity is related

- GIVEN a target event that carries a resource identity
- WHEN the relation is applied
- THEN the identity MUST be related to the source event's exact resource identity
  rather than to a rep number or handle class

### Requirement: WIT-TRACE-02
r[WIT-TRACE-02]

Stuttering or erasure MUST be permitted only for identified internal ABI
bookkeeping steps, and each erased class MUST carry its own justification. The
relation MUST preserve observable ordering, response-to-request pairing, exact
resource identity and every protected commit. A denied request MUST NOT be
erased, and host retirement MUST NOT be converted into a guest release.

#### Scenario: an erased denial is rejected

- GIVEN a relation variant that permits erasing a denied request
- WHEN the variant is checked
- THEN it MUST be rejected, because erasure would remove the evidence that the
  request did not take effect

#### Scenario: retirement is not a release

- GIVEN a target trace whose abnormal exit is followed by host retirement
- WHEN the relation is applied
- THEN the retirement MUST remain a host commit and MUST NOT be related to a
  guest release event

### Requirement: WIT-TRACE-03
r[WIT-TRACE-03]

The preservation corollaries required by `RegionProtocol-v1` MUST be derived
from the relation under explicitly declared host, identity, cost and cleanup
premises: permitted ordered prefixes, no forged identity, no unexplained read
commit, and at-most-once release and retirement. A normal-return postcondition
MUST be conditional on normal return. Prefix safety MUST NOT imply eventual
return or successful cleanup.

#### Scenario: prefix safety is not liveness

- GIVEN an execution that commits a permitted prefix and then traps
- WHEN the corollaries are reported
- THEN prefix safety MAY hold while termination and cleanup stay unknown, and no
  liveness claim MUST be derived

#### Scenario: unexplained commit

- GIVEN a target trace containing a committed region read with no related
  request
- WHEN the relation is applied
- THEN the corollary MUST fail rather than treating the commit as internal

### Requirement: WIT-TRACE-04
r[WIT-TRACE-04]

Quota and fuel boundaries MUST be modelled through an explicit cost or
accounting relation, or through an abstraction justified for the covered
subject. Additional target ABI steps MAY exhaust fuel before a source step
completes, and the committed prefix and the precise termination and cleanup
status MUST be preserved across that boundary. An unrestricted clause that the
target may trap anywhere MUST NOT be used to excuse arbitrary wrong code.

#### Scenario: fuel boundary preserves the prefix

- GIVEN a target trace that exhausts fuel after several committed steps
- WHEN the relation is applied
- THEN the committed prefix MUST be preserved and the termination status MUST be
  reported precisely rather than as a generic trap

#### Scenario: free trap clause is refused

- GIVEN a proposed relation that permits unmodelled traps at any step
- WHEN the variant is checked
- THEN it MUST be refused, because it admits arbitrary dropping or invention of
  commits

### Requirement: WIT-TRACE-05
r[WIT-TRACE-05]

A finite observable pilot MUST be chosen at the existing WIT interface, with the
instrumentation needed to distinguish trap, quota exhaustion and cleanup failure
documented. An absent observation MUST remain unknown, and MUST NOT be
synthesized into a successful protocol outcome. Unsupported cancellation MUST
remain unsupported until a real control exists.

#### Scenario: absent observation stays unknown

- GIVEN a run whose report collapses trap, quota and cleanup failure into one
  outcome label
- WHEN the pilot classifies the run
- THEN the affected classes MUST be reported unknown rather than assigned

#### Scenario: no invented cancellation lane

- GIVEN no real cancellation control at the interface
- WHEN fixtures are proposed
- THEN cancellation MUST be recorded unsupported rather than modelled from an
  absent observation

### Requirement: WIT-TRACE-06
r[WIT-TRACE-06]

The target model MUST be connected to the actual lowering and runtime steps
through a named correspondence route with pinned source and tool revisions. A
theorem relating two handwritten machines MUST remain a model theorem, and
Wasmtime and Canonical-ABI assumptions MUST stay visible until they are
established separately.

#### Scenario: model theorem is labelled

- GIVEN a proved relation theorem for the target model
- WHEN it is reported
- THEN it MUST be labelled a model theorem and MUST NOT be cited as an emitted
  step correspondence

#### Scenario: toolchain binding

- GIVEN a retained pilot receipt
- WHEN the evidence is replayed
- THEN the exact source, component, helper, engine and model revisions MUST be
  bound, and a stale binding MUST fail the replay
